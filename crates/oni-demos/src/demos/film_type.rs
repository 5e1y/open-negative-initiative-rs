//! Roll › Film Type: the hand picks Positive, then B&W, then Negative again; the scan is read as
//! each, the colour negative turning orange as a slide, grey as black and white.

use oni_settings::{ConversionMode, PipelineSettings};

use crate::encode::Tuning;
use super::Picture;
use super::gesture::{Clicks, States, labels, tab};
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo};
use crate::stage::Stage;

/// The clicks: Positive, B&W, then Negative, as the panel orders the tabs.
const PICKS: [(f32, usize); 3] = [(1.0, 1), (2.6, 2), (4.2, 0)];

pub struct FilmType {
    settings: PipelineSettings,
    clicks: Option<Clicks>,
    states: States,
}

impl FilmType {
    pub fn new() -> FilmType {
        FilmType { settings: photo::framed(photo::base()), clicks: None, states: States::default() }
    }
}

impl Demo for FilmType {
    fn topic(&self) -> &'static str {
        "film-type"
    }

    fn component(&self) -> &'static str {
        "PhotoTabs"
    }


    fn seconds(&self) -> f32 {
        6.0
    }

    fn prepare(&mut self, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        stage.set("labels", labels(&[oni_i18n::tr("Negative"), oni_i18n::tr("Positive"), oni_i18n::tr("B&W")]));
        let (w, h) = Picture::of(stage).pixels();
        // Each type as the panel's tab lays it, the windows carried as a mode change carries them.
        for mode in ConversionMode::ALL {
            let s = oni_core::parameters::carried(&self.settings, mode);
            self.states.pictures.push(photo.cover(&s, w, h)?);
        }
        let n = ConversionMode::ALL.len();
        let aims: Vec<(f32, (f32, f32))> = PICKS.iter().map(|&(t, i)| (t, tab(stage, i, n))).collect();
        self.clicks = Some(Clicks::at(&aims, 5.2));
        self.states.show(stage, 0)
    }

    /// Three whole pictures, each shown twice: fewer colours keep them under the ceiling.
    fn tuning(&self) -> Tuning {
        Tuning { tolerance: 12, colours: 96 }
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
