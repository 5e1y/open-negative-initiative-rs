//! The levels track, the wheel, and the blocks each photo and face lays out.

use super::*;

pub(crate) fn a_handle_dragged_on_the_levels_is_one_step_and_springs_back_on_undo() {
    let Some(o) = opened("levels") else { return };
    let (rig, w, ctl): (&common::Rig, &MainWindow, _) = (&o.rig, &o.shell.window, o.shell.ctl.clone());
    let g = w.global::<Editors>();
    on_tab(&o, 0, |g| g.set_levels_folded(false));
    let origin = editors::origin(&ctl, Plot::Levels);
    let size = editors::size(&ctl, Plot::Levels);
    assert!(size.w > 200.0, "the levels track is laid out: {size:?}");
    println!("shot {}", rig.shot("editors-levels").display());
    let knob = g.get_levels_knobs().row_data(4).unwrap();
    assert_eq!(knob.label.as_str(), "White");
    let red = || ctl.state.borrow().store().settings().levels.red;
    let before = red();
    let grab = (origin.0 + f64::from(knob.x) - 4.0, origin.1 + size.h / 2.0);
    press(&rig, grab);
    drag(&rig, grab, (-40.0, 0.0), 20);
    release(&rig, (grab.0 - 40.0, grab.1));
    rig.settle();
    let after = red();
    let position = |v: f32| oni_settings::levels::LevelsChannel::Red.position_of_value(v, oni_settings::ConversionMode::Negative);
    let travel = (position(before.white) - position(after.white)) * size.w as f32;
    assert!((travel - 40.0).abs() < 0.5, "the white moved by the pointer's travel: {travel}");
    assert_eq!((after.black, after.mid), (before.black, before.mid), "the other handles stay");
    assert!(g.get_levels_modified());
    let white_x = || f64::from(g.get_levels_knobs().row_data(4).unwrap().x);
    let x_of = |v: f32| f64::from(position(v)) * size.w;
    assert!((white_x() - x_of(after.white)).abs() < 1e-3, "the handle let go stands where the hand left it");
    // The undo is a value from elsewhere: the handle springs back over frames instead of jumping.
    w.global::<Bar>().invoke_act("undo".into());
    rig.pump();
    let flying = white_x();
    assert!((flying - x_of(before.white)).abs() > 1.0, "the handle jumped back: {flying}");
    rig.settle();
    assert_eq!(red(), before, "one undo takes the handle back");
    assert!((white_x() - x_of(before.white)).abs() < 1e-3, "the spring came to rest on the value");
    let _ = std::fs::remove_dir_all(&o.dir);
}

pub(crate) fn the_wheel_takes_the_arrows_once_pressed_and_its_swatches_name_their_bands() {
    let Some(o) = opened("wheel") else { return };
    let (rig, w, ctl): (&common::Rig, &MainWindow, _) = (&o.rig, &o.shell.window, o.shell.ctl.clone());
    let g = w.global::<Editors>();
    on_tab(&o, 2, |g| g.set_spectrogram_folded(false));
    let origin = editors::origin(&ctl, Plot::Wheel);
    let size = editors::size(&ctl, Plot::Wheel);
    assert!(size.w > 200.0 && (size.w - size.h).abs() < 0.5, "the wheel is a square: {size:?}");
    assert!(origin.1 + size.h < 2200.0, "the wheel is on screen: {origin:?}");
    let red = || ctl.state.borrow().store().settings().spectrogram.band(Band::Red);
    let up: slint::SharedString = slint::platform::Key::UpArrow.into();
    rig.key(&up);
    assert_eq!(red(), (0.0, 0.0, 0.0), "an unfocused wheel left the arrow alone");
    let wheel = editors::spectrogram::Wheel { side: size.w };
    let tip = wheel.position(editors::spectrogram::Grip::default(), &Spectrogram::default());
    let at = (origin.0 + tip.0 + 2.0, origin.1 + tip.1);
    press(&rig, at);
    release(&rig, at);
    rig.key(&up);
    let (_, saturation, _) = red();
    assert!((saturation - 0.04).abs() < 1e-6, "the arrow stepped the selected point: {saturation}");
    println!("shot {}", rig.shot("editors-wheel").display());
    // The band slider follows the wheel by its spring, a value from elsewhere.
    let knob = || g.get_band_fractions().row_data(1).unwrap();
    let range = oni_settings::spectrogram::Spectrogram::SATURATION_RANGE;
    let target = oni_app::track::position(0.04, &(range.0..=range.1), None);
    rig.settle();
    assert!((knob() - target).abs() < 1e-5, "the saturation knob rests on the value: {}", knob());

    // A press off the wheel releases it: the point unlights and the arrows go back to the window,
    // as the window's own press hook sends it before the press lands.
    let lit = || g.get_wheel_dots().iter().any(|d| d.halo > 0.0);
    assert!(lit(), "the focused wheel lights its point");
    editors::pressed_anywhere(&ctl, at);
    press(&rig, at);
    release(&rig, at);
    rig.key(&up);
    let (_, twice, _) = red();
    assert!((twice - 0.08).abs() < 1e-6, "twin: a press on the wheel keeps it: {twice}");
    let outside = (origin.0 - 4.0, origin.1 + size.h / 2.0);
    editors::pressed_anywhere(&ctl, outside);
    press(&rig, outside);
    release(&rig, outside);
    rig.settle();
    assert!(!lit(), "a press off the wheel left its point lit");
    rig.key(&up);
    assert_eq!(red().1, twice, "the arrow still reached the wheel");

    // A swatch tab names its band in the tooltip; a channel tab already shows its name.
    let tip = w.global::<oni_app::Tip>();
    let mut named = None;
    for dy in 0..80 {
        let at = LogicalPosition::new((origin.0 + 12.0) as f32, (origin.1 + size.h + dy as f64) as f32);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
        rig.pump();
        if tip.get_shown() {
            named = Some(tip.get_text().to_string());
            break;
        }
    }
    assert_eq!(named.as_deref(), Some(Band::ALL[0].label()), "the first swatch named no band");
    let _ = std::fs::remove_dir_all(&o.dir);
}

pub(crate) fn the_panel_lays_out_only_the_blocks_a_photo_and_a_face_have() {
    let Some(o) = opened("faces") else { return };
    let (rig, w): (&common::Rig, &MainWindow) = (&o.rig, &o.shell.window);
    let g = w.global::<Editors>();
    // A monochrome photo has no grade blocks: the wheel's plot is never laid out.
    let mut mono = EditorStore::new(Archive::new(vec![o.frame.clone()], |_: &Path| true));
    mono.open(&o.frame).unwrap();
    mono.apply(oni_core::Command::Mode(oni_settings::ConversionMode::Monochrome));
    let engine = oni_app::render::Engine::shared(&o.gpu);
    let options = Options { support: o.dir.join("support"), ..Options::default() };
    let second = Shell::new(Box::new(mono), engine, options).unwrap();
    second.window.show().unwrap();
    second.window.global::<App>().set_gallery(false);
    let g2 = second.window.global::<Editors>();
    second.window.global::<PanelState>().set_tab(2);
    g2.set_spectrogram_folded(false);
    rig.shot("editors-monochrome");
    rig.settle();
    assert!(g2.get_monochrome());
    assert_eq!(editors::size(&second.ctl, Plot::Wheel).w, 0.0, "a monochrome panel laid out the wheel");
    assert!(!g.get_low_precision(), "twin: a RAW is never flagged 8-bit");

    // An 8-bit scan is flagged under Per-Channel Levels.
    let scan = o.dir.join("scan.tif");
    std::fs::write(&scan, linear_raw_8_bit(4, 4)).unwrap();
    let scan = scan.to_string_lossy().into_owned();
    let mut eight = EditorStore::new(Archive::new(vec![scan.clone()], |_: &Path| false));
    eight.open(&scan).unwrap();
    let engine = oni_app::render::Engine::shared(&o.gpu);
    let options = Options { support: o.dir.join("support"), ..Options::default() };
    let third = Shell::new(Box::new(eight), engine, options).unwrap();
    third.window.show().unwrap();
    third.window.global::<App>().set_gallery(false);
    let g3 = third.window.global::<Editors>();
    third.window.global::<PanelState>().set_tab(0);
    g3.set_levels_folded(false);
    rig.shot("editors-eight-bit");
    rig.settle();
    assert!(g3.get_low_precision(), "an 8-bit scan carries no badge");
    assert!(editors::size(&third.ctl, Plot::Levels).w > 200.0, "the Pro face lays out Per-Channel Levels");

    // The Scanner face has no Per-Channel Levels: the block is never laid out.
    // The face switch takes a frame opened from the archive, as a hand opens one.
    let faces = o.dir.join("faces");
    std::fs::create_dir_all(&faces).unwrap();
    let raw = faces.join("frame.NEF");
    std::fs::write(&raw, b"").unwrap();
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&o.gpu);
    let picker: Box<dyn Fn() -> Option<PathBuf>> = Box::new(|| None);
    let options = Options { support: o.dir.join("faces-support"), caches: o.dir.join("faces-caches"), picker, ..Options::default() };
    let fourth = Shell::new(Box::new(store), engine, options).unwrap();
    fourth.window.show().unwrap();
    fourth.add(&raw);
    rig.until("the frame in the editor", Duration::from_secs(60), || {
        !fourth.window.global::<App>().get_gallery() && fourth.ctl.state.borrow().open_path().is_some()
    });
    fourth.window.global::<Bar>().invoke_act("face-scanner".into());
    let g4 = fourth.window.global::<Editors>();
    fourth.window.global::<PanelState>().set_tab(0);
    g4.set_levels_folded(false);
    rig.shot("editors-scanner-face");
    rig.settle();
    assert!(g4.get_scanner_face());
    assert_eq!(editors::size(&fourth.ctl, Plot::Levels).w, 0.0, "the Scanner face laid out Per-Channel Levels");
    let _ = std::fs::remove_dir_all(&o.dir);
}
