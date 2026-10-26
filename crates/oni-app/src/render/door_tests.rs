//! The surface's gate: a reconfigure waits one submission or upload band, never a whole render, and
//! the thread holding it passes its own door.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

/// A 12 MP frame of a ramp, the size an export renders whole.
fn decoded() -> Decoded {
    let (w, h) = (4000usize, 3000usize);
    let pixels = (0..w * h).map(|i| [0.1 + (i % w) as f32 / w as f32 * 0.8, 0.4, 0.6, 1.0]).collect();
    Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None }
}

/// Each hold the interface thread takes at a resize's pace while `busy` runs on another thread: the
/// time it waited, and how long `busy` ran.
fn holds_during(shared: &Shared, busy: impl FnOnce() + Send + 'static) -> (Vec<Duration>, Duration) {
    let done = Arc::new(AtomicBool::new(false));
    let (flag, started) = (done.clone(), Instant::now());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(());
        busy();
        flag.store(true, Ordering::Release);
    });
    rx.recv().unwrap();
    let mut holds = Vec::new();
    while !done.load(Ordering::Acquire) {
        let asked = Instant::now();
        shared.hold();
        holds.push(asked.elapsed());
        shared.release();
        std::thread::sleep(Duration::from_millis(8));
    }
    (holds, started.elapsed())
}

fn spread(holds: &[Duration]) -> String {
    let mut ms: Vec<f64> = holds.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    ms.sort_by(f64::total_cmp);
    let at = |q: f64| ms[((ms.len() - 1) as f64 * q).round() as usize];
    format!("{} holds, median {:.1} ms, p99 {:.1} ms, longest {:.1} ms", ms.len(), at(0.5), at(0.99), at(1.0))
}

/// A resize during an export's full-resolution render: the interface waits the submission in flight,
/// the spikes judged by the longest wait, not the mean.
#[test]
fn a_resize_during_an_export_waits_one_submission_not_the_render() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the gate's latency is not exercised");
        return;
    };
    let shared = Engine::shared(&device);
    let settings = engine::prewarm::busy();
    let exporting = shared.clone();
    let rendering = settings.clone();
    let (holds, took) = holds_during(&shared, move || {
        let mut e = exporting.lock().unwrap();
        crate::export::render_full(&mut e, decoded(), &rendering).unwrap();
    });
    let longest = holds.iter().max().copied().unwrap_or_default();
    println!("export render {:.0} ms; {}", took.as_secs_f64() * 1000.0, spread(&holds));
    assert!(holds.len() >= 5, "the render was too short to judge: {took:?}");
    assert!(longest < Duration::from_millis(100), "a resize waited {longest:?} during an export");

    // Twin: the gate passed once for the whole render, as a guard held through it, makes the resize wait it.
    let (mut alone, door) = (Engine::on(&device), shared.door());
    let (holds, took) = holds_during(&shared, move || {
        door.enter().unwrap();
        crate::export::render_full(&mut alone, decoded(), &settings).unwrap();
        door.leave();
    });
    let longest = holds.iter().max().copied().unwrap_or_default();
    println!("twin: export render {:.0} ms; {}", took.as_secs_f64() * 1000.0, spread(&holds));
    assert!(longest > took / 2, "the twin's whole-render gate went unseen: {longest:?} of {took:?}");
}

/// Each lock of the editor's engine while `busy` runs on another thread: how long each waited.
fn locks_during(shared: &Shared, busy: impl FnOnce() + Send + 'static) -> (Vec<Duration>, Duration) {
    let done = Arc::new(AtomicBool::new(false));
    let (flag, started) = (done.clone(), Instant::now());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(());
        busy();
        flag.store(true, Ordering::Release);
    });
    rx.recv().unwrap();
    let mut waits = Vec::new();
    while !done.load(Ordering::Acquire) {
        let asked = Instant::now();
        drop(shared.lock().unwrap());
        waits.push(asked.elapsed());
        std::thread::sleep(Duration::from_millis(8));
    }
    (waits, started.elapsed())
}

/// An export on an engine apart leaves the editor's engine free: its renders wait a submission, never
/// the export's whole frame.
#[test]
fn an_export_on_an_engine_apart_leaves_the_editors_engine_free() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the engine apart is not exercised");
        return;
    };
    let shared = Engine::shared(&device);
    let settings = engine::prewarm::busy();
    let apart = shared.apart().unwrap();
    let rendering = settings.clone();
    let (waits, took) = locks_during(&shared, move || {
        let mut e = apart.lock().unwrap();
        crate::export::render_full(&mut e, decoded(), &rendering).unwrap();
    });
    let longest = waits.iter().max().copied().unwrap_or_default();
    println!("export apart {:.0} ms; {}", took.as_secs_f64() * 1000.0, spread(&waits));
    assert!(waits.len() >= 5, "the render was too short to judge: {took:?}");
    assert!(longest < Duration::from_millis(50), "the editor's engine waited {longest:?} during an export apart");

    // Twin: the export on the editor's own engine makes its lock wait the whole render.
    let exporting = shared.clone();
    let (waits, took) = locks_during(&shared, move || {
        let mut e = exporting.lock().unwrap();
        crate::export::render_full(&mut e, decoded(), &settings).unwrap();
    });
    let longest = waits.iter().max().copied().unwrap_or_default();
    println!("twin: export shared {:.0} ms; {}", took.as_secs_f64() * 1000.0, spread(&waits));
    assert!(longest > took / 2, "the twin's shared lock went unseen: {longest:?} of {took:?}");
}

/// The interface thread holding the door renders through it: a lock taken before the queued release
/// runs would otherwise wait for the only thread that can release it.
#[test]
fn the_holding_thread_passes_its_own_door_and_no_other() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the gate's holder is not exercised");
        return;
    };
    let shared = Engine::shared(&device);
    let small = || Decoded { linear: Linear { width: 64, height: 48, pixels: vec![[0.4; 4]; 64 * 48] }, full_width: 64.0, undeclared: None };
    shared.hold();
    let negative = shared.lock().unwrap().prepare(small()).unwrap();
    let rest = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    assert!(shared.lock().unwrap().resident(&negative, &rest, None).is_ok(), "the holder renders while it holds");
    // Twin: another thread's render waits for the release.
    let (other, (tx, rx)) = (shared.clone(), mpsc::channel());
    std::thread::spawn(move || {
        let made = other.lock().unwrap().prepare(small()).is_ok();
        let _ = tx.send(made);
    });
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err(), "another thread submits while the door is held");
    shared.release();
    assert_eq!(rx.recv_timeout(Duration::from_secs(10)), Ok(true), "it passes once released");
}

/// The window's first locks during the launch prewarm wait one variant, not the whole compilation:
/// the prewarm still runs once the lock is granted.
#[test]
fn a_lock_during_the_prewarm_waits_one_variant_not_the_whole_prewarm() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the prewarm's locks are not exercised");
        return;
    };
    // How long the prewarm ran after a lock asked 20 ms in was granted.
    let left = |whole: bool| {
        let shared = Engine::shared(&device);
        let warming = shared.clone();
        let (tx, rx) = mpsc::channel();
        let warm = std::thread::spawn(move || {
            if whole {
                let mut e = warming.lock().unwrap();
                let _ = tx.send(());
                e.prewarm();
            } else {
                let _ = tx.send(());
                warming.prewarm();
            }
            Instant::now()
        });
        rx.recv().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        drop(shared.lock().unwrap());
        let granted = Instant::now();
        warm.join().unwrap().saturating_duration_since(granted)
    };
    let after = left(false);
    println!("the prewarm ran {:.0} ms past the lock", after.as_secs_f64() * 1000.0);
    assert!(after > Duration::from_millis(10), "the lock waited for the whole prewarm");
    // Twin: the engine held for the whole prewarm grants the lock only once it ends.
    let after = left(true);
    println!("twin: the prewarm ran {:.0} ms past the lock", after.as_secs_f64() * 1000.0);
    assert!(after < Duration::from_millis(10), "the twin's whole-prewarm lock went unseen: {after:?}");
}
