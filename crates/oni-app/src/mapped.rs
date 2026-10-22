//! Blocks of 16 MB and more mapped and unmapped directly: macOS keeps freed large blocks dirty for reuse
//! (0.7 GB of decodes), while the Metal driver's smaller command-buffer blocks need that reuse.

use std::alloc::{GlobalAlloc, Layout, System};

const MAPPED: usize = 16 << 20;

pub struct Mapped;

fn mapped(l: Layout) -> bool {
    l.size() >= MAPPED && l.align() <= 16384
}

// SAFETY: a block is mapped exactly when its layout says so, and every layout comes back as it was asked.
unsafe impl GlobalAlloc for Mapped {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if !mapped(l) {
            return unsafe { System.alloc(l) };
        }
        let p = unsafe { libc::mmap(std::ptr::null_mut(), l.size(), libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANON, -1, 0) };
        if p == libc::MAP_FAILED { std::ptr::null_mut() } else { p.cast() }
    }

    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // A fresh anonymous mapping reads zero.
        if mapped(l) { unsafe { self.alloc(l) } } else { unsafe { System.alloc_zeroed(l) } }
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if mapped(l) {
            unsafe { libc::munmap(p.cast(), l.size()) };
        } else {
            unsafe { System.dealloc(p, l) };
        }
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, size: usize) -> *mut u8 {
        let to = unsafe { Layout::from_size_align_unchecked(size, l.align()) };
        if !mapped(l) && !mapped(to) {
            return unsafe { System.realloc(p, l, size) };
        }
        let q = unsafe { self.alloc(to) };
        if !q.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(p, q, l.size().min(size)) };
            unsafe { self.dealloc(p, l) };
        }
        q
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn byte(i: usize) -> u8 {
        (i * 31 % 251) as u8
    }

    /// The first index where the block's first `n` bytes leave the pattern.
    fn broken(p: *const u8, n: usize) -> Option<usize> {
        (0..n).find(|&i| unsafe { *p.add(i) } != byte(i))
    }

    /// A block filled with the pattern, grown past the mapped size, then shrunk under it.
    fn moved(spoil: bool) -> Option<usize> {
        let small = Layout::from_size_align(MAPPED / 2, 8).unwrap();
        unsafe {
            let p = Mapped.alloc(small);
            (0..small.size()).for_each(|i| *p.add(i) = byte(i));
            let grown = Mapped.realloc(p, small, MAPPED * 2);
            if spoil {
                *grown.add(small.size() / 3) ^= 1;
            }
            let shrunk = Mapped.realloc(grown, Layout::from_size_align(MAPPED * 2, 8).unwrap(), small.size());
            let found = broken(shrunk, small.size());
            Mapped.dealloc(shrunk, small);
            found
        }
    }

    #[test]
    fn a_block_keeps_its_bytes_across_the_mapped_size() {
        assert_eq!(moved(false), None);
    }

    #[test]
    fn a_spoiled_byte_is_seen() {
        assert!(moved(true).is_some());
    }

    #[test]
    fn a_large_zeroed_block_reads_zero() {
        let l = Layout::from_size_align(MAPPED, 8).unwrap();
        unsafe {
            let p = Mapped.alloc_zeroed(l);
            assert!((0..l.size()).step_by(4096).all(|i| *p.add(i) == 0));
            Mapped.dealloc(p, l);
        }
    }
}
