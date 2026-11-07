//! The presets' picker: the built-in presets, then the ones the hand saves, each a name and every
//! setting it writes, recalled whole by its line; the name sheet saves and renames them.

use super::*;
use crate::MenuLine;
use oni_i18n::tr;

/// The preference holding them, in the order saved.
pub const KEPT: &str = "exportPresets";

#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    pub name: String,
    pub settings: Settings,
}

/// What the name sheet names: the settings as they stand, or the saved preset at that place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Naming {
    Save,
    Rename(usize),
}

/// The lines after the presets', in the order the picker lays them.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Rename(usize),
    Remove(usize),
    Save,
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

/// A built-in preset's name as the picker shows it.
fn title(preset: Preset) -> &'static str {
    match preset {
        Preset::Instagram => tr("Instagram"),
        Preset::FullDisplay => tr("Display"),
        Preset::FullPrint => tr("Print"),
        Preset::Web => tr("Web"),
    }
}

/// What a preset writes, as its line and the picker at rest say it: format, space and size.
pub fn summary(settings: &Settings) -> String {
    let format = match settings.format() {
        Format::Tiff16 => tr("TIFF 16-bit"),
        Format::Jpeg => tr("JPEG"),
    };
    let size = match settings.resize {
        Resize::Full => tr("Full resolution").to_owned(),
        Resize::LongestEdge(n) => tr!("{n} px", n = n),
        Resize::ShortestEdge(n) => tr!("{n} px on the short edge", n = n),
    };
    format!("{format} · {} · {size}", settings.space().title())
}

fn line(text: String, note: String, enabled: bool, rule: bool) -> MenuLine {
    MenuLine { text: text.into(), note: note.into(), enabled, rule, ..MenuLine::default() }
}

impl Card {
    /// The saved preset the settings equal.
    fn lit_kept(&self) -> Option<usize> {
        self.kept.iter().position(|k| k.settings == self.settings)
    }

    /// The preset the settings equal, read off the state: a saved one first, as its name is the
    /// hand's own, else a built-in one; saved ones stand after the built-in ones.
    pub(super) fn lit(&self) -> Option<usize> {
        let built_in = || Preset::matching(&self.settings).and_then(|p| Preset::ALL.iter().position(|q| *q == p));
        self.lit_kept().map(|i| Preset::ALL.len() + i).or_else(built_in)
    }

    /// Every preset's name and its settings, built-in then saved.
    fn presets(&self) -> Vec<(String, Settings)> {
        let built_in = Preset::ALL.iter().map(|p| (title(*p).to_owned(), p.applied(&self.settings)));
        built_in.chain(self.kept.iter().map(|k| (k.name.clone(), k.settings.clone()))).collect()
    }

    /// What follows the presets: renaming and removing the saved one lit, then saving the settings,
    /// out of reach while they equal a preset already.
    fn actions(&self) -> Vec<Action> {
        let saved = self.lit_kept();
        saved.map(Action::Rename).into_iter().chain(saved.map(Action::Remove)).chain([Action::Save]).collect()
    }

    fn lines(&self) -> Vec<MenuLine> {
        let presets = self.presets().into_iter().enumerate().map(|(i, (name, settings))| line(name, summary(&settings), true, i == Preset::ALL.len()));
        let actions = self.actions().into_iter().enumerate().map(|(i, action)| {
            let (text, enabled) = match action {
                Action::Rename(at) => (tr!("Rename “{name}”…", name = self.kept[at].name), true),
                Action::Remove(at) => (tr!("Remove “{name}”", name = self.kept[at].name), true),
                Action::Save => (tr("Save as Preset…").to_owned(), self.lit().is_none()),
            };
            line(text, String::new(), enabled, i == 0)
        });
        presets.chain(actions).collect()
    }

    fn keep_presets(&mut self) {
        self.kept_saver.ask(to_value(&self.kept));
    }
}

impl Ctl {
    /// A line of the picker: a preset's whole state written, or what saves, renames or removes one.
    pub(super) fn preset_chosen(self: &Rc<Self>, at: usize) {
        let count = Preset::ALL.len() + self.export.borrow().kept.len();
        if at >= count {
            let action = self.export.borrow().actions().get(at - count).copied();
            match action {
                Some(Action::Save) => self.naming(Naming::Save),
                Some(Action::Rename(i)) => self.naming(Naming::Rename(i)),
                Some(Action::Remove(i)) => self.preset_remove(i),
                None => {}
            }
            return;
        }
        let before = self.export.borrow().settings.space().clone();
        {
            let mut c = self.export.borrow_mut();
            c.settings = match Preset::ALL.get(at) {
                Some(preset) => preset.applied(&c.settings),
                None => c.kept[at - Preset::ALL.len()].settings.clone(),
            };
            // The preset wrote a space: a profile still to choose is no longer asked.
            c.pending = false;
        }
        if *self.export.borrow().settings.space() != before {
            self.space_changed();
        } else {
            self.export_changed(true);
        }
    }

    /// The name sheet opened on the name it changes, or on none for settings about to be saved.
    fn naming(&self, naming: Naming) {
        {
            let mut c = self.export.borrow_mut();
            if naming == Naming::Save && c.lit().is_some() {
                return;
            }
            c.draft = match naming {
                Naming::Rename(i) => c.kept[i].name.clone(),
                Naming::Save => String::new(),
            };
            c.naming = Some(naming);
        }
        self.publish_export();
    }

    /// The name sheet's field, as typed, so it is never written over under the hand.
    pub(super) fn preset_typed(&self, text: &str) {
        self.export.borrow_mut().draft = text.to_owned();
        self.publish_export();
    }

    /// The name sheet's commit: the settings kept under the name typed, lit at once, or the saved
    /// preset renamed; a blank name keeps the sheet open.
    pub(super) fn preset_save(&self) {
        {
            let mut c = self.export.borrow_mut();
            let name = c.draft.trim().to_owned();
            if name.is_empty() {
                return;
            }
            match c.naming {
                Some(Naming::Rename(i)) => c.kept[i].name = name,
                Some(Naming::Save) if c.lit().is_none() => {
                    let settings = c.settings.clone();
                    c.kept.push(Kept { name, settings });
                }
                _ => return,
            }
            c.naming = None;
            c.draft.clear();
            c.keep_presets();
        }
        self.naming_closed();
    }

    /// The name sheet closed with nothing kept.
    pub(super) fn preset_cancel(&self) {
        {
            let mut c = self.export.borrow_mut();
            c.naming = None;
            c.draft.clear();
        }
        self.naming_closed();
    }

    /// The sheet gone with the field that held the keyboard: the key monitor takes it back.
    fn naming_closed(&self) {
        self.publish_export();
        if let Some(w) = self.window() {
            w.invoke_refocus();
        }
    }

    /// A saved preset taken out of the picker; the settings it wrote stay as they are.
    fn preset_remove(&self, at: usize) {
        {
            let mut c = self.export.borrow_mut();
            c.kept.remove(at);
            c.keep_presets();
        }
        self.publish_export();
    }

    pub(super) fn publish_plate(&self, c: &Card, e: &Exporting) {
        let lit = c.lit();
        let presets = c.presets();
        e.set_lit_preset(lit.map_or(-1, |i| i as i32));
        let lines = c.lines();
        let shown = e.get_preset_lines();
        if shown.row_count() != lines.len() || lines.iter().enumerate().any(|(i, l)| shown.row_data(i).as_ref() != Some(l)) {
            e.set_preset_lines(ModelRc::new(VecModel::from(lines)));
        }
        let at_rest = lit.map(|i| presets[i].clone());
        e.set_preset_title(at_rest.as_ref().map_or(tr("No preset"), |(name, _)| name.as_str()).into());
        e.set_preset_summary(at_rest.map_or_else(String::new, |(_, s)| summary(&s)).into());
        e.set_naming_shown(c.naming.is_some());
        let (title, commit, note) = match c.naming {
            Some(Naming::Rename(i)) => (tr("Rename Preset"), tr("Rename"), summary(&c.kept[i].settings)),
            _ => (tr("Save as Preset"), tr("Save"), tr!("Keeps every export setting as it stands: {summary}.", summary = summary(&c.settings))),
        };
        e.set_naming_title(title.into());
        e.set_naming_commit(commit.into());
        e.set_naming_note(note.into());
        e.set_naming_name(c.draft.as_str().into());
        e.set_can_name(!c.draft.trim().is_empty());
    }
}

/// The saved presets, read back by the checks.
pub fn kept(ctl: &Ctl) -> Vec<Kept> {
    ctl.export.borrow().kept.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn web() -> Settings {
        let mut web = Settings::default();
        web.set_space(Space::Srgb);
        web.resize = Resize::LongestEdge(2048);
        web
    }

    #[test]
    fn saved_presets_read_back_as_written() {
        let mut client = Settings::default();
        client.set_format(Format::Tiff16);
        client.set_space(Space::AdobeRgb);
        let kept = vec![Kept { name: "Client TIFF".into(), settings: client }, Kept { name: "Web 2048".into(), settings: web() }];
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

    #[test]
    fn a_summary_names_the_format_the_space_and_the_size() {
        assert_eq!(summary(&Preset::Web.applied(&Settings::default())), "JPEG · sRGB · 2048 px");
        assert_eq!(summary(&Preset::FullPrint.applied(&Settings::default())), "TIFF 16-bit · Adobe RGB · Full resolution");
        let mut short = web();
        short.resize = Resize::ShortestEdge(1440);
        assert_eq!(summary(&short), "JPEG · sRGB · 1440 px on the short edge");
        // Twin: the quality is not among what the summary says, so two qualities read alike.
        let mut lower = Preset::Web.applied(&Settings::default());
        lower.jpeg_quality = 0.5;
        assert_eq!(summary(&lower), summary(&Preset::Web.applied(&Settings::default())));
    }

    /// 1.5.6 (eac39604) reads `preferences.json` whole as an object, its export settings with the
    /// decoder this build keeps unchanged, and writes it back with every key it does not know.
    #[test]
    fn the_presets_file_still_reads_in_1_5_6() {
        let dir = std::env::temp_dir().join(format!("oni-app-kept-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut prefs = Preferences::load(&dir);
        let saved = vec![Kept { name: "Web 2048".into(), settings: web() }];
        prefs.set(KEPT, to_value(&saved)).unwrap();
        prefs.set(oni_export::settings::PREFERENCE_KEY, Value::from(web().raw_value())).unwrap();
        let file: Value = serde_json::from_slice(&std::fs::read(Preferences::file_path(&dir)).unwrap()).unwrap();
        assert!(file.is_object(), "a file that is no object 1.5.6 sets aside whole");
        let read = file[oni_export::settings::PREFERENCE_KEY].as_str().and_then(Settings::from_raw_value);
        assert_eq!(read, Some(web()), "1.5.6 opens on the settings written");
        assert!(!file.to_string().contains("\"Web\""), "no built-in preset is written by its name, which 1.5.6 lacks");
        Preferences::load(&dir).set("soundMuted", Value::from(true)).unwrap();
        assert_eq!(from_value(Preferences::load(&dir).value(KEPT)), saved, "a key 1.5.6 writes keeps the presets");
        // Twin: the settings named by a preset in place of their own words read as nothing in 1.5.6.
        assert_eq!(Settings::from_raw_value("\"Web\""), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
