//! Each film of the stock files named, its reference and, for a colour negative, its filtration through the scanner, as
//! its sheet derives them, in JSON by file and film key, which scripts/datasheet-profiles.py lays back.
//! Run: cargo run -p oni-pipeline --example stock_references -- <files>.

use oni_pipeline::film;
use oni_settings::stocks::file::{self, File};
use oni_settings::film::FilmType;
use serde_json::{Map, Value, json};

fn main() {
    let mut out = Map::new();
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let File::Film(f) = file::read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}")) else { continue };
        let silver = f.kind == FilmType::BlackAndWhite;
        let mut films = Map::new();
        for b in f.film.iter().chain(f.developments.iter().filter_map(|d| d.film.as_ref())) {
            let (reversal, couplers) = (f.kind == FilmType::ColourSlide, b.couplers.unwrap_or(film::bake::COUPLERS));
            let d = match b.neutral {
                true => film::derive_neutral(b.profile.get()).map(|reference| film::Derived { reference, filtration: None }),
                false => film::derive(b.profile.get(), b.balance, silver, reversal, couplers, b.filtration),
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
