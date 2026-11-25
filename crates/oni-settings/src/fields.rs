//! Every field of `PipelineSettings` by its serialised name, the family each belongs to, and the
//! set of fields a write moved: what a view re-reads after a notch, and nothing more.

use crate::PipelineSettings;
use serde::de::{self, Deserialize, DeserializeOwned, Deserializer, Visitor};
use serde::Serialize;
use serde_json::Value;
use std::fmt;

mod ordered;

macro_rules! fields {
    ($($variant:ident => $name:literal, $member:ident, $family:ident;)*) => {
        /// One field of `PipelineSettings`, in declaration order.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Field { $($variant,)* }

        impl Field {
            pub const ALL: &'static [Field] = &[$(Field::$variant,)*];

            /// The name the sidecar writes it under.
            pub fn name(self) -> &'static str {
                match self { $(Field::$variant => $name,)* }
            }

            /// Who owns this field, named once beside it in the table.
            pub fn family(self) -> Family {
                match self { $(Field::$variant => Family::$family,)* }
            }

            /// True where the two settings hold a different value of this field.
            pub fn differs(self, a: &PipelineSettings, b: &PipelineSettings) -> bool {
                match self { $(Field::$variant => a.$member != b.$member,)* }
            }

            /// Carries this field's value from `from` onto `to`.
            pub fn carry(self, from: &PipelineSettings, to: &mut PipelineSettings) {
                match self { $(Field::$variant => to.$member = from.$member.clone(),)* }
            }

            fn patched_leaves(self, before: &PipelineSettings, after: &PipelineSettings, on: &mut PipelineSettings) {
                match self { $(Field::$variant => patched(&before.$member, &after.$member, &mut on.$member),)* }
            }
        }
    };
}

fields! {
    Mode => "mode", mode, Roll;
    InvertedSource => "invertedSource", inverted_source, Roll;
    VirtualFilm => "virtualFilm", virtual_film, Roll;
    Film => "film", film, Frame;
    ManualOrder => "manualOrder", manual_order, Frame;
    Sheet => "sheet", sheet, Frame;
    DecodedFromMosaic => "decodedFromMosaic", decoded_from_mosaic, Frame;
    CaptureStops => "captureStops", capture_stops, Frame;
    PrimariesConverted => "primariesConverted", primaries_converted, Frame;
    SourceEncoding => "sourceEncoding", source_encoding, Roll;
    RgbPasses => "rgbPasses", rgb_passes, Frame;
    PassOf => "passOf", pass_of, Frame;
    Geometry => "geometry", geometry, Frame;
    Chroma => "chroma", chroma, Look;
    Gains => "gains", gains, Roll;
    DensityCeiling => "densityCeiling", density_ceiling, Roll;
    DyeSet => "dyeSet", dye_set, Roll;
    FlatField => "flatField", flat_field, Roll;
    Corrections => "corrections", corrections, Frame;
    Levels => "levels", levels, Roll;
    ChromaDecompression => "chromaDecompression", chroma_decompression, Roll;
    ChromaDecompressionByHand => "chromaDecompressionByHand", chroma_decompression_by_hand, Roll;
    RollReference => "rollReference", roll_reference, Frame;
    MeasuredGains => "measuredGains", measured_gains, Frame;
    BalanceMethod => "balanceMethod", balance_method, Frame;
    BalanceFilters => "balanceFilters", balance_filters, Look;
    BalanceStrength => "balanceStrength", balance_strength, Roll;
    Print => "print", print, Look;
    PrintPaper => "printPaper", print_paper, Look;
    LegacyPrint => "legacyPrint", legacy_print, Look;
    Density => "density", density, Look;
    ManualCurves => "manualCurves", manual_curves, Look;
    Spectrogram => "spectrogram", spectrogram, Look;
    Saturation => "saturation", saturation, Look;
    Vignette => "vignette", vignette, Look;
    SecretSauce => "secretSauce", secret_sauce, Look;
    PaperCorrection => "paperCorrection", paper_correction, Look;
    Texture => "texture", texture, Look;
    Sharpen => "sharpen", sharpen, Look;
    Grain => "grain", grain, Look;
    Halation => "halation", halation, Look;
    Bloom => "bloom", bloom, Look;
    LensCorrection => "lensCorrection", lens_correction, Look;
}

impl Field {
    /// Lays on `on` this field's values that moved from `before` to `after`, and no other; a curve's
    /// point moved lands through the curve's own move, held between that frame's neighbours.
    pub fn patch(self, before: &PipelineSettings, after: &PipelineSettings, on: &mut PipelineSettings) {
        let own = ordered::bends(self).then(|| on.clone());
        self.patched_leaves(before, after, on);
        if let Some(own) = own {
            ordered::ordered(self, (before, after), &own, on);
        }
    }
}

/// `on` takes what moved from `before` to `after`, value by value: a list that changed its length, or
/// is another length on `on`, whole; one JSON cannot hold, as NaN, whole too.
fn patched<T: Serialize + DeserializeOwned + Clone>(before: &T, after: &T, on: &mut T) {
    let (Ok(b), Ok(a), Ok(mut o)) = (serde_json::to_value(before), serde_json::to_value(after), serde_json::to_value(&*on)) else {
        *on = after.clone();
        return;
    };
    leaves(&b, &a, &mut o);
    *on = serde_json::from_value(o).unwrap_or_else(|_| after.clone());
}

fn leaves(before: &Value, after: &Value, on: &mut Value) {
    match (before, after, on) {
        (Value::Object(b), Value::Object(a), Value::Object(o)) => {
            for (key, value) in a {
                match (b.get(key), o.get_mut(key)) {
                    (Some(was), _) if was == value => {}
                    (Some(was), Some(own)) => leaves(was, value, own),
                    _ => _ = o.insert(key.clone(), value.clone()),
                }
            }
            for key in b.keys().filter(|k| !a.contains_key(*k)) {
                o.remove(key);
            }
        }
        (Value::Array(b), Value::Array(a), Value::Array(o)) if b.len() == a.len() && a.len() == o.len() => {
            for ((was, value), own) in b.iter().zip(a).zip(o.iter_mut()).filter(|((was, value), _)| was != value) {
                leaves(was, value, own);
            }
        }
        (_, a, o) => *o = a.clone(),
    }
}

/// Who owns a field: the roll's pane, the look a paste carries, or the frame itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    Roll,
    Look,
    Frame,
}

impl Field {
    pub fn from_name(name: &str) -> Option<Field> {
        Field::ALL.iter().copied().find(|f| f.name() == name)
    }

    fn bit(self) -> u64 {
        1 << (self as u32)
    }
}

/// A set of fields as one word, so a notch's report costs nothing to build or to read.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FieldSet(u64);

impl FieldSet {
    pub const EMPTY: FieldSet = FieldSet(0);

    pub fn all() -> FieldSet {
        Field::ALL.iter().copied().collect()
    }

    /// The fields that differ between two settings.
    pub fn between(a: &PipelineSettings, b: &PipelineSettings) -> FieldSet {
        Field::ALL.iter().copied().filter(|f| f.differs(a, b)).collect()
    }

    pub fn of_family(family: Family) -> FieldSet {
        Field::ALL.iter().copied().filter(|f| f.family() == family).collect()
    }

    pub fn contains(self, field: Field) -> bool {
        self.0 & field.bit() != 0
    }

    pub fn insert(&mut self, field: Field) {
        self.0 |= field.bit();
    }

    pub fn union(self, other: FieldSet) -> FieldSet {
        FieldSet(self.0 | other.0)
    }

    pub fn intersects(self, other: FieldSet) -> bool {
        self.0 & other.0 != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn iter(self) -> impl Iterator<Item = Field> {
        Field::ALL.iter().copied().filter(move |f| self.contains(*f))
    }
}

impl FromIterator<Field> for FieldSet {
    fn from_iter<I: IntoIterator<Item = Field>>(iter: I) -> Self {
        let mut out = FieldSet::EMPTY;
        for field in iter {
            out.insert(field);
        }
        out
    }
}

impl fmt::Debug for FieldSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter().map(Field::name)).finish()
    }
}

/// The serialised field names `PipelineSettings` itself declares, read off its `Deserialize`, so a
/// field added to the struct appears here without anyone listing it.
pub fn declared_names() -> &'static [&'static str] {
    struct Capture;
    #[derive(Debug)]
    struct Fields(&'static [&'static str]);
    impl fmt::Display for Fields {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "fields captured")
        }
    }
    impl std::error::Error for Fields {}
    impl de::Error for Fields {
        fn custom<T: fmt::Display>(_: T) -> Self {
            Fields(&[])
        }
    }
    impl<'de> Deserializer<'de> for Capture {
        type Error = Fields;
        fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Fields> {
            Err(Fields(&[]))
        }
        fn deserialize_struct<V: Visitor<'de>>(
            self,
            _: &'static str,
            fields: &'static [&'static str],
            _: V,
        ) -> Result<V::Value, Fields> {
            Err(Fields(fields))
        }
        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit
            unit_struct newtype_struct seq tuple tuple_struct map enum identifier ignored_any
        }
    }
    match PipelineSettings::deserialize(Capture) {
        Err(Fields(names)) => names,
        Ok(_) => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_table_names_every_declared_field_in_order() {
        let table: Vec<&str> = Field::ALL.iter().map(|f| f.name()).collect();
        assert_eq!(table, declared_names());
        // Twin: a name the struct does not declare finds no field.
        assert_eq!(Field::from_name("phantom"), None);
        assert_eq!(Field::from_name("sharpen"), Some(Field::Sharpen));
    }

    #[test]
    fn a_patch_lays_the_values_moved_and_leaves_the_frame_s_own() {
        let a = PipelineSettings::default();
        let mut b = a.clone();
        b.print.exposure = 0.5;
        b.gains.stops[2] = 0.4;
        let mut own = a.clone();
        own.print.contrast = 0.3;
        own.print.exposure = -0.2;
        own.gains.stops = [0.1, 0.2, 0.3];
        let mut patched = own.clone();
        Field::Print.patch(&a, &b, &mut patched);
        Field::Gains.patch(&a, &b, &mut patched);
        assert_eq!((patched.print.exposure, patched.print.contrast), (0.5, 0.3));
        assert_eq!(patched.gains.stops, [0.1, 0.2, 0.4]);
        // Twin: carrying the field whole takes the contrast and the other gains too.
        let mut carried = own.clone();
        Field::Print.carry(&b, &mut carried);
        Field::Gains.carry(&b, &mut carried);
        assert_ne!(carried.print.contrast, 0.3);
        assert_ne!(carried.gains.stops, [0.1, 0.2, 0.4]);
    }

    #[test]
    fn a_change_reports_its_own_fields_alone() {
        let a = PipelineSettings::default();
        let mut b = a.clone();
        b.print.exposure += 0.5;
        b.sharpen.amount = a.sharpen.amount + 0.5;
        let moved = FieldSet::between(&a, &b);
        assert_eq!(moved.iter().collect::<Vec<_>>(), vec![Field::Print, Field::Sharpen]);
        // Twin: identical settings move nothing, and carrying a field closes its difference.
        assert!(FieldSet::between(&a, &a).is_empty());
        let mut c = a.clone();
        Field::Print.carry(&b, &mut c);
        assert_eq!(FieldSet::between(&c, &b).iter().collect::<Vec<_>>(), vec![Field::Sharpen]);
    }
}
