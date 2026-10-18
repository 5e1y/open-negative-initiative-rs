//! The deck: a hover previewing, a click applying, the card's menu.

use super::*;

impl Ctl {
    /// `dsHovered`: the card under the pointer previews; one naming the key measures the frame
    /// off this thread first, so the hover shows the balance the click writes.
    pub(super) fn hover_preset(self: &Rc<Self>, i: Option<usize>) {
        let measure = {
            let mut s = self.state.borrow_mut();
            let id = i.and_then(|i| s.shelf.held.get(i)).map(|p| p.id);
            if s.shelf.hovered == id {
                return;
            }
            s.shelf.hovered = id;
            // A new card asks the canvas again, whatever else stands still.
            s.requested_view = None;
            let automatic = id.filter(|id| s.shelf.held.iter().any(|p| p.id == *id && oni_core::parameters::auto_balances(&p.parameters)));
            let stale = |id: Id| s.shelf.hover_balance.as_ref().is_none_or(|(h, through, _)| *h != id || through != s.store.settings());
            match (automatic, s.open.clone()) {
                (Some(id), Some(path)) if !s.shelf.measuring && stale(id) => {
                    let through = s.store.settings().clone();
                    s.shelf.measuring = true;
                    Some((id, path, through))
                }
                _ => None,
            }
        };
        if let Some((id, path, through)) = measure {
            std::thread::spawn(move || {
                let outcome = crate::measuring::balance_outcomes(&[(path.clone(), through.clone())]).remove(&path);
                crate::app::with_shell(move |c| {
                    let landed = {
                        let mut s = c.state.borrow_mut();
                        s.shelf.measuring = false;
                        // Dropped if a slider moved meanwhile: it would show a balance of settings gone.
                        match outcome.filter(|_| s.store.settings() == &through) {
                            Some(o) => {
                                s.shelf.hover_balance = Some((id, through, o));
                                s.requested_view = None;
                                s.shelf.hovered == Some(id)
                            }
                            None => false,
                        }
                    };
                    if landed {
                        c.request_render();
                    }
                });
            });
        }
        self.publish_presets();
        self.request_render();
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
        // A preset naming the key measures each target through the balance key's own batch.
        if oni_core::parameters::auto_balances(&preset.parameters) {
            self.act("auto-balance");
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
