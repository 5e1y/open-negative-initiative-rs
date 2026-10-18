//! How the graph's errors read.

use super::*;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoAdapter(e) => write!(f, "no GPU adapter: {e}"),
            Error::NoDevice(e) => write!(f, "no GPU device: {e}"),
            Error::RegionOutside { region, extent } => write!(f, "{region:?} is not inside {extent:?}"),
            Error::EmptyInput { stage } => write!(f, "stage {stage} reads nothing of its input"),
            Error::TooLarge => write!(f, "one output pixel needs more than the GPU's texture side"),
            Error::Size { expected, got } => write!(f, "{got} pixels given for {expected}"),
        }
    }
}
