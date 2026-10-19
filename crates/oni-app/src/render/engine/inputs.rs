//! `refreshLevelsInput` and `refreshManualCurvesInput`: the distributions in front of the levels and
//! of the manual curves, read again only when what enters that stage moves.

use super::*;
use oni_measure::histogram::Histogram;
use oni_measure::reader::{Copy, Reader};
use oni_measure::truncate::{Stage, truncated};

/// One stage's last reading: the preview and copy it was read on, the settings entering the stage.
pub(super) struct Read {
    preview: u64,
    held: bool,
    entering: PipelineSettings,
    histogram: Histogram,
}

impl Engine {
    /// Both distributions through the measuring windows, on the small copy while `held`; a stage
    /// whose entering settings and copy hold keeps its reading, so a later slider reads nothing.
    pub fn inputs(&mut self, negative: &Negative, settings: &PipelineSettings, held: bool) -> Option<InputHistograms> {
        let id = negative.preview.id();
        if self.measures.as_ref().is_none_or(|(kept, _, _)| *kept != id) {
            let mut r = Reader { graph: &mut self.graph, pipeline: &mut self.pipeline };
            let measure = r.measuring(&negative.preview)?;
            // `measureSmall`: a third of the measuring copy's side, as `CILanczosScaleTransform` makes it.
            let small = self.graph.materialise(&Image::new(&measure).lanczos_anchored(MEASURE_SMALL)).ok()?;
            self.measures = Some((id, measure, small));
        }
        let (_, measure, small) = self.measures.as_ref()?;
        let source = if held { small.clone() } else { measure.clone() };
        let mut read = |slot: usize, stage: Stage| -> Histogram {
            let entering = truncated(settings, stage);
            if let Some(r) = &self.read[slot]
                && r.preview == id
                && r.held == held
                && r.entering == entering
            {
                return r.histogram.clone();
            }
            let copy = Copy { source: &source, full_width: Some(negative.full_width) };
            let histogram = Reader { graph: &mut self.graph, pipeline: &mut self.pipeline }.reading(copy, settings, stage).histogram;
            self.read[slot] = Some(Read { preview: id, held, entering, histogram: histogram.clone() });
            histogram
        };
        let levels = read(0, Stage::Levels);
        let curves = read(1, Stage::ManualCurves);
        Some(InputHistograms { levels, curves })
    }
}
