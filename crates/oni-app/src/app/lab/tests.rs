use super::*;
use oni_settings::scanner::ScannerClicks;

#[test]
fn the_screen_spells_every_count_and_the_balance_as_the_lab_does() {
    assert_eq!((spelled(0), spelled(5), spelled(-12)), ("0".into(), "+2.5".into(), "−6".into()));
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.scanner = ScannerClicks::new(3, -2, 0, 48, -20);
    s.scanner_machine = ScannerMachine::Frontier;
    let shown = screen(&s, Some((9, 38)));
    assert_eq!(shown.frame, "FRAME 09/38");
    assert_eq!(shown.machine, "FRONTIER · NEGATIVE");
    let values: Vec<&str> = shown.cells.iter().map(|c| c.1.as_str()).collect();
    assert_eq!(values, ["+1.5", "−1", "0", "+24", "−10"], "half the steps, in the units an old sidecar reads");
    assert_eq!(shown.cells[4].0, "TEMP");
    assert_eq!(shown.balance, "0.00 EV · 0.00 EV", "no Auto White Balance, no balance");
    // Auto White Balance's correction at the roll's strength acts on this face, read as Pro reads it.
    s.balance_filters = Some([0.4, -0.2, -0.4]);
    s.balance_strength = 0.5;
    let balanced = screen(&s, None);
    assert_eq!(balanced.balance, "WARM +0.40 EV · MAGENTA +0.10 EV");
    // Twin: Pro's Temperature moved by hand acts on Pro alone, so the screen never reads it.
    s.print.set_filters([1.0, 0.0, -1.0]);
    assert_eq!(crate::fields::Field::Temperature.readout(oni_core::Knob::Temperature.get(&s)), "Warm +2.00 EV");
    assert_eq!(screen(&s, None).balance, balanced.balance);
    // Twin: with no frame open the place is blank, and in black and white the channel says so.
    s.mode = ConversionMode::Monochrome;
    let mono = screen(&s, None);
    assert_eq!((mono.frame.as_str(), mono.machine.as_str()), ("FRAME --/--", "FRONTIER · B&W"));
}

#[test]
fn a_group_s_reset_clears_its_own_letters_alone() {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.scanner = ScannerClicks::new(3, -2, 1, 4, 5);
    let next = Group::Colour.rest(true).on(&s);
    assert_eq!(next.scanner, ScannerClicks::new(3, 0, 0, 0, 5));
    assert!(Group::Colour.modified(&s) && !Group::Colour.modified(&next));
    // Twin: Density's reset leaves the colour letters where they are.
    let next = Group::Density.rest(true).on(&s);
    assert_eq!(next.scanner, ScannerClicks::new(0, -2, 1, 4, 5));
    assert!(!Group::Effects.modified(&s));
    s.scanner_finish.vignette.stops = 0.3;
    s.vignette.stops = -0.4;
    assert!(Group::Effects.modified(&s));
    let rested = Group::Effects.rest(true).on(&s);
    assert_eq!((rested.scanner_finish.vignette.stops, rested.vignette.stops), (0.0, -0.4), "the Scanner's finishing, Pro's kept");
}

#[test]
fn the_face_is_remembered_across_launches() {
    let dir = std::env::temp_dir().join(format!("oni-app-lab-face-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(remembered(&dir), Face::Pro, "never written, the editor opens on Pro");
    remember(&dir, Face::Scanner);
    assert_eq!(remembered(&dir), Face::Scanner);
    remember(&dir, Face::Pro);
    assert_eq!(remembered(&dir), Face::Pro);
    let _ = std::fs::remove_dir_all(&dir);
}
