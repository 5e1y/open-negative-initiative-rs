//! The JSON text the Swift app writes: `JSONEncoder` with `.prettyPrinted` and `.sortedKeys`.
//! Values are first serialised to a typed tree, so a `Float` keeps its own shortest digits.

use serde::ser::{self, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::fmt;

/// A serialised value that remembers whether a number was a `Float` or a `Double`.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    F32(f32),
    F64(f64),
    Str(String),
    Arr(Vec<Node>),
    Obj(Vec<(String, Node)>),
}

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

pub fn to_node<T: Serialize + ?Sized>(value: &T) -> Result<Node, Error> {
    value.serialize(NodeSerializer)
}

impl Node {
    /// The tree as a `serde_json::Value`; a `Float` widens exactly, so it narrows back to itself.
    pub fn to_value(&self) -> Value {
        match self {
            Node::Null => Value::Null,
            Node::Bool(b) => Value::Bool(*b),
            Node::Int(i) => Value::from(*i),
            Node::UInt(u) => Value::from(*u),
            Node::F32(f) => serde_json::Number::from_f64(f64::from(*f)).map_or(Value::Null, Value::Number),
            Node::F64(f) => serde_json::Number::from_f64(*f).map_or(Value::Null, Value::Number),
            Node::Str(s) => Value::String(s.clone()),
            Node::Arr(items) => Value::Array(items.iter().map(Node::to_value).collect()),
            Node::Obj(entries) => {
                Value::Object(entries.iter().map(|(k, v)| (k.clone(), v.to_value())).collect())
            }
        }
    }
}

struct NodeSerializer;

pub struct SeqBuilder(Vec<Node>);
pub struct VariantSeqBuilder(&'static str, Vec<Node>);
pub struct MapBuilder(Vec<(String, Node)>, Option<String>);
pub struct VariantMapBuilder(&'static str, Vec<(String, Node)>);

impl ser::Serializer for NodeSerializer {
    type Ok = Node;
    type Error = Error;
    type SerializeSeq = SeqBuilder;
    type SerializeTuple = SeqBuilder;
    type SerializeTupleStruct = SeqBuilder;
    type SerializeTupleVariant = VariantSeqBuilder;
    type SerializeMap = MapBuilder;
    type SerializeStruct = MapBuilder;
    type SerializeStructVariant = VariantMapBuilder;

    fn serialize_bool(self, v: bool) -> Result<Node, Error> {
        Ok(Node::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<Node, Error> {
        Ok(Node::Int(v.into()))
    }
    fn serialize_i16(self, v: i16) -> Result<Node, Error> {
        Ok(Node::Int(v.into()))
    }
    fn serialize_i32(self, v: i32) -> Result<Node, Error> {
        Ok(Node::Int(v.into()))
    }
    fn serialize_i64(self, v: i64) -> Result<Node, Error> {
        Ok(Node::Int(v))
    }
    fn serialize_u8(self, v: u8) -> Result<Node, Error> {
        Ok(Node::UInt(v.into()))
    }
    fn serialize_u16(self, v: u16) -> Result<Node, Error> {
        Ok(Node::UInt(v.into()))
    }
    fn serialize_u32(self, v: u32) -> Result<Node, Error> {
        Ok(Node::UInt(v.into()))
    }
    fn serialize_u64(self, v: u64) -> Result<Node, Error> {
        Ok(Node::UInt(v))
    }
    fn serialize_f32(self, v: f32) -> Result<Node, Error> {
        if !v.is_finite() {
            return Err(Error(format!("{v} is not representable in JSON")));
        }
        Ok(Node::F32(v))
    }
    fn serialize_f64(self, v: f64) -> Result<Node, Error> {
        if !v.is_finite() {
            return Err(Error(format!("{v} is not representable in JSON")));
        }
        Ok(Node::F64(v))
    }
    fn serialize_char(self, v: char) -> Result<Node, Error> {
        Ok(Node::Str(v.to_string()))
    }
    fn serialize_str(self, v: &str) -> Result<Node, Error> {
        Ok(Node::Str(v.to_owned()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Node, Error> {
        Ok(Node::Arr(v.iter().map(|b| Node::UInt((*b).into())).collect()))
    }
    fn serialize_none(self) -> Result<Node, Error> {
        Ok(Node::Null)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Node, Error> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<Node, Error> {
        Ok(Node::Null)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<Node, Error> {
        Ok(Node::Null)
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, variant: &'static str) -> Result<Node, Error> {
        Ok(Node::Str(variant.to_owned()))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, value: &T) -> Result<Node, Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Node, Error> {
        Ok(Node::Obj(vec![(variant.to_owned(), value.serialize(NodeSerializer)?)]))
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<SeqBuilder, Error> {
        Ok(SeqBuilder(Vec::with_capacity(len.unwrap_or(0))))
    }
    fn serialize_tuple(self, len: usize) -> Result<SeqBuilder, Error> {
        Ok(SeqBuilder(Vec::with_capacity(len)))
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<SeqBuilder, Error> {
        Ok(SeqBuilder(Vec::with_capacity(len)))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<VariantSeqBuilder, Error> {
        Ok(VariantSeqBuilder(variant, Vec::new()))
    }
    fn serialize_map(self, _: Option<usize>) -> Result<MapBuilder, Error> {
        Ok(MapBuilder(Vec::new(), None))
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<MapBuilder, Error> {
        Ok(MapBuilder(Vec::new(), None))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<VariantMapBuilder, Error> {
        Ok(VariantMapBuilder(variant, Vec::new()))
    }
}

impl ser::SerializeSeq for SeqBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.0.push(value.serialize(NodeSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Arr(self.0))
    }
}

impl ser::SerializeTuple for SeqBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Arr(self.0))
    }
}

impl ser::SerializeTupleStruct for SeqBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Arr(self.0))
    }
}

impl ser::SerializeTupleVariant for VariantSeqBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.1.push(value.serialize(NodeSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Obj(vec![(self.0.to_owned(), Node::Arr(self.1))]))
    }
}

impl ser::SerializeMap for MapBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        match key.serialize(NodeSerializer)? {
            Node::Str(s) => {
                self.1 = Some(s);
                Ok(())
            }
            other => Err(Error(format!("a JSON key must be a string, not {other:?}"))),
        }
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let key = self.1.take().ok_or_else(|| Error("a map value came without its key".into()))?;
        self.0.push((key, value.serialize(NodeSerializer)?));
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Obj(self.0))
    }
}

impl ser::SerializeStruct for MapBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error> {
        self.0.push((key.to_owned(), value.serialize(NodeSerializer)?));
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Obj(self.0))
    }
}

impl ser::SerializeStructVariant for VariantMapBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error> {
        self.1.push((key.to_owned(), value.serialize(NodeSerializer)?));
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Obj(vec![(self.0.to_owned(), Node::Obj(self.1))]))
    }
}

// MARK: - The text

/// Foundation's `.sortedKeys` order: case-insensitive, then by code point to break a tie.
pub fn key_order(a: &str, b: &str) -> Ordering {
    let fold = |s: &str| s.chars().flat_map(char::to_lowercase).collect::<String>();
    fold(a).cmp(&fold(b)).then_with(|| a.cmp(b))
}

/// Swift's `description` digits for a finite number: decimal between 1e-4 and `big`,
/// exponential (`1e-05`, `1.5e+16`) outside, and a trailing `.0` dropped as `JSONEncoder` does.
fn swift_number(scientific: String, magnitude: f64, big: f64) -> String {
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let (negative, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let sign = if negative { "-" } else { "" };
    if magnitude != 0.0 && (exponent < -4 || magnitude > big) {
        let head = &digits[..1];
        let tail = &digits[1..];
        let mut out = format!("{sign}{head}");
        if !tail.is_empty() {
            out.push('.');
            out.push_str(tail);
        }
        let (exp_sign, exp_abs) = if exponent < 0 { ('-', -exponent) } else { ('+', exponent) };
        out.push_str(&format!("e{exp_sign}{exp_abs:02}"));
        return out;
    }
    let text = if exponent >= 0 {
        let whole_len = exponent as usize + 1;
        let (whole, fraction) = if digits.len() > whole_len {
            (digits[..whole_len].to_owned(), digits[whole_len..].to_owned())
        } else {
            (format!("{digits:0<whole_len$}"), String::new())
        };
        if fraction.is_empty() { whole } else { format!("{whole}.{fraction}") }
    } else {
        format!("0.{}{}", "0".repeat((-exponent - 1) as usize), digits)
    };
    format!("{sign}{text}")
}

pub fn format_f32(v: f32) -> String {
    swift_number(format!("{v:e}"), f64::from(v.abs()), 33_554_432.0)
}

pub fn format_f64(v: f64) -> String {
    swift_number(format!("{v:e}"), v.abs(), 18_014_398_509_481_984.0)
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_node(out: &mut String, node: &Node, pretty: bool, sorted: bool, depth: usize) {
    let indent = |out: &mut String, depth: usize| {
        if pretty {
            out.push_str(&"  ".repeat(depth));
        }
    };
    match node {
        Node::Null => out.push_str("null"),
        Node::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Node::Int(i) => out.push_str(&i.to_string()),
        Node::UInt(u) => out.push_str(&u.to_string()),
        Node::F32(f) => out.push_str(&format_f32(*f)),
        Node::F64(f) => out.push_str(&format_f64(*f)),
        Node::Str(s) => write_string(out, s),
        Node::Arr(items) => {
            out.push('[');
            if pretty {
                out.push('\n');
            }
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(if pretty { ",\n" } else { "," });
                }
                indent(out, depth + 1);
                write_node(out, item, pretty, sorted, depth + 1);
            }
            if pretty {
                out.push('\n');
                indent(out, depth);
            }
            out.push(']');
        }
        Node::Obj(entries) => {
            let mut order: Vec<&(String, Node)> = entries.iter().collect();
            if sorted {
                order.sort_by(|a, b| key_order(&a.0, &b.0));
            }
            out.push('{');
            if pretty {
                out.push('\n');
            }
            for (i, (key, value)) in order.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(if pretty { ",\n" } else { "," });
                }
                indent(out, depth + 1);
                write_string(out, key);
                out.push_str(if pretty { " : " } else { ":" });
                write_node(out, value, pretty, sorted, depth + 1);
            }
            if pretty {
                out.push('\n');
                indent(out, depth);
            }
            out.push('}');
        }
    }
}

/// `JSONEncoder` with `[.prettyPrinted, .sortedKeys]`: the sidecar's text.
pub fn pretty_sorted(node: &Node) -> String {
    let mut out = String::new();
    write_node(&mut out, node, true, true, 0);
    out
}

/// A plain `JSONEncoder`: compact, keys in declaration order.
pub fn compact(node: &Node) -> String {
    let mut out = String::new();
    write_node(&mut out, node, false, false, 0);
    out
}

// MARK: - Semantic equality

/// The first place two documents part, `None` when they agree: same keys, strings and booleans, and
/// numbers within an `f32` epsilon once read as the `f32` they store, whatever their decimal spelling.
pub fn semantic_difference(a: &Value, b: &Value) -> Option<String> {
    fn walk(a: &Value, b: &Value, path: &str) -> Option<String> {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                for key in x.keys().chain(y.keys()) {
                    let here = format!("{path}.{key}");
                    match (x.get(key), y.get(key)) {
                        (Some(p), Some(q)) => {
                            if let Some(found) = walk(p, q, &here) {
                                return Some(found);
                            }
                        }
                        (Some(_), None) => return Some(format!("{here}: only in the first")),
                        (None, Some(_)) => return Some(format!("{here}: only in the second")),
                        (None, None) => {}
                    }
                }
                None
            }
            (Value::Array(x), Value::Array(y)) => {
                if x.len() != y.len() {
                    return Some(format!("{path}: {} elements against {}", x.len(), y.len()));
                }
                x.iter().zip(y).enumerate().find_map(|(i, (p, q))| walk(p, q, &format!("{path}[{i}]")))
            }
            (Value::Number(x), Value::Number(y)) => {
                let (p, q) = (f64::from(x.as_f64().unwrap_or(f64::NAN) as f32), f64::from(y.as_f64().unwrap_or(f64::NAN) as f32));
                let scale = p.abs().max(q.abs()).max(1.0);
                if (p - q).abs() <= f64::from(f32::EPSILON) * scale {
                    None
                } else {
                    Some(format!("{path}: {p} against {q}"))
                }
            }
            _ if a == b => None,
            _ => Some(format!("{path}: {a} against {b}")),
        }
    }
    walk(a, b, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_as_swift_writes_them() {
        let cases: [(f32, &str); 9] = [
            (0.0, "0"),
            (1.0, "1"),
            (2.092, "2.092"),
            (0.10432158, "0.10432158"),
            (-1.1826692, "-1.1826692"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (1000.0, "1000"),
        ];
        for (value, text) in cases {
            assert_eq!(format_f32(value), text, "{value}");
        }
        assert_eq!(format_f64(0.1), "0.1");
        assert_eq!(format_f64(2.0), "2");
        assert_eq!(format_f64(1e16), "10000000000000000");
        assert_eq!(format_f64(1e17), "1e+17");
        // Twin: the same f32 widened to f64 carries digits Swift never writes for a Float.
        assert_ne!(format_f64(f64::from(0.10432158f32)), "0.10432158");
    }

    #[test]
    fn empty_containers_and_slashes_are_laid_as_foundation_lays_them() {
        let node = Node::Obj(vec![
            ("b".into(), Node::Arr(vec![])),
            ("a".into(), Node::Str("x/y".into())),
        ]);
        assert_eq!(pretty_sorted(&node), "{\n  \"a\" : \"x\\/y\",\n  \"b\" : [\n\n  ]\n}");
        assert_eq!(compact(&node), "{\"b\":[],\"a\":\"x\\/y\"}");
    }

    #[test]
    fn semantic_difference_sees_a_moved_number_and_forgives_rounding() {
        let a: Value = serde_json::from_str(r#"{"x":[0.10432158,1],"y":"n"}"#).unwrap();
        let rounded: Value = serde_json::json!({"x": [f64::from(0.10432158f32), 1.0], "y": "n"});
        assert_eq!(semantic_difference(&a, &rounded), None);
        let moved: Value = serde_json::from_str(r#"{"x":[0.1043226,1],"y":"n"}"#).unwrap();
        assert!(semantic_difference(&a, &moved).is_some());
        let renamed: Value = serde_json::from_str(r#"{"x":[0.10432158,1],"z":"n"}"#).unwrap();
        assert!(semantic_difference(&a, &renamed).is_some());
    }
}
