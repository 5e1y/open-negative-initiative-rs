//! `refreshLevelsInput` and `refreshManualCurvesInput`: the distributions in front of the levels and
//! of the manual curves, read again only when what enters that stage moves. Each lays the custom
//! mask and dust patches the canvas lays, as `Pipeline.reading` does.

use super::*;
use oni_measure::histogram::Histogram;
use oni_measure::reader::{Copy, Laying, Reader};
use oni_measure::truncate::{Stage, truncated};

/// One stage's last reading: the settings entering the stage, whose windows it is read through, and
/// the recipe read, which names the copy, the mask and every patch laid.
pub(super) struct Read {
    entering: PipelineSettings,
    recipe: u64,
    histogram: Histogram,
}

impl Engine {
    /// Both distributions through the measuring windows, the curves' on the face that renders, on the small
    /// copy while `held`; a stage whose entering settings and recipe hold keeps its reading.
    pub fn inputs(&mut self, negative: &Negative, settings: &PipelineSettings, held: bool) -> Option<InputHistograms> {
        let id = negative.preview.id();
        self.read_custom(settings);
        if self.measures.as_ref().is_none_or(|(kept, _, _)| *kept != id) {
            let mut r = Reader { graph: &mut self.graph, pipeline: &mut self.pipeline, laying: Laying::Nothing };
            let measure = r.measuring(&negative.preview)?;
            // `measureSmall`: a third of the measuring copy's side, as `CILanczosScaleTransform` makes it.
            let small = self.graph.materialise(&Image::new(&measure).lanczos_anchored(MEASURE_SMALL)).ok()?;
            self.measures = Some((id, measure, small));
        }
        let (_, measure, small) = self.measures.as_ref()?;
        let source = if held { small.clone() } else { measure.clone() };
        let mut read = |slot: usize, stage: Stage| -> Histogram {
            let entering = truncated(settings, stage);
            let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
            let copy = Copy { source: &source, full_width: Some(negative.full_width) };
            let mut reader = Reader { graph: &mut self.graph, pipeline: &mut self.pipeline, laying: Laying::Held(inputs) };
            let recipe = reader.image(copy, settings, stage).map(|image| image.key());
            if let Some(r) = &self.read[slot]
                && r.entering == entering
                && Some(r.recipe) == recipe
            {
                return r.histogram.clone();
            }
            let histogram = reader.reading(copy, settings, stage).histogram;
            self.read[slot] = recipe.map(|recipe| Read { entering, recipe, histogram: histogram.clone() });
            histogram
        };
        let levels = read(0, Stage::Levels);
        let curves = read(1, Stage::ManualCurves);
        Some(InputHistograms { levels, curves })
    }
}
