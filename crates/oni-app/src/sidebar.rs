//! `SidebarLayout` and `TreeEngineLayout`: the library column's photos as names, two thumbnails a
//! line or one, folders and rolls keeping their rows; and where the column scrolls to show a row.

/// How the column shows its photos, stored per mode under `sidebarLayout.<mode>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    List,
    Pairs,
    Singles,
}

impl Layout {
    pub const ALL: [Layout; 3] = [Layout::List, Layout::Pairs, Layout::Singles];

    /// Photos a line; none draws them as names.
    pub fn columns(self) -> usize {
        match self {
            Layout::List => 0,
            Layout::Pairs => 2,
            Layout::Singles => 1,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Layout::List => "Names only",
            Layout::Pairs => "Two thumbnails a line",
            Layout::Singles => "One thumbnail a line",
        }
    }

    /// The stored word, `SidebarLayout.rawValue`.
    pub fn key(self) -> &'static str {
        match self {
            Layout::List => "list",
            Layout::Pairs => "pairs",
            Layout::Singles => "singles",
        }
    }

    pub fn of_key(key: &str) -> Option<Layout> {
        Layout::ALL.into_iter().find(|l| l.key() == key)
    }

    pub fn next(self) -> Layout {
        Layout::ALL[(Layout::ALL.iter().position(|l| *l == self).unwrap_or(0) + 1) % Layout::ALL.len()]
    }
}

/// The design system's lengths the layout reads, in points.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    /// `filmSidebarRowHeight`.
    pub row: f32,
    /// `Space.sm`, `Space.md`, `Space.xxs`.
    pub small: f32,
    pub medium: f32,
    pub tiny: f32,
    /// `captionHeight`.
    pub caption: f32,
}

/// A row's place in the column, in points from its top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// `TreeEngineLayout.photoRatio`.
const PHOTO_RATIO: f32 = 3.0 / 2.0;

/// `TreeEngineLayout.frames`: a photo shares its line with its neighbours at the same depth, up to
/// `columns`; any other row takes the width. The frames, and the column's whole height.
pub fn frames(photo: &[bool], depth: &[usize], width: f32, columns: usize, m: Metrics) -> (Vec<Frame>, f32) {
    let across = columns.max(1);
    let cell = ((width - 2.0 * m.medium - (across - 1) as f32 * m.small) / across as f32).max(1.0);
    let line = cell / PHOTO_RATIO + m.tiny + m.caption + m.small;
    let mut frames = vec![Frame { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }; photo.len()];
    let mut y = m.small;
    let mut i = 0;
    while i < photo.len() {
        if photo[i] && columns > 0 {
            let mut slot = 0;
            while slot < across && i < photo.len() && photo[i] && (slot == 0 || depth[i] == depth[i - 1]) {
                frames[i] = Frame { x: m.medium + slot as f32 * (cell + m.small), y, width: cell, height: line };
                slot += 1;
                i += 1;
            }
            y += line;
        } else {
            frames[i] = Frame { x: 0.0, y, width, height: m.row };
            y += m.row;
            i += 1;
        }
    }
    (frames, m.row.max(y + m.small))
}

/// `TreeEngine.reveal`: the scroll that centres `frame` in a viewport `height` tall, held inside the column.
pub fn centred(frame: Frame, height: f32, total: f32) -> f32 {
    (frame.y - height / 2.0 + frame.height / 2.0).clamp(0.0, (total - height).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: Metrics = Metrics { row: 32.0, small: 8.0, medium: 16.0, tiny: 2.0, caption: 14.0 };

    #[test]
    fn photos_pair_up_beside_their_folder_and_a_folder_takes_the_width() {
        // A folder, three photos inside it, a second folder.
        let photo = [false, true, true, true, false];
        let depth = [0, 1, 1, 1, 0];
        let (f, total) = frames(&photo, &depth, 228.0, 2, M);
        assert_eq!(f[0], Frame { x: 0.0, y: 8.0, width: 228.0, height: 32.0 });
        let cell = (228.0 - 32.0 - 8.0) / 2.0;
        let line = cell / 1.5 + 2.0 + 14.0 + 8.0;
        assert_eq!((f[1].y, f[2].y, f[2].x), (40.0, 40.0, 16.0 + cell + 8.0), "two a line");
        assert_eq!((f[3].y, f[3].x), (40.0 + line, 16.0), "the third starts the next line");
        assert_eq!(f[4].y, 40.0 + 2.0 * line);
        assert_eq!(total, 40.0 + 2.0 * line + 32.0 + 8.0);
        // Twin: names only lays every photo as a row.
        let (rows, _) = frames(&photo, &depth, 228.0, Layout::List.columns(), M);
        assert_eq!((rows[2].y, rows[2].height), (8.0 + 2.0 * 32.0, 32.0));
    }

    #[test]
    fn a_row_is_centred_and_the_scroll_stays_inside_the_column() {
        let row = Frame { x: 0.0, y: 500.0, width: 200.0, height: 32.0 };
        assert_eq!(centred(row, 300.0, 2000.0), 500.0 - 150.0 + 16.0);
        assert_eq!(centred(Frame { y: 20.0, ..row }, 300.0, 2000.0), 0.0, "the top holds");
        // Twin: a row near the end scrolls no further than the column's end.
        assert_eq!(centred(Frame { y: 1990.0, ..row }, 300.0, 2000.0), 1700.0);
        assert_eq!(Layout::Singles.next(), Layout::List);
        assert_eq!(Layout::of_key("pairs"), Some(Layout::Pairs));
    }
}
