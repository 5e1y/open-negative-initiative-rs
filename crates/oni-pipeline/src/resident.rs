//! The images a frame's settings name but do not hold, read from their files onto a graph of its
//! own: the custom mask the settings name, kept while asked again, and the dust patches.

use crate::flat_field::Mask;
use crate::{CustomFlatField, Inputs};
use oni_graph::{Graph, Source};
use oni_settings::frame::FlatFieldMask;
use oni_settings::id::Id;
use oni_settings::PipelineSettings;
use std::collections::HashMap;
use std::sync::Arc;

/// Where the named images are read: the support folder's masks and the caches' patches.
pub trait Files: Send + Sync {
    /// A custom mask by its content id; `None` once its file is gone.
    fn mask(&self, id: &str) -> Option<Mask>;
    /// A correction's patch at full resolution: width, height, pixels; `None` while not generated.
    fn patch(&self, id: Id) -> Option<(u32, u32, Vec<[f32; 4]>)>;
}

/// What one graph holds of the named images; without files it holds nothing, so no input is laid.
#[derive(Default)]
pub struct Resident {
    files: Option<Arc<dyn Files>>,
    mask: Option<CustomFlatField>,
    patches: HashMap<Id, Source>,
}

impl Resident {
    pub fn new(files: Arc<dyn Files>) -> Resident {
        Resident { files: Some(files), mask: None, patches: HashMap::new() }
    }

    /// Every upload dropped, the files kept to read them again.
    pub fn emptied(self) -> Resident {
        Resident { files: self.files, mask: None, patches: HashMap::new() }
    }

    /// The patches dropped, so the next read takes them from their files: a correction's patch is
    /// generated again under the same id.
    pub fn fresh(&mut self) {
        self.patches.clear();
    }

    /// The mask and the patches `s` names uploaded, those missing read; a file that does not read is
    /// asked again next time, so a mask loaded later reaches the frames naming it.
    pub fn read(&mut self, graph: &mut Graph, s: &PipelineSettings) {
        let Some(files) = self.files.clone() else { return };
        for c in &s.corrections {
            if self.patches.contains_key(&c.id) {
                continue;
            }
            if let Some((w, h, pixels)) = files.patch(c.id)
                && let Ok(source) = graph.upload(w, h, &pixels)
            {
                self.patches.insert(c.id, source);
            }
        }
        let f = &s.flat_field;
        let Some(id) = f.custom.as_ref().filter(|_| f.mask == FlatFieldMask::Custom) else { return };
        if self.mask.as_ref().is_some_and(|m| &m.id == id) {
            return;
        }
        // One mask at a time: a roll names one, and each weighs 40 MB at the preview's side.
        self.mask = files.mask(id).and_then(|m| {
            let mask = graph.upload(m.width, m.height, &m.pixels).ok()?;
            Some(CustomFlatField { id: id.clone(), mask, peaks: m.peaks, per_channel: m.per_channel })
        });
    }

    /// The inputs `s` names among those read; the pipeline lays a mask only when its id is the frame's.
    pub fn inputs(&self, s: &PipelineSettings) -> Inputs<'_> {
        Inputs {
            patches: s.corrections.iter().filter_map(|c| Some((c.id, self.patches.get(&c.id)?))).collect(),
            custom_flat_field: self.mask.as_ref(),
        }
    }
}
