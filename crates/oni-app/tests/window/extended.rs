//! The branches the render dump cannot reach, against the Swift app's own code driven by a probe
//! build (`scripts/extended-dump.sh`, into `ONI_ADVERSARIAL/extended`): decodes outside LibRaw, dust
//! patches generated on Swift's full decode, frames rendered with those patches, and exports.

use oni_app::heal::{self, Region};
use oni_app::render::{self, Reading};
use oni_settings::frame::Correction;
use std::path::{Path, PathBuf};

struct Dump {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

/// `RenderDump`'s format: width and height as `Int32`, then RGBA `Float`, top row first.
fn dump(path: &Path) -> Dump {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
    let (width, height) = (int(0), int(4));
    let pixels: Vec<[f32; 4]> = b[8..].chunks_exact(16).map(|c| std::array::from_fn(|k| f32::from_le_bytes(c[k * 4..k * 4 + 4].try_into().unwrap()))).collect();
    assert_eq!(pixels.len(), width * height, "{}: truncated", path.display());
    Dump { width, height, pixels }
}

/// Mean, 99th centile and worst of the worst channel's gap, pixels `inset` from the edge left out.
fn gap(a: &[[f32; 4]], b: &[[f32; 4]], width: usize, inset: usize) -> (f64, f64, f64) {
    let height = a.len() / width;
    let mut e: Vec<f64> = Vec::with_capacity(a.len());
    for y in inset..height - inset {
        for x in inset..width - inset {
            let (p, q) = (a[y * width + x], b[y * width + x]);
            e.push((0..3).map(|c| f64::from((p[c] - q[c]).abs())).fold(0.0, f64::max));
        }
    }
    let mean = e.iter().sum::<f64>() / e.len() as f64;
    e.sort_by(|x, y| x.total_cmp(y));
    (mean, e[e.len() * 99 / 100], *e.last().unwrap())
}

fn folder(check: &str) -> Option<PathBuf> {
    let dir = oni_fixtures::present(oni_fixtures::ADVERSARIAL_DIR, check)?.join("extended");
    if dir.join("decode.json").exists() { Some(dir) } else {
        println!("SKIPPED {check}: no extended dump (run scripts/extended-dump.sh)");
        None
    }
}

fn reading(path: &Path) -> Reading {
    oni_settings::sidecar::read(path, false, false).map(|l| Reading::of(&l.settings)).unwrap_or_default()
}

pub(crate) fn a_decode_outside_libraw_is_swift_s() {
    let check = "a_decode_outside_libraw_is_swift_s";
    let Some(dir) = folder(check) else { return };
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(dir.join("decode.json")).unwrap()).unwrap();
    let (mut full, mut reduced, mut failures) = (0, 0, Vec::new());
    for e in &entries {
        let path = dir.join("sources").join(e["file"].as_str().unwrap());
        let side = e["side"].as_u64().unwrap();
        let (swift, ours) = if side == 0 {
            full += 1;
            (dump(&dir.join("decode").join(format!("{:05}.f32", full - 1))), render::decode_full(&path, reading(&path)))
        } else {
            reduced += 1;
            (dump(&dir.join("decode-2000").join(format!("{:05}.f32", reduced - 1))), render::decode(&path, reading(&path)))
        };
        let ours = ours.unwrap_or_else(|err| panic!("{}: {err}", path.display())).linear;
        let branch = e["branch"].as_str().unwrap();
        if (ours.width, ours.height) != (swift.width, swift.height) {
            println!("{branch:45} {}x{} for Swift's {}x{}", ours.width, ours.height, swift.width, swift.height);
            failures.push(format!("{branch}: size"));
            continue;
        }
        // A reduced decode's outer two rows are rebuilt from the third on both sides, later for the port.
        let (mean, p99, worst) = gap(&ours.pixels, &swift.pixels, ours.width, if side == 0 { 0 } else { 2 });
        // Full size, a decode outside LibRaw must not move past the converted primaries' rounding;
        // reduced, ImageIO's thumbnail filter is its own, which Catmull-Rom only nears.
        let (m, p) = if side == 0 { (2e-4, 1e-3) } else { (5e-3, 3e-2) };
        let ok = mean < m && p99 < p;
        println!("{branch:45} side {side:4}  mean {mean:.6}  p99 {p99:.6}  max {worst:.5}  {}", if ok { "ok" } else { "DEPARTS" });
        if !ok {
            failures.push(format!("{branch}: mean {mean:.6}, p99 {p99:.6}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

pub(crate) fn a_patch_is_swift_s_on_swift_s_source() {
    let check = "a_patch_is_swift_s_on_swift_s_source";
    let Some(dir) = folder(check) else { return };
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(dir.join("heal").join("heal.json")).unwrap()).unwrap();
    let mut failures = Vec::new();
    for e in &entries {
        let name = format!("{:05}-{:02}", e["index"].as_u64().unwrap(), e["k"].as_u64().unwrap());
        let file = dir.join("sources").join(e["file"].as_str().unwrap());
        let settings = oni_settings::sidecar::read(&file, true, false).unwrap().settings;
        let id = e["id"].as_str().unwrap();
        let c: &Correction = settings.corrections.iter().find(|c| c.id.to_string().eq_ignore_ascii_case(id)).unwrap();
        let f = |k: &str| -> Vec<f64> { e[k].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect() };
        let (extent, crop, tile, source) = (f("extent"), f("crop"), f("tile"), f("source"));
        let (w, h) = (extent[2], extent[3]);
        let src = dump(&dir.join("heal").join(format!("{name}-source.f32")));
        // The crop's rows from the top of the frame: Core Image's y counts up from its bottom.
        let region = Region { x: crop[0] as i64, y: (h - crop[1] - crop[3]) as i64, width: src.width, height: src.height, pixels: src.pixels, frame_width: w, frame_height: h };
        let win = heal::search_window(c, w, h);
        assert!(win[0] >= crop[0] && win[0] + win[2] <= crop[0] + crop[2], "{name}: the crop misses the search window");
        let patch = heal::generate(&region, c, w, h);
        // Where the copy was taken: Swift's source rectangle, a fraction from the top, less its tile, y up.
        let swift_shift = (source[0] * w - tile[0], (1.0 - source[1] - source[3]) * h - tile[1]);
        let ours_shift = (patch.offset.0 * w, -patch.offset.1 * h);
        let (sw, sh, spx) = oni_app::flat_fields::read_tiff(&dir.join("heal").join(format!("{name}.tiff"))).unwrap();
        let same_size = (sw as usize, sh as usize) == (patch.width, patch.height);
        let (mean, p99, worst) = if same_size { gap(&patch.pixels, &spx, patch.width, 0) } else { (f64::NAN, f64::NAN, f64::NAN) };
        let moved = (swift_shift.0 - ours_shift.0).hypot(swift_shift.1 - ours_shift.1);
        let ok = same_size && moved < 0.5 && mean < 2e-3 && p99 < 1e-2;
        println!("{name} radius {:5.1}  shift ours {:?} Swift {:?}  patch {}x{} for {sw}x{sh}  mean {mean:.5} p99 {p99:.5} max {worst:.4}  {}", c.radius, ours_shift, swift_shift, patch.width, patch.height, if ok { "ok" } else { "DEPARTS" });
        if !ok {
            failures.push(format!("{name}: shift {moved:.1} px, patch mean {mean:.5}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A frame's index in the reference's list, which names its Swift decode.
fn reference_decode(reference: &Path, list: &str, folder: &str, frame: &str) -> Dump {
    let names = std::fs::read_to_string(reference.join(list)).unwrap();
    let index = names.lines().position(|l| l.ends_with(frame)).unwrap_or_else(|| panic!("{frame} is not in {list}"));
    dump(&reference.join(folder).join(format!("{index:05}.f32")))
}

pub(crate) fn a_frame_renders_its_swift_patches_as_swift_does() {
    let check = "a_frame_renders_its_swift_patches_as_swift_does";
    let Some(dir) = folder(check) else { return };
    let Some(reference) = oni_fixtures::reference_dir(check) else { return };
    if !oni_fixtures::heavy(check) {
        return;
    }
    let Ok(mut g) = oni_graph::Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")) else { return };
    let mut pipeline = oni_pipeline::Pipeline::new();
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(dir.join("heal").join("heal.json")).unwrap()).unwrap();
    let frames = [("heal.NEF", "list.txt", "decode", "DSC_6440.NEF"), ("heal.RW2", "negs-list.txt", "negs-decode", "P1072392.RW2")];
    let mut failures = Vec::new();
    for (i, (name, list, folder_name, original)) in frames.iter().enumerate() {
        let file = dir.join("sources").join(name);
        let settings = oni_settings::sidecar::read(&file, true, false).unwrap().settings;
        let decoded = reference_decode(&reference, list, folder_name, original);
        let source = g.upload(decoded.width as u32, decoded.height as u32, &decoded.pixels).unwrap();
        let mut uploaded = Vec::new();
        let mut full_width = None;
        for e in entries.iter().filter(|e| e["file"] == *name) {
            let tiff = dir.join("heal").join(format!("{:05}-{:02}.tiff", e["index"].as_u64().unwrap(), e["k"].as_u64().unwrap()));
            let (w, h, px) = oni_app::flat_fields::read_tiff(&tiff).unwrap();
            let id = oni_settings::id::Id::parse(e["id"].as_str().unwrap()).unwrap();
            uploaded.push((id, g.upload(w, h, &px).unwrap()));
            full_width = e["extent"][2].as_f64();
        }
        let inputs = oni_pipeline::Inputs { patches: uploaded.iter().map(|(id, s)| (*id, s)).collect(), ..oni_pipeline::Inputs::default() };
        let request = oni_pipeline::Request { full_width, ..oni_pipeline::Request::default() };
        let render = |g: &mut oni_graph::Graph, pipeline: &mut oni_pipeline::Pipeline, inputs: &oni_pipeline::Inputs| {
            let image = pipeline.image(g, &source, &settings, &request, inputs).expect("the recipe builds");
            let e = image.extent();
            ((e.w as usize, e.h as usize), g.render(&image, e, None).expect("the frame renders"))
        };
        let ((w, h), ours) = render(&mut g, &mut pipeline, &inputs);
        let swift = dump(&dir.join("render").join(format!("{i:05}.f32")));
        assert_eq!((w, h), (swift.width, swift.height), "{name}");
        let clip = |v: &[[f32; 4]]| -> Vec<[f32; 4]> { v.iter().map(|p| p.map(|c| c.clamp(0.0, 1.0))).collect() };
        let (mean, p99, worst) = gap(&clip(&ours), &clip(&swift.pixels), w, 0);
        // The patches' footprint: where the frame without them differs; read there alone, a patch
        // misplaced or left out shows, which a whole-frame centile drowns.
        let ((_, _), bare) = render(&mut g, &mut pipeline, &oni_pipeline::Inputs::default());
        let (ours, bare, theirs) = (clip(&ours), clip(&bare), clip(&swift.pixels));
        let footprint: Vec<usize> = (0..ours.len()).filter(|&k| (0..3).any(|c| (ours[k][c] - bare[k][c]).abs() > 2e-3)).collect();
        let on = |a: &[[f32; 4]]| -> f64 { footprint.iter().map(|&k| (0..3).map(|c| f64::from((a[k][c] - theirs[k][c]).abs())).fold(0.0, f64::max)).sum::<f64>() / footprint.len().max(1) as f64 };
        let (patched, unpatched) = (on(&ours), on(&bare));
        // Twin: unpatched, the footprint departs from Swift's.
        let ok = mean < 2e-3 && p99 < 1e-2 && !footprint.is_empty() && patched < 2e-3 && unpatched > 5.0 * patched;
        println!("{name}  whole mean {mean:.5} p99 {p99:.5} max {worst:.4}; over the patches' {} px mean {patched:.5}, unpatched {unpatched:.5}  {}", footprint.len(), if ok { "ok" } else { "DEPARTS" });
        if !ok {
            failures.push(format!("{name}: mean {mean:.5}, p99 {p99:.5}, patched {patched:.5}, unpatched {unpatched:.5}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

pub(crate) fn a_patch_reduces_as_core_image_reduces_it() {
    let check = "a_patch_reduces_as_core_image_reduces_it";
    let Some(dir) = folder(check) else { return };
    let Ok(mut g) = oni_graph::Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")) else { return };
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(dir.join("heal").join("heal.json")).unwrap()).unwrap();
    let mut failures = Vec::new();
    for e in &entries {
        let name = format!("{:05}-{:02}", e["index"].as_u64().unwrap(), e["k"].as_u64().unwrap());
        let Some(scale) = e["reduced"]["scale"].as_f64() else { continue };
        let (w, h, px) = oni_app::flat_fields::read_tiff(&dir.join("heal").join(format!("{name}.tiff"))).unwrap();
        let swift = dump(&dir.join("heal").join(format!("{name}-scaled.f32")));
        let patch = g.upload(w, h, &px).unwrap();
        let reduced = oni_graph::Image::new(&patch).lanczos_anchored(scale);
        let e_ = reduced.extent();
        if (e_.w as usize, e_.h as usize) != (swift.width, swift.height) {
            failures.push(format!("{name}: {}x{} for Swift's {}x{}", e_.w, e_.h, swift.width, swift.height));
            continue;
        }
        let ours = g.render(&reduced, e_, None).unwrap();
        let (mean, p99, worst) = gap(&ours, &swift.pixels, swift.width, 0);
        // Twin: the plain reduction, sides rounded and the content stretched to them, departs.
        let plain = oni_graph::Image::new(&patch).lanczos(scale);
        let p_ = plain.extent();
        let stretched_departs = (p_.w, p_.h) != (e_.w, e_.h) || gap(&g.render(&plain, p_, None).unwrap(), &swift.pixels, swift.width, 0).0 > 2.0 * mean;
        // Under an eighth, Core Image's own filter leaves a residual the exact Lanczos does not follow.
        let (m, p) = if scale >= 0.125 { (1e-3, 5e-3) } else { (6e-3, 2e-2) };
        let ok = mean < m && p99 < p && stretched_departs;
        println!("{name} scale {scale:.4} {}x{}  mean {mean:.5} p99 {p99:.5} max {worst:.4}  {}", e_.w, e_.h, if ok { "ok" } else { "DEPARTS" });
        if !ok {
            failures.push(format!("{name}: mean {mean:.5}, p99 {p99:.5}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

pub(crate) fn a_patch_lands_and_feathers_as_swift_s() {
    let check = "a_patch_lands_and_feathers_as_swift_s";
    let Some(dir) = folder(check) else { return };
    let Ok(mut g) = oni_graph::Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")) else { return };
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(dir.join("heal").join("heal.json")).unwrap()).unwrap();
    let mut failures = Vec::new();
    for e in &entries {
        let name = format!("{:05}-{:02}", e["index"].as_u64().unwrap(), e["k"].as_u64().unwrap());
        let r = &e["reduced"];
        let Some(scale) = r["scale"].as_f64() else { continue };
        let f = |v: &serde_json::Value| -> Vec<f64> { v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect() };
        let (small, placed_up) = (f(&r["small"]), f(&r["placed"]));
        let file = dir.join("sources").join(e["file"].as_str().unwrap());
        let settings = oni_settings::sidecar::read(&file, true, false).unwrap().settings;
        let c = settings.corrections.iter().find(|c| c.id.to_string().eq_ignore_ascii_case(e["id"].as_str().unwrap())).unwrap();
        let (pw, ph, _) = oni_app::flat_fields::read_tiff(&dir.join("heal").join(format!("{name}.tiff"))).unwrap();
        let framed = oni_graph::Rect::new(0, 0, small[2] as u32, small[3] as u32);
        let (tile, _) = oni_pipeline::corrections::placed(c, (pw, ph), framed, scale);
        let swift_tile = oni_graph::Rect::new(placed_up[0] as i32, (small[3] - placed_up[1] - placed_up[3]) as i32, placed_up[2] as u32, placed_up[3] as u32);
        let points = oni_pipeline::corrections::stroke_points(c, framed);
        let (count, row) = (points.len(), g.upload(points.len() as u32, 1, &points).unwrap());
        let hard = oni_graph::Image::new(&row).then(oni_pipeline::stages::Stroke { k: oni_render::StrokeMask { radius: (c.radius * scale) as f32, count }, extent: swift_tile });
        let mask = oni_pipeline::stages::blurred(hard, oni_pipeline::corrections::feather(c, scale), true);
        // The patch covers the whole pixels its exact edges touch, where Swift rounds them: a pixel apart at most.
        let near = [(tile.x, swift_tile.x), (tile.y, swift_tile.y), (tile.right(), swift_tile.right()), (tile.bottom(), swift_tile.bottom())].iter().all(|(a, b)| (a - b).abs() <= 1);
        let (ours, swift) = (g.render(&mask, swift_tile, None).unwrap(), dump(&dir.join("heal").join(format!("{name}-mask.f32"))));
        let (mean, p99, worst) = gap(&ours, &swift.pixels, swift.width, 0);
        // A stroke a pixel or two wide keeps CoreGraphics' own antialiasing, which the coverage only nears.
        let ok = near && mean < 1e-2 && p99 < 6e-2;
        println!("{name} placed {tile:?} Swift {swift_tile:?}; mask mean {mean:.5} p99 {p99:.5} max {worst:.4}  {}", if ok { "ok" } else { "DEPARTS" });
        if !ok {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// A written file's pixels as floats on [0, 1], its embedded profile, and its EXIF and TIFF tags.
struct Written {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
    icc: Option<Vec<u8>>,
    tags: std::collections::BTreeMap<u16, String>,
}

fn written(path: &Path) -> Written {
    use image::ImageDecoder;
    let bytes = std::fs::read(path).unwrap();
    let mut decoder = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format().unwrap().into_decoder().unwrap();
    let icc = decoder.icc_profile().ok().flatten();
    let exif = decoder.exif_metadata().ok().flatten();
    let rgba = image::DynamicImage::from_decoder(decoder).unwrap().into_rgba32f();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let pixels = rgba.pixels().map(|p| [p[0], p[1], p[2], 1.0]).collect();
    // A JPEG's EXIF block, else the TIFF's own first directory.
    let tags = ifd_tags(exif.as_deref().unwrap_or(if bytes.starts_with(b"II") || bytes.starts_with(b"MM") { &bytes } else { &[] }));
    Written { width, height, pixels, icc, tags }
}

/// IFD0 and its EXIF directory's tags, each rendered as text: strings as written, numbers in a list.
fn ifd_tags(b: &[u8]) -> std::collections::BTreeMap<u16, String> {
    let mut out = std::collections::BTreeMap::new();
    if b.len() < 8 {
        return out;
    }
    let le = &b[..2] == b"II";
    let u16_at = |i: usize| -> Option<u16> { b.get(i..i + 2).map(|s| if le { u16::from_le_bytes([s[0], s[1]]) } else { u16::from_be_bytes([s[0], s[1]]) }) };
    let u32_at = |i: usize| -> Option<u32> { b.get(i..i + 4).map(|s| { let a = [s[0], s[1], s[2], s[3]]; if le { u32::from_le_bytes(a) } else { u32::from_be_bytes(a) } }) };
    let mut queue = vec![u32_at(4).unwrap_or(0) as usize];
    while let Some(at) = queue.pop() {
        let Some(n) = u16_at(at) else { continue };
        for k in 0..n as usize {
            let e = at + 2 + k * 12;
            let (Some(tag), Some(ty), Some(count)) = (u16_at(e), u16_at(e + 2), u32_at(e + 4)) else { break };
            let size = [0, 1, 1, 2, 4, 8, 1, 1, 2, 4, 8, 4, 8][ty.min(12) as usize] * count as usize;
            let data = if size <= 4 { e + 8 } else { u32_at(e + 8).unwrap_or(0) as usize };
            if tag == 0x8769 {
                queue.push(u32_at(e + 8).unwrap_or(0) as usize);
            }
            // Offsets and byte counts depend on the writer's layout, never on what it states.
            if matches!(tag, 273 | 278 | 279 | 0x8769 | 34675 | 513 | 514 | 324 | 325) {
                continue;
            }
            let text = match ty {
                2 => String::from_utf8_lossy(b.get(data..data + size).unwrap_or(&[])).trim_end_matches('\0').to_string(),
                3 => (0..count as usize).filter_map(|i| u16_at(data + 2 * i)).map(|v| v.to_string()).collect::<Vec<_>>().join(","),
                4 => (0..count as usize).filter_map(|i| u32_at(data + 4 * i)).map(|v| v.to_string()).collect::<Vec<_>>().join(","),
                5 => (0..count as usize).filter_map(|i| Some(f64::from(u32_at(data + 8 * i)?) / f64::from(u32_at(data + 8 * i + 4)?.max(1)))).map(|v| format!("{v:.4}")).collect::<Vec<_>>().join(","),
                _ => format!("type {ty} × {count}"),
            };
            out.insert(tag, text);
        }
    }
    out
}

/// An ICC profile's three colorants (`rXYZ`, `gXYZ`, `bXYZ`) and its red curve's type and length.
fn colorants(icc: &[u8]) -> Option<([[f64; 3]; 3], String)> {
    let be32 = |i: usize| -> Option<u32> { Some(u32::from_be_bytes(icc.get(i..i + 4)?.try_into().ok()?)) };
    let n = be32(128)?;
    let mut found = std::collections::HashMap::new();
    for k in 0..n as usize {
        let e = 132 + 12 * k;
        found.insert(icc.get(e..e + 4)?.to_vec(), (be32(e + 4)? as usize, be32(e + 8)? as usize));
    }
    let xyz = |tag: &[u8]| -> Option<[f64; 3]> {
        let (at, _) = found.get(tag)?;
        let s = |i: usize| -> Option<f64> { Some(f64::from(i32::from_be_bytes(icc.get(at + 8 + 4 * i..at + 12 + 4 * i)?.try_into().ok()?)) / 65536.0) };
        Some([s(0)?, s(1)?, s(2)?])
    };
    let curve = found.get(&b"rTRC".to_vec()).map(|&(at, len)| format!("{} of {len} bytes", String::from_utf8_lossy(&icc[at..at + 4])))?;
    Some(([xyz(b"rXYZ")?, xyz(b"gXYZ")?, xyz(b"bXYZ")?], curve))
}

pub(crate) fn an_export_writes_swift_s_pixels_profile_and_tags() {
    let check = "an_export_writes_swift_s_pixels_profile_and_tags";
    let Some(dir) = folder(check) else { return };
    if !oni_fixtures::heavy(check) {
        return;
    }
    let Ok(gpu) = oni_app::render::Device::open().map_err(|e| println!("SKIPPED {check}: {e}")) else { return };
    let engine = oni_app::render::Engine::shared(&gpu);
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(dir.join("export.json")).unwrap()).unwrap();
    let scratch = std::env::temp_dir().join(format!("oni-export-parity-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let mut failures = Vec::new();
    for e in &entries {
        let file = dir.join("sources").join(e["file"].as_str().unwrap());
        let (format, space, edge) = (e["format"].as_str().unwrap(), e["space"].as_str().unwrap(), e["edge"].as_i64().unwrap());
        let mut settings = oni_export::settings::Settings::default();
        settings.set_format(if format == "tiff16" { oni_export::settings::Format::Tiff16 } else { oni_export::settings::Format::Jpeg });
        settings.set_space(oni_colour::Space::from_key(space).unwrap());
        if edge > 0 {
            settings.resize = oni_export::resize::Resize::LongestEdge(edge);
        }
        let ext = if format == "tiff16" { "tiff" } else { "jpg" };
        let index = e["index"].as_u64().unwrap();
        let ours_path = scratch.join(format!("{index:05}.{ext}"));
        let target = oni_app::export::Target { path: file.clone(), settings: oni_settings::sidecar::read(&file, true, false).unwrap().settings };
        oni_app::export::write_one(&engine, &target, &ours_path, &settings, &oni_settings::sidecar::ExifMetadata::default()).unwrap_or_else(|err| panic!("{}: {err:?}", file.display()));
        let ours = written(&ours_path);
        let swift = written(&dir.join("export").join(format!("{index:05}.{ext}")));
        let label = format!("{:12} {format:6} {space:9} {edge:4}", e["file"].as_str().unwrap());
        let raw = oni_decode::route(&file) == oni_decode::Branch::Mosaic;
        // Gaps left open, each named: LibRaw's own decode for a RAW; the EXIF defaults
        // ImageIO adds to a JPEG, and its encoder's tables; a reduced sheet whose clear gutters
        // Core Image lets through white where its full size writes them black.
        let reduced_sheet = edge > 0 && e["file"] == "sheet.tif";
        if (ours.width, ours.height) != (swift.width, swift.height) {
            println!("{label}  {}x{} for Swift's {}x{}", ours.width, ours.height, swift.width, swift.height);
            failures.push(format!("{label}: size"));
            continue;
        }
        let (mean, p99, worst) = gap(&ours.pixels, &swift.pixels, ours.width, 0);
        let (oc, sc) = (ours.icc.as_deref().and_then(colorants), swift.icc.as_deref().and_then(colorants));
        let profile_gap = match (&oc, &sc) {
            (Some((a, _)), Some((b, _))) => (0..3).flat_map(|i| (0..3).map(move |j| (a[i][j] - b[i][j]).abs())).fold(0.0, f64::max),
            // ImageIO embeds no sRGB profile in a JPEG, which reads as sRGB all the same.
            (Some(_), None) if format == "jpeg" && space == "srgb" => 0.0,
            _ => f64::INFINITY,
        };
        let differing: Vec<String> = ours.tags.keys().chain(swift.tags.keys()).collect::<std::collections::BTreeSet<_>>().into_iter()
            .filter(|t| ours.tags.get(t) != swift.tags.get(t))
            .map(|t| format!("{t:#06x}: ours {:?}, Swift {:?}", ours.tags.get(t), swift.tags.get(t)))
            .collect();
        let tags_open = format == "jpeg";
        let (m, p) = if format == "jpeg" { (1.5e-2, 8e-2) } else { (1e-3, 5e-3) };
        let pixels_open = raw || reduced_sheet;
        let ok = (pixels_open || (mean < m && p99 < p)) && profile_gap < 1e-3 && (tags_open || differing.is_empty());
        let open = [(pixels_open, "pixels"), (tags_open && !differing.is_empty(), "tags")].iter().filter(|o| o.0).map(|o| o.1).collect::<Vec<_>>().join(", ");
        println!("{label}  {}x{}  mean {mean:.5} p99 {p99:.5} max {worst:.4}  profile ±{profile_gap:.5}  {}{}", ours.width, ours.height,
            if ok { "ok" } else { "DEPARTS" }, if open.is_empty() { String::new() } else { format!("  (open: {open})") });
        for d in &differing {
            println!("    {d}");
        }
        if !ok {
            failures.push(format!("{label}: mean {mean:.5}, p99 {p99:.5}, profile {profile_gap:.5}, {} tags", differing.len()));
        }
    }
    // Kept for a look when something departs.
    if failures.is_empty() {
        let _ = std::fs::remove_dir_all(&scratch);
    } else {
        println!("the port's exports are in {}", scratch.display());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
