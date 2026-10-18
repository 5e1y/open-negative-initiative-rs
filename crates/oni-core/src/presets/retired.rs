//! The presets earlier versions laid down, taken off the shelf when still untouched.

use super::*;

/// The presets the Swift app once laid on every machine, kept only to be recognised.
pub const RETIRED: [&[u8]; 5] = [
    include_bytes!("../../retired/Azure-3676f1b6.json"),
    include_bytes!("../../retired/Cinestill-800T-16aa3e97.json"),
    include_bytes!("../../retired/Gold-200-71bad116.json"),
    include_bytes!("../../retired/Portra-400-795c6944.json"),
    include_bytes!("../../retired/Redscale-b04c5354.json"),
];

/// The preference recording that shipped presets were taken back, so it runs once.
pub const RETIRED_KEY: &str = "retiredShippedPresets";

pub fn retired() -> Vec<Preset> {
    RETIRED.iter().filter_map(|bytes| decode(bytes)).collect()
}

/// Sends to the trash every held preset an earlier version laid down that still carries its name
/// and its look; renamed or retuned, a preset belongs to whoever has it. `done` is the record.
pub fn retire_shipped(
    shipped: &[Preset],
    directory: &Path,
    done: bool,
    discard: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Vec<Preset> {
    if done {
        return Vec::new();
    }
    let untouched: Vec<Preset> = load(directory).into_iter().filter(|held| shipped.iter().any(|s| s.same_look(held))).collect();
    for preset in &untouched {
        delete(preset, directory, discard);
    }
    untouched
}
