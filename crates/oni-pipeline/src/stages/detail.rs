//! Detail in two stages, rows then columns, each reading its margin around the pixel.

use super::*;

shader!(
    DETAIL_ROWS,
    [wgsl::DETAIL_TAPS, wgsl::DETAIL_ROWS],
    r#"
fn detail_source(x: i32, y: i32) -> vec3<f32> { return load(vec2<i32>(x, y)).rgb; }
fn pixel(p: vec2<i32>) -> vec4<f32> { return detail_rows_px(p.x, p.y); }
"#
);
shader!(
    DETAIL_COLUMNS,
    [wgsl::DETAIL_TAPS, wgsl::DETAIL_COLUMNS],
    r#"
fn detail_row(x: i32, y: i32) -> vec4<f32> { return load1(vec2<i32>(x, y)); }
fn pixel(p: vec2<i32>) -> vec4<f32> { return detail_columns_px(load(p), p.x, p.y); }
"#
);

/// `detailRows`: the source widened by `disk` each side, read clamped to its edges.
pub struct DetailRows(pub Detail);

impl Stage for DetailRows {
    fn name(&self) -> &'static str {
        "detail-rows"
    }

    fn shader(&self) -> &'static str {
        DETAIL_ROWS.as_str()
    }

    fn params(&self) -> Vec<u8> {
        uniform(HEAD, &self.0.rows())
    }

    fn extent(&self, input: Rect) -> Rect {
        input.outset(self.0.disk as i32, 0)
    }

    fn input_region(&self, output: Rect) -> Rect {
        output.outset(self.0.reach[0].max(self.0.reach[1]) as i32, 0)
    }
}

/// `detailColumns`: the rows pass its auxiliary image, read `disk` across and `margin` down.
pub struct DetailColumns(pub Detail);

impl Stage for DetailColumns {
    fn name(&self) -> &'static str {
        "detail-columns"
    }

    fn shader(&self) -> &'static str {
        DETAIL_COLUMNS.as_str()
    }

    fn params(&self) -> Vec<u8> {
        uniform(HEAD, &self.0.rows())
    }

    fn aux_region(&self, _: usize, output: Rect) -> Rect {
        output.outset(self.0.disk as i32, self.0.margin() as i32)
    }
}
