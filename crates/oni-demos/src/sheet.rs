//! A contact sheet: a few frames of each clip side by side, one clip a row, for a look at a glance.

use std::path::Path;

use crate::photo::Rgba;

/// Frames a row shows, spread evenly over the clip.
const PER_ROW: usize = 6;
const GAP: u32 = 8;

pub fn write(clips: &[Vec<Rgba>], out: &Path) -> Result<(), String> {
    let first = clips.iter().find_map(|c| c.first()).ok_or("no frame to lay")?;
    let (w, h) = (first.width, first.height);
    let (width, height) = (PER_ROW as u32 * (w + GAP) + GAP, clips.len() as u32 * (h + GAP) + GAP);
    let mut sheet = vec![40u8; (width * height * 4) as usize];
    for (row, clip) in clips.iter().enumerate() {
        for k in 0..PER_ROW.min(clip.len()) {
            let frame = &clip[k * clip.len() / PER_ROW];
            let (x0, y0) = (GAP + k as u32 * (w + GAP), GAP + row as u32 * (h + GAP));
            for y in 0..h.min(frame.height) {
                let from = (y * frame.width * 4) as usize;
                let to = (((y0 + y) * width + x0) * 4) as usize;
                let n = (w.min(frame.width) * 4) as usize;
                sheet[to..to + n].copy_from_slice(&frame.pixels[from..from + n]);
            }
        }
    }
    let file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&sheet).map_err(|e| e.to_string())
}
