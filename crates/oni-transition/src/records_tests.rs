use super::*;
use crate::test_support::Scratch;

#[test]
fn the_install_instant_is_written_once() {
    let scratch = Scratch::new("install");
    let first = State::begin(scratch.path()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let again = State::begin(scratch.path()).unwrap();
    assert_eq!(first.installed_at, again.installed_at);
    // Twin: an unreadable state is an error, never replaced by a new instant.
    std::fs::write(State::path(scratch.path()), b"{ torn").unwrap();
    assert!(State::begin(scratch.path()).is_err());
    assert_eq!(std::fs::read(State::path(scratch.path())).unwrap(), b"{ torn");
}

fn line(sidecar: &str, outcome: Outcome) -> Entry {
    Entry { sidecar: sidecar.into(), outcome, reason: None, before: "b".into(), after: None, stops: None, backup: None, at: 1.0 }
}

#[test]
fn a_torn_last_line_loses_only_itself() {
    let scratch = Scratch::new("ledger");
    let mut ledger = Ledger::default();
    ledger.record(scratch.path(), line("/a.NEF.json", Outcome::Corrected)).unwrap();
    ledger.record(scratch.path(), line("/b.NEF.json", Outcome::Failed)).unwrap();
    let mut file = std::fs::OpenOptions::new().append(true).open(Ledger::path(scratch.path())).unwrap();
    file.write_all(b"{\"sidecar\":\"/c.NEF.json\",\"outc").unwrap();
    let read = Ledger::load(scratch.path());
    assert_eq!(read.entries.len(), 2);
    assert!(read.settled("/a.NEF.json"));
    // Twin: a failure is tried again, and a sidecar never seen is not settled.
    assert!(!read.settled("/b.NEF.json") && !read.settled("/c.NEF.json"));
}

#[test]
fn a_backup_is_kept_by_its_digest_and_mapped_to_its_sidecar() {
    let scratch = Scratch::new("backup");
    let bytes = b"{\"version\":19}";
    let a = back_up(scratch.path(), Path::new("/roll/a.NEF.json"), bytes).unwrap();
    let b = back_up(scratch.path(), Path::new("/roll/b.NEF.json"), bytes).unwrap();
    assert_eq!(a, b);
    assert_eq!(std::fs::read(backup_folder(scratch.path()).join(&a)).unwrap(), bytes);
    let lines = manifest(scratch.path());
    assert_eq!(lines.iter().map(|l| l.original.as_str()).collect::<Vec<_>>(), ["/roll/a.NEF.json", "/roll/b.NEF.json"]);
    // Twin: other bytes are another backup.
    assert_ne!(back_up(scratch.path(), Path::new("/roll/a.NEF.json"), b"{}").unwrap(), a);
    assert_eq!(digest(bytes), lines[0].digest);
}
