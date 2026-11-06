//! Auto De-dust: on a click the frame's pixels past its window's dense end under a veil, the dust among
//! them ringed; on another, one correction on each speck ringed. Neither runs unasked, and the second writes what the first shows.

use super::*;
use oni_measure::dust;
use oni_measure::dust::Limits;
use oni_settings::PipelineSettings;
use oni_settings::frame::{Correction, Geometry};
use oni_settings::id::Id;

/// The mask's long side, about a 6K window's picture; a zoom past it shows its pixels as blocks.
const MASK_SIDE: usize = 3072;

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

/// A reading on the canvas: the kernel on its map, the texture the kernel writes and Slint samples.
pub(in crate::app) struct Shown {
    read: Read,
    reading: dust::Reading,
    kernel: display::DustKernel,
    target: wgpu::Texture,
    mask: Image,
    /// The specks last ringed, so a notch that changes none of them renews no ring.
    ringed: Vec<*const dust::Speck>,
    rings: Vec<[f64; 4]>,
}

impl Shown {
    /// The map on the GPU, its copy here let go, and the texture Slint samples; one refused shows the veil alone.
    fn new(gpu: &Gpu, read: Read, mut reading: dust::Reading) -> Shown {
        let map = std::mem::take(&mut reading.map);
        let kernel = display::DustKernel::new(&gpu.device, &gpu.queue, (map.width, map.height), &map.texels);
        let target = display::Gpu::target(&gpu.device, map.width, map.height);
        let mask = if gpu.reads_back {
            Image::default()
        } else {
            Image::try_from(target.clone()).unwrap_or_else(|e| {
                timing::line(&format!("the dust mask has no texture: {e}"));
                Image::default()
            })
        };
        Shown { read, reading, kernel, target, mask, ringed: Vec::new(), rings: Vec::new() }
    }

    /// The mask at `sensitivity`: one dispatch, nothing read or uploaded; a window that cannot take the GPU's texture reads it back.
    fn draw(&mut self, gpu: &Gpu, sensitivity: f32, lit: [f32; 3]) {
        self.kernel.run(&gpu.device, &gpu.queue, dust::notch(sensitivity), lit, &self.target);
        if gpu.reads_back {
            let (w, h) = (self.target.width(), self.target.height());
            if let Some(bytes) = display::read_back(&gpu.device, &gpu.queue, &self.target, w, h) {
                self.mask = image(&bytes, w as usize, h as usize);
            }
        }
    }

    /// The rings at `sensitivity`; true when they changed.
    fn ring(&mut self, sensitivity: f32) -> bool {
        let dust = self.reading.at(sensitivity);
        let ringed: Vec<*const dust::Speck> = dust.iter().map(|s| *s as *const dust::Speck).collect();
        if ringed == self.ringed {
            return false;
        }
        self.rings = rings(&dust, self.reading.width, self.reading.height);
        self.ringed = ringed;
        true
    }

    /// The corrections the rings stand for.
    fn corrections(&self, sensitivity: f32) -> Vec<Correction> {
        corrections(&self.reading.at(sensitivity), self.reading.width, self.reading.height)
    }
}

fn rings(dust: &[&dust::Speck], width: usize, height: usize) -> Vec<[f64; 4]> {
    let (fw, fh) = (width as f64, height as f64);
    dust.iter().map(|s| [f64::from(s.bounds[0]) / fw, f64::from(s.bounds[1]) / fh, f64::from(s.bounds[2] - s.bounds[0]) / fw, f64::from(s.bounds[3] - s.bounds[1]) / fh]).collect()
}

fn corrections(dust: &[&dust::Speck], width: usize, height: usize) -> Vec<Correction> {
    dust.iter()
        .map(|s| {
            let (points, radius) = dust::stroke(s, width, height);
            Correction { id: Id::new(), stroke: points.into_iter().map(|(x, y)| oni_settings::cg::Point { x, y }).collect(), radius }
        })
        .collect()
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

    /// The reading asked, once the frame's full resolution is there, which an opening or a build may bring: looked for at
    /// the pulse's pace, since an opening that brings it announces nothing. A frame left or that cannot be read drops the ask.
    pub(in crate::app) fn detection_due(self: &Rc<Self>) {
        {
            let s = self.state.borrow();
            if !s.dust.detect_asked || s.dust.detecting {
                return;
            }
        }
        let failed = self.window().is_none_or(|w| !w.global::<Canvas>().get_failure().is_empty());
        let full = if failed || !self.painting() { None } else { self.ensure_full() };
        let Some(full) = full else {
            let gone = {
                let s = self.state.borrow();
                failed || !self.painting() || s.open_path().is_none_or(|p| s.full_failed.contains(p))
            };
            if gone {
                self.state.borrow_mut().dust.detect_asked = false;
                self.publish_dedust();
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
            let result = engine.lock().and_then(|mut e| e.framed_whole(&full, &settings)).map(|(width, pixels)| dust::read(&pixels, width, &read.limits, MASK_SIDE));
            let took = started.elapsed();
            with_shell(move |c| c.detection_landed(read, result, took));
        });
    }

    fn detection_landed(self: &Rc<Self>, read: Read, result: Result<dust::Reading, String>, took: Duration) {
        {
            let mut s = self.state.borrow_mut();
            s.dust.detecting = false;
            let current = s.open_path().is_some_and(|p| Read::of(p, s.store.settings()) == read);
            match result {
                Ok(reading) => {
                    let count = reading.at(s.dust.sensitivity).len();
                    timing::line(&format!("{}: {count} specks of dust, {} pixels past the window, read in {:.0} ms", crate::anonymity::token(&read.path), reading.past, took.as_secs_f64() * 1000.0));
                    if current {
                        let shown = Shown::new(&s.gpu, read, reading);
                        s.dust.found = Some(shown);
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
        let (holds, sensitivity) = {
            let s = self.state.borrow();
            (s.open_path().is_some_and(|p| Read::of(p, s.store.settings()) == shown.read), s.dust.sensitivity)
        };
        let written = shown.corrections(sensitivity);
        if holds && !written.is_empty() {
            let mut next = self.state.borrow().store.settings().clone();
            next.corrections.extend(written);
            self.write(Command::Write(Box::new(next)));
        }
        self.publish_dedust();
    }

    /// The Sensitivity slider, held as the brush's is: a double press rests it, shown at once.
    pub(in crate::app) fn sensitivity_press(&self) -> bool {
        let double = {
            let mut s = self.state.borrow_mut();
            let double = s.dust.sensitivity_pressed_at.is_some_and(|t| t.elapsed() < system::double_click());
            s.dust.sensitivity_pressed_at = Some(Instant::now());
            s.dust.sensitivity_start = if double { None } else { Some(s.dust.sensitivity) };
            if double {
                s.dust.sensitivity = 0.0;
            }
            double
        };
        if double {
            self.keep_sensitivity();
            self.sensitivity_moved();
        }
        double
    }

    pub(in crate::app) fn sensitivity_drag(&self, translation: f32, span: f32, fine: bool) {
        {
            let mut s = self.state.borrow_mut();
            let Some(start) = s.dust.sensitivity_start else { return };
            s.dust.sensitivity = track::clamped(track::asked(start, translation, span, fine));
        }
        self.sensitivity_moved();
    }

    pub(in crate::app) fn sensitivity_release(&self) {
        if self.state.borrow_mut().dust.sensitivity_start.take().is_some() {
            self.keep_sensitivity();
        }
    }

    pub(in crate::app) fn sensitivity_scroll(self: &Rc<Self>, dx: f32, dy: f32, span: f32, fine: bool) -> bool {
        let from = self.state.borrow().dust.sensitivity;
        let Some(at) = self.slider_scrolled(from, dx, dy, span, fine, |c| c.keep_sensitivity()) else { return false };
        self.state.borrow_mut().dust.sensitivity = at;
        self.sensitivity_moved();
        true
    }

    /// The sensitivity the hand left, for every photo and launch, as the brush's size is.
    fn keep_sensitivity(&self) {
        let at = f64::from(self.state.borrow().dust.sensitivity);
        self.remember(preferences::DUST_SENSITIVITY, at.into());
    }

    /// A notch: the kernel's one dispatch, and the rings only where a speck came or went.
    fn sensitivity_moved(&self) {
        let frame = self.image_frame(false);
        self.lay_found(frame, false);
    }

    /// The tab's keys and note, and the veil and rings on the picture laid by `frame`.
    pub(in crate::app) fn publish_found(&self, frame: Option<(f64, f64, f64, f64)>) {
        {
            let mut s = self.state.borrow_mut();
            let stale = s.dust.found.as_ref().is_some_and(|f| s.open_path().is_none_or(|p| Read::of(p, s.store.settings()) != f.read));
            if stale {
                s.dust.found = None;
            }
        }
        self.lay_found(frame, true);
    }

    fn lay_found(&self, frame: Option<(f64, f64, f64, f64)>, whole: bool) {
        let Some(w) = self.window() else { return };
        let (d, c) = (w.global::<crate::Dedust>(), w.global::<Canvas>());
        let mut s = self.state.borrow_mut();
        let s = &mut *s;
        let sensitivity = s.dust.sensitivity;
        d.set_sensitivity(sensitivity);
        d.set_sensitivity_text(crate::fields::Readout::Percent.format(sensitivity).into());
        let lit = c.get_dust_lit();
        let lit = [lit.red(), lit.green(), lit.blue()].map(|v| f32::from(v) / 255.0);
        let mut ringed = whole;
        if let Some(f) = s.dust.found.as_mut() {
            f.draw(&s.gpu, sensitivity, lit);
            if c.get_dust_mask() != f.mask {
                c.set_dust_mask(f.mask.clone());
            }
            c.set_dust_revision(c.get_dust_revision().wrapping_add(1));
            ringed |= f.ring(sensitivity);
        }
        c.set_dust_shown(s.dust.found.is_some() && frame.is_some());
        if !ringed {
            return;
        }
        let working = s.dust.detecting || s.dust.detect_asked;
        d.set_detecting(working);
        let shown = s.dust.found.as_ref();
        d.set_found_shown(shown.is_some());
        let count = shown.map_or(0, |f| f.rings.len());
        d.set_found(count as i32);
        let note: SharedString = if working {
            tr("Reading the photo…").into()
        } else {
            match shown {
                Some(_) if count > 0 => trn("{n} speck of dust ringed", "{n} specks of dust ringed", count as u64).into(),
                Some(_) => tr("No dust at this sensitivity").into(),
                None => SharedString::default(),
            }
        };
        d.set_found_note(note);
        if shown.is_none() && c.get_dust_mask().size().width != 0 {
            c.set_dust_mask(Image::default());
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
    use oni_measure::dust::{Run, Speck};

    #[test]
    fn a_reading_writes_one_correction_per_speck_on_the_frames_fractions() {
        let run = Run { row: 50, start: 100, end: 110 };
        let speck = Speck { area: 10, runs: vec![run], bounds: [100, 50, 110, 51] };
        let written = corrections(&[&speck], 1000, 500);
        assert_eq!((written.len(), rings(&[&speck], 1000, 500).len()), (1, 1));
        let c = &written[0];
        assert!(c.stroke.len() >= 2);
        assert!(c.stroke.iter().all(|p| (0.1..0.111).contains(&p.x) && (0.1..0.102).contains(&p.y)), "{:?}", c.stroke);
        let ring = rings(&[&speck], 1000, 500)[0];
        assert!((ring[0] - 0.1).abs() < 1e-12 && (ring[1] - 0.1).abs() < 1e-12);
        // Twin: no dust writes nothing.
        assert!(corrections(&[], 1000, 500).is_empty());
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
