//! Paths as the archive keeps them: strings in their canonical form, compared and cut the way
//! Foundation's `NSString` path methods cut them. On Windows they are kept with `/` alone, which every
//! file call takes, so one string names one folder whatever separator the system handed.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::path::Path;

/// The path in the archive's form: on Windows each `\` a `/`; elsewhere as written, `\` being a
/// character a macOS name may hold.
pub fn normal(path: &str) -> Cow<'_, str> {
    if cfg!(windows) && path.contains('\\') { Cow::Owned(path.replace('\\', "/")) } else { Cow::Borrowed(path) }
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
pub fn holds(root: &str, path: &str) -> bool {
    let (root, path) = (normal(root), normal(path));
    if path == root {
        return true;
    }
    if root.ends_with('/') {
        return path.starts_with(&*root);
    }
    path.strip_prefix(&*root).is_some_and(|rest| rest.starts_with('/'))
}

/// The folder's path from the root, the root itself (or a folder outside it) by its own name.
pub fn relative(folder: &str, root: &str) -> String {
    if !holds(root, folder) || normal(folder) == normal(root) {
        return name(folder);
    }
    let (folder, root) = (normal(folder), normal(root));
    folder[root.len()..].trim_start_matches('/').to_owned()
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
            return short.to_owned();
        }
    }
    #[cfg(windows)]
    {
        if let Some(plain) = text.strip_prefix(r"\\?\")
            && plain.as_bytes().get(1) == Some(&b':')
        {
            return normal(plain).into_owned();
        }
    }
    normal(&text).into_owned()
}

/// `localizedStandardCompare`, the Finder's order: case folded, runs of digits read as numbers.
pub fn standard_order(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let run = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        it.next();
                    }
                    digits
                };
                let (m, n) = (run(&mut x), run(&mut y));
                let (m, n) = (m.trim_start_matches('0'), n.trim_start_matches('0'));
                let order = m.len().cmp(&n.len()).then_with(|| m.cmp(n));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(c), Some(d)) => {
                let order = c.to_lowercase().cmp(d.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                x.next();
                y.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_root_holds_its_tree_and_never_a_sibling_sharing_its_prefix() {
        assert!(holds("/r", "/r"));
        assert!(holds("/r", "/r/a/b"));
        assert!(holds("/r/", "/r/a"));
        // Twin: a bare prefix test would take the sibling.
        assert!("/r/ab".starts_with("/r/a"));
        assert!(!holds("/r/a", "/r/ab"));
        assert_eq!(relative("/r/a/b", "/r"), "a/b");
        assert_eq!(relative("/r", "/r"), "r");
        assert_eq!(relative("/s/x", "/r"), "x");
    }

    #[test]
    fn the_finder_order_reads_numbers_and_folds_case() {
        assert_eq!(standard_order("f2.RW2", "f10.RW2"), Ordering::Less);
        assert_eq!(standard_order("Roll b", "roll A"), Ordering::Greater);
        assert_eq!(standard_order("a", "A"), Ordering::Equal);
        // Twin: a plain byte order puts f10 first and upper case before lower.
        assert_eq!("f2.RW2".cmp("f10.RW2"), Ordering::Greater);
        assert_eq!("Roll b".cmp("roll A"), Ordering::Less);
    }

    #[test]
    fn cutting_follows_nsstring() {
        assert_eq!(parent("/a/b"), "/a");
        assert_eq!(parent("/a"), "/");
        assert_eq!(parent("/"), "/");
        assert_eq!(name("/a/b.RW2"), "b.RW2");
        assert_eq!(stem("/a/b.RW2"), "b");
        assert_eq!(extension("/a/Trip"), "");
        assert_eq!(join("/a", "/b.RW2"), "/a/b.RW2");
        // Twin: a bare path push lets an absolute name leave the folder.
        let mut bare = std::path::PathBuf::from("/a");
        bare.push("/b.RW2");
        assert_eq!(bare, Path::new("/b.RW2"));
    }

    /// Windows hands `\\` and `/` alike; the archive keeps `/`, so one folder is one string.
    #[cfg(windows)]
    #[test]
    fn a_windows_path_reads_one_way_whatever_its_separators() {
        assert_eq!(normal(r"C:\Scans\Roll 8"), "C:/Scans/Roll 8");
        assert!(holds(r"C:\Scans", "C:/Scans/Roll 8/A.NEF") && !holds("C:/Scans", r"C:\Scansion"));
        assert_eq!(join(r"C:\Scans", "Roll 8"), "C:/Scans/Roll 8");
        assert_eq!((parent(r"C:\Scans\Roll 8"), name(r"C:\Scans\A.NEF")), ("C:/Scans".to_string(), "A.NEF".to_string()));
        assert_eq!(relative(r"C:\Scans\Roll 8\Day 2", "C:/Scans"), "Roll 8/Day 2");
    }

    /// Twin: on a Mac `\\` is a character a name may hold, never a separator.
    #[cfg(not(windows))]
    #[test]
    fn a_backslash_stays_in_a_mac_name() {
        assert_eq!(normal(r"/Scans/a\b"), r"/Scans/a\b");
        assert_eq!(name(r"/Scans/a\b"), r"a\b");
    }
}
