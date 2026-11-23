//! The order the menus list stocks and papers in, read off their names.

use std::cmp::Ordering;

/// Names compared as a person reads them: letters without case, numbers by their value, so 50D comes before 200T.
pub fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let number = |s: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut n = 0u64;
                    while let Some(d) = s.peek().and_then(|c| c.to_digit(10)) {
                        n = n.saturating_mul(10).saturating_add(u64::from(d));
                        s.next();
                    }
                    n
                };
                match number(&mut a).cmp(&number(&mut b)) {
                    Ordering::Equal => {}
                    o => return o,
                }
            }
            (Some(x), Some(y)) => match x.to_lowercase().cmp(y.to_lowercase()) {
                Ordering::Equal => {
                    a.next();
                    b.next();
                }
                o => return o,
            },
        }
    }
}
