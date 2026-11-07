//! `projects.json`, read in any format an app wrote it and written in this one's, which names each
//! member by its path and its print; the first write over a former format keeps that file beside it.

use super::member::{Member, distinct, exclusive};
use super::Project;
use crate::paths;
use oni_settings::json;
use oni_settings::{Date, Id};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The format written: each member as its path and its print. Version 1 named prints, with their
/// last paths beside them or not, or paths alone.
pub const FORMAT: i64 = 2;

/// The one file every project lives in.
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

/// A project as any app wrote it.
#[derive(Deserialize)]
pub(super) struct Written {
    id: Id,
    name: String,
    description: String,
    #[serde(rename = "createdAt", with = "crate::reference_date")]
    created_at: Date,
    #[serde(rename = "manualOrder", default)]
    manual_order: Option<f64>,
    members: Vec<Writ>,
    #[serde(default)]
    paths: HashMap<String, String>,
}

/// One member as written: this format's pair, or a former format's text.
#[derive(Deserialize)]
#[serde(untagged)]
enum Writ {
    Member(Member),
    Text(String),
}

impl From<Written> for Project {
    fn from(written: Written) -> Project {
        let members = written
            .members
            .into_iter()
            .map(|writ| match writ {
                Writ::Member(m) => Member { path: m.path.map(|p| paths::normal(&p).into_owned()), ..m },
                Writ::Text(text) => former(text, &written.paths),
            })
            .collect();
        Project {
            id: written.id,
            name: written.name,
            description: written.description,
            created_at: written.created_at,
            manual_order: written.manual_order,
            members: distinct(members),
        }
    }
}

/// A former member: a print with the path it was last seen at, a path alone, or a Swift print,
/// each path spelt as the index spells it; the print it recorded is always kept.
fn former(text: String, recorded: &HashMap<String, String>) -> Member {
    let spelt = |path: &str| Some(paths::normal(path).into_owned());
    match recorded.get(&text) {
        Some(path) => Member { path: spelt(path), print: Some(text) },
        None if is_path(&text) => Member { path: spelt(&text), print: None },
        None => Member { path: None, print: Some(text) },
    }
}

/// Whether a former member's text is a path: one from a root, `/`, `\` or a drive's letter, where
/// a print starts with its file's size.
pub fn is_path(text: &str) -> bool {
    text.starts_with(['/', '\\']) || text.as_bytes().get(1) == Some(&b':')
}

pub fn file_path(support: &Path) -> PathBuf {
    support.join("projects.json")
}

/// Where the file a former format wrote is kept, once, before this format first replaces it.
pub fn former_path(support: &Path, version: i64) -> PathBuf {
    support.join(format!("projects-v{version}.json"))
}

pub fn load(support: &Path) -> ProjectsFile {
    let Some(mut file) = crate::read_json::<ProjectsFile>(&file_path(support)) else {
        return ProjectsFile::default();
    };
    file.projects = exclusive(&file.projects);
    file
}

/// The text `JSONEncoder` writes with `.prettyPrinted` and `.sortedKeys`.
pub fn encode(file: &ProjectsFile) -> Result<String, json::Error> {
    Ok(json::pretty_sorted(&json::to_node(file)?))
}

pub fn save(support: &Path, file: &ProjectsFile) -> std::io::Result<()> {
    std::fs::create_dir_all(support)?;
    keep_former(support)?;
    let text = encode(file).map_err(std::io::Error::other)?;
    crate::replace_readable(&file_path(support), text.as_bytes())
}

/// The file as a former format wrote it, copied beside it unless a copy of that format is kept already.
fn keep_former(support: &Path) -> std::io::Result<()> {
    #[derive(Deserialize)]
    struct Stamp {
        version: i64,
    }
    let Ok(bytes) = std::fs::read(file_path(support)) else { return Ok(()) };
    let Ok(Stamp { version }) = serde_json::from_slice::<Stamp>(&bytes) else { return Ok(()) };
    let kept = former_path(support, version);
    if version >= FORMAT || kept.exists() {
        return Ok(());
    }
    crate::write_atomically(&kept, &bytes)
}
