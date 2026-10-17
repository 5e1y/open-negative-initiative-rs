//! Reads the Swift app's `DSTokens.swift` and writes `ui/ds/tokens.slint`, one `export global DS`, so
//! no token is ever copied by hand.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

/// What a dark-only app resolves SwiftUI's appearance-dependent colours to, as straight sRGB.
/// The label alphas are AppKit's on the dark aqua appearance; the rest are its dark system colours.
const SYSTEM: &[(&str, [f64; 4])] = &[
    ("primary", [1.0, 1.0, 1.0, 0.8471]),
    ("secondary", [1.0, 1.0, 1.0, 0.549]),
    ("tertiary", [1.0, 1.0, 1.0, 0.2471]),
    ("black", [0.0, 0.0, 0.0, 1.0]),
    ("white", [1.0, 1.0, 1.0, 1.0]),
    ("orange", [1.0, 159.0 / 255.0, 10.0 / 255.0, 1.0]),
    ("teal", [64.0 / 255.0, 200.0 / 255.0, 224.0 / 255.0, 1.0]),
    ("mint", [102.0 / 255.0, 212.0 / 255.0, 207.0 / 255.0, 1.0]),
    ("pink", [1.0, 55.0 / 255.0, 95.0 / 255.0, 1.0]),
    ("accentColor", [10.0 / 255.0, 132.0 / 255.0, 1.0, 1.0]),
    // `underPageBackgroundColor` on the dark appearance, forced opaque as `Palette.panel` does.
    ("panel", [40.0 / 255.0, 40.0 / 255.0, 40.0 / 255.0, 1.0]),
];

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Length(f64),
    Float(f64),
    Int(i64),
    Duration(f64),
    Color([f64; 4]),
    Text(String),
}

#[derive(Debug, PartialEq)]
pub enum TokenError {
    Collision(String),
    Empty,
}

/// One `static let` or single-expression `static var`, its group being the enclosing `enum`.
struct Decl {
    group: String,
    name: String,
    ty: Option<String>,
    expr: String,
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn balanced(s: &str) -> bool {
    let open = s.matches(['(', '[']).count();
    let close = s.matches([')', ']']).count();
    open <= close
}

/// The declarations of `source`, each joined across the lines its parentheses span.
fn declarations(source: &str) -> Vec<Decl> {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = Vec::new();
    let mut group: Vec<(String, usize)> = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].split("//").next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix("enum ").or_else(|| line.strip_prefix("public enum ")) {
            let name = rest.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_owned();
            group.push((name, depth));
        }
        let statement = line.trim_start_matches("private ").trim_start_matches("public ");
        let is_let = statement.starts_with("static let ");
        let is_var = statement.starts_with("static var ");
        if is_let || is_var {
            let mut text = statement[11..].to_owned();
            let mut j = i;
            while !balanced(&text) && j + 1 < lines.len() {
                j += 1;
                text.push(' ');
                text.push_str(lines[j].split("//").next().unwrap_or("").trim());
            }
            // A computed `var` keeps its one-expression body; a longer one is not a constant.
            if is_var && text.trim_end().ends_with('{') && j + 2 < lines.len() && lines[j + 2].trim() == "}" {
                text.push_str(lines[j + 1].trim());
                text.push('}');
                j += 2;
            }
            if let Some(d) = parse_decl(&text, &group) {
                out.push(d);
            }
            for l in &lines[i..=j] {
                depth += l.matches('{').count();
                depth = depth.saturating_sub(l.matches('}').count());
            }
            i = j + 1;
        } else {
            depth += lines[i].matches('{').count();
            depth = depth.saturating_sub(lines[i].matches('}').count());
            i += 1;
        }
        while group.last().is_some_and(|(_, d)| depth <= *d) {
            group.pop();
        }
    }
    out
}

fn parse_decl(text: &str, group: &[(String, usize)]) -> Option<Decl> {
    let group = group.iter().filter(|(g, _)| g != "DS").map(|(g, _)| g.clone()).collect::<Vec<_>>().join(".");
    let name_end = text.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = text[..name_end].to_owned();
    let rest = text[name_end..].trim();
    let (ty, rest) = match rest.strip_prefix(':') {
        Some(r) => {
            let r = r.trim();
            let end = r.find(['=', '{']).unwrap_or(r.len());
            (Some(r[..end].trim().to_owned()), r[end..].trim())
        }
        None => (None, rest),
    };
    let expr = if let Some(e) = rest.strip_prefix('=') {
        e.trim().to_owned()
    } else if let Some(e) = rest.strip_prefix('{') {
        e.trim().strip_suffix('}')?.trim().to_owned()
    } else {
        return None;
    };
    Some(Decl { group, name, ty, expr })
}

/// A numeric expression: literals, `+ - * /`, parentheses and references to other tokens.
struct Expr<'a> {
    s: &'a [u8],
    at: usize,
    look: &'a dyn Fn(&str) -> Option<f64>,
}

impl Expr<'_> {
    fn skip(&mut self) {
        while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.s.get(self.at).copied()
    }

    fn sum(&mut self) -> Option<f64> {
        let mut v = self.product()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.at += 1;
            let r = self.product()?;
            v = if op == b'+' { v + r } else { v - r };
        }
        Some(v)
    }

    fn product(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        while let Some(op @ (b'*' | b'/')) = self.peek() {
            self.at += 1;
            let r = self.unary()?;
            v = if op == b'*' { v * r } else { v / r };
        }
        Some(v)
    }

    fn unary(&mut self) -> Option<f64> {
        match self.peek()? {
            b'-' => {
                self.at += 1;
                Some(-self.unary()?)
            }
            b'(' => {
                self.at += 1;
                let v = self.sum()?;
                (self.peek()? == b')').then(|| self.at += 1)?;
                Some(v)
            }
            c if c.is_ascii_digit() => {
                let start = self.at;
                if self.s[self.at..].starts_with(b"0x") {
                    self.at += 2;
                    while self.at < self.s.len() && self.s[self.at].is_ascii_hexdigit() {
                        self.at += 1;
                    }
                    let digits = std::str::from_utf8(&self.s[start + 2..self.at]).ok()?;
                    return i64::from_str_radix(digits, 16).ok().map(|v| v as f64);
                }
                while self.at < self.s.len() && (self.s[self.at].is_ascii_digit() || matches!(self.s[self.at], b'.' | b'_')) {
                    self.at += 1;
                }
                std::str::from_utf8(&self.s[start..self.at]).ok()?.replace('_', "").parse().ok()
            }
            c if c.is_ascii_alphabetic() => {
                let start = self.at;
                while self.at < self.s.len() && (self.s[self.at].is_ascii_alphanumeric() || self.s[self.at] == b'.') {
                    self.at += 1;
                }
                (self.look)(std::str::from_utf8(&self.s[start..self.at]).ok()?)
            }
            _ => None,
        }
    }
}

fn eval(expr: &str, look: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
    let mut e = Expr { s: expr.as_bytes(), at: 0, look };
    let v = e.sum()?;
    (e.peek().is_none()).then_some(v)
}

/// The named arguments of a call, `label: expr` split on top-level commas.
fn arguments(inner: &str) -> Vec<(String, String)> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in inner.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&inner[start..]);
    parts
        .into_iter()
        .map(|p| match p.split_once(':') {
            Some((l, v)) if !l.contains('(') => (l.trim().to_owned(), v.trim().to_owned()),
            _ => (String::new(), p.trim().to_owned()),
        })
        .collect()
}

fn call<'a>(expr: &'a str, head: &str) -> Option<&'a str> {
    expr.strip_prefix(head)?.strip_prefix('(')?.strip_suffix(')')
}

struct Resolver {
    group: String,
    numbers: HashMap<String, f64>,
    colours: HashMap<String, [f64; 4]>,
    texts: HashMap<String, String>,
}

impl Resolver {
    fn keys(&self, name: &str) -> Vec<String> {
        let name = name.strip_prefix("DS.").unwrap_or(name);
        let mut keys = vec![name.to_owned()];
        if !name.contains('.') {
            keys.insert(0, format!("{}.{name}", self.group));
        }
        keys
    }

    fn number(&self, name: &str) -> Option<f64> {
        self.keys(name).iter().find_map(|k| self.numbers.get(k).copied())
    }

    fn colour(&self, expr: &str) -> Option<[f64; 4]> {
        let expr = expr.trim();
        let num = |e: &str| eval(e, &|n| self.number(n));
        if let Some((base, alpha)) = expr.rsplit_once(".opacity(") {
            let mut c = self.colour(base)?;
            c[3] *= num(alpha.strip_suffix(')')?)?;
            return Some(c);
        }
        if let Some(inner) = call(expr, "Color").or_else(|| call(expr, "KeycapStyle.RGBA")) {
            let args: HashMap<String, String> = arguments(inner).into_iter().collect();
            let get = |k: &str| args.get(k).and_then(|e| num(e));
            if args.contains_key("nsColor") {
                return SYSTEM.iter().find(|(n, _)| *n == "panel").map(|(_, c)| *c);
            }
            let alpha = get("opacity").or_else(|| get("a")).unwrap_or(1.0);
            if let Some(w) = get("white") {
                return Some([w, w, w, alpha]);
            }
            let r = get("red").or_else(|| get("r"))?;
            let g = get("green").or_else(|| get("g"))?;
            let b = get("blue").or_else(|| get("b"))?;
            return Some([r, g, b, alpha]);
        }
        let bare = expr.trim_start_matches("Color.").trim_start_matches('.');
        if let Some((_, c)) = SYSTEM.iter().find(|(n, _)| *n == bare) {
            return Some(*c);
        }
        self.keys(expr).iter().find_map(|k| self.colours.get(k).copied())
    }

    fn value(&self, d: &Decl) -> Option<Vec<(String, Value)>> {
        let ty = d.ty.as_deref().unwrap_or("");
        let e = d.expr.as_str();
        let num = |e: &str| eval(e, &|n| self.number(n));
        let one = |v: Value| Some(vec![(String::new(), v)]);
        match ty {
            "CGFloat" => one(Value::Length(num(e)?)),
            "Double" => one(Value::Float(num(e)?)),
            "Int" => one(Value::Int(num(e)? as i64)),
            "TimeInterval" => one(Value::Duration(num(e)? * 1000.0)),
            "Duration" => one(Value::Duration(num(call(e, ".milliseconds")?)?)),
            "String" => one(Value::Text(e.trim_matches('"').to_owned())),
            "HierarchicalShapeStyle" => one(Value::Color(self.colour(e)?)),
            "Color" => one(Value::Color(self.colour(e)?)),
            "" => {
                if let Some(inner) = call(e, "Spec") {
                    let a: HashMap<String, String> = arguments(inner).into_iter().collect();
                    return Some(vec![
                        ("response".into(), Value::Float(num(a.get("response")?)?)),
                        ("damping".into(), Value::Float(num(a.get("dampingRatio")?)?)),
                    ]);
                }
                if let Some(inner) = call(e, "CGSize") {
                    let a: HashMap<String, String> = arguments(inner).into_iter().collect();
                    return Some(vec![
                        ("width".into(), Value::Length(num(a.get("width")?)?)),
                        ("height".into(), Value::Length(num(a.get("height")?)?)),
                    ]);
                }
                if let Some(font) = self.font(e) {
                    return Some(font);
                }
                if e.starts_with('"') {
                    return one(Value::Text(e.trim_matches('"').to_owned()));
                }
                if e.starts_with('(') || e.ends_with(".animation") || e.starts_with("Animation.") {
                    return None;
                }
                if let Some(v) = num(e).filter(|v| v.fract() == 0.0) {
                    return one(Value::Int(v as i64));
                }
                one(Value::Color(self.colour(e)?))
            }
            _ => None,
        }
    }

    /// `Font.custom(name, size:)` with an optional `.weight(…)`; the PostScript name gives the family.
    fn font(&self, e: &str) -> Option<Vec<(String, Value)>> {
        let (head, weight) = match e.split_once(").weight(.") {
            Some((h, w)) => (format!("{h})"), Some(w.trim_end_matches(')'))),
            None => (e.trim_end_matches(".monospacedDigit()").to_owned(), None),
        };
        let (family, size) = if let Some(inner) = call(&head, "Font.custom") {
            let args = arguments(inner);
            let first = &args.first()?.1;
            let name = match first.strip_prefix('"') {
                Some(quoted) => quoted.trim_end_matches('"').to_owned(),
                None => self.keys(first).iter().find_map(|k| self.texts.get(k).cloned())?,
            };
            let size = args.iter().find(|(l, _)| l == "size").map(|(_, v)| v.clone())?;
            (name.split('-').next().unwrap_or(&name).to_owned(), size)
        } else if let Some(inner) = call(&head, "Font.system") {
            let size = arguments(inner).into_iter().find(|(l, _)| l == "size")?.1;
            (String::new(), size)
        } else {
            return None;
        };
        let weight = match weight {
            None => 400,
            Some("light") => 300,
            Some("medium") => 500,
            Some("semibold") => 600,
            Some("bold") => 700,
            Some(_) => return None,
        };
        Some(vec![
            ("family".into(), Value::Text(family)),
            ("size".into(), Value::Length(eval(&size, &|n| self.number(n))?)),
            ("weight".into(), Value::Int(weight)),
        ])
    }
}

/// The names a catalogue lists, e.g. `Control.ratios`: a `CGFloat` there is a proportion, not a length.
fn catalogue(source: &str, group: &str, name: &str) -> Vec<String> {
    let Some(start) = source.find(&format!("static let {name}: [")) else { return Vec::new() };
    let body = &source[start..];
    let end = body.find("\n        ]").unwrap_or(body.len());
    let _ = group;
    body[..end].split("(\"").skip(1).filter_map(|s| s.split('"').next()).map(str::to_owned).collect()
}

fn literal(v: &Value) -> (&'static str, String) {
    let trim = |x: f64| {
        let s = format!("{:.6}", x);
        let s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
        if s == "-0" { "0".to_owned() } else { s }
    };
    match v {
        Value::Length(x) => ("length", format!("{}px", trim(*x))),
        Value::Float(x) => ("float", trim(*x)),
        Value::Int(x) => ("int", x.to_string()),
        Value::Duration(ms) => ("duration", format!("{}ms", trim(*ms))),
        Value::Color(c) => {
            let q = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
            ("color", format!("#{:02x}{:02x}{:02x}{:02x}", q(c[0]), q(c[1]), q(c[2]), q(c[3])))
        }
        Value::Text(t) => ("string", format!("{t:?}")),
    }
}

/// The Slint global for `tokens`, with `foreign` holding other Swift files a token reads from.
pub fn generate(tokens: &str, foreign: &[&str]) -> Result<String, TokenError> {
    let mut resolver = Resolver { group: String::new(), numbers: HashMap::new(), colours: HashMap::new(), texts: HashMap::new() };
    // A tuple elsewhere in the app, such as `ScreenGamut.lime`, lends its fields by name.
    for source in foreign {
        let mut group = String::new();
        for line in source.lines() {
            let t = line.trim();
            if let Some(r) = t.strip_prefix("enum ").or_else(|| t.strip_prefix("struct ")) {
                group = r.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_owned();
            }
            if let Some(r) = t.strip_prefix("static let ")
                && let Some((name, rest)) = r.split_once(" = (")
            {
                for (label, e) in arguments(rest.trim_end_matches(')')) {
                    if let Some(v) = eval(&e, &|_| None) {
                        resolver.numbers.insert(format!("{group}.{}.{label}", name.trim()), v);
                    }
                }
            }
        }
    }
    let decls = declarations(tokens);
    let ratios = catalogue(tokens, "Control", "ratios");
    let mut out: BTreeMap<String, (usize, Value)> = BTreeMap::new();
    let mut skipped = Vec::new();
    let mut clash = Vec::new();
    // References may point forward (`scrimAlpha`), so resolve until nothing new lands.
    let mut pending: Vec<&Decl> = decls.iter().filter(|d| !d.expr.starts_with('[') && d.name != "all").collect();
    let mut order = 0;
    loop {
        let before = pending.len();
        pending.retain(|d| {
            resolver.group = d.group.clone();
            let Some(values) = resolver.value(d) else { return true };
            for (suffix, mut v) in values {
                let key = format!("{}.{}", d.group, d.name);
                if let Value::Length(x) = v
                    && d.group == "Control"
                    && ratios.contains(&d.name)
                {
                    v = Value::Float(x);
                }
                match &v {
                    Value::Length(x) | Value::Float(x) if suffix.is_empty() => {
                        resolver.numbers.insert(key, *x);
                    }
                    Value::Int(x) if suffix.is_empty() => {
                        resolver.numbers.insert(key, *x as f64);
                    }
                    Value::Duration(ms) => {
                        resolver.numbers.insert(key, ms / 1000.0);
                    }
                    Value::Color(c) => {
                        resolver.colours.insert(key, *c);
                    }
                    Value::Text(t) if suffix.is_empty() => {
                        resolver.texts.insert(key, t.clone());
                    }
                    _ => {}
                }
                let name = d.name.strip_suffix("Spec").filter(|_| suffix == "response" || suffix == "damping").unwrap_or(&d.name);
                let mut full = format!("{}-{}", kebab(&d.group.replace('.', "-")), kebab(name));
                if !suffix.is_empty() {
                    full = format!("{full}-{suffix}");
                }
                order += 1;
                // `Typo.labelSize` and the size of `Typo.label` meet on one name: equal, they are one token.
                match out.get(&full) {
                    Some((_, held)) if *held != v => clash.push(format!("{full}: {held:?} and {v:?}")),
                    Some(_) => {}
                    None => {
                        out.insert(full, (order, v));
                    }
                }
            }
            false
        });
        if pending.len() == before {
            break;
        }
    }
    // An `Animation` is its spring or its duration, both already written above.
    for d in pending.into_iter().filter(|d| !d.expr.ends_with(".animation") && !d.expr.starts_with("Animation.")) {
        skipped.push(format!("{}.{}", d.group, d.name));
    }
    if let Some(c) = clash.into_iter().next() {
        return Err(TokenError::Collision(c));
    }
    if out.is_empty() {
        return Err(TokenError::Empty);
    }
    let mut seen = HashMap::new();
    for name in out.keys() {
        if let Some(prev) = seen.insert(name.to_lowercase(), name.clone()) {
            return Err(TokenError::Collision(format!("{prev} and {name}")));
        }
    }
    let mut text = String::new();
    text.push_str("// Generated by `cargo run -p oni-app --bin oni-tokens` from the Swift app's DSTokens.swift: never edit.\n");
    if !skipped.is_empty() {
        let _ = writeln!(text, "// Not constants, so not here: {}.", skipped.join(", "));
    }
    text.push_str("export global DS {\n");
    for (name, (_, v)) in &out {
        let (ty, lit) = literal(v);
        let _ = writeln!(text, "    out property <{ty}> {name}: {lit};");
    }
    for (name, c) in SYSTEM {
        let (ty, lit) = literal(&Value::Color(*c));
        let _ = writeln!(text, "    out property <{ty}> system-{}: {lit};", kebab(name));
    }
    text.push_str("}\n");
    Ok(text)
}

/// Throws when two Swift names land on one Slint name, which would leave only the last one.
pub fn check_unique(names: &[&str]) -> Result<(), TokenError> {
    let mut seen = HashMap::new();
    for n in names {
        if let Some(prev) = seen.insert(n.to_lowercase(), *n) {
            return Err(TokenError::Collision(format!("{prev} and {n}")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
enum DS {
    enum Space {
        static let md: CGFloat = 16
        static let lg: CGFloat = 24
    }
    enum Motion {
        static let settleSpec = Spec(response: 0.38, dampingRatio: 0.72)
        static let curtainDuration: TimeInterval = 0.14
        static let panelDelay: TimeInterval = curtainDuration
    }
    enum Palette {
        static let track = Color.primary.opacity(0.18)
        static let scrim = Color.black.opacity(scrimAlpha)
        static let keycap = Color(.sRGB, red: 0x31 / 255, green: 0x31 / 255,
                                  blue: 0x31 / 255, opacity: 1)
        private static let scrimAlpha: Double = 0.55
    }
    enum Control {
        static let panelWidth: CGFloat = 340
        static var inset: CGFloat { Space.md }
        static let width: CGFloat = Control.panelWidth - 2 * Space.md
        static let knobGrow: CGFloat = 1.07
        static let minWindow = CGSize(width: 1100, height: 680)
        static let ratios: [(String, CGFloat)] = [
            ("knobGrow", knobGrow),
        ]
    }
    enum Typo {
        static let value = Font.custom(sans, size: 11).monospacedDigit()
        static let label = Font.custom(sans, size: 13)
        static let labelSize: CGFloat = 13
        private static let sans = "Inter-Regular"
    }
}
"#;

    #[test]
    fn the_swift_constants_land_with_their_types() {
        let out = generate(SAMPLE, &[]).unwrap();
        for line in [
            "space-md: 16px;",
            "control-width: 308px;",
            "control-inset: 16px;",
            "control-knob-grow: 1.07;",
            "control-min-window-width: 1100px;",
            "motion-settle-response: 0.38;",
            "motion-settle-damping: 0.72;",
            "motion-panel-delay: 140ms;",
            "palette-keycap: #313131ff;",
            "palette-track: #ffffff27;",
            "palette-scrim: #0000008c;",
            "typo-value-family: \"Inter\";",
            "typo-value-size: 11px;",
        ] {
            assert!(out.contains(line), "missing {line} in\n{out}");
        }
        // Twin: a ratio typed as a length would scale with the display, and a wrong sum shows.
        assert!(!out.contains("control-knob-grow: 1.07px"));
        assert!(!out.contains("control-width: 340px"));
    }

    #[test]
    fn two_values_on_one_slint_name_are_refused() {
        assert!(generate(SAMPLE, &[]).unwrap().contains("typo-label-size: 13px;"));
        let clash = SAMPLE.replace("static let labelSize: CGFloat = 13", "static let labelSize: CGFloat = 14");
        assert!(matches!(generate(&clash, &[]), Err(TokenError::Collision(_))));
    }

    #[test]
    fn two_names_on_one_slint_name_are_refused() {
        assert!(check_unique(&["control-knob-line", "palette-knob-line"]).is_ok());
        assert!(matches!(check_unique(&["control-knob-line", "control-knob-line"]), Err(TokenError::Collision(_))));
    }

    #[test]
    fn the_committed_tokens_are_the_swift_ones() {
        let Some(root) = crate::swift_sources() else {
            println!("SKIPPED: no Swift sources (ONI_SWIFT_APP)");
            return;
        };
        let tokens = std::fs::read_to_string(root.join("DesignSystem/DSTokens.swift")).unwrap();
        let gamut = std::fs::read_to_string(root.join("Pipeline/ScreenGamut.swift")).unwrap_or_default();
        let fresh = generate(&tokens, &[&gamut]).unwrap();
        let committed = include_str!("../ui/ds/tokens.slint");
        assert_eq!(fresh, committed, "ui/ds/tokens.slint is stale: run cargo run -p oni-app --bin oni-tokens");
        // Twin: a token moved in the Swift source must make the committed file stale.
        let moved = tokens.replace("static let panelWidth: CGFloat = 340", "static let panelWidth: CGFloat = 341");
        assert_ne!(generate(&moved, &[&gamut]).unwrap(), committed);
    }
}
