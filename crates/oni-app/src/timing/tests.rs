//! The timing lines and the log: centiles, cadence, bounds, and a panic or a failed start written.

use super::*;

fn notches(latencies_ms: &[u64], gap_ms: u64) -> Notches {
    let mut n = Notches::default();
    let start = Instant::now();
    for (i, l) in latencies_ms.iter().enumerate() {
        n.record(Duration::from_millis(*l), start + Duration::from_millis(gap_ms * i as u64));
    }
    n
}

#[test]
fn the_cadence_counts_the_beats_and_the_stalls() {
    let steady = notches(&[3; 31], 33);
    let line = steady.cadence(0);
    assert!(line.contains("30 gaps") && line.contains("30 within 5 ms of 33.3 ms, 0 past 50 ms"), "{line}");
    // Twin: one render held back 60 ms is a stall and leaves the beat.
    let mut stalled = notches(&[3; 10], 33);
    stalled.record(Duration::from_millis(3), stalled.last_shown.unwrap() + Duration::from_millis(60));
    let line = stalled.cadence(0);
    assert!(line.contains("9 within 5 ms of 33.3 ms, 1 past 50 ms"), "{line}");
}

#[test]
fn the_centiles_read_the_slow_tail_and_the_gaps_between_frames() {
    // A hundred notches at 5 ms and two at 40 ms: the median stays, the 99th centile sees them.
    let mut latencies = vec![5u64; 100];
    latencies.extend([40, 40]);
    let n = notches(&latencies, 10);
    let text = n.summary(0, "drag");
    assert!(text.starts_with("drag: 102 notch renders"), "{text}");
    assert!(text.contains("p50 5.0 ms, p99 40.0 ms, worst 40.0 ms"), "{text}");
    assert!(text.contains("worst gap 10.0 ms, 101 of 101 past 8.3 ms"), "{text}");
    // Twin: at 8 ms apart no frame is past the budget.
    let quick = notches(&latencies, 8);
    assert!(quick.summary(0, "drag").contains("0 of 101 past 8.3 ms"));
}

#[test]
fn a_rest_between_two_drags_is_no_gap_and_a_report_covers_its_own_window() {
    let mut n = Notches::default();
    let start = Instant::now();
    n.record(Duration::from_millis(3), start);
    n.record(Duration::from_millis(3), start + Duration::from_secs(5));
    assert!(n.summary(0, "x").contains("no gap"), "a five-second rest is not a frame");
    // Twin: the same two renders 20 ms apart are a gap.
    let mut m = Notches::default();
    m.record(Duration::from_millis(3), start);
    m.record(Duration::from_millis(3), start + Duration::from_millis(20));
    assert!(m.summary(0, "x").contains("worst gap 20.0 ms"));

    let mut r = notches(&vec![2; REPORT_EVERY - 1], 5);
    assert!(r.due().is_none());
    r.record(Duration::from_millis(9), Instant::now() + Duration::from_secs(9));
    let report = r.due().expect("a report at the hundred-and-twentieth notch");
    assert!(report.contains(&format!("{REPORT_EVERY} notch renders")), "{report}");
    assert!(r.due().is_none(), "the next one waits for another window");
    let since = r.count();
    r.record(Duration::from_millis(7), Instant::now() + Duration::from_secs(20));
    assert!(r.summary(since, "bench").contains("1 notch renders; end to end p50 7.0 ms"));
}

#[test]
fn a_long_session_keeps_a_bounded_tail_and_its_whole_count() {
    let mut n = notches(&vec![2; 3 * KEPT], 5);
    for _ in 0..3 * KEPT {
        n.record_histogram(Duration::from_millis(1));
    }
    assert!(n.latencies.len() <= KEPT && n.gaps.len() <= KEPT && n.histograms.kept.len() <= KEPT);
    assert_eq!((n.count(), n.histogram_count()), (3 * KEPT, 3 * KEPT));
    // The last hundred and twenty renders slow: a report from their session index sees them alone.
    let since = n.count();
    for i in 0..REPORT_EVERY {
        n.record(Duration::from_millis(30), Instant::now() + Duration::from_millis(5 * i as u64));
    }
    let text = n.summary(since, "tail");
    assert!(text.starts_with(&format!("tail: {REPORT_EVERY} notch renders; end to end p50 30.0 ms")), "{text}");
    // Twin: a summary from a dropped index counts only what is kept, never past the slice.
    let whole = n.summary(0, "session");
    assert!(!whole.contains(&format!("{} notch renders", n.count())), "{whole}");
}

const CHILD: &str = "ONI_TIMING_CHILD";

/// This binary run again on one check, the log at `log`, `how` telling the child what to do.
fn child(how: &str, log: &Path) -> (std::process::Output, String) {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "timing::tests::the_child", "--nocapture", "--test-threads=1"])
        .env(CHILD, how)
        .env("ONI_TIMING_LOG", log)
        .output()
        .unwrap();
    (out, std::fs::read_to_string(log).unwrap_or_default())
}

/// The other half of the checks below; it does nothing in a run of its own.
#[test]
fn the_child() {
    let (Some(how), Some(log)) = (std::env::var_os(CHILD), std::env::var_os("ONI_TIMING_LOG")) else { return };
    open_file(Path::new(&log)).unwrap();
    match how.to_str() {
        Some("hooked") => keep_panics(),
        Some("fatal") => {
            fatal("no adapter answered");
            std::process::exit(1);
        }
        _ => {}
    }
    panic!("the child's own panic");
}

#[test]
fn a_panic_is_written_to_the_log_and_still_said_on_stderr() {
    let dir = std::env::temp_dir().join(format!("oni-timing-panic-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (out, log) = child("hooked", &dir.join("hooked.log"));
    assert!(!out.status.success());
    assert!(log.contains("panic on thread timing::tests::the_child") && log.contains("the child's own panic"), "{log}");
    assert!(log.contains("tests.rs:"), "the place is written: {log}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("the child's own panic") || String::from_utf8_lossy(&out.stderr).contains("the child's own panic"));
    // Twin: the same panic with the default hook alone leaves the log without it.
    let (out, log) = child("bare", &dir.join("bare.log"));
    assert!(!out.status.success());
    assert!(!log.contains("the child's own panic"), "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_start_writes_its_cause_before_the_process_ends() {
    let dir = std::env::temp_dir().join(format!("oni-timing-fatal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (out, log) = child("fatal", &dir.join("fatal.log"));
    assert_eq!(out.status.code(), Some(1));
    assert!(log.contains("fatal: no adapter answered"), "{log}");
    // Twin: a child ending on a panic without the hook has no such line.
    let (_, log) = child("bare", &dir.join("bare.log"));
    assert!(!log.contains("fatal:"), "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}
