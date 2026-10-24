//! A draft's base pick found again among the roll's frames: Swift stores it as an absolute path,
//! which a renamed folder, another mount point or another drive letter leaves pointing nowhere.

use super::model::Frame;
use oni_library::paths;

/// The frame the stored base names: its own path when a frame still has it, else the one frame of
/// that file name; `None` when no frame or several answer, so no ring shows on the wrong scan.
pub fn found(stored: &str, frames: &[Frame]) -> Option<String> {
    if let Some(f) = frames.iter().find(|f| paths::same(&f.path, stored)) {
        return Some(f.path.clone());
    }
    let name = paths::name(stored);
    let mut named = frames.iter().filter(|f| paths::name(&f.path) == name);
    match (named.next(), named.next()) {
        (Some(one), None) => Some(one.path.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(paths: &[&str]) -> Vec<Frame> {
        paths.iter().map(|p| Frame::new(p, "print")).collect()
    }

    #[test]
    fn a_draft_base_moved_with_its_folder_is_found_by_name() {
        let roll = frames(&["/Volumes/T7/Roll 8 bis/DSC_0001.NEF", "/Volumes/T7/Roll 8 bis/DSC_0002.NEF"]);
        let stored = "/Volumes/T7/Roll 8/DSC_0002.NEF";
        assert_eq!(found(stored, &roll).as_deref(), Some("/Volumes/T7/Roll 8 bis/DSC_0002.NEF"));
        let windows = frames(&["E:/Roll 8/DSC_0001.NEF", "E:/Roll 8/DSC_0002.NEF"]);
        assert_eq!(found(stored, &windows).as_deref(), Some("E:/Roll 8/DSC_0002.NEF"), "from a Mac to a Windows drive");
        assert_eq!(found("E:/Roll 8/DSC_0001.NEF", &windows).as_deref(), Some("E:/Roll 8/DSC_0001.NEF"), "its own path first");
        // Twin: a name no frame has, or two frames of one name, find no base, never a wrong one.
        assert_eq!(found("/Volumes/T7/Roll 8/DSC_0009.NEF", &roll), None);
        let twice = frames(&["/r/a/DSC_0002.NEF", "/r/b/DSC_0002.NEF"]);
        assert_eq!(found(stored, &twice), None);
    }
}
