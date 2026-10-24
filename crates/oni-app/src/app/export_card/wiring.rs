//! The export surface's callbacks.

use super::*;

impl Ctl {
    /// The proof every render the editor shows goes through, and the profiles the field lists.
    pub(in crate::app) fn attach_export(&self) {
        let wanted = self.export.borrow().wanted.clone();
        if let Ok(mut engine) = self.engine.lock() {
            engine.proof.wanted = Some(wanted);
        }
        self.list_profiles();
        self.publish_export();
    }

    /// The stripes as the last session left them.
    pub(in crate::app) fn restore_gamut(self: &Rc<Self>) {
        if self.export.borrow().gamut && !self.state.borrow().screen_gamut {
            self.toggle_screen_gamut();
        }
    }

    /// The File menu's item, which counts the selection off the surface.
    pub(in crate::app) fn publish_export_menu(&self, selected: usize, has_photo: bool) {
        let Some(w) = self.window() else { return };
        let c = self.export.borrow();
        if c.shown {
            return;
        }
        let e = w.global::<Exporting>();
        e.set_menu_title(label::menu(selected).into());
        e.set_menu_live(has_photo || selected > 0);
    }

    pub(in crate::app) fn wire_export(me: &Rc<Ctl>, window: &MainWindow) {
        let e = window.global::<Exporting>();
        let p = me.clone();
        e.on_preview_room(move |w, h, floor| {
            let scale = p.window().map_or(1.0, |w| f64::from(w.window().scale_factor()));
            p.preview_room((f64::from(w) * scale, f64::from(h) * scale, f64::from(floor) * scale));
        });
        let p = me.clone();
        e.on_preset(move |i| {
            let Some(preset) = Preset::ALL.get(i as usize).copied() else { return };
            let before = p.export.borrow().settings.space().clone();
            {
                let mut c = p.export.borrow_mut();
                c.settings = preset.applied(&c.settings);
            }
            if *p.export.borrow().settings.space() != before {
                p.space_changed();
            } else {
                p.export_changed(true);
            }
        });
        let p = me.clone();
        e.on_format_picked(move |i| {
            let Some(format) = Format::ALL.get(i as usize).copied() else { return };
            let before = p.export.borrow().settings.space().clone();
            p.export.borrow_mut().settings.set_format(format);
            if *p.export.borrow().settings.space() != before {
                p.space_changed();
            } else {
                p.export_changed(true);
            }
        });
        let p = me.clone();
        e.on_space_picked(move |i| p.space_tab(i.max(0) as usize));
        let p = me.clone();
        e.on_profile_picked(move |i| {
            let chosen = {
                let c = p.export.borrow();
                let Space::Profile(current) = c.settings.space() else { return };
                c.profile_titles(current).0.get(i.max(0) as usize).cloned()
            };
            if let Some(profile) = chosen {
                p.choose_profile(profile);
            }
        });
        let p = me.clone();
        e.on_pending_picked(move |i| {
            if i == 1 {
                p.load_profile();
            }
        });
        let p = me.clone();
        e.on_load_profile(move || p.load_profile());
        let p = me.clone();
        Ctl::wire_papers(&p, &e);
        let p = me.clone();
        e.on_colour_only_set(move |on| p.set_colour_only(on));
        let p = me.clone();
        e.on_compress_set(move |on| {
            p.export.borrow_mut().settings.compress_tiff = on;
            p.export_changed(true);
        });
        let p = me.clone();
        e.on_hdr_set(move |on| {
            p.export.borrow_mut().settings.hdr = on;
            p.export_changed(true);
        });
        let p = me.clone();
        e.on_quality_press(move |_fine| {
            let now = Instant::now();
            let double = {
                let mut c = p.export.borrow_mut();
                let double = c.quality.last_press.is_some_and(|t| now.duration_since(t) <= DOUBLE_CLICK);
                c.quality.last_press = Some(now);
                c.quality.start = if double { None } else { Some(Ctl::quality_fraction(c.settings.jpeg_quality)) };
                double
            };
            if double {
                p.set_quality(QUALITY_BASE, true);
            }
            double
        });
        let p = me.clone();
        e.on_quality_drag(move |translation, span, fine| {
            let Some(start) = p.export.borrow().quality.start else { return };
            let at = track::clamped(track::asked(start, translation, span, fine));
            p.set_quality(QUALITY.0 + f64::from(at) * (QUALITY.1 - QUALITY.0), false);
        });
        let p = me.clone();
        e.on_quality_release(move || {
            p.export.borrow_mut().quality.start = None;
            p.weigh_export();
        });
        let p = me.clone();
        e.on_quality_scroll(move |dx, dy, span, fine| {
            let from = Ctl::quality_fraction(p.export.borrow().settings.jpeg_quality);
            let Some(at) = p.slider_scrolled(from, dx, dy, span, fine, |c| c.weigh_export()) else { return false };
            p.set_quality(QUALITY.0 + f64::from(at) * (QUALITY.1 - QUALITY.0), false);
            true
        });
        let p = me.clone();
        e.on_quality_step(move |d| {
            let q = p.export.borrow().settings.jpeg_quality + f64::from(d) * (QUALITY.1 - QUALITY.0) / 100.0;
            p.set_quality(q, true);
        });
        let p = me.clone();
        e.on_size_typed(move |width, height| {
            let resize = {
                let c = p.export.borrow();
                let Some(full) = c.full() else { return };
                let (sw, sh) = export::size_texts(Some(full), c.settings.resize);
                export::typed_size(&width, &height, (&sw, &sh), full)
            };
            if let Some(resize) = resize {
                p.export.borrow_mut().settings.resize = resize;
            }
            p.export_changed(resize.is_some());
        });
        let p = me.clone();
        e.on_full_size(move || {
            p.export.borrow_mut().settings.resize = Resize::Full;
            p.export_changed(true);
        });
        let p = me.clone();
        e.on_edited(move |row, field, text| {
            let mut c = p.export.borrow_mut();
            let Some(Some(r)) = c.rows.get_mut(row.max(0) as usize) else { return };
            let text = text.to_string();
            match field.as_str() {
                "name" => r.name = text,
                "camera" => r.metadata.camera = text,
                "lens" => r.metadata.lens = text,
                "iso" => r.metadata.iso = text,
                "date" => r.metadata.date = text,
                "artist" => r.metadata.artist = text,
                "copyright" => r.metadata.copyright = text,
                _ => {}
            }
        });
        let p = me.clone();
        e.on_pick_film(move |row| {
            let current = {
                let mut c = p.export.borrow_mut();
                c.film_row = Some(row.max(0) as usize);
                let film = c.rows.get(row.max(0) as usize).cloned().flatten().map(|r| r.metadata.film).unwrap_or_default();
                FILM_STOCKS.iter().position(|k| k.display_name == film).map_or(-1, |i| i as i32)
            };
            // This frame alone: the stock is the frame's, not the roll's.
            let id = usize::try_from(current).ok().map(|i| FILM_STOCKS[i].id);
            p.open_stock_picker(crate::app::StockTarget::Export, id);
        });
        let p = me.clone();
        e.on_stock_chosen(move |i| {
            let row = p.export.borrow_mut().film_row.take();
            if let (Some(row), Some(kind)) = (row, usize::try_from(i).ok().and_then(|i| FILM_STOCKS.get(i)))
                && let Some(Some(r)) = p.export.borrow_mut().rows.get_mut(row)
            {
                r.metadata.film = kind.display_name.to_owned();
            }
            if let Some(row) = row {
                p.publish_line(row);
            }
            p.publish_export();
        });
        let p = me.clone();
        e.on_cancel(move || p.cancel_export());
        let p = me.clone();
        e.on_export(move || p.run_export());
        let p = me.clone();
        e.on_confirmed(move |replace| p.confirmed(replace));
        // `PaperCorrection.automatic`: the paper's own white, read off its profile alone.
        let p = me.clone();
        e.on_auto_paper(move || {
            let Some(paper) = p.export.borrow().settings.space().paper().cloned() else { return };
            let Some(found) = oni_colour::paper::automatic(&paper) else { return };
            let next = {
                let s = p.state.borrow();
                let mut next = s.store.settings().clone();
                next.paper_correction = found;
                next
            };
            p.write(oni_core::Command::Write(Box::new(next)));
        });
        let p = me.clone();
        e.on_notice_closed(move || {
            p.export.borrow_mut().notice = None;
            p.publish_export();
        });
    }
}
