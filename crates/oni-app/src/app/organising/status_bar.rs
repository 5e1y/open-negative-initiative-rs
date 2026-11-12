//! The status bar: what loads, else the selection, and the medal picker for the gallery and beside a photo.

use super::*;

impl Ctl {
    pub(in crate::app) fn publish_status(&self) {
        let Some(w) = self.window() else { return };
        let (leading, target, editor_target, seals) = {
            let s = self.state.borrow();
            // The photos the grid lays, on whichever surface shows: the archive, a project, the ONI Trash.
            let shown = s.order.len();
            // `statusSeal`: the word of the tile's seal, for the one frame the bar names when it leads passes.
            let seal = |editing: bool| {
                let path = picking::status_path(&s.picks.selection, editing, s.open.as_deref());
                path.filter(|p| s.entry(p).is_some_and(|e| e.leads_passes)).map_or("", |_| "RGB")
            };
            let seals = (seal(false), seal(true));
            let label = |p: &str| s.entry(p).map_or(Medal::None, |e| e.label);
            // `statusMedalTarget` beside a photo: the open frame stands in for an empty selection.
            let editing = picking::status_path(&s.picks.selection, true, s.open.as_deref());
            let editor_target = picking::medal_target(editing.as_deref(), &s.picks.selection, &label);
            if s.trash_shown() {
                (picking::status_leading(Some((s.trash_picks.selection.len(), s.org.trashed.len())), None, 0, shown), None, editor_target, ("", seals.1))
            } else {
                let path = picking::status_path(&s.picks.selection, false, s.open.as_deref());
                let status = path.as_deref().map(|p| picking::status_label(p, None));
                let mut leading = picking::status_leading(None, status, s.picks.selection.len(), shown);
                if let Some(missing) = s.missing_words() {
                    leading = format!("{leading} · {missing}");
                }
                (leading, picking::medal_target(path.as_deref(), &s.picks.selection, &label), editor_target, seals)
            }
        };
        let lib = w.global::<Library>();
        lib.set_status(leading.into());
        lib.set_status_seal(seals.0.into());
        lib.set_editor_seal(seals.1.into());
        lib.set_medal_shown(target.is_some());
        lib.set_medal_current(target.and_then(|t| t.0).map_or(-1, library::medal_index));
        lib.set_editor_medal_shown(editor_target.is_some());
        lib.set_editor_medal_current(editor_target.and_then(|t| t.0).map_or(-1, library::medal_index));
    }

    /// `Settings saved` after the editor's line: the open frame's sidecar holds what it shows, so an
    /// edit hides it until its write.
    pub(in crate::app) fn publish_saved(&self) {
        let Some(w) = self.window() else { return };
        let kept = self.state.borrow().store.kept();
        w.global::<crate::Canvas>().set_saved(kept);
    }

    /// `DSMedalPicker`'s tap: one photo toggles its own medal off, several all take it.
    pub(in crate::app) fn medal_picked(self: &Rc<Self>, medal: i32) {
        let editing = self.window().is_some_and(|w| !w.global::<App>().get_gallery());
        let target = {
            let s = self.state.borrow();
            let path = picking::status_path(&s.picks.selection, editing, s.open.as_deref());
            let label = |p: &str| s.entry(p).map_or(Medal::None, |e| e.label);
            picking::medal_target(path.as_deref(), &s.picks.selection, &label)
        };
        let Some((current, paths)) = target else { return };
        let applied = picking::medal_tapped(current, paths.len(), library::medal_of(medal));
        self.label_paths(&paths, applied);
        self.publish_bar();
    }

    /// A tile's medal key under the hand: that photo alone, whatever is selected, its own medal coming off.
    pub(in crate::app) fn tile_medal_picked(self: &Rc<Self>, index: usize, medal: i32) {
        let target = {
            let s = self.state.borrow();
            s.order.get(index).map(|p| (p.clone(), s.entry(p).map_or(Medal::None, |e| e.label)))
        };
        let Some((path, current)) = target else { return };
        let applied = picking::medal_tapped(Some(current), 1, library::medal_of(medal));
        self.label_paths(&[path], applied);
        self.publish_bar();
    }
}
