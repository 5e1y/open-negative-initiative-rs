//! A big-endian TIFF directory, the one structure a TIFF file and a JPEG's Exif segment share.

/// One field's value, in the four TIFF types the export writes.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Short(Vec<u16>),
    Long(Vec<u32>),
    /// Written as its UTF-8 bytes and a closing NUL.
    Ascii(String),
    Undefined(Vec<u8>),
}

impl Value {
    fn kind(&self) -> u16 {
        match self {
            Value::Short(_) => 3,
            Value::Long(_) => 4,
            Value::Ascii(_) => 2,
            Value::Undefined(_) => 7,
        }
    }

    fn count(&self) -> u32 {
        match self {
            Value::Short(v) => v.len() as u32,
            Value::Long(v) => v.len() as u32,
            Value::Ascii(s) => s.len() as u32 + 1,
            Value::Undefined(v) => v.len() as u32,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        match self {
            Value::Short(v) => v.iter().flat_map(|x| x.to_be_bytes()).collect(),
            Value::Long(v) => v.iter().flat_map(|x| x.to_be_bytes()).collect(),
            Value::Ascii(s) => s.bytes().chain([0]).collect(),
            Value::Undefined(v) => v.clone(),
        }
    }
}

/// A directory's entries, kept in ascending tag order as TIFF requires.
#[derive(Debug, Clone, Default)]
pub struct Directory {
    entries: Vec<(u16, Value)>,
}

impl Directory {
    pub fn set(&mut self, tag: u16, value: Value) {
        match self.entries.binary_search_by_key(&tag, |e| e.0) {
            Ok(i) => self.entries[i].1 = value,
            Err(i) => self.entries.insert(i, (tag, value)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bytes the directory takes once laid, its out-of-line values included, word-aligned.
    pub fn len(&self) -> usize {
        let values: usize = self.entries.iter().map(|(_, v)| padded(v.bytes().len())).filter(|n| *n > 4).sum();
        2 + 12 * self.entries.len() + 4 + values
    }

    /// The directory laid at file offset `at` (even), its values right after it, `next` chaining
    /// another directory or 0.
    pub fn laid(&self, at: u32, next: u32) -> Vec<u8> {
        let mut head = Vec::with_capacity(self.len());
        let mut tail = Vec::new();
        let values_at = at as usize + 2 + 12 * self.entries.len() + 4;
        head.extend((self.entries.len() as u16).to_be_bytes());
        for (tag, value) in &self.entries {
            let bytes = value.bytes();
            head.extend(tag.to_be_bytes());
            head.extend(value.kind().to_be_bytes());
            head.extend(value.count().to_be_bytes());
            if bytes.len() <= 4 {
                let mut inline = bytes;
                inline.resize(4, 0);
                head.extend(inline);
            } else {
                head.extend(((values_at + tail.len()) as u32).to_be_bytes());
                tail.extend(&bytes);
                if tail.len() % 2 == 1 {
                    tail.push(0);
                }
            }
        }
        head.extend(next.to_be_bytes());
        head.extend(tail);
        head
    }
}

fn padded(n: usize) -> usize {
    n + n % 2
}
