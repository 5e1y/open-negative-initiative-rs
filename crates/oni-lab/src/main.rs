//! The lab's binary: the live window, `--check` or `--shot FILE [COMPONENT] [WxH]`.

use std::path::Path;

use oni_lab::{compile, lab_dir, live, shot};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = lab_dir().join("lab.slint");
    let outcome = match args.first().map(String::as_str) {
        Some("--check") => {
            if compile(&path).has_errors() { Err("lab.slint does not compile".into()) } else { Ok(()) }
        }
        Some("--shot") => match args.get(1) {
            Some(file) => shot::take(&path, Path::new(file), args.get(2).map(String::as_str), args.get(3).map(String::as_str)),
            None => Err("--shot FILE [COMPONENT] [WxH]".into()),
        },
        Some(other) => Err(format!("unknown argument {other}: none, --check or --shot FILE [COMPONENT] [WxH]")),
        None => live::run(&path),
    };
    if let Err(e) = outcome {
        eprintln!("oni-lab: {e}");
        std::process::exit(1);
    }
}
