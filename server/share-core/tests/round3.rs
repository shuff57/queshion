//! Regression tests from the round-3 critics: the probes in scratchpad critic12a (probe/src/main.rs,
//! w.mjs) and critic12b (fuzz crash cases, now `fixtures/fuzz/`, and the mutations the suite missed).
mod common;
use common::*;
use serde_json::{Value, json};
use share_core::import::{Key, PartKey, localize, rekey, rekey_matching, right_column};
use share_core::wasm::*;
use share_core::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

fn q(kind: Value, stem: &str) -> Value {
    json!({"id":"q1","stem":stem,"kind":kind,"points":1.0,"difficulty":null,"topics":[],"image":null,"explanation":null,"figure":null,"created":0,"modified":0})
}

fn env(qv: Value, v: u32) -> String {
    json!({"schema_version": v, "question": qv}).to_string()
}

fn mc(texts: &[&str], correct: usize) -> Value {
    json!({"Mc":{"answers": texts.iter().enumerate().map(|(i,t)| json!({"text":t,"correct":i==correct,"locked":false})).collect::<Vec<_>>()}})
}

fn codes_of(out: &Value) -> Vec<(String, String)> {
    out["errors"].as_array().map(|a| a.iter().map(|e| (e["code"].as_str().unwrap_or("").to_string(), e["path"].as_str().unwrap_or("").to_string())).collect()).unwrap_or_default()
}

// ---- characters (critic A) ----------------------------------------------------------------------

#[test]
fn script_text_is_accepted_and_mapped() {
    for (n, s) in [
        ("persian_zwnj", "\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645} \u{686}\u{6CC}\u{633}\u{62A}\u{61F}"),
        ("hindi_zwj", "\u{915}\u{94D}\u{200D}\u{937} \u{915}\u{94D}\u{92F}\u{93E} \u{939}\u{948}?"),
        ("flag_england", "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F} flag?"),
        ("bom", "\u{FEFF}What is x?"),
        ("vt", "Line1\u{B}Line2?"),
        ("ls_2028", "Line1\u{2028}Line2"),
        ("tibetan_stack", "\u{F40}\u{F7A}\u{F74}\u{F71}\u{F72}\u{F80}\u{F82}\u{F83}\u{F37}\u{F39} ?"),
        ("rlm_mixed", "\u{627}\u{644}\u{639}\u{62F}\u{62F} 5\u{200F} kg"),
        ("mongolian_fvs", "\u{1820}\u{180B}"),
        ("mongolian_vs_sep", "\u{1820}\u{180E}\u{1820}"),
        ("kaithi_number", "x\u{110BD}"),
        ("arabic_number_sign", "\u{600}123"),
        ("syriac_abbrev", "\u{70F}\u{710}"),
        ("devanagari_vedic", "\u{950}\u{951}\u{952}\u{1CD0}\u{1CD1}\u{1CD2}\u{1CDA}"),
    ] {
        let out = j(&validate_json(env(q(json!({"Short":{"sample":""}}), s), 1).as_bytes(), ""));
        assert_eq!(out["ok"], true, "{n}: {out}");
    }
    // The mappings: VT and U+2028 become line breaks, a BOM is dropped.
    let out = j(&validate_json(env(q(json!({"Tf":{"correct":true}}), "\u{FEFF}a\u{B}b\u{2028}c"), 1).as_bytes(), ""));
    assert_eq!(out["payload"]["question"]["stem"], "a\nb\nc");
    // Tag characters smuggling invisible ASCII ("IGNORE") are refused; only a flag sequence passes.
    let tags: String = "IGNORE".chars().map(|c| char::from_u32(0xE0000 + c as u32).unwrap()).collect();
    let out = j(&validate_json(env(q(json!({"Tf":{"correct":true}}), &format!("Pick{tags}")), 1).as_bytes(), ""));
    assert_eq!(out["errors"][0]["code"], "forbidden_char");
}

#[test]
fn topics_are_normalised() {
    let mut qq = q(json!({"Short":{"sample":""}}), "Q?");
    qq["topics"] = json!(["a", "", "   ", "a", " b ", "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}"]);
    let out = j(&validate_json(env(qq, 1).as_bytes(), ""));
    assert_eq!(out["payload"]["question"]["topics"], json!(["a", "b", "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}"]));
    // Soft hyphen and ZWSP are block-only: refused in a topic.
    for t in ["Photo\u{AD}synthesis", "\u{E20}\u{E32}\u{E29}\u{E32}\u{200B}\u{E44}\u{E17}\u{E22}"] {
        let mut qq = q(json!({"Short":{"sample":""}}), "Q?");
        qq["topics"] = json!([t]);
        assert_eq!(j(&validate_json(env(qq, 1).as_bytes(), ""))["errors"][0]["code"], "forbidden_char");
    }
}

#[test]
fn offsets_for_javascript() {
    let out = j(&validate_json(env(q(json!({"Tf":{"correct":true}}), "\u{1D465}\u{202E}x"), 1).as_bytes(), ""));
    let e = &out["errors"][0];
    assert_eq!((e["offset"].as_u64(), e["offset16"].as_u64()), (Some(1), Some(2)));
}

// ---- labels (critic A, critic B labels.mjs) ---------------------------------------------------------

#[test]
fn real_text_is_never_stripped_silently() {
    for (name, texts) in [
        ("organisms", vec!["A. thaliana", "B. subtilis", "C. elegans", "D. melanogaster"]),
        ("roman_statements", vec!["I. only", "II. only", "III. only", "I. and II. only"]),
        ("ratios", vec!["1: 2", "2: 3", "3: 4", "4: 5"]),
        ("grams", vec!["2. 5 grams", "3. 5 grams"]),
        ("steps", vec!["1. Boil", "2. Stir", "3. Serve"]),
        ("ab_answers", vec!["A.", "B.", "C."]),
        ("vitamins", vec!["A: retinol", "B: thiamine", "C: ascorbic"]),
        ("grades", vec!["A - excellent", "B - good", "C - fair", "D - poor"]),
    ] {
        let p = env(q(mc(&texts, 0), "Which?"), 1);
        // No decision: refused, with the preview.
        let r = j(&strip_ok(p.as_bytes(), ""));
        assert_eq!(r["errors"][0]["code"], "labels_need_decision", "{name}");
        assert_eq!(r["label_findings"][0]["choices"][0]["before"], texts[0], "{name}");
        // Keep: text untouched, and (the labels are 1, 2, 3...) nothing moves.
        let r = j(&strip_ok(p.as_bytes(), r#"{"labels":[{"question":0,"choice":"keep"}]}"#));
        let out: Vec<&str> = r["payload"]["question"]["kind"]["Mc"]["answers"].as_array().unwrap().iter().map(|a| a["text"].as_str().unwrap()).collect();
        assert_eq!(out, texts, "{name}: Keep preserves the text and the order");
        // Kept labels that number the choices, or (the fourth statement) choices naming others.
        let reason = r["order_reasons"][0]["reason"].as_str().unwrap();
        assert!(reason == "labels_kept" || reason == "sibling_refs", "{name}: {reason}");
    }
    // Strip would collide ("only" x3) or empty a choice: refused, only Keep is possible.
    for texts in [vec!["I. only", "II. only", "III. only"], vec!["A.", "B.", "C."]] {
        let r = j(&strip_ok(env(q(mc(&texts, 0), "Which?"), 1).as_bytes(), r#"{"labels":[{"question":0,"choice":"strip"}]}"#));
        assert_eq!(r["errors"][0]["code"], "labels_strip_refused", "{texts:?}");
    }
    // Strip where it is clean: "1: 2" etc. become their text and are shuffled.
    let r = j(&strip_ok(env(q(mc(&["A. thaliana", "B. subtilis", "C. elegans"], 0), "Which?"), 1).as_bytes(), r#"{"labels":[{"question":0,"choice":"strip"}]}"#));
    let mut out: Vec<&str> = r["payload"]["question"]["kind"]["Mc"]["answers"].as_array().unwrap().iter().map(|a| a["text"].as_str().unwrap()).collect();
    out.sort();
    assert_eq!(out, ["elegans", "subtilis", "thaliana"], "the author chose this, after the preview");
}

#[test]
fn label_forms_that_leaked_the_order_are_found() {
    for texts in [
        vec!["A)Venus", "B)Mars", "C)Earth", "D)Jupiter"],
        vec!["A.Venus", "B.Mars", "C.Earth", "D.Jupiter"],
        vec!["\u{24B6} Venus", "\u{24B7} Mars", "\u{24B8} Earth", "\u{24B9} Jupiter"],
        vec!["\u{2460} Venus", "\u{2461} Mars", "\u{2462} Earth", "\u{2463} Jupiter"],
        vec!["Option A: Venus", "Option B: Mars", "Option C: Earth", "Option D: Jupiter"],
        vec!["\u{3B1}) Venus", "\u{3B2}) Mars", "\u{3B3}) Earth", "\u{3B4}) Jupiter"],
        vec!["\u{430}) Venus", "\u{431}) Mars", "\u{432}) Earth", "\u{433}) Jupiter"],
        vec!["\u{661}) Venus", "\u{662}) Mars", "\u{663}) Earth", "\u{664}) Jupiter"],
        vec!["A ) Venus", "B ) Mars", "C ) Earth", "D ) Jupiter"],
        vec!["Venus (A)", "Mars (B)", "Earth (C)", "Jupiter (D)"],
        vec!["B) Venus", "C) Mars", "D) Earth", "E) Jupiter"],
        vec!["A\u{3001}Venus", "B\u{3001}Mars", "C\u{3001}Earth", "D\u{3001}Jupiter"],
        vec!["A/ Venus", "B/ Mars", "C/ Earth", "D/ Jupiter"],
        vec!["i) Venus", "ii) Mars", "iii) Earth", "iv) Jupiter"],
        vec!["a) Venus", "B) Mars", "c) Earth", "D) Jupiter"],
        vec!["A) Venus", "B) Mars", "C) Earth", "None of these"],
        vec!["A)\nVenus", "B)\nMars", "C)\nEarth", "D)\nJupiter"],
        vec!["A)\tVenus", "B)\tMars", "C)\tEarth", "D)\tJupiter"],
        vec!["A)", "B)", "C) Earth", "D) Jupiter"],
    ] {
        let p = env(q(mc(&texts, 2), "Pick"), 1);
        let r = j(&strip_ok(p.as_bytes(), ""));
        assert_eq!(r["errors"][0]["code"], "labels_need_decision", "{texts:?}");
        // Keep: the order is not shuffled (the labels reveal it).
        let r = j(&strip_ok(p.as_bytes(), r#"{"labels":[{"question":0,"choice":"keep"}]}"#));
        let out: Vec<&str> = r["payload"]["question"]["kind"]["Mc"]["answers"].as_array().unwrap().iter().map(|a| a["text"].as_str().unwrap()).collect();
        assert_eq!(out, texts, "{texts:?}");
        assert_eq!(r["needs_order_ack"], json!([{"question": 0}]), "{texts:?}");
    }
}

#[test]
fn sibling_references_keep_the_order() {
    let p = env(q(mc(&["3", "4", "Both of the first two", "Both 3 and 4"], 0), "Which?"), 1);
    let _ = p;
    for texts in [vec!["Venus", "Mars", "Both A and B", "Neither"], vec!["x", "y", "z", "A and C only"], vec!["one", "two", "three", "All of the above"]] {
        let r = j(&strip_ok(env(q(mc(&texts, 0), "Which?"), 1).as_bytes(), ""));
        let out: Vec<&str> = r["payload"]["question"]["kind"]["Mc"]["answers"].as_array().unwrap_or_else(|| panic!("{r}")).iter().map(|a| a["text"].as_str().unwrap()).collect();
        assert_eq!(out, texts, "{texts:?}");
        assert_eq!(r["order_reasons"][0]["reason"], "sibling_refs");
    }
    // A locked "All of the above" keeps its slot and the rest may still be shuffled.
    let mut k = mc(&["one", "two", "three", "four", "five", "six", "All of the above"], 0);
    k["Mc"]["answers"][6]["locked"] = json!(true);
    let r = j(&strip_ok(env(q(k, "Which?"), 1).as_bytes(), ""));
    assert_eq!(r["needs_order_ack"], json!([]));
    assert_eq!(r["payload"]["question"]["kind"]["Mc"]["answers"][6]["text"], "All of the above");
}

#[test]
fn hashes_never_strip_labels_on_their_own() {
    let h = |p: &str| j(&match_hash_json(p.as_bytes(), ""))["hashes"][0].as_str().unwrap().to_string();
    assert_ne!(h(&env(q(mc(&["A. thaliana", "B. subtilis"], 0), "Which?"), 1)), h(&env(q(mc(&["thaliana", "subtilis"], 0), "Which?"), 1)));
    // Full vs stripped of the same original, for both decisions.
    let p = env(q(mc(&["A. Venus", "B. Mars", "C. Earth"], 1), "Which?"), 1);
    let v = ok(&p);
    for choice in [LabelChoice::Strip, LabelChoice::Keep] {
        let labels = vec![LabelDecision { question: 0, part: None, choice }];
        let s = strip_with_seed(v.payload.clone(), &StripOptions { image_ok: vec![], labels: labels.clone() }, seed(4)).unwrap();
        assert_eq!(match_hash_with(only_question(&v.payload), &question_labels(&labels, 0)), match_hash(only_question(&s.payload)), "{choice:?}");
    }
}

// ---- matching (critic A) ------------------------------------------------------------------------------

#[test]
fn shared_rights_never_print_a_blank_choice() {
    for (name, pairs, d) in [
        ("shared", vec![("Dog", "Mammal"), ("Cat", "Mammal"), ("Frog", "Amphibian")], vec![]),
        ("right_eq_distractor", vec![("a", "X"), ("b", "Y")], vec!["X"]),
        ("all_same", vec![("a", "X"), ("b", "X")], vec![]),
        ("basic", vec![("Fe", "Iron"), ("Cu", "Copper"), ("Au", "Gold")], vec!["Silver"]),
    ] {
        let m = json!({"Matching":{"pairs": pairs.iter().map(|(a,b)| json!({"left":a,"right":b})).collect::<Vec<_>>(), "distractors": d}});
        let v = ok(&env(q(m, "Match"), 1));
        let s = strip_with_seed(v.payload.clone(), &StripOptions::default(), seed(3)).unwrap();
        let served = ok(&s.payload.to_json());
        let local = localize(only_question(&served.payload));
        let Kind::Matching { pairs: lp, distractors: ld } = &local.kind else { panic!() };
        // No placeholder pairing in the local copy (round 4: it read as a key); the app's column
        // skips the empty rights and shows exactly the word bank.
        assert!(lp.iter().all(|p| p.right.is_empty()), "{name}: a placeholder right in the local form");
        let col = right_column(lp, ld);
        assert_eq!(&col, ld, "{name}");
        assert!(col.iter().all(|c| !c.trim().is_empty()), "{name}");
        // The local form re-validates (v2, keyless) and, re-keyed, publishes with the true key.
        assert_eq!(ok(&q_payload(&local)).payload.questions()[0], local, "{name}");
        let picks: Vec<usize> = pairs.iter().map(|(_, r)| col.iter().position(|c| c == r).unwrap()).collect();
        let keyed = rekey_matching(&local, &picks).unwrap();
        let Kind::Matching { pairs: kp, .. } = &keyed.kind else { panic!() };
        assert!(kp.iter().zip(&pairs).all(|(a, (_, r))| a.right == *r), "{name}");
        assert!(validate_for_publish(q_payload(&keyed).as_bytes(), &Limits::default(), &Structural, &PublishOptions::default()).is_ok(), "{name}");
        // Out-of-range picks are refused (index-safe).
        assert!(rekey_matching(&local, &vec![col.len(); lp.len()]).is_none());
    }
}

#[test]
fn localize_adds_no_placeholder_key() {
    // Every kind keeps its (empty) key fields as served; only the sentinel is added.
    for kind in [
        Kind::Matching { pairs: vec![Pair { left: "a".into(), right: String::new() }, Pair { left: "b".into(), right: String::new() }], distractors: vec!["Y".into(), "Z".into()] },
        Kind::Tf { correct: false },
        Kind::Mc { answers: vec![answer("x", false), answer("y", false)] },
        Kind::Blank { accept: vec![vec![]] },
    ] {
        let mut qq = question("m ___", kind.clone());
        qq.keyless = true;
        let l = localize(&qq);
        assert_eq!(l.kind, kind);
        assert!(l.keyless);
        assert_eq!(l.explanation.as_deref(), Some(KEYLESS_SENTINEL));
    }
    // A question that is not keyless is untouched.
    let qq = question("m", Kind::Tf { correct: true });
    assert_eq!(localize(&qq), qq);
}

#[test]
fn keyless_matching_rules() {
    // Whitespace-only rights (fuzz crash_71593 / crash_130974) and duplicate word-bank items.
    for (name, pairs, d, code) in [
        ("space_right", vec![("a", " "), ("b", "")], vec!["X", "Y"], "blank_text"),
        ("dup_bank", vec![("a", ""), ("b", "")], vec!["X", "X"], "duplicate_choice"),
        ("bank_repeats_right", vec![("a", "X"), ("b", "")], vec!["X", "Y"], "duplicate_choice"),
    ] {
        let mut m = q(json!({"Matching":{"pairs": pairs.iter().map(|(a,b)| json!({"left":a,"right":b})).collect::<Vec<_>>(), "distractors": d}}), "m");
        m["keyless"] = json!(true);
        let out = j(&validate_json(env(m, 2).as_bytes(), ""));
        assert!(codes_of(&out).iter().any(|(c, _)| c == code), "{name}: {out}");
    }
    // The cap on right-hand items does not depend on `keyless`.
    let pairs: Vec<Value> = (0..2).map(|i| json!({"left": format!("l{i}"), "right": ""})).collect();
    let mut m = q(json!({"Matching":{"pairs": pairs, "distractors": (0..29).map(|i| format!("d{i}")).collect::<Vec<_>>()}}), "m");
    m["keyless"] = json!(true);
    assert_eq!(j(&validate_json(env(m, 2).as_bytes(), ""))["errors"][0]["code"], "too_many");
}

// ---- keyless (critic A, critic B r1.mjs) ---------------------------------------------------------------

#[test]
fn keyless_questions_carry_no_key() {
    for (name, kind, extra) in [
        ("mc_correct", mc(&["a", "b"], 0), json!({})),
        ("tf_true", json!({"Tf":{"correct":true}}), json!({})),
        ("blank_accept", json!({"Blank":{"accept":[["x"]]}}), json!({})),
        ("short_sample", json!({"Short":{"sample":"because"}}), json!({})),
        ("mp_mc", json!({"Multipart":{"parts":[{"stem":"p","kind": mc(&["a","b"], 1)}]}}), json!({})),
        ("mp_tf", json!({"Multipart":{"parts":[{"stem":"p","kind":{"Tf":{"correct":true}}}]}}), json!({})),
        ("explanation", json!({"Tf":{"correct":false}}), json!({"explanation": "It is false because..."})),
        ("figure", json!({"Tf":{"correct":false}}), json!({"figure": {"id": "f", "params": {"answer": 3}}})),
    ] {
        let stem = if name == "blank_accept" { "x ___" } else { "s" };
        let mut m = q(kind, stem);
        m["keyless"] = json!(true);
        for (k, v) in extra.as_object().unwrap() {
            m[k] = v.clone();
        }
        let out = j(&validate_json(env(m, 2).as_bytes(), ""));
        assert!(codes_of(&out).iter().any(|(c, _)| c == "keyless_key_material"), "{name}: {out}");
    }
    // The fuzz crash cases: keyless Mc with correct flags and labels.
    for f in std::fs::read_dir(fixtures_dir().join("fuzz")).unwrap() {
        let b = std::fs::read(f.unwrap().path()).unwrap();
        if let Ok(v) = validate_bytes(&b, &Limits::default(), &Structural) {
            for q in v.payload.questions() {
                let _ = localize(q);
                let _ = rekey_matching(&localize(q), &[0, 1, 0]);
                assert_eq!(match_hash(&strip_without_shuffle(q, &QuestionLabels::new())), match_hash(q));
            }
            let labels = decide_all(&v.payload, LabelChoice::Strip);
            if let Ok(s) = strip(v.payload.clone(), &StripOptions { image_ok: vec![], labels: labels.clone() }) {
                let back = validate(&s.payload.to_json(), &Limits::default(), &Structural);
                assert!(back.is_ok(), "stripped output not re-validatable: {:?}", back.err());
                for (i, (a, q)) in v.payload.questions().iter().zip(s.payload.questions()).enumerate() {
                    assert_eq!(match_hash_with(a, &question_labels(&labels, i)), match_hash(q));
                }
            }
        }
    }
}

#[test]
fn v1_keyless_is_refused() {
    let mut m = q(json!({"Tf":{"correct":false}}), "T?");
    m["keyless"] = json!(true);
    assert_eq!(j(&validate_json(env(m.clone(), 1).as_bytes(), ""))["errors"][0]["code"], "v2_field_in_v1");
    // v2 keyless without hide_key: refused at publish; the sentinel alone too.
    assert_eq!(j(&validate_for_publish_json(env(m, 2).as_bytes(), ""))["errors"][0]["code"], "keyless_needs_hide_key");
    let mut s = q(json!({"Tf":{"correct":true}}), "T?");
    s["explanation"] = json!(KEYLESS_SENTINEL);
    assert_eq!(j(&validate_for_publish_json(env(s.clone(), 1).as_bytes(), ""))["errors"][0]["code"], "keyless_needs_hide_key");
    assert_eq!(j(&validate_for_publish_json(env(s, 1).as_bytes(), r#"{"hide_key":true}"#))["ok"], true);
}

#[test]
fn rekey_every_kind() {
    let kl = |kind: Kind, stem: &str| {
        let mut q = question(stem, kind);
        q.keyless = true;
        q.explanation = Some(KEYLESS_SENTINEL.into());
        q
    };
    let mcq = kl(Kind::Mc { answers: vec![answer("a", false), answer("b", false)] }, "s");
    let k = rekey(&mcq, &Key::Mc(vec![1])).unwrap();
    assert!(!k.keyless && k.explanation.is_none());
    assert!(matches!(&k.kind, Kind::Mc { answers } if answers[1].correct && !answers[0].correct));
    assert_eq!(rekey(&mcq, &Key::Mc(vec![])), Err("bad_key"));
    assert_eq!(rekey(&mcq, &Key::Mc(vec![5])), Err("bad_key"));
    assert_eq!(rekey(&mcq, &Key::Tf(true)), Err("bad_key"), "wrong kind");
    let tf = kl(Kind::Tf { correct: false }, "s");
    assert!(matches!(rekey(&tf, &Key::Tf(true)).unwrap().kind, Kind::Tf { correct: true }));
    let bl = kl(Kind::Blank { accept: vec![vec![], vec![]] }, "a ___ b ___");
    assert!(rekey(&bl, &Key::Blank(vec![vec!["x".into()], vec!["y".into(), "z".into()]])).is_ok());
    assert_eq!(rekey(&bl, &Key::Blank(vec![vec!["x".into()]])), Err("bad_key"));
    assert_eq!(rekey(&bl, &Key::Blank(vec![vec!["x".into()], vec![" ".into()]])), Err("bad_key"));
    let sh = kl(Kind::Short { sample: String::new() }, "s");
    assert!(matches!(rekey(&sh, &Key::Short("because".into())).unwrap().kind, Kind::Short { ref sample } if sample == "because"));
    let mp = kl(Kind::Multipart { parts: vec![Part { stem: "p".into(), kind: PartKind::Mc { answers: vec![answer("a", false), answer("b", false)] } }, Part { stem: "q".into(), kind: PartKind::Tf { correct: false } }] }, "s");
    let k = rekey(&mp, &Key::Multipart(vec![PartKey::Mc(vec![0]), PartKey::Tf(true)])).unwrap();
    assert!(validate_for_publish(q_payload(&k).as_bytes(), &Limits::default(), &Structural, &PublishOptions::default()).is_ok());
    assert_eq!(rekey(&mp, &Key::Multipart(vec![PartKey::Tf(true), PartKey::Tf(true)])), Err("bad_key"));
}

// ---- duplicates, sizes, caps (critics A and B) ---------------------------------------------------------

#[test]
fn duplicate_choices_refused_at_publish() {
    let out = j(&validate_for_publish_json(env(q(mc(&["5", "5"], 0), "Which?"), 1).as_bytes(), ""));
    assert_eq!(out["errors"][0]["code"], "duplicate_choice");
    let out = j(&validate_for_publish_json(env(q(mc(&["5", " 5 "], 0), "Which?"), 1).as_bytes(), ""));
    assert_eq!(out["errors"][0]["code"], "duplicate_choice", "compared in hash form");
}

#[test]
fn nfc_expansion_cannot_grow_past_the_body_limit() {
    // U+1D160 triples under NFC: a just-under-4 MB upload would serve 12 MB.
    let n = "\u{1D160}";
    let mut qs = vec![];
    let mut len = 200;
    for i in 0..1000 {
        let s = json!({"id": format!("q{i}"), "stem": n.repeat(682), "explanation": n.repeat(682), "kind": {"Tf": {"correct": true}}, "points": null, "difficulty": null, "created": 1, "modified": 1}).to_string();
        if len + s.len() > 4_190_000 {
            break;
        }
        len += s.len() + 1;
        qs.push(s);
    }
    let p = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","));
    let out = j(&validate_json(p.as_bytes(), ""));
    assert_eq!(out["errors"][0]["code"], "normalized_too_large", "{}", &out.to_string()[..200]);
    // Defaults written out can also grow a payload: refused rather than served too large.
    // 1,000 questions x 2 parts x 25 answers written as {"text": ...} grow by "correct" and
    // "locked" on the way out.
    let part = |p: usize| format!(r#"{{"stem":"p{p}","kind":{{"Mc":{{"answers":[{}]}}}}}}"#, (0..25).map(|k| format!(r#"{{"text":"{k}{}"}}"#, "x".repeat(42))).collect::<Vec<_>>().join(","));
    let tiny: Vec<String> = (0..1000).map(|i| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Multipart":{{"parts":[{},{}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#, part(0), part(1))).collect();
    let p = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, tiny.join(","));
    assert!(p.len() < 4 * 1024 * 1024);
    let out = j(&validate_json(p.as_bytes(), ""));
    assert_eq!(out["errors"][0]["code"], "output_too_large");
}

#[test]
fn error_and_dropped_lists_are_capped() {
    // 200 forbidden answers in one question: at most MAX_ERRORS errors.
    let ans: Vec<Value> = (0..20).map(|_| json!({"text": "\u{202E}"})).collect();
    let qs: Vec<Value> = (0..20).map(|i| json!({"id": format!("q{i}"), "stem": "s", "kind": {"Mc": {"answers": ans}}, "points": null, "difficulty": null, "created": 0, "modified": 0})).collect();
    let p = json!({"schema_version":1,"bank":{"id":"b","name":"n","created":0,"modified":0,"questions": qs}}).to_string();
    let out = j(&validate_json(p.as_bytes(), ""));
    assert_eq!(out["errors"].as_array().unwrap().len(), share_core::validate::MAX_ERRORS);
    // 400 objects with an unknown key: at most MAX_ERRORS dropped entries.
    let ans: Vec<Value> = (0..20).map(|k| json!({"text": format!("a{k}"), "zz": 0})).collect();
    let qs: Vec<Value> = (0..20).map(|i| json!({"id": format!("q{i}"), "stem": "s", "kind": {"Mc": {"answers": ans}}, "points": null, "difficulty": null, "created": 0, "modified": 0})).collect();
    let p = json!({"schema_version":1,"bank":{"id":"b","name":"n","created":0,"modified":0,"questions": qs}}).to_string();
    let out = j(&validate_json(p.as_bytes(), ""));
    assert_eq!(out["dropped"].as_array().unwrap().len(), share_core::validate::MAX_ERRORS);
    // image_ok: many out-of-range entries, capped and fast; many valid ones, fast.
    let qs: Vec<String> = (0..1000).map(|i| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Tf":{{"correct":true}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#)).collect();
    let p = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","));
    let t = std::time::Instant::now();
    let r = j(&strip_ok(p.as_bytes(), &json!({"image_ok": vec![1000; 200_000]}).to_string()));
    assert_eq!(r["errors"].as_array().unwrap().len(), share_core::validate::MAX_ERRORS);
    let r = j(&strip_ok(p.as_bytes(), &json!({"image_ok": vec![999; 200_000]}).to_string()));
    assert_eq!(r["ok"], true);
    assert!(t.elapsed().as_secs_f64() < if cfg!(debug_assertions) { 30.0 } else { 3.0 }, "{:?}", t.elapsed());
}

#[test]
fn license_url_hosts() {
    for (url, ok_) in [
        ("https://a..b/", false),
        ("https://.example.org/", false),
        ("https://-example.org/", false),
        ("https://example-.org/", false),
        ("https://example.org./", false),
        ("https://example.org:65536/", false),
        ("https://example.org:0/", false),
        ("https://example.org:65535/x", true),
        ("https://sub-1.example.org/", true),
    ] {
        assert_eq!(validate::is_web_url(url), ok_, "{url}");
    }
}

// ---- shuffle (critic B mutations M14, M18) ------------------------------------------------------------

#[test]
fn questions_in_a_bank_get_independent_shuffles() {
    let one = question("Pick", Kind::Mc { answers: (0..12).map(|i| answer(&format!("planet {i}"), i == 0)).collect() });
    let bank = QuestionBank { id: "b".into(), name: "n".into(), questions: vec![one.clone(), one], created: 0, modified: 0, details: Default::default(), origin: None, published: None };
    let v = ok(&Payload { schema_version: 1, body: Body::Bank(bank) }.to_json());
    let s = strip_with_seed(v.payload, &StripOptions::default(), seed(8)).unwrap();
    assert_ne!(s.payload.questions()[0].kind, s.payload.questions()[1].kind, "two identical questions, two permutations");
}

#[test]
fn below_rejects_the_biased_zone() {
    // For n just above 2^63 about half of all u64 values must be rejected; a missing rejection
    // shows up at once. Reference: the raw stream with the same rule.
    let n = (1usize << 63) + 1;
    let mut a = ShuffleRng::from_seed(seed(5));
    let mut raw = ShuffleRng::from_seed(seed(5));
    let threshold = (n as u64).wrapping_neg() % n as u64;
    for _ in 0..64 {
        let want = loop {
            let x = raw.next_u64();
            if x >= threshold {
                break (x % n as u64) as usize;
            }
        };
        assert_eq!(a.below(n), want);
    }
}

// ---- budgets (critic B mutations 1 and 2) ----------------------------------------------------------------

#[test]
fn figure_param_keys_and_nodes_are_charged() {
    let lim = Limits { parse_budget_bytes: 600_000, ..Default::default() };
    // 256 params whose keys are 1 KB each: about 256 KB of key bytes per figure.
    let kp = "k".repeat(1000);
    let ps: Vec<String> = (0..256).map(|k| format!(r#""{kp}{k}":0"#)).collect();
    let p = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#,
        (0..3).map(|i| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Tf":{{"correct":true}}}},"points":null,"difficulty":null,"created":1,"modified":1,"figure":{{"id":"f","params":{{{}}}}}}}"#, ps.join(","))).collect::<Vec<_>>().join(","));
    assert_eq!(validate(&p, &lim, &Structural).unwrap_err()[0].code, "too_much_memory");
    // 20,000 small values: about 0.8 MB at the per-node cost, so a 600 KB budget refuses them.
    let p = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#,
        (0..800).map(|i| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Blank":{{"accept":[{}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#, (0..20).map(|_| r#"["a"]"#).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(","));
    assert_eq!(validate(&p, &lim, &Structural).unwrap_err()[0].code, "too_much_memory");
}

// ---- the wasm-facing surface (critic A w.mjs) ------------------------------------------------------------

#[test]
fn options_and_bytes() {
    let tf = env(q(json!({"Tf":{"correct":true}}), "Q?"), 1);
    assert_eq!(j(&validate_json(tf.as_bytes(), r#"{"hide_key":"yes"}"#))["errors"][0]["code"], "bad_options");
    assert_eq!(j(&validate_json(tf.as_bytes(), r#"{"seed":[1]}"#))["errors"][0]["code"], "bad_options");
    assert_eq!(j(&validate_json(tf.replace("\"schema_version\":1", "\"schema_version\":1.0").as_bytes(), ""))["errors"][0]["code"], "expected_integer");
    assert_eq!(j(&expected_bytes())["errors"][0]["code"], "expected_bytes");
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(tf.as_bytes());
    assert_eq!(j(&validate_json(&bom, ""))["errors"][0]["code"], "invalid_json", "a BOM before the JSON is not JSON");
    assert_eq!(j(&validate_meta_json(&[0xFF], ""))["errors"][0]["code"], "invalid_utf8");
}

#[test]
fn figure_params_objects_are_counted() {
    // Per figure question: the question, kind, Tf, figure and params objects (5), plus the
    // envelope and the bank (2). With params counted, 10 questions need 52 objects.
    let p = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#,
        (0..10).map(|i| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Tf":{{"correct":true}}}},"points":null,"difficulty":null,"created":1,"modified":1,"figure":{{"id":"f","params":{{"a":1}}}}}}"#)).collect::<Vec<_>>().join(","));
    assert!(validate(&p, &Limits { max_objects: 52, ..Default::default() }, &Structural).is_ok());
    assert_eq!(validate(&p, &Limits { max_objects: 51, ..Default::default() }, &Structural).unwrap_err()[0].code, "too_many_objects");
}

#[test]
fn native_entry_points_cap_and_refuse_on_their_own() {
    // The native API (the app's import path) caps its own lists; the wasm output's truncation is
    // a second guard, not the only one.
    let ans: Vec<Value> = (0..20).map(|_| json!({"text": "\u{202E}"})).collect();
    let qs: Vec<Value> = (0..20).map(|i| json!({"id": format!("q{i}"), "stem": "s", "kind": {"Mc": {"answers": ans}}, "points": null, "difficulty": null, "created": 0, "modified": 0})).collect();
    let p = json!({"schema_version":1,"bank":{"id":"b","name":"n","created":0,"modified":0,"questions": qs}}).to_string();
    assert_eq!(validate(&p, &Limits::default(), &Structural).unwrap_err().len(), share_core::validate::MAX_ERRORS);
    let v = ok(&q_payload(&question("s", Kind::Tf { correct: true })));
    let e = strip_with_seed(v.payload.clone(), &StripOptions { image_ok: vec![5; 10_000], labels: vec![] }, seed(1)).unwrap_err();
    assert_eq!(e.len(), share_core::validate::MAX_ERRORS);
    // Strip where it is not allowed is refused by strip itself, not only by the publish check.
    let v = ok(&q_payload(&question("s", Kind::Mc { answers: vec![answer("I. only", true), answer("II. only", false), answer("III. only", false)] })));
    let e = strip_with_seed(v.payload, &StripOptions { image_ok: vec![], labels: vec![LabelDecision { question: 0, part: None, choice: LabelChoice::Strip }] }, seed(1)).unwrap_err();
    assert_eq!(e[0].code, "labels_strip_refused");
    // The importer's column skips whitespace-only items.
    let pairs = vec![Pair { left: "a".into(), right: " ".into() }, Pair { left: "b".into(), right: "X".into() }];
    assert_eq!(right_column(&pairs, &["\u{3000}".into(), "Y".into()]), ["X", "Y"]);
}
