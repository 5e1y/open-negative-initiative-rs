//! A batch written as a pipeline: two frames decoding, one on the GPU, one encoded and written, each
//! handed on in order, so a 24 Mpx batch runs at the decode's pace and never holds more than `IN_FLIGHT` frames.

use super::{Target, full_decode, render_full_beside};
use crate::render::{Reading, Shared};
use oni_export::file::Ledger;
use oni_export::{ExifMetadata, Failure, Landing, Settings};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, mpsc};

/// Frames decoded at once: LibRaw demosaics on one core, and the app's decode gate admits two.
pub const DECODERS: usize = crate::decode_gate::DECODES;

/// Frames held at once from their decode to their file, about 0.5 GB each at 24 Mpx: the decoders',
/// the GPU's and the writer's.
pub const IN_FLIGHT: usize = DECODERS + 2;

/// One frame of a batch: its photo, its file and the line written into it.
#[derive(Clone, Debug)]
pub struct Job {
    pub target: Target,
    pub destination: PathBuf,
    pub metadata: ExifMetadata,
}

/// How a batch ended: every file written, stopped by the hand, or the first frame that failed.
#[derive(Debug, PartialEq)]
pub enum End<E> {
    Done,
    Stopped,
    Failed(usize, E),
}

/// How the stages hand frames on: one at a time, or, for the check that sees the bound, unbounded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hand {
    Bounded,
    Unbounded,
}

/// A frame counted from its decode until it is dropped, written or not.
struct Held<'a> {
    count: &'a AtomicUsize,
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
    }
}

struct Shelf<'a, A, E> {
    ready: BTreeMap<usize, (Held<'a>, Result<A, E>)>,
    /// The frame the GPU stage takes next.
    expected: usize,
    decoders: usize,
    closed: bool,
}

/// `count` frames through `decode` (on `decoders` threads), `render`, then `write` here, in order; after a
/// stop or a failure nothing starts and nothing is written. Returns the end and the most frames held.
#[allow(clippy::too_many_arguments)]
pub fn staged<A: Send, B: Send, E: Send>(
    count: usize,
    decoders: usize,
    stop: &AtomicBool,
    decode: impl Fn(usize) -> Result<A, E> + Sync,
    mut render: impl FnMut(usize, A) -> Result<B, E> + Send,
    mut write: impl FnMut(usize, B) -> Result<(), E>,
    mut landed: impl FnMut(usize),
    hand: Hand,
) -> (End<E>, usize) {
    let decoders = decoders.max(1);
    let (held, peak, failed, next) = (AtomicUsize::new(0), AtomicUsize::new(0), AtomicBool::new(false), AtomicUsize::new(0));
    let halted = || stop.load(Ordering::SeqCst) || failed.load(Ordering::SeqCst);
    let enter = || {
        peak.fetch_max(held.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
        Held { count: &held }
    };
    let shelf = Mutex::new(Shelf { ready: BTreeMap::new(), expected: 0, decoders, closed: false });
    let turn = Condvar::new();
    let lock = || shelf.lock().unwrap_or_else(|e| e.into_inner());
    let end = std::thread::scope(|scope| {
        for _ in 0..decoders {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    // A decoder starts a frame only while the GPU stage is at most `decoders` frames behind it.
                    let mut s = lock();
                    while hand == Hand::Bounded && !s.closed && i >= s.expected + decoders {
                        s = turn.wait(s).unwrap_or_else(|e| e.into_inner());
                    }
                    if i >= count || s.closed || halted() {
                        // A frame never decoded closes the batch, so no decoder waits on it.
                        s.closed |= i < count;
                        break;
                    }
                    drop(s);
                    let frame = enter();
                    let decoded = decode(i);
                    lock().ready.insert(i, (frame, decoded));
                    turn.notify_all();
                }
                lock().decoders -= 1;
                turn.notify_all();
            });
        }
        let (to_writer, from_renderer) = match hand {
            Hand::Bounded => {
                let (tx, rx) = mpsc::sync_channel(0);
                (Sender::Bounded(tx), rx)
            }
            Hand::Unbounded => {
                let (tx, rx) = mpsc::channel();
                (Sender::Unbounded(tx), rx)
            }
        };
        let (shelf, turn) = (&shelf, &turn);
        let halted = &halted;
        scope.spawn(move || {
            loop {
                let mut s = shelf.lock().unwrap_or_else(|e| e.into_inner());
                let taken = loop {
                    let expected = s.expected;
                    if let Some(frame) = s.ready.remove(&expected) {
                        s.expected += 1;
                        break Some((expected, frame));
                    }
                    if s.decoders == 0 {
                        break None;
                    }
                    s = turn.wait(s).unwrap_or_else(|e| e.into_inner());
                };
                if halted() {
                    s.closed = true;
                }
                drop(s);
                turn.notify_all();
                let Some((i, (frame, decoded))) = taken else { break };
                if halted() {
                    continue;
                }
                let rendered = decoded.and_then(|a| render(i, a));
                if !to_writer.send((i, frame, rendered)) {
                    break;
                }
            }
            shelf.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
            turn.notify_all();
        });
        let mut end = End::Done;
        for (i, frame, rendered) in from_renderer {
            if matches!(end, End::Done) && stop.load(Ordering::SeqCst) {
                end = End::Stopped;
            }
            if !matches!(end, End::Done) {
                continue;
            }
            match rendered.and_then(|b| write(i, b)) {
                Ok(()) => {
                    drop(frame);
                    landed(i);
                }
                Err(e) => {
                    failed.store(true, Ordering::SeqCst);
                    end = End::Failed(i, e);
                }
            }
        }
        end
    });
    (end, peak.load(Ordering::SeqCst))
}

enum Sender<T> {
    Bounded(mpsc::SyncSender<T>),
    Unbounded(mpsc::Sender<T>),
}

impl<T> Sender<T> {
    fn send(&self, value: T) -> bool {
        match self {
            Sender::Bounded(tx) => tx.send(value).is_ok(),
            Sender::Unbounded(tx) => tx.send(value).is_ok(),
        }
    }
}

/// `writeExport`'s files: each frame decoded whole, rendered unproofed, written and its line kept in
/// its sidecar, the same bytes as `write_one` writes; `landed(i)` once frame i is on disk.
pub fn write_batch(engine: &Shared, jobs: &[Job], export: &Settings, ledger: &mut Ledger, replacing: bool, stop: &AtomicBool, landed: impl FnMut(usize)) -> (End<Failure>, usize) {
    let ledger = Mutex::new(ledger);
    let decode = |i: usize| {
        let job = &jobs[i];
        let held = ledger.lock().map_err(|_| Failure::Render)?;
        oni_export::lands(&job.target.path, &job.destination, &held, replacing)?;
        drop(held);
        full_decode(&job.target.path, Reading::of(&job.target.settings))
    };
    let render = |i: usize, decoded| render_full_beside(engine, decoded, &jobs[i].target.settings);
    let write = |i: usize, full| {
        let job = &jobs[i];
        let date = (!job.metadata.date.is_empty()).then_some(job.metadata.date.as_str());
        let bytes = oni_export::file_bytes(&job.target.path, full, &job.target.settings.paper_correction, export, &job.metadata, date)?;
        let mut ledger = ledger.lock().map_err(|_| Failure::Render)?;
        oni_export::land(Landing { path: &job.destination, ledger: &mut **ledger, replacing }, &bytes)?;
        drop(ledger);
        let mosaic = oni_decode::decodes_via_mosaic(&job.target.path);
        let _ = oni_settings::sidecar::set_exif(job.metadata.clone(), &job.target.path, mosaic);
        Ok(())
    };
    staged(jobs.len(), DECODERS, stop, decode, render, write, landed, Hand::Bounded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Ten frames whose writes are slow: the decoders would run ahead were nothing holding them back.
    fn run(hand: Hand, stop_after: Option<usize>, fail_at: Option<usize>) -> (End<usize>, usize, Vec<usize>) {
        let stop = AtomicBool::new(false);
        let mut written = Vec::new();
        let (end, peak) = staged(
            10,
            DECODERS,
            &stop,
            |i| if Some(i) == fail_at { Err(i) } else { Ok(i) },
            |_, a| Ok(a * 10),
            |i, b| {
                assert_eq!(b, i * 10, "each frame reaches the writer with its own render");
                std::thread::sleep(Duration::from_millis(15));
                Ok(())
            },
            |i| {
                written.push(i);
                if Some(written.len()) == stop_after {
                    stop.store(true, Ordering::SeqCst);
                }
            },
            hand,
        );
        (end, peak, written)
    }

    #[test]
    fn a_batch_writes_in_order_and_holds_no_more_than_its_frames_in_flight() {
        let (end, peak, written) = run(Hand::Bounded, None, None);
        assert_eq!(end, End::Done);
        assert_eq!(written, (0..10).collect::<Vec<_>>(), "the files land in the batch's order");
        assert!(peak <= IN_FLIGHT, "{peak} frames held at once");
        // Twin: handed on without a bound, the decoders run ahead of the slow writer.
        let (_, unbounded, _) = run(Hand::Unbounded, None, None);
        assert!(unbounded > IN_FLIGHT, "an unbounded batch held only {unbounded} frames");
    }

    /// The file with each embedded profile's creation date blanked: the profile is made as the file is written.
    fn undated(mut file: Vec<u8>) -> Vec<u8> {
        let at: Vec<usize> = file.windows(4).enumerate().filter(|(i, w)| *w == b"acsp" && *i >= 12).map(|(i, _)| i).collect();
        for i in at {
            file[i - 12..i].fill(0);
        }
        file
    }

    /// The batch writes, byte for byte, what one frame at a time writes, in either format.
    #[test]
    fn a_batch_writes_the_bytes_one_frame_at_a_time_writes() {
        let Ok(device) = crate::render::Device::open() else {
            eprintln!("no adapter: the batch's bytes are not exercised");
            return;
        };
        let engine = crate::render::Engine::shared(&device);
        let root = std::env::temp_dir().join(format!("oni-export-batch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let (w, h) = (301u32, 203u32);
        let targets: Vec<Target> = (0..3)
            .map(|k| {
                let path = root.join(format!("F{k}.tiff"));
                let data: Vec<u16> = (0..w * h * 3).map(|i| ((i * 7919 + k * 4099) % 50000 + 3000) as u16).collect();
                let file = std::fs::File::create(&path).unwrap();
                tiff::encoder::TiffEncoder::new(file).unwrap().write_image::<tiff::encoder::colortype::RGB16>(w, h, &data).unwrap();
                let mut settings = oni_settings::PipelineSettings::default();
                settings.density = 0.1 * k as f32;
                Target { path, settings }
            })
            .collect();
        for format in oni_export::Format::ALL {
            let mut export = Settings::default();
            export.set_format(format);
            let (one, many) = (root.join(format!("one-{}", format.key())), root.join(format!("many-{}", format.key())));
            std::fs::create_dir_all(&one).unwrap();
            std::fs::create_dir_all(&many).unwrap();
            let named = |dir: &std::path::Path, t: &Target| dir.join(oni_export::name(&t.path, "", format));
            for t in &targets {
                let to = Landing { path: &named(&one, t), ledger: &mut Ledger::none(), replacing: false };
                super::super::write_one(&engine, t, to, &export, &ExifMetadata::default()).unwrap();
            }
            let jobs: Vec<Job> = targets.iter().map(|t| Job { target: t.clone(), destination: named(&many, t), metadata: ExifMetadata::default() }).collect();
            let (end, _) = write_batch(&engine, &jobs, &export, &mut Ledger::none(), false, &AtomicBool::new(false), |_| {});
            assert_eq!(end, End::Done);
            for t in &targets {
                let (a, b) = (undated(std::fs::read(named(&one, t)).unwrap()), undated(std::fs::read(named(&many, t)).unwrap()));
                assert!(a == b, "{format:?} {} differs", t.path.display());
                // Twin: one byte of the picture moved is caught.
                let mut moved = b.clone();
                let last = moved.len() - 3;
                moved[last] ^= 1;
                assert!(a != moved);
            }
        }
        // The render beside the engine is the render inside it, bit for bit; one ulp off is caught.
        let decoded = || super::super::full_decode(&targets[1].path, Reading::default()).unwrap();
        let inside = super::super::render_full(&mut engine.lock().unwrap(), decoded(), &targets[1].settings).unwrap();
        let beside = render_full_beside(&engine, decoded(), &targets[1].settings).unwrap();
        let bits = |i: &oni_export::Image| i.pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&inside), bits(&beside));
        let mut off = beside.clone();
        off.pixels[1000] = f32::from_bits(off.pixels[1000].to_bits() + 1);
        assert_ne!(bits(&inside), bits(&off));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_stop_or_a_failure_writes_nothing_after_it() {
        let (end, _, written) = run(Hand::Bounded, Some(3), None);
        assert_eq!(end, End::Stopped);
        assert_eq!(written, [0, 1, 2], "the stop is honoured between files");
        let (end, _, written) = run(Hand::Bounded, None, Some(4));
        assert_eq!(end, End::Failed(4, 4));
        assert_eq!(written, [0, 1, 2, 3], "every frame before the failure is written");
        // Twin: without either, every frame is written.
        assert_eq!(run(Hand::Bounded, None, None).2.len(), 10);
    }
}
