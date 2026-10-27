//! The automatic keys' reader in the app: a graph of its own on the app's device, made on the first
//! press, so a reading never waits on the canvas's engine.

use crate::render;
use oni_core::balance::{Kept, Measure, Outcome};
use oni_measure::key::{Decoded, Measurer};
use oni_settings::PipelineSettings;
use oni_pipeline::flat_field::Mask;
use oni_pipeline::resident::Files;
use oni_settings::id::Id;
use crate::latest::{Latest, Taken};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// The support folder the custom masks are kept in and the caches the dust patches are kept in,
/// as the canvas's engine reads them; set as the window is made.
static FOLDERS: RwLock<(Option<PathBuf>, Option<PathBuf>)> = RwLock::new((None, None));

pub fn set_folders(support: &Path, caches: Option<PathBuf>) {
    if let Ok(mut f) = FOLDERS.write() {
        *f = (Some(support.to_path_buf()), caches);
    }
}

/// The images a measure or a roll preview lays, read where the canvas reads them.
pub struct Folders;

impl Files for Folders {
    fn mask(&self, id: &str) -> Option<Mask> {
        let support = FOLDERS.read().ok()?.0.clone()?;
        crate::flat_fields::read(&support, id)
    }

    fn patch(&self, id: Id) -> Option<(u32, u32, Vec<[f32; 4]>)> {
        let caches = FOLDERS.read().ok()?.1.clone()?;
        crate::patches::read(&caches, id)
    }
}

pub fn folders() -> Arc<dyn Files> {
    Arc::new(Folders)
}

/// The reader, or `None` where no GPU adapter answers; it decodes each file as the canvas does.
pub fn measurer() -> Option<&'static Measurer> {
    static MEASURER: OnceLock<Option<Measurer>> = OnceLock::new();
    MEASURER
        .get_or_init(|| {
            let graph = render::graph()?;
            Some(Measurer::new(
                graph,
                Box::new(|path: &Path, s: &PipelineSettings| {
                    let d = render::decode_for(path, render::Reading::of(s), render::PREVIEW_SIDE)?;
                    Ok(Decoded { width: d.linear.width as u32, height: d.linear.height as u32, pixels: d.linear.pixels, full_width: d.full_width })
                }),
                Box::new(|path: &Path, s: &PipelineSettings| format!("{:?} {:?}", render::Reading::of(s), sources(path, s))),
            )
            .laying(folders()))
        })
        .as_ref()
}

/// Every frame's Auto White Balance outcome on the settings given, the one kept when it still holds;
/// one read here is not kept, as these settings may be a preview's. A frame that reads nothing is absent.
pub fn balance_outcomes(frames: &[(String, PipelineSettings)]) -> HashMap<String, Outcome> {
    let m = measurer();
    frames.iter().filter_map(|(path, s)| kept(path, s).or_else(|| m?.balance(path, s)).map(|o| (path.clone(), o))).collect()
}

/// A file's size and change date, which any rewrite of it moves.
fn stat(path: &Path) -> Option<String> {
    let m = std::fs::metadata(path).ok()?;
    let at = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    Some(format!("{}@{at}", m.len()))
}

/// What a frame's decode reads: its original and its passes, each by size and change date, and the
/// cameras' rendition matrices; `None` when the original cannot be looked at.
fn sources(original: &Path, s: &PipelineSettings) -> Option<String> {
    let mut out = format!("{} rendition {:016x}", stat(original)?, oni_decode::rendition::print());
    if let Some(p) = &s.rgb_passes {
        let beside = |name: &str| original.parent().map_or_else(|| PathBuf::from(name), |f| f.join(name));
        for name in [&p.red, &p.green, &p.blue] {
            out += &format!(" pass {name} {:?}", stat(&beside(name)));
        }
    }
    Some(out)
}

/// What a frame's reading reads besides its settings and the reading's code: its decode's files,
/// the custom mask it lays and its dust patches; `None` when the original cannot be looked at.
pub fn facts(path: &str, s: &PipelineSettings) -> Option<String> {
    let mut out = sources(Path::new(path), s)?;
    let (support, caches) = FOLDERS.read().map(|f| f.clone()).unwrap_or_default();
    if s.flat_field.mask == oni_settings::frame::FlatFieldMask::Custom
        && let (Some(id), Some(support)) = (&s.flat_field.custom, &support)
    {
        // The id is the mask's content and its passes, which fix how it lays.
        out += &format!("\nmask {id} {:?}", stat(&crate::flat_fields::file(support, id)));
    }
    for c in &s.corrections {
        out += &format!("\npatch {} {:?}", c.id, caches.as_ref().and_then(|at| stat(&crate::patches::file(at, c.id))));
    }
    Some(out)
}

/// Outcomes kept this session, newest last, a few a frame so an undo finds the one before; a press
/// reads no sidecar for a frame read here.
static KEPT: Mutex<VecDeque<(String, Kept)>> = Mutex::new(VecDeque::new());
const REMEMBERED: usize = 512;
/// The fingerprint each frame was last read for: a reading overtaken by a newer one is never kept.
static ASKED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
/// The readings made, which the checks count.
pub static READINGS: AtomicUsize = AtomicUsize::new(0);

/// The frame's balance fingerprint as it reads now; `None` when its original cannot be looked at.
pub fn fingerprint(path: &str, s: &PipelineSettings) -> Option<String> {
    Some(oni_measure::key::balance_fingerprint(s, &facts(path, s)?))
}

/// The outcome kept for the frame as it reads now, from this session or its sidecar; never one
/// kept for anything else. A roll of one stands on a roll of its own at each press, as Swift's does.
pub fn kept(path: &str, s: &PipelineSettings) -> Option<Outcome> {
    let print = fingerprint(path, s)?;
    let held = KEPT.lock().unwrap_or_else(|e| e.into_inner()).iter().rev().find_map(|(p, k)| if p == path { k.outcome_for(&print) } else { None });
    let mut found = match held {
        Some(found) => found,
        None => {
            let k = Kept::of(&oni_settings::sidecar::auto_balance(Path::new(path))?)?;
            let found = k.outcome_for(&print)?;
            remember(path, k);
            found
        }
    };
    if s.roll_reference.is_none() {
        found.reference.id = oni_settings::Id::new();
    }
    Some(found)
}

fn remember(path: &str, k: Kept) {
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    kept.retain(|(p, q)| !(p == path && q.fingerprint == k.fingerprint));
    if kept.len() >= REMEMBERED {
        kept.pop_front();
    }
    kept.push_back((path.to_string(), k));
}

/// Reads the frame and keeps what it found under the fingerprint taken before the read, in this
/// session and in its sidecar, unless a newer reading of the frame was asked meanwhile.
pub fn measured(path: &str, s: &PipelineSettings) -> Option<Outcome> {
    let print = fingerprint(path, s);
    if let (Some(p), Ok(mut asked)) = (&print, ASKED.lock()) {
        asked.retain(|(q, _)| q != path);
        if asked.len() >= REMEMBERED {
            asked.remove(0);
        }
        asked.push((path.to_string(), p.clone()));
    }
    let found = measurer()?.balance(path, s)?;
    READINGS.fetch_add(1, Ordering::SeqCst);
    let Some(print) = print else { return Some(found) };
    // A file the reading read moved under it, or a newer reading asked: nothing is kept.
    let current = ASKED.lock().ok().is_some_and(|a| a.iter().any(|(q, p)| q == path && *p == print));
    if current && fingerprint(path, s).as_ref() == Some(&print) {
        let k = Kept::new(print, &found);
        remember(path, k.clone());
        if let Some(value) = k.value() {
            let _ = oni_settings::sidecar::set_auto_balance(Path::new(path), oni_decode::decodes_via_mosaic(Path::new(path)), &value);
        }
    }
    Some(found)
}

/// A frame read ahead: its path, its settings or `None` for its sidecar's, and whether its measuring
/// copy is made even when its balance is kept, as the open frame's is for the other keys.
pub type Ahead = (String, Option<PipelineSettings>, bool);

static AHEAD: Latest<Vec<Ahead>> = Latest::new();
static WARMER: std::sync::Once = std::sync::Once::new();

/// Reads the frames' balances ahead of a press, one at a time off the interface thread and behind
/// any decode the hand waits on; a newer list replaces this one, from its next frame.
pub fn ahead(frames: Vec<Ahead>) {
    WARMER.call_once(|| {
        let _ = std::thread::Builder::new().name("balance ahead".into()).spawn(|| {
            crate::decode_gate::in_background();
            while let Taken::Value(frames) = AHEAD.take(None) {
                let mut prepared = None;
                for (path, s, prepare) in frames {
                    if AHEAD.waiting() {
                        break;
                    }
                    let mosaic = oni_decode::decodes_via_mosaic(Path::new(&path));
                    let Some(s) = s.or_else(|| oni_settings::sidecar::read(Path::new(&path), mosaic, true).ok().map(|l| l.settings)) else { continue };
                    // A frame that cannot be read leaves the frames after it to be read.
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if prepare && let Some(m) = measurer() {
                            m.prepare(&path, &s);
                        }
                        if kept(&path, &s).is_none() {
                            measured(&path, &s);
                        }
                    }));
                    if prepare {
                        prepared = Some((path, s));
                    }
                }
                // The open frame's copy made last, so the copies of the frames read after it never push it out.
                if let (Some((path, s)), Some(m)) = (prepared, measurer()) {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.prepare(&path, &s)));
                }
            }
        });
    });
    AHEAD.put(frames);
}
