//! Frames moved whole or not at all: every file of a frame finds its place free, then moves; the
//! first refusal puts back what already moved. No move ever replaces a file.

use super::units::{Unit, steps};
use super::there;
use crate::gallery_history::Move;
use crate::names::Platform;
use crate::{paths, refusal};
use oni_i18n::tr;
use std::io;
use std::path::Path;
use std::time::Duration;

/// How a file moves, injectable so a check can refuse one halfway.
pub type Rename = dyn Fn(&Path, &Path) -> io::Result<()> + Sync;

/// What a carry did: the frames moved, and each frame that stayed with the cause.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Carried {
    pub made: Vec<Unit>,
    /// Each frame that stayed where it was, named by its first photo, with the cause in words.
    pub stayed: Vec<(String, String)>,
    /// Photos no longer where the record put them, skipped.
    pub absent: usize,
}

impl Carried {
    pub fn moves(&self) -> Vec<Move> {
        self.made.iter().flat_map(|u| u.moves.iter().cloned()).collect()
    }

    pub fn sizes(&self) -> Vec<usize> {
        self.made.iter().map(|u| u.moves.len()).collect()
    }
}

/// `from` to `to` unless something lies there already: never a replace, which `rename` does
/// silently on macOS and Windows alike. A file another app holds is tried again for a moment.
pub fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    retried(from, to, &rename_once, &std::thread::sleep, Platform::THIS)
}

fn rename_once(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let c = |p: &Path| std::ffi::CString::new(p.as_os_str().as_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput));
        let (f, t) = (c(from)?, c(to)?);
        // SAFETY: both are NUL-terminated paths that outlive the call.
        if unsafe { libc::renamex_np(f.as_ptr(), t.as_ptr(), libc::RENAME_EXCL) } == 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        // exFAT and network volumes refuse the flag itself: a look before the plain rename stands in.
        if !matches!(e.raw_os_error(), Some(libc::ENOTSUP) | Some(libc::EINVAL)) {
            return Err(e);
        }
    }
    if std::fs::symlink_metadata(to).is_ok() {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    std::fs::rename(from, to)
}

pub use oni_settings::atomic::PAUSES;

/// `rename` tried again after each pause while another app holds the file, or on Windows while a
/// file open inside a folder denies it; any other refusal, or the last one, is returned at once.
pub fn retried(from: &Path, to: &Path, rename: &dyn Fn(&Path, &Path) -> io::Result<()>, pause: &dyn Fn(Duration), platform: Platform) -> io::Result<()> {
    oni_settings::atomic::retried(from, to, rename, pause, platform == Platform::Windows)
}

/// Each unit moved whole: its photos still there, every place free, then file by file; a refusal
/// halfway moves the unit's files back. Photos gone from where the record put them are skipped.
pub fn carry(units: &[Unit], rename: &Rename) -> Carried {
    let mut out = Carried::default();
    for unit in units {
        let present: Vec<Move> = unit.moves.iter().filter(|m| there(&m.from)).cloned().collect();
        out.absent += unit.moves.len() - present.len();
        let Some(first) = present.first() else { continue };
        let label = paths::name(&first.from);
        let unit = Unit { moves: present };
        let steps = steps(&unit);
        if let Some((_, to)) = steps.iter().find(|(_, to)| there(to)) {
            out.stayed.push((label, tr!("“{name}” is already there", name = paths::name(to))));
            continue;
        }
        let mut done: Vec<&(String, String)> = Vec::new();
        let mut refused = None;
        for step in &steps {
            match rename(Path::new(&step.0), Path::new(&step.1)) {
                Ok(()) => done.push(step),
                Err(e) => {
                    refused = Some(tr!("{file}: {cause}", file = paths::name(&step.0), cause = cause(&e)));
                    break;
                }
            }
        }
        let Some(mut said) = refused else {
            out.made.push(unit);
            continue;
        };
        for (from, to) in done.into_iter().rev() {
            if let Err(e) = rename(Path::new(to), Path::new(from)) {
                said.push_str(&tr!("; {file} could not go back to “{folder}” ({cause})", file = paths::name(to), folder = paths::name(&paths::parent(from)), cause = cause(&e)));
            }
        }
        out.stayed.push((label, said));
    }
    out
}

fn cause(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::CrossesDevices => tr("it lies on another disk").into(),
        _ if refusal::denied_on_windows(e, Platform::THIS) => tr("another app is using it").into(),
        _ => refusal::cause(e),
    }
}

/// What a carry that left frames says: each frame named, at most three, then the count of the rest.
pub fn said(carried: &Carried) -> Option<String> {
    const NAMED: usize = 3;
    if carried.stayed.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = carried
        .stayed
        .iter()
        .take(NAMED)
        .map(|(frame, why)| tr!("“{frame}” stayed where it was: {why}.", frame = frame, why = why))
        .collect();
    if carried.stayed.len() > NAMED {
        lines.push(oni_i18n::trn("And one other frame.", "And {n} other frames.", (carried.stayed.len() - NAMED) as u64).into());
    }
    Some(lines.join("\n"))
}
