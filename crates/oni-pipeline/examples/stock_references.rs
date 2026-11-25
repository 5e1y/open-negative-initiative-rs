//! Each film of the stock files named, its reference and, where a paper reads it, its filtration, as its sheets derive
//! them, in JSON by file and film key, which scripts/datasheet-profiles.py lays back. A film's paper is read beside its
//! file. Run: cargo run -p oni-pipeline --example stock_references -- <files>.

use oni_pipeline::film;
use oni_settings::stocks::file::{self, File};
use oni_settings::film::FilmType;
use serde_json::{Map, Value, json};

/// The spectral sensitivities of the paper `key` names, in the folder of `beside`, by its own key or a retired one.
fn sensitivity(beside: &std::path::Path, key: &str) -> String {
    let folder = beside.parent().expect("a file in a folder");
    for path in std::fs::read_dir(folder).expect("the folder reads").flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")) {
        let Ok(File::Paper(p)) = file::read(&std::fs::read(&path).unwrap_or_default()) else { continue };
        if p.key == key || p.aliases.iter().any(|a| a == key) {
            return p.sensitivity.unwrap_or_else(|| panic!("{}: the paper holds no spectral sensitivity", path.display())).get().to_owned();
        }
    }
    panic!("{key}: no paper of that key beside {}", beside.display())
}

fn main() {
    let mut out = Map::new();
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let File::Film(f) = file::read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}")) else { continue };
        let silver = f.kind == FilmType::BlackAndWhite;
        let mut films = Map::new();
        for b in f.film.iter().chain(f.developments.iter().filter_map(|d| d.film.as_ref())) {
            let (reversal, couplers) = (f.kind == FilmType::ColourSlide, b.couplers.unwrap_or(film::bake::COUPLERS));
            let paper = b.read_by.as_deref().map(|key| sensitivity(std::path::Path::new(&path), key));
            let reader = paper.as_deref().map(|sensitivity| film::Reader { sensitivity, filtration: b.filtration });
            let d = match b.neutral {
                true => film::derive_neutral(b.profile.get()).map(|reference| film::Derived { reference, filtration: None }),
                false => film::derive(b.profile.get(), b.balance, silver, reversal, couplers, reader),
            }
            .unwrap_or_else(|e| panic!("{path}: {e}"));
            let r = d.reference;
            films.insert(b.key.clone(), json!({"stops": r.stops, "black": r.black, "white": r.white, "filtration": d.filtration}));
        }
        let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        out.insert(name, Value::Object(films));
    }
    println!("{}", Value::Object(out));
}
