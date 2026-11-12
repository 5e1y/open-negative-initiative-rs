//! `projects.json` as builds before `State/` wrote it, formats 1 to 4: texts naming each member by
//! its print or its path (1.5.6, with its placeholders), version 2's objects, `exactMembers` beside.

use crate::library::WAITING_MARK;
use crate::paths;
use crate::project::{Member, Project, distinct, exclusive, member};
use oni_settings::{Date, Id};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// The projects the file holds as the build that wrote it would show them, one photo a project.
pub fn read(bytes: &[u8]) -> Option<Vec<Project>> {
    let file: Incoming = serde_json::from_slice(bytes).ok()?;
    let named: HashSet<String> = file.projects.iter().flat_map(Written::named).collect();
    let projects: Vec<Project> = file.projects.into_iter().map(|w| w.read(&named)).collect();
    Some(exclusive(&projects))
}

#[derive(Deserialize)]
struct Incoming {
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

/// A member as an object names it, a print that is none naming its place where it has no path: a
/// path text a version 2 build took for a print, or 1.5.6's placeholder, kept as given.
fn spelt(m: Member) -> Member {
    let path = m.path.as_deref().or_else(|| m.print.as_deref().and_then(place)).map(|p| paths::normal(p).into_owned());
    Member { path, print: m.print.filter(|text| is_print(text) || waits(text)) }
}

/// Whether a text is 1.5.6's placeholder for a print not read yet.
fn waits(text: &str) -> bool {
    text.starts_with(WAITING_MARK)
}

/// Whether each text is its exact member's print or path, in order.
fn agree(exact: &[Member], texts: &[Writ]) -> bool {
    let names = |m: &Member, t: &String| m.print.as_ref() == Some(t) || m.path.as_ref() == Some(t);
    exact.len() == texts.len() && exact.iter().zip(texts).all(|(m, writ)| matches!(writ, Writ::Text(t) if names(m, t)))
}

/// A version 1 member: a path or a placeholder text, at the place a move recorded else the one it names, or
/// a print at its last path, given up where another member is named by it, as 1.5.6 records the copy it found.
fn former(text: &str, recorded: &HashMap<String, String>, named: &HashSet<String>) -> Member {
    let spelt = |path: &str| paths::normal(path).into_owned();
    let own = recorded.get(text).map(String::as_str);
    if !is_print(text) {
        let path = own.filter(|p| !p.is_empty()).or_else(|| place(text)).map(spelt);
        return Member { path, print: waits(text).then(|| text.to_owned()) };
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

/// Where a text that is no print stands: itself as a path, or the path after the placeholder's mark,
/// as 1.5.6 names a photo still being read.
fn place(text: &str) -> Option<&str> {
    if is_path(text) { Some(text) } else { text.strip_prefix(WAITING_MARK).filter(|p| !p.is_empty()) }
}

