//! The one place bar keys, menu items and shortcuts meet: `act`, and what the app keeps for the next launch.

use super::*;

impl Ctl {
    // MARK: - Gestures

    /// Bar keys, menu items and shortcuts meet here.
    pub fn act(self: &Rc<Self>, action: Act) {
        let Some(w) = self.window() else { return };
        let app = w.global::<App>();
        match action {
            Act::Undo | Act::Redo if app.get_gallery() => self.gallery_step(action == Act::Undo),
            Act::PrepareRoll => self.prepare_roll(),
            Act::LeaveRoll => self.leave_roll(),
            Act::ToggleMode if self.state.borrow().prepare.shown => {}
            Act::Undo => self.write(Command::Undo),
            Act::Redo => self.write(Command::Redo),
            Act::Reset => self.reset_all(),
            Act::ToggleMode if self.export.borrow().shown => {}
            Act::Export => self.export_act(),
            Act::ToggleMode => self.toggle_mode(),
            Act::HidePanel => app.set_panel_hidden(!app.get_panel_hidden()),
            Act::HideLibrary => app.set_library_hidden(!app.get_library_hidden()),
            Act::Smaller | Act::Larger if app.get_gallery() => {
                let by = if action == Act::Larger { 1 } else { -1 };
                let columns = gallery::column_step(self.state.borrow().columns, by);
                self.state.borrow_mut().columns = columns;
                self.remember(preferences::GALLERY_COLUMNS, columns.into());
                self.relayout();
            }
            Act::Smaller | Act::Larger => self.step_zoom(if action == Act::Larger { 1 } else { -1 }),
            Act::Compare if !app.get_gallery() && self.state.borrow().open.is_some() => self.toggle_compare(),
            Act::ScreenGamut => {
                self.toggle_screen_gamut();
                self.gamut_kept();
                self.publish_settings();
            }
            Act::BiggerBrush | Act::SmallerBrush if self.painting() => {
                // `EditorScene.biggerBrush`: four pixels a key, inside the brush's range.
                let by = if action == Act::BiggerBrush { 4.0 } else { -4.0 };
                let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
                {
                    let mut s = self.state.borrow_mut();
                    s.brush = (f64::from(s.brush) + by).clamp(low, high) as f32;
                }
                self.keep_brush();
                self.publish_dedust();
            }
            Act::BiggerBrush | Act::SmallerBrush => {}
            Act::Correction if self.state.borrow().open.is_some() => self.toggle_correcting(),
            Act::Framing if self.state.borrow().open.is_some() => self.open_crop(),
            Act::Mirror => self.framing_act(action),
            Act::ToggleSound => {
                let muted = !self.state.borrow().muted;
                self.state.borrow_mut().muted = muted;
                self.remember(preferences::SOUND_MUTED, muted.into());
                self.publish_settings();
            }
            Act::FullDecode => {
                let whole = !crate::render::FULL_DECODE.load(std::sync::atomic::Ordering::Relaxed);
                crate::render::FULL_DECODE.store(whole, std::sync::atomic::Ordering::Relaxed);
                self.remember(preferences::FULL_DECODE, whole.into());
                self.publish_settings();
            }
            Act::KeepZoom => {
                let keep = !crate::viewport::KEEP_ZOOM.load(std::sync::atomic::Ordering::Relaxed);
                crate::viewport::KEEP_ZOOM.store(keep, std::sync::atomic::Ordering::Relaxed);
                self.remember(preferences::KEEP_ZOOM, keep.into());
                self.publish_settings();
            }
            Act::CycleFilter => self.cycle_filter(),
            Act::ShowSelected => self.show_selected(),
            Act::Copy => {
                if let Some(store) = self.state.borrow_mut().store.editor() {
                    store.copy_settings();
                }
                self.board_out(crate::pasteboard::SETTINGS);
            }
            Act::Paste => self.paste(),
            Act::PasteToRoll => self.paste_to_roll(),
            Act::CopyFraming => {
                if let Some(store) = self.state.borrow_mut().store.editor() {
                    store.copy_framing();
                }
                self.board_out(crate::pasteboard::FRAMING);
                self.publish_framing();
            }
            Act::PasteFraming | Act::PasteFramingToRoll => self.paste_framing(action == Act::PasteFramingToRoll),
            Act::Remove => self.move_to_trash(),
            Act::Restore => {
                let picked: Vec<String> = {
                    let s = self.state.borrow();
                    s.order.iter().filter(|p| s.trash_picks.selection.contains(*p)).cloned().collect()
                };
                self.restore(&picked);
            }
            Act::NewRoll if app.get_gallery() => self.new_roll(),
            Act::ContactSheet => self.make_contact_sheet(),
            Act::ResetToDefault if app.get_gallery() => self.ask_reset_to_default(),
            Act::Ungroup => self.ungroup(),
            Act::Open => self.pick(),
            Act::Recheck | Act::ReadFolders => self.read_again(),
            Act::AutoBalance => self.auto_balance(),
            Act::SelectRollType => self.select_roll_type(),
            Act::SavePreset => self.save_preset(),
            Act::EditPresets => self.edit_presets(None),
            Act::ImportPresets => self.import_presets(),
            Act::PastePresetCode => self.paste_preset_code(),
            Act::ShowPresets => self.show_presets_folder(),
            Act::Console => self.open_console(),
            Act::DesignSystem => self.open_board(),
            Act::Welcome => self.open_welcome(),
            Act::WhatsNew => self.show_whats_new_again(),
            Act::Report => self.open_report(),
            Act::CheckUpdates | Act::Update => self.check_for_updates(true),
            Act::Wrapped => self.show_wrapped(oni_library::wrapped::today().0),
            Act::PairPhone => self.pair_phone(),
            Act::Settings => self.open_settings(),
            Act::FrontierExport => self.export_frontier_look(),
            Act::FrontierChoose => self.choose_frontier_look(),
            Act::FrontierMeasured => self.measured_frontier(),
            Act::CloseSettings => self.close_settings(),
            Act::ClosePairing => self.close_pairing(),
            Act::ForgetPhones => self.forget_phones(),
            Act::MedalNone => self.label(0),
            Act::MedalBronze => self.label(1),
            Act::MedalSilver => self.label(2),
            Act::MedalGold => self.label(3),
            Act::FaceScanner | Act::FacePro | Act::Emulator => self.lab_act(action),
            Act::Rotate | Act::MirrorVertically | Act::ResetCrop | Act::WindowArm | Act::WindowCopy | Act::WindowPaste | Act::WindowReset => self.framing_act(action),
            // A gesture its moment does not allow.
            Act::Compare | Act::Correction | Act::Framing | Act::NewRoll | Act::ResetToDefault => {}
        }
        self.publish_bar();
    }

    /// A preference written for the next launch; a failed write is said, the session going on.
    pub(super) fn remember(&self, key: &str, value: serde_json::Value) {
        let written = self.state.borrow_mut().preferences.set(key, value);
        if let Err(e) = written {
            self.say(&tr!("The preference cannot be kept: {e}", e = e));
        }
    }

    /// `QuitGuard.applicationShouldTerminate`: the sidecar written before the window goes; a write
    /// that fails asks first, Cancel keeping the app open to free space or reconnect the volume.
    pub(super) fn close_requested(self: &Rc<Self>) -> slint::CloseRequestResponse {
        // A roll in preparation keeps its current step, which otherwise only a step change writes.
        self.flush_drafts();
        if self.state.borrow().quitting {
            return slint::CloseRequestResponse::HideWindow;
        }
        let failure = self.state.borrow_mut().store.flush();
        // Edits kept from a refused write are lost on quitting too, so they ask the same.
        let kept = self.state.borrow().store.editor_ref().and_then(|e| e.unsaved().first().map(|u| u.message.clone()));
        let Some(message) = failure.or(kept) else { return slint::CloseRequestResponse::HideWindow };
        self.ask(
            tr("Your latest settings could not be saved"),
            &tr!("{message}\n\nQuitting now discards the changes made since the last successful save. Cancel to keep the app open and fix the problem \u{2014} for instance by freeing space or reconnecting the volume the originals live on.", message = message),
            tr("Quit Anyway"),
            false,
            Pending::QuitAnyway,
        );
        slint::CloseRequestResponse::KeepWindowShown
    }
}
