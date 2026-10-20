//! An ICC profile a file is converted into: reading its header and name, listing what is
//! installed, and installing what a maker ships.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use moxcms::{ColorProfile, ParsingOptions, ProfileText};

/// An ICC profile a file is converted into, identified by its path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutputProfile {
    pub path: String,
    pub name: String,
}

/// A profile read whole: its bytes and their parse, built once per path.
pub struct Loaded {
    pub data: Vec<u8>,
    pub profile: ColorProfile,
}

/// A paper profile runs to 10 MB and past it, a size a CMM's default ceiling refuses.
pub fn parsing_options() -> ParsingOptions {
    ParsingOptions {
        max_profile_size: 64 * 1024 * 1024,
        max_allowed_clut_size: 64 * 1024 * 1024,
        ..Default::default()
    }
}

/// Parses a profile and, as the ICC rule asks, reads the colorimetric tables from the perceptual
/// ones where the profile carries only those.
pub fn parse(data: &[u8]) -> Option<ColorProfile> {
    let mut profile = ColorProfile::new_from_slice_with_options(data, parsing_options()).ok()?;
    if profile.lut_a_to_b_colorimetric.is_none() {
        profile.lut_a_to_b_colorimetric = profile.lut_a_to_b_perceptual.clone();
    }
    if profile.lut_b_to_a_colorimetric.is_none() {
        profile.lut_b_to_a_colorimetric = profile.lut_b_to_a_perceptual.clone();
    }
    Some(profile)
}

static SPACES: LazyLock<Mutex<HashMap<String, Arc<Loaded>>>> = LazyLock::new(Default::default);
static SPACES_MADE: AtomicUsize = AtomicUsize::new(0);
/// The first 128 bytes of a file, read once: the export preference is decoded on every read.
static HEADERS: LazyLock<Mutex<HashMap<String, Vec<u8>>>> = LazyLock::new(Default::default);
/// Read once per path: naming a 10 MB paper profile on each decode freezes the interface.
static NAMES: LazyLock<Mutex<HashMap<String, Option<String>>>> = LazyLock::new(Default::default);
static READS: AtomicUsize = AtomicUsize::new(0);
static HELD: LazyLock<Mutex<Vec<OutputProfile>>> = LazyLock::new(Default::default);

/// The path with `.` and `..` resolved lexically, as a file URL standardises it.
pub fn standardized(path: &Path) -> String {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().into_owned()
}

impl OutputProfile {
    pub fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        let fallback = Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = Self::description(Path::new(&path)).unwrap_or(fallback);
        OutputProfile { path, name }
    }

    pub fn id(&self) -> &str {
        &self.path
    }

    /// `None` once gone or unreadable, which an export reports instead of writing sRGB.
    pub fn loaded(&self) -> Option<Arc<Loaded>> {
        if !Path::new(&self.path).exists() {
            return None;
        }
        if let Some(held) = SPACES.lock().unwrap().get(&self.path) {
            return Some(held.clone());
        }
        let data = fs::read(&self.path).ok()?;
        if !Self::is_rgb_output(&data) {
            return None;
        }
        let profile = parse(&data)?;
        let made = Arc::new(Loaded { data, profile });
        SPACES.lock().unwrap().insert(self.path.clone(), made.clone());
        SPACES_MADE.fetch_add(1, Ordering::Relaxed);
        Some(made)
    }

    pub fn spaces_made() -> usize {
        SPACES_MADE.load(Ordering::Relaxed)
    }

    pub fn names_read() -> usize {
        READS.load(Ordering::Relaxed)
    }

    fn header_bytes(path: &Path) -> Option<Vec<u8>> {
        let key = standardized(path);
        if !Path::new(&key).exists() {
            return None;
        }
        if let Some(held) = HEADERS.lock().unwrap().get(&key) {
            return Some(held.clone());
        }
        let mut bytes = Vec::with_capacity(128);
        fs::File::open(&key).ok()?.take(128).read_to_end(&mut bytes).ok()?;
        HEADERS.lock().unwrap().insert(key, bytes.clone());
        Some(bytes)
    }

    /// Reads the 128-byte header alone: a paper profile runs to 10 MB.
    pub fn is_usable(&self) -> bool {
        Self::is_usable_at(Path::new(&self.path))
    }

    pub fn is_usable_at(path: &Path) -> bool {
        Self::header_bytes(path).is_some_and(|h| Self::is_rgb_output(&h))
    }

    // MARK: - Reading a profile

    /// The header's device class and data colour space, bytes 12 to 19.
    pub fn header(data: &[u8]) -> Option<(String, String)> {
        if data.len() < 128 || &data[36..40] != b"acsp" {
            return None;
        }
        let text = |r: std::ops::Range<usize>| String::from_utf8_lossy(&data[r]).into_owned();
        Some((text(12..16), text(16..20)))
    }

    /// A printer, display or colour-space profile on RGB data. CMYK is refused: nothing renders it.
    pub fn is_rgb_output(data: &[u8]) -> bool {
        Self::header(data).is_some_and(|(class, space)| {
            space == "RGB " && ["prtr", "mntr", "spac"].contains(&class.as_str())
        })
    }

    /// What the Custom list offers: a printer or paper profile on RGB data.
    pub fn is_printer(data: &[u8]) -> bool {
        Self::header(data).is_some_and(|(class, space)| space == "RGB " && class == "prtr")
    }

    pub fn is_printer_at(path: &Path) -> bool {
        Self::header_bytes(path).is_some_and(|h| Self::is_printer(&h))
    }

    fn description(path: &Path) -> Option<String> {
        let key = standardized(path);
        if let Some(held) = NAMES.lock().unwrap().get(&key) {
            return held.clone();
        }
        READS.fetch_add(1, Ordering::Relaxed);
        let name = fs::read(&key).ok().and_then(|d| description_of(&d));
        NAMES.lock().unwrap().insert(key, name.clone());
        name
    }

    // MARK: - What is installed

    /// Where profiles live: the user's, the machine's, the system's and the printer drivers'.
    pub fn roots(user: &Path) -> Vec<PathBuf> {
        let mut roots = vec![user.to_path_buf()];
        if cfg!(target_os = "macos") {
            roots.extend(
                ["/Library/ColorSync/Profiles", "/System/Library/ColorSync/Profiles", "/Library/Printers"]
                    .map(PathBuf::from),
            );
        } else if cfg!(target_os = "windows") {
            let system = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
            roots.push(Path::new(&system).join(r"System32\spool\drivers\color"));
        } else {
            roots.push(PathBuf::from("/usr/share/color/icc"));
        }
        roots
    }

    pub fn last_installed() -> Vec<OutputProfile> {
        HELD.lock().unwrap().clone()
    }

    /// Every RGB printer profile in the folders, sorted by name; `remember` keeps the list for
    /// `last_installed`, which a walk of a check's own folder must not overwrite.
    pub fn installed(folders: &[PathBuf], remember: bool) -> Vec<OutputProfile> {
        let mut paths = Vec::new();
        for folder in folders {
            walk(folder, &mut paths);
        }
        let mut seen = HashSet::new();
        let mut listed: Vec<OutputProfile> = paths
            .iter()
            .map(|p| standardized(p))
            .filter(|p| seen.insert(p.clone()))
            .filter(|p| Self::is_printer_at(Path::new(p)))
            .map(OutputProfile::new)
            .collect();
        listed.sort_by(|a, b| oni_settings::collation::case_insensitive(&a.name, &b.name));
        if remember {
            *HELD.lock().unwrap() = listed.clone();
        }
        listed
    }

    // MARK: - Installing a profile the hand brings

    /// Installs every RGB printer profile in the files, bare or zipped, unpacking archives under
    /// `scratch`. An identical file already installed is reused.
    pub fn importing(paths: &[PathBuf], folder: &Path, scratch: &Path) -> (Vec<OutputProfile>, usize) {
        let work = scratch.join(format!("profile-import-{}", unique()));
        if fs::create_dir_all(&work).is_err() {
            return (vec![], paths.len());
        }
        let _ = fs::create_dir_all(folder);
        let mut candidates = Vec::new();
        for path in paths {
            collect(path, &mut candidates, &work, 0);
        }
        let mut installed = Vec::new();
        let mut refused = 0;
        for candidate in candidates {
            let placed = fs::read(&candidate).ok().filter(|d| Self::is_printer(d) && parse(d).is_some()).and_then(|d| {
                let name = candidate.file_name()?.to_string_lossy().into_owned();
                place(&d, &name, folder)
            });
            match placed {
                Some(p) => installed.push(OutputProfile::new(standardized(&p))),
                None => refused += 1,
            }
        }
        let _ = fs::remove_dir_all(&work);
        (installed, refused)
    }
}

/// The profile's own name: the English one where it carries several.
pub fn description_of(data: &[u8]) -> Option<String> {
    let text = ColorProfile::new_from_slice_with_options(data, parsing_options()).ok()?.description?;
    let name = match text {
        ProfileText::PlainString(s) => s,
        ProfileText::Localizable(list) => {
            let english = list.iter().find(|s| s.language == "en").or(list.first())?;
            english.value.clone()
        }
        ProfileText::Description(d) if !d.unicode_string.is_empty() => d.unicode_string,
        ProfileText::Description(d) => d.ascii_string,
    };
    let name = name.trim_end_matches('\0').trim().to_string();
    (!name.is_empty()).then_some(name)
}

fn walk(folder: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(&path, into),
            Ok(_) if has_extension(&path, &["icc", "icm"]) => into.push(path),
            _ => {}
        }
    }
}

fn has_extension(path: &Path, any: &[&str]) -> bool {
    path.extension().is_some_and(|e| any.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

pub(crate) fn unique() -> String {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:x}{:x}{:x}", std::process::id(), nanos, COUNT.fetch_add(1, Ordering::Relaxed))
}

/// `depth` counts archives only, so nested zips stop at four.
fn collect(path: &Path, found: &mut Vec<PathBuf>, scratch: &Path, depth: usize) {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if name.starts_with("._") || name == "__MACOSX" || !path.exists() {
        return;
    }
    if path.is_dir() {
        let mut children: Vec<PathBuf> =
            fs::read_dir(path).map(|e| e.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        children.sort();
        for child in children {
            collect(&child, found, scratch, depth);
        }
        return;
    }
    if has_extension(path, &["icc", "icm"]) {
        found.push(path.to_path_buf());
    } else if has_extension(path, &["zip"]) && depth < 4 {
        let target = scratch.join(unique());
        if unzip(path, &target) {
            collect(&target, found, scratch, depth + 1);
        }
    }
}

fn unzip(archive: &Path, folder: &Path) -> bool {
    let Ok(file) = fs::File::open(archive) else { return false };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return false };
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else { return false };
        // A name climbing out of the folder is skipped, never written.
        let Some(relative) = entry.enclosed_name() else { continue };
        let out = folder.join(relative);
        if entry.is_dir() {
            let _ = fs::create_dir_all(&out);
            continue;
        }
        if let Some(parent) = out.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let Ok(mut target) = fs::File::create(&out) else { return false };
        if std::io::copy(&mut entry, &mut target).is_err() {
            return false;
        }
    }
    true
}

fn place(data: &[u8], name: &str, folder: &Path) -> Option<PathBuf> {
    let as_path = Path::new(name);
    let stem = as_path.file_stem()?.to_string_lossy().into_owned();
    let ext = as_path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    for n in 1..=99 {
        let candidate = folder.join(if n == 1 { name.to_string() } else { format!("{stem} {n}.{ext}") });
        match fs::read(&candidate) {
            Ok(held) if held == data => return Some(candidate),
            Ok(_) => continue,
            Err(_) => {
                let temporary = folder.join(format!(".{}.part", unique()));
                let laid = fs::write(&temporary, data).and_then(|_| fs::rename(&temporary, &candidate));
                if laid.is_err() {
                    let _ = fs::remove_file(&temporary);
                    return None;
                }
                return Some(candidate);
            }
        }
    }
    None
}
