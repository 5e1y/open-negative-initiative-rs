//! The simulated catalogue: real gear under invented ids, with the real one's gaps (a brand, a year or
//! an aperture missing) kept on purpose so ONI meets them before the service opens.

use crate::piece::{Aperture, Key, Kind, Piece, Span};

/// The account's Kit on every start: one body and its lens.
pub const STARTING_KIT: [Key; 2] = [Key { kind: Kind::Camera, id: 1101 }, Key { kind: Kind::Lens, id: 2101 }];

/// A body: id, name, brand, model, mount, year.
type Body = (u64, &'static str, Option<&'static str>, &'static str, Option<&'static str>, Option<i32>);

/// A lens: id, name, brand, model, mounts, year, focal span, widest aperture at each end.
type Lens = (u64, &'static str, Option<&'static str>, &'static str, &'static [&'static str], Option<i32>, (f64, f64), Option<(f64, f64)>);

const BODIES: &[Body] = &[
    (1101, "Canon AE-1 Program", Some("Canon"), "AE-1 Program", Some("Canon FD"), Some(1981)),
    (1102, "Canon A-1", Some("Canon"), "A-1", Some("Canon FD"), Some(1978)),
    (1103, "Nikon FM2", Some("Nikon"), "FM2", Some("Nikon F"), Some(1982)),
    (1104, "Nikon F3", Some("Nikon"), "F3", Some("Nikon F"), Some(1980)),
    (1105, "Nikon FE2", Some("Nikon"), "FE2", Some("Nikon F"), Some(1983)),
    (1106, "Pentax MX", Some("Pentax"), "MX", Some("Pentax K"), Some(1976)),
    (1107, "Pentax K1000", None, "K1000", Some("Pentax K"), Some(1976)),
    (1108, "Olympus OM-1", Some("Olympus"), "OM-1", Some("Olympus OM"), Some(1972)),
    (1109, "Olympus OM-2n", Some("Olympus"), "OM-2n", Some("Olympus OM"), Some(1979)),
    (1110, "Minolta X-700", Some("Minolta"), "X-700", Some("Minolta SR"), Some(1981)),
    (1111, "Leica M6", Some("Leica"), "M6", Some("Leica M"), Some(1984)),
    (1112, "Leica M3", Some("Leica"), "M3", Some("Leica M"), Some(1954)),
    (1113, "Voigtländer Bessa R2A", Some("Voigtländer"), "Bessa R2A", Some("Leica M"), Some(2004)),
    (1114, "Mamiya 7 II", Some("Mamiya"), "7 II", Some("Mamiya 7"), Some(1999)),
    (1115, "Hasselblad 500C/M", None, "500C/M", Some("Hasselblad V"), Some(1970)),
    (1116, "Mamiya RB67 Pro", Some("Mamiya"), "RB67 Pro", Some("Mamiya RB"), None),
];

const LENSES: &[Lens] = &[
    (2101, "Canon FD 50mm f/1.4", Some("Canon"), "FD 50mm f/1.4", &["Canon FD"], Some(1979), (50.0, 50.0), Some((1.4, 1.4))),
    (2102, "Canon FD 28mm f/2.8", Some("Canon"), "FD 28mm f/2.8", &["Canon FD"], Some(1979), (28.0, 28.0), Some((2.8, 2.8))),
    (2103, "Canon FD 135mm f/2.8", Some("Canon"), "FD 135mm f/2.8", &["Canon FD"], Some(1979), (135.0, 135.0), Some((2.8, 2.8))),
    (2104, "Canon FD 35-105mm f/3.5", Some("Canon"), "FD 35-105mm f/3.5", &["Canon FD"], Some(1983), (35.0, 105.0), Some((3.5, 3.5))),
    (2105, "Canon EF 70-300mm f/4-5.6 IS USM", Some("Canon"), "EF 70-300mm f/4-5.6 IS USM", &["Canon EF"], Some(2005), (70.0, 300.0), Some((4.0, 5.6))),
    (2106, "Canon EF 50mm f/1.8 II", Some("Canon"), "EF 50mm f/1.8 II", &["Canon EF"], Some(1990), (50.0, 50.0), Some((1.8, 1.8))),
    (2107, "Nikon Nikkor 50mm f/1.8 AI-S", Some("Nikon"), "Nikkor 50mm f/1.8 AI-S", &["Nikon F"], Some(1981), (50.0, 50.0), Some((1.8, 1.8))),
    (2108, "Nikon Nikkor 28mm f/2.8 AI-S", Some("Nikon"), "Nikkor 28mm f/2.8 AI-S", &["Nikon F"], Some(1981), (28.0, 28.0), Some((2.8, 2.8))),
    (2109, "Nikon Zoom-Nikkor 35-70mm f/3.3-4.5", Some("Nikon"), "Zoom-Nikkor 35-70mm f/3.3-4.5", &["Nikon F"], Some(1984), (35.0, 70.0), Some((3.3, 4.5))),
    (2110, "Nikon Nikkor 105mm f/2.5 AI-S", Some("Nikon"), "Nikkor 105mm f/2.5 AI-S", &["Nikon F"], Some(1981), (105.0, 105.0), Some((2.5, 2.5))),
    (2111, "Leica Summicron-M 50mm f/2 (III)", Some("Leica"), "Summicron-M 50mm f/2 (III)", &["Leica M"], Some(1969), (50.0, 50.0), Some((2.0, 2.0))),
    (2112, "Leica Summicron-M 50mm f/2 (IV)", Some("Leica"), "Summicron-M 50mm f/2 (IV)", &["Leica M"], Some(1979), (50.0, 50.0), Some((2.0, 2.0))),
    (2113, "Leica Elmarit-M 28mm f/2.8", Some("Leica"), "Elmarit-M 28mm f/2.8", &["Leica M"], Some(1993), (28.0, 28.0), Some((2.8, 2.8))),
    (2114, "Leica Summilux-M 35mm f/1.4", Some("Leica"), "Summilux-M 35mm f/1.4", &["Leica M"], Some(1961), (35.0, 35.0), Some((1.4, 1.4))),
    (2115, "Olympus Zuiko 50mm f/1.8", Some("Olympus"), "Zuiko 50mm f/1.8", &["Olympus OM"], Some(1972), (50.0, 50.0), Some((1.8, 1.8))),
    (2116, "Olympus Zuiko 28mm f/2.8", Some("Olympus"), "Zuiko 28mm f/2.8", &["Olympus OM"], Some(1972), (28.0, 28.0), Some((2.8, 2.8))),
    (2117, "SMC Pentax-M 50mm f/1.7", Some("Pentax"), "SMC Pentax-M 50mm f/1.7", &["Pentax K"], Some(1977), (50.0, 50.0), Some((1.7, 1.7))),
    (2118, "SMC Pentax-M 28mm f/2.8", Some("Pentax"), "SMC Pentax-M 28mm f/2.8", &["Pentax K"], Some(1977), (28.0, 28.0), Some((2.8, 2.8))),
    (2119, "Minolta MD Rokkor-X 50mm f/1.7", Some("Minolta"), "MD Rokkor-X 50mm f/1.7", &["Minolta SR"], Some(1977), (50.0, 50.0), Some((1.7, 1.7))),
    (2120, "Mamiya N 80mm f/4 L", Some("Mamiya"), "N 80mm f/4 L", &["Mamiya 7"], Some(1995), (80.0, 80.0), Some((4.0, 4.0))),
    (2121, "Mamiya N 43mm f/4.5 L", Some("Mamiya"), "N 43mm f/4.5 L", &["Mamiya 7"], Some(1995), (43.0, 43.0), Some((4.5, 4.5))),
    (2122, "Mamiya Sekor C 127mm f/3.8", Some("Mamiya"), "Sekor C 127mm f/3.8", &["Mamiya RB"], Some(1974), (127.0, 127.0), None),
    (2123, "Hasselblad Planar C 80mm f/2.8", None, "Planar C 80mm f/2.8", &["Hasselblad V"], Some(1957), (80.0, 80.0), Some((2.8, 2.8))),
    (2124, "Hasselblad Distagon C 50mm f/4", Some("Hasselblad"), "Distagon C 50mm f/4", &["Hasselblad V"], Some(1957), (50.0, 50.0), Some((4.0, 4.0))),
    (2125, "Voigtländer Nokton 40mm f/1.4", Some("Voigtländer"), "Nokton 40mm f/1.4", &["Leica M"], Some(2008), (40.0, 40.0), Some((1.4, 1.4))),
    (2126, "Voigtländer Color Skopar 35mm f/2.5", Some("Voigtländer"), "Color Skopar 35mm f/2.5", &["Leica M"], Some(1999), (35.0, 35.0), Some((2.5, 2.5))),
    (
        2127,
        "Sigma 24mm f/2.8 Super-Wide II",
        Some("Sigma"),
        "24mm f/2.8 Super-Wide II",
        &["Canon FD", "Nikon F", "Pentax K", "Olympus OM", "Minolta SR"],
        Some(1985),
        (24.0, 24.0),
        Some((2.8, 2.8)),
    ),
];

/// Compacts and the lens each carries.
const COMPACTS: &[(Body, Lens)] = &[
    (
        (1117, "Contax T2", Some("Contax"), "T2", None, Some(1990)),
        (2901, "Carl Zeiss Sonnar 38mm f/2.8 T*", Some("Carl Zeiss"), "Sonnar 38mm f/2.8 T*", &[], Some(1990), (38.0, 38.0), Some((2.8, 2.8))),
    ),
    (
        (1118, "Olympus mju-II", Some("Olympus"), "mju-II", None, Some(1997)),
        (2902, "Olympus 35mm f/2.8", Some("Olympus"), "35mm f/2.8", &[], Some(1997), (35.0, 35.0), Some((2.8, 2.8))),
    ),
];

/// Every piece the service knows, bodies first; a compact's own lens is no piece of its own.
pub fn pieces(site: &str) -> Vec<Piece> {
    let mut all: Vec<Piece> = BODIES.iter().map(|&row| body(site, row)).collect();
    all.extend(COMPACTS.iter().map(|&(row, own)| Piece { built_in_lens: Some(Box::new(lens(site, own))), ..body(site, row) }));
    all.extend(LENSES.iter().map(|&row| lens(site, row)));
    all
}

fn body(site: &str, (id, name, brand, model, mount, year): Body) -> Piece {
    Piece {
        kind: Kind::Camera,
        id,
        name: name.into(),
        brand: brand.map(Into::into),
        model: model.into(),
        url: format!("{site}/cameras/{}", slug(name)),
        mount: mount.map(Into::into),
        mounts: None,
        year_introduced: year,
        focal_length_mm: None,
        max_aperture: None,
        built_in_lens: None,
    }
}

fn lens(site: &str, (id, name, brand, model, mounts, year, focal, aperture): Lens) -> Piece {
    Piece {
        kind: Kind::Lens,
        id,
        name: name.into(),
        brand: brand.map(Into::into),
        model: model.into(),
        url: format!("{site}/lenses/{}", slug(name)),
        mount: None,
        mounts: Some(mounts.iter().map(|m| m.to_string()).collect()),
        year_introduced: year,
        focal_length_mm: Some(Span { min: focal.0, max: focal.1 }),
        max_aperture: aperture.map(|(at_min_focal, at_max_focal)| Aperture { at_min_focal, at_max_focal }),
        built_in_lens: None,
    }
}

/// A page's name: the folded name's letters and digits, every other run one hyphen.
pub fn slug(name: &str) -> String {
    let folded = fold(name);
    let words: Vec<&str> = folded.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    words.join("-")
}

/// Lower case without the Latin accents, as the service matches words.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.to_lowercase().chars() {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => out.push('a'),
            'ç' | 'č' | 'ć' => out.push('c'),
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ě' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' | 'ī' => out.push('i'),
            'ñ' | 'ń' => out.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => out.push('o'),
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' => out.push('u'),
            'ý' | 'ÿ' => out.push('y'),
            'š' | 'ś' => out.push('s'),
            'ž' | 'ź' | 'ż' => out.push('z'),
            'ł' => out.push('l'),
            'ř' => out.push('r'),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            'ß' => out.push_str("ss"),
            other => out.push(other),
        }
    }
    out
}
