//! A photo opened in the editor: its thumbnail at once, its preview decoded off the interface thread.

use super::*;

mod missing;

impl Ctl {
    /// Opens a photo in the store; the canvas renders once its negative is decoded.
    pub(super) fn open_photo(self: &Rc<Self>, path: &str) {
        self.leave_together();
        let remembered = self.remembered_flat_field();
        let (failure, stranded) = {
            let mut s = self.state.borrow_mut();
            // The frame left is stranded when its edits join the unsaved ones with this opening.
            let leaving = s.store.editor_ref().and_then(|e| e.path()).map(str::to_string);
            let kept = |s: &State| leaving.as_ref().is_some_and(|l| s.store.editor_ref().is_some_and(|e| e.unsaved().iter().any(|u| &u.path == l)));
            let was_kept = kept(&s);
            // The flat-field sliders last dialled reach a frame that never had its own; a frame never
            // balanced opens with its measuring window proposed and armed.
            let armed = std::cell::Cell::new(false);
            let opened = s
                .store
                .open_adjusted(path, &|o| {
                    o.flat_field = crate::flat_fields::with_remembered(&o.flat_field, remembered);
                    armed.set(oni_core::balance::opening_window(o));
                })
                .err();
            s.route.armed = armed.get();
            s.open = Some(path.to_string());
            s.org.editing_project = if s.org.projects_surface { s.project_of(path) } else { None };
            s.opened_at = Instant::now();
            s.opens += 1;
            s.dust.next_photo();
            // Nothing the canvas asked of the last frame, its distributions included, stands under this one.
            s.requests = rendering::Requests::default();
            s.shelf.hovered = None;
            s.reading = Some(render::Reading::of(s.store.settings()));
            // Fitted, unless Settings keeps the zoom to walk a roll at 100 % comparing grain.
            s.viewport.next_photo(crate::viewport::KEEP_ZOOM.load(std::sync::atomic::Ordering::Relaxed));
            if s.full.as_ref().is_some_and(|(p, _)| p != path) {
                s.full = None;
            }
            for slide in &mut s.slides {
                slide.published = None;
            }
            (opened, kept(&s) && !was_kept)
        };
        let read_only = self.state.borrow_mut().store.editor().is_some_and(|e| !e.writable());
        // Checked before any decode, and said last, so no status of the opening covers it.
        let absent = self.say_absent(path);
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_read_only(read_only);
            w.global::<Canvas>().set_failure("".into());
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
            let mut s = self.state.borrow_mut();
            s.negative = None;
            s.full = None;
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
        static NEAR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let near: Vec<String> = {
            let s = self.state.borrow();
            let Some(at) = s.order.iter().position(|p| p == path) else { return };
            [at + 1, at.wrapping_sub(1)].iter().filter_map(|&i| s.order.get(i)).filter(|p| oni_decode::route(Path::new(p.as_str())) == oni_decode::Branch::Mosaic).cloned().collect()
        };
        let mine = NEAR.fetch_add(1, Ordering::Relaxed) + 1;
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            for p in near {
                if NEAR.load(Ordering::Relaxed) != mine {
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
            let s = self.state.borrow();
            (s.previews.clone(), s.reading.clone().unwrap_or_else(|| render::Reading::of(s.store.settings())))
        };
        let known = previews.lock().expect("previews").get(path, converted.slide);
        if let Some(negative) = known {
            self.state.borrow_mut().negative = Some((path.to_string(), negative));
            self.request_render();
            return self.balance_ahead();
        }
        self.state.borrow_mut().negative = None;
        let path = path.to_string();
        let engine = self.engine.clone();
        let ticket = crate::decode_gate::PREVIEW.ask();
        std::thread::spawn(move || {
            let Some(prepared) = crate::decode_gate::PREVIEW.run(ticket, || prepared_keeping(&engine, &path, converted.clone())) else { return };
            with_shell(move |c| match prepared {
                Ok((negative, full)) => {
                    c.state.borrow().previews.lock().expect("previews").insert(path.clone(), converted.slide, negative.clone());
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
                        c.balance_ahead();
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
