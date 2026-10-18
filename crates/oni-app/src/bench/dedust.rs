//! `ONI_BENCH=dedust`: a stroke painted in De-dust, its patch waited for, then removed.

use super::*;

/// De-dust through its own key, the brush keys and the canvas's painting callbacks; the patch
/// waited for, then the correction removed and the tool left.
pub(super) fn dedust(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let step = Cell::new(0usize);
    let done = RefCell::new(Some(done));
    let lines = RefCell::new(Vec::<String>::new());
    let painted = Cell::new(None::<Instant>);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let (d, c) = (w.global::<crate::Dedust>(), w.global::<crate::Canvas>());
        let corrections = || ctl.state.borrow().store().settings().corrections.clone();
        let n = step.get();
        step.set(n + 1);
        match n {
            0 => {
                if ctl.state.borrow().renders() < 1 || ctl.state.borrow().placed().is_none() {
                    step.set(0);
                    return;
                }
                ctl.act("correction");
                ctl.act("bigger-brush");
                ctl.act("bigger-brush");
            }
            1 => {
                lines.borrow_mut().push(format!("painting {}, brush {}", c.get_painting(), d.get_readout()));
                // A short stroke across the middle of the frame shown.
                let (x, y) = (c.get_frame_x() + c.get_frame_width() * 0.4, c.get_frame_y() + c.get_frame_height() * 0.45);
                c.invoke_paint_start(x, y);
                for k in 1..=8 {
                    c.invoke_paint_move(x + k as f32 * 3.0, y + k as f32);
                }
                c.invoke_paint_end();
                painted.set(Some(Instant::now()));
                let list = corrections();
                lines.borrow_mut().push(format!("{} correction, radius {:.0}, {} points, marks {}", list.len(), list.first().map_or(0.0, |c| c.radius), list.first().map_or(0, |c| c.stroke.len()), c.get_marks().row_count()));
            }
            2 => {
                let list = corrections();
                let Some(first) = list.first() else { return finish(timer, &done, &lines, "no correction to wait on") };
                if !crate::patches::file(&crate::patches::caches(), first.id).exists() {
                    if painted.get().is_some_and(|t| t.elapsed() > Duration::from_secs(30)) {
                        return finish(timer, &done, &lines, "the patch never came");
                    }
                    step.set(2);
                    return;
                }
                let row = d.get_rows().row_data(0);
                lines.borrow_mut().push(format!(
                    "patch generated {:.0} ms after the stroke; row {:?}, pending {:?}",
                    painted.get().map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0),
                    row.as_ref().map(|r| r.label.to_string()),
                    row.as_ref().map(|r| r.pending)
                ));
            }
            3 => {
                let id = corrections().first().map(|c| c.id);
                d.invoke_remove(0);
                let gone = id.is_some_and(|id| !crate::patches::file(&crate::patches::caches(), id).exists());
                lines.borrow_mut().push(format!("removed: {} corrections left, patch gone {gone}", corrections().len()));
                ctl.act("correction");
                lines.borrow_mut().push(format!("left De-dust: painting {}", c.get_painting()));
                ctl.flush();
                finish(timer, &done, &lines, "done");
            }
            _ => {}
        }
    });
}

pub(super) fn finish(timer: &Timer, done: &RefCell<Option<impl FnOnce(String)>>, lines: &RefCell<Vec<String>>, last: &str) {
    timer.stop();
    if let Some(done) = done.borrow_mut().take() {
        done(format!("bench, dedust:\n{}\n{last}", lines.borrow().join("\n")));
    }
}
