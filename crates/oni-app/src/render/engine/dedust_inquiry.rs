//! `ONI_DEDUST_INQUIRY=<folder>`: a frame's dust patches read at each stage and each definition the canvas
//! shows, dumped as raw floats for a measure outside; `ONI_DEDUST_INQUIRY_FILE` names the frame, `_OUT` the dump.
use super::*;
use oni_pipeline::corrections::PatchKey;
use std::io::Write;

struct Dump {
    out: PathBuf,
    index: std::fs::File,
}

impl Dump {
    fn put(&mut self, name: &str, region: Rect, pixels: &[[f32; 4]], scale: f64, points: &[(f64, f64)], radius: f64) {
        let data: &[u8] = bytemuck::cast_slice(pixels);
        std::fs::write(self.out.join(format!("{name}.f32")), data).expect("a dump");
        let pts: Vec<String> = points.iter().map(|(x, y)| format!("[{:.3},{:.3}]", x - f64::from(region.x), y - f64::from(region.y))).collect();
        writeln!(self.index, "{{\"name\":\"{name}\",\"w\":{},\"h\":{},\"scale\":{scale},\"radius\":{radius},\"points\":[{}]}}", region.w, region.h, pts.join(",")).expect("the index");
    }
}

#[test]
fn a_frame_s_dust_patches_read_at_each_stage_and_definition() {
    let Some(folder) = std::env::var_os("ONI_DEDUST_INQUIRY") else { return };
    let folder = PathBuf::from(folder);
    let Ok(file) = std::env::var("ONI_DEDUST_INQUIRY_FILE") else { return };
    let raw = folder.join(file);
    let out = PathBuf::from(std::env::var("ONI_DEDUST_INQUIRY_OUT").expect("a dump folder"));
    std::fs::create_dir_all(&out).unwrap();
    let mut dump = Dump { index: std::fs::File::create(out.join("index.jsonl")).unwrap(), out: out.clone() };
    let settings = oni_settings::sidecar::read(&raw, true).expect("the sidecar").settings;
    let device = Device::open().expect("a GPU");
    let mut engine = Engine::on(&device);
    engine.caches = std::env::var_os("ONI_DEDUST_INQUIRY_CACHES").map(PathBuf::from);
    engine.support = std::env::var_os("ONI_DEDUST_INQUIRY_SUPPORT").map(PathBuf::from);
    let reading = Reading::of(&settings);
    let negative = engine.prepare(decode(&raw, reading.clone()).expect("the preview decode")).expect("the preview");
    let full = engine.full(decode_full(&raw, reading).expect("the full decode")).expect("the full frame");
    let fe = full.extent();
    // Patches the caches lack are searched as the editor searches them, each on the frame healed of those before.
    if let Some(caches) = engine.caches.clone() {
        for (k, c) in settings.corrections.iter().enumerate() {
            let upstream = PipelineSettings { corrections: settings.corrections[..k].to_vec(), ..settings.clone() };
            if crate::patches::file(&caches, PatchKey::of(&settings, c)).exists() {
                continue;
            }
            let (region, size) = engine.search_region(&full, &upstream, c).expect("the region");
            let patch = crate::heal::generate(&region, c, size.0, size.1);
            crate::patches::write(&caches, PatchKey::of(&settings, c), patch.width as u32, patch.height as u32, &patch.pixels).expect("written");
            engine.forget_patch(PatchKey::of(&settings, c));
        }
    }
    println!("preview {:?}, full {:?}, full_width {}", negative.preview.extent(), fe, negative.full_width);
    for c in &settings.corrections {
        let key = PatchKey::of(&settings, c);
        println!("correction {} reading {:016x} patch on disk {}", c.id, key.reading, engine.caches.as_ref().is_some_and(|d| crate::patches::file(d, key).exists()));
    }
    let bare = PipelineSettings { corrections: Vec::new(), ..settings.clone() };
    // The strokes' points on the scan, in full pixels, and the box the measure reads about them.
    let pts: Vec<(f64, f64)> = settings.corrections.iter().flat_map(|c| c.stroke.iter().map(|p| (p.x * f64::from(fe.w), p.y * f64::from(fe.h)))).collect();
    let radius = settings.corrections.first().map_or(10.0, |c| c.radius);
    let (cx, cy) = (pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64, pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64);
    let half = 300.0;
    let placing = settings.placing((f64::from(fe.w), f64::from(fe.h)));
    let on_frame: Vec<(f64, f64)> = settings.corrections.iter().flat_map(|c| c.stroke.iter().map(|p| placing.on_frame(*p))).map(|p| (p.x, p.y)).collect();
    // Each definition: its source, read at `reduce`, its scale against the full frame.
    let sources: Vec<(&str, Source, f64)> = vec![("full", full.clone(), 1.0), ("preview", negative.preview.clone(), f64::from(negative.preview.extent().w) / negative.full_width)];
    for (label, source, base) in &sources {
        for want in [1.0, 0.5, 0.25, 0.125, 0.0625] {
            if want > *base + 1e-9 {
                continue;
            }
            let reduce = want / base;
            let request = Request { scale: reduce, full_width: Some(negative.full_width), ..Request::default() };
            let tag = format!("{label}-{want}");
            for (which, s) in [("with", &settings), ("bare", &bare)] {
                engine.read_custom(s);
                let inputs = inputs_of(&engine.patches, &engine.custom, s, negative.seed, None);
                let (graph, pipeline) = (&mut engine.graph, &mut engine.pipeline);
                let mut input = Image::new(source);
                if reduce != 1.0 {
                    input = input.lanczos(reduce);
                }
                let e = input.extent();
                let scale = f64::from(e.w) / negative.full_width;
                let around = Rect::new(((cx - half) * scale) as i32, ((cy - half) * scale) as i32, (2.0 * half * scale) as u32, (2.0 * half * scale) as u32);
                let scaled: Vec<(f64, f64)> = pts.iter().map(|p| (p.0 * scale, p.1 * scale)).collect();
                let patched = pipeline.patched(graph, input.clone(), s, scale, &inputs).unwrap();
                let px = graph.render(&patched, around, None).unwrap();
                dump.put(&format!("{tag}-{which}-patched"), around, &px, scale, &scaled, radius * scale);
                let healed = pipeline.healed(graph, input.clone(), s, scale, &inputs).unwrap();
                let px = graph.render(&healed, around, None).unwrap();
                dump.put(&format!("{tag}-{which}-healed"), around, &px, scale, &scaled, radius * scale);
                let image = pipeline.image(graph, source, s, &request, &inputs).unwrap();
                let ie = image.extent();
                let mapped: Vec<(f64, f64)> = on_frame.iter().map(|p| (f64::from(ie.x) + p.0 * f64::from(ie.w), f64::from(ie.y) + p.1 * f64::from(ie.h))).collect();
                let (mx, my) = (mapped.iter().map(|p| p.0).sum::<f64>() / mapped.len() as f64, mapped.iter().map(|p| p.1).sum::<f64>() / mapped.len() as f64);
                let r = (half * scale).max(8.0);
                let at = Rect::from_corners(((mx - r) as i32).max(ie.x), ((my - r) as i32).max(ie.y), ((mx + r) as i32).min(ie.right()), ((my + r) as i32).min(ie.bottom()));
                let px = graph.render(&image, at, None).unwrap();
                dump.put(&format!("{tag}-{which}-output"), at, &px, scale, &mapped, radius * scale);
                if want <= 0.125 {
                    let px = graph.render(&image, ie, None).unwrap();
                    dump.put(&format!("{tag}-{which}-whole"), ie, &px, scale, &mapped, radius * scale);
                }
                graph.purge();
            }
        }
    }
}

#[test]
fn a_frame_s_dust_patches_searched_again() {
    let Some(folder) = std::env::var_os("ONI_DEDUST_INQUIRY") else { return };
    let Ok(file) = std::env::var("ONI_DEDUST_INQUIRY_FILE") else { return };
    let raw = PathBuf::from(folder).join(file);
    let settings = oni_settings::sidecar::read(&raw, true).expect("the sidecar").settings;
    let device = Device::open().expect("a GPU");
    let mut engine = Engine::on(&device);
    engine.caches = std::env::var_os("ONI_DEDUST_INQUIRY_CACHES").map(PathBuf::from);
    let full = engine.full(decode_full(&raw, Reading::of(&settings)).expect("the full decode")).expect("the full frame");
    for (k, c) in settings.corrections.iter().enumerate() {
        let upstream = PipelineSettings { corrections: settings.corrections[..k].to_vec(), ..settings.clone() };
        let (region, size) = engine.search_region(&full, &upstream, c).expect("the region");
        let patch = crate::heal::generate(&region, c, size.0, size.1);
        let mean = |px: &[[f32; 4]]| -> [f32; 3] { std::array::from_fn(|i| px.iter().map(|p| p[i]).sum::<f32>() / px.len() as f32) };
        println!("{} rect {:?} offset px ({:.0}, {:.0}) fallback {} patch mean {:?}", c.id, patch.rect, patch.offset.0 * size.0, -patch.offset.1 * size.1, patch.fallback, mean(&patch.pixels));
        if let Some(caches) = &engine.caches
            && let Some((_, _, px)) = crate::patches::read(caches, PatchKey::of(&settings, c))
        {
            println!("  owner's patch mean {:?}", mean(&px));
        }
    }
}

/// Strokes laid at random on a frame's clean ground, where the scan itself is the answer: each patch's mean under its
/// stroke against the scan's, in density, as `generate` makes it and as its wide-ring twin did. `_FILES` lists the frames.
#[test]
fn random_strokes_healed_on_clean_ground() {
    let Some(folder) = std::env::var_os("ONI_DEDUST_INQUIRY") else { return };
    let Ok(files) = std::env::var("ONI_DEDUST_INQUIRY_FILES") else { return };
    let device = Device::open().expect("a GPU");
    let mut engine = Engine::on(&device);
    let (mut ours, mut theirs) = (Vec::new(), Vec::new());
    for name in files.split(',') {
        let raw = PathBuf::from(&folder).join(name);
        let settings = oni_settings::sidecar::read(&raw, true).map(|l| l.settings).unwrap_or_else(|_| PipelineSettings::rest(oni_settings::ConversionMode::Negative, true));
        let full = engine.full(decode_full(&raw, Reading::of(&settings)).expect("the full decode")).expect("the full frame");
        let (w, h) = (f64::from(full.extent().w), f64::from(full.extent().h));
        let mut state = 0x9e37_79b9_7f4a_7c15u64 ^ name.len() as u64;
        let mut next = move || {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        for k in 0..36 {
            let radius = [8.0, 20.0, 40.0][k % 3];
            let (x, y) = (0.1 + 0.8 * next(), 0.1 + 0.8 * next());
            use oni_settings::{cg::Point, frame::Correction};
            let c = Correction { radius, ..Correction::new(vec![Point::new(x, y), Point::new(x + radius / w, y + radius / h)]) };
            let one = PipelineSettings { corrections: vec![c.clone()], ..settings.clone() };
            let (region, size) = engine.search_region(&full, &one, &c).expect("the region");
            let under = |p: &crate::heal::Patch| -> [f64; 3] {
                let (mut patched, mut scan, mut n) = ([0.0f64; 3], [0.0f64; 3], 0.0);
                for row in 0..p.height {
                    for col in 0..p.width {
                        let (px, py) = (p.rect[0] + col as f64 + 0.5, p.rect[1] + row as f64 + 0.5);
                        let (ax, ay, bx, by) = (x * size.0, y * size.1, (x + radius / w) * size.0, (y + radius / h) * size.1);
                        let t = (((px - ax) * (bx - ax) + (py - ay) * (by - ay)) / ((bx - ax).powi(2) + (by - ay).powi(2))).clamp(0.0, 1.0);
                        if (px - ax - t * (bx - ax)).hypot(py - ay - t * (by - ay)) > radius {
                            continue;
                        }
                        let i = (py as i64 - region.y) as usize * region.width + (px as i64 - region.x) as usize;
                        for ch in 0..3 {
                            patched[ch] += f64::from(p.pixels[row * p.width + col][ch]);
                            scan[ch] += f64::from(region.pixels[i][ch]);
                        }
                        n += 1.0;
                    }
                }
                std::array::from_fn(|ch| 1000.0 * (patched[ch] / n).max(1e-6).log10() - 1000.0 * (scan[ch] / n).max(1e-6).log10())
            };
            ours.push(under(&crate::heal::generate(&region, &c, size.0, size.1)));
            theirs.push(under(&crate::heal::inquiry::on_ring(&region, &c, size.0, size.1)));
            engine.purge();
        }
    }
    let said = |title: &str, v: &[[f64; 3]]| {
        let mut worst: Vec<f64> = v.iter().map(|e| e.iter().fold(0.0f64, |a, b| a.max(b.abs()))).collect();
        worst.sort_by(f64::total_cmp);
        let q = |f: f64| worst[((worst.len() - 1) as f64 * f) as usize];
        let mean: [f64; 3] = std::array::from_fn(|ch| v.iter().map(|e| e[ch]).sum::<f64>() / v.len() as f64);
        println!("{title:<10} {} strokes: worst channel |ΔD| median {:.1} mD, p90 {:.1}, max {:.1}; mean signed ΔD {:?}", v.len(), q(0.5), q(0.9), q(1.0), mean.map(|m| (m * 10.0).round() / 10.0));
    };
    said("band", &ours);
    said("wide ring", &theirs);
}

/// Strokes on clean ground of each kind of photo, stored and moved (flat-field, gains, crop, black and white), printed at
/// 1, 1/4 and 1/16: the print under each against the bare frame's, in sRGB code. `_TOUR` lists the frames.
#[test]
fn the_tour_of_de_dust() {
    use oni_settings::{cg::Point, frame::Correction};
    let Some(folder) = std::env::var_os("ONI_DEDUST_INQUIRY") else { return };
    let Ok(files) = std::env::var("ONI_DEDUST_INQUIRY_TOUR") else { return };
    let device = Device::open().expect("a GPU");
    let mut engine = Engine::on(&device);
    engine.support = std::env::var_os("ONI_DEDUST_INQUIRY_SUPPORT").map(PathBuf::from);
    for name in files.split(',') {
        let raw = PathBuf::from(&folder).join(name);
        let stored = oni_settings::sidecar::read(&raw, true).expect("the sidecar").settings;
        let reading = Reading::of(&stored);
        let negative = engine.prepare(decode(&raw, reading.clone()).expect("the preview")).expect("the preview laid");
        let full = engine.full(decode_full(&raw, reading).expect("the full decode")).expect("the full frame");
        let (w, h) = (f64::from(full.extent().w), f64::from(full.extent().h));
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let strokes: Vec<Correction> = (0..9)
            .map(|k| {
                let (radius, x, y) = ([8.0, 20.0, 40.0][k % 3], 0.2 + 0.6 * next(), 0.2 + 0.6 * next());
                Correction { radius, ..Correction::new(vec![Point::new(x, y), Point::new(x + radius / w, y + radius / h)]) }
            })
            .collect();
        let caches = std::env::temp_dir().join(format!("oni-dedust-tour-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&caches);
        engine.caches = Some(caches.clone());
        for (k, c) in strokes.iter().enumerate() {
            let upstream = PipelineSettings { corrections: strokes[..k].to_vec(), ..stored.clone() };
            let (region, size) = engine.search_region(&full, &upstream, c).expect("the region");
            let patch = crate::heal::generate(&region, c, size.0, size.1);
            crate::patches::write(&caches, PatchKey::of(&stored, c), patch.width as u32, patch.height as u32, &patch.pixels).expect("written");
            engine.purge();
        }
        let mut variants: Vec<(&str, PipelineSettings)> = vec![("as stored", stored.clone())];
        let mut moved = stored.clone();
        moved.flat_field.amount = 0.0;
        variants.push(("no flat-field", moved));
        let mut moved = stored.clone();
        moved.gains.stops = [moved.gains.stops[0] + 0.4, moved.gains.stops[1], moved.gains.stops[2] - 0.3];
        variants.push(("gains moved", moved));
        let mut moved = stored.clone();
        moved.geometry.crop = oni_settings::cg::Rect { x: 0.15, y: 0.12, width: 0.7, height: 0.78 };
        moved.geometry.angle += 2.5;
        variants.push(("crop, angle", moved));
        if stored.mode == oni_settings::ConversionMode::Negative {
            variants.push(("black and white", PipelineSettings { mode: oni_settings::ConversionMode::Monochrome, ..stored.clone() }));
        }
        for (title, s) in variants {
            let with = PipelineSettings { corrections: strokes.clone(), ..s.clone() };
            let placing = s.placing((w, h));
            let mut line = format!("{name:<14} {title:<16}");
            for (label, source, want) in [("100 %", full.clone(), 1.0), ("1/4", full.clone(), 0.25), ("1/16", negative.preview.clone(), 0.0625)] {
                let base = f64::from(source.extent().w) / negative.full_width;
                let request = Request { scale: want / base, full_width: Some(negative.full_width), ..Request::default() };
                let mut diffs: Vec<f64> = Vec::new();
                let mut odd = 0;
                let images: Vec<Vec<[f32; 4]>> = [&with, &s]
                    .iter()
                    .map(|set| {
                        engine.read_custom(set);
                        let inputs = inputs_of(&engine.patches, &engine.custom, set, negative.seed, None);
                        let image = engine.pipeline.image(&mut engine.graph, &source, set, &request, &inputs).expect("the print");
                        let e = image.extent();
                        let px = engine.graph.render(&image, e, None).expect("rendered");
                        engine.graph.purge();
                        let mut out = vec![[0.0f32; 4]; 0];
                        out.push([e.x as f32, e.y as f32, e.w as f32, e.h as f32]);
                        out.extend(px);
                        out
                    })
                    .collect();
                let e = images[0][0];
                for c in &strokes {
                    let mid = Point::new(c.stroke.iter().map(|p| p.x).sum::<f64>() / 2.0, c.stroke.iter().map(|p| p.y).sum::<f64>() / 2.0);
                    let f = placing.on_frame(mid);
                    let r = (c.radius * want * 0.6).max(0.6);
                    let (cx, cy) = (f.x * f64::from(e[2]), f.y * f64::from(e[3]));
                    if cx < r + 2.0 || cy < r + 2.0 || cx > f64::from(e[2]) - r - 2.0 || cy > f64::from(e[3]) - r - 2.0 {
                        continue;
                    }
                    let (mut a, mut b, mut area) = ([0.0f64; 3], [0.0f64; 3], 0.0);
                    for y in (cy - r).floor() as i64..=(cy + r).ceil() as i64 {
                        for x in (cx - r).floor() as i64..=(cx + r).ceil() as i64 {
                            if ((x as f64 + 0.5 - cx).hypot(y as f64 + 0.5 - cy)) > r + 0.5 {
                                continue;
                            }
                            let i = 1 + (y as usize) * e[2] as usize + x as usize;
                            area += 1.0;
                            for ch in 0..3 {
                                a[ch] += f64::from(images[0][i][ch]);
                                b[ch] += f64::from(images[1][i][ch]);
                            }
                        }
                    }
                    let n = a.iter().zip(&b).filter(|(x, y)| !x.is_finite() || !y.is_finite()).count();
                    odd += n;
                    let code = |v: f64| 255.0 * v.clamp(0.0, 1.0).powf(1.0 / 2.2);
                    diffs.push((0..3).map(|ch| (code(a[ch] / area) - code(b[ch] / area)).abs()).fold(0.0, f64::max));
                }
                diffs.sort_by(f64::total_cmp);
                let (median, worst) = (diffs.get(diffs.len() / 2).copied().unwrap_or(0.0), diffs.last().copied().unwrap_or(0.0));
                line += &format!(" | {label} n{} median {median:.1} worst {worst:.1}{}", diffs.len(), if odd > 0 { " NON-FINITE" } else { "" });
            }
            println!("{line}");
        }
        let _ = std::fs::remove_dir_all(&caches);
    }
}
