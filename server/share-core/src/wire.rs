//! Wire types: the JSON the app's `src/model.rs` writes, plus the schema-v2 fields of
//! docs/shared-pool.md ("Import": `keyless`, `origin`, `published`).
//!
//! Serialisation is byte-for-byte the app's for schema v1 (same field order, same enum tagging,
//! `null` written for empty options, `details` and its empty strings left out). The v2 fields are
//! skipped while they hold their defaults, so a v1 question serialises exactly as the app does.
//! Fixture tests in this crate and in the app (`tests/share_core_fixtures.rs`) hold both sides to it.
//!
//! These derives are for building and writing values. **Untrusted JSON never goes through
//! `Deserialize`**: `validate` walks a strictly parsed tree instead, so that duplicate keys, depth,
//! unknown fields and every limit are handled in one place and errors never echo input text.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Lowest and highest payload schema versions this crate reads and writes.
pub const SCHEMA_MIN: u32 = 1;
pub const SCHEMA_MAX: u32 = 2;
/// The schema version a stripped payload is always written as (it carries `keyless`).
pub const SCHEMA_STRIPPED: u32 = 2;

/// Explanation text an importer stores on a keyless question so that older builds, which drop
/// the unknown `keyless` field on re-save, still print a warning on the answer key
/// (docs/shared-pool.md, "The stripped (keyless) form"). Set by the importing client, never by
/// `strip`: the served stripped form carries no explanation at all.
pub const KEYLESS_SENTINEL: &str =
    "No answer key: this question was imported without its key. Add the correct answers before printing a key.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty { Low, Medium, High }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    pub file: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub text: String,
    #[serde(default)] pub correct: bool,
    #[serde(default)] pub locked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pair {
    pub left: String,
    pub right: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub stem: String,
    pub kind: PartKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartKind {
    Mc { answers: Vec<Answer> },
    Tf { correct: bool },
    Short { sample: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Mc { answers: Vec<Answer> },
    Tf { correct: bool },
    Matching { pairs: Vec<Pair>, distractors: Vec<String> },
    Blank { accept: Vec<Vec<String>> },
    Short { sample: String },
    Multipart { parts: Vec<Part> },
}

impl Kind {
    /// The external tag the app writes (`"Mc"`, `"Tf"`, ...).
    pub fn tag(&self) -> &'static str {
        match self {
            Kind::Mc { .. } => "Mc",
            Kind::Tf { .. } => "Tf",
            Kind::Matching { .. } => "Matching",
            Kind::Blank { .. } => "Blank",
            Kind::Short { .. } => "Short",
            Kind::Multipart { .. } => "Multipart",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FigureSpec {
    pub id: String,
    #[serde(default)] pub params: BTreeMap<String, serde_json::Value>,
}

/// One earlier link of an attribution chain, oldest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainLink {
    pub author_name: String,
    pub item_id: String,
    pub qid: String,
    pub version: u32,
    pub license: String,
    pub modified: bool,
}

/// Provenance of an imported question or bank (schema v2). `qid` is empty on a bank's origin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub item_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub qid: String,
    pub publish_id: String,
    pub version: u32,
    pub author_name: String,
    pub license: String,
    /// `full` or `stripped`.
    pub form: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub served_sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub match_hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub chain: Vec<ChainLink>,
}

/// The author's own record of where a local question or bank was published (schema v2).
/// `qid` is empty on a bank.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Published {
    pub item_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub qid: String,
    pub publish_id: String,
    pub version: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub stem: String,
    pub kind: Kind,
    pub points: Option<f64>,
    pub difficulty: Option<Difficulty>,
    #[serde(default)] pub topics: Vec<String>,
    #[serde(default)] pub image: Option<ImageRef>,
    #[serde(default)] pub explanation: Option<String>,
    #[serde(default)] pub figure: Option<FigureSpec>,
    pub created: i64,
    pub modified: i64,
    /// v2: the question was imported without its answer key.
    #[serde(default, skip_serializing_if = "is_false")] pub keyless: bool,
    /// v2: the author asks that the choices are never shuffled (an order that matters, or
    /// choices that refer to each other in ways share-core cannot detect). Honoured by `strip`;
    /// not part of `match_hash` (order never is).
    #[serde(default, skip_serializing_if = "is_false")] pub keep_order: bool,
    /// v2: provenance of an imported copy.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub origin: Option<Origin>,
    /// v2: the author's publish record.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub published: Option<Published>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankDetails {
    #[serde(default, skip_serializing_if = "String::is_empty")] pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub license_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub license_url: String,
}

impl BankDetails {
    /// The app's rule (`model::BankDetails::is_empty`): blank after trimming counts as empty.
    pub fn is_empty(&self) -> bool {
        [&self.description, &self.author, &self.license_name, &self.license_url].iter().all(|s| s.trim().is_empty())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuestionBank {
    pub id: String,
    pub name: String,
    pub questions: Vec<Question>,
    pub created: i64,
    pub modified: i64,
    #[serde(default, skip_serializing_if = "BankDetails::is_empty")] pub details: BankDetails,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub origin: Option<Origin>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub published: Option<Published>,
}

/// What a payload carries: one bank (an item of kind `bank`) or one question.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Body {
    #[serde(rename = "bank")] Bank(QuestionBank),
    #[serde(rename = "question")] Question(Question),
}

/// The envelope every pool payload travels in:
/// `{"schema_version": 1, "question": {...}}` or `{"schema_version": 1, "bank": {...}}`.
/// The inner object is exactly the app's `Question` / `QuestionBank` JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct Payload {
    pub schema_version: u32,
    pub body: Body,
}

impl Payload {
    pub fn questions(&self) -> &[Question] {
        match &self.body {
            Body::Bank(b) => &b.questions,
            Body::Question(q) => std::slice::from_ref(q),
        }
    }
    pub fn questions_mut(&mut self) -> &mut [Question] {
        match &mut self.body {
            Body::Bank(b) => &mut b.questions,
            Body::Question(q) => std::slice::from_mut(q),
        }
    }
    /// The envelope as a JSON value (keys in a fixed, sorted order: see `hash::canonical_json`).
    pub fn to_value(&self) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        m.insert("schema_version".into(), self.schema_version.into());
        let (k, v) = match &self.body {
            Body::Bank(b) => ("bank", serde_json::to_value(b)),
            Body::Question(q) => ("question", serde_json::to_value(q)),
        };
        // Serialising these plain derives cannot fail (no maps with non-string keys, and every
        // float was checked finite by `validate`); a non-finite float would become null.
        m.insert(k.into(), v.unwrap_or(serde_json::Value::Null));
        serde_json::Value::Object(m)
    }
    /// Length in bytes of `to_json()`.
    pub fn json_len(&self) -> usize {
        self.to_json().len()
    }
    /// Canonical JSON (`hash::write_payload`; equal to `canonical_json(&self.to_value())`).
    pub fn to_json(&self) -> String {
        let mut s = String::new();
        crate::hash::write_payload(self, &mut s);
        s
    }
}

fn is_false(b: &bool) -> bool { !*b }
