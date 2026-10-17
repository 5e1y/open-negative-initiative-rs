//! `Histogram`: counts per bin over the graduation window, and the ends an area reading carries.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Histogram {
    pub rgb: Vec<[f32; 3]>,
    pub luma: Vec<f32>,
    /// Per channel, where the data starts and stops as a reading carried by an area says it does, on
    /// the counted 0…1 window; `None` when the counts alone place the ends.
    pub ends: Option<Vec<[f32; 2]>>,
}

impl Histogram {
    pub fn is_empty(&self) -> bool {
        self.rgb.len() < 2
    }

    pub fn channel(&self, c: usize) -> Vec<f32> {
        self.rgb.iter().map(|b| b[c]).collect()
    }
}
