//! Roll › Flat-Field: a scan lit unevenly by its panel, the corners falling off; the hand turns
//! the flat-field on and the light evens out, then off again.

use oni_settings::PipelineSettings;
use oni_settings::frame::FlatFieldMask;
use slint_interpreter::Value;

use super::Picture;
use super::gesture::{Clicks, States};
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo};
use oni_app::render::Linear;
use crate::stage::Stage;

/// How much of the shipped panel's fall-off the scan is lit with, on top of its own, which the
/// photograph's settings take out at 0.27: with it, the Amount at its most takes both out.
const UNEVEN: f32 = 0.73;
/// The clicks: on, then off.
const CLICKS: [f32; 2] = [1.0, 3.4];

pub struct FlatField {
    settings: PipelineSettings,
    clicks: Option<Clicks>,
    states: States,
}

impl FlatField {
    pub fn new() -> FlatField {
        FlatField { settings: photo::base(), clicks: None, states: States::default() }
    }
}

/// The scan as the shipped panel lights it: each pixel by the mask stretched on the frame, divided
/// by its green peak as the kernel divides it, dosed at `UNEVEN`.
fn lit(l: &mut Linear) {
    let mask = oni_pipeline::flat_field::valoi();
    let peak = mask.peaks[1];
    let (mw, mh) = (mask.width as f32, mask.height as f32);
    for y in 0..l.height {
        for x in 0..l.width {
            let mx = ((x as f32 + 0.5) / l.width as f32 * mw - 0.5).clamp(0.0, mw - 1.0);
            let my = ((y as f32 + 0.5) / l.height as f32 * mh - 0.5).clamp(0.0, mh - 1.0);
            let m = bilinear(&mask, mx, my);
            let p = &mut l.pixels[y * l.width + x];
            for c in 0..3 {
                p[c] *= 1.0 + UNEVEN * (m[c] / peak - 1.0);
            }
        }
    }
}

fn bilinear(mask: &oni_pipeline::flat_field::Mask, x: f32, y: f32) -> [f32; 3] {
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(mask.width as usize - 1), (y0 + 1).min(mask.height as usize - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: usize, y: usize| mask.pixels[y * mask.width as usize + x];
    std::array::from_fn(|c| {
        let top = at(x0, y0)[c] * (1.0 - fx) + at(x1, y0)[c] * fx;
        let bottom = at(x0, y1)[c] * (1.0 - fx) + at(x1, y1)[c] * fx;
        top * (1.0 - fy) + bottom * fy
    })
}

impl Demo for FlatField {
    fn topic(&self) -> &'static str {
        "flat-field"
    }

    fn component(&self) -> &'static str {
        "PhotoToggle"
    }


    fn seconds(&self) -> f32 {
        5.0
    }

    fn prepare(&mut self, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        let (w, h) = Picture::of(stage).pixels();
        photo.restage(&self.settings, lit)?;
        let (mut off, mut on) = (self.settings.clone(), self.settings.clone());
        off.flat_field.mask = FlatFieldMask::None;
        on.flat_field.mask = FlatFieldMask::ValoiEasy35V1;
        on.flat_field.amount = (self.settings.flat_field.amount + UNEVEN).min(1.0);
        self.states.pictures = vec![photo.cover(&off, w, h)?, photo.cover(&on, w, h)?];
        let switch = (stage.number("switch-x"), stage.number("switch-y"));
        self.clicks = Some(Clicks::at(&CLICKS.map(|t| (t, switch)), 4.4));
        stage.set("on", Value::Bool(false));
        self.states.show(stage, 0)
    }

    fn hand(&self, t: f32) -> Hand {
        self.clicks.as_ref().expect("prepared").hand(t)
    }

    fn pose(&mut self, _t: f32, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        let on = stage.flag("on");
        self.states.show(stage, usize::from(on))
    }
}
