//! Runs each check in a child process of this binary, several at once, and reports as libtest does.

use std::io::Write;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Half the cores by default: the checks share one GPU, and some time what they draw.
fn jobs() -> usize {
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::env::var("ONI_JOBS").ok().and_then(|j| j.parse().ok()).unwrap_or((cores / 2).max(1))
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
    let chosen: Vec<&str> = checks.iter().map(|(n, _)| *n).filter(|n| filters.is_empty() || filters.iter().any(|f| if exact { *n == f.as_str() } else { n.contains(f.as_str()) })).collect();
    println!("\nrunning {} tests", chosen.len());
    let me = std::env::current_exe().expect("this binary");
    let next = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    let started = std::time::Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..jobs().min(chosen.len().max(1)) {
            scope.spawn(|| {
                while let Some(&name) = chosen.get(next.fetch_add(1, Ordering::SeqCst)) {
                    let out = Command::new(&me).env("ONI_ONE", name).output().expect("a child check");
                    let mut stdout = std::io::stdout().lock();
                    let verdict = if out.status.success() { "ok" } else { "FAILED" };
                    let _ = writeln!(stdout, "test {name} ... {verdict}");
                    if shown || !out.status.success() {
                        let _ = stdout.write_all(&out.stdout);
                        let _ = stdout.write_all(&out.stderr);
                    }
                    if !out.status.success() {
                        failed.lock().unwrap().push(name);
                    }
                }
            });
        }
    });
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
