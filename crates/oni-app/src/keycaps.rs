//! `Raster.keycap` for every window: the design system's `KeycapLook` drawn by oni-keycap, one cache
//! for the whole interface thread, so a still key is drawn once and a pressed one once a frame.

use std::cell::RefCell;

use oni_keycap::{Cache, Look, Stats};
use slint::{ComponentHandle, Global, Image, Rgba8Pixel, SharedPixelBuffer};

use crate::{KeycapLook, Raster};

/// The renders kept: a screen's keys hold a few megabytes, so a press's in-between depths fit many times.
const BUDGET: usize = 48 << 20;

thread_local! {
    static CACHE: RefCell<Cache<Image>> = RefCell::new(Cache::new(BUDGET));
}

/// The compiled `KeycapLook`, field by field.
pub fn look(k: &KeycapLook) -> Look {
    let c = k.albedo;
    Look {
        width: k.width as f64,
        height: k.height as f64,
        depth: k.depth as f64,
        albedo: [c.red(), c.green(), c.blue(), c.alpha()],
        radius: k.radius as f64,
        face_radius: k.face_radius as f64,
        factor: k.factor as f64,
        margin: k.margin as f64,
        scale: k.scale as f64,
        level: k.level,
        shadow: k.shadow,
        rim: k.rim,
        azimuth: k.azimuth as f64,
        elevation: k.elevation as f64,
        ambient: k.ambient as f64,
        diffuse: k.diffuse as f64,
        midtone: k.midtone as f64,
        wall_top: k.wall_top as f64,
        wall_bottom: k.wall_bottom as f64,
        wall_side: k.wall_side as f64,
        rise: k.rise as f64,
        dish: k.dish as f64,
        seam_blur: k.seam_blur as f64,
        socket_rest: k.socket_rest as f64,
        socket_pressed: k.socket_pressed as f64,
        socket_softness: k.socket_softness as f64,
        scale_pressed: k.scale_pressed as f64,
        travel: k.travel as f64,
        shadow_opacity: k.shadow_opacity as f64,
        shadow_blur: k.shadow_blur as f64,
        shadow_y: k.shadow_y as f64,
        bottom_lift: k.bottom_lift as f64,
    }
}

/// The key `k` describes, at its own scale (the window's, which the look carries); empty for no size.
pub fn draw(k: &KeycapLook, window_scale: f32) -> Image {
    CACHE.with(|cache| {
        cache.borrow_mut().get(&look(k), window_scale as f64, |drawn| {
            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&drawn.pixels, drawn.width, drawn.height);
            Image::from_rgba8_premultiplied(buffer)
        })
    })
    .unwrap_or_default()
}

/// Answers `Raster.keycap` on `window`, whose keys otherwise show their flat stand-in.
pub fn serve<C>(window: &C)
where
    C: ComponentHandle + 'static,
    for<'a> Raster<'a>: Global<'a, C>,
{
    let weak = window.as_weak();
    window.global::<Raster>().on_keycap(move |k| {
        let scale = weak.upgrade().map_or(1.0, |w| w.window().scale_factor());
        draw(&k, scale)
    });
}

/// What the keys cost on this thread so far.
pub fn stats() -> Stats {
    CACHE.with(|cache| cache.borrow().stats)
}
