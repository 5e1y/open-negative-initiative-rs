use super::*;
use oni_settings::scanner::ScannerClicks;

#[test]
fn the_screen_spells_every_count_and_finish_as_the_lab_does() {
    assert_eq!([-3, -1, 1, 3].map(legend), ["−3", "−", "+", "+3"]);
    assert_eq!((spelled(0), spelled(5), spelled(-12)), ("0".into(), "+2.5".into(), "−6".into()));
    assert_eq!((percent(0.004), percent(0.15), percent(-0.05)), ("0 %".into(), "+15 %".into(), "−5 %".into()));
    assert_eq!((stops(0.004), stops(0.3), stops(-1.25)), ("0 EV".into(), "+0.3 EV".into(), "−1.2 EV".into()));
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.scanner = ScannerClicks::new(3, -2, 0, 48, -20);
    s.scanner_finish.contrast = 0.2;
    s.scanner_machine = ScannerMachine::Frontier;
    let shown = screen(&s, Some((9, 38)));
    assert_eq!(shown.frame, "FRAME 09/38");
    assert_eq!(shown.machine, "FRONTIER · NEGA");
    let values: Vec<&str> = shown.cells.iter().map(|c| c.1.as_str()).collect();
    assert_eq!(values, ["+1.5", "−1", "0", "+24", "−10"], "half the steps, in the units an old sidecar reads");
    assert_eq!(shown.cells[4].0, "TEMP");
    assert_eq!(shown.finish, ["CT +20 %", "VIG 0 EV", "CHR 0 %"]);
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
