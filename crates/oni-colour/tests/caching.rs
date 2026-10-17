//! A paper profile is read once per path: its name, and its parse. One test, alone in its
//! binary, since the counters are the process's own.

mod common;

use oni_colour::{OutputProfile, Space};

#[test]
fn a_paper_is_named_and_built_once() {
    let dir = common::sandbox("caching");
    let file = dir.join("Lustre.icc");
    common::write_stand_in(&file, "Lustre", 0.03, None, false);
    let paper = OutputProfile::new(file.to_string_lossy());
    let key = Space::Profile(paper.clone()).key();

    let before = OutputProfile::names_read();
    for _ in 0..50 {
        let _ = Space::from_key(&key);
    }
    assert_eq!(OutputProfile::names_read(), before, "reading the stored paper space fifty times names it no more");
    // Twin: a path never named before is opened once.
    let other = dir.join("Baryta.icc");
    common::write_stand_in(&other, "Baryta", 0.02, None, false);
    let _ = OutputProfile::new(other.to_string_lossy());
    assert_eq!(OutputProfile::names_read(), before + 1);

    let made = OutputProfile::spaces_made();
    for _ in 0..50 {
        let _ = paper.loaded();
    }
    let after = OutputProfile::spaces_made();
    assert!(after <= made + 1, "asking a paper's space fifty times builds it once ({} built)", after - made);
    // Twin: a new profile's space is built on its first ask.
    let _ = OutputProfile::new(other.to_string_lossy()).loaded();
    assert_eq!(OutputProfile::spaces_made(), after + 1);
}
