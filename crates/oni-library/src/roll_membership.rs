//! The frames standing on one roll's reference, read off one key of each sidecar: a gesture on
//! Roll Settings writes every frame naming the same `RollReference` id in the folders it starts from.

use crate::paths;
use oni_settings::{ConversionMode, Id, PipelineSettings, sidecar};
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

/// What makes frames one roll: the reference a measure wrote, or for a slide, which a measure never
/// gives one (owner's call), the folder it sits in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RollKey {
    Measured(Id),
    Slides(String),
}

impl From<Id> for RollKey {
    fn from(id: Id) -> RollKey {
        RollKey::Measured(id)
    }
}

impl RollKey {
    /// The roll `settings` stand on at `path`; a slide's contact sheet stands on its folder's slides,
    /// as a negative's does on its reference, while a slide's pass stands on none.
    pub fn of(path: &str, settings: &PipelineSettings) -> Option<RollKey> {
        match &settings.roll_reference {
            Some(r) => Some(RollKey::Measured(r.id)),
            None => slide(settings.mode, settings.pass_of.is_some(), path),
        }
    }
}

fn slide(mode: ConversionMode, pass: bool, path: &str) -> Option<RollKey> {
    (mode == ConversionMode::Positive && !pass).then(|| RollKey::Slides(paths::parent(path)))
}

/// `RollKey::of` read off the sidecar's own keys, the rest of it never decoded.
fn key_in(path: &str, settings: &Value) -> Option<RollKey> {
    if let Some(id) = reference_in(settings) {
        return Some(RollKey::Measured(id));
    }
    let mode = settings.get("mode").and_then(|m| serde_json::from_value::<ConversionMode>(m.clone()).ok())?;
    let pass = settings.get("passOf").is_some_and(|p| !p.is_null());
    slide(mode, pass, path)
}

/// The frames among `paths` standing on roll `key`: a slide roll is its folder's slides alone.
pub fn members(key: &RollKey, paths: &[String]) -> HashSet<String> {
    match key {
        RollKey::Measured(id) => measured(*id, paths),
        RollKey::Slides(folder) => paths
            .iter()
            .filter(|p| paths::parent(p) == *folder && settings(p).and_then(|s| key_in(p, &s)).as_ref() == Some(key))
            .cloned()
            .collect(),
    }
}

fn settings(path: &str) -> Option<Value> {
    let data = std::fs::read(sidecar::url_for(Path::new(path))).ok()?;
    let mut document: Value = serde_json::from_slice(&data).ok()?;
    let settings = document.get_mut("settings")?.take();
    (!settings.is_null()).then_some(settings)
}

fn reference_in(settings: &Value) -> Option<Id> {
    let reference = settings.as_object()?.get("rollReference")?;
    if reference.is_null() {
        return None;
    }
    let id = reference.as_object()?.get("id")?;
    if id.is_null() {
        return None;
    }
    Id::parse(id.as_str()?)
}

/// The reference a frame's sidecar names, `None` for a frame never measured or unread.
pub fn reference(path: &str) -> Option<Id> {
    reference_in(&settings(path)?)
}

fn has_sheet(settings: &Value) -> bool {
    settings.get("sheet").is_some_and(|s| !s.is_null())
}

/// Whether `path` is a contact sheet, read off its sidecar's grid alone.
pub fn is_sheet(path: &str) -> bool {
    settings(path).is_some_and(|s| has_sheet(&s))
}

/// Whether `path` is the contact sheet of its folder's slide roll, which no reference names.
pub fn is_slide_sheet(path: &str) -> bool {
    settings(path).is_some_and(|s| has_sheet(&s) && matches!(key_in(path, &s), Some(RollKey::Slides(_))))
}

/// The reference a contact sheet's sidecar names, `None` for a photograph or a sheet of none.
pub fn sheet_reference(path: &str) -> Option<Id> {
    let settings = settings(path)?;
    if !has_sheet(&settings) {
        return None;
    }
    reference_in(&settings)
}

/// What a Reset to Default reaches: frames, contact sheets and the rolls they stand on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResetReach {
    pub frames: BTreeSet<String>,
    pub sheets: BTreeSet<String>,
    pub references: BTreeSet<Id>,
}

/// The paths among `paths` in a folder holding one of `anchors`: a copy of a roll beside it names the
/// same reference, and a gesture on one roll never writes the other's sidecars.
pub fn beside(anchors: &[String], paths: &[String]) -> Vec<String> {
    let folders: HashSet<String> = anchors.iter().map(|a| paths::parent(a)).collect();
    paths.iter().filter(|p| folders.contains(&paths::parent(p))).cloned().collect()
}

/// `RollSettings.resetReach`: the rolls `picked` stands on, every frame naming one of them beside
/// it, and their contact sheets; a sheet picked brings its roll, never itself as a frame.
pub fn reset_reach(picked: &[String], library: &[String]) -> ResetReach {
    let mut reach = ResetReach::default();
    for path in picked {
        if let Some(id) = sheet_reference(path).or_else(|| reference(path)) {
            reach.references.insert(id);
        }
        if !is_sheet(path) {
            reach.frames.insert(path.clone());
        }
    }
    if reach.references.is_empty() {
        return reach;
    }
    for path in &beside(picked, library) {
        if let Some(id) = sheet_reference(path) {
            if reach.references.contains(&id) {
                reach.sheets.insert(path.clone());
            }
        } else if let Some(id) = reference(path)
            && reach.references.contains(&id)
        {
            reach.frames.insert(path.clone());
        }
    }
    reach
}

/// The frames among `paths` whose sidecar names reference `id`.
pub fn measured(id: Id, paths: &[String]) -> HashSet<String> {
    paths.iter().filter(|p| reference(p) == Some(id)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use oni_settings::PipelineSettings;
    use oni_settings::frame::RollReference;
    use oni_settings::sidecar::WriteOptions;

    /// "Roll 8" and its Finder copy name one reference: a reset or a Roll Settings gesture on one
    /// reaches its own folder's frames, never the copy's.
    #[test]
    fn a_copy_of_a_roll_beside_it_is_out_of_reach() {
        let scratch = Scratch::new("roll-membership-copy");
        let root = paths::normal(&scratch.path().to_string_lossy()).into_owned();
        let reference = RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None };
        let mut library = Vec::new();
        for folder in ["Roll 8", "Roll 8 copy"] {
            for name in ["a.ARW", "b.ARW"] {
                let path = paths::join(&paths::join(&root, folder), name);
                std::fs::create_dir_all(paths::parent(&path)).unwrap();
                std::fs::write(&path, b"").unwrap();
                let settings = PipelineSettings { roll_reference: Some(reference.clone()), ..PipelineSettings::default() };
                sidecar::write(&settings, Path::new(&path), WriteOptions::default()).unwrap();
                library.push(path);
            }
        }
        let reach = reset_reach(&library[..1], &library);
        assert_eq!(reach.frames, library[..2].iter().cloned().collect::<BTreeSet<_>>());
        assert_eq!(measured(reference.id, &beside(&library[2..3], &library)), library[2..].iter().cloned().collect());
        // Twin: a pick in both folders reaches both, as the reference alone would.
        let both = reset_reach(&[library[0].clone(), library[3].clone()], &library);
        assert_eq!(both.frames.len(), 4);
    }

    /// A slide roll, which no measure gives a reference, is its folder's slides and their sheet: a
    /// negative never measured and the slides of the folder beside stay out.
    #[test]
    fn a_slide_roll_is_its_folder_s_slides() {
        let scratch = Scratch::new("roll-membership-slides");
        let root = paths::normal(&scratch.path().to_string_lossy()).into_owned();
        let slide = PipelineSettings { mode: ConversionMode::Positive, ..PipelineSettings::default() };
        let grid = oni_settings::frame::ContactSheetGrid { columns: 1, rows: 1, frames: 1, tile: Default::default(), margin: 0.0, windows: None };
        let sheet = PipelineSettings { sheet: Some(grid), ..slide.clone() };
        let laid = [("Slides", "a.ARW", &slide), ("Slides", "b.ARW", &slide), ("Slides", "c.ARW", &PipelineSettings::default()), ("Slides", "sheet.tif", &sheet), ("Slides 2", "d.ARW", &slide)];
        let mut library = Vec::new();
        for (folder, name, settings) in laid {
            let path = paths::join(&paths::join(&root, folder), name);
            std::fs::create_dir_all(paths::parent(&path)).unwrap();
            std::fs::write(&path, b"").unwrap();
            sidecar::write(settings, Path::new(&path), WriteOptions::default()).unwrap();
            library.push(path);
        }
        let key = RollKey::of(&library[0], &slide).expect("a slide stands on its folder");
        assert_eq!(members(&key, &library), [0, 1, 3].map(|i| library[i].clone()).into_iter().collect());
        assert!(is_slide_sheet(&library[3]) && !is_slide_sheet(&library[0]));
        // Twin: the folder beside is a roll of its own, and a negative never measured stands on none.
        assert_eq!(members(&RollKey::of(&library[4], &slide).unwrap(), &library), library[4..].iter().cloned().collect());
        assert_eq!(RollKey::of(&library[2], &PipelineSettings::default()), None);
        assert_eq!(RollKey::of(&library[3], &sheet), Some(key));
        let pass = PipelineSettings { pass_of: Some(library[0].clone()), ..slide.clone() };
        assert_eq!(RollKey::of(&library[0], &pass), None, "a slide's pass stands on none");
    }
}
