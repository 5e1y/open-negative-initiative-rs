//! Copy and paste of settings, whole or by family.

use super::*;
use crate::preset_parameters;

/// The board's key for the rows a copy carries when it carries fewer than the look's all.
const CARRIES: &str = "carries";

/// What a paste lays: the look of the settings copied, the rows the copy left out excepted.
pub(super) struct Copied {
    pub donor: PipelineSettings,
    left_out: BTreeSet<String>,
}

impl Copied {
    /// The look laid whole, then each row left out given back to the photo it lands on.
    pub(super) fn laid(&self, target: &PipelineSettings) -> PipelineSettings {
        parameters::apply(&self.left_out, target, &look_settings::copied(&self.donor, target))
    }
}

/// The codec's text, naming the rows it carries when they are fewer than the look's.
fn board_text(settings: &PipelineSettings, carries: &BTreeSet<String>) -> Result<String, String> {
    let text = codec::encode_settings(settings).map_err(|e| e.to_string())?;
    if *carries == preset_parameters::pasted_rows() {
        return Ok(text);
    }
    let mut document: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    document[CARRIES] = carries.iter().cloned().collect::<Vec<String>>().into();
    Ok(document.to_string())
}

impl EditorStore {
    /// Copy Settings on `from`, else the open frame, read off its sidecar when not open, carrying
    /// the look's `carries` alone.
    pub fn copy_settings(&mut self, from: Option<&str>, carries: &BTreeSet<String>) {
        let read = match from.filter(|p| self.path() != Some(*p)) {
            None => Ok(self.settings.clone()),
            Some(path) => {
                let original = Path::new(path);
                sidecar::original_present(original).and_then(|()| sidecar::read(original, self.mosaic(path), true)).map(|l| l.settings).map_err(|e| refusal::said(&e))
            }
        };
        match read.and_then(|settings| board_text(&settings, carries)) {
            Ok(text) => self.board.settings = Some(text),
            Err(e) => self.failure = Some(e),
        }
    }

    pub(super) fn paste_source(&self) -> Option<Copied> {
        let text = self.board.settings.as_ref()?;
        let donor = oni_settings::migrate::document(text.as_bytes()).ok()?.settings;
        let named = serde_json::from_str::<serde_json::Value>(text).ok();
        let carries = named.as_ref().and_then(|v| v.get(CARRIES)).and_then(serde_json::Value::as_array);
        let left_out = match carries {
            Some(rows) => {
                let kept: BTreeSet<String> = rows.iter().filter_map(serde_json::Value::as_str).map(str::to_string).collect();
                preset_parameters::pasted_rows().difference(&kept).cloned().collect()
            }
            None => BTreeSet::new(),
        };
        Some(Copied { donor, left_out })
    }

    pub(super) fn reference_of(&self, path: &str) -> Option<Id> {
        if self.path() == Some(path) {
            return self.settings.roll_reference.as_ref().map(|r| r.id);
        }
        roll_membership::reference(path)
    }

    /// Whether pasting onto `targets` lays one roll's look on another, which the hand confirms first.
    pub fn paste_check(&self, targets: &[String]) -> PasteCheck {
        let Some(copied) = self.paste_source() else { return PasteCheck::Nothing };
        if targets.is_empty() {
            return PasteCheck::Nothing;
        }
        let roll = copied.donor.roll_reference.as_ref().map(|r| r.id);
        if targets.iter().any(|t| self.reference_of(t) != roll) { PasteCheck::AnotherRoll } else { PasteCheck::Ready }
    }

    /// Lays the copied Color and Details tabs, or the rows the copy carries of them, on every target, one step.
    pub fn paste_settings(&mut self, targets: &[String]) -> usize {
        let Some(copied) = self.paste_source() else { return 0 };
        // `before.count`: Swift counts the open frame among the targets whether or not the paste moved it.
        let open_unmoved = self.path().is_some_and(|p| targets.iter().any(|t| t == p)) && copied.laid(&self.settings) == self.settings;
        let laid = self.lay_on(targets, |_, s| copied.laid(s), |n| {
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
        let Some(copied) = self.paste_source() else { return };
        let Some(key) = self.roll_key() else {
            let open: Vec<String> = self.path().map(str::to_string).into_iter().collect();
            self.paste_settings(&open);
            return;
        };
        let pasted = copied.laid(&self.settings);
        if pasted == self.settings {
            let (was, now) = self.paste_on_roll(&copied, key);
            self.history.record(&was, &now, "", None);
            return;
        }
        self.roll_look = Some((copied, key));
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
            |n| trn("Paste crop", "Paste crop to {n} photos", n as u64),
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
