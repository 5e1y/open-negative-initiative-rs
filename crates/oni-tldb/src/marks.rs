//! The two marks the sign-in's pages show, inline so no page fetches anything: The Lens DB's aperture, inked
//! by the page's own colours, and ONI's orange keycap.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::sync::LazyLock;

const APERTURE: &str = include_str!("../assets/tldb-icon.svg");
const ONI_ICON: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../oni-ui/ui/icons/app-icon.png"));

/// The aperture at `size` pixels: its disc takes the page's `--disc`, its blades `--blades`, so a dark page
/// inverts it; black on white where the page sets neither.
pub fn tldb(size: u32) -> String {
    APERTURE
        .replace("width=\"64\" height=\"64\"", &format!("width=\"{size}\" height=\"{size}\" aria-hidden=\"true\" focusable=\"false\""))
        .replace("fill=\"#000\"", "style=\"fill:var(--disc,#000)\"")
        .replace("fill=\"#fff\"", "style=\"fill:var(--blades,#fff)\"")
}

/// ONI's app icon as a `data:` address, for an `<img>`.
pub fn oni() -> &'static str {
    static URI: LazyLock<String> = LazyLock::new(|| format!("data:image/png;base64,{}", STANDARD.encode(ONI_ICON)));
    &URI
}
