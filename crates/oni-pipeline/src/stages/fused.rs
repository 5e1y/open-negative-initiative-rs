//! Consecutive kernels that read only their input, laid in one pass: one read and one write of the
//! frame instead of one each, the bandwidth an 8 GB M1 lacks.

use super::*;
use oni_graph::Image;
use std::collections::HashMap;
use std::sync::Mutex;

/// Several kernels run one after the other on each pixel, each with its own parameters.
pub struct Fused {
    name: &'static str,
    shader: &'static str,
    params: Vec<u8>,
}

impl Stage for Fused {
    fn name(&self) -> &'static str {
        self.name
    }

    fn shader(&self) -> &'static str {
        self.shader
    }

    fn params(&self) -> Vec<u8> {
        self.params.clone()
    }
}

/// The chains composed so far, by their kernels' names: a handful, each made once for the run.
static COMPOSED: Mutex<Option<HashMap<String, (&'static str, &'static str)>>> = Mutex::new(None);

/// `kernels` on `image` in order, each run of those reading only their input laid as one pass.
pub fn laid(image: Image, kernels: Vec<PerPixel>) -> Image {
    let mut out = image;
    let mut run: Vec<PerPixel> = Vec::new();
    for k in kernels {
        if k.kernel.alone.is_some() {
            run.push(k);
            continue;
        }
        out = flush(out, std::mem::take(&mut run)).then(k);
    }
    flush(out, run)
}

fn flush(image: Image, mut run: Vec<PerPixel>) -> Image {
    match run.len() {
        0 => image,
        1 => image.then(run.pop().expect("one")),
        _ => image.then(fused(&run)),
    }
}

fn fused(run: &[PerPixel]) -> Fused {
    let key = run.iter().map(|k| k.name).collect::<Vec<_>>().join("+");
    let (name, shader) = {
        let mut held = COMPOSED.lock().unwrap_or_else(|e| e.into_inner());
        *held.get_or_insert_with(HashMap::new).entry(key.clone()).or_insert_with(|| {
            let shader = compose(run);
            (Box::leak(format!("fused:{key}").into_boxed_str()), Box::leak(shader.into_boxed_str()))
        })
    };
    Fused { name, shader, params: run.iter().flat_map(|k| k.params.clone()).collect() }
}

/// Each kernel's body with its names suffixed by its place and its `P` a member of one uniform.
fn compose(run: &[PerPixel]) -> String {
    let mut out = String::from(wgsl::PRELUDE);
    let mut members = Vec::new();
    let mut calls = Vec::new();
    for (i, k) in run.iter().enumerate() {
        let (part, call) = k.kernel.alone.expect("only kernels reading their input alone are fused");
        let names = defined(part);
        out.push('\n');
        out.push_str(&renamed(part, &names, i));
        members.push(format!("k{i}: Params_{i}"));
        calls.push(format!("    v = {};", renamed(call, &names, i)));
    }
    out.push_str(&format!("\nstruct Params {{ {} }}\n", members.join(", ")));
    out.push_str("@group(0) @binding(3) var<uniform> P: Params;\n");
    out.push_str(&format!("fn pixel(p: vec2<i32>) -> vec4<f32> {{\n    var v = load(p);\n{}\n    return v;\n}}\n", calls.join("\n")));
    out
}

/// The functions, constants and structs a body declares.
fn defined(body: &str) -> Vec<String> {
    let mut names = Vec::new();
    for word in ["fn", "const", "struct"] {
        let mut rest = body;
        let mark = format!("{word} ");
        while let Some(at) = rest.find(&mark) {
            let before = rest[..at].chars().last();
            rest = &rest[at + mark.len()..];
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    names
}

/// `text` with each of `names` suffixed `_i` as a whole word, never a field after a dot, and `P.`
/// read as `P.ki.`.
fn renamed(text: &str, names: &[String], i: usize) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String, next: Option<char>| {
        if word.is_empty() {
            return;
        }
        if out.ends_with('.') {
            out.push_str(word);
        } else if word == "P" && next == Some('.') {
            out.push_str(&format!("P.k{i}"));
        } else if names.iter().any(|n| n == word) {
            out.push_str(&format!("{word}_{i}"));
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in text.chars() {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        flush(&mut word, &mut out, Some(c));
        out.push(c);
    }
    flush(&mut word, &mut out, None);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kernel_s_names_take_its_place_and_its_fields_stay() {
        let body = "struct Params { gain: vec4<f32> }\nconst K: f32 = 2.0;\nfn gain_px(s: vec4<f32>) -> vec4<f32> { return s * P.gain.x * K + vec4<f32>(s.gain_px); }";
        let names = defined(body);
        assert_eq!(names, ["gain_px", "K", "Params"]);
        let out = renamed(body, &names, 3);
        assert!(out.contains("struct Params_3 {") && out.contains("const K_3") && out.contains("fn gain_px_3("), "{out}");
        assert!(out.contains("P.k3.gain.x * K_3"), "{out}");
        // Twin: a field named as a function keeps its name after the dot.
        assert!(out.contains("s.gain_px)") && !out.contains("s.gain_px_3"), "{out}");
    }
}
