//! The one home of the app's calls to the system: the double-click interval, Reduce Motion, the file
//! dialogs, the file manager, the Trash and the general clipboard.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Slint's and AppKit's default, where the system names none.
pub const DEFAULT_DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// `NSEvent.doubleClickInterval` on macOS, `GetDoubleClickTime` on Windows, as the person set it.
pub fn double_click() -> Duration {
    platform::double_click().filter(|d| !d.is_zero()).unwrap_or(DEFAULT_DOUBLE_CLICK)
}

mod about;
mod answers;
mod motion;
pub use about::described;
pub use motion::reduce_motion;

static VERIFIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The app's own window asks the system which keys are down; a check's synthetic clicks carry keys
/// no hand holds, so they are taken as sent.
pub fn verify_held_keys() {
    VERIFIED.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// A click's Command and Shift kept only while the system still holds them down: a release another
/// app took, a screenshot's shortcut's, never leaves the window believing the key held.
pub fn held(command: bool, shift: bool) -> (bool, bool) {
    let down = VERIFIED.load(std::sync::atomic::Ordering::Relaxed).then(platform::modifiers).flatten();
    kept((command, shift), down)
}

fn kept(asked: (bool, bool), down: Option<(bool, bool)>) -> (bool, bool) {
    down.map_or(asked, |(c, s)| (asked.0 && c, asked.1 && s))
}

/// `NSOpenPanel` for one folder, or the system's own dialog; `None` when the hand cancels or no
/// dialog exists on this system.
pub fn pick_folder(message: &str) -> Option<PathBuf> {
    match answers::folder() {
        Some(folder) => answers::taken(&folder),
        None => platform::pick_folder(message),
    }
}

/// `NSOpenPanel` for several files with one of `extensions` (bare, without the dot; empty takes any
/// file); empty when the hand cancels.
pub fn pick_files(message: &str, extensions: &[&str]) -> Vec<PathBuf> {
    match answers::folder() {
        Some(folder) => answers::taken(&folder).into_iter().collect(),
        None => platform::pick_files(message, extensions),
    }
}

/// `NSOpenPanel` for one file with one of `extensions`; `None` when the hand cancels.
pub fn pick_file(message: &str, extensions: &[&str]) -> Option<PathBuf> {
    match answers::folder() {
        Some(folder) => answers::taken(&folder),
        None => platform::pick_file(message, extensions),
    }
}

/// `NSSavePanel` opened on `suggested_name`, limited to `extensions`; `None` when the hand cancels.
pub fn save_file(message: &str, suggested_name: &str, extensions: &[&str]) -> Option<PathBuf> {
    match answers::folder() {
        Some(folder) => answers::taken(&folder).map(|answer| answers::saved(answer, suggested_name)),
        None => platform::save_file(message, suggested_name, extensions),
    }
}

/// The person's Downloads folder, where what the app hands them lands: an update's archive, the year's cards.
pub fn downloads() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default();
    home.join("Downloads")
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
    guarded(path, platform::trash)
}

/// NotFound before the platform is asked, which may answer otherwise for a missing file.
fn guarded(path: &Path, platform: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<()> {
    if path.exists() { platform(path) } else { Err(io::Error::new(io::ErrorKind::NotFound, path.display().to_string())) }
}

/// A helper program started and waited for on a thread of its own, so its end leaves no zombie.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn launched(mut command: std::process::Command) -> io::Result<u32> {
    let mut child = command.spawn()?;
    let id = child.id();
    std::thread::Builder::new().name("launched".into()).spawn(move || child.wait())?;
    Ok(id)
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

/// The folder every platform's support directory holds for this app.
pub const FOLDER: &str = "Open Negative Initiative";

/// The app's support folder: `ONI_SUPPORT`, else the platform's application-support directory.
pub fn support() -> PathBuf {
    if let Some(dir) = std::env::var_os("ONI_SUPPORT") {
        return PathBuf::from(dir);
    }
    let home = oni_library::navigator::home_folder;
    let base = if cfg!(target_os = "macos") {
        home().join("Library/Application Support")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home)
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
    };
    base.join(FOLDER)
}

/// `AppFolders.caches`, which the system may purge and the index rebuilds from; `ONI_CACHES` gives
/// a check its own.
pub fn caches() -> PathBuf {
    if let Some(dir) = std::env::var_os("ONI_CACHES") {
        return PathBuf::from(dir);
    }
    let home = oni_library::navigator::home_folder;
    let base = if cfg!(target_os = "macos") {
        home().join("Library/Caches")
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(home)
    } else {
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".cache"))
    };
    base.join(FOLDER)
}

/// Where the support folder's durable but rebuildable files go: on Windows the local application
/// data, as the roaming one syncs at every sign-in; elsewhere, and for a check's folder, `support`.
pub fn local_support(support: &Path) -> PathBuf {
    local_of(support, cfg!(windows), std::env::var_os("APPDATA"), std::env::var_os("LOCALAPPDATA"))
}

fn local_of(support: &Path, windows: bool, roaming: Option<std::ffi::OsString>, local: Option<std::ffi::OsString>) -> PathBuf {
    let folder = |base: std::ffi::OsString| PathBuf::from(base).join(FOLDER);
    match (roaming.map(folder), local) {
        (Some(roamed), Some(local)) if windows && roamed == support => folder(local),
        _ => support.to_path_buf(),
    }
}

#[cfg(target_os = "macos")]
#[path = "system/macos.rs"]
mod platform;

#[cfg(target_os = "windows")]
#[path = "system/windows.rs"]
mod platform;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[path = "system/other.rs"]
mod platform;

#[cfg(test)]
mod tests;
