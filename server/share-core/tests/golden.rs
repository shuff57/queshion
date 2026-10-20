//! Per-kind golden tests over the app-generated fixtures (`fixtures/model`), the stored stripped
//! goldens (`fixtures/stripped`, seed `[7; 32]`) and the stored hash vectors (`fixtures/hashes.json`).
//! Regenerate the goldens after an intended change with
//! `SHARE_CORE_FIXTURES=write cargo test --test golden`.
mod common;
use common::*;
use share_core::*;

fn write_mode() -> bool {
    std::env::var("SHARE_CORE_FIXTURES").as_deref() == Ok("write")
}

fn golden(rel: &str, text: &str) {
    let path = fixtures_dir().join(rel);
    if write_mode() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let on_disk = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{} missing (SHARE_CORE_FIXTURES=write)", path.display()));
    assert_eq!(on_disk, text, "{rel} changed");
}

fn pretty(v: &serde_json::Value) -> String {
    serde_json::to_string_pretty(v).unwrap() + "\n"
}

const SEED: [u8; 32] = [7; 32];

fn stripped(name: &str, image_ok: Vec<usize>) -> Stripped {
    let v = ok(&fixture_payload(name));
    let o = opts_for(&v.payload, image_ok);
    strip_with_seed(v.payload.clone(), &o, SEED).unwrap()
}

#[test]
fn fixtures_round_trip_unchanged_and_canonical() {
    for name in MODEL {
        let raw: serde_json::Value = serde_json::from_str(&model_json(name)).unwrap();
        let v = ok(&fixture_payload(name));
        assert!(v.dropped.is_empty(), "{name}: {:?}", v.dropped);
        let key = if name.starts_with("bank") { "bank" } else { "question" };
        assert_eq!(v.payload.to_value()[key], raw, "{name}");
        // Pretty serialisation of the wire type is byte-identical to the app's file.
        let text = match &v.payload.body {
            Body::Bank(b) => serde_json::to_string_pretty(b).unwrap(),
            Body::Question(q) => serde_json::to_string_pretty(q).unwrap(),
        } + "\n";
        assert_eq!(text, model_json(name), "{name}");
        // The canonical writer equals canonical_json of serde's tree.
        assert_eq!(v.payload.to_json(), canonical_json(&v.payload.to_value()), "{name}");
        // Validation is idempotent on its own output.
        let again = ok(&v.payload.to_json());
        assert_eq!(again.payload, v.payload, "{name}");
    }
}

#[test]
fn stripped_goldens() {
    for name in MODEL {
        let s = stripped(name, vec![]);
        golden(&format!("stripped/{name}.json"), &pretty(&s.payload.to_value()));
        // The stripped form is itself a valid (v2) payload and re-validates to itself.
        assert_eq!(s.payload.schema_version, 2);
        let again = ok(&s.payload.to_json());
        assert_eq!(again.payload, s.payload, "{name}");
        assert_eq!(s.payload.to_json(), canonical_json(&s.payload.to_value()), "{name}");
        // The importer's local form (`import::localize`), as the app will store it.
        let local = import::localize_payload(&s.payload);
        golden(&format!("local/{name}.json"), &pretty(&local.to_value()));
        let back = ok(&local.to_json());
        assert_eq!(back.payload, local, "{name}: the local form is a valid payload");
    }
}

#[test]
fn hash_vectors() {
    let mut out = serde_json::Map::new();
    for name in MODEL {
        let v = ok(&fixture_payload(name));
        let m: Vec<String> = v.payload.questions().iter().map(match_hash).collect();
        let f: Vec<String> = v.payload.questions().iter().map(full_hash).collect();
        out.insert(name.into(), serde_json::json!({ "match_hash": m, "full_hash": f }));
    }
    golden("hashes.json", &pretty(&serde_json::Value::Object(out)));
}

#[test]
fn mc_single_answer() {
    let s = stripped("question_mc", vec![]);
    let q = only_question(&s.payload);
    let Kind::Mc { answers } = &q.kind else { panic!() };
    assert!(answers.iter().all(|a| !a.correct));
    // The locked "None of these" keeps slot 3; the others are a permutation of the originals.
    assert_eq!(answers[3].text, "None of these");
    assert!(answers[3].locked);
    let mut t: Vec<&str> = answers[..3].iter().map(|a| a.text.as_str()).collect();
    t.sort();
    assert_eq!(t, ["Mars", "Mercury", "Venus"]);
    assert!(q.keyless && q.explanation.is_none() && q.figure.is_none());
    assert_eq!(q.topics, ["Planets", "Solar system"], "topics are kept (spec)");
    assert_eq!(q.points, Some(2.0));
    assert!(s.needs_order_ack.is_empty());
}

#[test]
fn mc_multiple_answer_with_labels() {
    let s = stripped("question_mc_multi", vec![]);
    let q = only_question(&s.payload);
    let Kind::Mc { answers } = &q.kind else { panic!() };
    let mut t: Vec<&str> = answers.iter().map(|a| a.text.as_str()).collect();
    t.sort();
    assert_eq!(t, ["Earth", "Jupiter", "Mars", "Saturn"], "labels removed");
    assert!(answers.iter().all(|a| !a.correct));
    assert_eq!(s.labels_removed, [Pos { question: 0, part: None }]);
}

#[test]
fn tf() {
    let s = stripped("question_tf", vec![]);
    let q = only_question(&s.payload);
    assert_eq!(q.kind, Kind::Tf { correct: false });
    assert!(q.keyless && q.explanation.is_none());
}

#[test]
fn matching_word_bank() {
    let s = stripped("question_matching", vec![]);
    let q = only_question(&s.payload);
    let Kind::Matching { pairs, distractors } = &q.kind else { panic!() };
    let lefts: Vec<&str> = pairs.iter().map(|p| p.left.as_str()).collect();
    assert_eq!(lefts, ["Mitochondrion", "Ribosome", "Chloroplast"], "left order kept");
    assert!(pairs.iter().all(|p| p.right.is_empty()));
    let mut bank: Vec<&str> = distractors.iter().map(|s| s.as_str()).collect();
    bank.sort();
    // "Makes ATP" was the right item of two pairs: it appears once (Moodle de-duplicates choices too).
    assert_eq!(bank, ["Builds proteins", "Digests waste", "Makes ATP", "Stores DNA"]);
}

#[test]
fn blank_keeps_count() {
    let s = stripped("question_blank", vec![]);
    let q = only_question(&s.payload);
    assert_eq!(q.kind, Kind::Blank { accept: vec![vec![], vec![]] });
}

#[test]
fn short_sample_cleared() {
    let s = stripped("question_short", vec![]);
    assert_eq!(only_question(&s.payload).kind, Kind::Short { sample: String::new() });
}

#[test]
fn multipart_parts() {
    let s = stripped("question_multipart", vec![]);
    let q = only_question(&s.payload);
    let Kind::Multipart { parts } = &q.kind else { panic!() };
    let PartKind::Mc { answers } = &parts[0].kind else { panic!() };
    assert!(answers.iter().all(|a| !a.correct));
    assert_eq!(parts[1].kind, PartKind::Tf { correct: false });
    assert_eq!(parts[2].kind, PartKind::Short { sample: String::new() });
    assert_eq!(parts[0].stem, "At the top, its velocity is:");
}

#[test]
fn figure_always_removed_image_only_when_ticked() {
    let s = stripped("question_figure_image", vec![]);
    let q = only_question(&s.payload);
    assert!(q.figure.is_none() && q.image.is_none());
    assert_eq!((s.figures_removed.as_slice(), s.images_removed.as_slice()), (&[0][..], &[0][..]));
    let s = stripped("question_figure_image", vec![0]);
    let q = only_question(&s.payload);
    assert!(q.figure.is_none(), "the tick keeps the rendered image only, never the figure spec");
    assert!(q.image.is_some());
    assert_eq!(s.images_kept, [0]);
}

#[test]
fn bank_strips_every_question_and_keeps_details() {
    let s = stripped("bank_all_kinds", vec![7]);
    let Body::Bank(b) = &s.payload.body else { panic!() };
    assert_eq!(b.questions.len(), 8);
    assert!(b.questions.iter().all(|q| q.keyless && q.explanation.is_none() && q.figure.is_none()));
    assert!(b.questions[7].image.is_some(), "position 7 was ticked");
    assert_eq!(b.details.author, "A. Teacher");
    let json = s.payload.to_json();
    assert!(!json.contains("\"correct\":true"));
    for secret in ["Mercury orbits", "At 1 atm.", "Rayleigh", "mitochondria", "Gravity still acts", "Read the three digits", "micropipette-reading", "15.7 µL"] {
        assert!(!json.contains(secret), "{secret} leaked");
    }
}
