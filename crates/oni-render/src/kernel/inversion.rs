//! The pure inversion, where no paper prints: each channel's window share is the display's sRGB code of the light it
//! shows, decoded in the working space, the code's curve carried on past either end; no other curve, and no bound.

use super::*;

/// The light an sRGB code holds, its straight foot carried on under none and its power past one.
fn decoded(code: f32) -> f32 {
    if code <= 0.04045 { code / 12.92 } else { ((code + 0.055) / 1.055).powf(2.4) }
}

/// One pixel's window shares shown with no paper, in linear Rec. 2020; alpha kept.
pub fn inversion(t: V4) -> V4 {
    with_alpha(map(rgb(t), |_, share| decoded(share)), t[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_share_is_the_srgb_code_of_its_light_and_carries_on_past_the_windows_ends() {
        let shown = inversion([0.0, 0.5, 1.0, 0.25]);
        assert_eq!((shown[0], shown[2], shown[3]), (0.0, 1.0, 0.25));
        assert!((shown[1] - 0.214_041_14).abs() < 1e-6, "{}", shown[1]);
        // The code's straight foot, then its power: a share of 0.04 is its 12.92th.
        assert!((inversion([0.04; 4])[0] - 0.04 / 12.92).abs() < 1e-8);
        // Past either end the same curve carries on, rising: the output clip alone bounds the picture.
        let past = inversion([-0.3, 1.2, 1.7, 1.0]);
        assert!((past[0] + 0.3 / 12.92).abs() < 1e-7 && past[1] > 1.4 && past[2] > past[1], "{past:?}");
        let ramp: Vec<f32> = (-40..=200).map(|i| inversion([i as f32 * 0.01; 4])[0]).collect();
        assert!(ramp.windows(2).all(|w| w[1] > w[0]), "the code's curve rises all along");
        // Twins: a display gamma's power, or the share taken as light, read another grey; a share held at the
        // window's white would show a brighter one no differently.
        assert!((shown[1] - 0.5f32.powf(DISPLAY_GAMMA)).abs() > 3e-3 && (shown[1] - 0.5).abs() > 0.2);
        assert!(past[1] - inversion([1.0; 4])[0] > 0.4);
    }
}
