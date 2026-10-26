//! The custom masks and dust patches the renders read from their files, kept on the GPU under a budget.

use super::*;

/// What the masks and patches read from their files hold on the GPU past the renders naming them,
/// about three masks at the preview's side; one let go is read again from its file.
pub(crate) const INPUTS_KEPT: u64 = 128 << 20;

/// A mask or a patch on the GPU, its bytes there and the read that last named it.
pub(crate) struct Kept<T> {
    value: T,
    bytes: u64,
    used: u64,
}

impl<T> Kept<T> {
    pub(super) fn new(value: T, laid: &Source, used: u64) -> Kept<T> {
        Kept { value, bytes: laid.extent().area() * 16, used }
    }
}

impl Engine {
    /// Each correction's patch from the cache, read and uploaded once; one not generated yet is left
    /// out, and asked for again on the next render.
    pub(super) fn read_patches(&mut self, settings: &PipelineSettings) {
        let Some(caches) = self.caches.clone() else { return };
        for c in &settings.corrections {
            if let Some(kept) = self.patches.get_mut(&c.id) {
                kept.used = self.reads;
                continue;
            }
            let read = crate::patches::read(&caches, c.id).and_then(|(w, h, pixels)| self.graph.upload(w, h, &pixels).ok());
            if let Some(patch) = read {
                self.patches.insert(c.id, Kept::new(patch.clone(), &patch, self.reads));
            }
        }
    }

    /// The custom mask and the patches the settings name, each read and uploaded once while kept;
    /// past `INPUTS_KEPT` the longest unused are let go, never one these settings name.
    pub(super) fn read_custom(&mut self, settings: &PipelineSettings) {
        self.reads += 1;
        self.read_patches(settings);
        self.read_mask(settings);
        self.let_go();
    }

    fn read_mask(&mut self, settings: &PipelineSettings) {
        let f = &settings.flat_field;
        let (Some(id), Some(support)) = (f.custom.as_ref().filter(|_| f.mask == FlatFieldMask::Custom), self.support.as_ref()) else { return };
        // A miss is asked again, as Swift keeps only hits: a mask loaded later reaches the frames naming it.
        if let Some(kept) = self.custom.get_mut(id) {
            kept.used = self.reads;
            return;
        }
        let read = crate::flat_fields::read(support, id).and_then(|m| {
            let mask = self.graph.upload(m.width, m.height, &m.pixels).ok()?;
            Some(CustomFlatField { id: id.clone(), mask, peaks: m.peaks, per_channel: m.per_channel })
        });
        if let Some(mask) = read {
            let laid = mask.mask.clone();
            self.custom.insert(id.clone(), Kept::new(mask, &laid, self.reads));
        }
    }

    fn let_go(&mut self) {
        let now = self.reads;
        while self.inputs_kept() > INPUTS_KEPT {
            let mask = self.custom.iter().filter(|(_, k)| k.used != now).min_by_key(|(_, k)| k.used).map(|(id, k)| (k.used, id.clone()));
            let patch = self.patches.iter().filter(|(_, k)| k.used != now).min_by_key(|(_, k)| k.used).map(|(id, k)| (k.used, *id));
            match (mask, patch) {
                (Some((m, id)), p) if p.is_none_or(|(p, _)| m <= p) => drop(self.custom.remove(&id)),
                (_, Some((_, id))) => drop(self.patches.remove(&id)),
                _ => break,
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn keeps_patch(&self, id: Id) -> bool {
        self.patches.contains_key(&id)
    }

    /// The masks and patches held on the GPU, in bytes.
    pub(crate) fn inputs_kept(&self) -> u64 {
        self.custom.values().map(|k| k.bytes).sum::<u64>() + self.patches.values().map(|k| k.bytes).sum::<u64>()
    }
}

/// The patches of the settings' own corrections that have been generated.
pub(crate) fn patches_for<'a>(kept: &'a HashMap<Id, Kept<Source>>, settings: &PipelineSettings) -> HashMap<Id, &'a Source> {
    settings.corrections.iter().filter_map(|c| Some((c.id, &kept.get(&c.id)?.value))).collect()
}

/// The custom mask read for the settings' own id, which the pipeline lays only when the ids match.
pub(crate) fn custom_for<'a>(kept: &'a HashMap<String, Kept<CustomFlatField>>, settings: &PipelineSettings) -> Option<&'a CustomFlatField> {
    kept.get(settings.flat_field.custom.as_deref()?).map(|k| &k.value)
}
