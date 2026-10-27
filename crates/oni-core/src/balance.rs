//! The automatic balance key: it runs only on a click, reads the image through a measurement the
//! host provides, and writes only what it shows, one undoable step.

use oni_settings::axis::graduation;
use oni_settings::blocks::{chroma_decompression, separation};
use oni_settings::frame::RollReference;
use oni_settings::geometry::proposal;
use oni_settings::levels::{Levels, LevelsChannel};
use oni_settings::settings::roll_settings;
use oni_settings::{ConversionMode, PipelineSettings};

/// A channel's handles as fractions of the graduation window, not the densities the handles store.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Suggestion {
    pub black: f32,
    pub white: f32,
    pub mid: f32,
    pub shadows: f32,
    pub highlights: f32,
}

impl Suggestion {
    pub fn ends(black: f32, white: f32) -> Suggestion {
        Suggestion { black, white, mid: 0.5, shadows: 0.25, highlights: 0.75 }
    }
}

pub fn placed(suggestion: &Suggestion, mode: ConversionMode) -> Levels {
    Levels {
        black: graduation::value(suggestion.black, mode),
        white: graduation::value(suggestion.white, mode),
        mid: suggestion.mid,
        shadows: suggestion.shadows,
        highlights: suggestion.highlights,
    }
}

/// The two steps as one value: the gains, then each channel's ends at those gains.
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    pub stops: [f32; 3],
    pub levels: [Suggestion; 3],
}

impl Balance {
    pub fn of(reference: &RollReference) -> Balance {
        Balance {
            stops: reference.stops,
            levels: [0, 1, 2].map(|c| Suggestion::ends(reference.black[c], reference.white[c])),
        }
    }
}

/// The balance laid on a frame; its fractions land on that frame's own axis.
pub fn applied(balance: &Balance, settings: &PipelineSettings) -> PipelineSettings {
    let mut out = settings.clone();
    out.gains.stops = balance.stops;
    // A key reads the film's own colour and its dyes' whole spill, unless the hand dialled that dose.
    if !settings.chroma_decompression_by_hand {
        out.chroma_decompression = chroma_decompression::BASE;
    }
    if !settings.separation_by_hand {
        out.separation = separation::BASE;
    }
    out.levels.red = placed(&balance.levels[0], settings.mode);
    out.levels.green = placed(&balance.levels[1], settings.mode);
    out.levels.blue = placed(&balance.levels[2], settings.mode);
    out
}

/// The Auto levels key's balance on a frame: gains and windows, no key named.
pub fn levelled(balance: &Balance, settings: &PipelineSettings) -> PipelineSettings {
    let mut out = applied(balance, settings);
    out.balance_method = None;
    out
}

/// A frame standing on its roll's reference; no key named, so a paste inside the roll carries every handle raw.
pub fn written(reference: &RollReference, settings: &PipelineSettings) -> PipelineSettings {
    let mut out = applied(&Balance::of(reference), settings);
    out.balance_method = None;
    out.roll_reference = Some(reference.clone());
    out.roll_reference_declined = false;
    out
}

/// `EditorScene.unbalanced`: no balance key named, the gains and per-channel windows at rest.
pub fn unbalanced(settings: &PipelineSettings) -> bool {
    let rest = PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic);
    settings.balance_method.is_none()
        && settings.gains == rest.gains
        && LevelsChannel::PER_CHANNEL.iter().all(|&c| settings.levels.get(c) == rest.levels.get(c))
}

/// `adopt`: a frame never balanced opens where a balance begins, the window its key proposes armed;
/// a sheet drops the margin an earlier opening proposed, so it reads its grid. Whether it is armed.
/// A slide is never measured, so it never opens on a window (owner's call, beyond Swift).
pub fn opening_window(settings: &mut PipelineSettings) -> bool {
    let sheet = settings.sheet.is_some();
    let armed = unbalanced(settings) && !sheet && settings.mode != ConversionMode::Positive;
    let g = &mut settings.geometry;
    if armed {
        g.propose_measure_window(false);
    }
    if sheet && g.measure_window == Some(proposal(g.crop, false)) {
        g.measure_window = None;
    }
    armed
}

/// What the balance key found on one frame: the roll it stands on, none for a slide, which is read
/// as it stands, and the full correction.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub reference: Option<RollReference>,
    /// Each channel's exposure on the print that would print the frame's light grey.
    pub filters: [f32; 3],
}

/// The outcome laid on a frame, dosed by its Balance strength.
pub fn applied_outcome(outcome: &Outcome, settings: &PipelineSettings) -> PipelineSettings {
    let mut out = match (&settings.roll_reference, &outcome.reference) {
        (None, Some(reference)) => written(reference, settings),
        _ => settings.clone(),
    };
    out.balance_method = None;
    out.balance_filters = Some(outcome.filters);
    let strength = out.balance_strength;
    roll_settings::dosed(&out, strength)
}

/// Every outcome carrying the mean of all their corrections; each keeps its own roll reference.
pub fn averaged(outcomes: &[Outcome]) -> Vec<Outcome> {
    let count = outcomes.len().max(1) as f32;
    let mut mean = [0.0f32; 3];
    for outcome in outcomes {
        for (sum, value) in mean.iter_mut().zip(outcome.filters) {
            *sum += value;
        }
    }
    let mean = mean.map(|v| v / count);
    outcomes.iter().map(|o| Outcome { reference: o.reference.clone(), filters: mean }).collect()
}

/// Reads one frame through the host's renderer: the store itself holds no pixels.
pub trait Measure {
    /// The key's outcome on the frame at `path` with these settings, `None` where nothing reads.
    fn balance(&self, path: &str, settings: &PipelineSettings) -> Option<Outcome>;
}

impl<F: Fn(&str, &PipelineSettings) -> Option<Outcome>> Measure for F {
    fn balance(&self, path: &str, settings: &PipelineSettings) -> Option<Outcome> {
        self(path, settings)
    }
}

/// How a press over several frames lands: each for itself, or every one on the mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    Each,
    Average,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmeasurable(pub String);

impl std::fmt::Display for Unmeasurable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = std::path::Path::new(&self.0).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        f.write_str(&oni_i18n::tr!("{name}: nothing to balance. The file could not be read, or one of its channels is too flat to place points in.", name = name))
    }
}

/// The frames to write, in order. Each lands what it read before an unreadable frame stops the
/// run; Average needs every reading, so one unreadable frame writes nothing.
pub fn run(
    frames: &[(String, PipelineSettings)],
    strategy: Strategy,
    measure: &dyn Measure,
) -> (Vec<(String, Outcome)>, Option<Unmeasurable>) {
    let mut found = Vec::with_capacity(frames.len());
    for (path, settings) in frames {
        match measure.balance(path, settings) {
            Some(outcome) => found.push((path.clone(), outcome)),
            None if strategy == Strategy::Each => return (found, Some(Unmeasurable(path.clone()))),
            None => return (Vec::new(), Some(Unmeasurable(path.clone()))),
        }
    }
    if strategy == Strategy::Average {
        let mean = averaged(&found.iter().map(|(_, o)| o.clone()).collect::<Vec<_>>());
        found = found.into_iter().map(|(p, _)| p).zip(mean).collect();
    }
    (found, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::Id;

    fn reference() -> RollReference {
        RollReference { id: Id::new(), stops: [0.2, 0.0, -0.3], black: [0.1, 0.12, 0.15], white: [0.8, 0.82, 0.9], base: None, median: None }
    }

    #[test]
    fn a_frame_written_onto_its_roll_takes_the_rolls_gains_and_windows() {
        let r = reference();
        let rest = PipelineSettings::rest(ConversionMode::Negative, true);
        let on = written(&r, &rest);
        assert_eq!(on.gains.stops, r.stops);
        assert_eq!(on.levels.blue.black, graduation::value(0.15, ConversionMode::Negative));
        assert_eq!(on.roll_reference.as_ref(), Some(&r));
        // Twin: the same fraction placed in positive mode lands elsewhere on the axis.
        assert_ne!(on.levels.blue.black, graduation::value(0.15, ConversionMode::Positive));
    }

    #[test]
    fn the_outcome_is_dosed_by_balance_strength() {
        let outcome = Outcome { reference: Some(reference()), filters: [0.4, 0.0, -0.4] };
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.balance_strength = 0.5;
        let out = applied_outcome(&outcome, &s);
        assert_eq!(out.balance_filters, Some([0.4, 0.0, -0.4]));
        assert_eq!(out.print.red.exposure, 0.2);
        // Twin: at full strength the same outcome writes the whole correction.
        s.balance_strength = 1.0;
        assert_eq!(applied_outcome(&outcome, &s).print.red.exposure, 0.4);
    }

    #[test]
    fn average_gives_every_frame_the_mean_and_an_unreadable_frame_stops_the_run() {
        let r = reference();
        let frames: Vec<(String, PipelineSettings)> =
            ["a", "b"].iter().map(|p| (p.to_string(), PipelineSettings::default())).collect();
        let measure = |path: &str, _: &PipelineSettings| {
            Some(Outcome { reference: Some(r.clone()), filters: if path == "a" { [0.2, 0.0, 0.0] } else { [0.4, 0.0, 0.0] } })
        };
        let each = run(&frames, Strategy::Each, &measure).0;
        let mean = run(&frames, Strategy::Average, &measure).0;
        assert_eq!((each[0].1.filters[0], each[1].1.filters[0]), (0.2, 0.4));
        assert!((mean[0].1.filters[0] - 0.3).abs() < 1e-6 && mean[0].1.filters == mean[1].1.filters);
        // Twin: an unreadable frame stops Each after the frames before it, and Average before any.
        let blind = |path: &str, s: &PipelineSettings| if path == "b" { None } else { measure(path, s) };
        let (landed, failure) = run(&frames, Strategy::Each, &blind);
        assert_eq!((landed.len(), failure), (1, Some(Unmeasurable("b".into()))));
        assert!(run(&frames, Strategy::Average, &blind).0.is_empty());
    }

    #[test]
    fn a_frame_never_balanced_opens_with_its_window_proposed_and_armed() {
        let mut fresh = PipelineSettings::rest(ConversionMode::Negative, true);
        assert!(opening_window(&mut fresh));
        assert_eq!(fresh.geometry.measure_window, Some(proposal(fresh.geometry.crop, false)));
        // A slide, never measured, opens as it stands: no window proposed, none armed.
        let mut slide = PipelineSettings::rest(ConversionMode::Positive, true);
        assert!(unbalanced(&slide) && !opening_window(&mut slide) && slide.geometry.measure_window.is_none());
        let mut balanced = written(&reference(), &PipelineSettings::rest(ConversionMode::Negative, true));
        assert!(!unbalanced(&balanced));
        assert!(!opening_window(&mut balanced) && balanced.geometry.measure_window.is_none(), "a balanced frame opens as it was");
        let mut named = PipelineSettings::rest(ConversionMode::Negative, true);
        named.balance_method = Some("balance".into());
        assert!(!unbalanced(&named), "a key named is a balance");
        // A sheet reads its grid: the margin an earlier opening proposed is dropped, never proposed again.
        let mut sheet = PipelineSettings::rest(ConversionMode::Negative, true);
        sheet.sheet = Some(oni_settings::frame::ContactSheetGrid { columns: 2, rows: 1, frames: 2, tile: oni_settings::cg::Size { width: 1.0, height: 1.0 }, margin: 0.0, windows: None });
        sheet.geometry.measure_window = Some(proposal(sheet.geometry.crop, false));
        assert!(!opening_window(&mut sheet));
        assert_eq!(sheet.geometry.measure_window, None);
        // Twin: a window the hand drew on the sheet stays.
        let drawn = oni_settings::cg::Rect { x: 0.2, y: 0.2, width: 0.3, height: 0.3 };
        sheet.geometry.measure_window = Some(drawn);
        opening_window(&mut sheet);
        assert_eq!(sheet.geometry.measure_window, Some(drawn));
    }
}
