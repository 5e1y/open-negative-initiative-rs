/// A rectangle of whole pixels: `x`, `y` the top-left corner, rows running down as the buffers do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// The rectangle from `(x0, y0)` included to `(x1, y1)` excluded, empty when reversed.
    pub fn from_corners(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Self { x: x0, y: y0, w: (x1 - x0).max(0) as u32, h: (y1 - y0).max(0) as u32 }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w as i32
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h as i32
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub fn area(&self) -> u64 {
        self.w as u64 * self.h as u64
    }

    /// Grown by `dx` columns and `dy` rows on each side: a neighbourhood stage's margin.
    pub fn outset(&self, dx: i32, dy: i32) -> Self {
        Self::from_corners(self.x - dx, self.y - dy, self.right() + dx, self.bottom() + dy)
    }

    pub fn intersect(&self, other: &Rect) -> Self {
        Self::from_corners(
            self.x.max(other.x),
            self.y.max(other.y),
            self.right().min(other.right()),
            self.bottom().min(other.bottom()),
        )
    }

    /// The part of `extent` a clamped read of `self` loads: their intersection, else the edge rows
    /// or columns nearest to it.
    pub fn nearest_in(&self, extent: &Rect) -> Self {
        let x0 = self.x.clamp(extent.x, extent.right() - 1);
        let y0 = self.y.clamp(extent.y, extent.bottom() - 1);
        Self::from_corners(x0, y0, self.right().clamp(x0 + 1, extent.right()), self.bottom().clamp(y0 + 1, extent.bottom()))
    }

    /// The smallest rectangle holding both.
    pub fn union(&self, other: &Rect) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Self::from_corners(
            self.x.min(other.x),
            self.y.min(other.y),
            self.right().max(other.right()),
            self.bottom().max(other.bottom()),
        )
    }

    pub fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x && other.y >= self.y && other.right() <= self.right() && other.bottom() <= self.bottom()
    }
}
