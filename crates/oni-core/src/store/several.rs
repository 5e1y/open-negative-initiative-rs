//! Gestures on several frames at once, each one gallery step.

use super::*;

impl EditorStore {
    /// Each target back to its imported state, one step for the gesture.
    pub fn reset_frames(&mut self, targets: &[String]) -> usize {
        let scanner = self.scanner_face;
        self.lay_on(targets, |_, s| parameters::reset_blank(s, scanner), |n| {
            trn("Reset {n} photo", "Reset {n} photos", n as u64)
        })
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
                    let label = trn("Balance {n} photo", "Balance {n} photos", n as u64);
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
            let label = trn("Flat-field on {n} photo", "Flat-field on {n} photos", n as u64);
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
            let label = trn("Join the roll", "Join the roll with {n} photos", n as u64);
            self.gallery.record(&was, &now, &label, None);
        }
        n
    }

    /// `updateRoll`: the roll on every target, the open frame's in memory and written at rest, the
    /// others' sidecars at once as one gallery step, the open frame in it too.
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

    /// A roll's marker and what it writes on its frames, one gallery step: the marker first, so a
    /// marker the disk refuses leaves every frame as it was.
    pub fn lay_roll(&mut self, paths: &[String], marker: Option<Marker>, edit: impl Fn(&mut PipelineSettings), label: &str) -> usize {
        self.roll_on(paths, marker, edit, |_| label.to_string())
    }

    fn roll_on(&mut self, paths: &[String], marker: Option<Marker>, edit: impl Fn(&mut PipelineSettings), label: impl Fn(usize) -> String) -> usize {
        if let Some(m) = marker.as_ref().filter(|m| m.before != m.after) {
            if let Err(e) = oni_library::folders::set_roll(m.after.as_ref(), &m.folder) {
                self.failure = Some(oni_i18n::tr!("The roll could not be written in its folder: {error}", error = refusal::cause(&e)));
                return 0;
            }
            self.marked = Some((m.folder.clone(), m.after.clone()));
        }
        let open = self.path().map(str::to_string);
        let mut before = Settings::new();
        let mut after = Settings::new();
        let mut next = self.settings.clone();
        edit(&mut next);
        // The open frame joins the gallery step, as `applyFilm`'s, and records its own editor step.
        if let Some(open) = open.as_ref().filter(|p| paths.contains(p) && self.takes_hands())
            && self.settings != next
        {
            before.insert(open.clone(), self.settings.clone());
            after.insert(open.clone(), next.clone());
            let now = self.now();
            self.replace(next);
            self.last_change = Some(now);
            self.settle_beside(now, paths);
        }
        let others: Vec<String> = paths.iter().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        let (was, now) = self.modify_with(&others, true, |_, s| {
            let mut next = s.clone();
            edit(&mut next);
            Some(next)
        });
        before.extend(was);
        after.extend(now);
        let count = before.len();
        self.gallery.record_marked(&before, &after, &label(count), marker);
        count
    }
}
