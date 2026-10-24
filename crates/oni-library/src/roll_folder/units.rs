//! A frame as the disk holds it: the photo, an RGB frame's other two passes, the JPEG of a RAW+JPEG
//! pair, each with ONI's sidecar and the sidecars other apps keep beside it. It moves as one.

use super::there;
use crate::gallery_history::Move;
use crate::{paths, rgb_passes};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// One frame's photos, each to its new path; the files beside each follow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub moves: Vec<Move>,
}

/// What ONI and other apps keep beside a photo under its whole name: ONI's sidecar first, so a
/// photo never lands without its settings, then darktable's, RawTherapee's, DxO's and ON1's.
const BY_NAME: &[&str] = &[".json", ".xmp", ".pp3", ".dop", ".on1"];

/// What Lightroom and Capture One keep beside a RAW under its stem.
const BY_STEM: &[&str] = &[".xmp"];

/// The camera RAWs among the importable images: the one half of a RAW+JPEG pair.
const RAW: &[&str] = &[
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "rw2", "raw", "rwl", "orf", "raf", "pef", "srw", "3fr",
    "fff", "iiq", "mos", "mrw", "x3f", "erf", "kdc", "dcr", "mef", "dcs", "gpr",
];

const JPEG: &[&str] = &["jpg", "jpeg"];

fn is(path: &str, kinds: &[&str]) -> bool {
    let ext = paths::extension(path).to_ascii_lowercase();
    !ext.is_empty() && kinds.contains(&ext.as_str())
}

/// A name as a case-insensitive, composed disk compares it.
fn key(name: &str) -> String {
    paths::composed(name).to_lowercase()
}

/// A folder's names as the disk spells them, found by `key`.
#[derive(Default)]
struct Listings(HashMap<String, HashMap<String, String>>);

impl Listings {
    fn of(&mut self, folder: &str) -> &HashMap<String, String> {
        self.0.entry(folder.to_owned()).or_insert_with(|| {
            let names = std::fs::read_dir(folder).map(|l| l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>());
            names.unwrap_or_default().into_iter().map(|n| (key(&n), n)).collect()
        })
    }

    /// The files beside `from` that follow it to `to`, each as (from, to), in the order they move.
    fn beside(&mut self, from: &str, to: &str) -> Vec<(String, String)> {
        let (folder, into) = (paths::parent(from), paths::parent(to));
        let (name, new_name) = (paths::name(from), paths::name(to));
        let raw = is(from, RAW) && !Path::new(from).is_dir();
        let (stem, new_stem) = (paths::stem(from), paths::stem(to));
        let listing = self.of(&folder);
        let mut out = Vec::new();
        let mut follow = |base: &str, new_base: &str, suffix: &str| {
            if let Some(actual) = listing.get(&key(&format!("{base}{suffix}"))) {
                // The suffix keeps the disk's own spelling, `.XMP` as `.xmp`.
                let spelt = paths::composed(actual);
                let tail = &spelt[spelt.len().saturating_sub(suffix.len())..];
                let pair = (paths::join(&folder, actual), paths::join(&into, &format!("{new_base}{tail}")));
                if !out.contains(&pair) {
                    out.push(pair);
                }
            }
        };
        if !Path::new(from).is_dir() {
            BY_NAME.iter().for_each(|s| follow(&name, &new_name, s));
        }
        if raw {
            BY_STEM.iter().for_each(|s| follow(&stem, &new_stem, s));
        }
        out
    }

    /// The other half of a RAW+JPEG pair beside `photo`, as the disk spells it.
    fn pair(&mut self, photo: &str) -> Vec<String> {
        let wanted: &[&str] = if is(photo, RAW) {
            JPEG
        } else if is(photo, JPEG) {
            RAW
        } else {
            return Vec::new();
        };
        let (folder, stem) = (paths::parent(photo), key(&paths::stem(photo)));
        let mut out: Vec<String> = self
            .of(&folder)
            .values()
            .filter(|n| key(&paths::stem(n)) == stem && is(n, wanted))
            .map(|n| paths::join(&folder, &paths::composed(n)))
            .collect();
        out.sort();
        out
    }
}

/// Every file that moves with `units`: the photos and every file beside them.
pub fn files(units: &[Unit]) -> usize {
    units.iter().map(|u| steps(u).len()).sum()
}

/// Every move a unit makes, the files beside each photo before it, each file once: a stem's
/// `.xmp` shared by two photos of the frame moves with the first.
pub(super) fn steps(unit: &Unit) -> Vec<(String, String)> {
    let mut listings = Listings::default();
    let mut once = HashSet::new();
    unit.moves
        .iter()
        .flat_map(|m| {
            let mut s = listings.beside(&m.from, &m.to);
            s.push((m.from.clone(), m.to.clone()));
            s
        })
        .filter(|(from, _)| once.insert(key(from)))
        .collect()
}

/// Each item into `folder` as one frame with what goes with it. Passes keep their names or stay;
/// any other frame is numbered past every name taken there, sidecars' included: `P107 2.RW2`.
pub fn plan_units(items: &[String], folder: &str) -> Vec<Unit> {
    let mut listings = Listings::default();
    let mut frames: Vec<(Vec<String>, bool)> = Vec::new();
    // Paths compared as the disk compares them, so a pair's JPEG picked too is one file.
    let mut seen: HashSet<String> = HashSet::new();
    for item in items {
        if seen.contains(&key(item)) {
            continue;
        }
        let passes = rgb_passes::group(item);
        let pinned = passes.is_some();
        let mut members = passes.unwrap_or_else(|| vec![item.clone()]);
        for m in members.clone() {
            let partners: Vec<String> = listings.pair(&m).into_iter().filter(|p| !members.iter().any(|q| key(q) == key(p))).collect();
            members.extend(partners);
        }
        members.retain(|m| there(m) && !seen.contains(&key(m)));
        if members.is_empty() {
            continue;
        }
        seen.extend(members.iter().map(|m| key(m)));
        frames.push((members, pinned));
    }
    let mut claimed: HashSet<String> = HashSet::new();
    let taken = |names: &[String], claimed: &HashSet<String>| names.iter().any(|n| claimed.contains(&key(n)) || there(&paths::join(folder, n)));
    let mut out = Vec::new();
    for (members, pinned) in frames {
        let mut rank = 1;
        let chosen = loop {
            let moves: Vec<Move> = members.iter().map(|m| Move { from: m.clone(), to: paths::join(folder, &numbered(m, rank)) }).collect();
            let names: Vec<String> = moves
                .iter()
                .flat_map(|m| std::iter::once(m.to.clone()).chain(listings.beside(&m.from, &m.to).into_iter().map(|(_, to)| to)))
                .map(|to| paths::name(&to))
                .collect();
            if !taken(&names, &claimed) {
                break Some((moves, names));
            }
            if pinned {
                break None;
            }
            rank += 1;
        };
        if let Some((moves, names)) = chosen {
            claimed.extend(names.iter().map(|n| key(n)));
            out.push(Unit { moves });
        }
    }
    out
}

/// `name` past rank 1 as the Finder numbers a copy: `Tokyo 2`, `P107 2.RW2`.
fn numbered(item: &str, rank: usize) -> String {
    let file = paths::name(item);
    if rank == 1 {
        return file;
    }
    let ext = if Path::new(item).is_dir() { String::new() } else { paths::extension(&file) };
    if ext.is_empty() { format!("{file} {rank}") } else { format!("{} {rank}.{ext}", paths::stem(&file)) }
}
