//! The Scanner face's sliders, letters then effects: each one's value and range in steps, and its
//! readout in a true unit, a letter's in what it moves, an effect as Pro's slider of the same quantity.

use super::{EFFECTS, field, same};
use crate::fields::{Axis, Readout};
use slint::{ModelRc, SharedString, VecModel};
use oni_link::scanner::Letter;
use oni_pipeline::scanner::MACHINE;
use oni_settings::PipelineSettings;
use oni_settings::scanner::ScannerClicks;

pub struct Slider {
    pub steps: i32,
    pub low: i32,
    pub high: i32,
    pub readout: String,
    /// What one step reads, in the readout's unit: a number typed over the readout finds its steps.
    pub per_step: f32,
}

/// What a letter's `steps` move, signed as the key: D, C, M and Y the light in stops, the measured
/// click's density on the key's channel, D's the mean of three; TEMP the share of the machine's look.
fn moved(letter: Letter, steps: i64) -> f32 {
    let sign = steps.signum() as f32;
    let mut keys = ScannerClicks::default();
    let (key, channel) = match letter {
        Letter::D => (&mut keys.d, None),
        Letter::C => (&mut keys.c, Some(0)),
        Letter::M => (&mut keys.m, Some(1)),
        Letter::Y => (&mut keys.y, Some(2)),
        Letter::W => return sign * MACHINE.warmth(steps).map_or(0.0, |(_, share)| share),
    };
    *key = steps;
    let density = MACHINE.offset(&keys);
    let on = channel.map_or(density.iter().sum::<f32>() / 3.0, |c| density[c]);
    sign * on.abs() / 2f32.log10()
}

/// How a letter writes what it moves: stops, or TEMP's share in percent after the side it leans to.
fn readout(letter: Letter) -> (Readout, Option<Axis>) {
    match letter {
        Letter::W => (Readout::SignedPercent, Some(Axis::Warmth)),
        _ => (Readout::Stops, None),
    }
}

/// Every slider of `settings`, D, C, M, Y and Temperature, then Contrast, Vignette and Chrome.
pub fn sliders(settings: &PipelineSettings) -> Vec<Slider> {
    let s = &settings.scanner;
    let letters = Letter::ALL.into_iter().zip([s.d, s.c, s.m, s.y, s.w]).map(|(letter, count)| {
        let ((low, high), (kind, axis)) = (letter.range(), readout(letter));
        let per_step = kind.scaled(moved(letter, 1)) as f32;
        Slider { steps: count as i32, low: low.into(), high: high.into(), readout: kind.along(axis, moved(letter, count)), per_step }
    });
    let f = &settings.scanner_finish;
    let channel = oni_core::store::channel(settings.mode);
    let effects = EFFECTS.into_iter().zip([f.contrast, f.vignette.stops, f.secret_sauce]).map(move |(effect, value)| {
        let (low, high) = effect.range(channel);
        let steps = |v: f32| (v / effect.step()).round() as i32;
        let per_step = field(effect).spec().readout.scaled(effect.step()) as f32;
        Slider { steps: steps(value), low: steps(low), high: steps(high), readout: field(effect).readout(value), per_step }
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
        assert_eq!((all[1].steps, all[1].low, all[1].high), (4, -48, 48));
        assert_eq!((all[4].low, all[4].high), (-20, 20), "Temperature has its own range, ±10 units in half steps");
        // A notch of the rail is one step, half a unit: 97 places on density, 41 on Temperature.
        assert_eq!((all[0].high - all[0].low + 1, all[4].high - all[4].low + 1), (97, 41));
        assert_eq!((all[5].steps, all[5].low, all[5].high, all[5].readout.as_str()), (3, -10, 10, "+15%"));
        assert_eq!((all[6].steps, all[6].readout.as_str()), (-3, "−0.30 EV"));
        assert_eq!(all[6].readout, crate::fields::Field::Vignette.readout(-0.3), "one format on both faces");
        assert_eq!((all[7].low, all[7].high), (0, 30));
        // Twin: a value read in its own unit, not in steps, would put Contrast at 0.
        assert_ne!(all[5].steps, 0);
    }

    #[test]
    fn a_letter_reads_what_it_moves_in_a_true_unit() {
        let mut settings = PipelineSettings::default();
        settings.scanner = ScannerClicks::new(3, 4, 0, -2, -4);
        let all = sliders(&settings);
        let read: Vec<&str> = all[..5].iter().map(|s| s.readout.as_str()).collect();
        // C's whole click lifts red 0.039 in density, 0.13 stop; D's three quarters move the three 0.051.
        assert_eq!(read, ["+0.17 EV", "+0.13 EV", "0.00 EV", "\u{2212}0.08 EV", "Cool \u{2212}20%"]);
        // Twin: the screen's count, the one the Lab's glass shows, says neither unit nor size.
        assert_eq!(oni_link::scanner::spelled(settings.scanner.c), "+2");
        // Twin: C read on green, the channel it barely touches, would read nothing.
        let c = MACHINE.offset(&ScannerClicks::new(0, 4, 0, 0, 0));
        assert_eq!(Readout::Stops.format(c[1] / 2f32.log10()), "0.00 EV");
    }

    #[test]
    fn a_number_typed_over_a_scanner_readout_finds_its_steps() {
        let all = sliders(&PipelineSettings::default());
        let per: Vec<f32> = all.iter().map(|s| s.per_step).collect();
        let near = |a: f32, b: f32| (a - b).abs() < 5e-4;
        // A press moves D 0.057 EV, C 0.033, M 0.027, Y 0.039, TEMP 5% of the look; an effect 5% or 0.1 EV.
        let expected = [0.0568, 0.0325, 0.0265, 0.0387, 5.0, 5.0, 0.1, 5.0];
        assert!(per.iter().zip(expected).all(|(a, b)| near(*a, b)), "{per:?}");
        let typed = |i: usize, text: &str| crate::slider_track::typed_steps(&all[i].readout, text, all[i].per_step);
        assert_eq!((typed(1, "+0.26"), typed(4, "-15"), typed(5, "-15"), typed(6, "0.4")), (8, -3, -3, 4));
        // Twin: counted in the readout's last digit, Contrast's 15% would be fifteen steps, not three.
        assert_eq!(crate::slider_track::typed_steps(&all[5].readout, "15", 0.0), 15);
    }
}
