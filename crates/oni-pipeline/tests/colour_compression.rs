//! A paper's colour compression (`paper::compressed`): at 1 the look is its file's to the bit, at 0 each channel reads its
//! own dye alone, and at every value up to the reach its file bakes a grey prints as at 1 while the saturated colours give
//! more of their chroma the further it goes. Each verdict has a twin.

use oni_pipeline::conversion;
use oni_pipeline::film::bake;
use oni_pipeline::paper;
use oni_render::{PAPER_CURVE_NODES, Paper};
use oni_settings::{ConversionMode, PaperStock, PipelineSettings};

fn on(p: PaperStock, k: f32) -> PipelineSettings {
    let mut s = PipelineSettings { print_paper: Some(p), ..PipelineSettings::rest(ConversionMode::Negative, true) };
    s.print.colour_compression = k;
    s
}

fn paper_at(p: PaperStock, k: f32) -> Paper {
    conversion::stage47(&on(p, k)).paper().expect("a paper")
}

fn limit(p: PaperStock) -> f32 {
    p.compression_limit().expect("every shipped paper bakes its reach")
}

/// The strengths a check walks, from pure dyes to the paper's reach.
fn strengths(p: PaperStock) -> Vec<f32> {
    (0..=20).map(|i| limit(p) * i as f32 / 20.0).collect()
}

/// CIELAB chroma of a linear Rec. 2020 light on D65.
fn chroma(v: [f32; 3]) -> f64 {
    let xyz = bake::apply(&bake::rec2020_to_xyz(), v.map(f64::from));
    let white = bake::apply(&bake::rec2020_to_xyz(), [1.0; 3]);
    let f = |t: f64| if t > (6.0f64 / 29.0).powi(3) { t.cbrt() } else { t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0 };
    let [x, y, z] = std::array::from_fn(|c| f(xyz[c] / white[c]));
    (500.0 * (x - y)).hypot(200.0 * (y - z))
}

#[test]
fn at_one_every_paper_prints_its_files_look_to_the_bit() {
    for p in PaperStock::all() {
        let (k, sheet) = (paper_at(p, 1.0), paper::sheet(p).unwrap());
        assert!(k.curves == sheet.curves && k.crosstalk == sheet.crosstalk, "{p:?}: the look as its file bakes it");
        // The setting at rest writes nothing, so a photo of an earlier format prints the same paper.
        assert_eq!(PipelineSettings::default().print.colour_compression, 1.0);
        // Twin: a hair off 1 moves the curves.
        assert!(paper_at(p, 1.01).curves != sheet.curves, "{p:?}");
    }
}

#[test]
fn a_grey_prints_alike_at_every_strength_up_to_the_papers_reach() {
    for p in PaperStock::all() {
        let rest = paper_at(p, 1.0);
        let grey = |q: &Paper| (0..=40).map(|i| -0.2 + 1.4 * i as f32 / 40.0).map(|t| q.dyed(q.amounts([t; 3]))).collect::<Vec<_>>();
        let off = |q: &Paper| grey(q).iter().zip(grey(&rest)).map(|(a, b)| (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0f32, f32::max)).fold(0.0f32, f32::max);
        let worst = strengths(p).iter().map(|&k| off(&paper_at(p, k))).fold(0.0f32, f32::max);
        println!("{}: a grey's densities within {worst:.1e} of the rest's from 0 to {}", p.name(), limit(p));
        assert!(worst < 1e-4, "{p:?}: {worst}");
        // Twin: the crosstalk compressed with the curves left as baked tints the greys.
        let unmixed = Paper { curves: rest.curves, ..paper_at(p, 0.0) };
        assert!(off(&unmixed) > 0.02, "{p:?}: {}", off(&unmixed));
    }
}

#[test]
fn at_zero_each_channel_reads_its_own_dye_alone() {
    for p in PaperStock::all() {
        let pure = paper_at(p, 0.0);
        let side = (0..3).flat_map(|r| (0..3).filter(move |c| *c != r).map(move |c| (r, c))).map(|(r, c)| pure.crosstalk[r][c].abs()).fold(0.0f32, f32::max);
        // More red dye moves red's density alone.
        let (a, more) = (pure.amounts([0.5; 3]), pure.amounts([0.3, 0.5, 0.5]));
        let (d, e) = (pure.dyed(a), pure.dyed([more[0], a[1], a[2]]));
        assert!(side == 0.0 && d[1] == e[1] && d[2] == e[2] && d[0] != e[0], "{p:?}: {side}");
        // Twin: as baked, red's dye reads on green too.
        let rest = paper_at(p, 1.0);
        let (d, e) = (rest.dyed(rest.amounts([0.5; 3])), rest.dyed([rest.amounts([0.3, 0.5, 0.5])[0], rest.amounts([0.5; 3])[1], rest.amounts([0.5; 3])[2]]));
        assert!(d[1] != e[1], "{p:?}");
    }
}

#[test]
fn the_saturated_colours_give_more_of_their_chroma_the_further_it_goes() {
    const SATURATED: [[f32; 3]; 6] = [[0.8, 0.5, 0.5], [0.5, 0.8, 0.5], [0.5, 0.5, 0.8], [0.2, 0.5, 0.5], [0.5, 0.2, 0.5], [0.5, 0.5, 0.2]];
    for p in PaperStock::all() {
        let mean = |k: f32| SATURATED.iter().map(|s| chroma(paper_at(p, k).shown(*s))).sum::<f64>() / 6.0;
        let along: Vec<f64> = strengths(p).iter().map(|&k| mean(k)).collect();
        println!("{}: saturated chroma {:.1} at 0, {:.1} at 1, {:.1} at {}", p.name(), along[0], mean(1.0), along[20], limit(p));
        assert!(along.windows(2).all(|w| w[1] < w[0]), "{p:?}: {along:?}");
        // Twin: the reach never passes the widest travel, and a value past it prints as the reach.
        assert!(limit(p) <= oni_settings::tone::Print::COMPRESSION_RANGE.1 && paper_at(p, 9.0) == paper_at(p, limit(p)), "{p:?}");
    }
}

#[test]
fn at_its_reach_each_layer_still_falls_with_the_share_and_holds_no_negative_dye() {
    for p in PaperStock::all() {
        let q = paper_at(p, limit(p));
        let (rise, low) = (1..PAPER_CURVE_NODES).fold((0.0f32, 0.0f32), |(rise, low), i| {
            (rise.max((0..3).map(|c| q.curves[i][c] - q.curves[i - 1][c]).fold(0.0, f32::max)), low.min(q.curves[i].iter().copied().fold(0.0, f32::min)))
        });
        println!("{}: at {} a layer rises at most {rise:.1e} and holds down to {low:.1e}", p.name(), limit(p));
        assert!(rise <= 1e-3 && low >= -5e-3, "{p:?}: {rise} {low}");
    }
    // Twin: Ektacolor Edge's curves past its reach by its margin rise or go negative.
    let edge = PaperStock::of("kodak-ektacolor-edge");
    let past = paper::compressed(paper::sheet(edge).unwrap(), limit(edge) / 0.8 + 0.05).0;
    let rise = (1..PAPER_CURVE_NODES).map(|i| (0..3).map(|c| past[i][c] - past[i - 1][c]).fold(0.0f32, f32::max)).fold(0.0f32, f32::max);
    let low = past.iter().flatten().copied().fold(0.0f32, f32::min);
    assert!(rise > 1e-3 || low < -5e-3, "{rise} {low}");
}
