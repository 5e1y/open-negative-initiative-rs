//! `ensureGenerated`: each stroke without a patch given one on its frame's full resolution, by the
//! editor's run and by every render written to a file alike, since a render lays only patches on disk.

use crate::render::Shared;
use oni_graph::Source;
use oni_pipeline::corrections::PatchKey;
use oni_settings::PipelineSettings;
use oni_settings::frame::Correction;
use std::collections::HashSet;
use std::path::Path;

/// A patch written: its key, whether its search fell back, and the offset it was copied from.
pub type Made = (PatchKey, bool, (f64, f64));

/// A patch written, or the key of one that could not be and why.
pub type Tried = Result<Made, (PatchKey, String)>;

/// The corrections of `s` holding a stroke whose patch is neither in `caches` nor in `skip`.
pub fn pending(caches: &Path, s: &PipelineSettings, skip: &HashSet<PatchKey>) -> Vec<usize> {
    let upstream = oni_pipeline::corrections::upstream(s);
    let key = |c: &Correction| PatchKey { id: c.id, upstream };
    (0..s.corrections.len()).filter(|&i| !s.corrections[i].stroke.is_empty() && !skip.contains(&key(&s.corrections[i])) && !crate::patches::file(caches, key(&s.corrections[i])).exists()).collect()
}

/// Each pending correction of the settings `latest` gives, read again before each patch, made on `full`
/// in the order `next` picks, until none is left or `latest` gives none; one failed joins `skip`, tried once.
pub fn ensure(engine: &Shared, full: &Source, caches: &Path, skip: &mut HashSet<PatchKey>, mut latest: impl FnMut() -> Option<PipelineSettings>, mut each: impl FnMut(Tried)) {
    let scan = full.extent();
    while let Some(settings) = latest() {
        let cs = &settings.corrections;
        let Some(k) = super::next(cs, &pending(caches, &settings, skip), f64::from(scan.w), f64::from(scan.h)) else { break };
        let made = generated(engine, full, &settings, &cs[k], caches).map_err(|e| (PatchKey::of(&settings, &cs[k]), e));
        if let Err((key, _)) = &made {
            skip.insert(*key);
        }
        each(made);
    }
}

/// A correction's patch searched on the scan healed of the other patches made, written to the cache
/// under the settings it was cut on, and the textures its search laid freed, as a run of large strokes would otherwise pile them up.
fn generated(engine: &Shared, full: &Source, settings: &PipelineSettings, c: &Correction, caches: &Path) -> Result<Made, String> {
    let (region, size) = engine.lock()?.search_region(full, settings, c)?;
    let patch = super::generate(&region, c, size.0, size.1);
    drop(region);
    let key = PatchKey::of(settings, c);
    crate::patches::write(caches, key, patch.width as u32, patch.height as u32, &patch.pixels)?;
    let mut e = engine.lock()?;
    e.forget_patch(key);
    e.purge();
    Ok((key, patch.fallback, patch.offset))
}
