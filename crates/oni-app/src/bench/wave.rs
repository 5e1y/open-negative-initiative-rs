//! `ONI_BENCH=wave`: a roll never prepared opened from the gallery at the screen's size, its frames laid
//! on Prepare's sheet at once under Reduce Motion, then landing as a wave; the interface's frames timed
//! over each, Prepare cancelled after, which writes nothing.

use super::*;

/// What one opening is watched for: the wave's last cell landed, and a breath after.
const WATCHED: Duration = Duration::from_millis(1500);
/// How long a step may wait for the window before the bench gives up, saying where.
const STALL: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
enum Step {
    Loading,
    Leaving { since: Instant },
    Gallery { pass: usize, since: Instant },
    Opening { pass: usize },
}

pub(super) fn wave(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let step = Cell::new(Step::Loading);
    let since = Cell::new(Instant::now());
    let lines = RefCell::new(Vec::<String>::new());
    let done = RefCell::new(Some(done));
    let drawn = frame_clock(ctl);
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let Some(w) = ctl.window() else { return };
        let finish = |text: String| {
            held.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        let prepare = w.global::<crate::Preparing>();
        let lib = w.global::<crate::Library>();
        let stalled = |why: &str| {
            if since.get().elapsed() > STALL {
                finish(format!("wave: stalled, {why}\n{}", lines.borrow().join("\n")));
            }
        };
        match step.get() {
            // The first frame opened by itself is the roll's, never prepared: Prepare shows, and leaves.
            Step::Loading => {
                if prepare.get_shown() {
                    maximise(&w);
                    step.set(Step::Leaving { since: Instant::now() });
                } else if started.elapsed() > LOADING {
                    finish("wave: Prepare never showed; open a roll never prepared".into());
                }
            }
            Step::Leaving { since: at } => {
                if at.elapsed() < Duration::from_secs(2) {
                    return;
                }
                let size = w.window().size();
                lines.borrow_mut().push(format!("window: {} x {} px", size.width, size.height));
                prepare.invoke_cancel();
                since.set(Instant::now());
                step.set(Step::Gallery { pass: 0, since: Instant::now() });
            }
            Step::Gallery { pass, since: at } => {
                let laid = slint::Model::iter(&lib.get_tiles()).filter(|t| t.width > 0.0 && t.picture.size().width > 0).count();
                if prepare.get_shown() || !w.global::<crate::App>().get_gallery() || laid < 12 || at.elapsed() < Duration::from_secs(2) {
                    return stalled("the gallery never laid its thumbnails");
                }
                w.global::<crate::Motion>().set_reduced(pass == 0);
                let Some(t) = slint::Model::iter(&lib.get_tiles()).find(|t| t.width > 0.0) else { return };
                since.set(Instant::now());
                lib.invoke_tile_clicked(t.index, false, false);
                lib.invoke_tile_clicked(t.index, false, false);
                step.set(Step::Opening { pass });
            }
            Step::Opening { pass } => {
                if !prepare.get_shown() {
                    return stalled("the double click opened no Prepare");
                }
                if since.get().elapsed() < WATCHED {
                    return;
                }
                let how = if pass == 0 { "laid at once" } else { "as a wave" };
                lines.borrow_mut().push(format!("gallery to Prepare, {} cells, {how}: {}", slint::Model::row_count(&prepare.get_cells()), frames_since(&drawn.borrow(), since.get())));
                prepare.invoke_cancel();
                if pass == 0 {
                    since.set(Instant::now());
                    step.set(Step::Gallery { pass: 1, since: Instant::now() });
                } else {
                    w.global::<crate::Motion>().set_reduced(false);
                    finish(format!("wave:\n{}", lines.borrow().join("\n")));
                }
            }
        }
    });
}
