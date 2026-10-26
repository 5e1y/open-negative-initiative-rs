//! `ONI_BENCH_EXPORT=<n>`: n frames of roll #8 exported stage by stage, then as a batch, and the surface's
//! parameter changes timed on them; the knobs (`ONI_BENCH_FORMAT`, `_KEEP`, `_SAME`…) are read below.

use oni_app::export::{self, Target};
use oni_app::render::{Device, Engine};
use oni_export::{Format, Settings};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Roll #8's first `n` frames as hard links beside copies of their sidecars, so nothing is written beside the originals.
fn mirrored(roll: &Path, into: &Path, n: usize) -> Vec<PathBuf> {
    std::fs::create_dir_all(into).unwrap();
    let mut raws: Vec<PathBuf> = std::fs::read_dir(roll).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "NEF")).collect();
    raws.sort();
    raws.truncate(n);
    raws.iter()
        .map(|from| {
            let to = into.join(from.file_name().unwrap());
            std::fs::hard_link(from, &to).unwrap();
            let sidecar = oni_settings::sidecar::url_for(from);
            if sidecar.exists() {
                std::fs::copy(&sidecar, oni_settings::sidecar::url_for(&to)).unwrap();
            }
            to
        })
        .collect()
}

fn targets(paths: &[PathBuf]) -> Vec<Target> {
    paths
        .iter()
        .map(|p| {
            let mosaic = oni_decode::decodes_via_mosaic(p);
            let settings = oni_settings::sidecar::read(p, mosaic, true).map(|l| l.settings).unwrap_or_default();
            Target { path: p.clone(), settings }
        })
        .collect()
}

/// The process's peak resident memory in MB.
fn peak_mb() -> f64 {
    #[cfg(target_os = "macos")]
    {
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
        usage.ru_maxrss as f64 / 1e6
    }
    #[cfg(not(target_os = "macos"))]
    0.0
}

/// The file with each embedded profile's creation date blanked: the profile is made as the file is written.
fn undated(mut file: Vec<u8>) -> Vec<u8> {
    let at: Vec<usize> = file.windows(4).enumerate().filter(|(i, w)| *w == b"acsp" && *i >= 12).map(|(i, _)| i).collect();
    for i in at {
        file[i - 12..i].fill(0);
    }
    file
}

/// FNV-1a over the samples' bits: one ulp anywhere changes it.
fn bits(samples: &[f32]) -> u64 {
    samples.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, v| v.to_bits().to_le_bytes().iter().fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3)))
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// `ONI_BENCH_FORMAT=tiff` writes TIFF 16-bit; `ONI_BENCH_KEEP=<dir>` keeps the files, which a later run's
/// `ONI_BENCH_SAME=<dir>` compares byte for byte; `ONI_BENCH_SEQUENTIAL` and `_UNSTAGED` pick the passes.
pub(crate) fn an_export_is_timed_stage_by_stage() {
    let Some(n) = std::env::var("ONI_BENCH_EXPORT").ok().and_then(|n| n.parse::<usize>().ok()) else {
        return eprintln!("ONI_BENCH_EXPORT is unset: the export bench is not exercised");
    };
    let roll = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    if !roll.is_dir() {
        return eprintln!("roll #8 is absent: the export bench is not exercised");
    }
    let gpu = match Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the export bench is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-export-speed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let paths = mirrored(&roll, &root.join("Roll"), n);
    let all = targets(&paths);
    let mut export = Settings::default();
    if std::env::var("ONI_BENCH_FORMAT").is_ok_and(|f| f == "tiff") {
        export.set_format(Format::Tiff16);
    }
    let engine = Engine::shared(&gpu);
    let metadata: Vec<_> = all.iter().map(|t| export::opening_row(&t.path, None, "", "").metadata).collect();
    println!("{} frames, {:?}, {}", all.len(), export.format(), engine.lock().unwrap().adapter());

    // Each stage on its own, frame by frame, the decodes kept in a folder of their own.
    oni_decode::cache::keep_in(root.join("Staged decodes"), 64 << 30);
    let staging = std::env::var_os("ONI_BENCH_UNSTAGED").is_none();
    let staged = root.join("staged");
    std::fs::create_dir_all(&staged).unwrap();
    let mut sums = [0.0f64; 7];
    let mut hashes = String::new();
    for (t, m) in all.iter().zip(&metadata).filter(|_| staging) {
        let mut at = Instant::now();
        let mut lap = |i: usize, at: &mut Instant| {
            sums[i] += ms(*at);
            *at = Instant::now();
        };
        let decoded = export::full_decode(&t.path, oni_app::render::Reading::of(&t.settings)).unwrap();
        lap(0, &mut at);
        let full = export::render_full(&mut engine.lock().unwrap(), decoded, &t.settings).unwrap();
        lap(1, &mut at);
        hashes.push_str(&format!("{} {:016x}\n", t.path.file_name().unwrap().to_string_lossy(), bits(&full.pixels)));
        at = Instant::now();
        let scale = export.resize.scale(full.width as f64, full.height as f64);
        let mut out = oni_export::resized(full, scale);
        oni_colour::paper::laid(&t.settings.paper_correction, &mut out.pixels, export.space());
        lap(2, &mut at);
        let rendered = oni_export::converted(&out, &export).unwrap();
        drop(out);
        lap(3, &mut at);
        let date = oni_export::capture_date(&t.path);
        lap(4, &mut at);
        let data = oni_export::encoded(&rendered, &export, date.as_deref(), m).unwrap();
        lap(5, &mut at);
        let to = staged.join(oni_export::name(&t.path, "", export.format()));
        oni_export::file::put(&to, &data, &oni_export::file::Ledger::none(), false).unwrap();
        lap(6, &mut at);
    }
    let names = ["decode", "upload, render, readback", "resize and paper", "convert", "capture date", "encode", "write"];
    let count = all.len() as f64;
    for (name, sum) in names.iter().zip(sums) {
        println!("  {name:<26} {:>8.0} ms a frame", sum / count);
    }
    println!("  {:<26} {:>8.0} ms a frame", "sum", sums.iter().sum::<f64>() / count);

    // The batch as the surface writes it, its decodes kept in a fresh folder, so it starts cold too.
    oni_decode::cache::keep_in(root.join("Batch decodes"), 64 << 30);
    let out = std::env::var_os("ONI_BENCH_KEEP").map(PathBuf::from).unwrap_or_else(|| root.join("out"));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let destinations: Vec<PathBuf> = all.iter().map(|t| out.join(oni_export::name(&t.path, "", export.format()))).collect();
    let started = Instant::now();
    let mut ledger = oni_export::file::Ledger::in_support(&root.join("support"));
    let mut held = 1;
    if std::env::var_os("ONI_BENCH_SEQUENTIAL").is_some() {
        for ((t, d), m) in all.iter().zip(&destinations).zip(&metadata) {
            let to = oni_export::Landing { path: d, ledger: &mut ledger, replacing: false };
            export::write_one(&engine, t, to, &export, m).unwrap();
        }
    } else {
        let jobs: Vec<export::batch::Job> = all.iter().zip(&destinations).zip(&metadata).map(|((t, d), m)| export::batch::Job { target: t.clone(), destination: d.clone(), metadata: m.clone() }).collect();
        let stop = std::sync::atomic::AtomicBool::new(false);
        // The engine apart the surface writes on, as the app runs it.
        let apart = engine.apart().unwrap();
        let (end, peak) = export::batch::write_batch(&apart, &jobs, &export, &mut ledger, false, &stop, |_| {});
        assert_eq!(end, export::batch::End::Done);
        held = peak;
    }
    let wall = ms(started);
    println!("batch of {}: {:.0} ms wall, {:.0} ms a frame; {held} frames held at most; peak memory {:.0} MB", all.len(), wall, wall / count, peak_mb());

    if std::env::var_os("ONI_BENCH_KEEP").is_some() {
        std::fs::write(out.join("renders.txt"), &hashes).unwrap();
    }
    if let Some(reference) = std::env::var_os("ONI_BENCH_SAME").map(PathBuf::from) {
        assert_eq!(std::fs::read_to_string(reference.join("renders.txt")).unwrap(), hashes, "the float renders moved");
        for d in &destinations {
            let theirs = undated(std::fs::read(reference.join(d.file_name().unwrap())).expect("the reference file"));
            let ours = undated(std::fs::read(d).unwrap());
            assert!(ours == theirs, "{} differs from its reference", d.display());
            // Twin: one byte of the picture changed is caught.
            let mut moved = ours.clone();
            let last = moved.len() - 3;
            moved[last] ^= 1;
            assert!(moved != theirs);
        }
        println!("every file is byte for byte its reference, the profile's creation date aside");
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// The interface thread's time in each pump while `done` is not yet true, and the wall time until it is.
fn busy_until(rig: &crate::common::Rig, done: impl Fn() -> bool) -> (f64, f64) {
    let (started, mut busy) = (Instant::now(), 0.0);
    while !done() {
        assert!(started.elapsed().as_secs() < 600, "the surface never caught up");
        let at = Instant::now();
        rig.pump();
        busy += ms(at);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    (busy, ms(started))
}

/// `ONI_BENCH_EXPORT=<n>`: the surface entered on n frames, then each kind of parameter change timed on
/// the interface thread and until every line and the estimate are up to date again.
pub(crate) fn a_parameter_change_on_a_batch_is_timed() {
    use oni_app::Exporting;
    use slint::ComponentHandle;
    let Some(n) = std::env::var("ONI_BENCH_EXPORT").ok().and_then(|n| n.parse::<usize>().ok()) else {
        return eprintln!("ONI_BENCH_EXPORT is unset: the parameter bench is not exercised");
    };
    let roll = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    if !roll.is_dir() {
        return eprintln!("roll #8 is absent: the parameter bench is not exercised");
    }
    let gpu = match Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the parameter bench is not exercised"),
    };
    let rig = crate::common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let root = std::env::temp_dir().join(format!("oni-export-params-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let all = targets(&mirrored(&roll, &root.join("Roll"), n));
    oni_decode::cache::keep_in(root.join("Decodes"), 64 << 30);
    let store = oni_core::EditorStore::new(oni_core::Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = oni_app::app::Options { canvas: false, support: root.join("support"), caches: root.join("caches"), picker: Box::new(|| None), ..Default::default() };
    let shell = oni_app::app::Shell::new(Box::new(store), Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let e = shell.window.global::<Exporting>();
    let lines = || oni_app::app::export_card::lines(&shell.window);
    let current = || {
        let l = lines();
        l.len() == all.len() && l.iter().all(|l| l.ready && l.state == oni_app::PreviewState::Shown) && oni_app::app::export_card::estimate(&ctl).0.is_some()
    };

    let at = Instant::now();
    ctl.enter_export(all.clone());
    e.invoke_preview_room(1400.0, 900.0, 320.0);
    let entered = ms(at);
    let (busy, wall) = busy_until(&rig, current);
    println!("enter on {}: {entered:.1} ms on the interface thread, then {busy:.0} ms in its pumps; up to date after {wall:.0} ms", all.len());

    let weighings = || oni_app::app::export_card::estimate(&ctl).1;
    let change = |what: &str, act: &dyn Fn()| {
        let before = weighings();
        let at = Instant::now();
        act();
        let call = ms(at);
        let (busy, wall) = busy_until(&rig, current);
        println!("{what:<28} {call:>7.1} ms in the call, {busy:>6.0} ms in pumps, up to date after {wall:>6.0} ms, {} weighings", weighings() - before);
    };
    change("ten quality steps", &|| (0..10).for_each(|_| e.invoke_quality_step(-1)));
    change("format TIFF", &|| e.invoke_format_picked(0));
    change("format JPEG", &|| e.invoke_format_picked(1));
    change("long edge 2000", &|| e.invoke_size_typed("2000".into(), e.get_height_text()));
    change("full size", &|| e.invoke_full_size());
    change("space sRGB", &|| e.invoke_space_picked(0));
    change("space Display P3", &|| e.invoke_space_picked(1));
    println!("peak memory {:.0} MB", peak_mb());
    ctl.leave_export();
    rig.settle();
    let _ = std::fs::remove_dir_all(&root);
}

/// A burst of ten quality steps on three frames weighs the estimate twice at most, on the last
/// quality, and keeps that quality in the preferences once.
pub(crate) fn a_burst_of_changes_weighs_the_estimate_twice_at_most() {
    use oni_app::Exporting;
    use slint::ComponentHandle;
    let gpu = match Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the estimate's coalescing is not exercised"),
    };
    let rig = crate::common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let root = std::env::temp_dir().join(format!("oni-export-burst-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let all: Vec<Target> = (0..3)
        .map(|k| {
            let path = root.join(format!("Scan {k}.tif"));
            crate::common::plain_tiff(&path, 600, 400, 0.6 + 0.1 * k as f32);
            Target { path, settings: Default::default() }
        })
        .collect();
    let support = root.join("support");
    let store = oni_core::EditorStore::new(oni_core::Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = oni_app::app::Options { canvas: false, support: support.clone(), caches: root.join("caches"), picker: Box::new(|| None), ..Default::default() };
    let shell = oni_app::app::Shell::new(Box::new(store), Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let e = shell.window.global::<Exporting>();
    let estimate = || oni_app::app::export_card::estimate(&ctl);
    ctl.enter_export(all);
    e.invoke_preview_room(480.0, 320.0, 160.0);
    rig.until("the first estimate", std::time::Duration::from_secs(60), || estimate().0.is_some());

    let before = estimate().1;
    (0..10).for_each(|_| e.invoke_quality_step(-1));
    rig.until("the burst's estimate", std::time::Duration::from_secs(60), || estimate().0.is_some());
    let weighed = estimate().1 - before;
    assert!((1..=2).contains(&weighed), "ten steps weighed {weighed} times");
    let quality = oni_app::app::export_card::card(&ctl).settings.jpeg_quality;
    let kept = || {
        let prefs: serde_json::Value = serde_json::from_slice(&std::fs::read(support.join("preferences.json")).ok()?).ok()?;
        oni_export::Settings::from_raw_value(prefs[oni_export::settings::PREFERENCE_KEY].as_str()?).map(|s| s.jpeg_quality)
    };
    rig.until("the quality kept", std::time::Duration::from_secs(10), || kept() == Some(quality));
    // Twin: four steps each waited for are weighed four times.
    let before = estimate().1;
    for _ in 0..4 {
        e.invoke_quality_step(1);
        rig.until("a step's estimate", std::time::Duration::from_secs(60), || estimate().0.is_some());
    }
    assert_eq!(estimate().1 - before, 4);
    ctl.leave_export();
    rig.settle();
    let _ = std::fs::remove_dir_all(&root);
}
