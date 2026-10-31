//! Every Slint callback wired to its handler, once, when the window is built.

use super::*;

impl Ctl {
    pub(super) fn wire(me: &Rc<Ctl>, window: &MainWindow) {
        Ctl::wire_canvas(me, window);
        Ctl::wire_framing(me, window);
        Ctl::wire_dedust(me, window);
        Ctl::wire_missing(me, window);
        crate::editors::wire(me, window);
        let g = window.global::<StoreGlobal>();

        let p = me.clone();
        g.on_press(move |field, _fine| {
            // A frame out of sight takes no hand: the press is answered as handled, so no drag starts.
            let Some(f) = Field::from_index(field).filter(|_| !p.inert()) else { return true };
            let i = f.index() as usize;
            let now = Instant::now();
            let double = {
                let mut s = p.state.borrow_mut();
                let slide = &mut s.slides[i];
                // `DSClick` reads AppKit's click count, which keeps counting past two.
                let double = slide.last_press.is_some_and(|t| now.duration_since(t) <= system::double_click());
                slide.last_press = Some(now);
                slide.resetting = double;
                slide.start = if double { None } else { Some(slide.shown) };
                if !double {
                    s.store.hold();
                    s.hands.fetch_add(1, Ordering::Relaxed);
                }
                double
            };
            if double {
                let base = f.double_click_base(p.state.borrow().store.settings());
                p.set(f, base);
            }
            double
        });

        let p = me.clone();
        g.on_drag(move |field, translation, span, fine| {
            let Some(f) = Field::from_index(field) else { return };
            let i = f.index() as usize;
            let (asked, live, offer) = {
                let mut s = p.state.borrow_mut();
                let range = track_on(f, s.store.settings());
                let slide = &mut s.slides[i];
                let (false, Some(start)) = (slide.resetting, slide.start) else { return };
                let asked = track::clamped(track::asked(start, translation, span, fine));
                let live = stored_on(f, asked, &range);
                let now = Instant::now();
                slide.live = Some(live);
                slide.moved_at = Some(now);
                let offer = slide.hand.offer(now);
                if offer == Offer::Now {
                    s.moved_at = Some(now);
                }
                (asked, live, offer)
            };
            p.place(f, asked);
            if let Some(w) = p.window() {
                set_readout(&w.global::<StoreGlobal>(), f, f.readout(live).into());
            }
            match offer {
                Offer::Now => p.set(f, live),
                Offer::After(delay) => {
                    let q = Rc::downgrade(&p);
                    p.trailing[i].start(TimerMode::SingleShot, delay, move || {
                        let Some(q) = q.upgrade() else { return };
                        let live = {
                            let mut s = q.state.borrow_mut();
                            s.slides[i].hand.trailed(Instant::now());
                            s.moved_at = s.slides[i].moved_at;
                            s.slides[i].live
                        };
                        if let Some(live) = live {
                            q.set(f, live);
                        }
                    });
                }
                Offer::Wait => {}
            }
        });

        let p = me.clone();
        g.on_release(move |field| {
            let Some(f) = Field::from_index(field) else { return };
            let i = f.index() as usize;
            let live = {
                let mut s = p.state.borrow_mut();
                let _ = s.hands.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
                let slide = &mut s.slides[i];
                slide.start = None;
                slide.resetting = false;
                slide.hand.released();
                slide.live.take()
            };
            p.trailing[i].stop();
            if let Some(live) = live {
                p.state.borrow_mut().store.apply(Command::Set { knob: f.knob(), value: live });
            }
            let released = std::time::Instant::now();
            p.state.borrow_mut().store.release();
            // The step's record and any roll it reaches, read and written on this thread.
            if released.elapsed() > std::time::Duration::from_millis(5) {
                timing::line(&format!("a release recorded its step in {:.1} ms", released.elapsed().as_secs_f64() * 1000.0));
            }
            p.publish(false);
            p.schedule_redraw();
            if p.state.borrow().last_change.is_some() {
                p.schedule_write();
            }
            // The readout leaves the hand's value for the store's, which a clamp may have moved.
            if let Some(w) = p.window() {
                let v = p.state.borrow().store.get(f);
                set_readout(&w.global::<StoreGlobal>(), f, f.readout(v).into());
            }
        });

        // A scroll along the track writes the store at each event; one across it scrolls the panel.
        let p = me.clone();
        g.on_scroll(move |field, dx, dy, span, fine| {
            let Some(f) = Field::from_index(field).filter(|_| !p.inert()) else { return false };
            if dx == 0.0 || dx.abs() <= dy.abs() {
                return false;
            }
            let from = p.state.borrow().slides[f.index() as usize].shown;
            let asked = track::clamped(track::asked(from, Ctl::scroll_travel(dx), span, fine));
            p.scroll_kept_alive();
            p.place(f, asked);
            let range = track_on(f, p.state.borrow().store.settings());
            p.set(f, stored_on(f, asked, &range));
            true
        });

        let p = me.clone();
        g.on_step(move |field, direction| {
            let Some(f) = Field::from_index(field) else { return };
            let (low, high) = f.spec().range;
            let v = p.state.borrow().store.get(f) + direction as f32 * (high - low) / 100.0;
            p.set(f, v);
        });

        let p = me.clone();
        g.on_reset(move |block| {
            let Some(b) = Block::from_index(block) else { return };
            let command = store::reset(p.state.borrow().store.settings(), b);
            p.write(command);
        });

        let p = me.clone();
        window.global::<Bar>().on_act(move |a| p.act(a));
        let p = me.clone();
        window.global::<App>().on_menu(move |a| p.act(a));
        let p = me.clone();
        window.global::<App>().on_sheet_changed(move || p.publish_menus());
        let p = me.clone();
        window.global::<App>().on_key(move |text, control, meta, shift, option, repeat| {
            // Slint already hands ⌘ over as `control` on a Mac, and the Control key as `meta`.
            p.key(&text, control, shift, !(control || meta || shift || option), repeat)
        });
        Ctl::wire_lab(me, window);
        Ctl::wire_settings(me, window);
        me.adopt_remembered_face();
        me.adopt_frontier_look();
        let p = me.clone();
        window.global::<Pairing>().on_close(move || p.act(crate::Act::ClosePairing));
        let p = me.clone();
        window.global::<Pairing>().on_forget(move || p.act(crate::Act::ForgetPhones));
        let p = me.clone();
        window.global::<crate::Notice>().on_close(move || p.neutral_decode_notice_dismissed());
        let p = me.clone();
        window.global::<PanelState>().on_auto_balance(move || {
            p.journal_balanced();
            p.act(crate::Act::AutoBalance);
        });
        let p = me.clone();
        window.global::<PanelState>().on_auto_levels(move || p.auto_levels());
        let p = me.clone();
        window.global::<PanelState>().on_film_type_picked(move |i| {
            p.film_type(i);
            p.publish_bar();
        });
        let p = me.clone();
        window.global::<Confirm>().on_answered(move |go| p.answered(go));

        let lib = window.global::<Library>();
        let p = me.clone();
        lib.on_row_clicked(move |i, command, shift| {
            let (command, shift) = crate::system::held(command, shift);
            p.row_clicked(i.max(0) as usize, command, shift);
        });
        me.wire_tree(&lib);
        let p = me.clone();
        lib.on_tile_clicked(move |i, toggle, extend| {
            p.grid_took_keys();
            let (toggle, extend) = crate::system::held(toggle, extend);
            p.tile_clicked(i.max(0) as usize, toggle, extend);
        });
        let p = me.clone();
        lib.on_tile_opened(move |i| p.tile_opened(i.max(0) as usize));
        Ctl::wire_organising(me, window);
        Ctl::wire_navigator(me, window);
        Ctl::wire_presets(me, window);
        Ctl::wire_rolls(me, window);
        Ctl::wire_roll_tab(me, window);
        Ctl::wire_help(me, window);
        Ctl::wire_transition(me, window);
        Ctl::wire_told(me, window);
        crate::platform::publish_keys(window);
        Ctl::wire_stock_picker(me, window);
        Ctl::wire_wrapped(me, window);
        Ctl::wire_loupe(me, window);
        Ctl::wire_tree_rename(me, window);
        Ctl::wire_folds(me, window);
        let p = me.clone();
        window.window().on_close_requested(move || p.close_requested());
        Ctl::wire_prepare(me, window);
        let p = me.clone();
        lib.on_scrolled(move |by| {
            {
                let mut s = p.state.borrow_mut();
                s.scroll = s.layout.clamped(s.scroll + by, s.grid.1);
            }
            let (moved, scroll) = {
                let s = p.state.borrow();
                (s.layout.cells(s.scroll, s.grid.1) != s.visible || s.renaming_heading(), s.scroll)
            };
            // Only a change of the visible range rebuilds the tiles; the offset alone moves them.
            if moved {
                p.publish_tiles();
            } else if let Some(w) = p.window() {
                w.global::<Library>().set_scroll(scroll);
                p.publish_pinned();
            }
        });
        let p = me.clone();
        lib.on_banner_acted(move |i| p.banner_acted(i.max(0) as usize));
        let p = me.clone();
        lib.on_resized(move |w, h| {
            let changed = {
                let mut s = p.state.borrow_mut();
                let changed = s.grid != (w, h);
                s.grid = (w, h);
                changed
            };
            if changed {
                p.relayout();
            }
        });

        let p = Rc::downgrade(me);
        me.pulse.start(TimerMode::Repeated, PULSE, move || {
            if let Some(p) = p.upgrade() {
                p.pulse();
            }
        });
    }
}

impl Ctl {
    pub(super) fn wire_dedust(me: &Rc<Ctl>, window: &MainWindow) {
        let d = window.global::<crate::Dedust>();
        let p = me.clone();
        d.on_press(move |_| p.brush_press());
        let p = me.clone();
        d.on_drag(move |t, span, fine| p.brush_drag(t, span, fine));
        let p = me.clone();
        d.on_scroll(move |dx, dy, span, fine| p.brush_scroll(dx, dy, span, fine));
        let p = me.clone();
        d.on_release(move || {
            let dragged = p.state.borrow_mut().brush_start.take().is_some();
            if dragged {
                p.keep_brush();
            }
            p.publish_dedust();
        });
        let p = me.clone();
        d.on_remove(move |i| p.remove_correction(usize::try_from(i).unwrap_or(usize::MAX)));
        let c = window.global::<Canvas>();
        let p = me.clone();
        c.on_paint_start(move |x, y| p.paint_at(x, y, true));
        let p = me.clone();
        c.on_paint_move(move |x, y| p.paint_at(x, y, false));
        let p = me.clone();
        c.on_paint_end(move || p.paint_end());
    }

    pub(super) fn wire_framing(me: &Rc<Ctl>, window: &MainWindow) {
        let f = window.global::<crate::Framing>();
        let ratios = [oni_settings::frame::AspectRatio::ThreeTwo, oni_settings::frame::AspectRatio::FourThree, oni_settings::frame::AspectRatio::Square, oni_settings::frame::AspectRatio::SixSeven, oni_settings::frame::AspectRatio::FiveFour];
        let p = me.clone();
        f.on_ratio_picked(move |i| {
            let ratio = usize::try_from(i - 1).ok().and_then(|k| ratios.get(k).copied());
            p.choose_framing(|g| g.ratio = ratio);
        });
        let p = me.clone();
        f.on_portrait_toggled(move |on| p.choose_framing(|g| g.portrait = on));
        let p = me.clone();
        f.on_act(move |action| p.act(action));
        let p = me.clone();
        window.global::<PanelState>().on_tab_picked(move |tab| p.tab_opened(tab));
        let c = window.global::<Canvas>();
        let p = me.clone();
        c.on_frame_grabbed(move |w, h| p.frame_grabbed(w, h));
        let p = me.clone();
        c.on_frame_dragged(move |_, _, dx, dy| p.frame_dragged(dx, dy));
        let p = me.clone();
        c.on_frame_released(move |_| p.frame_released());
    }

    /// `NegativeView`'s gestures: a drag pans, a double click toggles 100 %, the wheel zooms about
    /// the pointer, a touch surface pans unless ⌘ is held, a pinch zooms about its centre.
    pub(super) fn wire_canvas(me: &Rc<Ctl>, window: &MainWindow) {
        let c = window.global::<Canvas>();
        let backing = move |p: &Rc<Ctl>| p.window().map_or(2.0, |w| f64::from(w.window().scale_factor()));

        let p = me.clone();
        c.on_resized(move || p.resized());

        let p = me.clone();
        c.on_split_dragged(move |x| {
            let width = p.window().map_or(0.0, |w| w.global::<Canvas>().get_width());
            if width <= 0.0 || p.state.borrow().compare.split.is_none() {
                return;
            }
            let split = (x / width).clamp(0.0, 1.0);
            p.state.borrow_mut().compare.split = Some(split);
            if let Some(w) = p.window() {
                w.global::<Canvas>().set_split(split);
            }
        });

        let p = me.clone();
        c.on_double_clicked(move || p.navigate(|v, full, size| v.toggle_hundred(full, size)));

        let p = me.clone();
        c.on_grabbed(move |x, y| p.state.borrow_mut().grab = Some((x, y)));

        let p = me.clone();
        c.on_dragged(move |x, y| {
            let Some((gx, gy)) = p.state.borrow_mut().grab.replace((x, y)) else { return };
            let b = backing(&p);
            p.by_hand(|v, full, size| v.pan(viewport::pan_step(f64::from(x - gx), f64::from(y - gy), b), full, size));
        });

        let p = me.clone();
        c.on_scrolled(move |dx, dy, x, y, command| {
            if p.state.borrow().full_size.is_none() {
                return false;
            }
            let b = backing(&p);
            let wheel = viewport::take_wheel().unwrap_or(viewport::Wheel { dx: f64::from(dx), dy: f64::from(dy), precise: true });
            let at = (f64::from(x) * b, f64::from(y) * b);
            if viewport::zooms(wheel.precise, command) {
                let factor = viewport::zoom_factor(wheel.dy, wheel.precise);
                p.by_hand(|v, full, size| v.zoom_by(factor, at, full, size));
            } else {
                p.by_hand(|v, full, size| v.pan(viewport::pan_step(wheel.dx, wheel.dy, b), full, size));
            }
            true
        });

        let p = me.clone();
        c.on_pinch_started(move || p.state.borrow_mut().pinch = 1.0);

        let p = me.clone();
        c.on_pinched(move |scale, x, y| {
            let factor = {
                let mut s = p.state.borrow_mut();
                let factor = f64::from(scale / s.pinch.max(1e-6));
                s.pinch = scale;
                factor
            };
            let b = backing(&p);
            p.by_hand(|v, full, size| v.zoom_by(factor, (f64::from(x) * b, f64::from(y) * b), full, size));
        });
    }
}
