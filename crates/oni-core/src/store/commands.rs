//! The commands a hand or a key lands on the open frame, and their steps.

use super::*;

impl EditorStore {
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
        if self.inert() {
            return Applied::default();
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
    pub(super) fn stored_by_face(&self, edited: PipelineSettings) -> PipelineSettings {
        if self.scanner_face { self.settings.storing_scanner_edit(&edited) } else { edited }
    }

    pub(super) fn commanded(&self, command: Command) -> PipelineSettings {
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
    pub(super) fn settle(&mut self, now: f64) {
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
        if let Some(serial) = serial {
            let after = self.settings.clone();
            self.propagate_together(&before, &after, serial);
        }
    }

    pub(super) fn propagate_roll(&mut self, before: &PipelineSettings, after: &PipelineSettings, serial: u64) {
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

    pub(super) fn paste_on_roll(&mut self, donor: &PipelineSettings, id: Id, serial: Option<u64>) {
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

    pub(super) fn others_on_roll(&mut self, id: Id) -> Vec<String> {
        let open = self.path().map(str::to_string);
        self.measured_with(id).into_iter().filter(|p| Some(p) != open.as_ref()).collect()
    }

    /// Reads each frame's sidecar, lays `transform` and writes what moved; a sidecar from a newer
    /// version is left alone. The before and after of every frame written.
    pub(super) fn modify(
        &mut self,
        paths: &[String],
        transform: impl Fn(&str, &PipelineSettings) -> Option<PipelineSettings>,
    ) -> (Settings, Settings) {
        self.modify_with(paths, false, transform)
    }

    /// `modify`, keeping each sidecar's modification date when `preserving` so the thumbnail key holds.
    pub(super) fn modify_with(
        &mut self,
        paths: &[String],
        preserving: bool,
        transform: impl Fn(&str, &PipelineSettings) -> Option<PipelineSettings>,
    ) -> (Settings, Settings) {
        // One thread per file: a path twice would have two threads write one sidecar.
        let mut seen = std::collections::HashSet::new();
        let paths: Vec<String> = paths.iter().filter(|p| seen.insert(p.as_str())).cloned().collect();
        let paths = &paths[..];
        let mosaics: Vec<bool> = paths.iter().map(|p| self.mosaic(p)).collect();
        let loaded = in_parallel(paths, |i, path| sidecar::read(Path::new(path), mosaics[i], true));
        let mut writes = Vec::new();
        for (path, loaded) in paths.iter().zip(loaded) {
            let loaded = match loaded {
                Ok(loaded) if loaded.writable => loaded,
                Ok(_) => continue,
                Err(e) => {
                    self.failure = Some(e.to_string());
                    continue;
                }
            };
            let Some(next) = transform(path, &loaded.settings) else { continue };
            if next != loaded.settings {
                writes.push((path.clone(), loaded.settings, next));
            }
        }
        let options = WriteOptions { preserving_revision: preserving, ..WriteOptions::default() };
        let written = in_parallel(&writes, |_, (path, _, next)| sidecar::write(next, Path::new(path), options.clone()));
        let mut before = Settings::new();
        let mut after = Settings::new();
        for ((path, was, next), result) in writes.into_iter().zip(written) {
            match result {
                Ok(()) => {
                    before.insert(path.clone(), was);
                    after.insert(path, next);
                }
                Err(e) => self.failure = Some(e.to_string()),
            }
        }
        self.together.landed(&after);
        (before, after)
    }
}

/// `work` on each item, a few threads at once, the results in the items' order: a roll's sidecars
/// read and written side by side, each file its own, as Swift's batch moves them off the main thread.
fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(usize, &T) -> R + Sync) -> Vec<R> {
    const THREADS: usize = 8;
    if items.len() < 2 {
        return items.iter().enumerate().map(|(i, t)| work(i, t)).collect();
    }
    let chunk = items.len().div_ceil(THREADS);
    std::thread::scope(|s| {
        let work = &work;
        let handles: Vec<_> = items
            .chunks(chunk)
            .enumerate()
            .map(|(c, part)| s.spawn(move || part.iter().enumerate().map(|(j, t)| work(c * chunk + j, t)).collect::<Vec<R>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("a sidecar thread")).collect()
    })
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
