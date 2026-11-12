//! A sidecar from before format 21 kept its dust strokes on the cropped, oriented frame: read once,
//! they move onto the scan through the geometry they were painted under, and are written so.

use oni_settings::cg::{Point, Rect};
use oni_settings::migrate::chain::STROKES_ON_SCAN_VERSION;
use oni_settings::migrate::{self, Facts, Refusal};
use oni_settings::frame::{Correction, Geometry};
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::migrate::strokes::caught_up;
use oni_settings::{ConversionMode, PipelineSettings};
use std::path::Path;
use std::sync::Arc;

const SCAN: (f64, f64) = (6048.0, 4032.0);

fn painted(geometry: Geometry) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.geometry = geometry;
    s.corrections = vec![Correction::new(vec![Point::new(0.2, 0.3), Point::new(0.25, 0.31)]), Correction::new(vec![Point::new(0.7, 0.8), Point::new(0.7001, 0.8)])];
    s
}

fn framed() -> Geometry {
    Geometry { quarter_turns: 1, angle: 2.5, crop: Rect::new(0.1, 0.15, 0.7, 0.6), ..Geometry::default() }
}

fn stored(version: i64, settings: &PipelineSettings, original: &Path) {
    sidecar::write(settings, original, WriteOptions::default()).unwrap();
    let path = sidecar::url_for(original);
    let current = oni_settings::document::decode(&std::fs::read(&path).unwrap()).unwrap();
    let value = oni_settings::migrate::reshape::as_stored(&current, version);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
}

#[test]
fn an_older_stroke_moves_onto_the_scan_once_and_an_unknown_size_keeps_it_read_only() {
    let before = STROKES_ON_SCAN_VERSION - 1;
    let old = painted(framed());
    let placing = framed().placing(SCAN);
    let moved = caught_up(before, old.clone(), &|_| Ok(SCAN)).unwrap();
    for (c, was) in moved.corrections.iter().zip(&old.corrections) {
        for (p, q) in c.stroke.iter().zip(&was.stroke) {
            let wanted = placing.on_scan(*q);
            assert!((p.x - wanted.x).abs() < 1e-12 && (p.y - wanted.y).abs() < 1e-12, "{p:?} for {wanted:?}");
            // Twin: the stroke as stored lies elsewhere on the scan.
            assert!((p.x - q.x).hypot(p.y - q.y) > 0.05, "{p:?} against {q:?}");
        }
    }
    assert_eq!((moved.id_of_corrections(), moved.geometry), (old.id_of_corrections(), old.geometry), "only the points move");
    // A stroke of this format, a neutral framing and no stroke at all ask nothing and stay.
    let unasked = |_: &PipelineSettings| -> Result<(f64, f64), Refusal> { panic!("the size was asked") };
    assert_eq!(caught_up(STROKES_ON_SCAN_VERSION, old.clone(), &unasked), Ok(old.clone()));
    let whole = painted(Geometry::default());
    assert_eq!(caught_up(before, whole.clone(), &unasked), Ok(whole));
    let bare = PipelineSettings { corrections: Vec::new(), ..old.clone() };
    assert_eq!(caught_up(before, bare.clone(), &unasked), Ok(bare));
    // No size known: refused, so the frame is never written in this format with its strokes astray.
    assert_eq!(caught_up(before, old, &|_| Err(Refusal::ScanUnknown)), Err(Refusal::ScanUnknown));
}

#[test]
fn a_sidecar_read_through_the_installed_size_is_written_on_the_scan_at_its_first_write() {
    let dir = std::env::temp_dir().join(format!("oni-strokes-on-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (known, unknown) = (dir.join("DSC_0001.NEF"), dir.join("DSC_0002.NEF"));
    let held = known.clone();
    migrate::install(Some(Arc::new(move |p: &Path, mosaic: Option<bool>, then: &mut dyn FnMut(&Facts)| {
        then(&Facts { mosaic: migrate::Mosaic::Known(mosaic.unwrap_or(true)), matrix: &|| Ok(None), scan: &|_| (p == held).then_some(SCAN).ok_or(Refusal::ScanUnknown) })
    })));
    let old = painted(framed());
    for original in [&known, &unknown] {
        std::fs::write(original, b"raw").unwrap();
        stored(STROKES_ON_SCAN_VERSION - 1, &old, original);
    }
    let read = sidecar::read(&known, true).unwrap();
    assert!(read.writable);
    assert_eq!(read.settings.corrections[0].stroke[0], framed().placing(SCAN).on_scan(old.corrections[0].stroke[0]));
    assert!(matches!(migrate::standing(&std::fs::read(sidecar::url_for(&known)).unwrap()), migrate::Standing::Older(_)), "a read writes nothing");
    sidecar::write(&read.settings, &known, WriteOptions::default()).unwrap();
    let written = std::fs::read(sidecar::url_for(&known)).unwrap();
    assert_eq!(migrate::standing(&written), migrate::Standing::Current, "the write brought it on the scan");
    assert_eq!(sidecar::read(&known, true).unwrap().settings, read.settings, "read again, it moves no further");
    // Twin: a file whose size is unknown opens read-only with its strokes as stored, its file untouched.
    let before = std::fs::read(sidecar::url_for(&unknown)).unwrap();
    let stays = sidecar::read(&unknown, true).unwrap();
    assert!(!stays.writable && stays.held_back == Some(Refusal::ScanUnknown));
    assert_eq!(stays.settings.corrections, old.corrections);
    assert_eq!(std::fs::read(sidecar::url_for(&unknown)).unwrap(), before);
    assert!(matches!(sidecar::write(&stays.settings, &unknown, WriteOptions::default()), Err(sidecar::Error::Held(Refusal::ScanUnknown))));
    migrate::install(None);
    let _ = std::fs::remove_dir_all(&dir);
}

trait Ids {
    fn id_of_corrections(&self) -> Vec<(oni_settings::id::Id, f64)>;
}

impl Ids for PipelineSettings {
    fn id_of_corrections(&self) -> Vec<(oni_settings::id::Id, f64)> {
        self.corrections.iter().map(|c| (c.id, c.radius)).collect()
    }
}
