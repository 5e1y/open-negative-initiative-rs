//! Fitting the display face: the largest size at which a scene's giant words hold in their frame, and
//! where they break. The advances are Zalando Sans Expanded Black's own, in thousandths of an em.

const CAPS: [u16; 26] = [967, 900, 962, 960, 822, 809, 990, 931, 337, 565, 925, 759, 1088, 940, 977, 862, 1036, 908, 880, 852, 895, 983, 1287, 950, 906, 846];
const DIGITS: [u16; 10] = [913, 543, 847, 881, 890, 876, 885, 819, 880, 885];
/// A lower-case letter at its widest but `m` and `w`; a mark or a sign at the widest of those a figure holds.
const LOWER: u16 = 800;
const SIGN: u16 = 600;
const SPACE: u16 = 276;
/// Lines of capitals set solid: the face's capitals stand 0.714 em tall.
pub const LEADING: f32 = 0.9;

/// A character's advance; an accented capital is its letter's, a narrow space a space's.
fn advance(c: char) -> u16 {
    let plain = match c {
        'À'..='Å' => 'A',
        'Ç' => 'C',
        'È'..='Ë' => 'E',
        'Ì'..='Ï' => 'I',
        'Ñ' => 'N',
        'Ò'..='Ö' => 'O',
        'Ù'..='Ü' => 'U',
        other => other,
    };
    match plain {
        'A'..='Z' => CAPS[plain as usize - 'A' as usize],
        '0'..='9' => DIGITS[plain as usize - '0' as usize],
        'm' | 'w' | 'Œ' | 'Æ' | '%' | '#' | '@' => 1200,
        c if c.is_whitespace() => SPACE,
        c if c.is_alphabetic() => LOWER,
        _ => SIGN,
    }
}

/// A run's width in ems.
pub fn width(text: &str) -> f32 {
    text.chars().map(|c| f32::from(advance(c))).sum::<f32>() / 1000.0
}

/// `text`'s words broken into lines no wider than `ems`, each as full as it goes; a word wider than a
/// line stands alone on its own. A no-break space breaks nothing: a number's groups stay together.
fn broken(text: &str, ems: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split([' ', '\n']).filter(|word| !word.is_empty()) {
        match lines.last_mut() {
            Some(line) if width(line) + f32::from(SPACE) / 1000.0 + width(word) <= ems => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// The largest size, from `most` down to `least`, at which `text` holds in a box `across` wide and `down`
/// tall, and the text broken as that size breaks it. Sizes and the box are in one unit.
pub fn fit(text: &str, across: f32, down: f32, most: f32, least: f32) -> (f32, String) {
    let mut size = most;
    loop {
        let lines = broken(text, across / size);
        let holds = lines.iter().all(|l| width(l) * size <= across) && lines.len() as f32 * size * LEADING <= down;
        if holds || size <= least {
            return (size, lines.join("\n"));
        }
        size = (size - 2.0).max(least);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_figure_stands_giant_and_a_long_name_breaks_and_shrinks_to_hold() {
        // Two digits at the largest size: 1.74 em across, 261 units of the 292 they have.
        assert_eq!(fit("42", 292.0, 200.0, 150.0, 30.0), (150.0, "42".to_string()));
        assert_eq!(fit("42", 292.0, 100.0, 150.0, 30.0).0, 110.0, "a lower box sets it smaller: 110 x 0.9 is 99");
        // Four digits do not hold at 150: the size comes down until they do.
        let (size, text) = fit("1\u{202f}732", 292.0, 200.0, 150.0, 30.0);
        assert!(text == "1\u{202f}732" && width(&text) * size <= 292.0 && width(&text) * (size + 2.0) > 292.0, "{size}");
        // Twin: the same groups an ordinary space apart are two words, broken where they hold larger.
        assert_eq!(fit("1 732", 292.0, 200.0, 150.0, 30.0).1, "1\n732");
        // A name of two words breaks where a line is full, each line inside the box, the lines inside its height.
        let (size, text) = fit("KODAK GOLD", 292.0, 200.0, 150.0, 30.0);
        assert_eq!(text, "KODAK\nGOLD");
        assert!(text.lines().all(|l| width(l) * size <= 292.0) && 2.0 * size * LEADING <= 200.0, "{size}");
        let (small, long) = fit("THE NIGHT DEVELOPER", 292.0, 200.0, 150.0, 30.0);
        assert!(long.lines().count() >= 2 && long.lines().all(|l| width(l) * small <= 292.0) && long.lines().count() as f32 * small * LEADING <= 200.0, "{small} {long}");
        assert!(small < size, "the longer name is set smaller");
        // Twin: at the size the short figure takes, the long name runs out of its box.
        assert!(width("DEVELOPER") * 150.0 > 292.0);
        // A word no size holds is set at the least, on its own line.
        assert_eq!(fit("WWWWWWWWWWWWWWWWWWWW", 100.0, 100.0, 80.0, 30.0).0, 30.0);
    }

    #[test]
    fn an_accented_capital_is_as_wide_as_its_letter() {
        assert_eq!(width("AOÛT"), width("AOUT"));
        assert!((width("É") - 0.822).abs() < 1e-6);
        assert!(width("I") < width("M"));
    }
}
