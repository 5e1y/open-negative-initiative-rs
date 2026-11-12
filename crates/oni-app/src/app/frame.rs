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
    /// What the canvas asked of it and was given, and Compare over it.
    pub requests: rendering::Requests,
    pub compare: rendering::Compare,
    /// The last render of it placed.
    pub placed: Option<Placed>,
    /// Its settings as a canvas tool found them, which the tool's Cancel puts back.
    pub tool_entry: Option<oni_settings::PipelineSettings>,
    pub frame_gesture: Option<cropping::FrameGesture>,
    /// The frame a straightening gesture set out from, the one it last wrote, and when.
    pub straightening: Option<(Geometry, Geometry, Instant)>,
    /// The stroke being painted, in fractions of the rendered frame, y down.
    pub painting_stroke: dedusting::live::Live,
}

impl Frame {
    /// `path` opened under `reading`. The full resolution stays when `previous` held the same frame.
    pub fn opened(path: &str, reading: render::Reading, previous: Option<Frame>) -> Frame {
        Frame {
            path: path.to_string(),
            reading,
            negative: None,
            full: previous.filter(|p| p.path == path).and_then(|p| p.full),
            requests: rendering::Requests::default(),
            compare: rendering::Compare::default(),
            placed: None,
            tool_entry: None,
            frame_gesture: None,
            straightening: None,
            painting_stroke: Default::default(),
        }
    }

    /// Its full resolution let go, up to 1.6 GB on a 24 MP frame; the view builds it again once it asks.
    pub fn release_full(&mut self) {
        self.full = None;
    }

    /// Read again under `reading`: what was decoded under the last one goes.
    pub fn reread(&mut self, reading: render::Reading) {
        self.reading = reading;
        self.negative = None;
        self.full = None;
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
        f.placed = Some(Placed { x: 0.0, y: 0.0, width: 600.0, height: 400.0, full: (6048.0, 4032.0), whole: true, at_full_resolution: false });
        f
    }

    #[test]
    fn another_frame_opened_takes_no_place_from_the_last() {
        let next = Frame::opened("b", render::Reading::default(), Some(held("a")));
        assert!(next.placed.is_none(), "the last picture's place is not this one's");
        assert!(next.full.is_none() && next.negative.is_none() && next.requests.rendered == 0);
        assert!(held("a").placed.is_some(), "twin: a frame whose render was placed holds its place");
    }
}
