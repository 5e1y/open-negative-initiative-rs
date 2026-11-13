//! A scene in the film's language. The interpreter reads `@tr`'s English alone, so every file it loads
//! has each text of `@tr` replaced by the catalogue's, the one the compiled interface bundles.

use std::path::Path;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use oni_i18n::Language;
use slint_interpreter::{CompilationResult, Compiler};

/// `file` compiled with every `@tr` of it and of what it imports spoken in `language`.
pub fn compile(file: &Path, language: Language) -> CompilationResult {
    let mut compiler = Compiler::default();
    compiler.set_file_loader(move |path| {
        let source = std::fs::read_to_string(path).ok().map(|s| Ok(spoken(&s, language)));
        Box::pin(async move { source })
    });
    let source = match std::fs::read_to_string(file) {
        Ok(s) => spoken(&s, language),
        Err(_) => return oni_lab::compile(file),
    };
    let mut future = pin!(compiler.build_from_source(source, file.to_path_buf()));
    match future.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result,
        Poll::Pending => unreachable!("the interpreter's compiler is synchronous"),
    }
}

/// `source` with the texts of its `@tr` in `language`: the English, or the plural's English, each
/// replaced by its form in the catalogue; a text the catalogue lacks stays English.
pub fn spoken(source: &str, language: Language) -> String {
    if language == Language::English {
        return source.to_string();
    }
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some((before, at, after)) = next_tr(rest) {
        out.push_str(before);
        out.push_str(at);
        rest = after;
        // `@tr("context" => "text" | "plural" % n, …)`: the texts are the literals before the first comma
        // or parenthesis outside them, the one before `=>` a context, which the catalogues hold none of.
        let mut singular: Option<String> = None;
        loop {
            let skipped = rest.len() - rest.trim_start().len();
            out.push_str(&rest[..skipped]);
            rest = &rest[skipped..];
            let Some((literal, text, tail)) = string_literal(rest) else { break };
            let context = tail.trim_start().starts_with("=>");
            let translated = match (&singular, context) {
                (_, true) => None,
                (Some(one), false) => oni_i18n::translated(language, one, Some(1)),
                (None, false) => oni_i18n::translated(language, &text, Some(0)),
            };
            match translated {
                Some(t) => out.push_str(&quoted(t)),
                None => out.push_str(literal),
            }
            rest = tail;
            let skipped = rest.len() - rest.trim_start().len();
            out.push_str(&rest[..skipped]);
            rest = &rest[skipped..];
            let glue = ["=>", "|"].into_iter().find(|g| rest.starts_with(g));
            let Some(glue) = glue else { break };
            if glue == "|" {
                singular = Some(text);
            }
            out.push_str(glue);
            rest = &rest[glue.len()..];
        }
    }
    out.push_str(rest);
    out
}

/// The text before the next `@tr(` outside comments and strings, the marker itself, and what follows.
fn next_tr(source: &str) -> Option<(&str, &str, &str)> {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if source[i..].starts_with("//") {
            i += source[i..].find('\n').unwrap_or(source.len() - i);
        } else if source[i..].starts_with("/*") {
            i += source[i..].find("*/").map_or(source.len() - i, |e| e + 2);
        } else if bytes[i] == b'"' {
            i += string_literal(&source[i..]).map_or(1, |(literal, _, _)| literal.len());
        } else if source[i..].starts_with("@tr(") {
            return Some((&source[..i], &source[i..i + 4], &source[i + 4..]));
        } else {
            i += source[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    None
}

/// A string literal at the start of `s`: its source, its text with the escapes read, and what follows.
fn string_literal(s: &str) -> Option<(&str, String, &str)> {
    let mut chars = s.char_indices();
    if chars.next()?.1 != '"' {
        return None;
    }
    let mut text = String::new();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((&s[..=i], text, &s[i + 1..])),
            '\\' => match chars.next()?.1 {
                'n' => text.push('\n'),
                other => text.push(other),
            },
            c => text.push(c),
        }
    }
    None
}

/// `text` as a Slint string literal.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_speaks_the_catalogue_and_leaves_the_rest_as_written() {
        let source = r#"// @tr("Colour") in a comment stays
Text { text: @tr("Colour"); help: "Colour"; other: @tr("No such text in any catalogue"); }
Text { text: @tr("Shared by the {} photos of this roll.", n); }"#;
        let french = spoken(source, Language::French);
        assert!(french.contains(r#"text: @tr("Couleur")"#), "{french}");
        assert!(french.starts_with(r#"// @tr("Colour") in a comment stays"#), "a comment is not code");
        assert!(french.contains(r#"help: "Colour""#), "a plain string is no text of @tr");
        assert!(french.contains(r#"@tr("No such text in any catalogue")"#), "a text the catalogue lacks stays English");
        assert!(french.contains(r#"@tr("Partagée par les {} photos de cette pellicule.", n)"#), "the arguments follow the text: {french}");
        let plural = spoken(r#"@tr("One photo has no thumbnail: its original cannot be reached." | "{n} photos have no thumbnail: their originals cannot be reached." % n)"#, Language::French);
        assert!(plural.contains(r#"@tr("Une photo n’a pas de vignette"#) && plural.contains(r#"| "{n} photos n’ont pas"#), "each form its own: {plural}");
        // Twin: in English the source is the source.
        assert_eq!(spoken(source, Language::English), source);
        assert_ne!(french, source);
    }

    #[test]
    fn every_scene_compiles_in_every_language() {
        let mut scenes: Vec<_> = [crate::film::scenes(), crate::welcome::scenes()]
            .iter()
            .flat_map(|d| std::fs::read_dir(d).unwrap().map(|e| e.unwrap().path()))
            .filter(|p| p.extension().is_some_and(|e| e == "slint"))
            .collect();
        scenes.sort();
        assert!(scenes.len() > 10, "the scenes were not found");
        for language in Language::ALL {
            for scene in &scenes {
                assert!(!compile(scene, language).has_errors(), "{} in {language:?}", scene.display());
            }
        }
        // Twin: a scene that does not compile is seen.
        let broken = std::env::temp_dir().join(format!("oni-demos-broken-{}.slint", std::process::id()));
        std::fs::write(&broken, "export component X inherits Window { width: @tr(\"Colour\"); }").unwrap();
        assert!(compile(&broken, Language::French).has_errors());
        let _ = std::fs::remove_file(&broken);
    }
}
