//! One slider's demo: the photograph, whole or a detail zoomed in, over the slider; the hand drags
//! the knob and double-clicks it back to rest, the picture rendered at the value it holds.

use std::collections::BTreeMap;

use oni_app::fields::Field;
use oni_settings::PipelineSettings;

use super::gesture::{Drag, SLIDE};
use super::{Picture, fraction_of, stored_at};
use crate::encode::Tuning;
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo, Rgba};
use crate::stage::Stage;

/// What the picture box shows of the photograph.
#[derive(Clone, Copy)]
pub enum View {
    /// The whole frame, cut about its centre to the box.
    Whole,
    /// A detail centred on `at`, a fraction of the frame, each of its pixels `zoom` screen pixels.
    Detail { at: (f64, f64), zoom: u32 },
}

/// What a slider's demo shows: its scene, the photograph's settings and view, the slider, the hand's stops.
pub struct Spec {
    pub topic: &'static str,
    /// The scene's component, exposing `amount`, `knob-x`, `knob-y` and `travel`.
    pub component: &'static str,
    /// The photograph's own settings changed for the demo, before the slider moves.
    pub settings: fn(PipelineSettings) -> PipelineSettings,
    pub view: View,
    pub field: Field,
    /// Where the hand drags the knob from its rest, a fraction of the track.
    pub to: f32,
    /// Renders between the rest and `to`, so the picture follows the knob step by step; each step
    /// costs a GIF a redrawn photograph, the more pixels it changes the more.
    pub levels: f32,
    pub tuning: Tuning,
}

pub struct OneSlider {
    spec: Spec,
    settings: PipelineSettings,
    size: (u32, u32),
    drag: Option<Drag>,
    renders: BTreeMap<i32, Rgba>,
}

impl OneSlider {
    pub fn new(spec: Spec) -> OneSlider {
        let settings = (spec.settings)(photo::base());
        OneSlider { spec, settings, size: (0, 0), drag: None, renders: BTreeMap::new() }
    }
}

/// The photograph at `s` as `view` shows it in a box of `size` pixels.
pub fn pictured(photo: &mut Photo, s: &PipelineSettings, view: View, size: (u32, u32)) -> Result<Rgba, String> {
    let (w, h) = size;
    match view {
        View::Whole => photo.cover(s, w, h),
        View::Detail { at, zoom } => {
            let detail = photo.detail(s, at, w.div_ceil(zoom), h.div_ceil(zoom))?;
            Ok(if zoom > 1 { photo::magnified(&detail, zoom, w, h) } else { detail })
        }
    }
}

impl Demo for OneSlider {
    fn topic(&self) -> &'static str {
        self.spec.topic
    }

    fn component(&self) -> &'static str {
        self.spec.component
    }

    fn seconds(&self) -> f32 {
        SLIDE
    }

    /// The knob rests where a double click brings it, the slider's own rest on these settings.
    fn prepare(&mut self, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        self.size = Picture::of(stage).pixels();
        let f = self.spec.field;
        let rest = fraction_of(f, &f.knob().set(&self.settings, f.double_click_base(&self.settings)));
        self.drag = Some(Drag::lay(stage, rest, self.spec.to, self.spec.levels, true));
        Ok(())
    }

    fn tuning(&self) -> Tuning {
        self.spec.tuning
    }

    fn hand(&self, t: f32) -> Hand {
        self.drag.as_ref().expect("prepared").hand(t)
    }

    fn pose(&mut self, t: f32, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        let Some((step, fraction)) = self.drag.as_mut().ok_or("not prepared")?.moved(t, stage) else { return Ok(()) };
        if !self.renders.contains_key(&step) {
            let s = stored_at(self.spec.field, &self.settings, fraction);
            self.renders.insert(step, pictured(photo, &s, self.spec.view, self.size)?);
        }
        stage.set_image("photo", &self.renders[&step]);
        Ok(())
    }
}
