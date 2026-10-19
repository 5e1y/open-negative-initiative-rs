//! The version this Mac ran before, for the installation ping's `from`: the app's own mark, else
//! the Swift app's, so a move from Swift counts as an update and never as a first installation.

/// The Swift app's defaults domain, where its `Updates.reportInstall` marks `lastLaunchedVersion`.
pub const SWIFT_DOMAIN: &str = "fr.courtillon.open-negative-initiative";

/// `from` for a launch of `version`: `None` when this version was reported already, else the prior
/// version: ours, the Swift app's, `inconnue` for a copy that checked without a mark, or none at all.
pub fn from(version: &str, ours: Option<String>, swift: Option<String>, checked: bool) -> Option<Option<String>> {
    let prior = ours.or(swift);
    if prior.as_deref() == Some(version) {
        return None;
    }
    Some(prior.or_else(|| checked.then(|| super::UNKNOWN_PRIOR.to_string())))
}

/// The Swift app's last launched version on this Mac, read from its preferences. Reads the disk.
pub fn swift() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = std::process::Command::new("/usr/bin/defaults").args(["read", SWIFT_DOMAIN, super::LAST_LAUNCHED_KEY]).output().ok()?;
    let v = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !v.is_empty()).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_from_swift_reports_an_update_from_its_version() {
        let s = |v: &str| Some(v.to_string());
        assert_eq!(from("1.4.0", None, s("1.3.1"), true), Some(s("1.3.1")), "from the Swift app");
        assert_eq!(from("1.4.0", s("1.4.0-rc"), s("1.3.1"), true), Some(s("1.4.0-rc")), "our own mark first");
        assert_eq!(from("1.4.0", None, None, false), Some(None), "a first installation");
        assert_eq!(from("1.4.0", None, None, true), Some(s("inconnue")));
        assert_eq!(from("1.4.0", s("1.4.0"), None, true), None, "reported once");
        // Twin: without the Swift app's mark the same launch reads as a first installation.
        assert_ne!(from("1.4.0", None, s("1.3.1"), false), from("1.4.0", None, None, false));
    }
}
