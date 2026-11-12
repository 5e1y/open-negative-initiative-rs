//! A photo opened in the editor: its thumbnail at once, its preview decoded off the interface thread.

use super::*;

mod missing;

impl Ctl {
    /// Opens a photo in the store, with no mode; the canvas renders once its negative is decoded.
    pub(super) fn open_photo(self: &Rc<Self>, path: &str) {
        // A hand still down lands on the frame it was on; a frame held belongs to the photo left.
        self.let_go_all();
        self.end_frame_gesture();
        self.leave_together();
        self.leave_landed();
        self.leave_modes();
        let remembered = self.remembered_flat_field();
        let (failure, stranded) = {
            let mut s = self.state.borrow_mut();
            // The frame left is stranded when its edits join the unsaved ones with this opening.
            let leaving = s.store.path().map(str::to_string);
            let kept = |s: &State| leaving.as_ref().is_some_and(|l| s.store.unsaved().iter().any(|u| &u.path == l));
            let was_kept = kept(&s);
            // The flat-field sliders last dialled reach a frame that never had its own; a frame never
            // balanced opens with its measuring window proposed and armed.
            let armed = std::cell::Cell::new(false);
            let opened = s.store.open_adjusted(path, |o| {
                o.flat_field = crate::flat_fields::with_remembered(&o.flat_field, remembered);
                armed.set(oni_core::balance::opening_window(o));
            });
            let away = matches!(opened, Err(oni_settings::sidecar::Error::Held(oni_settings::migrate::Refusal::Away)));
            let opened = opened.err().map(|e| oni_library::refusal::said(&e));
            s.route.armed = armed.get();
            // A frame of its own: nothing the canvas asked of the last one, its distributions included, stands under it.
            let reading = render::Reading::of(s.store.settings());
            let previous = s.frame.take();
            s.frame = Some(frame::Frame { away, ..frame::Frame::opened(path, reading, previous) });
            s.org.editing_project = if s.org.projects_surface { s.project_of(path) } else { None };
            s.opened_at = Instant::now();
            s.opens += 1;
            s.dust.next_photo();
            s.shelf.hovered = None;
            // Fitted, unless Settings keeps the zoom to walk a roll at 100 % comparing grain.
            s.viewport.next_photo(crate::viewport::KEEP_ZOOM.load(std::sync::atomic::Ordering::Relaxed));
            for slide in &mut s.slides {
                slide.published = None;
            }
            (opened, kept(&s) && !was_kept)
        };
        let read_only = !self.state.borrow_mut().store.writable();
        // Checked before any decode, and said last, so no status of the opening covers it.
        let absent = self.say_absent(path);
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_read_only(read_only);
            w.global::<Canvas>().set_failure("".into());
            w.global::<Canvas>().set_convertible(false);
            w.global::<Canvas>().set_busy(!absent);
            if !absent {
                w.global::<Canvas>().set_status(tr!("{name} · decoding", name = stem(path)).into());
            }
        }
        match failure {
            Some(message) if !absent => self.say(&message),
            _ => {}
        }
        self.show_thumbnail(path);
        self.publish_status();
        self.publish(true);
        self.publish_tree();
        self.schedule_redraw();
        if absent {
            if let Some(f) = self.state.borrow_mut().frame_for(path) {
                f.release_full();
            }
        } else {
            self.decode_open(path);
        }
        if stranded {
            self.ask_unsaved();
        }
        self.prefetch_neighbours(path);
        self.require_roll(path);
        self.journal_opened(path);
    }

    /// The frames either side of the one opened, decoded into the disk cache behind the hand's own,
    /// so a step to one reads its decode back rather than LibRaw's; a later opening drops the rest.
    fn prefetch_neighbours(&self, path: &str) {
        let near: Vec<String> = {
            let s = self.state.borrow();
            let Some(at) = s.order.iter().position(|p| p == path) else { return };
            [at + 1, at.wrapping_sub(1)].iter().filter_map(|&i| s.order.get(i)).filter(|p| oni_decode::route(Path::new(p.as_str())) == oni_decode::Branch::Mosaic).cloned().collect()
        };
        let mine = self.state.borrow().asks.near.next();
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            for p in near {
                if !mine.current() {
                    return;
                }
                let _ = render::decode(Path::new(&p), render::Reading::default());
            }
        });
    }

    /// The open frame's preview: the one kept, else a decode off this thread by the reading its
    /// settings ask for.
    pub(super) fn decode_open(self: &Rc<Self>, path: &str) {
        let (previews, converted) = {
            let mut s = self.state.borrow_mut();
            let Some(f) = s.frame_for(path) else { return };
            let converted = f.reading.clone();
            (s.previews.clone(), converted)
        };
        let known = previews.lock().expect("previews").get(path, &converted);
        if let Some(negative) = known {
            if let Some(f) = self.state.borrow_mut().frame_for(path) {
                f.negative = Some(negative);
            }
            if self.downloaded(path) {
                return;
            }
            self.request_render();
            return self.balance_ahead();
        }
        let path = path.to_string();
        let engine = self.engine.clone();
        let ticket = crate::decode_gate::PREVIEW.ask();
        std::thread::spawn(move || {
            let Some(prepared) = crate::decode_gate::PREVIEW.run(ticket, || prepared_keeping(&engine, &path, converted.clone())) else { return };
            with_shell(move |c| c.opening_decoded(&path, &converted, prepared));
        });
    }

    /// Whether `path`, opened read-only as its original was in the cloud, was opened again now that its
    /// decode has the file: its sidecar brought first, so the frame opens editable on it.
    fn downloaded(self: &Rc<Self>, path: &str) -> bool {
        let away = self.state.borrow_mut().frame_for(path).is_some_and(|f| std::mem::take(&mut f.away));
        if !away {
            return false;
        }
        crate::migration::gated(path);
        let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
        let still = oni_settings::sidecar::read(Path::new(path), mosaic).is_ok_and(|l| l.held_back == Some(oni_settings::migrate::Refusal::Away));
        if !still {
            self.open_photo(path);
        }
        !still
    }

    /// A preview decode of `path` under `reading`, kept among the previews and installed only while the
    /// open frame is `path` read so; public so a check lands one by hand.
    pub fn opening_decoded(self: &Rc<Self>, path: &str, reading: &render::Reading, prepared: Result<(Arc<Negative>, Option<oni_graph::Source>), String>) {
        if let Ok((negative, _)) = &prepared {
            self.state.borrow().previews.lock().expect("previews").insert(path.to_string(), reading.clone(), negative.clone());
        }
        // A decode under a reading the frame has since left installs nothing and blanks nothing.
        if !self.state.borrow_mut().wants(path, reading) {
            return;
        }
        match prepared {
            Ok((negative, full)) => {
                if let Some(f) = self.state.borrow_mut().frame_for(path) {
                    // The decode read the whole frame: it is the full resolution, kept rather than read again.
                    if f.full.is_none() {
                        f.full = full;
                    }
                    f.negative = Some(negative);
                }
                if self.downloaded(path) {
                    return;
                }
                self.request_render();
                self.publish_roll_tab();
                self.balance_ahead();
            }
            Err(e) => self.decode_failed(path, e),
        }
    }

    /// `redecode()`: the open frame read again once its sidecar names another reading.
    pub(super) fn redecode(self: &Rc<Self>) {
        let Some(path) = self.state.borrow().open_path().map(str::to_string) else { return };
        {
            let mut s = self.state.borrow_mut();
            let reading = render::Reading::of(s.store.settings());
            if let Some(f) = s.frame_for(&path) {
                f.reread(reading);
            }
            s.previews.lock().expect("previews").remove(&path);
            s.full_failed.remove(&path);
        }
        self.decode_open(&path);
    }
}
