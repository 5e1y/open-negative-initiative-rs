//! The open frame's settings, the one place a gesture writes them. A notch is one write and
//! reports the fields it moved; the sidecar stays the truth and is written through `oni-settings`.

use crate::balance::{self, Measure, Strategy};
use crate::command::{Command, Scan};
use crate::fields::{Family, FieldSet};
use crate::history::{Companion, History};
use crate::parameters;
use crate::presets::Preset;
use oni_library::gallery_history::{self, GalleryHistory, Marker, Step, Trash};
use oni_library::{refusal, roll_membership, trash};
use oni_i18n::{tr, trn};
use oni_keys::{Channel, Effect, Letter, Machine};
use oni_settings::codec;
use oni_settings::frame::Geometry;
use oni_settings::settings::{look_settings, roll_settings};
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Date, Face, Id, PipelineSettings, ScannerMachine};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::time::Instant;

mod opening;
mod commands;
mod undo;
mod paste;
mod several;
mod oni_trash;
mod scanner;
mod together;

pub use opening::Unsaved;
pub use oni_library::roll_folder::{Walk, Walked};
pub use oni_trash::mark_run;
pub use scanner::{channel, machine_for, machine_of};

pub mod roll;

/// Seconds an unheld edit rests before its step is recorded, on the deferred release.
pub const SETTLE_DELAY: f64 = 0.15;

type Settings = BTreeMap<String, PipelineSettings>;

/// The host's view of the library: every frame's path, and which decode through the RAW mosaic,
/// which the host reads off the bytes.
pub struct Archive {
    pub paths: Vec<String>,
    /// Shared, so a mark run off the host's thread reads the same rule.
    pub mosaic: std::sync::Arc<dyn Fn(&Path) -> bool + Send + Sync>,
}

impl Archive {
    pub fn new(paths: Vec<String>, mosaic: impl Fn(&Path) -> bool + Send + Sync + 'static) -> Archive {
        Archive { paths, mosaic: std::sync::Arc::new(mosaic) }
    }
}

/// Which undo stack answers: the open frame's in the editor, the gestures on several in the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Editor,
    Gallery,
}

/// What one command did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Applied {
    /// The open frame's fields it moved.
    pub changed: FieldSet,
    /// A walk the host performs, the store holding no list of frames.
    pub walk: Option<i8>,
}

/// The settings pasted between frames, as the codec writes them so a paste reads as a sidecar does.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Board {
    pub settings: Option<String>,
    pub framing: Option<Geometry>,
}

/// Whether a paste lays one roll's look on another, which the hand confirms first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteCheck {
    Nothing,
    Ready,
    AnotherRoll,
}

struct Open {
    path: String,
    writable: bool,
    /// What the disk holds, so a write of the same settings is skipped.
    persisted: Option<PipelineSettings>,
    /// Its original cannot be seen: every hand on it is refused, nothing is written.
    absent: bool,
    /// The original's size and date as it opened, so edits kept for it never land on another file.
    stamp: Option<opening::Stamp>,
}

pub struct EditorStore {
    open: Option<Open>,
    settings: PipelineSettings,
    history: History,
    gallery: GalleryHistory,
    archive: Archive,
    pub board: Board,
    /// Open on the Scanner face: the frame adopts it, its Color tab edits `ScannerFinish`, its reset
    /// keeps Chrome and clears the clicks. Set through `Command::Face`.
    pub scanner_face: bool,
    held: bool,
    /// When a lifted hand's step is recorded; every change before it moves it on.
    deferred: Option<f64>,
    /// A hand edit of a Roll Settings field waiting for its step, to write the roll with it.
    roll_touched: bool,
    /// A look laid on the open frame waiting for its step, to lay it on the rest of the roll.
    roll_look: Option<(PipelineSettings, Id)>,
    /// Frames edited together with the open one, every step laid on them too.
    together: together::Together,
    measured: HashMap<Id, BTreeSet<String>>,
    pending: FieldSet,
    revision: u64,
    last_change: Option<f64>,
    epoch: Instant,
    /// The last failure a gesture met, for the host to show.
    pub failure: Option<String>,
    /// The last undo or redo of a roll on the disk, for the host to run off its thread.
    pub walking: Option<oni_library::roll_folder::Walk>,
    /// The roll marker a gesture, an undo or a redo last wrote, for the host to show the folder's roll.
    pub marked: Option<(String, Option<oni_settings::Film>)>,
    /// Frames left with edits their sidecar refused, kept until a write takes them or the hand drops them.
    unsaved: Vec<opening::Unsaved>,
}

impl EditorStore {
    pub fn new(archive: Archive) -> EditorStore {
        EditorStore {
            open: None,
            settings: PipelineSettings::default(),
            history: History::default(),
            gallery: GalleryHistory::default(),
            archive,
            board: Board::default(),
            scanner_face: false,
            held: false,
            deferred: None,
            roll_touched: false,
            roll_look: None,
            together: together::Together::default(),
            measured: HashMap::new(),
            pending: FieldSet::EMPTY,
            revision: 0,
            last_change: None,
            epoch: Instant::now(),
            failure: None,
            walking: None,
            marked: None,
            unsaved: Vec::new(),
        }
    }

    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    pub fn settings(&self) -> &PipelineSettings {
        &self.settings
    }

    pub fn path(&self) -> Option<&str> {
        self.open.as_ref().map(|o| o.path.as_str())
    }

    pub fn writable(&self) -> bool {
        self.open.as_ref().is_some_and(|o| o.writable)
    }

    /// The open frame's original cannot be seen, so no hand moves its settings.
    pub fn inert(&self) -> bool {
        self.open.as_ref().is_some_and(|o| o.absent)
    }

    /// Whether a gesture may change the open frame: its original in sight and its sidecar one this
    /// version writes, or the change would show and never be saved.
    pub fn takes_hands(&self) -> bool {
        self.writable() && !self.inert()
    }

    /// Bumped by every change, from whatever hand.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self, surface: Surface) -> bool {
        match surface {
            Surface::Editor => self.history.can_undo(),
            Surface::Gallery => self.gallery.can_undo(),
        }
    }

    pub fn can_redo(&self, surface: Surface) -> bool {
        match surface {
            Surface::Editor => self.history.can_redo(),
            Surface::Gallery => self.gallery.can_redo(),
        }
    }

    pub fn gallery_history(&self) -> &GalleryHistory {
        &self.gallery
    }

    /// `measuredGroups = [:]`: every roll's frames found again on the next ask.
    pub fn forget_measured(&mut self) {
        self.measured.clear();
    }

    /// The library's paths: a roll whose frames are all still there stays found, as Swift keeps it;
    /// one that lost a frame, moved or gone, is scanned again on the next ask.
    pub fn set_archive_paths(&mut self, paths: Vec<String>) {
        let held: std::collections::HashSet<&String> = paths.iter().collect();
        self.measured.retain(|_, members| members.iter().all(|p| held.contains(p)));
        self.archive.paths = paths;
    }

    /// The fields moved since the last call: what a view re-reads, and nothing else.
    pub fn take_changes(&mut self) -> FieldSet {
        std::mem::take(&mut self.pending)
    }

    fn mosaic(&self, path: &str) -> bool {
        (self.archive.mosaic)(Path::new(path))
    }

    /// The archive's rule for a mosaic, for a mark run off this thread.
    pub fn mosaic_rule(&self) -> std::sync::Arc<dyn Fn(&Path) -> bool + Send + Sync> {
        self.archive.mosaic.clone()
    }

    pub fn record_disk(&mut self, disk: Disk, label: &str) {
        self.gallery.record_disk(disk, label);
    }
}

/// Re-exported for the host, which records disk gestures on the same gallery stack.
pub use gallery_history::Disk;

#[cfg(test)]
mod tests;
