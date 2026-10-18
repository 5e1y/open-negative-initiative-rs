//! The automatic keys' reader in the app: a graph of its own, made on the first press, so a reading
//! never waits on the canvas's engine nor holds it while the window's surface is reconfigured.

use crate::render;
use oni_core::balance::{Measure, Outcome};
use oni_measure::key::{Decoded, Measurer};
use oni_settings::PipelineSettings;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// The reader, or `None` where no GPU adapter answers; it decodes each file as the canvas does.
pub fn measurer() -> Option<&'static Measurer> {
    static MEASURER: OnceLock<Option<Measurer>> = OnceLock::new();
    MEASURER
        .get_or_init(|| {
            let graph = oni_graph::Graph::new().ok()?;
            Some(Measurer::new(
                graph,
                Box::new(|path: &Path, s: &PipelineSettings| {
                    let d = render::decode(path, render::Reading::of(s))?;
                    Ok(Decoded { width: d.linear.width as u32, height: d.linear.height as u32, pixels: d.linear.pixels, full_width: d.full_width })
                }),
            ))
        })
        .as_ref()
}

/// Every frame's Auto White Balance outcome on the settings it will be balanced from; a frame that
/// reads nothing is absent, and the store's run stops on it.
pub fn balance_outcomes(frames: &[(String, PipelineSettings)]) -> HashMap<String, Outcome> {
    let Some(m) = measurer() else { return HashMap::new() };
    frames.iter().filter_map(|(path, s)| m.balance(path, s).map(|o| (path.clone(), o))).collect()
}
