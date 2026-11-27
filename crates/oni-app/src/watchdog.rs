//! `Diagnostics`' watchdog: a thread of its own asks the interface thread to answer every 0.1 s, a request
//! unanswered past 0.35 s is a block written to the log while it lasts, the footprint beside it, and the console
//! counts them; a block past `LONG_BLOCK` writes where the interface thread waits, read by the system's `sample`.

use std::cell::RefCell;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// `Diagnostics.stallFloor` and how often the interface thread is asked.
pub const STALL_FLOOR: Duration = Duration::from_millis(350);
pub const BEAT: Duration = Duration::from_millis(100);
/// A block this long is no slow frame but a wait that may never end: its stack is worth the log.
pub const LONG_BLOCK: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Store {
    /// When the request still unanswered was asked.
    asked: Option<Instant>,
    /// How long the last answered request waited.
    waited: Duration,
    witness: String,
    reporting: bool,
    sampled: bool,
    stalls: usize,
}

static STORE: Mutex<Store> =
    Mutex::new(Store { asked: None, waited: Duration::ZERO, witness: String::new(), reporting: false, sampled: false, stalls: 0 });

thread_local! {
    /// What the interface shows, read by the thread that answers: the interface thread alone sets it.
    static WITNESS: RefCell<Option<Box<dyn Fn() -> String>>> = const { RefCell::new(None) };
}

/// What the interface thread says it shows as it answers, set on that thread.
pub fn witness(said: impl Fn() -> String + 'static) {
    WITNESS.with(|w| *w.borrow_mut() = Some(Box::new(said)));
}

/// The interface thread answers the request waiting, if any, with what it shows.
pub fn answer(at: Instant, witness: String) {
    if let Ok(mut s) = STORE.lock() {
        if let Some(asked) = s.asked.take() {
            s.waited = at.saturating_duration_since(asked);
        }
        s.witness = witness;
    }
}

/// One turn of the watchdog at `now`: a block begun or ended is written to the log, a new request handed to `ask`
/// when none waits; how long the interface thread has left a request unanswered while blocked.
pub fn check(now: Instant, ask: impl FnOnce()) -> Option<Duration> {
    let (since, seen, began, ended, long, asking) = {
        let mut s = STORE.lock().ok()?;
        let waiting = s.asked.map(|asked| now.saturating_duration_since(asked));
        let blocked = waiting.is_some_and(|w| w > STALL_FLOOR);
        let since = waiting.unwrap_or(s.waited);
        let (began, ended) = (blocked && !s.reporting, !blocked && s.reporting);
        if began {
            s.stalls += 1;
            s.sampled = false;
        }
        let long = blocked && since > LONG_BLOCK && !s.sampled;
        s.sampled |= long;
        s.reporting = blocked;
        let asking = s.asked.is_none();
        if asking {
            s.asked = Some(now);
        }
        (since, s.witness.clone(), began, ended, long, asking)
    };
    if asking {
        ask();
    }
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
    (since > STALL_FLOOR && !ended).then_some(since)
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
            check(Instant::now(), ask_interface);
        }
    });
}

/// The interface thread asked through the main dispatch queue, served whenever the thread is free, the system's modal
/// loops included, and never while code holds it: a timer stops in those loops and sleeps under App Nap.
#[cfg(target_os = "macos")]
fn ask_interface() {
    #[repr(C)]
    struct Queue {
        _opaque: [u8; 0],
    }
    unsafe extern "C" {
        static _dispatch_main_q: Queue;
        fn dispatch_async_f(queue: *const Queue, context: *mut std::ffi::c_void, work: extern "C" fn(*mut std::ffi::c_void));
    }
    extern "C" fn answered(_: *mut std::ffi::c_void) {
        answered_here();
    }
    // SAFETY: the main queue lives as long as the process, and `answered` reads nothing through its context.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, std::ptr::null_mut(), answered) };
}

/// The interface thread asked through Slint's event loop.
#[cfg(not(target_os = "macos"))]
fn ask_interface() {
    let _ = slint::invoke_from_event_loop(answered_here);
}

/// The answer, on the interface thread.
fn answered_here() {
    let witness = WITNESS.with(|w| w.borrow().as_ref().map_or_else(|| "the starter".to_string(), |said| said()));
    answer(Instant::now(), witness);
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

/// What the system charges the process, swap included, as Activity Monitor shows.
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

    /// A request unanswered past the floor is one block written once, its end written once; the twin, a request
    /// answered within the floor, is none, however long no timer ran. One test, as the store is the process's own.
    #[test]
    fn a_request_unanswered_past_the_floor_counts_one_block() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        answer(t0, "the gallery".into());
        let before = blocks();
        // Asked and answered at once, ten seconds apart: a thread at rest answers, it is not blocked.
        for second in 0..10 {
            assert_eq!(check(at(1000 * second), || answer(at(1000 * second + 5), "the gallery".into())), None, "twin: an answer in time");
        }
        assert_eq!(blocks(), before, "an answering thread is no block");
        let asked = at(20_000);
        assert_eq!(check(asked, || {}), None, "asked, not yet late");
        assert!(check(asked + Duration::from_millis(500), || panic!("one request at a time")).is_some());
        assert!(check(asked + Duration::from_millis(900), || {}).is_some(), "still the same block");
        assert_eq!(blocks(), before + 1, "one block, however long it lasts");
        answer(asked + Duration::from_millis(950), "the editor".into());
        assert_eq!(check(asked + Duration::from_millis(1000), || {}), None, "recovered");
        assert!(footprint().is_none_or(|b| b > 1_000_000), "a footprint in bytes");
    }

    /// The main thread's heaviest path, innermost first, its lighter branch and the next thread left out;
    /// the twin, a report holding no main thread, reads none.
    #[test]
    fn a_sample_report_gives_the_main_threads_heaviest_path() {
        let report = concat!(
            "Call graph:\n    2290 Thread_1   DispatchQueue_1: com.apple.main-thread  (serial)\n",
            "    + 2290 start  (in dyld) + 6992  [0x1]\n    +   2290 main  (in oni) + 52  [0x2]\n",
            "    +     2288 mach_msg  (in libsystem_kernel.dylib) + 24  [0x3]\n    +     2 other_branch  (in oni) + 1  [0x4]\n",
            "    2290 Thread_2: log\n    + 2290 thread_start  (in libsystem_pthread.dylib) + 8  [0x5]\n",
        );
        assert_eq!(main_thread_frames(report), ["mach_msg (libsystem_kernel.dylib)", "main (oni)", "start (dyld)"]);
        assert!(main_thread_frames(&report.replace("com.apple.main-thread", "com.apple.other")).is_empty(), "twin: no main thread, no frames");
    }
}
