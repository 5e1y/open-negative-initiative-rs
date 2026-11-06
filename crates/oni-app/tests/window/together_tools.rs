//! The editor's tools on frames shown together: Compare Before/After splits the whole sheet, the
//! before drawn on each frame's own before; the face and the proof redraw every tile; the crop, the
//! measuring window and De-dust, each frame's own, cannot be opened.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Bar, Canvas, Exporting, Library, Menus, PanelState, Store as StoreGlobal};
use oni_settings::{Face, sidecar};
use oni_core::{Archive, EditorStore};
use slint::{ComponentHandle, Model};
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn compare_splits_the_sheet_and_the_crop_waits_for_one_frame() {
    let face = |p: &std::path::Path| sidecar::read(p, false, true).map(|l| l.settings.face).ok();
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Edit Together's tools are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-together-tools-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5)] {
        common::graded_tiff(&folder.join(name), 150, 100, tone);
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, canvas, panel, bar) = (w.global::<Library>(), w.global::<Canvas>(), w.global::<PanelState>(), w.global::<Bar>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| oni_app::app::tiles(w).iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    lib.invoke_tile_clicked(tile("A"), false, false);
    lib.invoke_tile_clicked(tile("B"), true, false);
    lib.invoke_tile_clicked(tile("C"), true, false);
    rig.pump();
    lib.invoke_tile_opened(tile("B"));
    rig.until("the grid", Duration::from_secs(60), || canvas.get_status().as_str() == "3 frames together");

    // A step on every frame, so each one's before differs from its after.
    let g = w.global::<StoreGlobal>();
    let field = Field::Exposure.index();
    g.invoke_press(field, false);
    for k in 1..=8 {
        g.invoke_drag(field, k as f32 * 10.0, 300.0, false);
        rig.pump();
    }
    g.invoke_release(field);
    rig.pump();

    // Compare on the sheet: the before is the sheet's own, laid where the after lies.
    ctl.act(oni_app::Act::Compare);
    rig.until("the sheet's before", Duration::from_secs(60), || canvas.get_before().size().width > 0);
    rig.settle();
    let frame = (canvas.get_frame_x(), canvas.get_frame_y(), canvas.get_frame_width(), canvas.get_frame_height());
    let before = (canvas.get_before_x(), canvas.get_before_y(), canvas.get_before_width(), canvas.get_before_height());
    println!("shot {}", rig.shot("together-compare").display());
    assert!(canvas.get_split() >= 0.0, "Compare is on");
    let near = |a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)| [a.0 - b.0, a.1 - b.1, a.2 - b.2, a.3 - b.3].iter().all(|d| d.abs() < 1.0);
    assert!(near(frame, before), "the before lies on the after: {frame:?} {before:?}");
    // The first tile's centre, in both textures: the same place, a different picture.
    let centre = |image: slint::Image| {
        let b = image.to_rgba8().expect("pixels");
        let k = (f64::from(b.width()) - 4.0) / f64::from(3 * 1024 + 2 * 30);
        let (x, y) = ((2.0 + 512.0 * k) as usize, (2.0 + 341.0 * k) as usize);
        let p = b.as_slice()[y * b.width() as usize + x];
        [p.r, p.g, p.b]
    };
    // Beyond the dither's step, which each encode draws anew.
    let apart = |a: [u8; 3], b: [u8; 3]| a.iter().zip(&b).any(|(a, b)| a.abs_diff(*b) > 4);
    let (after_px, before_px) = (centre(canvas.get_frame()), centre(canvas.get_before()));
    assert_ne!(after_px, before_px, "the before is drawn on the frame's own before, not its after");
    assert!(before_px.iter().any(|c| *c > 8), "twin: a picture, not the sheet's black: {before_px:?}");
    ctl.act(oni_app::Act::Compare);
    rig.pump();
    assert!(canvas.get_split() < 0.0 && canvas.get_before().size().width == 0, "Compare off lets the before go");

    // The Scanner face reaches every frame: A's tile is drawn again on it.
    rig.settle();
    let a = centre(canvas.get_frame());
    ctl.act(oni_app::Act::FaceScanner);
    rig.until("A on the Scanner face", Duration::from_secs(60), || face(&folder.join("A.tif")) == Some(Face::Scanner) && apart(centre(canvas.get_frame()), a));
    ctl.act(oni_app::Act::FacePro);
    rig.until("A back on Pro", Duration::from_secs(60), || face(&folder.join("A.tif")) == Some(Face::Pro));
    // A new proof draws the sheet again though no setting moved.
    rig.settle();
    let drawn = canvas.get_frame_revision();
    rig.settle();
    assert_eq!(canvas.get_frame_revision(), drawn, "twin: nothing draws the sheet on its own");
    w.global::<Exporting>().invoke_space_picked(0);
    rig.until("the sheet under the new proof", Duration::from_secs(60), || canvas.get_frame_revision() != drawn);

    // The crop is each frame's own: its key and its menu are out of reach while frames show together.
    let live = w.global::<Menus>().get_live();
    assert!(!bar.get_single() && !live.framing && !live.mirror && !live.correction, "the crop greyed, De-dust with it");
    assert!(!canvas.get_window_shown(), "no measuring window over the sheet");
    ctl.act(oni_app::Act::Framing);
    rig.pump();
    assert_ne!(panel.get_tab(), 1, "⌘K opens no crop on frames together");
    ctl.act(oni_app::Act::Correction);
    rig.pump();
    assert_ne!(panel.get_tab(), 4, "nor De-dust");
    assert!(!bar.get_single());

    // Twin: one frame alone again, the crop opens.
    let row = |n: &str| lib.get_rows().iter().position(|r| r.kind == 2 && r.title == n).unwrap_or_else(|| panic!("{n}'s row")) as i32;
    lib.invoke_row_clicked(row("B"), false, false);
    rig.until("B alone", Duration::from_secs(60), || canvas.get_status().contains(" × "));
    assert!(bar.get_single());
    ctl.act(oni_app::Act::Framing);
    rig.pump();
    assert_eq!(panel.get_tab(), 1, "twin: ⌘K opens the crop on one frame");
    // A frame joining from the crop takes the panel out of it.
    lib.invoke_row_clicked(row("A"), true, false);
    rig.until("B and A together", Duration::from_secs(30), || canvas.get_status().as_str() == "2 frames together");
    assert_eq!(panel.get_tab(), 2, "the crop left for the Color tab");
    assert!(!canvas.get_cropping());
    let _ = std::fs::remove_dir_all(&root);
}
