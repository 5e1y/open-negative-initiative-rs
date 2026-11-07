//! The save sheet and the editor: the form, its pause-driven write, its keys.

use super::*;

impl Ctl {
    /// `savePreset`: the sheet opens on the first free name and the rows the photo moved; never
    /// over an open sheet, whose edit it would drop.
    pub(in crate::app) fn save_preset(self: &Rc<Self>) {
        if self.state.borrow().open.is_none() || self.state.borrow().shelf_form_open() {
            return;
        }
        self.clear_hover();
        {
            let mut s = self.state.borrow_mut();
            let names: Vec<&str> = s.shelf.held.iter().map(|p| p.name.as_str()).collect();
            let settings = s.store.settings().clone();
            let draft = Preset::new(&presets::default_name(&names), carries::proposed(&settings), Tint::default(), settings);
            s.shelf.form = Some(Form::Saving(draft));
        }
        self.publish_form(true);
        self.publish_bar();
    }

    /// Copy Settings: the sheet on the boxes the last copy carried, the look's all at first, for the
    /// photo `from` or the open one, never over an open sheet; told not to ask again, those at once.
    pub(in crate::app) fn ask_copy(self: &Rc<Self>, from: Option<String>) {
        if (from.is_none() && self.state.borrow().open.is_none()) || self.state.borrow().shelf_form_open() {
            return;
        }
        let (rows, at_once) = {
            let s = self.state.borrow();
            let whole = carries::pasted_rows();
            let rows = s.preferences.value(crate::preferences::COPY_CARRIES).and_then(serde_json::Value::as_array);
            let remembered: BTreeSet<String> =
                rows.into_iter().flatten().filter_map(serde_json::Value::as_str).filter(|r| whole.contains(*r)).map(str::to_string).collect();
            (if remembered.is_empty() { whole } else { remembered }, s.shelf.copy_at_once)
        };
        if at_once {
            return self.copy(rows, from);
        }
        self.clear_hover();
        let draft = Preset::new("", rows, Tint::default(), PipelineSettings::default());
        self.state.borrow_mut().shelf.form = Some(Form::Copying { draft, from, at_once: false });
        self.publish_form(true);
        self.publish_bar();
    }

    /// The copy laid on the board with `rows`, which the next copy opens on.
    fn copy(self: &Rc<Self>, rows: BTreeSet<String>, from: Option<String>) {
        self.remember(crate::preferences::COPY_CARRIES, rows.iter().cloned().collect::<Vec<_>>().into());
        let failure = {
            let mut s = self.state.borrow_mut();
            s.store.editor().and_then(|store| {
                store.copy_settings(from.as_deref(), &rows);
                store.failure.take()
            })
        };
        match failure {
            Some(message) => self.told(tr("Copy Settings"), &message),
            None => self.board_out(crate::pasteboard::SETTINGS),
        }
        self.publish_bar();
    }

    /// The copy sheet's commit: nothing while no box is ticked; its "Don't ask again" kept till quit.
    fn copy_confirmed(self: &Rc<Self>, rows: BTreeSet<String>, from: Option<String>, at_once: bool) {
        if rows.is_empty() {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            s.shelf.form = None;
            s.shelf.copy_at_once = at_once;
        }
        self.publish_form(false);
        self.refocus();
        self.copy(rows, from);
    }

    /// The sheet's commit: one preset, one file, one write, laid on the settings the photo has now;
    /// or the copy.
    pub(super) fn save_preset_confirmed(self: &Rc<Self>) {
        let copy = match self.state.borrow().shelf.form.as_ref() {
            Some(Form::Copying { draft, from, at_once }) => Some((draft.parameters.clone(), from.clone(), *at_once)),
            _ => None,
        };
        if let Some((rows, from, at_once)) = copy {
            return self.copy_confirmed(rows, from, at_once);
        }
        let draft = match self.state.borrow().shelf.form.as_ref() {
            Some(Form::Saving(d)) => d.clone(),
            _ => return,
        };
        let name = draft.name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let settings = self.state.borrow().store.settings().clone();
        let preset = Preset::new(&name, draft.parameters, draft.tint, settings);
        self.state.borrow_mut().shelf.form = None;
        self.publish_form(false);
        self.refocus();
        match presets::save(&preset, &self.presets_folder()) {
            Ok(()) => self.reload_presets(),
            Err(e) => self.say(&e.to_string()),
        }
        self.publish_bar();
    }

    /// `editingPresets`: the editor on a copy of the shelf, selecting `initial` or the first.
    pub(in crate::app) fn edit_presets(self: &Rc<Self>, initial: Option<Id>) {
        self.clear_hover();
        {
            let mut s = self.state.borrow_mut();
            let working = s.shelf.held.clone();
            let selected = initial.and_then(|id| working.iter().position(|p| p.id == id)).or((!working.is_empty()).then_some(0));
            s.shelf.form = Some(Form::Editing { working, selected });
        }
        self.publish_form(true);
        self.publish_bar();
    }

    pub(super) fn preset_picked(self: &Rc<Self>, i: Option<usize>) {
        let changed = {
            let s = self.state.borrow();
            matches!(s.shelf.form.as_ref(), Some(Form::Editing { selected, working }) if *selected != i && i.is_some_and(|i| i < working.len()))
        };
        if !changed {
            return;
        }
        // Leaving a preset writes it at once: a pending write would be dropped by the next row.
        self.settle_selected();
        if let Some(Form::Editing { selected, .. }) = self.state.borrow_mut().shelf.form.as_mut() {
            *selected = i;
        }
        self.publish_form(true);
    }

    pub(super) fn settle_selected(&self) {
        let folder = self.presets_folder();
        let preset = {
            let mut s = self.state.borrow_mut();
            s.shelf.serial += 1;
            let Some(Form::Editing { working, selected: Some(i) }) = s.shelf.form.as_mut() else { return };
            let Some(p) = working.get_mut(*i) else { return };
            p.name = presets::writable_name(p, &folder);
            p.clone()
        };
        self.commit_preset(&preset);
    }

    /// The footer's keys act on the selected preset, the deck's on the card pointed at.
    pub(super) fn editor_act(self: &Rc<Self>, act: CardAct) {
        let id = match self.state.borrow().shelf.form.as_ref() {
            Some(Form::Editing { working, selected: Some(i) }) => working.get(*i).map(|p| p.id),
            _ => None,
        };
        let Some(id) = id else { return };
        match act {
            CardAct::Duplicate => {
                self.settle_selected();
                if let Some(copy) = self.duplicate_preset(id) {
                    if let Some(Form::Editing { working, selected }) = self.state.borrow_mut().shelf.form.as_mut() {
                        working.push(copy);
                        *selected = Some(working.len() - 1);
                    }
                    self.publish_form(true);
                }
            }
            CardAct::Share => self.share_preset(id),
            CardAct::CopyCode => {
                self.settle_selected();
                self.copy_preset_code(id);
            }
            CardAct::Delete => self.ask_delete_preset(id),
            CardAct::Edit | CardAct::Update => {}
        }
    }

    /// An edit of the form: the draft takes it, and the editor writes once the hand rests.
    pub(super) fn edit_form(self: &Rc<Self>, change: impl FnOnce(&mut Preset)) {
        let serial = {
            let mut s = self.state.borrow_mut();
            let shelf = &mut s.shelf;
            let Some(form) = shelf.form.as_mut() else { return };
            let Some(current) = form.current() else { return };
            change(current);
            shelf.serial += 1;
            matches!(form, Form::Editing { .. }).then_some(shelf.serial)
        };
        self.publish_form(false);
        if let Some(serial) = serial {
            let me = Rc::downgrade(self);
            // `Sidecar.writeDelay`, shared so the two never drift.
            slint::Timer::single_shot(WRITE_DELAY, move || {
                let Some(c) = me.upgrade() else { return };
                if c.state.borrow().shelf.serial != serial {
                    return;
                }
                let preset = match c.state.borrow().shelf.form.as_ref() {
                    Some(Form::Editing { working, selected: Some(i) }) => working.get(*i).cloned(),
                    _ => None,
                };
                // Written as typed: a blank name is settled only when the preset is left.
                if let Some(p) = preset.filter(|p| !p.name.trim().is_empty()) {
                    c.commit_preset(&p);
                }
            });
        }
    }

    /// Done, Cancel or Escape: the editor writes what it holds first.
    pub(super) fn close_preset_form(self: &Rc<Self>) {
        if matches!(self.state.borrow().shelf.form, Some(Form::Editing { .. })) {
            self.settle_selected();
        }
        self.state.borrow_mut().shelf.form = None;
        self.publish_form(false);
        self.refocus();
        self.publish_bar();
    }

    /// A sheet gone takes its field's focus with it: the window's keys get the keyboard back.
    pub(super) fn refocus(&self) {
        if let Some(w) = self.window() {
            w.invoke_refocus();
        }
    }

    /// Escape and Return on a sheet: Cancel, and Save or Done.
    pub(in crate::app) fn preset_form_key(self: &Rc<Self>, text: &str) -> bool {
        let saving = match self.state.borrow().shelf.form {
            None => return false,
            Some(Form::Saving(_) | Form::Copying { .. }) => true,
            Some(Form::Editing { .. }) => false,
        };
        match text {
            "\u{1b}" => self.close_preset_form(),
            "\r" | "\n" if saving => self.save_preset_confirmed(),
            "\r" | "\n" => self.close_preset_form(),
            _ => return false,
        }
        true
    }

    /// The sheets' state; `fresh` lays the form's fields again from the preset, as a new row does.
    pub(super) fn publish_form(&self, fresh: bool) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Presets>();
        let mut s = self.state.borrow_mut();
        let (saving, editing) = match s.shelf.form {
            Some(Form::Saving(_) | Form::Copying { .. }) => (true, false),
            Some(Form::Editing { .. }) => (false, true),
            None => (false, false),
        };
        let copying = s.shelf.form.as_ref().is_some_and(Form::copying);
        g.set_copy_at_once(matches!(s.shelf.form, Some(Form::Copying { at_once: true, .. })));
        g.set_saving(saving);
        g.set_editing(editing);
        g.set_copying(copying);
        g.set_carries_help(tr(if copying { carries::COPY_CARRIES_HELP } else { carries::CARRIES_HELP }).into());
        if let Some(Form::Editing { working, selected }) = s.shelf.form.as_ref() {
            if let Some(fresh) = refreshed(&g.get_rows(), working.iter().map(card).collect()) {
                g.set_rows(fresh);
            }
            g.set_selected(selected.map_or(-1, |i| i as i32));
        }
        let current = s.shelf.form.as_mut().and_then(|f| f.current()).cloned();
        g.set_form_shown(current.is_some());
        let Some(p) = current else {
            g.set_form_instance(ModelRc::new(VecModel::from(Vec::<i32>::new())));
            drop(s);
            if !editing {
                self.publish_presets();
            }
            return;
        };
        g.set_form_name(p.name.clone().into());
        // A copy takes no name: it is ready while it carries something.
        g.set_form_valid(if copying { !p.parameters.is_empty() } else { !p.name.trim().is_empty() });
        g.set_form_note(p.note.clone().into());
        g.set_form_tint(Tint::ALL.iter().position(|t| *t == p.tint).unwrap_or(0) as i32);
        if let Some(fresh) = refreshed(&g.get_form_carries(), carries_lines(&p.parameters, copying)) {
            g.set_form_carries(fresh);
        }
        g.set_form_balance(WhiteBalance::ALL.iter().position(|b| *b == carries::white_balance(&p.parameters)).unwrap_or(0) as i32);
        g.set_form_empty(p.parameters.is_empty());
        if fresh {
            g.set_form_instance(ModelRc::new(VecModel::from(vec![s.shelf.serial as i32])));
        }
    }
}

/// A list laid again row by row when its length holds, so a keystroke keeps the rows' hover and
/// scroll; a new model, returned, only when rows come or go.
fn refreshed<T: Clone + PartialEq + 'static>(model: &ModelRc<T>, rows: Vec<T>) -> Option<ModelRc<T>> {
    if model.row_count() != rows.len() {
        return Some(ModelRc::new(VecModel::from(rows)));
    }
    for (i, row) in rows.into_iter().enumerate() {
        if model.row_data(i).as_ref() != Some(&row) {
            model.set_row_data(i, row);
        }
    }
    None
}
