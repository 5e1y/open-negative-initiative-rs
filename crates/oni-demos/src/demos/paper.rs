//! Color › Paper Correction: the frame proofed on a paper, the hand presses the automatic key, the
//! three gains spring to what the app reads off the paper's profile and the grey prints grey.

use oni_app::fields::Field;
use oni_colour::{OutputProfile, Space};
use oni_settings::PipelineSettings;
use slint_interpreter::Value;

use super::{Picture, fraction_of};
use crate::encode::Tuning;
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo, Rgba};
use crate::stage::Stage;
use crate::track::{Presses, Track};

/// The paper it is proofed on, among the user's installed profiles: the one whose correction is the
/// strongest, a fifth of a stop of blue, so the gains are seen to move.
const PAPER: &str = "Library/ColorSync/Profiles/ILFORD_CANpro-300_IGPSP_PPPS.icc";
/// When the gains go back to zero, so the loop starts again where it began.
const RESET: f32 = 5.0;
const GAINS: [Field; 3] = [Field::PaperRed, Field::PaperGreen, Field::PaperBlue];
const ROWS: [&str; 3] = ["red", "green", "blue"];

pub struct PaperCorrection {
    plain: PipelineSettings,
    corrected: PipelineSettings,
    pictures: Option<(Rgba, Rgba)>,
    path: Track<(f32, f32)>,
    presses: Presses,
    shown: Option<bool>,
}

impl PaperCorrection {
    pub fn new() -> PaperCorrection {
        let mut plain = photo::base();
        // The cream sail over the sand, the photograph's palest tones, where a paper's white shows.
        plain.geometry.crop = oni_settings::cg::Rect::new(0.3, 0.37, 0.45, 0.28);
        PaperCorrection { corrected: plain.clone(), plain, pictures: None, path: Track::default(), presses: Presses::default(), shown: None }
    }

    fn show(&mut self, stage: &Stage, corrected: bool) -> Result<(), String> {
        let (plain, done) = self.pictures.as_ref().ok_or("not prepared")?;
        let (picture, settings) = if corrected { (done, &self.corrected) } else { (plain, &self.plain) };
        stage.set_image("photo", picture);
        for (row, field) in ROWS.iter().zip(GAINS) {
            stage.set(row, Value::Number(f64::from(fraction_of(field, settings))));
        }
        self.shown = Some(corrected);
        Ok(())
    }
}

impl Demo for PaperCorrection {
    fn topic(&self) -> &'static str {
        "paper-correction"
    }

    fn component(&self) -> &'static str {
        "PaperCorrection"
    }


    fn seconds(&self) -> f32 {
        5.5
    }

    fn prepare(&mut self, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
        let file = home.join(PAPER);
        if !file.is_file() {
            return Err(format!("{} is missing: the paper this demo is proofed on", file.display()));
        }
        let paper = OutputProfile::new(file.to_string_lossy().into_owned());
        let found = oni_colour::paper::automatic(&paper).ok_or("Auto Paper Correction read nothing off the paper")?;
        self.corrected.paper_correction = found;
        photo.proof(Space::Profile(paper));
        let (w, h) = Picture::of(stage).pixels();
        self.pictures = Some((photo.cover(&self.plain, w, h)?, photo.cover(&self.corrected, w, h)?));
        let key = (stage.number("key-x"), stage.number("key-y"));
        let rest = (key.0 + 46.0, key.1 - 58.0);
        let aside = (key.0 + 30.0, key.1 + 14.0);
        self.show(stage, false)?;
        self.path = Track::from(rest).hold(0.5).to(1.1, key).hold(1.6).to(2.3, aside).hold(3.8).to(4.5, rest);
        self.presses = Presses(vec![(1.25, 1.35)]);
        Ok(())
    }

    /// A fifth of a stop of blue on a paper's white: a pixel is drawn again past a small stray.
    fn tuning(&self) -> Tuning {
        Tuning { tolerance: 3, colours: 128 }
    }

    fn hand(&self, t: f32) -> Hand {
        Hand { at: self.path.at(t), down: self.presses.down(t) }
    }

    fn pose(&mut self, t: f32, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        if t >= RESET && stage.flag("corrected") {
            stage.set("corrected", Value::Bool(false));
        }
        let corrected = stage.flag("corrected");
        if self.shown == Some(corrected) {
            return Ok(());
        }
        self.show(stage, corrected)
    }
}
