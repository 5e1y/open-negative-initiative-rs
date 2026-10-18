//! The design system's guard: every `.slint` file outside `ui/ds/` is read for a value or a styled
//! primitive the design system owns, so a screen restyles only through a token or a component.

use std::path::{Path, PathBuf};

/// One refusal: where it is, which rule, and the offending text.
#[derive(Clone, Debug, PartialEq)]
pub struct Breach {
    pub file: PathBuf,
    pub line: usize,
    pub rule: Rule,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Length,
    Duration,
    Angle,
    Colour,
    Typography,
    Primitive,
    Styling,
    Opacity,
    Import,
}

/// Exceptions, each a file, the exact text it may hold, and why; empty while none is unavoidable.
pub const ALLOW: &[(&str, &str, &str)] = &[];

const LENGTH_UNITS: &[&str] = &["px", "phys-px", "rem", "pt", "cm", "mm", "in", "%"];
const DURATION_UNITS: &[&str] = &["ms", "s"];
const ANGLE_UNITS: &[&str] = &["deg", "rad", "turn", "grad"];
/// The elements the design system wraps: `Label`, `TextField`, `Icon`, `Picture`, `Menu`, `Scroller`.
const PRIMITIVES: &[&str] = &["Text", "TextInput", "Image", "PopupWindow", "Path", "Window", "Flickable", "ScrollView"];
const TYPOGRAPHY: &[&str] = &["font-size", "font-weight", "font-family", "font-italic", "letter-spacing", "default-font-family", "default-font-size", "default-font-weight"];
const STYLING: &[&str] = &[
    "background",
    "color",
    "colorize",
    "border-radius",
    "border-width",
    "border-color",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
    "drop-shadow-blur",
    "drop-shadow-color",
    "drop-shadow-offset-x",
    "drop-shadow-offset-y",
];
/// CSS's named colours, which Slint accepts as bare identifiers.
const NAMED: &[&str] = &[
    "aliceblue", "antiquewhite", "aqua", "aquamarine", "azure", "beige", "bisque", "black", "blanchedalmond", "blue",
    "blueviolet", "brown", "burlywood", "cadetblue", "chartreuse", "chocolate", "coral", "cornflowerblue", "cornsilk",
    "crimson", "cyan", "darkblue", "darkcyan", "darkgoldenrod", "darkgray", "darkgreen", "darkgrey", "darkkhaki",
    "darkmagenta", "darkolivegreen", "darkorange", "darkorchid", "darkred", "darksalmon", "darkseagreen",
    "darkslateblue", "darkslategray", "darkslategrey", "darkturquoise", "darkviolet", "deeppink", "deepskyblue",
    "dimgray", "dimgrey", "dodgerblue", "firebrick", "floralwhite", "forestgreen", "fuchsia", "gainsboro",
    "ghostwhite", "gold", "goldenrod", "gray", "green", "greenyellow", "grey", "honeydew", "hotpink", "indianred",
    "indigo", "ivory", "khaki", "lavender", "lavenderblush", "lawngreen", "lemonchiffon", "lightblue", "lightcoral",
    "lightcyan", "lightgoldenrodyellow", "lightgray", "lightgreen", "lightgrey", "lightpink", "lightsalmon",
    "lightseagreen", "lightskyblue", "lightslategray", "lightslategrey", "lightsteelblue", "lightyellow", "lime",
    "limegreen", "linen", "magenta", "maroon", "mediumaquamarine", "mediumblue", "mediumorchid", "mediumpurple",
    "mediumseagreen", "mediumslateblue", "mediumspringgreen", "mediumturquoise", "mediumvioletred", "midnightblue",
    "mintcream", "mistyrose", "moccasin", "navajowhite", "navy", "oldlace", "olive", "olivedrab", "orange",
    "orangered", "orchid", "palegoldenrod", "palegreen", "paleturquoise", "palevioletred", "papayawhip",
    "peachpuff", "peru", "pink", "plum", "powderblue", "purple", "rebeccapurple", "red", "rosybrown", "royalblue",
    "saddlebrown", "salmon", "sandybrown", "seagreen", "seashell", "sienna", "silver", "skyblue", "slateblue",
    "slategray", "slategrey", "snow", "springgreen", "steelblue", "tan", "teal", "thistle", "tomato", "transparent",
    "turquoise", "violet", "wheat", "white", "whitesmoke", "yellow", "yellowgreen",
];
const COLOUR_CALLS: &[&str] = &["rgb", "rgba", "hsl", "hsla", "@linear-gradient", "@radial-gradient", "@conic-gradient"];

fn ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// The source with comments and string contents blanked, so each byte keeps its line.
pub fn code_only(source: &str) -> String {
    let b: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if b[i] == '/' && b.get(i + 1) == Some(&'*') {
            while i < b.len() && !(b[i] == '*' && b.get(i + 1) == Some(&'/')) {
                out.push(if b[i] == '\n' { '\n' } else { ' ' });
                i += 1;
            }
            out.push_str("  ");
            i += 2;
        } else if b[i] == '"' {
            out.push('"');
            i += 1;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\\' {
                    out.push(' ');
                    i += 1;
                }
                if i < b.len() {
                    out.push(if b[i] == '\n' { '\n' } else { ' ' });
                    i += 1;
                }
            }
            out.push('"');
            i += 1;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

/// The breaches of one file's text, `path` naming it in the report.
pub fn scan_source(path: &Path, source: &str) -> Vec<Breach> {
    let code = code_only(source);
    let mut out = Vec::new();
    for (n, line) in code.lines().enumerate() {
        let mut found = |rule: Rule, text: &str| out.push(Breach { file: path.to_path_buf(), line: n + 1, rule, text: text.trim().to_owned() });
        let original = source.lines().nth(n).unwrap_or("");
        if original.contains("std-widgets.slint") {
            found(Rule::Import, original);
        }
        let c: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < c.len() {
            let before = if i == 0 { ' ' } else { c[i - 1] };
            if c[i] == '#' && c.get(i + 1).is_some_and(|d| d.is_ascii_hexdigit()) {
                let end = (i + 1..c.len()).find(|&j| !c[j].is_ascii_hexdigit()).unwrap_or(c.len());
                found(Rule::Colour, &c[i..end].iter().collect::<String>());
                i = end;
            } else if c[i].is_ascii_digit() && !ident_char(before) && before != '.' {
                let mut j = i;
                while j < c.len() && (c[j].is_ascii_digit() || c[j] == '.') {
                    j += 1;
                }
                let mut k = j;
                while k < c.len() && (c[k].is_ascii_alphabetic() || c[k] == '%' || (c[k] == '-' && c.get(k + 1).is_some_and(|x| x.is_ascii_alphabetic()) && k > j)) {
                    k += 1;
                }
                let number: String = c[i..j].iter().collect();
                let unit: String = c[j..k].iter().collect();
                let zero = number.parse::<f64>().is_ok_and(|v| v == 0.0);
                let literal = format!("{number}{unit}");
                if LENGTH_UNITS.contains(&unit.as_str()) && !zero {
                    found(Rule::Length, &literal);
                } else if DURATION_UNITS.contains(&unit.as_str()) && !zero {
                    found(Rule::Duration, &literal);
                } else if ANGLE_UNITS.contains(&unit.as_str()) {
                    found(Rule::Angle, &literal);
                }
                i = k.max(i + 1);
            } else if (c[i].is_ascii_alphabetic() || c[i] == '@') && !ident_char(before) && before != '.' {
                let end = (i + 1..c.len()).find(|&j| !ident_char(c[j])).unwrap_or(c.len());
                let word: String = c[i..end].iter().collect();
                let rest: String = c[end..].iter().collect();
                let next = rest.trim_start();
                let property = next.starts_with(':') && !next.starts_with(":=");
                if property && TYPOGRAPHY.contains(&word.as_str()) {
                    found(Rule::Typography, line);
                } else if property && STYLING.contains(&word.as_str()) {
                    found(Rule::Styling, line);
                } else if property && word == "opacity" {
                    let value = next[1..].trim_start();
                    let number: String = value.chars().take_while(|d| d.is_ascii_digit() || *d == '.').collect();
                    let tail = value[number.len()..].trim_start();
                    if !number.is_empty() && (tail.starts_with(';') || tail.is_empty()) && number.parse::<f64>().is_ok_and(|v| v != 0.0 && v != 1.0) {
                        found(Rule::Opacity, line);
                    }
                } else if PRIMITIVES.contains(&word.as_str()) && (next.starts_with('{') || line[..line.find(&word).unwrap_or(0)].trim_end().ends_with("inherits")) {
                    found(Rule::Primitive, line);
                } else if COLOUR_CALLS.contains(&word.as_str()) && next.starts_with('(') {
                    found(Rule::Colour, line);
                } else if word == "Colors" && next.starts_with('.') {
                    found(Rule::Colour, line);
                } else if NAMED.contains(&word.as_str()) && !next.starts_with(':') && !next.starts_with('(') && !next.starts_with('{') && !next.starts_with('.') {
                    found(Rule::Colour, &word);
                }
                i = end;
            } else {
                i += 1;
            }
        }
    }
    out
}

fn allowed(b: &Breach, ui: &Path) -> bool {
    let rel = b.file.strip_prefix(ui).unwrap_or(&b.file).to_string_lossy().replace('\\', "/");
    ALLOW.iter().any(|(file, text, _)| rel == *file && b.text.contains(text))
}

/// Every breach in the `.slint` files under `ui`, the design system's own folder `ds/` excepted.
pub fn scan(ui: &Path) -> Vec<Breach> {
    let mut files = Vec::new();
    let mut stack = vec![ui.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path != ui.join("ds") {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "slint") {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap_or_default();
        out.extend(scan_source(&file, &source).into_iter().filter(|b| !allowed(b, ui)));
    }
    out
}

/// The crate's own `ui` folder.
pub fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/ui")
}
