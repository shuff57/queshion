//! The shared-pool wire format against the app's own model (docs/shared-pool.md, "One share-core crate").
//!
//! `server/share-core/fixtures/model/*.json` are written from `vershion::model` values by this test
//! and are what share-core's own tests read. This test fails if the app's serialisation of those
//! values no longer matches the files, and checks share-core both ways: everything share-core
//! accepts and writes (full and stripped) deserialises through the app's types, and share-core
//! writes the app's JSON byte for byte for schema v1.
//!
//! Regenerate the fixtures after an intended model change (then re-run share-core's tests, which
//! regenerate their stripped goldens the same way):
//!
//! ```sh
//! SHARE_CORE_FIXTURES=write cargo test --test share_core_fixtures
//! (cd server/share-core && SHARE_CORE_FIXTURES=write cargo test --test golden)
//! ```
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;
use std::path::PathBuf;

use vershion::figures::{self, Kind as FKind, Value as FValue};
use vershion::model::*;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("server/share-core/fixtures/model")
}

const HASH: &str = "3f2a9c0b1d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8";

fn q(id: &str, stem: &str, kind: Kind) -> Question {
    Question {
        id: id.into(),
        stem: stem.into(),
        kind,
        points: Some(1.0),
        difficulty: None,
        topics: Vec::new(),
        image: None,
        explanation: None,
        figure: None,
        created: 1_760_000_000_000,
        modified: 1_760_000_100_000,
    }
}

fn ans(text: &str, correct: bool) -> Answer {
    Answer { text: text.into(), correct, locked: false }
}

/// A figure spec exactly as the figure editor writes one: the registry's defaults through `spec_for`.
fn pipette_spec() -> FigureSpec {
    let reg = figures::registry();
    let def = reg.iter().find(|d| d.id == "micropipette-reading").expect("micropipette figure registered");
    let mut p = figures::Params::defaults(&(def.params)());
    p.set_f("volume", 15.7);
    figures::spec::spec_for(def.id, &p)
}

/// (name, value, the app's own pretty JSON of it in struct field order).
fn fixtures() -> Vec<(&'static str, serde_json::Value, String)> {
    let mut mc = q("q00101a2b3c", "Which planet is closest to the Sun?\n\n**Hint:** think *small*.", Kind::Mc {
        answers: vec![ans("Venus", false), ans("Mercury", true), ans("Mars", false), Answer { text: "None of these".into(), correct: false, locked: true }],
    });
    mc.points = Some(2.0);
    mc.difficulty = Some(Difficulty::Medium);
    mc.topics = vec!["Planets".into(), "Solar system".into()];
    mc.explanation = Some("Mercury orbits at about 0.39 AU.".into());

    let mut ma = q("q00201a2b3c", "Select every gas giant.", Kind::Mc {
        answers: vec![ans("A. Jupiter", true), ans("B. Earth", false), ans("C. Saturn", true), ans("D. Mars", false)],
    });
    ma.difficulty = Some(Difficulty::High);

    let mut tf = q("q00301a2b3c", "Water boils at 100 °C at sea level.", Kind::Tf { correct: true });
    tf.points = None;
    tf.explanation = Some("At 1 atm.".into());

    let mut matching = q("q00401a2b3c", "Match each organelle to its job.", Kind::Matching {
        pairs: vec![
            Pair { left: "Mitochondrion".into(), right: "Makes ATP".into() },
            Pair { left: "Ribosome".into(), right: "Builds proteins".into() },
            Pair { left: "Chloroplast".into(), right: "Makes ATP".into() },
        ],
        distractors: vec!["Stores DNA".into(), "Digests waste".into()],
    });
    matching.difficulty = Some(Difficulty::Low);

    let blank = q("q00501a2b3c", "The powerhouse of the cell is the ___; it makes ___.", Kind::Blank {
        accept: vec![vec!["mitochondrion".into(), "mitochondria".into()], vec!["ATP".into()]],
    });

    let mut short = q("q00601a2b3c", "Explain why the sky is blue.", Kind::Short { sample: "Rayleigh scattering of shorter wavelengths.".into() });
    short.topics = vec!["Optics".into()];

    let multipart = q("q00701a2b3c", "A ball is thrown straight up.", Kind::Multipart {
        parts: vec![
            Part { stem: "At the top, its velocity is:".into(), kind: PartKind::Mc { answers: vec![ans("zero", true), ans("maximum", false), ans("negative", false)] } },
            Part { stem: "Its acceleration at the top is zero.".into(), kind: PartKind::Tf { correct: false } },
            Part { stem: "Why?".into(), kind: PartKind::Short { sample: "Gravity still acts: a = -g.".into() } },
        ],
    });

    let mut figure = q("q00801a2b3c", "What volume does the micropipette read?", Kind::Short { sample: "15.7 µL".into() });
    figure.image = Some(ImageRef { file: format!("media/{HASH}.png"), width: 640, height: 480 });
    figure.figure = Some(pipette_spec());
    figure.explanation = Some("Read the three digits on the display.".into());

    // Two lefts share a right item, and a distractor repeats it.
    let shared = q("q00901a2b3c", "Match each animal to its class.", Kind::Matching {
        pairs: vec![
            Pair { left: "Dog".into(), right: "Mammal".into() },
            Pair { left: "Cat".into(), right: "Mammal".into() },
            Pair { left: "Frog".into(), right: "Amphibian".into() },
        ],
        distractors: vec!["Mammal".into(), "Bird".into()],
    });

    let questions = vec![mc.clone(), ma.clone(), tf.clone(), matching.clone(), blank.clone(), short.clone(), multipart.clone(), figure.clone()];
    let bank = QuestionBank {
        id: "b00101a2b3c".into(),
        name: "Unit 3: Cells and Space".into(),
        questions,
        created: 1_760_000_000_000,
        modified: 1_760_000_200_000,
        details: BankDetails {
            description: "Mixed review.\nAll kinds.".into(),
            author: "A. Teacher".into(),
            license_name: "CC BY 4.0".into(),
            license_url: "https://creativecommons.org/licenses/by/4.0/".into(),
        },
    };
    let plain = QuestionBank { id: "b00201a2b3c".into(), name: "Old bank".into(), questions: vec![tf.clone()], created: 1, modified: 2, details: BankDetails::default() };

    let v = |x: &dyn ToJson| (x.json(), x.pretty());
    vec![
        { let (j, t) = v(&mc); ("question_mc", j, t) },
        { let (j, t) = v(&ma); ("question_mc_multi", j, t) },
        { let (j, t) = v(&tf); ("question_tf", j, t) },
        { let (j, t) = v(&matching); ("question_matching", j, t) },
        { let (j, t) = v(&blank); ("question_blank", j, t) },
        { let (j, t) = v(&short); ("question_short", j, t) },
        { let (j, t) = v(&multipart); ("question_multipart", j, t) },
        { let (j, t) = v(&figure); ("question_figure_image", j, t) },
        { let (j, t) = v(&shared); ("question_matching_shared", j, t) },
        { let (j, t) = v(&bank); ("bank_all_kinds", j, t) },
        { let (j, t) = v(&plain); ("bank_no_details", j, t) },
    ]
}

trait ToJson {
    fn json(&self) -> serde_json::Value;
    fn pretty(&self) -> String;
}
impl<T: serde::Serialize> ToJson for T {
    fn json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
    fn pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap() + "\n"
    }
}

fn envelope(name: &str, inner: &serde_json::Value) -> String {
    let key = if name.starts_with("bank") { "bank" } else { "question" };
    serde_json::json!({ "schema_version": 1, key: inner }).to_string()
}

#[test]
fn the_app_and_share_core_agree_on_the_keyless_sentinel() {
    assert_eq!(KEYLESS_SENTINEL, share_core::KEYLESS_SENTINEL);
}

#[test]
fn model_fixtures_match_the_app_serialisation() {
    let write = std::env::var("SHARE_CORE_FIXTURES").as_deref() == Ok("write");
    std::fs::create_dir_all(dir()).unwrap();
    for (name, _, text) in fixtures() {
        let path = dir().join(format!("{name}.json"));
        if write {
            std::fs::write(&path, &text).unwrap();
            continue;
        }
        let on_disk = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{} missing; regenerate (see module docs)", path.display()));
        assert_eq!(on_disk, text, "{name}: the app's JSON changed; regenerate the fixtures if intended");
    }
}

#[test]
fn fixtures_deserialise_through_the_app_types_unchanged() {
    for (name, value, _) in fixtures() {
        let text = serde_json::to_string(&value).unwrap();
        let back = if name.starts_with("bank") {
            serde_json::to_value(serde_json::from_str::<QuestionBank>(&text).unwrap()).unwrap()
        } else {
            serde_json::to_value(serde_json::from_str::<Question>(&text).unwrap()).unwrap()
        };
        assert_eq!(back, value, "{name}");
    }
}

/// share-core's figure registry input built from the real `figures::registry()`.
fn declared() -> share_core::Declared {
    let mut m = BTreeMap::new();
    for def in figures::registry() {
        let specs = (def.params)()
            .into_iter()
            .map(|d| {
                let default_s = match &d.default {
                    FValue::S(s) => s.clone(),
                    _ => String::new(),
                };
                let kind = match d.kind {
                    FKind::Float { min, max, .. } => share_core::ParamKind::Float { min, max },
                    FKind::Int { min, max } => share_core::ParamKind::Int { min, max },
                    FKind::Bool => share_core::ParamKind::Bool,
                    FKind::Choice(o) => share_core::ParamKind::Choice { options: o.iter().map(|s| s.to_string()).collect(), default: default_s },
                    FKind::Text => share_core::ParamKind::Text,
                    FKind::Long | FKind::List | FKind::Data { .. } => share_core::ParamKind::Long,
                    FKind::Color(p) => share_core::ParamKind::Color { palette: p.iter().map(|s| s.to_string()).collect(), default: default_s },
                };
                share_core::ParamSpec { key: d.key.to_string(), kind }
            })
            .collect();
        m.insert(def.id.to_string(), specs);
    }
    share_core::Declared(m)
}

#[test]
fn every_registry_id_and_param_key_has_the_shape_share_core_accepts() {
    for def in figures::registry() {
        assert!(share_core::figures::is_figure_id(def.id), "figure id {:?}", def.id);
        for d in (def.params)() {
            assert!(share_core::figures::is_param_key(d.key), "{}: param key {:?}", def.id, d.key);
        }
    }
}

#[test]
fn every_registry_default_spec_survives_validation_unchanged() {
    // A spec made of each figure's defaults is what the editor saves for an untouched figure; with
    // the real registry share-core must keep it exactly (nothing dropped, nothing clamped).
    let reg = declared();
    for def in figures::registry() {
        let p = figures::Params::defaults(&(def.params)());
        let mut question = q("qfig", "Figure", Kind::Short { sample: String::new() });
        question.figure = Some(figures::spec::spec_for(def.id, &p));
        let qv = serde_json::to_value(&question).unwrap();
        let env = envelope("question", &qv);
        let limits = share_core::Limits::default();
        let v = share_core::validate(&env, &limits, &reg).unwrap_or_else(|e| panic!("{}: {e:?}", def.id));
        let share_core::Body::Question(out) = &v.payload.body else { panic!() };
        let fig = out.figure.as_ref().expect("figure kept");
        let orig = question.figure.as_ref().unwrap();
        assert_eq!(fig.id, orig.id);
        assert_eq!(fig.params, orig.params, "{}: params changed: dropped {:?}", def.id, v.dropped);
    }
}

#[test]
fn registry_clamps_like_params_normalize_and_drops_unknown() {
    let reg = declared();
    let mut question = q("qfig", "Figure", Kind::Short { sample: String::new() });
    let mut spec = pipette_spec();
    spec.params.insert("volume".into(), serde_json::json!(1e9));
    spec.params.insert("size".into(), serde_json::json!("not a size"));
    spec.params.insert("evil".into(), serde_json::json!("x"));
    question.figure = Some(spec.clone());
    let env = envelope("question", &serde_json::to_value(&question).unwrap());
    let v = share_core::validate(&env, &share_core::Limits::default(), &reg).unwrap();
    let share_core::Body::Question(out) = &v.payload.body else { panic!() };
    let got = &out.figure.as_ref().unwrap().params;
    assert!(!got.contains_key("evil"));

    // The app's own reading of the same spec, after normalize: share-core must agree.
    let def = figures::registry().into_iter().find(|d| d.id == "micropipette-reading").unwrap();
    let app = figures::spec::params_from_spec(&def, &FigureSpec { id: spec.id.clone(), params: got.clone() });
    let app_from_raw = figures::spec::params_from_spec(&def, &spec);
    assert_eq!(app.0, app_from_raw.0, "share-core's clamped params read back as the app's normalize does");

    // An unknown figure id is dropped with the real registry.
    let mut unknown = question.clone();
    unknown.figure = Some(FigureSpec { id: "no-such-figure".into(), params: BTreeMap::new() });
    let env = envelope("question", &serde_json::to_value(&unknown).unwrap());
    let v = share_core::validate(&env, &share_core::Limits::default(), &reg).unwrap();
    let share_core::Body::Question(out) = &v.payload.body else { panic!() };
    assert!(out.figure.is_none());
}

#[test]
fn share_core_writes_the_app_json_and_the_app_reads_share_core_output() {
    let mut seed = [0u8; 32];
    seed[0] = 42;
    for (name, value, pretty) in fixtures() {
        let env = envelope(name, &value);
        let v = share_core::validate(&env, &share_core::Limits::default(), &declared()).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let key = if name.starts_with("bank") { "bank" } else { "question" };
        // v1, unchanged input: share-core writes exactly the app's JSON.
        let written = match &v.payload.body {
            share_core::Body::Bank(b) => serde_json::to_string_pretty(b).unwrap(),
            share_core::Body::Question(q) => serde_json::to_string_pretty(q).unwrap(),
        } + "\n";
        assert_eq!(written, pretty, "{name}: share-core's JSON differs from the app's, byte for byte");
        assert_eq!(v.payload.to_value()[key], value, "{name}");

        // Stripped (v2) output loads in the app; the app drops `keyless` (a v2 field it does not
        // know yet) and nothing else.
        // The author's label decisions: Strip where allowed.
        let labels = share_core::label_findings(&v.payload)
            .into_iter()
            .map(|f| share_core::LabelDecision { question: f.question, part: f.part, choice: if f.strip_allowed { share_core::LabelChoice::Strip } else { share_core::LabelChoice::Keep } })
            .collect();
        let s = share_core::strip_with_seed(v.payload.clone(), &share_core::StripOptions { image_ok: vec![], labels }, seed).unwrap();
        assert_eq!(s.payload.schema_version, 2);
        let mut stripped = s.payload.to_value()[key].clone();
        let app_back = if key == "bank" {
            serde_json::to_value(serde_json::from_value::<QuestionBank>(stripped.clone()).unwrap()).unwrap()
        } else {
            serde_json::to_value(serde_json::from_value::<Question>(stripped.clone()).unwrap()).unwrap()
        };
        let remove_keyless = |q: &mut serde_json::Value| {
            assert_eq!(q["keyless"], serde_json::json!(true));
            q.as_object_mut().unwrap().remove("keyless");
        };
        if key == "bank" {
            stripped["questions"].as_array_mut().unwrap().iter_mut().for_each(remove_keyless);
        } else {
            remove_keyless(&mut stripped);
        }
        assert_eq!(app_back, stripped, "{name}: the app lost something other than keyless");
    }
}

/// The import contract (`share_core::import`): the local form of every stripped fixture loads
/// through the app's types, and the app's right column for a matching question (built as
/// `src/examui.rs` builds it: `pairs[].right` then `distractors`) has no empty choice and no
/// duplicate, so neither this build nor an older one prints a broken sheet.
#[test]
fn imported_keyless_copies_load_and_print_cleanly_in_the_app() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("server/share-core/fixtures/local");
    for (name, _, _) in fixtures() {
        let text = std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let qs: Vec<Question> = if name.starts_with("bank") {
            serde_json::from_value::<QuestionBank>(v["bank"].clone()).unwrap().questions
        } else {
            vec![serde_json::from_value::<Question>(v["question"].clone()).unwrap()]
        };
        for q in qs {
            assert!(q.explanation.as_deref().is_some_and(|e| e.starts_with("No answer key")), "{name}: the sentinel prints on older builds");
            assert!(is_keyless(&q), "{name}: the app sees it as keyless (no key prints)");
            if let Kind::Matching { pairs, distractors } = &q.kind {
                // No placeholder pairing (it would read as a key); the app's own column (shared
                // by sheet and key) skips the empty rights and is exactly share-core's.
                assert!(pairs.iter().all(|p| p.right.is_empty()), "{name}: a placeholder right side");
                let column = right_column(pairs, distractors);
                assert!(column.iter().all(|r| !r.trim().is_empty()), "{name}: empty choice");
                let set: std::collections::BTreeSet<&String> = column.iter().collect();
                assert_eq!(set.len(), column.len(), "{name}: duplicate choice");
                let sc: Vec<share_core::Pair> = pairs.iter().map(|p| share_core::Pair { left: p.left.clone(), right: p.right.clone() }).collect();
                assert_eq!(column, share_core::import::right_column(&sc, distractors), "{name}");
            }
            if let Kind::Mc { answers } = &q.kind {
                assert!(answers.iter().all(|a| !a.correct));
            }
        }
    }
}
