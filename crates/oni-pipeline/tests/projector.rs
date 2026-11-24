//! A scanned slide on the projector's screen: the GPU's dye solve against its scalar reference on every branch, the bare
//! film projected as the screen's white and a dense one as its veil, the unmeasured bands held, Projected blacks' two
//! ends, the film's black held under every Lighting slider, Lighting through the stock's curve keeping a grey grey, and the
//! editor's chain laying it all. Each verdict has a twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_graph::Image;
use oni_pipeline::film::projection::{self, FLARE};
use oni_pipeline::film::profile::Profile;
use oni_pipeline::{Pipeline, Request, negative, projector, stages};
use oni_render::{DYE_HIGH, DYE_LOW, DyeAmounts, HeldBlack, cpu};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, DyeSet, PipelineSettings, VirtualStock};

const VELVIA: VirtualStock = VirtualStock::of("fujifilm-velvia-100");

/// A slide of the owner's roll, on Velvia 100, its gains putting the backlight on 1.
fn slide() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
    (s.dye_set, s.gains.stops) = (DyeSet::of(VELVIA.key()), [0.035, 0.03, -0.065]);
    s
}

/// The scan pixel whose light under the scanner's lamp, the frame's gains laid, is the model's `light`.
fn scan_of(s: &PipelineSettings, light: [f32; 3]) -> V4 {
    let k = projector::scanned(s, VELVIA);
    [light[0] / k.gain[0], light[1] / k.gain[1], light[2] / k.gain[2], 1.0]
}

/// Each channel's amounts, as the projection codes them, read back.
fn amounts(p: V4) -> V4 {
    [-p[0].max(1e-30).log10(), -p[1].max(1e-30).log10(), -p[2].max(1e-30).log10(), p[3]]
}

#[test]
fn the_gpu_reads_a_scan_back_to_its_dyes_as_its_reference_does() {
    let s = slide();
    let scanned = projection::scanned(VELVIA);
    let k = projector::scanned(&s, VELVIA);
    // The bare film's light reads no dye; each dye alone at unit amount reads that amount back.
    let bare = cpu::amounts([1.0; 3], &k, &scanned.table);
    assert!(bare.iter().all(|a| a.abs() < 1e-3), "{bare:?}");
    let Some(mut g) = graph("the_gpu_reads_a_scan_back_to_its_dyes_as_its_reference_does") else { return };
    let (w, h) = (29, 17);
    let mut pixels: Vec<V4> = noise(w, h, 5, -3.5, 0.2).iter().map(|q| [10f32.powf(q[0]), 10f32.powf(q[1]), 10f32.powf(q[2]), q[3]]).collect();
    // A black scan under the floor, a light past the open lamp under the least amount, a dense one past the most, a
    // saturated colour, alpha carried.
    let open = k.gain.map(|v| 1.0 / v);
    let edges: [V4; 5] = [[0.0, -0.01, 0.0, 1.0], [100.0 * open[0], 100.0 * open[1], 100.0 * open[2], 1.0], [1e-7, 1e-7, 1e-7, 1.0], [0.5, 1e-3, 1e-4, 1.0], [0.2, 0.1, 0.05, 0.25]];
    pixels[..edges.len()].copy_from_slice(&edges);
    let reference = mapped(&pixels, |q| cpu::dye_amounts(q, &k, &scanned.table));
    let read = |p: V4| amounts(p);
    assert!((0..3).all(|c| (read(reference[1])[c] - DYE_LOW).abs() < 1e-6 && (read(reference[2])[c] - DYE_HIGH).abs() < 1e-3), "{:?} {:?}", read(reference[1]), read(reference[2]));
    let source = g.upload(w, h, &pixels).unwrap();
    let table = g.upload(scanned.kernel.bands as u32, 2, &scanned.table).unwrap();
    let out = whole(&mut g, &Image::new(&source).then_reading(stages::dye_amounts(&k), &[Image::new(&table)]));
    // Judged on the amounts, so a dense pixel weighs as a clear one. Twin: the scan read at the roll's gains a hair off.
    let off = DyeAmounts { gain: k.gain.map(|v| v * 1.01), ..k.clone() };
    judge("a scan read back to its dyes", &mapped(&out, read), &mapped(&reference, read), &mapped(&pixels, |q| read(cpu::dye_amounts(q, &off, &scanned.table))));
    // A table whose dyes absorb nothing moves no amount: its first guess stands, as the GPU's does.
    let clear: Vec<V4> = scanned.table.iter().enumerate().map(|(i, v)| if i < scanned.kernel.bands { [0.0, 0.0, 0.0, v[3]] } else { *v }).collect();
    let still = g.upload(scanned.kernel.bands as u32, 2, &clear).unwrap();
    let held = whole(&mut g, &Image::new(&source).then_reading(stages::dye_amounts(&k), &[Image::new(&still)]));
    let guessed = mapped(&pixels, |q| cpu::dye_amounts(q, &k, &clear));
    judge("a solve no dye moves", &mapped(&held, read), &mapped(&guessed, read), &mapped(&reference, read));
}

#[test]
fn the_bare_film_is_the_screens_white_and_a_dense_one_its_veil() {
    let p = projection::projected(VELVIA);
    let bare = cpu::projection([1.0; 4], &p.projection, &p.table);
    assert!((0..3).all(|c| (bare[c] - 1.0 - FLARE).abs() < 1e-3), "{bare:?}");
    // The film's densest shows as the veil and a hair more: its held dyes stop the lamp's ends.
    let densest = p.black.map(|v| -(v - FLARE).log10());
    println!("Velvia 100's densest as the screen shows it: D {densest:?}");
    assert!(densest.iter().all(|d| *d > 3.0) && p.black.iter().all(|v| *v < 1.2 * FLARE), "{:?}", p.black);
    // Twin: the bands the sheet leaves unmeasured read as clear, the former table, let the lamp's ends through.
    let profile = Profile::read(VELVIA.entry().profile.as_bytes()).unwrap();
    let n = p.projection.bands;
    let unheld: Vec<[f32; 4]> = (0..2 * n)
        .map(|i| match (i < n, profile.channel_density.get(i)) {
            (true, Some(d)) => [d[0].unwrap_or(0.0) as f32, d[1].unwrap_or(0.0) as f32, d[2].unwrap_or(0.0) as f32, profile.base_density[i].unwrap_or(0.0) as f32],
            _ => p.table[i],
        })
        .collect();
    let curves = profile.density_curves.iter().fold([0.0f64; 3], |m, d| std::array::from_fn(|c| m[c].max(d[c])));
    let dense = [10f32.powf(-curves[0] as f32), 10f32.powf(-curves[1] as f32), 10f32.powf(-curves[2] as f32), 1.0];
    let leaky = cpu::projection(dense, &p.projection, &unheld);
    let leaked: [f32; 3] = std::array::from_fn(|c| -(leaky[c] - FLARE).log10());
    println!("the same black with the unmeasured bands clear: D {leaked:?}");
    assert!(leaked[0] < 2.8 && leaked[2] < 2.8, "{leaked:?}");
}

#[test]
fn projected_blacks_keep_the_screens_ends_and_off_lay_them_on_pure_black_and_white() {
    let s = slide();
    let p = projection::projected(VELVIA);
    let curves = Profile::read(VELVIA.entry().profile.as_bytes()).unwrap().density_curves;
    let densest: [f32; 3] = std::array::from_fn(|c| curves.iter().map(|d| d[c]).fold(f64::NEG_INFINITY, f64::max) as f32);
    let black = [10f32.powf(-densest[0]), 10f32.powf(-densest[1]), 10f32.powf(-densest[2]), 1.0];
    let (pure_black, pure_white) = (projector::shown(black, &s, VELVIA), projector::shown([1.0; 4], &s, VELVIA));
    assert!((0..3).all(|c| pure_black[c].abs() < 1e-4 && (pure_white[c] - 1.0).abs() < 1e-3), "{pure_black:?} {pure_white:?}");
    let on = PipelineSettings { print: Print { projected_blacks: true, ..s.print }, ..s.clone() };
    let (screen_black, screen_white) = (projector::shown(black, &on, VELVIA), projector::shown([1.0; 4], &on, VELVIA));
    assert!((0..3).all(|c| (screen_black[c] - p.black[c]).abs() < 1e-6 && (screen_white[c] - 1.0 - FLARE).abs() < 1e-3));
    // Twin: the screen's black is no pure black.
    assert!(screen_black.iter().take(3).all(|v| *v > FLARE * 0.9), "{screen_black:?}");
    // Lighting leaves the ends where they lie: the film's densest holds under Exposure, the bare film is the white.
    let lit = PipelineSettings { print: Print { exposure: 1.0, ..s.print }, ..s.clone() };
    let lit_black = projector::shown(black, &lit, VELVIA);
    assert!((0..3).all(|c| lit_black[c].abs() < 2e-3), "{lit_black:?}");
}

#[test]
fn the_films_black_stays_where_the_slide_at_rest_shows_it_under_every_exposure_and_the_mids_follow_whole() {
    let s = slide();
    let p = projection::projected(VELVIA);
    let curves = Profile::read(VELVIA.entry().profile.as_bytes()).unwrap().density_curves;
    let densest: [f32; 3] = std::array::from_fn(|c| curves.iter().map(|d| d[c]).fold(f64::NEG_INFINITY, f64::max) as f32);
    let coded = |a: [f32; 3]| [10f32.powf(-a[0]), 10f32.powf(-a[1]), 10f32.powf(-a[2]), 1.0];
    // The film's densest, and a scan's black a little lighter, as the owner's Velvia roll holds it: each held within 5e-5
    // of its rest, a twentieth of L* 1, where unheld Exposure +2 lifts it by more than 2e-3.
    let blacks = [coded(densest), coded([2.70, 3.40, 2.87])];
    let at = |s: &PipelineSettings, e: f32| PipelineSettings { print: Print { exposure: e, ..s.print }, ..s.clone() };
    // Today's projection of Lighting, unheld: the slide re-exposed and shown as it lands.
    let unheld = |s: &PipelineSettings, q: V4| {
        let shown = cpu::projection(projector::lighting(s, VELVIA).map_or(q, |n| cpu::negative(q, &n)), &p.projection, &p.table);
        projector::ends(s, VELVIA).map_or(shown, |m| cpu::color_matrix(shown, &m))
    };
    for veiled in [false, true] {
        let s = PipelineSettings { print: Print { projected_blacks: veiled, ..s.print }, ..s.clone() };
        for black in blacks {
            let rest = projector::shown(black, &s, VELVIA);
            for e in [-2.0f32, -1.0, 1.0, 2.0] {
                let held = projector::shown(black, &at(&s, e), VELVIA);
                assert!((0..3).all(|c| (held[c] - rest[c]).abs() < 5e-5), "Projected blacks {veiled}, {e:+}: {held:?} {rest:?}");
            }
            // Twin: unheld, Exposure +2 lifts that black into a colour, its blue the most.
            let lifted = unheld(&at(&s, 2.0), black);
            println!("Projected blacks {veiled}: black {:?} held at {:?}, unheld at +2 {:?}", &black[..3], &rest[..3], &lifted[..3]);
            assert!(lifted[2] - rest[2] > 2e-3 && lifted[2] > lifted[1], "{lifted:?}");
        }
    }
    // A tone the slide shows past the reach at rest follows Lighting whole, within the float's rounding.
    let scanned = projection::scanned(VELVIA);
    let k = projector::scanned(&s, VELVIA);
    let mids: Vec<V4> = noise(17, 11, 6, 0.08, 0.9).iter().map(|q| cpu::dye_amounts(*q, &k, &scanned.table)).collect();
    let mut followed = 0;
    for e in [-2.0f32, -1.0, 1.0, 2.0] {
        let lit = at(&s, e);
        for q in &mids {
            let rest = projector::shown(*q, &s, VELVIA);
            if (0..3).map(|c| oni_render::LUMA[c] * rest[c]).sum::<f32>() < projector::held(&s, VELVIA).reach {
                continue;
            }
            let (held, whole) = (projector::shown(*q, &lit, VELVIA), unheld(&lit, *q));
            assert!((0..3).all(|c| (held[c] - whole[c]).abs() <= 1e-6 * (1.0 + whole[c].abs())), "{e:+}: {held:?} {whole:?}");
            followed += 1;
        }
    }
    assert!(followed > 300, "{followed}");
    // Twin: a deep shadow, under the reach at rest, takes only part of the move.
    let deep = cpu::dye_amounts([0.004, 0.003, 0.004, 1.0], &k, &scanned.table);
    let (held, whole, rest) = (projector::shown(deep, &at(&s, 2.0), VELVIA), unheld(&at(&s, 2.0), deep), projector::shown(deep, &s, VELVIA));
    println!("a deep shadow at +2: rest {:?}, held {:?}, unheld {:?}", &rest[..3], &held[..3], &whole[..3]);
    assert!(held[1] > rest[1] && held[1] - rest[1] < 0.5 * (whole[1] - rest[1]));
}

#[test]
fn the_gpu_holds_a_slides_black_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_holds_a_slides_black_as_its_reference_does") else { return };
    let (w, h) = (23, 13);
    // The rest from under its black to past the reach, the lit picture anywhere, so every branch of the share is taken.
    let rest: Vec<V4> = noise(w, h, 4, -0.01, 0.08).iter().map(|q| [q[0], q[1], q[2], 1.0]).collect();
    let lit: Vec<V4> = noise(w, h, 5, 0.0, 1.0).iter().map(|q| [q[0], q[1], q[2], 0.5]).collect();
    let k = HeldBlack { black: 0.005, reach: 0.044 };
    let (a, b) = (g.upload(w, h, &lit).unwrap(), g.upload(w, h, &rest).unwrap());
    let out = whole(&mut g, &Image::new(&a).then_reading(stages::held_black(&k), &[Image::new(&b)]));
    let reference: Vec<V4> = lit.iter().zip(&rest).map(|(l, r)| cpu::held_black(*l, *r, &k)).collect();
    // Twin: the reach doubled.
    let twin = HeldBlack { reach: 0.088, ..k };
    judge("a slide's black held", &out, &reference, &lit.iter().zip(&rest).map(|(l, r)| cpu::held_black(*l, *r, &twin)).collect::<Vec<_>>());
}

#[test]
fn lighting_re_exposes_the_slide_through_its_films_curve_and_a_grey_stays_grey() {
    let s = slide();
    let scanned = projection::scanned(VELVIA);
    let (le, tone) = projection::tone(VELVIA);
    let k = projector::scanned(&s, VELVIA);
    let lit = PipelineSettings { print: Print { exposure: 1.0, ..s.print }, ..s.clone() };
    let n = negative::slide(&lit, VELVIA).expect("Exposure moves the slide");
    let at = |e: f64| std::array::from_fn::<f32, 3, _>(|c| oni_pipeline::film::bake::interpolate(e, &le, &tone, c) as f32);
    for e in [-1.0f64, -0.5, -0.25, 0.0, 0.5] {
        // A grey of the film at `e`, its amounts as the sheet's green layer lays a neutral.
        let grey = at(e);
        let coded = [10f32.powf(-grey[0]), 10f32.powf(-grey[1]), 10f32.powf(-grey[2]), 1.0];
        let moved = amounts(cpu::negative(coded, &n));
        let want = at(e + std::f64::consts::LOG10_2);
        println!("a grey at log exposure {e:+}: {grey:?} to {:?}, its sheet a stop over {want:?}", &moved[..3]);
        assert!((0..3).all(|c| (moved[c] - want[c]).abs() < 1e-3), "{moved:?} {want:?}");
        // Past the reach the black is held under, its screen is the film's grey a stop over, as the projector's lamp shows
        // it: brighter, its colour that grey's.
        let veiled = PipelineSettings { print: Print { projected_blacks: true, ..lit.print }, ..lit.clone() };
        let rest = PipelineSettings { print: Print { exposure: 0.0, ..veiled.print }, ..veiled.clone() };
        let at_rest = projector::shown(coded, &rest, VELVIA);
        if (0..3).map(|c| oni_render::LUMA[c] * at_rest[c]).sum::<f32>() < projector::held(&veiled, VELVIA).black + projector::held(&veiled, VELVIA).reach {
            continue;
        }
        let shown = projector::shown(coded, &veiled, VELVIA);
        let over = projector::shown([10f32.powf(-want[0]), 10f32.powf(-want[1]), 10f32.powf(-want[2]), 1.0], &rest, VELVIA);
        println!("  its screen: {:?}, the grey a stop over at rest {:?}", &shown[..3], &over[..3]);
        assert!((0..3).all(|c| (shown[c] / over[c] - 1.0).abs() < 1e-3) && shown[1] > projector::shown(coded, &rest, VELVIA)[1], "{shown:?}");
    }
    // A grey scanned reads back on the film's neutral: its amounts lie on the tone the Lighting reads.
    let scan_grey = cpu::amounts([0.1; 3], &k, &scanned.table);
    let e = (0..le.len()).min_by(|a, b| (tone[*a][1] - f64::from(scan_grey[1])).abs().total_cmp(&(tone[*b][1] - f64::from(scan_grey[1])).abs())).unwrap();
    assert!((0..3).all(|c| (tone[e][c] as f32 - scan_grey[c]).abs() < 0.05), "{scan_grey:?} {:?}", tone[e]);
    // Twin: each layer read back and developed again on its own sheet curve, rather than on the neutral the green layer
    // lays, turns the grey a stop over off its colour.
    let profile = Profile::read(VELVIA.entry().profile.as_bytes()).unwrap();
    let (sheet_le, curves) = (&profile.log_exposure, &profile.density_curves);
    let falling: Vec<[f64; 3]> = curves.iter().map(|d| d.map(|v| -v)).collect();
    let les: Vec<[f64; 3]> = sheet_le.iter().map(|e| [*e; 3]).collect();
    let grey = at(0.0);
    let own: [f32; 3] = std::array::from_fn(|c| {
        let column: Vec<f64> = falling.iter().map(|d| d[c]).collect();
        let e = oni_pipeline::film::bake::interpolate(-f64::from(grey[c]), &column, &les, c) + std::f64::consts::LOG10_2;
        oni_pipeline::film::bake::interpolate(e, sheet_le, curves, c) as f32
    });
    let veiled = PipelineSettings { print: Print { projected_blacks: true, ..s.print }, ..s.clone() };
    let screen = |a: [f32; 3]| projector::shown([10f32.powf(-a[0]), 10f32.powf(-a[1]), 10f32.powf(-a[2]), 1.0], &veiled, VELVIA);
    let (by_layer, neutral) = (screen(own), screen(at(std::f64::consts::LOG10_2)));
    let apart = (0..3).map(|c| (by_layer[c] / neutral[c] - 1.0).abs()).fold(0.0f32, f32::max);
    println!("a grey a stop over, each layer on its own curve: {:?}, on the neutral {:?}, {apart:.3} apart", &by_layer[..3], &neutral[..3]);
    assert!(apart > 2e-2, "{apart}");
    // Twin: at rest Lighting lays nothing.
    assert!(negative::slide(&s, VELVIA).is_none());
}

#[test]
fn the_editors_chain_projects_a_scanned_slide_and_a_measure_reads_its_scan_through_the_windows() {
    let s = slide().per_pixel_only();
    assert!(s.projector() == Some(VELVIA) && s.paper().is_none() && !s.takes_paper());
    // A slide whose roll names no profiled reversal film projects on the default one.
    assert_eq!(PipelineSettings::rest(ConversionMode::Positive, true).projector(), Some(VirtualStock::SLIDE));
    let Some(mut g) = graph("the_editors_chain_projects_a_scanned_slide_and_a_measure_reads_its_scan_through_the_windows") else { return };
    let (w, h) = (23, 13);
    let mut pixels: Vec<V4> = noise(w, h, 3, -3.0, 0.0).iter().map(|q| [10f32.powf(q[0]), 10f32.powf(q[1]), 10f32.powf(q[2]), 1.0]).collect();
    pixels[0] = scan_of(&s, [1.0; 3]);
    let source = g.upload(w, h, &pixels).unwrap();
    let scanned = projection::scanned(VELVIA);
    let clipped = |q: V4| q.map(|v| v.clamp(0.0, 1.0));
    let expected = |s: &PipelineSettings| mapped(&pixels, |q| clipped(projector::shown(cpu::dye_amounts(q, &projector::scanned(s, VELVIA), &scanned.table), s, VELVIA)));
    let mut pipeline = Pipeline::new();
    let mut render = |s: &PipelineSettings, request: &Request| pipeline.render(&mut g, &source, s, request, &Default::default(), None).unwrap().1;
    let shown = render(&s, &Request::default());
    assert!(shown[0][..3].iter().all(|v| (v - 1.0).abs() < 2e-3), "the bare film on pure white: {:?}", shown[0]);
    // Twin: the scan read without its gains.
    let ungained = PipelineSettings { gains: Default::default(), ..s.clone() };
    judge("a slide projected", &shown, &expected(&s), &expected(&ungained));
    // Lighting and the colour tools on the dyes, Projected blacks on.
    let mut lit = s.clone();
    (lit.print.exposure, lit.print.contrast, lit.print.shadows, lit.print.white_point, lit.density) = (0.7, 0.2, 0.1, 0.03, 0.3);
    lit.print.set_filters([0.1, 0.0, -0.15]);
    lit.print.projected_blacks = true;
    lit.saturation.points[1].y = 0.7;
    judge("a lit slide projected", &render(&lit, &Request::default()), &expected(&lit), &expected(&s));
    // A measure upstream of the print reads the scan through the windows, as stage 4-7 does any frame's.
    let measuring = Request { measuring: true, ..Request::default() };
    let bypassed = PipelineSettings { print: Print { bypassed: true, ..s.print }, ..s.clone() };
    let read = render(&bypassed, &measuring);
    let k = oni_pipeline::conversion::stage47(&bypassed);
    judge("a slide measured", &read, &mapped(&pixels, |q| cpu::stage47(q, &k)), &expected(&s));
}
