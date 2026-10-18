//! Gestures on several frames at once, each one gallery step.

use super::*;

impl EditorStore {
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
}
