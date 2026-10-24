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
        let (titles, live, film_type, targets, filter_word) = {
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
            let (split, screen_gamut, decoded) = (s.compare.split.is_some(), s.screen_gamut, s.negative.is_some());
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
                    (u, r, ul.map_or(tr("Undo").into(), |l| tr!("Undo {action}", action = l)), rl.map_or(tr("Redo").into(), |l| tr!("Redo {action}", action = l)))
                }
                (true, None) => (false, false, tr("Undo").to_string(), tr("Redo").to_string()),
                (false, _) => (editor_undo.0, editor_undo.1, tr("Undo").to_string(), tr("Redo").to_string()),
            };
            let can_reset = count > 0 && (count != 1 || !open || settings != oni_core::parameters::reset_blank(&settings, scanner_face));
            let (exporting, writing) = {
                let c = self.export.borrow();
                (c.shown.then_some(c.targets.len()), c.exporting)
            };
            let titles = MenuTitles {
                mode: if gallery { tr("Editor") } else { tr("Gallery") }.into(),
                export: crate::export::label::menu(exporting.unwrap_or(selected)).into(),
                remove: if count > 1 { tr!("Move {count} Photos to ONI Trash", count = count) } else { tr("Move to ONI Trash").into() }.into(),
                restore: if restoring > 1 { tr!("Restore {count} Photos", count = restoring) } else { tr("Restore").into() }.into(),
                paste: if count > 1 { tr!("Paste Settings to {count} Photos", count = count) } else { tr("Paste Settings").into() }.into(),
                paste_to_roll: if reach > 0 { tr!("Paste to Roll ({count} Photos)", count = reach + 1) } else { tr("Paste to Roll").into() }.into(),
                undo: undo_title.into(),
                redo: redo_title.into(),
                smaller: if gallery { tr("Smaller Thumbnails") } else { tr("Zoom Out") }.into(),
                larger: if gallery { tr("Larger Thumbnails") } else { tr("Zoom In") }.into(),
                sound: if muted { tr("Unmute Key Clicks") } else { tr("Mute Key Clicks") }.into(),
                compare: if split { tr("Close Comparison") } else { tr("Compare Before/After") }.into(),
                library: if library_hidden { tr("Show the Library") } else { tr("Hide the Library") }.into(),
                panel: if panel_hidden { tr("Show the Panel") } else { tr("Hide the Panel") }.into(),
                correcting: if dedust { tr("Done Correcting") } else { tr("Remove Dust & Scratches") }.into(),
                mirror: if settings.geometry.mirrored { tr("Unmirror") } else { tr("Mirror Horizontally") }.into(),
                filter: filter.map_or(tr("Filter: All").into(), |m| tr!("Filter: {medal}", medal = library::medal(m))).into(),
                roll: if count > 1 { tr!("New Roll from {count} Photos", count = count) } else { tr("New Roll from Selection").into() }.into(),
                ungroup: if members > 1 { tr!("Ungroup {count} Photos", count = members) } else { tr("Ungroup").into() }.into(),
                sheet: sheets::title(s.making_sheet.is_some(), selected).into(),
                screen_gamut: if screen_gamut { tr("Hide What This Screen Cannot Show") } else { tr("Show What This Screen Cannot Show") }.into(),
            };
            // A gesture this build lacks stays dark, whatever Swift's rule would say.
            let live = MenuLive {
                open: true,
                toggle_mode: !gallery || anchor,
                // On the surface ⌘E confirms; elsewhere it opens on the selection, else the open photo.
                export: exporting.map_or(open || selected > 0, |n| n > 0 && !writing),
                // A read under way is the refresh already.
                refresh: read_done || !opened,
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
                emulator: open && !gallery,
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
                save_preset: open && !modal,
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
                    refresh: live.refresh,
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
            let filter_word = filter.map_or(tr("ALL").to_string(), |m| library::medal(m).to_uppercase());
            (titles, live, film_type, count, filter_word)
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
        bar.set_filter(filter_word.into());
    }
}
