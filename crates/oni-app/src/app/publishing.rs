//! Publishing the store to the window: every moved slider, the bar, and the deferred sidecar write.

use super::*;

impl Ctl {
    // MARK: - Publishing

    /// Every moved slider's value to the panel; a knob under the hand keeps its place, one moved by
    /// a command springs there, and `jump` places them all at once, as opening a photo does.
    pub(crate) fn publish(self: &Rc<Self>, jump: bool) {
        let Some(w) = self.window() else { return };
        let g = w.global::<StoreGlobal>();
        let (moved, edited) = {
            let mut s = self.state.borrow_mut();
            // Any new state of the open frame is written, a slider's or not: a roll joined or reset moves none.
            let revision = s.store.revision();
            let edited = revision != s.published_revision;
            s.published_revision = revision;
            let mut moved: Vec<Field> = s.store.changed();
            if jump {
                moved = Field::ALL.to_vec();
            }
            let mut out = Vec::new();
            for f in moved {
                let v = s.store.get(f);
                let slide = &mut s.slides[f.index() as usize];
                if !jump && slide.published == Some(v) {
                    continue;
                }
                slide.published = Some(v);
                out.push((f, v, slide.live.is_some()));
            }
            (out, edited)
        };
        for (f, v, held) in &moved {
            let text: SharedString = f.spec().readout.format(*v).into();
            set_value(&g, *f, text.clone());
            if !held {
                set_readout(&g, *f, text);
                if jump {
                    self.place(*f, position(*f, *v));
                } else {
                    self.spring(*f, position(*f, *v));
                }
            }
        }
        let modified = {
            let s = self.state.borrow();
            Block::ALL.map(|b| (b, store::is_modified(s.store.settings(), b)))
        };
        for (b, on) in modified {
            set_modified(&g, b, on);
        }
        let reread = edited && {
            let s = self.state.borrow();
            s.open.is_some() && s.reading.as_ref().is_some_and(|r| *r != render::Reading::of(s.store.settings()))
        };
        if reread {
            self.redecode();
        }
        if edited || !moved.is_empty() || jump {
            self.state.borrow_mut().last_change = Some(Instant::now());
            self.schedule_write();
        }
        self.refresh_phone();
        self.publish_bar();
        self.publish_framing();
        self.publish_dedust();
        crate::editors::publish(self, jump);
        self.request_render();
        // Every edit brings the library column back onto the photo it acts on.
        self.centre_tree();
    }

    /// Every property is read before any is set, so no Slint callback runs under the state's borrow.
    pub(super) fn publish_bar(&self) {
        let Some(w) = self.window() else { return };
        self.follow_open_photo();
        self.publish_menus();
        let bar = w.global::<Bar>();
        let (has_photo, selected) = {
            let s = self.state.borrow();
            (s.open.is_some(), s.picks.selection.len())
        };
        let (live, titles) = (w.global::<Menus>().get_live(), w.global::<Menus>().get_titles());
        bar.set_has_photo(has_photo);
        bar.set_can_undo(live.undo);
        bar.set_can_redo(live.redo);
        bar.set_undo_title(titles.undo);
        bar.set_redo_title(titles.redo);
        bar.set_can_reset(live.reset);
        bar.set_targets(selected.max(usize::from(has_photo)) as i32);
        bar.set_export_title(crate::export::label::key(selected.max(usize::from(has_photo))).into());
        bar.set_editing(!w.global::<App>().get_gallery());
        self.publish_lab();
        self.publish_export_menu(selected, has_photo);
    }

    pub(super) fn place(&self, f: Field, at: f32) {
        let i = f.index() as usize;
        self.arrivals[i].stop();
        {
            let mut s = self.state.borrow_mut();
            s.slides[i].arrival = None;
            s.slides[i].shown = at;
        }
        if let Some(w) = self.window() {
            set_fraction(&w.global::<StoreGlobal>(), f, at);
        }
    }

    /// `DS.Motion.settle`: a value arriving from elsewhere springs the knob; the canvas takes it at once.
    pub(super) fn spring(self: &Rc<Self>, f: Field, to: f32) {
        let i = f.index() as usize;
        let from = {
            let mut s = self.state.borrow_mut();
            let from = s.slides[i].shown;
            s.slides[i].arrival = Some((from, to, Instant::now()));
            from
        };
        if from == to {
            return self.place(f, to);
        }
        let me = Rc::downgrade(self);
        self.arrivals[i].start(TimerMode::Repeated, FRAME_TICK, move || {
            let Some(me) = me.upgrade() else { return };
            let arrival = me.state.borrow().slides[i].arrival;
            let Some((from, to, at)) = arrival else { return me.arrivals[i].stop() };
            let t = at.elapsed().as_secs_f64();
            let spring = Spring::SETTLE;
            if t >= spring.settled_after() {
                me.place(f, to);
            } else {
                let at = from + (to - from) * spring.value(t) as f32;
                me.state.borrow_mut().slides[i].shown = at;
                if let Some(w) = me.window() {
                    set_fraction(&w.global::<StoreGlobal>(), f, at);
                }
            }
        });
    }

    // MARK: - Writing

    /// One notch: one store write, then what moved is published and the canvas asked again.
    pub(super) fn write(self: &Rc<Self>, command: Command) {
        let (remembers, was) = (masks::remembers(&command), self.state.borrow().store.settings().flat_field.clone());
        self.state.borrow_mut().store.apply(command);
        if remembers {
            self.remember_flat_field(&was);
        }
        self.publish(false);
    }

    pub(crate) fn set(self: &Rc<Self>, f: Field, stored: f32) {
        if f == Field::Angle {
            return self.straighten(stored);
        }
        self.write(Command::Set { knob: f.knob(), value: stored });
    }
}
