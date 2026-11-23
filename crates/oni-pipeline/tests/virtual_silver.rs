//! A black and white virtual negative: the scene weighed by the stock's one layer, developed on its curve to a
//! neutral silver, scanned, then read as a monochrome roll's negative is. Each verdict has a twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_graph::Image;
use oni_pipeline::film::{self, GREY};
use oni_pipeline::{Pipeline, Request, conversion, stages};
use oni_render::{VirtualNegative, cpu};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

fn silver() -> impl Iterator<Item = VirtualStock> {
    VirtualStock::all().into_iter().filter(|s| s.is_silver())
}

fn scan(stock: VirtualStock, rgb: [f32; 3]) -> [f32; 3] {
    let baked = film::stock(stock);
    let out = cpu::virtual_negative([rgb[0], rgb[1], rgb[2], 1.0], &baked.kernel, &baked.table);
    [out[0], out[1], out[2]]
}

/// The scan's density, green's.
fn density(stock: VirtualStock, rgb: [f32; 3]) -> f32 {
    -scan(stock, rgb)[1].log10()
}

/// Rec. 2020's luma, the light a black and white print keeps.
fn luma(rgb: [f32; 3]) -> f32 {
    0.2627 * rgb[0] + 0.6780 * rgb[1] + 0.0593 * rgb[2]
}

/// A red, a green and a blue at a grey's luma, and that grey: sRGB's primaries in Rec. 2020.
fn patches() -> [[f32; 3]; 4] {
    let at_grey = |c: [f32; 3]| c.map(|v| v * GREY / luma(c));
    [at_grey([0.6274, 0.0691, 0.0164]), at_grey([0.3293, 0.9195, 0.0880]), at_grey([0.0433, 0.0114, 0.8956]), [GREY; 3]]
}

/// Every patch, ramp step and hue scans neutral on every black and white film. Twin: a colour negative's grey scan is not.
#[test]
fn a_silver_negative_scans_neutral() {
    let hues = (0..12).map(|h| {
        let a = h as f32 / 12.0 * std::f32::consts::TAU;
        [a.cos(), (a - 2.1).cos(), (a + 2.1).cos()].map(|v| GREY * (1.0 + 0.8 * v))
    });
    let ramp = (-16..=16).map(|h| [GREY * 2f32.powf(h as f32 / 2.0); 3]);
    let scenes: Vec<[f32; 3]> = hues.chain(ramp).chain(patches()).collect();
    for stock in silver() {
        let worst = scenes.iter().map(|rgb| {
            let t = scan(stock, *rgb);
            (t[0] - t[1]).abs().max((t[2] - t[1]).abs()) / t[1]
        });
        let worst = worst.fold(0.0f32, f32::max);
        assert!(worst < 1e-5, "{stock:?}: a scan {worst:.1e} off neutral");
    }
    let portra = scan(VirtualStock::of("kodak-portra-800"), [GREY; 3]);
    assert!((portra[0] - portra[2]).abs() / portra[1] > 0.1, "twin: a colour negative's orange mask: {portra:?}");
}

/// An orthochromatic film, blind to red, develops a red at a grey's luma at least 0.35 density thinner than that grey
/// and its blue denser; a panchromatic film's red no more than 0.15 thinner. Printed, the red falls under a fifth of
/// the grey's light on the orthochromatic film and stays over 0.7 of it on a panchromatic one. Twin: Ilford's
/// orthochromatic film's red at least 0.3 thinner than HP5's.
#[test]
fn an_orthochromatic_film_holds_red_thin_and_a_panchromatic_one_does_not() {
    let [red, green, blue, grey] = patches();
    let ortho = |s: VirtualStock| s.key().contains("ortho");
    let thin = |s: VirtualStock, c| density(s, grey) - density(s, c);
    for stock in silver() {
        let (r, g, b) = (thin(stock, red), thin(stock, green), thin(stock, blue));
        println!("{:<24} red {r:+.3} green {g:+.3} blue {b:+.3} density under the grey's", stock.name());
        if ortho(stock) {
            assert!(r > 0.35 && b < 0.0, "{stock:?}: red {r:.3}, blue {b:.3} under its grey");
        } else {
            assert!(r < 0.15, "{stock:?}: a panchromatic film's red {r:.3} thinner than its grey");
        }
    }
    let printed = |s: VirtualStock, c: [f32; 3]| {
        let k = conversion::stage47(&film::on_reference(s, &s.reference()));
        cpu::stage47(cpu::virtual_negative([c[0], c[1], c[2], 1.0], &film::stock(s).kernel, &film::stock(s).table), &k)[1]
    };
    for stock in silver() {
        let share = printed(stock, red) / printed(stock, grey);
        println!("{:<24} its red printed at {share:.3} of its grey", stock.name());
        assert!(if ortho(stock) { share < 0.2 } else { share > 0.7 }, "{stock:?}: {share:.3}");
    }
    let (o, p) = (VirtualStock::of("ilford-ortho-plus"), VirtualStock::of("ilford-hp5-plus"));
    assert!(thin(o, red) > thin(p, red) + 0.3, "twin: {} against {}", thin(o, red), thin(p, red));
}

/// A grey ramp from 10 stops under the grey to 10 over develops on every film's curve with its density never falling,
/// and rising across the grey's 3 stops either side, the range every sheet prints. Twin: a table whose curve dips is seen.
#[test]
fn a_silver_negative_densifies_monotonically_along_a_ramp() {
    let ramp: Vec<f32> = (-40..=40).map(|q| GREY * 2f32.powf(q as f32 / 4.0)).collect();
    let rising = |kernel: &VirtualNegative, table: &[[f32; 4]]| {
        let d: Vec<f32> = ramp.iter().map(|v| -cpu::virtual_negative([*v, *v, *v, 1.0], kernel, table)[1].log10()).collect();
        let never_falls = d.windows(2).all(|w| w[1] >= w[0] - 1e-6);
        let mid = &d[28..=52];
        never_falls && mid.windows(2).all(|w| w[1] > w[0])
    };
    for stock in silver() {
        let baked = film::stock(stock);
        assert!(rising(&baked.kernel, &baked.table), "{stock:?}");
    }
    let baked = film::stock(VirtualStock::of("ilford-hp5-plus"));
    let (w, _) = baked.kernel.table_size();
    let mut dipped = baked.table.clone();
    let row = (baked.kernel.tc_nodes + 1) * w;
    for i in 100..140 {
        dipped[row + i] = [0.05; 4];
    }
    assert!(!rising(&baked.kernel, &dipped), "twin: a dip in the curve");
}

/// The GPU's virtual negative on a black and white film against its scalar reference, on scenes reaching every
/// branch. Twins: the scene's light a hundredth stronger, and another film's layer.
#[test]
fn the_gpu_develops_the_silver_negative_its_reference_does() {
    let Some(mut g) = graph("the_gpu_develops_the_silver_negative_its_reference_does") else { return };
    let (w, h) = (41, 29);
    let mut pixels = noise(w, h, 5, -0.05, 6.0);
    let edges: [V4; 7] = [
        [0.0, 0.0, 0.0, 1.0],
        [-0.2, -0.1, -0.3, 1.0],
        [4000.0, 3000.0, 2000.0, 1.0],
        [0.3, -0.05, -0.05, 1.0],
        [-0.05, -0.05, 0.3, 0.5],
        [1e-6, 0.0, 0.0, 1.0],
        [GREY, GREY, GREY, 1.0],
    ];
    pixels[..edges.len()].copy_from_slice(&edges);
    let input = g.upload(w, h, &pixels).unwrap();
    for (stock, other) in [(VirtualStock::of("ilford-ortho-plus"), VirtualStock::of("ilford-fp4-plus")), (VirtualStock::of("kodak-tri-x-400"), VirtualStock::of("foma-ortho-400"))] {
        let baked = film::stock(stock);
        let (tw, th) = baked.kernel.table_size();
        let table = g.upload(tw as u32, th as u32, &baked.table).unwrap();
        let k = baked.kernel_at(-0.6);
        let image = Image::new(&input).then_reading(stages::virtual_negative(&k), &[Image::new(&table)]);
        let out = whole(&mut g, &image);
        let reference = mapped(&pixels, |p| cpu::virtual_negative(p, &k, &baked.table));
        let brighter = VirtualNegative { exposure: k.exposure * 1.01, ..k.clone() };
        judge(&format!("{stock:?}'s negative"), &out, &reference, &mapped(&pixels, |p| cpu::virtual_negative(p, &brighter, &baked.table)));
        let theirs = film::stock(other);
        let twin = mapped(&pixels, |p| cpu::virtual_negative(p, &theirs.kernel_at(-0.6), &theirs.table));
        judge(&format!("{stock:?}'s negative, {other:?}'s layer"), &out, &reference, &twin);
    }
}

/// A digital photo at rest on `stock`, a pixel printing alone.
fn shot(stock: VirtualStock) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true).per_pixel_only().on_stock(stock);
    s.grain.amount = 0.0;
    s.halation.amount = 0.0;
    s
}

/// A digital photo on a black and white film prints in black and white, and is its scan read as a monochrome roll's
/// negative, pixel for pixel, on paper or not; its own settings on a colour film print colour again. Twins: the
/// scan read as a colour negative is another picture, and the colour film's print is no grey.
#[test]
fn a_photo_on_a_black_and_white_film_is_its_negative_read_as_a_monochrome_roll() {
    let Some(mut g) = graph("a_photo_on_a_black_and_white_film_is_its_negative_read_as_a_monochrome_roll") else { return };
    let (w, h) = (32, 12);
    let scene = noise(w, h, 3, 0.0, 1.2);
    let source = g.upload(w, h, &scene).unwrap();
    let mut pipeline = Pipeline::new();
    for paperless in [false, true] {
        let s = PipelineSettings { paperless, ..shot(VirtualStock::of("ilford-hp5-plus")) };
        assert!(s.is_monochrome() && s.mode == ConversionMode::Digital);
        let on = pipeline.render(&mut g, &source, &s, &Request::default(), &Default::default(), None).unwrap().1;
        assert!(on.iter().all(|p| (p[0] - p[1]).abs() < 1e-5 && (p[2] - p[1]).abs() < 1e-5), "a black and white print");
        let image = Image::new(&source).then(stages::color_matrix(&conversion::scene_white(&s).unwrap()));
        let scanned = pipeline.filmed(&mut g, image, &s).unwrap();
        let read_back = whole(&mut g, &scanned);
        let scan = g.upload(w, h, &read_back).unwrap();
        let mut read = |mode| {
            let r = PipelineSettings { mode, ..s.clone() };
            pipeline.render(&mut g, &scan, &r, &Request::default(), &Default::default(), None).unwrap().1
        };
        let (mono, colour) = (read(ConversionMode::Monochrome), read(ConversionMode::Negative));
        judge(&format!("a photo on a black and white film read as a monochrome roll, paperless {paperless}"), &on, &mono, &colour);
    }
    let back = shot(VirtualStock::of("ilford-hp5-plus")).on_stock(VirtualStock::of("kodak-portra-800"));
    assert_eq!(back, shot(VirtualStock::of("kodak-portra-800")), "nothing of the black and white film stays");
    assert!(!back.is_monochrome());
    let coloured = pipeline.render(&mut g, &source, &back, &Request::default(), &Default::default(), None).unwrap().1;
    let chroma = coloured.iter().map(|p| (p[0] - p[2]).abs()).fold(0.0f32, f32::max);
    assert!(chroma > 0.05, "twin: the colour film prints colour, {chroma}");
}

/// Lighting on a photo shot on a black and white film develops that film's own curve again, one layer and no coupler,
/// where a scanned black and white roll develops the C-41 profiles' mean: Exposure moves the grey, Contrast holds it,
/// a filter tints nothing. Twins: the roll's curve is another one, and Exposure's grey is no longer the rest's.
#[test]
fn lighting_develops_a_black_and_white_films_own_curve() {
    use oni_pipeline::negative::{self, Film};
    use oni_settings::tone::Print;
    let stock = VirtualStock::of("ilford-hp5-plus");
    let s = shot(stock);
    assert_eq!(negative::Film::of(&s), Film::Stock(stock));
    let roll = PipelineSettings::rest(ConversionMode::Monochrome, true);
    assert_eq!(negative::Film::of(&roll), Film::Silver);
    let (own, mean) = (negative::development(Film::Stock(stock)), negative::development(Film::Silver));
    assert_eq!(own.couplers, [[0.0; 3]; 3]);
    let profile = film::profile::Silver::read(stock.entry().profile.as_bytes()).unwrap();
    let curve: Vec<[f64; 3]> = profile.density_curve.iter().map(|d| [*d; 3]).collect();
    let sheet = |e: f32| film::bake::interpolate(f64::from(e), &profile.log_exposure, &curve, 0) as f32;
    let off = (0..own.after.len()).map(|i| (own.after[i][1] - sheet(own.log_low + i as f32 * own.log_step)).abs()).fold(0.0f32, f32::max);
    let roll_off = (0..own.after.len()).map(|i| {
        let e = own.log_low + i as f32 * own.log_step;
        let j = (((e - mean.log_low) / mean.log_step).round().max(0.0) as usize).min(mean.after.len() - 1);
        (mean.after[j][1] - sheet(e)).abs()
    });
    let roll_off = roll_off.fold(0.0f32, f32::max);
    println!("HP5's development {off:.4} off its sheet, the roll's mean {roll_off:.3}");
    // A flat foot read back rises at the least slope, a few thousandths over its sheet.
    assert!(off < 1e-2 && roll_off > 0.05, "{off} {roll_off}");
    let baked = film::stock(stock);
    let grey = cpu::virtual_negative([GREY, GREY, GREY, 1.0], &baked.kernel, &baked.table);
    let printed = |p: Print| cpu::stage47(grey, &conversion::stage47(&PipelineSettings { print: p, ..s.clone() }));
    let rest = printed(s.print);
    let lighter = printed(Print { exposure: s.print.exposure + 1.0, ..s.print });
    let harder = printed(Print { contrast: 0.5, ..s.print });
    let mut warm = s.print;
    warm.set_filters(Print::TEMPERATURE_AXIS);
    let warmed = printed(warm);
    println!("the grey at rest {:.4}, a stop of Exposure {:.4}, Contrast +0.5 {:.4}, warmer {:.4}", rest[1], lighter[1], harder[1], warmed[1]);
    assert!((harder[1] / rest[1] - 1.0).abs() < 1e-2, "Contrast holds the grey");
    assert!((warmed[1] - rest[1]).abs() < 1e-4 && (warmed[0] - warmed[2]).abs() < 1e-4, "a filter tints nothing");
    assert!((lighter[1] / rest[1] - 1.0).abs() > 0.1, "twin: Exposure moves the grey");
}
