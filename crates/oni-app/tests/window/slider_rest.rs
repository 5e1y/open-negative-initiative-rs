//! One slider mechanic on both faces, through the window's pointer: a drag reaching rest holds there
//! for a few points and leaves it without a jump, and a double click on the name brings it back.

use i_slint_backend_testing::AccessibleRole;
use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{App, Library, Store};
use oni_core::{Archive, EditorStore, Knob};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn press(rig: &common::Rig, at: LogicalPosition) {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
}

fn release(rig: &common::Rig, at: LogicalPosition) {
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
}

pub(crate) fn a_drag_holds_at_rest_and_a_double_click_on_the_name_brings_it_back() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-slider-rest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Roll");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("A.NEF"), b"not a raw").unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2 * 1280, 2 * 800));
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &Path| true));
    let options = Options { support: root.join("Support"), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    shell.open_folder(&folder);
    rig.until("the library read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    w.global::<Library>().invoke_tile_opened(0);
    rig.pump();
    assert!(!w.global::<App>().get_gallery(), "the tile opens in the editor, on Pro");
    // The RAW notice is modal: closed first, so the pointer reaches the panel.
    w.global::<oni_app::Notice>().invoke_close();
    rig.settle();

    // Pro's Temperature rests at its centre, ticked on its rail, as the Scanner's letters rest at zero.
    let g = w.global::<Store>();
    let field = Field::Temperature;
    let rest = g.get_rests().row_data(field.index() as usize).unwrap();
    assert!((rest - 0.5).abs() < 1e-6, "Temperature rests mid-track: {rest}");
    let temperature = || Knob::Temperature.get(ctl.state.borrow().store().settings());
    assert_eq!(temperature(), 0.0);
    let slider = common::the(w, AccessibleRole::Slider, "Temperature");
    // The rail runs inside the knob's half width either side, the knob on the slider's lowest row.
    let (left, travel) = (slider.x + 10.0, slider.width - 20.0);
    let y = slider.y + slider.height - 13.0;
    let knob = LogicalPosition::new(left + rest * travel, y);
    press(&rig, knob);
    let mut shown = Vec::new();
    for dx in 1..=20 {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(knob.x + dx as f32, y) });
        rig.pump();
        shown.push(oni_app::app::fraction(&g, field));
    }
    release(&rig, LogicalPosition::new(knob.x + 20.0, y));
    // Six points of the hand held on rest, the readout at zero; then the knob goes on a point a point.
    let points: Vec<f32> = shown.iter().map(|f| (f - rest) * travel).collect();
    assert!(points[..6].iter().all(|p| p.abs() < 1e-3), "the drag holds at rest: {points:?}");
    assert!((points[6] - 1.0).abs() < 0.05, "and leaves it without a jump: {points:?}");
    assert!((points[19] - 14.0).abs() < 0.05, "the hand's twenty points less the detent's six: {points:?}");
    // Twin: a knob left under the hand would stand twenty points past rest.
    assert!((points[19] - 20.0).abs() > 5.0);
    let moved = temperature();
    assert!(moved > 0.0, "the release writes the drag's value");

    // A double click on the name brings it back to rest; twin: a single click leaves it.
    let name = common::placed(w, AccessibleRole::Text, "Temperature");
    let name = name.iter().find(|p| p.y >= slider.y - 1.0 && p.y < slider.y + 20.0).copied().expect("the slider's name");
    let (x, y) = name.middle();
    let at = LogicalPosition::new(x, y);
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    press(&rig, at);
    release(&rig, at);
    assert_eq!(temperature(), moved, "a single click on the name moves nothing");
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    press(&rig, at);
    release(&rig, at);
    press(&rig, at);
    release(&rig, at);
    rig.settle();
    assert_eq!(temperature(), 0.0, "the name's double click brings Temperature back to rest");
    println!("shot {}", rig.shot("slider-rest").display());
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
