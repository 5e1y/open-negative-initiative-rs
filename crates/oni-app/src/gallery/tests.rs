use super::*;

#[test]
fn reveal_moves_the_least_to_show_a_cell_and_not_at_all_when_it_shows() {
    let l = Layout::new(&roll(60), 6, 1000.0, Metrics::GALLERY);
    let viewport = 400.0;
    assert_eq!(l.reveal(0, 0.0, viewport), None, "the first cell is in view at the top");
    let last = l.frame(59).unwrap();
    let down = l.reveal(59, 0.0, viewport).unwrap();
    assert!((down - (last.y + last.height + l.metrics.gutter - viewport)).abs() < 1e-3, "{down}");
    // Twin: from the bottom, the first cell scrolls back to its band's top, never above zero.
    let up = l.reveal(0, down, viewport).unwrap();
    assert!(up >= 0.0 && up <= l.frame(0).unwrap().y, "{up}");
    assert_eq!(l.reveal(59, down, viewport), None);
}

#[test]
fn a_narrow_grid_lays_fewer_columns_than_asked_and_no_tile_under_its_least() {
    // 500 pt, the grid of an 1100 pt window: twelve asked, six fit.
    let narrow = Layout::new(&roll(38), 12, 500.0, Metrics::GALLERY);
    assert_eq!(narrow.columns, 6);
    assert!(narrow.tile.0 >= Metrics::GALLERY.least, "{:?}", narrow.tile);
    // A wide grid takes the twelve asked, and a count under the fit is kept.
    assert_eq!(Layout::new(&roll(38), 12, 1600.0, Metrics::GALLERY).columns, 12);
    assert_eq!(Layout::new(&roll(38), 4, 500.0, Metrics::GALLERY).columns, 4);
    // Twin: with no least, the narrow grid's tiles fall to half the podium's width.
    let bare = Layout::new(&roll(38), 12, 500.0, Metrics { least: 0.0, ..Metrics::GALLERY });
    assert!(bare.columns == 12 && bare.tile.0 < Metrics::GALLERY.least / 1.5, "{:?}", bare.tile);
}

#[test]
fn a_landing_centres_its_cell_within_the_content_and_tops_without_one() {
    let l = Layout::new(&roll(120), 6, 1000.0, Metrics::GALLERY);
    let viewport = 400.0;
    let middle = l.frame(60).unwrap();
    let landed = l.landing(Some(60), viewport);
    assert!((landed + viewport / 2.0 - (middle.y + middle.height / 2.0)).abs() < 1e-3, "{landed}");
    assert_eq!(l.landing(Some(119), viewport), l.height - viewport, "the last row clamps to the end");
    assert_eq!(l.landing(Some(0), viewport), 0.0, "the first row clamps to the top");
    assert_eq!(l.landing(None, viewport), 0.0);
    // Twin: the reveal's shortest move leaves the middle cell at the viewport's edge, not centred.
    let revealed = l.reveal(60, 0.0, viewport).unwrap();
    assert!((revealed - landed).abs() > 1.0, "{revealed} {landed}");
}

#[test]
fn a_drop_lands_beside_the_nearer_half_and_past_a_band_s_last_cell_ends_that_band() {
    let groups = vec![("Roll 10".to_string(), String::new(), 3), ("Roll 8".to_string(), String::new(), 2)];
    let l = Layout::new(&groups, 6, 1000.0, Metrics::GALLERY);
    let c = l.frame(1).unwrap();
    assert_eq!((l.boundary(c.x + 1.0, c.y + 1.0), l.boundary(c.x + c.width - 1.0, c.y + 1.0)), (Some((1, false)), Some((1, true))));
    let end = l.frame(2).unwrap();
    assert_eq!(l.boundary(end.x + end.width - 1.0, end.y + 1.0), Some((2, true)), "past Roll 10's last cell");
    let line = l.boundary_line((2, true), 2.0).unwrap();
    assert_eq!((line.x, line.y), (end.x + end.width + 4.0 - 1.0, end.y), "the line in Roll 10's band, after its last cell");
    // Twin: the start of the next band draws its line in Roll 8's band, a band lower.
    let start = l.boundary_line((3, false), 2.0).unwrap();
    assert!(start.y > line.y && start.x < line.x, "{start:?} {line:?}");
    assert_eq!(l.boundary_line((1, false), 2.0).unwrap().x, c.x - 4.0 - 1.0);
}

#[test]
fn a_band_s_header_is_held_at_the_top_until_its_end_pushes_it_off() {
    let groups = vec![("A".to_string(), String::new(), 30), ("B".to_string(), String::new(), 30)];
    let l = Layout::new(&groups, 6, 1000.0, Metrics::GALLERY);
    let (a, b) = (&l.bands[0], &l.bands[1]);
    assert_eq!(l.pinned(0.0), None, "the first header shows in place at the top");
    assert_eq!(l.pinned(a.top + 100.0), Some((0, a.top + 100.0)), "held at the top edge");
    let pushed = a.bottom - 10.0;
    assert_eq!(l.pinned(pushed), Some((0, a.bottom - Metrics::GALLERY.header)), "the band's end pushes it up");
    // Twin: past the second band's top the second header is the one held, never the first.
    assert_eq!(l.pinned(b.top + 50.0), Some((1, b.top + 50.0)));
    assert_eq!(l.pinned(b.top), None, "the second header in place");
}

#[test]
fn the_slots_hold_the_cells_in_view_wherever_the_grid_scrolls() {
    let groups = vec![("A".to_string(), String::new(), 37), ("B".to_string(), String::new(), 5), ("C".to_string(), String::new(), 600)];
    let l = Layout::new(&groups, 6, 1000.0, Metrics::GALLERY);
    let viewport = 700.0;
    let slots = l.slots(viewport);
    let lens: std::collections::HashSet<usize> = (0..2000).map(|k| l.cells(k as f32 * 7.3, viewport).len()).collect();
    assert!(lens.iter().all(|n| *n <= slots), "{lens:?} within {slots}");
    // Twin: the cells in view vary as the grid scrolls, so a count taken from them would remake tiles.
    assert!(lens.len() > 1, "{lens:?}");
    assert!(slots < l.count / 4, "the slots stay near the view: {slots}");
}

fn roll(count: usize) -> Vec<(String, String, usize)> {
    vec![("Roll #8".into(), String::new(), count)]
}

#[test]
fn the_tile_follows_the_width_and_the_cells_sit_on_the_swift_grid() {
    let l = Layout::new(&roll(10), 6, 1000.0, Metrics::GALLERY);
    let tile = (1000.0 - 48.0 - 40.0) / 6.0;
    assert_eq!(l.tile, (tile, tile * 2.0 / 3.0));
    let second_row = l.frame(6).unwrap();
    assert_eq!((second_row.x, second_row.y), (24.0, 24.0 + 37.0 + 8.0 + l.row_pitch()));
    assert_eq!(l.height, 24.0 + 45.0 + 2.0 * l.cell.1 + 8.0 + 24.0);
    assert_eq!(l.index_at(second_row.x + 1.0, second_row.y + 1.0), Some(6));
    // Twin: a point past the last cell clamps to it rather than refusing.
    assert_eq!(l.index_at(990.0, second_row.y + 1.0), Some(9));
}

#[test]
fn a_click_reaches_a_cell_inside_its_frame_and_the_ground_reaches_none() {
    let l = Layout::new(&roll(10), 6, 1000.0, Metrics::GALLERY);
    let (first, second) = (l.frame(0).unwrap(), l.frame(1).unwrap());
    assert_eq!(l.cell_at(first.x + 1.0, first.y + 1.0), Some(0));
    let gutter = (first.x + first.width + (second.x - first.x - first.width) / 2.0, first.y + 1.0);
    let past = l.frame(9).unwrap();
    let empty = (past.x + past.width + 40.0, past.y + 1.0);
    let under = (first.x + 1.0, l.height + 4.0);
    for (x, y) in [gutter, empty, under] {
        assert_eq!(l.cell_at(x, y), None, "the ground at ({x}, {y})");
        // Twin: the nearest cell there is a cell, which a drop takes and a click must not.
        assert!(l.index_at(x, y).is_some());
    }
}

#[test]
fn a_scroll_asks_only_for_the_rows_in_view() {
    let l = Layout::new(&roll(600), 6, 1000.0, Metrics::GALLERY);
    let pitch = l.row_pitch();
    let top = 30.0 * pitch;
    let range = l.cells(top, 4.0 * pitch);
    assert!(range.len() <= 6 * 7, "{range:?} holds more than the view and its slack");
    assert!(range.contains(&(l.index_at(24.0, top + 1.0).unwrap())));
    // Twin: the whole roll is far more than what one view draws.
    assert!(l.count > 10 * range.len());
    assert_eq!(l.cells(0.0, 0.0), 0..0);
}
