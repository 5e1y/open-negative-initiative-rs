//! Wiring the presets, and the shelf read back from its folder and published.

use super::*;

impl Ctl {
    pub(in crate::app) fn wire_presets(me: &Rc<Ctl>, window: &crate::MainWindow) {
        me.retire_shipped_presets();
        me.reload_presets();
        let g = window.global::<Presets>();
        g.set_tints(ModelRc::new(VecModel::from(Tint::ALL.iter().map(|t| colour(t.components())).collect::<Vec<_>>())));
        g.set_tint_names(ModelRc::new(VecModel::from(Tint::ALL.iter().map(|t| SharedString::from(t.label())).collect::<Vec<_>>())));
        g.set_carries_help(carries::CARRIES_HELP.into());
        g.set_balance_help(carries::WHITE_BALANCE_HELP.into());
        g.set_carries_nothing(carries::CARRIES_NOTHING.into());
        g.set_balance_lines(ModelRc::new(VecModel::from(WhiteBalance::ALL.iter().map(|b| line(b.label(), true, false)).collect::<Vec<_>>())));
        let index = |i: i32| usize::try_from(i).ok();
        let p = me.clone();
        g.on_hover(move |i| p.hover_preset(index(i)));
        let p = me.clone();
        g.on_apply(move |i| p.apply_preset_at(index(i)));
        let p = me.clone();
        g.on_create(move || p.save_preset());
        let p = me.clone();
        g.on_card_menu(move |i| p.preset_card_menu(index(i)));
        let p = me.clone();
        g.on_menu_chosen(move |i| p.preset_menu_chosen(index(i)));
        let p = me.clone();
        g.on_form_named(move |t| p.edit_form(|f| f.name = t.to_string()));
        let p = me.clone();
        g.on_form_noted(move |t| p.edit_form(|f| f.note = t.to_string()));
        let p = me.clone();
        g.on_form_tinted(move |i| p.edit_form(|f| f.tint = index(i).and_then(|i| Tint::ALL.get(i).copied()).unwrap_or_default()));
        let p = me.clone();
        g.on_form_ticked(move |i, on| {
            if let Some(entry) = index(i).and_then(entry_at) {
                p.edit_form(|f| f.parameters = carries::with_entry(entry, on, &f.parameters));
            }
        });
        let p = me.clone();
        g.on_form_balanced(move |i| {
            if let Some(choice) = index(i).and_then(|i| WhiteBalance::ALL.get(i).copied()) {
                p.edit_form(|f| f.parameters = carries::with_white_balance(choice, &f.parameters));
            }
        });
        let p = me.clone();
        g.on_save_cancelled(move || p.close_preset_form());
        let p = me.clone();
        g.on_save_confirmed(move || p.save_preset_confirmed());
        let p = me.clone();
        g.on_picked(move |i| p.preset_picked(index(i)));
        let p = me.clone();
        g.on_duplicate(move || p.editor_act(CardAct::Duplicate));
        let p = me.clone();
        g.on_share(move || p.editor_act(CardAct::Share));
        let p = me.clone();
        g.on_copy_code(move || p.editor_act(CardAct::CopyCode));
        let p = me.clone();
        g.on_delete(move || p.editor_act(CardAct::Delete));
        let p = me.clone();
        g.on_done(move || p.close_preset_form());
        let menus = window.global::<Menus>();
        let p = me.clone();
        menus.on_apply_preset(move |i| p.apply_preset_at(index(i)));
        let p = me.clone();
        menus.on_delete_preset(move |i| {
            let id = index(i).and_then(|i| p.state.borrow().shelf.held.get(i).map(|h| h.id));
            if let Some(id) = id {
                p.ask_delete_preset(id);
            }
        });
        let p = me.clone();
        window.global::<Advice>().on_closed(move || p.advice_closed());
    }

    pub(super) fn presets_folder(&self) -> PathBuf {
        presets::directory(&self.state.borrow().support)
    }

    /// `retireShipped()`: once per support folder, the presets an earlier version laid down and
    /// nobody changed go to the Trash before the deck first shows.
    pub(super) fn retire_shipped_presets(&self) {
        let support = self.state.borrow().support.clone();
        let mut prefs = Preferences::load(&support);
        let done = prefs.bool(presets::RETIRED_KEY).unwrap_or(false);
        if done {
            return;
        }
        let s = self.state.borrow();
        presets::retire_shipped(&presets::retired(), &presets::directory(&support), false, &mut |p: &Path| (s.discard)(p));
        drop(s);
        if let Err(e) = prefs.set(presets::RETIRED_KEY, true.into()) {
            self.say(&format!("The retired presets cannot be recorded: {e}"));
        }
    }

    /// `Propagation.load()`: the screen is what is on disk.
    pub(in crate::app) fn reload_presets(&self) {
        let support = self.state.borrow().support.clone();
        let held = presets::load_shelf(&support);
        {
            let mut s = self.state.borrow_mut();
            let shelf = &mut s.shelf;
            if shelf.hovered.is_some_and(|id| !held.iter().any(|p| p.id == id)) {
                shelf.hovered = None;
            }
            shelf.held = held;
        }
        self.publish_presets();
    }

    /// The deck, its hint and the Settings menu's list, from the shelf and the targets.
    pub(in crate::app) fn publish_presets(&self) {
        let Some(w) = self.window() else { return };
        let (cards, lines, names, hovered, count, title, open) = {
            let s = self.state.borrow();
            let targets = s.preset_targets();
            let title = match targets.as_slice() {
                [] => "Apply".to_string(),
                [only] => format!("Apply to \u{201c}{}\u{201d}", stem(only)),
                many => format!("Apply to {} Photos", many.len()),
            };
            let held = &s.shelf.held;
            (
                held.iter().map(card).collect::<Vec<_>>(),
                held.iter().map(|p| SharedString::from(format!("{} \u{b7} {}", p.name, carries::label(&p.parameters)))).collect::<Vec<_>>(),
                held.iter().map(|p| SharedString::from(p.name.as_str())).collect::<Vec<_>>(),
                s.shelf.hovered.and_then(|id| held.iter().position(|p| p.id == id)).map_or(-1, |i| i as i32),
                targets.len(),
                title,
                s.open.is_some(),
            )
        };
        let g = w.global::<Presets>();
        let fresh = Some((cards.clone(), lines.clone()));
        if self.state.borrow().shelf.published != fresh {
            g.set_cards(ModelRc::new(VecModel::from(cards)));
            let menus = w.global::<Menus>();
            menus.set_presets(ModelRc::new(VecModel::from(lines)));
            menus.set_preset_names(ModelRc::new(VecModel::from(names)));
            self.state.borrow_mut().shelf.published = fresh;
        }
        g.set_hovered(hovered);
        g.set_can_apply(count > 0);
        g.set_can_update(open);
        g.set_hint(if count > 1 { format!("Hover to preview \u{b7} click applies to {count} photos") } else { "Hover to preview \u{b7} click applies".into() }.into());
        w.global::<Menus>().set_apply_title(title.into());
    }
}
