//! The model's own checks, each with a twin that breaks it.

use crate::light::{Light, Wall, dish_normal};
use crate::style::{KeycapStyle, Variant};
use crate::{Frame, render, render_in};

#[test]
fn the_four_normals_are_unit_and_outward() {
    let style = KeycapStyle::default();
    for wall in Wall::ALL {
        let n = wall.normal(wall.run(&style), style.rise);
        assert!(((n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt() - 1.0).abs() < 1e-9);
        assert!(n.2 > 0.0, "{wall:?} faces away from the viewer");
    }
    let (top, bottom) = (Wall::Top.normal(8.0, 6.0), Wall::Bottom.normal(8.0, 6.0));
    assert!(top.1 < 0.0 && bottom.1 > 0.0);
}

#[test]
fn the_dish_lights_the_bottom_more_than_the_top() {
    let light = Light::new(&KeycapStyle::default());
    let (top, bottom) = (light.shade(dish_normal(0.0, 4.0, 40.0)), light.shade(dish_normal(1.0, 4.0, 40.0)));
    assert!(bottom > top + 0.05, "top {top:.3}, bottom {bottom:.3}");
}

#[test]
fn twin_a_flat_face_is_uniform() {
    let light = Light::new(&KeycapStyle::default());
    let (top, bottom) = (light.shade(dish_normal(0.0, 0.0, 40.0)), light.shade(dish_normal(1.0, 0.0, 40.0)));
    assert!((bottom - top).abs() < 1e-12);
}

#[test]
fn midtone_match_keeps_the_flat_face_at_the_albedo() {
    let light = Light::new(&KeycapStyle::default());
    assert!((light.shade((0.0, 0.0, 1.0)) - 1.0).abs() < 1e-9);
    let raw = Light { midtone_match: 0.0, ..light };
    assert!((raw.shade((0.0, 0.0, 1.0)) - 1.0).abs() > 0.1, "the twin: no match, no albedo");
}

#[test]
fn every_pressed_quantity_lands_on_its_ends_and_between() {
    let s = KeycapStyle::default();
    let h = s.hover_press_fraction;
    let pairs = [
        (s.travel_at(0.0), s.travel_at(h), s.travel_at(1.0), 0.0, s.travel),
        (s.scale_at(0.0), s.scale_at(h), s.scale_at(1.0), 1.0, s.scale_pressed),
        (s.socket_shadow_at(0.0), s.socket_shadow_at(h), s.socket_shadow_at(1.0), s.socket_shadow_rest, s.socket_shadow_pressed),
        (s.shadow_offset_at(0.0), s.shadow_offset_at(h), s.shadow_offset_at(1.0), s.shadow_y, s.press_shadow_y),
        (s.shadow_strength_at(0.0), s.shadow_strength_at(h), s.shadow_strength_at(1.0), s.shadow_opacity, s.press_shadow_opacity),
    ];
    for (rest, hover, pressed, want_rest, want_pressed) in pairs {
        assert_eq!((rest, pressed), (want_rest, want_pressed));
        assert!((hover - rest) * (pressed - hover) > 0.0, "the hover is not partial: {rest} {hover} {pressed}");
    }
}

#[test]
fn a_variant_scales_lengths_and_sets_weights() {
    let small = Variant::Small.applied(&KeycapStyle::default());
    assert!((small.wall_bottom - 12.0 * 0.525).abs() < 1e-12);
    assert_eq!((small.font_size, small.icon_stroke, small.contour_width), (10.5, 1.5, 0.0));
    let mini = Variant::Mini.applied(&KeycapStyle::default());
    assert!((mini.corner_radius - 16.0 * 0.38).abs() < 1e-12);
}

#[test]
fn the_render_is_deterministic_and_sized() {
    let style = Variant::Small.applied(&KeycapStyle::default());
    let a = render(&style, 120.0, 40.0, 0.3, 2.0);
    assert_eq!(a, render(&style, 120.0, 40.0, 0.3, 2.0));
    assert_eq!(a.width as f64, ((120.0 + 2.0 * a.margin) * 2.0f64).ceil());
    assert_eq!(a.pixels.len(), (a.width * a.height * 4) as usize);
}

/// The alpha of the pixel at (`x`, `y`) points from the key's top-left.
fn alpha(style: &KeycapStyle, depth: f64, x: f64, y: f64) -> u8 {
    let frame = Frame { width: 100.0, height: 70.0, scale: 2.0, margin: 30.0 };
    let image = render_in(style, frame, depth);
    let (px, py) = (((x + 30.0) * 2.0) as u32, ((y + 30.0) * 2.0) as u32);
    image.pixels[((py * image.width + px) * 4 + 3) as usize]
}

#[test]
fn the_cast_shadow_falls_below_and_flattens_when_pressed() {
    let style = KeycapStyle::default();
    assert!(alpha(&style, 0.0, 50.0, 75.0) > 10, "no shadow below a key at rest");
    assert_eq!(alpha(&style, 1.0, 50.0, 75.0), 0, "the twin: a pressed key casts none");
}

#[test]
fn the_socket_rim_is_darkest_at_the_edge() {
    let mut style = KeycapStyle { albedo: crate::Rgba::white(1.0), ..KeycapStyle::default() };
    style.shadow_opacity = 0.0;
    let frame = Frame { width: 100.0, height: 70.0, scale: 4.0, margin: 4.0 };
    let image = render_in(&style, frame, 1.0);
    let row = ((4.0 + 35.0) * 4.0) as u32;
    let luma = |x: f64| image.pixels[((row * image.width + ((4.0 + x) * 4.0) as u32) * 4) as usize];
    // Pressed, the cap has shrunk 2.5 pt from each side: its edge lies at x = 2.5.
    let ramp: Vec<u8> = [3.0, 4.0, 5.0, 6.0, 7.0].iter().map(|&x| luma(x)).collect();
    assert!(ramp.windows(2).all(|w| w[0] <= w[1]), "the rim brightens toward the edge: {ramp:?}");
    style.socket_shadow_pressed = 0.0;
    let bare = render_in(&style, frame, 1.0);
    let bare_luma = bare.pixels[((row * bare.width + ((4.0 + 4.0) * 4.0) as u32) * 4) as usize];
    assert!(bare_luma > ramp[1] + 20, "the twin: without a rim the edge is lit, {bare_luma} vs {}", ramp[1]);
}

/// The plane of a single lit pixel blurred by `sigma`, read along its row.
fn spread(sigma: f32) -> Vec<f32> {
    let mut plane = crate::blur::Plane::new(101, 101, 1);
    plane.data[50 * 101 + 50] = 1.0;
    crate::blur::gaussian(&mut plane, sigma);
    (0..101).map(|x| (0..101).map(|y| plane.data[y * 101 + x]).sum()).collect()
}

fn gauss(x: f32, sigma: f32) -> f32 {
    (-x * x / (2.0 * sigma * sigma)).exp() / (sigma * (2.0 * std::f32::consts::PI).sqrt())
}

#[test]
fn the_blur_is_a_gaussian_of_its_deviation() {
    for sigma in [1.5f32, 6.0, 12.0] {
        let row = spread(sigma);
        let variance: f32 = row.iter().enumerate().map(|(i, v)| v * (i as f32 - 50.0).powi(2)).sum();
        assert!((variance.sqrt() / sigma - 1.0).abs() < 0.05, "σ {sigma}: measured {}", variance.sqrt());
        assert!((row[50] / gauss(0.0, sigma) - 1.0).abs() < 0.08, "σ {sigma}: peak {}", row[50]);
    }
    // The twin: half the deviation is told apart.
    let half = spread(3.0);
    assert!((half[50] / gauss(0.0, 6.0) - 1.0).abs() > 0.5);
}

/// Times a 120 × 40 small key at scale 2; `cargo test -p oni-keycap --release cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn cost() {
    let style = Variant::Small.applied(&KeycapStyle::default());
    for (w, h) in [(120.0, 40.0), (140.0, 64.0), (40.0, 40.0)] {
        let start = std::time::Instant::now();
        for i in 0..50 {
            render(&style, w, h, i as f64 / 50.0, 2.0);
        }
        eprintln!("{w} × {h} pt at 2×: {:.2} ms a render", start.elapsed().as_secs_f64() * 1000.0 / 50.0);
    }
}


/// The red at (`x`, `y`) points from the key's top-left, on a 100 × 70 key at 2×, no margin.
fn red(style: &KeycapStyle, x: f64, y: f64) -> u8 {
    let image = render_in(style, Frame { width: 100.0, height: 70.0, scale: 2.0, margin: 0.0 }, 0.0);
    let (px, py) = ((x * 2.0) as u32, (y * 2.0) as u32);
    image.pixels[((py * image.width + px) * 4) as usize]
}

#[test]
fn a_lifted_bottom_wall_is_lighter_and_the_top_wall_keeps_its_shade() {
    let swift = KeycapStyle { shadow_opacity: 0.0, ..KeycapStyle::default() };
    let lifted = KeycapStyle { bottom_lift: 0.6, ..swift.clone() };
    // The bottom wall's middle, a few points above the foot; the top wall's, a few below the head.
    let (bottom, top) = ((50.0, 64.0), (50.0, 4.0));
    assert!(red(&lifted, bottom.0, bottom.1) > red(&swift, bottom.0, bottom.1) + 8);
    // Twin: the top wall does not move, so the lift is the bottom's alone.
    assert_eq!(red(&lifted, top.0, top.1), red(&swift, top.0, top.1));
}
