use super::*;

#[test]
fn the_readouts_print_as_the_swift_formats_do() {
    assert_eq!(Field::Exposure.spec().readout.format(1.5), "+1.50 EV");
    assert_eq!(Field::ColorDensity.spec().readout.format(-0.25), "-25 %");
    assert_eq!(Field::Temperature.spec().readout.format(1.0), "+50");
    assert_eq!(Field::Tint.spec().readout.format(-1.5), "-100");
    assert_eq!(Field::Chrome.spec().readout.format(0.5), "50%");
    assert_eq!(Field::BalanceStrength.spec().readout.format(0.67), "67 %");
    assert_eq!(Field::SharpenRadius.spec().readout.format(2.092), "2.1 px");
    assert_eq!(Field::Angle.spec().readout.format(-1.0), "-1.00°");
    // Twin: Temperature's readout is a share of its top, not the stored stops.
    assert_ne!(Field::Temperature.spec().readout.format(1.0), Readout::Signed.format(1.0));
    // The black point reads in the sense its slider moves: stored negative, shown positive.
    assert!(Field::BlackPoint.readout(-0.1).starts_with('+'), "{}", Field::BlackPoint.readout(-0.1));
    // Twin: the stored value's own sign is what Swift never shows.
    assert!(Field::BlackPoint.spec().readout.format(-0.1).starts_with('-'));
    assert_eq!(Field::DyeSeparation.readout(0.6), "60 %");
}

#[test]
fn every_field_has_its_place_in_the_slint_store() {
    let store = include_str!("../../../oni-ui/ui/store.slint");
    for f in Field::ALL {
        assert_eq!(Field::from_index(f.index()), Some(f));
        for suffix in ["fraction", "readout", "value"] {
            assert!(store.contains(&format!("{}-{suffix};", f.slint_name())), "{f:?} has no {suffix}");
        }
    }
    // The panel passes each slider's own index back.
    let panel = panel_source();
    for f in Field::ALL {
        // Source Balance names each row by the ends the mode gives it, so its label is the panel's.
        let named = |l: &str| l.contains(&format!("label: @tr(\"{}\");", f.spec().label)) || f.block() == Some(Block::SourceBalance);
        let wired = panel.lines().any(|l| {
            l.contains(&format!("field: {}; label: ", f.index())) && l.contains(&format!("fraction: Store.{}-fraction;", f.slint_name())) && named(l)
        });
        assert!(wired, "{f:?} is not wired by its index, label and fraction");
    }
    // Twin: a field the store never declared is caught.
    assert!(!store.contains("grain-fraction;"));
}

#[test]
fn each_slider_spans_its_knobs_range_on_a_mosaic_negative() {
    let s = oni_settings::PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    for f in Field::ALL {
        assert_eq!(f.knob().range(&s), Some(f.spec().range), "{f:?}");
    }
    // Twin: on a plain file the ceiling's axis is another, which the spec does not claim to follow.
    let plain = oni_settings::PipelineSettings::rest(oni_settings::ConversionMode::Negative, false);
    assert_ne!(Field::DensityCeiling.knob().range(&plain), Some(Field::DensityCeiling.spec().range));
}

#[test]
fn a_double_click_on_the_ceiling_lands_on_the_frames_own_rest() {
    use oni_settings::{ConversionMode, PipelineSettings};
    let slide = PipelineSettings::rest(ConversionMode::Positive, true);
    assert_eq!(Field::DensityCeiling.base(&slide), slide.density_ceiling);
    let negative = PipelineSettings::rest(ConversionMode::Negative, true);
    assert_eq!(Field::DensityCeiling.base(&negative), negative.density_ceiling);
    // Twin: the two rests differ, so a base read for the negative alone fails on the slide.
    assert_ne!(slide.density_ceiling, Field::DensityCeiling.spec().base);
    assert_eq!(Field::Exposure.base(&slide), 0.0, "Exposure's base is `Tone.neutral`, never the mode's rest");
    let mut measured = negative.clone();
    let id = oni_settings::Id::new();
    measured.roll_reference = Some(oni_settings::frame::RollReference { id, stops: [0.3, 0.0, -0.2], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
    assert_eq!(Field::GainRed.double_click_base(&measured), 0.0, "a double click lands on Gains.base");
    // Twin: the block's reset lands on the roll's stops.
    assert_eq!(Field::GainRed.base(&measured), 0.3);
}

/// The panel's Slint, its tabs' files after it.
pub(crate) fn panel_source() -> String {
    [
        include_str!("../../../oni-ui/ui/panel.slint"),
        include_str!("../../../oni-ui/ui/panel/state.slint"),
        include_str!("../../../oni-ui/ui/panel/parts.slint"),
        include_str!("../../../oni-ui/ui/panel/roll_tab.slint"),
        include_str!("../../../oni-ui/ui/panel/crop_tab.slint"),
        include_str!("../../../oni-ui/ui/panel/dedust_tab.slint"),
        include_str!("../../../oni-ui/ui/panel/color_tab.slint"),
        include_str!("../../../oni-ui/ui/panel/details_tab.slint"),
    ]
    .concat()
}

/// The `title:` of each block in one pane of the panel, in order.
fn pane_titles(panel: &str, pane: &str) -> Vec<String> {
    let body = &panel[panel.find(&format!("component {pane} ")).expect("the pane")..];
    let next = ["\ncomponent ", "\nexport component "].iter().filter_map(|m| body[1..].find(m)).min();
    let body = &body[..next.map_or(body.len(), |i| i + 1)];
    body.lines().filter_map(|l| l.trim().strip_prefix("title: @tr(\"")?.split('"').next().map(str::to_string)).collect()
}

/// Whether `shown` keeps `order`'s relative order, every one of its blocks found there.
fn follows(shown: &[String], order: &[String]) -> bool {
    let mut at = 0;
    shown.iter().all(|t| match order[at..].iter().position(|o| o == t) {
        Some(i) => {
            at += i + 1;
            true
        }
        None => false,
    })
}

/// `SettingsPane.body`'s sections as drawn: `manualBalance` is a property laid where the body
/// names it, though its text comes later in the file.
fn swift_roll_order(text: &str) -> Vec<String> {
    let sections = |t: &str| -> Vec<String> { t.split("DSSection(\"").skip(1).filter_map(|t| t.split('"').next().map(str::to_string)).collect() };
    let body = &text[text.find("var body").expect("body")..text.find("private var rollBlock").expect("rollBlock")];
    let manual = &text[text.find("private var manualBalance").expect("manualBalance")..text.find("// MARK: - Keys").expect("keys")];
    let at = body.find("manualBalance\n").expect("the body names manualBalance");
    [sections(&body[..at]), sections(manual), sections(&body[at..])].concat()
}

#[test]
fn the_roll_tab_lays_its_blocks_in_the_swift_order() {
    // `SettingsPane.body`, read from the Swift source when it is beside this checkout.
    let swift: Vec<String> = match crate::swift_sources().and_then(|d| std::fs::read_to_string(d.join("UI/SettingsPane.swift")).ok()) {
        Some(text) => swift_roll_order(&text),
        None => ["Measured Roll", "Film Type", "Flat-Field", "Source Encoding", "Source Balance", "Density Ceiling", "Per-Channel Levels", "Dye Separation", "Balance Strength"]
            .map(String::from)
            .to_vec(),
    };
    let shown = pane_titles(&panel_source(), "RollPane");
    assert!(shown.len() >= 6, "{shown:?}");
    assert!(follows(&shown, &swift), "the Roll tab shows {shown:?}, Swift lays {swift:?}");
    // Twin: Dye Separation and Balance Strength ahead of the ceiling break the order.
    let early: Vec<String> = ["Measured Roll", "Film Type", "Flat-Field", "Dye Separation", "Balance Strength", "Density Ceiling"].map(String::from).to_vec();
    assert!(!follows(&early, &swift));
}

#[test]
fn the_black_point_runs_the_other_way() {
    let f = Field::BlackPoint;
    assert_eq!(f.track(), (-Print::BLACK_POINT_RANGE.1, -Print::BLACK_POINT_RANGE.0));
    assert_eq!(f.shown(0.1), -0.1);
    assert_eq!(Field::WhitePoint.shown(0.1), 0.1);
}
