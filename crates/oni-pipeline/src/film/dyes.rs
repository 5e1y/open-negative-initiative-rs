//! The stock profiles ONI ships, each written from its maker's data sheet, and the C-41 profiles whose mean stands for a
//! stock no profile measures.

/// The C-41 profiles the generic set averages: the nine still colour negatives most shot today.
pub const C41: [&str; 9] = [
    "kodak_portra_160",
    "kodak_portra_400",
    "kodak_portra_800",
    "kodak_ektar_100",
    "kodak_gold_200",
    "kodak_ultra_max_400",
    "fujifilm_c200",
    "fujifilm_pro_400h",
    "fujifilm_superia_x_tra_400",
];

/// A profile's file, written from its maker's data sheet.
pub fn source(name: &str) -> Option<&'static [u8]> {
    super::datasheet::PROFILES.iter().find(|(n, _)| *n == name).map(|(_, json)| *json)
}
