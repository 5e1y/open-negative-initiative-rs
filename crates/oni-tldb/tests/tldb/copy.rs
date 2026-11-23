//! The Kit kept in the support folder: written and read back whole, and a typed name found in it.

use oni_tldb::{Kept, Kind, Read, fake};
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("oni-tldb-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

fn kept() -> Kept {
    let fake = fake::start();
    let Ok(Read::Fresh { kit, etag }) = fake.client().kit(None) else { panic!("no Kit") };
    Kept { account: kit.account.display_name, items: kit.items, etag, signed: Some(true) }
}

#[test]
fn a_kept_kit_reads_back_whole() {
    let support = scratch("roundtrip");
    assert_eq!(Kept::load(&support), Kept::default());
    let kept = kept();
    kept.save(&support).unwrap();
    assert_eq!(Kept::load(&support), kept);
    // Twin: a file that does not parse reads as nothing, never as the Kit.
    std::fs::write(Kept::file_path(&support), b"{ not json").unwrap();
    assert_eq!(Kept::load(&support), Kept::default());
    let _ = std::fs::remove_dir_all(&support);
}

#[test]
fn the_copy_says_signed_in_without_the_vault_and_an_older_copy_by_its_account() {
    let support = scratch("signed");
    let mut kept = kept();
    kept.save(&support).unwrap();
    assert!(Kept::load(&support).signed_in(), "a sign-in's mark read back");
    kept.signed = Some(false);
    kept.save(&support).unwrap();
    assert!(!Kept::load(&support).signed_in(), "twin: signed out by the service, the copy kept says out");
    // A copy written before the mark: its account stands for a token kept, no account for none.
    let older = r#"{"account":"Ada","items":[],"etag":null}"#;
    std::fs::write(Kept::file_path(&support), older).unwrap();
    assert!(Kept::load(&support).signed_in());
    std::fs::write(Kept::file_path(&support), r#"{"items":[]}"#).unwrap();
    assert!(!Kept::load(&support).signed_in(), "twin: no account read, no token presumed");
    let _ = std::fs::remove_dir_all(&support);
}

#[test]
fn a_typed_name_finds_its_piece_by_name_or_model() {
    let kept = kept();
    let ae1 = kept.named(Kind::Camera, "canon ae-1 program").expect("by name");
    assert_eq!(ae1.id, 1101);
    assert_eq!(kept.named(Kind::Camera, "  AE-1   PROGRAM ").map(|p| p.id), Some(1101));
    assert_eq!(kept.named(Kind::Lens, "FD 50mm F/1.4").map(|p| p.id), Some(2101));
    assert_eq!(kept.of(Kind::Lens).count(), 1);
    // Twin: a near name, another kind or nothing typed finds nothing.
    assert_eq!(kept.named(Kind::Camera, "Canon AE-1"), None);
    assert_eq!(kept.named(Kind::Lens, "Canon AE-1 Program"), None);
    assert_eq!(kept.named(Kind::Camera, "  "), None);
}
