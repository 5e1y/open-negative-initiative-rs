//! The Scanner face's sliders, letters then effects: each one's value and range in steps, and its
//! readout, a letter's steps in the screen's unit, an effect in its own.

use super::{EFFECTS, percent, same, spelled, stops};
use slint::{ModelRc, SharedString, VecModel};
use oni_link::scanner::{Effect, Letter};
use oni_settings::PipelineSettings;

pub struct Slider {
    pub steps: i32,
    pub low: i32,
    pub high: i32,
    pub readout: String,
    /// What one step reads, in the readout's unit: a number typed over the readout finds its steps.
    pub per_step: f32,
}

/// A readout's number, as `Typing` reads it.
fn read(readout: &str) -> f32 {
    crate::slider_track::read(readout).map_or(0.0, |(n, _)| n as f32)
}

/// Every slider of `settings`, D, C, M, Y and TEMP, then Contrast, Vignette and Chrome.
pub fn sliders(settings: &PipelineSettings) -> Vec<Slider> {
    let s = &settings.scanner;
    let letters = Letter::ALL.into_iter().zip([s.d, s.c, s.m, s.y, s.w]).map(|(letter, count)| {
        let (low, high) = letter.range();
        Slider { steps: count as i32, low: low.into(), high: high.into(), readout: spelled(count), per_step: read(&spelled(1)) }
    });
    let f = &settings.scanner_finish;
    let channel = oni_core::store::channel(settings.mode);
    let effects = EFFECTS.into_iter().zip([f.contrast, f.vignette.stops, f.secret_sauce]).map(move |(effect, value)| {
        let (low, high) = effect.range(channel);
        let steps = |v: f32| (v / effect.step()).round() as i32;
        Slider { steps: steps(value), low: steps(low), high: steps(high), readout: unit(effect, value), per_step: read(&unit(effect, effect.step())) }
    });
    letters.chain(effects).collect()
}

/// The sliders on the Lab's model, each array written only when it changed.
pub fn publish(lab: &crate::Lab, settings: &PipelineSettings) {
    let sliders = sliders(settings);
    let ints = |f: fn(&Slider) -> i32| sliders.iter().map(f).collect::<Vec<i32>>();
    let (steps, lows, highs) = (ints(|s| s.steps), ints(|s| s.low), ints(|s| s.high));
    let readouts: Vec<SharedString> = sliders.iter().map(|s| s.readout.as_str().into()).collect();
    let per_step: Vec<f32> = sliders.iter().map(|s| s.per_step).collect();
    if !same(&lab.get_steps(), &steps) {
        lab.set_steps(ModelRc::new(VecModel::from(steps)));
    }
    if !same(&lab.get_lows(), &lows) || !same(&lab.get_highs(), &highs) {
        lab.set_lows(ModelRc::new(VecModel::from(lows)));
        lab.set_highs(ModelRc::new(VecModel::from(highs)));
    }
    if !same(&lab.get_readouts(), &readouts) {
        lab.set_readouts(ModelRc::new(VecModel::from(readouts)));
    }
    if !same(&lab.get_step_reads(), &per_step) {
        lab.set_step_reads(ModelRc::new(VecModel::from(per_step)));
    }
}

/// An effect's value as the screen writes it: Contrast and Chrome in percent, Vignette in stops.
fn unit(effect: Effect, value: f32) -> String {
    match effect {
        Effect::Vignette => stops(value),
        Effect::Contrast | Effect::Chrome => percent(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_slider_reads_its_value_and_range_in_steps() {
        let mut settings = PipelineSettings::default();
        settings.scanner.c = 4;
        settings.scanner_finish.contrast = 0.15;
        settings.scanner_finish.vignette.stops = -0.3;
        let all = sliders(&settings);
        assert_eq!(all.len(), 8);
        assert_eq!((all[1].steps, all[1].low, all[1].high, all[1].readout.as_str()), (4, -48, 48, "+2"));
        assert_eq!((all[4].low, all[4].high), (-20, 20), "TEMP has its own range, ±10 units in half steps");
        // A notch of the rail is one step, half a unit: 97 places on density, 41 on TEMP.
        assert_eq!((all[0].high - all[0].low + 1, all[4].high - all[4].low + 1), (97, 41));
        assert_eq!((all[5].steps, all[5].low, all[5].high, all[5].readout.as_str()), (3, -10, 10, "+15%"));
        assert_eq!((all[6].steps, all[6].readout.as_str()), (-3, "−0.3 EV"));
        assert_eq!((all[7].low, all[7].high), (0, 30));
        // Twin: a value read in its own unit, not in steps, would put Contrast at 0.
        assert_ne!(all[5].steps, 0);
    }

    #[test]
    fn a_number_typed_over_a_scanner_readout_finds_its_steps() {
        let all = sliders(&PipelineSettings::default());
        let per: Vec<f32> = all.iter().map(|s| s.per_step).collect();
        assert_eq!(per, [0.5, 0.5, 0.5, 0.5, 0.5, 5.0, 0.1, 5.0], "half a unit a letter, 5% or 0.1 EV an effect");
        let typed = |i: usize, text: &str| crate::slider_track::typed_steps(&all[i].readout, text, all[i].per_step);
        assert_eq!((typed(1, "+3.5"), typed(5, "-15"), typed(6, "0.4")), (7, -3, 4));
        // Twin: counted in the readout's last digit, Contrast's 15% would be fifteen steps, not three.
        assert_eq!(crate::slider_track::typed_steps(&all[5].readout, "15", 0.0), 15);
    }
}
