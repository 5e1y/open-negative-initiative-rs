//! The print's black and white points reaching into each channel's window.

use super::*;

/// `Print.reaching`: each window's ends moved by the held points' share of its span, held inside
/// `bounds` when given; the print bypassed or at rest leaves the windows as they are.
pub fn reaching(print: &Print, levels: &LevelsSet, bounds: Option<RangeInclusive<f32>>) -> LevelsSet {
    if print.bypassed || (print.black_point == 0.0 && print.white_point == 0.0) {
        return *levels;
    }
    let (black, white) = print.held_points();
    let reach = |w: Levels| {
        let span = w.white - w.black;
        let mut out = Levels { black: w.black + black * span, white: w.white - white * span, ..w };
        if let Some(b) = &bounds {
            out.black = out.black.clamp(*b.start(), *b.end());
            out.white = out.white.clamp(*b.start(), *b.end());
        }
        out
    };
    LevelsSet { red: reach(levels.red), green: reach(levels.green), blue: reach(levels.blue) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_points_reach_into_each_window_and_a_bypassed_print_leaves_them() {
        let levels = LevelsSet::default();
        let print = Print { black_point: 0.1, white_point: 0.2, ..Print::default() };
        let reached = reaching(&print, &levels, None);
        assert!((reached.green.black - 0.3).abs() < 1e-6 && (reached.green.white - 2.4).abs() < 1e-6);
        let held = reaching(&print, &levels, Some(0.5..=2.0));
        assert_eq!((held.red.black, held.red.white), (0.5, 2.0));
        // Twin: the bypassed print reaches nothing.
        assert_eq!(reaching(&Print { bypassed: true, ..print }, &levels, None), levels);
    }
}
