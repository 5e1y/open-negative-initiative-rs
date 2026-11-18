// ONI: each interface frame's stages, kept on the interface thread while a bench asks, so the chain
// from a notch to the screen is measured whole, its end included.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

/// One interface frame: when its drawable was asked, each stage's time and what it drew again.
#[derive(Clone, Debug)]
pub struct FrameRecord {
    pub began: Instant,
    /// Waiting for a drawable.
    pub acquired: Duration,
    /// From the drawable to the items' start: the clear and the notifiers.
    pub prepared: Duration,
    pub items: Duration,
    /// The flush to the drawable and its submit.
    pub submitted: Duration,
    /// When the present call returned.
    pub presented: Instant,
    pub submits: u32,
    /// Each layer drawn again: its size in pixels and its time.
    pub layers: Vec<(u32, u32, Duration)>,
    /// Each shadow blurred again: its size in pixels and where its element lies.
    pub shadows: Vec<(u32, u32, String)>,
}

thread_local! {
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    static CURRENT: RefCell<Option<FrameRecord>> = const { RefCell::new(None) };
    static FRAMES: RefCell<Vec<FrameRecord>> = const { RefCell::new(Vec::new()) };
}

/// The frames are kept from now on while `on`; off drops those kept.
pub fn oni_record(on: bool) {
    RECORDING.with(|r| r.set(on));
    if !on {
        FRAMES.with(|f| f.borrow_mut().clear());
    }
}

/// The frames kept since the last call.
pub fn oni_frames() -> Vec<FrameRecord> {
    FRAMES.with(|f| std::mem::take(&mut *f.borrow_mut()))
}

/// A frame whose drawable was asked at `began` and given now.
pub(crate) fn began(began: Instant) {
    if RECORDING.with(Cell::get) {
        let now = Instant::now();
        let frame = FrameRecord {
            began,
            acquired: now - began,
            prepared: Duration::ZERO,
            items: Duration::ZERO,
            submitted: Duration::ZERO,
            presented: now,
            submits: 0,
            layers: Vec::new(),
            shadows: Vec::new(),
        };
        CURRENT.with(|c| *c.borrow_mut() = Some(frame));
    }
}

/// `with` applied to the frame being drawn, where one is recorded.
pub(crate) fn with(with: impl FnOnce(&mut FrameRecord)) {
    CURRENT.with(|c| {
        if let Some(frame) = c.borrow_mut().as_mut() {
            with(frame);
        }
    });
}

/// The frame presented now, kept.
pub(crate) fn presented() {
    if let Some(mut frame) = CURRENT.with(|c| c.borrow_mut().take()) {
        frame.presented = Instant::now();
        FRAMES.with(|f| f.borrow_mut().push(frame));
    }
}

/// Where `item` lies in the window, in points, and its size, which names it on screen.
pub(crate) fn named(item: &i_slint_core::items::ItemRc) -> String {
    let (at, size) = (item.map_to_window(Default::default()), item.geometry().size);
    format!("{:.0}×{:.0} at {:.0},{:.0}", size.width, size.height, at.x, at.y)
}
