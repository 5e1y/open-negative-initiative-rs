//! The welcome's films, staged: scenes of the app's own blocks and components at their real size, played
//! by a timeline that moves the hand and answers for the app; the photographs shown as exported.

pub mod interface;

use std::path::{Path, PathBuf};

use crate::photo::Rgba;

/// The welcome's stage in points, `WelcomeStyle`'s, filmed at two pixels a point: 1120 × 1004.
pub const SIZE: (f32, f32) = (560.0, 502.0);

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
            fault(i, "a colour carried by the scene");
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

/// The 1.5.6 before's frames, then the film from the wipe's last frame on: where the wipe's frames join
/// them, the before's last frame meeting the after's first.
pub fn joined(before: Vec<Rgba>, after: Vec<Rgba>) -> Vec<Rgba> {
    before.into_iter().chain(after).collect()
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
