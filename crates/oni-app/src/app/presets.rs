//! The presets in the window (`PresetCarousel`, `PresetSaveSheet`, `PresetEditor`, the Settings
//! menu's list): the shelf read back from its folder after every write, a hover previewing on the
//! canvas, a click applying to the targets as one gallery step, and the two sheets.

use super::{Ctl, Pending, WRITE_DELAY, stem};
use crate::preferences::Preferences;
use crate::{Advice, CarriesLine, MenuLine, Menus, PresetCard, Presets, system};
use oni_core::balance::{self, Outcome};
use oni_core::preset_parameters::{self as carries, WhiteBalance};
use oni_core::presets::{self, Preset, Tint};
use oni_settings::{Id, PipelineSettings};
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Default)]
pub(super) struct Shelf {
    pub held: Vec<Preset>,
    /// The card under the pointer: kept out of the settings, so a hover never writes one.
    pub hovered: Option<Id>,
    /// What the hovered automatic preset measures, with the settings it was measured on.
    hover_balance: Option<(Id, PipelineSettings, Outcome)>,
    measuring: bool,
    menu: Option<Id>,
    form: Option<Form>,
    /// Raised by every edit in the editor; a write armed under an older count is dropped.
    serial: u64,
    /// The deck and the menu's list as last handed over: a new model would drop the hovered card.
    published: Option<(Vec<PresetCard>, Vec<SharedString>)>,
}

enum Form {
    Saving(Preset),
    Editing { working: Vec<Preset>, selected: Option<usize> },
}

impl Form {
    fn current(&mut self) -> Option<&mut Preset> {
        match self {
            Form::Saving(draft) => Some(draft),
            Form::Editing { working, selected } => selected.and_then(|i| working.get_mut(i)),
        }
    }
}

/// What the deck's right click offers, in `PresetCarousel.card`'s order.
#[derive(Clone, Copy)]
enum CardAct {
    Edit,
    Update,
    Duplicate,
    Share,
    CopyCode,
    Delete,
}

const CARD_MENU: [(CardAct, &str); 6] = [
    (CardAct::Edit, "Edit…"),
    (CardAct::Update, "Update with Current Settings"),
    (CardAct::Duplicate, "Duplicate"),
    (CardAct::Share, "Share Preset…"),
    (CardAct::CopyCode, "Copy Preset Code"),
    (CardAct::Delete, "Delete"),
];

fn colour(c: [f64; 3]) -> Color {
    let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::from_rgb_u8(byte(c[0]), byte(c[1]), byte(c[2]))
}

fn card(p: &Preset) -> PresetCard {
    PresetCard {
        abbreviation: p.abbreviation().into(),
        name: p.name.clone().into(),
        help: p.help().into(),
        carries: carries::label(&p.parameters).into(),
        fill: colour(p.tint.components()),
        ink: if p.tint.wants_dark_ink() { Color::from_rgb_u8(0, 0, 0) } else { Color::from_rgb_u8(255, 255, 255) },
    }
}

/// The Carries picker's lines: each block's title, the balance choice under Color, then its boxes.
fn carries_lines(parameters: &std::collections::BTreeSet<String>) -> Vec<CarriesLine> {
    let mut lines = Vec::new();
    for block in &carries::BLOCKS {
        lines.push(CarriesLine { kind: 0, text: block.title.into(), on: false });
        if block.id == "Colour" {
            lines.push(CarriesLine { kind: 1, text: carries::white_balance(parameters).label().into(), on: false });
        }
        for entry in block.entries {
            lines.push(CarriesLine { kind: 2, text: entry.label.into(), on: carries::is_on(entry, parameters) });
        }
    }
    lines
}

/// The box a Carries line stands for, `None` for a title or the balance choice.
fn entry_at(line: usize) -> Option<&'static carries::Entry> {
    let mut at = 0;
    for block in &carries::BLOCKS {
        at += 1 + usize::from(block.id == "Colour");
        for entry in block.entries {
            if at == line {
                return Some(entry);
            }
            at += 1;
        }
    }
    None
}

fn line(text: &str, enabled: bool, rule: bool) -> MenuLine {
    MenuLine { text: text.into(), enabled, rule, opens: false }
}

impl Ctl {
    pub(super) fn wire_presets(me: &Rc<Ctl>, window: &crate::MainWindow) {
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

    fn presets_folder(&self) -> PathBuf {
        presets::directory(&self.state.borrow().support)
    }

    /// `retireShipped()`: once per support folder, the presets an earlier version laid down and
    /// nobody changed go to the Trash before the deck first shows.
    fn retire_shipped_presets(&self) {
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
    pub(super) fn reload_presets(&self) {
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
    pub(super) fn publish_presets(&self) {
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

    // MARK: - The deck

    /// `dsHovered`: the card under the pointer previews; one naming the key measures the frame
    /// off this thread first, so the hover shows the balance the click writes.
    fn hover_preset(self: &Rc<Self>, i: Option<usize>) {
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
                super::with_shell(move |c| {
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

    fn apply_preset_at(self: &Rc<Self>, i: Option<usize>) {
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

    fn preset_card_menu(self: &Rc<Self>, i: Option<usize>) -> bool {
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

    fn preset_menu_chosen(self: &Rc<Self>, i: Option<usize>) {
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

    fn held(&self, id: Id) -> Option<Preset> {
        self.state.borrow().shelf.held.iter().find(|p| p.id == id).cloned()
    }

    // MARK: - The shelf's gestures

    /// Through `save`, so an edit rewrites the file in place; a preset unchanged writes nothing.
    fn commit_preset(&self, preset: &Preset) {
        if self.state.borrow().shelf.held.iter().any(|p| p == preset) {
            return;
        }
        match presets::save(preset, &self.presets_folder()) {
            Ok(()) => self.reload_presets(),
            Err(e) => self.say(&e.to_string()),
        }
    }

    fn duplicate_preset(&self, id: Id) -> Option<Preset> {
        let copy = self.held(id)?.duplicated();
        if let Err(e) = presets::save(&copy, &self.presets_folder()) {
            self.say(&e.to_string());
            return None;
        }
        self.reload_presets();
        Some(copy)
    }

    fn remove_preset(&self, id: Id) {
        let Some(preset) = self.held(id) else { return };
        if self.state.borrow().shelf.hovered == Some(id) {
            self.state.borrow_mut().shelf.hovered = None;
        }
        let folder = self.presets_folder();
        let s = self.state.borrow();
        presets::delete(&preset, &folder, &mut |p: &Path| (s.discard)(p));
        drop(s);
        self.reload_presets();
    }

    /// `sharePreset`: the preset's own file, wherever the save panel says.
    fn share_preset(&self, id: Id) {
        let Some(preset) = self.held(id) else { return };
        let Some(destination) = system::save_file("Share Preset", &presets::file_name(&preset), &["json"]) else { return };
        let written = presets::shareable(&preset).map_err(|e| e.to_string()).and_then(|t| std::fs::write(&destination, t).map_err(|e| e.to_string()));
        if let Err(e) = written {
            self.say(&e);
        }
    }

    /// `copyPresetCode`: the general clipboard on purpose, the code being for another person.
    fn copy_preset_code(&self, id: Id) {
        let Some(preset) = self.held(id) else { return };
        match presets::share_code(&preset) {
            Ok(code) => system::set_text(&code),
            Err(e) => self.say(&e.to_string()),
        }
    }

    /// `pastePresetCode`: a code adopted as an import is, or a notice saying why not.
    pub(super) fn paste_preset_code(&self) {
        self.adopt_preset_code(&system::get_text().unwrap_or_default());
    }

    pub fn adopt_preset_code(&self, text: &str) {
        let Some(incoming) = presets::from_share_code(text) else {
            let message = if text.contains(presets::SHARE_CODE_PREFIX) {
                "The code is incomplete or was changed on the way. Ask for it to be copied again.".to_string()
            } else {
                format!("Copy a code starting with {}, then paste it here.", presets::SHARE_CODE_PREFIX)
            };
            return self.advise("No Preset Code on the Clipboard", &message);
        };
        let name = incoming.name.clone();
        let taken = presets::adopting(vec![incoming], &self.presets_folder());
        self.reload_presets();
        if taken.is_empty() {
            self.advise(&format!("\u{201c}{name}\u{201d} Is Already in Your Presets"), "");
        }
    }

    /// `importPresets`: files from the open panel, never overwriting, and a notice for those left out.
    pub(super) fn import_presets(&self) {
        let paths = system::pick_files("Import Presets", &["json"]);
        if !paths.is_empty() {
            self.import_preset_files(&paths);
        }
    }

    pub fn import_preset_files(&self, paths: &[PathBuf]) {
        let taken = presets::importing(paths, &self.presets_folder());
        self.reload_presets();
        let skipped = paths.len() - taken.len();
        if skipped == 0 {
            return;
        }
        let title = if taken.is_empty() { "No Preset Imported".to_string() } else { format!("{} of {} Presets Imported", taken.len(), paths.len()) };
        let message = if skipped == 1 {
            "One file was left out: it is already in your presets, or it is not a preset.".to_string()
        } else {
            format!("{skipped} files were left out: they are already in your presets, or they are not presets.")
        };
        self.advise(&title, &message);
    }

    pub(super) fn show_presets_folder(&self) {
        let folder = self.presets_folder();
        let _ = std::fs::create_dir_all(&folder);
        system::reveal(&folder);
    }

    /// `PresetDialogs.request(updating: true)`: the stored settings replaced, the rest kept.
    fn ask_update_preset(self: &Rc<Self>, id: Id) {
        let Some(p) = self.held(id) else { return };
        self.ask(
            &format!("Replace \u{201c}{}\u{201d} with This Photo\u{2019}s Settings?", p.name),
            "The settings it stored are replaced. Its name, color, description and the rows it carries stay as they are.",
            "Replace",
            Pending::UpdatePreset(id),
        );
    }

    fn ask_delete_preset(self: &Rc<Self>, id: Id) {
        let Some(p) = self.held(id) else { return };
        self.ask(
            &format!("Delete the Preset \u{201c}{}\u{201d}?", p.name),
            "Its file goes to the Trash, where it can be put back. Photos it was applied to keep their settings.",
            "Delete",
            Pending::DeletePreset(id),
        );
    }

    pub(super) fn update_preset(&self, id: Id) {
        let Some(mut p) = self.held(id) else { return };
        p.settings = self.state.borrow().store.settings().clone();
        self.commit_preset(&p);
        self.refresh_working(&p);
    }

    /// A delete confirmed: from the editor, the neighbour is selected before the row goes.
    pub(super) fn delete_preset(&self, id: Id) {
        self.remove_preset(id);
        let editing = {
            let mut s = self.state.borrow_mut();
            match s.shelf.form.as_mut() {
                Some(Form::Editing { working, selected }) => {
                    working.retain(|p| p.id != id);
                    *selected = (!working.is_empty()).then_some(0);
                    true
                }
                _ => false,
            }
        };
        if editing {
            self.publish_form(true);
        }
    }

    /// The editor's copy follows a write made from the deck while it is open.
    fn refresh_working(&self, preset: &Preset) {
        if let Some(Form::Editing { working, .. }) = self.state.borrow_mut().shelf.form.as_mut()
            && let Some(w) = working.iter_mut().find(|w| w.id == preset.id)
        {
            *w = preset.clone();
        }
    }

    pub(super) fn advise(&self, title: &str, message: &str) {
        let Some(w) = self.window() else { return };
        let a = w.global::<Advice>();
        a.set_title(title.into());
        a.set_message(message.into());
        a.set_shown(true);
    }

    pub(super) fn advice_closed(&self) {
        if let Some(w) = self.window() {
            w.global::<Advice>().set_shown(false);
        }
        self.refocus();
        self.publish_bar();
    }

    // MARK: - The sheets

    /// `savePreset`: the sheet opens on the first free name and the rows the photo moved.
    pub(super) fn save_preset(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            if s.open.is_none() {
                return;
            }
            let names: Vec<&str> = s.shelf.held.iter().map(|p| p.name.as_str()).collect();
            let settings = s.store.settings().clone();
            let draft = Preset::new(&presets::default_name(&names), carries::proposed(&settings), Tint::default(), settings);
            s.shelf.hovered = None;
            s.shelf.form = Some(Form::Saving(draft));
        }
        self.publish_form(true);
        self.publish_bar();
    }

    /// One preset, one file, one write, laid on the settings the photo has now.
    fn save_preset_confirmed(self: &Rc<Self>) {
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
    pub(super) fn edit_presets(self: &Rc<Self>, initial: Option<Id>) {
        {
            let mut s = self.state.borrow_mut();
            let working = s.shelf.held.clone();
            let selected = initial.and_then(|id| working.iter().position(|p| p.id == id)).or((!working.is_empty()).then_some(0));
            s.shelf.form = Some(Form::Editing { working, selected });
        }
        self.publish_form(true);
        self.publish_bar();
    }

    fn preset_picked(self: &Rc<Self>, i: Option<usize>) {
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

    fn settle_selected(&self) {
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
    fn editor_act(self: &Rc<Self>, act: CardAct) {
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
    fn edit_form(self: &Rc<Self>, change: impl FnOnce(&mut Preset)) {
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

    /// Done, Cancel or the scrim: the editor writes what it holds first.
    fn close_preset_form(self: &Rc<Self>) {
        if matches!(self.state.borrow().shelf.form, Some(Form::Editing { .. })) {
            self.settle_selected();
        }
        self.state.borrow_mut().shelf.form = None;
        self.publish_form(false);
        self.refocus();
        self.publish_bar();
    }

    /// A sheet gone takes its field's focus with it: the window's keys get the keyboard back.
    fn refocus(&self) {
        if let Some(w) = self.window() {
            w.invoke_refocus();
        }
    }

    /// Escape and Return on a sheet: Cancel, and Save or Done.
    pub(super) fn preset_form_key(self: &Rc<Self>, text: &str) -> bool {
        let saving = match self.state.borrow().shelf.form {
            None => return false,
            Some(Form::Saving(_)) => true,
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
    fn publish_form(&self, fresh: bool) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Presets>();
        let mut s = self.state.borrow_mut();
        let (saving, editing) = match s.shelf.form {
            Some(Form::Saving(_)) => (true, false),
            Some(Form::Editing { .. }) => (false, true),
            None => (false, false),
        };
        g.set_saving(saving);
        g.set_editing(editing);
        if let Some(Form::Editing { working, selected }) = s.shelf.form.as_ref() {
            g.set_rows(ModelRc::new(VecModel::from(working.iter().map(card).collect::<Vec<_>>())));
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
        g.set_form_note(p.note.clone().into());
        g.set_form_tint(Tint::ALL.iter().position(|t| *t == p.tint).unwrap_or(0) as i32);
        g.set_form_carries(ModelRc::new(VecModel::from(carries_lines(&p.parameters))));
        g.set_form_empty(p.parameters.is_empty());
        if fresh {
            g.set_form_instance(ModelRc::new(VecModel::from(vec![s.shelf.serial as i32])));
        }
    }
}

impl super::State {
    pub(super) fn shelf_form_open(&self) -> bool {
        self.shelf.form.is_some()
    }

    /// `propagationTargets`: the selection, else the open photo.
    pub(super) fn preset_targets(&self) -> Vec<String> {
        if self.picks.selection.is_empty() { self.open.clone().into_iter().collect() } else { self.selected() }
    }

    /// `previewedSettings`: the hovered preset laid over the open frame's settings, with the balance
    /// an automatic one measured when it was measured on these settings.
    pub(super) fn preset_preview(&self, settings: &PipelineSettings) -> Option<PipelineSettings> {
        let preset = self.shelf.hovered.and_then(|id| self.shelf.held.iter().find(|p| p.id == id))?;
        let laid = presets::previewed(Some(preset), settings);
        match &self.shelf.hover_balance {
            Some((id, through, outcome)) if *id == preset.id && through == settings && oni_core::parameters::auto_balances(&preset.parameters) => {
                Some(balance::applied_outcome(outcome, &laid))
            }
            _ => Some(laid),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_carries_box_is_found_at_its_own_line() {
        let parameters = carries::proposed(&PipelineSettings::default());
        let lines = carries_lines(&parameters);
        let boxes: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.kind == 2).map(|(i, _)| i).collect();
        assert_eq!(boxes.len(), carries::entries().count());
        for (i, entry) in boxes.iter().zip(carries::entries()) {
            assert_eq!(entry_at(*i).map(|e| e.id), Some(entry.id));
            assert_eq!(lines[*i].text.as_str(), entry.label);
        }
        // Twin: a title or the balance choice is no box.
        assert!(lines.iter().enumerate().filter(|(_, l)| l.kind != 2).all(|(i, _)| entry_at(i).is_none()));
    }
}
