//! A demo filmed: its scene laid on the stage, posed frame by frame at 20 a second, the hand drawn
//! over each shot, the clip encoded. A demo is a `Demo`: what it shows and when.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cursor;
use crate::encode::{self, Clip, Region, Tuning};
use crate::photo::{self, Photo, Rgba};
use crate::stage::{SCALE, Stage};

/// `tutorial.rs`'s pace: the app plays a clip frame by frame at its own delays.
pub const FPS: u32 = 20;
/// The help card's picture, in points: its width less its padding, square.
pub const SIDE: f32 = 244.0;

/// Where the hand is in a frame, in the scene's points, and whether it is down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hand {
    pub at: (f32, f32),
    pub down: bool,
}

/// One demo: a scene file in `scenes/` over the demos' photograph, and a timeline.
pub trait Demo {
    /// The help topic, which names the scene file and the GIF.
    fn topic(&self) -> &'static str;
    /// The scene file's exported window.
    fn component(&self) -> &'static str;
    fn seconds(&self) -> f32;
    /// Once, the stage laid: renders what never changes and plans the hand's path on the scene.
    fn prepare(&mut self, stage: &Stage, photo: &mut Photo) -> Result<(), String>;
    /// The hand at `t` seconds.
    fn hand(&self, t: f32) -> Hand;
    /// After the hand's events at `t`: what the scene shows, read from the controls it moved.
    fn pose(&mut self, t: f32, stage: &Stage, photo: &mut Photo) -> Result<(), String>;
    /// Whether it shows words, the tabs' names, and so is filmed once per language.
    fn worded(&self) -> bool {
        false
    }
    /// How its GIF is packed under the weight ceiling.
    fn tuning(&self) -> Tuning {
        Tuning { tolerance: 10, colours: 128 }
    }
}

pub fn scenes() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scenes")
}

/// Every frame of `demo`, the hand drawn on each, and where its photograph lies.
pub fn shoot(demo: &mut dyn Demo) -> Result<(Vec<Rgba>, Region), String> {
    let file = scenes().join(format!("{}.slint", demo.topic()));
    let mut stage = Stage::new(&file, demo.component(), (SIDE, SIDE))?;
    let mut photo = Photo::open(&photo::source())?;
    demo.prepare(&stage, &mut photo)?;
    stage.settle();
    let px = |name: &str| (stage.number(name) * SCALE).round() as u32;
    let region = Region { x: px("photo-x"), y: px("photo-y"), width: px("photo-width"), height: px("photo-height") };
    let count = (demo.seconds() * FPS as f32).round() as u32;
    let mut frames = Vec::with_capacity(count as usize);
    for i in 0..count {
        let t = i as f32 / FPS as f32;
        let hand = demo.hand(t);
        stage.pointer(hand.at, hand.down);
        demo.pose(t, &stage, &mut photo)?;
        let mut shot = stage.shoot(Duration::from_millis(u64::from(i) * 1000 / u64::from(FPS)))?;
        cursor::draw(&mut shot, hand.at, hand.down, SCALE);
        frames.push(shot);
    }
    Ok((frames, region))
}

/// The demo's GIF and the frames it was made of.
pub fn film(demo: &mut dyn Demo) -> Result<(Vec<u8>, Vec<Rgba>), String> {
    let (frames, photo) = shoot(demo)?;
    let clip = Clip { frames, delay: (100 / FPS) as u16, photo };
    let gif = encode::gif(&clip, demo.tuning())?;
    Ok((gif, clip.frames))
}
