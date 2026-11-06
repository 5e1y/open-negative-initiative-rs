//! Copy and paste of settings, whole or by family.

use super::*;

impl EditorStore {
    pub fn copy_settings(&mut self) {
        match codec::encode_settings(&self.settings) {
            Ok(text) => self.board.settings = Some(text),
            Err(e) => self.failure = Some(e.to_string()),
        }
    }

    /// Copy Settings on any photo: the open frame's in memory, another's read off its sidecar.
    pub fn copy_settings_of(&mut self, path: &str) {
        if self.path() == Some(path) {
            return self.copy_settings();
        }
        let original = Path::new(path);
        let read = sidecar::original_present(original).and_then(|()| sidecar::read(original, self.mosaic(path), true));
        match read.map(|loaded| codec::encode_settings(&loaded.settings)) {
            Ok(Ok(text)) => self.board.settings = Some(text),
            Ok(Err(e)) => self.failure = Some(e.to_string()),
            Err(e) => self.failure = Some(refusal::said(&e)),
        }
    }

    pub(super) fn paste_source(&self) -> Option<PipelineSettings> {
        let text = self.board.settings.as_ref()?;
        codec::decode(text.as_bytes()).ok().map(|d| d.settings)
    }

    pub(super) fn reference_of(&self, path: &str) -> Option<Id> {
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

    /// Lays the copied Color and Details tabs, whole, on every target, one step.
    pub fn paste_settings(&mut self, targets: &[String]) -> usize {
        let Some(donor) = self.paste_source() else { return 0 };
        // `before.count`: Swift counts the open frame among the targets whether or not the paste moved it.
        let open_unmoved = self.path().is_some_and(|p| targets.iter().any(|t| t == p)) && look_settings::copied(&donor, &self.settings) == self.settings;
        let laid = self.lay_on(targets, |_, s| look_settings::copied(&donor, s), |n| {
            trn("Paste settings", "Paste settings to {n} photos", n as u64)
        });
        laid + usize::from(open_unmoved)
    }

    /// The open frame in memory, then every other frame of its roll as part of that one step; a
    /// frame on no roll takes the paste alone.
    pub fn paste_to_roll(&mut self) {
        self.release();
        if !self.takes_hands() {
            return;
        }
        let Some(donor) = self.paste_source() else { return };
        let Some(key) = self.roll_key() else {
            let open: Vec<String> = self.path().map(str::to_string).into_iter().collect();
            self.paste_settings(&open);
            return;
        };
        let pasted = look_settings::copied(&donor, &self.settings);
        if pasted == self.settings {
            let (was, now) = self.paste_on_roll(&donor, key);
            self.history.record(&was, &now, "", None);
            return;
        }
        self.roll_look = Some((donor, key));
        let now = self.now();
        self.replace(pasted);
        self.last_change = Some(now);
        self.settle(now);
    }

    pub fn copy_framing(&mut self) {
        self.board.framing = Some(self.settings.geometry);
    }

    /// The copied framing on every target, one step; fitting it inside each image is the
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
            |n| trn("Paste framing", "Paste framing to {n} photos", n as u64),
        )
    }

    /// One step for a gesture on several frames: the open one in memory, the frames together with it,
    /// the others on disk; the targets it changed.
    pub(super) fn lay_on(
        &mut self,
        targets: &[String],
        transform: impl Fn(&str, &PipelineSettings) -> PipelineSettings,
        label: impl Fn(usize) -> String,
    ) -> usize {
        self.release();
        let mut step = Written::default();
        let open = self.path().map(str::to_string);
        if let Some(open) = open.as_ref().filter(|p| targets.contains(p) && self.takes_hands()) {
            let next = transform(open, &self.settings);
            step = self.lay_open(next, targets);
        }
        let others: Vec<String> = targets.iter().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        gather(&mut step, self.modify(&others, |p, s| Some(transform(p, s))));
        let count = step.0.keys().filter(|p| targets.contains(p)).count();
        self.history.record(&step.0, &step.1, &label(count), None);
        count
    }
}
