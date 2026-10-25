//! Swift's `KeycapStyle`: every field the render and the press read, at Swift's defaults, with its
//! depth interpolation and its variants. Lengths are points; a render scales them to pixels.

/// An sRGB colour with straight alpha, as Swift's `KeycapStyle.RGBA`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub const fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn white(w: f64) -> Self {
        Self::new(w, w, w)
    }

    /// From 8-bit sRGB components, as the design tokens write them.
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r: r as f64 / 255.0, g: g as f64 / 255.0, b: b as f64 / 255.0, a: a as f64 / 255.0 }
    }

    /// `RGBA.shaded`: each channel times the shade, clipped at white; alpha untouched.
    pub fn shaded(self, shade: f64) -> Self {
        Self { r: (self.r * shade).min(1.0), g: (self.g * shade).min(1.0), b: (self.b * shade).min(1.0), a: self.a }
    }
}

/// The press and release curves: a timing curve or a spring, as Swift's `KeycapStyle.Timing`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Timing {
    Custom,
    Grab,
    Settle,
    Lift,
    Travel,
    ReferencePress,
    ReferenceRelease,
}

/// What a `Timing` animates with: a cubic Bézier over a duration, or a spring's response and damping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    Curve { x1: f64, y1: f64, x2: f64, y2: f64, duration: f64 },
    Spring { response: f64, damping: f64 },
}

impl Timing {
    /// `KeycapTokens.Motion` for the four springs, the reference's two CSS curves for the press.
    pub fn motion(self, response: f64, damping: f64) -> Motion {
        match self {
            Timing::ReferencePress => Motion::Curve { x1: 0.0, y1: 0.0, x2: 0.0, y2: 1.0, duration: 0.1 },
            Timing::ReferenceRelease => Motion::Curve { x1: 0.79, y1: 0.03, x2: 0.43, y2: 1.0, duration: 0.02 },
            Timing::Custom => Motion::Spring { response, damping },
            Timing::Grab => Motion::Spring { response: 0.22, damping: 0.62 },
            Timing::Settle => Motion::Spring { response: 0.38, damping: 0.72 },
            Timing::Lift => Motion::Spring { response: 0.16, damping: 0.55 },
            Timing::Travel => Motion::Spring { response: 0.52, damping: 1.0 },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeycapStyle {
    pub padding_h: f64,
    pub padding_v: f64,
    pub corner_radius: f64,
    /// SwiftUI's `.continuous` corner, else a circular arc.
    pub squircle: bool,
    /// The aperture, its contour and the rim shadow: one material seen three ways.
    pub socket_color: Rgba,
    pub contour_width: f64,
    pub wall_top: f64,
    pub wall_bottom: f64,
    pub wall_left: f64,
    pub wall_right: f64,
    pub rise: f64,
    pub face_radius: f64,
    pub dish_depth: f64,
    pub seam_blur: f64,
    pub albedo: Rgba,
    pub ambient: f64,
    pub diffuse: f64,
    pub light_azimuth: f64,
    pub light_elevation: f64,
    /// 1 makes the albedo the flat face's colour rather than a ceiling.
    pub midtone_match: f64,
    pub socket_shadow_softness: f64,
    pub socket_shadow_offset: f64,
    pub socket_shadow_rest: f64,
    pub socket_shadow_pressed: f64,
    pub shadow_color: Rgba,
    pub shadow_blur: f64,
    pub shadow_y: f64,
    pub shadow_opacity: f64,
    /// How far the bottom wall's shade moves toward the flat face's: 0 is Swift's, 1 a bottom as lit as the top.
    pub bottom_lift: f64,
    pub press_shadow_y: f64,
    pub press_shadow_opacity: f64,
    pub scale_pressed: f64,
    pub travel: f64,
    pub hover_press_fraction: f64,
    /// The shortest time the cap stays down, in seconds: a tap otherwise plays a tenth of the press.
    pub min_hold: f64,
    pub press_timing: Timing,
    pub press_response: f64,
    pub press_damping: f64,
    pub separate_release: bool,
    pub release_timing: Timing,
    pub release_response: f64,
    pub release_damping: f64,
    pub font_size: f64,
    pub icon_size: f64,
    pub icon_stroke: f64,
    pub tracking: f64,
    pub label_color: Rgba,
    pub label_offset_y: f64,
}

impl Default for KeycapStyle {
    fn default() -> Self {
        Self {
            padding_h: 32.0,
            padding_v: 26.0,
            corner_radius: 16.0,
            squircle: true,
            socket_color: Rgba::new(0.0, 0.0, 0.0),
            contour_width: 0.0,
            wall_top: 8.0,
            wall_bottom: 12.0,
            wall_left: 8.0,
            wall_right: 8.0,
            rise: 6.0,
            face_radius: 14.0,
            dish_depth: 4.0,
            seam_blur: 3.0,
            albedo: Rgba::new(1.0, 1.0, 1.0),
            ambient: 0.25,
            diffuse: 1.0,
            light_azimuth: 100.0,
            light_elevation: 33.0,
            midtone_match: 1.0,
            socket_shadow_softness: 0.5,
            socket_shadow_offset: 0.0,
            socket_shadow_rest: 2.0,
            socket_shadow_pressed: 6.0,
            shadow_color: Rgba::new(0.0, 0.0, 0.0),
            shadow_blur: 8.0,
            shadow_y: 10.0,
            shadow_opacity: 0.2,
            bottom_lift: 0.0,
            press_shadow_y: 0.0,
            press_shadow_opacity: 0.0,
            scale_pressed: 0.95,
            travel: 1.0,
            hover_press_fraction: 0.25,
            min_hold: 0.14,
            press_timing: Timing::ReferencePress,
            press_response: 0.1,
            press_damping: 0.8,
            separate_release: true,
            release_timing: Timing::ReferenceRelease,
            release_response: 0.26,
            release_damping: 0.55,
            font_size: 14.0,
            icon_size: 28.0,
            icon_stroke: 2.5,
            tracking: 0.5,
            label_color: Rgba::new(0.1298, 0.1298, 0.1298),
            label_offset_y: 0.0,
        }
    }
}

/// The least share of the cap the face keeps; below it the walls shrink but keep their shading.
pub const MIN_FACE_SHARE: f64 = 0.4;

/// A rectangle in points: origin and size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl KeycapStyle {
    /// `KeycapStyle.hardened`: the frozen look every app key starts from.
    pub fn hardened() -> Self {
        Self::default()
    }

    // One depth: 0 at rest, `hover_press_fraction` hovered, 1 pressed.
    pub fn travel_at(&self, depth: f64) -> f64 {
        self.travel * depth
    }

    pub fn scale_at(&self, depth: f64) -> f64 {
        1.0 + (self.scale_pressed - 1.0) * depth
    }

    pub fn socket_shadow_at(&self, depth: f64) -> f64 {
        self.socket_shadow_rest + (self.socket_shadow_pressed - self.socket_shadow_rest) * depth
    }

    pub fn shadow_offset_at(&self, depth: f64) -> f64 {
        self.shadow_y + (self.press_shadow_y - self.shadow_y) * depth
    }

    pub fn shadow_strength_at(&self, depth: f64) -> f64 {
        self.shadow_opacity + (self.press_shadow_opacity - self.shadow_opacity) * depth
    }

    pub fn press_motion(&self) -> Motion {
        self.press_timing.motion(self.press_response, self.press_damping)
    }

    pub fn release_motion(&self) -> Motion {
        if self.separate_release {
            self.release_timing.motion(self.release_response, self.release_damping)
        } else {
            self.press_motion()
        }
    }

    /// Centres the label on the face, apart from the padding so retuning the walls keeps the height.
    pub fn label_centring(&self) -> f64 {
        (self.wall_top - self.wall_bottom) / 2.0
    }

    /// Scales every length but the weights (`contour_width`, `font_size`, `icon_stroke`).
    pub fn scaled(&self, factor: f64) -> Self {
        let mut out = self.clone();
        for v in [
            &mut out.padding_h, &mut out.padding_v, &mut out.corner_radius, &mut out.wall_top,
            &mut out.wall_bottom, &mut out.wall_left, &mut out.wall_right, &mut out.rise,
            &mut out.face_radius, &mut out.dish_depth, &mut out.seam_blur, &mut out.socket_shadow_rest,
            &mut out.socket_shadow_pressed, &mut out.socket_shadow_offset, &mut out.shadow_blur,
            &mut out.shadow_y, &mut out.press_shadow_y, &mut out.travel, &mut out.label_offset_y,
            &mut out.icon_size,
        ] {
            *v *= factor;
        }
        out
    }

    /// The face inside a cap of `w` × `h`; walls squeezed alike when they would leave it under 40 %.
    pub fn face_rect(&self, w: f64, h: f64) -> Rect {
        let squeeze = fit(self.wall_top + self.wall_bottom, h).min(fit(self.wall_left + self.wall_right, w));
        Rect {
            x: self.wall_left * squeeze,
            y: self.wall_top * squeeze,
            w: (w - (self.wall_left + self.wall_right) * squeeze).max(0.0),
            h: (h - (self.wall_top + self.wall_bottom) * squeeze).max(0.0),
        }
    }
}

fn fit(runs: f64, extent: f64) -> f64 {
    let allowed = extent * (1.0 - MIN_FACE_SHARE);
    if runs > allowed && runs > 0.0 { allowed / runs } else { 1.0 }
}

/// `Keycap.Variant`: every length scaled, the legend and the glyph stroke set rather than scaled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Large,
    Small,
    Mini,
}

impl Variant {
    pub fn factor(self) -> f64 {
        match self {
            Variant::Large => 1.0,
            Variant::Small => 0.525,
            Variant::Mini => 0.38,
        }
    }

    pub fn font_size(self) -> f64 {
        match self {
            Variant::Large => 14.0,
            Variant::Small => 10.5,
            Variant::Mini => 9.0,
        }
    }

    pub fn icon_stroke(self) -> f64 {
        match self {
            Variant::Large => 2.5,
            Variant::Small => 1.5,
            Variant::Mini => 1.0,
        }
    }

    pub fn applied(self, style: &KeycapStyle) -> KeycapStyle {
        let mut out = style.scaled(self.factor());
        out.icon_stroke = self.icon_stroke();
        out.font_size = self.font_size();
        out
    }
}
