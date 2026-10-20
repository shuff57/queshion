//! Regression tests from the round-4 critics (scratchpad critic3a: security probes; critic3b:
//! behaviour probes). Each test is the probe's input with the behaviour the fixes require.
mod common;
use common::*;
use serde_json::{Value, json};
use share_core::labels::{SiblingRef, decoration, key_in_formatting, sibling_ref};
use share_core::wasm::*;
use share_core::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

fn mcq(texts: &[&str], correct: &[usize], locked: &[usize], stem: &str) -> String {
    json!({"schema_version": 1, "question": {"id":"q1","stem":stem,"kind":{"Mc":{"answers": texts.iter().enumerate().map(|(i,t)| json!({"text":t,"correct":correct.contains(&i),"locked":locked.contains(&i)})).collect::<Vec<_>>()}},"points":1.0,"difficulty":null,"created":0,"modified":0}}).to_string()
}

fn served(o: &Value) -> Vec<String> {
    o["payload"]["question"]["kind"]["Mc"]["answers"].as_array().unwrap_or_else(|| panic!("{o}")).iter().map(|a| a["text"].as_str().unwrap().to_string()).collect()
}

// ---- S1: findings and previews stay small ---------------------------------------------------------

fn many_choices(nq: usize, np: usize, w: usize) -> String {
    let l = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let part = |k: usize| format!(r#"{{"stem":"p","kind":{{"Mc":{{"answers":[{}]}}}}}}"#, (0..26).map(|i| format!(r#"{{"text":"{}. w{k}{i}{}"{}}}"#, l[i] as char, "x".repeat(w), if i == 0 { r#","correct":true"# } else { "" })).collect::<Vec<_>>().join(","));
    let qs: Vec<String> = (0..nq).map(|i| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Multipart":{{"parts":[{}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#, (0..np).map(part).collect::<Vec<_>>().join(","))).collect();
    format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","))
}

#[test]
fn findings_and_previews_are_capped() {
    let p = many_choices(41, 50, 25);
    let f = j(&label_findings_json(p.as_bytes(), ""));
    assert_eq!(f["label_findings_total"], 2050);
    let listed = f["label_findings"].as_array().unwrap();
    assert_eq!(listed.len(), 2000, "at most MAX_FINDINGS_OUT listed");
    let with: usize = listed.iter().filter(|x| x.get("choices").is_some()).count();
    assert!(with > 0 && listed.iter().any(|x| x["truncated"] == true), "previews stop at the budget");
    let preview_bytes: usize = listed.iter().filter_map(|x| x.get("choices")).map(|c| c.to_string().len()).sum();
    assert!(preview_bytes < 2 * share_core::strip::PREVIEW_BUDGET, "{preview_bytes}");
    // The refusal carries the same capped list; strip results carry decisions only.
    let r = j(&strip_ok(p.as_bytes(), ""));
    assert_eq!(r["errors"][0]["code"], "labels_need_decision");
    assert!(r.to_string().len() < 2 * 1024 * 1024, "{}", r.to_string().len());
    let v = ok(&p);
    let o = j(&strip_ok(p.as_bytes(), &labels_json(&v.payload, LabelChoice::Strip)));
    assert_eq!(o["ok"], true);
    assert!(o["label_findings"].as_array().unwrap().iter().all(|x| x.get("choices").is_none()));
}

// ---- S2: every stripped Matching revalidates ------------------------------------------------------

#[test]
fn stripped_matching_always_revalidates() {
    for (np, nd) in [(2, 1), (2, 26), (2, 27), (2, 28), (50, 26), (50, 76), (10, 30), (10, 26)] {
        let q = json!({"schema_version":1,"question":{"id":"q","stem":"Match","points":null,"difficulty":null,"created":0,"modified":0,
            "kind":{"Matching":{"pairs": (0..np).map(|i| json!({"left": format!("L{i}"), "right": format!("R{i}")})).collect::<Vec<_>>(), "distractors": (0..nd).map(|i| format!("D{i}")).collect::<Vec<_>>()}}}}).to_string();
        let v = j(&validate_json(q.as_bytes(), ""));
        let s = j(&strip_ok(q.as_bytes(), ""));
        if np + nd > np + 26 {
            assert_eq!(v["errors"][0]["code"], "too_many", "{np} {nd}");
            assert_eq!(s["ok"], false);
        } else {
            assert_eq!(v["ok"], true, "{np} {nd}: {v}");
            let back = j(&validate_json(serde_json::to_string(&s["payload"]).unwrap().as_bytes(), ""));
            assert_eq!(back["ok"], true, "{np} {nd}: {back}");
        }
    }
    // Shared rights count once in the union.
    let q = json!({"schema_version":1,"question":{"id":"q","stem":"Match","points":null,"difficulty":null,"created":0,"modified":0,
        "kind":{"Matching":{"pairs":[{"left":"a","right":"X"},{"left":"b","right":"X"}], "distractors": (0..27).map(|i| format!("D{i}")).collect::<Vec<_>>()}}}}).to_string();
    assert_eq!(j(&validate_json(q.as_bytes(), ""))["ok"], true, "union X + 27 = 28 = 2 + 26");
}

// ---- B1: sibling references, keep_order, the shuffle report ----------------------------------------

#[test]
fn sibling_reference_forms() {
    for t in [
        "both a and b", "a and c", "a & b", "Both a & b", "A and B are correct", "Both of them", "Neither of them", "Both answers",
        "a, b, and c", "All except B", "Only B", "B only", "C is correct but not D", "Choices a and b", "(a) and (b)", "a and b only",
        "A+B", "A/B", "Both A and B.", "A and B both", "A and/or B", "A + B", "All but C", "1 and 2", "I and II", "Both", "Neither",
        "The previous two answers", "The first two", "Both (a) and (b)", "B", "(c)", "Ambas", "a y b", "a et b", "A und B",
    ] {
        assert_eq!(sibling_ref(t), Some(SiblingRef::Labels), "{t}");
    }
    for t in [
        "All above", "None of above", "All of them", "None of the others", "All the options", "All of the choices above", "All of the previous",
        "Todas las anteriores", "Toutes ces r\u{e9}ponses", "Ninguna de las anteriores", "All are correct", "All the answers above", "None of the options listed",
        "None of the above.",
    ] {
        assert_eq!(sibling_ref(t), Some(SiblingRef::Above), "{t}");
    }
    for t in ["Mitochondria", "A large and bright star", "a cell and a nucleus", "Vitamin C", "I think so", "Mars and Venus", "x = 2", "the Golgi apparatus"] {
        assert_eq!(sibling_ref(t), None, "{t}");
    }
}

#[test]
fn sibling_references_freeze_the_order_even_with_strip() {
    let p = mcq(&["a) Mitochondria", "b) Ribosome", "c) Both (a) and (b)", "d) Nucleus"], &[2], &[], "Pick");
    let o = j(&strip_seed_ok(p.as_bytes(), &[1; 32], r#"{"labels":[{"question":0,"choice":"strip"}]}"#));
    assert_eq!(served(&o), ["Mitochondria", "Ribosome", "Both (a) and (b)", "Nucleus"], "labels stripped, order frozen");
    assert_eq!(o["order_reasons"][0]["reason"], "sibling_refs");
    // The ack is required, not optional.
    let r = j(&strip_with_seed_json(p.as_bytes(), &[1; 32], r#"{"labels":[{"question":0,"choice":"strip"}]}"#));
    assert_eq!(r["errors"][0]["code"], "order_ack_required");
    assert_eq!(r["needs_order_ack"], json!([{"question": 0}]));
    for texts in [
        vec!["Mitochondria", "Ribosome", "A and/or B", "Nucleus"],
        vec!["Mitochondria", "Ribosome", "All except A", "Nucleus"],
        vec!["Mitochondria", "Ribosome", "1 and 2", "Nucleus"],
        vec!["Mitochondria", "Ribosome", "Todas las anteriores", "Nucleus"],
        vec!["Mitochondria", "Ribosome", "Both", "Nucleus"],
        vec!["Mitochondria", "Ribosome", "The first two", "Nucleus"],
    ] {
        let o = j(&strip_ok(mcq(&texts, &[2], &[], "Pick").as_bytes(), ""));
        assert_eq!(served(&o), texts, "{texts:?}");
    }
}

#[test]
fn author_keep_order_and_the_served_order_report() {
    let mut q = question("Order?", Kind::Mc { answers: (0..6).map(|i| answer(&format!("step {i}"), i == 0)).collect() });
    q.keep_order = true;
    let p = q_payload(&q);
    let o = j(&strip_ok(p.as_bytes(), ""));
    assert_eq!(served(&o), (0..6).map(|i| format!("step {i}")).collect::<Vec<_>>());
    assert_eq!(o["order_reasons"][0]["reason"], "author_keep_order");
    assert_eq!(o["served_orders"], json!([]));
    // v1 cannot carry it: refused rather than silently shuffled.
    assert_eq!(j(&validate_json(q_payload_v1(&q).as_bytes(), ""))["errors"][0]["code"], "v2_field_in_v1");
    // Without it the question is shuffled, and the report says how: order[k] = author index at k.
    q.keep_order = false;
    let o = j(&strip_seed_ok(q_payload(&q).as_bytes(), &[4; 32], ""));
    let order: Vec<usize> = serde_json::from_value(o["served_orders"][0]["order"].clone()).unwrap();
    let texts = served(&o);
    for (k, &from) in order.iter().enumerate() {
        assert_eq!(texts[k], format!("step {from}"));
    }
    // A matching word bank is reported too (indices into the author's right column).
    let m = question("m", Kind::Matching { pairs: (0..4).map(|i| Pair { left: format!("l{i}"), right: format!("r{i}") }).collect(), distractors: vec!["d".into()] });
    let o = j(&strip_seed_ok(q_payload(&m).as_bytes(), &[4; 32], ""));
    let r = &o["served_orders"][0];
    assert_eq!(r["matching"], true);
    let col = ["r0", "r1", "r2", "r3", "d"];
    let bank: Vec<String> = serde_json::from_value(o["payload"]["question"]["kind"]["Matching"]["distractors"].clone()).unwrap();
    for (k, i) in r["order"].as_array().unwrap().iter().enumerate() {
        assert_eq!(bank[k], col[i.as_u64().unwrap() as usize]);
    }
    // keep_order is in the full hash, not in match_hash.
    let mut a = question("Order?", Kind::Tf { correct: true });
    let b = a.clone();
    a.keep_order = true;
    assert_eq!(match_hash(&a), match_hash(&b));
    assert_ne!(full_hash(&a), full_hash(&b));
}

// ---- B2: keys carried by formatting ----------------------------------------------------------------

#[test]
fn formatting_cannot_carry_the_key() {
    for t in [
        vec!["**Venus**", "Mars", "Earth"],
        vec!["Venus*", "Mars", "Earth"],
        vec!["*Venus*", "Mars", "Earth"],
        vec!["\u{2713} Venus", "Mars", "Earth"],
        vec!["Venus (correct)", "Mars", "Earth"],
        vec!["<b>Venus</b>", "Mars", "Earth"],
        vec!["Venus", "**Mars**", "**Earth**"],
    ] {
        let p = mcq(&t, &[0], &[], "Hottest?");
        let o = j(&strip_ok(p.as_bytes(), ""));
        assert_eq!(o["errors"][0]["code"], "key_in_formatting", "{t:?}");
        assert_eq!(o["errors"][0]["path"], "question.kind.Mc.answers[0]", "indices only");
        assert!(!o.to_string().contains("Venus"));
        // The full (non-hide_key) publish is fine: the key is public there anyway.
        assert_eq!(j(&validate_for_publish_json(p.as_bytes(), ""))["ok"], true);
    }
    // The same decoration on every choice is fine, and so is decoration on a wrong one only.
    for (t, c) in [(vec!["**Venus**", "**Mars**", "**Earth**"], 0), (vec!["Venus", "Mars", "Earth"], 1), (vec!["Venus", "**Mars**", "Earth"], 0)] {
        let o = j(&strip_ok(mcq(&t, &[c], &[], "Hottest?").as_bytes(), ""));
        assert_eq!(o["ok"], true, "{t:?}: {o}");
    }
    assert_eq!(decoration("**x**"), decoration("**y**"));
    assert_ne!(decoration("x (correct)"), decoration("x"));
    assert!(key_in_formatting(&[("a", true), ("b", false)]).is_empty());
}

// ---- B3: no placeholder key on import (the share-core side) ----------------------------------------

#[test]
fn keyless_copies_carry_no_placeholder() {
    let p = json!({"schema_version":1,"question":{"id":"q","stem":"Match","points":null,"difficulty":null,"created":0,"modified":0,
        "kind":{"Matching":{"pairs":[{"left":"Dog","right":"Mammal"},{"left":"Frog","right":"Amphibian"}],"distractors":["Bird"]}}}}).to_string();
    let s = j(&strip_ok(p.as_bytes(), ""));
    let st = validate(&serde_json::to_string(&s["payload"]).unwrap(), &Limits::default(), &Structural).unwrap();
    let local = import::localize(only_question(&st.payload));
    let Kind::Matching { pairs, distractors } = &local.kind else { panic!() };
    assert!(pairs.iter().all(|p| p.right.is_empty()));
    assert_eq!(distractors.len(), 3);
    // Until re-keyed it is refused as a keyed publish.
    assert_eq!(j(&validate_for_publish_json(q_payload(&local).as_bytes(), ""))["errors"][0]["code"], "keyless_needs_hide_key");
}

// ---- non-blocking fixes ------------------------------------------------------------------------------

#[test]
fn rising_labels_with_gaps_reveal_the_order() {
    for t in [vec!["A. Venus", "B. Mars", "D. Earth"], vec!["A. Venus", "C. Mars", "E. Earth"], vec!["Venus", "B. Mars", "C. Earth"]] {
        let p = mcq(&t, &[0], &[], "Pick");
        let o = j(&strip_ok(p.as_bytes(), r#"{"labels":[{"question":0,"choice":"keep"}]}"#));
        assert_eq!(served(&o), t, "{t:?}");
        assert_eq!(o["needs_order_ack"], json!([{"question": 0}]));
    }
}

#[test]
fn decisions_are_exact() {
    let labelled = mcq(&["A) Venus", "B) Mars"], &[0], &[], "Hot?");
    for (opts, code) in [
        (r#"{"labels":[{"question":0,"choice":"keep"},{"question":0,"choice":"strip"}]}"#, "labels_duplicate"),
        (r#"{"labels":[{"question":0,"choice":"keep"},{"question":0,"part":3,"choice":"keep"}]}"#, "labels_unused"),
    ] {
        let a = j(&strip_ok(labelled.as_bytes(), opts));
        assert_eq!(a["errors"][0]["code"], code, "{opts}");
        let mut po: Value = serde_json::from_str(opts).unwrap();
        po["hide_key"] = json!(true);
        let b = j(&validate_for_publish_json(labelled.as_bytes(), &po.to_string()));
        assert_eq!(b["errors"][0]["code"], code, "validate_for_publish agrees: {opts}");
    }
    // A decision for a question with no finding.
    let plain = mcq(&["Venus", "Mars"], &[0], &[], "Hot?");
    assert_eq!(j(&strip_ok(plain.as_bytes(), r#"{"labels":[{"question":0,"choice":"strip"}]}"#))["errors"][0]["code"], "labels_unused");
}

#[test]
fn only_the_recommended_flag_tag_sequences() {
    let tf = |stem: &str| json!({"schema_version":1,"question":{"id":"q","stem":stem,"kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":0,"modified":0}}).to_string();
    let tag = |s: &str| format!("\u{1F3F4}{}\u{E007F}", s.chars().map(|c| char::from_u32(0xE0000 + c as u32).unwrap()).collect::<String>());
    for ok_ in ["gbeng", "gbsct", "gbwls"] {
        assert_eq!(j(&validate_json(tf(&format!("Flag {}", tag(ok_))).as_bytes(), ""))["ok"], true, "{ok_}");
    }
    for bad in ["answer", "isb", "usca", "gbxyz"] {
        assert_eq!(j(&validate_json(tf(&format!("Pick {}", tag(bad))).as_bytes(), ""))["errors"][0]["code"], "forbidden_char", "{bad}");
    }
}

#[test]
fn mark_runs_cannot_be_split_or_hidden() {
    let tf = |stem: &str| json!({"schema_version":1,"question":{"id":"q","stem":stem,"kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":0,"modified":0}}).to_string();
    // ZWJ between groups of marks does not end the run.
    let z = format!("a{}", format!("{}\u{200D}", "\u{301}".repeat(16)).repeat(200));
    assert_eq!(j(&validate_json(tf(&z).as_bytes(), ""))["errors"][0]["code"], "mark_run");
    // U+0F73 is two marks in decomposition: nine of them are refused, at an input offset.
    let r = j(&validate_json(tf(&format!("x{}", "\u{F73}".repeat(9))).as_bytes(), ""));
    assert_eq!(r["errors"][0]["code"], "mark_run");
    assert!(r["errors"][0]["offset"].as_u64().unwrap() <= 9, "{r}");
    // Precomposed letters are not runs.
    assert_eq!(j(&validate_json(tf(&"\u{e9}".repeat(4000)).as_bytes(), ""))["ok"], true);
}

#[test]
fn nfc_changes_are_reported() {
    let tf = |stem: &str| json!({"schema_version":1,"question":{"id":"q","stem":stem,"kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":0,"modified":0}}).to_string();
    for (s, want) in [("R = 5 \u{2126}", "R = 5 \u{3A9}"), ("1 \u{212B}", "1 \u{C5}"), ("\u{F900}", "\u{8C48}")] {
        let o = j(&validate_json(tf(s).as_bytes(), ""));
        assert_eq!(o["payload"]["question"]["stem"], want);
        assert_eq!(o["dropped"], json!([{"path": "question.stem", "reason": "nfc_changed"}]), "{s}");
    }
    assert_eq!(j(&validate_json(tf("plain").as_bytes(), ""))["dropped"], json!([]));
}

#[test]
fn memory_limits_can_only_be_lowered() {
    let d = Limits::default();
    let raised = Limits { body_bytes: usize::MAX, max_depth: 1 << 20, max_values: usize::MAX, max_objects: usize::MAX, parse_budget_bytes: usize::MAX, figure_params: usize::MAX, ..Default::default() }.sane();
    assert_eq!((raised.body_bytes, raised.max_depth, raised.max_values, raised.max_objects, raised.parse_budget_bytes, raised.figure_params), (d.body_bytes, d.max_depth, d.max_values, d.max_objects, d.parse_budget_bytes, d.figure_params));
    let lowered = Limits { max_values: 10, ..Default::default() }.sane();
    assert_eq!(lowered.max_values, 10);
}

#[test]
fn declared_registry_cost_is_linear() {
    let specs: Vec<Value> = (0..30_000).map(|i| json!({"key": format!("k{i}"), "type": "bool"})).collect();
    let opts = json!({"figures": {"f": specs}}).to_string();
    let ps = (0..256).map(|k| format!(r#""z{k}":1"#)).collect::<Vec<_>>().join(",");
    let qs: Vec<String> = (0..300).map(|i| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Tf":{{"correct":true}}}},"points":null,"difficulty":null,"created":1,"modified":1,"figure":{{"id":"f","params":{{{ps}}}}}}}"#)).collect();
    let big = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","));
    let t = std::time::Instant::now();
    assert_eq!(j(&validate_json(big.as_bytes(), &opts))["ok"], true);
    assert!(t.elapsed().as_secs_f64() < if cfg!(debug_assertions) { 30.0 } else { 1.0 }, "{:?}", t.elapsed());
}
