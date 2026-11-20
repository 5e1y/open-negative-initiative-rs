//! How a piece reads under its name: what tells it from its kin, the catalogue's gaps left out.

use oni_i18n::tr;
use oni_tldb::{Kind, Piece};

/// A length or an aperture without a needless decimal: 50, 5.6.
fn number(v: f64) -> String {
    let tenths = (v * 10.0).round() / 10.0;
    if tenths.fract() == 0.0 { format!("{tenths:.0}") } else { format!("{tenths:.1}") }
}

/// Both ends, or one when they agree: 70–300, 50.
fn span(a: f64, b: f64) -> String {
    if (a - b).abs() < 0.05 { number(a) } else { format!("{}–{}", number(a), number(b)) }
}

/// A body's mount or own lens and its year; a lens's focal lengths, aperture, mounts and year.
pub fn detail(piece: &Piece) -> String {
    let mut parts: Vec<String> = Vec::new();
    match piece.kind {
        Kind::Camera => {
            if let Some(own) = &piece.built_in_lens {
                parts.push(own.model.clone());
            } else if let Some(mount) = &piece.mount {
                parts.push(mount.clone());
            }
        }
        Kind::Lens => {
            if let Some(f) = piece.focal_length_mm {
                parts.push(format!("{} mm", span(f.min, f.max)));
            }
            if let Some(a) = piece.max_aperture {
                parts.push(format!("f/{}", span(a.at_min_focal, a.at_max_focal)));
            }
            if let Some(mounts) = piece.mounts.as_ref().filter(|m| !m.is_empty()) {
                parts.push(mounts.join(" / "));
            }
        }
    }
    if let Some(year) = piece.year_introduced {
        parts.push(year.to_string());
    }
    parts.join(" · ")
}

/// A search's answer names its kind first, as bodies and lenses come mixed.
pub fn found(piece: &Piece) -> String {
    let kind = match piece.kind {
        Kind::Camera => tr("Body"),
        Kind::Lens => tr("Lens"),
    };
    let rest = detail(piece);
    if rest.is_empty() { kind.to_string() } else { format!("{kind} · {rest}") }
}

/// The search as the service takes it: 200 characters and 10 words at most.
pub fn query(typed: &str) -> String {
    let words: Vec<&str> = typed.split_whitespace().take(10).collect();
    words.join(" ").chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use oni_tldb::{Aperture, Span};

    fn lens(focal: (f64, f64), aperture: Option<(f64, f64)>) -> Piece {
        Piece {
            kind: Kind::Lens,
            id: 1,
            name: "Canon EF 70-300mm f/4-5.6 IS USM".into(),
            brand: Some("Canon".into()),
            model: "EF 70-300mm f/4-5.6 IS USM".into(),
            url: String::new(),
            mount: None,
            mounts: Some(vec!["Canon EF".into()]),
            year_introduced: Some(2005),
            focal_length_mm: Some(Span { min: focal.0, max: focal.1 }),
            max_aperture: aperture.map(|(a, b)| Aperture { at_min_focal: a, at_max_focal: b }),
            built_in_lens: None,
        }
    }

    #[test]
    fn a_lens_reads_its_focal_lengths_aperture_mount_and_year() {
        assert_eq!(detail(&lens((70.0, 300.0), Some((4.0, 5.6)))), "70–300 mm · f/4–5.6 · Canon EF · 2005");
        assert_eq!(detail(&lens((50.0, 50.0), Some((1.4, 1.4)))), "50 mm · f/1.4 · Canon EF · 2005", "a prime reads one length");
        // Twin: an aperture the catalogue lacks leaves no empty "f/" behind.
        assert_eq!(detail(&lens((28.0, 28.0), None)), "28 mm · Canon EF · 2005");
    }

    #[test]
    fn a_search_keeps_to_the_service_bounds() {
        let long = (0..14).map(|i| format!("w{i}")).collect::<Vec<_>>().join("  ");
        assert_eq!(query(&long).split(' ').count(), 10);
        assert_eq!(query(&"x".repeat(300)).chars().count(), 200);
        // Twin: a search within the bounds passes as typed, its spaces settled.
        assert_eq!(query("  canon   ae "), "canon ae");
    }
}
