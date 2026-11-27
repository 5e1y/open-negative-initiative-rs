//! The system's calls where ONI ships no native code: no dialog, no file manager, no Trash.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn double_click() -> Option<Duration> {
    None
}

pub fn modifiers() -> Option<(bool, bool)> {
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
