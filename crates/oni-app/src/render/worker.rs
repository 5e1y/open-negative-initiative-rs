//! The render slot: one job in flight and one waiting, a newer one replacing the waiting one.

use super::*;

pub struct Job<I> {
    pub revision: u64,
    pub path: PathBuf,
    pub negative: Arc<I>,
    pub settings: PipelineSettings,
    pub kind: crate::timing::Kind,
    /// When the store asked for it, or the frame was opened, which the end-to-end time starts from.
    pub asked: Instant,
    /// What the canvas shows; none renders the whole preview.
    pub view: Option<View>,
    /// Compare's before, the bar's reset frame with the dust kept, rendered on the same view.
    pub reference: Option<PipelineSettings>,
    /// A gesture holds the frame: the input histograms read the small measuring copy.
    pub held: bool,
}

/// What a render hands the canvas: a texture the display kernel encodes, or for the software
/// renderer the linear image with its bytes encoded on the render thread.
pub enum Picture {
    Resident(Source),
    Pixels { linear: Linear, rgba8: Vec<u8> },
}

/// A finished render, with the histogram of what is shown when the render thread measured it.
pub struct Done {
    pub revision: u64,
    pub path: PathBuf,
    pub picture: Picture,
    pub width: usize,
    pub height: usize,
    pub histogram: Option<oni_measure::histogram::Histogram>,
    pub kind: crate::timing::Kind,
    pub asked: Instant,
    pub pipeline: Duration,
    /// Where the canvas lays it; none fills the canvas.
    pub placed: Option<Placed>,
    pub before: Option<(Source, Option<Placed>)>,
}

/// What the render thread hands back: a render as soon as it is drawn, then, at most once an image
/// interval, the levels' and curves' input distributions, which wait on the GPU for 25 ms or so.
pub enum Delivery {
    /// A failure names its frame, so one that lands after the frame was left is dropped.
    Render(Result<Done, (PathBuf, String)>),
    Inputs { path: PathBuf, inputs: InputHistograms },
    /// The whole picture the histogram counts, made after the canvas's render at the image's cadence.
    Counted { path: PathBuf, picture: Source },
}

/// What the levels and the manual curves are drawn over: the distributions in front of each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputHistograms {
    pub levels: oni_measure::histogram::Histogram,
    pub curves: oni_measure::histogram::Histogram,
}

/// How long the hand rests before the input histograms are read on the full measuring copy.
const REST: Duration = Duration::from_millis(150);

/// `Negative.measureSmall`'s scale of the measuring copy.
pub(crate) const MEASURE_SMALL: f64 = 0.35;

pub(crate) struct Slot<I> {
    waiting: Option<Job<I>>,
    closed: bool,
}

pub struct Worker<I> {
    slot: Arc<(Mutex<Slot<I>>, Condvar)>,
    /// Set while the display kernel draws the canvas, so a notch stays on the GPU end to end.
    pub on_gpu: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl<I: Send + Sync + 'static> Worker<I> {
    pub fn new(mut renderer: Box<dyn Renderer<I>>, screen: Screen, deliver: impl Fn(Delivery) + Send + 'static) -> Worker<I> {
        let slot = Arc::new((Mutex::new(Slot { waiting: None, closed: false }), Condvar::new()));
        let shared = slot.clone();
        let on_gpu = Arc::new(AtomicBool::new(false));
        let gpu = on_gpu.clone();
        let thread = std::thread::Builder::new()
            .name("render".into())
            .spawn(move || {
                let mut seed = 0u32;
                // The before is kept while its settings and the view hold, so a notch never renders it again.
                let mut kept: Option<(PathBuf, PipelineSettings, Option<ViewKey>, Source, Option<Placed>)> = None;
                let mut last_inputs = Instant::now() - Duration::from_secs(1);
                let mut last_counted = last_inputs;
                // The last job read on the small copy, read again on the full one once the hand rests.
                let mut resting: Option<(PathBuf, Arc<I>, PipelineSettings)> = None;
                // The distributions read last: they hang on the frame and its settings, never on the view.
                let mut measured: Option<(PathBuf, PipelineSettings, bool)> = None;
                loop {
                    let job = {
                        let (lock, wake) = &*shared;
                        let mut s = lock.lock().expect("render slot");
                        while s.waiting.is_none() && !s.closed {
                            if resting.is_none() {
                                s = wake.wait(s).expect("render slot");
                                continue;
                            }
                            let (next, waited) = wake.wait_timeout(s, REST).expect("render slot");
                            s = next;
                            if waited.timed_out() && s.waiting.is_none() && !s.closed {
                                drop(s);
                                if let Some((path, negative, settings)) = resting.take()
                                    && measured.as_ref() != Some(&(path.clone(), settings.clone(), false))
                                    && let Some(inputs) = renderer.inputs(&negative, &settings, false)
                                {
                                    measured = Some((path.clone(), settings, false));
                                    deliver(Delivery::Inputs { path, inputs });
                                }
                                s = lock.lock().expect("render slot");
                            }
                        }
                        if s.closed {
                            return;
                        }
                        s.waiting.take().expect("a job")
                    };
                    seed = seed.wrapping_add(1);
                    let started = Instant::now();
                    let resident = if gpu.load(Ordering::Relaxed) { renderer.resident(&job.negative, &job.settings, job.view.as_ref()) } else { None };
                    let done = match resident {
                        Some(made) => made.map(|(source, placed)| {
                            let e = source.extent();
                            let (width, height) = (e.w as usize, e.h as usize);
                            let picture = Picture::Resident(source);
                            Done { revision: job.revision, path: job.path.clone(), picture, width, height, histogram: None, kind: job.kind, asked: job.asked, pipeline: started.elapsed(), placed, before: None }
                        }),
                        None => renderer.render(&job.negative, &job.settings).map(|linear| {
                            let pipeline = started.elapsed();
                            let rgba8 = encoded(&linear, &Params::new(screen, linear.width as u32, linear.height as u32, seed), 1);
                            let histogram = Some(crate::histogram::measure(&linear.pixels, linear.width, linear.height, screen));
                            let (width, height) = (linear.width, linear.height);
                            let picture = Picture::Pixels { linear, rgba8 };
                            Done { revision: job.revision, path: job.path.clone(), picture, width, height, histogram, kind: job.kind, asked: job.asked, pipeline, placed: None, before: None }
                        }),
                    };
                    let before = match (&job.reference, gpu.load(Ordering::Relaxed)) {
                        (Some(reference), true) => {
                            let key = job.view.as_ref().map(View::key);
                            match &kept {
                                Some((p, s, k, source, placed)) if *p == job.path && s == reference && *k == key => Some((source.clone(), *placed)),
                                _ => match renderer.resident(&job.negative, reference, job.view.as_ref()) {
                                    Some(Ok((source, placed))) => {
                                        kept = Some((job.path.clone(), reference.clone(), key, source.clone(), placed));
                                        Some((source, placed))
                                    }
                                    _ => None,
                                },
                            }
                        }
                        // Compare off lets its kept before go, a canvas of floats.
                        _ => {
                            kept = None;
                            None
                        }
                    };
                    let drawn = done.is_ok();
                    deliver(Delivery::Render(done.map(|d| Done { before, ..d }).map_err(|e| (job.path.clone(), e))));
                    // At the image's own cadence, and always on the last job of a burst; under the hand on
                    // the small copy, the full one read once the hand has rested.
                    let quiet = shared.0.lock().map(|s| s.waiting.is_none()).unwrap_or(true);
                    if drawn && gpu.load(Ordering::Relaxed) && (quiet || last_counted.elapsed() >= crate::timing::IMAGE_INTERVAL) {
                        last_counted = Instant::now();
                        if let Some(picture) = renderer.counted(&job.negative, &job.settings) {
                            deliver(Delivery::Counted { path: job.path.clone(), picture });
                        }
                    }
                    let read = (job.path.clone(), job.settings.clone(), job.held);
                    if drawn && measured.as_ref() != Some(&read) && (quiet || last_inputs.elapsed() >= crate::timing::IMAGE_INTERVAL) {
                        last_inputs = Instant::now();
                        if let Some(inputs) = renderer.inputs(&job.negative, &job.settings, job.held) {
                            deliver(Delivery::Inputs { path: job.path.clone(), inputs });
                        }
                        measured = Some(read);
                        resting = job.held.then(|| (job.path.clone(), job.negative.clone(), job.settings.clone()));
                    }
                }
            })
            .expect("render thread");
        Worker { slot, on_gpu, thread: Some(thread) }
    }

    /// Lays `job` in the slot, replacing a job still waiting there.
    pub fn submit(&self, job: Job<I>) {
        let (lock, wake) = &*self.slot;
        lock.lock().expect("render slot").waiting = Some(job);
        wake.notify_one();
    }
}

impl<I> Drop for Worker<I> {
    fn drop(&mut self) {
        let (lock, wake) = &*self.slot;
        lock.lock().expect("render slot").closed = true;
        wake.notify_one();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
