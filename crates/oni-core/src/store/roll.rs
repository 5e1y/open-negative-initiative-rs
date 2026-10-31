//! The two gestures that make and unmake a roll: a measure landing on every frame Prepare the Roll
//! read, and Reset to Default, its inverse, reaching whole rolls and their contact sheets.

use super::{EditorStore, Settings};
use crate::parameters;
use oni_i18n::trn;
use oni_library::gallery_history::Trash;
use oni_library::roll_membership::ResetReach;
use oni_library::{rgb_passes, trash};
use oni_settings::frame::Geometry;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{Date, PipelineSettings};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// One frame a measure wrote: its path, its settings before, its settings on the roll.
pub type WrittenFrame = (String, PipelineSettings, PipelineSettings);

/// `RGBPasses.joined`: a pass under its lead takes the lead's framing with its name.
pub fn joined(pass: &PipelineSettings, lead: &str, framing: Geometry) -> PipelineSettings {
    PipelineSettings { pass_of: Some(lead.to_string()), geometry: framing, ..pass.clone() }
}

/// `RGBPasses.split`: back to unconverted on the lead's framing, `None` keeping the pass's own.
pub fn split(pass: &PipelineSettings, framing: Option<Geometry>) -> PipelineSettings {
    let mut out = parameters::unconverted(pass);
    if let Some(framing) = framing {
        out.geometry = framing;
    }
    out
}

/// Reset to Default: back to unconverted, a pass split from its lead, every crop gone (owner's call).
fn defaulted(settings: &PipelineSettings, framing: Option<Geometry>) -> PipelineSettings {
    PipelineSettings { geometry: Geometry::default(), ..split(settings, framing) }
}

/// The undo label of a measure, counting frames, never the passes riding with them.
pub fn measure_label(frames: usize) -> String {
    trn("Measure the roll", "Measure the roll of {n} photos", frames as u64)
}

pub fn reset_label(frames: usize) -> String {
    trn("Reset to Default · {n} photo", "Reset to Default · {n} photos", frames as u64)
}

impl EditorStore {
    /// `landRoll`: the open frame in memory, the others written with their draft cleared, a triplet's
    /// passes joined to their lead, every frame read but not written losing its draft; one gallery step.
    pub fn land_roll(&mut self, written: &[WrittenFrame], prepared: &[String]) -> usize {
        self.measured.clear();
        let open = self.path().map(str::to_string);
        let mut before = Settings::new();
        let mut after = Settings::new();
        let handled: Vec<String> = written.iter().map(|(p, _, _)| p.clone()).collect();
        for (path, was, now) in written {
            if Some(path) == open.as_ref() {
                if !self.takes_hands() {
                    continue;
                }
                before.insert(path.clone(), self.settings.clone());
                after.insert(path.clone(), now.clone());
                let at = self.now();
                self.replace(now.clone());
                self.last_change = Some(at);
                self.settle_beside(at, &handled);
                continue;
            }
            let options = WriteOptions { roll_draft: Some(None), ..WriteOptions::default() };
            match sidecar::write(now, Path::new(path), options) {
                Ok(()) => {
                    before.insert(path.clone(), was.clone());
                    after.insert(path.clone(), now.clone());
                }
                Err(e) => self.failure = Some(oni_library::refusal::said(&e)),
            }
        }
        // A frame of passes: its two other files name their lead, in the same step so one undo takes it back.
        for (lead, _, now) in written {
            let Some(passes) = &now.rgb_passes else { continue };
            let name = oni_library::paths::name(lead);
            for pass in rgb_passes::paths_beside(passes, lead) {
                if pass == *lead {
                    continue;
                }
                let Ok(loaded) = sidecar::read(Path::new(&pass), self.mosaic(&pass), true) else { continue };
                if !loaded.writable {
                    continue;
                }
                let next = joined(&loaded.settings, &name, now.geometry);
                if next == loaded.settings || sidecar::write(&next, Path::new(&pass), WriteOptions::default()).is_err() {
                    continue;
                }
                before.insert(pass.clone(), loaded.settings);
                after.insert(pass, next);
            }
        }
        // The open frame and those left out drop the draft here, before the open frame's deferred write reads its file.
        let measured: BTreeSet<&String> = written.iter().map(|(p, _, _)| p).filter(|p| Some(*p) != open.as_ref()).collect();
        for path in prepared.iter().filter(|p| !measured.contains(p)) {
            let _ = sidecar::set_roll_draft(None, Path::new(path), self.mosaic(path));
        }
        let frames = before.keys().filter(|p| after.get(*p).is_some_and(|s| s.pass_of.is_none())).count();
        self.gallery.record(&before, &after, &measure_label(frames), None);
        frames
    }

    /// `resetToDefault`: every frame reached back to unconverted, a triplet split into its three
    /// shots on the crop drawn, the rolls' sheets to the ONI Trash, all as one gallery step.
    pub fn reset_to_default(&mut self, reach: &ResetReach) -> usize {
        let open = self.path().map(str::to_string);
        let frames: Vec<String> = reach.frames.iter().cloned().collect();
        let files: BTreeSet<String> = rgb_passes::expanded(&frames).into_iter().collect();
        let mut before = Settings::new();
        let mut after = Settings::new();
        // The open frame's passes are read before it is blanked.
        let mut framings: HashMap<String, Geometry> = HashMap::new();
        if let Some(current) = open.as_ref().filter(|p| files.contains(*p)) {
            if let Some(passes) = &self.settings.rgb_passes {
                for pass in rgb_passes::paths_beside(passes, current) {
                    if pass != *current {
                        framings.insert(pass, self.settings.geometry);
                    }
                }
            }
            let blank = defaulted(&self.settings, None);
            if blank != self.settings && self.takes_hands() {
                before.insert(current.clone(), self.settings.clone());
                after.insert(current.clone(), blank.clone());
                let at = self.now();
                self.replace(blank);
                self.last_change = Some(at);
                let handled: Vec<String> = files.iter().cloned().collect();
                self.settle_beside(at, &handled);
            }
        }
        let others: Vec<String> = files.iter().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        for (pass, framing) in rgb_passes::framings(&others) {
            framings.entry(pass).or_insert(framing);
        }
        let (was, now) = self.modify(&others, |path, loaded| Some(defaulted(loaded, framings.get(path).copied())));
        before.extend(was);
        after.extend(now);
        for id in &reach.references {
            self.measured.remove(&oni_library::roll_membership::RollKey::Measured(*id));
        }
        let sheets: Vec<String> = reach.sheets.iter().cloned().collect();
        let mut binned = trash::Outcome::default();
        if !sheets.is_empty() {
            let mosaic = &self.archive.mosaic;
            binned = trash::mark(&sheets, Some(Date::now()), &|p: &Path| mosaic(p));
            self.took(&binned, true);
        }
        let marks = Trash { paths: binned.done.iter().cloned().collect(), marked: true };
        self.gallery.record(&before, &after, &reset_label(reach.frames.len()), Some(marks));
        reach.frames.len()
    }
}
