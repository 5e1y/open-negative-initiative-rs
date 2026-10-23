//! `ONI_BENCH=dedust`: a stroke painted in De-dust, its patch waited for, then removed;
//! `ONI_DUST_STROKES=n` paints n strokes at the largest brush at once, as a hand clearing a frame.

use super::*;

/// The stroke's moves, a 1000 Hz mouse's half second.
const MOVES: usize = 500;

fn strokes() -> usize {
    std::env::var("ONI_DUST_STROKES").ok().and_then(|v| v.parse().ok()).unwrap_or(1).max(1)
}

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
                ctl.act(crate::Act::Correction);
                for _ in 0..if strokes() > 1 { 40 } else { 2 } {
                    ctl.act(crate::Act::BiggerBrush);
                }
            }
            1 => {
                lines.borrow_mut().push(format!("painting {}, brush {}", c.get_painting(), d.get_readout()));
                // Short strokes across the frame shown, the first in its middle.
                let n = strokes();
                let mut each = 0.0;
                for s in 0..n {
                    let (fx, fy) = if n == 1 { (0.4, 0.45) } else { (0.1 + 0.7 * (s % 6) as f32 / 5.0, 0.15 + 0.6 * (s / 6 % 6) as f32 / 5.0) };
                    let (x, y) = (c.get_frame_x() + c.get_frame_width() * fx, c.get_frame_y() + c.get_frame_height() * fy);
                    // Moves a point apart, as a 1000 Hz mouse sends them: each one's cost on the hand's thread.
                    c.invoke_paint_start(x, y);
                    let moved = Instant::now();
                    for k in 1..=MOVES {
                        c.invoke_paint_move(x + k as f32 * 0.25 / n as f32, y + (k as f32 * 0.05).sin() * 4.0);
                    }
                    each = moved.elapsed().as_secs_f64() * 1e6 / MOVES as f64;
                    c.invoke_paint_end();
                }
                lines.borrow_mut().push(format!("{MOVES} moves: {each:.1} us each on the hand's thread"));
                painted.set(Some(Instant::now()));
                let list = corrections();
                lines.borrow_mut().push(format!("{} correction, radius {:.0}, {} points, marks {}", list.len(), list.first().map_or(0.0, |c| c.radius), list.first().map_or(0, |c| c.stroke.len()), c.get_marks().row_count()));
            }
            2 => {
                let list = corrections();
                if list.is_empty() {
                    return finish(timer, &done, &lines, "no correction to wait on");
                }
                if !list.iter().all(|c| crate::patches::file(&crate::patches::caches(), c.id).exists()) {
                    if painted.get().is_some_and(|t| t.elapsed() > Duration::from_secs(120)) {
                        return finish(timer, &done, &lines, "the patch never came");
                    }
                    step.set(2);
                    return;
                }
                let row = d.get_rows().row_data(0);
                // The row's reminder of the dust lands beside the patch.
                if row.as_ref().is_some_and(|r| r.original.size().width == 0) && painted.get().is_some_and(|t| t.elapsed() < Duration::from_secs(30)) {
                    step.set(2);
                    return;
                }
                lines.borrow_mut().push(format!(
                    "{} patches and reminder {:.0} ms after the strokes; row {:?}, pending {:?}, reminder {:?}",
                    list.len(),
                    painted.get().map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0),
                    row.as_ref().map(|r| r.label.to_string()),
                    row.as_ref().map(|r| r.pending),
                    row.as_ref().map(|r| (r.original.size().width, r.original.size().height))
                ));
            }
            3 => {
                let id = corrections().first().map(|c| c.id);
                for _ in 0..corrections().len() {
                    d.invoke_remove(0);
                }
                let gone = id.is_some_and(|id| !crate::patches::file(&crate::patches::caches(), id).exists());
                lines.borrow_mut().push(format!("removed: {} corrections left, patch gone {gone}", corrections().len()));
                ctl.act(crate::Act::Correction);
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
