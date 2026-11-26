//! `ONI_BENCH=motion`: the window the size of its screen, each of the interface's journeys walked twice,
//! under Reduce Motion, each screen laid at once, then moving: the gallery, a tile opened by a double
//! click, the panel's panes taken, the export opened and closed, the interface's frames timed over each.

use super::*;

/// What a journey's frames are counted over: the travel's pace and its landing, and a breath after.
const WATCHED: Duration = Duration::from_millis(900);
/// How long a step may wait for the window to reach it before the bench gives up.
const STALL: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Loading,
    Sizing { since: Instant },
    Gallery,
    Picked,
    Pane { at: usize },
    Opened,
    Exported,
    Closed,
}

/// The panel's panes taken from the tab bar, crossing it both ways.
const PANES: [crate::PanelTab; 3] = [crate::PanelTab::Details, crate::PanelTab::Roll, crate::PanelTab::Color];

fn pick(w: &crate::MainWindow, tab: crate::PanelTab) {
    let panel = w.global::<crate::PanelState>();
    panel.set_tab(tab);
    panel.invoke_tab_picked(tab);
}

pub(super) fn motion(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let step = Cell::new(Step::Loading);
    let pass = Cell::new(0usize);
    let since = Cell::new(Instant::now());
    let lines = RefCell::new(Vec::<String>::new());
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let drawn = frame_clock(ctl);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let app = w.global::<crate::App>();
        let lib = w.global::<crate::Library>();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        // Each journey watched for its whole pace before the next starts.
        let watched = || since.get().elapsed() >= WATCHED;
        // A step waiting past its due ends the bench, saying where: a bench never waits without end.
        let stalled = |why: &str| {
            if since.get().elapsed() > STALL {
                finish(format!("motion: stalled, {why}\n{}", lines.borrow().join("\n")));
            }
        };
        let journey = |name: &str| {
            let moving = if pass.get() == 0 { "laid at once" } else { "moving" };
            lines.borrow_mut().push(format!("{name}, {moving}: {}", frames_since(&drawn.borrow(), since.get())));
            since.set(Instant::now());
        };
        match step.get() {
            Step::Loading => {
                if ctl.state.borrow().renders() >= 1 && ctl.state.borrow().placed().is_some() {
                    w.window().set_maximized(true);
                    step.set(Step::Sizing { since: Instant::now() });
                } else if started.elapsed() > LOADING {
                    finish("motion: no frame rendered".into());
                }
            }
            Step::Sizing { since: at } => {
                if at.elapsed() < Duration::from_secs(2) {
                    return;
                }
                let size = w.window().size();
                lines.borrow_mut().push(format!("window: {} x {} px", size.width, size.height));
                w.global::<crate::Motion>().set_reduced(true);
                since.set(Instant::now());
                ctl.act(crate::Act::ToggleMode);
                step.set(Step::Gallery);
            }
            Step::Gallery => {
                // A tile laid on screen in the open photo's folder, which opened in the editor: a photo of a
                // roll never prepared opens on Prepare the Roll instead.
                let t = {
                    let s = ctl.state.borrow();
                    let open = s.open_path().map(std::path::Path::new);
                    let folder = open.and_then(std::path::Path::parent);
                    let beside = |t: &crate::Tile| {
                        let path = s.order().get(t.index as usize).map(std::path::Path::new);
                        t.width > 0.0 && path != open && path.and_then(std::path::Path::parent) == folder
                    };
                    crate::app::tiles(&w).into_iter().filter(beside).nth(pass.get())
                };
                let Some(t) = t else { return stalled("no tile of the open photo's folder laid in the gallery") };
                // A gesture on the disk under way refuses an opening, as the launch's sweep of a large tree does.
                if ctl.state.borrow().disk_busy() {
                    return stalled("the disk stayed busy");
                }
                if !watched() || !app.get_gallery() {
                    return stalled("the gallery never showed");
                }
                journey("editor to gallery");
                // Opened as a double click's two clicks reach the grid, whatever lies over it on screen.
                lib.invoke_tile_clicked(t.index, false, false);
                lib.invoke_tile_clicked(t.index, false, false);
                step.set(Step::Picked);
            }
            Step::Picked => {
                if !watched() || app.get_gallery() {
                    let prepare = w.global::<crate::Preparing>().get_shown();
                    let (tiles, rows) = lit(&w);
                    let why = if prepare { "the double click opened Prepare the Roll".to_string() } else { format!("the double click opened no photo (lit {tiles:?} in the grid, {rows:?} in the column)") };
                    return stalled(&why);
                }
                journey("gallery to editor");
                pick(&w, PANES[0]);
                step.set(Step::Pane { at: 0 });
            }
            Step::Pane { at } => {
                if !watched() {
                    return;
                }
                journey(&format!("pane {:?}", PANES[at]));
                if at + 1 < PANES.len() {
                    pick(&w, PANES[at + 1]);
                    step.set(Step::Pane { at: at + 1 });
                } else {
                    ctl.act(crate::Act::Export);
                    step.set(Step::Opened);
                }
            }
            Step::Opened => {
                if !watched() {
                    return;
                }
                journey("editor to export");
                ctl.leave_export();
                step.set(Step::Exported);
            }
            Step::Exported => {
                if !watched() {
                    return;
                }
                journey("export to editor");
                step.set(Step::Closed);
            }
            Step::Closed => {
                if pass.get() == 0 {
                    pass.set(1);
                    w.global::<crate::Motion>().set_reduced(false);
                    since.set(Instant::now());
                    ctl.act(crate::Act::ToggleMode);
                    step.set(Step::Gallery);
                } else {
                    finish(format!("motion:\n{}", lines.borrow().join("\n")));
                }
            }
        }
    });
}
