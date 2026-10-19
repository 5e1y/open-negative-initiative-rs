//! Every kernel compiled before the hand needs it: Metal builds a pipeline at its first dispatch,
//! half a second for Texture's, which a first notch would otherwise wait on.

use super::*;
use oni_settings::{ConversionMode, Face, ScannerMachine};

impl Engine {
    /// Renders a small frame through each face, film and finishing branch the editor can reach.
    pub fn prewarm(&mut self) {
        let side = 64;
        let pixels = (0..side * side).map(|i| [0.2 + (i % 13) as f32 * 0.05, 0.4, 0.6, 1.0]).collect();
        let decoded = Decoded { linear: Linear { width: side, height: side, pixels }, full_width: side as f64, undeclared: None };
        let Ok(negative) = self.prepare(decoded) else { return };
        let mut busy = PipelineSettings::rest(ConversionMode::Negative, true);
        busy.capture_stops = 0.5;
        busy.geometry.angle = 1.0;
        busy.chroma.force = 0.5;
        busy.sharpen.amount = 1.0;
        busy.texture.amount = 0.5;
        busy.vignette.stops = 0.5;
        busy.secret_sauce = 0.5;
        busy.spectrogram.hue[0] = 0.1;
        busy.manual_curves.contrast = 0.2;
        let mut variants = vec![busy.clone()];
        variants.push(PipelineSettings { texture: Default::default(), ..busy.clone() });
        variants.push(PipelineSettings { mode: ConversionMode::Monochrome, ..busy.clone() });
        variants.push(PipelineSettings { face: Face::Scanner, ..busy.clone() });
        variants.push(PipelineSettings { face: Face::Scanner, scanner_machine: ScannerMachine::Frontier, ..busy.clone() });
        for s in &variants {
            let _ = self.resident(&negative, s, None);
            let _ = self.inputs(&negative, s, false);
        }
        // What it left is keyed by its own small preview, so the open frame's copies stand.
        self.graph.purge();
    }
}
