//! Gestures on several frames at once, each one step.

use super::*;
use oni_library::history::Medals;
use oni_settings::Medal;

impl EditorStore {
    /// Each target back to its imported state, one step for the gesture.
    pub fn reset_frames(&mut self, targets: &[String]) -> usize {
        let scanner = self.scanner_face;
        self.reset_each(targets, |s| parameters::reset_blank(s, scanner))
    }

    /// Each target back to its own rest, `rested` read off its own settings, one step: a rest depends
    /// on the frame's mode, decode and measure, so no frame takes another's.
    pub fn reset_each(&mut self, targets: &[String], rested: impl Fn(&PipelineSettings) -> PipelineSettings) -> usize {
        self.lay_on(targets, |_, s| rested(s), |n| trn("Reset {n} photo", "Reset {n} photos", n as u64))
    }

    /// The conversion mode on every target; the open frame alone takes it as its own edit.
    pub fn apply_mode(&mut self, mode: ConversionMode, targets: &[String]) -> usize {
        if targets.len() == 1 && self.path() == Some(targets[0].as_str()) {
            return usize::from(!self.apply(Command::Mode(mode)).changed.is_empty());
        }
        self.lay_on(targets, |_, s| parameters::carried(s, mode), |n| {
            trn("Mode on {n} photo", "Mode on {n} photos", n as u64)
        })
    }

    /// The preset's rows on every target, one step; a preset naming the balance key measures each.
    pub fn apply_preset(&mut self, preset: &Preset, targets: &[String], measure: Option<&dyn Measure>) -> usize {
        let name = preset.name.clone();
        let count = self.lay_on(targets, |_, s| preset.apply(s), |n| {
            trn("Apply \u{201c}{name}\u{201d}", "Apply \u{201c}{name}\u{201d} to {n} photos", n as u64).replace("{name}", &name)
        });
        if parameters::auto_balances(&preset.parameters)
            && let Some(measure) = measure
        {
            self.auto_balance(targets, Strategy::Each, measure);
        }
        count
    }

    /// The balance key on every target, read from each frame's own sidecar, never the screen; one
    /// step, so one undo takes the press back.
    pub fn auto_balance(&mut self, targets: &[String], strategy: Strategy, measure: &dyn Measure) -> Option<String> {
        self.release();
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
        let mut step = Written::default();
        if let Some(outcome) = open.as_ref().and_then(|p| outcomes.get(p))
            && self.writable()
        {
            let next = balance::applied_outcome(outcome, &self.settings);
            step = self.lay_open(next, targets);
        }
        let others: Vec<String> = outcomes.keys().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        gather(&mut step, self.modify(&others, |p, s| outcomes.get(p).map(|o| balance::applied_outcome(o, s))));
        let n = step.0.keys().filter(|p| outcomes.contains_key(*p)).count();
        let label = trn("Balance {n} photo", "Balance {n} photos", n as u64);
        self.history.record(&step.0, &step.1, &label, None);
        let message = failure.map(|f| f.to_string());
        if message.is_some() {
            self.failure = message.clone();
        }
        message
    }

    /// The roll's settings and reference on the frames of its folder never measured with it.
    /// `applyFlatField(to:)`: the open frame's flat-field written on others, one step.
    pub fn apply_flat_field(&mut self, paths: &[String]) -> usize {
        self.release();
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
            let label = trn("Flat-field on {n} photo", "Flat-field on {n} photos", n as u64);
            self.history.record(&was, &now, &label, None);
        }
        n
    }

    pub fn join_roll(&mut self, paths: &[String]) -> usize {
        self.release();
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
            // A roll never scanned is found whole on the next ask, never as the frames joined alone.
            if let Some(group) = self.measured.get_mut(&RollKey::Measured(reference.id)) {
                group.extend(now.keys().cloned());
            }
            let label = trn("Join the roll", "Join the roll with {n} photos", n as u64);
            self.history.record(&was, &now, &label, None);
        }
        n
    }

    /// `updateRoll`: the roll on every target, the open frame's in memory and written at rest, the
    /// others' sidecars at once as one step, the open frame in it too.
    pub fn set_film(&mut self, paths: &[String], film: Option<oni_settings::Film>, label: &str) -> usize {
        self.film_on(paths, film, |_| label.to_string())
    }

    /// `applyFilm`: `set_film`, its step counting the photos; `None` ungroups.
    pub fn apply_film(&mut self, paths: &[String], film: Option<oni_settings::Film>, label: &str) -> usize {
        self.film_on(paths, film, |n| trn("{label} · {n} photo", "{label} · {n} photos", n as u64).replace("{label}", label))
    }

    fn film_on(&mut self, paths: &[String], film: Option<oni_settings::Film>, label: impl Fn(usize) -> String) -> usize {
        self.roll_on(paths, None, |s| s.film = film.clone(), label)
    }

    /// The medal on each target's sidecar, its settings untouched, one step; answers each
    /// target's write, a refusal for the host to say.
    pub fn lay_medal(&mut self, targets: &[String], medal: Medal) -> Vec<(String, Result<(), sidecar::Error>)> {
        self.release();
        let mut worn = BTreeMap::new();
        let written: Vec<(String, Result<(), sidecar::Error>)> = targets
            .iter()
            .map(|path| {
                let (original, mosaic) = (Path::new(path), self.mosaic(path));
                let before = sidecar::read(original, mosaic, true).map_or(Medal::None, |l| l.label);
                let done = sidecar::set_label(medal, original, mosaic);
                if done.is_ok() {
                    worn.insert(path.clone(), before);
                }
                (path.clone(), done)
            })
            .collect();
        let changed = worn.values().filter(|m| **m != medal).count();
        let label = trn("Medal on {n} photo", "Medal on {n} photos", changed as u64);
        self.history.record_medals(Medals { before: worn, after: medal }, &label);
        written
    }

    /// A medal step laid back: each photo's own medal on undo, the one they took on redo.
    pub(super) fn lay_medals(&mut self, medals: &Medals, undoing: bool) {
        for (path, worn) in &medals.before {
            let medal = if undoing { *worn } else { medals.after };
            if let Err(e) = sidecar::set_label(medal, Path::new(path), self.mosaic(path)) {
                self.failure = Some(refusal::said(&e));
            }
        }
    }

    /// A roll's marker and what it writes on its frames, one step: the marker first, so a
    /// marker the disk refuses leaves every frame as it was.
    pub fn lay_roll(&mut self, paths: &[String], marker: Option<Marker>, edit: impl Fn(&mut PipelineSettings), label: &str) -> usize {
        self.roll_on(paths, marker, edit, |_| label.to_string())
    }

    fn roll_on(&mut self, paths: &[String], marker: Option<Marker>, edit: impl Fn(&mut PipelineSettings), label: impl Fn(usize) -> String) -> usize {
        self.release();
        if let Some(m) = marker.as_ref().filter(|m| m.before != m.after) {
            if let Err(e) = oni_library::folders::set_roll(m.after.as_ref(), &m.folder) {
                self.failure = Some(oni_i18n::tr!("The roll could not be written in its folder: {error}", error = refusal::cause(&e)));
                return 0;
            }
            self.marked = Some((m.folder.clone(), m.after.clone()));
        }
        let open = self.path().map(str::to_string);
        let mut step = Written::default();
        // The open frame joins the step, as `applyFilm`'s.
        if open.as_ref().is_some_and(|p| paths.contains(p)) && self.takes_hands() {
            let mut next = self.settings.clone();
            edit(&mut next);
            step = self.lay_open(next, paths);
        }
        let others: Vec<String> = paths.iter().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        gather(&mut step, self.modify_with(&others, true, |_, s| {
            let mut next = s.clone();
            edit(&mut next);
            Some(next)
        }));
        let count = step.0.keys().filter(|p| paths.contains(p)).count();
        self.history.record_marked(&step.0, &step.1, &label(count), marker);
        count
    }
}
