//! A second launch on a support folder another process holds writes nothing there, and says so.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-instance-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("support")).unwrap();
    std::fs::create_dir_all(dir.join("caches")).unwrap();
    dir
}

/// The app as a hand would launch it, on the check's own folders, behind the other windows.
fn launch(root: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_oni"))
        .env("ONI_SUPPORT", root.join("support"))
        .env("ONI_CACHES", root.join("caches"))
        .env("ONI_INDEX", root.join("caches/index.sqlite"))
        .env("ONI_BACKGROUND", "1")
        .env("ONI_LANGUAGE", "en")
        .env_remove("ONI_RAW_DIR")
        .env_remove("ONI_BENCH")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// Every file under `dir` with its bytes and its modification date.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>, SystemTime)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path.clone());
                out.push((path, Vec::new(), SystemTime::UNIX_EPOCH));
            } else {
                let modified = entry.metadata().unwrap().modified().unwrap();
                out.push((path.clone(), std::fs::read(&path).unwrap(), modified));
            }
        }
    }
    out.sort();
    out
}

fn waited(child: &mut Child, until: impl Fn() -> bool, limit: Duration) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if until() {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    panic!("the launch neither ended nor went on within {limit:?}");
}

#[test]
fn a_second_instance_does_not_write() {
    let root = scratch("second");
    let support = root.join("support");
    std::fs::write(support.join("favourites.json"), br#"{"version":1,"paths":["\/Scans"]}"#).unwrap();
    std::fs::write(support.join("preferences.json"), br#"{"openedFolder": "/Scans"}"#).unwrap();
    std::fs::write(support.join("projects.json"), br#"{"projects":[],"version":1}"#).unwrap();
    let first = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(support.join("instance.lock")).unwrap();
    first.try_lock().unwrap();
    let (support_before, caches_before) = (snapshot(&support), snapshot(&root.join("caches")));
    let mut second = launch(&root);
    let status = waited(&mut second, || false, Duration::from_secs(60)).unwrap();
    let mut said = String::new();
    std::io::Read::read_to_string(&mut second.stderr.take().unwrap(), &mut said).unwrap();
    assert_eq!(status.code(), Some(2), "{said}");
    assert!(said.contains("already open"), "{said}");
    assert_eq!(snapshot(&support), support_before, "the support folder is as it was");
    assert_eq!(snapshot(&root.join("caches")), caches_before, "the caches are as they were");
    // Twin: once the first lets go, the same launch goes on and writes its log.
    drop(first);
    let log = support.join("Logs/oni.log");
    let mut alone = launch(&root);
    let ended = waited(&mut alone, || log.exists(), Duration::from_secs(60));
    let _ = alone.kill();
    let _ = alone.wait();
    assert!(ended.is_none() && log.exists(), "the launch alone went on: {ended:?}");
    let _ = std::fs::remove_dir_all(&root);
}
