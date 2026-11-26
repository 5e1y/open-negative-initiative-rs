//! `Diagnostics`' watchdog: the interface thread beats every 0.1 s, a thread of its own writes a
//! block past 0.35 s to the log while it lasts, the footprint beside it, and the console counts them;
//! a block past `LONG_BLOCK` writes where the interface thread waits, read by the system's `sample`.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// `Diagnostics.stallFloor` and `beat`.
pub const STALL_FLOOR: Duration = Duration::from_millis(350);
pub const BEAT: Duration = Duration::from_millis(100);
/// A block this long is no slow frame but a wait that may never end: its stack is worth the log.
pub const LONG_BLOCK: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Store {
    last_seen: Option<Instant>,
    witness: String,
    reporting: bool,
    sampled: bool,
    stalls: usize,
}

static STORE: Mutex<Store> = Mutex::new(Store { last_seen: None, witness: String::new(), reporting: false, sampled: false, stalls: 0 });

/// `mainThreadBeat`: the interface thread says it answers, and what it was showing.
pub fn beat(at: Instant, witness: String) {
    if let Ok(mut s) = STORE.lock() {
        s.last_seen = Some(at);
        s.witness = witness;
    }
}

/// One beat of the watchdog: a block begun or ended is written to the log; how long the interface
/// thread has been silent while blocked.
pub fn check(now: Instant) -> Option<Duration> {
    let (since, seen, began, ended, long) = {
        let mut s = STORE.lock().ok()?;
        let since = now.saturating_duration_since(s.last_seen?);
        let blocked = since > STALL_FLOOR;
        let (began, ended) = (blocked && !s.reporting, !blocked && s.reporting);
        if began {
            s.stalls += 1;
            s.sampled = false;
        }
        let long = blocked && since > LONG_BLOCK && !s.sampled;
        s.sampled |= long;
        s.reporting = blocked;
        (since, s.witness.clone(), began, ended, long)
    };
    if long {
        let _ = std::thread::Builder::new().name("watchdog sample".into()).spawn(|| {
            let frames = waiting_frames();
            let said = if frames.is_empty() { "unread".to_string() } else { frames.join(" ← ") };
            crate::timing::line(&format!("BLOCKED · where the interface thread waits, innermost first: {said}"));
        });
    }
    let mb = footprint().map_or_else(|| "unknown".to_string(), |b| format!("{:.0} MB", b as f64 / 1_048_576.0));
    if began {
        crate::timing::line(&format!("BLOCKED · the interface thread has not answered for {:.2} s · footprint {mb} · was: {seen}", since.as_secs_f64()));
    } else if ended {
        crate::timing::line(&format!("recovered · the interface thread answers again after {:.2} s · footprint {mb}", since.as_secs_f64()));
    }
    (since > STALL_FLOOR).then_some(since)
}

/// The blocks counted this session.
pub fn blocks() -> usize {
    STORE.lock().map_or(0, |s| s.stalls)
}

/// Watches from a thread of its own: a detector on the blocked thread would write nothing.
pub fn start() {
    let _ = std::thread::Builder::new().name("watchdog".into()).spawn(|| {
        loop {
            std::thread::sleep(BEAT);
            check(Instant::now());
        }
    });
}

/// The interface thread's frames as the system's `sample` reads them over a second, innermost first: each
/// function and the module it lies in, never a path or an address; none where the tool is missing.
fn waiting_frames() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        let Ok(out) = std::process::Command::new("/usr/bin/sample").args([&std::process::id().to_string(), "1"]).output() else { return Vec::new() };
        main_thread_frames(&String::from_utf8_lossy(&out.stdout))
    }
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}

/// The heaviest path of the main thread's call tree in a `sample` report, innermost first, each frame as
/// `function (module)`: the tree's first line at each depth is its heaviest.
pub fn main_thread_frames(report: &str) -> Vec<String> {
    let mut frames = Vec::new();
    let mut lines = report.lines().skip_while(|l| !l.contains("com.apple.main-thread"));
    lines.next();
    let mut depth = 0;
    for l in lines {
        let Some(at) = l.find(|c: char| c.is_ascii_digit()) else { break };
        if at <= depth && !frames.is_empty() || !l.trim_start().starts_with(['+', '!', ':', '|', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
            break;
        }
        depth = at;
        let body = l[at..].split_once(' ').map_or("", |(_, rest)| rest).trim();
        let function = body.split("  (in ").next().unwrap_or(body).trim();
        let module = body.split("  (in ").nth(1).and_then(|m| m.split(')').next()).unwrap_or("?");
        frames.push(format!("{function} ({module})"));
    }
    frames.reverse();
    frames.truncate(24);
    frames
}

/// `Footprint.bytes`: what the system charges the process, swap included, as Activity Monitor shows.
fn footprint() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let mut info: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
        let read = unsafe { libc::proc_pid_rusage(libc::getpid(), libc::RUSAGE_INFO_V2, (&mut info as *mut libc::rusage_info_v2).cast()) };
        return (read == 0).then_some(info.ri_phys_footprint);
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let read = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, size) };
        return (read != 0).then_some(c.PagefileUsage as u64);
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A silence past the floor is one block written once, its end written once; the twin, a
    /// silence under the floor, is none. One test, as the store is the process's own.
    #[test]
    fn a_silence_past_the_floor_counts_one_block() {
        let t0 = Instant::now();
        beat(t0, "the gallery".into());
        let before = blocks();
        assert_eq!(check(t0 + Duration::from_millis(200)), None, "twin: 0.2 s is not a block");
        assert!(check(t0 + Duration::from_millis(500)).is_some());
        assert!(check(t0 + Duration::from_millis(900)).is_some(), "still the same block");
        assert_eq!(blocks(), before + 1, "one block, however long it lasts");
        beat(t0 + Duration::from_millis(950), "the editor".into());
        assert_eq!(check(t0 + Duration::from_millis(1000)), None, "recovered");
        assert!(footprint().is_none_or(|b| b > 1_000_000), "a footprint in bytes");
    }

    /// The main thread's heaviest path, innermost first, its lighter branch and the next thread left out;
    /// the twin, a report holding no main thread, reads none.
    #[test]
    fn a_sample_report_gives_the_main_threads_heaviest_path() {
        let report = "Call graph:\n    2290 Thread_1   DispatchQueue_1: com.apple.main-thread  (serial)\n    + 2290 start  (in dyld) + 6992  [0x1]\n    +   2290 main  (in oni) + 52  [0x2]\n    +     2288 mach_msg  (in libsystem_kernel.dylib) + 24  [0x3]\n    +     2 other_branch  (in oni) + 1  [0x4]\n    2290 Thread_2: log\n    + 2290 thread_start  (in libsystem_pthread.dylib) + 8  [0x5]\n";
        assert_eq!(main_thread_frames(report), ["mach_msg (libsystem_kernel.dylib)", "main (oni)", "start (dyld)"]);
        assert!(main_thread_frames(&report.replace("com.apple.main-thread", "com.apple.other")).is_empty(), "twin: no main thread, no frames");
    }
}
