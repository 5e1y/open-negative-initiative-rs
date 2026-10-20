//! The Lab's values: one set reproduces ONI Pro at rest (neutral), one the measured machine (Frontier).

use super::paper::NODES;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// The colour stage's hue sectors, centred every 30° of output Oklab hue from 0°.
pub const SECTORS: usize = 12;
/// The grey cast's lightness nodes, 0.1 to 0.9.
pub const GREYS: usize = 5;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabParams {
    /// What each channel borrows from the other two, in density: R←G, R←B, G←R, G←B, B←R, B←G.
    pub matrix: [f64; 6],
    /// The paper curve's code at each of its density nodes.
    pub paper: [f64; NODES],
    /// Red's and blue's contrast against green's, around the pivot.
    pub paper_gain: [f64; 2],
    /// Red's and blue's density offset against green's.
    pub paper_offset: [f64; 2],
    /// Degrees added to a sector's hue once its chroma passes 0.03.
    pub hue: [f64; SECTORS],
    /// Degrees added per 0.1 of chroma.
    pub twist: [f64; SECTORS],
    pub chroma: [f64; SECTORS],
    /// Chroma gained per unit of lightness above 0.5.
    pub saturation_by_l: f64,
    /// Oklab a and b added to every colour at each grey lightness node.
    pub grey_a: [f64; GREYS],
    pub grey_b: [f64; GREYS],
    /// The share of the measured table's residual added back, 0 to 1.
    pub character: f64,
}

/// The published neutral set: the chain fitted to ONI Pro at rest on synthetic negatives.
pub const SEED_NEUTRAL: LabParams = LabParams {
    matrix: [-0.181, -0.008, -0.023, 0.041, 0.053, 0.063],
    paper: [0.0, 0.0, 0.139, 0.341, 0.519, 0.659, 0.771, 0.860, 0.929, 0.984, 1.0, 1.0, 1.0, 1.0, 1.0],
    paper_gain: [1.088, 0.947],
    paper_offset: [-0.025, -0.119],
    hue: [0.5, 0.6, 0.8, 0.3, 0.7, 1.9, 1.1, -2.1, -3.8, -1.8, 1.0, 1.3],
    twist: [-0.8, -0.4, 0.4, 0.4, 0.1, 0.1, -0.1, -0.9, -0.8, 0.3, 1.0, 0.1],
    chroma: [1.00, 1.00, 1.01, 1.03, 1.04, 1.06, 1.08, 1.07, 1.01, 0.95, 0.94, 0.98],
    saturation_by_l: 0.02,
    grey_a: [0.005, 0.004, 0.002, 0.003, 0.002],
    grey_b: [0.001, -0.003, -0.001, -0.002, -0.001],
    character: 0.0,
};

/// The published Frontier set: the chain fitted to the 63 measured renders.
pub const SEED_FRONTIER: LabParams = LabParams {
    matrix: [0.047, 0.024, 0.100, -0.023, 0.011, 0.233],
    paper: [0.001, 0.001, 0.015, 0.059, 0.129, 0.272, 0.475, 0.680, 0.825, 0.906, 0.950, 0.974, 0.986, 0.998, 1.0],
    paper_gain: [1.090, 0.951],
    paper_offset: [-0.050, -0.047],
    hue: [0.4, -0.2, -4.7, -0.6, 10.5, 13.0, 2.2, -8.7, -11.2, -10.7, -10.3, -5.8],
    twist: [-3.9, 0.1, 5.9, 2.3, -4.3, -1.2, 7.8, 11.2, 9.8, 10.5, 9.7, 2.3],
    chroma: [0.96, 0.89, 0.95, 1.00, 0.91, 0.83, 0.97, 1.14, 1.12, 1.00, 1.00, 1.03],
    saturation_by_l: 0.06,
    grey_a: [0.009, 0.002, -0.009, 0.002, 0.004],
    grey_b: [0.007, 0.004, 0.011, 0.012, 0.004],
    character: 1.0,
};

impl LabParams {
    /// The matrix with each row summing to one, the diagonal taking what the others lend.
    pub fn rows(&self) -> [[f64; 3]; 3] {
        let m = &self.matrix;
        [[1.0 - m[0] - m[1], m[0], m[1]], [m[2], 1.0 - m[2] - m[3], m[3]], [m[4], m[5], 1.0 - m[4] - m[5]]]
    }

    pub fn with_character(&self, character: f64) -> Self {
        LabParams { character, ..self.clone() }
    }

    /// The same values with the colour stage at rest and no character: the paper stage alone.
    pub fn paper_only(&self) -> Self {
        LabParams {
            hue: [0.0; SECTORS],
            twist: [0.0; SECTORS],
            chroma: [1.0; SECTORS],
            saturation_by_l: 0.0,
            grey_a: [0.0; GREYS],
            grey_b: [0.0; GREYS],
            character: 0.0,
            ..self.clone()
        }
    }
}

/// The fitted sets the Lab ships, written by `examples/look_lab_fit.rs` and never by hand.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fitted {
    pub neutral: LabParams,
    pub frontier: LabParams,
    pub figures: serde_json::Value,
}

pub static FITTED: LazyLock<Fitted> =
    LazyLock::new(|| serde_json::from_slice(include_bytes!("../../resources/scanner/lab.json")).expect("lab.json"));
