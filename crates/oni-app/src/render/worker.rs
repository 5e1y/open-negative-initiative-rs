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
    Render(Result<Done, String>),
    Inputs { path: PathBuf, inputs: InputHistograms },
}

/// What the levels and the manual curves are drawn over: the distributions in front of each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputHistograms {
    pub levels: oni_measure::histogram::Histogram,
    pub curves: oni_measure::histogram::Histogram,
}

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
                let mut kept: Option<(PipelineSettings, Option<ViewKey>, Source, Option<Placed>)> = None;
                let mut last_inputs = Instant::now() - Duration::from_secs(1);
                loop {
                    let job = {
                        let (lock, wake) = &*shared;
                        let mut s = lock.lock().expect("render slot");
                        while s.waiting.is_none() && !s.closed {
                            s = wake.wait(s).expect("render slot");
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
                                Some((s, k, source, placed)) if s == reference && *k == key => Some((source.clone(), *placed)),
                                _ => match renderer.resident(&job.negative, reference, job.view.as_ref()) {
                                    Some(Ok((source, placed))) => {
                                        kept = Some((reference.clone(), key, source.clone(), placed));
                                        Some((source, placed))
                                    }
                                    _ => None,
                                },
                            }
                        }
                        _ => None,
                    };
                    let drawn = done.is_ok();
                    deliver(Delivery::Render(done.map(|d| Done { before, ..d })));
                    // At the image's own cadence, and always on the last job of a burst, so the hand's
                    // release lands the full copy's reading.
                    let quiet = shared.0.lock().map(|s| s.waiting.is_none()).unwrap_or(true);
                    if drawn && (quiet || last_inputs.elapsed() >= crate::timing::IMAGE_INTERVAL) {
                        last_inputs = Instant::now();
                        if let Some(inputs) = renderer.inputs(&job.negative, &job.settings, job.held && !quiet) {
                            deliver(Delivery::Inputs { path: job.path.clone(), inputs });
                        }
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
