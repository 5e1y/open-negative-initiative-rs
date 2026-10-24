//! The one home of the app's calls to the system: the double-click interval, the file dialogs, the
//! file manager, the Trash and the general clipboard.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Slint's and AppKit's default, where the system names none.
pub const DEFAULT_DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// `NSEvent.doubleClickInterval` on macOS, `GetDoubleClickTime` on Windows, as the person set it.
pub fn double_click() -> Duration {
    platform::double_click().filter(|d| !d.is_zero()).unwrap_or(DEFAULT_DOUBLE_CLICK)
}

/// `NSOpenPanel` for one folder, or the system's own dialog; `None` when the hand cancels or no
/// dialog exists on this system.
pub fn pick_folder(message: &str) -> Option<PathBuf> {
    platform::pick_folder(message)
}

/// `NSOpenPanel` for several files with one of `extensions` (bare, without the dot; empty takes any
/// file); empty when the hand cancels.
pub fn pick_files(message: &str, extensions: &[&str]) -> Vec<PathBuf> {
    platform::pick_files(message, extensions)
}

/// `NSOpenPanel` for one file with one of `extensions`; `None` when the hand cancels.
pub fn pick_file(message: &str, extensions: &[&str]) -> Option<PathBuf> {
    platform::pick_file(message, extensions)
}

/// `NSSavePanel` opened on `suggested_name`, limited to `extensions`; `None` when the hand cancels.
pub fn save_file(message: &str, suggested_name: &str, extensions: &[&str]) -> Option<PathBuf> {
    platform::save_file(message, suggested_name, extensions)
}

/// Opens the Finder or the Explorer with `path` selected, as Show in Finder does.
pub fn reveal(path: &Path) {
    platform::reveal(path);
}

/// `activateFileViewerSelecting`: one file viewer with every path of `paths` selected.
pub fn reveal_all(paths: &[PathBuf]) {
    platform::reveal_all(paths);
}

/// Opens a folder in the Finder or the Explorer, as `NSWorkspace.open` does.
pub fn show_folder(path: &Path) {
    platform::show_folder(path);
}

/// Explorer's spelling of a path: backslashes, which its command line needs to find a file, and no
/// `\\?\` prefix, which it does not read.
pub fn explorer_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    match text.strip_prefix(r"\\?\UNC\") {
        Some(share) => format!(r"\\{share}"),
        None => text.strip_prefix(r"\\?\").map_or(text.clone(), str::to_owned),
    }
}

/// The argument that has Explorer open `path`'s folder with `path` selected.
pub fn explorer_select(path: &Path) -> String {
    format!("/select,\"{}\"", explorer_path(path))
}

/// Opens a web page in the person's browser, as `NSWorkspace.open` does.
pub fn open_url(url: &str) {
    platform::open_url(url);
}

/// The system Trash, for files the app owns outright (preset files); a library member is never
/// trashed, it is marked in the ONI Trash.
pub fn trash(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, path.display().to_string()));
    }
    platform::trash(path)
}

/// A word in a window of the system's, said before the app's own window exists.
pub fn alert(title: &str, message: &str) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(title)
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (title, message);
}

/// Writes `text` to the general clipboard; a system without one ignores it.
pub fn set_text(text: &str) {
    if let Ok(mut board) = arboard::Clipboard::new() {
        let _ = board.set_text(text);
    }
}

/// The general clipboard's text, `None` when it holds none or the system has no clipboard.
pub fn get_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// Whether this system has a folder picker at all, so the app says so instead of doing nothing.
pub const HAS_PICKER: bool = cfg!(any(target_os = "macos", target_os = "windows"));

/// `AppFolders.caches`, which the system may purge and the index rebuilds from; `ONI_CACHES` gives
/// a check its own.
pub fn caches() -> PathBuf {
    if let Some(dir) = std::env::var_os("ONI_CACHES") {
        return PathBuf::from(dir);
    }
    let home = || std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let base = if cfg!(target_os = "macos") {
        home().join("Library/Caches")
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(home)
    } else {
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".cache"))
    };
    base.join(crate::timing::FOLDER)
}

/// Where the support folder's durable but rebuildable files go: on Windows the local application
/// data, as the roaming one syncs at every sign-in; elsewhere, and for a check's folder, `support`.
pub fn local_support(support: &Path) -> PathBuf {
    local_of(support, cfg!(windows), std::env::var_os("APPDATA"), std::env::var_os("LOCALAPPDATA"))
}

fn local_of(support: &Path, windows: bool, roaming: Option<std::ffi::OsString>, local: Option<std::ffi::OsString>) -> PathBuf {
    let folder = |base: std::ffi::OsString| PathBuf::from(base).join(crate::timing::FOLDER);
    match (roaming.map(folder), local) {
        (Some(roamed), Some(local)) if windows && roamed == support => folder(local),
        _ => support.to_path_buf(),
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::io;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    pub fn double_click() -> Option<Duration> {
        let seconds = objc2_app_kit::NSEvent::doubleClickInterval();
        Duration::try_from_secs_f64(seconds).ok()
    }

    pub fn pick_folder(message: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(message).pick_folder()
    }

    fn dialog(message: &str, extensions: &[&str]) -> rfd::FileDialog {
        let dialog = rfd::FileDialog::new().set_title(message);
        if extensions.is_empty() { dialog } else { dialog.add_filter("", extensions) }
    }

    pub fn pick_files(message: &str, extensions: &[&str]) -> Vec<PathBuf> {
        dialog(message, extensions).pick_files().unwrap_or_default()
    }

    pub fn pick_file(message: &str, extensions: &[&str]) -> Option<PathBuf> {
        dialog(message, extensions).pick_file()
    }

    pub fn save_file(message: &str, suggested_name: &str, extensions: &[&str]) -> Option<PathBuf> {
        dialog(message, extensions).set_file_name(suggested_name).save_file()
    }

    pub fn reveal(path: &Path) {
        let _ = std::process::Command::new("/usr/bin/open").arg("-R").arg(path).spawn();
    }

    pub fn reveal_all(paths: &[PathBuf]) {
        use objc2_foundation::{NSArray, NSString, NSURL};
        let urls: Vec<_> = paths.iter().map(|p| NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy()))).collect();
        objc2_app_kit::NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&urls));
    }

    pub fn open_url(url: &str) {
        let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    }

    pub fn show_folder(path: &Path) {
        let _ = std::process::Command::new("/usr/bin/open").arg(path).spawn();
    }

    pub fn trash(path: &Path) -> io::Result<()> {
        use objc2_foundation::{NSFileManager, NSString, NSURL};
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, None)
            .map_err(|e| io::Error::other(e.localizedDescription().to_string()))
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::io;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    pub fn double_click() -> Option<Duration> {
        // SAFETY: a plain read of a system setting, with no argument.
        let ms = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() };
        Some(Duration::from_millis(u64::from(ms)))
    }

    pub fn pick_folder(message: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(message).pick_folder()
    }

    fn dialog(message: &str, extensions: &[&str]) -> rfd::FileDialog {
        let dialog = rfd::FileDialog::new().set_title(message);
        if extensions.is_empty() { dialog } else { dialog.add_filter("", extensions) }
    }

    pub fn pick_files(message: &str, extensions: &[&str]) -> Vec<PathBuf> {
        dialog(message, extensions).pick_files().unwrap_or_default()
    }

    pub fn pick_file(message: &str, extensions: &[&str]) -> Option<PathBuf> {
        dialog(message, extensions).pick_file()
    }

    pub fn save_file(message: &str, suggested_name: &str, extensions: &[&str]) -> Option<PathBuf> {
        dialog(message, extensions).set_file_name(suggested_name).save_file()
    }

    /// Explorer selects one file a call: the first stands for the scope.
    pub fn reveal_all(paths: &[PathBuf]) {
        if let Some(first) = paths.first() {
            reveal(first);
        }
    }

    pub fn reveal(path: &Path) {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer").raw_arg(super::explorer_select(path)).spawn();
    }

    pub fn open_url(url: &str) {
        let _ = std::process::Command::new("explorer").arg(url).spawn();
    }

    pub fn show_folder(path: &Path) {
        let _ = std::process::Command::new("explorer").arg(super::explorer_path(path)).spawn();
    }

    pub fn trash(path: &Path) -> io::Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::*;
        // The source list ends with two NULs; `FOF_ALLOWUNDO` sends it to the Recycle Bin.
        let mut from: Vec<u16> = std::path::absolute(path)?.as_os_str().encode_wide().collect();
        from.extend([0, 0]);
        let mut op = SHFILEOPSTRUCTW {
            wFunc: FO_DELETE,
            pFrom: from.as_ptr(),
            fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as u16,
            ..Default::default()
        };
        // SAFETY: `op` and the list it points at live until the call returns.
        let code = unsafe { SHFileOperationW(&mut op) };
        if code == 0 && op.fAnyOperationsAborted == 0 {
            Ok(())
        } else {
            Err(io::Error::other(format!("SHFileOperationW failed with {code}")))
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use std::io;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    pub fn double_click() -> Option<Duration> {
        None
    }

    pub fn pick_folder(_message: &str) -> Option<PathBuf> {
        None
    }

    pub fn pick_files(_message: &str, _extensions: &[&str]) -> Vec<PathBuf> {
        Vec::new()
    }

    pub fn pick_file(_message: &str, _extensions: &[&str]) -> Option<PathBuf> {
        None
    }

    pub fn save_file(_message: &str, _suggested_name: &str, _extensions: &[&str]) -> Option<PathBuf> {
        None
    }

    pub fn reveal(_path: &Path) {}

    pub fn reveal_all(_paths: &[PathBuf]) {}

    pub fn open_url(_url: &str) {}

    pub fn show_folder(_path: &Path) {}

    pub fn trash(_path: &Path) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "no system Trash on this system"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_double_click_is_the_systems_and_never_zero() {
        let d = double_click();
        assert!(d > Duration::ZERO && d <= Duration::from_secs(5), "{d:?}");
        // Twin: a system answering zero would make every second press a double click; it falls back.
        assert_eq!(Some(Duration::ZERO).filter(|d| !d.is_zero()).unwrap_or(DEFAULT_DOUBLE_CLICK), DEFAULT_DOUBLE_CLICK);
    }

    #[test]
    fn a_missing_file_is_never_reported_trashed() {
        let gone = std::env::temp_dir().join(format!("oni-trash-{}-absent", std::process::id()));
        assert_eq!(trash(&gone).unwrap_err().kind(), io::ErrorKind::NotFound);
        // Twin: an existing file passes the guard and reaches the platform, which never says NotFound.
        let here = std::env::temp_dir().join(format!("oni-trash-{}-here", std::process::id()));
        std::fs::write(&here, b"x").unwrap();
        if cfg!(not(any(target_os = "macos", target_os = "windows"))) {
            assert_ne!(trash(&here).unwrap_err().kind(), io::ErrorKind::NotFound);
        }
        let _ = std::fs::remove_file(&here);
    }

    #[test]
    fn reveal_hands_explorer_backslashes() {
        let photo = Path::new("D:/Scans/Roll 8/DSC_0001.NEF");
        assert_eq!(explorer_select(photo), r#"/select,"D:\Scans\Roll 8\DSC_0001.NEF""#);
        assert_eq!(explorer_path(Path::new("//nas/film/Roll 8")), r"\\nas\film\Roll 8");
        assert_eq!(explorer_path(Path::new(r"\\?\C:\Scans")), r"C:\Scans");
        assert_eq!(explorer_path(Path::new(r"\\?\UNC\nas\film")), r"\\nas\film");
        // Twin: the path as the archive spells it, which Explorer answered with Documents.
        assert!(format!("/select,\"{}\"", photo.display()).contains('/'));
    }

    #[test]
    fn windows_keeps_rebuildable_files_off_the_roaming_profile() {
        let (roaming, local) = (Some("C:/Users/a/AppData/Roaming".into()), Some("C:/Users/a/AppData/Local".into()));
        let support = Path::new("C:/Users/a/AppData/Roaming").join(crate::timing::FOLDER);
        let chosen = local_of(&support, true, roaming.clone(), local.clone());
        assert_eq!(chosen, Path::new("C:/Users/a/AppData/Local").join(crate::timing::FOLDER));
        // Twin: a check's own support folder, and every other system, keep the support folder.
        let own = Path::new("C:/Temp/oni-check/Support");
        assert_eq!(local_of(own, true, roaming.clone(), local.clone()), own);
        assert_eq!(local_of(&support, false, roaming, local), support);
    }

    /// `AppFolders.selfCheck`: a check's folders are stand-ins named after its run, and the
    /// platform's own folders are named in the two routes alone.
    #[test]
    fn a_checks_folders_are_its_own_and_the_platforms_have_one_route() {
        let options = crate::app::Options::default();
        let run = std::process::id().to_string();
        let temp = std::env::temp_dir();
        for folder in [&options.support, &options.caches] {
            assert!(folder.starts_with(&temp) && folder.to_string_lossy().contains(&run), "{}", folder.display());
        }
        // Twin: the folder the app itself hands is not a stand-in.
        if std::env::var_os("ONI_CACHES").is_none() {
            assert!(!caches().starts_with(&temp), "{}", caches().display());
        }
        let platform = ["Library/Caches", "Library/Application Support", "LOCALAPPDATA", "\"APPDATA", "XDG_CACHE_HOME", "XDG_DATA_HOME"];
        let names = |text: &str| platform.iter().any(|p| text.contains(p));
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() { walk(&path, out) } else if path.extension().is_some_and(|e| e == "rs") { out.push(path) }
            }
        }
        let mut files = Vec::new();
        walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let offenders: Vec<String> = files
            .iter()
            .filter(|f| !["system.rs", "timing.rs"].iter().any(|r| f.ends_with(r)))
            .filter(|f| names(&std::fs::read_to_string(f).unwrap()))
            .map(|f| f.display().to_string())
            .collect();
        assert!(files.len() > 50 && offenders.is_empty(), "{offenders:?}");
        // Twin: a hand-built caches path is refused.
        assert!(names("home().join(\"Library/Caches\")"));
    }
}
