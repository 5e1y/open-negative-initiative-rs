//! The neighbourhood probe and the detail tracks it judges: chroma denoise and sharpening on the reference roll.

use super::*;

mod probe {
    use oni_settings::axis::pipeline::LUMA_WEIGHTS;

    /// Large enough to carry sky, grain and edges, small enough that sixteen radii stay quick.
    pub const SIDE: usize = 1024;

    /// A centred square of the full-resolution decode: centred, since picking the busiest tile would grade the film.
    pub fn region(full: &oni_decode::Linear) -> (Vec<[f32; 4]>, usize, f64) {
        let cut = SIDE.min(full.width.min(full.height));
        let (x0, y0) = ((full.width - cut) / 2, (full.height - cut) / 2);
        let px = (0..cut).flat_map(|y| full.pixels[(y0 + y) * full.width + x0..(y0 + y) * full.width + x0 + cut].to_vec()).collect();
        (px, cut, full.width.max(full.height) as f64)
    }

    /// Luma per pixel and the three departures from grey, as both stages split them.
    pub fn planes(px: &[[f32; 4]]) -> (Vec<f32>, Vec<f32>) {
        let w = LUMA_WEIGHTS;
        let luma: Vec<f32> = px.iter().map(|p| p[0] * w[0] + p[1] * w[1] + p[2] * w[2]).collect();
        let chroma = px.iter().zip(&luma).flat_map(|(p, &y)| [p[0] - y, p[1] - y, p[2] - y]).collect();
        (luma, chroma)
    }

    /// Flat and edge pixels, cut on the luma gradient's median and top decile, the border in neither.
    pub fn split(luma: &[f32], side: usize) -> (Vec<bool>, Vec<bool>) {
        let mut gradient = vec![0.0f32; side * side];
        for y in 1..side - 1 {
            for x in 1..side - 1 {
                let i = y * side + x;
                gradient[i] = (luma[i + 1] - luma[i - 1]).abs() + (luma[i + side] - luma[i - side]).abs();
            }
        }
        let mut sorted = gradient.clone();
        sorted.sort_by(f32::total_cmp);
        let (median, decile) = (sorted[sorted.len() / 2], sorted[sorted.len() * 9 / 10]);
        let (mut flat, mut edges) = (vec![false; side * side], vec![false; side * side]);
        for y in 1..side - 1 {
            for x in 1..side - 1 {
                let i = y * side + x;
                flat[i] = gradient[i] <= median;
                edges[i] = gradient[i] >= decile;
            }
        }
        (flat, edges)
    }

    /// Mean chroma step between horizontal neighbours; on the flat half it is the noise the stage removes.
    pub fn neighbour_spread(chroma: &[f32], side: usize, mask: &[bool]) -> f32 {
        let (mut total, mut counted) = (0.0f64, 0usize);
        for y in 0..side {
            for x in 0..side - 1 {
                let i = y * side + x;
                if mask[i] && mask[i + 1] {
                    (0..3).for_each(|c| total += f64::from((chroma[(i + 1) * 3 + c] - chroma[i * 3 + c]).abs()));
                    counted += 3;
                }
            }
        }
        if counted == 0 { 0.0 } else { (total / counted as f64) as f32 }
    }

    /// RMS move of the masked pixels; on edges it is colour carried across a boundary.
    pub fn displacement(before: &[f32], after: &[f32], mask: &[bool]) -> f32 {
        let (mut total, mut counted) = (0.0f64, 0usize);
        for (i, _) in mask.iter().enumerate().filter(|m| *m.1) {
            (0..3).for_each(|c| total += f64::from(after[i * 3 + c] - before[i * 3 + c]).powi(2));
            counted += 3;
        }
        if counted == 0 { 0.0 } else { (total / counted as f64).sqrt() as f32 }
    }

    /// Mean luma step between horizontal neighbours; on edges it is the contour sharpening buys.
    pub fn step(luma: &[f32], side: usize, mask: &[bool]) -> f32 {
        let (mut total, mut counted) = (0.0f64, 0usize);
        for y in 0..side {
            for x in 0..side - 1 {
                let i = y * side + x;
                if mask[i] && mask[i + 1] {
                    total += f64::from((luma[i + 1] - luma[i]).abs());
                    counted += 1;
                }
            }
        }
        if counted == 0 { 0.0 } else { (total / counted as f64) as f32 }
    }

    /// The band the source holds in a 3 px square window, its edge clamped.
    pub fn morphology(luma: &[f32], side: usize, maximum: bool) -> Vec<f32> {
        let at = |x: i64, y: i64| luma[(y.clamp(0, side as i64 - 1) as usize) * side + x.clamp(0, side as i64 - 1) as usize];
        (0..side * side)
            .map(|i| {
                let (x, y) = ((i % side) as i64, (i / side) as i64);
                let window = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| at(x + dx, y + dy));
                if maximum { window.fold(f32::MIN, f32::max) } else { window.fold(f32::MAX, f32::min) }
            })
            .collect()
    }

    /// The share of a curve's rise past a radius.
    pub fn beyond(sweep: &[f64], curve: &[f32], past: f64) -> f64 {
        let Some(&total) = curve.last().filter(|&&t| t > 0.0 && sweep.len() == curve.len()) else { return 0.0 };
        let here = sweep.iter().position(|&r| r >= past).map_or(curve[curve.len() - 1], |i| curve[i]);
        f64::from(total - here) / f64::from(total)
    }

    /// Geometric from a quarter pixel, sixteen points, sampling the bottom of the range finely.
    pub fn sweep(top: f64) -> Vec<f64> {
        let step = (top / 0.25).powf(1.0 / 15.0);
        (0..16).map(|k| 0.25 * step.powi(k)).collect()
    }

    /// Where a curve's rise per pixel falls to a tenth of its best, read after the peak, and that best.
    pub fn exhaustion(sweep: &[f64], curve: &[f32]) -> (f64, f64) {
        let (mut mids, mut rate) = (Vec::new(), Vec::new());
        for i in 0..sweep.len() - 1 {
            let span = sweep[i + 1] - sweep[i];
            if span > 0.0 {
                mids.push((sweep[i] + sweep[i + 1]) / 2.0);
                rate.push(f64::from(curve[i + 1] - curve[i]) / span);
            }
        }
        let peak = rate.iter().copied().fold(f64::MIN, f64::max);
        if peak <= 0.0 {
            return (0.0, 0.0);
        }
        let wanted = 0.1 * peak;
        let crest = rate.iter().position(|&r| r == peak).unwrap_or(0);
        for i in crest + 1..rate.len() {
            if rate[i] <= wanted {
                let (high, low) = (rate[i - 1], rate[i]);
                let share = if high > low { (high - wanted) / (high - low) } else { 0.0 };
                return (mids[i - 1] + share * (mids[i] - mids[i - 1]), peak);
            }
        }
        (mids[mids.len() - 1], peak)
    }
}

/// `Pipeline.applyDetail` alone at full resolution: the rows pass, then the columns beside the source.
fn detailed(g: &mut Graph, source: &oni_graph::Source, k: Option<oni_render::Detail>) -> Vec<[f32; 4]> {
    let image = oni_graph::Image::new(source);
    let extent = image.extent();
    let Some(k) = k else { return g.render(&image, extent, None).unwrap() };
    let rows = image.clone().then(oni_pipeline::stages::DetailRows(k));
    g.render(&image.then_reading(oni_pipeline::stages::DetailColumns(k), &[rows]), extent, None).unwrap()
}

/// The first frame of the reference roll, decoded whole, or `None` after a skip line.
fn reference_frame(check: &str) -> Option<(String, oni_decode::Linear)> {
    if !oni_fixtures::heavy(check) {
        return None;
    }
    let dir = oni_fixtures::raw_dir(check)?;
    let mut files: Vec<_> = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "NEF")).collect();
    files.sort();
    let file = files.first()?;
    Some((file.file_name()?.to_string_lossy().into_owned(), oni_decode::decode(file, 1.0).ok()?))
}

// MARK: - ChromaDenoise.selfCheck(source:)

#[test]
fn the_chroma_track_reaches_the_films_dose_and_stops_short_of_smearing() {
    let check = "the_chroma_track_reaches_the_films_dose_and_stops_short_of_smearing";
    let Some((name, full)) = reference_frame(check) else { return };
    let Some(mut g) = graph(check) else { return };
    let (px, side, long_side) = probe::region(&full);
    let source = g.upload(side as u32, side as u32, &px).unwrap();
    let (luma, before) = probe::planes(&px);
    let (flat, edges) = probe::split(&luma, side);
    let mut measure = |radius: f32| {
        let dose = oni_settings::blocks::ChromaDenoise { radius, force: 1.0 };
        let (_, after) = probe::planes(&detailed(&mut g, &source, finishing::detail(Some(&dose), None, 1.0)));
        (probe::neighbour_spread(&after, side, &flat), probe::displacement(&before, &after, &edges))
    };
    let sweep = probe::sweep(48.0);
    let rest = measure(0.0);
    let (mut noise, mut bleed) = (Vec::new(), Vec::new());
    for &radius in &sweep {
        let point = measure(radius as f32);
        noise.push(rest.0 - point.0);
        bleed.push(point.1);
        println!("{name}: radius {radius:8.2} px, noise removed {:.6}, colour displaced at edges {:.6}", rest.0 - point.0, point.1);
    }
    let (reach, peak) = probe::exhaustion(&sweep, &noise);
    assert!(reach > 0.0, "denoising is never spent ({peak:.2e} per px at its peak)");
    let top = f64::from(oni_settings::blocks::ChromaDenoise::RADIUS_RANGE.1);
    assert!(top >= reach, "the track runs to {top} px and the film's dose sits at {reach:.2}");
    let noise_left = probe::beyond(&sweep, &noise, top);
    let smear_left = probe::beyond(&sweep, &bleed, top);
    assert!(smear_left >= 2.0 * noise_left, "past {top} px, {:.1} % noise left against {:.1} % smear", noise_left * 100.0, smear_left * 100.0);
    // Twin: a stop at the floor a preview leaves inert would leave most of the noise unreachable.
    let at_floor = probe::beyond(&sweep, &noise, pipeline::preview_visible_radius());
    assert!(at_floor > noise_left * 2.0, "a stop at the floor leaves {:.1} % against {:.1} %", at_floor * 100.0, noise_left * 100.0);
    // Twin: a radius of 0.01 of the long side, fixed to the frame, holds that dose inside its first tenth.
    assert!(reach / (0.01 * long_side) < 0.1);
}

// MARK: - Sharpen.selfCheck(source:)

#[test]
fn the_sharpening_track_reaches_the_films_edge_and_stops_short_of_halo() {
    let check = "the_sharpening_track_reaches_the_films_edge_and_stops_short_of_halo";
    let Some((name, full)) = reference_frame(check) else { return };
    let Some(mut g) = graph(check) else { return };
    let (px, side, long_side) = probe::region(&full);
    let source = g.upload(side as u32, side as u32, &px).unwrap();
    let (luma, _) = probe::planes(&px);
    let (_, edges) = probe::split(&luma, side);
    let (ceiling, base) = (probe::morphology(&luma, side, true), probe::morphology(&luma, side, false));
    let mut measure = |radius: f64| {
        let dose = oni_settings::blocks::Sharpen { radius: radius as f32, amount: 1.0, ..Default::default() };
        let (after, _) = probe::planes(&detailed(&mut g, &source, finishing::detail(None, Some(&dose), 1.0)));
        let (mut rim, mut counted) = (0.0f64, 0usize);
        for i in (0..side * side).filter(|&i| edges[i]) {
            rim += f64::from((after[i] - ceiling[i]).max(0.0) + (base[i] - after[i]).max(0.0));
            counted += 1;
        }
        (probe::step(&after, side, &edges), if counted == 0 { 0.0 } else { (rim / counted as f64) as f32 })
    };
    let sweep = probe::sweep(32.0);
    let rest = probe::step(&luma, side, &edges);
    let (mut contour, mut rim) = (Vec::new(), Vec::new());
    for &radius in &sweep {
        let point = measure(radius);
        contour.push(point.0 - rest);
        rim.push(point.1);
        println!("{name}: radius {radius:8.2} px, edge lifted {:.6}, rim outside the band {:.6}", point.0 - rest, point.1);
    }
    let (reach, peak) = probe::exhaustion(&sweep, &contour);
    assert!(reach > 0.0, "the edge is never spent ({peak:.2e} per px at its peak)");
    let (floor, top) = oni_settings::blocks::Sharpen::radius_range();
    let top = f64::from(top);
    assert!(top >= reach, "the track runs to {top} px and the film's edge is spent at {reach:.2}");
    let edge_left = probe::beyond(&sweep, &contour, top);
    let halo_left = probe::beyond(&sweep, &rim, top);
    assert!(edge_left < 0.2 && halo_left >= edge_left, "past {top} px, {:.1} % edge left against {:.1} % rim", edge_left * 100.0, halo_left * 100.0);
    // Twin: a stop at the bottom of the track would leave most of the edge unlifted.
    let at_floor = probe::beyond(&sweep, &contour, f64::from(floor));
    assert!(at_floor > edge_left * 2.0, "a stop at {floor} px leaves {:.1} % against {:.1} %", at_floor * 100.0, edge_left * 100.0);
    // Twin: a track of 0.002 of the long side, fixed to the frame, runs far past the edge's own answer.
    assert!(top / (0.002 * long_side) < 0.75);
}
