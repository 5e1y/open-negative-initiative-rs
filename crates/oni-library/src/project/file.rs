//! `State/projects.json`: every project in one file, each member by its path and its print.

use super::Project;
use super::member::exclusive;
use crate::state::{self, Read};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Moved at every change of the file's shape; a newer one is shown and never written.
pub const FORMAT: i64 = 5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectsFile {
    pub version: i64,
    pub projects: Vec<Project>,
}

impl Default for ProjectsFile {
    fn default() -> Self {
        ProjectsFile { version: FORMAT, projects: Vec::new() }
    }
}

impl ProjectsFile {
    /// Read from a newer build's file: shown as it is, never changed by this one.
    pub fn is_newer(&self) -> bool {
        self.version > FORMAT
    }
}

pub fn file_path(support: &Path) -> PathBuf {
    state::folder(support).join("projects.json")
}

/// Empty while absent or unreadable, an unreadable file set aside; one photo a project, the newest's.
pub fn load(support: &Path) -> ProjectsFile {
    let file = match state::read::<ProjectsFile>(&file_path(support), FORMAT) {
        Read::Current(file) => file,
        Read::Newer(version, read) => ProjectsFile { version, ..read.unwrap_or_default() },
        Read::Absent | Read::Unreadable => return ProjectsFile::default(),
    };
    ProjectsFile { projects: exclusive(&file.projects), ..file }
}

/// The file written in this format, a member naming nothing left out; a newer file is never written over.
pub fn save(support: &Path, file: &ProjectsFile) -> std::io::Result<()> {
    if file.is_newer() {
        return Err(state::left_newer());
    }
    let mut projects = file.projects.clone();
    for project in &mut projects {
        project.members.retain(|m| m.path.is_some() || m.print.is_some());
    }
    state::write(&file_path(support), &ProjectsFile { version: FORMAT, projects })
}
