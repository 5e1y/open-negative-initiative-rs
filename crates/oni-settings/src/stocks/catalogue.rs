//! The catalogue the files build: every stock a roll names, every virtual film, dye set and paper, each by its key, in
//! the order the menus list them. A file whose keys clash or whose names reach nothing is refused, never half taken.

use super::file::{Describes, File, FilmBlock};
use super::order::natural;
use super::{Loaded, Origin, Refusal, intern};
use super::entries::*;
use crate::dyes::DyeSet;
use crate::film::{FilmStockKind, FilmType};
use crate::paper::PaperStock;
use crate::virtual_film::VirtualStock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as Atomic};

/// One space's keys: each entry's place, and each retired key's entry.
#[derive(Debug, Default)]
struct Names {
    index: HashMap<&'static str, usize>,
    aliases: HashMap<&'static str, &'static str>,
}

#[derive(Debug, Default)]
pub struct Catalogue {
    films: Vec<FilmStockKind>,
    virtuals: Vec<VirtualEntry>,
    dyes: Vec<DyeEntry>,
    papers: Vec<PaperEntry>,
    canisters: HashMap<String, Canister>,
    names: [Names; 4],
    /// Each build's own number: a menu or a drawn canister laid from an older one is laid again.
    pub revision: u64,
}

/// A key a file claims, with the file it comes from.
type Claim = (Space, String);

impl Catalogue {
    /// The catalogue of `files` and `svgs`: a file of the person's stands in for the shipped one of its key; one clashing,
    /// dropping a key its shipped file defined or naming what no file defines is refused, with why.
    pub fn build(files: Vec<Loaded>, svgs: Vec<(String, Origin, Arc<[u8]>)>) -> (Catalogue, Vec<Refusal>) {
        let mut refused = Vec::new();
        let own = |l: &Loaded| l.origin != Origin::Shipped;
        let replaced: Vec<(Describes, String)> = files.iter().filter(|l| own(l)).map(|l| (describes(&l.file), l.file.key().to_owned())).collect();
        let (mut stood, mut taken): (Vec<Loaded>, Vec<Loaded>) =
            files.into_iter().partition(|l| !own(l) && replaced.contains(&(describes(&l.file), l.file.key().to_owned())));
        taken.sort_by(|a, b| own(a).cmp(&own(b)).then_with(|| a.name.cmp(&b.name)));
        // A file standing for a shipped one keeps every key that one defined: other stocks name them.
        taken.retain(|l| {
            let Some(at) = stood.iter().position(|s| own(l) && describes(&s.file) == describes(&l.file) && s.file.key() == l.file.key()) else { return true };
            let shipped = stood.remove(at);
            let kept = claims(&l.file);
            match claims(&shipped.file).into_iter().find(|c| !kept.contains(c)) {
                Some((_, key)) => {
                    refused.push(Refusal { name: l.name.clone(), reason: format!("it drops the key \"{key}\" the shipped {} defines", shipped.name) });
                    stood.push(shipped);
                    false
                }
                None => true,
            }
        });
        taken.extend(stood);
        taken.sort_by(|a, b| own(a).cmp(&own(b)).then_with(|| a.name.cmp(&b.name)));
        let mut canisters: HashMap<String, Canister> = HashMap::new();
        let mut svgs = svgs;
        svgs.sort_by_key(|(_, origin, _)| *origin != Origin::Shipped);
        for (name, _, bytes) in svgs {
            let hash = super::hash(&bytes);
            canisters.insert(name, Canister { bytes, hash });
        }
        loop {
            match assemble(&taken) {
                Ok(mut c) => {
                    c.canisters = canisters;
                    static BUILDS: AtomicU64 = AtomicU64::new(1);
                    c.revision = BUILDS.fetch_add(1, Atomic::Relaxed);
                    return (c, refused);
                }
                Err((at, reason)) => {
                    let l = taken.remove(at);
                    refused.push(Refusal { name: l.name, reason });
                }
            }
        }
    }

    /// Every stock a roll names, in the menus' order: the generic ones, then by brand and name.
    pub fn films(&self) -> &[FilmStockKind] {
        &self.films
    }

    pub fn virtuals(&self) -> &[VirtualEntry] {
        &self.virtuals
    }

    pub fn dye_sets(&self) -> &[DyeEntry] {
        &self.dyes
    }

    pub fn papers(&self) -> &[PaperEntry] {
        &self.papers
    }

    pub fn film(&self, key: &str) -> Option<&FilmStockKind> {
        self.at(Space::Film, key).map(|i| &self.films[i])
    }

    pub fn virtual_film(&self, stock: VirtualStock) -> Option<&VirtualEntry> {
        self.at(Space::Virtual, stock.key()).map(|i| &self.virtuals[i])
    }

    pub fn dye_set(&self, set: DyeSet) -> Option<&DyeEntry> {
        self.at(Space::Dyes, set.key()).map(|i| &self.dyes[i])
    }

    pub fn paper(&self, paper: PaperStock) -> Option<&PaperEntry> {
        self.at(Space::Paper, paper.key()).map(|i| &self.papers[i])
    }

    pub fn canister(&self, name: &str) -> Option<&Canister> {
        self.canisters.get(name)
    }

    /// The key a stored `key` reads as: its entry's where it names one, by itself or retired, else itself, kept so a
    /// file brought back finds its photos.
    pub fn canonical(&self, space: Space, key: &str) -> &'static str {
        let names = &self.names[space as usize];
        if let Some((k, _)) = names.index.get_key_value(key) {
            return k;
        }
        names.aliases.get(key).copied().unwrap_or_else(|| intern(key))
    }

    fn at(&self, space: Space, key: &str) -> Option<usize> {
        let names = &self.names[space as usize];
        names.index.get(key).or_else(|| names.index.get(names.aliases.get(key)?)).copied()
    }
}

fn describes(file: &File) -> Describes {
    match file {
        File::Film(_) => Describes::Film,
        File::Paper(_) => Describes::Paper,
    }
}

/// Every key a file defines, each in its space, retired ones included.
fn claims(file: &File) -> Vec<Claim> {
    fn claim(out: &mut Vec<Claim>, space: Space, key: &str, aliases: &[String]) {
        out.push((space, key.to_owned()));
        out.extend(aliases.iter().map(|a| (space, a.clone())));
    }
    fn block(out: &mut Vec<Claim>, b: &FilmBlock) {
        claim(out, Space::Virtual, &b.key, &b.aliases);
        if b.dyes == b.key {
            claim(out, Space::Dyes, &b.key, &b.aliases);
        }
    }
    let mut out = Vec::new();
    match file {
        File::Film(f) => {
            claim(&mut out, Space::Film, &f.key, &f.aliases);
            f.film.iter().for_each(|b| block(&mut out, b));
            for d in &f.developments {
                claim(&mut out, Space::Film, &d.key, &d.aliases);
                d.film.iter().for_each(|b| block(&mut out, b));
            }
            if let Some(m) = &f.dye_set {
                claim(&mut out, Space::Dyes, &m.key, &m.aliases);
            }
        }
        File::Paper(p) => claim(&mut out, Space::Paper, &p.key, &p.aliases),
    }
    out
}

/// The catalogue of `files` taken whole, or the place of the first file to refuse and why.
fn assemble(files: &[Loaded]) -> Result<Catalogue, (usize, String)> {
    let mut c = Catalogue::default();
    let mut defined: HashMap<Claim, usize> = HashMap::new();
    for (at, l) in files.iter().enumerate() {
        let wanted = claims(&l.file);
        for (n, claim) in wanted.iter().enumerate() {
            if let Some(other) = defined.get(claim) {
                return Err((at, format!("it defines the key \"{}\", as {} does", claim.1, files[*other].name)));
            }
            if wanted[..n].contains(claim) {
                return Err((at, format!("it defines the key \"{}\" twice", claim.1)));
            }
        }
        defined.extend(wanted.into_iter().map(|c| (c, at)));
    }
    let mut means: Vec<(usize, &'static str, Vec<String>)> = Vec::new();
    for (at, l) in files.iter().enumerate() {
        match &l.file {
            File::Film(f) => {
                let missing = |space: Space, key: &str| format!("it names \"{key}\", which no file defines as {}", match space {
                    Space::Dyes => "a dye set",
                    Space::Virtual => "a virtual film",
                    Space::Paper => "a paper",
                    Space::Film => "a stock",
                });
                let parent = FilmStockKind {
                    id: intern(&f.key),
                    display_name: intern(&f.name),
                    brand: intern(&f.brand),
                    iso: f.iso,
                    kind: f.kind,
                    offered: f.offered,
                    line: f.line,
                    generic: f.generic,
                    canister: f.canister.as_deref().map(intern),
                    dyes: None,
                    film: None,
                };
                let stocks = std::iter::once((&f.key, &f.name, f.iso, &f.dyes, &f.film))
                    .chain(f.developments.iter().map(|d| (&d.key, &d.name, d.iso, &d.dyes, &d.film)));
                for (key, name, iso, dyes, film) in stocks {
                    let dyes = match dyes {
                        Some(d) => Some(DyeSet::of(canonical_in(&defined, files, Space::Dyes, d).ok_or_else(|| (at, missing(Space::Dyes, d)))?)),
                        None => None,
                    };
                    let kind = FilmStockKind { id: intern(key), display_name: intern(name), iso, dyes, film: film.as_ref().map(|b| VirtualStock::of(intern(&b.key))), ..parent };
                    if let Some(b) = film {
                        let reference = b.reference.ok_or_else(|| (at, format!("the film \"{}\" holds no reference", b.key)))?;
                        let set = canonical_in(&defined, files, Space::Dyes, &b.dyes).ok_or_else(|| (at, missing(Space::Dyes, &b.dyes)))?;
                        let profile: Arc<str> = Arc::from(b.profile.get());
                        let revision = super::mixed(l.hash, super::hash(b.key.as_bytes()));
                        c.virtuals.push(VirtualEntry {
                            stock: VirtualStock::of(intern(&b.key)),
                            name: kind.display_name,
                            catalogue: kind.id,
                            balance: b.balance,
                            dyes: DyeSet::of(set),
                            reference,
                            profile: profile.clone(),
                            silver: f.kind == FilmType::BlackAndWhite,
                            reversal: f.kind == FilmType::ColourSlide,
                            neutral: b.neutral,
                            couplers: b.couplers,
                            curves: b.curves.as_deref().map(Arc::from),
                            effects: b.effects.unwrap_or_default().rests(),
                            revision,
                        });
                        if b.dyes == b.key {
                            c.dyes.push(DyeEntry { set: DyeSet::of(intern(&b.key)), name: Some(kind.display_name), source: DyeSource::Profile(profile), revision });
                        }
                    }
                    c.films.push(kind);
                }
                if let Some(m) = &f.dye_set {
                    means.push((at, intern(&m.key), m.mean_of.clone()));
                }
            }
            File::Paper(p) => {
                let (look, profile) = (Arc::from(p.look.get()), p.profile.as_ref().map(|r| Arc::from(r.get())));
                c.papers.push(PaperEntry { paper: PaperStock::of(intern(&p.key)), name: intern(&p.name), brand: intern(&p.brand), look, profile, revision: l.hash });
            }
        }
    }
    for (at, key, of) in means {
        let mut revision = super::hash(key.as_bytes());
        let mut sets = Vec::new();
        for film in &of {
            let named = canonical_in(&defined, files, Space::Virtual, film).and_then(|k| c.virtuals.iter().find(|v| v.stock.key() == k));
            let own = named.and_then(|v| c.dyes.iter().find(|d| d.set == v.dyes && matches!(d.source, DyeSource::Profile(_))));
            let Some(own) = own else { return Err((at, format!("its mean names \"{film}\", which is no colour film measuring its own dyes"))) };
            revision = super::mixed(revision, own.revision);
            sets.push(own.set);
        }
        c.dyes.push(DyeEntry { set: DyeSet::of(key), name: None, source: DyeSource::Mean(sets), revision });
    }
    c.films.sort_by(|a, b| (!a.generic).cmp(&!b.generic).then_with(|| natural(a.brand, b.brand)).then_with(|| natural(a.display_name, b.display_name)));
    let place = |key: &str| c.films.iter().position(|k| k.id == key);
    c.virtuals.sort_by_key(|v| place(v.catalogue));
    let mut dyes = std::mem::take(&mut c.dyes);
    dyes.sort_by_key(|d| c.films.iter().position(|k| k.dyes == Some(d.set) || k.film.is_some_and(|f| f.key() == d.set.key())));
    c.dyes = dyes;
    c.papers.sort_by(|a, b| natural(a.brand, b.brand).then_with(|| natural(a.name, b.name)));
    for (claim, at) in &defined {
        let (space, key) = claim;
        let names = &mut c.names[*space as usize];
        let entry = entry_key(&files[*at].file, *space, key);
        if entry == key.as_str() {
            continue;
        }
        names.aliases.insert(intern(key), intern(entry));
    }
    let index = |names: &mut Names, keys: &mut dyn Iterator<Item = &'static str>| {
        for (i, k) in keys.enumerate() {
            names.index.insert(k, i);
        }
    };
    let [films, virtuals, dye_sets, papers] = &mut c.names;
    index(films, &mut c.films.iter().map(|k| k.id));
    index(virtuals, &mut c.virtuals.iter().map(|v| v.stock.key()));
    index(dye_sets, &mut c.dyes.iter().map(|d| d.set.key()));
    index(papers, &mut c.papers.iter().map(|p| p.paper.key()));
    Ok(c)
}

/// The key `key` of `space` reads as, once every file claimed its own: its own where an entry has it, its entry's where
/// it is retired, none where no file defines it.
fn canonical_in(defined: &HashMap<Claim, usize>, files: &[Loaded], space: Space, key: &str) -> Option<&'static str> {
    let at = defined.get(&(space, key.to_owned()))?;
    Some(intern(entry_key(&files[*at].file, space, key)))
}

/// The entry's own key in `file` that `key` of `space` names, a retired key's entry included.
fn entry_key<'a>(file: &'a File, space: Space, key: &'a str) -> &'a str {
    let owner = |own: &'a str, aliases: &'a [String]| (own == key || aliases.iter().any(|a| a == key)).then_some(own);
    let found = match (file, space) {
        (File::Paper(p), Space::Paper) => owner(&p.key, &p.aliases),
        (File::Film(f), Space::Film) => owner(&f.key, &f.aliases).or_else(|| f.developments.iter().find_map(|d| owner(&d.key, &d.aliases))),
        (File::Film(f), Space::Virtual | Space::Dyes) => {
            let blocks = f.film.iter().chain(f.developments.iter().filter_map(|d| d.film.as_ref()));
            let mut found = None;
            for b in blocks {
                if space == Space::Virtual || b.dyes == b.key {
                    found = found.or_else(|| owner(&b.key, &b.aliases));
                }
            }
            found.or_else(|| f.dye_set.as_ref().filter(|_| space == Space::Dyes).and_then(|m| owner(&m.key, &m.aliases)))
        }
        _ => None,
    };
    found.unwrap_or(key)
}
