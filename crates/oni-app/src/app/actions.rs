//! The one place bar keys, menu items and shortcuts meet: `act`, and what the app keeps for the next launch.

use super::*;

impl Ctl {
    // MARK: - Gestures

    /// Bar keys, menu items and shortcuts meet here; what this build does not carry says so.
    pub fn act(self: &Rc<Self>, action: &str) {
        let Some(w) = self.window() else { return };
        let app = w.global::<App>();
        match action {
            "undo" | "redo" if app.get_gallery() => self.gallery_step(action == "undo"),
            "prepare-roll" => self.prepare_roll(),
            "leave-roll" => self.leave_roll(),
            "toggle-mode" if self.state.borrow().prepare.shown => {}
            "undo" => self.write(Command::Undo),
            "redo" => self.write(Command::Redo),
            "reset" => self.reset_all(),
            "toggle-mode" if self.export.borrow().shown => {}
            "export" => self.export_act(action),
            "toggle-mode" => self.toggle_mode(),
            "hide-panel" => app.set_panel_hidden(!app.get_panel_hidden()),
            "hide-library" => app.set_library_hidden(!app.get_library_hidden()),
            "smaller" | "larger" if app.get_gallery() => {
                let by = if action == "larger" { 1 } else { -1 };
                let columns = gallery::column_step(self.state.borrow().columns, by);
                self.state.borrow_mut().columns = columns;
                self.remember(preferences::GALLERY_COLUMNS, columns.into());
                self.relayout();
            }
            "smaller" | "larger" => self.step_zoom(if action == "larger" { 1 } else { -1 }),
            "compare" if !app.get_gallery() && self.state.borrow().open.is_some() => self.toggle_compare(),
            "screen-gamut" => {
                self.toggle_screen_gamut();
                self.gamut_kept();
            }
            "bigger-brush" | "smaller-brush" if self.painting() => {
                // `EditorScene.biggerBrush`: four pixels a key, inside the brush's range.
                let by = if action == "bigger-brush" { 4.0 } else { -4.0 };
                let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
                {
                    let mut s = self.state.borrow_mut();
                    s.brush = (f64::from(s.brush) + by).clamp(low, high) as f32;
                }
                self.keep_brush();
                self.publish_dedust();
            }
            "bigger-brush" | "smaller-brush" => {}
            "correction" if self.state.borrow().open.is_some() => self.toggle_correcting(),
            "framing" if self.state.borrow().open.is_some() => self.open_crop(),
            "mirror" => self.framing_act("mirror"),
            "toggle-sound" => {
                let muted = !self.state.borrow().muted;
                self.state.borrow_mut().muted = muted;
                self.remember(preferences::SOUND_MUTED, muted.into());
                self.publish_settings();
            }
            "full-decode" => {
                let whole = !crate::render::FULL_DECODE.load(std::sync::atomic::Ordering::Relaxed);
                crate::render::FULL_DECODE.store(whole, std::sync::atomic::Ordering::Relaxed);
                self.remember(preferences::FULL_DECODE, whole.into());
                self.publish_settings();
            }
            "keep-zoom" => {
                let keep = !crate::viewport::KEEP_ZOOM.load(std::sync::atomic::Ordering::Relaxed);
                crate::viewport::KEEP_ZOOM.store(keep, std::sync::atomic::Ordering::Relaxed);
                self.remember(preferences::KEEP_ZOOM, keep.into());
                self.publish_settings();
            }
            "cycle-filter" => self.cycle_filter(),
            "show-selected" => self.show_selected(),
            "copy" => {
                if let Some(store) = self.state.borrow_mut().store.editor() {
                    store.copy_settings();
                }
                self.board_out(crate::pasteboard::SETTINGS);
            }
            "paste" => self.paste(),
            "paste-to-roll" => self.paste_to_roll(),
            "copy-framing" => {
                if let Some(store) = self.state.borrow_mut().store.editor() {
                    store.copy_framing();
                }
                self.board_out(crate::pasteboard::FRAMING);
                self.publish_framing();
            }
            "paste-framing" | "paste-framing-to-roll" => self.paste_framing(action == "paste-framing-to-roll"),
            "remove" => self.move_to_trash(),
            "restore" => {
                let picked: Vec<String> = {
                    let s = self.state.borrow();
                    s.order.iter().filter(|p| s.trash_picks.selection.contains(*p)).cloned().collect()
                };
                self.restore(&picked);
            }
            "new-roll" if app.get_gallery() => self.new_roll(),
            "contact-sheet" => self.make_contact_sheet(),
            "reset-to-default" if app.get_gallery() => self.ask_reset_to_default(),
            "ungroup" => self.ungroup(),
            "confirm-yes" | "confirm-no" => self.answered(action == "confirm-yes"),
            "open" => self.pick(),
            "recheck" | "read-folders" => self.read_again(),
            "auto-balance" => self.auto_balance(),
            "select-roll-type" => self.select_roll_type(),
            "save-preset" => self.save_preset(),
            "edit-presets" => self.edit_presets(None),
            "import-presets" => self.import_presets(),
            "paste-preset-code" => self.paste_preset_code(),
            "show-presets" => self.show_presets_folder(),
            "console" => self.open_console(),
            "welcome" => self.open_welcome(),
            "whats-new" => self.show_whats_new_again(),
            "report" => self.open_report(),
            "check-updates" | "update" => self.check_for_updates(true),
            "wrapped" => self.show_wrapped(oni_library::wrapped::today().0),
            "pair-phone" => self.pair_phone(),
            "settings" => self.open_settings(),
            "frontier-export" => self.export_frontier_look(),
            "frontier-choose" => self.choose_frontier_look(),
            "frontier-measured" => self.measured_frontier(),
            "close-settings" => self.close_settings(),
            "close-pairing" => self.close_pairing(),
            "forget-phones" => self.forget_phones(),
            a if a.starts_with("medal-") => {
                if let Ok(m) = a[6..].parse::<i32>() {
                    self.label(m);
                }
            }
            a if self.lab_act(a) => {}
            other => self.say(&format!("“{other}” is not in this build yet")),
        }
        self.publish_bar();
    }

    /// A preference written for the next launch; a failed write is said, the session going on.
    pub(super) fn remember(&self, key: &str, value: serde_json::Value) {
        let written = self.state.borrow_mut().preferences.set(key, value);
        if let Err(e) = written {
            self.say(&format!("The preference cannot be kept: {e}"));
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
        let Some(message) = failure else { return slint::CloseRequestResponse::HideWindow };
        self.ask(
            "Your latest settings could not be saved",
            &format!("{message}\n\nQuitting now discards the changes made since the last successful save. Cancel to keep the app open and fix the problem \u{2014} for instance by freeing space or reconnecting the volume the originals live on."),
            "Quit Anyway",
            false,
            Pending::QuitAnyway,
        );
        slint::CloseRequestResponse::KeepWindowShown
    }
}
