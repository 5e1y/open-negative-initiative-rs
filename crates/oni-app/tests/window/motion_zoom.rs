//! The photo's flight to and from a zoomed view: it lands where the canvas shows it at rest, zoom and pan included,
//! grown past the frame and cut by the canvas, so the view never jumps once it has landed.

use oni_app::fields::Field;
use oni_app::{Act, Canvas, Library, Store};
use slint::ComponentHandle;
use slint::platform::software_renderer::Rgb565Pixel;
use std::time::Duration;

use crate::common;

/// The instants each flight is shot at, in ms from its start, past its whole pace.
const STEPS: [u64; 8] = [0, 40, 80, 140, 220, 320, 400, 1000];
/// The instant the last frame of the flight is taken, its spring nearly at rest.
const LAST: u64 = 400;

/// Where the light crosses its middle along a row of the canvas: the stripes' edges, in window pixels.
fn edges((w, px): &(usize, Vec<Rgb565Pixel>), y: usize, (left, right): (usize, usize)) -> Vec<usize> {
    let light = |v: u16| u32::from((v >> 11) & 0x1f) * 255 / 31 + u32::from((v >> 5) & 0x3f) * 255 / 63 + u32::from(v & 0x1f) * 255 / 31;
    let row: Vec<u32> = (left..right).map(|x| light(px[y * w + x].0)).collect();
    let (lo, hi) = (*row.iter().min().unwrap_or(&0), *row.iter().max().unwrap_or(&0));
    if hi - lo < 90 {
        return Vec::new();
    }
    let mid = (lo + hi) / 2;
    row.windows(2).enumerate().filter(|(_, p)| (p[0] > mid) != (p[1] > mid)).map(|(i, _)| left + i).collect()
}

/// Whether two rows show the photo at one zoom and place: their stripes as wide within 5 %, the edge nearest the middle
/// within a quarter of a stripe, the thumbnail that flies holding a tenth of the render's pixels at 100 %.
fn together(a: &[usize], b: &[usize]) -> bool {
    let spacing = |e: &[usize]| {
        let mut d: Vec<usize> = e.windows(2).map(|p| p[1] - p[0]).collect();
        d.sort_unstable();
        d.get(d.len() / 2).copied().unwrap_or(0) as f64
    };
    if a.len() < 2 || b.len() < 2 {
        return false;
    }
    let (sa, sb) = (spacing(a), spacing(b));
    let middle = (b[0] + b[b.len() - 1]) / 2;
    let nearest = |e: &[usize]| *e.iter().min_by_key(|x| x.abs_diff(middle)).unwrap_or(&0);
    (sa / sb - 1.0).abs() <= 0.05 && (nearest(a).abs_diff(nearest(b)) as f64) <= sb / 4.0
}

/// Shoots `name` at each of `STEPS`, the clock held from its start, and returns the frame drawn at `LAST`.
fn shoot(rig: &common::Rig, name: &str, start: impl FnOnce()) -> (usize, Vec<Rgb565Pixel>) {
    rig.freeze();
    start();
    let (mut at, mut last) = (0, None);
    for step in STEPS {
        rig.advance(Duration::from_millis(step - at));
        at = step;
        rig.shot(&format!("motion-zoom-{name}-{step:04}"));
        if step == LAST {
            last = Some(rig.drawn());
        }
    }
    rig.thaw();
    last.expect("the last frame")
}

pub(crate) fn the_photo_flies_to_and_from_its_zoomed_view_and_lands_without_a_jump() {
    let root = common::Root::new("motion-zoom");
    let folder = root.scans();
    // Wide stripes: a view at 100 % and the photo fitted lay their edges far apart.
    for name in ["A.tif", "B.tif"] {
        common::pixels_tiff(&folder.join(name), 3000, 2000, |x, _| if (x / 150) % 2 == 0 { [0.2; 3] } else { [0.8; 3] });
    }
    let Some(app) = common::launch(root).size(1400, 900).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (canvas, lib) = (w.global::<Canvas>(), w.global::<Library>());
    app.edit_first();
    canvas.invoke_double_clicked();
    rig.until("the view at 100 %", Duration::from_secs(120), || ctl.state.borrow().placed().is_some_and(|p| !p.whole));
    rig.settle();
    // A row of the canvas at two fifths of its height, clear of the measuring window's marks.
    let row = || {
        let flight = w.global::<oni_app::Flight>();
        let (x, y) = (flight.get_canvas_x(), flight.get_canvas_y());
        ((y + canvas.get_height() * 0.4) as usize, ((x + 20.0) as usize, (x + canvas.get_width() - 20.0) as usize))
    };
    // The photo converted, as every photo the editor shows once its roll is prepared: its tile is then its render, the
    // picture that flies, where a frame never converted shows its scan.
    let tile = || slint::Model::row_data(&lib.get_tiles(), 0).map(|t| t.picture.to_rgba8().map(|b| b.as_bytes().iter().map(|v| u64::from(*v)).sum::<u64>()));
    let scanned = tile();
    w.global::<Store>().invoke_step(Field::Exposure.index(), 1);
    ctl.act(Act::ToggleMode);
    rig.until("the tile of the photo converted", Duration::from_secs(60), || tile() != scanned);
    rig.settle();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("the view at 100 % again", Duration::from_secs(120), || ctl.state.borrow().placed().is_some_and(|p| !p.whole));
    rig.settle();
    let (y, span) = row();
    let zoomed = edges(&rig.drawn(), y, span);
    assert!(zoomed.len() >= 2, "the view at 100 % shows the stripes: {zoomed:?}");

    // Back to the gallery from the zoomed view, then the photo opened again on its zoom.
    shoot(rig, "out", || ctl.act(Act::ToggleMode));
    rig.settle();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    let last = shoot(rig, "in", || {
        lib.invoke_tile_clicked(0, false, false);
        lib.invoke_tile_clicked(0, false, false);
    });
    rig.settle();
    let rest = rig.drawn();
    let (landing, landed) = (edges(&last, y, span), edges(&rest, y, span));
    println!("zoom flight: edges at rest {landed:?}, in the flight's last frame {landing:?}, before leaving {zoomed:?}");
    assert!(together(&landed, &zoomed), "the photo opened again shows its zoom: {landed:?} against {zoomed:?}");
    assert!(together(&landing, &landed), "the flight's last frame lies on the view at rest: {landing:?} against {landed:?}");
    // Twin: the photo fitted, as the flight laid it before, does not lie on the zoomed view.
    canvas.invoke_double_clicked();
    rig.until("the photo fitted", Duration::from_secs(60), || ctl.state.borrow().placed().is_some_and(|p| p.whole));
    rig.settle();
    let fitted = edges(&rig.drawn(), y, span);
    assert!(!together(&fitted, &landed), "the fitted photo reads as the zoomed view: {fitted:?}");
}
