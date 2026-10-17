//! Writes `ui/ds/tokens.slint` from the Swift app's `DSTokens.swift`; `ONI_SWIFT_APP` names its
//! `Sources/OpenNegative` when the checkout is not beside this workspace.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = oni_app::swift_sources().ok_or("no Swift sources: set ONI_SWIFT_APP to Sources/OpenNegative")?;
    let tokens = std::fs::read_to_string(root.join("DesignSystem/DSTokens.swift"))?;
    let gamut = std::fs::read_to_string(root.join("Pipeline/ScreenGamut.swift")).unwrap_or_default();
    let out = oni_app::tokens::generate(&tokens, &[&gamut]).map_err(|e| format!("{e:?}"))?;
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui/ds/tokens.slint");
    std::fs::write(&path, out)?;
    println!("wrote {}", path.display());
    Ok(())
}
