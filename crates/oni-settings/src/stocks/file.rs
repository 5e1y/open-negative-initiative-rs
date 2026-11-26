//! A stock's or a paper's file as a person writes it: plain JSON, every field named, unknown ones refused so a
//! misspelt one is seen; a profile is kept as written, the pipeline reading its numbers.

use crate::film::FilmType;
use crate::virtual_film::{Balance, FilmEffects, StockReference};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

/// The format this build writes and reads; a file of a later one is refused, never half read.
pub const FORMAT: u32 = 1;

/// What a file describes, its `oni` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Describes {
    Film,
    Paper,
}

/// A film stock as a roll names it, with the virtual film it is where its data sheet is profiled.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmFile {
    pub oni: Describes,
    pub format: u32,
    /// The key a roll stores, never shown.
    pub key: String,
    /// Retired keys that read as this stock.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// A trade name, shown as written; a generic stock's is a description, translated.
    pub name: String,
    pub brand: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub generic: bool,
    #[serde(rename = "type")]
    pub kind: FilmType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iso: Option<u16>,
    /// The canister's SVG beside the file; none draws its type's generic one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canister: Option<String>,
    /// The picker lists it; a stock it no longer lists still reads on a roll that has it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub offered: bool,
    /// A whole line, its stock never named.
    #[serde(default, skip_serializing_if = "is_false")]
    pub line: bool,
    /// The scanned stock whose curve Lighting reads a roll on it back through; none in black and white or on a slide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dyes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub film: Option<FilmBlock>,
    /// The stock developed otherwise, a push: each a stock of its own on this one's brand, type and canister.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub developments: Vec<Development>,
    /// A dye set of no stock's own, the mean of the sets named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dye_set: Option<MeanSet>,
}

/// The virtual film a digital photo is shot on: its data sheet's profile, its light, its dyes and its reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmBlock {
    /// The key a digital photo stores; also its dye set's where `dyes` names it.
    pub key: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub balance: Balance,
    /// The scanned stock its negative is read through: its own key makes its profile that stock's.
    pub dyes: String,
    /// Derived from the profile when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<StockReference>,
    /// A layer offset a file of the person's own may still hold from before ONI 2.0, read and left aside: a neutral
    /// exposes every layer at the same log exposure (owner's call).
    #[serde(default, skip_serializing)]
    pub filtration: Option<serde::de::IgnoredAny>,
    /// A film of no colour of its own: no spectrum, no coupler, pure dyes; its profile's `data` holds `range`, the density
    /// from its bare film to the sensor's white, and its tone curve, `density` on `log_exposure` about a grey.
    #[serde(default, skip_serializing_if = "is_false")]
    pub neutral: bool,
    /// The data sheet's profile as `scripts/datasheet-profiles.py` writes it: `info`, `provenance` and `data`.
    pub profile: Box<RawValue>,
    /// Its own DIR coupler matrix, row the donor layer's density, column the receiving layer's log exposure; the common
    /// one where left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub couplers: Option<[[f64; 3]; 3]>,
    /// The rests a digital photo's film effects take on this film; one left out is the effect's own rest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<Effects>,
    /// A `.cube` file's look, which a digital photo on this film is read through in place of its negative.
    #[serde(skip)]
    pub look: Option<super::cube::Look>,
}

/// A film's own effects, each on the film: lengths in micrometres of a 135 frame.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    /// The share of a clipped light's exposure its glow adds, and the glow's deviation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halation: Option<Halation>,
    /// The emulsion's own blur, its deviation, as the sheet's MTF holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bloom: Option<f32>,
    /// Its grain: Grain's Amount and a grain's width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grain: Option<Grain>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grain {
    pub amount: f32,
    pub size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Halation {
    pub dose: f32,
    pub size: f32,
}

impl Effects {
    /// The rests a photo on this film takes, each effect the file leaves out at its own.
    pub fn rests(&self) -> FilmEffects {
        let own = FilmEffects::default();
        FilmEffects {
            halation: self.halation.map_or(own.halation, |h| crate::blocks::Halation { amount: h.dose, size: h.size }),
            bloom: self.bloom.map_or(own.bloom, |size| crate::blocks::Bloom { size }),
            grain: self.grain.map_or(own.grain, |g| crate::blocks::Grain { amount: g.amount, size: g.size }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Development {
    pub key: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iso: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dyes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub film: Option<FilmBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeanSet {
    pub key: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Film keys whose profiles' dyes are averaged at each wavelength most of them measure.
    pub mean_of: Vec<String>,
}

/// A colour paper: a data sheet's profile, or the Frontier SP-3000's fit of its measured table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaperFile {
    pub oni: Describes,
    pub format: u32,
    pub key: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub name: String,
    pub brand: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<Box<RawValue>>,
    /// The fit `docs/research/frontier/frontier_print_fit.py` writes, with its provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontier: Option<Box<RawValue>>,
}

/// One file read: a stock or a paper.
#[derive(Debug, Clone)]
pub enum File {
    Film(FilmFile),
    Paper(PaperFile),
}

fn yes() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Deserialize)]
struct Head {
    oni: Describes,
    format: u32,
}

/// The file `bytes` hold, or why it is refused: not JSON, a field missing or unknown, a later format, a key or a name
/// that cannot be stored.
pub fn read(bytes: &[u8]) -> Result<File, String> {
    let head: Head = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if head.format > FORMAT || head.format == 0 {
        return Err(format!("format {} is not one this ONI reads (it reads {FORMAT})", head.format));
    }
    let file = match head.oni {
        Describes::Film => File::Film(serde_json::from_slice(bytes).map_err(|e| e.to_string())?),
        Describes::Paper => File::Paper(serde_json::from_slice(bytes).map_err(|e| e.to_string())?),
    };
    file.checked()
}

impl File {
    /// The file's own key: a file of the person's with a shipped file's key stands in its place.
    pub fn key(&self) -> &str {
        match self {
            File::Film(f) => &f.key,
            File::Paper(p) => &p.key,
        }
    }

    pub fn to_json(&self) -> String {
        match self {
            File::Film(f) => serde_json::to_string_pretty(f),
            File::Paper(p) => serde_json::to_string_pretty(p),
        }
        .unwrap_or_default()
    }

    fn checked(self) -> Result<File, String> {
        match &self {
            File::Film(f) => {
                named(&f.key, &f.aliases, &f.name)?;
                text("brand", &f.brand)?;
                if let Some(c) = &f.canister {
                    canister(c)?;
                }
                for d in &f.developments {
                    named(&d.key, &d.aliases, &d.name)?;
                    if let Some(film) = &d.film {
                        block(film)?;
                    }
                }
                if let Some(film) = &f.film {
                    block(film)?;
                }
                if let Some(set) = &f.dye_set {
                    named(&set.key, &set.aliases, "mean")?;
                    if set.mean_of.is_empty() {
                        return Err("a dye set's mean_of names no film".into());
                    }
                }
            }
            File::Paper(p) => {
                named(&p.key, &p.aliases, &p.name)?;
                text("brand", &p.brand)?;
                if p.profile.is_some() && p.frontier.is_some() {
                    return Err("a paper holds a profile or a frontier fit, one of them".into());
                }
                if p.profile.is_none() && p.frontier.is_none() {
                    return Err("a paper holds a profile or a frontier fit".into());
                }
            }
        }
        Ok(self)
    }
}

fn block(film: &FilmBlock) -> Result<(), String> {
    named(&film.key, &film.aliases, "film")?;
    key(&film.dyes)?;
    match film.reference {
        Some(r) if !r.stops.iter().chain(&r.black).chain(&r.white).all(|v| v.is_finite()) => Err("a reference holding a number that is not finite".into()),
        _ => Ok(()),
    }
}

fn named(k: &str, aliases: &[String], name: &str) -> Result<(), String> {
    key(k)?;
    aliases.iter().try_for_each(|a| key(a))?;
    text("name", name)
}

/// A key a sidecar stores as is: letters, digits and hyphens.
fn key(k: &str) -> Result<(), String> {
    if k.is_empty() || k.len() > 80 || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(format!("the key \"{k}\" holds more than letters, digits and hyphens"));
    }
    Ok(())
}

fn text(field: &str, t: &str) -> Result<(), String> {
    if t.trim().is_empty() || t.len() > 120 {
        return Err(format!("the {field} is empty or longer than 120 characters"));
    }
    Ok(())
}

/// A canister's SVG lies beside the file: a bare name, never a path.
fn canister(name: &str) -> Result<(), String> {
    if !name.ends_with(".svg") || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(format!("the canister \"{name}\" is not an SVG's name beside the file"));
    }
    Ok(())
}
