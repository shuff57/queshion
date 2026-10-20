//! `match_hash` and `full_hash`: what they see and what they do not.
mod common;
use common::*;
use share_core::*;

fn mc(texts: &[(&str, bool)]) -> Question {
    question("Which?", Kind::Mc { answers: texts.iter().map(|(t, c)| answer(t, *c)).collect() })
}

#[test]
fn differing_only_in_the_key_gives_the_same_match_hash() {
    // Mc: a different correct choice, and single vs multiple answer.
    let a = mc(&[("Venus", false), ("Mercury", true), ("Mars", false)]);
    let b = mc(&[("Venus", true), ("Mercury", false), ("Mars", false)]);
    let c = mc(&[("Venus", true), ("Mercury", true), ("Mars", true)]);
    assert_eq!(match_hash(&a), match_hash(&b));
    assert_eq!(match_hash(&a), match_hash(&c));
    assert_ne!(full_hash(&a), full_hash(&b), "full_hash is for integrity and does see the key");
    // Tf: true vs false.
    let t = question("Sky is blue", Kind::Tf { correct: true });
    let f = question("Sky is blue", Kind::Tf { correct: false });
    assert_eq!(match_hash(&t), match_hash(&f));
    assert_ne!(full_hash(&t), full_hash(&f));
    // Matching: a different pairing.
    let m = |r: [&str; 2]| question("Match", Kind::Matching {
        pairs: vec![Pair { left: "a".into(), right: r[0].into() }, Pair { left: "b".into(), right: r[1].into() }],
        distractors: vec!["z".into()],
    });
    assert_eq!(match_hash(&m(["x", "y"])), match_hash(&m(["y", "x"])));
    // Blank and Short: different accepted answers / samples.
    let bl = |v: &str| question("The ___", Kind::Blank { accept: vec![vec![v.into()]] });
    assert_eq!(match_hash(&bl("cat")), match_hash(&bl("dog")));
    let sh = |v: &str| question("Why?", Kind::Short { sample: v.into() });
    assert_eq!(match_hash(&sh("because")), match_hash(&sh("")));
    // Multipart: every part's key.
    let mp = |c: bool| question("Ball", Kind::Multipart { parts: vec![
        Part { stem: "p".into(), kind: PartKind::Tf { correct: c } },
        Part { stem: "q".into(), kind: PartKind::Mc { answers: vec![answer("x", c), answer("y", !c)] } },
        Part { stem: "r".into(), kind: PartKind::Short { sample: if c { "a".into() } else { "b".into() } } },
    ] });
    assert_eq!(match_hash(&mp(true)), match_hash(&mp(false)));
}

#[test]
fn full_and_stripped_forms_match_for_every_fixture_and_seed() {
    for name in MODEL {
        let v = ok(&fixture_payload(name));
        for s in 0..20 {
            let image_ok = (0..v.payload.questions().len()).filter(|i| i % 2 == 0).collect();
            let o = opts_for(&v.payload, image_ok);
            let st = strip_with_seed(v.payload.clone(), &o, seed(s)).unwrap();
            for (i, (a, b)) in v.payload.questions().iter().zip(st.payload.questions()).enumerate() {
                assert_eq!(match_hash_with(a, &question_labels(&o.labels, i)), match_hash(b), "{name} seed {s}");
            }
        }
    }
}

#[test]
fn ignores_order_ids_times_and_presentation() {
    let a = mc(&[("Venus", false), ("Mercury", true), ("Mars", false)]);
    let mut b = mc(&[("Mars", false), ("Venus", true), ("Mercury", false)]);
    b.id = "other".into();
    b.created = 99;
    b.modified = 100;
    b.points = Some(5.0);
    b.difficulty = Some(Difficulty::High);
    b.topics = vec!["t".into()];
    b.explanation = Some("e".into());
    b.image = Some(ImageRef { file: format!("media/{}.png", "a".repeat(64)), width: 1, height: 1 });
    b.figure = Some(FigureSpec { id: "f".into(), params: Default::default() });
    b.keyless = true;
    if let Kind::Mc { answers } = &mut b.kind {
        answers[0].locked = true;
    }
    assert_eq!(match_hash(&a), match_hash(&b));
}

#[test]
fn labels_unicode_and_whitespace_are_normalised() {
    let none = QuestionLabels::new();
    let strip_ = |_q: &Question| -> QuestionLabels { [(None, LabelChoice::Strip)].into_iter().collect() };
    let keep: QuestionLabels = [(None, LabelChoice::Keep)].into_iter().collect();
    let plain = mc(&[("Venus", false), ("Mercury", true)]);
    let labelled = mc(&[("A. Venus", false), ("B) Mercury", true)]);
    // match_hash never removes labels: labelled and plain are different questions...
    assert_ne!(match_hash(&plain), match_hash(&labelled));
    // ...unless the author strips them, and then the served text really is the plain one.
    assert_eq!(match_hash_with(&labelled, &strip_(&labelled)), match_hash(&plain));
    let st = strip_question(&labelled, false, &strip_(&labelled), &mut ShuffleRng::from_seed(seed(1))).unwrap();
    assert_eq!(match_hash(&st), match_hash(&plain));
    let st = strip_question(&labelled, false, &keep, &mut ShuffleRng::from_seed(seed(1))).unwrap();
    assert_eq!(match_hash(&st), match_hash_with(&labelled, &keep));
    assert_eq!(match_hash(&st), match_hash(&labelled));
    // No decision: refused.
    assert_eq!(strip_question(&labelled, false, &none, &mut ShuffleRng::from_seed(seed(1))).unwrap_err()[0].code, "labels_need_decision");
    // Two consistent layers are both removed by Strip.
    let double = mc(&[("A. (i) Venus", false), ("B. (ii) Mercury", true)]);
    assert_eq!(match_hash_with(&double, &strip_(&double)), match_hash(&plain));
    // Real text: "A. thaliana / B. subtilis" is not "thaliana / subtilis".
    let ath = mc(&[("A. thaliana", true), ("B. subtilis", false)]);
    assert_ne!(match_hash(&ath), match_hash(&mc(&[("thaliana", true), ("subtilis", false)])));
    let ecoli = mc(&[("E. coli", true), ("S. aureus", false), ("B. subtilis", false)]);
    let kcoli = mc(&[("K. coli", true), ("T. aureus", false), ("Q. subtilis", false)]);
    assert_ne!(match_hash(&ecoli), match_hash(&kcoli));
    let st = strip_question(&ecoli, false, &keep, &mut ShuffleRng::from_seed(seed(5))).unwrap();
    assert_eq!(match_hash(&st), match_hash(&ecoli));
    // Every enumerator style, full-width included, with Strip.
    for labels in [["1. a", "2. b"], ["(a) a", "(b) b"], ["i. a", "ii. b"], ["\u{FF21}\u{FF0E}a", "\u{FF22}\u{FF0E}b"], ["A- a", "B- b"], ["A)a", "B)b"], ["\u{24B6} a", "\u{24B7} b"], ["a (A)", "b (B)"]] {
        let q = mc(&[(labels[0], true), (labels[1], false)]);
        assert_eq!(match_hash_with(&q, &strip_(&q)), match_hash(&mc(&[("a", false), ("b", true)])), "{labels:?}");
        let st = strip_question(&q, false, &strip_(&q), &mut ShuffleRng::from_seed(seed(6))).unwrap();
        assert_eq!(match_hash(&st), match_hash_with(&q, &strip_(&q)), "{labels:?}");
    }
    // NFC: composed and decomposed é; whitespace runs.
    let comp = question("Caf\u{e9}  au\nlait ", Kind::Tf { correct: true });
    let decomp = question(" Cafe\u{301} au lait", Kind::Tf { correct: true });
    assert_eq!(match_hash(&comp), match_hash(&question("Caf\u{e9} au\nlait", Kind::Tf { correct: true })));
    assert_ne!(match_hash(&comp), match_hash(&decomp), "line breaks are structure");
    let table = question("a | b\nc | d", Kind::Tf { correct: true });
    let line = question("a | b c | d", Kind::Tf { correct: true });
    assert_ne!(match_hash(&table), match_hash(&line));
    let spaced = question("  a  |  b \n c | d ", Kind::Tf { correct: true });
    assert_eq!(match_hash(&table), match_hash(&spaced));
}

#[test]
fn sees_what_a_student_sees() {
    let base = mc(&[("Venus", false), ("Mercury", true)]);
    let mut stem = base.clone();
    stem.stem = "Which one?".into();
    assert_ne!(match_hash(&base), match_hash(&stem));
    let choices = mc(&[("Venus", false), ("Pluto", true)]);
    assert_ne!(match_hash(&base), match_hash(&choices));
    let extra = mc(&[("Venus", false), ("Mercury", true), ("Mars", false)]);
    assert_ne!(match_hash(&base), match_hash(&extra));
    let tf = question("Which?", Kind::Tf { correct: true });
    assert_ne!(match_hash(&tf), match_hash(&question("Which?", Kind::Short { sample: String::new() })), "kind is in it");
    let b1 = question("The ___", Kind::Blank { accept: vec![vec![]] });
    let b2 = question("The ___", Kind::Blank { accept: vec![vec![], vec![]] });
    assert_ne!(match_hash(&b1), match_hash(&b2), "blank count is in it");
    // Multipart part order is structure.
    let p = |a: &str, b: &str| question("M", Kind::Multipart { parts: vec![
        Part { stem: a.into(), kind: PartKind::Tf { correct: true } },
        Part { stem: b.into(), kind: PartKind::Tf { correct: true } },
    ] });
    assert_ne!(match_hash(&p("x", "y")), match_hash(&p("y", "x")));
}

#[test]
fn matching_word_bank_is_a_set() {
    let full = question("Match", Kind::Matching {
        pairs: vec![Pair { left: "a".into(), right: "same".into() }, Pair { left: "b".into(), right: "same".into() }],
        distractors: vec!["same".into(), "other".into()],
    });
    let stripped = strip_question(&full, false, &QuestionLabels::new(), &mut ShuffleRng::from_seed(seed(4))).unwrap();
    let Kind::Matching { distractors, .. } = &stripped.kind else { panic!() };
    assert_eq!(distractors.len(), 2);
    assert_eq!(match_hash(&full), match_hash(&stripped));
}

#[test]
fn skeleton_documents_the_covered_fields() {
    let q = mc(&[("A. x", true), ("B. y", false)]);
    let s = canonical_json(&match_skeleton(&q));
    assert_eq!(s, r#"{"choices":["A. x","B. y"],"kind":"Mc","stem":"Which?"}"#, "labels are kept unless stripped");
    let m = question("M", Kind::Matching { pairs: vec![Pair { left: "l".into(), right: "r".into() }], distractors: vec!["d".into()] });
    assert_eq!(canonical_json(&match_skeleton(&m)), r#"{"bank":["d","r"],"kind":"Matching","left":["l"],"stem":"M"}"#);
    let b = question("B", Kind::Blank { accept: vec![vec!["k".into()]; 3] });
    assert_eq!(canonical_json(&match_skeleton(&b)), r#"{"blanks":3,"kind":"Blank","stem":"B"}"#);
}

#[test]
fn canonical_json_is_sorted_and_compact() {
    let v = serde_json::json!({"b": [1, {"z": true, "a": null}], "a": "é\"\n"});
    assert_eq!(canonical_json(&v), "{\"a\":\"é\\\"\\n\",\"b\":[1,{\"a\":null,\"z\":true}]}");
}
