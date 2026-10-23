//! The Roll tab's settings blocks (`SettingsPane`): the flat-field's toggle, mask and sliders, the
//! Source Encoding a file naming no profile is read by, Source Balance's named ends, and the dye
//! family the separation runs.

use super::Ctl;
use crate::{MenuLine, PanelState};
use oni_i18n::tr;
use oni_core::Command;
use oni_settings::blocks::SeparationFamily;
use oni_settings::frame::{FlatField, FlatFieldMask};
use oni_settings::{PipelineSettings, SourceEncoding};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

/// `Separation.Family.allCases`, in the bar's order.
const FAMILIES: [SeparationFamily; 5] =
    [SeparationFamily::KodakC41, SeparationFamily::FujiC41, SeparationFamily::GenericC41, SeparationFamily::Ecn2, SeparationFamily::E6];

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

impl Ctl {
    pub(super) fn wire_roll_tab(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let panel = window.global::<PanelState>();
        let p = me.clone();
        panel.on_dye_family_picked(move |i| {
            if let Some(family) = usize::try_from(i).ok().and_then(|i| FAMILIES.get(i)) {
                p.write_settings(|s| s.separation_family = Some(*family));
            }
        });
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
        panel.on_mask_chosen(move |i| p.mask_chosen(usize::try_from(i).ok()));
        let p = me.clone();
        panel.on_flat_apply_selection(move || {
            let picked: Vec<String> = {
                let s = p.state.borrow();
                s.selected().into_iter().filter(|q| Some(q) != s.open.as_ref()).collect()
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

    /// The Roll tab's settings blocks, from the open frame's settings and decode.
    pub(super) fn publish_roll_tab(&self) {
        let Some(w) = self.window() else { return };
        let (settings, undeclared, entries, picked) = {
            let s = self.state.borrow();
            let undeclared = s.negative.as_ref().and_then(|(_, n)| n.undeclared);
            let picked = s.picks.selection.iter().filter(|p| Some(*p) != s.open.as_ref()).count();
            (s.store.settings().clone(), undeclared, s.masks.clone(), picked)
        };
        let panel = w.global::<PanelState>();
        let family = settings.separation_family.or_else(|| SeparationFamily::resting(settings.mode)).unwrap_or(SeparationFamily::GenericC41);
        panel.set_dye_family(FAMILIES.iter().position(|f| *f == family).unwrap_or(2) as i32);
        panel.set_gains_inverted(settings.mode.inverts_gain_sense());
        panel.set_monochrome(settings.mode.is_monochrome());
        panel.set_sheet(settings.sheet.is_some());
        let flat = &settings.flat_field;
        panel.set_flat_field_on(flat.mask != FlatFieldMask::None);
        panel.set_masked(flat.mask != FlatFieldMask::None && !(flat.mask == FlatFieldMask::Custom && flat.custom.is_none()));
        let options = mask_options(flat, &entries);
        let current = chosen(flat);
        panel.set_mask_title(options.iter().find(|(l, _)| *l == current).map_or(tr("None selected").to_string(), |(_, t)| t.clone()).into());
        let lines: Vec<MenuLine> = options.iter().map(|(_, t)| MenuLine { text: t.as_str().into(), enabled: true, rule: false, opens: false }).collect();
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
