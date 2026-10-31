//! What a preset is chosen from: one row per serialisable concept, taken whole or by its sliders,
//! and the states a reset lands on.

use crate::balance;
use oni_settings::levels::Levels;
use oni_settings::{ConversionMode, PipelineSettings};
use std::collections::BTreeSet;
use std::sync::OnceLock;

type Carry = fn(&PipelineSettings, &mut PipelineSettings);
type SliderCarry = dyn Fn(&PipelineSettings, &mut PipelineSettings) + Send + Sync;

/// Carries one value from a source onto a destination.
pub struct Slider {
    pub id: String,
    pub carry: Box<SliderCarry>,
}

pub struct Row {
    pub id: &'static str,
    /// The `PipelineSettings` fields this row answers for; the completeness check reads it.
    pub covers: &'static [&'static str],
    pub sliders: Vec<Slider>,
    pub carry: Carry,
}

/// Fields outside every row, with the reason: absent rather than unticked, so none is ticked by mistake.
pub const EXCLUDED: [(&str, &str); 16] = [
    ("geometry", "depends on this frame's content: pasting one destroys hand-made cropping"),
    ("mode", "negative or positive is what the frame is, not a look one applies"),
    ("invertedSource", "a positive read as its own negative is what the frame is, as its mode"),
    ("film", "which roll a frame belongs to: pasting a look would move photographs about"),
    ("manualOrder", "a frame's own hand-placed position among its siblings"),
    ("sheet", "a contact sheet's own grid, what this scan is"),
    ("corrections", "a defect on this negative, which pasted would paint dust onto another photograph"),
    ("decodedFromMosaic", "which decoder read the file, a fact of the file setting its axis's rest"),
    ("lensCorrected", "the geometry this frame's dust corrections were placed on"),
    ("primariesConverted", "the decode this frame's windows were measured on"),
    ("rollReference", "the window this frame's roll was measured on, which a look from another roll must not move"),
    ("rollReferenceDeclined", "a choice kept for this frame alone"),
    ("measuredGains", "the gains this slide's own measure placed, which no other frame's measure is"),
    ("captureStops", "how this scan's capture was exposed against its roll's"),
    ("rgbPasses", "the three files this frame is composed of"),
    ("passOf", "the frame a pass belongs to"),
];

fn slider(id: &str, carry: impl Fn(&PipelineSettings, &mut PipelineSettings) + Send + Sync + 'static) -> Slider {
    Slider { id: id.to_string(), carry: Box::new(carry) }
}

fn handles(row: &str, channel: &str, pick: fn(&mut PipelineSettings) -> &mut Levels) -> Vec<Slider> {
    let handle = |name: &str, set: fn(&mut Levels, &Levels)| {
        slider(&format!("{row}.{channel}.{name}"), move |from, to| {
            let mut source = from.clone();
            let value = *pick(&mut source);
            set(pick(to), &value);
        })
    };
    vec![
        handle("black", |l, f| l.black = f.black),
        handle("shadows", |l, f| l.shadows = f.shadows),
        handle("mid", |l, f| l.mid = f.mid),
        handle("highlights", |l, f| l.highlights = f.highlights),
        handle("white", |l, f| l.white = f.white),
    ]
}

macro_rules! carry {
    ($($path:tt)+) => {
        |from: &PipelineSettings, to: &mut PipelineSettings| to.$($path)+ = from.$($path)+.clone()
    };
}

/// Every row, in form order.
pub fn all() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let mut levels = handles("levelsPerChannel", "red", |s| &mut s.levels.red);
        levels.extend(handles("levelsPerChannel", "green", |s| &mut s.levels.green));
        levels.extend(handles("levelsPerChannel", "blue", |s| &mut s.levels.blue));
        vec![
            Row {
                id: "gains",
                covers: &["gains"],
                sliders: vec![
                    slider("gains.red", carry!(gains.stops[0])),
                    slider("gains.green", carry!(gains.stops[1])),
                    slider("gains.blue", carry!(gains.stops[2])),
                ],
                carry: carry!(gains),
            },
            Row { id: "densityCeiling", covers: &["densityCeiling"], sliders: vec![], carry: carry!(density_ceiling) },
            Row {
                id: "separation",
                covers: &["separation", "separationFamily", "separationByHand"],
                sliders: vec![],
                carry: |from, to| {
                    to.separation = from.separation;
                    to.separation_family = from.separation_family;
                    to.separation_by_hand = from.separation_by_hand;
                },
            },
            Row {
                id: "levelsPerChannel",
                covers: &["levels", "balanceMethod"],
                sliders: levels,
                carry: |from, to| {
                    to.levels = from.levels;
                    to.balance_method = from.balance_method.clone();
                },
            },
            // Lighting, under the id stored presets name; the channels' exposures travel with Color.
            Row {
                id: "levelsGlobal",
                covers: &["print"],
                sliders: vec![
                    slider("print.exposure", carry!(print.exposure)),
                    slider("print.contrast", carry!(print.contrast)),
                    slider("print.shadows", carry!(print.shadows)),
                    slider("print.highlights", carry!(print.highlights)),
                    slider("print.blackPoint", carry!(print.black_point)),
                    slider("print.whitePoint", carry!(print.white_point)),
                ],
                carry: |from, to| {
                    let filters = (to.print.red, to.print.green, to.print.blue);
                    to.print = from.print;
                    (to.print.red, to.print.green, to.print.blue) = filters;
                },
            },
            // Temperature, Tint, the balance correction and its share, under the id stored presets name.
            Row {
                id: "curves",
                covers: &["balanceFilters", "balanceStrength"],
                sliders: vec![
                    slider("print.red", carry!(print.red)),
                    slider("print.green", carry!(print.green)),
                    slider("print.blue", carry!(print.blue)),
                    slider("balanceFilters", carry!(balance_filters)),
                    slider("balanceStrength", carry!(balance_strength)),
                ],
                carry: |from, to| {
                    (to.print.red, to.print.green, to.print.blue) = (from.print.red, from.print.green, from.print.blue);
                    (to.balance_filters, to.balance_strength) = (from.balance_filters, from.balance_strength);
                },
            },
            // The other curve set, a row of its own: sharing one would paste the wrong stage's tables.
            Row { id: "manualContrast", covers: &["manualCurves"], sliders: vec![], carry: carry!(manual_curves.contrast) },
            Row {
                id: "manualCurves",
                covers: &["manualCurves"],
                sliders: vec![
                    slider("manualCurves.luma", carry!(manual_curves.luma)),
                    slider("manualCurves.linked", carry!(manual_curves.linked)),
                    slider("manualCurves.red", carry!(manual_curves.red)),
                    slider("manualCurves.green", carry!(manual_curves.green)),
                    slider("manualCurves.blue", carry!(manual_curves.blue)),
                ],
                carry: |from, to| {
                    let contrast = to.manual_curves.contrast;
                    to.manual_curves = from.manual_curves.clone();
                    to.manual_curves.contrast = contrast;
                },
            },
            Row { id: "density", covers: &["density"], sliders: vec![], carry: carry!(density) },
            // Eight bands on two axes are one gesture, so the mixer travels whole or not at all.
            Row { id: "spectrogram", covers: &["spectrogram"], sliders: vec![], carry: carry!(spectrogram) },
            Row { id: "saturation", covers: &["saturation"], sliders: vec![], carry: carry!(saturation) },
            Row { id: "secretSauce", covers: &["secretSauce"], sliders: vec![], carry: carry!(secret_sauce) },
            Row {
                id: "paperCorrection",
                covers: &["paperCorrection"],
                sliders: vec![
                    slider("paperCorrection.red", carry!(paper_correction.stops[0])),
                    slider("paperCorrection.green", carry!(paper_correction.stops[1])),
                    slider("paperCorrection.blue", carry!(paper_correction.stops[2])),
                ],
                carry: carry!(paper_correction),
            },
            Row {
                id: "vignette",
                covers: &["vignette"],
                sliders: vec![slider("vignette.stops", carry!(vignette.stops))],
                carry: carry!(vignette),
            },
            Row {
                id: "scanner",
                covers: &["scanner", "scannerFinish", "scannerMachine"],
                sliders: vec![
                    slider("scanner.d", carry!(scanner.d)),
                    slider("scanner.c", carry!(scanner.c)),
                    slider("scanner.m", carry!(scanner.m)),
                    slider("scanner.y", carry!(scanner.y)),
                    slider("scanner.w", carry!(scanner.w)),
                ],
                carry: |from, to| {
                    to.scanner = from.scanner;
                    (to.scanner_finish, to.scanner_machine) = (from.scanner_finish.clone(), from.scanner_machine);
                },
            },
            // Travels with a preset, so one saved off the scanner opens on it.
            Row { id: "face", covers: &["face"], sliders: vec![], carry: carry!(face) },
            Row {
                id: "chroma",
                covers: &["chroma"],
                sliders: vec![slider("chroma.force", carry!(chroma.force)), slider("chroma.radius", carry!(chroma.radius))],
                carry: carry!(chroma),
            },
            Row {
                id: "sharpen",
                covers: &["sharpen"],
                sliders: vec![
                    slider("sharpen.amount", carry!(sharpen.amount)),
                    slider("sharpen.radius", carry!(sharpen.radius)),
                    slider("sharpen.halo", carry!(sharpen.halo)),
                ],
                carry: carry!(sharpen),
            },
            Row {
                id: "texture",
                covers: &["texture"],
                sliders: vec![slider("texture.amount", carry!(texture.amount))],
                carry: carry!(texture),
            },
            Row { id: "sourceEncoding", covers: &["sourceEncoding"], sliders: vec![], carry: carry!(source_encoding) },
            Row {
                id: "chromaDecompression",
                covers: &["chromaDecompression", "chromaDecompressionByHand"],
                sliders: vec![],
                carry: |from, to| {
                    to.chroma_decompression = from.chroma_decompression;
                    to.chroma_decompression_by_hand = from.chroma_decompression_by_hand;
                },
            },
            Row {
                id: "flatField",
                covers: &["flatField"],
                sliders: vec![
                    // The mask's id rides with the choice of mask, or a pasted Custom names nothing.
                    slider("flatField.mask", |from, to| {
                        to.flat_field.mask = from.flat_field.mask;
                        to.flat_field.custom = from.flat_field.custom.clone();
                    }),
                    slider("flatField.amount", carry!(flat_field.amount)),
                ],
                carry: carry!(flat_field),
            },
        ]
    })
}

/// Carries the chosen rows and sliders onto a destination; a row named whole wins over its sliders.
pub fn apply(chosen: &BTreeSet<String>, source: &PipelineSettings, destination: &PipelineSettings) -> PipelineSettings {
    let mut out = destination.clone();
    for row in all() {
        if chosen.contains(row.id) {
            (row.carry)(source, &mut out);
            continue;
        }
        for slider in row.sliders.iter().filter(|s| chosen.contains(&s.id)) {
            (slider.carry)(source, &mut out);
        }
    }
    out
}

/// An automatic balance is named, not valued: the preset carries the method and each target is measured.
pub const AUTO_BALANCE_PREFIX: &str = "autoBalance.";
pub const AUTO_BALANCE_NAME: &str = "autoBalance.filters";
/// The rows an automatic balance writes, which a manual one writes too: hence one choice on screen.
pub const WHITE_BALANCE_ROWS: [&str; 2] = ["gains", "levelsPerChannel"];

pub fn auto_balances(chosen: &BTreeSet<String>) -> bool {
    chosen.contains(AUTO_BALANCE_NAME)
}

/// Maps a retired automatic-balance key to the live one.
pub fn without_retired_methods(chosen: BTreeSet<String>) -> BTreeSet<String> {
    let kept: BTreeSet<String> = chosen.iter().filter(|c| !c.starts_with(AUTO_BALANCE_PREFIX)).cloned().collect();
    if kept.len() == chosen.len() {
        return chosen;
    }
    let mut out = kept;
    out.insert(AUTO_BALANCE_NAME.to_string());
    out
}

/// The rows a photograph has moved off rest, read through `carry`: sharpening rests on.
pub fn changed(settings: &PipelineSettings) -> BTreeSet<&'static str> {
    let resting = PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic);
    all()
        .iter()
        .filter(|row| {
            let mut carried = resting.clone();
            (row.carry)(settings, &mut carried);
            carried != resting
        })
        .map(|row| row.id)
        .collect()
}

/// What every reset lands on: rest in the frame's mode, plus the file's own facts and its roll.
pub fn imported(settings: &PipelineSettings) -> PipelineSettings {
    let mut block = PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic);
    block.inverted_source = settings.inverted_source;
    block.sheet = settings.sheet.clone();
    block.film = settings.film.clone();
    block.face = settings.face;
    // The framing stays, or every reset costs it again; the flat-field describes the rig.
    block.geometry = settings.geometry;
    block.flat_field = settings.flat_field.clone();
    block.source_encoding = settings.source_encoding;
    block.lens_corrected = settings.lens_corrected;
    block.primaries_converted = settings.primaries_converted;
    block.capture_stops = settings.capture_stops;
    block.rgb_passes = settings.rgb_passes.clone();
    block.pass_of = settings.pass_of.clone();
    // The dye family names the stock; the dose is a look and returns to rest.
    block.separation_family = settings.separation_family;
    block.roll_reference_declined = settings.roll_reference_declined;
    // A slide measured on its gains alone stands on them, as a negative stands on its roll.
    if settings.roll_reference.is_none()
        && let Some(gains) = settings.measured_gains
    {
        block.measured_gains = Some(gains);
        block.gains.stops = gains;
    }
    match &settings.roll_reference {
        Some(reference) => balance::written(reference, &block),
        None => block,
    }
}

/// Taken back to unconverted: measurement and look go, framing, flat-field and dust corrections stay.
pub fn unconverted(settings: &PipelineSettings) -> PipelineSettings {
    let mut unmeasured = settings.clone();
    unmeasured.roll_reference = None;
    unmeasured.measured_gains = None;
    let mut block = imported(&unmeasured);
    block.corrections = settings.corrections.clone();
    block.capture_stops = 0.0;
    block.primaries_converted = false;
    (block.rgb_passes, block.pass_of, block.roll_reference_declined) = (None, None, false);
    block
}

/// The bar's reset: the look back to rest and the crop gone, the measure's conversion kept, a roll's
/// reference or a slide's measured gains (owner's call). On the Scanner face Chrome is part of the
/// face, the clicks are the edit.
pub fn reset_blank(settings: &PipelineSettings, scanner_face: bool) -> PipelineSettings {
    let mut blank = imported(settings);
    blank.geometry = oni_settings::frame::Geometry::reset_keeping(&settings.geometry);
    if scanner_face {
        blank.secret_sauce = settings.secret_sauce;
    } else {
        blank.scanner = settings.scanner;
    }
    blank
}

/// A mode change carrying the axis and the windows still at one mode's rest onto the other's.
pub fn carried(block: &PipelineSettings, next: ConversionMode) -> PipelineSettings {
    use oni_settings::axis::density_axis;
    let mut moved = block.clone();
    moved.mode = next;
    // A positive read as a negative is one as long as it stays a negative.
    moved.inverted_source &= next == ConversionMode::Negative;
    moved.density_ceiling = density_axis::carried(block.density_ceiling, block.decoded_from_mosaic, block.mode, next);
    moved.levels = carried_levels(block, next, moved.density_ceiling);
    if block.print.exposure == oni_settings::tone::Print::resting_exposure(block.mode) {
        moved.print.exposure = oni_settings::tone::Print::resting_exposure(next);
    }
    moved
}

/// Moves the blacks still on one mode's rest onto the other's, leaving a placed one where it is.
fn carried_levels(block: &PipelineSettings, next: ConversionMode, to_ceiling: f32) -> oni_settings::levels::LevelsSet {
    let mut out = block.levels;
    if block.mode == next {
        return out;
    }
    let rest = Levels::resting_black(block.mode, block.density_ceiling);
    for window in [&mut out.red, &mut out.green, &mut out.blue] {
        if window.black == rest {
            window.set_black(Levels::resting_black(next, to_ceiling));
        }
    }
    out
}

#[cfg(test)]
mod tests;
