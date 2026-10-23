//! A gettext catalogue read as the Slint compiler reads it: each `msgid` and its forms, a plural's
//! `msgstr[i]` in order; the header and the comments are skipped.

use std::collections::HashMap;

/// Each English text and its translated forms, one unless the entry has a plural.
pub fn parse(text: &str) -> HashMap<String, Vec<String>> {
    let mut entries = HashMap::new();
    let (mut id, mut forms): (Option<String>, Vec<String>) = (None, Vec::new());
    // Which string a continuation line extends: the id, or a form by its index.
    let mut target: Option<Option<usize>> = None;
    let mut close = |id: &mut Option<String>, forms: &mut Vec<String>| {
        if let Some(id) = id.take().filter(|i| !i.is_empty()) {
            entries.insert(id, std::mem::take(forms));
        }
        forms.clear();
    };
    for line in text.lines().map(str::trim) {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("msgid ") {
            close(&mut id, &mut forms);
            id = Some(unquoted(rest));
            target = Some(None);
        } else if line.starts_with("msgid_plural ") || line.starts_with("msgctxt ") {
            target = None;
        } else if let Some(rest) = line.strip_prefix("msgstr[") {
            let (index, value) = rest.split_once(']').unwrap_or(("0", rest));
            let index: usize = index.parse().unwrap_or(0);
            forms.resize(forms.len().max(index + 1), String::new());
            forms[index] = unquoted(value.trim());
            target = Some(Some(index));
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            forms = vec![unquoted(rest)];
            target = Some(Some(0));
        } else if line.starts_with('"') {
            match target {
                Some(None) => id.get_or_insert_with(String::new).push_str(&unquoted(line)),
                Some(Some(i)) => forms[i].push_str(&unquoted(line)),
                None => {}
            }
        }
    }
    close(&mut id, &mut forms);
    entries
}

/// A quoted gettext string's text, its escapes read.
fn unquoted(quoted: &str) -> String {
    let inner = quoted.trim().strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(quoted);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalogue_reads_its_entries_their_continuations_and_plurals() {
        let po = r#"msgid ""
msgstr ""
"Language: fr\n"

#: app.slint
msgid "Open Folder…"
msgstr "Ouvrir un dossier…"

msgid "A long "
"line"
msgstr "Une longue "
"ligne \"citée\""

msgid "{n} photo"
msgid_plural "{n} photos"
msgstr[0] "{n} photo"
msgstr[1] "{n} photos"
"#;
        let entries = parse(po);
        assert_eq!(entries["Open Folder…"], vec!["Ouvrir un dossier…"]);
        assert_eq!(entries["A long line"], vec!["Une longue ligne \"citée\""]);
        assert_eq!(entries["{n} photo"], vec!["{n} photo", "{n} photos"]);
        assert_eq!(entries.len(), 3, "the header is no entry");
        // Twin: a continuation read as a new entry would leave the id cut short.
        assert!(!entries.contains_key("A long "));
    }
}
