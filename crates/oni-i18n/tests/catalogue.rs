//! The catalogues hold every text the app shows, in every language it speaks: each `@tr` of the
//! interface and each `tr`, `tr!` and `trn` of the Rust side. `scripts/i18n` adds the new ones.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const LANGUAGES: [&str; 2] = ["fr", "es"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files(&path, extension, out);
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
}

/// A quoted literal starting at `at` (on its opening quote): its text, its escapes read, and where it ends.
fn literal(source: &str, at: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();
    if bytes.get(at) != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = source[at + 1..].char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, at + 1 + i + 1)),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'u' => {
                    let hex: String = chars.by_ref().map(|(_, c)| c).skip(1).take_while(|c| *c != '}').collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

fn skip_space(source: &str, mut at: usize) -> usize {
    while source[at..].starts_with(char::is_whitespace) {
        at += source[at..].chars().next().map_or(1, char::len_utf8);
    }
    at
}

/// One text and, for a plural, its English plural.
type Text = (String, Option<String>);

/// Every text a call named `call` gives, in order: `@tr("…")` and `@tr("…" | "…" % n)` in Slint,
/// `tr("…")`, `tr!("…"` and `trn("…", "…"` in Rust.
fn texts(source: &str, call: &str, plural_bar: bool) -> Vec<Text> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = source[from..].find(call) {
        let start = from + found;
        from = start + call.len();
        // `tr(` must not end a longer name or a method, as `attr(` or `x.tr(` do.
        let before = source[..start].chars().last();
        if !call.starts_with('@') && before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
            continue;
        }
        let at = skip_space(source, from);
        let Some((one, end)) = literal(source, at) else { continue };
        let after = skip_space(source, end);
        let many = if plural_bar && source[after..].starts_with('|') {
            literal(source, skip_space(source, after + 1)).map(|(m, _)| m)
        } else if !plural_bar && call.starts_with("trn") && source[after..].starts_with(',') {
            literal(source, skip_space(source, after + 1)).map(|(m, _)| m)
        } else {
            None
        };
        out.push((one, many));
    }
    out
}

/// Every text of the sources, each once, in the order first met, with the files that use it.
fn sources() -> Vec<(Text, BTreeSet<String>)> {
    let root = root();
    let mut found: Vec<(Text, BTreeSet<String>)> = Vec::new();
    let mut add = |text: Text, file: &Path| {
        let name = file.strip_prefix(&root).unwrap_or(file).to_string_lossy().replace('\\', "/");
        match found.iter_mut().find(|(t, _)| t.0 == text.0) {
            Some((_, files)) => {
                files.insert(name);
            }
            None => found.push((text, BTreeSet::from([name]))),
        }
    };
    let mut slint = Vec::new();
    files(&root.join("crates/oni-ui/ui"), "slint", &mut slint);
    for file in slint.iter().filter(|f| !f.ends_with("ds/catalogue.slint") && !f.parent().is_some_and(|d| d.ends_with("ds/board"))) {
        let source = std::fs::read_to_string(file).unwrap();
        texts(&source, "@tr(", true).into_iter().for_each(|t| add(t, file));
    }
    let mut rust = Vec::new();
    for dir in ["crates/oni-app/src", "crates/oni-core/src", "crates/oni-library/src", "crates/oni-export/src"] {
        files(&root.join(dir), "rs", &mut rust);
    }
    for file in &rust {
        let source = std::fs::read_to_string(file).unwrap();
        // A module's checks hold no text the app shows.
        let source = source.split("#[cfg(test)]").next().unwrap_or_default();
        for call in ["tr(", "tr!(", "trn(", "marked("] {
            texts(source, call, false).into_iter().for_each(|t| add(t, file));
        }
    }
    found
}

/// The `{…}` places of a text, which its translation keeps.
fn places(text: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else { break };
        *out.entry(rest[open..open + close + 1].to_string()).or_default() += 1;
        rest = &rest[open + close + 1..];
    }
    out
}

fn quoted(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\t', "\\t");
    format!("\"{escaped}\"")
}

fn catalogue_path(language: &str) -> PathBuf {
    root().join(format!("crates/oni-i18n/lang/{language}/LC_MESSAGES/oni-ui.po"))
}

/// The catalogue rewritten on the sources: their texts in order, each translation kept, new ones empty.
fn written(texts: &[(Text, BTreeSet<String>)], old: &str) -> String {
    let header: String = old.split("\n\n").next().unwrap_or_default().to_string();
    let kept = entries(old);
    let mut out = header;
    out.push('\n');
    for ((one, many), files) in texts {
        out.push('\n');
        out.push_str(&format!("#: {}\n", files.iter().cloned().collect::<Vec<_>>().join(" ")));
        out.push_str(&format!("msgid {}\n", quoted(one)));
        let forms = kept.get(one).cloned().unwrap_or_default();
        match many {
            Some(many) => {
                out.push_str(&format!("msgid_plural {}\n", quoted(many)));
                for i in 0..2 {
                    out.push_str(&format!("msgstr[{i}] {}\n", quoted(forms.get(i).map_or("", String::as_str))));
                }
            }
            None => out.push_str(&format!("msgstr {}\n", quoted(forms.first().map_or("", String::as_str)))),
        }
    }
    out
}

#[path = "../src/po.rs"]
mod po;

/// A catalogue's entries as the app reads them.
fn entries(po: &str) -> BTreeMap<String, Vec<String>> {
    po::parse(po).into_iter().collect()
}

#[test]
fn every_text_is_in_every_catalogue_and_keeps_its_places() {
    let texts = sources();
    assert!(texts.len() > 300, "the sources give {} texts: the extraction lost its calls", texts.len());
    let write = std::env::var_os("ONI_I18N_WRITE").is_some();
    let mut problems = Vec::new();
    for language in LANGUAGES {
        let path = catalogue_path(language);
        let old = std::fs::read_to_string(&path).unwrap();
        if write {
            std::fs::write(&path, written(&texts, &old)).unwrap();
        }
        let po = std::fs::read_to_string(&path).unwrap();
        let entries = entries(&po);
        let mut missing = 0;
        for ((one, many), _) in &texts {
            let forms = entries.get(one).cloned().unwrap_or_default();
            let wanted = if many.is_some() { 2 } else { 1 };
            if forms.len() < wanted || forms.iter().take(wanted).any(String::is_empty) {
                missing += 1;
                if missing <= 10 {
                    problems.push(format!("{language}: no translation for {one:?}"));
                }
                continue;
            }
            for (i, form) in forms.iter().take(wanted).enumerate() {
                let source = if i == 0 { one } else { many.as_ref().unwrap() };
                // A plural's singular may name its count in words, as "une photo" does.
                if places(form) != places(source) && !(many.is_some() && places(form).is_empty()) {
                    problems.push(format!("{language}: {form:?} does not keep the places of {source:?}"));
                }
            }
        }
        if missing > 10 {
            problems.push(format!("{language}: {missing} texts untranslated in all"));
        }
        let known: BTreeSet<&String> = texts.iter().map(|((one, _), _)| one).collect();
        let stale: Vec<&String> = entries.keys().filter(|k| !known.contains(k)).collect();
        if !stale.is_empty() {
            problems.push(format!("{language}: {} entries no source uses, first {:?}; run scripts/i18n", stale.len(), stale[0]));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_extraction_reads_slint_and_rust_calls_and_skips_longer_names() {
    let slint = r#"text: @tr("Open Folder…"); help: @tr("{n} photo" | "{n} photos" % count); x: attr("no");"#;
    assert_eq!(texts(slint, "@tr(", true), vec![("Open Folder…".to_string(), None), ("{n} photo".to_string(), Some("{n} photos".to_string()))]);
    let rust = r#"say(tr("Saved \u{2014} done")); let s = attr("no"); trn("{n} roll", "{n} rolls", n); x.str("no");"#;
    assert_eq!(texts(rust, "tr(", false), vec![("Saved \u{2014} done".to_string(), None)]);
    assert_eq!(texts(rust, "trn(", false), vec![("{n} roll".to_string(), Some("{n} rolls".to_string()))]);
    // Twin: a reader blind to the name before the call would take `attr("no")` as a text.
    assert!(!texts(rust, "tr(", false).iter().any(|(t, _)| t == "no"));
}

#[test]
fn a_translation_must_keep_the_places_of_its_text() {
    assert_eq!(places("{n} photos in {roll}"), places("{roll} : {n} photos"));
    assert_ne!(places("{} photos"), places("photos"), "twin: a dropped place is seen");
}

/// The interface's literals a reader would see untranslated: words outside `@tr`, short of the
/// identifiers, numbers, symbols and type names a `.slint` file also quotes.
fn bare_words(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let code = line.trim();
        if code.starts_with("//") || code.starts_with("import") || code.starts_with("export {") || code.contains(" from \"") {
            continue;
        }
        let mut at = 0;
        while let Some(found) = line[at..].find('"') {
            let start = at + found;
            let Some((text, end)) = literal(line, start) else { break };
            at = end;
            let before = line[..start].trim_end();
            let called = ["@tr(", "@keys(", "@image-url(", "read("].iter().any(|c| line[..start].rfind(c).is_some_and(|i| !line[i..start].contains(')')));
            let compared = before.ends_with("==") || before.ends_with("!=") || ["family:", "mono:", "sans:", "commands:"].iter().any(|k| before.ends_with(k));
            let identifier = text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_/:".contains(c));
            let word = text.as_bytes().windows(2).any(|w| w[0].is_ascii_alphabetic() && w[1].is_ascii_alphabetic());
            // A literal inside `@tr`'s list of arguments, after the text, is code.
            if word && !called && !compared && !identifier {
                out.push(text);
            }
        }
    }
    out
}

#[test]
fn no_word_of_the_interface_is_left_outside_tr() {
    let root = root();
    let mut slint = Vec::new();
    files(&root.join("crates/oni-ui/ui"), "slint", &mut slint);
    // The catalogue and the state board are the design system's own sample sheets; the tokens name fonts; the language names stay in their own tongue.
    let own = ["English", "Français", "Español"];
    let bare: Vec<String> = slint
        .iter()
        .filter(|f| !f.ends_with("ds/catalogue.slint") && !f.ends_with("ds/tokens.slint") && !f.parent().is_some_and(|d| d.ends_with("ds/board")))
        .flat_map(|f| bare_words(&std::fs::read_to_string(f).unwrap()).into_iter().map(move |w| format!("{}: {w:?}", f.file_name().unwrap().to_string_lossy())))
        .filter(|w| !own.iter().any(|o| w.ends_with(&format!("{o:?}"))))
        .collect();
    assert!(bare.is_empty(), "words outside @tr:\n{}", bare.join("\n"));
    // Twin: a label written bare is caught.
    assert_eq!(bare_words(r#"Button { text: "Open"; help: @tr("Opens"); }"#), vec!["Open".to_string()]);
}
