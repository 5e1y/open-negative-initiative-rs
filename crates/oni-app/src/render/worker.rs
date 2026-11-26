//! The render slot: one job in flight until the GPU has run it and one waiting, a newer one replacing the waiting one.

use super::*;
use crate::latest::{Latest, Taken};

#[derive(Clone)]
pub struct Job {
    pub revision: u64,
    pub path: PathBuf,
    pub negative: Arc<Negative>,
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
    /// A plot over the picture's histogram is on screen, which the whole picture is counted for.
    pub counted: bool,
    /// The levels' and the curves' plots on screen, whose input distributions alone are read.
    pub inputs: [bool; 2],
}

/// A finished render: the texture the display kernel encodes into the canvas.
pub struct Done {
    pub revision: u64,
    pub path: PathBuf,
    pub picture: Source,
    pub kind: crate::timing::Kind,
    pub asked: Instant,
    pub pipeline: Duration,
    /// When it was drawn, `pipeline` after its start.
    pub drawn: Instant,
    /// Where the canvas lays it; none fills the canvas.
    pub placed: Option<Placed>,
    /// Compare's before, the same region as `picture`, so it lies at `placed` too.
    pub before: Option<Source>,
    /// The viewport and canvas `placed` was laid by: a landing lays it again where the view now puts it.
    pub view: Option<(crate::viewport::Viewport, (f64, f64))>,
    /// Rendered lighter, under a moving hand.
    pub coarse: bool,
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

/// What the levels and the manual curves are drawn over: the distributions in front of each, none
/// for a plot off screen, which keeps what it last showed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputHistograms {
    pub levels: Option<oni_measure::histogram::Histogram>,
    pub curves: Option<oni_measure::histogram::Histogram>,
}

/// How long the hand rests before the input histograms are read on the full measuring copy.
const REST: Duration = Duration::from_millis(150);

/// `Negative.measureSmall`'s scale of the measuring copy.
pub(crate) const MEASURE_SMALL: f64 = 0.35;

pub struct Worker {
    slot: Arc<Latest<Job>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    pub fn new(mut renderer: Box<dyn Renderer>, deliver: impl Fn(Delivery) + Send + 'static) -> Worker {
        let slot: Arc<Latest<Job>> = Arc::new(Latest::new());
        let shared = slot.clone();
        let thread = std::thread::Builder::new()
            .name("render".into())
            .spawn(move || {
                let mut last_inputs = Instant::now() - Duration::from_secs(1);
                let mut last_counted = last_inputs;
                // The hand's last job, read on the full copy once the hand rests.
                let mut resting: Option<(PathBuf, Arc<Negative>, PipelineSettings, [bool; 2])> = None;
                // A held job drawn on a digital photo's preview scan, rendered again sharp once the hand rests.
                let mut sharpen: Option<Job> = None;
                loop {
                    let (job, sharpening) = loop {
                        match shared.take((resting.is_some() || sharpen.is_some()).then_some(REST)) {
                            Taken::Value(job) => break (job, false),
                            Taken::Closed => return,
                            Taken::TimedOut if sharpen.is_some() => break (Job { held: false, ..sharpen.take().expect("a job to sharpen") }, true),
                            Taken::TimedOut => {
                                if let Some((path, negative, settings, wanted)) = resting.take()
                                    && let Some(inputs) = renderer.inputs(&negative, &settings, false, wanted)
                                {
                                    deliver(Delivery::Inputs { path, inputs });
                                }
                            }
                        }
                    };
                    let started = Instant::now();
                    // A digital photo's full scan is made only once the hand or the pointer rests: the job that
                    // sharpens the last one drawn on the preview's scan alone makes it.
                    sharpen = None;
                    renderer.hold_scan(!sharpening);
                    let done = renderer.resident(&job.negative, &job.settings, job.view.as_ref()).map(|(picture, placed)| Done {
                        revision: job.revision,
                        path: job.path.clone(),
                        picture,
                        kind: job.kind,
                        asked: job.asked,
                        pipeline: started.elapsed(),
                        drawn: Instant::now(),
                        placed,
                        before: None,
                        view: job.view.as_ref().map(|v| (v.viewport, v.size)),
                        coarse: job.view.as_ref().is_some_and(|v| v.coarse),
                    });
                    // The engine keeps the before while what it reads holds; Compare off lets it go.
                    let before = renderer.before(&job.negative, job.reference.as_ref(), job.view.as_ref());
                    let drawn = done.is_ok();
                    deliver(Delivery::Render(done.map(|d| Done { before, ..d }).map_err(|e| (job.path.clone(), e))));
                    // At the image's own cadence, and always on the last job of a burst; under the hand on
                    // the small copy, the full one read once the hand has rested.
                    let quiet = !shared.waiting();
                    if drawn && job.counted && (quiet || last_counted.elapsed() >= crate::timing::IMAGE_INTERVAL) {
                        last_counted = Instant::now();
                        if let Some(picture) = renderer.counted(&job.negative, &job.settings) {
                            deliver(Delivery::Counted { path: job.path.clone(), picture });
                        }
                    }
                    // A held job is never a burst's last, though the next notch may not be waiting yet: it
                    // reads at the image's cadence alone, as each reading holds the next notch 6 ms or more.
                    let due = last_inputs.elapsed() >= crate::timing::IMAGE_INTERVAL || (quiet && !job.held);
                    let wanted = job.inputs.contains(&true);
                    if drawn && wanted && due {
                        last_inputs = Instant::now();
                        if let Some(inputs) = renderer.inputs(&job.negative, &job.settings, job.held, job.inputs) {
                            deliver(Delivery::Inputs { path: job.path.clone(), inputs });
                        }
                    }
                    if drawn {
                        resting = (job.held && wanted).then(|| (job.path.clone(), job.negative.clone(), job.settings.clone(), job.inputs));
                        if renderer.provisional() {
                            sharpen = Some(job);
                        }
                    }
                    // The next job waits for the GPU to run this one: taken sooner, it queues behind it there,
                    // and the window's frames behind both, 50 to 100 ms late under a pinch.
                    renderer.ran();
                }
            })
            .expect("render thread");
        Worker { slot, thread: Some(thread) }
    }

    /// Lays `job` in the slot, replacing a job still waiting there.
    pub fn submit(&self, job: Job) {
        self.slot.put(job);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.slot.close();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
