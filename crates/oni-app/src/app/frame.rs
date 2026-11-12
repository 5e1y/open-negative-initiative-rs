//! The frame the editor holds: its path and reading, what was decoded and rendered of it and what the
//! canvas asked, owned together so another frame or a close lets all of it go at once.

use super::*;

pub(in crate::app) struct Frame {
    pub path: String,
    /// How its preview was decoded: a sidecar naming another reading decodes it again.
    pub reading: render::Reading,
    /// Its preview, which the canvas renders, and its full-resolution pixels once built.
    pub negative: Option<Arc<Negative>>,
    pub full: Option<oni_graph::Source>,
    /// The heal run cutting patches on `full`, which ends with it.
    pub heal: Option<rendering::Healing>,
    /// What the canvas asked of it and was given, and Compare over it.
    pub requests: rendering::Requests,
    pub compare: rendering::Compare,
    /// The last render placed, and the displayed image's size in full-resolution pixels.
    pub placed: Option<Placed>,
    pub full_size: Option<(f64, f64)>,
    /// Its settings as a canvas tool found them, which the tool's Cancel puts back.
    pub tool_entry: Option<oni_settings::PipelineSettings>,
    pub frame_gesture: Option<cropping::FrameGesture>,
    /// The frame a straightening gesture set out from, the one it last wrote, and when.
    pub straightening: Option<(Geometry, Geometry, Instant)>,
    /// The stroke being painted, in fractions of the rendered frame, y down.
    pub painting_stroke: dedusting::live::Live,
}

impl Frame {
    /// `path` opened under `reading`. The full resolution and its heal run stay when `previous` held the same
    /// frame; the last picture's place and size stay until this one's render lands, as its thumbnail is laid by them.
    pub fn opened(path: &str, reading: render::Reading, previous: Option<Frame>) -> Frame {
        let (full, heal, placed, full_size) = previous.map_or((None, None, None, None), |mut p| {
            let same = p.path == path;
            (p.full.take().filter(|_| same), p.heal.take().filter(|_| same), p.placed, p.full_size)
        });
        Frame {
            path: path.to_string(),
            reading,
            negative: None,
            full,
            heal,
            requests: rendering::Requests::default(),
            compare: rendering::Compare::default(),
            placed,
            full_size,
            tool_entry: None,
            frame_gesture: None,
            straightening: None,
            painting_stroke: Default::default(),
        }
    }

    /// Its full resolution let go, up to 1.6 GB on a 24 MP frame, and the run cutting patches on it; the
    /// view builds it again once it asks.
    pub fn release_full(&mut self) {
        self.full = None;
        self.heal = None;
    }

    /// Read again under `reading`: what was decoded under the last one goes.
    pub fn reread(&mut self, reading: render::Reading) {
        self.reading = reading;
        self.negative = None;
        self.full = None;
        self.heal = None;
    }
}

impl State {
    /// The open frame when it is `path`'s: the one guard every landing of a background read goes
    /// through, as one for a frame left is stale.
    pub(in crate::app) fn frame_for(&mut self, path: &str) -> Option<&mut Frame> {
        self.frame.as_mut().filter(|f| f.path == path)
    }

    /// Whether a decode of `path` under `reading` is the open frame's: any other is superseded.
    pub(super) fn wants(&mut self, path: &str, reading: &render::Reading) -> bool {
        self.frame_for(path).is_some_and(|f| f.reading == *reading)
    }

    pub fn open_path(&self) -> Option<&str> {
        self.frame.as_ref().map(|f| f.path.as_str())
    }

    /// The open frame's preview, once decoded.
    pub(in crate::app) fn negative(&self) -> Option<&Arc<Negative>> {
        self.frame.as_ref()?.negative.as_ref()
    }

    /// The open frame's path and preview, once decoded, for a thread to carry.
    pub(in crate::app) fn decoded(&self) -> Option<(String, Arc<Negative>)> {
        let f = self.frame.as_ref()?;
        Some((f.path.clone(), f.negative.clone()?))
    }

    /// The displayed image's size in full-resolution pixels, from the last render placed.
    pub fn full_size(&self) -> Option<(f64, f64)> {
        self.frame.as_ref()?.full_size
    }
}

impl Ctl {
    /// The one close of the open frame, whose file is gone or moved out: everything of it goes, the
    /// held crop handle let go and Compare's split taken off the canvas with it.
    pub(in crate::app) fn close_frame(self: &Rc<Self>) {
        self.end_frame_gesture();
        let comparing = self.state.borrow_mut().frame.take().is_some_and(|f| f.compare.split.is_some());
        if comparing {
            self.show_split(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(path: &str) -> Frame {
        let mut f = Frame::opened(path, render::Reading::default(), None);
        f.full_size = Some((6048.0, 4032.0));
        f
    }

    #[test]
    fn another_frame_opened_finds_only_the_last_pictures_place() {
        let next = Frame::opened("b", render::Reading::default(), Some(held("a")));
        assert_eq!(next.full_size, Some((6048.0, 4032.0)), "the last picture's size lays the thumbnail");
        assert!(next.full.is_none() && next.negative.is_none() && next.requests.rendered == 0);
        let first = Frame::opened("b", render::Reading::default(), None);
        assert_eq!(first.full_size, None, "twin: with no frame before, no size is guessed");
    }

    #[test]
    fn a_heal_run_ends_with_the_full_resolution_it_reads() {
        let running = |f: &mut Frame| {
            let (run, latest) = rendering::Healing::new(oni_settings::PipelineSettings::default());
            f.heal = Some(run);
            latest
        };
        let reads = |latest: &std::sync::Arc<std::sync::Mutex<Option<oni_settings::PipelineSettings>>>| latest.lock().unwrap().is_some();
        let mut f = held("a");
        let latest = running(&mut f);
        f.reread(render::Reading { slide: true, ..render::Reading::default() });
        assert!(!reads(&latest), "a reread left the run cutting on the old decode");
        let latest = running(&mut f);
        f.release_full();
        assert!(!reads(&latest), "the full resolution let go left its run");
        let latest = running(&mut f);
        let other = Frame::opened("b", render::Reading::default(), Some(f));
        assert!(!reads(&latest), "another frame opened left the last one's run");
        let mut f = other;
        let latest = running(&mut f);
        drop(f);
        assert!(!reads(&latest), "a closed frame left its run");
        // Twins: a frame still held, or opened again with its full resolution, keeps its run.
        let mut f = held("a");
        let latest = running(&mut f);
        f.requests = rendering::Requests::default();
        assert!(reads(&latest), "a run ended though its frame was held");
        let again = Frame::opened("a", render::Reading::default(), Some(f));
        assert!(reads(&latest) && again.heal.is_some(), "the same frame opened again lost its run");
    }
}
