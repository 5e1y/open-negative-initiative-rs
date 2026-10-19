//! `menuState`: every menu item's title and whether a gesture can reach it now.

use super::*;

impl Ctl {
    /// `menuState`: every item's title and whether a gesture can reach it now, from the state the
    /// bar reads too, so the two surfaces cannot disagree.
    pub(super) fn publish_menus(&self) {
        let Some(w) = self.window() else { return };
        self.board_in();
        let app = w.global::<App>();
        let (gallery, library_hidden, panel_hidden) = (app.get_gallery(), app.get_library_hidden(), app.get_panel_hidden());
        let dedust = w.global::<PanelState>().get_tab() == 4;
        let (painting, canvas) = (self.painting(), self.canvas_size());
        let (titles, live, film_type, targets) = {
            let mut s = self.state.borrow_mut();
            let open = s.open.is_some();
            let selected = s.picks.selection.len();
            let trash = s.trash_shown() && gallery;
            let restoring = s.trash_picks.selection.len();
            let count = if selected > 0 { selected } else { usize::from(open) };
            let anchor = s.picks.anchor.is_some();
            // `modalOpen`: the export surface or any sheet, Report a Bug and the year's deck included.
            let sheet = w.get_sheet_shown() || s.shelf_form_open();
            let modal = self.export.borrow().shown || sheet;
            let presets = s.shelf.held.len();
            let members = s.roll_members(gallery).len();
            let preparing = s.prepare.shown;
            let (opened, read_done, filter, muted, columns) = (s.opened.is_some(), s.read_done, s.filter, s.muted, s.columns);
            let (split, screen_gamut, decoded) = (s.split.is_some(), s.screen_gamut, s.negative.is_some());
            // `zoomRung(by:)`: a step of the ladder, until the canvas is measured or at either end.
            let rung = |by: i32| match (s.full_size, canvas) {
                (Some(full), Some(view)) if !gallery && decoded => Viewport::rung(s.viewport.effective_scale(full, view), s.viewport.is_fitted(), by).is_some(),
                _ => false,
            };
            let (zoom_out, zoom_in) = (rung(-1), rung(1));
            let settings = s.store.settings().clone();
            let editor_undo = (s.store.can_undo(), s.store.can_redo());
            let open_path = s.open.clone();
            let store = s.store.editor();
            let gallery_history = store.as_ref().map(|e| {
                let h = e.gallery_history();
                (h.can_undo(), h.can_redo(), h.undo_label().map(str::to_string), h.redo_label().map(str::to_string))
            });
            let (has_settings, scanner_face) =
                store.as_ref().map_or((false, false), |e| (e.has_copied_settings(), e.scanner_face));
            let reach = settings
                .roll_reference
                .as_ref()
                .and_then(|r| store.as_ref().and_then(|e| e.measured_group(r.id)))
                .map_or(0, |m| m.len() - usize::from(open_path.as_ref().is_some_and(|p| m.contains(p))));
            let (can_undo, can_redo, undo_title, redo_title) = match (gallery, gallery_history) {
                (true, Some((u, r, ul, rl))) => {
                    (u, r, ul.map_or("Undo".into(), |l| format!("Undo {l}")), rl.map_or("Redo".into(), |l| format!("Redo {l}")))
                }
                (true, None) => (false, false, "Undo".to_string(), "Redo".to_string()),
                (false, _) => (editor_undo.0, editor_undo.1, "Undo".to_string(), "Redo".to_string()),
            };
            let can_reset = count > 0 && (count != 1 || !open || settings != oni_core::parameters::reset_blank(&settings, scanner_face));
            let (exporting, writing) = {
                let c = self.export.borrow();
                (c.shown.then_some(c.targets.len()), c.exporting)
            };
            let plural = |n: usize, one: &str, many: String| if n > 1 { many } else { one.to_string() };
            let titles = MenuTitles {
                mode: if gallery { "Editor" } else { "Gallery" }.into(),
                export: crate::export::label::menu(exporting.unwrap_or(selected)).into(),
                remove: plural(count, "Move to ONI Trash", format!("Move {count} Photos to ONI Trash")).into(),
                restore: plural(restoring, "Restore", format!("Restore {restoring} Photos")).into(),
                paste: plural(count, "Paste Settings", format!("Paste Settings to {count} Photos")).into(),
                paste_to_roll: if reach > 0 { format!("Paste to Roll ({} Photos)", reach + 1) } else { "Paste to Roll".into() }.into(),
                undo: undo_title.into(),
                redo: redo_title.into(),
                smaller: if gallery { "Smaller Thumbnails" } else { "Zoom Out" }.into(),
                larger: if gallery { "Larger Thumbnails" } else { "Zoom In" }.into(),
                sound: if muted { "Unmute Key Clicks" } else { "Mute Key Clicks" }.into(),
                compare: if split { "Close Comparison" } else { "Compare Before/After" }.into(),
                library: if library_hidden { "Show the Library" } else { "Hide the Library" }.into(),
                panel: if panel_hidden { "Show the Panel" } else { "Hide the Panel" }.into(),
                correcting: if dedust { "Done Correcting" } else { "Remove Dust & Scratches" }.into(),
                mirror: if settings.geometry.mirrored { "Unmirror" } else { "Mirror Horizontally" }.into(),
                filter: filter.map_or("Filter: All".into(), |m| format!("Filter: {}", library::medal(m))).into(),
                roll: plural(count, "New Roll from Selection", format!("New Roll from {count} Photos")).into(),
                ungroup: plural(members, "Ungroup", format!("Ungroup {members} Photos")).into(),
                sheet: sheets::title(s.making_sheet.is_some(), selected).into(),
                screen_gamut: if screen_gamut { "Hide What This Screen Cannot Show" } else { "Show What This Screen Cannot Show" }.into(),
            };
            // A gesture this build lacks stays dark, whatever Swift's rule would say.
            let live = MenuLive {
                open: true,
                toggle_mode: !gallery || anchor,
                // On the surface ⌘E confirms; elsewhere it opens on the selection, else the open photo.
                export: exporting.map_or(open || selected > 0, |n| n > 0 && !writing),
                recheck: read_done || !opened,
                read_folders: opened,
                show_selected: gallery && (open || selected > 0),
                remove: count > 0,
                restore: trash && restoring > 0,
                undo: can_undo,
                redo: can_redo,
                reset: can_reset,
                smaller: if gallery { gallery::column_step(columns, -1) != columns } else { zoom_out },
                larger: if gallery { gallery::column_step(columns, 1) != columns } else { zoom_in },
                compare: !gallery && decoded,
                hide_library: !gallery,
                hide_panel: !gallery,
                framing: open && !gallery,
                correction: open,
                bigger_brush: painting,
                smaller_brush: painting,
                mirror: open,
                screen_gamut: true,
                emulator: false,
                medal: selected > 0 && !modal,
                cycle_filter: true,
                new_roll: gallery && count > 0,
                ungroup: members > 0,
                contact_sheet: sheets::can_make(s.making_sheet.is_some(), selected),
                measure_roll: !preparing && count > 0,
                reset_to_default: gallery && count > 0,
                copy_settings: open,
                paste_settings: has_settings && count > 0,
                paste_to_roll: has_settings && open,
                save_preset: open,
                edit_presets: !modal && presets > 0,
                import_presets: !modal,
                show_presets: !modal,
                paste_preset_code: !modal,
                apply_preset: count > 0,
            };
            // `.roll` is neither the gallery nor the editor: their own gestures, and those on the editor it
            // hides, go dark there.
            let live = if preparing {
                MenuLive {
                    open: live.open,
                    recheck: live.recheck,
                    read_folders: live.read_folders,
                    screen_gamut: live.screen_gamut,
                    cycle_filter: live.cycle_filter,
                    edit_presets: live.edit_presets,
                    import_presets: live.import_presets,
                    show_presets: live.show_presets,
                    paste_preset_code: live.paste_preset_code,
                    ..MenuLive::default()
                }
            } else {
                live
            };
            // A sheet is modal: nothing behind it moves until it closes.
            let live = if sheet { MenuLive::default() } else { live };
            let film_type = ConversionMode::ALL.iter().position(|m| *m == settings.mode).unwrap_or(0) as i32;
            (titles, live, film_type, count)
        };
        let menus = w.global::<Menus>();
        menus.set_titles(titles);
        menus.set_live(live.clone());
        let panel = w.global::<PanelState>();
        panel.set_film_type(film_type);
        panel.set_targets(targets as i32);
        let bar = w.global::<Bar>();
        bar.set_can_paste(live.paste_settings);
        bar.set_can_sheet(live.contact_sheet);
        bar.set_sheet_title(menus.get_titles().sheet);
        bar.set_can_prepare(live.measure_roll);
        panel.set_can_prepare(true);
        self.publish_presets();
        self.publish_rolls();
        self.publish_roll_tab();
        bar.set_filter(menus.get_titles().filter.trim_start_matches("Filter: ").to_uppercase().into());
    }
}
