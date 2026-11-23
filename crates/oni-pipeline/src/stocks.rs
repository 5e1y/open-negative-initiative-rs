//! The person's own stocks and papers read beside the shipped ones and baked to be checked: one that fails is refused
//! with why, its version taken before staying in use; a film leaving out its reference takes its profile's.

use crate::film;
use crate::paper::{self, frontier};
use oni_settings::stocks::file::{self, File, FilmBlock};
use oni_settings::stocks::{self, Catalogue, Loaded, Origin, Refusal};
use oni_settings::virtual_film::StockReference;
use oni_settings::{DyeSet, film::FilmType};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What a read of the folder built: the catalogue to put in use, and the refusals not told before.
pub struct Reloaded {
    pub catalogue: Catalogue,
    pub refused: Vec<Told>,
}

/// A file refused, and whether the version of it taken before stays in use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Told {
    pub refusal: Refusal,
    pub earlier_kept: bool,
}

/// The folder's reader, which remembers what each file's content read as and what the catalogue in use took.
pub struct Loader {
    folder: PathBuf,
    shipped: Vec<Loaded>,
    shipped_svgs: Vec<(String, Origin, Arc<[u8]>)>,
    /// Each content's verdict: the references its films derive, or why it is refused.
    checked: HashMap<u64, Result<Vec<StockReference>, String>>,
    /// The bytes each file was last taken with, which stand in while an edit of it is refused.
    in_use: HashMap<PathBuf, Vec<u8>>,
    told: HashSet<Refusal>,
    /// The folder's files and contents as last read: a read finding them alike builds nothing.
    seen: Option<u64>,
}

impl Loader {
    pub fn new(folder: PathBuf) -> Loader {
        let (shipped, shipped_svgs) = stocks::shipped();
        Loader { folder, shipped, shipped_svgs, checked: HashMap::new(), in_use: HashMap::new(), told: HashSet::new(), seen: None }
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The folder read again whole: every `.json` a stock or a paper, every `.cube` a look, every `.svg` a canister,
    /// hidden files left out; none where its files and their contents are as last read.
    pub fn reload(&mut self) -> Option<Reloaded> {
        let mut svgs = self.shipped_svgs.clone();
        let mut candidates: Vec<(PathBuf, Vec<u8>)> = Vec::new();
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&self.folder).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        entries.sort();
        let mut seen = stocks::hash(b"");
        for path in entries {
            let name = file_name(&path);
            if name.starts_with('.') || ![".svg", ".json", ".cube"].iter().any(|e| name.ends_with(e)) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            seen = stocks::hash(&[seen.to_le_bytes().as_slice(), name.as_bytes(), &stocks::hash(&bytes).to_le_bytes()].concat());
            if name.ends_with(".svg") {
                svgs.push((name, Origin::Own(path), Arc::from(bytes)));
            } else {
                candidates.push((path, bytes));
            }
        }
        if self.seen.replace(seen) == Some(seen) {
            return None;
        }
        let mut refused = Vec::new();
        let mut own: Vec<(PathBuf, Vec<u8>, Loaded)> = Vec::new();
        for (path, bytes) in candidates {
            match self.check(&path, &bytes) {
                Ok(l) => own.push((path, bytes, l)),
                Err(reason) => {
                    let earlier = self.previous(&path, &bytes);
                    refused.push(Told { refusal: Refusal { name: file_name(&path), reason }, earlier_kept: earlier.is_some() });
                    own.extend(earlier);
                }
            }
        }
        let catalogue = loop {
            let files = self.shipped.iter().cloned().chain(own.iter().map(|(_, _, l)| l.clone())).collect();
            let (catalogue, left) = Catalogue::build(files, svgs.clone());
            let mut stood_in = false;
            for r in left {
                let mut earlier_kept = false;
                if let Some(at) = own.iter().position(|(p, _, _)| file_name(p) == r.name) {
                    let (path, bytes, _) = own.remove(at);
                    if let Some(l) = self.previous(&path, &bytes) {
                        own.push(l);
                        (stood_in, earlier_kept) = (true, true);
                    }
                }
                refused.push(Told { refusal: r, earlier_kept });
            }
            if !stood_in {
                break catalogue;
            }
        };
        self.in_use = own.into_iter().map(|(p, b, _)| (p, b)).collect();
        let fresh = refused.iter().filter(|r| !self.told.contains(&r.refusal)).cloned().collect();
        self.told = refused.into_iter().map(|r| r.refusal).collect();
        Some(Reloaded { catalogue, refused: fresh })
    }

    /// The version of `path` the catalogue in use took, where it is not `bytes` and still reads.
    fn previous(&mut self, path: &Path, bytes: &[u8]) -> Option<(PathBuf, Vec<u8>, Loaded)> {
        let kept = self.in_use.get(path).filter(|b| b.as_slice() != bytes)?.clone();
        let l = self.check(path, &kept).ok()?;
        Some((path.to_owned(), kept, l))
    }

    /// `bytes` read as the person's file at `path`, each film's profile baked and each paper's print read.
    fn check(&mut self, path: &Path, bytes: &[u8]) -> Result<Loaded, String> {
        if file_name(path).ends_with(".cube") {
            return self.look(path, bytes);
        }
        let hash = stocks::hash(bytes);
        let mut file = file::read(bytes)?;
        let references = match self.checked.get(&hash) {
            Some(verdict) => verdict.clone()?,
            None => {
                let verdict = checked(&file);
                self.checked.insert(hash, verdict.clone());
                verdict?
            }
        };
        if let File::Film(f) = &mut file {
            let blocks = f.film.iter_mut().chain(f.developments.iter_mut().filter_map(|d| d.film.as_mut()));
            for (b, derived) in blocks.zip(references) {
                b.reference.get_or_insert(derived);
            }
        }
        Ok(Loaded { name: file_name(path), origin: Origin::Own(path.to_owned()), file, hash })
    }
}

/// The references a file's films derive, in their order, or why one of its profiles or its print does not read.
impl Loader {
    /// A `.cube` file as a virtual film of its own: the default film's file under the cube's key and title, its look the
    /// cube's, offered to digital photos alone; its grain and its internegative's paper read that film.
    fn look(&self, path: &Path, bytes: &[u8]) -> Result<Loaded, String> {
        let (title, look) = oni_settings::stocks::cube::read(bytes)?;
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let key = look_key(&stem);
        let default = oni_settings::VirtualStock::default().key();
        let mut f = self
            .shipped
            .iter()
            .find_map(|l| match &l.file {
                File::Film(f) if f.film.as_ref().is_some_and(|b| b.key == default) => Some(f.clone()),
                _ => None,
            })
            .ok_or("the default film is not shipped")?;
        (f.key, f.name, f.brand) = (key.clone(), title.unwrap_or(stem).chars().take(120).collect(), "LUT".into());
        (f.aliases, f.generic, f.iso, f.canister, f.offered, f.line, f.developments, f.dye_set) = (vec![], false, None, None, false, false, vec![], None);
        let b = f.film.as_mut().expect("found by its film");
        (b.key, b.aliases, b.look) = (key, vec![], Some(look));
        Ok(Loaded { name: file_name(path), origin: Origin::Own(path.to_owned()), file: File::Film(f), hash: stocks::hash(bytes) })
    }
}

/// The key a look's file name gives: `lut-` and its letters and digits, lower case, any other run a hyphen, so no
/// shipped stock's key is ever taken.
pub fn look_key(stem: &str) -> String {
    let mut key = String::from("lut-");
    for c in stem.chars().flat_map(char::to_lowercase) {
        match c {
            'a'..='z' | '0'..='9' => key.push(c),
            _ if !key.ends_with('-') => key.push('-'),
            _ => {}
        }
    }
    key.trim_end_matches('-').chars().take(80).collect()
}

fn checked(file: &File) -> Result<Vec<StockReference>, String> {
    match file {
        File::Film(f) => {
            let silver = f.kind == FilmType::BlackAndWhite;
            let blocks: Vec<&FilmBlock> = f.film.iter().chain(f.developments.iter().filter_map(|d| d.film.as_ref())).collect();
            blocks
                .into_iter()
                .map(|b| {
                    let couplers = b.couplers.unwrap_or(film::bake::COUPLERS);
                    let (reversal, dyes) = (f.kind == FilmType::ColourSlide, DyeSet::of(stocks::intern(&b.dyes)));
                    film::derive(b.profile.get(), b.balance, silver, reversal, dyes, couplers).map_err(|e| format!("the film \"{}\": {e}", b.key))
                })
                .collect()
        }
        File::Paper(p) => {
            if let Some(profile) = &p.profile {
                paper::validate(profile.get().as_bytes())?;
            }
            if let Some(fit) = &p.frontier {
                frontier::read(fit.get())?;
            }
            Ok(Vec::new())
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}
