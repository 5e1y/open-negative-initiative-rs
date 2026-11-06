//! Every field of `PipelineSettings` by its serialised name, the family each belongs to, and the
//! set of fields a write moved: what a view re-reads after a notch, and nothing more.

use oni_settings::PipelineSettings;
use oni_settings::settings::{look_settings, roll_settings};
use serde::de::{self, Deserialize, DeserializeOwned, Deserializer, Visitor};
use serde::Serialize;
use serde_json::Value;
use std::fmt;

macro_rules! fields {
    ($($variant:ident => $name:literal, $member:ident;)*) => {
        /// One field of `PipelineSettings`, in declaration order.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Field { $($variant,)* }

        impl Field {
            pub const ALL: &'static [Field] = &[$(Field::$variant,)*];

            /// The name the sidecar writes it under.
            pub fn name(self) -> &'static str {
                match self { $(Field::$variant => $name,)* }
            }

            /// True where the two settings hold a different value of this field.
            pub fn differs(self, a: &PipelineSettings, b: &PipelineSettings) -> bool {
                match self { $(Field::$variant => a.$member != b.$member,)* }
            }

            /// Carries this field's value from `from` onto `to`.
            pub fn carry(self, from: &PipelineSettings, to: &mut PipelineSettings) {
                match self { $(Field::$variant => to.$member = from.$member.clone(),)* }
            }

            /// Lays on `on` this field's values that moved from `before` to `after`, and no other.
            pub fn patch(self, before: &PipelineSettings, after: &PipelineSettings, on: &mut PipelineSettings) {
                match self { $(Field::$variant => patched(&before.$member, &after.$member, &mut on.$member),)* }
            }
        }
    };
}

fields! {
    Mode => "mode", mode;
    InvertedSource => "invertedSource", inverted_source;
    Film => "film", film;
    ManualOrder => "manualOrder", manual_order;
    Sheet => "sheet", sheet;
    DecodedFromMosaic => "decodedFromMosaic", decoded_from_mosaic;
    CaptureStops => "captureStops", capture_stops;
    LensCorrected => "lensCorrected", lens_corrected;
    PrimariesConverted => "primariesConverted", primaries_converted;
    SourceEncoding => "sourceEncoding", source_encoding;
    RgbPasses => "rgbPasses", rgb_passes;
    PassOf => "passOf", pass_of;
    Geometry => "geometry", geometry;
    Chroma => "chroma", chroma;
    Gains => "gains", gains;
    DensityCeiling => "densityCeiling", density_ceiling;
    Separation => "separation", separation;
    SeparationFamily => "separationFamily", separation_family;
    SeparationByHand => "separationByHand", separation_by_hand;
    FlatField => "flatField", flat_field;
    Corrections => "corrections", corrections;
    Levels => "levels", levels;
    ChromaDecompression => "chromaDecompression", chroma_decompression;
    ChromaDecompressionByHand => "chromaDecompressionByHand", chroma_decompression_by_hand;
    RollReference => "rollReference", roll_reference;
    RollReferenceDeclined => "rollReferenceDeclined", roll_reference_declined;
    MeasuredGains => "measuredGains", measured_gains;
    BalanceMethod => "balanceMethod", balance_method;
    BalanceFilters => "balanceFilters", balance_filters;
    BalanceStrength => "balanceStrength", balance_strength;
    Print => "print", print;
    Density => "density", density;
    ManualCurves => "manualCurves", manual_curves;
    Spectrogram => "spectrogram", spectrogram;
    Saturation => "saturation", saturation;
    Vignette => "vignette", vignette;
    SecretSauce => "secretSauce", secret_sauce;
    PaperCorrection => "paperCorrection", paper_correction;
    Face => "face", face;
    Scanner => "scanner", scanner;
    ScannerMachine => "scannerMachine", scanner_machine;
    ScannerFinish => "scannerFinish", scanner_finish;
    Texture => "texture", texture;
    Sharpen => "sharpen", sharpen;
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

    /// The family whose list names this field; `None` for a field placed nowhere.
    pub fn family(self) -> Option<Family> {
        let name = self.name();
        if roll_settings::FIELD_NAMES.contains(&name) {
            Some(Family::Roll)
        } else if look_settings::FIELDS.contains(&name) {
            Some(Family::Look)
        } else if look_settings::FRAME_FIELDS.contains(&name) {
            Some(Family::Frame)
        } else {
            None
        }
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
        Field::ALL.iter().copied().filter(|f| f.family() == Some(family)).collect()
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

/// What is wrong with a placement of `names` in three families: a field in none, or in two.
pub fn placement_faults(names: &[&str], families: [&[&str]; 3]) -> Vec<String> {
    let mut faults = Vec::new();
    for name in names {
        let homes = families.iter().filter(|f| f.contains(name)).count();
        if homes != 1 {
            faults.push(format!("{name} is placed in {homes} families"));
        }
    }
    for family in families {
        for name in family {
            if !names.contains(name) {
                faults.push(format!("{name} is placed but is no field"));
            }
        }
    }
    faults
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAMILIES: [&[&str]; 3] = [&roll_settings::FIELD_NAMES, &look_settings::FIELDS, &look_settings::FRAME_FIELDS];

    #[test]
    fn every_field_is_the_rolls_the_looks_or_the_frames_and_one_only() {
        let names = declared_names();
        assert_eq!(names.len(), 44, "{names:?}");
        assert_eq!(placement_faults(names, FAMILIES), Vec::<String>::new());
        // Twin: a field the families leave out, and one in two of them, are both refused.
        let mut grown = names.to_vec();
        grown.push("phantom");
        assert_eq!(placement_faults(&grown, FAMILIES), vec!["phantom is placed in 0 families".to_string()]);
        let look_twice: Vec<&str> = look_settings::FIELDS.iter().copied().chain(["mode"]).collect();
        let faults = placement_faults(names, [&roll_settings::FIELD_NAMES, &look_twice, &look_settings::FRAME_FIELDS]);
        assert_eq!(faults, vec!["mode is placed in 2 families".to_string()]);
    }

    #[test]
    fn the_field_table_names_every_declared_field_in_order() {
        let table: Vec<&str> = Field::ALL.iter().map(|f| f.name()).collect();
        assert_eq!(table, declared_names());
        assert!(Field::ALL.iter().all(|f| f.family().is_some()));
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
