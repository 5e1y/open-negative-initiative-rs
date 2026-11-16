//! The Lens DB: a person's Kit of bodies and lenses, kept on their TLDB account, read, searched and changed
//! through its partner API (v1) once they signed in by OAuth 2 with PKCE in the system browser.

pub mod api;
pub mod copy;
pub mod fake;
pub mod marks;
pub mod oauth;
pub mod piece;

pub use api::{Account, Client, Failure, Kit, Read, Service};
pub use copy::Kept;
pub use piece::{Aperture, Key, Kind, Piece, Span};

/// ONI's public client at The Lens DB: no secret, PKCE alone proves the sign-in.
pub const CLIENT_ID: &str = "oni";
/// The service itself; `ONI_TLDB=fake` swaps it for the simulated one in `fake`.
pub const SITE: &str = "https://thelensdb.com";
