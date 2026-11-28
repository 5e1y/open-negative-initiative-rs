//! Publishing the store to the window: every moved slider, the bar, and the deferred sidecar write.

use super::*;

impl Ctl {
    // MARK: - Publishing

    /// Every moved slider's value to the panel; a knob under the hand keeps its place, one moved by
    /// a command springs there, and `jump` places them all at once, as opening a photo does.
    pub(crate) fn publish(self: &Rc<Self>, jump: bool) {
        let started = Instant::now();
        self.publish_store(jump);
        self.state.borrow_mut().notches.record_publish(started.elapsed());
    }

    fn publish_store(self: &Rc<Self>, jump: bool) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Store>();
        let (moved, edited) = {
            let mut s = self.state.borrow_mut();
            // Any new state of the open frame is written, a slider's or not: a roll joined or reset moves none.
            let revision = s.store.revision();
            let edited = revision != s.published_revision;
            s.published_revision = revision;
            let mut moved: Vec<Field> = store::changed(&mut s.store);
            let ranges: Vec<_> = Field::ALL.iter().map(|f| f.knob().range(s.store.settings())).collect();
            if jump {
                moved = Field::ALL.to_vec();
            }
            let reranged: Vec<Field> = Field::ALL.into_iter().filter(|f| !moved.contains(f) && s.slides[f.index() as usize].range != ranges[f.index() as usize]).collect();
            moved.extend(reranged);
            let mut out = Vec::new();
            for f in moved {
                let (v, range) = (store::get(&s.store, f), ranges[f.index() as usize]);
                let slide = &mut s.slides[f.index() as usize];
                if !jump && slide.published == Some(v) && slide.range == range {
                    continue;
                }
                (slide.published, slide.range) = (Some(v), range);
                out.push((f, v, slide.live().is_some()));
            }
            (out, edited)
        };
        self.publish_spans();
        for (f, v, held) in &moved {
            let text: SharedString = self.readout_at(*f, *v).into();
            set_value(&g, *f, text.clone());
            if !held {
                set_readout(&g, *f, text);
                let track = track_on(*f, self.state.borrow().store.settings());
                let at = position_on(*f, *v, &track);
                // A notched knob goes from notch to notch at once, as the value does: it never travels between two.
                if jump || track.levels.is_some() {
                    self.place(*f, at);
                } else {
                    self.spring(*f, at);
                }
            }
        }
        // A reset key shows while a frame shown stands off the rest it would take, the open one or one beside it.
        // The filters' shared move, which Temperature and Tint leave out, told where they show.
        let shared = self.state.borrow().store.settings().print.filters().iter().sum::<f32>() / 3.0;
        let told = if shared.abs() < 0.005 { String::new() } else { oni_i18n::tr!("C, M and Y move the light by {stops}", stops = self.readout_at(Field::Cyan, shared)) };
        g.set_filters_shared(told.into());
        let beside = self.beside().blocks;
        let (modified, rests, marked) = {
            let s = self.state.borrow();
            let shown = |i: usize| (Block::ALL[i], beside[i] || store::is_modified(s.store.settings(), Block::ALL[i]));
            let marked = store::channel_marked(s.store.settings());
            (std::array::from_fn::<_, { Block::ALL.len() }, _>(shown), crate::slider_track::rests(s.store.settings()), marked)
        };
        let notches = crate::slider_track::notches(self.state.borrow().store.settings());
        let laid = g.get_notches();
        let renotched: Vec<Field> = Field::ALL.into_iter().filter(|f| laid.row_data(f.index() as usize) != notches.get(f.index() as usize).copied()).collect();
        if !renotched.is_empty() {
            g.set_notches(ModelRc::new(VecModel::from(notches)));
            // A track that gains or loses its notches names them, or no more, though its value stays.
            self.publish_readouts(renotched);
        }
        for (b, on) in modified {
            set_modified(&g, b, on);
        }
        let tabs = g.get_channel_marked();
        if (0..3).any(|c| tabs.row_data(c) != Some(marked[c])) {
            g.set_channel_marked(ModelRc::new(VecModel::from(marked.to_vec())));
        }
        let shown = g.get_rests();
        if shown.row_count() != rests.len() || rests.iter().enumerate().any(|(i, r)| shown.row_data(i) != Some(*r)) {
            g.set_rests(ModelRc::new(VecModel::from(rests)));
        }
        let reread = edited && {
            let s = self.state.borrow();
            s.frame.as_ref().is_some_and(|f| f.reading != render::Reading::of(s.store.settings()))
        };
        if reread {
            self.redecode();
        }
        if edited || !moved.is_empty() || jump {
            self.state.borrow_mut().last_change = Some(Instant::now());
            self.schedule_write();
            self.publish_saved();
        }
        self.refresh_phone();
        self.publish_bar();
        self.publish_framing();
        self.publish_dedust();
        crate::editors::publish(self, jump);
        self.request_render();
    }

    /// Every property is read before any is set, so no Slint callback runs under the state's borrow.
    pub(super) fn publish_bar(&self) {
        let Some(w) = self.window() else { return };
        self.follow_open_photo();
        self.reveal_project();
        self.publish_menus();
        let bar = w.global::<Bar>();
        let gallery = w.global::<App>().get_gallery();
        let (has_photo, single, targets, archive) = {
            let s = self.state.borrow();
            let archive = !s.org.projects_surface && !s.trash_shown();
            (s.frame.is_some(), s.together.is_none(), s.target_count(gallery), archive)
        };
        let (live, titles) = (w.global::<Menus>().get_live(), w.global::<Menus>().get_titles());
        bar.set_has_photo(has_photo);
        bar.set_single(single);
        bar.set_can_undo(live.undo);
        bar.set_can_redo(live.redo);
        bar.set_undo_title(titles.undo);
        bar.set_redo_title(titles.redo);
        bar.set_can_reset(live.reset);
        bar.set_targets(targets as i32);
        bar.set_editing(!gallery);
        bar.set_archive(archive);
        self.publish_mode();
    }

    pub(super) fn place(&self, f: Field, at: f32) {
        self.state.borrow_mut().slides[f.index() as usize].knob.aim(at, true, Instant::now());
        if let Some(w) = self.window() {
            set_fraction(&w.global::<Store>(), f, at);
        }
    }

    /// A value arriving from elsewhere springs the knob; the canvas takes it at once.
    pub(super) fn spring(self: &Rc<Self>, f: Field, to: f32) {
        let now = Instant::now();
        let moving = {
            let knob = &mut self.state.borrow_mut().slides[f.index() as usize].knob;
            knob.aim(to, false, now);
            knob.moving(now)
        };
        if !moving {
            return self.place(f, to);
        }
        if !self.arrival.running() {
            self.state.borrow_mut().arrivals += 1;
            let me = Rc::downgrade(self);
            self.arrival.start(TimerMode::Repeated, FRAME_TICK, move || {
                if let Some(me) = me.upgrade() {
                    me.arrive();
                }
            });
        }
    }

    /// A panel knob is springing, read by the checks.
    pub fn arriving(&self) -> bool {
        self.arrival.running()
    }

    /// One frame of the panel's springs: each moving knob drawn where it shows, the timer stopped once all rest.
    fn arrive(&self) {
        let now = Instant::now();
        let (drawn, resting) = {
            let mut s = self.state.borrow_mut();
            let drawn: Vec<(Field, f32)> =
                Field::ALL.into_iter().zip(s.slides.iter_mut()).filter_map(|(f, slide)| Some((f, slide.knob.step(now)?))).collect();
            (drawn, !s.slides.iter().any(|slide| slide.knob.moving(now)))
        };
        if let Some(w) = self.window() {
            let g = w.global::<Store>();
            for (f, at) in drawn {
                set_fraction(&g, f, at);
            }
        }
        if resting {
            self.arrival.stop();
        }
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

    /// The hand's value for `f`: a move, which frames shown together make by the same distance.
    pub(crate) fn set(self: &Rc<Self>, f: Field, stored: f32) {
        if f == Field::Angle {
            return self.straighten(stored);
        }
        let command = store::moved(&self.state.borrow().store, f, stored);
        self.write(command);
    }

    /// A block's reset key: the open frame back to its rest, each frame shown with it to its own.
    pub(super) fn reset_block(self: &Rc<Self>, b: Block) {
        self.write(store::reset(b));
    }

    /// `f` back to its rest on every frame shown, as a double click on its rail or on its name asks.
    pub(crate) fn rest(self: &Rc<Self>, f: Field) {
        let stored = f.base(self.state.borrow().store.settings());
        if f == Field::Angle {
            return self.straighten(stored);
        }
        self.write(Command::Set { knob: f.knob(), value: stored });
    }
}
