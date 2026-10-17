//! The frames standing on one roll's reference, read off one key of each sidecar: a gesture on
//! Roll Settings writes every frame naming the same `RollReference` id. The folder is not the roll.

use oni_settings::{Id, sidecar};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

/// The reference a frame's sidecar names, `None` for a frame never measured or unread.
pub fn reference(path: &str) -> Option<Id> {
    let data = std::fs::read(sidecar::url_for(Path::new(path))).ok()?;
    let document: Value = serde_json::from_slice(&data).ok()?;
    let settings = document.get("settings")?;
    if settings.is_null() {
        return None;
    }
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

/// The frames among `paths` whose sidecar names reference `id`.
pub fn measured(id: Id, paths: &[String]) -> HashSet<String> {
    paths.iter().filter(|p| reference(p) == Some(id)).cloned().collect()
}
