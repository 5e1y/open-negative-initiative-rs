//! `ONI_BENCH=crop`: the Crop tab's keys and the frame's handles, Esc in a drag, every step undone.

use super::*;

/// The Crop tab's keys and the canvas's crop frame driven through their own callbacks, each step's
/// geometry read back, then every step undone.
pub(super) fn crop(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let step = Cell::new(0usize);
    let done = RefCell::new(Some(done));
    let lines = RefCell::new(Vec::<String>::new());
    let start = RefCell::new(None::<oni_settings::frame::Geometry>);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let (f, c, panel) = (w.global::<crate::Framing>(), w.global::<crate::Canvas>(), w.global::<crate::PanelState>());
        let geometry = || ctl.state.borrow().store().settings().geometry.clone();
        let note = |what: &str| {
            let g = geometry();
            lines.borrow_mut().push(format!("{what}: crop {:.3} {:.3} {:.3} {:.3}, turns {}, angle {:.2}, mirrored {}, ratio {:?}, kept {}", g.crop.x, g.crop.y, g.crop.width, g.crop.height, g.quarter_turns, g.angle, g.mirrored, g.ratio, f.get_kept()));
        };
        let n = step.get();
        step.set(n + 1);
        match n {
            0 => {
                if ctl.state.borrow().renders() < 1 || ctl.state.borrow().placed().is_none() {
                    step.set(0);
                    return;
                }
                *start.borrow_mut() = Some(geometry());
                panel.set_tab(1);
                panel.invoke_tab_picked(1);
            }
            1 => {
                note(&format!("framing {}", if c.get_cropping() { "on" } else { "off" }));
                f.invoke_ratio_picked(3);
            }
            2 => {
                note("1:1");
                f.invoke_act("rotate".into());
            }
            3 => {
                note("rotated");
                f.invoke_act("mirror".into());
            }
            4 => {
                note("mirrored");
                // `CropOverlay`'s Esc: the held corner back where the press found it, the rest of the drag spent.
                let pressed = geometry().crop;
                c.invoke_frame_grabbed(false, 4);
                c.invoke_frame_dragged(false, 4, -40.0, -20.0);
                let moved = geometry().crop != pressed;
                let escape: slint::SharedString = slint::platform::Key::Escape.into();
                w.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: escape.clone() });
                w.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: escape });
                c.invoke_frame_dragged(false, 4, -60.0, -30.0);
                c.invoke_frame_released(false);
                lines.borrow_mut().push(format!("Esc in a drag that moved the corner ({moved}) put the frame back: {}", geometry().crop == pressed));
                c.invoke_frame_grabbed(false, 4);
                for k in 1..=10 {
                    c.invoke_frame_dragged(false, 4, -4.0 * k as f32, -2.0 * k as f32);
                }
                c.invoke_frame_released(false);
            }
            5 => {
                note("bottom-right corner dragged in");
                let g = w.global::<StoreGlobal>();
                let field = Field::Angle.index();
                g.invoke_press(field, false);
                for k in 1..=10 {
                    g.invoke_drag(field, k as f32 * 2.0, SPAN, false);
                }
                g.invoke_release(field);
            }
            6 => note("straightened"),
            7..=12 => ctl.act("undo"),
            13 => {
                note("undone");
                let back = start.borrow().clone() == Some(geometry());
                timer.stop();
                panel.set_tab(2);
                panel.invoke_tab_picked(2);
                let report = format!("bench, crop:\n{}\nundone to the frame it started from: {back}", lines.borrow().join("\n"));
                if let Some(done) = done.borrow_mut().take() {
                    done(report);
                }
            }
            _ => {}
        }
    });
}
