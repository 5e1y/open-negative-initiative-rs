//! A migration is never half-written: a refused rename, a process killed mid-write, a read-only folder
//! each leave the old file as it was. An older build meets a file of this format read-only.

use super::corpus::{corpus, facts, golden, laid, reading, scratch};
use super::{Outcome, Refusal, Standing, chain, file_through, file_with, standing};
use crate::date::Date;
use crate::document::{self, VERSION};
use crate::modes::Medal;
use crate::settings::PipelineSettings;
use crate::sidecar;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

const SOURCE: &str = "19-rust/040-P1073163.RW2.json";

/// The temporaries a write left in `folder`.
fn temporaries(folder: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(folder).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "tmp")).collect()
}

#[test]
fn a_refused_rename_leaves_the_file_byte_for_byte_and_the_next_pass_brings_it() {
    let dir = scratch("refused-rename");
    let path = corpus().join(SOURCE);
    let old = std::fs::read(&path).unwrap();
    let original = laid(&dir, &path, &old);
    let kept = facts(&path);
    let refused = kept.facts(|f| file_through(&original, f, &|_, _| Err(std::io::Error::other("refused"))));
    assert!(matches!(refused, Err(Refusal::Unwritable(_))), "{refused:?}");
    assert_eq!(std::fs::read(sidecar::url_for(&original)).unwrap(), old);
    assert!(temporaries(original.parent().unwrap()).is_empty(), "no temporary left");
    assert!(matches!(kept.facts(|f| file_with(&original, f)), Ok(Outcome::Migrated { from: 19 })));
    assert_eq!(reading(&original, kept.mosaic), golden(&path));
    let _ = std::fs::remove_dir_all(&dir);
}

/// What the child process does to the sidecar it is handed before it is killed.
const CHILD: &str = "ONI_MIGRATE_KILLED";

/// Run alone in a child process by the check below: killed once the temporary is written, or, as
/// the twin, once a writer in place has truncated the target and written half of it.
#[test]
fn killed_mid_write_child() {
    let Some(how) = std::env::var_os(CHILD) else { return };
    let how = how.to_string_lossy().into_owned();
    let (mode, original) = how.split_once(':').unwrap();
    let original = PathBuf::from(original);
    let kept = facts(&corpus().join(SOURCE));
    if mode == "in-place" {
        let text = std::fs::read(sidecar::url_for(&original)).unwrap();
        std::fs::write(sidecar::url_for(&original), &text[..text.len() / 2]).unwrap();
        std::process::abort();
    }
    let _ = kept.facts(|f| file_through(&original, f, &|_, _| std::process::abort()));
}

fn killed(mode: &str, original: &Path) {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "migrate::checks::killed_mid_write_child", "--test-threads", "1"])
        .env(CHILD, format!("{mode}:{}", original.display()))
        .status()
        .unwrap();
    assert!(!status.success(), "the child was killed");
}

#[test]
fn a_process_killed_once_the_temporary_is_written_leaves_the_old_file_and_the_next_run_brings_it() {
    let dir = scratch("killed");
    let path = corpus().join(SOURCE);
    let old = std::fs::read(&path).unwrap();
    let original = laid(&dir, &path, &old);
    killed("verified", &original);
    assert_eq!(std::fs::read(sidecar::url_for(&original)).unwrap(), old, "the old bytes, whole");
    assert_eq!(temporaries(original.parent().unwrap()).len(), 1, "the kill left its temporary");
    let kept = facts(&path);
    assert!(matches!(kept.facts(|f| file_with(&original, f)), Ok(Outcome::Migrated { .. })));
    assert!(temporaries(original.parent().unwrap()).is_empty(), "the next write took the temporary");
    assert_eq!(reading(&original, kept.mosaic), golden(&path));
    // Twin: a writer that truncates the target and then writes leaves half a file under the same kill.
    let twin = laid(&dir.join("twin"), &path, &old);
    killed("in-place", &twin);
    let half = std::fs::read(sidecar::url_for(&twin)).unwrap();
    assert!(half != old && serde_json::from_slice::<Value>(&half).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_read_only_folder_reads_its_photo_as_it_read_refuses_a_write_and_is_brought_once_writable() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("read-only");
    let path = corpus().join(SOURCE);
    let old = std::fs::read(&path).unwrap();
    let original = laid(&dir, &path, &old);
    let folder = original.parent().unwrap().to_path_buf();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    let kept = facts(&path);
    let refused = kept.facts(|f| file_with(&original, f));
    let loaded = sidecar::read(&original, kept.mosaic);
    let label = sidecar::set_label(Medal::Gold, &original, kept.mosaic);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(refused, Err(Refusal::Unwritable(_))), "{refused:?}");
    let loaded = loaded.unwrap();
    let settings = |text: &str| text.split_once("settings: ").map(|(_, s)| s.to_owned());
    assert_eq!(settings(&loaded.said(None)), settings(&golden(&path)), "its edits as they read");
    assert!(matches!(label, Err(sidecar::Error::Held(Refusal::Unwritable(_)))), "{label:?}");
    assert_eq!(std::fs::read(sidecar::url_for(&original)).unwrap(), old);
    // Twin: a reader falling back to a new photo would show the rest, not the edits.
    assert_ne!(loaded.settings, PipelineSettings::rest(crate::ConversionMode::Negative, kept.mosaic));
    // Writable again, a read still writes nothing; the next pass brings it.
    let _ = sidecar::read(&original, kept.mosaic).unwrap();
    assert_eq!(std::fs::read(sidecar::url_for(&original)).unwrap(), old, "a read writes nothing");
    assert!(matches!(kept.facts(|f| file_with(&original, f)), Ok(Outcome::Migrated { .. })));
    assert_eq!(reading(&original, kept.mosaic), golden(&path));
    assert_eq!(standing(&std::fs::read(sidecar::url_for(&original)).unwrap()), Standing::Current);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The EXIF line as builds 1.4.0 to 1.5.6 typed it: seven strings, each required.
#[derive(Deserialize)]
#[allow(dead_code)]
struct WholeLine {
    camera: String,
    lens: String,
    film: String,
    iso: String,
    date: String,
    artist: String,
    copyright: String,
}

/// The envelope as 1.5.6 typed it, over its resting document.
#[derive(Deserialize)]
#[allow(dead_code)]
struct Shipped {
    version: i64,
    #[serde(rename = "importedAt", default)]
    imported_at: Option<Date>,
    #[serde(default)]
    label: Option<Medal>,
    #[serde(default)]
    exif: Option<WholeLine>,
    #[serde(rename = "trashedAt", default)]
    trashed_at: Option<Date>,
    settings: PipelineSettings,
}

/// The reader of 1.5.6 (format 20): the document over its rest, typed, then its guard. Whether it
/// may write, or why it cannot read.
fn shipped_reader(bytes: &[u8]) -> Result<bool, String> {
    const SHIPPED: i64 = 20;
    let stored: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let completed = chain::merged(&stored, chain::neutral_document());
    let read: Shipped = serde_json::from_value(completed).map_err(|e| e.to_string())?;
    Ok(read.version <= SHIPPED)
}

#[test]
fn an_older_build_reads_a_file_of_this_format_read_only() {
    let dir = scratch("shipped");
    let path = corpus().join(SOURCE);
    let original = laid(&dir, &path, &std::fs::read(&path).unwrap());
    facts(&path).facts(|f| file_with(&original, f)).unwrap();
    let current = std::fs::read(sidecar::url_for(&original)).unwrap();
    let mut whole: Value = serde_json::from_slice(&current).unwrap();
    assert!(whole.get("exifLine").is_some() && whole.get("exif").is_none(), "a line set by hand, under its own key");
    // A line of two fields, one cleared: at this format's version an older build reads it, and its guard never writes it.
    whole["exifLine"] = serde_json::json!({"lens": "Color Skopar 28mm", "film": ""});
    whole["version"] = Value::from(VERSION.max(22));
    assert_eq!(shipped_reader(&serde_json::to_vec(&whole).unwrap()), Ok(false));
    // Twin: the same line under `exif` would make that reader call the whole sidecar unreadable.
    let line = whole.as_object_mut().unwrap().remove("exifLine").unwrap();
    whole["exif"] = line;
    assert!(shipped_reader(&serde_json::to_vec(&whole).unwrap()).unwrap_err().contains("missing field"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 1.5.6's own sidecar code, built from the commit it shipped from, run on `original`: what its read
/// says, then whether its label and its whole write land. Needs git and cargo, so it runs on demand.
fn shipped_build(scratch: &Path, original: &Path) -> String {
    const SHIPPED: &str = "eac39604";
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let crate_dir = scratch.join("crates/oni-settings");
    if !crate_dir.join("src").exists() {
        let archive = std::process::Command::new("git").current_dir(&root).args(["archive", "--format=tar", SHIPPED, "crates/oni-settings/src"]).output().unwrap();
        assert!(archive.status.success(), "git archive {SHIPPED}");
        std::fs::create_dir_all(scratch).unwrap();
        let mut untar = std::process::Command::new("tar").current_dir(scratch).args(["-xf", "-"]).stdin(std::process::Stdio::piped()).spawn().unwrap();
        std::io::Write::write_all(untar.stdin.as_mut().unwrap(), &archive.stdout).unwrap();
        assert!(untar.wait().unwrap().success());
        let manifest = r#"[package]
name = "oni-settings"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["float_roundtrip"] }
uuid = { version = "1", features = ["v4"] }
"#;
        std::fs::write(crate_dir.join("Cargo.toml"), manifest).unwrap();
        let reader = scratch.join("reader");
        std::fs::create_dir_all(reader.join("src")).unwrap();
        let reader_manifest = r#"[package]
name = "reader"
version = "0.1.0"
edition = "2024"

[workspace]

[dependencies]
oni-settings = { path = "../crates/oni-settings" }
"#;
        std::fs::write(reader.join("Cargo.toml"), reader_manifest).unwrap();
        std::fs::copy(root.join("Cargo.lock"), reader.join("Cargo.lock")).unwrap();
        let main = r#"use oni_settings::{Medal, PipelineSettings, sidecar};
fn main() {
    let original = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let before = std::fs::read(sidecar::url_for(&original)).unwrap();
    match sidecar::read(&original, true, false) {
        Ok(l) => println!("read: writable {}", l.writable),
        Err(e) => println!("read: refused ({e})"),
    }
    let label = sidecar::set_label(Medal::Gold, &original, true);
    let write = sidecar::write(&PipelineSettings::default(), &original, Default::default());
    println!("label: {}", if label.is_ok() { "written" } else { "refused" });
    println!("write: {}", if write.is_ok() { "written" } else { "refused" });
    println!("file: {}", if std::fs::read(sidecar::url_for(&original)).unwrap() == before { "unchanged" } else { "changed" });
}
"#;
        std::fs::write(reader.join("src/main.rs"), main).unwrap();
    }
    let run = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["run", "-q", "--offline", "--manifest-path"])
        .arg(scratch.join("reader/Cargo.toml"))
        .arg("--")
        .arg(original)
        .env("CARGO_TARGET_DIR", scratch.join("target"))
        .env_remove("RUSTFLAGS")
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    String::from_utf8(run.stdout).unwrap()
}

/// `cargo test -p oni-settings -- --ignored the_shipped_build`: 1.5.6's real reader and writers.
#[test]
#[ignore = "builds 1.5.6's sidecar code from git, about a minute"]
fn the_shipped_build_reads_a_file_of_this_format_read_only_and_writes_nothing() {
    let dir = scratch("shipped-build");
    let path = corpus().join(SOURCE);
    let original = laid(&dir.join("files"), &path, &std::fs::read(&path).unwrap());
    facts(&path).facts(|f| file_with(&original, f)).unwrap();
    let said = shipped_build(&dir.join("build"), &original);
    assert_eq!(said, "read: writable false\nlabel: refused\nwrite: refused\nfile: unchanged\n", "{said}");
    // Twin: the same build on a file of its own format reads it writable and writes it.
    let own = laid(&dir.join("own"), &path, &std::fs::read(corpus().join("20-rust/002-DSC_6438.NEF.json")).unwrap());
    let said = shipped_build(&dir.join("build"), &own);
    assert_eq!(said, "read: writable true\nlabel: written\nwrite: written\nfile: changed\n", "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_of_a_newer_format_is_left_as_it_is() {
    let dir = scratch("newer");
    let source = corpus().join(SOURCE);
    let original = laid(&dir, &source, &std::fs::read(&source).unwrap());
    facts(&source).facts(|f| file_with(&original, f)).unwrap();
    let mut newer: Value = serde_json::from_slice(&std::fs::read(sidecar::url_for(&original)).unwrap()).unwrap();
    newer["version"] = Value::from(VERSION + 1);
    newer["laterKey"] = Value::from(1);
    let bytes = serde_json::to_vec_pretty(&newer).unwrap();
    std::fs::write(sidecar::url_for(&original), &bytes).unwrap();
    assert_eq!(facts(&source).facts(|f| file_with(&original, f)), Err(Refusal::Newer(VERSION + 1)));
    let loaded = sidecar::read(&original, true).unwrap();
    assert!(!loaded.writable);
    assert!(matches!(sidecar::set_label(Medal::Gold, &original, true), Err(sidecar::Error::Newer)));
    assert_eq!(std::fs::read(sidecar::url_for(&original)).unwrap(), bytes, "untouched");
    // Twin: the same document at this format's version is written.
    newer["version"] = Value::from(VERSION);
    std::fs::write(sidecar::url_for(&original), serde_json::to_vec(&newer).unwrap()).unwrap();
    sidecar::set_label(Medal::Gold, &original, true).unwrap();
    assert_ne!(std::fs::read(sidecar::url_for(&original)).unwrap(), bytes);
    let _ = (document::FORMAT, std::fs::remove_dir_all(&dir));
}
