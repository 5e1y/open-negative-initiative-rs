//! The presets the hand saves: each a name and every setting it writes, laid after the built-in
//! keys, recalled whole by its key, renamed in its name's field and removed.

use super::*;
use crate::{PresetKey, PresetRow};

/// The preference holding them, in the order saved.
pub const KEPT: &str = "exportPresets";

/// Keys a row of the plate holds.
const ROW: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    pub name: String,
    pub settings: Settings,
}

/// The preference's value: each preset's name and its settings as the settings' own preference writes them.
pub fn to_value(kept: &[Kept]) -> Value {
    let item = |k: &Kept| serde_json::json!({ "name": k.name, "settings": serde_json::from_str::<Value>(&k.settings.raw_value()).unwrap_or_default() });
    Value::Array(kept.iter().map(item).collect())
}

/// Read back tolerantly: an item without a name or settings is left out, never the rest.
pub fn from_value(value: Option<&Value>) -> Vec<Kept> {
    let items = value.and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let read = |v: &Value| {
        let name = v.get("name")?.as_str()?.trim();
        let settings = Settings::from_raw_value(&v.get("settings")?.to_string())?;
        (!name.is_empty()).then(|| Kept { name: name.to_owned(), settings })
    };
    items.iter().filter_map(read).collect()
}

/// A built-in key's name as the plate shows it.
fn title(preset: Preset) -> &'static str {
    match preset {
        Preset::Instagram => oni_i18n::tr("Instagram"),
        Preset::FullDisplay => oni_i18n::tr("Display"),
        Preset::FullPrint => oni_i18n::tr("Print"),
    }
}

impl Card {
    /// The saved preset the settings equal.
    fn lit_kept(&self) -> Option<usize> {
        self.kept.iter().position(|k| k.settings == self.settings)
    }

    /// The key the settings equal: a built-in one, else a saved one after them, read off the state.
    pub(super) fn lit(&self) -> Option<usize> {
        let built_in = Preset::matching(&self.settings).and_then(|p| Preset::ALL.iter().position(|q| *q == p));
        built_in.or_else(|| self.lit_kept().map(|i| Preset::ALL.len() + i))
    }

    /// Every key, built-in then saved, three to a row.
    fn rows(&self) -> Vec<PresetRow> {
        let lit = self.lit();
        let names = Preset::ALL.iter().map(|p| title(*p).to_owned()).chain(self.kept.iter().map(|k| k.name.clone()));
        let keys: Vec<PresetKey> = names.enumerate().map(|(i, name)| PresetKey { name: name.into(), on: lit == Some(i), at: i as i32 }).collect();
        keys.chunks(ROW).map(|row| PresetRow { keys: ModelRc::new(VecModel::from(row.to_vec())) }).collect()
    }

    fn keep_presets(&mut self) {
        self.kept_saver.ask(to_value(&self.kept));
    }
}

impl Ctl {
    /// A key's whole state written: a built-in one's, or a saved one's settings as they were saved.
    pub(super) fn preset_chosen(self: &Rc<Self>, at: usize) {
        let before = self.export.borrow().settings.space().clone();
        {
            let mut c = self.export.borrow_mut();
            let next = match Preset::ALL.get(at) {
                Some(preset) => preset.applied(&c.settings),
                None => match c.kept.get(at - Preset::ALL.len()) {
                    Some(kept) => kept.settings.clone(),
                    None => return,
                },
            };
            c.settings = next;
            // The key wrote a space: a profile still to choose is no longer asked.
            c.pending = false;
        }
        if *self.export.borrow().settings.space() != before {
            self.space_changed();
        } else {
            self.export_changed(true);
        }
    }

    /// The name field: it renames the saved preset lit, else names the settings about to be saved.
    pub(super) fn preset_typed(&self, text: &str) {
        {
            let mut c = self.export.borrow_mut();
            match c.lit_kept() {
                // As typed, so the field is never written over under the hand; blank keeps the name.
                Some(i) if !text.trim().is_empty() => {
                    c.kept[i].name = text.to_owned();
                    c.keep_presets();
                }
                Some(_) => return,
                None => c.draft = text.to_owned(),
            }
        }
        self.publish_export();
    }

    /// The settings as they stand kept under the name typed, its key lit at once.
    pub(super) fn preset_save(&self) {
        {
            let mut c = self.export.borrow_mut();
            let name = c.draft.trim().to_owned();
            if name.is_empty() || c.lit().is_some() {
                return;
            }
            let settings = c.settings.clone();
            c.kept.push(Kept { name, settings });
            c.draft.clear();
            c.keep_presets();
        }
        self.publish_export();
    }

    /// The saved preset lit taken off the plate; the settings it wrote stay as they are.
    pub(super) fn preset_remove(&self) {
        {
            let mut c = self.export.borrow_mut();
            let Some(i) = c.lit_kept() else { return };
            c.kept.remove(i);
            c.keep_presets();
        }
        self.publish_export();
    }

    pub(super) fn publish_plate(&self, c: &Card, e: &Exporting) {
        let lit = c.lit();
        let saved = c.lit_kept();
        e.set_lit_preset(lit.map_or(-1, |i| i as i32));
        e.set_preset_rows(ModelRc::new(VecModel::from(c.rows())));
        e.set_preset_line(lit.is_none_or(|i| i >= Preset::ALL.len()));
        e.set_preset_saved(saved.is_some());
        e.set_preset_name(saved.map_or(c.draft.as_str(), |i| c.kept[i].name.as_str()).into());
        e.set_can_save_preset(saved.is_none() && !c.draft.trim().is_empty());
    }
}

/// The saved presets, read back by the checks.
pub fn kept(ctl: &Ctl) -> Vec<Kept> {
    ctl.export.borrow().kept.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_presets_read_back_as_written() {
        let mut client = Settings::default();
        client.set_format(Format::Tiff16);
        client.set_space(Space::AdobeRgb);
        let mut web = Settings::default();
        web.set_space(Space::Srgb);
        web.resize = Resize::LongestEdge(2048);
        web.jpeg_quality = 0.85;
        let kept = vec![Kept { name: "Client TIFF".into(), settings: client }, Kept { name: "Web 2048".into(), settings: web }];
        let value = to_value(&kept);
        assert_eq!(from_value(Some(&value)), kept);
        assert_eq!(value[1]["settings"]["resizePixels"], 2048, "the settings in their own preference's words");
        // Twin: an item without a name, or a value that is no list, reads as nothing, the rest kept.
        let mut broken = value.clone();
        broken[0]["name"] = Value::from("  ");
        assert_eq!(from_value(Some(&broken)), kept[1..]);
        assert!(from_value(Some(&Value::from("Client TIFF"))).is_empty());
        assert!(from_value(None).is_empty());
    }
}
