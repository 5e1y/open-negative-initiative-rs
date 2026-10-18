//! What every editor shares: the design system's lengths it hit-tests with, the plot's size, and the
//! SVG path data its lines are drawn from, in the plot's own points.

/// The `DSTokens.swift` lengths the editors measure with, held to `tokens.slint` by a check.
pub mod ds {
    pub const GRAB_SIZE: f64 = 24.0;
    pub const HANDLE_SIZE: f64 = 9.0;
    pub const HANDLE_THICKNESS: f64 = 24.0;
    pub const HANDLE_LENGTH: f64 = 34.0;
    pub const HANDLE_GROW: f64 = 1.45;
    pub const RADIUS_BADGE: f64 = 8.0;
    pub const SPACE_XS: f64 = 4.0;
    pub const SPACE_MD: f64 = 16.0;
    pub const STROKE_HAIRLINE: f64 = 0.5;
    pub const STROKE_LINE: f64 = 1.0;
    pub const STROKE_TRACE: f64 = 1.5;
    pub const DASH_LENGTH: f64 = 5.0;
    /// `DSHistogram`'s marker dash, written in the Swift source rather than a token.
    pub const MARKER_DASH: f64 = 3.0;

    /// Each constant beside the token it copies, for the check that holds them together.
    pub const TOKENS: &[(&str, f64)] = &[
        ("control-grab-size", GRAB_SIZE),
        ("control-handle-size", HANDLE_SIZE),
        ("control-handle-thickness", HANDLE_THICKNESS),
        ("control-handle-length", HANDLE_LENGTH),
        ("control-handle-grow", HANDLE_GROW),
        ("radius-badge", RADIUS_BADGE),
        ("space-xs", SPACE_XS),
        ("space-md", SPACE_MD),
        ("stroke-hairline", STROKE_HAIRLINE),
        ("stroke-line", STROKE_LINE),
        ("stroke-trace", STROKE_TRACE),
        ("control-dash-length", DASH_LENGTH),
    ];
}

/// A plot's size in points, as laid out.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

impl Size {
    pub const fn new(w: f64, h: f64) -> Size {
        Size { w, h }
    }
}

pub fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// One open polyline as path data.
pub fn polyline(points: &[(f64, f64)]) -> String {
    let mut out = String::with_capacity(points.len() * 16);
    for (i, (x, y)) in points.iter().enumerate() {
        out.push_str(if i == 0 { "M " } else { " L " });
        out.push_str(&format!("{x:.2} {y:.2}"));
    }
    out
}

/// Several straight segments as one path.
pub fn segments(lines: &[((f64, f64), (f64, f64))]) -> String {
    lines.iter().map(|(a, b)| polyline(&[*a, *b])).collect::<Vec<_>>().join(" ")
}

/// A polyline cut into dashes `on` long with gaps as long, measured along its length.
pub fn dashed(points: &[(f64, f64)], on: f64) -> String {
    let mut dashes: Vec<Vec<(f64, f64)>> = Vec::new();
    let (mut along, mut drawing) = (0.0, true);
    let mut current: Vec<(f64, f64)> = points.first().copied().into_iter().collect();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = distance(a, b);
        let mut from = 0.0;
        while length - from > 0.0 {
            let room = on - along;
            let step = room.min(length - from);
            from += step;
            along += step;
            let t = if length > 0.0 { from / length } else { 1.0 };
            let at = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            if drawing {
                current.push(at);
            }
            if along >= on {
                along = 0.0;
                if drawing {
                    dashes.push(std::mem::take(&mut current));
                } else {
                    current = vec![at];
                }
                drawing = !drawing;
            }
        }
    }
    if drawing && current.len() > 1 {
        dashes.push(current);
    }
    dashes.iter().map(|d| polyline(d)).collect::<Vec<_>>().join(" ")
}

/// A circle as path data, for rings drawn as lines.
pub fn circle(centre: (f64, f64), radius: f64) -> String {
    polyline(&ring(centre, radius, 96))
}

fn ring(centre: (f64, f64), radius: f64, steps: usize) -> Vec<(f64, f64)> {
    (0..=steps)
        .map(|i| {
            let a = i as f64 / steps as f64 * std::f64::consts::TAU;
            (centre.0 + radius * a.cos(), centre.1 + radius * a.sin())
        })
        .collect()
}

pub fn circle_points(centre: (f64, f64), radius: f64) -> Vec<(f64, f64)> {
    ring(centre, radius, 96)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_editors_lengths_are_the_design_system_s() {
        let tokens = include_str!("../../../oni-ui/ui/ds/tokens.slint");
        for (name, value) in ds::TOKENS {
            let line = tokens.lines().find(|l| l.contains(&format!(" {name}:"))).unwrap_or_else(|| panic!("{name} absent"));
            let written: f64 = line.split(':').nth(1).unwrap().trim().trim_end_matches(';').trim_end_matches("px").parse().unwrap();
            assert_eq!(written, *value, "{name}");
        }
        // Twin: a token the editors would read wrongly is caught.
        assert!(!tokens.contains(" control-grab-size: 14px;"));
    }

    #[test]
    fn a_dashed_line_alternates_its_dashes_and_gaps() {
        let d = dashed(&[(0.0, 0.0), (30.0, 0.0)], 5.0);
        assert_eq!(d.matches('M').count(), 3, "{d}");
        assert!(d.starts_with("M 0.00 0.00 L 5.00 0.00 M 10.00 0.00"), "{d}");
        // Twin: a solid line is one stroke.
        assert_eq!(polyline(&[(0.0, 0.0), (30.0, 0.0)]).matches('M').count(), 1);
    }
}

/// The inks a plot draws with; the design system maps each to its token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    Guide,
    GuideFaint,
    Marker,
    Separator,
    Active,
    Idle,
    Track,
    Primary,
    Canvas,
    Black,
    White,
    Red,
    Green,
    Blue,
    GreyQuarter,
    GreyHalf,
    GreyThreeQuarters,
    /// A spectrogram band's hue, by the band's index.
    Band(usize),
}

/// A stroked path in the plot's points.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub commands: String,
    pub ink: Ink,
    pub width: f64,
    pub opacity: f64,
}

impl Line {
    pub fn new(commands: String, ink: Ink, width: f64) -> Line {
        Line { commands, ink, width, opacity: 1.0 }
    }
}

/// A disc centred at `x`, `y`, filled and edged; `halo` is the diameter of a ring around it, or 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Dot {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub fill: Ink,
    pub edge: Ink,
    pub edge_width: f64,
    pub halo: f64,
}

impl Dot {
    pub fn filled(at: (f64, f64), size: f64, fill: Ink) -> Dot {
        Dot { x: at.0, y: at.1, size, fill, edge: fill, edge_width: 0.0, halo: 0.0 }
    }
}

/// What one plot shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Drawing {
    pub lines: Vec<Line>,
    pub dots: Vec<Dot>,
}
