#![allow(dead_code)]
use share_core::*;
use std::path::PathBuf;

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// The model fixtures written by the app's `tests/share_core_fixtures.rs`.
pub const MODEL: [&str; 11] = [
    "question_mc",
    "question_mc_multi",
    "question_tf",
    "question_matching",
    "question_blank",
    "question_short",
    "question_multipart",
    "question_figure_image",
    "question_matching_shared",
    "bank_all_kinds",
    "bank_no_details",
];

pub fn model_json(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join("model").join(format!("{name}.json"))).expect("fixture (regenerate from the app)")
}

pub fn envelope_of(name: &str, inner: &str, schema: u32) -> String {
    let key = if name.starts_with("bank") { "bank" } else { "question" };
    format!("{{\"schema_version\":{schema},\"{key}\":{inner}}}")
}

pub fn fixture_payload(name: &str) -> String {
    envelope_of(name, &model_json(name), 1)
}

pub fn ok(json: &str) -> Validated {
    validate(json, &Limits::default(), &Structural).unwrap_or_else(|e| panic!("expected valid: {e:?}"))
}

pub fn errs(json: &str) -> Vec<ValidationError> {
    match validate(json, &Limits::default(), &Structural) {
        Ok(_) => panic!("expected invalid"),
        Err(e) => e,
    }
}

pub fn codes(json: &str) -> Vec<(&'static str, String)> {
    errs(json).into_iter().map(|e| (e.code, e.path)).collect()
}

pub fn question(stem: &str, kind: Kind) -> Question {
    Question {
        id: "q1".into(),
        stem: stem.into(),
        kind,
        points: Some(1.0),
        difficulty: None,
        topics: vec![],
        image: None,
        explanation: None,
        figure: None,
        created: 1,
        modified: 2,
        keyless: false,
        keep_order: false,
        origin: None,
        published: None,
    }
}

pub fn q_payload(q: &Question) -> String {
    Payload { schema_version: 2, body: Body::Question(q.clone()) }.to_json()
}

pub fn q_payload_v1(q: &Question) -> String {
    Payload { schema_version: 1, body: Body::Question(q.clone()) }.to_json()
}

pub fn answer(text: &str, correct: bool) -> Answer {
    Answer { text: text.into(), correct, locked: false }
}

pub fn seed(n: u8) -> [u8; 32] {
    let mut s = [0u8; 32];
    s[0] = n;
    s[31] = n.wrapping_mul(37);
    s
}

pub fn only_question(p: &Payload) -> &Question {
    match &p.body {
        Body::Question(q) => q,
        Body::Bank(b) => &b.questions[0],
    }
}

/// A decision for every label finding: `prefer` where allowed (Strip may be refused), else Keep.
pub fn decide_all(p: &Payload, prefer: LabelChoice) -> Vec<LabelDecision> {
    label_findings(p)
        .into_iter()
        .map(|f| LabelDecision {
            question: f.question,
            part: f.part,
            choice: if prefer == LabelChoice::Strip && !f.strip_allowed { LabelChoice::Keep } else { prefer },
        })
        .collect()
}

/// Options with every finding decided (Strip where allowed).
pub fn opts_for(p: &Payload, image_ok: Vec<usize>) -> StripOptions {
    StripOptions { image_ok, labels: decide_all(p, LabelChoice::Strip) }
}

/// `{"labels":[...]}` for the wasm options.
pub fn labels_json(p: &Payload, prefer: LabelChoice) -> String {
    serde_json::json!({ "labels": decide_all(p, prefer) }).to_string()
}

/// `wasm::strip_json` / `strip_with_seed_json` as an author who acknowledges every preserved
/// order: on `order_ack_required` the listed positions are added to `order_acks` and the call is
/// repeated. (Tests of the requirement itself call the wasm functions directly.)
pub fn strip_ok(p: &[u8], opts: &str) -> String {
    acked(opts, |o| share_core::wasm::strip_json(p, o))
}

pub fn strip_seed_ok(p: &[u8], seed: &[u8], opts: &str) -> String {
    acked(opts, |o| share_core::wasm::strip_with_seed_json(p, seed, o))
}

fn acked(opts: &str, call: impl Fn(&str) -> String) -> String {
    let out = call(opts);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    if v["errors"][0]["code"] != "order_ack_required" {
        return out;
    }
    let mut o: serde_json::Value = if opts.trim().is_empty() { serde_json::json!({}) } else { serde_json::from_str(opts).unwrap() };
    o["order_acks"] = v["needs_order_ack"].clone();
    call(&o.to_string())
}
