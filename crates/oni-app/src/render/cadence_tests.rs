//! The readings' cadence through the worker: under the hand, at rest, and for the plots on screen.

use super::*;

/// Renders on the engine and logs each reading asked: held or not, and the plots it is asked for.
struct Logged(Shared, Arc<Mutex<Vec<(bool, [bool; 2])>>>);

impl Renderer for Logged {
    fn resident(&mut self, negative: &Negative, settings: &PipelineSettings, view: Option<&View>) -> Result<(Source, Option<Placed>), String> {
        self.0.resident(negative, settings, view)
    }

    fn inputs(&mut self, _: &Negative, _: &PipelineSettings, held: bool, wanted: [bool; 2]) -> Option<InputHistograms> {
        self.1.lock().unwrap().push((held, wanted));
        Some(InputHistograms::default())
    }

    fn ran(&mut self) {
        self.0.ran();
    }
}

/// Twelve notches, each asked 8 ms after the last landed, then the hand's rest: the readings asked,
/// and the image intervals the notches spanned, plus one; none where no adapter is.
fn readings(held: bool, wanted: [bool; 2]) -> Option<(Vec<(bool, [bool; 2])>, f64)> {
    let engine = Engine::shared(&Device::open().ok()?);
    let decoded = Decoded { linear: Linear { width: 8, height: 8, pixels: vec![[0.3, 0.2, 0.1, 1.0]; 64] }, full_width: 8.0, undeclared: None };
    let negative = Arc::new(engine.prepare(decoded).ok()?);
    let log = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = Worker::new(Box::new(Logged(engine, log.clone())), move |d| {
        if let Delivery::Render(_) = d {
            let _ = tx.send(());
        }
    });
    let job = |revision: u64, held: bool| {
        let (kind, asked) = (crate::timing::Kind::Notch, Instant::now());
        let settings = PipelineSettings { capture_stops: revision as f32 * 0.01, ..PipelineSettings::default() };
        Job { revision, path: PathBuf::new(), negative: negative.clone(), settings, kind, asked, view: None, reference: None, held, counted: false, inputs: wanted }
    };
    // The first render compiles the kernels; its own readings are let go before the notches.
    worker.submit(job(0, false));
    rx.recv_timeout(Duration::from_secs(60)).ok()?;
    std::thread::sleep(Duration::from_millis(300));
    log.lock().unwrap().clear();
    let started = Instant::now();
    for revision in 1..=12 {
        worker.submit(job(revision, held));
        rx.recv_timeout(Duration::from_secs(10)).ok()?;
        std::thread::sleep(Duration::from_millis(8));
    }
    let intervals = started.elapsed().as_secs_f64() / crate::timing::IMAGE_INTERVAL.as_secs_f64() + 1.0;
    std::thread::sleep(Duration::from_millis(400));
    let log = log.lock().unwrap().clone();
    Some((log, intervals))
}

/// Under the hand the distributions are read at the image's cadence, each notch landing before the next
/// is asked, then once on the full copy as the hand rests.
#[test]
fn a_held_notch_reads_its_inputs_at_the_image_cadence_and_the_rest_reads_once() {
    let Some((held, intervals)) = readings(true, [true; 2]) else { return eprintln!("no adapter: the readings' cadence is not exercised") };
    let (small, full) = (held.iter().filter(|(h, _)| *h).count(), held.iter().filter(|(h, _)| !h).count());
    assert!(small >= 1 && small as f64 <= intervals, "{small} readings under the hand for {intervals:.1} intervals");
    assert_eq!((full, held.last().map(|(h, _)| *h)), (1, Some(false)), "one reading on the full copy, at rest");
    // Twin: the same notches not held each read, as each is its burst's last, however long they took.
    let Some((loose, _)) = readings(false, [true; 2]) else { return };
    assert!(loose.len() >= 12, "{} readings for 12 loose notches", loose.len());
}

/// No reading is asked while neither plot is on screen; one on screen is read alone.
#[test]
fn the_inputs_are_read_for_a_plot_on_screen_alone() {
    let Some((none, _)) = readings(true, [false; 2]) else { return eprintln!("no adapter: the plots' readings are not exercised") };
    assert_eq!(none, vec![], "no plot on screen, no reading");
    // Twin: the curves open, each reading is asked for them alone.
    let Some((curves, _)) = readings(true, [false, true]) else { return };
    assert!(!curves.is_empty() && curves.iter().all(|(_, w)| *w == [false, true]), "{curves:?}");
}
