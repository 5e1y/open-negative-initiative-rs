//! The print's black and white points reaching into each channel's window.

use super::*;

/// `Print.reaching`: each window's ends moved by its channel's held points' share of its span, held
/// inside `bounds` when given; the print bypassed or its points at rest leaves the windows as they are.
pub fn reaching(print: &Print, levels: &LevelsSet, bounds: Option<RangeInclusive<f32>>, mono: bool) -> LevelsSet {
    if print.bypassed || print.points_at_rest(mono) {
        return *levels;
    }
    let points = print.channel_points(mono);
    let reach = |w: Levels, (black, white): (f32, f32)| {
        let span = w.white - w.black;
        let mut out = Levels { black: w.black + black * span, white: w.white - white * span, ..w };
        if let Some(b) = &bounds {
            out.black = out.black.clamp(*b.start(), *b.end());
            out.white = out.white.clamp(*b.start(), *b.end());
        }
        out
    };
    LevelsSet { red: reach(levels.red, points[0]), green: reach(levels.green, points[1]), blue: reach(levels.blue, points[2]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_points_reach_into_each_window_and_a_bypassed_print_leaves_them() {
        let levels = LevelsSet::default();
        let print = Print { black_point: 0.1, white_point: 0.2, ..Print::default() };
        let reached = reaching(&print, &levels, None, false);
        assert!((reached.green.black - 0.3).abs() < 1e-6 && (reached.green.white - 2.4).abs() < 1e-6);
        let held = reaching(&print, &levels, Some(0.5..=2.0), false);
        assert_eq!((held.red.black, held.red.white), (0.5, 2.0));
        // Twin: the bypassed print reaches nothing.
        assert_eq!(reaching(&Print { bypassed: true, ..print }, &levels, None, false), levels);
    }

    #[test]
    fn a_channels_own_point_moves_its_window_alone() {
        let levels = LevelsSet::default();
        let global = Print { black_point: 0.05, white_point: 0.1, ..Print::default() };
        let before = reaching(&global, &levels, None, false);
        let red = Print { channel_white: [0.1, 0.0, 0.0], channel_black: [0.04, 0.0, 0.0], ..global };
        let after = reaching(&red, &levels, None, false);
        assert_eq!((after.green, after.blue), (before.green, before.blue));
        assert!(after.red.white < before.red.white - 0.1 && after.red.black > before.red.black + 0.05, "{:?} {:?}", after.red, before.red);
        // Twin: the offset laid on all three channels, as the global point would, moves green too.
        let all = Print { channel_white: [0.1; 3], channel_black: [0.04; 3], ..global };
        assert_ne!(reaching(&all, &levels, None, false).green, before.green);
        // Each window keeps the minimum span however far its own points go.
        let far = Print { black_point: 0.2, white_point: 1.0 / 3.0, channel_black: [0.0, 0.2, 0.0], channel_white: [0.0, 1.0 / 3.0, 0.0], ..Print::default() };
        let g = reaching(&far, &levels, None, false).green;
        assert!(g.white - g.black >= Print::MINIMUM_SPAN * (levels.green.white - levels.green.black) - 1e-5, "{g:?}");
    }

    #[test]
    fn at_rest_and_on_black_and_white_the_channels_points_leave_the_global_fold() {
        let levels = LevelsSet::default();
        let global = Print { black_point: 0.07, white_point: 0.125, ..Print::default() };
        // Zero channel points: the global fold to the bit, and no fold at all with the global pair at zero.
        let former = |w: Levels| {
            let (black, white) = global.held_points();
            let span = w.white - w.black;
            Levels { black: w.black + black * span, white: w.white - white * span, ..w }
        };
        let reached = reaching(&global, &levels, None, false);
        assert_eq!(reached, LevelsSet { red: former(levels.red), green: former(levels.green), blue: former(levels.blue) });
        assert_eq!(reaching(&Print::default(), &levels, None, false), levels);
        // Equal offsets on the three channels are the global point moved by as much, to the bit.
        let spread = Print { white_point: 0.0625, channel_white: [0.0625; 3], ..global };
        assert_eq!(reaching(&spread, &levels, None, false), reached);
        // Black and white reads the global pair alone; twin: colour reads red's own.
        let red = Print { channel_white: [0.2, 0.0, 0.0], ..global };
        assert_eq!(reaching(&red, &levels, None, true), reaching(&global, &levels, None, true));
        assert_ne!(reaching(&red, &levels, None, false), reached);
    }
}
