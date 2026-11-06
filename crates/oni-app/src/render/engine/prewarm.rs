//! Every kernel compiled before the hand needs it: Metal builds a pipeline at its first dispatch,
//! half a second for Texture's, which a first notch would otherwise wait on.

use super::*;
use oni_settings::{ConversionMode, Face, ScannerMachine};

/// An edit with every finishing kernel on, the set a busy notch takes.
pub(crate) fn busy() -> PipelineSettings {
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
    busy
}

/// The frame sizes prewarmed: a small one, then strips long enough for Texture, which rests under a
/// 750 px side, at both its blurs' reaches.
const SIZES: [(usize, usize); 3] = [(64, 64), (768, 8), (6048, 8)];

/// The face, film and finishing branches the editor can reach, each a set of kernels compiled once.
fn variants() -> Vec<PipelineSettings> {
    let busy = busy();
    let mut variants = vec![busy.clone()];
    variants.push(PipelineSettings { texture: Default::default(), ..busy.clone() });
    variants.push(PipelineSettings { mode: ConversionMode::Monochrome, ..busy.clone() });
    variants.push(PipelineSettings { face: Face::Scanner, ..busy.clone() });
    variants.push(PipelineSettings { face: Face::Scanner, scanner_machine: ScannerMachine::Frontier, ..busy.clone() });
    // The finishing's fused passes are one shader per set of kernels on: the usual sets too.
    let mut plain = busy.clone();
    plain.vignette.stops = 0.0;
    variants.push(plain.clone());
    plain.spectrogram = Default::default();
    variants.push(plain.clone());
    plain.secret_sauce = 0.0;
    variants.push(plain);
    variants.push(PipelineSettings::rest(ConversionMode::Negative, true));
    variants
}

/// How far the kernels' compilation has come, which the starter shows and waits on.
#[derive(Default)]
pub struct Kernels {
    done: std::sync::atomic::AtomicUsize,
    finished: std::sync::atomic::AtomicBool,
}

impl Kernels {
    /// The variants compiled in all, one step of the count each.
    pub fn total() -> usize {
        SIZES.len() * variants().len()
    }

    pub fn done(&self) -> usize {
        self.done.load(std::sync::atomic::Ordering::Acquire).min(Self::total())
    }

    pub fn finished(&self) -> bool {
        self.finished.load(std::sync::atomic::Ordering::Acquire)
    }
}

/// Marks the compilation finished however it ends, a panic included: the launch never waits on a
/// thread that is gone.
struct Finishing<'a>(&'a Kernels);

impl Drop for Finishing<'_> {
    fn drop(&mut self) {
        self.0.finished.store(true, std::sync::atomic::Ordering::Release);
    }
}

impl Shared {
    /// `Engine::prewarm`, the engine taken for one variant at a time: the window and the first render
    /// wait one variant at most, never the whole compilation.
    pub fn prewarm(&self) {
        self.prewarm_counted(&Kernels::default());
    }

    /// `prewarm`, each variant counted in `kernels` as it lands.
    pub fn prewarm_counted(&self, kernels: &Kernels) {
        let _finishing = Finishing(kernels);
        let variants = variants();
        for (width, height) in SIZES {
            let Some(negative) = self.lock().ok().and_then(|mut e| e.prewarm_frame(width, height)) else {
                kernels.done.fetch_add(variants.len(), std::sync::atomic::Ordering::AcqRel);
                continue;
            };
            for s in &variants {
                self.give_way();
                if let Ok(mut e) = self.lock() {
                    e.prewarm_variant(&negative, s);
                }
                kernels.done.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
        }
        if let Ok(mut e) = self.lock() {
            e.graph.purge();
        }
    }
}

#[cfg(test)]
mod kernels_tests {
    use super::*;

    #[test]
    fn the_kernels_read_finished_however_their_thread_ends() {
        let kernels = std::sync::Arc::new(Kernels::default());
        let held = kernels.clone();
        let ended = std::thread::spawn(move || {
            let _finishing = Finishing(&held);
            held.done.fetch_add(Kernels::total() + 5, std::sync::atomic::Ordering::AcqRel);
            panic!("a variant that fails");
        })
        .join();
        assert!(ended.is_err() && kernels.finished());
        assert_eq!(kernels.done(), Kernels::total(), "the count never reads past the whole");
        // Twin: a compilation under way is not finished.
        assert!(!Kernels::default().finished());
    }
}

impl Engine {
    /// Renders a small frame through each face, film and finishing branch the editor can reach, then
    /// strips long enough for Texture at both its blurs' reaches.
    pub fn prewarm(&mut self) {
        for (width, height) in SIZES {
            self.prewarm_at(width, height);
        }
        // What it left is keyed by its own small previews, so the open frame's copies stand.
        self.graph.purge();
    }

    /// The pipelines compiled so far, which a prewarmed engine's first notch adds none to.
    pub fn compiled(&self) -> usize {
        self.graph.stats().pipelines
    }

    pub(crate) fn prewarm_at(&mut self, width: usize, height: usize) {
        let Some(negative) = self.prewarm_frame(width, height) else { return };
        for s in &variants() {
            self.prewarm_variant(&negative, s);
        }
    }

    fn prewarm_frame(&mut self, width: usize, height: usize) -> Option<Negative> {
        let pixels = (0..width * height).map(|i| [0.2 + (i % 13) as f32 * 0.05, 0.4, 0.6, 1.0]).collect();
        let decoded = Decoded { linear: Linear { width, height, pixels }, full_width: width as f64, undeclared: None };
        self.prepare(decoded).ok()
    }

    fn prewarm_variant(&mut self, negative: &Negative, s: &PipelineSettings) {
        let _ = self.resident(negative, s, None);
        let _ = self.inputs(negative, s, false);
    }
}
