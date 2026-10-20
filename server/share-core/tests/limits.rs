//! Boundary tests for every limit, the character rules on every text field, the strict reader,
//! unknown-field dropping, schema versions, and that errors never echo input.
mod common;
use common::*;
use share_core::*;

fn mc(n: usize) -> Question {
    question("s", Kind::Mc { answers: (0..n).map(|i| answer(&format!("a{i}"), i == 0)).collect() })
}

fn pair2() -> Pair {
    Pair { left: "l2".into(), right: "r2".into() }
}

fn has(json: &str, code: &str, path: &str) {
    let c = codes(json);
    assert!(c.iter().any(|(k, p)| *k == code && p == path), "want {code} at {path}, got {c:?}");
}

#[test]
fn stem_and_explanation_bytes() {
    let at = |n: usize| question(&"é".repeat(n / 2), Kind::Tf { correct: true });
    ok(&q_payload(&at(8192)));
    has(&q_payload(&at(8194)), "too_long", "question.stem");
    let mut q = at(10);
    q.explanation = Some("x".repeat(8192));
    ok(&q_payload(&q));
    q.explanation = Some("x".repeat(8193));
    has(&q_payload(&q), "too_long", "question.explanation");
}

#[test]
fn answer_count_and_text_bytes() {
    ok(&q_payload(&mc(26)));
    has(&q_payload(&mc(27)), "too_many", "question.kind.Mc.answers");
    let q = question("s", Kind::Mc { answers: vec![answer(&"x".repeat(2048), true), answer("b", false)] });
    ok(&q_payload(&q));
    let q = question("s", Kind::Mc { answers: vec![answer(&"x".repeat(2049), true), answer("b", false)] });
    has(&q_payload(&q), "too_long", "question.kind.Mc.answers[0].text");
}

#[test]
fn matching_pairs_and_distractors() {
    let m = |np: usize, nd: usize, keyless: bool| {
        let mut q = question("s", Kind::Matching {
            pairs: (0..np).map(|i| Pair { left: format!("l{i}"), right: if keyless { String::new() } else { format!("r{i}") } }).collect(),
            distractors: (0..nd).map(|i| format!("d{i}")).collect(),
        });
        q.keyless = keyless;
        q_payload(&q)
    };
    ok(&m(50, 26, false));
    has(&m(51, 0, false), "too_many", "question.kind.Matching.pairs");
    // The distinct right-hand items (pair rights + distractors) are capped at pairs + 26: the
    // stripped word bank is that union, so it always revalidates.
    ok(&m(2, 26, false));
    has(&m(2, 27, false), "too_many", "question.kind.Matching.distractors");
    has(&m(2, 28, false), "too_many", "question.kind.Matching.distractors");
    has(&m(1, 0, false), "too_few", "question.kind.Matching.pairs");
    // A served (keyless) word bank: empty rights, the whole bank in distractors.
    ok(&m(50, 76, true));
    has(&m(50, 77, true), "too_many", "question.kind.Matching.distractors");
    let q = question("s", Kind::Matching { pairs: vec![Pair { left: "l".into(), right: "r".repeat(2049) }, pair2()], distractors: vec![] });
    has(&q_payload(&q), "too_long", "question.kind.Matching.pairs[0].right");
}

#[test]
fn blanks_and_variants() {
    let b = |nb: usize, nv: usize| q_payload(&question(&format!("s{}", " ___".repeat(nb)), Kind::Blank { accept: vec![vec!["v".to_string(); nv]; nb] }));
    ok(&b(50, 20));
    has(&b(51, 1), "too_many", "question.kind.Blank.accept");
    has(&b(1, 21), "too_many", "question.kind.Blank.accept[0]");
    let q = question("s ___", Kind::Blank { accept: vec![vec!["x".repeat(2049)]] });
    has(&q_payload(&q), "too_long", "question.kind.Blank.accept[0][0]");
}

#[test]
fn parts() {
    let p = |n: usize| q_payload(&question("s", Kind::Multipart { parts: (0..n).map(|_| Part { stem: "p".into(), kind: PartKind::Tf { correct: true } }).collect() }));
    ok(&p(50));
    has(&p(51), "too_many", "question.kind.Multipart.parts");
    let q = question("s", Kind::Multipart { parts: vec![Part { stem: "x".repeat(8193), kind: PartKind::Short { sample: "y".repeat(2049) } }] });
    let c = codes(&q_payload(&q));
    assert!(c.contains(&("too_long", "question.kind.Multipart.parts[0].stem".into())), "{c:?}");
    assert!(c.contains(&("too_long", "question.kind.Multipart.parts[0].kind.Short.sample".into())), "{c:?}");
}

#[test]
fn topics_count_and_chars() {
    let mut q = mc(2);
    q.topics = vec!["t".into(); 20];
    ok(&q_payload(&q));
    q.topics = vec!["t".into(); 21];
    has(&q_payload(&q), "too_many", "question.topics");
    // 64 characters, not bytes: 64 two-byte characters pass.
    q.topics = vec!["é".repeat(64)];
    ok(&q_payload(&q));
    q.topics = vec!["é".repeat(65)];
    has(&q_payload(&q), "too_long", "question.topics[0]");
}

#[test]
fn bank_size_and_text() {
    let bank = |n: usize, name: &str| {
        Payload {
            schema_version: 1,
            body: Body::Bank(QuestionBank { id: "b".into(), name: name.into(), questions: vec![question("s", Kind::Tf { correct: true }); n], created: 0, modified: 0, details: Default::default(), origin: None, published: None }),
        }
        .to_json()
    };
    ok(&bank(1000, &"n".repeat(200)));
    has(&bank(1001, "n"), "too_many", "bank.questions");
    has(&bank(1, &"n".repeat(201)), "too_long", "bank.name");
    let mut b = QuestionBank { id: "b".into(), name: "n".into(), questions: vec![], created: 0, modified: 0, details: Default::default(), origin: None, published: None };
    b.details.description = "d".repeat(2049);
    b.details.license_url = "javascript:alert(1)".into();
    let j = Payload { schema_version: 1, body: Body::Bank(b) }.to_json();
    has(&j, "too_long", "bank.details.description");
    has(&j, "bad_url", "bank.details.license_url");
}

#[test]
fn body_size_and_depth() {
    let lim = Limits::default();
    let big = format!("{{\"schema_version\":1,\"question\":{{\"pad\":\"{}\"}}}}", "x".repeat(lim.body_bytes));
    assert_eq!(errs(&big)[0].code, "body_too_large");
    let deep = format!("{{\"schema_version\":1,\"question\":{}{}}}", "[".repeat(20), "]".repeat(20));
    assert_eq!(errs(&deep)[0].code, "too_deep");
    // The deepest legal shape (12 containers) passes at the default 16.
    let q = question("s", Kind::Multipart { parts: vec![Part { stem: "p".into(), kind: PartKind::Mc { answers: vec![answer("a", true), answer("b", false)] } }] });
    let b = Payload { schema_version: 1, body: Body::Bank(QuestionBank { id: "b".into(), name: "n".into(), questions: vec![q], created: 0, modified: 0, details: Default::default(), origin: None, published: None }) };
    ok(&b.to_json());
    let tight = Limits { max_depth: 11, ..Default::default() };
    assert_eq!(validate(&b.to_json(), &tight, &Structural).unwrap_err()[0].code, "too_deep");
}

#[test]
fn value_count() {
    // 26 answers x 4 values + the question's own: well inside; a flood of tiny values is refused
    // before any of it is walked.
    ok(&q_payload(&mc(26)));
    let flood = format!("{{\"schema_version\":1,\"question\":{{\"pad\":[{}0]}}}}", "0,".repeat(600_000));
    let e = errs(&flood);
    assert_eq!((e[0].code, e[0].limit), ("too_many_values", Some(200_000)));
}

#[test]
fn image_file_and_dimensions() {
    let h = "a".repeat(64);
    let with = |file: &str, w: u32, hh: u32| {
        let mut q = mc(2);
        q.image = Some(ImageRef { file: file.into(), width: w, height: hh });
        q_payload(&q)
    };
    for ext in ["png", "jpg", "gif", "webp"] {
        ok(&with(&format!("media/{h}.{ext}"), 1, 4096));
    }
    for bad in [
        format!("media/{h}.svg"),
        format!("media/{h}.PNG"),
        format!("media/{}.png", "A".repeat(64)),
        format!("media/{}.png", "a".repeat(63)),
        format!("media/{}.png", "a".repeat(65)),
        format!("../media/{h}.png"),
        format!("media/../{h}.png"),
        format!("/media/{h}.png"),
        format!("media/{h}.png.exe"),
        format!("media/{h}.jpeg"),
        format!("media\\{h}.png"),
        format!("https://evil.example/{h}.png"),
        format!("media/{h}.png\u{0}"),
        String::new(),
    ] {
        has(&with(&bad, 10, 10), "bad_image_file", "question.image.file");
    }
    has(&with(&format!("media/{h}.png"), 0, 10), "out_of_range", "question.image.width");
    has(&with(&format!("media/{h}.png"), 10, 4097), "out_of_range", "question.image.height");
}

#[test]
fn figure_params_structural_default() {
    let fig = |params: serde_json::Value| {
        let mut q = mc(2);
        q.figure = Some(FigureSpec { id: "free-body-diagram".into(), params: serde_json::from_value(params).unwrap() });
        q_payload(&q)
    };
    // Scalars kept; arrays, objects and nulls dropped; malformed keys dropped.
    let v = ok(&fig(serde_json::json!({"a": 1.5, "b": true, "c": "text", "d": [1], "e": {"x": 1}, "f": null, "Bad-Key": 1})));
    let f = only_question(&v.payload).figure.clone().unwrap();
    assert_eq!(f.params.keys().collect::<Vec<_>>(), ["a", "b", "c"]);
    assert!(v.dropped.iter().any(|d| d.reason == "param_type"));
    assert!(v.dropped.iter().any(|d| d.reason == "param_unknown"));
    // Text params: 4 KB each; the whole params object 16 KB of JSON.
    ok(&fig(serde_json::json!({"t": "x".repeat(4096)})));
    has(&fig(serde_json::json!({"t": "x".repeat(4097)})), "too_long", "question.figure.params.<text>");
    let many: serde_json::Map<String, serde_json::Value> = (0..5).map(|i| (format!("t{i}"), serde_json::json!("x".repeat(4000)))).collect();
    has(&fig(serde_json::Value::Object(many)), "too_long", "question.figure.params");
    // Figure ids must look like registry slugs.
    let mut q = mc(2);
    q.figure = Some(FigureSpec { id: "<script>".into(), params: Default::default() });
    has(&q_payload(&q), "bad_figure_id", "question.figure.id");
}

#[test]
fn figure_params_declared_registry() {
    let reg = Declared(
        [(
            "pipette".to_string(),
            vec![
                ParamSpec { key: "volume".into(), kind: ParamKind::Float { min: 0.0, max: 20.0 } },
                ParamSpec { key: "n".into(), kind: ParamKind::Int { min: 1, max: 5 } },
                ParamSpec { key: "on".into(), kind: ParamKind::Bool },
                ParamSpec { key: "size".into(), kind: ParamKind::Choice { options: vec!["P20".into(), "P200".into()], default: "P20".into() } },
                ParamSpec { key: "ink".into(), kind: ParamKind::Color { palette: vec!["#111111".into()], default: "#111111".into() } },
                ParamSpec { key: "label".into(), kind: ParamKind::Text },
                ParamSpec { key: "bad".into(), kind: ParamKind::Float { min: 5.0, max: 1.0 } },
            ],
        )]
        .into_iter()
        .collect(),
    );
    let mut q = mc(2);
    q.figure = Some(FigureSpec {
        id: "pipette".into(),
        params: serde_json::from_value(serde_json::json!({
            "volume": 99.0, "n": 2.6, "on": "yes", "size": "P1000", "ink": "red\" onload=\"x", "label": "ok", "extra": 1, "bad": 3.0
        }))
        .unwrap(),
    });
    let v = validate(&q_payload(&q), &Limits::default(), &reg).unwrap();
    let p = only_question(&v.payload).figure.clone().unwrap().params;
    assert_eq!(p["volume"], serde_json::json!(20.0));
    assert_eq!(p["n"], serde_json::json!(3));
    assert_eq!(p["size"], serde_json::json!("P20"));
    assert_eq!(p["ink"], serde_json::json!("#111111"), "a colour outside the palette is never kept");
    assert_eq!(p["label"], serde_json::json!("ok"));
    assert!(!p.contains_key("on") && !p.contains_key("extra") && !p.contains_key("bad"), "{p:?}");
    // Unknown figure id: dropped with a declared registry.
    q.figure = Some(FigureSpec { id: "other".into(), params: Default::default() });
    let v = validate(&q_payload(&q), &Limits::default(), &reg).unwrap();
    assert!(only_question(&v.payload).figure.is_none());
    assert!(v.dropped.iter().any(|d| d.reason == "figure_unknown"));
}

#[test]
fn points_ids_and_numbers() {
    let mut q = mc(2);
    q.points = Some(1000.0);
    ok(&q_payload(&q));
    q.points = Some(1000.5);
    has(&q_payload(&q), "out_of_range", "question.points");
    q.points = Some(-1.0);
    has(&q_payload(&q), "out_of_range", "question.points");
    let mut q = mc(2);
    q.id = "has space".into();
    has(&q_payload(&q), "bad_id", "question.id");
    q.id = "x".repeat(65);
    has(&q_payload(&q), "bad_id", "question.id");
    q.id = String::new();
    has(&q_payload(&q), "bad_id", "question.id");
    // `created` must be an integer, as the app's i64.
    let j = q_payload(&mc(2)).replace("\"created\":1", "\"created\":1.5");
    has(&j, "expected_integer", "question.created");
}

/// Every text field, by class: block fields allow LF and TAB, line fields allow neither, and both
/// refuse every other control, format, bidi, separator, private-use and unassigned character.
#[test]
fn forbidden_characters_in_every_text_field() {
    type Set = fn(&mut Question, &str);
    let block: Vec<(&str, Set)> = vec![
        ("question.stem", |q, s| q.stem = s.into()),
        ("question.explanation", |q, s| q.explanation = Some(s.into())),
        ("question.kind.Mc.answers[0].text", |q, s| q.kind = Kind::Mc { answers: vec![answer(s, true), answer("b", false)] }),
        ("question.kind.Matching.pairs[0].left", |q, s| q.kind = Kind::Matching { pairs: vec![Pair { left: s.into(), right: "r".into() }, pair2()], distractors: vec![] }),
        ("question.kind.Matching.pairs[0].right", |q, s| q.kind = Kind::Matching { pairs: vec![Pair { left: "l".into(), right: s.into() }, pair2()], distractors: vec![] }),
        ("question.kind.Matching.distractors[0]", |q, s| q.kind = Kind::Matching { pairs: vec![pair2(), pair2()], distractors: vec![s.into()] }),
        ("question.kind.Blank.accept[0][0]", |q, s| {
            q.stem = "s ___".into();
            q.kind = Kind::Blank { accept: vec![vec![s.into()]] }
        }),
        ("question.kind.Short.sample", |q, s| q.kind = Kind::Short { sample: s.into() }),
        ("question.kind.Multipart.parts[0].stem", |q, s| q.kind = Kind::Multipart { parts: vec![Part { stem: s.into(), kind: PartKind::Tf { correct: true } }] }),
        ("question.figure.params.<text>", |q, s| {
            q.figure = Some(FigureSpec { id: "f".into(), params: [("t".to_string(), serde_json::json!(s))].into_iter().collect() })
        }),
    ];
    let line: Vec<(&str, Set)> = vec![("question.topics[0]", |q, s| q.topics = vec![s.into()])];
    // Refused in every class.
    let bad = [
        "\u{202E}", "\u{202A}", "\u{2066}", "\u{2069}", "\u{206A}", "\u{0}", "\u{1B}", "\u{7F}", "\u{85}",
        "\u{FFF9}", "\u{E0001}", "\u{E000}", "\u{FFFF}", "\u{3164}", "\u{115F}", "\u{FFA0}", "\u{2800}",
    ];
    // Allowed in block text, refused in line text.
    let block_only = ["\u{200B}", "\u{AD}", "\u{61C}", "\u{2062}", "\u{180E}"];
    for (_, set) in &block {
        for b in block_only {
            let mut q = mc(2);
            set(&mut q, &format!("ok{b}ok"));
            ok(&q_payload(&q));
        }
    }
    for (path, set) in &line {
        for b in block_only {
            let mut q = mc(2);
            set(&mut q, &format!("ok{b}ok"));
            has(&q_payload(&q), "forbidden_char", path);
        }
    }
    for (path, set) in block.iter().chain(line.iter()) {
        for b in bad {
            let mut q = mc(2);
            set(&mut q, &format!("ok{b}ok"));
            has(&q_payload(&q), "forbidden_char", path);
        }
    }
    for (path, set) in &block {
        let mut q = mc(2);
        set(&mut q, "line one\nline\ttwo\r\nthree");
        let v = validate(&q_payload(&q), &Limits::default(), &Structural).unwrap_or_else(|e| panic!("{path}: {e:?}"));
        assert!(!v.payload.to_json().contains("\\r"), "{path}: CR normalised");
    }
    for (path, set) in &line {
        for c in ["\n", "\t"] {
            let mut q = mc(2);
            set(&mut q, &format!("a{c}b"));
            has(&q_payload(&q), "forbidden_char", path);
        }
    }
    // Bank name and details (line fields) too.
    for (field, path) in [("name", "bank.name"), ("author", "bank.details.author"), ("license_name", "bank.details.license_name")] {
        let mut b = QuestionBank { id: "b".into(), name: "n".into(), questions: vec![], created: 0, modified: 0, details: Default::default(), origin: None, published: None };
        let s = "x\u{202E}y\n".to_string();
        match field {
            "name" => b.name = s,
            "author" => b.details.author = s,
            _ => b.details.license_name = s,
        }
        has(&Payload { schema_version: 1, body: Body::Bank(b) }.to_json(), "forbidden_char", path);
    }
}

#[test]
fn errors_never_echo_input() {
    let secret = "SECRET\u{202E}VALUE";
    let mut q = mc(2);
    q.stem = format!("{secret}{}", "x".repeat(9000));
    q.topics = vec![secret.into()];
    q.kind = Kind::Mc { answers: vec![answer(secret, true)] };
    let json = q_payload(&q).replace("\"Mc\"", "\"SECRETKIND\"").replace("\"created\":1", "\"created\":\"SECRETNUM\"");
    let e = errs(&json);
    let out = serde_json::to_string(&e).unwrap();
    assert!(!out.contains("SECRET"), "{out}");
    // Unknown fields are reported by the object they were in, never by name.
    let j = q_payload(&mc(2)).replacen("{", "{\"SECRETFIELD\":1,", 2);
    let v = ok(&j);
    assert!(!serde_json::to_string(&v.dropped).unwrap().contains("SECRET"));
    assert!(!v.payload.to_json().contains("SECRET"));
    // The wasm surface likewise.
    let out = share_core::wasm::validate_json(json.as_bytes(), "");
    assert!(out.starts_with("{\"errors\":") && !out.contains("SECRET"), "{out}");
}

#[test]
fn strict_reader_and_envelope() {
    let base = q_payload(&mc(2));
    has(&base.replacen("\"correct\":false", "\"correct\":false,\"correct\":true", 1), "duplicate_key", "");
    assert_eq!(errs("{\"schema_version\":1,\"question\":{},\"question\":{}}")[0].code, "duplicate_key");
    assert_eq!(errs("not json")[0].code, "invalid_json");
    assert_eq!(errs("[]")[0].code, "expected_object");
    has(&base.replace("\"schema_version\":2", "\"schema_version\":3"), "schema_version_unsupported", "schema_version");
    has(&base.replace("\"schema_version\":2", "\"schema_version\":0"), "schema_version_unsupported", "schema_version");
    has(&base.replace(",\"schema_version\":2", ""), "missing", "schema_version");
    let both = base.replacen("{", "{\"bank\":{},", 1);
    has(&both, "ambiguous_kind", "");
    has("{\"schema_version\":1}", "missing", "question");
    // Kind tagging is the app's: exactly one known variant.
    has(&base.replace("{\"Mc\":", "{\"Tf\":{\"correct\":true},\"Mc\":"), "ambiguous_kind", "question.kind");
    has(&base.replace("{\"Mc\":", "{\"Essay\":"), "unknown_kind", "question.kind");
    has(&base.replace("\"correct\":false", "\"correct\":0"), "expected_bool", "question.kind.Mc.answers[1].correct");
    // Missing required fields, as the app's serde requires them.
    has(&base.replace("\"stem\":\"s\",", ""), "missing", "question.stem");
}

#[test]
fn unknown_and_version_gated_fields_are_dropped() {
    let mut q = question("s", Kind::Mc { answers: vec![answer("a", false), answer("b", false)] });
    q.keyless = true;
    q.published = Some(Published { item_id: "i".into(), qid: "q".into(), publish_id: "p".into(), version: 1 });
    // v2 keeps them.
    let v = ok(&q_payload(&q));
    assert!(only_question(&v.payload).keyless && only_question(&v.payload).published.is_some());
    // v1 does not know them: `published` is dropped and reported; `keyless: true` is refused,
    // because a keyless question read as a full one would serve its placeholder key.
    has(&q_payload_v1(&q), "v2_field_in_v1", "question.keyless");
    q.keyless = false;
    let v = ok(&q_payload_v1(&q));
    let back = only_question(&v.payload);
    assert!(!back.keyless && back.published.is_none());
    assert_eq!(v.dropped, [Dropped { path: "question".into(), reason: "unknown_field" }]);
    // Unknown fields anywhere are dropped (mass assignment, ASVS 15.3.3).
    let j = q_payload(&mc(2)).replace("\"locked\":false", "\"locked\":false,\"is_admin\":true");
    let v = ok(&j);
    assert!(!v.payload.to_json().contains("is_admin"));
    assert_eq!(v.dropped.len(), 2);
}

#[test]
fn origin_fields() {
    let mut q = mc(2);
    q.origin = Some(Origin {
        item_id: "i1".into(),
        qid: "q1".into(),
        publish_id: "p1".into(),
        version: 1,
        author_name: "Ms. X".into(),
        license: "CC-BY-4.0".into(),
        form: "full".into(),
        served_sha256: "0".repeat(64),
        match_hash: String::new(),
        chain: vec![ChainLink { author_name: "Mr. Y".into(), item_id: "i0".into(), qid: "q0".into(), version: 2, license: "CC0-1.0".into(), modified: true }; 32],
    });
    ok(&q_payload(&q));
    let mut bad = q.clone();
    bad.origin.as_mut().unwrap().license = "GPL".into();
    has(&q_payload(&bad), "bad_license", "question.origin.license");
    let mut bad = q.clone();
    bad.origin.as_mut().unwrap().served_sha256 = "Z".repeat(64);
    has(&q_payload(&bad), "bad_hash", "question.origin.served_sha256");
    let mut bad = q.clone();
    let link = bad.origin.as_ref().unwrap().chain[0].clone();
    bad.origin.as_mut().unwrap().chain.push(link);
    has(&q_payload(&bad), "too_many", "question.origin.chain");
    let mut bad = q.clone();
    bad.origin.as_mut().unwrap().form = "partial".into();
    has(&q_payload(&bad), "bad_form", "question.origin.form");
}

#[test]
fn meta() {
    let lim = Limits::default();
    let m = validate_meta(r#"{"title":"  Unit  3 ﬁnal ","tags":["Cells","cells"," DNA "],"standards":["hs-ls1-1"],"subject":"Biology","x":1}"#, &lim).unwrap();
    assert_eq!(m.title, "Unit 3 final");
    assert_eq!(m.tags, ["cells", "dna"]);
    assert_eq!(m.standards, ["HS-LS1-1"]);
    assert!(validate_meta(r#"{"title":""}"#, &lim).is_err());
    assert!(validate_meta("{\"title\":\"a\\u202eb\"}", &lim).is_err());
    let tags: Vec<String> = (0..31).map(|i| format!("t{i}")).collect();
    assert!(validate_meta(&serde_json::json!({"title": "t", "tags": tags}).to_string(), &lim).is_err());
    assert!(validate_meta(&serde_json::json!({"title": "t", "tags": ["x".repeat(65)]}).to_string(), &lim).is_err());
    assert!(validate_meta(&serde_json::json!({"title": "t".repeat(201)}).to_string(), &lim).is_err());
}
