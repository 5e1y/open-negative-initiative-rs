//! Adobe DNG Converter, the reader of Nikon's High Efficiency RAW that LibRaw lacks: found where its
//! installer puts it, its pinned installer fetched from Adobe and opened, a folder's frames converted.

use oni_i18n::tr;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// The installer ONI fetches, about 1.8 GB; Adobe keeps each version's link, so this one never moves.
#[cfg(target_os = "macos")]
pub const INSTALLER: &str = "https://download.adobe.com/pub/adobe/dng/mac/DNGConverter_18_0.dmg";
#[cfg(target_os = "windows")]
pub const INSTALLER: &str = "https://download.adobe.com/pub/adobe/dng/win/AdobeDNGConverter_x64_18_0.exe";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const INSTALLER: &str = "";
/// Files handed to one run: few enough that the count moves, enough that its start-up is shared.
const BATCH: usize = 4;

/// The converter's program: `ONI_DNG_CONVERTER` for the checks, else where Adobe's installer puts it.
pub fn installed() -> Option<PathBuf> {
    if let Some(program) = std::env::var_os("ONI_DNG_CONVERTER") {
        return Some(PathBuf::from(program)).filter(|p| p.is_file());
    }
    candidates().into_iter().find(|p| p.is_file())
}

#[cfg(target_os = "macos")]
fn candidates() -> Vec<PathBuf> {
    let program = "Adobe DNG Converter.app/Contents/MacOS/Adobe DNG Converter";
    let home = std::env::var_os("HOME").map(|h| Path::new(&h).join("Applications"));
    [Some(PathBuf::from("/Applications")), home].into_iter().flatten().map(|a| a.join(program)).collect()
}

#[cfg(target_os = "windows")]
fn candidates() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|d| Path::new(&d).join("Adobe").join("Adobe DNG Converter").join("Adobe DNG Converter.exe"))
        .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn candidates() -> Vec<PathBuf> {
    Vec::new()
}

/// The DNG Adobe writes for `raw`: its name with `.dng`, beside it.
pub fn twin(raw: &Path) -> PathBuf {
    raw.with_extension("dng")
}

fn has_twin(raw: &Path) -> bool {
    twin(raw).exists() || raw.with_extension("DNG").exists()
}

fn is_nef(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("nef"))
}

/// Each NEF's answer, by its path and size: a folder read again probes no file twice.
static PROBED: Mutex<Option<HashMap<(String, u64), bool>>> = Mutex::new(None);

/// `oni_decode::is_nikon_he`, remembered for the file as it stands.
pub fn is_nikon_he(path: &Path) -> bool {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let key = (path.to_string_lossy().into_owned(), size);
    if let Some(known) = PROBED.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|p| p.get(&key).copied()) {
        return known;
    }
    let he = oni_decode::is_nikon_he(path);
    PROBED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(key, he);
    he
}

/// The High Efficiency NEFs among `paths` with no DNG beside them yet, in their order.
pub fn convertible<P: AsRef<Path>>(paths: &[P]) -> Vec<PathBuf> {
    paths.iter().map(|p| p.as_ref()).filter(|p| is_nef(p) && !has_twin(p) && is_nikon_he(p)).map(Path::to_path_buf).collect()
}

/// A folder's listing without the High Efficiency NEFs whose DNG it lists: the DNG is the photo.
pub fn superseded(images: &mut Vec<String>) {
    let listed: HashSet<String> = images.iter().map(|p| p.to_lowercase()).collect();
    images.retain(|p| {
        let path = Path::new(p);
        !(is_nef(path) && listed.contains(&twin(path).to_string_lossy().to_lowercase()) && is_nikon_he(path))
    });
}

/// Each file converted to a lossless DNG 1.4 beside it, its mosaic kept; `going` hears how many are
/// done and stops the rest by answering false. A file whose DNG already stands is never handed over.
pub fn convert(converter: &Path, raws: &[PathBuf], going: &dyn Fn(usize) -> bool) -> Result<Vec<PathBuf>, String> {
    let mut converted = Vec::new();
    let mut said = None;
    // Each run writes into one folder: a batch never spans two.
    let mut batches: Vec<&[PathBuf]> = Vec::new();
    for folder in raws.chunk_by(|a, b| a.parent() == b.parent()) {
        batches.extend(folder.chunks(BATCH));
    }
    for batch in batches {
        let fresh: Vec<&PathBuf> = batch.iter().filter(|r| !has_twin(r)).collect();
        if let Some(folder) = fresh.first().and_then(|r| r.parent()) {
            let mut run = Command::new(converter);
            run.args(["-c", "-dng1.4", "-p1", "-d"]).arg(folder).args(&fresh);
            quiet(&mut run);
            if let Err(e) = run.output() {
                said = Some(e.to_string());
            }
        }
        converted.extend(batch.iter().filter(|r| has_twin(r)).cloned());
        if !going(converted.len()) {
            break;
        }
    }
    match said {
        Some(e) if converted.is_empty() => Err(tr!("Adobe DNG Converter could not run: {error}", error = e)),
        _ if converted.is_empty() && !raws.is_empty() => Err(tr("Adobe DNG Converter wrote no DNG.").into()),
        _ => Ok(converted),
    }
}

/// A console program run from a window opens no console of its own on Windows.
fn quiet(command: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = command;
}

/// The pinned installer downloaded into `folder`, `going` told its bytes and stopping it on false;
/// one already whole there, from an install left before its end, is taken again.
pub fn fetch(folder: &Path, going: &dyn Fn(u64, Option<u64>) -> bool) -> Result<PathBuf, String> {
    let kept = INSTALLER.rsplit('/').next().map(|name| folder.join(name)).filter(|p| p.is_file());
    match kept {
        Some(kept) => Ok(kept),
        None => crate::web::download_counted(INSTALLER, folder, going),
    }
}

/// The installer opened once its signature is Adobe's; the person goes through Adobe's licence and
/// the system's password, and this answers when the installer has closed.
#[cfg(target_os = "macos")]
pub fn run_installer(dmg: &Path) -> Result<(), String> {
    let mount = dmg.with_extension("mounted");
    std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    let attached = Command::new("/usr/bin/hdiutil").args(["attach", "-nobrowse", "-readonly", "-mountpoint"]).arg(&mount).arg(dmg).status();
    if !attached.is_ok_and(|s| s.success()) {
        return Err(tr("the installer's disk image cannot be opened").into());
    }
    let opened = (|| {
        let pkg = std::fs::read_dir(&mount)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "pkg"))
            .ok_or(tr("the disk image holds no installer"))?;
        let checked = Command::new("/usr/sbin/pkgutil").arg("--check-signature").arg(&pkg).output().map_err(|e| e.to_string())?;
        if !(checked.status.success() && adobe_signed(&String::from_utf8_lossy(&checked.stdout))) {
            return Err(tr("the installer is not signed by Adobe").into());
        }
        let shown = Command::new("/usr/bin/open").args(["-W", "-a", "Installer"]).arg(&pkg).status().map_err(|e| e.to_string())?;
        if shown.success() { Ok(()) } else { Err(tr("the installer cannot be opened").into()) }
    })();
    let _ = Command::new("/usr/bin/hdiutil").args(["detach", "-quiet"]).arg(&mount).status();
    let _ = std::fs::remove_dir(&mount);
    opened
}

/// `pkgutil --check-signature`'s report of a package Apple notarised and Adobe signed.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn adobe_signed(report: &str) -> bool {
    report.contains("Status: signed by a developer certificate issued by Apple")
        && report.contains("Notarization: trusted by the Apple notary service")
        && report.contains("Developer ID Installer: Adobe Inc. (JQ525L2MZD)")
}

/// The installer run once Windows finds Adobe's valid signature on it; its own elevation asks.
#[cfg(target_os = "windows")]
pub fn run_installer(exe: &Path) -> Result<(), String> {
    let quoted = exe.to_string_lossy().replace('\'', "''");
    let check = format!("$s = Get-AuthenticodeSignature -LiteralPath '{quoted}'; if ($s.Status -eq 'Valid' -and $s.SignerCertificate.Subject -match 'O=Adobe Inc\\.') {{ exit 0 }} else {{ exit 1 }}");
    let mut checking = Command::new("powershell");
    checking.args(["-NoProfile", "-NonInteractive", "-Command", &check]);
    quiet(&mut checking);
    if !checking.status().is_ok_and(|s| s.success()) {
        return Err(tr("the installer is not signed by Adobe").into());
    }
    let mut running = Command::new("powershell");
    running.args(["-NoProfile", "-NonInteractive", "-Command", &format!("Start-Process -FilePath '{quoted}' -Wait")]);
    quiet(&mut running);
    if running.status().is_ok_and(|s| s.success()) { Ok(()) } else { Err(tr("the installer was not run").into()) }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn run_installer(_: &Path) -> Result<(), String> {
    Err(tr("Adobe DNG Converter runs on macOS and Windows only").into())
}

#[cfg(test)]
mod tests;
