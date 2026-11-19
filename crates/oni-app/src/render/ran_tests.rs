//! The slot's one render in flight is one the GPU has not run yet: the worker takes no job before.

use super::*;
use std::sync::mpsc;

/// The GPU's time behind each render, which the CPU's side of a gesture render is a hundredth of.
const GPU: Duration = Duration::from_millis(30);

/// Renders on the engine as if its GPU ran each render `GPU` after its CPU side: each render's start
/// and the GPU's end of it are logged; `waits` false keeps today's empty `ran`.
struct Queued {
    engine: Shared,
    waits: bool,
    /// Each render's start and when the GPU ends it.
    log: Arc<Mutex<Vec<(Instant, Instant)>>>,
}

impl Renderer for Queued {
    fn resident(&mut self, negative: &Negative, settings: &PipelineSettings, view: Option<&View>) -> Result<(Source, Option<Placed>), String> {
        let started = Instant::now();
        // The GPU runs in order: a render queued behind the last ends `GPU` after that one does.
        let end = self.log.lock().unwrap().last().map_or(started, |(_, e)| (*e).max(started)) + GPU;
        self.log.lock().unwrap().push((started, end));
        self.engine.resident(negative, settings, view)
    }

    fn ran(&mut self) {
        let end = self.log.lock().unwrap().last().map(|(_, e)| *e);
        if let (true, Some(end)) = (self.waits, end) {
            std::thread::sleep(end.saturating_duration_since(Instant::now()));
        }
    }
}

/// Twenty notches asked 5 ms apart: each render's start and GPU end, and the revisions rendered.
fn burst(waits: bool) -> Option<(Vec<(Instant, Instant)>, Vec<u64>)> {
    let engine = Engine::shared(&Device::open().ok()?);
    let decoded = Decoded { linear: Linear { width: 8, height: 8, pixels: vec![[0.3, 0.2, 0.1, 1.0]; 64] }, full_width: 8.0, undeclared: None, seed: [0, 0] };
    let negative = Arc::new(engine.prepare(decoded).ok()?);
    let log = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    let worker = Worker::new(Box::new(Queued { engine, waits, log: log.clone() }), move |d| {
        if let Delivery::Render(Ok(d)) = d {
            let _ = tx.send(d.revision);
        }
    });
    let job = |revision: u64| {
        let settings = PipelineSettings { capture_stops: revision as f32 * 0.01, ..PipelineSettings::default() };
        let (kind, asked) = (crate::timing::Kind::Notch, Instant::now());
        Job { revision, path: PathBuf::new(), negative: negative.clone(), settings, kind, asked, view: None, reference: None, held: true, counted: false, inputs: [false; 2] }
    };
    // The first render compiles the kernels; the slot is judged on what follows.
    worker.submit(job(0));
    rx.recv_timeout(Duration::from_secs(60)).ok()?;
    std::thread::sleep(GPU * 2);
    log.lock().unwrap().clear();
    for revision in 1..=20 {
        worker.submit(job(revision));
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut revisions = Vec::new();
    while let Ok(r) = rx.recv_timeout(Duration::from_millis(500)) {
        revisions.push(r);
    }
    let log = log.lock().unwrap().clone();
    Some((log, revisions))
}

#[test]
fn the_worker_takes_no_job_before_the_gpu_ran_the_last() {
    let Some((renders, revisions)) = burst(true) else { return eprintln!("no adapter: the slot's wait is not exercised") };
    let overlapped = |r: &[(Instant, Instant)]| r.windows(2).filter(|w| w[1].0 < w[0].1).count();
    assert_eq!(overlapped(&renders), 0, "a render started while the GPU still ran the one before");
    assert_eq!(revisions.last(), Some(&20), "the latest notch is always rendered");
    // Twenty notches over 100 ms, a render's GPU 30 ms: those asked meanwhile collapse to the latest.
    assert!(revisions.len() <= 6, "{revisions:?}: the jobs asked during a render were not collapsed");
    // Twin: today's empty wait takes each notch as its CPU side ends, the GPU's queue growing behind.
    let Some((queued, _)) = burst(false) else { return };
    assert!(overlapped(&queued) >= 5, "{} renders overlapped the one before with no wait", overlapped(&queued));
}
