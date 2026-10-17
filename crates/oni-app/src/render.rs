//! Render jobs off the interface thread. A slider writes the latest settings into one slot: one job
//! renders while one waits, and a newer job replaces the waiting one, so a drag never queues.

use crate::display::{self, Params, Screen};
use oni_settings::PipelineSettings;
use oni_ui_poc::frame::{self, Linear};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

/// The conversion a canvas copy goes through: settings in, linear Rec. 2020 out.
pub trait Renderer: Send {
    fn render(&mut self, negative: &Linear, settings: &PipelineSettings) -> Result<Linear, String>;
}

/// Stages 4 to 7 of the Pro face on the CPU, as the proof of concept runs them, until the whole
/// `Pipeline.apply` lands behind this trait.
pub struct Stage47;

impl Renderer for Stage47 {
    fn render(&mut self, negative: &Linear, settings: &PipelineSettings) -> Result<Linear, String> {
        let text = oni_settings::codec::encode_settings(settings).map_err(|e| e.to_string())?;
        let document: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let set = frame::as_set(&document).map_err(|e| format!("{e:?}"))?;
        Ok(frame::converted(negative, set.capture_stops, &set.stage47))
    }
}

/// A frame decoded once and reduced to the canvas copy, shared by the editor and the gallery.
pub fn decode(path: &Path, side: usize) -> Result<Linear, String> {
    let decoded = oni_decode::decode(path, 1.0).map_err(|e| format!("{}: {e:?}", path.display()))?;
    Ok(Linear { width: decoded.width, height: decoded.height, pixels: decoded.pixels }.reduced(side))
}

pub struct Job {
    pub revision: u64,
    pub path: PathBuf,
    pub negative: Arc<Linear>,
    pub settings: PipelineSettings,
}

/// A finished render: the linear image for the GPU kernel and, encoded on this thread, the bytes
/// the software renderer shows, with the histogram of what is shown.
pub struct Done {
    pub revision: u64,
    pub path: PathBuf,
    pub linear: Linear,
    pub rgba8: Vec<u8>,
    pub histogram: Histogram,
}

#[derive(Default)]
struct Slot {
    waiting: Option<Job>,
    closed: bool,
}

pub struct Worker {
    slot: Arc<(Mutex<Slot>, Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    pub fn new(mut renderer: Box<dyn Renderer>, screen: Screen, deliver: impl Fn(Result<Done, String>) + Send + 'static) -> Worker {
        let slot: Arc<(Mutex<Slot>, Condvar)> = Arc::default();
        let shared = slot.clone();
        let thread = std::thread::Builder::new()
            .name("render".into())
            .spawn(move || {
                let mut seed = 0u32;
                loop {
                    let job = {
                        let (lock, wake) = &*shared;
                        let mut s = lock.lock().expect("render slot");
                        while s.waiting.is_none() && !s.closed {
                            s = wake.wait(s).expect("render slot");
                        }
                        if s.closed {
                            return;
                        }
                        s.waiting.take().expect("a job")
                    };
                    seed = seed.wrapping_add(1);
                    let done = renderer.render(&job.negative, &job.settings).map(|linear| {
                        let k = Params::new(screen, linear.width as u32, linear.height as u32, seed);
                        let rgba8 = display::encode(&linear.pixels, &k);
                        let histogram = Histogram::of(&rgba8);
                        Done { revision: job.revision, path: job.path, linear, rgba8, histogram }
                    });
                    deliver(done);
                }
            })
            .expect("render thread");
        Worker { slot, thread: Some(thread) }
    }

    /// Lays `job` in the slot, replacing a job still waiting there.
    pub fn submit(&self, job: Job) {
        let (lock, wake) = &*self.slot;
        lock.lock().expect("render slot").waiting = Some(job);
        wake.notify_one();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.slot;
        lock.lock().expect("render slot").closed = true;
        wake.notify_one();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The shown image's three channels in 256 bins each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Histogram {
    pub bins: Vec<[u32; 3]>,
}

impl Histogram {
    pub fn of(rgba8: &[u8]) -> Histogram {
        let mut bins = vec![[0u32; 3]; 256];
        for p in rgba8.chunks_exact(4) {
            for c in 0..3 {
                bins[p[c] as usize][c] += 1;
            }
        }
        Histogram { bins }
    }

    /// `DSHistogram`: each channel's bins as a filled curve, red, green and blue added where they
    /// overlap, the tallest bin at the top; premultiplied RGBA8 of `w` by `h`.
    pub fn raster(&self, w: usize, h: usize) -> Vec<u8> {
        let mut out = vec![0u8; w * h * 4];
        if self.bins.is_empty() || w == 0 || h == 0 {
            return out;
        }
        // The two ends hold every clipped pixel; scaling by them would flatten the rest.
        let top = self.bins[1..255].iter().flat_map(|b| b.iter().copied()).max().unwrap_or(1).max(1) as f32;
        for x in 0..w {
            let bin = (x * 256 / w).min(255);
            for c in 0..3 {
                let tall = ((self.bins[bin][c] as f32 / top).min(1.0) * h as f32).round() as usize;
                for y in h - tall..h {
                    let px = &mut out[(y * w + x) * 4..(y * w + x) * 4 + 4];
                    px[c] = px[c].saturating_add(150);
                    px[3] = px[3].max(150);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Counts its renders and holds each one long enough for jobs to pile up behind it.
    struct Slow(Arc<Mutex<Vec<u64>>>);

    impl Renderer for Slow {
        fn render(&mut self, negative: &Linear, settings: &PipelineSettings) -> Result<Linear, String> {
            std::thread::sleep(Duration::from_millis(60));
            self.0.lock().unwrap().push(settings.print.exposure.to_bits() as u64);
            Ok(negative.clone())
        }
    }

    #[test]
    fn a_burst_of_notches_renders_the_first_and_the_last_only() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::channel();
        let worker = Worker::new(Box::new(Slow(seen.clone())), Screen::Srgb, move |d| tx.send(d.map(|d| d.revision)).unwrap());
        let negative = Arc::new(Linear { width: 2, height: 1, pixels: vec![[0.5; 4]; 2] });
        for revision in 1..=20u64 {
            let mut settings = PipelineSettings::default();
            settings.print.exposure = revision as f32;
            worker.submit(Job { revision, path: PathBuf::new(), negative: negative.clone(), settings });
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut revisions = Vec::new();
        while let Ok(r) = rx.recv_timeout(Duration::from_millis(500)) {
            revisions.push(r.unwrap());
        }
        assert_eq!(revisions.last(), Some(&20), "the latest settings are always rendered");
        assert!(revisions.len() <= 3, "{revisions:?}: the slot replaced the waiting jobs");
        // Twin: twenty submissions, so a queue would have rendered twenty.
        assert!(seen.lock().unwrap().len() < 20);
    }

    #[test]
    fn the_histogram_counts_each_channel_on_its_own() {
        let h = Histogram::of(&[10, 20, 30, 255, 10, 10, 10, 255]);
        assert_eq!(h.bins[10], [2, 1, 1]);
        assert_eq!(h.bins[20], [0, 1, 0]);
        let raster = h.raster(256, 10);
        // Bin 10 is the tallest red: its column is red to the top, its neighbour empty.
        assert!(raster[10 * 4] > 0 && raster[11 * 4 + 3] == 0);
    }

    #[test]
    fn the_stand_in_renders_a_resting_negative_to_a_positive() {
        let settings = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
        let thin = Linear { width: 1, height: 2, pixels: vec![[0.6, 0.4, 0.3, 1.0], [0.05, 0.03, 0.02, 1.0]] };
        let out = Stage47.render(&thin, &settings).unwrap();
        let luma = |p: [f32; 4]| p[0] + p[1] + p[2];
        assert!(luma(out.pixels[0]) < luma(out.pixels[1]), "light through a thin negative prints dark: the conversion inverts");
        // Twin: the positive mode is refused by the stand-in rather than drawn wrong.
        let mut positive = settings.clone();
        positive.mode = oni_settings::ConversionMode::Positive;
        assert!(Stage47.render(&thin, &positive).is_err());
    }
}
