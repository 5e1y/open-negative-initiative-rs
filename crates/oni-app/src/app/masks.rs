//! The Roll tab's mask library (`EditorScene+FlatField`, `FlatFieldBlock`): Use This Photo, Load…,
//! Rename, Folder, the offer to move a photo out of the library, and the sliders remembered.

use super::{Ctl, Picker, Shower, with_shell};
use crate::flat_fields::{self, Entry, Failure, Picture};
use crate::render::{self, Shared};
use crate::{App, Canvas, MaskDialog, PanelState};
use oni_core::Command;
use oni_settings::frame::{FlatField, FlatFieldMask};
use oni_settings::sidecar;
use slint::ComponentHandle;
use std::path::Path;
use std::rc::Rc;

/// `FlatFieldBlock.load`'s open panel message.
pub const LOAD_MESSAGE: &str = "Choose a picture of the empty light panel, shot as the negatives are.";

/// `offerToMoveOut`'s dialog.
pub const MOVE_OUT: (&str, &str) = (
    "Move This Photo Out of the Library?",
    "It is now a flat-field mask. The file moves to FlatField Presets in the app's folder, and its settings and thumbnail leave the library.",
);

/// What the open dialog goes ahead with once its key is pressed.
enum Asked {
    MoveOut(String),
    Rename { id: String, current: String },
}

/// The block's work under way, the dialog asked, and the system panels a check replaces.
pub(super) struct Masking {
    setting: bool,
    loading: bool,
    asked: Option<Asked>,
    chooser: Picker,
    show: Shower,
}

impl Masking {
    pub(super) fn new(chooser: Picker, show: Shower) -> Masking {
        Masking { setting: false, loading: false, asked: None, chooser, show }
    }
}

/// The mask's decode, through the photographs' own path and the reading its sidecar asks for.
fn picture(engine: &Shared, path: &Path) -> Option<Picture> {
    let mosaic = oni_decode::decodes_via_mosaic(path);
    let reading = sidecar::read(path, mosaic, false).map(|l| render::Reading::of(&l.settings)).unwrap_or_default();
    let decoded = render::decode(path, reading).ok()?;
    let linear = engine.lock().ok()?.preview_pixels(decoded).ok()?;
    Some((linear.width, linear.height, linear.pixels))
}

impl Ctl {
    pub(super) fn wire_masks(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let panel = window.global::<PanelState>();
        let p = me.clone();
        panel.on_take_mask(move || p.take_mask());
        let p = me.clone();
        panel.on_load_mask(move || p.load_mask());
        let p = me.clone();
        panel.on_rename_mask(move || p.rename_mask());
        let p = me.clone();
        panel.on_mask_folder(move || p.mask_folder());
        let p = me.clone();
        window.global::<MaskDialog>().on_answered(move |go, typed| p.mask_answered(go, typed.as_str()));
    }

    /// `FlatFieldActions`' keys: Use This Photo while a frame is open and none is being set, Load…
    /// while none loads, Rename while the frame names a loaded mask.
    pub(super) fn publish_masks(&self, panel: &PanelState, flat: &FlatField) {
        let (open, setting, loading) = {
            let s = self.state.borrow();
            (s.open.is_some(), s.masking.setting, s.masking.loading)
        };
        panel.set_can_take_mask(open && !setting);
        panel.set_taking_mask(setting);
        panel.set_can_load_mask(!loading);
        panel.set_loading_mask(loading);
        panel.set_can_rename_mask(flat.mask == FlatFieldMask::Custom && flat.custom.is_some());
    }

    fn masks_changed(&self) {
        let mut s = self.state.borrow_mut();
        s.masks = flat_fields::entries(&s.support);
    }

    // MARK: - The flat-field taken from a photograph

    /// `setFlatFieldFromPhoto`: the open frame decoded as a mask off this thread, then set as its
    /// own flat-field at the whole amount.
    fn take_mask(self: &Rc<Self>) {
        let (path, support) = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.open.clone().filter(|_| !s.masking.setting) else { return };
            s.masking.setting = true;
            // The decode reads the frame's sidecar: its passes and its encoding as the hand left them.
            let _ = s.store.flush();
            (path, s.support.clone())
        };
        self.publish_roll_tab();
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let result = flat_fields::load(&support, Path::new(&path), |p| picture(&engine, p));
            with_shell(move |c| c.mask_taken(&path, result));
        });
    }

    fn mask_taken(self: &Rc<Self>, path: &str, result: Result<Entry, Failure>) {
        self.state.borrow_mut().masking.setting = false;
        self.masks_changed();
        match result {
            Ok(entry) if self.state.borrow().open.as_deref() == Some(path) => {
                self.write_settings(|s| {
                    s.flat_field.mask = FlatFieldMask::Custom;
                    s.flat_field.custom = Some(entry.id.clone());
                    s.flat_field.amount = FlatField::default().amount;
                });
                self.offer_to_move_out(path);
            }
            Ok(_) => {}
            Err(failure) => self.advise("No Mask Set", failure.message()),
        }
        self.publish_roll_tab();
    }

    /// `FlatFieldBlock.load`: a picture of the empty panel chosen, decoded off this thread, then
    /// named as the frame's mask.
    fn load_mask(self: &Rc<Self>) {
        if self.state.borrow().masking.loading {
            return;
        }
        // Taken out first: a modal panel runs a nested event loop, which may reach the state.
        let chooser = std::mem::replace(&mut self.state.borrow_mut().masking.chooser, Box::new(|| None));
        let chosen = chooser();
        self.state.borrow_mut().masking.chooser = chooser;
        let Some(file) = chosen else { return };
        let support = {
            let mut s = self.state.borrow_mut();
            s.masking.loading = true;
            s.support.clone()
        };
        self.publish_roll_tab();
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let result = flat_fields::load(&support, &file, |p| picture(&engine, p));
            with_shell(move |c| c.mask_loaded(&file, result));
        });
    }

    fn mask_loaded(self: &Rc<Self>, file: &Path, result: Result<Entry, Failure>) {
        self.state.borrow_mut().masking.loading = false;
        self.masks_changed();
        match result {
            Ok(entry) => {
                self.write_settings(|s| {
                    s.flat_field.mask = FlatFieldMask::Custom;
                    s.flat_field.custom = Some(entry.id.clone());
                });
                self.offer_to_move_out(&oni_library::paths::canonical(file));
            }
            Err(failure) => self.advise("No Mask Loaded", failure.message()),
        }
        self.publish_roll_tab();
    }

    // MARK: - Moving the photo out

    /// `offerToMoveOut`: asked only of a library member.
    fn offer_to_move_out(&self, path: &str) {
        let member = {
            let s = self.state.borrow();
            match &s.index {
                Some(index) => index.all().iter().any(|e| e.path == path),
                None => s.rows.iter().any(|e| e.path == path),
            }
        };
        if member {
            self.ask_mask(MOVE_OUT.0, MOVE_OUT.1, None, "Keep in Library", "Move Out", Asked::MoveOut(path.to_string()));
        }
    }

    /// `moveOut`: the frame closed if open, the file moved to FlatField Presets, then its neighbour
    /// in the roll opened under the same flat-field, or the gallery when none is left.
    fn move_out(self: &Rc<Self>, path: &str) {
        let (was_open, neighbour, carried) = {
            let s = self.state.borrow();
            let was_open = s.open.as_deref() == Some(path);
            let open = s.open.as_deref().unwrap_or_default();
            let frames: Vec<&str> = s
                .archive
                .as_ref()
                .and_then(|a| a.groups.iter().find(|g| g.entries.iter().any(|e| e.path == open)))
                .map(|g| g.entries.iter().map(|e| e.path.as_str()).collect())
                .unwrap_or_default();
            let at = frames.iter().position(|p| *p == path);
            let neighbour = at.and_then(|i| frames.get(i + 1).or_else(|| i.checked_sub(1).and_then(|j| frames.get(j)))).map(|p| p.to_string());
            (was_open, neighbour, s.store.settings().flat_field.clone())
        };
        if was_open {
            let mut s = self.state.borrow_mut();
            if let Some(store) = s.store.editor() {
                store.close();
            }
            s.open = None;
            s.negative = None;
            s.full = None;
            s.previews.lock().expect("previews").remove(path);
        }
        let moved = {
            let s = self.state.borrow();
            flat_fields::move_out(path, &flat_fields::presets(&s.support), s.index.as_ref())
        };
        if moved.is_none() {
            self.say("The photo could not be moved to FlatField Presets.");
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            s.rows.retain(|e| e.path != path);
            s.order.retain(|p| p != path);
            s.picks.selection.remove(path);
        }
        self.lay_archive();
        self.publish_tiles();
        if !was_open {
            return;
        }
        match neighbour {
            Some(next) => {
                self.open_in_editor(&next);
                self.write_settings(|s| s.flat_field = carried);
            }
            None => {
                if let Some(w) = self.window() {
                    w.global::<App>().set_gallery(true);
                    w.global::<Canvas>().set_frame(slint::Image::default());
                }
                self.publish(true);
                self.publish_tiles();
            }
        }
    }

    // MARK: - Rename and Folder

    /// `renameActiveFlatField`: the frame's own mask renamed in the list, its id kept.
    fn rename_mask(&self) {
        let (id, current) = {
            let s = self.state.borrow();
            let f = &s.store.settings().flat_field;
            let Some(id) = f.custom.clone().filter(|_| f.mask == FlatFieldMask::Custom) else { return };
            let current = s.masks.iter().find(|e| e.id == id).map(|e| e.name.clone()).unwrap_or_default();
            (id, current)
        };
        self.ask_mask("Rename This Flat-Field Mask", "", Some(&current), "Cancel", "Rename", Asked::Rename { id, current: current.clone() });
    }

    /// `openFlatFieldFolder`: FlatField Presets, made when absent, in the Finder.
    fn mask_folder(&self) {
        let folder = flat_fields::presets(&self.state.borrow().support);
        if std::fs::create_dir_all(&folder).is_err() {
            return;
        }
        let show = std::mem::replace(&mut self.state.borrow_mut().masking.show, Box::new(|_| {}));
        show(&folder);
        self.state.borrow_mut().masking.show = show;
    }

    // MARK: - The dialog

    /// `DSDialogRequest` with a cancel of its own name and, for a rename, its field.
    fn ask_mask(&self, title: &str, message: &str, field: Option<&str>, cancel: &str, key: &str, asked: Asked) {
        self.state.borrow_mut().masking.asked = Some(asked);
        let Some(w) = self.window() else { return };
        let d = w.global::<MaskDialog>();
        d.set_title(title.into());
        d.set_message(message.into());
        d.set_field(field.is_some());
        d.set_text(field.unwrap_or_default().into());
        d.set_cancel(cancel.to_uppercase().into());
        d.set_key(key.to_uppercase().into());
        // Before the sheet lays its field, which then takes the keys itself.
        w.invoke_refocus();
        d.set_shown(true);
        self.publish_bar();
    }

    pub(super) fn mask_answered(self: &Rc<Self>, go: bool, typed: &str) {
        if let Some(w) = self.window() {
            w.global::<MaskDialog>().set_shown(false);
            w.invoke_refocus();
        }
        let asked = self.state.borrow_mut().masking.asked.take();
        match asked.filter(|_| go) {
            Some(Asked::MoveOut(path)) => self.move_out(&path),
            Some(Asked::Rename { id, current }) => {
                let name = typed.trim_matches([' ', '\t']);
                if !name.is_empty() && name != current {
                    flat_fields::rename(&self.state.borrow().support, &id, name);
                    self.masks_changed();
                    self.publish_roll_tab();
                }
            }
            None => {}
        }
        self.publish_bar();
    }

    /// Escape answers the sheet's cancel and Return its key, as `.cancelAction` and `.defaultAction` do.
    pub(super) fn mask_dialog_key(self: &Rc<Self>, text: &str) -> bool {
        let Some(w) = self.window() else { return false };
        let d = w.global::<MaskDialog>();
        if !d.get_shown() {
            return false;
        }
        match text {
            "\u{1b}" => self.mask_answered(false, ""),
            "\r" | "\n" => self.mask_answered(true, d.get_text().as_str()),
            _ => {}
        }
        true
    }

    // MARK: - The sliders remembered

    /// `FlatField.remember`: a hand's change of Amount, Temperature or Tint kept for the next frame.
    pub(super) fn remember_flat_field(&self, was: &FlatField) {
        let mut s = self.state.borrow_mut();
        let now = s.store.settings().flat_field.clone();
        if (was.amount, was.temperature, was.tint) == (now.amount, now.temperature, now.tint) {
            return;
        }
        if let Err(e) = flat_fields::remember(&mut s.preferences, &now) {
            crate::timing::line(&format!("the flat-field sliders could not be kept: {e}"));
        }
    }

    /// `withRemembered` on the frame being opened.
    pub(super) fn remembered_flat_field(&self) -> Option<[f64; 3]> {
        flat_fields::remembered(&self.state.borrow().preferences)
    }
}

/// Whether a command is a hand's own edit, which keeps the sliders: undo and redo replay a state.
pub(super) fn remembers(command: &Command) -> bool {
    !matches!(command, Command::Undo | Command::Redo)
}
