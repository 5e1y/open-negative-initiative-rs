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

/// Two digits at `at` read as a number no greater than `most`.
fn two(text: &str, at: usize, least: u32, most: u32) -> Option<u32> {
    let b = text.as_bytes().get(at..at + 2)?;
    let n = b.iter().all(u8::is_ascii_digit).then(|| u32::from(b[0] - b'0') * 10 + u32::from(b[1] - b'0'))?;
    (least..=most).contains(&n).then_some(n)
}

/// A typed date in EXIF's nineteen characters, the file's only form: a day, dashed or in EXIF's
/// colons, then its time to the minute or second, midnight when none. Empty writes no date.
pub(super) fn exif(typed: &str) -> Option<String> {
    let t = typed.trim();
    if t.is_empty() {
        return Some(String::new());
    }
    if !(dated(t, b'-') || dated(t, b':')) {
        return None;
    }
    let (month, day) = (two(t, 5, 1, 12)?, two(t, 8, 1, 31)?);
    let (hour, minute, second) = match t.len() {
        10 => (0, 0, 0),
        16 | 19 if t.as_bytes()[10] == b' ' && t.as_bytes()[13] == b':' => {
            let second = if t.len() == 19 { (t.as_bytes()[16] == b':').then(|| two(t, 17, 0, 59))?? } else { 0 };
            (two(t, 11, 0, 23)?, two(t, 14, 0, 59)?, second)
        }
        _ => return None,
    };
    Some(format!("{}:{month:02}:{day:02} {hour:02}:{minute:02}:{second:02}", &t[0..4]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EXIF's date shape: `YYYY:MM:DD HH:MM:SS`, nineteen characters.
    fn exifs(date: &str) -> bool {
        let b = date.as_bytes();
        b.len() == 19 && dated(date, b':') && b[10] == b' ' && b[13] == b':' && b[16] == b':'
    }

    #[test]
    fn the_field_shows_a_dashed_date_and_the_file_keeps_exifs() {
        assert_eq!(shown("2026:09:14 16:46:19"), "2026-09-14 16:46:19");
        assert_eq!(exif(&shown("2026:09:14 16:46:19")).as_deref(), Some("2026:09:14 16:46:19"), "a date left as shown is written as read");
        assert_eq!(shown("sometime in 2026"), "sometime in 2026");
        assert_eq!(shown(""), "");
        assert_ne!(shown("2026:09:14 16:46:19"), "2026:09:14 16:46:19", "the raw EXIF date is not what the field shows");
    }

    #[test]
    fn a_typed_date_reaches_the_file_in_exifs_nineteen_characters() {
        assert_eq!(exif("2026-09-14 16:46:19").as_deref(), Some("2026:09:14 16:46:19"));
        assert_eq!(exif("2026:09:14 16:46:19").as_deref(), Some("2026:09:14 16:46:19"), "a date pasted in EXIF's form is kept");
        assert_eq!(exif("2026-09-14 16:46").as_deref(), Some("2026:09:14 16:46:00"));
        assert_eq!(exif("2026-09-14").as_deref(), Some("2026:09:14 00:00:00"));
        assert_eq!(exif(" 2026-09-14 ").as_deref(), Some("2026:09:14 00:00:00"));
        assert_eq!(exif("").as_deref(), Some(""), "a cleared field writes no date");
        assert!(["2026-09-14 16:46:19", "2026-09-14", "2026-09-14 16:46"].iter().all(|t| exif(t).as_deref().is_some_and(exifs)));
        // Words, half a date, a day or an hour out of range, a time half typed: nothing is written.
        for typed in ["summer 1998", "2026-09-1", "14-09-2026", "2026-13-14", "2026-09-40", "2026-09-14 24:00", "2026-09-14 16:4", "2026-09-14T16:46", "2026-09-14 16:46:60"] {
            assert_eq!(exif(typed), None, "{typed}");
        }
        // Twin: the dashes swapped alone, as a field once wrote, leave ten characters EXIF refuses.
        assert!(!exifs(&"2026-09-14".replace('-', ":")));
    }
}
