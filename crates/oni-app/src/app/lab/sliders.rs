//! The Scanner face's sliders, letters then effects: each one's value and range in steps, and its
//! readout, the letter's count as the screen spells it, an effect in its own unit.

use super::{EFFECTS, same, spelled};
use slint::{ModelRc, SharedString, VecModel};
use oni_link::scanner::{Effect, Letter};
use oni_settings::PipelineSettings;

pub struct Slider {
    pub steps: i32,
    pub low: i32,
    pub high: i32,
    pub readout: String,
}

/// Every slider of `settings`, D, C, M, Y and TEMP, then Contrast, Vignette and Chrome.
pub fn sliders(settings: &PipelineSettings) -> Vec<Slider> {
    let s = &settings.scanner;
    let letters = Letter::ALL.into_iter().zip([s.d, s.c, s.m, s.y, s.w]).map(|(letter, count)| {
        let (low, high) = letter.range();
        Slider { steps: count as i32, low: low.into(), high: high.into(), readout: spelled(count) }
    });
    let f = &settings.scanner_finish;
    let channel = oni_core::store::channel(settings.mode);
    let effects = EFFECTS.into_iter().zip([f.contrast, f.vignette.stops, f.secret_sauce]).map(move |(effect, value)| {
        let (low, high) = effect.range(channel);
        let steps = |v: f32| (v / effect.step()).round() as i32;
        Slider { steps: steps(value), low: steps(low), high: steps(high), readout: unit(effect, value) }
    });
    letters.chain(effects).collect()
}

/// The sliders on the Lab's model, each array written only when it changed.
pub fn publish(lab: &crate::Lab, settings: &PipelineSettings) {
    let sliders = sliders(settings);
    let ints = |f: fn(&Slider) -> i32| sliders.iter().map(f).collect::<Vec<i32>>();
    let (steps, lows, highs) = (ints(|s| s.steps), ints(|s| s.low), ints(|s| s.high));
    let readouts: Vec<SharedString> = sliders.iter().map(|s| s.readout.as_str().into()).collect();
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
}

/// An effect's value as the hand reads it: Contrast and Chrome in percent, Vignette in stops.
fn unit(effect: Effect, value: f32) -> String {
    let sign = if value < 0.0 { "−" } else if value > 0.0 { "+" } else { "" };
    match effect {
        Effect::Vignette => format!("{sign}{:.1} EV", value.abs()),
        Effect::Contrast | Effect::Chrome => format!("{sign}{:.0} %", (value * 100.0).abs()),
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
        assert_eq!((all[1].steps, all[1].low, all[1].high, all[1].readout.as_str()), (4, -24, 24, "+4"));
        assert_eq!((all[4].low, all[4].high), (-10, 10), "TEMP has its own range");
        assert_eq!((all[5].steps, all[5].low, all[5].high, all[5].readout.as_str()), (3, -10, 10, "+15 %"));
        assert_eq!((all[6].steps, all[6].readout.as_str()), (-3, "−0.3 EV"));
        assert_eq!((all[7].low, all[7].high), (0, 30));
        // Twin: a value read in its own unit, not in steps, would put Contrast at 0.
        assert_ne!(all[5].steps, 0);
    }
}
