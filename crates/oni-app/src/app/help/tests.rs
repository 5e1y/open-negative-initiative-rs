//! What's New's notes, the editions, and the console's tail of the log.

use super::*;

#[test]
fn the_notes_are_swifts_and_each_names_its_heading_once() {
    let ids: Vec<&str> = NOTES.iter().map(|(i, _, _)| *i).collect();
    assert_eq!(ids, ["lighting.1.3.1", "space.1.3.1"]);
    assert!(NOTES.iter().all(|(_, w, _)| *w == "New" || *w == "Updated"));
    let mut unique = ids.clone();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());
    assert_eq!((SEEN_KEY, WELCOME_KEY, EDITION), ("whatsNewSeen", "welcomeSeen", "2.0"));
}

#[test]
fn the_menu_and_the_heading_name_the_editions_release() {
    assert_eq!(release(EDITION), EDITION);
    assert_eq!(release("1.3"), "1.3");
    assert_eq!(release("2.0.4.1"), "2.0");
    // Twin: a patch is not what both name.
    assert_ne!(release("1.5.1"), "1.5.1");
    // The sheet names the app's own release, so a new release brings its own edition.
    assert_eq!(EDITION, release(env!("CARGO_PKG_VERSION")));
}

#[test]
fn the_welcome_shows_once_a_release_whatever_patch_wrote_it_seen() {
    assert!(welcome_due(None));
    assert!(!welcome_due(Some(EDITION)));
    let patch = format!("{EDITION}.1");
    assert!(!welcome_due(Some(&patch)), "a patch of this release already showed it");
    // Twins: an earlier release's sheet, or the one 1.6.0 wrote, leaves this one due.
    assert!(welcome_due(Some("1.4")));
    assert!(welcome_due(Some("1.6.0")));
}

#[test]
fn the_pause_is_said_on_its_days_alone() {
    for day in [(2026, 12, 15), (2026, 12, 31), (2027, 1, 1), (2027, 1, 15)] {
        assert!(paused(day), "{day:?} lies in the pause");
    }
    // Twins: the days just outside it, and the same days a year later, say nothing.
    for day in [(2026, 12, 14), (2027, 1, 16), (2027, 12, 20), (2026, 1, 10)] {
        assert!(!paused(day), "{day:?} lies outside the pause");
    }
}

#[test]
fn the_message_names_no_inbox_and_its_links_are_the_sites() {
    assert!(DONATE.starts_with("https://ko-fi.com/"));
    for link in [INSTAGRAM, CONTEST] {
        assert!(link.starts_with("https://") && !link.contains('@'), "{link}: a page, never an address");
    }
    assert!([CONTEST_TAG, MADE_WITH].iter().all(|tag| tag.starts_with('#') && !tag.contains(' ')));
}

/// `WhatsNew.selfCheck`: every note hangs on a heading the interface draws, and every heading's
/// id has its note, or a note is never seen and a heading draws nothing.
#[test]
fn every_note_hangs_on_a_heading_and_every_heading_has_its_note() {
    fn walk(dir: &std::path::Path, hung: &mut std::collections::BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, hung);
            } else if path.extension().is_some_and(|e| e == "slint") {
                let text = std::fs::read_to_string(&path).unwrap();
                hung.extend(text.split("News.read(\"").skip(1).filter_map(|rest| rest.split('"').next().map(String::from)));
            }
        }
    }
    let mut hung = std::collections::BTreeSet::new();
    walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/ui"), &mut hung);
    let noted: std::collections::BTreeSet<String> = NOTES.iter().map(|(i, _, _)| (*i).to_string()).collect();
    assert!(!hung.is_empty());
    assert_eq!(hung, noted, "each note on a heading, each heading's id noted");
    // Twin: a note named nowhere is caught.
    let mut ghost = noted.clone();
    ghost.insert("ghost.1.3.1".into());
    assert_eq!(ghost.difference(&hung).collect::<Vec<_>>(), ["ghost.1.3.1"]);
}

#[test]
fn the_console_reads_only_what_came_and_keeps_the_last_lines() {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("oni-console-tail-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("oni.log");
    let mut file = File::create(&log).unwrap();
    for i in 0..CONSOLE_LINES + 50 {
        writeln!(file, "line {i}").unwrap();
    }
    write!(file, "half").unwrap();
    let mut tail = Tail::default();
    assert!(tail.follow(&log));
    let text = tail.text();
    assert_eq!(text.lines().count(), CONSOLE_LINES);
    assert!(text.starts_with("line 50\n") && text.ends_with(&format!("line {}", CONSOLE_LINES + 49)), "a line half written waits");
    assert!(!tail.follow(&log), "nothing new, nothing to lay out again");
    writeln!(file, " done").unwrap();
    assert!(tail.follow(&log));
    assert!(tail.text().ends_with("\nhalf done"));
    // Twin: a log started again from nothing is read from its start, not from the old offset.
    std::fs::write(&log, "fresh\n").unwrap();
    assert!(tail.follow(&log));
    assert_eq!(tail.text(), "fresh");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_first_read_of_a_long_log_starts_near_its_end_on_a_whole_line() {
    let dir = std::env::temp_dir().join(format!("oni-console-long-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("oni.log");
    let line = format!("{}\n", "x".repeat(99));
    std::fs::write(&log, line.repeat((3 * FIRST_READ / 100) as usize)).unwrap();
    let mut tail = Tail::default();
    assert!(tail.follow(&log));
    assert_eq!(tail.lines.len(), CONSOLE_LINES);
    assert!(tail.lines.iter().all(|l| l.len() == 99), "no line cut at the read's start");
    // Twin: the read started inside a line, whose cut end alone would be 75 characters.
    let start = std::fs::metadata(&log).unwrap().len() - FIRST_READ;
    assert_eq!(100 - start % 100, 76);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_gifts_opened_are_kept_for_this_edition_alone() {
    use super::welcome::progress;
    assert_eq!(progress(Some(&format!("{EDITION}:5"))), 0b0101, "the first and third gifts opened");
    assert_eq!(progress(None), 0, "nothing opened on a first launch");
    // Twin: another edition's gifts, or a value no build wrote, open nothing here.
    assert_eq!(progress(Some("1.9:15")), 0);
    assert_eq!(progress(Some(&format!("{EDITION}:x"))), 0);
}
