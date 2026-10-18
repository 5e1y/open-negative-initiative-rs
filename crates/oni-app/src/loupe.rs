//! `Loupe`: the Details tab's band, the pipeline at the file's own scale over the source point the
//! canvas holds in the middle of its view, so a Detail radius is judged in source pixels.

use crate::viewport::Viewport;
use oni_graph::Rect;

/// Drawable pixels per source pixel at rest; above 1 the export's pixels are interpolated and grain reads at arm's length.
pub const REST: f64 = 1.5;
/// The badge's other ratio.
pub const CLOSE: f64 = 2.0;

/// A named ratio, not a zoom percentage: what is judged is a radius in source pixels.
pub fn label(magnification: f64) -> String {
    format!("{}%", (magnification * 100.0).round() as i64)
}

/// The badge's click: rest and close, one for the other.
pub fn toggled(magnification: f64) -> f64 {
    if magnification == REST { CLOSE } else { REST }
}

/// The source point at the view's centre; fitted, the frame's middle. The offset is y down, as the
/// canvas places the image, so reading it back subtracts on both axes.
pub fn centre(extent: Rect, viewport: &Viewport) -> (f64, f64) {
    let mid = (f64::from(extent.x) + f64::from(extent.w) / 2.0, f64::from(extent.y) + f64::from(extent.h) / 2.0);
    match viewport.scale {
        Some(scale) if scale > 0.0 => (mid.0 - viewport.offset.0 / scale, mid.1 - viewport.offset.1 / scale),
        _ => mid,
    }
}

/// Held inside the image and rounded onto the source grid, so one drawable pixel is one whole source pixel.
pub fn region(extent: Rect, viewport: &Viewport, drawable: (f64, f64), magnification: f64) -> Option<Rect> {
    if extent.w < 1 || extent.h < 1 || drawable.0 < 1.0 || drawable.1 < 1.0 || magnification <= 0.0 {
        return None;
    }
    let (ew, eh) = (f64::from(extent.w), f64::from(extent.h));
    let size = ((drawable.0 / magnification).floor().min(ew), (drawable.1 / magnification).floor().min(eh));
    if size.0 < 1.0 || size.1 < 1.0 {
        return None;
    }
    let looked = centre(extent, viewport);
    let (x0, y0) = (f64::from(extent.x), f64::from(extent.y));
    let x = (looked.0 - size.0 / 2.0).round().max(x0).min(x0 + ew - size.0);
    let y = (looked.1 - size.1 / 2.0).round().max(y0).min(y0 + eh - size.1);
    Some(Rect::new(x as i32, y as i32, size.0 as u32, size.1 as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: Rect = Rect::new(0, 0, 1200, 800);
    /// `DS.Control.panelWidth * 2` by `mainHistogramHeight * 2`, the band the Swift check lays.
    const DRAWABLE: (f64, f64) = (640.0, 360.0);

    /// Where the canvas draws a source point: the image centre at the view's centre plus the offset.
    fn placed(p: (f64, f64), v: &Viewport) -> (f64, f64) {
        let s = v.scale.unwrap_or(1.0);
        (DRAWABLE.0 / 2.0 + v.offset.0 + (p.0 - 600.0) * s, DRAWABLE.1 / 2.0 + v.offset.1 + (p.1 - 400.0) * s)
    }

    #[test]
    fn the_region_is_centred_on_what_the_canvas_holds_in_the_middle_and_stays_inside() {
        let port = Viewport { scale: Some(1.0), offset: (-137.0, 91.0) };
        let landed = placed(centre(FULL, &port), &port);
        let target = (DRAWABLE.0 / 2.0, DRAWABLE.1 / 2.0);
        assert!((landed.0 - target.0).hypot(landed.1 - target.1) < 0.5, "{landed:?}");
        // Twin: adding the vertical offset instead, the point lands off the view's middle.
        let flipped = placed((600.0 + 137.0, 400.0 + 91.0), &port);
        assert!((flipped.0 - target.0).hypot(flipped.1 - target.1) > 1.0);
        assert_eq!(centre(FULL, &Viewport::default()), (600.0, 400.0), "fitted: the frame's middle");

        let zone = region(FULL, &port, DRAWABLE, 1.0).unwrap();
        assert_eq!((zone.w, zone.h), (640, 360));
        assert_eq!((zone.x, zone.y), (417, 129), "rounded onto the source grid around (737, 309)");
        let rest = region(FULL, &port, DRAWABLE, REST).unwrap();
        assert_eq!((rest.w, rest.h), (426, 240), "a third fewer source pixels at rest");
        // Pushed into a corner, the region stays inside the frame instead of showing the ground.
        let corner = Viewport { scale: Some(4.0), offset: (40_000.0, -40_000.0) };
        let pushed = region(FULL, &corner, DRAWABLE, 1.0).unwrap();
        assert!(FULL.contains(&pushed) && (pushed.x, pushed.bottom()) == (0, 800), "{pushed:?}");
        // A band wider than the frame holds the whole frame, never more.
        let small = Rect::new(0, 0, 300, 200);
        assert_eq!(region(small, &port, DRAWABLE, 1.0), Some(small));
        assert_eq!(region(FULL, &port, (0.0, 360.0), 1.0), None, "an unlaid band asks for nothing");
    }

    #[test]
    fn the_badge_names_the_ratio_and_swaps_rest_for_close() {
        assert_eq!((label(REST), label(CLOSE)), ("150%".to_string(), "200%".to_string()));
        assert_eq!((toggled(REST), toggled(CLOSE)), (CLOSE, REST));
        assert_ne!(label(1.0), label(REST), "twin: the label reads the ratio it is given");
    }
}
