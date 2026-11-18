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
    Separation,
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
        Field::Separation,
        Field::BalanceStrength,
    ];
}

pub const FIELD_NAMES: [&str; 13] = [
    "mode",
    "invertedSource",
    "sourceEncoding",
    "flatField",
    "gains",
    "densityCeiling",
    "levels",
    "chromaDecompression",
    "chromaDecompressionByHand",
    "separation",
    "separationFamily",
    "separationByHand",
    "balanceStrength",
];

pub fn changed(before: &PipelineSettings, after: &PipelineSettings) -> BTreeSet<Field> {
    Field::ALL
        .into_iter()
        .filter(|field| match field {
            Field::Mode => before.mode != after.mode || before.inverted_source != after.inverted_source,
            Field::SourceEncoding => before.source_encoding != after.source_encoding,
            Field::FlatField => before.flat_field != after.flat_field,
            Field::Gains => before.gains != after.gains,
            Field::DensityCeiling => before.density_ceiling != after.density_ceiling,
            Field::Levels => before.levels != after.levels,
            Field::Decompression => {
                (before.chroma_decompression, before.chroma_decompression_by_hand)
                    != (after.chroma_decompression, after.chroma_decompression_by_hand)
            }
            Field::Separation => {
                before.separation != after.separation
                    || before.separation_by_hand != after.separation_by_hand
                    || before.separation_family != after.separation_family
            }
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
            Field::Mode => {
                out.mode = donor.mode;
                out.inverted_source = donor.inverted_source;
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
            Field::Separation => {
                out.separation = donor.separation;
                out.separation_by_hand = donor.separation_by_hand;
                out.separation_family = donor.separation_family;
            }
            Field::BalanceStrength => out = dosed(&out, donor.balance_strength),
        }
    }
    out
}
