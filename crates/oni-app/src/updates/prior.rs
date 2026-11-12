//! The version this machine ran before, for the installation ping's `from`: the app's own mark, which
//! the first launch takes from the former app's preferences, so a move from it counts as an update.

/// `from` for a launch of `version`: `None` when this version was reported already, else the prior
/// version: the mark, `inconnue` for a copy that checked without a mark, or none at all.
pub fn from(version: &str, prior: Option<String>, checked: bool) -> Option<Option<String>> {
    if prior.as_deref() == Some(version) {
        return None;
    }
    Some(prior.or_else(|| checked.then(|| super::UNKNOWN_PRIOR.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_launch_reports_an_update_from_the_version_marked() {
        let s = |v: &str| Some(v.to_string());
        assert_eq!(from("1.4.0", s("1.3.1"), true), Some(s("1.3.1")), "from the version marked");
        assert_eq!(from("1.4.0", None, false), Some(None), "a first installation");
        assert_eq!(from("1.4.0", None, true), Some(s("inconnue")));
        assert_eq!(from("1.4.0", s("1.4.0"), true), None, "reported once");
        // Twin: without the mark the same launch reads as a first installation.
        assert_ne!(from("1.4.0", s("1.3.1"), false), from("1.4.0", None, false));
    }
}
