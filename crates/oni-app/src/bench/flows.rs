//! `ONI_BENCH=flows`: the owner's flow through the window's pointer events: pick, double-click, edit, back.

use super::*;

/// The frames the flow opens from the gallery, by their place in it.
pub(super) const FLOW_TILES: [usize; 3] = [1, 4, 7];

#[derive(Clone, Copy)]
pub(super) enum Flow {
    Loading,
    Gallery { frame: usize },
    Picked { frame: usize },
    Opening { frame: usize, at: Instant },
    Editing { frame: usize, notch: usize },
    Shown { frame: usize, revision: u64 },
    Back { frame: usize, since: Instant },
    Below { name: usize },
}

/// A press and a release where the hand would put them, through the window's own event path.
pub(super) fn click_at(w: &crate::MainWindow, at: slint::LogicalPosition) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let window = w.window();
    window.dispatch_event(WindowEvent::PointerMoved { position: at });
    window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
}

/// The tile at place `i` in the gallery, when it is laid on screen.
pub(super) fn tile(w: &crate::MainWindow, i: usize) -> Option<crate::Tile> {
    crate::app::tiles(w).into_iter().find(|t| t.index as usize == i)
}

/// The tiles and the sidebar's photo rows lit, by name.
pub(super) fn lit(w: &crate::MainWindow) -> (Vec<String>, Vec<String>) {
    let lib = w.global::<crate::Library>();
    let tiles = crate::app::tiles(w);
    let rows = lib.get_rows();
    (
        tiles.into_iter().filter(|t| t.selected).map(|t| t.caption.to_string()).collect(),
        (0..rows.row_count()).filter_map(|k| rows.row_data(k)).filter(|r| r.kind == 2 && r.selected).map(|r| r.title.to_string()).collect(),
    )
}

pub(super) fn flows(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Flow::Loading);
    let done = RefCell::new(Some(done));
    let failures = RefCell::new(Vec::<String>::new());
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
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
        let fail = |what: String| failures.borrow_mut().push(what);
        let centre = |t: &crate::Tile| slint::LogicalPosition::new(lib.get_grid_x() + t.x + t.width / 2.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0);
        // One name lit in the grid and the same one, alone, in the sidebar.
        let check = |name: &str, when: &str| {
            let (tiles, rows) = lit(&w);
            if tiles != [name] || rows != [name] {
                fail(format!("{name} {when}: the gallery lights {tiles:?}, the sidebar {rows:?}"));
            }
        };
        match phase.get() {
            Flow::Loading => {
                if ctl.state.borrow().preparing() {
                    // `requireRoll`: a roll never prepared opens every frame on Prepare the Roll.
                    finish("flows: the roll was never prepared, so each frame opens on Prepare the Roll; run the flow on a prepared roll".into());
                } else if ctl.state.borrow().renders() >= 1 && ctl.state.borrow().placed().is_some() {
                    ctl.act(crate::Act::ToggleMode);
                    phase.set(Flow::Gallery { frame: 0 });
                } else if started.elapsed() > LOADING {
                    finish("flows: no frame rendered".into());
                }
            }
            Flow::Gallery { frame } => {
                let Some(t) = tile(&w, FLOW_TILES[frame]).filter(|_| app.get_gallery() && lib.get_grid_x() > 0.0) else { return };
                // The double-click interval apart from the last click, so this one starts afresh.
                click_at(&w, centre(&t));
                phase.set(Flow::Picked { frame });
            }
            Flow::Picked { frame } => {
                let Some(t) = tile(&w, FLOW_TILES[frame]) else { return };
                let name = t.caption.to_string();
                if !app.get_gallery() {
                    fail(format!("{name}: one click left the gallery"));
                }
                check(&name, "clicked once");
                // The second click of a double click, inside the system's interval.
                click_at(&w, centre(&t));
                phase.set(Flow::Opening { frame, at: Instant::now() });
            }
            Flow::Opening { frame, at } => {
                let name = tile(&w, FLOW_TILES[frame]).map(|t| t.caption.to_string()).unwrap_or_default();
                let open = ctl.state.borrow().open_path().is_some_and(|p| p.contains(&name));
                if !app.get_gallery() && open && ctl.state.borrow().placed().is_some() {
                    check(&name, "opened");
                    w.global::<StoreGlobal>().invoke_press(Field::Exposure.index(), false);
                    phase.set(Flow::Editing { frame, notch: 0 });
                } else if at.elapsed() > Duration::from_secs(60) {
                    fail(format!("{name}: a double click did not open it (gallery {}, open {:?})", app.get_gallery(), ctl.state.borrow().open_path()));
                    finish(format!("flows: {}", failures.borrow().join("; ")));
                }
            }
            Flow::Editing { frame, notch } => {
                let g = w.global::<StoreGlobal>();
                let notch = notch + 1;
                g.invoke_drag(Field::Exposure.index(), notch as f32 * STEP, SPAN, false);
                // The window resized mid-edit, as entering the editor resizes the canvas.
                let (width, height) = SIZES[notch % SIZES.len()];
                w.window().set_size(slint::LogicalSize::new(width, height));
                if notch < 12 {
                    phase.set(Flow::Editing { frame, notch });
                } else {
                    g.invoke_release(Field::Exposure.index());
                    phase.set(Flow::Shown { frame, revision: ctl.state.borrow().store().revision() });
                }
            }
            Flow::Shown { frame, revision } => {
                if ctl.state.borrow().rendered() < revision {
                    return;
                }
                // The edit undone, so the test copy's sidecar reads as it did.
                ctl.act(crate::Act::Undo);
                ctl.act(crate::Act::ToggleMode);
                phase.set(Flow::Back { frame, since: Instant::now() });
            }
            Flow::Back { frame, since } => {
                // The gallery lands on the open photo once it shows: the hand looks after it has.
                if since.elapsed() < Duration::from_millis(300) {
                    return;
                }
                let Some(t) = tile(&w, FLOW_TILES[frame]).filter(|_| app.get_gallery()) else { return };
                check(&t.caption, "back in the gallery");
                if frame + 1 < FLOW_TILES.len() {
                    phase.set(Flow::Gallery { frame: frame + 1 });
                } else {
                    // Under the last row, where the document ends: the picks stay as they were.
                    let below = slint::LogicalPosition::new(lib.get_grid_x() + 4.0, lib.get_grid_y() + lib.get_content_height() - lib.get_scroll() + 4.0);
                    click_at(&w, below);
                    phase.set(Flow::Below { name: FLOW_TILES[frame] });
                }
            }
            Flow::Below { name } => {
                if let Some(t) = tile(&w, name) {
                    check(&t.caption, "after a click under the last row");
                }
                let failed = failures.borrow();
                finish(if failed.is_empty() {
                    format!("flows: {} frames picked, double-clicked open and edited through resizes, a click under the rows changed nothing, no failure", FLOW_TILES.len())
                } else {
                    format!("flows: {}", failed.join("; "))
                });
            }
        }
    });
}
