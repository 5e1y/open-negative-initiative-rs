//! Every motion as the real window draws it: its first frame comes once the event loop has run its timers, 20 to 60 ms
//! after the gesture, and must still show the motion on its way: the photo's flight, a pane, a step of Prepare, the wave.

use oni_app::{Library, PanelState, PanelTab, Preparing};
use slint::ComponentHandle;
use slint::platform::software_renderer::Rgb565Pixel;
use std::time::Duration;

use crate::common;

type Drawn = (usize, Vec<Rgb565Pixel>);

/// A part of the window, left, top, right and bottom, in its pixels.
type Region = (usize, usize, usize, usize);

/// The share of a region's pixels whose red, green and blue, summed, differ by more than `apart` between two frames.
fn changed_share(a: &Drawn, b: &Drawn, (left, top, right, bottom): Region, apart: u32) -> f32 {
    let light = |v: u16| u32::from((v >> 11) & 0x1f) * 255 / 31 + u32::from((v >> 5) & 0x3f) * 255 / 63 + u32::from(v & 0x1f) * 255 / 31;
    let (mut changed, mut n) = (0u32, 0u32);
    for y in (top..bottom).step_by(4) {
        for x in (left..right).step_by(4) {
            let (p, q) = (light(a.1[y * a.0 + x].0), light(b.1[y * b.0 + x].0));
            changed += u32::from(p.abs_diff(q) > apart);
            n += 1;
        }
    }
    changed as f32 / n.max(1) as f32
}

/// The frames drawn 40 ms after `start`, then once at rest and once more: the clock held, as the real window draws only
/// once the loop has run the timers that come due before its next frame.
fn drawn_after(rig: &common::Rig, name: &str, start: impl FnOnce()) -> (Drawn, Drawn, Drawn) {
    rig.freeze();
    start();
    rig.advance(Duration::from_millis(40));
    let early = rig.drawn();
    rig.shot(&format!("motion-clock-{name}-040"));
    rig.advance(Duration::from_millis(1500));
    let rest = rig.drawn();
    rig.shot(&format!("motion-clock-{name}-rest"));
    rig.advance(Duration::from_millis(500));
    let still = rig.drawn();
    rig.thaw();
    (early, rest, still)
}

/// The motion's first frame differs from its rest over `region`; twin, two frames at rest do not, so the measure
/// sees no motion where none is.
fn moving_at_first(name: &str, (early, rest, still): &(Drawn, Drawn, Drawn), region: Region, apart: u32, share: f32) {
    let (moved, settled) = (changed_share(early, rest, region, apart), changed_share(rest, still, region, apart));
    println!("{name}: {moved:.2} of the region moved at 40 ms, {settled:.2} at rest");
    assert!(moved > share, "{name}: 40 ms in, the motion is still on its way: {moved:.2} of the region differs from its rest");
    assert!(settled < 0.02, "{name}: at rest, nothing moves: {settled:.2}");
}

fn tones(root: &common::Root, count: usize) -> std::path::PathBuf {
    let folder = root.scans();
    for i in 0..count {
        let v = if i % 2 == 0 { 0.15 } else { 0.85 };
        common::pixels_tiff(&folder.join(format!("{}.tif", char::from(b'A' + i as u8))), 600, 400, move |_, _| [v; 3]);
    }
    folder
}

/// The tile double-clicked flies into the canvas; a pane taken from the tab bar slides in.
pub(crate) fn the_flight_and_the_panes_are_drawn_on_their_way_after_the_timers_ran() {
    let root = common::Root::new("motion-clock");
    let folder = tones(&root, 4);
    let Some(app) = common::launch(root).size(1400, 900).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    let lib = w.global::<Library>();
    app.read(&folder);
    rig.until("the thumbnails", Duration::from_secs(60), || {
        rig.pump();
        slint::Model::iter(&lib.get_tiles()).all(|t| t.picture.size().width > 0)
    });
    rig.settle();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    let flight = drawn_after(rig, "flight", || {
        lib.invoke_tile_clicked(1, false, false);
        lib.invoke_tile_clicked(1, false, false);
    });
    // The canvas's middle, where the photo lands.
    moving_at_first("the flight into the editor", &flight, (500, 300, 900, 600), 76, 0.3);
    rig.settle();
    let panel = w.global::<PanelState>();
    let panes = drawn_after(rig, "pane", || {
        panel.set_tab(PanelTab::Details);
        panel.invoke_tab_picked(PanelTab::Details);
    });
    // The right column under its tab bar, where the pane slides in.
    moving_at_first("the pane", &panes, (1100, 200, 1390, 850), 76, 0.1);
}

/// A step of Prepare slides its body in; a roll opened from the gallery lands on its sheet as a wave.
pub(crate) fn prepare_s_step_and_wave_are_drawn_on_their_way_after_the_timers_ran() {
    let root = common::Root::new("motion-clock-prepare");
    let folder = tones(&root, 12);
    let Some(app) = common::launch(root).size(1400, 900).with(|o| o.require_roll = true).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    let (lib, prepare) = (w.global::<Library>(), w.global::<Preparing>());
    app.read(&folder);
    rig.until("the thumbnails", Duration::from_secs(60), || {
        rig.pump();
        slint::Model::iter(&lib.get_tiles()).all(|t| t.picture.size().width > 0)
    });
    rig.settle();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    let wave = drawn_after(rig, "wave", || {
        lib.invoke_tile_clicked(3, false, false);
        lib.invoke_tile_clicked(3, false, false);
        rig.until("the cells", Duration::from_secs(30), || prepare.get_shown() && slint::Model::row_count(&prepare.get_cells()) == 12);
    });
    // The sheet, right of the column and left of the step's, its cells still dark on its ground.
    moving_at_first("the wave", &wave, (280, 150, 1050, 800), 12, 0.1);
    rig.settle();
    let step = drawn_after(rig, "step", || prepare.invoke_step_picked(2));
    // The step's column, its body coming in faded.
    moving_at_first("Prepare's step", &step, (1100, 200, 1390, 850), 76, 0.05);
}
