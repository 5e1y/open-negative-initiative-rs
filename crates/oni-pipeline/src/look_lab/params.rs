//! The Lab's values: one set reproduces ONI Pro at rest (neutral), one the measured machine (Frontier).

use super::paper::NODES;

#[derive(Clone, Debug, PartialEq)]
pub struct LabParams {
    /// What each channel borrows from the other two, in density: R←G, R←B, G←R, G←B, B←R, B←G.
    pub matrix: [f64; 6],
    /// The paper curve's code at each of its density nodes.
    pub paper: [f64; NODES],
    /// Red's and blue's contrast against green's, around the pivot.
    pub paper_gain: [f64; 2],
    /// Red's and blue's density offset against green's.
    pub paper_offset: [f64; 2],
}

/// The published neutral set: the paper stage fitted to ONI Pro at rest on synthetic negatives.
pub const SEED_NEUTRAL: LabParams = LabParams {
    matrix: [-0.181, -0.008, -0.023, 0.041, 0.053, 0.063],
    paper: [0.0, 0.0, 0.139, 0.341, 0.519, 0.659, 0.771, 0.860, 0.929, 0.984, 1.0, 1.0, 1.0, 1.0, 1.0],
    paper_gain: [1.088, 0.947],
    paper_offset: [-0.025, -0.119],
};

/// The published Frontier set: the paper stage fitted to the 63 measured renders.
pub const SEED_FRONTIER: LabParams = LabParams {
    matrix: [0.047, 0.024, 0.100, -0.023, 0.011, 0.233],
    paper: [0.001, 0.001, 0.015, 0.059, 0.129, 0.272, 0.475, 0.680, 0.825, 0.906, 0.950, 0.974, 0.986, 0.998, 1.0],
    paper_gain: [1.090, 0.951],
    paper_offset: [-0.050, -0.047],
};

impl LabParams {
    /// The matrix with each row summing to one, the diagonal taking what the others lend.
    pub fn rows(&self) -> [[f64; 3]; 3] {
        let m = &self.matrix;
        [[1.0 - m[0] - m[1], m[0], m[1]], [m[2], 1.0 - m[2] - m[3], m[3]], [m[4], m[5], 1.0 - m[4] - m[5]]]
    }
}
