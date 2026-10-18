//! `CaptureExposure`: how a scan was exposed when it was digitised, read off its EXIF. A roll is
//! measured as one only if every frame was captured alike; an automatic camera captures each apart.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureExposure {
    pub shutter: f64,
    pub aperture: f64,
    pub iso: f64,
}

impl CaptureExposure {
    /// The light gathered, in stops: twice the shutter or the ISO is one more, one stop of aperture one fewer.
    pub fn stops(&self) -> f32 {
        (self.shutter.log2() + (self.iso / 100.0).log2() - 2.0 * self.aperture.log2()) as f32
    }

    /// As a photographer writes it: `1/50 s, f/5.6, ISO 100`.
    pub fn label(&self) -> String {
        let speed = if self.shutter >= 1.0 { format!("{:.1} s", self.shutter) } else { format!("1/{} s", (1.0 / self.shutter).round() as i64) };
        let aperture = if self.aperture < 10.0 { format!("{:.1}", self.aperture) } else { format!("{:.0}", self.aperture) };
        format!("{speed}, f/{aperture}, ISO {}", self.iso.round() as i64)
    }

    pub fn read(path: &Path) -> Option<CaptureExposure> {
        let (shutter, aperture, iso) = crate::exif::exposure(path)?;
        (shutter > 0.0 && aperture > 0.0 && iso > 0.0).then_some(CaptureExposure { shutter, aperture, iso })
    }

    /// The settings most of the roll shares; on a tie the first met, as `max(by:)` keeps it.
    pub fn common(captures: &[Option<CaptureExposure>]) -> Option<CaptureExposure> {
        let read: Vec<CaptureExposure> = captures.iter().flatten().copied().collect();
        let count = |c: &CaptureExposure| read.iter().filter(|o| *o == c).count();
        let mut best: Option<(CaptureExposure, usize)> = None;
        for c in &read {
            let n = count(c);
            if best.is_none_or(|(_, m)| m < n) {
                best = Some((*c, n));
            }
        }
        best.map(|(c, _)| c)
    }

    /// Each frame's stops past the roll's common settings; a frame whose EXIF reads nothing stays at zero.
    pub fn offsets(captures: &[Option<CaptureExposure>]) -> Vec<f32> {
        let Some(common) = Self::common(captures) else { return vec![0.0; captures.len()] };
        captures.iter().map(|c| c.map_or(0.0, |c| c.stops() - common.stops())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `CaptureExposure.selfCheck`, its arithmetic part; the render part lives with the pipeline.
    #[test]
    fn a_capture_reads_in_stops_against_the_settings_most_of_its_roll_shares() {
        let base = CaptureExposure { shutter: 1.0 / 50.0, aperture: 5.6, iso: 100.0 };
        let longer = CaptureExposure { shutter: 1.0 / 25.0, ..base };
        let faster = CaptureExposure { iso: 200.0, ..base };
        let closed = CaptureExposure { aperture: 8.0, ..base };
        assert!((longer.stops() - base.stops() - 1.0).abs() < 1e-6 && (faster.stops() - base.stops() - 1.0).abs() < 1e-6);
        assert!((closed.stops() - base.stops() + 1.029).abs() < 0.01);
        let found = CaptureExposure::offsets(&[Some(base), Some(longer), Some(base), None, Some(closed)]);
        let expected = [0.0, 1.0, 0.0, 0.0, closed.stops() - base.stops()];
        assert!(found.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-6), "{found:?}");
        // Twin: a roll led by its odd frame still reads against the others.
        assert_eq!(CaptureExposure::offsets(&[Some(longer), Some(base), Some(base)]), vec![1.0, 0.0, 0.0]);
        assert_eq!(base.label(), "1/50 s, f/5.6, ISO 100");
        assert_eq!(CaptureExposure { shutter: 2.0, aperture: 11.0, iso: 400.0 }.label(), "2.0 s, f/11, ISO 400");
        // A tie keeps the first met.
        assert_eq!(CaptureExposure::common(&[Some(longer), Some(base)]), Some(longer));
    }
}
