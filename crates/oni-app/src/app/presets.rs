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
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::path::{Path, PathBuf};
use std::rc::Rc;

mod shelf;
mod deck;
mod gestures;
mod sheets;

#[derive(Default)]
pub(super) struct Shelf {
    pub held: Vec<Preset>,
    /// The card under the pointer: kept out of the settings, so a hover never writes one.
    pub hovered: Option<Id>,
    /// What an automatic preset previews: the open photo's balance, measured on the settings it had.
    hover_balance: Option<(String, PipelineSettings, Outcome)>,
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

/// `PresetTint.ink`: black on the light tints, white on the dark.
fn ink(tint: Tint) -> Color {
    if tint.wants_dark_ink() { Color::from_rgb_u8(0, 0, 0) } else { Color::from_rgb_u8(255, 255, 255) }
}

fn card(p: &Preset) -> PresetCard {
    PresetCard {
        abbreviation: p.abbreviation().into(),
        name: p.name.clone().into(),
        help: p.help().into(),
        carries: carries::label(&p.parameters).into(),
        fill: colour(p.tint.components()),
        ink: ink(p.tint),
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


impl super::State {
    pub(super) fn shelf_form_open(&self) -> bool {
        self.shelf.form.is_some()
    }

    /// `propagationTargets`: the selection, else the open photo.
    pub(super) fn preset_targets(&self) -> Vec<String> {
        if self.picks.selection.is_empty() { self.open.clone().into_iter().collect() } else { self.selected() }
    }

    /// The open frame's settings as the canvas and the loupe show them, a hovered preset laid over.
    pub(super) fn live_settings(&self) -> PipelineSettings {
        self.previewed(self.store.settings().clone())
    }

    /// `previewedSettings`: the hovered preset laid over `settings`, with the balance an automatic one
    /// measured when it was measured on these settings; `settings` as they are without one.
    pub(super) fn previewed(&self, settings: PipelineSettings) -> PipelineSettings {
        let Some(preset) = self.shelf.hovered.and_then(|id| self.shelf.held.iter().find(|p| p.id == id)) else { return settings };
        let laid = presets::previewed(Some(preset), &settings);
        match &self.shelf.hover_balance {
            Some((on, through, outcome)) if self.open.as_ref() == Some(on) && *through == settings && oni_core::parameters::auto_balances(&preset.parameters) => {
                balance::applied_outcome(outcome, &laid)
            }
            _ => laid,
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
