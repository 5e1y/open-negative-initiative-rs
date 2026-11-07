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
        let whole = self.roll_whole(gallery);
        let (titles, live, film_type, targets, filter, select_all) = {
            let mut s = self.state.borrow_mut();
            let open = s.open.is_some();
            let selected = s.picks.selection.len();
            let trash = s.trash_shown() && gallery;
            let restoring = s.trash_picks.selection.len();
            let count = s.target_count(gallery);
            // In the gallery with nothing picked, the editor opens on the photo it stands on and
            // the roll gestures act on the roll shown.
            let subject = s.editor_subject().is_some();
            let roll = s.roll_targets(gallery).len();
            // `modalOpen`: the export surface or any sheet, Report a Bug and the year's deck included.
            let sheet = w.get_sheet_shown() || s.shelf_form_open();
            let modal = self.export.borrow().shown || sheet;
            let presets = s.shelf.held.len();
            let members = s.roll_members(gallery).len();
            let preparing = s.prepare.shown;
            // The bar's ⌘A: the gallery's photos, or those beside the one open.
            let select_all = !modal && !preparing && if gallery { !s.order.is_empty() } else { open };
            let (opened, read_done, filter, muted, columns) = (s.opened.is_some(), s.read_done, s.filter, s.muted, s.columns);
            let (split, screen_gamut, decoded) = (s.compare.split.is_some(), s.screen_gamut, s.negative.is_some());
            // The crop and De-dust are each frame's own, out of reach while frames show together.
            let single = s.together.is_none();
            // `zoomRung(by:)`: a step of the ladder, until the canvas is measured or at either end.
            let rung = |by: i32| match (s.full_size, canvas) {
                (Some(full), Some(view)) if !gallery && decoded => s.viewport.rung(by, full, view).is_some(),
                _ => false,
            };
            let (zoom_out, zoom_in) = (rung(-1), rung(1));
            let settings = s.store.settings().clone();
            let open_path = s.open.clone();
            let mut store = s.store.editor();
            // A slide roll is scanned once for the title's count, then kept until a frame joins or leaves it.
            if let Some(e) = store.as_deref_mut()
                && let Some(key @ oni_library::roll_membership::RollKey::Slides(_)) = e.roll_key()
                && e.measured_group(key.clone()).is_none()
            {
                e.measured_with(key);
            }
            // One history in every mode: the same step answers ⌘Z in the gallery and the editor.
            let (can_undo, can_redo, undo_title, redo_title) = store.as_ref().map_or_else(
                || (false, false, tr("Undo").to_string(), tr("Redo").to_string()),
                |e| {
                    let h = e.history();
                    let name = |step| step_name(step, open_path.as_deref());
                    let undo = h.next_undo().and_then(name).map_or(tr("Undo").into(), |l| tr!("Undo {action}", action = l));
                    let redo = h.next_redo().and_then(name).map_or(tr("Redo").into(), |l| tr!("Redo {action}", action = l));
                    (h.can_undo(), h.can_redo(), undo, redo)
                },
            );
            let (has_settings, scanner_face) =
                store.as_ref().map_or((false, false), |e| (e.has_copied_settings(), e.scanner_face));
            let reach = store
                .as_ref()
                .and_then(|e| e.roll_key().and_then(|k| e.measured_group(k)))
                .map_or(0, |m| m.len() - usize::from(open_path.as_ref().is_some_and(|p| m.contains(p))));
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
                paste_to_roll: if reach > 0 { tr!("Paste to {count} Photos of the Roll", count = reach + 1) } else { tr("Paste to Roll").into() }.into(),
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
                filter: filter.map_or(tr("Filter: All Photos").into(), |m| tr!("Filter: {medal}", medal = library::medal(m))).into(),
                roll: if count > 1 { tr!("Move {count} Photos into a New Roll…", count = count) } else { tr("Move into a New Roll…").into() }.into(),
                ungroup: if members > 1 { tr!("Ungroup {count} Photos", count = members) } else { tr("Ungroup").into() }.into(),
                project: crate::organise::new_project(count).into(),
                screen_gamut: if screen_gamut { tr("Hide What This Screen Cannot Show") } else { tr("Show What This Screen Cannot Show") }.into(),
            };
            // A gesture this build lacks stays dark, whatever Swift's rule would say.
            let live = MenuLive {
                open: true,
                toggle_mode: !gallery || subject,
                // On the surface ⌘E confirms; elsewhere it opens on the selection, else the open photo.
                export: exporting.map_or(count > 0, |n| n > 0 && !writing),
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
                fit: !gallery && decoded,
                compare: !gallery && decoded,
                hide_library: !gallery,
                hide_panel: !gallery,
                framing: open && !gallery && single,
                correction: open && single,
                bigger_brush: painting,
                smaller_brush: painting,
                mirror: open && single,
                screen_gamut: true,
                emulator: open && !gallery,
                medal: selected > 0 && !modal,
                cycle_filter: true,
                new_roll: gallery && count > 0,
                new_project: count > 0 && !trash,
                ungroup: members > 0,
                measure_roll: !preparing && roll > 0,
                reset_to_default: gallery && roll > 0,
                // In the gallery, the one photo picked, as its menu's Copy Settings.
                copy_settings: !modal && if gallery { count == 1 } else { open },
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
            (titles, live, film_type, count, filter, select_all)
        };
        let menus = w.global::<Menus>();
        menus.set_titles(titles);
        menus.set_live(live.clone());
        let panel = w.global::<PanelState>();
        panel.set_film_type(film_type);
        panel.set_targets(targets as i32);
        let bar = w.global::<Bar>();
        bar.set_can_paste(live.paste_settings);
        bar.set_can_select_all(select_all);
        bar.set_whole(whole);
        bar.set_can_prepare(live.measure_roll);
        bar.set_can_toggle_mode(live.toggle_mode);
        panel.set_can_prepare(true);
        self.publish_presets();
        self.publish_rolls();
        self.publish_roll_tab();
        w.global::<Library>().set_filter_medal(filter.map_or(0, library::medal_index));
    }
}

/// A step's name: its gesture's own, else the one slider an edit moved, as the panel names it, read on
/// the open frame when the step wrote it; an edit that moved several values keeps none.
fn step_name(step: &oni_library::history::Step, open: Option<&str>) -> Option<String> {
    if !step.label.is_empty() {
        return Some(step.label.clone());
    }
    let ends = |p: &str| Some((step.before.get(p)?, step.after.get(p)?));
    let (before, after) = open.and_then(ends).or_else(|| step.before.keys().find_map(|p| ends(p)))?;
    let mut moved = Field::ALL.into_iter().filter(|f| f.knob().get(before) != f.knob().get(after));
    match (moved.next(), moved.next()) {
        (Some(one), None) => Some(tr(one.spec().label).to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_library::history::{Settings, Step};
    use oni_settings::PipelineSettings;

    fn step(label: &str, moved: &[(Field, f32)]) -> Step {
        let before = PipelineSettings::default();
        let after = moved.iter().fold(before.clone(), |s, (f, by)| f.knob().set(&s, f.knob().get(&s) + by));
        let one = |s: PipelineSettings| Settings::from([("a".to_owned(), s)]);
        Step { before: one(before), after: one(after), label: label.into(), disk: None, trash: None, marker: None, medals: None }
    }

    #[test]
    fn an_edit_is_named_by_the_one_slider_it_moved() {
        assert_eq!(step_name(&step("", &[(Field::Exposure, 0.5)]), Some("a")).as_deref(), Some("Exposure"));
        assert_eq!(step_name(&step("", &[(Field::Temperature, 0.3)]), None).as_deref(), Some("Temperature"), "read off its one frame");
        assert_eq!(step_name(&step("Reset 2 photos", &[(Field::Exposure, 0.5)]), Some("a")).as_deref(), Some("Reset 2 photos"));
        // Twins: an edit that moved two values, or none, keeps no name.
        assert_eq!(step_name(&step("", &[(Field::Exposure, 0.5), (Field::Contrast, 0.2)]), Some("a")), None);
        assert_eq!(step_name(&step("", &[]), Some("a")), None);
    }
}
