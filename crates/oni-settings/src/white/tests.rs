use super::*;

fn white(kelvin: f32, tint: f32) -> White {
    White { kelvin, tint }
}

#[test]
fn a_node_reads_its_own_table_entry_and_the_mosaic_white_is_5000_k_exactly() {
    assert_eq!(uv(white(5000.0, 0.0)), MOSAIC_WHITE);
    assert_eq!(of_uv(MOSAIC_WHITE), white(5000.0, 0.0));
    assert_eq!(uv(white(4000.0, 0.0)), (NODES[16].1, NODES[16].2));
    // Twin: 10 K away is another white.
    assert_ne!(uv(white(5010.0, 0.0)), MOSAIC_WHITE);
}

#[test]
fn kelvin_and_tint_come_back_from_their_white_over_the_whole_range() {
    for kelvin in [2000.0, 2850.0, 3200.0, 4300.0, 5000.0, 5500.0, 6504.0, 7500.0, 10000.0, 15000.0, 25000.0] {
        for tint in [-150.0, -40.0, 0.0, 9.8, 60.0, 150.0] {
            let back = of_uv(uv(white(kelvin, tint)));
            assert!((back.kelvin - kelvin).abs() < 0.5 && (back.tint - tint).abs() < 0.05, "{kelvin} {tint}: {back:?}");
        }
    }
}

#[test]
fn d65_and_illuminant_a_read_as_dng_reads_them() {
    // CIE D65 and A by their xy; McCamy's cubic puts them at 6504 K and 2856 K, on the locus' side.
    let d65 = of_uv(FILE_WHITE);
    assert!((d65.kelvin - 6504.0).abs() < 2.0 && (d65.tint - 9.8).abs() < 1.0, "{d65:?}");
    let a = of_uv(uv_of_xy(0.44757, 0.40745));
    assert!((a.kelvin - 2856.0).abs() < 3.0 && a.tint.abs() < 1.0, "{a:?}");
    // Twin: D50 is no D65.
    assert!((of_uv(uv_of_xy(0.3457, 0.3585)).kelvin - 5003.0).abs() < 3.0);
}

#[test]
fn a_positive_tint_is_greener_light_and_a_higher_kelvin_bluer_light() {
    let (u0, v0) = uv(white(5000.0, 0.0));
    let (_, v1) = uv(white(5000.0, 20.0));
    assert!(v1 > v0, "a magenta tint balances a green light, above the locus");
    let (u2, _) = uv(white(8000.0, 0.0));
    assert!(u2 < u0, "a higher kelvin stands toward blue");
}

#[test]
fn the_relative_balance_is_the_identity_on_its_own_white() {
    for anchor in [MOSAIC_WHITE, FILE_WHITE] {
        let m = relative(anchor, anchor);
        for (r, row) in m.iter().enumerate() {
            for (c, x) in row.iter().enumerate() {
                assert!((x - if r == c { 1.0 } else { 0.0 }).abs() < 1e-6, "{m:?}");
            }
        }
    }
}

/// A grey card lit by `light`, as a decode made on white `anchor` hands it out in Rec. 2020.
fn decoded_card(light: Uv, anchor: Uv) -> [f64; 3] {
    let to_d65 = relative(anchor, FILE_WHITE).map(|r| r.map(f64::from));
    apply(&to_d65, apply(&rec2020_from_xyz(), xyz(light)))
}

#[test]
fn a_grey_card_under_the_chosen_light_renders_neutral() {
    let tungsten = uv(white(3200.0, 0.0));
    let card = decoded_card(tungsten, MOSAIC_WHITE);
    assert!(card[2] < card[0] * 0.8, "the decode shows the tungsten cast: {card:?}");
    let m = relative(tungsten, MOSAIC_WHITE).map(|r| r.map(f64::from));
    let balanced = apply(&m, card);
    let mean = balanced.iter().sum::<f64>() / 3.0;
    assert!(balanced.iter().all(|c| (c / mean - 1.0).abs() < 1e-4), "{balanced:?}");
    // Twin: the balance taken the wrong way round doubles the cast.
    let wrong = apply(&relative(MOSAIC_WHITE, tungsten).map(|r| r.map(f64::from)), card);
    assert!(wrong[2] / wrong[0] < card[2] / card[0] * 0.9, "{wrong:?}");
}

#[test]
fn a_positive_tint_lowers_green_and_a_higher_kelvin_warms() {
    let grey = [0.18; 3];
    let tinted = apply(&relative(uv(white(5000.0, 30.0)), MOSAIC_WHITE).map(|r| r.map(f64::from)), grey);
    assert!(tinted[1] < tinted[0] && tinted[1] < tinted[2], "{tinted:?}");
    let warm = apply(&relative(uv(white(7000.0, 0.0)), MOSAIC_WHITE).map(|r| r.map(f64::from)), grey);
    assert!(warm[0] > warm[2], "{warm:?}");
    // Twin: the opposite tint raises green.
    let green = apply(&relative(uv(white(5000.0, -30.0)), MOSAIC_WHITE).map(|r| r.map(f64::from)), grey);
    assert!(green[1] > green[0]);
}
