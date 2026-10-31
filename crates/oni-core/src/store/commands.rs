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
    /// on every other frame of the same roll, as part of that one step.
    pub(super) fn settle(&mut self, now: f64) {
        self.settle_beside(now, &[]);
    }

    /// `settle`, the frames of `handled` left to the gesture writing them itself: a frame together
    /// among them is written once, and the gesture's gallery step reads its true before.
    pub(super) fn settle_beside(&mut self, now: f64, handled: &[String]) {
        self.deferred = None;
        let before = self.history.current().clone();
        let touched = std::mem::take(&mut self.roll_touched);
        let look = self.roll_look.take();
        let serial = self.history.record(self.settings.clone(), now);
        if let (true, Some(serial)) = (touched, serial) {
            let after = self.settings.clone();
            self.propagate_roll(&before, &after, serial);
        }
        if let Some((donor, key)) = look {
            self.paste_on_roll(&donor, key, serial);
        }
        if let Some(serial) = serial {
            let after = self.settings.clone();
            self.propagate_together(&before, &after, serial, handled);
        }
    }

    /// A Roll Settings step on the open frame's roll; a slide roll keeps each slide's own levels and
    /// film type, the one a slide's own Auto levels placed and the one that makes it a slide.
    pub(super) fn propagate_roll(&mut self, before: &PipelineSettings, after: &PipelineSettings, serial: u64) {
        let Some(path) = self.path().map(str::to_string) else { return };
        let Some(key) = RollKey::of(&path, after).or_else(|| RollKey::of(&path, before)) else { return };
        let mut fields = roll_settings::changed(before, after);
        if matches!(key, RollKey::Slides(_)) {
            fields.remove(&roll_settings::Field::Levels);
            fields.remove(&roll_settings::Field::Mode);
        }
        if fields.is_empty() {
            return;
        }
        let others = self.others_on_roll(&key);
        let offset = matches!(key, RollKey::Slides(_)) && fields.contains(&roll_settings::Field::Gains);
        let base = |s: &PipelineSettings| s.measured_gains.unwrap_or_default();
        let (was, now) = self.modify(&others, |p, loaded| {
            (RollKey::of(p, loaded).as_ref() == Some(&key)).then(|| {
                let mut out = roll_settings::copied(&fields, after, loaded);
                // Each slide keeps its own measured balance: the hand's move lands on it as an offset.
                if offset {
                    out.gains.stops = std::array::from_fn(|c| base(loaded)[c] + after.gains.stops[c] - base(after)[c]);
                }
                out
            })
        });
        if !was.is_empty() {
            self.history.attach(Companion { before: was, after: now }, serial);
        }
    }

    pub(super) fn paste_on_roll(&mut self, donor: &PipelineSettings, key: RollKey, serial: Option<u64>) {
        let others = self.others_on_roll(&key);
        let (was, now) = self.modify(&others, |p, loaded| {
            (RollKey::of(p, loaded).as_ref() == Some(&key)).then(|| look_settings::copied(donor, loaded))
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

    /// The roll the open frame stands on, as its settings in memory say.
    pub fn roll_key(&self) -> Option<RollKey> {
        RollKey::of(self.path()?, &self.settings)
    }

    /// The frames already found on roll `key`, never scanning: what a menu title may read.
    pub fn measured_group(&self, key: impl Into<RollKey>) -> Option<&BTreeSet<String>> {
        self.measured.get(&key.into())
    }

    /// Whether Copy Settings and the framing's Copy have filled the board.
    pub fn has_copied_settings(&self) -> bool {
        self.board.settings.is_some()
    }

    /// The frames on roll `key` beside the open frame, scanned once per roll: a copy of the roll
    /// elsewhere is not reached. The open frame counts by its settings in memory.
    pub fn measured_with(&mut self, key: impl Into<RollKey>) -> BTreeSet<String> {
        let key = key.into();
        let mut found = match self.measured.get(&key) {
            Some(known) => known.clone(),
            None => {
                let scanned: BTreeSet<String> = roll_membership::members(&key, &self.archive.paths).into_iter().collect();
                self.measured.insert(key.clone(), scanned.clone());
                scanned
            }
        };
        if let Some(open) = &self.open {
            let folder = oni_library::paths::parent(&open.path);
            found.retain(|p| oni_library::paths::parent(p) == folder);
            if RollKey::of(&open.path, &self.settings).as_ref() == Some(&key) {
                found.insert(open.path.clone());
            } else {
                found.remove(&open.path);
            }
        }
        found
    }

    pub(super) fn others_on_roll(&mut self, key: &RollKey) -> Vec<String> {
        let open = self.path().map(str::to_string);
        self.measured_with(key.clone()).into_iter().filter(|p| Some(p) != open.as_ref()).collect()
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
