//! The screens stand on the design system alone: no literal value or styled primitive outside `ui/ds/`,
//! and a planted one in a copy of the tree is caught, while the same text in a comment is not.

use oni_app::strict::{self, Rule};
use std::path::{Path, PathBuf};

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

fn plant(ui: &Path, file: &str, text: &str) {
    let path = ui.join(file);
    let source = std::fs::read_to_string(&path).unwrap();
    let anchor = "export component";
    let at = source.find(anchor).expect("the file declares a component");
    let body = at + source[at..].find('{').unwrap() + 1;
    std::fs::write(&path, format!("{}\n    {text}\n{}", &source[..body], &source[body..])).unwrap();
}

#[test]
fn screens_use_only_the_design_system() {
    let breaches = strict::scan(&strict::ui_dir());
    for b in &breaches {
        println!("{}:{} {:?}: {}", b.file.display(), b.line, b.rule, b.text);
    }
    assert!(breaches.is_empty(), "{} breaches outside ui/ds", breaches.len());
    for (file, text, why) in strict::ALLOW {
        assert!(!why.is_empty(), "{file}: '{text}' is allowed without a reason");
    }
}

#[test]
fn a_planted_value_is_refused_outside_the_design_system_only() {
    let plants: &[(&str, Rule)] = &[
        ("width: 7px;", Rule::Length),
        ("height: 2phys-px;", Rule::Length),
        ("width: 50%;", Rule::Length),
        ("padding: 1.5rem;", Rule::Length),
        ("animate x { duration: 120ms; }", Rule::Duration),
        ("animate y { duration: 1s; }", Rule::Duration),
        ("property <color> c: #ff8800;", Rule::Colour),
        ("property <color> d: rgb(255, 0, 0);", Rule::Colour),
        ("property <color> e: hsl(10, 50%, 50%);", Rule::Colour),
        ("property <color> f: white;", Rule::Colour),
        ("property <brush> g: @linear-gradient(90deg, Colour.ink 0%, Colour.accent 100%);", Rule::Colour),
        ("Rectangle { background: Colour.accent; }", Rule::Styling),
        ("Rectangle { border-radius: Radius.card; }", Rule::Styling),
        ("Label { font-size: Type.label; }", Rule::Typography),
        ("Text { text: \"raw\"; }", Rule::Primitive),
        ("Image { source: Icons.undo; }", Rule::Primitive),
        ("Flickable { }", Rule::Primitive),
        ("Rectangle { opacity: 0.5; }", Rule::Opacity),
    ];
    let root = std::env::temp_dir().join(format!("oni-ds-strict-{}", std::process::id()));
    for (k, (text, rule)) in plants.iter().enumerate() {
        let ui: PathBuf = root.join(format!("plant-{k}"));
        let _ = std::fs::remove_dir_all(&ui);
        copy(&strict::ui_dir(), &ui);
        plant(&ui, "panel.slint", text);
        let found = strict::scan(&ui);
        assert!(found.iter().any(|b| b.rule == *rule && b.file.ends_with("panel.slint")), "{text}: {found:?}");

        // Twin: the same text in the design system's own folder passes, so the scope is what refuses it.
        let ds = root.join(format!("ds-{k}"));
        let _ = std::fs::remove_dir_all(&ds);
        copy(&strict::ui_dir(), &ds);
        plant(&ds, "ds/controls.slint", text);
        assert!(strict::scan(&ds).is_empty(), "{text} inside ui/ds is the design system's own");
    }
    // Twin: a literal in a comment or a string is not a value on screen.
    let ui = root.join("quoted");
    let _ = std::fs::remove_dir_all(&ui);
    copy(&strict::ui_dir(), &ui);
    plant(&ui, "panel.slint", "// width: 7px; #ff0000\n    in property <string> note: \"7px wide, 120ms, white\";");
    assert!(strict::scan(&ui).is_empty());
    // Twin: zero needs no token, so `0px` is not a breach while `1px` is.
    let zero = strict::scan_source(Path::new("z.slint"), "x: 0px; y: 1px;");
    assert_eq!(zero.len(), 1);
    assert_eq!(zero[0].text, "1px");
    // Twin: std-widgets reached around the design system is refused.
    let import = strict::scan_source(Path::new("i.slint"), "import { Slider } from \"std-widgets.slint\";");
    assert!(import.iter().any(|b| b.rule == Rule::Import));
    let _ = std::fs::remove_dir_all(&root);
}
