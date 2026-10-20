//! The JSON surface the Worker calls (`share_core::wasm`), exercised natively. The same functions
//! run in the wasm build; `tests/node/check.mjs` runs them there and compares the outputs.
mod common;
use common::*;
use serde_json::Value;
use share_core::wasm::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

#[test]
fn validate_ok_and_errors() {
    let out = j(&validate_json(fixture_payload("question_mc").as_bytes(), ""));
    assert_eq!(out["ok"], true);
    assert_eq!(out["payload"]["schema_version"], 1);
    let out = j(&validate_json(b"{", ""));
    assert_eq!(out["ok"], false);
    assert_eq!(out["errors"][0]["code"], "invalid_json");
    let out = j(&validate_json(fixture_payload("question_mc").as_bytes(), "{\"limits\":{\"stem_bytes\":5}}"));
    assert_eq!(out["errors"][0]["code"], "too_long");
    assert_eq!(out["errors"][0]["limit"], 5);
    let out = j(&validate_json(fixture_payload("question_mc").as_bytes(), "{\"nope\":1}"));
    assert_eq!(out["errors"][0]["code"], "bad_options");
    // Duplicate keys in the options are refused too (serde's derive refuses duplicate fields).
    let out = j(&validate_json(b"{\"schema_version\":1}", r#"{"limits":{"max_depth":1},"limits":{"max_depth":100}}"#));
    assert_eq!(out["errors"][0]["code"], "bad_options");
}

#[test]
fn bytes_must_be_utf8() {
    let mut p = fixture_payload("question_tf").into_bytes();
    let i = p.windows(5).position(|w| w == b"Water").unwrap();
    p[i] = 0xFF;
    assert_eq!(j(&validate_json(&p, ""))["errors"][0]["code"], "invalid_utf8");
    // An encoded lone surrogate (CESU-style ED A0 80) is not UTF-8 either.
    p[i] = b'W';
    let mut s = p.clone();
    s.splice(i..i, [0xED, 0xA0, 0x80]);
    assert_eq!(j(&validate_json(&s, ""))["errors"][0]["code"], "invalid_utf8");
}

#[test]
fn strip_draws_its_own_seed_and_reports() {
    let p = fixture_payload("bank_all_kinds");
    let v = ok(&p);
    let mut opts: serde_json::Value = serde_json::from_str(&labels_json(&v.payload, share_core::LabelChoice::Strip)).unwrap();
    opts["image_ok"] = serde_json::json!([7]);
    let a = j(&strip_ok(p.as_bytes(), &opts.to_string()));
    assert_eq!(a["ok"], true, "{a}");
    assert_eq!(a["payload"]["schema_version"], 2);
    assert_eq!(a["images_kept"], serde_json::json!([7]));
    assert_eq!(a["figures_removed"], serde_json::json!([7]));
    assert_eq!(a["labels_removed"], serde_json::json!([{"question": 1}]));
    // The stripped hashes are those of the full body with the author's decisions applied.
    let labels: Vec<share_core::LabelDecision> = serde_json::from_value(opts["labels"].clone()).unwrap();
    let want: Vec<String> = v.payload.questions().iter().enumerate().map(|(i, q)| share_core::match_hash_with(q, &share_core::question_labels(&labels, i))).collect();
    assert_eq!(a["match_hashes"], serde_json::json!(want));
    // Without Strip decisions the plain match_hash is the full body's.
    let full = j(&match_hash_json(p.as_bytes(), ""));
    assert_ne!(a["match_hashes"][1], full["hashes"][1], "question 1's labels were stripped");
    assert_eq!(a["match_hashes"][0], full["hashes"][0]);
    // Two production strips use two different seeds (orders differ with overwhelming probability
    // across five tries of a 26-way shuffle; the choices are the same).
    let q = question("Pick", share_core::Kind::Mc { answers: (0..26).map(|i| answer(&format!("planet {i}"), i == 0)).collect() });
    let pq = q_payload(&q);
    let orders: std::collections::BTreeSet<String> = (0..5).map(|_| j(&strip_ok(pq.as_bytes(), ""))["payload"].to_string()).collect();
    assert!(orders.len() > 1, "strip must not be deterministic");
    // The seeded variant (tests only) still matches the stored golden byte for byte.
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(fixtures_dir().join("stripped/bank_all_kinds.json")).unwrap()).unwrap();
    let lab = labels_json(&v.payload, share_core::LabelChoice::Strip);
    let out = j(&strip_seed_ok(p.as_bytes(), &[7u8; 32], &lab));
    assert_eq!(out["payload"], golden);
    // Without the decision: refused, with the preview.
    let r = j(&strip_ok(p.as_bytes(), ""));
    assert_eq!(r["errors"][0]["code"], "labels_need_decision");
    assert_eq!(r["label_findings"][0]["choices"][0]["before"], "A. Jupiter");
    assert_eq!(r["label_findings"][0]["choices"][0]["after"], "Jupiter");
    for bad in [&[0u8; 31][..], &[0u8; 33][..], &[][..]] {
        assert_eq!(j(&strip_seed_ok(p.as_bytes(), bad, &lab))["errors"][0]["code"], "bad_seed");
    }
    let st = serde_json::to_string(&out["payload"]).unwrap();
    assert_eq!(out["stripped_hashes"], j(&full_hash_json(st.as_bytes(), ""))["hashes"]);
}

#[test]
fn image_ok_out_of_range_is_an_error() {
    let out = j(&strip_ok(fixture_payload("question_tf").as_bytes(), "{\"image_ok\":[99]}"));
    assert_eq!(out["ok"], false);
    assert_eq!(out["errors"][0]["code"], "image_ok_out_of_range");
    assert_eq!(out["errors"][0]["path"], "image_ok[0]");
}

#[test]
fn publish_entry_point() {
    // A keyless question may only be published with hide_key.
    let mut q = question("Which?", share_core::Kind::Mc { answers: vec![answer("a", false), answer("b", false)] });
    q.keyless = true;
    let p = q_payload(&q);
    assert_eq!(j(&validate_for_publish_json(p.as_bytes(), ""))["errors"][0]["code"], "keyless_needs_hide_key");
    assert_eq!(j(&validate_for_publish_json(p.as_bytes(), "{\"hide_key\":true}"))["ok"], true);
    // A full question needs its key.
    q.keyless = false;
    let out = j(&validate_for_publish_json(q_payload(&q).as_bytes(), ""));
    assert_eq!(out["errors"][0]["code"], "no_correct");
}

#[test]
fn hashes_and_meta() {
    let out = j(&full_hash_json(fixture_payload("question_tf").as_bytes(), ""));
    assert_eq!(out["hashes"].as_array().unwrap().len(), 1);
    let out = j(&validate_meta_json(b"{\"title\":\"T\",\"tags\":[\"A\"]}", ""));
    assert_eq!(out["meta"]["tags"], serde_json::json!(["a"]));
    let v = j(&schema_versions());
    assert_eq!((v["min"].clone(), v["max"].clone(), v["stripped"].clone()), (1.into(), 2.into(), 2.into()));
}

#[test]
fn declared_registry_over_json() {
    let mut q = question("s", share_core::Kind::Tf { correct: true });
    q.figure = Some(share_core::FigureSpec { id: "pip".into(), params: [("v".to_string(), serde_json::json!(50.0))].into_iter().collect() });
    let opts = r#"{"figures":{"pip":[{"key":"v","type":"float","min":0.0,"max":20.0}]}}"#;
    let out = j(&validate_json(q_payload(&q).as_bytes(), opts));
    assert_eq!(out["payload"]["question"]["figure"]["params"]["v"], 20.0);
}

#[test]
fn argon2_still_exported() {
    let tag = share_core::argon2id(b"password", b"saltsaltsaltsalt", 8, 1, 1).unwrap();
    assert_eq!(tag.len(), 32);
}
