//! What the automatic keys read: the frame rendered up to a stage, read on blocks or as a whole,
//! and the rules that turn that reading into the handles a key writes. Nothing here runs unasked.

// `max(a).min(b)` and `!(x > 0)` send a NaN to the floor or the refusal, where `clamp` passes it on.
#![allow(clippy::manual_clamp, clippy::neg_cmp_op_on_partial_ord, clippy::type_complexity)]

pub mod auto_levels;
pub mod blocks;
pub mod choice;
pub mod filter_balance;
pub mod histogram;
pub mod key;
pub mod pipette;
pub mod plane;
pub mod reader;
pub mod registration;
pub mod roll;
pub mod truncate;
pub mod window;

pub use histogram::Histogram;
pub use plane::Plane;
