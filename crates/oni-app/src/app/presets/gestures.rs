//! The shelf's gestures: save, duplicate, delete, share, import, the code and their dialogs.

use super::*;

impl Ctl {
    /// Through `save`, so an edit rewrites the file in place; a preset unchanged writes nothing.
    pub(super) fn commit_preset(&self, preset: &Preset) {
        if self.state.borrow().shelf.held.iter().any(|p| p == preset) {
            return;
        }
        match presets::save(preset, &self.presets_folder()) {
            Ok(()) => self.reload_presets(),
            Err(e) => self.say(&e.to_string()),
        }
    }

    pub(super) fn duplicate_preset(&self, id: Id) -> Option<Preset> {
        let copy = self.held(id)?.duplicated();
        if let Err(e) = presets::save(&copy, &self.presets_folder()) {
            self.say(&e.to_string());
            return None;
        }
        self.reload_presets();
        Some(copy)
    }

    pub(super) fn remove_preset(&self, id: Id) {
        let Some(preset) = self.held(id) else { return };
        if self.state.borrow().shelf.hovered == Some(id) {
            self.clear_hover();
        }
        let folder = self.presets_folder();
        let s = self.state.borrow();
        presets::delete(&preset, &folder, &mut |p: &Path| (s.discard)(p));
        drop(s);
        self.reload_presets();
    }

    /// `sharePreset`: the preset's own file, wherever the save panel says.
    pub(super) fn share_preset(&self, id: Id) {
        let Some(preset) = self.held(id) else { return };
        let Some(destination) = system::save_file(tr("Share Preset"), &presets::file_name(&preset), &["json"]) else { return };
        let written = presets::shareable(&preset).map_err(|e| e.to_string()).and_then(|t| std::fs::write(&destination, t).map_err(|e| e.to_string()));
        if let Err(e) = written {
            self.say(&e);
        }
    }

    /// `copyPresetCode`: the general clipboard on purpose, the code being for another person.
    pub(super) fn copy_preset_code(&self, id: Id) {
        let Some(preset) = self.held(id) else { return };
        match presets::share_code(&preset) {
            Ok(code) => system::set_text(&code),
            Err(e) => self.say(&e.to_string()),
        }
    }

    /// `pastePresetCode`: a code adopted as an import is, or a notice saying why not.
    pub(in crate::app) fn paste_preset_code(&self) {
        self.adopt_preset_code(&system::get_text().unwrap_or_default());
    }

    pub fn adopt_preset_code(&self, text: &str) {
        let Some(incoming) = presets::from_share_code(text) else {
            let message = if text.contains(presets::SHARE_CODE_PREFIX) {
                tr("The code is incomplete or was changed on the way. Ask for it to be copied again.").to_string()
            } else {
                tr!("Copy a code starting with {prefix}, then paste it here.", prefix = presets::SHARE_CODE_PREFIX)
            };
            return self.advise(tr("No Preset Code on the Clipboard"), &message);
        };
        let name = incoming.name.clone();
        let taken = presets::adopting(vec![incoming], &self.presets_folder());
        self.reload_presets();
        if taken.is_empty() {
            self.advise(&tr!("\u{201c}{name}\u{201d} Is Already in Your Presets", name = name), "");
        }
    }

    /// `importPresets`: files from the open panel, never overwriting, and a notice for those left out.
    pub(in crate::app) fn import_presets(&self) {
        let paths = system::pick_files(tr("Import Presets"), &["json"]);
        if !paths.is_empty() {
            self.import_preset_files(&paths);
        }
    }

    pub fn import_preset_files(&self, paths: &[PathBuf]) {
        let taken = presets::importing(paths, &self.presets_folder());
        self.reload_presets();
        let skipped = paths.len() - taken.len();
        if skipped == 0 {
            return;
        }
        let title = if taken.is_empty() { tr("No Preset Imported").to_string() } else { tr!("{taken} of {total} Presets Imported", taken = taken.len(), total = paths.len()) };
        let message = trn(
            "One file was left out: it is already in your presets, or it is not a preset.",
            "{n} files were left out: they are already in your presets, or they are not presets.",
            skipped as u64,
        );
        self.advise(&title, &message);
    }

    pub(in crate::app) fn show_presets_folder(&self) {
        let folder = self.presets_folder();
        let _ = std::fs::create_dir_all(&folder);
        system::reveal(&folder);
    }

    /// `PresetDialogs.request(updating: true)`: the stored settings replaced, the rest kept.
    pub(super) fn ask_update_preset(self: &Rc<Self>, id: Id) {
        let Some(p) = self.held(id) else { return };
        self.ask(
            &tr!("Replace \u{201c}{name}\u{201d} with This Photo\u{2019}s Settings?", name = p.name),
            tr("The settings it stored are replaced. Its name, color, description and the rows it carries stay as they are."),
            tr("Replace"),
            true,
            Pending::UpdatePreset(id),
        );
    }

    pub(super) fn ask_delete_preset(self: &Rc<Self>, id: Id) {
        let Some(p) = self.held(id) else { return };
        self.ask(
            &tr!("Delete the Preset \u{201c}{name}\u{201d}?", name = p.name),
            tr("Its file goes to the Trash, where it can be put back. Photos it was applied to keep their settings."),
            tr("Delete"),
            true,
            Pending::DeletePreset(id),
        );
    }

    pub(in crate::app) fn update_preset(&self, id: Id) {
        let Some(mut p) = self.held(id) else { return };
        p.settings = self.state.borrow().store.settings().clone();
        self.commit_preset(&p);
        self.refresh_working(&p);
    }

    /// A delete confirmed: from the editor, the neighbour is selected before the row goes.
    pub(in crate::app) fn delete_preset(&self, id: Id) {
        self.remove_preset(id);
        let editing = {
            let mut s = self.state.borrow_mut();
            match s.shelf.form.as_mut() {
                Some(Form::Editing { working, selected }) => {
                    working.retain(|p| p.id != id);
                    *selected = (!working.is_empty()).then_some(0);
                    true
                }
                _ => false,
            }
        };
        if editing {
            self.publish_form(true);
        }
    }

    /// The editor's copy follows a write made from the deck while it is open.
    pub(super) fn refresh_working(&self, preset: &Preset) {
        if let Some(Form::Editing { working, .. }) = self.state.borrow_mut().shelf.form.as_mut()
            && let Some(w) = working.iter_mut().find(|w| w.id == preset.id)
        {
            *w = preset.clone();
        }
    }

    pub(in crate::app) fn advise(&self, title: &str, message: &str) {
        let Some(w) = self.window() else { return };
        let a = w.global::<Advice>();
        a.set_title(title.into());
        a.set_message(message.into());
        a.set_shown(true);
    }

    pub(in crate::app) fn advice_closed(&self) {
        if let Some(w) = self.window() {
            w.global::<Advice>().set_shown(false);
        }
        self.refocus();
        self.publish_bar();
    }
}
