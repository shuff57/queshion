//! Strict, schema-directed JSON reading for untrusted payloads (ASVS 5.0 1.5.2, 1.5.3, 2.4.1).
//!
//! * **Duplicate keys are refused.** `serde_json::Value` and JavaScript's `JSON.parse` keep the
//!   last of two equal keys, other parsers the first; refusing them leaves nothing to disagree on.
//! * **Unknown keys are never stored.** The reader follows a [`Shape`] (the payload schema): a key
//!   the shape does not name is skipped without allocating, and its object is marked `dropped` so
//!   the validator can report it (by path, never by name).
//! * **Arrays are capped while reading.** At most `cap + 1` elements are kept (enough for the
//!   validator to report `too_many`); the rest are skipped.
//! * **Memory is budgeted while reading**, before any of it is walked: a value count, an object
//!   count, and an allocation estimate in bytes ([`Budget`]). The byte limit alone does not bound
//!   memory (`[{"":0},...]` is 7 bytes per object).
//! * **Depth is limited everywhere**, skipped content included.
//!
//! Otherwise it accepts exactly RFC 8259 JSON as `serde_json` reads it (no comments, no trailing
//! commas, no BOM, no lone surrogates, numbers that fit an f64).
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::cell::Cell;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Not well-formed JSON (or a number outside f64, a lone surrogate, trailing data...).
    Syntax,
    /// Not UTF-8 (byte input only).
    Utf8,
    /// Nested deeper than the limit.
    Depth,
    /// An object repeats a key.
    DuplicateKey,
    /// Larger than the byte limit (checked before parsing).
    TooLarge,
    /// More values than `max_values`.
    TooManyValues,
    /// More objects than `max_objects`.
    TooManyObjects,
    /// The allocation estimate went over `parse_budget_bytes`.
    OverBudget,
}

impl ParseError {
    pub fn code(self) -> &'static str {
        match self {
            ParseError::Syntax => "invalid_json",
            ParseError::Utf8 => "invalid_utf8",
            ParseError::Depth => "too_deep",
            ParseError::DuplicateKey => "duplicate_key",
            ParseError::TooLarge => "body_too_large",
            ParseError::TooManyValues => "too_many_values",
            ParseError::TooManyObjects => "too_many_objects",
            ParseError::OverBudget => "too_much_memory",
        }
    }
}

/// An array cap, resolved against the caller's limits.
#[derive(Clone, Copy, Debug)]
pub enum Cap {
    Questions,
    McAnswers,
    Pairs,
    /// Distractors: pairs + distractors (the validator checks the exact sum).
    Distractors,
    Blanks,
    Variants,
    Parts,
    Topics,
    Chain,
    Tags,
    Standards,
}

/// What the reader expects at a position.
#[derive(Debug)]
pub enum Shape {
    /// A scalar (null, bool, number, string). A container here is skipped and read as `Skipped`.
    Scalar,
    /// An object with these keys; others are skipped.
    Obj(&'static [(&'static str, &'static Shape)]),
    /// An externally tagged enum: like `Obj`, but an unknown key is kept as `Key::Unknown` with
    /// a `Skipped` value, so the validator can say "unknown kind" instead of "missing".
    Tagged(&'static [(&'static str, &'static Shape)]),
    /// An array of these, at most `cap + 1` kept.
    Arr(Cap, &'static Shape),
    /// An object with any keys (figure params): scalar values, at most `figure_params + 1` kept.
    Params,
}

#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub max_bytes: usize,
    pub max_depth: usize,
    pub max_values: usize,
    pub max_objects: usize,
    /// Estimated bytes the tree may allocate.
    pub max_alloc: usize,
}

/// Per-node allocation estimate: a `Node` is 24 bytes natively (smaller on wasm32) plus allocator
/// rounding; `tests/memory.rs` checks the estimate stays above the real allocation.
const NODE_COST: usize = 40;
/// Extra per object (the entry vector's header and growth slack) and per key.
const OBJ_COST: usize = 48;
const ENTRY_COST: usize = 16;

pub trait Caps {
    fn cap(&self, c: Cap) -> usize;
    fn params_cap(&self) -> usize;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Known(&'static str),
    /// A figure param key.
    Owned(Box<str>),
    /// An unknown variant of a tagged enum (its name is not kept).
    Unknown,
}

impl Key {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Key::Known(k) => Some(k),
            Key::Owned(k) => Some(k),
            Key::Unknown => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Obj {
    pub entries: Vec<(Key, Node)>,
    /// Unknown keys were skipped.
    pub dropped: bool,
}

impl Obj {
    pub fn get(&self, k: &str) -> Option<&Node> {
        self.entries.iter().find(|(key, _)| key.as_str() == Some(k)).map(|(_, v)| v)
    }
    pub fn contains_key(&self, k: &str) -> bool {
        self.get(k).is_some()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Node {
    #[default]
    Null,
    Bool(bool),
    Num(serde_json::Number),
    Str(Box<str>),
    Arr(Vec<Node>),
    /// Boxed, so every `Node` stays 24 bytes (the allocation estimate below relies on it).
    Obj(Box<Obj>),
    /// A container where the shape expected a scalar (or an ignored subtree).
    Skipped,
}

impl Node {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Node::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Node::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Node::Num(n) => n.as_i64(),
            _ => None,
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Node::Num(n) => n.as_u64(),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Node::Num(n) => n.as_f64(),
            _ => None,
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Node::Null)
    }
    pub fn as_array(&self) -> Option<&[Node]> {
        match self {
            Node::Arr(a) => Some(a),
            _ => None,
        }
    }
    /// Takes the value of key `k` out, leaving `Null` (so a large subtree can be freed as soon as
    /// it has been read).
    pub fn take(&mut self, k: &str) -> Option<Node> {
        match self {
            Node::Obj(o) => o.entries.iter_mut().find(|(key, _)| key.as_str() == Some(k)).map(|(_, v)| std::mem::take(v)),
            _ => None,
        }
    }
    pub fn as_object(&self) -> Option<&Obj> {
        match self {
            Node::Obj(o) => Some(o.as_ref()),
            _ => None,
        }
    }
}

struct Cx<'a> {
    b: Budget,
    caps: &'a dyn Caps,
    fail: Cell<Option<ParseError>>,
    values: Cell<usize>,
    objects: Cell<usize>,
    alloc: Cell<usize>,
}

impl Cx<'_> {
    fn stop<E: de::Error>(&self, e: ParseError) -> E {
        if self.fail.get().is_none() {
            self.fail.set(Some(e));
        }
        E::custom("stop")
    }
    fn value<E: de::Error>(&self) -> Result<(), E> {
        let v = self.values.get() + 1;
        self.values.set(v);
        if v > self.b.max_values {
            return Err(self.stop(ParseError::TooManyValues));
        }
        Ok(())
    }
    fn charge<E: de::Error>(&self, bytes: usize) -> Result<(), E> {
        let a = self.alloc.get().saturating_add(bytes);
        self.alloc.set(a);
        if a > self.b.max_alloc {
            return Err(self.stop(ParseError::OverBudget));
        }
        Ok(())
    }
    fn object<E: de::Error>(&self) -> Result<(), E> {
        let o = self.objects.get() + 1;
        self.objects.set(o);
        if o > self.b.max_objects {
            return Err(self.stop(ParseError::TooManyObjects));
        }
        self.charge(OBJ_COST)
    }
    fn deeper<E: de::Error>(&self, depth: usize) -> Result<usize, E> {
        if depth + 1 > self.b.max_depth {
            return Err(self.stop(ParseError::Depth));
        }
        Ok(depth + 1)
    }
}

/// Parses `s` along `shape`.
pub fn parse(s: &str, shape: &'static Shape, budget: Budget, caps: &dyn Caps) -> Result<Node, ParseError> {
    parse_stats(s, shape, budget, caps).map(|(n, _)| n)
}

/// `parse`, also returning the allocation estimate the tree was charged (tests compare it with
/// the real allocation: the estimate must be an upper bound).
pub fn parse_stats(s: &str, shape: &'static Shape, budget: Budget, caps: &dyn Caps) -> Result<(Node, usize), ParseError> {
    if s.len() > budget.max_bytes {
        return Err(ParseError::TooLarge);
    }
    let cx = Cx { b: budget, caps, fail: Cell::new(None), values: Cell::new(0), objects: Cell::new(0), alloc: Cell::new(0) };
    let mut de = serde_json::Deserializer::from_str(s);
    let v = Read { cx: &cx, depth: 0, shape }.deserialize(&mut de);
    match (v, cx.fail.get()) {
        (_, Some(e)) => Err(e),
        (Err(_), None) => Err(ParseError::Syntax),
        (Ok(v), None) => match de.end() {
            Ok(()) => Ok((v, cx.alloc.get())),
            Err(_) => Err(ParseError::Syntax),
        },
    }
}

/// `parse` for UTF-8 bytes (refused if not valid UTF-8).
pub fn parse_bytes(b: &[u8], shape: &'static Shape, budget: Budget, caps: &dyn Caps) -> Result<Node, ParseError> {
    if b.len() > budget.max_bytes {
        return Err(ParseError::TooLarge);
    }
    let s = std::str::from_utf8(b).map_err(|_| ParseError::Utf8)?;
    parse(s, shape, budget, caps)
}

struct Read<'c, 'a> {
    cx: &'c Cx<'a>,
    depth: usize,
    shape: &'static Shape,
}

impl<'de> DeserializeSeed<'de> for Read<'_, '_> {
    type Value = Node;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Node, D::Error> {
        d.deserialize_any(self)
    }
}

/// Reads and discards a subtree, still enforcing depth and the value count.
struct Skip<'c, 'a> {
    cx: &'c Cx<'a>,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for Skip<'_, '_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Skip<'_, '_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("JSON")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> { self.cx.value() }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> { self.cx.value() }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> { self.cx.value() }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> { self.cx.value() }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> { self.cx.value() }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> { self.cx.value() }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        self.cx.value()?;
        let d = self.cx.deeper(self.depth)?;
        while a.next_element_seed(Skip { cx: self.cx, depth: d })?.is_some() {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        self.cx.value()?;
        let d = self.cx.deeper(self.depth)?;
        while a.next_key::<de::IgnoredAny>()?.is_some() {
            a.next_value_seed(Skip { cx: self.cx, depth: d })?;
        }
        Ok(())
    }
}

/// Matches an object key against the shape's names without allocating.
struct KeySeed {
    names: &'static [(&'static str, &'static Shape)],
}

enum KeyHit {
    Known(usize),
    Unknown,
}

impl<'de> DeserializeSeed<'de> for KeySeed {
    type Value = KeyHit;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<KeyHit, D::Error> {
        d.deserialize_str(self)
    }
}

impl<'de> Visitor<'de> for KeySeed {
    type Value = KeyHit;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a key")
    }
    fn visit_str<E>(self, v: &str) -> Result<KeyHit, E> {
        Ok(match self.names.iter().position(|(n, _)| *n == v) {
            Some(i) => KeyHit::Known(i),
            None => KeyHit::Unknown,
        })
    }
}

impl<'de> Visitor<'de> for Read<'_, '_> {
    type Value = Node;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("JSON")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Node, E> {
        self.cx.value()?;
        self.cx.charge(NODE_COST)?;
        Ok(Node::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Node, E> {
        self.cx.value()?;
        self.cx.charge(NODE_COST)?;
        Ok(Node::Num(v.into()))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Node, E> {
        self.cx.value()?;
        self.cx.charge(NODE_COST)?;
        Ok(Node::Num(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Node, E> {
        self.cx.value()?;
        self.cx.charge(NODE_COST)?;
        serde_json::Number::from_f64(v).map(Node::Num).ok_or_else(|| self.cx.stop(ParseError::Syntax))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Node, E> {
        self.cx.value()?;
        self.cx.charge(NODE_COST + v.len())?;
        Ok(Node::Str(v.into()))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Node, E> {
        self.cx.value()?;
        self.cx.charge(NODE_COST)?;
        Ok(Node::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Node, A::Error> {
        self.cx.value()?;
        self.cx.charge(NODE_COST)?;
        let d = self.cx.deeper(self.depth)?;
        let Shape::Arr(cap, elem) = self.shape else {
            while a.next_element_seed(Skip { cx: self.cx, depth: d })?.is_some() {}
            return Ok(Node::Skipped);
        };
        let keep = self.cx.caps.cap(*cap).saturating_add(1);
        let mut v = Vec::new();
        loop {
            if v.len() < keep {
                match a.next_element_seed(Read { cx: self.cx, depth: d, shape: elem })? {
                    Some(x) => v.push(x),
                    None => break,
                }
            } else if a.next_element_seed(Skip { cx: self.cx, depth: d })?.is_none() {
                break;
            }
        }
        v.shrink_to_fit();
        Ok(Node::Arr(v))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Node, A::Error> {
        self.cx.value()?;
        self.cx.charge(NODE_COST)?;
        let d = self.cx.deeper(self.depth)?;
        let (names, tagged) = match self.shape {
            Shape::Obj(n) => (*n, false),
            Shape::Tagged(n) => (*n, true),
            Shape::Params => return self.params(a, d),
            _ => {
                while a.next_key::<de::IgnoredAny>()?.is_some() {
                    a.next_value_seed(Skip { cx: self.cx, depth: d })?;
                }
                return Ok(Node::Skipped);
            }
        };
        self.cx.object()?;
        let mut o = Obj::default();
        while let Some(hit) = a.next_key_seed(KeySeed { names })? {
            match hit {
                KeyHit::Known(i) => {
                    let (name, shape) = names[i];
                    if o.contains_key(name) {
                        return Err(self.cx.stop(ParseError::DuplicateKey));
                    }
                    self.cx.charge(ENTRY_COST)?;
                    let v = a.next_value_seed(Read { cx: self.cx, depth: d, shape })?;
                    o.entries.push((Key::Known(name), v));
                }
                KeyHit::Unknown if tagged => {
                    a.next_value_seed(Skip { cx: self.cx, depth: d })?;
                    if !o.entries.iter().any(|(k, _)| *k == Key::Unknown) {
                        self.cx.charge(ENTRY_COST)?;
                        o.entries.push((Key::Unknown, Node::Skipped));
                    } else {
                        // Two unknown variants: still more than one key; keep one marker per object.
                        o.dropped = true;
                    }
                }
                KeyHit::Unknown => {
                    a.next_value_seed(Skip { cx: self.cx, depth: d })?;
                    o.dropped = true;
                }
            }
        }
        o.entries.shrink_to_fit();
        Ok(Node::Obj(Box::new(o)))
    }
}

impl Read<'_, '_> {
    fn params<'de, A: MapAccess<'de>>(self, mut a: A, d: usize) -> Result<Node, A::Error> {
        self.cx.object()?;
        let keep = self.cx.caps.params_cap().saturating_add(1);
        let mut o = Obj::default();
        while let Some(k) = a.next_key::<String>()? {
            if o.len() >= keep {
                a.next_value_seed(Skip { cx: self.cx, depth: d })?;
                continue;
            }
            if o.entries.iter().any(|(e, _)| e.as_str() == Some(&k)) {
                return Err(self.cx.stop(ParseError::DuplicateKey));
            }
            self.cx.charge(ENTRY_COST + k.len())?;
            let v = a.next_value_seed(Read { cx: self.cx, depth: d, shape: &Shape::Scalar })?;
            o.entries.push((Key::Owned(k.into_boxed_str()), v));
        }
        o.entries.shrink_to_fit();
        Ok(Node::Obj(Box::new(o)))
    }
}

// ---- the payload schema ---------------------------------------------------------------------

macro_rules! obj {
    ($($k:literal => $s:expr),* $(,)?) => { Shape::Obj(&[$(($k, $s)),*]) };
}

pub static ANSWER: Shape = obj! { "text" => &Shape::Scalar, "correct" => &Shape::Scalar, "locked" => &Shape::Scalar };
static ANSWERS: Shape = Shape::Arr(Cap::McAnswers, &ANSWER);
static MC: Shape = obj! { "answers" => &ANSWERS };
static TF: Shape = obj! { "correct" => &Shape::Scalar };
static SHORT: Shape = obj! { "sample" => &Shape::Scalar };
static PAIR: Shape = obj! { "left" => &Shape::Scalar, "right" => &Shape::Scalar };
static MATCHING: Shape = obj! {
    "pairs" => &Shape::Arr(Cap::Pairs, &PAIR),
    "distractors" => &Shape::Arr(Cap::Distractors, &Shape::Scalar),
};
static VARIANTS: Shape = Shape::Arr(Cap::Variants, &Shape::Scalar);
static BLANK: Shape = obj! { "accept" => &Shape::Arr(Cap::Blanks, &VARIANTS) };
static PART_KIND: Shape = Shape::Tagged(&[("Mc", &MC), ("Tf", &TF), ("Short", &SHORT)]);
static PART: Shape = obj! { "stem" => &Shape::Scalar, "kind" => &PART_KIND };
static MULTIPART: Shape = obj! { "parts" => &Shape::Arr(Cap::Parts, &PART) };
static KIND: Shape = Shape::Tagged(&[("Mc", &MC), ("Tf", &TF), ("Matching", &MATCHING), ("Blank", &BLANK), ("Short", &SHORT), ("Multipart", &MULTIPART)]);
static IMAGE: Shape = obj! { "file" => &Shape::Scalar, "width" => &Shape::Scalar, "height" => &Shape::Scalar };
static FIGURE: Shape = obj! { "id" => &Shape::Scalar, "params" => &Shape::Params };
static LINK: Shape = obj! {
    "author_name" => &Shape::Scalar, "item_id" => &Shape::Scalar, "qid" => &Shape::Scalar,
    "version" => &Shape::Scalar, "license" => &Shape::Scalar, "modified" => &Shape::Scalar,
};
static ORIGIN: Shape = obj! {
    "item_id" => &Shape::Scalar, "qid" => &Shape::Scalar, "publish_id" => &Shape::Scalar,
    "version" => &Shape::Scalar, "author_name" => &Shape::Scalar, "license" => &Shape::Scalar,
    "form" => &Shape::Scalar, "served_sha256" => &Shape::Scalar, "match_hash" => &Shape::Scalar,
    "chain" => &Shape::Arr(Cap::Chain, &LINK),
};
static PUBLISHED: Shape = obj! { "item_id" => &Shape::Scalar, "qid" => &Shape::Scalar, "publish_id" => &Shape::Scalar, "version" => &Shape::Scalar };
pub static QUESTION: Shape = obj! {
    "id" => &Shape::Scalar, "stem" => &Shape::Scalar, "kind" => &KIND, "points" => &Shape::Scalar,
    "difficulty" => &Shape::Scalar, "topics" => &Shape::Arr(Cap::Topics, &Shape::Scalar),
    "image" => &IMAGE, "explanation" => &Shape::Scalar, "figure" => &FIGURE,
    "created" => &Shape::Scalar, "modified" => &Shape::Scalar,
    "keyless" => &Shape::Scalar, "keep_order" => &Shape::Scalar, "origin" => &ORIGIN, "published" => &PUBLISHED,
};
static DETAILS: Shape = obj! { "description" => &Shape::Scalar, "author" => &Shape::Scalar, "license_name" => &Shape::Scalar, "license_url" => &Shape::Scalar };
static BANK: Shape = obj! {
    "id" => &Shape::Scalar, "name" => &Shape::Scalar, "questions" => &Shape::Arr(Cap::Questions, &QUESTION),
    "created" => &Shape::Scalar, "modified" => &Shape::Scalar, "details" => &DETAILS,
    "origin" => &ORIGIN, "published" => &PUBLISHED,
};
/// The payload envelope.
pub static ENVELOPE: Shape = obj! { "schema_version" => &Shape::Scalar, "question" => &QUESTION, "bank" => &BANK };
/// Publish metadata.
pub static META: Shape = obj! {
    "title" => &Shape::Scalar, "subject" => &Shape::Scalar, "course" => &Shape::Scalar, "grade" => &Shape::Scalar,
    "tags" => &Shape::Arr(Cap::Tags, &Shape::Scalar), "standards" => &Shape::Arr(Cap::Standards, &Shape::Scalar),
};

#[cfg(test)]
mod tests {
    use super::*;

    struct C;
    impl Caps for C {
        fn cap(&self, _: Cap) -> usize {
            3
        }
        fn params_cap(&self) -> usize {
            3
        }
    }
    fn b() -> Budget {
        Budget { max_bytes: 1 << 20, max_depth: 8, max_values: 10_000, max_objects: 1000, max_alloc: 1 << 22 }
    }
    static ANY: Shape = obj! { "a" => &Shape::Scalar, "b" => &Shape::Arr(Cap::Tags, &Shape::Scalar), "p" => &Shape::Params, "k" => &Shape::Tagged(&[("X", &Shape::Scalar)]) };

    #[test]
    fn strict_reading() {
        let ok = parse(r#"{"a":1,"b":[true,null,"x",1.5,9,9],"zz":{"q":[1,2]}}"#, &ANY, b(), &C).unwrap();
        let o = ok.as_object().unwrap();
        assert!(o.dropped, "unknown key skipped");
        assert_eq!(o.len(), 2);
        assert_eq!(o.get("b").unwrap().as_array().unwrap().len(), 4, "cap + 1 kept");
        assert_eq!(parse(r#"{"a":1,"a":2}"#, &ANY, b(), &C), Err(ParseError::DuplicateKey));
        assert_eq!(parse(r#"{"p":{"x":1,"x":2}}"#, &ANY, b(), &C), Err(ParseError::DuplicateKey));
        assert_eq!(parse(&format!("{{\"zz\":{}{}}}", "[".repeat(9), "]".repeat(9)), &ANY, b(), &C), Err(ParseError::Depth));
        assert_eq!(parse(&"[".repeat(100_000), &ANY, b(), &C), Err(ParseError::Depth));
        assert_eq!(parse("{} x", &ANY, b(), &C), Err(ParseError::Syntax));
        assert_eq!(parse("{\"a\":1,}", &ANY, b(), &C), Err(ParseError::Syntax));
        assert_eq!(parse("\u{feff}{}", &ANY, b(), &C), Err(ParseError::Syntax));
        assert_eq!(parse(r#"{"a":"\ud800"}"#, &ANY, b(), &C), Err(ParseError::Syntax));
        assert_eq!(parse("[1e400]", &ANY, b(), &C), Err(ParseError::Syntax));
        assert_eq!(parse("{}   ", &ANY, Budget { max_bytes: 4, ..b() }, &C), Err(ParseError::TooLarge));
        assert_eq!(parse_bytes(b"{\"a\":\"\xff\"}", &ANY, b(), &C), Err(ParseError::Utf8));
        let k = parse(r#"{"k":{"Y":{"deep":[1]}}}"#, &ANY, b(), &C).unwrap();
        assert_eq!(k.as_object().unwrap().get("k").unwrap().as_object().unwrap().entries[0], (Key::Unknown, Node::Skipped));
    }

    #[test]
    fn budgets() {
        let many = format!("{{\"zz\":[{}0]}}", "0,".repeat(20_000));
        assert_eq!(parse(&many, &ANY, b(), &C), Err(ParseError::TooManyValues), "skipped values count too");
        let objs = format!("{{\"p\":{{}},\"zz\":[{}]}}", ["{}"; 10].join(","));
        assert!(parse(&objs, &ANY, b(), &C).is_ok(), "skipped objects are not stored");
        let s = format!("{{\"a\":\"{}\"}}", "x".repeat(5000));
        assert_eq!(parse(&s, &ANY, Budget { max_alloc: 4000, ..b() }, &C), Err(ParseError::OverBudget));
        static DEEP: Shape = Shape::Arr(Cap::Tags, &Shape::Obj(&[]));
        assert_eq!(parse("[{},{},{}]", &DEEP, Budget { max_objects: 2, ..b() }, &C), Err(ParseError::TooManyObjects));
    }
}
