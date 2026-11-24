//! The pure inversion, where no paper prints: each channel's window share, held within 0 and 1, is the display's sRGB
//! code of the light it shows, decoded in the working space; no curve of any other kind.

use super::*;

/// The light an sRGB code holds.
fn decoded(code: f32) -> f32 {
    if code <= 0.04045 { code / 12.92 } else { ((code + 0.055) / 1.055).powf(2.4) }
}

/// One pixel's window shares shown with no paper, in linear Rec. 2020; alpha kept.
pub fn inversion(t: V4) -> V4 {
    with_alpha(map(rgb(t), |_, share| decoded(clamp(share, 0.0, 1.0))), t[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_share_is_the_srgb_code_of_its_light_held_between_the_windows_ends() {
        let shown = inversion([0.0, 0.5, 1.0, 0.25]);
        assert_eq!((shown[0], shown[2], shown[3]), (0.0, 1.0, 0.25));
        assert!((shown[1] - 0.214_041_14).abs() < 1e-6, "{}", shown[1]);
        // The code's straight foot, then its power: a share of 0.04 is its 12.92th.
        assert!((inversion([0.04; 4])[0] - 0.04 / 12.92).abs() < 1e-8);
        // A share past either end of the window shows that end.
        assert_eq!(rgb(inversion([-0.3, 1.7, f32::NAN, 1.0])), [0.0, 1.0, 0.0]);
        // Twins: a display gamma's power, or the share taken as light, read another grey.
        assert!((shown[1] - 0.5f32.powf(DISPLAY_GAMMA)).abs() > 3e-3 && (shown[1] - 0.5).abs() > 0.2);
    }
}
