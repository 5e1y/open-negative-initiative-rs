use super::*;
use oni_settings::scanner::ScannerClicks;

#[test]
fn the_screen_spells_every_count_and_finish_as_the_lab_does() {
    assert_eq!([-3, -1, 1, 3].map(legend), ["−3", "−", "+", "+3"]);
    assert_eq!((spelled(0), spelled(5), spelled(-12)), ("0".into(), "+2.5".into(), "−6".into()));
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.scanner = ScannerClicks::new(3, -2, 0, 48, -20);
    s.scanner_finish.contrast = 0.2;
    s.scanner_machine = ScannerMachine::Frontier;
    let shown = screen(&s, Some((9, 38)));
    assert_eq!(shown.frame, "FRAME 09/38");
    assert_eq!(shown.machine, "FRONTIER · NEGATIVE");
    let values: Vec<&str> = shown.cells.iter().map(|c| c.1.as_str()).collect();
    assert_eq!(values, ["+1.5", "−1", "0", "+24", "−10"], "half the steps, in the units an old sidecar reads");
    assert_eq!(shown.cells[4].0, "TEMP");
    assert_eq!(shown.finish, ["CONTRAST +20%", "VIGNETTE 0.00 EV", "CHROME 0%"]);
    // Each effect reads as Pro's slider of the same quantity; twin: Pro's Contrast is no bare number.
    assert_eq!(shown.finish[0], format!("CONTRAST {}", crate::fields::Field::Contrast.readout(0.2)));
    assert_ne!(crate::fields::Field::Contrast.readout(0.2), "+0.20");
    // Twin: in black and white Chrome leaves the screen, and with no frame open the place is blank.
    s.mode = ConversionMode::Monochrome;
    let mono = screen(&s, None);
    assert_eq!((mono.frame.as_str(), mono.machine.as_str(), mono.finish.len()), ("FRAME --/--", "FRONTIER · B&W", 2));
}

#[test]
fn a_group_s_reset_clears_its_own_letters_alone() {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.scanner = ScannerClicks::new(3, -2, 1, 4, 5);
    let Command::Write(next) = Group::Colour.reset(&s, &s) else { panic!("a letter group writes the clicks") };
    assert_eq!(next.scanner, ScannerClicks::new(3, 0, 0, 0, 5));
    assert!(Group::Colour.modified(&s) && !Group::Colour.modified(&next));
    // Twin: Density's reset leaves the colour letters where they are.
    let Command::Write(next) = Group::Density.reset(&s, &s) else { panic!() };
    assert_eq!(next.scanner, ScannerClicks::new(0, -2, 1, 4, 5));
    assert!(!Group::Effects.modified(&s));
    s.scanner_finish.vignette.stops = 0.3;
    assert!(Group::Effects.modified(&s));
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
