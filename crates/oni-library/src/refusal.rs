//! What a sidecar write that did not happen says, in the reader's language: the cause typed by the
//! sidecar, the photos named, so a refusal never reads as a success or a bare system error.

use oni_i18n::{tr, trn};
use oni_settings::sidecar::Error;

/// One refused write, its cause in words.
pub fn said(error: &Error) -> String {
    match error {
        Error::Unreachable => tr("The original cannot be found, its disk disconnected or the file moved. Nothing was changed.").into(),
        Error::Newer => tr("A newer version of ONI wrote these settings, and this one never rewrites them.").into(),
        other => other.to_string(),
    }
}

/// What a refusal of the disk means, in words a reader knows; the system's own message for the rest.
pub fn cause(error: &std::io::Error) -> String {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NotFound => tr("it is no longer there").into(),
        ErrorKind::PermissionDenied => tr("it cannot be written to").into(),
        ErrorKind::AlreadyExists => tr("an item of that name is already there").into(),
        ErrorKind::ReadOnlyFilesystem => tr("its disk is read-only").into(),
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => tr("the disk is full").into(),
        ErrorKind::InvalidFilename => tr("the disk refuses that name").into(),
        _ => error.to_string(),
    }
}

/// Why photos were left out because their originals are not on disk.
pub fn why_gone(count: usize) -> String {
    trn(
        "its original cannot be found, its disk disconnected or the file moved. Nothing was changed.",
        "their originals cannot be found, their disk disconnected or the files moved. Nothing was changed.",
        count as u64,
    )
}

/// The one sentence a gesture says for the photos whose originals it could not reach.
pub fn left_out(count: usize) -> String {
    let head = trn("One photo was left out", "{n} photos were left out", count as u64);
    tr!("{head}: {why}", head = head, why = why_gone(count))
}
