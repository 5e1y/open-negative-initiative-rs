//! Roll › Measured Roll: this photo over three framings of it, its roll's others; the hand drags
//! Source Balance's Blue ↔ Yellow and every photo turns with it, then back.

use oni_app::fields::Field;
use oni_settings::PipelineSettings;
use oni_settings::cg::Rect;

use super::gesture::Drag;
use super::{Picture, fraction_of, stored_at};
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo, Rgba};
use crate::stage::{SCALE, Stage};

/// The roll's other photos, crops of this one in `first`, `second`, `third`, as fractions, y up as the
/// settings keep it: the strollers' legs, the cap, the red sail's edge on the sand.
const OTHERS: [(&str, Rect); 3] = [
    ("first", Rect::new(0.25, 0.7, 0.3, 0.3)),
    ("second", Rect::new(0.4, 0.38, 0.3, 0.3)),
    ("third", Rect::new(0.12, 0.45, 0.3, 0.3)),
];
const FIELD: Field = Field::GainBlue;
/// How far the knob is dragged, a share of its track, and the renders on the way.
const SHIFT: f32 = 0.2;
const LEVELS: f32 = 6.0;

pub struct MeasuredRoll {
    settings: PipelineSettings,
    drag: Option<Drag>,
    /// Each step's pictures: this photo's, then the others' in their order.
    renders: Vec<Vec<Rgba>>,
}

impl MeasuredRoll {
    pub fn new() -> MeasuredRoll {
        MeasuredRoll { settings: photo::framed(photo::base()), drag: None, renders: Vec::new() }
    }
}

/// Every step's picture of one framing, its knob moved by the same share of the track as this photo's.
fn rendered(photo: &mut Photo, s: &PipelineSettings, drag: &Drag, size: (u32, u32)) -> Result<Vec<Rgba>, String> {
    let rest = fraction_of(FIELD, s);
    drag.steps().map(|step| photo.cover(&stored_at(FIELD, s, rest + drag.at(step) - drag.rest), size.0, size.1)).collect()
}

impl Demo for MeasuredRoll {
    fn topic(&self) -> &'static str {
        "measured-roll"
    }

    fn component(&self) -> &'static str {
        "MeasuredRoll"
    }


    fn seconds(&self) -> f32 {
        super::gesture::SLIDE
    }

    fn prepare(&mut self, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        let rest = fraction_of(FIELD, &self.settings);
        let drag = Drag::lay(stage, rest, rest + SHIFT, LEVELS, false);
        let thumb = ((stage.number("thumb-width") * SCALE).round() as u32, (stage.number("thumb-height") * SCALE).round() as u32);
        let mut columns = vec![rendered(photo, &self.settings, &drag, Picture::of(stage).pixels())?];
        for (_, crop) in OTHERS {
            let mut framed = self.settings.clone();
            framed.geometry.crop = crop;
            columns.push(rendered(photo, &framed, &drag, thumb)?);
        }
        self.renders = drag.steps().map(|step| columns.iter().map(|c| c[step as usize].clone()).collect()).collect();
        self.drag = Some(drag);
        Ok(())
    }

    fn hand(&self, t: f32) -> Hand {
        self.drag.as_ref().expect("prepared").hand(t)
    }

    fn pose(&mut self, t: f32, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        let Some((step, _)) = self.drag.as_mut().ok_or("not prepared")?.moved(t, stage) else { return Ok(()) };
        let pictures = self.renders.get(step as usize).ok_or("a step past the mark")?;
        stage.set_image("photo", &pictures[0]);
        for ((name, _), picture) in OTHERS.iter().zip(&pictures[1..]) {
            stage.set_image(name, picture);
        }
        Ok(())
    }
}
