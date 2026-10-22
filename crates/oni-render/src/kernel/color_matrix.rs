//! Core Image's `CIColorMatrix`.

use super::*;

/// `CIColorMatrix`: each output channel a dot of the input with its vector, plus the bias.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMatrix {
    pub r_vector: V4,
    pub g_vector: V4,
    pub b_vector: V4,
    pub a_vector: V4,
    pub bias_vector: V4,
}

impl Default for ColorMatrix {
    /// Core Image's defaults: the identity, no bias.
    fn default() -> Self {
        Self {
            r_vector: [1.0, 0.0, 0.0, 0.0],
            g_vector: [0.0, 1.0, 0.0, 0.0],
            b_vector: [0.0, 0.0, 1.0, 0.0],
            a_vector: [0.0, 0.0, 0.0, 1.0],
            bias_vector: [0.0; 4],
        }
    }
}

impl ColorMatrix {
    pub fn rows(&self) -> Vec<V4> {
        vec![self.r_vector, self.g_vector, self.b_vector, self.a_vector, self.bias_vector]
    }
}

pub fn color_matrix(s: V4, k: &ColorMatrix) -> V4 {
    let rows = [k.r_vector, k.g_vector, k.b_vector, k.a_vector];
    std::array::from_fn(|c| dot4(s, rows[c]) + k.bias_vector[c])
}
