//! Core Graphics values as `Codable` writes them: a point `[x, y]`, a size `[w, h]`, a rectangle
//! `[[x, y], [w, h]]`, every component a `Double`.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// The leading `N` items of an array, as Core Graphics' decoders read them: extra items are ignored.
fn leading<'de, T: Deserialize<'de>, const N: usize, D: Deserializer<'de>>(d: D) -> Result<[T; N], D::Error> {
    let items = Vec::<T>::deserialize(d)?;
    let found = items.len();
    let head: Vec<T> = items.into_iter().take(N).collect();
    head.try_into().map_err(|_| de::Error::custom(format!("expected {N} elements, found {found}")))
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl Serialize for Point {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        [self.x, self.y].serialize(s)
    }
}

impl<'de> Deserialize<'de> for Point {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let [x, y] = leading::<f64, 2, D>(d)?;
        Ok(Self { x, y })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Serialize for Size {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        [self.width, self.height].serialize(s)
    }
}

impl<'de> Deserialize<'de> for Size {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let [width, height] = leading::<f64, 2, D>(d)?;
        Ok(Self { width, height })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const UNIT: Rect = Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 };

    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }
}

impl Serialize for Rect {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        [[self.x, self.y], [self.width, self.height]].serialize(s)
    }
}

impl<'de> Deserialize<'de> for Rect {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let [origin, size] = leading::<Point, 2, D>(d)?;
        Ok(Self { x: origin.x, y: origin.y, width: size.x, height: size.y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trailing_component_is_ignored_and_a_missing_one_refused() {
        let point: Point = serde_json::from_str("[0.25, 0.5, 9]").unwrap();
        assert_eq!(point, Point::new(0.25, 0.5));
        let rect: Rect = serde_json::from_str("[[0.1, 0.2], [0.3, 0.4, 7], [5, 6]]").unwrap();
        assert_eq!(rect, Rect::new(0.1, 0.2, 0.3, 0.4));
        // Twins: one component short is no point, and a size written as one number no rectangle.
        assert!(serde_json::from_str::<Point>("[0.25]").is_err());
        assert!(serde_json::from_str::<Rect>("[[0.1, 0.2], 0.3]").is_err());
    }
}
