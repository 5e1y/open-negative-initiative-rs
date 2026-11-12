//! The canvas by the keyboard: Tab from the photos walks the bar, the sidebar, the canvas, then the
//! panel, whose sliders stay in the tree four frames together; the canvas holding the keyboard is read
//! as the photo, its arrows place the measuring window, the crop and their right and bottom edges,
//! Return keeps them; Prepare's floor places the base's picker and the measuring window alike.

use oni_app::PanelTab;
use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::app::{Options, Shell};
use oni_app::{Bar, Canvas, Framing, Library, PanelState, Preparing};
use oni_core::{Archive, EditorStore};
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, SharedString};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;
use crate::panel_bindings::opened;

fn arrow(key: Key) -> SharedString {
    key.into()
}

/// A key as winit hands it over: the window's key hook first, then Slint.
fn press(rig: &common::Rig, w: &oni_app::MainWindow, text: &str) {
    oni_app::outside::keyed(w.window(), || w.invoke_refocus());
    rig.key(text);
}

/// The middle of what holds the keyboard, in window points.
fn focused_middle(w: &oni_app::MainWindow) -> Option<(f32, f32)> {
    let inner = i_slint_core::window::WindowInner::from_pub(w.window());
    let item = inner.focus_item.borrow().upgrade()?;
    let (at, size) = (item.map_to_window(Default::default()), item.geometry().size);
    Some((at.x + size.width / 2.0, at.y + size.height / 2.0))
}

/// Tab from the photos until the canvas holds the keyboard, read as the photo it shows.
fn tab_to_canvas(rig: &common::Rig, w: &oni_app::MainWindow, label: &str) {
    w.invoke_refocus();
    for _ in 0..120 {
        press(rig, w, "\t");
        if common::focused_label(w) == label {
            return;
        }
    }
    panic!("Tab never reached the canvas read as {label:?}");
}

/// A shell on `count` graded frames, the first opened in the editor and rendered.
fn editor(name: &str, count: usize) -> Option<(common::Rig, Shell, PathBuf)> {
    let gpu = oni_app::render::Device::open().map_err(|e| eprintln!("{e}: the canvas's keys are not exercised")).ok()?;
    let root = std::env::temp_dir().join(format!("oni-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for i in 0..count {
        common::graded_tiff(&folder.join(format!("{}.tif", char::from(b'A' + i as u8))), 300, 200, 0.9 - 0.15 * i as f32);
    }
    let rig = common::rig(std::env::temp_dir());
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support, caches, picker: Box::new(|| None), require_roll: name.starts_with("prepare"), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let lib = shell.window.global::<Library>();
    let first = oni_app::app::tiles(&shell.window).into_iter().min_by_key(|t| t.index)?.index;
    lib.invoke_tile_clicked(first, false, false);
    if !name.starts_with("prepare") {
        lib.invoke_tile_clicked(first, false, false);
        rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    }
    rig.settle();
    Some((rig, shell, root))
}

/// The regions Tab walks from the photos, each named as it is entered, until it comes round; and
/// whether the canvas itself took the keyboard, read as the photo.
fn walk(rig: &common::Rig, w: &oni_app::MainWindow, back: bool) -> (Vec<&'static str>, bool) {
    let photo = w.global::<Canvas>().get_status().to_string();
    // The canvas's own place, in the points the focus is mapped to: the bar above it, the columns beside.
    let canvas = common::the(w, Role::Image, &photo);
    let (mut regions, mut read) = (Vec::new(), false);
    w.invoke_refocus();
    for _ in 0..160 {
        oni_app::outside::keyed(w.window(), || w.invoke_refocus());
        if back { rig.chord(Key::Shift, "\t") } else { rig.key("\t") }
        let Some((x, y)) = focused_middle(w) else { continue };
        read |= common::focused_label(w) == photo;
        let region = if y < canvas.y { "bar" } else if x < canvas.x { "sidebar" } else if x >= canvas.x + canvas.width { "panel" } else { "canvas" };
        if regions.last() != Some(&region) {
            regions.push(region);
        }
        if regions.len() == 5 {
            break;
        }
    }
    (regions, read)
}

pub(crate) fn tab_walks_the_bar_the_sidebar_the_canvas_then_the_panel() {
    let Some((rig, shell, root)) = editor("canvas-tab", 4) else { return };
    let w = &shell.window;
    let (forward, read) = walk(&rig, w, false);
    assert_eq!(forward, ["bar", "sidebar", "canvas", "panel", "bar"], "Tab from the photos walks the window from its top");
    assert!(read, "the canvas takes the keyboard, read as the photo");
    // Twin: ⇧Tab walks the same round the other way, so the order read is the walk's own.
    let (back, _) = walk(&rig, w, true);
    assert_eq!(back[..4], ["panel", "canvas", "sidebar", "bar"]);

    // Four frames together beside the library, in the evaluation's window: Lighting's sliders on screen
    // stay in the tree, however many elements come before them.
    rig.window.set_size(slint::PhysicalSize::new(1728, 1017));
    w.invoke_refocus();
    rig.chord(Key::Control, "a");
    rig.until("four frames together", Duration::from_secs(60), || w.global::<Canvas>().get_together());
    rig.settle();
    let exposure = common::placed(w, Role::Slider, "Exposure");
    assert_eq!(exposure.len(), 1, "Lighting's Exposure is in the tree, the library shown");
    println!("elements shown before Lighting's Exposure: {}", before_exposure(w));
    // Twin: Details shown, Colour's sliders leave the tree.
    let panel = w.global::<PanelState>();
    panel.set_tab(PanelTab::Details);
    panel.invoke_tab_picked(PanelTab::Details);
    rig.settle();
    assert!(common::placed(w, Role::Slider, "Exposure").is_empty(), "twin: a hidden pane's slider is not read");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

/// How many elements a walk of the whole tree meets before the panel's Exposure slider.
fn before_exposure(w: &oni_app::MainWindow) -> usize {
    use i_slint_backend_testing::ElementRoot;
    let all = w.root_element().query_descendants().match_descendants().find_all();
    all.iter().position(|e| e.accessible_role() == Some(Role::Slider) && e.accessible_label().is_some_and(|l| l == "Exposure")).unwrap_or(all.len())
}

pub(crate) fn the_canvas_holding_the_keyboard_places_the_window_the_crop_and_their_edges() {
    let Some((rig, shell, root)) = opened("canvas-keys") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, framing, canvas) = (w.global::<PanelState>(), w.global::<Framing>(), w.global::<Canvas>());
    let geometry = || ctl.state.borrow().store().settings().geometry.clone();
    let (right, down) = (arrow(Key::RightArrow), arrow(Key::DownArrow));
    let photo = canvas.get_status().to_string();
    let element = || ElementHandle::find_by_accessible_label(w, &photo).find(|e| e.accessible_role() == Some(Role::Image)).expect("the canvas read as the photo");

    // A frame never balanced opens with its window drawn on Colour; the photos' arrow leaves it.
    assert!(framing.get_armed() && canvas.get_window_shown() && panel.get_tab() == PanelTab::Color);
    let window = geometry().measure_window.expect("a window proposed");
    press(&rig, w, &right);
    assert_eq!(geometry().measure_window, Some(window), "twin: the photos holding the keyboard, the arrow moves no window");
    // Tab to the canvas: its arrows move the window, ⌥ its right or bottom edge, and it says so.
    tab_to_canvas(&rig, w, &photo);
    assert!(element().accessible_description().is_some_and(|d| d.starts_with("Arrows move the measuring window")));
    press(&rig, w, &right);
    let moved = geometry().measure_window.unwrap();
    assert!(moved.x > window.x && (moved.width - window.width).abs() < 1e-9, "the arrow moves the window: {moved:?}");
    rig.chord(Key::Alt, &down);
    let taller = geometry().measure_window.unwrap();
    assert!(taller.height > moved.height && (taller.y + taller.height - moved.y - moved.height).abs() < 1e-9, "⌥↓ lowers its bottom edge, the top kept: {taller:?}");
    rig.shot("canvas-keys-window");
    // The canvas's default action is Return's: the window applied.
    element().invoke_accessible_default_action();
    rig.pump();
    assert!(!framing.get_armed() && !canvas.get_window_shown(), "the canvas's action applies the window");

    // In the crop its arrows move the crop, ⌥→ its right edge alone; Return keeps it and leaves.
    rig.chord(Key::Control, "k");
    framing.invoke_ratio_picked(0);
    rig.settle();
    assert_eq!(panel.get_tab(), PanelTab::Crop);
    tab_to_canvas(&rig, w, &canvas.get_status());
    let crop = geometry().crop;
    rig.chord(Key::Alt, &arrow(Key::LeftArrow));
    let narrower = geometry().crop;
    assert!(narrower.width < crop.width && (narrower.x - crop.x).abs() < 1e-9, "⌥← draws the right edge in, the left kept: {narrower:?}");
    press(&rig, w, &right);
    let shifted = geometry().crop;
    assert!(shifted.x > narrower.x && (shifted.width - narrower.width).abs() < 1e-9, "twin: the bare arrow moves the left edge with it: {shifted:?}");
    press(&rig, w, "\n");
    assert_eq!((panel.get_tab(), geometry().crop), (PanelTab::Color, shifted), "Return keeps the crop and leaves it");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn prepares_floor_places_the_base_picker_and_the_window_by_the_keyboard() {
    let Some((rig, shell, root)) = editor("prepare-keys", 3) else { return };
    let w = &shell.window;
    let p = w.global::<Preparing>();
    let right = arrow(Key::RightArrow);
    w.global::<Bar>().invoke_act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(30), || p.get_shown());
    p.invoke_step_picked(2);
    rig.until("the scan read", Duration::from_secs(60), || p.get_scan_ready());
    rig.settle();

    // The photos' arrow places nothing; Tab to the floor and its arrows walk the picker, its loupe with it.
    press(&rig, w, &right);
    assert!(!p.get_loupe_shown(), "twin: the photos holding the keyboard, no picker moves");
    tab_to_canvas(&rig, w, &p.get_base_name());
    press(&rig, w, &right);
    let (x, y) = (p.get_loupe_x(), p.get_loupe_y());
    let middle = (p.get_scan_x() + p.get_scan_width() / 2.0, p.get_scan_y() + p.get_scan_height() / 2.0);
    assert!(p.get_loupe_shown() && (x - middle.0 - 1.0).abs() < 0.01 && (y - middle.1).abs() < 0.01, "the picker starts mid-scan: {x}, {y}");
    rig.chord(Key::Shift, &right);
    assert!((p.get_loupe_x() - x - 10.0).abs() < 0.01, "⇧ moves it ten times as far");
    assert!(!p.get_ring_shown());
    let picker = p.get_loupe_x();
    press(&rig, w, "\n");
    assert_eq!(p.get_step(), 3, "Return picks the film under the picker and goes on");
    p.invoke_step_picked(2);
    rig.pump();
    assert!(p.get_ring_shown() && (p.get_ring_x() - picker).abs() < 0.01, "the pick lies where the picker stood");
    rig.shot("canvas-keys-base");

    // On the window step its arrows move the window, ⌥ its right edge; the photos' arrow walks the frames.
    p.invoke_step_picked(4);
    rig.settle();
    // Drawn first, so the frame card is laid and its window placed.
    let _ = rig.drawn();
    let (wx, ww, name) = (p.get_window_x(), p.get_window_width(), p.get_frame_name().to_string());
    tab_to_canvas(&rig, w, &name);
    press(&rig, w, &right);
    assert!((p.get_window_x() - wx - 1.0).abs() < 0.01 && (p.get_window_width() - ww).abs() < 0.01, "the arrow moves the window");
    rig.chord(Key::Alt, &right);
    assert!((p.get_window_x() - wx - 1.0).abs() < 0.01 && (p.get_window_width() - ww - 1.0).abs() < 0.01, "⌥→ draws its right edge out");
    w.invoke_refocus();
    press(&rig, w, &right);
    assert!(p.get_frame_name() != name && (p.get_window_x() - wx - 1.0).abs() < 0.01, "twin: the photos' arrow walks the frames");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
