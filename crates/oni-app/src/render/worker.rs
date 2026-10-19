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
    pub histogram: Option<Histogram>,
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

impl InputHistograms {
    /// `HistogramTraces`: each channel's counts coarsened to the plot's columns, `HistogramScale`'s
    /// heights (the peak off the two bins at each end, then the 0.42 power), filled at 28 % under an
    /// 85 % edge, a channel out of focus at a fifth; premultiplied RGBA8 of `w` by `h`.
    pub fn raster(histogram: &oni_measure::histogram::Histogram, focus: Option<usize>, w: usize, h: usize) -> Vec<u8> {
        let mut out = vec![[0f32; 4]; w * h];
        let n = histogram.rgb.len();
        if n < 2 || w == 0 || h == 0 {
            return vec![0; w * h * 4];
        }
        for c in 0..3 {
            let counts: Vec<f32> = histogram.rgb.iter().map(|b| b[c].max(0.0)).collect();
            let interior = if n > 4 { &counts[2..n - 2] } else { &counts[..] };
            let peak = interior.iter().copied().fold(0.0, f32::max);
            if peak <= 0.0 {
                continue;
            }
            let column = |x: usize| {
                let (a, b) = (x * n / w, ((x + 1) * n / w).max(x * n / w + 1).min(n));
                let mean = counts[a..b].iter().sum::<f32>() / (b - a) as f32;
                (mean / peak).powf(0.42).min(1.0)
            };
            let strength = if focus.is_some_and(|f| f != c) { 0.2 } else { 1.0 };
            for x in 0..w {
                let top = h as f32 * (1.0 - column(x));
                for y in 0..h {
                    let edge = (y as f32 - top).abs() < 1.0;
                    let a = if edge { 0.85 } else if y as f32 >= top { 0.28 } else { continue } * strength;
                    let px = &mut out[y * w + x];
                    // Source over, premultiplied, in the channel's own primary.
                    for k in 0..3 {
                        px[k] = if k == c { a } else { 0.0 } + px[k] * (1.0 - a);
                    }
                    px[3] = a + px[3] * (1.0 - a);
                }
            }
        }
        out.iter().flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)).collect()
    }
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
                            let histogram = Some(Histogram::of(&rgba8));
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
