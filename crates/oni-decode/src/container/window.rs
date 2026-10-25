//! A file read a window at a time, for the probe of a TIFF's directories.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// A file read a window at a time: a directory's fields and arrays cost one read, not one a field.
pub(super) struct Window {
    file: File,
    len: u64,
    reach: u64,
    start: u64,
    bytes: Vec<u8>,
    pub(super) fills: usize,
}

/// One read's reach: a camera's IFDs and their arrays sit in a few of them.
pub(super) const WINDOW: u64 = 64 << 10;

impl Window {
    pub(super) fn new(file: File, len: u64, reach: u64) -> Window {
        Window { file, len, reach, start: 0, bytes: Vec::new(), fills: 0 }
    }

    pub(super) fn take<const N: usize>(&mut self, i: u64) -> Option<[u8; N]> {
        let end = i.checked_add(N as u64)?;
        if end > self.len {
            return None;
        }
        if i < self.start || end > self.start + self.bytes.len() as u64 {
            // A quarter of the reach behind the field, as a directory's values may lie before it.
            self.start = i.saturating_sub(self.reach / 4);
            self.bytes.resize((self.reach.max(end - self.start)).min(self.len - self.start) as usize, 0);
            self.fills += 1;
            self.file.seek(SeekFrom::Start(self.start)).ok()?;
            if self.file.read_exact(&mut self.bytes).is_err() {
                self.bytes.clear();
                return None;
            }
        }
        let at = (i - self.start) as usize;
        self.bytes[at..at + N].try_into().ok()
    }
}
