//! `ONI_BENCH=prepare`: the roll prepared through its steps, measured, landed and undone.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Prep {
    Loading,
    Picked,
    Entering,
    Walking,
    Base,
    Flags,
    Window,
    Measuring,
    Landed,
}

/// The panel's step key, at the footer's trailing end.
pub(super) fn action_key(p: &crate::Preparing) -> slint::LogicalPosition {
    slint::LogicalPosition::new(p.get_footer_x() + p.get_footer_width() - 12.0, p.get_footer_y() + p.get_footer_height() / 2.0)
}

pub(super) fn prepare(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Prep::Loading);
    let done = RefCell::new(Some(done));
    let lines = RefCell::new(Vec::<String>::new());
    let started = Instant::now();
    let since = Cell::new(Instant::now());
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let p = w.global::<crate::Preparing>();
        let lib = w.global::<crate::Library>();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(format!("bench, prepare:\n{}\n{text}", lines.borrow().join("\n")));
            }
        };
        let note = |what: String| {
            lines.borrow_mut().push(format!("{what}: {:.0} ms", since.get().elapsed().as_secs_f64() * 1000.0));
            since.set(Instant::now());
        };
        let stuck = |what: &str| since.get().elapsed() > Duration::from_secs(120) && { finish(format!("prepare: stuck {what} at step {}", p.get_step())); true };
        match phase.get() {
            Prep::Loading => {
                if ctl.state.borrow().renders() >= 1 {
                    ctl.act("toggle-mode");
                    since.set(Instant::now());
                    phase.set(Prep::Picked);
                } else if started.elapsed() > LOADING {
                    finish("prepare: no frame rendered".into());
                }
            }
            Prep::Picked => {
                let Some(t) = tile(&w, 0).filter(|_| lib.get_grid_x() > 0.0) else { return };
                click_at(&w, slint::LogicalPosition::new(lib.get_grid_x() + t.x + t.width / 2.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0));
                w.global::<crate::Bar>().invoke_act("prepare-roll".into());
                phase.set(Prep::Entering);
            }
            Prep::Entering => {
                if p.get_shown() && ctl.state.borrow().prepare.read_whole() {
                    note(format!("{} frames entered and read", p.get_cells().row_count()));
                    p.invoke_step_picked(0);
                    phase.set(Prep::Walking);
                } else {
                    stuck("entering");
                }
            }
            Prep::Walking => {
                if p.get_step() == 2 {
                    phase.set(Prep::Base);
                } else {
                    click_at(&w, action_key(&p));
                }
            }
            Prep::Base => {
                if !p.get_scan_ready() {
                    stuck("on the base's scan");
                    return;
                }
                note("the base's scan shown".into());
                // The scan's top-left rebate, where a holder leaves bare film.
                let at = slint::LogicalPosition::new(p.get_base_origin_x() + p.get_scan_x() + p.get_scan_width() * 0.02, p.get_base_origin_y() + p.get_scan_y() + p.get_scan_height() * 0.5);
                pointer_to(&w, at);
                click_at(&w, at);
                phase.set(Prep::Flags);
            }
            Prep::Flags => {
                if p.get_step() != 3 {
                    stuck("picking the base");
                    return;
                }
                note(format!("base picked, {}", p.get_base_note()));
                let width = (p.get_sheet_width() - 2.0 * 24.0 - 5.0 * 16.0) / 6.0;
                click_at(&w, slint::LogicalPosition::new(p.get_sheet_origin_x() + 24.0 + width / 2.0, p.get_sheet_origin_y() + 24.0 + width / 3.0));
                lines.borrow_mut().push(format!("flagged the first frame: {}", p.get_flags_note()));
                click_at(&w, action_key(&p));
                phase.set(Prep::Window);
            }
            Prep::Window => {
                if p.get_step() != 4 {
                    stuck("reaching the window");
                    return;
                }
                lines.borrow_mut().push(p.get_action_title().to_string());
                since.set(Instant::now());
                click_at(&w, action_key(&p));
                phase.set(Prep::Measuring);
            }
            Prep::Measuring => {
                if !p.get_shown() {
                    let app = w.global::<crate::App>();
                    note(format!("measured and landed, the editor {}", if app.get_gallery() { "not open" } else { "open" }));
                    phase.set(Prep::Landed);
                } else if !stuck("measuring") && w.global::<crate::Advice>().get_shown() {
                    finish(format!("prepare: {}", w.global::<crate::Advice>().get_message()));
                }
            }
            Prep::Landed => {
                ctl.act("toggle-mode");
                let label = w.global::<crate::Menus>().get_titles().undo.to_string();
                // Taken back, so the test copy reads its own measure again.
                ctl.act("undo");
                finish(format!("prepare: {label} landed and undone in {:.1} s, no failure", started.elapsed().as_secs_f64()));
            }
        }
    });
}
