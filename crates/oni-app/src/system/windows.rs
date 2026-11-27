//! The system's calls on Windows: the file dialogs, the Explorer and the Recycle Bin.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn double_click() -> Option<Duration> {
    // SAFETY: a plain read of a system setting, with no argument.
    let ms = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() };
    Some(Duration::from_millis(u64::from(ms)))
}

pub fn modifiers() -> Option<(bool, bool)> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT};
    // SAFETY: a plain read of a key's state, the high bit set while it is down.
    let down = |key: u16| unsafe { GetKeyState(i32::from(key)) } < 0;
    Some((down(VK_CONTROL), down(VK_SHIFT)))
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
    let mut explorer = std::process::Command::new("explorer");
    explorer.raw_arg(super::explorer_select(path));
    let _ = super::launched(explorer);
}

pub fn open_url(url: &str) {
    let mut explorer = std::process::Command::new("explorer");
    explorer.arg(url);
    let _ = super::launched(explorer);
}

pub fn show_folder(path: &Path) {
    let mut explorer = std::process::Command::new("explorer");
    explorer.arg(super::explorer_path(path));
    let _ = super::launched(explorer);
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
