use super::*;
use crate::test_support::Scratch;
use oni_settings::frame::RollReference;
use oni_settings::{ConversionMode, Id};

const ZF: [f64; 3] = [0.063, -0.026, -0.126];

/// A frame as the Swift app leaves a prepared roll: measured gains and the roll's reference.
fn prepared() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.gains.stops = [-1.1826692, 0.0, 1.324959];
    s.levels.red.black = 0.8755958;
    s.roll_reference = Some(RollReference {
        id: Id::new(),
        stops: [-1.1826692, 0.0, 1.324959],
        black: [0.46861503, 0.47104105, 0.46834671],
        white: [0.71532, 0.7508647, 0.831724],
        base: None,
        median: Some([0.3658495, 0.35180447, 0.32368147]),
    });
    s
}

#[test]
fn only_the_gains_before_the_logarithm_move() {
    let s = prepared();
    let out = shifted(&s, ZF);
    for c in 0..3 {
        assert!((out.gains.stops[c] - (s.gains.stops[c] + ZF[c] as f32)).abs() < 1e-6);
        let (r, o) = (s.roll_reference.as_ref().unwrap(), out.roll_reference.as_ref().unwrap());
        assert!((o.stops[c] - (r.stops[c] + ZF[c] as f32)).abs() < 1e-6);
        assert_eq!((o.black, o.white, o.median), (r.black, r.white, r.median));
    }
    assert_eq!((out.levels.clone(), out.print.clone(), out.density_ceiling), (s.levels.clone(), s.print.clone(), s.density_ceiling));
    // Twin: the shift the other way round lands elsewhere, and a frame with no reference gains none.
    assert_ne!(shifted(&s, ZF.map(|v| -v)).gains, out.gains);
    let bare = PipelineSettings::rest(ConversionMode::Negative, true);
    assert!(shifted(&bare, ZF).roll_reference.is_none());
}

/// A RAW's name and its sidecar in `folder`; the RAW's bytes are never read.
fn frame(folder: &Path, name: &str, settings: &PipelineSettings) -> std::path::PathBuf {
    let raw = folder.join(name);
    std::fs::write(&raw, b"a RAW's bytes").unwrap();
    sidecar::write(settings, &raw, sidecar::WriteOptions::default()).unwrap();
    raw
}

#[test]
fn a_sidecar_is_backed_up_corrected_and_restored_to_its_bytes() {
    let (support, roll) = (Scratch::new("support"), Scratch::new("roll"));
    let raw = frame(roll.path(), "DSC_0001.NEF", &prepared());
    let found = std::fs::read(sidecar::url_for(&raw)).unwrap();
    let e = correct(support.path(), &raw, ZF);
    assert_eq!(e.outcome, Outcome::Corrected, "{e:?}");
    let read = sidecar::read(&raw, true, true).unwrap().settings;
    assert!((read.gains.stops[2] - (1.324959 + ZF[2] as f32)).abs() < 1e-6);
    let kept = records::backup_folder(support.path()).join(e.backup.clone().unwrap());
    assert_eq!(std::fs::read(kept).unwrap(), found);
    let mut ledger = Ledger::default();
    ledger.record(support.path(), e).unwrap();
    assert!(restore(support.path(), Some(Path::new("/elsewhere"))).restored.is_empty(), "a folder restores its own sidecars alone");
    let restored = restore(support.path(), Some(roll.path()));
    assert_eq!(restored.restored.len(), 1);
    assert_eq!(std::fs::read(sidecar::url_for(&raw)).unwrap(), found);
    // Twin: a sidecar edited after its correction is kept, never overwritten by the backup.
    let other = frame(roll.path(), "DSC_0002.NEF", &prepared());
    ledger.record(support.path(), correct(support.path(), &other, ZF)).unwrap();
    let mut edited = sidecar::read(&other, true, true).unwrap().settings;
    edited.density = 0.3;
    sidecar::write(&edited, &other, sidecar::WriteOptions::default()).unwrap();
    let again = restore(support.path(), None);
    assert_eq!(again.kept.iter().filter(|(p, _)| p.contains("DSC_0002")).count(), 1);
    assert_eq!(sidecar::read(&other, true, true).unwrap().settings.density, 0.3);
}

#[test]
fn a_newer_unreadable_or_passes_sidecar_is_left_byte_for_byte() {
    let (support, roll) = (Scratch::new("support"), Scratch::new("roll"));
    let newer = frame(roll.path(), "A.NEF", &prepared());
    let text = std::fs::read_to_string(sidecar::url_for(&newer)).unwrap().replace("\"version\" : 19", "\"version\" : 99");
    std::fs::write(sidecar::url_for(&newer), &text).unwrap();
    let torn = roll.path().join("B.NEF");
    std::fs::write(&torn, b"raw").unwrap();
    std::fs::write(sidecar::url_for(&torn), b"{ \"settings\" : ").unwrap();
    let mut passes = prepared();
    passes.pass_of = Some("C-lead.NEF".into());
    let pass = frame(roll.path(), "C.NEF", &passes);
    for (raw, why) in [(&newer, "newer"), (&torn, "unreadable"), (&pass, "RGB passes")] {
        let before = std::fs::read(sidecar::url_for(raw)).unwrap();
        let e = correct(support.path(), raw, ZF);
        assert_eq!(e.outcome, Outcome::Skipped, "{e:?}");
        assert!(e.reason.unwrap().contains(why));
        assert_eq!(std::fs::read(sidecar::url_for(raw)).unwrap(), before);
    }
    assert!(records::manifest(support.path()).is_empty(), "nothing left alone is backed up");
    // Twin: the same sidecar at the current version is corrected.
    let current = frame(roll.path(), "D.NEF", &prepared());
    assert_eq!(correct(support.path(), &current, ZF).outcome, Outcome::Corrected);
}
