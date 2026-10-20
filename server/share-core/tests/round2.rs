//! Regression tests from the round-2 critics' probes (scratchpad critic11a/p.mjs, p2.mjs and
//! critic11b/probe): each case is the probe's input with the behaviour the fixes require.
mod common;
use common::*;
use serde_json::{Value, json};
use share_core::wasm::*;
use share_core::*;
use std::time::Instant;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

/// The probes' `Q(stem, kind, v)` helper.
fn qj(stem: &str, kind: Value, v: u32) -> String {
    json!({ "schema_version": v, "question": { "id": "q", "stem": stem, "kind": kind, "points": null, "difficulty": null, "created": 0, "modified": 0 } }).to_string()
}

fn tf() -> Value {
    json!({ "Tf": { "correct": true } })
}

fn hash(p: &str) -> String {
    j(&match_hash_json(p.as_bytes(), ""))["hashes"][0].as_str().unwrap_or_default().to_string()
}

// ---- p.mjs: legitimate text ----------------------------------------------------------------------

#[test]
fn legitimate_scripts_and_emoji_are_accepted() {
    let cases = [
        ("emoji_zwj_family", "Count: \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"),
        ("emoji_vs16", "Heart \u{2764}\u{FE0F}"),
        ("persian_zwnj", "\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}"),
        ("arabic", "\u{645}\u{627} \u{647}\u{648} \u{627}\u{644}\u{62C}\u{648}\u{627}\u{628}\u{61F}"),
        ("hebrew", "\u{5DE}\u{5D4} \u{5D4}\u{5EA}\u{5E9}\u{5D5}\u{5D1}\u{5D4}?"),
        ("arabic_letter_mark", "x \u{61C} y"),
        ("lrm_in_mixed", "\u{5D4}\u{5E0}\u{5D5}\u{5E1}\u{5D7}\u{5D4} \u{200E}H\u{2082}O\u{200E}"),
        ("hindi_conj_zwj", "\u{915}\u{94D}\u{200D}\u{937}"),
        ("hangul", "\u{D55C}\u{AD6D}\u{C5B4}"),
        ("combining", "x\u{302} + y\u{303}"),
        ("nbsp_units", "5\u{A0}kg and 10\u{202F}m/s"),
        ("soft_hyphen", "photo\u{AD}synthesis"),
        ("invisible_times", "2\u{2062}x"),
        ("flag_scotland", "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}"),
        ("wordjoiner", "a\u{2060}b"),
        ("latex", "$\\frac{a}{b}^{2}$ \u{B2} \u{2211}"),
        ("tabs_nl", "a\tb\nc"),
        ("long_line", &"x".repeat(8000)),
        ("emoji_skin", "\u{1F44B}\u{1F3FD}"),
        ("math_alnum", "\u{1D465}"),
        ("zwsp_thai", "\u{E2A}\u{E27}\u{E31}\u{E2A}\u{200B}\u{E14}\u{E35}"),
        ("ideographic_variant", "\u{8FBB}\u{E0100}"),
    ];
    for (name, stem) in cases {
        let out = j(&validate_json(qj(stem, tf(), 1).as_bytes(), ""));
        assert_eq!(out["ok"], true, "{name}: {out}");
    }
    // Titles: the joiners and directional marks are allowed in line text too.
    for name in ["\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}", "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}", "\u{5DE}\u{5D4} \u{5D4}\u{5EA}\u{5E9}\u{5D5}\u{5D1}\u{5D4}"] {
        let out = j(&validate_meta_json(json!({ "title": name }).to_string().as_bytes(), ""));
        assert_eq!(out["ok"], true, "{out}");
    }
}

#[test]
fn attack_strings_are_refused_with_position() {
    for (stem, cp, offset) in [
        ("ab\u{202E}cd", "U+202E", 2),
        ("a\u{2066}b", "U+2066", 1),
        ("x\u{FFF9}y", "U+FFF9", 1),
        ("x\u{206B}y", "U+206B", 1),
        ("\u{3164}", "U+3164", 0),
        ("x\u{115F}", "U+115F", 1),
        ("x\u{FFA0}", "U+FFA0", 1),
        ("\u{2800}\u{2800}", "U+2800", 0),
        ("x\u{1D173}", "U+1D173", 1),
        ("x\u{E0041}y", "U+E0041", 1),
    ] {
        let out = j(&validate_json(qj(stem, tf(), 1).as_bytes(), ""));
        let e = &out["errors"][0];
        assert_eq!((e["code"].as_str(), e["cp"].as_str(), e["offset"].as_u64()), (Some("forbidden_char"), Some(cp), Some(offset)), "{out}");
        assert!(!out.to_string().contains(stem), "no echo");
    }
    // Zalgo: more than 8 combining marks in a row.
    let out = j(&validate_json(qj(&format!("a{}", "\u{301}".repeat(share_core::text::MAX_MARK_RUN + 1)), tf(), 1).as_bytes(), ""));
    assert_eq!(out["errors"][0]["code"], "mark_run");
    // Invisible-only stems.
    for stem in [" ", "\u{200B}\u{200D}", "\u{AD}", "\u{301}", "\n\t "] {
        let out = j(&validate_json(qj(stem, tf(), 1).as_bytes(), ""));
        assert_eq!(out["errors"][0]["code"], "blank_text", "{}", stem.escape_unicode());
    }
}

// ---- p.mjs: degenerate questions -------------------------------------------------------------------

#[test]
fn degenerate_questions_are_refused() {
    for (name, stem, kind, code, path) in [
        ("empty_stem_tf", " ", tf(), "blank_text", "question.stem"),
        ("mc_zero", "q", json!({"Mc":{"answers":[]}}), "too_few", "question.kind.Mc.answers"),
        ("mc_one", "q", json!({"Mc":{"answers":[{"text":"a","correct":true}]}}), "too_few", "question.kind.Mc.answers"),
        ("match_zero", "q", json!({"Matching":{"pairs":[],"distractors":[]}}), "too_few", "question.kind.Matching.pairs"),
        ("match_emptyleft", "q", json!({"Matching":{"pairs":[{"left":"","right":""},{"left":"a","right":"b"}],"distractors":[]}}), "blank_text", "question.kind.Matching.pairs[0].left"),
        ("blank_zero", "q", json!({"Blank":{"accept":[]}}), "too_few", "question.kind.Blank.accept"),
        ("blank_mismatch", "a ___ b ___ c ___", json!({"Blank":{"accept":[["x"]]}}), "blank_count_mismatch", "question.kind.Blank.accept"),
        ("mp_zero", "q", json!({"Multipart":{"parts":[]}}), "too_few", "question.kind.Multipart.parts"),
        ("mp_blank_part", "q", json!({"Multipart":{"parts":[{"stem":" ","kind":{"Tf":{"correct":true}}}]}}), "blank_text", "question.kind.Multipart.parts[0].stem"),
        ("mp_mc_one", "q", json!({"Multipart":{"parts":[{"stem":"p","kind":{"Mc":{"answers":[{"text":"a"}]}}}]}}), "too_few", "question.kind.Multipart.parts[0].kind.Mc.answers"),
        ("mc_blank_choice", "q", json!({"Mc":{"answers":[{"text":"a"},{"text":"\u{200B}"}]}}), "blank_text", "question.kind.Mc.answers[1].text"),
    ] {
        let out = j(&validate_json(qj(stem, kind, 1).as_bytes(), ""));
        let errs = out["errors"].as_array().unwrap_or_else(|| panic!("{name} accepted"));
        assert!(errs.iter().any(|e| e["code"] == code && e["path"] == path), "{name}: {out}");
    }
    // Key completeness is a publish rule: a received keyless question needs none.
    let nocorrect = qj("q", json!({"Mc":{"answers":[{"text":"a"},{"text":"b"}]}}), 1);
    assert_eq!(j(&validate_json(nocorrect.as_bytes(), ""))["ok"], true);
    let out = j(&validate_for_publish_json(nocorrect.as_bytes(), ""));
    assert_eq!((out["errors"][0]["code"].as_str(), out["errors"][0]["path"].as_str()), (Some("no_correct"), Some("question.kind.Mc.answers")));
    let emptyvar = qj("a ___", json!({"Blank":{"accept":[[]]}}), 1);
    assert_eq!(j(&validate_json(emptyvar.as_bytes(), ""))["ok"], true);
    assert_eq!(j(&validate_for_publish_json(emptyvar.as_bytes(), ""))["errors"][0]["code"], "no_correct");
    let mp = qj("q", json!({"Multipart":{"parts":[{"stem":"p","kind":{"Mc":{"answers":[{"text":"a"},{"text":"b"}]}}}]}}), 1);
    assert_eq!(j(&validate_for_publish_json(mp.as_bytes(), ""))["errors"][0]["path"], "question.kind.Multipart.parts[0].kind.Mc.answers");
    // A full Matching question needs every right side; only a keyless one may leave them empty.
    let rights = qj("m", json!({"Matching":{"pairs":[{"left":"a","right":""},{"left":"b","right":"c"}],"distractors":[]}}), 1);
    assert_eq!(j(&validate_json(rights.as_bytes(), ""))["errors"][0]["path"], "question.kind.Matching.pairs[0].right");
}

// ---- p.mjs / p2.mjs: matching --------------------------------------------------------------------

#[test]
fn matching_stripped_contract_and_rekey() {
    let mt = qj("Match", json!({"Matching":{"pairs":[{"left":"France","right":"Paris"},{"left":"Texas","right":"Paris"},{"left":"Japan","right":"Tokyo"}],"distractors":[]}}), 1);
    let s = j(&strip_ok(mt.as_bytes(), ""));
    let k = &s["payload"]["question"]["kind"]["Matching"];
    assert!(k["pairs"].as_array().unwrap().iter().all(|p| p["right"] == ""));
    let mut bank: Vec<&str> = k["distractors"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    bank.sort();
    assert_eq!(bank, ["Paris", "Tokyo"]);

    // 30 pairs, 10 distractors: strip, re-validate, re-enter the key, republish; and the v1 re-save
    // of the imported (localised) copy.
    let pairs: Vec<Value> = (0..30).map(|i| json!({"left": format!("L{i}"), "right": format!("R{i}")})).collect();
    let full = qj("m", json!({"Matching":{"pairs": pairs, "distractors": (0..10).map(|i| format!("D{i}")).collect::<Vec<_>>()}}), 1);
    let st = j(&strip_ok(full.as_bytes(), ""));
    let stripped = serde_json::to_string(&st["payload"]).unwrap();
    assert_eq!(j(&validate_json(stripped.as_bytes(), ""))["ok"], true, "stripped revalidates");
    let v = validate(&stripped, &Limits::default(), &Structural).unwrap();
    let q = only_question(&v.payload).clone();
    let local = import::localize(&q);
    let Kind::Matching { pairs, distractors } = &local.kind else { panic!() };
    // No placeholder key: the rights stay empty and the app's column is the word bank.
    assert!(pairs.iter().all(|p| p.right.is_empty()), "no fake pairing in the local copy");
    assert_eq!(import::right_column(pairs, distractors).len(), 40);
    assert_eq!(local.explanation.as_deref(), Some(KEYLESS_SENTINEL));
    // An old app re-saving it as v1 (keyless dropped) leaves a question with no key: refused, so
    // it cannot be passed on as keyed until it is re-keyed.
    let mut v1 = local.clone();
    v1.keyless = false;
    let p1 = q_payload_v1(&v1);
    assert_eq!(j(&validate_json(p1.as_bytes(), ""))["errors"][0]["code"], "blank_text", "v1 re-save of the un-keyed copy");
    // The teacher re-enters the true key.
    let column = import::right_column(pairs, distractors);
    let picks: Vec<usize> = (0..30).map(|i| column.iter().position(|c| *c == format!("R{i}")).unwrap()).collect();
    let keyed = import::rekey_matching(&local, &picks).unwrap();
    let Kind::Matching { pairs, distractors } = &keyed.kind else { panic!() };
    assert!(pairs.iter().enumerate().all(|(i, p)| p.right == format!("R{i}")));
    let mut d = distractors.clone();
    d.sort();
    assert_eq!(d, (0..10).map(|i| format!("D{i}")).collect::<Vec<_>>(), "no duplicates, nothing lost");
    assert!(!keyed.keyless && keyed.explanation.is_none());
    let republish = q_payload(&keyed);
    assert_eq!(j(&validate_for_publish_json(republish.as_bytes(), ""))["ok"], true, "keyed-back republish");
    assert_eq!(hash(&republish), hash(&full), "same question, same match_hash");
    // A full publish that claims keyless gets no higher cap and cannot be published without hide_key.
    let mut claim = keyed.clone();
    claim.keyless = true;
    if let Kind::Matching { distractors, .. } = &mut claim.kind {
        distractors.extend((0..47).map(|i| format!("X{i}")));
    }
    let out = j(&validate_json(q_payload(&claim).as_bytes(), ""));
    assert_eq!(out["errors"][0]["code"], "too_many");
}

// ---- p.mjs / p2.mjs: hash ----------------------------------------------------------------------------

#[test]
fn hash_pairs() {
    let t = tf();
    for (name, a, b, equal) in [
        ("table vs line", qj("a | b\nc | d", t.clone(), 1), qj("a | b c | d", t.clone(), 1), false),
        ("para vs space", qj("Line one\n\nLine two", t.clone(), 1), qj("Line one Line two", t.clone(), 1), false),
        ("code ws", qj("`x  y`", t.clone(), 1), qj("`x y`", t.clone(), 1), true),
        ("mc label I.", qj("Which?", json!({"Mc":{"answers":[{"text":"I. only"},{"text":"V. only"},{"text":"X"}]}}), 1), qj("Which?", json!({"Mc":{"answers":[{"text":"only"},{"text":"only"},{"text":"X"}]}}), 1), false),
        ("blank vs short", qj("x ___", json!({"Blank":{"accept":[["1"]]}}), 1), qj("x ___", json!({"Short":{"sample":"1"}}), 1), false),
        ("matching swap left/right", qj("m", json!({"Matching":{"pairs":[{"left":"A","right":"1"},{"left":"B","right":"2"}],"distractors":[]}}), 1), qj("m", json!({"Matching":{"pairs":[{"left":"1","right":"A"},{"left":"2","right":"B"}],"distractors":[]}}), 1), false),
        ("mc dup choice", qj("q", json!({"Mc":{"answers":[{"text":"5"},{"text":"5"},{"text":"6"}]}}), 1), qj("q", json!({"Mc":{"answers":[{"text":"5"},{"text":"6"},{"text":"6"}]}}), 1), false),
        ("fullwidth", qj("x\u{FF11}", t.clone(), 1), qj("x1", t.clone(), 1), false),
        ("case", qj("Paris", t.clone(), 1), qj("paris", t.clone(), 1), false),
        ("E. coli vs K. coli", qj("Which is Gram-negative?", json!({"Mc":{"answers":[{"text":"E. coli","correct":true},{"text":"S. aureus"},{"text":"B. subtilis"}]}}), 1),
            qj("Which is Gram-negative?", json!({"Mc":{"answers":[{"text":"K. coli"},{"text":"T. aureus"},{"text":"Q. subtilis"}]}}), 1), false),
    ] {
        let (ha, hb) = (hash(&a), hash(&b));
        assert!(!ha.is_empty() && !hb.is_empty(), "{name}");
        assert_eq!(ha == hb, equal, "{name}");
    }
    // The biology choices look like labels but are not order-revealing: the author keeps them,
    // nothing is removed, and the question is still shuffled.
    let bio = qj("Which is Gram-negative?", json!({"Mc":{"answers":[{"text":"E. coli","correct":true},{"text":"S. aureus"},{"text":"B. subtilis"}]}}), 1);
    assert_eq!(j(&strip_ok(bio.as_bytes(), ""))["errors"][0]["code"], "labels_need_decision");
    let s = j(&strip_ok(bio.as_bytes(), r#"{"labels":[{"question":0,"choice":"keep"}]}"#));
    let mut texts: Vec<&str> = s["payload"]["question"]["kind"]["Mc"]["answers"].as_array().unwrap().iter().map(|a| a["text"].as_str().unwrap()).collect();
    texts.sort();
    assert_eq!(texts, ["B. subtilis", "E. coli", "S. aureus"]);
    assert_eq!(s["labels_removed"], json!([]));
    assert_eq!(s["needs_order_ack"], json!([]));
}

// ---- critic11b/probe ---------------------------------------------------------------------------------

#[test]
fn bank_origin_hashes_do_not_survive_strip() {
    let h = "a".repeat(64);
    let bank = format!(
        r#"{{"schema_version":2,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{{"id":"q","stem":"s","kind":{{"Tf":{{"correct":true}}}},"points":null,"difficulty":null,"created":1,"modified":1}}],"origin":{{"item_id":"i","publish_id":"p","version":1,"author_name":"A","license":"CC0-1.0","form":"full","served_sha256":"{h}","match_hash":"{h}"}},"published":{{"item_id":"PUBX","publish_id":"PUBY","version":3}}}}}}"#
    );
    let r = strip_ok(bank.as_bytes(), "");
    assert!(j(&r)["ok"] == true, "{r}");
    assert!(!r.contains(&h) && !r.contains("PUBX") && !r.contains("PUBY"), "{r}");
    assert!(r.contains("\"author_name\":\"A\""), "the rest of the origin stays");
}

#[test]
fn label_variants() {
    for (labels, want, removed) in [
        (["1. a", "2. b", "3. c", "4. d"], ["a", "b", "c", "d"], true),
        (["(a) a", "(b) b", "(c) c", "(d) d"], ["a", "b", "c", "d"], true),
        (["i. a", "ii. b", "iii. c", "iv. d"], ["a", "b", "c", "d"], true),
        (["\u{FF21}\u{FF0E}a", "\u{FF22}\u{FF0E}b", "\u{FF23}\u{FF0E}c", "\u{FF24}\u{FF0E}d"], ["a", "b", "c", "d"], true),
        (["A- a", "B- b", "C- c", "D- d"], ["a", "b", "c", "d"], true),
        (["a", "b", "C. c", "d"], ["a", "b", "C. c", "d"], false),
    ] {
        let ans: Vec<Value> = labels.iter().enumerate().map(|(i, l)| json!({"text": l, "correct": i == 1})).collect();
        let p = qj("s", json!({"Mc":{"answers": ans}}), 1);
        let choice = if removed { "strip" } else { "keep" };
        let v = j(&strip_seed_ok(p.as_bytes(), &[9; 32], &json!({"labels":[{"question":0,"choice":choice}]}).to_string()));
        let texts: Vec<String> = v["payload"]["question"]["kind"]["Mc"]["answers"].as_array().unwrap_or_else(|| panic!("{v}")).iter().map(|a| a["text"].as_str().unwrap().to_string()).collect();
        if removed {
            let (mut t, mut w) = (texts.clone(), want.iter().map(|s| s.to_string()).collect::<Vec<_>>());
            t.sort();
            w.sort();
            assert_eq!(t, w, "{labels:?}");
            assert_eq!(v["labels_removed"], json!([{"question": 0}]), "{labels:?}");
        } else {
            // A lone position label kept: nothing moves, and the order must be acknowledged.
            assert_eq!(texts, want, "{labels:?}");
            assert_eq!(v["needs_order_ack"], json!([{"question": 0}]));
            assert_eq!(v["order_reasons"][0]["reason"], "labels_kept");
            // Strip is not allowed here.
            let r = j(&strip_ok(p.as_bytes(), &json!({"labels":[{"question":0,"choice":"strip"}]}).to_string()));
            assert_eq!(r["errors"][0]["code"], "labels_strip_refused");
        }
    }
}

#[test]
fn numbers_are_canonical() {
    let p = r#"{"schema_version":1,"question":{"id":"q","stem":"s","kind":{"Tf":{"correct":true}},"points":-0.0,"difficulty":null,"created":1,"modified":1,"figure":{"id":"f","params":{"a":18446744073709551615,"b":1.0,"c":1,"d":-0.0,"e":2.5}}}}"#;
    let out = j(&validate_json(p.as_bytes(), ""));
    let q = &out["payload"]["question"];
    assert_eq!(q["points"].to_string(), "0.0");
    let params = &q["figure"]["params"];
    assert_eq!(params["a"].to_string(), "18446744073709551615");
    assert_eq!((params["b"].to_string(), params["c"].to_string(), params["d"].to_string(), params["e"].to_string()), ("1".into(), "1".into(), "0".into(), "2.5".into()));
    // So 1 and 1.0 hash the same.
    let other = p.replace("\"b\":1.0", "\"b\":1").replace("\"points\":-0.0", "\"points\":0");
    assert_eq!(j(&full_hash_json(p.as_bytes(), ""))["hashes"], j(&full_hash_json(other.as_bytes(), ""))["hashes"]);
}

#[test]
fn declared_registry_edge_specs() {
    let p = r##"{"schema_version":1,"question":{"id":"q","stem":"s","kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":1,"modified":1,"figure":{"id":"f","params":{"a":1e308,"b":-1e308,"c":"x","d":"P1","e":"red\" onload=\"x","g":"#111111"}}}}"##;
    let opts = r##"{"figures":{"f":[{"key":"a","type":"int","min":0,"max":5},{"key":"b","type":"float","min":5,"max":1},{"key":"c","type":"choice","options":[],"default":"zz"},{"key":"d","type":"choice","options":["P1","P2"],"default":"P9"},{"key":"e","type":"color","palette":["#111111"],"default":"#333333"},{"key":"g","type":"color","palette":["#111111"],"default":"red\" x=\""}]}}"##;
    let out = j(&validate_json(p.as_bytes(), opts));
    let params = &out["payload"]["question"]["figure"]["params"];
    assert_eq!(params["a"], 5);
    assert!(params.get("b").is_none() && params.get("c").is_none() && params.get("d").is_none() && params.get("g").is_none(), "{params}");
    // A colour default outside the palette is fine when it is a #rrggbb colour (the app has one);
    // anything else outside the palette becomes that default.
    assert_eq!(params["e"], "#333333");
    assert!(out["dropped"].as_array().unwrap().iter().any(|d| d["reason"] == "param_spec_invalid"));
}

#[test]
fn hostile_options_cannot_lift_the_safety_bounds() {
    let deep = format!(r#"{{"schema_version":1,"question":{{"x":{}0{}}}}}"#, "[".repeat(100_000), "]".repeat(100_000));
    let out = j(&validate_json(deep.as_bytes(), r#"{"limits":{"max_depth":1000000,"max_values":100000000,"parse_budget_bytes":100000000000}}"#));
    assert_eq!(out["errors"][0]["code"], "too_deep");
    assert_eq!(out["errors"][0]["limit"], 16, "memory-sizing limits can be lowered, never raised");
}

#[test]
fn slow_paths_stay_fast() {
    // Label layers: linear, not quadratic.
    // 640 layers: the canonical output (defaults written out) must still fit the 4 MB limit.
    let lab = "A. ".repeat(640);
    let lab2 = "B. ".repeat(640);
    let q = |i: usize| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Mc":{{"answers":[{{"text":"{lab}x","correct":true}},{{"text":"{lab2}y"}}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#);
    let qs: Vec<String> = (0..1000).map(q).collect();
    let p = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","));
    let t = Instant::now();
    assert_eq!(j(&match_hash_json(p.as_bytes(), ""))["ok"], true);
    let keep: Vec<Value> = (0..1000).map(|i| json!({"question": i, "choice": "keep"})).collect();
    let out = strip_ok(p.as_bytes(), &json!({ "labels": keep }).to_string());
    assert!(out.contains("\"ok\":true"), "{}", &out[..300.min(out.len())]);
    // Release: about 0.4 s on the build machine; unoptimised builds are ~50x slower at NFC.
    let bound = if cfg!(debug_assertions) { 90.0 } else { 15.0 }; // generous: the mutation runs share the machine
    assert!(t.elapsed().as_secs_f64() < bound, "{:?}", t.elapsed());
    // NFC of a 4 MB run of combining marks or of an expanding character: refused before any of it is built.
    let s = format!("a{}", "\u{0301}\u{0316}".repeat(1_000_000));
    let t = Instant::now();
    assert_eq!(j(&validate_json(qj(&s, tf(), 1).as_bytes(), ""))["errors"][0]["code"], "mark_run");
    let s = "\u{1D160}".repeat(1_000_000);
    assert_eq!(j(&validate_json(qj(&s, tf(), 1).as_bytes(), ""))["errors"][0]["code"], "too_long");
    assert!(t.elapsed().as_secs_f64() < if cfg!(debug_assertions) { 20.0 } else { 1.0 }, "{:?}", t.elapsed());
}

#[test]
fn https_only_license_urls() {
    for (url, ok_) in [
        ("https://creativecommons.org/licenses/by/4.0/", true),
        ("https://example.org:8443/x?y#z", true),
        ("http://creativecommons.org/licenses/by/4.0/", false),
        ("https://user:pw@evil.example/", false),
        ("https://evil.example@good.example/", false),
        ("javascript:alert(1)", false),
        ("https://", false),
        ("https://localhost/", false),
        ("https://a.example/<script>", false),
        ("https://exa mple.org/", false),
    ] {
        assert_eq!(validate::is_web_url(url), ok_, "{url}");
    }
}
