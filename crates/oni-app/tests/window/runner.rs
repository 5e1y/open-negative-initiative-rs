//! Runs each check in a child process of this binary, several at once, the longest last time first, and reports as
//! libtest does.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// One check a core by default: a check mostly waits on its springs, its renders and the interface's compile, so twelve
/// at once took 540 s where eight took 680 and six 770 (M2 Max, 303 checks).
fn jobs() -> usize {
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::env::var("ONI_JOBS").ok().and_then(|j| j.parse().ok()).unwrap_or(cores)
}

pub fn run(checks: &[(&str, fn())]) {
    if let Ok(name) = std::env::var("ONI_ONE") {
        let (_, check) = checks.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no check named {name}"));
        return check();
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    // None is ignored, so a listing of the ignored ones is empty.
    if args.iter().any(|a| a == "--list") {
        if !args.iter().any(|a| a == "--ignored") {
            checks.iter().for_each(|(n, _)| println!("{n}: test"));
        }
        return;
    }
    let exact = args.iter().any(|a| a == "--exact");
    let shown = args.iter().any(|a| a == "--nocapture");
    let filters: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    let mut chosen: Vec<&str> = checks.iter().map(|(n, _)| *n).filter(|n| filters.is_empty() || filters.iter().any(|f| if exact { *n == f.as_str() } else { n.contains(f.as_str()) })).collect();
    // A long check started last holds the whole run: each starts by its last time, one never timed first.
    let mut timings = timings();
    chosen.sort_by(|a, b| timings.get(*b).unwrap_or(&f64::INFINITY).total_cmp(timings.get(*a).unwrap_or(&f64::INFINITY)));
    println!("\nrunning {} tests", chosen.len());
    let me = std::env::current_exe().expect("this binary");
    let next = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    let taken = Mutex::new(Vec::new());
    let started = std::time::Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..jobs().min(chosen.len().max(1)) {
            scope.spawn(|| {
                while let Some(&name) = chosen.get(next.fetch_add(1, Ordering::SeqCst)) {
                    let began = std::time::Instant::now();
                    let out = Command::new(&me).env("ONI_ONE", name).output().expect("a child check");
                    taken.lock().unwrap().push((name, began.elapsed().as_secs_f64()));
                    let mut stdout = std::io::stdout().lock();
                    let verdict = if out.status.success() { "ok" } else { "FAILED" };
                    let _ = writeln!(stdout, "test {name} ... {verdict}");
                    if shown || !out.status.success() {
                        let _ = stdout.write_all(&out.stdout);
                        let _ = stdout.write_all(&out.stderr);
                    } else {
                        let _ = stdout.write_all(&skips(name, &out.stdout, &out.stderr));
                    }
                    if !out.status.success() {
                        failed.lock().unwrap().push(name);
                    }
                }
            });
        }
    });
    let mut taken = taken.into_inner().unwrap();
    timings.extend(taken.iter().map(|(n, t)| (n.to_string(), *t)));
    let lines: String = timings.iter().map(|(n, t)| format!("{t:.1}\t{n}\n")).collect();
    let _ = std::fs::write(timings_file(), lines);
    taken.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("\nslowest checks:");
    taken.iter().take(10).for_each(|(n, t)| println!("    {t:6.1} s  {n}"));
    let failed = failed.into_inner().unwrap();
    let verdict = if failed.is_empty() { "ok" } else { "FAILED" };
    println!(
        "\ntest result: {verdict}. {} passed; {} failed; 0 ignored; 0 measured; {} filtered out; finished in {:.2}s\n",
        chosen.len() - failed.len(),
        failed.len(),
        checks.len() - chosen.len(),
        started.elapsed().as_secs_f64()
    );
    if !failed.is_empty() {
        failed.iter().for_each(|n| println!("    {n}"));
        std::process::exit(101);
    }
}

/// A passing check's skip lines under libtest's header for its output, so `scripts/check` reads which
/// checks returned early and why, as it does for every other binary.
fn skips(name: &str, stdout: &[u8], stderr: &[u8]) -> Vec<u8> {
    let text = [String::from_utf8_lossy(stdout), String::from_utf8_lossy(stderr)].concat();
    let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("SKIPPED") || l.contains("not exercised")).collect();
    if lines.is_empty() {
        return Vec::new();
    }
    format!("---- {name} stdout ----\n{}\n", lines.join("\n")).into_bytes()
}

/// Each check's seconds on its last run, kept beside the build.
fn timings_file() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("window-timings.tsv")
}

fn timings() -> BTreeMap<String, f64> {
    let text = std::fs::read_to_string(timings_file()).unwrap_or_default();
    text.lines().filter_map(|l| l.split_once('\t')).filter_map(|(t, n)| Some((n.to_string(), t.parse().ok()?))).collect()
}
