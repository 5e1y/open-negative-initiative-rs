//! `projects.json`, read in any format an app wrote it and written in one every build reads: the
//! members as version 1 names them, with each one's exact path and print beside, which older builds drop.

use super::member::{self, Member, distinct, exclusive};
use crate::library::WAITING_MARK;
use super::Project;
use crate::paths;
use oni_settings::json;
use oni_settings::{Date, Id};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The format written: version 1's texts and paths, which 1.5.6 reads, and `exactMembers` beside
/// them. Version 2 wrote each member as an object, which no build before it parses.
pub const FORMAT: i64 = 3;

/// The one file every project lives in.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "Incoming")]
pub struct ProjectsFile {
    pub version: i64,
    pub projects: Vec<Project>,
}

impl Default for ProjectsFile {
    fn default() -> Self {
        ProjectsFile { version: FORMAT, projects: Vec::new() }
    }
}

#[derive(Deserialize)]
struct Incoming {
    version: i64,
    projects: Vec<Written>,
}

/// A project as any app wrote it.
#[derive(Deserialize)]
struct Written {
    id: Id,
    name: String,
    description: String,
    #[serde(rename = "createdAt", with = "crate::reference_date")]
    created_at: Date,
    #[serde(rename = "manualOrder", default)]
    manual_order: Option<f64>,
    members: Vec<Writ>,
    #[serde(default)]
    paths: Option<HashMap<String, String>>,
    #[serde(rename = "exactMembers", default)]
    exact: Option<Vec<Member>>,
}

/// One member as written: a text, or version 2's object.
#[derive(Deserialize)]
#[serde(untagged)]
enum Writ {
    Member(Member),
    Text(String),
}

impl From<Incoming> for ProjectsFile {
    fn from(file: Incoming) -> ProjectsFile {
        let named: HashSet<String> = file.projects.iter().flat_map(Written::named).collect();
        ProjectsFile { version: file.version, projects: file.projects.into_iter().map(|w| w.read(&named)).collect() }
    }
}

impl Written {
    /// The paths its texts name members by, as `former` keys them.
    fn named(&self) -> Vec<String> {
        let recorded = self.paths.as_ref();
        let own = |text: &String| recorded.and_then(|r| r.get(text)).unwrap_or(text).to_lowercase();
        self.members
            .iter()
            .filter_map(|w| match w {
                Writ::Text(text) if is_path(text) => Some(own(text)),
                _ => None,
            })
            .collect()
    }

    /// The exact members while the texts beside still name them one for one, else the texts read
    /// as version 1: an older build rewrote them and dropped the rest.
    fn read(self, named: &HashSet<String>) -> Project {
        let recorded = self.paths.unwrap_or_default();
        // A print shaped as a path is a path text a version 2 build took for a print with its path beside.
        let spelt = |m: Member| Member { path: m.path.map(|p| paths::normal(&p).into_owned()), print: m.print.filter(|p| is_print(p)) };
        let members: Vec<Member> = match self.exact.filter(|exact| agree(exact, &self.members)) {
            Some(exact) => exact.into_iter().map(spelt).collect(),
            None => self
                .members
                .into_iter()
                .map(|writ| match writ {
                    Writ::Member(m) => spelt(m),
                    Writ::Text(text) => former(&text, &recorded, named),
                })
                .collect(),
        };
        Project { id: self.id, name: self.name, description: self.description, created_at: self.created_at, manual_order: self.manual_order, members: distinct(members) }
    }
}

/// Whether each text is its exact member's print or path, in order.
fn agree(exact: &[Member], texts: &[Writ]) -> bool {
    let names = |m: &Member, t: &String| m.print.as_ref() == Some(t) || m.path.as_ref() == Some(t);
    exact.len() == texts.len() && exact.iter().zip(texts).all(|(m, writ)| matches!(writ, Writ::Text(t) if names(m, t)))
}

/// A version 1 member: a path, carried where a move recorded it, or a print with the path it was last
/// seen at; a print gives up a path another member is named by, as 1.5.6 records the copy it found.
fn former(text: &str, recorded: &HashMap<String, String>, named: &HashSet<String>) -> Member {
    let spelt = |path: &str| paths::normal(path).into_owned();
    let own = recorded.get(text).map(String::as_str);
    if is_path(text) {
        return Member { path: Some(spelt(own.unwrap_or(text))), print: None };
    }
    // 1.5.6 names a photo still being read by its placeholder, the photo's path after the mark.
    if !is_print(text) {
        return Member { path: own.or_else(|| text.strip_prefix(WAITING_MARK)).filter(|p| !p.is_empty()).map(spelt), print: None };
    }
    let path = own.filter(|p| !named.contains(&p.to_lowercase())).map(spelt);
    Member { path, print: Some(text.to_owned()) }
}

/// Whether a former member's text is a path: one from a root, `/`, `\` or a drive's letter, where
/// a print starts with its file's size.
pub fn is_path(text: &str) -> bool {
    text.starts_with(['/', '\\']) || text.as_bytes().get(1) == Some(&b':')
}

/// Whether a text is a print as read: neither a path nor a placeholder for a print not read yet.
fn is_print(text: &str) -> bool {
    !is_path(text) && member::read(text).is_some()
}

#[derive(Serialize)]
struct Outgoing<'a> {
    version: i64,
    projects: Vec<Out<'a>>,
}

/// A project as this format writes it; `members` and `paths` are what 1.5.6 reads.
#[derive(Serialize)]
struct Out<'a> {
    id: Id,
    name: &'a str,
    description: &'a str,
    #[serde(rename = "createdAt", with = "crate::reference_date")]
    created_at: Date,
    #[serde(rename = "manualOrder", skip_serializing_if = "Option::is_none")]
    manual_order: Option<f64>,
    members: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    paths: BTreeMap<String, String>,
    #[serde(rename = "exactMembers")]
    exact: Vec<&'a Member>,
}

/// Each member named by its print the first time the file names that print, a print alone first,
/// by its path after, so no two members share a text 1.5.6 takes for one photo; each text's path beside.
fn written(projects: &[Project]) -> Vec<Out<'_>> {
    let members = projects.iter().flat_map(|p| &p.members);
    let mut used: HashSet<&str> = members.filter(|m| m.path.is_none()).filter_map(|m| m.print.as_deref()).collect();
    let mut out = Vec::with_capacity(projects.len());
    for p in projects {
        let (mut members, mut recorded, mut exact) = (Vec::new(), BTreeMap::new(), Vec::new());
        for m in &p.members {
            let text = match (m.print.as_deref(), m.path.as_deref()) {
                (Some(print), None) => print,
                (Some(print), Some(_)) if used.insert(print) => print,
                (_, Some(path)) => path,
                (None, None) => continue,
            };
            if let Some(path) = &m.path {
                recorded.insert(text.to_owned(), path.clone());
            }
            members.push(text.to_owned());
            exact.push(m);
        }
        let (id, name, description, created_at, manual_order) = (p.id, p.name.as_str(), p.description.as_str(), p.created_at, p.manual_order);
        out.push(Out { id, name, description, created_at, manual_order, members, paths: recorded, exact });
    }
    out
}

pub fn file_path(support: &Path) -> PathBuf {
    support.join("projects.json")
}

/// Where the file a former format wrote is kept, once, before this format first replaces it.
pub fn former_path(support: &Path, version: i64) -> PathBuf {
    support.join(format!("projects-v{version}.json"))
}

/// The version a file says, when it says one.
fn stamp(bytes: &[u8]) -> Option<i64> {
    #[derive(Deserialize)]
    struct Stamp {
        version: i64,
    }
    serde_json::from_slice::<Stamp>(bytes).ok().map(|s| s.version)
}

/// The file at `path`, `None` when absent or unreadable; one a newer build wrote is read as far as
/// this build can and never set aside, as that build reads it whole.
pub(crate) fn read(path: &Path) -> Option<ProjectsFile> {
    let bytes = std::fs::read(path).ok()?;
    match stamp(&bytes) {
        Some(version) if version > FORMAT => serde_json::from_slice(&bytes).ok(),
        _ => crate::read_json(path),
    }
}

pub fn load(support: &Path) -> ProjectsFile {
    let Some(mut file) = read(&file_path(support)) else {
        return ProjectsFile::default();
    };
    file.projects = exclusive(&file.projects);
    file
}

/// The text `JSONEncoder` writes with `.prettyPrinted` and `.sortedKeys`, always in this format.
pub fn encode(file: &ProjectsFile) -> Result<String, json::Error> {
    Ok(json::pretty_sorted(&json::to_node(&Outgoing { version: FORMAT, projects: written(&file.projects) })?))
}

pub fn save(support: &Path, file: &ProjectsFile) -> std::io::Result<()> {
    std::fs::create_dir_all(support)?;
    keep_former(support)?;
    let text = encode(file).map_err(std::io::Error::other)?;
    crate::replace_readable(&file_path(support), text.as_bytes())
}

/// The file as a former format wrote it, copied beside it unless a copy of that format is kept
/// already; a file a newer build wrote is never written over.
fn keep_former(support: &Path) -> std::io::Result<()> {
    let Ok(bytes) = std::fs::read(file_path(support)) else { return Ok(()) };
    let Some(version) = stamp(&bytes) else { return Ok(()) };
    if version > FORMAT {
        return Err(std::io::Error::other(oni_i18n::tr("their file comes from a newer version of ONI, which this one never overwrites")));
    }
    let kept = former_path(support, version);
    if version == FORMAT || kept.exists() {
        return Ok(());
    }
    crate::write_atomically(&kept, &bytes)
}
