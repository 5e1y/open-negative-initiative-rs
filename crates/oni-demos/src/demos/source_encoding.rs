//! Roll › Source Encoding: a linear scan naming no profile, read as Linear, looks right; the hand
//! picks sRGB and it reads washed out, then Linear again. Both readings are the app's own.

use oni_settings::{PipelineSettings, SourceEncoding};

use super::Picture;
use super::gesture::{Clicks, States, labels, tab};
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo};
use crate::stage::Stage;

/// The panel's readings in its tabs' order, as it lays them for a scan read in grey levels: two, so
/// each name has the room it needs in the card.
const READINGS: [SourceEncoding; 2] = [SourceEncoding::Srgb, SourceEncoding::Linear];
/// It starts read right, as linear; the clicks: sRGB, then Linear again.
const START: usize = 1;
const PICKS: [(f32, usize); 2] = [(1.0, 0), (3.0, 1)];

pub struct SourceEncodingDemo {
    settings: PipelineSettings,
    clicks: Option<Clicks>,
    states: States,
}

impl SourceEncodingDemo {
    pub fn new() -> SourceEncodingDemo {
        SourceEncodingDemo { settings: photo::framed(photo::base()), clicks: None, states: States::default() }
    }
}

impl Demo for SourceEncodingDemo {
    fn topic(&self) -> &'static str {
        "source-encoding"
    }

    fn component(&self) -> &'static str {
        "PhotoTabs"
    }


    fn seconds(&self) -> f32 {
        5.0
    }

    fn prepare(&mut self, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        stage.set("labels", labels(&[oni_i18n::tr("sRGB"), oni_i18n::tr("Linear")]));
        stage.set("selected", slint_interpreter::Value::Number(START as f64));
        let (w, h) = Picture::of(stage).pixels();
        // The decode stands for the scanner's linear file, its codes read as each encoding says.
        for encoding in READINGS {
            photo.restage(&self.settings, |l| {
                l.pixels.iter_mut().for_each(|p| p[..3].iter_mut().for_each(|c| *c = c.clamp(0.0, 1.0)));
                oni_colour::source::Source::of(None, Some(encoding), false).linearise(&mut l.pixels, true);
            })?;
            self.states.pictures.push(photo.cover(&self.settings, w, h)?);
        }
        let aims: Vec<(f32, (f32, f32))> = PICKS.iter().map(|&(t, i)| (t, tab(stage, i, READINGS.len()))).collect();
        self.clicks = Some(Clicks::at(&aims, 4.4));
        self.states.show(stage, START)
    }

    fn worded(&self) -> bool {
        true
    }

    fn hand(&self, t: f32) -> Hand {
        self.clicks.as_ref().expect("prepared").hand(t)
    }

    fn pose(&mut self, _t: f32, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        let picked = stage.number("selected").max(0.0) as usize;
        self.states.show(stage, picked)
    }
}
