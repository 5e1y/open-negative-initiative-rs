//! The presets in the window (`PresetCarousel`, `PresetSaveSheet`, `PresetEditor`, the Settings
//! menu's list): the shelf read back from its folder after every write, a hover previewing on the
//! canvas, a click applying to the targets as one gallery step, and the two sheets.

use super::{Ctl, Pending, WRITE_DELAY, stem};
use crate::preferences::Preferences;
use crate::{Advice, CarriesLine, MenuGlyph, MenuLine, Menus, PresetCard, Presets, system};
use oni_core::balance::{self, Outcome};
use oni_core::preset_parameters::{self as carries, WhiteBalance};
use oni_core::presets::{self, Preset, Tint};
use oni_settings::{Id, PipelineSettings};
use oni_i18n::{marked, tr, trn};
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::collections::BTreeSet;
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
    /// Copy Settings told not to ask again: until the app quits, Copy takes the last boxes at once.
    copy_at_once: bool,
    /// Raised by every edit in the editor; a write armed under an older count is dropped.
    serial: u64,
    /// The deck and the menu's list as last handed over: a new model would drop the hovered card.
    published: Option<(Vec<PresetCard>, Vec<SharedString>)>,
}

enum Form {
    Saving(Preset),
    /// Copy Settings: the boxes of the look the copy carries, of the photo `from` or the open one,
    /// and whether its commit stops the asking.
    Copying { draft: Preset, from: Option<String>, at_once: bool },
    Editing { working: Vec<Preset>, selected: Option<usize> },
}

impl Form {
    fn current(&mut self) -> Option<&mut Preset> {
        match self {
            Form::Saving(draft) | Form::Copying { draft, .. } => Some(draft),
            Form::Editing { working, selected } => selected.and_then(|i| working.get_mut(i)),
        }
    }

    fn copying(&self) -> bool {
        matches!(self, Form::Copying { .. })
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

const CARD_MENU: [(CardAct, MenuGlyph, &str); 6] = [
    (CardAct::Edit, MenuGlyph::Edit, marked("Edit…")),
    (CardAct::Update, MenuGlyph::Update, marked("Update with Current Settings")),
    (CardAct::Duplicate, MenuGlyph::Copy, marked("Duplicate")),
    (CardAct::Share, MenuGlyph::Export, marked("Share Preset…")),
    (CardAct::CopyCode, MenuGlyph::Code, marked("Copy Preset Code")),
    (CardAct::Delete, MenuGlyph::Trash, marked("Delete")),
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

/// One line of the Carries picker: a block's title or one of its boxes.
enum Line {
    Title(&'static str),
    Box(&'static carries::Entry),
}

/// The column a block stands in: Colour, the longest, alone in the first, the others beside it.
fn column(block: &carries::Block) -> usize {
    usize::from(block.id != "Colour")
}

/// Each block's title then its boxes, each with its column; a copy shows the boxes of the look alone.
/// The white balance, a choice and no box, stands over them all.
fn layout(copying: bool) -> Vec<(usize, Line)> {
    let mut lines = Vec::new();
    for block in &carries::BLOCKS {
        let boxes: Vec<Line> = block.entries.iter().filter(|e| !copying || carries::pasted(e)).map(Line::Box).collect();
        if boxes.is_empty() {
            continue;
        }
        let at = column(block);
        lines.push((at, Line::Title(block.title)));
        lines.extend(boxes.into_iter().map(|b| (at, b)));
    }
    lines
}

fn carries_lines(parameters: &BTreeSet<String>, copying: bool) -> Vec<CarriesLine> {
    let mut started = [false; 2];
    layout(copying)
        .into_iter()
        .map(|(at, line)| {
            let (column, lead) = (at as i32, !std::mem::replace(&mut started[at], true));
            match line {
                Line::Title(title) => CarriesLine { title: true, text: tr(title).into(), on: false, column, lead },
                Line::Box(entry) => CarriesLine { title: false, text: tr(entry.label).into(), on: carries::is_on(entry, parameters), column, lead },
            }
        })
        .collect()
}

/// The box a Carries line stands for, `None` for a title.
fn entry_at(line: usize, copying: bool) -> Option<&'static carries::Entry> {
    match layout(copying).into_iter().nth(line)?.1 {
        Line::Box(entry) => Some(entry),
        Line::Title(_) => None,
    }
}

fn line(glyph: MenuGlyph, text: &str, enabled: bool, rule: bool) -> MenuLine {
    MenuLine { glyph, text: text.into(), enabled, rule, ..MenuLine::default() }
}


impl super::State {
    pub(super) fn shelf_form_open(&self) -> bool {
        self.shelf.form.is_some()
    }

    /// `propagationTargets`: the selection, else the open photo.
    pub(super) fn preset_targets(&self) -> Vec<String> {
        if self.picks.selection.is_empty() { self.open_path().map(str::to_string).into_iter().collect() } else { self.selected() }
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
            Some((on, through, outcome)) if self.open_path() == Some(on.as_str()) && *through == settings && oni_core::parameters::auto_balances(&preset.parameters) => {
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
        for copying in [false, true] {
            let lines = carries_lines(&parameters, copying);
            let boxes: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| !l.title).map(|(i, _)| i).collect();
            let offered: Vec<&carries::Entry> = carries::entries().filter(|e| !copying || carries::pasted(e)).collect();
            assert_eq!(boxes.len(), offered.len());
            for (i, entry) in boxes.iter().zip(offered) {
                assert_eq!(entry_at(*i, copying).map(|e| e.id), Some(entry.id));
                assert_eq!(lines[*i].text.as_str(), entry.label);
            }
            // Twin: a title is no box.
            assert!(lines.iter().enumerate().filter(|(_, l)| l.title).all(|(i, _)| entry_at(i, copying).is_none()));
        }
        assert!(carries_lines(&parameters, true).len() < carries_lines(&parameters, false).len(), "a copy offers fewer boxes");
    }

    #[test]
    fn each_column_opens_on_a_title_that_leads_it_alone() {
        let parameters = carries::proposed(&PipelineSettings::default());
        for copying in [false, true] {
            let lines = carries_lines(&parameters, copying);
            for column in [0, 1] {
                let own: Vec<&CarriesLine> = lines.iter().filter(|l| l.column == column).collect();
                assert!(own.first().is_some_and(|l| l.title && l.lead), "column {column} opens on its leading title");
                // Twin: no later line leads, or its block would stand flush against the one above.
                assert!(own.iter().skip(1).all(|l| !l.lead), "column {column} has one lead");
            }
            let colour = lines.iter().find(|l| l.title && l.text == "Colour").unwrap();
            assert_eq!(colour.column, 0, "Colour stands alone in the first column");
            assert!(lines.iter().filter(|l| l.column == 0).all(|l| !l.title || l.text == "Colour"));
        }
    }
}
