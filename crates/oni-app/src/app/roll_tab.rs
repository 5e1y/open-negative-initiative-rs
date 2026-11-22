//! The Roll tab's settings blocks (`SettingsPane`): the flat-field's toggle, mask and sliders, the
//! Scan Profile a file naming no profile is read by, Source Balance's named ends, and the measured dyes
//! the separation runs.

use super::Ctl;
use crate::{MenuLine, PanelState};
use oni_i18n::tr;
use oni_core::Command;
use oni_settings::frame::{FlatField, FlatFieldMask};
use oni_settings::{ConversionMode, DyeSet, LegacyMachine, LegacyPrint, PipelineSettings, SourceEncoding, VirtualStock};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

/// The name a measured dye set goes by: its stock's, the generic set's own.
pub(super) fn dye_set_name(set: DyeSet) -> String {
    set.stock_name().map_or_else(|| tr("Generic C-41").to_string(), str::to_string)
}

/// The enlarger's filtration the paper prints through, in CC, as the Colour tab says it.
pub(super) fn filtration_line(cc: [f32; 3]) -> String {
    let [c, m, y] = cc.map(|v| format!("{:.0}", v.max(0.0)));
    oni_i18n::tr!("Enlarger filtration: C {c}, M {m}, Y {y}", c = c, m = m, y = y)
}

/// A legacy print's name as its switch reads it: the machine's look over ONI's print, or on a paper the keys alone.
pub(super) fn legacy_title(legacy: &LegacyPrint, on_paper: bool) -> &'static str {
    match (on_paper, legacy.machine) {
        (true, _) => tr("Scanner keys (legacy)"),
        (false, LegacyMachine::Oni) => tr("Scanner ONI look (legacy)"),
        (false, LegacyMachine::Frontier) => tr("Scanner Frontier look (legacy)"),
    }
}

/// The keypad's steps as its screen read them, in points: a step is half a point.
pub(super) fn legacy_keys(legacy: &LegacyPrint) -> String {
    let point = |steps: i64| crate::fields::number(steps as f64 / LegacyPrint::PER_POINT as f64, 1, true, crate::fields::Unit::Bare);
    let (d, c, m, y, w) = (point(legacy.d), point(legacy.c), point(legacy.m), point(legacy.y), point(legacy.w));
    oni_i18n::tr!("D {d} · C {c} · M {m} · Y {y} · TEMP {w}", d = d, c = c, m = m, y = y, w = w)
}

/// The name the Film Type tabs give a mode, wherever a sentence names it.
pub(super) fn mode_name(mode: ConversionMode) -> &'static str {
    match mode {
        ConversionMode::Negative => tr("Negative"),
        ConversionMode::Positive => tr("Positive"),
        ConversionMode::Monochrome => tr("B&W"),
        ConversionMode::Digital => tr("Digital"),
    }
}

/// `SourceEncoding.allCases`; a grey file offers the first two.
const ENCODINGS: [SourceEncoding; 3] = [SourceEncoding::Srgb, SourceEncoding::Linear, SourceEncoding::DisplayP3];

/// What a line of the mask picker chooses.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum MaskLine {
    None,
    Shipped,
    Custom(String),
}

/// `FlatFieldBlock.options`: none, the shipped mask while it is the frame's, a missing one, then each loaded.
pub(super) fn mask_options(flat: &FlatField, entries: &[crate::flat_fields::Entry]) -> Vec<(MaskLine, String)> {
    let mut list = vec![(MaskLine::None, tr("None selected").to_string())];
    if flat.mask == FlatFieldMask::ValoiEasy35V1 {
        list.push((MaskLine::Shipped, "Valoi 35v1".to_string()));
    } else if let Some(id) = flat.custom.as_ref().filter(|id| !entries.iter().any(|e| &e.id == *id)) {
        list.push((MaskLine::Custom(id.clone()), tr("Missing mask").to_string()));
    }
    list.extend(entries.iter().map(|e| (MaskLine::Custom(e.id.clone()), e.name.clone())));
    list
}

pub(super) fn chosen(flat: &FlatField) -> MaskLine {
    match (flat.mask, &flat.custom) {
        (FlatFieldMask::ValoiEasy35V1, _) => MaskLine::Shipped,
        (_, Some(id)) => MaskLine::Custom(id.clone()),
        _ => MaskLine::None,
    }
}

/// `FlatFieldBlock.choice`: the shipped mask, none yet chosen, or a loaded one.
pub(super) fn chose(flat: &FlatField, line: MaskLine) -> FlatField {
    let mut next = flat.clone();
    match line {
        MaskLine::Shipped => next.mask = FlatFieldMask::ValoiEasy35V1,
        MaskLine::None => {
            next.mask = FlatFieldMask::Custom;
            next.custom = None;
        }
        MaskLine::Custom(id) => {
            next.mask = FlatFieldMask::Custom;
            next.custom = Some(id);
        }
    }
    next
}

/// `FlatFieldBlock.on`: on takes a custom mask yet to be chosen at the whole amount; off keeps the rest.
pub fn toggled(flat: &FlatField, on: bool) -> FlatField {
    let mut next = flat.clone();
    if on {
        next.mask = FlatFieldMask::Custom;
        next.custom = None;
        next.amount = FlatField::default().amount;
    } else {
        next.mask = FlatFieldMask::None;
    }
    next
}

/// The Film block's tab for a photo: its stock's place.
/// The photo shot on `stock`: the one it stands on kept, another placed on its own reference, as Prepare the
/// Roll places a roll's; a photo shot on no film left as it is.
pub fn picked_film(s: &PipelineSettings, stock: VirtualStock) -> PipelineSettings {
    match s.film() {
        Some(f) if f.stock != stock => s.on_stock(stock),
        _ => s.clone(),
    }
}

/// The Film block's read-only lines: the stock's own white, then the reference its negative is read on.
pub fn film_lines(s: &PipelineSettings) -> [String; 4] {
    let Some(film) = s.film() else { return Default::default() };
    let r = film.stock.reference();
    let stops = |v: [f32; 3]| v.map(|x| crate::fields::number(f64::from(x), 2, true, crate::fields::Unit::Stops));
    let shares = |v: [f32; 3]| v.map(|x| crate::fields::number(f64::from(x) * 100.0, 1, false, crate::fields::Unit::Percent));
    let channels = |[red, green, blue]: [String; 3]| oni_i18n::tr!("R {red} · G {green} · B {blue}", red = red, green = green, blue = blue);
    [
        oni_i18n::tr!("{kelvin} K, the light the stock is balanced for", kelvin = (film.stock.white().kelvin / 10.0).round() * 10.0),
        channels(stops(r.stops)),
        channels(shares(r.black)),
        channels(shares(r.white)),
    ]
}

impl Ctl {
    pub(super) fn wire_roll_tab(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let panel = window.global::<PanelState>();
        let p = me.clone();
        panel.on_encoding_picked(move |i| {
            if let Some(&encoding) = usize::try_from(i).ok().and_then(|i| ENCODINGS.get(i)) {
                p.write_settings(|s| s.source_encoding = (encoding != SourceEncoding::Srgb).then_some(encoding));
            }
        });
        let p = me.clone();
        panel.on_encoding_reset(move || p.write_settings(|s| s.source_encoding = None));
        let p = me.clone();
        panel.on_flat_field_toggled(move |on| p.write_settings(|s| s.flat_field = toggled(&s.flat_field, on)));
        let p = me.clone();
        panel.on_enlarger_toggled(move |on| p.write_settings(|s| s.print_paper = on.then(|| s.fitting_paper())));
        let p = me.clone();
        panel.on_on_paper_toggled(move |on| p.write_settings(|s| s.paperless = !on));
        let p = me.clone();
        panel.on_colour_compensation_toggled(move |on| p.write_settings(|s| s.print.colour_compensation = on));
        let p = me.clone();
        panel.on_paper_blacks_toggled(move |on| p.write_settings(|s| s.print.paper_blacks = on));
        let p = me.clone();
        panel.on_legacy_toggled(move |on| {
            p.write_settings(|s| {
                if let Some(l) = s.legacy_print.as_mut() {
                    l.laid = on;
                }
            })
        });
        let p = me.clone();
        panel.on_mask_chosen(move |i| p.mask_chosen(usize::try_from(i).ok()));
        let p = me.clone();
        panel.on_flat_apply_selection(move || {
            let picked: Vec<String> = {
                let s = p.state.borrow();
                s.selected().into_iter().filter(|q| Some(q.as_str()) != s.open_path()).collect()
            };
            p.gesture(&picked, |store| store.apply_flat_field(&picked));
        });
        Ctl::wire_masks(me, window);
    }

    /// One structured edit of the open frame, as one undo step.
    pub(super) fn write_settings(self: &Rc<Self>, change: impl FnOnce(&mut PipelineSettings)) {
        let mut next = self.state.borrow().store.settings().clone();
        change(&mut next);
        if &next == self.state.borrow().store.settings() {
            return;
        }
        self.write(Command::Write(Box::new(next)));
    }

    fn mask_chosen(self: &Rc<Self>, i: Option<usize>) {
        let options = {
            let s = self.state.borrow();
            mask_options(&s.store.settings().flat_field, &s.masks)
        };
        let Some((line, _)) = i.and_then(|i| options.get(i)).cloned() else { return };
        self.write_settings(|s| s.flat_field = chose(&s.flat_field, line));
    }

    /// The Dye Separation block on the measured dyes the open frame unmixes.
    pub(super) fn publish_dyes(&self) {
        let Some(w) = self.window() else { return };
        let (set, stale) = {
            let s = self.state.borrow();
            // The roll card read its frames on another state of the open photo: it reads them again.
            let stale = s.org.frames.as_ref().is_some_and(|((_, bearing), _)| *bearing != s.open_bearing());
            (s.store.settings().dye_set, stale)
        };
        let panel = w.global::<PanelState>();
        panel.set_dye_set(dye_set_name(set).into());
        panel.set_dye_set_generic(set.is_generic());
        if stale {
            self.publish_info();
        }
    }

    /// What the panel shows of the open frame's mode: the dye separation's block, a slide's and a digital photo's, and
    /// the print's switches.
    pub(super) fn publish_mode(&self) {
        let Some(w) = self.window() else { return };
        let (settings, cmy) = {
            let s = self.state.borrow();
            (s.store.settings().clone(), s.preferences.filtration_cmy())
        };
        let panel = w.global::<PanelState>();
        panel.set_filtration_cmy(cmy);
        panel.set_separation_shown(!settings.mode.is_monochrome() && settings.rgb_passes.is_none() && !settings.inverted_source);
        panel.set_slide(settings.mode.is_slide());
        panel.set_digital(settings.mode.is_digital());
        self.publish_paper(&panel, &settings);
    }

    /// The Colour tab's print switches: Lighting's colour compensation, and the enlarger where one can print the photo,
    /// Print on paper, the paper's name, the filtration it prints through and Paper blacks.
    pub(super) fn publish_paper(&self, panel: &PanelState, settings: &PipelineSettings) {
        panel.set_colour_compensation(settings.print.colour_compensation);
        panel.set_paper_blacks(settings.print.paper_blacks);
        panel.set_paper_shown(settings.takes_paper());
        panel.set_enlarger_on(settings.paper().is_some());
        panel.set_on_paper(!settings.paperless);
        panel.set_paper_title(crate::stocks::paper_label(settings.fitting_paper()));
        let line = oni_pipeline::conversion::printing(settings).map_or_else(String::new, |p| filtration_line(p.filtration));
        panel.set_paper_filtration(line.into());
        let legacy = settings.legacy_print;
        panel.set_legacy_title(legacy.map_or("", |l| legacy_title(&l, settings.paper().is_some())).into());
        panel.set_legacy_laid(legacy.is_some_and(|l| l.laid));
        panel.set_legacy_keys(legacy.as_ref().map_or_else(String::new, legacy_keys).into());
    }

    /// The Roll tab's settings blocks, from the open frame's settings and decode.
    pub(super) fn publish_roll_tab(&self) {
        let Some(w) = self.window() else { return };
        let (settings, undeclared, entries, picked) = {
            let s = self.state.borrow();
            let undeclared = s.negative().and_then(|n| n.undeclared);
            let picked = s.picks.selection.iter().filter(|p| Some(p.as_str()) != s.open_path()).count();
            (s.store.settings().clone(), undeclared, s.masks.clone(), picked)
        };
        let panel = w.global::<PanelState>();
        self.publish_dyes();
        panel.set_gains_inverted(settings.mode.inverts_gain_sense());
        panel.set_monochrome(settings.mode.is_monochrome());
        panel.set_stock(self.open_canister());
        let [white, gains, black, ends] = film_lines(&settings);
        panel.set_film_white(white.into());
        panel.set_film_gains(gains.into());
        panel.set_film_black(black.into());
        panel.set_film_ends(ends.into());
        panel.set_sheet(settings.sheet.is_some());
        let flat = &settings.flat_field;
        panel.set_flat_field_on(flat.mask != FlatFieldMask::None);
        panel.set_masked(flat.mask != FlatFieldMask::None && !(flat.mask == FlatFieldMask::Custom && flat.custom.is_none()));
        let options = mask_options(flat, &entries);
        let current = chosen(flat);
        panel.set_mask_title(options.iter().find(|(l, _)| *l == current).map_or(tr("None selected").to_string(), |(_, t)| t.clone()).into());
        let lines: Vec<MenuLine> = options.iter().map(|(_, t)| MenuLine { text: t.as_str().into(), enabled: true, ..MenuLine::default() }).collect();
        panel.set_mask_lines(ModelRc::new(VecModel::from(lines)));
        panel.set_masks_empty(entries.is_empty());
        self.publish_masks(&panel, flat);
        panel.set_flat_picked(picked as i32);
        panel.set_undeclared(undeclared.is_some());
        panel.set_grey_source(undeclared == Some(true));
        let encoding = settings.source_encoding.unwrap_or(SourceEncoding::Srgb);
        panel.set_encoding(ENCODINGS.iter().position(|e| *e == encoding).unwrap_or(0) as i32);
        panel.set_encoding_modified(settings.source_encoding.is_some());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dye_set_goes_by_its_stocks_name_and_the_generic_set_by_its_own() {
        assert_eq!(dye_set_name(DyeSet::KodakPortra400), "Kodak Portra 400");
        // Twin: the generic set names no stock.
        assert_eq!(dye_set_name(DyeSet::GenericC41), "Generic C-41");
    }

    #[test]
    fn the_toggle_takes_a_mask_to_choose_at_the_whole_amount_and_off_keeps_the_rest() {
        let dialled = FlatField { mask: FlatFieldMask::ValoiEasy35V1, custom: Some("abc".into()), amount: 0.4, temperature: 0.3, tint: -0.2 };
        let on = toggled(&dialled, true);
        assert_eq!((on.mask, on.custom.as_deref(), on.amount), (FlatFieldMask::Custom, None, 1.0));
        assert_eq!((on.temperature, on.tint), (0.3, -0.2), "the colour stays as the hand left it");
        let off = toggled(&dialled, false);
        assert_eq!(off, FlatField { mask: FlatFieldMask::None, ..dialled.clone() }, "twin: off touches nothing else");
    }

    #[test]
    fn the_picker_lists_the_shipped_mask_only_while_it_is_the_frames_and_names_a_missing_one() {
        let entries = vec![crate::flat_fields::Entry { id: "a1".into(), name: "Rig".into(), per_channel: None }];
        let titles = |f: &FlatField| mask_options(f, &entries).into_iter().map(|(_, t)| t).collect::<Vec<_>>();
        assert_eq!(titles(&FlatField::default()), ["None selected", "Rig"]);
        assert_eq!(titles(&FlatField { mask: FlatFieldMask::ValoiEasy35V1, ..FlatField::default() }), ["None selected", "Valoi 35v1", "Rig"]);
        let gone = FlatField { mask: FlatFieldMask::Custom, custom: Some("zz".into()), ..FlatField::default() };
        assert_eq!(titles(&gone), ["None selected", "Missing mask", "Rig"]);
        // Twin: a mask held in the library is listed once, under its own name.
        let held = FlatField { mask: FlatFieldMask::Custom, custom: Some("a1".into()), ..FlatField::default() };
        assert_eq!(titles(&held), ["None selected", "Rig"]);
        assert_eq!(chosen(&held), MaskLine::Custom("a1".into()));
    }
}
