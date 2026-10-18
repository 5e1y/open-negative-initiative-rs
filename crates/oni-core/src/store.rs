//! The open frame's settings, the one place a gesture writes them. A notch is one write and
//! reports the fields it moved; the sidecar stays the truth and is written through `oni-settings`.

use crate::balance::{self, Measure, Strategy};
use crate::command::{Command, Scan};
use crate::fields::{Family, Field, FieldSet};
use crate::history::{Companion, History};
use crate::parameters;
use crate::presets::Preset;
use oni_library::gallery_history::{self, GalleryHistory, Step, Trash};
use oni_library::{roll_folder, roll_membership, trash};
use oni_link::scanner::{Channel, Effect, Letter, Machine};
use oni_settings::codec;
use oni_settings::frame::Geometry;
use oni_settings::settings::{look_settings, roll_settings};
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Date, Face, Id, PipelineSettings, ScannerMachine};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::time::Instant;

pub mod roll;

/// Seconds an unheld edit rests before its step is recorded, on the deferred release.
pub const SETTLE_DELAY: f64 = 0.15;

type Settings = BTreeMap<String, PipelineSettings>;

/// The host's view of the library: every frame's path, and which decode through the RAW mosaic,
/// which the host reads off the bytes.
pub struct Archive {
    pub paths: Vec<String>,
    pub mosaic: Box<dyn Fn(&Path) -> bool + Send + Sync>,
}

impl Archive {
    pub fn new(paths: Vec<String>, mosaic: impl Fn(&Path) -> bool + Send + Sync + 'static) -> Archive {
        Archive { paths, mosaic: Box::new(mosaic) }
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
    measured: HashMap<Id, BTreeSet<String>>,
    pending: FieldSet,
    revision: u64,
    last_change: Option<f64>,
    epoch: Instant,
    /// The last failure a gesture met, for the host to show.
    pub failure: Option<String>,
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
            measured: HashMap::new(),
            pending: FieldSet::EMPTY,
            revision: 0,
            last_change: None,
            epoch: Instant::now(),
            failure: None,
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

    pub fn archive_paths(&self) -> &[String] {
        &self.archive.paths
    }

    /// `measuredGroups = [:]`: every roll's frames found again on the next ask.
    pub fn forget_measured(&mut self) {
        self.measured.clear();
    }

    pub fn set_archive_paths(&mut self, paths: Vec<String>) {
        self.archive.paths = paths;
        self.measured.clear();
    }

    /// The fields moved since the last call: what a view re-reads, and nothing else.
    pub fn take_changes(&mut self) -> FieldSet {
        std::mem::take(&mut self.pending)
    }

    fn mosaic(&self, path: &str) -> bool {
        (self.archive.mosaic)(Path::new(path))
    }

    // MARK: - Opening and writing

    /// Opens a frame on its sidecar, writing the previous one first; its history starts here, so
    /// an undo never climbs into another photo's.
    pub fn open(&mut self, path: &str) -> Result<(), sidecar::Error> {
        self.release();
        self.flush();
        let read = sidecar::read(Path::new(path), self.mosaic(path), true);
        let (settings, writable, persisted, result) = match read {
            Ok(loaded) => {
                let persisted = (!loaded.is_new).then(|| loaded.settings.clone());
                (loaded.settings, loaded.writable, persisted, Ok(()))
            }
            // A corrupt sidecar is reported and never overwritten, so it stays repairable by hand.
            Err(e) => (PipelineSettings::rest(ConversionMode::Negative, self.mosaic(path)), false, None, Err(e)),
        };
        self.open = Some(Open { path: path.to_string(), writable, persisted });
        // `adopt` starts the history on the stored settings, then `adoptFace` changes the face, which
        // the deferred `record` keeps as the first undo step (`EditorScene+Actions.swift:91-95`).
        self.history = History::new(settings.clone());
        let adopted = PipelineSettings { face: self.face(), ..settings };
        self.history.record(adopted.clone(), f64::NEG_INFINITY);
        self.replace(adopted);
        self.pending = FieldSet::all();
        self.roll_touched = false;
        self.roll_look = None;
        result
    }

    pub fn close(&mut self) {
        self.release();
        self.flush();
        self.open = None;
        self.history = History::default();
        self.replace(PipelineSettings::default());
        self.pending = FieldSet::all();
    }

    /// Writes the open frame's sidecar when the disk differs; the failure's message otherwise.
    pub fn flush(&mut self) -> Option<String> {
        let open = self.open.as_ref()?;
        let failure =
            sidecar::flush(&self.settings, Some(Path::new(&open.path)), open.persisted.as_ref(), open.writable);
        if failure.is_none() && open.writable && sidecar::should_write(&self.settings, Path::new(&open.path)) {
            self.open.as_mut().expect("open frame").persisted = Some(self.settings.clone());
        }
        if let Some(message) = &failure {
            self.failure = Some(message.clone());
        }
        failure
    }

    /// Flushes once the hand has rested `WRITE_DELAY`, so a drag writes the sidecar once.
    pub fn flush_if_due(&mut self, now: f64) -> Option<String> {
        let due = !self.held && self.last_change.is_some_and(|t| now - t >= sidecar::WRITE_DELAY.as_secs_f64());
        if !due {
            return None;
        }
        self.last_change = None;
        self.flush()
    }

    fn replace(&mut self, next: PipelineSettings) -> FieldSet {
        let changed = FieldSet::between(&self.settings, &next);
        if !changed.is_empty() {
            self.settings = next;
            self.pending = self.pending.union(changed);
            self.revision += 1;
        }
        changed
    }

    // MARK: - Commands

    /// A hand on a control: its notches land at once and the step is recorded on release.
    pub fn hold(&mut self) {
        self.held = true;
    }

    /// The hand lifts and the step is recorded at once, with any step still waiting.
    pub fn release(&mut self) {
        let now = self.now();
        self.release_at(now);
    }

    pub fn release_at(&mut self, now: f64) {
        if std::mem::take(&mut self.held) || self.deferred.is_some() {
            self.settle(now);
        }
    }

    /// The hand lifts and the step waits `SETTLE_DELAY` past the last change, so a press landing
    /// sooner, a double click's second, joins it (`EditorScene+Effects.swift:59-62`).
    pub fn release_deferred(&mut self) {
        let now = self.now();
        self.release_deferred_at(now);
    }

    pub fn release_deferred_at(&mut self, now: f64) {
        if std::mem::take(&mut self.held) {
            self.deferred = Some(now + SETTLE_DELAY);
        }
    }

    /// The next unheld change waits `SETTLE_DELAY` like a lifted hand's; a hand on a control
    /// records it at its own release.
    pub fn defer(&mut self) {
        let now = self.now();
        self.defer_at(now);
    }

    pub fn defer_at(&mut self, now: f64) {
        if !self.held && self.deferred.is_none() {
            self.deferred = Some(now + SETTLE_DELAY);
        }
    }

    /// Records the waiting step once due; how long it still waits, `None` when nothing waits
    /// unheld, a hand back on a control recording it at its own release.
    pub fn settle_if_due(&mut self) -> Option<std::time::Duration> {
        let now = self.now();
        self.settle_if_due_at(now).map(std::time::Duration::from_secs_f64)
    }

    pub fn settle_if_due_at(&mut self, now: f64) -> Option<f64> {
        let due = self.deferred.filter(|_| !self.held)?;
        if now < due {
            return Some(due - now);
        }
        self.settle(now);
        None
    }

    pub fn apply(&mut self, command: Command) -> Applied {
        let now = self.now();
        self.apply_at(command, now)
    }

    /// Applies a command as at `now`, seconds on the store's own clock.
    pub fn apply_at(&mut self, command: Command, now: f64) -> Applied {
        if let Command::Face(face) = command {
            self.scanner_face = face == Face::Scanner;
        }
        if self.open.is_none() {
            return Applied::default();
        }
        match command {
            Command::Undo => return Applied { changed: self.undo(Surface::Editor), walk: None },
            Command::Redo => return Applied { changed: self.redo(Surface::Editor), walk: None },
            Command::Scan(Scan::Walk { by }) => return Applied { changed: FieldSet::EMPTY, walk: Some(by) },
            _ => {}
        }
        let by_hand = !matches!(command, Command::Reset);
        let next = self.commanded(command);
        let changed = self.replace(next);
        if changed.is_empty() {
            return Applied::default();
        }
        if by_hand && changed.intersects(FieldSet::of_family(Family::Roll)) {
            self.roll_touched = true;
        }
        self.last_change = Some(now);
        if !self.held {
            match self.deferred {
                Some(_) => self.deferred = Some(now + SETTLE_DELAY),
                None => self.settle(now),
            }
        }
        Applied { changed, walk: None }
    }

    /// The face the open frame is edited on.
    pub fn face(&self) -> Face {
        if self.scanner_face { Face::Scanner } else { Face::Pro }
    }

    /// `colourBinding`: the settings as the open face's Color tab reads them, the Scanner's own
    /// finishing in Pro's fields on the Scanner face.
    pub fn face_view(&self) -> PipelineSettings {
        if self.scanner_face { self.settings.as_scanner() } else { self.settings.clone() }
    }

    /// An edit made on `face_view`, stored so the other face's finishing stays where it was.
    fn stored_by_face(&self, edited: PipelineSettings) -> PipelineSettings {
        if self.scanner_face { self.settings.storing_scanner_edit(&edited) } else { edited }
    }

    fn commanded(&self, command: Command) -> PipelineSettings {
        let s = &self.settings;
        match command {
            Command::Set { knob, value } => self.stored_by_face(knob.set(&self.face_view(), value)),
            Command::Step { knob, by } => {
                let view = self.face_view();
                self.stored_by_face(knob.set(&view, knob.get(&view) + by))
            }
            Command::Write(next) => *next,
            Command::Finish(next) => self.stored_by_face(*next),
            Command::Face(face) => PipelineSettings { face, ..s.clone() },
            Command::Mode(mode) => adapted(s, mode),
            Command::Reset => parameters::reset_blank(s, self.scanner_face),
            Command::Scan(scan) => scanned(s, scan),
            Command::Undo | Command::Redo => s.clone(),
        }
    }

    /// Records the open frame's step, then lays a Roll Settings gesture or a look pasted on the roll
    /// on every other frame naming the same reference, as part of that one step.
    fn settle(&mut self, now: f64) {
        self.deferred = None;
        let before = self.history.current().clone();
        let touched = std::mem::take(&mut self.roll_touched);
        let look = self.roll_look.take();
        let serial = self.history.record(self.settings.clone(), now);
        if let (true, Some(serial)) = (touched, serial) {
            let after = self.settings.clone();
            self.propagate_roll(&before, &after, serial);
        }
        if let Some((donor, id)) = look {
            self.paste_on_roll(&donor, id, serial);
        }
    }

    fn propagate_roll(&mut self, before: &PipelineSettings, after: &PipelineSettings, serial: u64) {
        let fields = roll_settings::changed(before, after);
        let Some(id) = after.roll_reference.as_ref().map(|r| r.id) else { return };
        if fields.is_empty() {
            return;
        }
        let others = self.others_on_roll(id);
        let (was, now) = self.modify(&others, |_, loaded| {
            if loaded.roll_reference.as_ref().map(|r| r.id) != Some(id) {
                return None;
            }
            Some(roll_settings::copied(&fields, after, loaded))
        });
        if !was.is_empty() {
            self.history.attach(Companion { before: was, after: now }, serial);
        }
    }

    fn paste_on_roll(&mut self, donor: &PipelineSettings, id: Id, serial: Option<u64>) {
        let others = self.others_on_roll(id);
        let (was, now) = self.modify(&others, |_, loaded| {
            if loaded.roll_reference.as_ref().map(|r| r.id) != Some(id) {
                return None;
            }
            Some(look_settings::copied(donor, loaded))
        });
        if was.is_empty() {
            return;
        }
        let companion = Companion { before: was, after: now };
        match serial {
            Some(serial) => self.history.attach(companion, serial),
            None => self.history.lay(companion),
        }
    }

    /// The frames already found naming reference `id`, never scanning: what a menu title may read.
    pub fn measured_group(&self, id: Id) -> Option<&BTreeSet<String>> {
        self.measured.get(&id)
    }

    /// Whether Copy Settings and the framing's Copy have filled the board.
    pub fn has_copied_settings(&self) -> bool {
        self.board.settings.is_some()
    }

    /// The frames naming reference `id`, scanned once per reference; the open frame counts by its
    /// settings in memory, its sidecar possibly lagging.
    pub fn measured_with(&mut self, id: Id) -> BTreeSet<String> {
        let mut found = match self.measured.get(&id) {
            Some(known) => known.clone(),
            None => {
                let scanned: BTreeSet<String> = roll_membership::measured(id, &self.archive.paths).into_iter().collect();
                self.measured.insert(id, scanned.clone());
                scanned
            }
        };
        if let Some(open) = &self.open {
            if self.settings.roll_reference.as_ref().map(|r| r.id) == Some(id) {
                found.insert(open.path.clone());
            } else {
                found.remove(&open.path);
            }
        }
        found
    }

    fn others_on_roll(&mut self, id: Id) -> Vec<String> {
        let open = self.path().map(str::to_string);
        self.measured_with(id).into_iter().filter(|p| Some(p) != open.as_ref()).collect()
    }

    /// Reads each frame's sidecar, lays `transform` and writes what moved; a sidecar from a newer
    /// version is left alone. The before and after of every frame written.
    fn modify(
        &mut self,
        paths: &[String],
        transform: impl Fn(&str, &PipelineSettings) -> Option<PipelineSettings>,
    ) -> (Settings, Settings) {
        self.modify_with(paths, false, transform)
    }

    /// `modify`, keeping each sidecar's modification date when `preserving` so the thumbnail key holds.
    fn modify_with(
        &mut self,
        paths: &[String],
        preserving: bool,
        transform: impl Fn(&str, &PipelineSettings) -> Option<PipelineSettings>,
    ) -> (Settings, Settings) {
        let mut before = Settings::new();
        let mut after = Settings::new();
        for path in paths {
            let original = Path::new(path);
            let loaded = match sidecar::read(original, self.mosaic(path), true) {
                Ok(loaded) if loaded.writable => loaded,
                Ok(_) => continue,
                Err(e) => {
                    self.failure = Some(e.to_string());
                    continue;
                }
            };
            let Some(next) = transform(path, &loaded.settings) else { continue };
            if next == loaded.settings {
                continue;
            }
            match sidecar::write(&next, original, WriteOptions { preserving_revision: preserving, ..WriteOptions::default() }) {
                Ok(()) => {
                    before.insert(path.clone(), loaded.settings);
                    after.insert(path.clone(), next);
                }
                Err(e) => self.failure = Some(e.to_string()),
            }
        }
        (before, after)
    }

    // MARK: - Undo

    pub fn undo(&mut self, surface: Surface) -> FieldSet {
        match surface {
            Surface::Editor => {
                let others = self.history.undo_companion().cloned();
                let Some(state) = self.history.undo() else { return FieldSet::EMPTY };
                let changed = self.replace(state);
                self.last_change = Some(self.now());
                if let Some(others) = others {
                    self.write_back(&others.before);
                }
                changed
            }
            Surface::Gallery => match self.gallery.undo_step() {
                Some(step) => self.take_back(step, true),
                None => FieldSet::EMPTY,
            },
        }
    }

    pub fn redo(&mut self, surface: Surface) -> FieldSet {
        match surface {
            Surface::Editor => {
                let others = self.history.redo_companion().cloned();
                let Some(state) = self.history.redo() else { return FieldSet::EMPTY };
                let changed = self.replace(state);
                self.last_change = Some(self.now());
                if let Some(others) = others {
                    self.write_back(&others.after);
                }
                changed
            }
            Surface::Gallery => match self.gallery.redo_step() {
                Some(step) => self.take_back(step, false),
                None => FieldSet::EMPTY,
            },
        }
    }

    fn take_back(&mut self, step: Step, undoing: bool) -> FieldSet {
        if let Some(disk) = &step.disk {
            if undoing {
                roll_folder::take_back(disk);
            } else {
                roll_folder::lay(disk);
            }
            return FieldSet::EMPTY;
        }
        if let Some(marks) = &step.trash {
            self.lay_trash(marks, undoing);
        }
        self.write_back(if undoing { &step.before } else { &step.after })
    }

    /// Lays restored settings back: the open frame in memory, so the panel never lags, the others
    /// through their sidecars.
    fn write_back(&mut self, restored: &Settings) -> FieldSet {
        for id in restored.values().filter_map(|s| s.roll_reference.as_ref().map(|r| r.id)) {
            self.measured.remove(&id);
        }
        let open = self.path().map(str::to_string);
        let mut changed = FieldSet::EMPTY;
        if let Some(wanted) = open.as_ref().and_then(|p| restored.get(p)) {
            changed = self.replace(wanted.clone());
            self.last_change = Some(self.now());
        }
        let others: Vec<String> = restored.keys().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        self.modify(&others, |path, _| restored.get(path).cloned());
        changed
    }

    // MARK: - Copy and paste

    pub fn copy_settings(&mut self) {
        match codec::encode_settings(&self.settings) {
            Ok(text) => self.board.settings = Some(text),
            Err(e) => self.failure = Some(e.to_string()),
        }
    }

    fn paste_source(&self) -> Option<PipelineSettings> {
        let text = self.board.settings.as_ref()?;
        codec::decode(text.as_bytes()).ok().map(|d| d.settings)
    }

    fn reference_of(&self, path: &str) -> Option<Id> {
        if self.path() == Some(path) {
            return self.settings.roll_reference.as_ref().map(|r| r.id);
        }
        roll_membership::reference(path)
    }

    /// Whether pasting onto `targets` lays one roll's look on another, which the hand confirms first.
    pub fn paste_check(&self, targets: &[String]) -> PasteCheck {
        let Some(donor) = self.paste_source() else { return PasteCheck::Nothing };
        if targets.is_empty() {
            return PasteCheck::Nothing;
        }
        let roll = donor.roll_reference.as_ref().map(|r| r.id);
        if targets.iter().any(|t| self.reference_of(t) != roll) { PasteCheck::AnotherRoll } else { PasteCheck::Ready }
    }

    /// Lays the copied Color and Details tabs, whole, on every target, one gallery step.
    pub fn paste_settings(&mut self, targets: &[String]) -> usize {
        let Some(donor) = self.paste_source() else { return 0 };
        self.lay_on(targets, |_, s| look_settings::copied(&donor, s), |n| {
            if n == 1 { "Paste settings".into() } else { format!("Paste settings to {n} photos") }
        })
    }

    /// The open frame in memory, then every other frame of its roll as part of that one step; a
    /// frame never measured takes the paste alone.
    pub fn paste_to_roll(&mut self) {
        let Some(donor) = self.paste_source() else { return };
        let Some(id) = self.settings.roll_reference.as_ref().map(|r| r.id) else {
            let open: Vec<String> = self.path().map(str::to_string).into_iter().collect();
            self.paste_settings(&open);
            return;
        };
        let pasted = look_settings::copied(&donor, &self.settings);
        if pasted == self.settings {
            self.paste_on_roll(&donor, id, None);
            return;
        }
        self.roll_look = Some((donor, id));
        let now = self.now();
        self.replace(pasted);
        self.last_change = Some(now);
        self.settle(now);
    }

    pub fn copy_framing(&mut self) {
        self.board.framing = Some(self.settings.geometry);
    }

    /// The copied framing on every target, one gallery step; fitting it inside each image is the
    /// host's, which knows the sizes.
    /// `pasteFraming(to:)`: only the open frame is fitted, to `open_size`, its own full size.
    pub fn paste_framing(&mut self, targets: &[String], open_size: Option<(f64, f64)>) -> usize {
        let Some(framing) = self.board.framing else { return 0 };
        let open = self.path().map(str::to_string);
        self.lay_on(
            targets,
            |path, s| {
                let mut geometry = framing;
                if let (Some(size), true) = (open_size, open.as_deref() == Some(path)) {
                    geometry.fit_crop_inside_image(size);
                }
                PipelineSettings { geometry, ..s.clone() }
            },
            |n| if n == 1 { "Paste framing".into() } else { format!("Paste framing to {n} photos") },
        )
    }

    /// One gallery step for a gesture on several frames: the open one in memory, the others on disk.
    fn lay_on(
        &mut self,
        targets: &[String],
        transform: impl Fn(&str, &PipelineSettings) -> PipelineSettings,
        label: impl Fn(usize) -> String,
    ) -> usize {
        let mut before = Settings::new();
        let mut after = Settings::new();
        let open = self.path().map(str::to_string);
        if let Some(open) = open.as_ref().filter(|p| targets.contains(p)) {
            let next = transform(open, &self.settings);
            if next != self.settings {
                before.insert(open.clone(), self.settings.clone());
                after.insert(open.clone(), next.clone());
                let now = self.now();
                self.replace(next);
                self.last_change = Some(now);
                self.settle(now);
            }
        }
        let others: Vec<String> = targets.iter().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        let (was, now) = self.modify(&others, |p, s| Some(transform(p, s)));
        before.extend(was);
        after.extend(now);
        let count = before.len();
        self.gallery.record(&before, &after, &label(count), None);
        count
    }

    // MARK: - Gestures on several frames

    /// Each target back to its imported state, one step for the gesture.
    pub fn reset_frames(&mut self, targets: &[String]) -> usize {
        let scanner = self.scanner_face;
        self.lay_on(targets, |_, s| parameters::reset_blank(s, scanner), |n| {
            if n == 1 { "Reset 1 photo".into() } else { format!("Reset {n} photos") }
        })
    }

    /// The conversion mode on every target; the open frame alone takes it as its own edit.
    pub fn apply_mode(&mut self, mode: ConversionMode, targets: &[String]) -> usize {
        if targets.len() == 1 && self.path() == Some(targets[0].as_str()) {
            return usize::from(!self.apply(Command::Mode(mode)).changed.is_empty());
        }
        self.lay_on(targets, |_, s| parameters::carried(s, mode), |n| {
            if n == 1 { "Mode on 1 photo".into() } else { format!("Mode on {n} photos") }
        })
    }

    /// The preset's rows on every target, one step; a preset naming the balance key measures each.
    pub fn apply_preset(&mut self, preset: &Preset, targets: &[String], measure: Option<&dyn Measure>) -> usize {
        let name = preset.name.clone();
        let count = self.lay_on(targets, |_, s| preset.apply(s), |n| {
            if n == 1 { format!("Apply \u{201c}{name}\u{201d}") } else { format!("Apply \u{201c}{name}\u{201d} to {n} photos") }
        });
        if parameters::auto_balances(&preset.parameters)
            && let Some(measure) = measure
        {
            self.auto_balance(targets, Strategy::Each, measure);
        }
        count
    }

    /// The balance key on every target, read from each frame's own sidecar, never the screen; the
    /// open frame's step carries the others, so one undo takes the press back.
    pub fn auto_balance(&mut self, targets: &[String], strategy: Strategy, measure: &dyn Measure) -> Option<String> {
        let open = self.path().map(str::to_string);
        let frames: Vec<(String, PipelineSettings)> = targets
            .iter()
            .map(|path| {
                let settings = if Some(path) == open.as_ref() {
                    self.settings.clone()
                } else {
                    sidecar::read(Path::new(path), self.mosaic(path), true).map(|l| l.settings).unwrap_or_default()
                };
                (path.clone(), settings)
            })
            .collect();
        let (landed, failure) = balance::run(&frames, strategy, measure);
        let outcomes: HashMap<String, balance::Outcome> = landed.into_iter().collect();
        let mut serial = None;
        if let Some(outcome) = open.as_ref().and_then(|p| outcomes.get(p))
            && self.writable()
        {
            let now = self.now();
            let next = balance::applied_outcome(outcome, &self.settings);
            self.replace(next);
            self.last_change = Some(now);
            serial = self.history.record(self.settings.clone(), now);
        }
        let others: Vec<String> = outcomes.keys().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        let (was, now) = self.modify(&others, |p, s| outcomes.get(p).map(|o| balance::applied_outcome(o, s)));
        if !was.is_empty() {
            match serial {
                Some(serial) => self.history.attach(Companion { before: was, after: now }, serial),
                None => {
                    let n = was.len();
                    let label = if n == 1 { "Balance 1 photo".into() } else { format!("Balance {n} photos") };
                    self.gallery.record(&was, &now, &label, None);
                }
            }
        }
        let message = failure.map(|f| f.to_string());
        if message.is_some() {
            self.failure = message.clone();
        }
        message
    }

    /// The roll's settings and reference on the frames of its folder never measured with it.
    /// `applyFlatField(to:)`: the open frame's flat-field written on others, one gallery step.
    pub fn apply_flat_field(&mut self, paths: &[String]) -> usize {
        let wanted = self.settings.flat_field.clone();
        let open = self.path().map(str::to_string);
        let others: Vec<String> = paths.iter().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        let (was, now) = self.modify(&others, |_, loaded| {
            let mut next = loaded.clone();
            next.flat_field = wanted.clone();
            Some(next)
        });
        let n = was.len();
        if n > 0 {
            let label = if n == 1 { "Flat-field on 1 photo".into() } else { format!("Flat-field on {n} photos") };
            self.gallery.record(&was, &now, &label, None);
        }
        n
    }

    pub fn join_roll(&mut self, paths: &[String]) -> usize {
        let Some(reference) = self.settings.roll_reference.clone() else { return 0 };
        let donor = self.settings.clone();
        let all: BTreeSet<roll_settings::Field> = roll_settings::Field::ALL.into_iter().collect();
        let (was, now) = self.modify(paths, |_, loaded| {
            let mut joined = roll_settings::copied(&all, &donor, loaded);
            joined.roll_reference = Some(reference.clone());
            joined.roll_reference_declined = false;
            // The windows were placed on the decode the roll read; a joining RAW keeps no primaries.
            joined.primaries_converted = donor.primaries_converted && !loaded.decoded_from_mosaic;
            Some(joined)
        });
        let n = was.len();
        if n > 0 {
            self.measured.entry(reference.id).or_default().extend(now.keys().cloned());
            let label = if n == 1 { "Join the roll".into() } else { format!("Join the roll with {n} photos") };
            self.gallery.record(&was, &now, &label, None);
        }
        n
    }

    /// `updateRoll` and `applyFilm`: the roll on every target, the open frame's in memory and
    /// written at rest, the others' sidecars at once as one gallery step; `None` ungroups.
    pub fn set_film(&mut self, paths: &[String], film: Option<oni_settings::Film>, label: &str) -> usize {
        let open = self.path().map(str::to_string);
        let mut count = 0;
        if open.as_ref().is_some_and(|p| paths.contains(p)) {
            let mut next = self.settings.clone();
            next.film = film.clone();
            if !self.replace(next).is_empty() {
                self.last_change = Some(self.now());
                count += 1;
            }
        }
        let others: Vec<String> = paths.iter().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        let (was, now) = self.modify_with(&others, true, |_, s| {
            let mut next = s.clone();
            next.film = film.clone();
            Some(next)
        });
        count += was.len();
        self.gallery.record(&was, &now, label, None);
        count
    }

    // MARK: - The ONI Trash

    /// Marks each frame's sidecar, its files untouched; the open frame among them closes.
    pub fn move_to_trash(&mut self, paths: &[String]) -> usize {
        self.mark(paths, Some(Date::now()))
    }

    pub fn restore_from_trash(&mut self, paths: &[String]) -> usize {
        self.mark(paths, None)
    }

    fn mark(&mut self, paths: &[String], date: Option<Date>) -> usize {
        if self.path().is_some_and(|p| paths.iter().any(|t| t == p)) {
            self.flush();
        }
        let files = oni_library::rgb_passes::expanded(paths);
        let mosaic = &self.archive.mosaic;
        let outcome = trash::mark(&files, date, &|p: &Path| mosaic(p));
        self.took(&outcome, date.is_some());
        let frames = paths.iter().filter(|p| outcome.done.contains(p)).count();
        let label = match (date.is_some(), frames) {
            (true, 1) => "Move to ONI Trash".to_string(),
            (true, n) => format!("Move {n} Photos to ONI Trash"),
            (false, 1) => "Restore 1 Photo".to_string(),
            (false, n) => format!("Restore {n} Photos"),
        };
        let marks = Trash { paths: outcome.done.iter().cloned().collect(), marked: date.is_some() };
        self.gallery.record_trash(marks, &label);
        frames
    }

    fn lay_trash(&mut self, marks: &Trash, undoing: bool) {
        let date = (marks.marked != undoing).then(Date::now);
        let files: Vec<String> = marks.paths.iter().cloned().collect();
        if date.is_some() && self.path().is_some_and(|p| marks.paths.contains(p)) {
            self.flush();
        }
        let mosaic = &self.archive.mosaic;
        let outcome = trash::mark(&files, date, &|p: &Path| mosaic(p));
        self.took(&outcome, date.is_some());
    }

    fn took(&mut self, outcome: &trash::Outcome, marked: bool) {
        if let Some(message) = trash::report(outcome, !marked) {
            self.failure = Some(message);
        }
        if marked && self.path().is_some_and(|p| outcome.done.iter().any(|d| d == p)) {
            self.open = None;
            self.history = History::default();
            self.replace(PipelineSettings::default());
        }
    }
}

/// A mode flip carries the axis and the windows still at one mode's rest onto the other's.
fn adapted(s: &PipelineSettings, mode: ConversionMode) -> PipelineSettings {
    let mut next = parameters::carried(s, mode);
    next.print.exposure = s.print.exposure;
    next
}

/// The Scanner face's keys on its own fields; the ranges are the phone's, the desktop's the same.
fn scanned(s: &PipelineSettings, scan: Scan) -> PipelineSettings {
    let mut out = s.clone();
    match scan {
        Scan::Step { letter, by } => {
            let (lo, hi) = letter.range();
            let clicks = clicks_mut(&mut out, letter);
            *clicks = (*clicks + i64::from(by)).clamp(i64::from(lo), i64::from(hi));
        }
        Scan::Reset { letter } => *clicks_mut(&mut out, letter) = 0,
        Scan::StepEffect { effect, by } => {
            let channel = channel(s.mode);
            if let Some(value) = effect_mut(&mut out, effect) {
                let (lo, hi) = effect.range(channel);
                let clicks = (*value / effect.step()).round() + f32::from(by);
                *value = (clicks * effect.step()).clamp(lo, hi);
            }
        }
        Scan::ResetEffect { effect } => {
            if let Some(value) = effect_mut(&mut out, effect) {
                *value = 0.0;
            }
        }
        Scan::Machine(machine) => out.scanner_machine = machine_of(machine),
        Scan::Walk { .. } => {}
    }
    out
}

fn clicks_mut(s: &mut PipelineSettings, letter: Letter) -> &mut i64 {
    match letter {
        Letter::D => &mut s.scanner.d,
        Letter::C => &mut s.scanner.c,
        Letter::M => &mut s.scanner.m,
        Letter::Y => &mut s.scanner.y,
        Letter::W => &mut s.scanner.w,
    }
}

/// Chrome has no key in black and white.
fn effect_mut(s: &mut PipelineSettings, effect: Effect) -> Option<&mut f32> {
    match effect {
        Effect::Contrast => Some(&mut s.scanner_finish.contrast),
        Effect::Vignette => Some(&mut s.scanner_finish.vignette.stops),
        Effect::Chrome if s.mode.is_monochrome() => None,
        Effect::Chrome => Some(&mut s.scanner_finish.secret_sauce),
    }
}

pub fn channel(mode: ConversionMode) -> Channel {
    match mode {
        ConversionMode::Negative => Channel::Negative,
        ConversionMode::Positive => Channel::Positive,
        ConversionMode::Monochrome => Channel::Monochrome,
    }
}

pub fn machine_of(machine: Machine) -> ScannerMachine {
    match machine {
        Machine::Oni => ScannerMachine::Oni,
        Machine::Frontier => ScannerMachine::Frontier,
    }
}

pub fn machine_for(machine: ScannerMachine) -> Machine {
    match machine {
        ScannerMachine::Oni => Machine::Oni,
        ScannerMachine::Frontier => Machine::Frontier,
    }
}

/// Re-exported for the host, which records disk gestures on the same gallery stack.
pub use gallery_history::Disk;

impl EditorStore {
    pub fn record_disk(&mut self, disk: Disk, label: &str) {
        self.gallery.record_disk(disk, label);
    }

    /// Whether a field of the open frame belongs to its roll, which a hand edit writes roll-wide.
    pub fn is_roll_field(field: Field) -> bool {
        field.family() == Some(Family::Roll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knob::Knob;

    fn fresh(label: &str) -> (EditorStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("oni-core-store-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let frame = dir.join("frame.tif").to_string_lossy().into_owned();
        let mut store = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| false));
        store.open(&frame).unwrap();
        (store, dir)
    }

    #[test]
    fn a_notch_reports_its_field_once_and_an_unmoved_value_reports_nothing() {
        let (mut store, dir) = fresh("changes");
        store.take_changes();
        let revision = store.revision();
        store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
        assert_eq!(store.take_changes().iter().collect::<Vec<_>>(), vec![Field::Vignette]);
        assert!(store.take_changes().is_empty());
        assert_eq!(store.revision(), revision + 1);
        // Twin: the same value again moves nothing, bumps nothing and opens no step.
        let applied = store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 5.0);
        assert!(applied.changed.is_empty() && store.take_changes().is_empty());
        assert_eq!(store.revision(), revision + 1);
        store.undo(Surface::Editor);
        assert!(!store.can_undo(Surface::Editor));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two presses on two settings, the second landing `gap` seconds after the first lifts.
    fn two_presses(store: &mut EditorStore, gap: f64) {
        store.hold();
        store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
        store.release_deferred_at(0.1);
        let pressed = 0.1 + gap;
        let _ = store.settle_if_due_at(pressed);
        store.hold();
        store.apply_at(Command::Set { knob: Knob::Texture, value: 0.3 }, pressed);
        store.release_deferred_at(pressed + 0.05);
        assert!(store.settle_if_due_at(pressed + 0.05 + SETTLE_DELAY - 0.01).is_some(), "still waiting");
        assert_eq!(store.settle_if_due_at(pressed + 0.05 + SETTLE_DELAY + 0.01), None);
    }

    #[test]
    fn a_press_within_the_settle_delay_joins_the_step_before_it() {
        let (mut store, dir) = fresh("settle");
        let rest = (Knob::Vignette.get(store.settings()), Knob::Texture.get(store.settings()));
        two_presses(&mut store, 0.1);
        store.undo(Surface::Editor);
        assert_eq!((Knob::Vignette.get(store.settings()), Knob::Texture.get(store.settings())), rest, "one step for both presses");
        assert!(!store.can_undo(Surface::Editor));
        // Twin: the second press after the delay makes a step of its own.
        let (mut apart, dir2) = fresh("settle-apart");
        two_presses(&mut apart, SETTLE_DELAY + 0.05);
        apart.undo(Surface::Editor);
        assert_eq!((Knob::Vignette.get(apart.settings()), Knob::Texture.get(apart.settings())), (0.5, rest.1));
        assert!(apart.can_undo(Surface::Editor));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    #[test]
    fn an_unheld_change_moves_the_waiting_step_on_and_a_plain_release_records_it() {
        let (mut store, dir) = fresh("settle-moved");
        store.hold();
        store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
        store.release_deferred_at(0.0);
        store.apply_at(Command::Set { knob: Knob::Texture, value: 0.3 }, 0.1);
        assert!(store.settle_if_due_at(0.2).is_some(), "the key's change restarted the delay");
        assert!(!store.can_undo(Surface::Editor), "twin: nothing recorded before the delay");
        // An open or a slider's release records what waits, so it never reaches the next frame.
        store.release_at(0.2);
        assert!(store.can_undo(Surface::Editor));
        assert_eq!(store.settle_if_due_at(1.0), None);
        store.undo(Surface::Editor);
        assert!(!store.can_undo(Surface::Editor), "both edits are one step");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_untouched_frame_gains_no_sidecar_and_a_rested_edit_writes_one() {
        let (mut store, dir) = fresh("flush");
        let written = sidecar::url_for(&dir.join("frame.tif"));
        store.flush();
        assert!(!written.exists());
        store.apply_at(Command::Step { knob: Knob::Texture, by: 0.4 }, 0.0);
        assert_eq!(store.flush_if_due(0.1), None);
        assert!(!written.exists(), "a hand still moving writes nothing");
        store.flush_if_due(1.0);
        assert!(written.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_roll_set_on_its_frames_writes_the_others_at_once_keeps_their_dates_and_undoes_as_one_step() {
        let dir = std::env::temp_dir().join(format!("oni-core-store-film-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let [a, b, c] = ["a.tif", "b.tif", "c.tif"].map(|n| dir.join(n).to_string_lossy().into_owned());
        for p in [&b, &c] {
            sidecar::write(&PipelineSettings::default(), Path::new(p), WriteOptions::default()).unwrap();
        }
        let stamp = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        std::fs::File::options().write(true).open(sidecar::url_for(Path::new(&b))).unwrap().set_modified(stamp).unwrap();
        let mut store = EditorStore::new(Archive::new(vec![a.clone(), b.clone(), c.clone()], |_: &Path| false));
        store.open(&a).unwrap();
        let film = oni_settings::Film::new("Roll 3");
        let read = |p: &str| sidecar::read(Path::new(p), false, true).unwrap().settings.film;
        assert_eq!(store.set_film(&[a.clone(), b.clone()], Some(film.clone()), "Rename Roll"), 2);
        assert_eq!(store.settings().film.as_ref(), Some(&film), "the open frame takes it in memory");
        assert!(!sidecar::url_for(Path::new(&a)).exists(), "and writes it at rest, not at once");
        assert_eq!(read(&b), Some(film.clone()));
        let kept = std::fs::metadata(sidecar::url_for(Path::new(&b))).unwrap().modified().unwrap();
        assert_eq!(kept, stamp, "the revision holds, so the thumbnail key does");
        assert_eq!(read(&c), None, "twin: a frame not named keeps its roll");
        assert_eq!(store.gallery_history().undo_label(), Some("Rename Roll"));
        assert_eq!(store.set_film(&[b.clone()], Some(film.clone()), "Rename Roll"), 0, "an unchanged roll writes nothing");
        store.undo(Surface::Gallery);
        assert_eq!(read(&b), None, "one undo takes the others back");
        // Twin: `None` ungroups.
        store.set_film(&[b.clone()], Some(film.clone()), "Set Film Stock");
        store.set_film(&[b.clone()], None, "Ungroup");
        assert_eq!(read(&b), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
