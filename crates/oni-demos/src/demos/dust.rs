//! Dust & Scratches: the hand paints over a speck of dust, the app's own heal searches and makes its
//! patch, and the photograph shows it healed, with the stroke's mark and where the patch was copied.

use std::fmt::Write;

use oni_settings::PipelineSettings;
use oni_settings::cg::Point;
use oni_settings::frame::Correction;
use oni_settings::id::Id;
use slint_interpreter::Value;

use super::Picture;
use crate::encode::Tuning;
use crate::film::{Demo, Hand};
use crate::photo::{self, Photo, Rgba};
use crate::stage::{SCALE, Stage};
use crate::track::{Presses, Track};

/// A hair curled on the red sail, a fraction of the framed frame, and the detail shown about it.
const SPECK: (f64, f64) = (0.207, 0.482);
const ZOOM: u32 = 2;
/// The brush's radius in the frame's pixels, its slider's range `Correction::RADIUS_RANGE`.
const RADIUS: f64 = 8.0;
/// The stroke along the hair, in points from the detail's centre, as a hand paints over it.
const SCRUB: [(f32, f32); 7] = [(-57.0, 39.0), (-45.0, 10.0), (-25.0, -20.0), (-5.0, -37.0), (11.0, -36.0), (25.0, -36.0), (56.0, -39.0)];
/// When the hand presses and lets go, the patch lands, and the picture is dusty again for the loop.
const PRESS: (f32, f32) = (1.1, 2.5);
const LANDS: f32 = 2.9;
const RESET: f32 = 5.25;
/// The correction's id, fixed so the tool writes the same patch file on every run.
const ID: &str = "D0510000-0000-4000-8000-000000000001";

pub struct Dust {
    settings: PipelineSettings,
    pictures: Option<(Rgba, Rgba)>,
    path: Track<(f32, f32)>,
    presses: Presses,
    /// The stroke in the photograph's points, each with the second the hand laid it.
    stroke: Vec<(f32, (f32, f32))>,
    /// The copied place's offset from the stroke, in the photograph's points.
    offset: (f32, f32),
    /// What the scene shows, as last posed: 0 dusty, 1 painting, 2 healed.
    shown: Option<(u8, usize)>,
}

impl Dust {
    pub fn new() -> Dust {
        let settings = photo::base();
        Dust { settings, pictures: None, path: Track::default(), presses: Presses::default(), stroke: Vec::new(), offset: (0.0, 0.0), shown: None }
    }

    fn polyline(points: impl Iterator<Item = (f32, f32)>) -> String {
        let mut path = String::new();
        for (i, (x, y)) in points.enumerate() {
            let _ = write!(path, "{}{x:.2} {y:.2}", if i == 0 { "M " } else { " L " });
        }
        path
    }

    /// The stroke's box padded by the brush, in the photograph's points.
    fn bounds(&self, radius: f32) -> (f32, f32, f32, f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (_, (x, y)) in &self.stroke {
            (x0, y0, x1, y1) = (x0.min(*x), y0.min(*y), x1.max(*x), y1.max(*y));
        }
        (x0 - radius, y0 - radius, x1 - x0 + 2.0 * radius, y1 - y0 + 2.0 * radius)
    }

    fn show(&mut self, stage: &Stage, state: u8, laid: usize) -> Result<(), String> {
        let (dusty, healed) = self.pictures.as_ref().ok_or("not prepared")?;
        stage.set_image("photo", if state == 2 { healed } else { dusty });
        let painted = Self::polyline(self.stroke.iter().take(laid).map(|(_, p)| *p));
        stage.set("stroke", Value::String(if state == 1 && laid > 1 { painted.into() } else { "".into() }));
        let healed = state == 2;
        let radius = (RADIUS * f64::from(ZOOM) / f64::from(SCALE)) as f32;
        let (x, y, w, h) = self.bounds(radius);
        for (name, v) in [("mark-x", x), ("mark-y", y), ("mark-width", w), ("mark-height", h)] {
            stage.set(name, Value::Number(f64::from(v)));
        }
        stage.set("marked", Value::Bool(healed));
        // Where the patch was copied from, as the canvas shows it, when the detail holds it whole: a
        // line running off the picture would point at nothing seen.
        let (dx, dy) = self.offset;
        let size = (stage.number("photo-width"), stage.number("photo-height"));
        let seen = healed && x + dx >= 0.0 && y + dy >= 0.0 && x + dx + w <= size.0 && y + dy + h <= size.1;
        let source = Self::polyline(self.stroke.iter().map(|(_, (x, y))| (x + dx, y + dy)));
        stage.set("source", Value::String(if seen { source.into() } else { "".into() }));
        let centre = (x + w / 2.0, y + h / 2.0);
        stage.set("source-line", Value::String(if seen { dashed(centre, (centre.0 + dx, centre.1 + dy)).into() } else { "".into() }));
        self.shown = Some((state, laid));
        Ok(())
    }
}

/// `DSGuide`'s dashes, four points on and four off, from `a` toward `b` and never past it.
fn dashed(a: (f32, f32), b: (f32, f32)) -> String {
    let length = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let at = |d: f32| (a.0 + (b.0 - a.0) * d / length, a.1 + (b.1 - a.1) * d / length);
    let mut path = String::new();
    let mut d = 0.0;
    while d < length {
        let (from, to) = (at(d), at((d + 4.0).min(length)));
        let _ = write!(path, "{}M {:.2} {:.2} L {:.2} {:.2}", if d > 0.0 { " " } else { "" }, from.0, from.1, to.0, to.1);
        d += 8.0;
    }
    path
}

impl Demo for Dust {
    fn topic(&self) -> &'static str {
        "dust"
    }

    fn component(&self) -> &'static str {
        "Dust"
    }


    fn seconds(&self) -> f32 {
        5.5
    }

    fn prepare(&mut self, stage: &Stage, photo: &mut Photo) -> Result<(), String> {
        let (w, h) = Picture::of(stage).pixels();
        let (rw, rh) = (w.div_ceil(ZOOM), h.div_ceil(ZOOM));
        let (fw, fh) = photo.framed_size(&self.settings)?;
        // Where `Photo::detail` lays the region about the speck, in the frame's pixels.
        let x0 = ((f64::from(fw) * SPECK.0) as i64 - i64::from(rw / 2)).clamp(0, i64::from(fw - rw)) as f64;
        let y0 = ((f64::from(fh) * SPECK.1) as i64 - i64::from(rh / 2)).clamp(0, i64::from(fh - rh)) as f64;
        let points = f64::from(ZOOM) / f64::from(SCALE);
        let speck = (((SPECK.0 * f64::from(fw) - x0) * points) as f32, ((SPECK.1 * f64::from(fh) - y0) * points) as f32);
        let (px, py) = (stage.number("photo-x"), stage.number("photo-y"));
        let on = |(dx, dy): (f32, f32)| (px + speck.0 + dx, py + speck.1 + dy);
        let rest = (px + stage.number("photo-width") - 36.0, py + stage.number("photo-height") - 36.0);
        let mut path = Track::from(rest).hold(0.4).to(1.0, on(SCRUB[0])).hold(PRESS.0 + 0.05);
        let span = (PRESS.1 - PRESS.0 - 0.15) / (SCRUB.len() - 1) as f32;
        for (i, p) in SCRUB.iter().enumerate().skip(1) {
            path = path.to(PRESS.0 + 0.05 + i as f32 * span, on(*p));
        }
        self.path = path.hold(LANDS - 0.1).to(LANDS + 0.5, rest);
        self.presses = Presses(vec![PRESS]);
        // The stroke where the hand passes while down, a point every hundredth of a second.
        self.stroke = (0..)
            .map(|k| PRESS.0 + k as f32 * 0.01)
            .take_while(|t| *t < PRESS.1)
            .map(|t| (t, self.path.at(t)))
            .map(|(t, (x, y))| (t, (x - px, y - py)))
            .collect();
        self.stroke.dedup_by(|a, b| a.1 == b.1);
        let fraction = |(x, y): (f32, f32)| Point { x: (x0 + f64::from(x) / points) / f64::from(fw), y: (y0 + f64::from(y) / points) / f64::from(fh) };
        let id = Id::parse(ID).ok_or("the correction's id")?;
        let correction = Correction { id, stroke: self.stroke.iter().map(|(_, p)| fraction(*p)).collect(), radius: RADIUS };
        let mut healed = self.settings.clone();
        healed.corrections.push(correction.clone());
        let caches = std::env::temp_dir().join(format!("oni-demos-dust-{}", std::process::id()));
        let offset = photo.heal(&healed, &correction, &caches);
        let pictures = offset.as_ref().map_err(Clone::clone).and_then(|_| {
            let picture = |s: &PipelineSettings, photo: &mut Photo| photo.detail(s, SPECK, rw, rh).map(|d| photo::magnified(&d, ZOOM, w, h));
            Ok((picture(&self.settings, photo)?, picture(&healed, photo)?))
        });
        let _ = std::fs::remove_dir_all(&caches);
        let offset = offset?;
        self.offset = ((offset.0 * f64::from(fw) * points) as f32, (offset.1 * f64::from(fh) * points) as f32);
        self.pictures = Some(pictures?);
        stage.set("brush-diameter", Value::Number(2.0 * RADIUS * points));
        stage.set("stroke-width", Value::Number(2.0 * RADIUS * points));
        self.show(stage, 0, 0)
    }

    /// Two pictures and the overlay's thin lines: the default packing holds them.
    fn tuning(&self) -> Tuning {
        Tuning { tolerance: 8, colours: 128 }
    }

    fn hand(&self, t: f32) -> Hand {
        Hand { at: self.path.at(t), down: self.presses.down(t) }
    }

    fn pose(&mut self, t: f32, stage: &Stage, _photo: &mut Photo) -> Result<(), String> {
        let state = if t >= RESET || t < PRESS.0 {
            0
        } else if t < LANDS {
            1
        } else {
            2
        };
        let laid = if state == 1 { self.stroke.iter().filter(|(s, _)| *s <= t).count() } else { 0 };
        if self.shown == Some((state, laid)) {
            return Ok(());
        }
        self.show(stage, state, laid)
    }
}
