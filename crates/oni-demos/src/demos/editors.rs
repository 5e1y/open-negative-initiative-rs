//! The three editors of the Color tab, each stroke laid by the app's own gesture code: Curves'
//! press and drag, the wheel's grab, the saturation curve's presses.

use oni_app::editors::geometry::{Drawing, Size};
use oni_app::editors::{curve, saturation, spectrogram};
use oni_settings::PipelineSettings;
use oni_settings::cg::Point;
use oni_settings::levels::LevelsChannel;
use oni_settings::spectrogram::Band;

use super::editing::{Editor, Edits, Stroke};

/// Curves: a point raised in the middle, then one lowered in the shadows, the print brightening
/// and gaining contrast.
pub fn curves() -> Edits {
    Edits::new("curves", Box::new(Curves))
}

/// Spectrogram Finishing: the yellow band turned toward green, then the red band pulled out.
pub fn spectrogram() -> Edits {
    Edits::new("spectrogram", Box::new(Wheel))
}

/// Saturation Curve: the middle tones raised, then the shadows lowered toward grey.
pub fn saturation_curve() -> Edits {
    Edits::new("saturation-curve", Box::new(Saturation))
}

struct Curves;

impl Editor for Curves {
    fn strokes(&self, size: Size, _s: &PipelineSettings) -> Vec<Stroke> {
        let at = |x: f64, y: f64| curve::screen(Point::new(x, y), size);
        vec![(at(0.5, 0.5), at(0.5, 0.64)), (at(0.2, 0.2), at(0.2, 0.12))]
    }

    fn edit(&self, s: &PipelineSettings, strokes: &[Stroke], _held: bool, size: Size) -> (PipelineSettings, Drawing) {
        let mut out = s.clone();
        let c = out.manual_curves.get_mut(LevelsChannel::Linked);
        for (a, b) in strokes {
            if let Some(i) = curve::press(c, *a, size) {
                curve::drag(c, i, *b, size);
            }
        }
        let drawing = curve::drawing(out.manual_curves.get(LevelsChannel::Linked), LevelsChannel::Linked, size);
        (out, drawing)
    }
}

struct Wheel;

impl Wheel {
    fn wheel(size: Size) -> spectrogram::Wheel {
        spectrogram::Wheel { side: size.w }
    }
}

impl Editor for Wheel {
    fn plot(&self) -> (bool, bool) {
        (false, false)
    }

    /// Larger than a curve's, as the wheel draws its eight bands small inside its square.
    fn side(&self) -> f32 {
        132.0
    }

    fn strokes(&self, size: Size, s: &PipelineSettings) -> Vec<Stroke> {
        let wheel = Wheel::wheel(size);
        let c = wheel.centre();
        // Yellow, the sand and the cream sail, turned most of its room toward green, the radius kept.
        let yellow = wheel.tip(Band::Yellow, &s.spectrogram);
        let r = ((yellow.0 - c.0).powi(2) + (yellow.1 - c.1).powi(2)).sqrt();
        let angle = f64::from(Band::Yellow.centre() + 0.7 * Band::Yellow.hue_range().1).to_radians();
        let turned = (c.0 + r * angle.cos(), c.1 - r * angle.sin());
        // Red, the sail, pulled out along its spoke, stronger.
        let red = wheel.tip(Band::Red, &s.spectrogram);
        let out = (c.0 + (red.0 - c.0) * 1.6, c.1 + (red.1 - c.1) * 1.6);
        vec![(yellow, turned), (red, out)]
    }

    fn edit(&self, s: &PipelineSettings, strokes: &[Stroke], _held: bool, size: Size) -> (PipelineSettings, Drawing) {
        let wheel = Wheel::wheel(size);
        let mut out = s.clone();
        let mut active = None;
        for (a, b) in strokes {
            let grab = spectrogram::grab(wheel, *a, &out.spectrogram);
            out.spectrogram = spectrogram::dragged(wheel, grab, (b.0 - a.0, b.1 - a.1), &out.spectrogram);
            active = Some(grab.grip);
        }
        // The point a press took keeps the focus, lit, as in the panel.
        let drawing = spectrogram::drawing(wheel, &out.spectrogram, active, None);
        (out, drawing)
    }
}

struct Saturation;

impl Editor for Saturation {
    fn plot(&self) -> (bool, bool) {
        (true, true)
    }

    fn strokes(&self, size: Size, s: &PipelineSettings) -> Vec<Stroke> {
        let at = |i: usize| saturation::screen(s.saturation.points[i], size);
        // The middle raised, the sand richer; then the shadows lowered, the red sail toward grey.
        let (mid, low) = (at(2), at(1));
        vec![(mid, (mid.0, mid.1 - 0.3 * size.h)), (low, (low.0, low.1 + 0.16 * size.h))]
    }

    fn edit(&self, s: &PipelineSettings, strokes: &[Stroke], held: bool, size: Size) -> (PipelineSettings, Drawing) {
        let mut out = s.clone();
        let mut presses = saturation::Presses::default();
        for (a, b) in strokes {
            presses.release();
            presses.press(&mut out.saturation, *a, size);
            presses.drag(&mut out.saturation, *b, size);
        }
        if !held {
            presses.release();
        }
        let drawing = saturation::drawing(&out.saturation, presses.dragged, size);
        (out, drawing)
    }
}
