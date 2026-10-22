//! The automatic keys' reader in the app: a graph of its own on the app's device, made on the first
//! press, so a reading never waits on the canvas's engine.

use crate::render;
use oni_core::balance::{Measure, Outcome};
use oni_measure::key::{Decoded, Measurer};
use oni_settings::PipelineSettings;
use oni_pipeline::flat_field::Mask;
use oni_pipeline::resident::Files;
use oni_settings::id::Id;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

/// The support folder the custom masks are kept in and the caches the dust patches are kept in,
/// as the canvas's engine reads them; set as the window is made.
static FOLDERS: RwLock<(Option<PathBuf>, Option<PathBuf>)> = RwLock::new((None, None));

pub fn set_folders(support: &Path, caches: Option<PathBuf>) {
    if let Ok(mut f) = FOLDERS.write() {
        *f = (Some(support.to_path_buf()), caches);
    }
}

/// The images a measure or a roll preview lays, read where the canvas reads them.
pub struct Folders;

impl Files for Folders {
    fn mask(&self, id: &str) -> Option<Mask> {
        let support = FOLDERS.read().ok()?.0.clone()?;
        crate::flat_fields::read(&support, id)
    }

    fn patch(&self, id: Id) -> Option<(u32, u32, Vec<[f32; 4]>)> {
        let caches = FOLDERS.read().ok()?.1.clone()?;
        crate::patches::read(&caches, id)
    }
}

pub fn folders() -> Arc<dyn Files> {
    Arc::new(Folders)
}

/// The reader, or `None` where no GPU adapter answers; it decodes each file as the canvas does.
pub fn measurer() -> Option<&'static Measurer> {
    static MEASURER: OnceLock<Option<Measurer>> = OnceLock::new();
    MEASURER
        .get_or_init(|| {
            let graph = render::graph()?;
            Some(Measurer::new(
                graph,
                Box::new(|path: &Path, s: &PipelineSettings| {
                    let d = render::decode_for(path, render::Reading::of(s), render::PREVIEW_SIDE)?;
                    Ok(Decoded { width: d.linear.width as u32, height: d.linear.height as u32, pixels: d.linear.pixels, full_width: d.full_width })
                }),
                Box::new(|s: &PipelineSettings| format!("{:?}", render::Reading::of(s))),
            )
            .laying(folders()))
        })
        .as_ref()
}

/// Every frame's Auto White Balance outcome on the settings it will be balanced from; a frame that
/// reads nothing is absent, and the store's run stops on it.
pub fn balance_outcomes(frames: &[(String, PipelineSettings)]) -> HashMap<String, Outcome> {
    let Some(m) = measurer() else { return HashMap::new() };
    frames.iter().filter_map(|(path, s)| m.balance(path, s).map(|o| (path.clone(), o))).collect()
}

/// The open frame's measuring copy made off the interface thread once its preview stands, so Auto
/// White Balance reads at once; the latest frame asked wins a walk.
pub fn prepare(path: &str, s: &PipelineSettings) {
    let (path, s) = (path.to_string(), s.clone());
    let ticket = crate::decode_gate::MEASURE.ask();
    std::thread::spawn(move || {
        crate::decode_gate::in_background();
        crate::decode_gate::MEASURE.run(ticket, || measurer().map(|m| m.prepare(&path, &s)));
    });
}
