//! Auto De-dust: on a click the frame's pixels past its roll's window under a veil, the dust among them
//! ringed; on another, one correction on each speck ringed. Neither runs unasked, and the second writes what the first shows.

use super::*;
use oni_measure::dust::{self, Limits, Run};
use oni_settings::PipelineSettings;
use oni_settings::frame::{Correction, Geometry};
use oni_settings::id::Id;

/// The mask's long side, about a 6K window's picture; a zoom past it shows its pixels as blocks.
const MASK_SIDE: usize = 3072;
/// The mask's alpha over the dust and over the rest past the window, which is shown but left alone.
const DUST_ALPHA: u8 = 255;
const PAST_ALPHA: u8 = 110;

/// What a reading was made on: a change to any of it leaves the reading showing what no longer holds.
#[derive(Clone, PartialEq)]
pub(in crate::app) struct Read {
    path: String,
    corrections: Vec<Id>,
    geometry: Geometry,
    limits: Limits,
}

impl Read {
    fn of(path: &str, settings: &PipelineSettings) -> Read {
        Read { path: path.to_string(), corrections: settings.corrections.iter().map(|c| c.id).collect(), geometry: settings.geometry.clone(), limits: Limits::of(settings) }
    }
}

/// A reading on the canvas: its mask, a ring per speck in fractions of the frame, y down, and the correction each one writes.
pub(in crate::app) struct Shown {
    read: Read,
    mask: Image,
    rings: Vec<[f64; 4]>,
    corrections: Vec<Correction>,
}

/// A reading off the interface thread, before its mask is an image.
struct Reading {
    size: (usize, usize),
    rgba: Vec<u8>,
    rings: Vec<[f64; 4]>,
    corrections: Vec<Correction>,
    past: u64,
}

/// The mask, each pixel the most lit its source pixels ask: a speck never fades in the reduction.
fn mask(found: &dust::Found) -> ((usize, usize), Vec<u8>) {
    let scale = (MASK_SIDE as f64 / found.width.max(found.height).max(1) as f64).min(1.0);
    let side = |v: usize| ((v as f64 * scale).ceil() as usize).max(1);
    let (w, h) = (side(found.width), side(found.height));
    let mut rgba = vec![255u8; w * h * 4];
    rgba.chunks_exact_mut(4).for_each(|p| p[3] = 0);
    let mut lay = |run: &Run, alpha: u8| {
        let row = ((f64::from(run.row) * scale) as usize).min(h - 1);
        let from = ((f64::from(run.start) * scale) as usize).min(w - 1);
        let to = ((f64::from(run.end) * scale).ceil() as usize).clamp(from + 1, w);
        for x in from..to {
            let a = &mut rgba[(row * w + x) * 4 + 3];
            *a = (*a).max(alpha);
        }
    };
    found.past.iter().for_each(|r| lay(r, PAST_ALPHA));
    found.dust.iter().flat_map(|s| &s.runs).for_each(|r| lay(r, DUST_ALPHA));
    ((w, h), rgba)
}

fn reading(found: &dust::Found) -> Reading {
    let (size, rgba) = mask(found);
    let (fw, fh) = (found.width as f64, found.height as f64);
    let rings = found.dust.iter().map(|s| [f64::from(s.bounds[0]) / fw, f64::from(s.bounds[1]) / fh, f64::from(s.bounds[2] - s.bounds[0]) / fw, f64::from(s.bounds[3] - s.bounds[1]) / fh]).collect();
    let corrections = found
        .dust
        .iter()
        .map(|s| {
            let (points, radius) = dust::stroke(s, found.width, found.height);
            Correction { id: Id::new(), stroke: points.into_iter().map(|(x, y)| oni_settings::cg::Point { x, y }).collect(), radius }
        })
        .collect();
    Reading { size, rgba, rings, corrections, past: found.past.iter().map(|r| u64::from(r.end - r.start)).sum() }
}

impl Ctl {
    /// The Detect key: a reading shown goes, else one is asked, once the full resolution is there.
    pub(in crate::app) fn toggle_detection(self: &Rc<Self>) {
        if self.state.borrow_mut().dust.found.take().is_some() {
            return self.publish_dedust();
        }
        self.state.borrow_mut().dust.detect_asked = true;
        self.detection_due();
        self.publish_dedust();
    }

    /// The reading asked, on the frame's full resolution, which a patch reads too and the build's landing brings.
    pub(in crate::app) fn detection_due(self: &Rc<Self>) {
        {
            let s = self.state.borrow();
            if !s.dust.detect_asked || s.dust.detecting {
                return;
            }
        }
        let Some(full) = self.ensure_full() else {
            let mut s = self.state.borrow_mut();
            if s.open_path().is_none_or(|p| s.full_failed.contains(p)) {
                s.dust.detect_asked = false;
            }
            return;
        };
        let (path, settings) = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.open_path().map(str::to_string) else { return };
            s.dust.detect_asked = false;
            s.dust.detecting = true;
            (path, s.store.settings().clone())
        };
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            let started = Instant::now();
            let read = Read::of(&path, &settings);
            let result = engine.lock().and_then(|mut e| e.framed_whole(&full, &settings)).map(|(width, pixels)| reading(&dust::read(&pixels, width, &read.limits)));
            let took = started.elapsed();
            with_shell(move |c| c.detection_landed(read, result, took));
        });
    }

    fn detection_landed(self: &Rc<Self>, read: Read, result: Result<Reading, String>, took: Duration) {
        {
            let mut s = self.state.borrow_mut();
            s.dust.detecting = false;
            let current = s.open_path().is_some_and(|p| Read::of(p, s.store.settings()) == read);
            match result {
                Ok(r) => {
                    timing::line(&format!("{}: {} specks of dust, {} pixels past the window, read in {:.0} ms", crate::anonymity::token(&read.path), r.rings.len(), r.past, took.as_secs_f64() * 1000.0));
                    if current {
                        s.dust.found = Some(Shown { mask: image(&r.rgba, r.size.0, r.size.1), read, rings: r.rings, corrections: r.corrections });
                    }
                }
                Err(e) => timing::line(&format!("dust not read: {e}")),
            }
        }
        self.publish_dedust();
    }

    /// The Remove key: the corrections the rings stand for, one undo step, their patches made next as a hand's are.
    pub(in crate::app) fn remove_found(self: &Rc<Self>) {
        let Some(shown) = self.state.borrow_mut().dust.found.take() else { return };
        let holds = {
            let s = self.state.borrow();
            s.open_path().is_some_and(|p| Read::of(p, s.store.settings()) == shown.read)
        };
        if holds && !shown.corrections.is_empty() {
            let mut next = self.state.borrow().store.settings().clone();
            next.corrections.extend(shown.corrections);
            self.write(Command::Write(Box::new(next)));
        }
        self.publish_dedust();
    }

    /// The tab's keys and note, and the veil and rings on the picture laid by `frame`.
    pub(in crate::app) fn publish_found(&self, frame: Option<(f64, f64, f64, f64)>) {
        let Some(w) = self.window() else { return };
        let mut s = self.state.borrow_mut();
        let stale = s.dust.found.as_ref().is_some_and(|f| s.open_path().is_none_or(|p| Read::of(p, s.store.settings()) != f.read));
        if stale {
            s.dust.found = None;
        }
        let d = w.global::<crate::Dedust>();
        d.set_detecting(s.dust.detecting || s.dust.detect_asked);
        let shown = s.dust.found.as_ref();
        d.set_found_shown(shown.is_some());
        let count = shown.map_or(0, |f| f.rings.len());
        d.set_found(count as i32);
        let note: SharedString = if s.dust.detecting || s.dust.detect_asked {
            tr("Reading the photo…").into()
        } else {
            match shown {
                Some(_) if count > 0 => trn("{n} speck of dust ringed", "{n} specks of dust ringed", count as u64).into(),
                Some(_) => tr("No dust past the roll's window").into(),
                None => SharedString::default(),
            }
        };
        d.set_found_note(note);
        let c = w.global::<Canvas>();
        c.set_dust_shown(shown.is_some() && frame.is_some());
        let mask = shown.map_or_else(Image::default, |f| f.mask.clone());
        if c.get_dust_mask() != mask {
            c.set_dust_mask(mask);
        }
        let marks: Vec<Mark> = match (shown, frame) {
            (Some(f), Some(p)) => f.rings.iter().map(|r| Mark { x: (p.0 + r[0] * p.2) as f32, y: (p.1 + r[1] * p.3) as f32, width: (r[2] * p.2) as f32, height: (r[3] * p.3) as f32, number: 0 }).collect(),
            _ => Vec::new(),
        };
        if let Some(marks) = renewed(&c.get_dust_marks(), marks) {
            c.set_dust_marks(marks);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_measure::dust::{Found, Side, Speck};

    fn found(width: usize, height: usize, past: Vec<Run>, dust: Vec<Speck>) -> Found {
        Found { width, height, past, dust }
    }

    #[test]
    fn a_mask_lights_dust_whole_and_the_rest_past_the_window_at_its_share() {
        let run = |row, start, end| Run { row, start, end };
        let speck = Speck { side: Side::Opaque, area: 2, runs: vec![run(1, 2, 4)], bounds: [2, 1, 4, 2] };
        let ((w, h), rgba) = mask(&found(6, 3, vec![run(1, 2, 4), run(2, 0, 1)], vec![speck]));
        assert_eq!((w, h), (6, 3));
        let alpha = |x: usize, y: usize| rgba[(y * w + x) * 4 + 3];
        assert_eq!(alpha(2, 1), DUST_ALPHA);
        assert_eq!(alpha(0, 2), PAST_ALPHA, "past the window, not dust");
        assert_eq!(alpha(0, 0), 0, "twin: inside the window is clear");
        assert!(rgba.chunks_exact(4).all(|p| p[..3] == [255, 255, 255]), "the alpha alone carries it");
    }

    #[test]
    fn a_lone_pixel_of_a_large_frame_stays_lit_in_the_reduced_mask() {
        let run = Run { row: 3999, start: 6047, end: 6048 };
        let ((w, h), rgba) = mask(&found(6048, 4000, vec![run], Vec::new()));
        assert_eq!(w, MASK_SIDE);
        assert_eq!(h, 2032);
        assert_eq!(rgba.chunks_exact(4).filter(|p| p[3] > 0).count(), 1);
        // Twin: lying past the mask's last pixel, rounding would have lost it.
        assert_eq!(rgba[((h - 1) * w + w - 1) * 4 + 3], PAST_ALPHA);
    }

    #[test]
    fn a_reading_writes_one_correction_per_speck_on_the_frames_fractions() {
        let run = Run { row: 50, start: 100, end: 110 };
        let speck = Speck { side: Side::Opaque, area: 10, runs: vec![run], bounds: [100, 50, 110, 51] };
        let r = reading(&found(1000, 500, vec![run], vec![speck]));
        assert_eq!((r.corrections.len(), r.rings.len(), r.past), (1, 1, 10));
        let c = &r.corrections[0];
        assert!(c.stroke.len() >= 2);
        assert!(c.stroke.iter().all(|p| (0.1..0.111).contains(&p.x) && (0.1..0.102).contains(&p.y)), "{:?}", c.stroke);
        assert!((r.rings[0][0] - 0.1).abs() < 1e-12 && (r.rings[0][1] - 0.1).abs() < 1e-12);
        // Twin: what is past the window but no dust writes nothing.
        assert!(reading(&found(1000, 500, vec![run], Vec::new())).corrections.is_empty());
    }

    #[test]
    fn a_reading_stops_holding_once_the_frame_moves_under_it() {
        let mut s = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
        let read = Read::of("a", &s);
        assert!(Read::of("a", &s) == read);
        assert!(Read::of("b", &s) != read, "another frame");
        s.corrections.push(Correction::new(vec![oni_settings::cg::Point::new(0.5, 0.5)]));
        assert!(Read::of("a", &s) != read, "a stroke painted since");
    }
}
