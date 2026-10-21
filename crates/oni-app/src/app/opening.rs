//! A photo opened in the editor: its thumbnail at once, its preview decoded off the interface thread.

use super::*;

impl Ctl {
    /// Opens a photo in the store; the canvas renders once its negative is decoded.
    pub(super) fn open_photo(self: &Rc<Self>, path: &str) {
        self.leave_together();
        let kept = self.remembered_flat_field();
        let failure = {
            let mut s = self.state.borrow_mut();
            let flush = s.store.flush();
            // The flat-field sliders last dialled reach a frame that never had its own; a frame never
            // balanced opens with its measuring window proposed and armed.
            let armed = std::cell::Cell::new(false);
            let opened = s
                .store
                .open_adjusted(path, &|o| {
                    o.flat_field = crate::flat_fields::with_remembered(&o.flat_field, kept);
                    armed.set(oni_core::balance::opening_window(o));
                })
                .err();
            s.route.armed = armed.get();
            s.open = Some(path.to_string());
            s.opened_at = Instant::now();
            s.opens += 1;
            // `load(_:)`: numbering restarts per photo, and another photo's patch sources never show.
            s.numbers.clear();
            s.next_number = 1;
            s.sources.clear();
            s.fallback.clear();
            // A heal that failed (a drive asleep) is tried again when its photo is opened again.
            s.unhealable.clear();
            // Another photo's distributions never stand under this one's handles.
            s.inputs = None;
            s.shelf.hovered = None;
            s.reading = Some(render::Reading::of(s.store.settings()));
            s.inputs_drawn = None;
            s.rendered = 0;
            s.requested = 0;
            // Fitted, unless Settings keeps the zoom to walk a roll at 100 % comparing grain.
            s.viewport.next_photo(crate::viewport::KEEP_ZOOM.load(std::sync::atomic::Ordering::Relaxed));
            s.requested_view = None;
            if s.full.as_ref().is_some_and(|(p, _)| p != path) {
                s.full = None;
            }
            for slide in &mut s.slides {
                slide.published = None;
            }
            flush.or(opened)
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        let read_only = self.state.borrow_mut().store.editor().is_some_and(|e| !e.writable());
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_read_only(read_only);
            w.global::<Canvas>().set_failure("".into());
            w.global::<Canvas>().set_busy(true);
            w.global::<Canvas>().set_status(format!("{} · decoding", stem(path)).into());
        }
        self.show_thumbnail(path);
        self.publish_status();
        self.publish(true);
        self.publish_tree();
        self.schedule_redraw();
        self.decode_open(path);
        self.require_roll(path);
        self.journal_opened(path);
    }

    /// The open frame's preview: the one kept, else a decode off this thread by the reading its
    /// settings ask for.
    pub(super) fn decode_open(self: &Rc<Self>, path: &str) {
        let (previews, converted) = {
            let s = self.state.borrow();
            (s.previews.clone(), s.reading.clone().unwrap_or_else(|| render::Reading::of(s.store.settings())))
        };
        let known = previews.lock().expect("previews").get(path);
        if let Some(negative) = known {
            self.state.borrow_mut().negative = Some((path.to_string(), negative));
            self.prepare_measure(path);
            return self.request_render();
        }
        self.state.borrow_mut().negative = None;
        let path = path.to_string();
        let engine = self.engine.clone();
        let ticket = crate::decode_gate::PREVIEW.ask();
        std::thread::spawn(move || {
            let Some(prepared) = crate::decode_gate::PREVIEW.run(ticket, || prepared_keeping(&engine, &path, converted.clone())) else { return };
            with_shell(move |c| match prepared {
                Ok((negative, full)) => {
                    c.state.borrow().previews.lock().expect("previews").insert(path.clone(), negative.clone());
                    if c.state.borrow().open_path() == Some(path.as_str()) {
                        let mut s = c.state.borrow_mut();
                        // The decode read the whole frame: it is the full resolution, kept rather than read again.
                        if let Some(full) = full.filter(|_| s.full.is_none() && s.reading.as_ref().is_none_or(|r| *r == converted)) {
                            s.full = Some((path.clone(), full));
                        }
                        drop(s);
                        c.state.borrow_mut().negative = Some((path.clone(), negative));
                        c.request_render();
                        c.publish_roll_tab();
                        c.prepare_measure(&path);
                    }
                }
                Err(e) => {
                    if c.state.borrow().open_path() == Some(path.as_str()) {
                        c.deliver(Delivery::Render(Err((PathBuf::from(&path), format!("{}: {e}", stem(&path))))));
                    }
                }
            });
        });
    }

    fn prepare_measure(&self, path: &str) {
        crate::measuring::prepare(path, self.state.borrow().store.settings());
    }

    /// `redecode()`: the open frame read again once its sidecar names another reading.
    pub(super) fn redecode(self: &Rc<Self>) {
        let Some(path) = self.state.borrow().open.clone() else { return };
        {
            let mut s = self.state.borrow_mut();
            s.reading = Some(render::Reading::of(s.store.settings()));
            s.previews.lock().expect("previews").remove(&path);
            s.negative = None;
            s.full = None;
            s.full_failed.remove(&path);
        }
        self.decode_open(&path);
    }
}
