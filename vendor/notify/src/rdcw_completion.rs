//! ONI: what a ReadDirectoryChangesW completion asks of its watch, apart from the Win32 calls so a
//! check on any system reads it.
#![allow(missing_docs)]

use crate::event::{Event, EventKind, Flag};
use std::path::PathBuf;

// The completion codes `handle_event` meets, from winerror.h.
pub const ERROR_SUCCESS: u32 = 0;
pub const ERROR_ACCESS_DENIED: u32 = 5;
pub const ERROR_OPERATION_ABORTED: u32 = 995;
/// The buffer overflowed: what changed is lost and the directory must be listed again.
pub const ERROR_NOTIFY_ENUM_DIR: u32 = 1022;

/// What one completion asks of the watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// The watch was removed or the watcher stopped.
    Stopped,
    /// The watched directory is gone: the watch ends.
    Gone,
    /// The buffer holds what changed: it is read, and the next read starts.
    Changes,
    /// What changed was lost, an overflow or nothing written: a rescan is told and the next read starts.
    Rescan,
    /// Any other error: the watch ends.
    Failed,
}

/// The completion's meaning; `exists` is asked only for an access denied.
pub fn completion(error_code: u32, bytes_written: u32, exists: impl FnOnce() -> bool) -> Completion {
    match error_code {
        ERROR_OPERATION_ABORTED => Completion::Stopped,
        ERROR_ACCESS_DENIED if !exists() => Completion::Gone,
        ERROR_SUCCESS | ERROR_ACCESS_DENIED if bytes_written == 0 => Completion::Rescan,
        ERROR_SUCCESS | ERROR_ACCESS_DENIED => Completion::Changes,
        ERROR_NOTIFY_ENUM_DIR => Completion::Rescan,
        _ => Completion::Failed,
    }
}

/// The event a lost burst tells: everything watched under `dir` is to be listed again.
pub fn rescan(dir: PathBuf) -> Event {
    Event::new(EventKind::Other).set_flag(Flag::Rescan).add_path(dir)
}
