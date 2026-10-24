//! Paths as the archive keeps them: one spelling per folder, compared and cut the way Foundation's
//! `NSString` path methods cut them. On Windows they are kept with `/` alone, which every file call
//! takes; on macOS composed (NFC), as exFAT lists `Été` decomposed where its real path is composed.

use std::borrow::Cow;
use std::path::Path;
use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};

mod order;
pub use order::standard_order;

/// The path in the archive's spelling: on Windows each `\` a `/`; on macOS composed, which every Mac
/// volume resolves to the same file; elsewhere as written, the disk telling the two forms apart.
pub fn normal(path: &str) -> Cow<'_, str> {
    if cfg!(windows) && path.contains('\\') {
        return Cow::Owned(path.replace('\\', "/"));
    }
    if cfg!(target_os = "macos") { composed(path) } else { Cow::Borrowed(path) }
}

/// The text in NFC, borrowed when it is composed already, as nearly every name is.
pub fn composed(text: &str) -> Cow<'_, str> {
    if text.is_ascii() || is_nfc_quick(text.chars()) == IsNormalized::Yes {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.nfc().collect())
    }
}

/// `deletingLastPathComponent`: one level up, `/` staying `/`.
pub fn parent(path: &str) -> String {
    let path = normal(path);
    match Path::new(&*path).parent() {
        Some(up) => normal(&up.to_string_lossy()).into_owned(),
        None => path.into_owned(),
    }
}

/// `lastPathComponent`: the last name, `/` naming itself.
pub fn name(path: &str) -> String {
    let path = normal(path);
    match Path::new(&*path).file_name() {
        Some(last) => last.to_string_lossy().into_owned(),
        None => path.into_owned(),
    }
}

/// `pathExtension`, empty for a name without one.
pub fn extension(path: &str) -> String {
    Path::new(&*normal(path)).extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `deletingPathExtension` of the last name.
pub fn stem(path: &str) -> String {
    Path::new(&*normal(path)).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| name(path))
}

/// `appendingPathComponent`: a name read from a sidecar starting with a separator stays inside `folder`.
pub fn join(folder: &str, name: &str) -> String {
    let (folder, name) = (normal(folder), normal(name));
    let name = name.trim_start_matches('/');
    if cfg!(windows) {
        return if folder.is_empty() || folder.ends_with('/') { format!("{folder}{name}") } else { format!("{folder}/{name}") };
    }
    Path::new(&*folder).join(name).to_string_lossy().into_owned()
}

/// Whether `path` is `root` or lies under it; a sibling whose name only starts like `root` is not.
/// On a volume that ignores case, `roll 8` holds what the disk now spells `Roll 8`.
pub fn holds(root: &str, path: &str) -> bool {
    let (root, path) = (normal(root), normal(path));
    under(&root, &path) || (under(&folded(&root), &folded(&path)) && ignores_case(&root))
}

/// Whether two paths name one folder or file: one spelling, or two a case-blind volume takes as one.
pub fn same(a: &str, b: &str) -> bool {
    let (a, b) = (normal(a), normal(b));
    a == b || (folded(&a) == folded(&b) && ignores_case(&a))
}

fn under(root: &str, path: &str) -> bool {
    if path == root {
        return true;
    }
    if root.ends_with('/') {
        return path.starts_with(root);
    }
    path.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
}

fn folded(text: &str) -> Cow<'_, str> {
    if text.chars().any(char::is_uppercase) { Cow::Owned(text.to_lowercase()) } else { Cow::Borrowed(text) }
}

/// What lies below `root` in `path`, as `path` spells it; `None` outside it. Cut by names, never
/// by bytes, as the two may differ in case or composition.
pub fn below(root: &str, path: &str) -> Option<String> {
    if !holds(root, path) {
        return None;
    }
    let (root, path) = (normal(root), normal(path));
    let depth = root.trim_end_matches('/').split('/').count();
    Some(path.split('/').skip(depth).collect::<Vec<_>>().join("/"))
}

/// `path` carried from under `old` to under `new`, as a rename or a move takes it; else as it is.
pub fn moved(path: &str, old: &str, new: &str) -> String {
    match below(old, path) {
        Some(rest) if rest.is_empty() => new.to_owned(),
        Some(rest) => join(new, &rest),
        None => path.to_owned(),
    }
}

/// The folder's path from the root, the root itself (or a folder outside it) by its own name.
pub fn relative(folder: &str, root: &str) -> String {
    match below(root, folder) {
        Some(rest) if !rest.is_empty() => rest,
        _ => name(folder),
    }
}

/// Whether the volume holding `path` takes `Roll 8` and `roll 8` for one name: Windows' and the Mac's
/// by default do, each Mac volume asked once; Linux's do not.
pub fn ignores_case(path: &str) -> bool {
    #[cfg(target_os = "macos")]
    return mac_volume_ignores_case(path);
    #[cfg(not(target_os = "macos"))]
    let _ = path;
    #[cfg(not(target_os = "macos"))]
    return cfg!(windows);
}

#[cfg(target_os = "macos")]
fn mac_volume_ignores_case(path: &str) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static ASKED: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let volume = match path.strip_prefix("/Volumes/") {
        Some(rest) => format!("/Volumes/{}", rest.split('/').next().unwrap_or_default()),
        None => "/".to_owned(),
    };
    let asked = ASKED.get_or_init(Mutex::default);
    if let Some(known) = asked.lock().ok().and_then(|m| m.get(&volume).copied()) {
        return known;
    }
    let Ok(text) = std::ffi::CString::new(volume.as_bytes()) else { return true };
    // SAFETY: `text` is a NUL-terminated path that outlives the call, which only reads it.
    let answer = unsafe { libc::pathconf(text.as_ptr(), libc::_PC_CASE_SENSITIVE) };
    // 1 tells case apart; 0, or a volume gone that cannot answer, is the Mac's default, blind to it.
    let ignores = answer != 1;
    if answer >= 0 && let Ok(mut known) = asked.lock() {
        known.insert(volume, ignores);
    }
    ignores
}

/// The path with its symbolic links resolved, so a folder reached through a link and through its
/// real path never becomes two entries for the same photo; a path that cannot resolve stays as written.
pub fn canonical(path: &Path) -> String {
    let Ok(resolved) = std::fs::canonicalize(path) else {
        return normal(&path.to_string_lossy()).into_owned();
    };
    let text = resolved.to_string_lossy().into_owned();
    #[cfg(target_os = "macos")]
    {
        // `resolvingSymlinksInPath` drops `/private` when the shorter path names the same file.
        if let Some(short) = text.strip_prefix("/private")
            && short.starts_with('/')
            && std::fs::canonicalize(short).is_ok_and(|p| p == resolved)
        {
            return normal(short).into_owned();
        }
    }
    if cfg!(windows) {
        let resolve = |root: &str| std::fs::canonicalize(root).ok().map(|p| p.to_string_lossy().into_owned());
        return windows_spelling(&text, &normal(&path.to_string_lossy()), &resolve);
    }
    normal(&text).into_owned()
}

/// One spelling for what Windows resolves: `\\?\UNC\nas\film` is `//nas/film`, `\\?\C:` is `C:`, and a
/// mapped or `subst` drive keeps its letter rather than the share or folder behind it.
fn windows_spelling(resolved: &str, asked: &str, resolve: &dyn Fn(&str) -> Option<String>) -> String {
    let plain = |text: &str| -> String {
        let text = text.replace('\\', "/");
        match text.strip_prefix("//?/UNC/") {
            Some(share) => format!("//{share}"),
            None => text.strip_prefix("//?/").map(str::to_owned).unwrap_or(text),
        }
    };
    let drive = |p: &str| -> Option<String> {
        let b = p.as_bytes();
        (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':').then(|| p[..2].to_ascii_uppercase())
    };
    let out = plain(resolved);
    let Some(letter) = drive(asked) else { return out };
    if drive(&out).as_ref() == Some(&letter) {
        return out;
    }
    let Some(behind) = resolve(&format!("{letter}\\")).map(|r| plain(&r)) else { return out };
    let behind = behind.trim_end_matches('/');
    match out.get(..behind.len()) {
        Some(head) if head.eq_ignore_ascii_case(behind) && out[behind.len()..].chars().next().is_none_or(|c| c == '/') => {
            format!("{letter}/{}", out[behind.len()..].trim_start_matches('/'))
        }
        _ => out,
    }
}

#[cfg(test)]
mod tests;
