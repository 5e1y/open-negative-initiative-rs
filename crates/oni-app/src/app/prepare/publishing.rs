//! What the surface shows, from the roll's state.

use super::*;

impl Ctl {
    // MARK: - Publishing

    /// Every value the roll's panel, card and step keys show, read before any is set.
    pub(in crate::app) fn publish_prepare(&self) {
        let Some(w) = self.window() else { return };
        let p = w.global::<Preparing>();
        let shown = self.state.borrow().prepare.shown;
        p.set_shown(shown);
        if !shown {
            p.set_measuring(false);
            return;
        }
        let mut s = self.state.borrow_mut();
        Self::ask_thumbnails(&mut s);
        let radius = base_radius(&s);
        let masks = s.masks.clone();
        let (taking, picked, loading) = (s.masking.setting, s.prepare.flat_pick.is_some(), s.prepare.loading_mask);
        let settled = s.prepare.frames.iter().filter(|f| s.thumbs.contains(&f.fingerprint) || s.failed.contains(&f.fingerprint)).count();
        let frames = s.prepare.frames.clone();
        let mut pictures = Vec::with_capacity(frames.len());
        for f in &frames {
            let tile = s.prepare.tiles.get(&f.path).cloned();
            let picture = match tile {
                Some(t) => t,
                None => s.thumbs.get(&f.fingerprint).unwrap_or_default(),
            };
            pictures.push(picture);
        }
        let shown_frame = s.prepare.shown_frame().cloned();
        let card = shown_frame.as_ref().map(|f| match &s.prepare.card {
            Some((path, image)) if *path == f.path => image.clone(),
            _ => s.prepare.tiles.get(&f.path).cloned().unwrap_or_default(),
        });
        let card = match (card, &shown_frame) {
            (Some(c), Some(f)) if c.size().width == 0 => s.thumbs.get(&f.fingerprint).unwrap_or_default(),
            (c, _) => c.unwrap_or_default(),
        };
        let r = &s.prepare;
        let (_, apart) = apart(&frames);
        let ticked = r.ticked();
        let flagged = r.flagged();
        let measuring = r.progress;
        let m = measured(&frames);
        let last = m.len().saturating_sub(1);
        let step = r.step;
        let status = match measuring {
            Some((done, total)) => format!("Measuring the roll · {done} of {total}"),
            None if settled < frames.len() => format!("Loading the roll's thumbnails · {settled} of {}", frames.len()),
            None => String::new(),
        };
        let cells: Vec<RollCell> = frames
            .iter()
            .zip(pictures)
            .enumerate()
            .map(|(i, (f, picture))| RollCell {
                picture,
                marked: match step {
                    Step::FlatField => r.flat_pick.as_deref() == Some(f.path.as_str()),
                    Step::Base => r.base_frame == i,
                    _ => f.ticked,
                },
                flagged: f.flagged,
                apart: apart.contains(&f.path),
                ticked: f.ticked,
                rgb: f.passes.is_some(),
            })
            .collect();
        let options = crate::app::roll_tab::mask_options(&r.flat, &masks);
        let current = crate::app::roll_tab::chosen(&r.flat);
        let mask_title = options.iter().find(|(l, _)| *l == current).map_or("None selected".to_string(), |(_, t)| t.clone());
        let lines: Vec<MenuLine> = options.iter().map(|(_, t)| MenuLine { text: t.as_str().into(), enabled: true, rule: false, opens: false }).collect();
        let fractions: Vec<f32> = (0..5).map(|i| Self::slider_fraction(r, radius, i)).collect();
        let readouts: Vec<SharedString> = (0..5)
            .map(|i| match i {
                AMOUNT | TEMPERATURE | TINT => flat_field(i).spec().readout.format(Self::slider_value(r, radius, i)),
                PICKER => format!("{radius:.0} px"),
                _ => format!("{} / {}", r.at.min(last) + 1, m.len()),
            })
            .map(SharedString::from)
            .collect();
        let look = r.look();
        let base_note = r.base.as_ref().map_or("No base picked yet.".to_string(), |b| format!("On {}", oni_library::paths::name(&b.path)));
        let action = if step == Step::Window { measure_title(ticked - flagged) } else if filled(step, &r.flat, r.base.is_some()) { "NEXT".into() } else { "SKIP".into() };
        let heading = match measuring {
            Some((done, total)) => format!("Measuring {done} of {total}\u{2026}"),
            None => step.title().to_string(),
        };
        let passes_note = if r.capture == Capture::RgbPasses && r.grouping.is_none() {
            format!("{} frames, grouped when the roll was last measured.", frames.len())
        } else {
            String::new()
        };
        let skipped: Vec<bool> = Step::ALL.iter().map(|s| s.skipped(r.capture)).collect();
        let (film, capture, flat, left_out, at, has_base) = (r.film, r.capture, r.flat.clone(), r.left_out, r.at, r.base.is_some());
        let shown_path = shown_frame.as_ref().map(|f| f.path.clone());
        let show_card = shown_path != r.shown_path;
        let base_path = (step == Step::Base).then(|| r.base_shown().map(|f| f.path.clone())).flatten();
        let ask_scan = base_path.is_some() && base_path != r.asked_scan;
        if let Some(previews) = &r.previews {
            previews.update(look);
            if show_card {
                previews.show(shown_path.as_deref());
            }
            if ask_scan {
                previews.scan(base_path.as_deref());
            }
        }
        if show_card {
            s.prepare.shown_path = shown_path;
        }
        if ask_scan {
            s.prepare.asked_scan = base_path;
        }
        drop(s);
        p.set_measuring(measuring.is_some());
        p.set_step(step.index() as i32);
        p.set_skipped(ModelRc::new(VecModel::from(skipped)));
        p.set_heading(heading.into());
        p.set_detail(if measuring.is_some() { SharedString::new() } else { step.detail().into() });
        p.set_status(status.into());
        p.set_differ(if measuring.is_some() { SharedString::new() } else { differ_text(&frames).into() });
        p.set_left_out(left_out_text(left_out).into());
        p.set_film(ConversionMode::ALL.iter().position(|f| *f == film).unwrap_or(0) as i32);
        p.set_capture(Capture::ALL.iter().position(|c| *c == capture).unwrap_or(1) as i32);
        p.set_passes_note(passes_note.into());
        p.set_flat_on(flat.mask != oni_settings::frame::FlatFieldMask::None);
        p.set_flat_masked(flat.mask != oni_settings::frame::FlatFieldMask::None && !(flat.mask == oni_settings::frame::FlatFieldMask::Custom && flat.custom.is_none()));
        p.set_monochrome(film.is_monochrome());
        p.set_masks_empty(masks.is_empty());
        p.set_mask_title(mask_title.into());
        p.set_mask_lines(ModelRc::new(VecModel::from(lines)));
        p.set_can_take_flat(picked);
        p.set_taking_flat(taking);
        p.set_loading_flat(loading);
        p.set_fractions(ModelRc::new(VecModel::from(fractions)));
        p.set_readouts(ModelRc::new(VecModel::from(readouts)));
        p.set_base_note(base_note.into());
        p.set_has_base(has_base);
        p.set_flags_note(format!("{} measured, {flagged} flagged", ticked - flagged).into());
        p.set_can_clear_flags(flagged > 0);
        p.set_frame_name(m.get(at).map(|f| oni_library::paths::name(&f.path)).unwrap_or_default().into());
        p.set_can_previous(at > 0);
        p.set_can_next(at < last);
        p.set_cells(ModelRc::new(VecModel::from(cells)));
        p.set_back_shown(step != Step::Film && measuring.is_none());
        p.set_action_title(action.into());
        p.set_action_enabled(step != Step::Window || ticked > flagged);
        p.set_card_picture(card);
        self.publish_passes(&p);
        self.publish_card();
        self.publish_base();
    }

    /// The card's picture where its crop lies in the turned scan, the window on its own step.
    pub(in crate::app) fn publish_card(&self) {
        let Some(w) = self.window() else { return };
        let Some(b) = self.card_box() else { return };
        let (crop, window, on_window) = {
            let r = &self.state.borrow().prepare;
            (r.shown_frame().map_or(Rect::UNIT, |f| f.crop), r.window, r.step == Step::Window)
        };
        let p = w.global::<Preparing>();
        let (x, y, pw, ph) = frames::shown(crop, b);
        p.set_picture_x(x as f32);
        p.set_picture_y(y as f32);
        p.set_picture_width(pw as f32);
        p.set_picture_height(ph as f32);
        p.set_window_shown(on_window);
        let (x, y, ww, wh) = frames::shown(window, b);
        p.set_window_x(x as f32);
        p.set_window_y(y as f32);
        p.set_window_width(ww as f32);
        p.set_window_height(wh as f32);
    }

    /// The base step's scan, its picked ring, and the loupe under the pointer.
    pub(in crate::app) fn publish_base(&self) {
        let Some(w) = self.window() else { return };
        let p = w.global::<Preparing>();
        let (name, index, count) = {
            let r = &self.state.borrow().prepare;
            (r.base_shown().map(|f| oni_library::paths::name(&f.path)).unwrap_or_default(), r.base_frame.min(r.frames.len().saturating_sub(1)), r.frames.len())
        };
        p.set_base_name(name.into());
        p.set_base_can_previous(index > 0);
        p.set_base_can_next(index + 1 < count);
        let Some((rect, size)) = self.scan_rect() else {
            p.set_scan_ready(false);
            p.set_loupe_shown(false);
            return;
        };
        let s = self.state.borrow();
        let radius = base_radius(&s);
        let r = &s.prepare;
        let Some((path, scan, _)) = r.scan.clone() else { return };
        p.set_scan_ready(true);
        p.set_scan(scan.clone());
        p.set_scan_x(rect.0 as f32);
        p.set_scan_y(rect.1 as f32);
        p.set_scan_width(rect.2 as f32);
        p.set_scan_height(rect.3 as f32);
        let picked = r.base.as_ref().filter(|b| b.path == path);
        p.set_ring_shown(picked.is_some());
        if let Some(b) = picked {
            p.set_ring_x((rect.0 + b.unit.0 * rect.2) as f32);
            p.set_ring_y((rect.1 + b.unit.1 * rect.3) as f32);
            p.set_ring_diameter((2.0 * b.radius / size.0.max(1) as f64 * rect.2) as f32);
        }
        let Some(pointer) = r.pointer.filter(|(x, y)| *x >= rect.0 && *y >= rect.1 && *x <= rect.0 + rect.2 && *y <= rect.1 + rect.3) else {
            p.set_loupe_shown(false);
            return;
        };
        let whole = r.full.as_ref().filter(|(f, _, _)| *f == path);
        let (source, (width, height)) = whole.map_or((scan, size), |(_, i, s)| (i.clone(), *s));
        let l = loupe(pointer, rect, size.0, (width, height), whole.is_some(), radius, picked.map(|b| (b.unit, b.radius)));
        p.set_loupe_shown(true);
        p.set_loupe_x(pointer.0 as f32);
        p.set_loupe_y(pointer.1 as f32);
        p.set_loupe_source(source);
        p.set_loupe_cut(ModelRc::new(VecModel::from(l.cut.to_vec())));
        p.set_loupe_shown_at(ModelRc::new(VecModel::from(l.at.iter().map(|v| *v as f32).collect::<Vec<_>>())));
        p.set_loupe_rings(ModelRc::new(VecModel::from(l.rings.iter().map(|v| *v as f32).collect::<Vec<_>>())));
        p.set_loupe_picked(l.picked);
    }
}
