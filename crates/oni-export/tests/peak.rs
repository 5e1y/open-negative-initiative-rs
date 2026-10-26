//! The memory an export holds at its peak, counted by this binary's own allocator: one test alone
//! in its binary, since the count is the process's.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use oni_colour::PaperCorrection;
use oni_export::file::Ledger;
use oni_export::{ExifMetadata, Format, Image, Landing, Settings, write};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

const SIDE: (usize, usize) = (3000, 2000);
const FRAME: usize = SIDE.0 * SIDE.1 * 4 * 4;

/// Bytes held past the start at the export's peak, with `held` kept alive throughout.
fn peak_of_export(dir: &std::path::Path, held: Option<&Image>) -> usize {
    let source = dir.join("P1.NEF");
    std::fs::write(&source, b"raw").unwrap();
    let path = dir.join(format!("P1-{}.tiff", held.is_some()));
    let mut ledger = Ledger::in_support(&dir.join("Support"));
    let mut export = Settings::default();
    export.set_format(Format::Tiff16);
    let render = || Ok(Image { width: SIDE.0, height: SIDE.1, pixels: vec![0.4; SIDE.0 * SIDE.1 * 4] });
    let to = Landing { path: &path, ledger: &mut ledger, replacing: false };
    let start = LIVE.load(Ordering::Relaxed);
    PEAK.store(start, Ordering::Relaxed);
    let twin = held.map(|h| h.clone());
    write(&source, to, render, &PaperCorrection::default(), &export, &ExifMetadata::default(), None).unwrap();
    drop(twin);
    PEAK.load(Ordering::Relaxed) - start
}

#[test]
fn an_export_holds_one_float_frame_at_its_peak() {
    let dir = std::env::temp_dir().join(format!("oni-export-peak-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let peak = peak_of_export(&dir, None);
    println!("peak {:.0} MB for a {:.0} MB frame", peak as f64 / 1e6, FRAME as f64 / 1e6);
    // The frame, its 16-bit copy (half its size) and one band in conversion: under 1.75 frames.
    assert!(peak * 4 < FRAME * 7, "peak {peak} for a frame of {FRAME}");
    // Twin: a second float frame held through the export breaks the bound.
    let frame = Image { width: SIDE.0, height: SIDE.1, pixels: vec![0.4; SIDE.0 * SIDE.1 * 4] };
    let doubled = peak_of_export(&dir, Some(&frame));
    assert!(doubled * 4 >= FRAME * 7, "peak {doubled} with a second frame held");
    let _ = std::fs::remove_dir_all(&dir);
}
