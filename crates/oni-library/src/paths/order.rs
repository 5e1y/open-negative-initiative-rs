//! `localizedStandardCompare`, the Finder's order, read without a locale: letters first, then their
//! accents, then their case, runs of digits as numbers, and a name composed or decomposed alike.

use std::cmp::Ordering;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// One step of a name: a space or a sign, then a number, then a letter by its base, in that order.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    Sign(char),
    /// The digits' count with their leading zeros gone, then the digits: 2 before 10.
    Number(usize, String),
    Letter(char),
}

/// A name's steps, each with its accents and whether it was a capital, which break ties in turn.
#[derive(Debug, Default)]
struct Weighed {
    steps: Vec<Step>,
    accents: Vec<String>,
    capitals: Vec<bool>,
}

/// The base letters of a Latin letter that decomposition leaves whole.
fn bases(c: char) -> Vec<char> {
    match c {
        'æ' => vec!['a', 'e'],
        'œ' => vec!['o', 'e'],
        'ß' => vec!['s', 's'],
        'ø' => vec!['o'],
        'đ' => vec!['d'],
        'ł' => vec!['l'],
        'ı' => vec!['i'],
        _ => vec![c],
    }
}

fn weigh(name: &str) -> Weighed {
    let chars: Vec<char> = name.nfd().collect();
    let mut out = Weighed::default();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let digits: String = chars[start..i].iter().collect::<String>().trim_start_matches('0').to_owned();
            out.steps.push(Step::Number(digits.len(), digits));
            out.accents.push(String::new());
            out.capitals.push(false);
            continue;
        }
        i += 1;
        let mut marks = String::new();
        while i < chars.len() && is_combining_mark(chars[i]) {
            marks.push(chars[i]);
            i += 1;
        }
        if !c.is_alphabetic() || is_combining_mark(c) {
            out.steps.push(Step::Sign(c));
            out.accents.push(marks);
            out.capitals.push(false);
            continue;
        }
        let capital = c.is_uppercase();
        for lower in c.to_lowercase().filter(|l| !is_combining_mark(*l)) {
            for base in bases(lower) {
                out.steps.push(Step::Letter(base));
                out.accents.push(marks.clone());
                out.capitals.push(capital);
            }
        }
    }
    out
}

/// The Finder's order: `Été` among the E's, `Roll 2` before `Roll 10`, a small letter before its
/// capital, and two names only equal when they are one name in two compositions.
pub fn standard_order(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    if a.is_ascii() && b.is_ascii() {
        return ascii_order(a.as_bytes(), b.as_bytes());
    }
    let (x, y) = (weigh(a), weigh(b));
    x.steps
        .cmp(&y.steps)
        .then_with(|| x.accents.cmp(&y.accents))
        .then_with(|| x.capitals.cmp(&y.capitals))
        .then_with(|| a.nfc().cmp(b.nfc()))
}

/// `standard_order` for two ASCII names in one pass and no allocation, as a survey sorts thousands.
fn ascii_order(a: &[u8], b: &[u8]) -> Ordering {
    let class = |c: u8| if c.is_ascii_digit() { 1 } else if c.is_ascii_alphabetic() { 2 } else { 0 };
    let (mut i, mut j, mut case) = (0, 0, Ordering::Equal);
    loop {
        let (Some(&c), Some(&d)) = (a.get(i), b.get(j)) else {
            return (a.len() - i).cmp(&(b.len() - j)).then(case).then_with(|| a.cmp(b));
        };
        if class(c) == 1 && class(d) == 1 {
            let run = |s: &[u8], from: usize| from + s[from..].iter().take_while(|c| c.is_ascii_digit()).count();
            let bare = |digits: &[u8]| -> usize { digits.iter().take_while(|&&c| c == b'0').count() };
            let (m1, n1) = (run(a, i), run(b, j));
            let (m, n) = (&a[i + bare(&a[i..m1])..m1], &b[j + bare(&b[j..n1])..n1]);
            let order = m.len().cmp(&n.len()).then_with(|| m.cmp(n));
            if order != Ordering::Equal {
                return order;
            }
            (i, j) = (m1, n1);
            continue;
        }
        let order = class(c).cmp(&class(d)).then_with(|| c.to_ascii_lowercase().cmp(&d.to_ascii_lowercase()));
        if order != Ordering::Equal {
            return order;
        }
        if case == Ordering::Equal {
            case = c.is_ascii_uppercase().cmp(&d.is_ascii_uppercase());
        }
        (i, j) = (i + 1, j + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(names: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        out.sort_by(|a, b| standard_order(a, b));
        out
    }

    #[test]
    fn the_finder_order_reads_numbers_and_folds_case() {
        assert_eq!(standard_order("f2.RW2", "f10.RW2"), Ordering::Less);
        assert_eq!(standard_order("Roll 2", "Roll 10"), Ordering::Less);
        assert_eq!(standard_order("Roll b", "roll A"), Ordering::Greater);
        assert_eq!(standard_order("a", "A"), Ordering::Less, "a small letter before its capital, as ICU");
        assert_eq!(standard_order("A", "a"), Ordering::Greater);
        // Twin: a plain byte order puts f10 first and upper case before lower.
        assert_eq!("f2.RW2".cmp("f10.RW2"), Ordering::Greater);
        assert_eq!("Roll b".cmp("roll A"), Ordering::Less);
    }

    /// The pass for ASCII names orders as the general one, which a name with one accent takes.
    #[test]
    fn the_ascii_pass_orders_as_the_general_one() {
        let names = ["Roll 2", "roll 10", "Roll 02", "a", "A", "a_b", "a b", "a1", "a-1", "", "007", "7", "Zoo", "zoo2", "f10.RW2"];
        for a in names {
            for b in names {
                let (x, y) = (weigh(a), weigh(b));
                let general = x.steps.cmp(&y.steps).then_with(|| x.capitals.cmp(&y.capitals)).then_with(|| a.cmp(b));
                assert_eq!(standard_order(a, b), general, "{a:?} against {b:?}");
            }
        }
        // Twin: the pass that ignored digits as numbers would put Roll 10 before Roll 2.
        assert_eq!("roll 10".to_lowercase().cmp(&"roll 2".to_lowercase()), Ordering::Less);
    }

    #[test]
    fn standard_order_puts_ete_among_the_e() {
        let nfc = "\u{c9}t\u{e9} 2025";
        let nfd = "E\u{301}te\u{301} 2025";
        assert_ne!(nfc.as_bytes(), nfd.as_bytes());
        let expected_with = |ete: &str| -> Vec<String> {
            ["Archives", ete, "Etna", "Ezra", "F\u{e9}vrier", "Zoo"].iter().map(|n| n.to_string()).collect()
        };
        for ete in [nfc, nfd] {
            assert_eq!(sorted(&["Zoo", "F\u{e9}vrier", ete, "Ezra", "Archives", "Etna"]), expected_with(ete));
        }
        assert_eq!(standard_order(nfc, nfd), Ordering::Equal, "one name in two compositions");
        assert_eq!(standard_order("Etre", nfc), Ordering::Greater, "the letters decide before the accents");
        assert_eq!(standard_order("Ete 2025", nfc), Ordering::Less, "a bare letter before its accented twin");
        assert_eq!(sorted(&["Oeuvre", "\u{152}uf", "Ostende"]), ["\u{152}uf", "Oeuvre", "Ostende"]);
        assert_eq!(sorted(&["Ni\u{f1}o", "Nina", "Noir"]), ["Nina", "Ni\u{f1}o", "Noir"]);
        // Twin: the code points' order, case folded, puts the composed Été after Zoo and the
        // decomposed one between Ezra and Février, two places for one name.
        let points = |names: &[&str]| {
            let mut out: Vec<String> = names.iter().map(|n| n.to_string()).collect();
            out.sort_by_key(|n| n.to_lowercase());
            out
        };
        let listed = ["Zoo", "F\u{e9}vrier", "Ezra", "Archives", "Etna"];
        let with = |ete: &'static str| [&listed[..], &[ete]].concat();
        assert_eq!(points(&with(nfc)).last().map(String::as_str), Some(nfc));
        let decomposed = points(&with(nfd));
        let at = decomposed.iter().position(|n| n == nfd).unwrap();
        assert_eq!((decomposed[at - 1].as_str(), decomposed[at + 1].as_str()), ("Ezra", "F\u{e9}vrier"));
    }
}
