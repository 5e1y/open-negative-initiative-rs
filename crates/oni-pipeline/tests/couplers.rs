//! A film file's own DIR coupler matrix: the bake reads it, a file without one reads the
//! common matrix, and the couplers move a colour while the grey stays on the data sheet's curve. Each verdict has a twin.

use oni_pipeline::film::{self, GREY, bake::COUPLERS};
use oni_render::cpu;
use oni_settings::VirtualStock;

#[test]
fn a_films_own_couplers_move_its_colours_and_leave_its_grey() {
    let v = VirtualStock::of("kodak-vision3-250d");
    let own = film::couplers(&v.entry());
    assert_ne!(own, COUPLERS, "Vision3 250D's file holds its own matrix");
    // Twins: a file holding none reads the common matrix; each colour negative holds its own, derived from its sheet.
    assert_eq!(film::couplers(&VirtualStock::of("ilford-hp5-plus").entry()), COUPLERS);
    let portra = film::couplers(&VirtualStock::of("kodak-portra-800").entry());
    assert!(portra != own && portra != COUPLERS && portra.iter().flatten().any(|v| *v > 0.0), "{portra:?}");
    let f32s = own.map(|r| r.map(|x| x as f32));
    assert_eq!(film::stock(v).kernel.couplers, f32s);

    // A grey exposes the three layers alike: the couplers then leave it on the sheet's curve.
    let entry = v.entry();
    let (mine, common) = (film::bake_through(&entry, own).unwrap(), film::bake_through(&entry, COUPLERS).unwrap());
    let read = |s: &film::Stock, p: [f32; 4]| cpu::virtual_negative(p, &s.kernel_at(0.0), &s.table);
    let (grey, red) = ([GREY, GREY, GREY, 1.0], [0.4, 0.08, 0.05, 1.0]);
    let apart = |p| (0..3).map(|c| (read(&mine, p)[c] / read(&common, p)[c] - 1.0).abs()).fold(0.0f32, f32::max);
    println!("grey {:.2e}, red {:.2e}", apart(grey), apart(red));
    assert!(apart(grey) < 2e-3, "the grey moved {}", apart(grey));
    assert!(apart(red) > 2e-2, "the red did not move: the check sees nothing");
}
