//! `SidebarLayout` and `TreeEngineLayout`: the library column's photos as names, two thumbnails a
//! line or one, folders and rolls keeping their rows; and where the column scrolls to show a row.

use oni_i18n::tr;

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
            Layout::List => tr("Names only"),
            Layout::Pairs => tr("Two thumbnails a line"),
            Layout::Singles => tr("One thumbnail a line"),
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

/// `TreeEngineLayout.row(at:)`: the row whose frame holds the point; the gaps between thumbnails hold none.
pub fn row_at(x: f32, y: f32, frames: &[Frame]) -> Option<usize> {
    let first = frames.partition_point(|f| f.y + f.height <= y);
    let mut i = first;
    while i < frames.len() && frames[i].y <= y {
        let f = frames[i];
        if y < f.y + f.height && x >= f.x && x < f.x + f.width {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `PinnedRow.pinned`: the open row whose children pass under the column's top, held there, and
/// how far the row after its children pushes it up (zero or less); none while it is in view.
pub fn pinned(top: f32, frames: &[Frame], total: f32, depths: &[usize], open: &[bool], row: f32) -> Option<(usize, f32)> {
    let first = frames.partition_point(|f| f.y + f.height <= top);
    if first >= frames.len() {
        return None;
    }
    let mut depth = depths[first];
    for i in (0..=first).rev() {
        if open[i] && (depths[i] < depth || i == first) {
            if frames[i].y >= top {
                return None;
            }
            let end = (i + 1..depths.len()).find(|&e| depths[e] <= depths[i]).unwrap_or(depths.len());
            let next = frames.get(end).map_or(total, |f| f.y);
            return Some((i, (next - top - row).min(0.0)));
        }
        depth = depth.min(depths[i]);
    }
    None
}

/// `TreeDropTarget`: a drop onto a row, or between two, before the row named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropTarget {
    Join(usize),
    Before(usize),
}

/// `TreeDropTarget.resolve`: a name row reorders on its top and bottom strips, a thumbnail on its
/// left and right thirds, else the drop joins the row; past a thumbnail, only before the next of its run.
pub fn drop_target(x: f32, y: f32, frames: &[Frame], photos: &[bool], depths: &[usize], reorders: bool, strip: f32) -> Option<DropTarget> {
    let i = row_at(x, y, frames)?;
    if !reorders {
        return Some(DropTarget::Join(i));
    }
    let f = frames[i];
    let next = i + 1 < frames.len();
    if photos[i] {
        let within = x - f.x;
        if within < f.width / 3.0 {
            return Some(DropTarget::Before(i));
        }
        if within > f.width * 2.0 / 3.0 && next && photos[i + 1] && depths[i + 1] == depths[i] {
            return Some(DropTarget::Before(i + 1));
        }
        return Some(DropTarget::Join(i));
    }
    let within = y - f.y;
    if within < strip {
        return Some(DropTarget::Before(i));
    }
    if within > f.height - strip && next {
        return Some(DropTarget::Before(i + 1));
    }
    Some(DropTarget::Join(i))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Frame {
        Frame { x, y, width, height }
    }

    /// `TreeDropTarget.selfCheck` (`Checks/UI/ListPicking+Checks.swift:35`).
    #[test]
    fn a_drop_over_the_tree_lands_where_its_strips_say() {
        // A folder, two photos of one run, a deeper photo, then a last folder.
        let frames = [rect(0.0, 0.0, 200.0, 24.0), rect(0.0, 24.0, 90.0, 60.0), rect(90.0, 24.0, 90.0, 60.0), rect(0.0, 84.0, 90.0, 60.0), rect(0.0, 144.0, 200.0, 24.0)];
        let photos = [false, true, true, true, false];
        let depths = [0, 1, 1, 2, 0];
        let at = |x: f32, y: f32, d: &[usize], reorders: bool| drop_target(x, y, &frames, &photos, d, reorders, 6.0);
        use DropTarget::{Before, Join};
        assert_eq!((at(50.0, 2.0, &depths, true), at(50.0, 22.0, &depths, true), at(50.0, 12.0, &depths, true)), (Some(Before(0)), Some(Before(1)), Some(Join(0))));
        assert_eq!((at(10.0, 50.0, &depths, true), at(45.0, 50.0, &depths, true), at(80.0, 50.0, &depths, true)), (Some(Before(1)), Some(Join(1)), Some(Before(2))));
        assert_eq!(at(170.0, 50.0, &depths, true), Some(Join(2)), "past a photo whose next row is deeper, its right third joins it");
        assert_eq!((at(50.0, 166.0, &depths, true), at(50.0, 400.0, &depths, true)), (Some(Join(4)), None));
        assert_eq!((at(50.0, 2.0, &depths, false), at(10.0, 50.0, &depths, false)), (Some(Join(0)), Some(Join(1))), "a tree that does not reorder only joins");
        // Twin: the run rule dropped, the deeper photo is landed before.
        assert_eq!(at(170.0, 50.0, &[1; 5], true), Some(Before(3)));
    }

    /// `rowViewSelfCheck`'s reading and pinning (`Checks/UI/TreeEngine+Checks.swift:39-74`).
    #[test]
    fn a_row_is_read_off_its_frame_and_an_open_roll_holds_the_top_over_its_photos() {
        let (frames, total) = frames(&[false, true, true, true, false], &[0, 1, 1, 1, 0], 200.0, 2, M);
        let (h, top) = (M.row, M.small);
        assert_eq!((row_at(5.0, top, &frames), row_at(5.0, top + h - 0.01, &frames)), (Some(0), Some(0)));
        let pair = frames[1].y + frames[1].height / 2.0;
        let middle = |f: Frame| f.x + f.width / 2.0;
        assert_eq!((row_at(middle(frames[1]), pair, &frames), row_at(middle(frames[2]), pair, &frames)), (Some(1), Some(2)));
        assert_eq!((row_at(5.0, top - 0.01, &frames), row_at(5.0, total, &frames)), (None, None), "above the first row and below the last, nothing");
        // Twin: read down the column alone, the right photo goes to its left neighbour.
        assert_ne!(frames.iter().position(|f| f.y <= pair && pair < f.y + f.height), Some(2));

        let photo = [false, true, true, true, true, false, true];
        let depths = [0, 1, 1, 1, 1, 0, 1];
        let open = [true, false, false, false, false, true, false];
        let (laid, total) = super::frames(&photo, &depths, 200.0, 2, M);
        let mid = |f: Frame| f.y + f.height / 2.0;
        assert_eq!(pinned(mid(laid[2]), &laid, total, &depths, &open, h), Some((0, 0.0)), "the roll holds the top over its photos");
        assert_eq!(pinned(laid[0].y - 1.0, &laid, total, &depths, &open, h), None, "not before");
        let pushed = pinned(laid[5].y - h / 2.0, &laid, total, &depths, &open, h).expect("the roll still held");
        assert!(pushed.0 == 0 && pushed.1 < 0.0, "the next roll pushes it up: {pushed:?}");
        assert_eq!(pinned(mid(laid[6]), &laid, total, &depths, &open, h), Some((5, 0.0)), "the next roll takes its place");
        // Twin: the same rows with the roll closed pin nothing.
        let closed = [false, false, false, false, false, true, false];
        assert_eq!(pinned(mid(laid[2]), &laid, total, &depths, &closed, h), None);
    }

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
