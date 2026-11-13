//! The welcome's films, staged: scenes of the app's own blocks and components at their real size, played
//! by a timeline that moves the hand and answers for the app; the photographs shown as exported.

pub mod dedust;
pub mod interface;

use std::path::{Path, PathBuf};

use crate::photo::Rgba;

/// The welcome's stage in points: the films' size, at two pixels a point.
pub const SIZE: (f32, f32) = (oni_app::welcome::SIZE.0 as f32 / crate::stage::SCALE, oni_app::welcome::SIZE.1 as f32 / crate::stage::SCALE);

/// The welcome's scenes.
pub fn scenes() -> PathBuf {
    crate::film::scenes().join("welcome")
}

/// The interface film's photograph, which only `ONI_WELCOME_PHOTO` names: its folder is the owner's.
pub fn photo() -> Result<PathBuf, String> {
    let path = std::env::var_os("ONI_WELCOME_PHOTO").map(PathBuf::from).ok_or("set ONI_WELCOME_PHOTO to the film's photograph")?;
    if path.is_file() { Ok(path) } else { Err(format!("ONI_WELCOME_PHOTO names no file: {}", path.display())) }
}

/// Where a scene breaks the staging's rule: it holds layout and wiring only, so a `Path`, a colour, a
/// size not a token or an animation of its own each make one fault, named with its line.
pub fn faults(source: &str) -> Vec<String> {
    let code = bare(source);
    let mut faults = Vec::new();
    let mut fault = |at: usize, what: &str| faults.push(format!("line {}: {what}", code[..at].matches('\n').count() + 1));
    let bytes = code.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphabetic() || c == b'_' || c == b'@' {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-') {
                i += 1;
            }
            let word = &code[start..i];
            let next = code[i..].trim_start();
            match word {
                "Path" => fault(start, "a Path draws what a component should show"),
                "Colour" | "Colors" | "Palette" if next.starts_with('.') => fault(start, "a colour named in the scene"),
                "rgb" | "rgba" | "hsv" | "hsl" if next.starts_with('(') => fault(start, "a colour written in the scene"),
                "@linear-gradient" | "@radial-gradient" | "@conic-gradient" => fault(start, "a gradient written in the scene"),
                "background" | "color" | "fill" | "stroke" | "border-color" | "drop-shadow-color" if next.starts_with(':') => {
                    fault(start, "a colour set by the scene")
                }
                "animate" | "Timer" | "transitions" => fault(start, "an animation written in the scene"),
                _ => {}
            }
        } else if c == b'#' && bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit) {
            fault(i, "a colour written in the scene");
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let unit = code[i..].chars().take_while(|c| c.is_ascii_alphabetic() || *c == '%').collect::<String>();
            if ["px", "phx", "pt", "rem", "cm", "mm", "in", "%"].contains(&unit.as_str()) {
                fault(start, "a size that is no token");
            }
            i += unit.len();
        } else if c == b'<' && (code[i..].starts_with("<color>") || code[i..].starts_with("<brush>")) {
            // A token read out for the timeline is no colour of the scene's; one given to it is.
            let line = code[..i].rfind('\n').map_or(0, |n| n + 1);
            if !code[line..i].trim_start().starts_with("out property") {
                fault(i, "a colour carried by the scene");
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    faults
}

/// `source` with its comments blanked and its strings emptied, lines kept, so only code is read.
fn bare(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(c) = rest.chars().next() {
        if rest.starts_with("//") {
            let end = rest.find('\n').unwrap_or(rest.len());
            rest = &rest[end..];
        } else if rest.starts_with("/*") {
            let end = rest.find("*/").map_or(rest.len(), |e| e + 2);
            out.extend(rest[..end].chars().filter(|c| *c == '\n'));
            rest = &rest[end..];
        } else if c == '"' {
            let mut end = 1;
            let mut chars = rest[1..].chars();
            while let Some(c) = chars.next() {
                end += c.len_utf8();
                if c == '\\' {
                    end += chars.next().map_or(0, char::len_utf8);
                } else if c == '"' {
                    break;
                }
            }
            out.push_str("\"\"");
            rest = &rest[end.min(rest.len())..];
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// How long the edge takes to cross from the before to the after.
pub const WIPE: f32 = 0.9;

/// The least a hand rests where it rests, in frames: 0.45 s, so a key under it is seen to answer.
pub const REST: usize = 9;

/// The 1.5.6 before's frames, each rest of its hand held to `REST`, the wipe from its last frame to the
/// after's first across the columns right of `from` (the photograph is one in both), then the after.
pub fn joined(before: Vec<Rgba>, after: Vec<Rgba>, from: u32) -> Result<Vec<Rgba>, String> {
    let before = rested(before, REST);
    let (last, first) = (before.last().ok_or("no frame before")?, after.first().ok_or("no frame after")?);
    let wipe = crate::wipe::frames(last, first, from, WIPE)?;
    Ok(before.into_iter().chain(wipe).chain(after).collect())
}

/// `frames` with every still run between two moves held to `least` frames; the opening and closing
/// rests, which no move bounds on both sides, are kept as they are.
pub fn rested(frames: Vec<Rgba>, least: usize) -> Vec<Rgba> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        match runs.last_mut() {
            Some((start, len)) if frames[*start] == *f => *len += 1,
            _ => runs.push((i, 1)),
        }
    }
    let inner = 1..runs.len().saturating_sub(1);
    let mut out = Vec::with_capacity(frames.len());
    for (k, (start, len)) in runs.iter().enumerate() {
        // A run of one is a frame of a move; a run of two or more, the hand at rest.
        let held = if inner.contains(&k) && *len > 1 { (*len).max(least) } else { *len };
        out.extend(std::iter::repeat_n(frames[*start].clone(), held));
    }
    out
}

/// Every PNG of `folder`, in their names' order: a film's frames shot elsewhere.
pub fn frames(folder: &Path) -> Result<Vec<Rgba>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
        .map_err(|e| format!("{}: {e}", folder.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    files.sort();
    files.iter().map(|f| read_png(f)).collect()
}

/// An 8-bit PNG as RGBA.
pub fn read_png(file: &Path) -> Result<Rgba, String> {
    let named = |e: &dyn std::fmt::Display| format!("{}: {e}", file.display());
    let mut decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).map_err(|e| named(&e))?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(|e| named(&e))?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or_else(|| named(&"too large"))?];
    let info = reader.next_frame(&mut pixels).map_err(|e| named(&e))?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(named(&"not 8 bits a channel"));
    }
    pixels.truncate(info.buffer_size());
    Ok(Rgba { width: info.width, height: info.height, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: u8) -> Rgba {
        Rgba { width: 400, height: 3, pixels: vec![v; 400 * 3 * 4] }
    }

    #[test]
    fn the_before_runs_into_the_after_through_the_wipe() {
        let (before, after) = (vec![flat(10), flat(20)], vec![flat(200), flat(210)]);
        let all = joined(before, after, 0).unwrap();
        let wiped = (WIPE * crate::film::FPS as f32).round() as usize;
        assert_eq!(all.len(), 2 + wiped + 2);
        assert!(all[0] == flat(10) && all[1] == flat(20), "the before first, as shot");
        assert!(all[2] != flat(20) && all[2] != flat(200), "the edge has begun on the wipe's first frame");
        assert!(all[1 + wiped] == flat(200) && all[2 + wiped] == flat(200), "the wipe lands on the after's first");
        // Twin: a hard cut would have no frame mixing the two.
        assert!(all.iter().any(|f| f.pixels.contains(&20) && f.pixels.contains(&200)));
        assert!(joined(Vec::new(), vec![flat(1)], 0).is_err(), "no before, no join");
    }

    #[test]
    fn each_rest_of_the_hand_between_two_moves_is_held() {
        let f = |v: u8| flat(v);
        // At rest, a move, a short rest, a move, a rest to the end.
        let frames = vec![f(0), f(0), f(1), f(2), f(2), f(3), f(4), f(4)];
        let held = rested(frames.clone(), 5);
        assert_eq!(held.iter().filter(|x| **x == f(2)).count(), 5, "the short rest held");
        assert_eq!(held.iter().filter(|x| **x == f(0)).count(), 2, "the opening rest kept");
        assert_eq!(held.iter().filter(|x| **x == f(4)).count(), 2, "the closing rest kept");
        assert_eq!(held.len(), frames.len() + 3);
        // Twin: a frame of a move is never held.
        assert_eq!(held.iter().filter(|x| **x == f(1)).count(), 1);
    }
}
