//! The surface's callbacks and the thread the measure runs on.

use super::*;

impl Ctl {
    pub(in crate::app) fn wire_prepare(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("prepare the roll".into())
            .spawn(move || {
                for job in rx {
                    job();
                }
            })
            .expect("prepare thread");
        {
            let mut s = me.state.borrow_mut();
            s.prepare.serial = Some(tx);
            s.prepare.previews = Some(Previews::start(|session, out| with_shell(move |c| c.preview_landed(session, out))));
        }
        let p = window.global::<Preparing>();
        Self::wire_passes(me, &p);
        let c = me.clone();
        p.on_step_picked(move |i| c.step_to(Step::ALL.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or_default()));
        let c = me.clone();
        p.on_film_picked(move |i| {
            if let Some(m) = usize::try_from(i).ok().and_then(|i| ConversionMode::ALL.get(i).copied()) {
                c.state.borrow_mut().prepare.film = m;
                c.publish_prepare();
            }
        });
        let c = me.clone();
        p.on_capture_picked(move |i| {
            if let Some(capture) = usize::try_from(i).ok().and_then(|i| Capture::ALL.get(i).copied()) {
                c.capture_picked(capture);
            }
        });
        let c = me.clone();
        p.on_flat_toggled(move |on| {
            {
                let mut s = c.state.borrow_mut();
                s.prepare.flat = crate::app::roll_tab::toggled(&s.prepare.flat, on);
            }
            c.publish_prepare();
        });
        let c = me.clone();
        p.on_mask_chosen(move |i| c.roll_mask_chosen(usize::try_from(i).ok()));
        let c = me.clone();
        p.on_press(move |i, _| c.slider_press(usize::try_from(i).unwrap_or(usize::MAX)));
        let c = me.clone();
        p.on_drag(move |i, t, span, fine| c.slider_drag(usize::try_from(i).unwrap_or(usize::MAX), t, span, fine));
        let c = me.clone();
        p.on_release(move |_| c.state.borrow_mut().prepare.slide = None);
        let c = me.clone();
        p.on_clear_base(move || {
            c.state.borrow_mut().prepare.base = None;
            c.publish_prepare();
        });
        let c = me.clone();
        p.on_clear_flags(move || {
            for f in &mut c.state.borrow_mut().prepare.frames {
                f.flagged = false;
            }
            c.publish_prepare();
        });
        let c = me.clone();
        p.on_previous(move || c.walk_window(-1));
        let c = me.clone();
        p.on_next(move || c.walk_window(1));
        let c = me.clone();
        p.on_cell_clicked(move |i| c.cell_clicked(usize::try_from(i).unwrap_or(usize::MAX)));
        let c = me.clone();
        p.on_sheet_clicked(move |i| {
            flagging(&mut c.state.borrow_mut().prepare.frames, usize::try_from(i).unwrap_or(usize::MAX));
            c.publish_prepare();
        });
        let c = me.clone();
        p.on_cancel(move || c.roll_cancel());
        let c = me.clone();
        p.on_back(move || {
            let (step, capture) = {
                let r = &c.state.borrow().prepare;
                (r.step, r.capture)
            };
            c.step_to(neighbour(step, -1, capture));
        });
        let c = me.clone();
        p.on_advance(move || c.roll_advance());
        let c = me.clone();
        p.on_card_resized(move || c.publish_prepare());
        let c = me.clone();
        p.on_window_grabbed(move |h| c.window_grabbed(h));
        let c = me.clone();
        p.on_window_dragged(move |_, dx, dy| c.window_dragged(f64::from(dx), f64::from(dy)));
        let c = me.clone();
        p.on_window_released(move || c.state.borrow_mut().prepare.window_gesture = None);
        let c = me.clone();
        p.on_base_previous(move || c.walk_base(-1));
        let c = me.clone();
        p.on_base_next(move || c.walk_base(1));
        let c = me.clone();
        p.on_base_resized(move || c.publish_prepare());
        let c = me.clone();
        p.on_base_hover(move |x, y, on| {
            c.state.borrow_mut().prepare.pointer = on.then_some((f64::from(x), f64::from(y)));
            c.publish_base();
        });
        let c = me.clone();
        p.on_base_clicked(move |x, y| c.base_clicked(f64::from(x), f64::from(y)));
        let c = me.clone();
        window.global::<crate::PanelState>().on_prepare_roll(move || c.prepare_roll());
    }
}
