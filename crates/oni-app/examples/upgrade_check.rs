//! A Swift user's upgrade, headless: every sidecar under `scans` read, then a copy of rolls rendered and
//! written back, and a copy of the support folder loaded. Never point `rolls` or `support` at originals.
//! `cargo run -p oni-app --example upgrade_check -- <scans> <rolls copy> <support copy> <index copy>`

use oni_app::display::{Params, Screen};
use oni_app::{render, thumbnails};
use oni_settings::sidecar::{self, WriteOptions};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("._") {
            continue;
        }
        if p.is_dir() {
            walk(&p, out);
        } else if name.ends_with(".json") {
            out.push(p);
        }
    }
}

fn original_of(sidecar: &Path) -> PathBuf {
    sidecar.with_extension("")
}

fn mosaic(original: &Path) -> bool {
    oni_decode::decodes_via_mosaic(original)
}

/// Every sidecar and roll marker read as the app reads them; nothing is written.
fn read_all(scans: &Path) -> usize {
    let mut files = Vec::new();
    walk(scans, &mut files);
    let (mut sidecars, mut markers, mut failures) = (0, 0, 0);
    for f in &files {
        if f.file_name().is_some_and(|n| n == oni_settings::film::roll_folder::ROLL_MARKER) {
            markers += 1;
            if oni_settings::film::roll_folder::roll(f.parent().unwrap()).is_none() {
                failures += 1;
                println!("FAIL marker {}", f.display());
            }
            continue;
        }
        sidecars += 1;
        let original = original_of(f);
        match sidecar::read(&original, mosaic(&original), true) {
            Ok(l) if l.is_new || l.foreign || !l.writable => {
                failures += 1;
                println!("FAIL {} new {} foreign {} writable {}", f.display(), l.is_new, l.foreign, l.writable);
            }
            Ok(_) => {}
            Err(e) => {
                failures += 1;
                println!("FAIL {}: {e}", f.display());
            }
        }
    }
    println!("read: {sidecars} sidecars, {markers} roll markers, {failures} failures");
    failures
}

fn luminance(rgba: &[u8]) -> f64 {
    let px = rgba.len() / 4;
    let sum: f64 = rgba.chunks_exact(4).map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64).sum();
    sum / px.max(1) as f64 / 255.0
}

fn spread(rgba: &[u8]) -> f64 {
    let m = luminance(rgba);
    let px = (rgba.len() / 4).max(1) as f64;
    let var: f64 = rgba.chunks_exact(4).map(|p| ((0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64) / 255.0 - m).powi(2)).sum();
    (var / px).sqrt()
}

/// `Thumbnails.render` as the gallery's worker runs it, the sRGB tile returned.
fn tile(engine: &mut render::Engine, original: &Path) -> Result<Vec<u8>, String> {
    let settings = sidecar::read(original, mosaic(original), true).map_err(|e| e.to_string())?.settings;
    if !oni_app::scans::is_converted(&settings)
        && let Some(jpeg) = oni_app::scans::scan(original)
    {
        return thumbnails::decoded(&jpeg, Screen::Srgb).map(|t| t.0).ok_or("scan unreadable".into());
    }
    let decoded = render::decode_for(original, render::Reading::of(&settings), render::PREVIEW_SIDE)?;
    let negative = engine.prepare(decoded)?;
    let linear = engine.render(&negative, &settings.per_pixel_only(), Some(thumbnails::SIDE))?;
    let (w, h) = (linear.width, linear.height);
    Ok(render::encoded(&linear, &Params::new(Screen::Srgb, w as u32, h as u32, 1), 1))
}

/// Each copied frame rendered and compared with the Swift thumbnail, then written back with a key
/// this app does not know, read again to the same settings and the key kept.
fn copies(rolls: &Path, support: &Path) -> usize {
    let mut files = Vec::new();
    walk(rolls, &mut files);
    files.retain(|f| !f.file_name().is_some_and(|n| n == oni_settings::film::roll_folder::ROLL_MARKER));
    files.sort();
    let device = render::Device::open().expect("GPU");
    let mut engine = render::Engine::on(&device);
    let (mut failures, mut compared) = (0, 0);
    for f in &files {
        let original = original_of(f);
        let name = f.file_name().unwrap().to_string_lossy();
        let before = std::fs::read(f).unwrap();
        let version = serde_json::from_slice::<Value>(&before).unwrap()["version"].clone();
        match tile(&mut engine, &original) {
            Err(e) => {
                failures += 1;
                println!("FAIL render {name}: {e}");
            }
            Ok(rgba) => {
                let (mean, sd) = (luminance(&rgba), spread(&rgba));
                let swift = oni_library::relink::fingerprint(&original)
                    .and_then(|fp| std::fs::read(thumbnails::file(support, &fp)).ok())
                    .and_then(|data| thumbnails::decoded(&data, Screen::Srgb))
                    .map(|t| luminance(&t.0));
                let off = swift.is_some_and(|s| (s - mean).abs() > 0.15);
                let bad = mean < 0.02 || mean > 0.98 || sd < 0.01 || off;
                compared += swift.is_some() as usize;
                failures += bad as usize;
                println!("{} v{version} {name}: mean {mean:.3} sd {sd:.3} swift {}", if bad { "FAIL" } else { "ok" }, swift.map_or("-".into(), |s| format!("{s:.3}")));
            }
        }
        let loaded = sidecar::read(&original, mosaic(&original), true).unwrap();
        if std::fs::read(f).unwrap() != before {
            failures += 1;
            println!("FAIL {name}: rewritten by a read or a render");
        }
        let mut stored: Value = serde_json::from_slice(&before).unwrap();
        stored["futureTop"] = Value::from("kept");
        stored["settings"]["futureInner"] = Value::from(7);
        std::fs::write(f, serde_json::to_vec_pretty(&stored).unwrap()).unwrap();
        if let Err(e) = sidecar::write(&loaded.settings, &original, WriteOptions::default()) {
            failures += 1;
            println!("FAIL write {name}: {e}");
            continue;
        }
        let again = sidecar::read(&original, mosaic(&original), true).unwrap();
        let written: Value = serde_json::from_slice(&std::fs::read(f).unwrap()).unwrap();
        // An older document's extra keys are the ones its migrations consumed: only the current one carries them.
        let current = version == oni_settings::codec::VERSION;
        let kept = !current || (written["futureTop"] == "kept" && written["settings"]["futureInner"] == 7);
        let same = again.settings == loaded.settings && again.label == loaded.label && again.roll_draft == loaded.roll_draft && again.imported_at == loaded.imported_at;
        let lost: Vec<&String> = stored.as_object().unwrap().keys().filter(|k| k.as_str() != "futureTop" && written.get(k.as_str()).is_none()).collect();
        if !kept || !same || !lost.is_empty() {
            failures += 1;
            println!("FAIL round trip {name}: kept {kept} same {same} lost {lost:?}");
        }
    }
    println!("copies: {} frames, {compared} beside a Swift thumbnail, {failures} failures", files.len());
    failures
}

fn count(path: &Path, key: &str) -> usize {
    std::fs::read(path).ok().and_then(|d| serde_json::from_slice::<Value>(&d).ok()).and_then(|v| v[key].as_array().map(Vec::len)).unwrap_or(0)
}

/// The support folder's files loaded through the app's readers, each against the raw JSON's count.
fn support_folder(support: &Path, index: &Path) -> usize {
    let mut failures = 0;
    let mut check = |what: &str, ok: bool, detail: String| {
        failures += !ok as usize;
        println!("{} {what}: {detail}", if ok { "ok" } else { "FAIL" });
    };
    let presets = oni_core::presets::load_shelf(support);
    let files = std::fs::read_dir(support.join("Presets")).map(|d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "json")).count()).unwrap_or(0);
    let shared = presets.iter().filter(|p| oni_core::presets::share_code(p).ok().and_then(|c| oni_core::presets::from_share_code(&c)).is_some_and(|q| q.same_look(p))).count();
    check("presets", presets.len() == files && shared == files, format!("{} of {files} files, {shared} share codes round trip", presets.len()));
    let masks = oni_app::flat_fields::entries(support);
    let resolved = masks.iter().filter(|m| oni_app::flat_fields::folder(support).join(format!("{}.tiff", m.id)).exists()).count();
    check("flat-field masks", !masks.is_empty() && resolved == masks.len(), format!("{resolved} of {} resolve", masks.len()));
    let projects = oni_library::project::load(support);
    let raw = count(&support.join("projects.json"), "projects");
    check("projects", projects.projects.len() == raw, format!("{} of {raw}", projects.projects.len()));
    let favourites = oni_library::favourites::Favourites::load(support);
    let raw = count(&support.join("favourites.json"), "paths");
    check("favourites", favourites.as_ref().is_some_and(|f| f.paths.len() == raw), format!("{:?} of {raw}", favourites.map(|f| f.paths.len())));
    let roots = oni_library::roots::Roots::load(support);
    let raw = count(&support.join("roots.json"), "paths");
    check("roots", roots.paths.len() == raw, format!("{} of {raw}", roots.paths.len()));
    let library = oni_library::library::load(support);
    let raw = count(&support.join("library.json"), "files");
    check("library.json", library.files.len() == raw, format!("{} of {raw}", library.files.len()));
    let bookmarks = oni_library::bookmarks::Bookmarks::load(support);
    check("bookmarks", true, format!("{} places", bookmarks.places.len()));
    let opened = oni_library::LibraryIndex::open(index);
    check("library index", opened.is_some(), format!("{:?} rows", opened.as_ref().map(|i| i.count())));
    let fresh = support.join("fresh-index.sqlite");
    let rebuilt = oni_library::LibraryIndex::open(&fresh).is_some_and(|i| i.rebuild(&[]).is_ok());
    check("library index rebuilt", rebuilt, String::new());
    let table = oni_transition::table::Table::load(support);
    check("camera transition table", table.is_ok(), format!("{:?} cameras", table.as_ref().map(|t| t.cameras.len()).ok()));
    let (records, names) = oni_library::journal::Journal::new(support.join("Journal")).read();
    check("journal", !records.is_empty(), format!("{} records, {} names", records.len(), names.len()));
    failures
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [scans, rolls, support, index] = args.as_slice() else {
        eprintln!("upgrade_check <scans> <rolls copy> <support copy> <index copy>");
        std::process::exit(2);
    };
    let failures = read_all(scans) + copies(rolls, support) + support_folder(support, index);
    println!("{failures} failures");
    std::process::exit((failures > 0) as i32);
}
