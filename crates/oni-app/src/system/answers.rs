//! The file dialogs' answers under `ONI_ANSWERS`. An evaluation drives the app through Slint's MCP
//! server, which reaches no system panel, so it lays the path a panel would return beforehand.

use std::path::{Path, PathBuf};

/// `ONI_ANSWERS`: the folder an evaluation lays each dialog's answer in; unset, the panels show.
pub(super) fn folder() -> Option<PathBuf> {
    std::env::var_os("ONI_ANSWERS").map(PathBuf::from)
}

/// The path laid in `folder`, taken once: a dialog that finds none answers as a cancelled one.
pub(super) fn taken(folder: &Path) -> Option<PathBuf> {
    let file = folder.join("answer");
    let text = std::fs::read_to_string(&file).ok()?;
    std::fs::remove_file(&file).ok()?;
    let path = PathBuf::from(text.trim_end());
    (!path.as_os_str().is_empty()).then_some(path)
}

/// A save panel's answer: a folder takes the suggested name inside it, a file path stands as given.
pub(super) fn saved(answer: PathBuf, suggested_name: &str) -> PathBuf {
    if answer.is_dir() { answer.join(suggested_name) } else { answer }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oni-app-answers-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_answer_is_taken_once() {
        let dir = folder("once");
        std::fs::write(dir.join("answer"), format!("{}\n", dir.display())).unwrap();
        assert_eq!(taken(&dir), Some(dir.clone()));
        // Twin: the same dialog opened again finds nothing and answers as cancelled.
        assert_eq!(taken(&dir), None);
    }

    #[test]
    fn a_save_into_a_folder_keeps_the_suggested_name() {
        let dir = folder("save");
        assert_eq!(saved(dir.clone(), "DSC_6432.jpg"), dir.join("DSC_6432.jpg"));
        // Twin: a path that is no folder stands as the file itself.
        let file = dir.join("named.jpg");
        assert_eq!(saved(file.clone(), "DSC_6432.jpg"), file);
    }
}
