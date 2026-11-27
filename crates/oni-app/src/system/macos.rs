//! The system's calls on macOS: the file dialogs, the Finder, NSWorkspace and the Trash.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn double_click() -> Option<Duration> {
    let seconds = objc2_app_kit::NSEvent::doubleClickInterval();
    Duration::try_from_secs_f64(seconds).ok()
}

pub fn modifiers() -> Option<(bool, bool)> {
    use objc2_app_kit::NSEventModifierFlags as Flags;
    let f = objc2_app_kit::NSEvent::modifierFlags_class();
    Some((f.contains(Flags::Command) || f.contains(Flags::Control), f.contains(Flags::Shift)))
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
    let mut open = std::process::Command::new("/usr/bin/open");
    open.arg("-R").arg(path);
    let _ = super::launched(open);
}

pub fn reveal_all(paths: &[PathBuf]) {
    use objc2_foundation::{NSArray, NSString, NSURL};
    let urls: Vec<_> = paths.iter().map(|p| NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy()))).collect();
    objc2_app_kit::NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&urls));
}

pub fn open_url(url: &str) {
    let mut open = std::process::Command::new("/usr/bin/open");
    open.arg(url);
    let _ = super::launched(open);
}

pub fn show_folder(path: &Path) {
    let mut open = std::process::Command::new("/usr/bin/open");
    open.arg(path);
    let _ = super::launched(open);
}

pub fn trash(path: &Path) -> io::Result<()> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|e| io::Error::other(e.localizedDescription().to_string()))
}
