//! The rows a preset carries, as the save sheet and the editor show them: one box per interface
//! name, grouped by block, and the white balance as one choice of three.

use crate::parameters::{self, AUTO_BALANCE_NAME, WHITE_BALANCE_ROWS};
use oni_i18n::{marked, tr};
use oni_settings::PipelineSettings;
use std::collections::BTreeSet;

pub struct Entry {
    pub id: &'static str,
    pub label: &'static str,
    pub rows: &'static [&'static str],
}

pub struct Block {
    pub id: &'static str,
    pub title: &'static str,
    pub entries: &'static [Entry],
}

const fn entry(id: &'static str, label: &'static str, rows: &'static [&'static str]) -> Entry {
    Entry { id, label, rows }
}

pub const BLOCKS: [Block; 4] = [
    Block {
        id: "Settings",
        title: marked("Settings"),
        entries: &[
            entry("sourceEncoding", marked("Source Encoding"), &["sourceEncoding"]),
            entry("flatField", marked("Flat-Field"), &["flatField"]),
            entry("separation", marked("Dye Separation"), &["separation"]),
        ],
    },
    Block {
        id: "Colour",
        title: marked("Color"),
        entries: &[
            entry("colour", marked("Color (Temperature, Tint)"), &["curves"]),
            entry("density", marked("Color Density"), &["density"]),
            entry("chromaDecompression", marked("Chroma Decompression"), &["chromaDecompression"]),
            entry("lighting", marked("Lighting"), &["levelsGlobal"]),
            entry("vignette", marked("Vignette"), &["vignette"]),
            entry("secretSauce", marked("Chrome"), &["secretSauce"]),
            entry("paperCorrection", marked("Paper Correction"), &["paperCorrection"]),
            entry("manualCurves", marked("Curves"), &["manualCurves", "manualContrast"]),
            entry("densityCeiling", marked("Density Ceiling"), &["densityCeiling"]),
            entry("spectrogram", marked("Spectrogram Finishing"), &["spectrogram"]),
            entry("saturation", marked("Saturation Curve"), &["saturation"]),
        ],
    },
    Block {
        id: "Details",
        title: marked("Details"),
        entries: &[
            entry("sharpen", marked("Sharpening"), &["sharpen"]),
            entry("chroma", marked("Chroma Denoise"), &["chroma"]),
            entry("texture", marked("Texture"), &["texture"]),
        ],
    },
    // One gesture: the keypad's clicks mean the scanner's render, so the face travels with them.
    Block { id: "Scanner", title: marked("Scanner"), entries: &[entry("scanner", marked("Scanner Keypad"), &["scanner", "face"])] },
];

/// The Color block's note on the white balance choice.
pub const WHITE_BALANCE_HELP: &str = marked("Not carried leaves every photo its own balance. Copied WB pastes this photo's gains and per-channel levels as numbers. Automatic WB re-measures each photo it lands on.");

/// What both sheets say about framing, absent from the rows rather than unticked.
pub const CARRIES_HELP: &str = marked("A preset never carries crop, straightening, rotation or the conversion mode.");

/// Shown under the boxes when none is ticked.
pub const CARRIES_NOTHING: &str = marked("Carries nothing \u{2014} applying it would change no setting.");

pub fn entries() -> impl Iterator<Item = &'static Entry> {
    BLOCKS.iter().flat_map(|b| b.entries.iter())
}

/// A choice and not a box: manual and automatic write the same two rows, so two ticks could name
/// a state that means nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteBalance {
    None,
    Manual,
    /// The key, not its numbers: applying it measures the frame it lands on.
    Automatic,
}

impl WhiteBalance {
    pub const ALL: [WhiteBalance; 3] = [WhiteBalance::None, WhiteBalance::Manual, WhiteBalance::Automatic];

    pub fn label(self) -> &'static str {
        match self {
            WhiteBalance::None => tr("Not carried"),
            WhiteBalance::Manual => tr("Copied WB (gains + per-channel levels)"),
            WhiteBalance::Automatic => tr("Automatic WB (Temperature, Tint)"),
        }
    }

    pub fn names(self) -> &'static [&'static str] {
        match self {
            WhiteBalance::None => &[],
            WhiteBalance::Manual => &WHITE_BALANCE_ROWS,
            WhiteBalance::Automatic => &[AUTO_BALANCE_NAME],
        }
    }
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// Read back from a stored selection; the automatic wins, a suggestion writing a complete state.
pub fn white_balance(parameters: &BTreeSet<String>) -> WhiteBalance {
    if parameters::auto_balances(parameters) {
        return WhiteBalance::Automatic;
    }
    if WHITE_BALANCE_ROWS.iter().all(|r| parameters.contains(*r)) { WhiteBalance::Manual } else { WhiteBalance::None }
}

/// Changing the choice clears the one before it rather than stacking.
pub fn with_white_balance(choice: WhiteBalance, parameters: &BTreeSet<String>) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = parameters
        .iter()
        .filter(|p| !WHITE_BALANCE_ROWS.contains(&p.as_str()) && p.as_str() != AUTO_BALANCE_NAME)
        .cloned()
        .collect();
    out.extend(set(choice.names()));
    out
}

/// What a new preset opens on: the rows the photo moved, the white balance not carried.
pub fn proposed(settings: &PipelineSettings) -> BTreeSet<String> {
    let moved: BTreeSet<String> = parameters::changed(settings).into_iter().map(str::to_string).collect();
    with_white_balance(WhiteBalance::None, &moved)
}

pub fn is_on(entry: &Entry, parameters: &BTreeSet<String>) -> bool {
    entry.rows.iter().all(|r| parameters.contains(*r))
}

pub fn with_entry(entry: &Entry, on: bool, parameters: &BTreeSet<String>) -> BTreeSet<String> {
    let mut out = parameters.clone();
    for row in entry.rows {
        if on {
            out.insert(row.to_string());
        } else {
            out.remove(*row);
        }
    }
    out
}

/// A selection named as the interface names it; a row taken without its partner is named raw.
pub fn label(parameters: &BTreeSet<String>) -> String {
    let mut named: Vec<String> = Vec::new();
    let mut covered: BTreeSet<String> = BTreeSet::new();
    let balance = white_balance(parameters);
    if balance != WhiteBalance::None {
        named.push(balance.label().to_string());
        covered.extend(set(&WHITE_BALANCE_ROWS));
        covered.insert(AUTO_BALANCE_NAME.to_string());
    }
    for entry in entries().filter(|e| is_on(e, parameters)) {
        named.push(tr(entry.label).to_string());
        covered.extend(set(entry.rows));
    }
    named.extend(parameters.difference(&covered).cloned());
    match named.len() {
        0 => tr("empty").to_string(),
        1 => named.remove(0),
        n => oni_i18n::tr!("{list} and {last}", list = named[..n - 1].join(", "), last = named[n - 1]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::ConversionMode;

    fn rows(names: &[&str]) -> BTreeSet<String> {
        set(names)
    }

    /// `PresetParameters.selfCheck`: nothing may be shown under no name.
    #[test]
    fn every_row_is_shown_under_one_interface_name() {
        let mut shown: Vec<&str> = entries().flat_map(|e| e.rows.iter().copied()).collect();
        shown.extend(WHITE_BALANCE_ROWS);
        let known: Vec<&str> = parameters::all().iter().map(|r| r.id).collect();
        let unnamed: Vec<&&str> = known.iter().filter(|k| !shown.contains(k)).collect();
        let invented: Vec<&&str> = shown.iter().filter(|s| !known.contains(s)).collect();
        assert!(unnamed.is_empty(), "unnamed: {unnamed:?}");
        assert!(invented.is_empty(), "invented: {invented:?}");
        assert_eq!(shown.len(), shown.iter().collect::<BTreeSet<_>>().len(), "a row shown twice");
        // Twin: withholding the last box names its row back as unnamed.
        let withheld: Vec<&&str> = known.iter().filter(|k| !shown[..shown.len() - 1].contains(k)).collect();
        assert_eq!(withheld, vec![&shown[shown.len() - 1]]);
    }

    #[test]
    fn the_white_balance_is_one_choice_of_three() {
        assert_eq!(white_balance(&rows(&[])), WhiteBalance::None);
        assert_eq!(white_balance(&rows(&["gains", "levelsPerChannel"])), WhiteBalance::Manual);
        assert_eq!(white_balance(&rows(&["gains"])), WhiteBalance::None, "manual needs both rows");
        assert_eq!(white_balance(&set(WhiteBalance::Automatic.names())), WhiteBalance::Automatic);
        // Twin: naming both, the automatic wins over the values.
        assert_eq!(white_balance(&rows(&["gains", "levelsPerChannel", AUTO_BALANCE_NAME])), WhiteBalance::Automatic);
        let auto = set(WhiteBalance::Automatic.names());
        assert!(with_white_balance(WhiteBalance::None, &auto).is_empty());
        assert_eq!(with_white_balance(WhiteBalance::Manual, &auto), set(&WHITE_BALANCE_ROWS), "the choice replaces, never stacks");
    }

    #[test]
    fn a_selection_is_named_as_the_interface_names_it() {
        assert_eq!(label(&rows(&["sharpen"])), "Sharpening");
        assert_eq!(label(&rows(&["gains", "levelsPerChannel"])), "Copied WB (gains + per-channel levels)");
        assert_eq!(label(&rows(&["gains"])), "gains", "a row without its partner is named raw");
        assert_eq!(label(&set(WhiteBalance::Automatic.names())), "Automatic WB (Temperature, Tint)");
        assert_eq!(label(&rows(&["sharpen", "texture", "density"])), "Color Density, Sharpening and Texture");
        assert_eq!(label(&rows(&[])), "empty");
        // Curves names two rows: one alone is named raw, never as the box.
        assert_eq!(label(&rows(&["manualCurves"])), "manualCurves");
    }

    #[test]
    fn a_new_preset_opens_on_the_moved_rows_with_the_balance_not_carried() {
        let mut balanced = PipelineSettings::rest(ConversionMode::Negative, false);
        balanced.gains.stops[0] = 0.8;
        balanced.levels.red.set_black(0.2);
        balanced.texture.amount = 0.4;
        let opening = proposed(&balanced);
        assert_eq!(white_balance(&opening), WhiteBalance::None);
        assert!(opening.contains("texture"));
        // Twin: what the frame moved alone would carry the balance.
        let moved: BTreeSet<String> = parameters::changed(&balanced).into_iter().map(str::to_string).collect();
        assert_eq!(white_balance(&moved), WhiteBalance::Manual);
    }

    #[test]
    fn a_box_ticks_and_unticks_all_its_rows() {
        let curves = entries().find(|e| e.id == "manualCurves").unwrap();
        let on = with_entry(curves, true, &rows(&["sharpen"]));
        assert_eq!(on, rows(&["sharpen", "manualCurves", "manualContrast"]));
        assert!(is_on(curves, &on));
        let off = with_entry(curves, false, &on);
        assert_eq!(off, rows(&["sharpen"]));
        // Twin: one of its two rows alone does not read as ticked.
        assert!(!is_on(curves, &rows(&["manualCurves"])));
    }
}
