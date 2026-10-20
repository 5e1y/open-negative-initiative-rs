//! The deck: a hover previewing, a click applying, the card's menu.

use super::*;

impl Ctl {
    /// `dsHovered`: the card under the pointer previews; one naming the key measures the frame
    /// off this thread first, so the hover shows the balance the click writes.
    pub(super) fn hover_preset(self: &Rc<Self>, i: Option<usize>) {
        {
            let mut s = self.state.borrow_mut();
            let id = i.and_then(|i| s.shelf.held.get(i)).map(|p| p.id);
            if s.shelf.hovered == id {
                return;
            }
            s.shelf.hovered = id;
            // A new card asks the canvas again, whatever else stands still.
            s.requested_view = None;
        }
        self.measure_hover_balance();
        self.publish_presets();
        self.request_render();
    }

    /// The preview gone with the deck or under a sheet: no exit event reaches a card no longer shown.
    pub(in crate::app) fn clear_hover(&self) {
        {
            let mut s = self.state.borrow_mut();
            if s.shelf.hovered.is_none() {
                return;
            }
            s.shelf.hovered = None;
            s.requested_view = None;
        }
        self.publish_presets();
        // Asked from the loop, as a shelf write reaches here without the shell's handle.
        crate::app::with_shell(|c| c.request_render());
    }

    /// `measureHoverBalance`: the frame measured once on its settings, which no card changes, so
    /// every automatic card shares it; checked again as it lands, so the newest hover always wins.
    fn measure_hover_balance(self: &Rc<Self>) {
        let (path, through) = {
            let mut s = self.state.borrow_mut();
            let automatic = s.shelf.hovered.is_some_and(|id| s.shelf.held.iter().any(|p| p.id == id && oni_core::parameters::auto_balances(&p.parameters)));
            let Some(path) = s.open.clone().filter(|_| automatic && !s.shelf.measuring) else { return };
            let fresh = s.shelf.hover_balance.as_ref().is_some_and(|(on, through, _)| *on == path && through == s.store.settings());
            if fresh {
                return;
            }
            s.shelf.measuring = true;
            (path, s.store.settings().clone())
        };
        std::thread::spawn(move || {
            let outcome = crate::measuring::balance_outcomes(&[(path.clone(), through.clone())]).remove(&path);
            crate::app::with_shell(move |c| {
                let moved = {
                    let mut s = c.state.borrow_mut();
                    s.shelf.measuring = false;
                    // Dropped if a slider or the photo moved meanwhile: it would show a balance of settings gone.
                    let moved = s.store.settings() != &through || s.open.as_deref() != Some(path.as_str());
                    if let Some(o) = outcome.filter(|_| !moved) {
                        s.shelf.hover_balance = Some((path, through, o));
                        s.requested_view = None;
                    }
                    moved
                };
                // A frame that cannot be measured is not asked again until the hover moves.
                if moved {
                    c.measure_hover_balance();
                }
                c.request_render();
            });
        });
    }

    pub(super) fn apply_preset_at(self: &Rc<Self>, i: Option<usize>) {
        let (preset, targets) = {
            let s = self.state.borrow();
            let Some(p) = i.and_then(|i| s.shelf.held.get(i)).cloned() else { return };
            (p, s.preset_targets())
        };
        if targets.is_empty() {
            return;
        }
        self.journal_preset(&targets, &preset.name);
        self.gesture(&targets, |store| store.apply_preset(&preset, &targets, None));
        if !oni_core::balance::unbalanced(self.state.borrow().store.settings()) {
            self.disarm_window();
        }
        // A preset naming the key measures each target through the balance key's batch, Each, never asking.
        if oni_core::parameters::auto_balances(&preset.parameters) {
            self.run_auto_balance(oni_core::balance::Strategy::Each);
        }
        self.publish_presets();
        self.request_render();
    }

    pub(super) fn preset_card_menu(self: &Rc<Self>, i: Option<usize>) -> bool {
        let Some(w) = self.window() else { return false };
        let (id, name, open) = {
            let s = self.state.borrow();
            let Some(p) = i.and_then(|i| s.shelf.held.get(i)) else { return false };
            (p.id, p.name.clone(), s.open.is_some())
        };
        self.state.borrow_mut().shelf.menu = Some(id);
        let lines: Vec<MenuLine> = CARD_MENU
            .iter()
            .map(|(act, text)| match act {
                CardAct::Update => line(text, open, false),
                CardAct::Delete => line(&format!("Delete \u{201c}{name}\u{201d}"), true, true),
                _ => line(text, true, false),
            })
            .collect();
        w.global::<Presets>().set_menu_lines(ModelRc::new(VecModel::from(lines)));
        true
    }

    pub(super) fn preset_menu_chosen(self: &Rc<Self>, i: Option<usize>) {
        let id = self.state.borrow_mut().shelf.menu.take();
        let (Some(id), Some((act, _))) = (id, i.and_then(|i| CARD_MENU.get(i))) else { return };
        match act {
            CardAct::Edit => self.edit_presets(Some(id)),
            CardAct::Update => self.ask_update_preset(id),
            CardAct::Duplicate => {
                self.duplicate_preset(id);
            }
            CardAct::Share => self.share_preset(id),
            CardAct::CopyCode => self.copy_preset_code(id),
            CardAct::Delete => self.ask_delete_preset(id),
        }
    }

    pub(super) fn held(&self, id: Id) -> Option<Preset> {
        self.state.borrow().shelf.held.iter().find(|p| p.id == id).cloned()
    }
}
