use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Mode,
    SourceEncoding,
    FlatField,
    Gains,
    DensityCeiling,
    Levels,
    Decompression,
    DyeSet,
    BalanceStrength,
}

impl Field {
    pub const ALL: [Field; 9] = [
        Field::Mode,
        Field::SourceEncoding,
        Field::FlatField,
        Field::Gains,
        Field::DensityCeiling,
        Field::Levels,
        Field::Decompression,
        Field::DyeSet,
        Field::BalanceStrength,
    ];
}

pub fn changed(before: &PipelineSettings, after: &PipelineSettings) -> BTreeSet<Field> {
    Field::ALL
        .into_iter()
        .filter(|field| match field {
            Field::Mode => {
                (before.mode, before.inverted_source, before.virtual_film) != (after.mode, after.inverted_source, after.virtual_film)
            }
            Field::SourceEncoding => before.source_encoding != after.source_encoding,
            Field::FlatField => before.flat_field != after.flat_field,
            Field::Gains => before.gains != after.gains,
            Field::DensityCeiling => before.density_ceiling != after.density_ceiling,
            Field::Levels => before.levels != after.levels,
            Field::Decompression => {
                (before.chroma_decompression, before.chroma_decompression_by_hand)
                    != (after.chroma_decompression, after.chroma_decompression_by_hand)
            }
            Field::DyeSet => before.dye_set != after.dye_set,
            Field::BalanceStrength => before.balance_strength != after.balance_strength,
        })
        .collect()
}

/// Balance strength re-doses the filters a frame no key balanced leaves alone.
pub fn dosed(settings: &PipelineSettings, strength: f32) -> PipelineSettings {
    let mut out = settings.clone();
    out.balance_strength = strength.clamp(filter_balance::STRENGTH_RANGE.0, filter_balance::STRENGTH_RANGE.1);
    if let Some(full) = settings.balance_filters {
        let s = out.balance_strength;
        out.print.red = Tone::exposure(full[0] * s);
        out.print.green = Tone::exposure(full[1] * s);
        out.print.blue = Tone::exposure(full[2] * s);
    }
    out
}

/// `target` carrying `donor`'s value of each field; a contact sheet never takes a second flat-field.
pub fn copied(fields: &BTreeSet<Field>, donor: &PipelineSettings, target: &PipelineSettings) -> PipelineSettings {
    let mut out = target.clone();
    for field in fields {
        if *field == Field::FlatField && target.sheet.is_some() {
            continue;
        }
        match field {
            // The frame's own values still on its mode's rest take the donor's mode's.
            Field::Mode => {
                out = out.carried(donor.mode);
                out.inverted_source = donor.inverted_source;
                out.virtual_film = donor.virtual_film;
            }
            Field::SourceEncoding => out.source_encoding = donor.source_encoding,
            Field::FlatField => out.flat_field = donor.flat_field.clone(),
            Field::Gains => out.gains = donor.gains,
            Field::DensityCeiling => out.density_ceiling = donor.density_ceiling,
            Field::Levels => out.levels = donor.levels,
            Field::Decompression => {
                out.chroma_decompression = donor.chroma_decompression;
                out.chroma_decompression_by_hand = donor.chroma_decompression_by_hand;
            }
            Field::DyeSet => out.dye_set = donor.dye_set,
            Field::BalanceStrength => out = dosed(&out, donor.balance_strength),
        }
    }
    on_own_film(target, out)
}

/// `out`, a frame once `target`, reading its negative on its own film's gains, windows and dyes: a digital photo's
/// reference is its film's, so a roll never lays another film's on it, and a film it newly takes is placed on its own.
pub fn on_own_film(target: &PipelineSettings, mut out: PipelineSettings) -> PipelineSettings {
    let Some(film) = out.film() else { return out };
    let own = if target.film().map(|f| f.stock) == Some(film.stock) { target.clone() } else { out.on_stock(film.stock) };
    (out.gains, out.levels, out.dye_set) = (own.gains, own.levels, own.dye_set);
    out
}
