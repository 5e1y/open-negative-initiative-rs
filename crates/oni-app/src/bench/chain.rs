//! The chain from a notch to the screen: each notch's ask, render, delivery and encode, closed at the first
//! present after its encode and at the GPU's end of that frame; each interface frame's stages, the layers
//! drawn again and the shadows blurred; and the view's moves and renders, which `ONI_BENCH=sweep` reads.

use i_slint_renderer_femtovg::{FrameRecord, oni_frames, oni_record};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// One notch on its way to the screen.
#[derive(Clone, Copy, Debug)]
pub struct Stages {
    pub asked: Instant,
    pub started: Instant,
    pub drawn: Instant,
    pub delivered: Instant,
    pub encoded: Instant,
}

/// A navigation applied on the interface thread: when it began, how long it took and the scale it left.
#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub at: Instant,
    pub took: Duration,
    pub scale: f64,
}

/// A view's render encoded into the canvas: its stages, whether lighter, and the scale it was rendered at
/// against the one the view had when it landed.
#[derive(Clone, Copy, Debug)]
pub struct Viewed {
    pub stages: Stages,
    pub coarse: bool,
    pub asked_scale: f64,
    pub landed_scale: f64,
}

/// The canvas as the interface thread left it: when, whether Compare's before shown holds the after's
/// region (none while no before shows), and whether a display texture was made, for the after or the before.
#[derive(Clone, Copy, Debug, Default)]
pub struct Canvas {
    pub at: Option<Instant>,
    pub before: Option<bool>,
    pub made: Option<Side>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    After,
    Before,
}

/// What a bench recorded: the notches, the view's moves and renders, the canvas's states and the
/// interface's frames.
pub struct Recorded {
    pub notches: Vec<Stages>,
    pub moves: Vec<Move>,
    pub views: Vec<Viewed>,
    pub canvas: Vec<Canvas>,
    pub frames: Vec<FrameRecord>,
}

thread_local! {
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    static NOTCHES: RefCell<Vec<Stages>> = const { RefCell::new(Vec::new()) };
    static MOVES: RefCell<Vec<Move>> = const { RefCell::new(Vec::new()) };
    static VIEWS: RefCell<Vec<Viewed>> = const { RefCell::new(Vec::new()) };
    static CANVAS: RefCell<Vec<Canvas>> = const { RefCell::new(Vec::new()) };
}

/// Every notch, move, view render and interface frame recorded from now on, those before let go.
pub fn record() {
    RECORDING.with(|r| r.set(true));
    NOTCHES.with(|n| n.borrow_mut().clear());
    MOVES.with(|m| m.borrow_mut().clear());
    VIEWS.with(|v| v.borrow_mut().clear());
    CANVAS.with(|c| c.borrow_mut().clear());
    oni_record(true);
    let _ = oni_frames();
}

pub fn recording() -> bool {
    RECORDING.with(Cell::get)
}

/// A navigation applied, kept while a bench records.
pub fn moved(m: Move) {
    if recording() {
        MOVES.with(|v| v.borrow_mut().push(m));
    }
}

/// A view's render encoded, kept while a bench records.
pub fn viewed(v: Viewed) {
    if recording() {
        VIEWS.with(|k| k.borrow_mut().push(v));
    }
}

/// The canvas laid or encoded, kept while a bench records.
pub fn canvas(c: Canvas) {
    if recording() {
        CANVAS.with(|k| k.borrow_mut().push(Canvas { at: Some(Instant::now()), ..c }));
    }
}

/// Everything recorded since `record`, which stops recording.
pub fn taken() -> Recorded {
    RECORDING.with(|r| r.set(false));
    let frames = oni_frames();
    oni_record(false);
    Recorded {
        notches: NOTCHES.with(|n| std::mem::take(&mut *n.borrow_mut())),
        moves: MOVES.with(|m| std::mem::take(&mut *m.borrow_mut())),
        views: VIEWS.with(|v| std::mem::take(&mut *v.borrow_mut())),
        canvas: CANVAS.with(|c| std::mem::take(&mut *c.borrow_mut())),
        frames,
    }
}

/// The photon of what the interface thread had done by `t`: the GPU's end of the first frame begun after,
/// as the frame drawn on the same thread holds it; none while that frame's end is unknown.
pub fn photon(t: Instant, frames: &[FrameRecord]) -> Option<Instant> {
    frames.iter().find(|f| f.began >= t).and_then(|f| f.gpu.get().copied())
}

/// A notch encoded into the canvas, kept while a bench records.
pub fn notch(stages: Stages) {
    if RECORDING.with(Cell::get) {
        NOTCHES.with(|n| n.borrow_mut().push(stages));
    }
}

/// For each encode, the first present at or after it; none for one no frame presented yet.
pub(crate) fn presented(encoded: &[Instant], presents: &[Instant]) -> Vec<Option<Instant>> {
    encoded.iter().map(|e| presents.iter().copied().filter(|p| p >= e).min()).collect()
}

/// The report of everything recorded since `record`, which stops recording.
pub fn report() -> String {
    let r = taken();
    said(&r.notches, &r.frames)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// p50, p90, p99 and worst of `samples`, each the sample at its rank.
pub(super) fn centiles(mut samples: Vec<Duration>) -> String {
    if samples.is_empty() {
        return "none".into();
    }
    samples.sort_unstable();
    let at = |q: f64| ms(samples[((samples.len() as f64 * q) as usize).min(samples.len() - 1)]);
    format!("p50 {:.2}, p90 {:.2}, p99 {:.2}, worst {:.2} ms", at(0.5), at(0.9), at(0.99), at(1.0))
}

fn said(notches: &[Stages], frames: &[FrameRecord]) -> String {
    let presents: Vec<Instant> = frames.iter().map(|f| f.presented).collect();
    let closed = presented(&notches.iter().map(|n| n.encoded).collect::<Vec<_>>(), &presents);
    let pairs: Vec<(&Stages, Instant)> = notches.iter().zip(closed).filter_map(|(n, p)| Some((n, p?))).collect();
    let stage = |f: &dyn Fn(&Stages, Instant) -> Duration| centiles(pairs.iter().map(|(n, p)| f(n, *p)).collect());
    let mut out = format!("chain: {} notches encoded, {} presented after their encode", notches.len(), pairs.len());
    let _ = write!(out, "\n  asked to render start {}", stage(&|n, _| n.started - n.asked));
    let _ = write!(out, "\n  render {}", stage(&|n, _| n.drawn - n.started));
    let _ = write!(out, "\n  render to delivered {}", stage(&|n, _| n.delivered.saturating_duration_since(n.drawn)));
    let _ = write!(out, "\n  delivered to encoded {}", stage(&|n, _| n.encoded - n.delivered));
    let _ = write!(out, "\n  encoded to presented {}", stage(&|n, p| p - n.encoded));
    let _ = write!(out, "\n  asked to presented {}", stage(&|n, p| p - n.asked));
    let photons: Vec<Duration> = notches.iter().filter_map(|n| Some(photon(n.encoded, frames)? - n.asked)).collect();
    let _ = write!(out, "\n  asked to the GPU's end of its frame {}", centiles(photons));
    let gaps: Vec<Duration> = presents.windows(2).map(|w| w[1] - w[0]).collect();
    let _ = write!(out, "\nframes: {} presented; gap {}", frames.len(), centiles(gaps));
    let part = |f: &dyn Fn(&FrameRecord) -> Duration| centiles(frames.iter().map(f).collect());
    let _ = write!(out, "\n  acquire {}\n  prepare {}", part(&|f| f.acquired), part(&|f| f.prepared));
    let _ = write!(out, "\n  items {}\n  submit {}", part(&|f| f.items), part(&|f| f.submitted));
    let _ = write!(out, "\n  whole {}", part(&|f| f.presented - f.began));
    let submits = frames.iter().map(|f| f.submits).max().unwrap_or(0);
    let shadows: usize = frames.iter().map(|f| f.shadows.len()).sum();
    let shadowed = frames.iter().filter(|f| !f.shadows.is_empty()).count();
    let _ = write!(out, "\n  submits a frame at most {submits}; shadows blurred {shadows}, in {shadowed} frames");
    // Each layer size drawn again: in how many frames, and its time.
    let mut layers: BTreeMap<(u32, u32), Vec<Duration>> = BTreeMap::new();
    for (w, h, took) in frames.iter().flat_map(|f| f.layers.iter()) {
        layers.entry((*w, *h)).or_default().push(*took);
    }
    for ((w, h), took) in layers {
        let _ = write!(out, "\n  layer {w}×{h} drawn {} times, {}", took.len(), centiles(took));
    }
    let mut blurred: BTreeMap<(u32, u32, &str), usize> = BTreeMap::new();
    for (w, h, named) in frames.iter().flat_map(|f| f.shadows.iter()) {
        *blurred.entry((*w, *h, named.as_str())).or_default() += 1;
    }
    for ((w, h, named), n) in blurred {
        let _ = write!(out, "\n  shadow {w}×{h} blurred {n} times: {named}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notch_closes_at_the_first_present_after_its_encode() {
        let t = Instant::now();
        let at = |ms: u64| t + Duration::from_millis(ms);
        let presents = [at(5), at(13), at(21), at(29)];
        let closed = presented(&[at(4), at(13), at(14), at(30)], &presents);
        assert_eq!(closed, vec![Some(at(5)), Some(at(13)), Some(at(21)), None]);
        // Twin: closing at the present before the encode reads a frame that never showed it.
        let before: Vec<Option<Instant>> = [at(4), at(13), at(14)].iter().map(|e| presents.iter().copied().filter(|p| p < e).max()).collect();
        assert_ne!(before, closed[..3].to_vec());
    }

    #[test]
    fn a_move_closes_at_the_gpu_end_of_the_first_frame_drawn_after_it() {
        let t = Instant::now();
        let at = |ms: u64| t + Duration::from_millis(ms);
        // Frames begun every 8 ms, presented 2 ms later, their GPU work done 48 ms after the present.
        let frames: Vec<FrameRecord> = (0..6u64).map(|k| frame(at(8 * k), at(8 * k + 2), Some(at(8 * k + 50)))).collect();
        let m = Move { at: at(3), took: Duration::from_millis(1), scale: 1.0 };
        let closed = photon(m.at + m.took, &frames).map(|p| p - m.at);
        assert_eq!(closed, Some(Duration::from_millis(55)), "the frame begun at 8 ms ends on the GPU at 58 ms");
        // Twin: closing at the same frame's present call reads the move 48 ms early.
        let presented = frames.iter().find(|f| f.began >= m.at + m.took).map(|f| f.presented - m.at);
        assert_eq!(closed.zip(presented).map(|(c, p)| c - p), Some(Duration::from_millis(48)));
        // A frame whose end is not known yet closes nothing.
        assert_eq!(photon(at(9), &[frame(at(16), at(18), None)]), None);
    }

    fn frame(began: Instant, presented: Instant, gpu: Option<Instant>) -> FrameRecord {
        let end = std::sync::Arc::new(std::sync::OnceLock::new());
        if let Some(g) = gpu {
            let _ = end.set(g);
        }
        let zero = Duration::ZERO;
        FrameRecord { began, acquired: zero, prepared: zero, items: zero, submitted: zero, presented, gpu: end, submits: 1, layers: Vec::new(), shadows: Vec::new() }
    }
}
