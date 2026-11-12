//! The capture date as a person reads it: the file and the sidecar keep EXIF's `2026:09:14 16:46:19`,
//! the field shows and takes `2026-09-14 16:46:19`.

/// Whether `text` opens on a year, a month and a day set apart by `mark`.
fn dated(text: &str, mark: u8) -> bool {
    let b = text.as_bytes();
    b.len() >= 10 && b[4] == mark && b[7] == mark && [0..4, 5..7, 8..10].into_iter().all(|r| b[r].iter().all(u8::is_ascii_digit))
}

/// The date's first ten bytes with `mark` between its year, month and day, the rest as it stands.
fn marked(text: &str, mark: char) -> String {
    format!("{}{mark}{}{mark}{}{}", &text[0..4], &text[5..7], &text[8..10], &text[10..])
}

/// An EXIF date as the field shows it; anything else as it stands.
pub(super) fn shown(exif: &str) -> String {
    if dated(exif, b':') { marked(exif, '-') } else { exif.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_shows_a_dashed_date_and_the_file_keeps_exifs() {
        assert_eq!(shown("2026:09:14 16:46:19"), "2026-09-14 16:46:19");
        assert_eq!(oni_settings::exif::date_of(&shown("2026:09:14 16:46:19")).as_deref(), Some("2026:09:14 16:46:19"), "a date left as shown is written as read");
        assert_eq!(shown("sometime in 2026"), "sometime in 2026");
        assert_eq!(shown(""), "");
        assert_ne!(shown("2026:09:14 16:46:19"), "2026:09:14 16:46:19", "the raw EXIF date is not what the field shows");
    }
}
