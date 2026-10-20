//! Canonical encoding and the two question hashes.
//!
//! # `canonical_json`
//! Compact JSON with object keys sorted by code point (= UTF-8 byte order), no insignificant
//! whitespace, strings escaped exactly as `serde_json` escapes them (`"`, `\`, and U+0000-001F;
//! everything else as raw UTF-8), numbers as `serde_json` prints them (integers in decimal, floats
//! in shortest round-trip form, e.g. `1.0`). Only Rust (natively or as wasm) produces it, so no
//! cross-language float formatting is involved.
//!
//! # `match_hash` (compared across items; safe to show anyone)
//! `SHA-256("vershion/match_hash/v1\n" || canonical_json(skeleton))`, lowercase hex, where the
//! skeleton is `strip_without_shuffle` reduced to what a student can see anyway:
//!
//! | Kind | In the skeleton |
//! |---|---|
//! | every kind | `kind` tag; `stem` |
//! | `Mc` | `choices`: every answer text **as it is** (no label removal), **sorted** |
//! | `Tf`, `Short` | nothing more |
//! | `Matching` | `left`: left items, sorted; `bank`: the word bank = right items and distractors together, empties removed, de-duplicated, sorted |
//! | `Blank` | `blanks`: the number of blanks |
//! | `Multipart` | `parts`, **in order**: each part's `stem` and `kind`, plus `choices` (as written, sorted) for an Mc part |
//!
//! Every text is in `text::hash_form`: NFC; runs of spaces and tabs collapsed to one space and
//! each line trimmed; line breaks kept, so a table and a sentence differ.
//!
//! **Not in it:** any `correct` flag, which or how many answers are correct, `locked`, answer
//! order, which right item belongs to which left item, `accept` lists, `sample`, `explanation`,
//! `figure`, `image`, `topics`, `points`, `difficulty`, `id`, `created`, `modified`, `keyless`,
//! `origin`, `published`, the schema version, and anything about the bank. So it is the same for
//! a question and its stripped form, for every shuffle seed, and for every key of the same
//! question: it cannot be used to test a guessed key.
//!
//! Labels are never removed by `match_hash` itself. When the author chose `Strip` for a question
//! (`labels`), its served text differs from the full text, so the server hashes the full body with
//! [`match_hash_with`] and the author's recorded decisions: that equals `match_hash` of the
//! stripped form. Without decisions (or with `Keep`), both are the plain texts. So "A. thaliana /
//! B. subtilis" and "thaliana / subtilis" hash differently unless the author stripped the first,
//! in which case they really are served identically.
//!
//! # `full_hash` (integrity of one body; never compared across items, never sent to a non-author)
//! `SHA-256("vershion/question/v1\n" || canonical_json(question))` over the **whole** validated
//! question, every field, key included. For a small answer space it is a key oracle (a Tf has two
//! possible keys), which is why the spec keeps `full_sha256` away from everyone but the author.
//! The same function over the served form (full or stripped) is the spec's `served_sha256`
//! ([`served_hash`]). It is computed with [`write_question`], the canonical writer for the wire
//! types (the same bytes as `canonical_json(serde_json::to_value(q))`, without building the tree).
use crate::strip::{QuestionLabels, strip_without_shuffle};
use crate::text::hash_form;
use crate::wire::{Answer, Body, ChainLink, Kind, Origin, PartKind, Payload, Published, Question, QuestionBank};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// Writes `v` as canonical JSON (see the module docs).
pub fn canonical_json(v: &Value) -> String {
    let mut out = String::new();
    write(v, &mut out);
    out
}

fn write(v: &Value, out: &mut String) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String((*k).clone()).to_string());
                out.push(':');
                write(&m[k.as_str()], out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    let d = Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in d {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap_or('0'));
        s.push(char::from_digit((b & 15) as u32, 16).unwrap_or('0'));
    }
    s
}

fn choices(answers: &[Answer]) -> Vec<String> {
    let mut t: Vec<String> = answers.iter().map(|a| hash_form(&a.text)).collect();
    t.sort();
    t
}

/// The key-free skeleton `match_hash` covers (see the module docs).
pub fn match_skeleton(q: &Question) -> Value {
    let mut m = Map::new();
    m.insert("kind".into(), q.kind.tag().into());
    m.insert("stem".into(), hash_form(&q.stem).into());
    match &q.kind {
        Kind::Mc { answers } => {
            m.insert("choices".into(), json!(choices(answers)));
        }
        Kind::Tf { .. } | Kind::Short { .. } => {}
        Kind::Matching { pairs, distractors } => {
            let mut left: Vec<String> = pairs.iter().map(|p| hash_form(&p.left)).collect();
            left.sort();
            let mut bank: Vec<String> = pairs
                .iter()
                .map(|p| hash_form(&p.right))
                .chain(distractors.iter().map(|d| hash_form(d)))
                .filter(|s| !s.is_empty())
                .collect();
            bank.sort();
            bank.dedup();
            m.insert("left".into(), json!(left));
            m.insert("bank".into(), json!(bank));
        }
        Kind::Blank { accept } => {
            m.insert("blanks".into(), json!(accept.len()));
        }
        Kind::Multipart { parts } => {
            let ps: Vec<Value> = parts
                .iter()
                .map(|p| {
                    let mut pm = Map::new();
                    pm.insert("stem".into(), hash_form(&p.stem).into());
                    match &p.kind {
                        PartKind::Mc { answers } => {
                            pm.insert("kind".into(), "Mc".into());
                            pm.insert("choices".into(), json!(choices(answers)));
                        }
                        PartKind::Tf { .. } => {
                            pm.insert("kind".into(), "Tf".into());
                        }
                        PartKind::Short { .. } => {
                            pm.insert("kind".into(), "Short".into());
                        }
                    }
                    Value::Object(pm)
                })
                .collect();
            m.insert("parts".into(), Value::Array(ps));
        }
    }
    Value::Object(m)
}

/// The cross-item match hash of one question (see the module docs).
pub fn match_hash(q: &Question) -> String {
    let mut data = b"vershion/match_hash/v1\n".to_vec();
    data.extend_from_slice(canonical_json(&match_skeleton(q)).as_bytes());
    sha256_hex(&data)
}

/// `match_hash` of the form `strip` serves for the author's label decisions `labels` (see the
/// module docs).
pub fn match_hash_with(q: &Question, labels: &QuestionLabels) -> String {
    match_hash(&strip_without_shuffle(q, labels))
}

/// The integrity hash of one whole question (see the module docs).
pub fn full_hash(q: &Question) -> String {
    let mut out = String::from("vershion/question/v1\n");
    write_question(q, &mut out);
    sha256_hex(out.as_bytes())
}

/// The spec's `served_sha256`: `full_hash` of whichever form was served.
pub fn served_hash(q: &Question) -> String {
    full_hash(q)
}

// ---- the canonical writer for the wire types ---------------------------------------------------
//
// Keys in code point order, the same skipping rules as the serde derives in `wire`, numbers and
// strings exactly as serde_json writes them. `tests/golden.rs` checks it against
// `canonical_json(serde_json::to_value(..))` on every fixture and the property tests on every case.

fn s_(v: &str, out: &mut String) {
    out.push_str(&Value::String(v.to_string()).to_string());
}

fn k_(k: &str, first: &mut bool, out: &mut String) {
    if !*first {
        out.push(',');
    }
    *first = false;
    s_(k, out);
    out.push(':');
}

fn strs(v: &[String], out: &mut String) {
    out.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        s_(x, out);
    }
    out.push(']');
}

fn answers_(a: &[Answer], out: &mut String) {
    out.push_str("{\"answers\":[");
    for (i, x) in a.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(if x.correct { "{\"correct\":true,\"locked\":" } else { "{\"correct\":false,\"locked\":" });
        out.push_str(if x.locked { "true" } else { "false" });
        out.push_str(",\"text\":");
        s_(&x.text, out);
        out.push('}');
    }
    out.push_str("]}");
}

fn kind_(k: &Kind, out: &mut String) {
    out.push('{');
    s_(k.tag(), out);
    out.push(':');
    match k {
        Kind::Mc { answers } => answers_(answers, out),
        Kind::Tf { correct } => out.push_str(if *correct { "{\"correct\":true}" } else { "{\"correct\":false}" }),
        Kind::Matching { pairs, distractors } => {
            out.push_str("{\"distractors\":");
            strs(distractors, out);
            out.push_str(",\"pairs\":[");
            for (i, p) in pairs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str("{\"left\":");
                s_(&p.left, out);
                out.push_str(",\"right\":");
                s_(&p.right, out);
                out.push('}');
            }
            out.push_str("]}");
        }
        Kind::Blank { accept } => {
            out.push_str("{\"accept\":[");
            for (i, a) in accept.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                strs(a, out);
            }
            out.push_str("]}");
        }
        Kind::Short { sample } => {
            out.push_str("{\"sample\":");
            s_(sample, out);
            out.push('}');
        }
        Kind::Multipart { parts } => {
            out.push_str("{\"parts\":[");
            for (i, p) in parts.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str("{\"kind\":{");
                match &p.kind {
                    PartKind::Mc { answers } => {
                        out.push_str("\"Mc\":");
                        answers_(answers, out);
                    }
                    PartKind::Tf { correct } => out.push_str(if *correct { "\"Tf\":{\"correct\":true}" } else { "\"Tf\":{\"correct\":false}" }),
                    PartKind::Short { sample } => {
                        out.push_str("\"Short\":{\"sample\":");
                        s_(sample, out);
                        out.push('}');
                    }
                }
                out.push_str("},\"stem\":");
                s_(&p.stem, out);
                out.push('}');
            }
            out.push_str("]}");
        }
    }
    out.push('}');
}

fn link_(l: &ChainLink, out: &mut String) {
    out.push_str("{\"author_name\":");
    s_(&l.author_name, out);
    out.push_str(",\"item_id\":");
    s_(&l.item_id, out);
    out.push_str(",\"license\":");
    s_(&l.license, out);
    out.push_str(if l.modified { ",\"modified\":true" } else { ",\"modified\":false" });
    out.push_str(",\"qid\":");
    s_(&l.qid, out);
    out.push_str(&format!(",\"version\":{}}}", l.version));
}

fn origin_(o: &Origin, out: &mut String) {
    let mut f = true;
    out.push('{');
    k_("author_name", &mut f, out);
    s_(&o.author_name, out);
    if !o.chain.is_empty() {
        k_("chain", &mut f, out);
        out.push('[');
        for (i, l) in o.chain.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            link_(l, out);
        }
        out.push(']');
    }
    k_("form", &mut f, out);
    s_(&o.form, out);
    k_("item_id", &mut f, out);
    s_(&o.item_id, out);
    k_("license", &mut f, out);
    s_(&o.license, out);
    if !o.match_hash.is_empty() {
        k_("match_hash", &mut f, out);
        s_(&o.match_hash, out);
    }
    k_("publish_id", &mut f, out);
    s_(&o.publish_id, out);
    if !o.qid.is_empty() {
        k_("qid", &mut f, out);
        s_(&o.qid, out);
    }
    if !o.served_sha256.is_empty() {
        k_("served_sha256", &mut f, out);
        s_(&o.served_sha256, out);
    }
    k_("version", &mut f, out);
    out.push_str(&o.version.to_string());
    out.push('}');
}

fn published_(p: &Published, out: &mut String) {
    out.push_str("{\"item_id\":");
    s_(&p.item_id, out);
    out.push_str(",\"publish_id\":");
    s_(&p.publish_id, out);
    if !p.qid.is_empty() {
        out.push_str(",\"qid\":");
        s_(&p.qid, out);
    }
    out.push_str(&format!(",\"version\":{}}}", p.version));
}

/// Canonical JSON of one question.
pub fn write_question(q: &Question, out: &mut String) {
    let mut f = true;
    out.push('{');
    k_("created", &mut f, out);
    out.push_str(&q.created.to_string());
    k_("difficulty", &mut f, out);
    out.push_str(&serde_json::to_string(&q.difficulty).unwrap_or_else(|_| "null".into()));
    k_("explanation", &mut f, out);
    match &q.explanation {
        Some(e) => s_(e, out),
        None => out.push_str("null"),
    }
    k_("figure", &mut f, out);
    match &q.figure {
        Some(fig) => {
            out.push_str("{\"id\":");
            s_(&fig.id, out);
            out.push_str(",\"params\":");
            let m: Map<String, Value> = fig.params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            write(&Value::Object(m), out);
            out.push('}');
        }
        None => out.push_str("null"),
    }
    k_("id", &mut f, out);
    s_(&q.id, out);
    k_("image", &mut f, out);
    match &q.image {
        Some(i) => {
            out.push_str("{\"file\":");
            s_(&i.file, out);
            out.push_str(&format!(",\"height\":{},\"width\":{}}}", i.height, i.width));
        }
        None => out.push_str("null"),
    }
    if q.keep_order {
        k_("keep_order", &mut f, out);
        out.push_str("true");
    }
    if q.keyless {
        k_("keyless", &mut f, out);
        out.push_str("true");
    }
    k_("kind", &mut f, out);
    kind_(&q.kind, out);
    k_("modified", &mut f, out);
    out.push_str(&q.modified.to_string());
    if let Some(o) = &q.origin {
        k_("origin", &mut f, out);
        origin_(o, out);
    }
    k_("points", &mut f, out);
    match q.points {
        Some(p) => out.push_str(&serde_json::to_string(&p).unwrap_or_else(|_| "null".into())),
        None => out.push_str("null"),
    }
    if let Some(p) = &q.published {
        k_("published", &mut f, out);
        published_(p, out);
    }
    k_("stem", &mut f, out);
    s_(&q.stem, out);
    k_("topics", &mut f, out);
    strs(&q.topics, out);
    out.push('}');
}

fn bank_(b: &QuestionBank, out: &mut String) {
    let mut f = true;
    out.push('{');
    k_("created", &mut f, out);
    out.push_str(&b.created.to_string());
    if !b.details.is_empty() {
        k_("details", &mut f, out);
        let mut g = true;
        out.push('{');
        for (k, v) in [("author", &b.details.author), ("description", &b.details.description), ("license_name", &b.details.license_name), ("license_url", &b.details.license_url)] {
            if !v.is_empty() {
                k_(k, &mut g, out);
                s_(v, out);
            }
        }
        out.push('}');
    }
    k_("id", &mut f, out);
    s_(&b.id, out);
    k_("modified", &mut f, out);
    out.push_str(&b.modified.to_string());
    k_("name", &mut f, out);
    s_(&b.name, out);
    if let Some(o) = &b.origin {
        k_("origin", &mut f, out);
        origin_(o, out);
    }
    if let Some(p) = &b.published {
        k_("published", &mut f, out);
        published_(p, out);
    }
    k_("questions", &mut f, out);
    out.push('[');
    for (i, q) in b.questions.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_question(q, out);
    }
    out.push_str("]}");
}

/// Canonical JSON of a payload envelope.
pub fn write_payload(p: &Payload, out: &mut String) {
    match &p.body {
        Body::Bank(b) => {
            out.push_str("{\"bank\":");
            bank_(b, out);
        }
        Body::Question(q) => {
            out.push_str("{\"question\":");
            write_question(q, out);
        }
    }
    out.push_str(&format!(",\"schema_version\":{}}}", p.schema_version));
}
