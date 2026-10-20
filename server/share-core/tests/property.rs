//! Property tests: no key material survives `strip`, and `match_hash` is blind to keys and shuffles.
//!
//! Each case builds a random payload (a question or a bank of up to five) in which every key-bearing
//! field holds a unique marker: explanations, `Short`/part samples, `Blank` accept entries, figure
//! ids, param keys and param values, image file names (unticked), `origin.served_sha256` /
//! `origin.match_hash`, and `published` ids. It strips the payload, serialises the result, and
//! asserts that no key marker appears anywhere in it, that the non-key markers (stems, part stems,
//! choices, left items, topics, bank name) all do, that no `correct` flag survives, that every
//! matching right side is empty and the word bank is exactly the set of right items and
//! distractors, and that labels are gone whenever two or more choices carried one. It also checks
//! the hash invariants on every case.
mod common;
use common::*;
use share_core::*;
use std::collections::BTreeSet;

const CASES: usize = 4000;

struct Gen {
    style: usize,
    rng: ShuffleRng,
    n: usize,
    key: Vec<String>,
    visible: Vec<String>,
}

impl Gen {
    fn new(case: usize) -> Gen {
        let mut s = [0u8; 32];
        s[..8].copy_from_slice(&(case as u64).to_le_bytes());
        s[8] = 0xA5;
        Gen { style: 0, rng: ShuffleRng::from_seed(s), n: 0, key: vec![], visible: vec![] }
    }
    fn coin(&mut self) -> bool {
        self.rng.below(2) == 1
    }
    fn upto(&mut self, n: usize) -> usize {
        self.rng.below(n + 1)
    }
    /// A key marker (`KEY000123Z`), also usable as an id or a lower-cased slug.
    fn key(&mut self) -> String {
        self.n += 1;
        let m = format!("KEY{:06}Z", self.n);
        self.key.push(m.clone());
        m
    }
    fn vis(&mut self, tag: &str) -> String {
        self.n += 1;
        let m = format!("{tag}{:06}V", self.n);
        self.visible.push(m.clone());
        m
    }
    /// Some surrounding text, sometimes multi-line, sometimes with non-ASCII.
    fn wrap(&mut self, m: &str) -> String {
        match self.rng.below(4) {
            0 => m.to_string(),
            1 => format!("The {m} item"),
            2 => format!("Line one {m}\nline two\twith a tab"),
            _ => format!("Énergie {m} — ½ µL"),
        }
    }
    fn hex(&mut self) -> String {
        // A unique 64-hex string; recorded as a key marker by the caller when it must not survive.
        self.n += 1;
        format!("{:064x}", (self.n as u128) * 0x9E37_79B9_7F4A_7C15_u128 + 0xABCDEF)
    }
    /// A position label in this question's style (A. / A) / (a) / Ⓐ / 1. / i. / Option A: / A、).
    fn label(&mut self, i: usize, on: bool) -> String {
        if !on {
            return String::new();
        }
        const ROMAN: [&str; 10] = ["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x"];
        let l = (b'A' + (i % 26) as u8) as char;
        match self.style {
            0 => format!("{l}. "),
            1 => format!("{l})"),
            2 => format!("({}) ", l.to_ascii_lowercase()),
            3 => format!("{} ", char::from_u32(0x24B6 + (i % 26) as u32).unwrap()),
            4 => format!("{}. ", i + 1),
            5 => format!("{}. ", ROMAN[i % 10]),
            6 => format!("Option {l}: "),
            _ => format!("{l}\u{3001}"),
        }
    }
    fn answers(&mut self) -> (Vec<Answer>, bool) {
        let n = 2 + self.upto(6);
        let labelled = self.rng.below(3) == 0;
        self.style = self.rng.below(8);
        let mut v = Vec::new();
        let mut labels = 0;
        for i in 0..n {
            let on = labelled && self.rng.below(5) != 0;
            labels += on as usize;
            let l = self.label(i, on);
            let t = match self.rng.below(8) {
                // Edge shapes for the label rule: a label and nothing else, a label and a line
                // break, a double label. These carry no visible marker.
                0 if on => format!("{l}{}", ["", " ", "\n"][self.rng.below(3)]),
                1 => format!("{l}\n{}", self.label(i + 1, true)),
                _ => {
                    let m = self.vis("CHOICE");
                    format!("{l}{}", self.wrap(&m))
                }
            };
            v.push(Answer { text: t, correct: self.coin(), locked: self.rng.below(4) == 0 });
        }
        // Sometimes a choice that names its siblings.
        if self.rng.below(6) == 0 {
            let t = ["Both A and B", "All of the above", "None of these", "I and II only"][self.rng.below(4)];
            v.push(Answer { text: t.into(), correct: false, locked: self.coin() });
        }
        (v, labels >= 2)
    }
    fn part(&mut self) -> Part {
        let m = self.vis("PSTEM");
        let stem = self.wrap(&m);
        let kind = match self.rng.below(3) {
            0 => PartKind::Mc { answers: self.answers().0 },
            1 => PartKind::Tf { correct: self.coin() },
            _ => {
                let k = self.key();
                PartKind::Short { sample: self.wrap(&k) }
            }
        };
        Part { stem, kind }
    }
    fn question(&mut self, i: usize) -> (Question, bool) {
        let m = self.vis("STEM");
        let stem = self.wrap(&m);
        let mut blanks = 0;
        let kind = match self.rng.below(6) {
            0 => Kind::Mc { answers: self.answers().0 },
            1 => Kind::Tf { correct: self.coin() },
            2 => {
                let np = 2 + self.upto(4);
                let share = np >= 2 && self.coin();
                let mut pairs: Vec<Pair> = Vec::new();
                for i in 0..np {
                    let l = self.vis("LEFT");
                    // Sometimes two lefts share a right item.
                    let r = if share && i == 1 { pairs[0].right.clone() } else { self.vis("RIGHT") };
                    pairs.push(Pair { left: self.wrap(&l), right: r });
                }
                let nd = self.upto(4);
                let distractors = (0..nd).map(|_| self.vis("DISTRACTOR")).collect();
                Kind::Matching { pairs, distractors }
            }
            3 => {
                let nb = 1 + self.upto(4);
                let accept = (0..nb).map(|_| (0..1 + self.upto(3)).map(|_| self.key()).collect()).collect();
                blanks = nb;
                Kind::Blank { accept }
            }
            4 => {
                let k = self.key();
                Kind::Short { sample: self.wrap(&k) }
            }
            _ => Kind::Multipart { parts: (0..1 + self.upto(4)).map(|_| self.part()).collect() },
        };
        let stem = stem + &" ___".repeat(blanks);
        let topics = (0..self.upto(3)).map(|_| self.vis("TOPIC")).collect();
        let explanation = if self.coin() {
            let k = self.key();
            Some(self.wrap(&k))
        } else {
            None
        };
        let figure = if self.coin() {
            let id = self.key().to_lowercase();
            let mut params = std::collections::BTreeMap::new();
            for _ in 0..1 + self.upto(4) {
                let k = self.key().to_lowercase();
                let v = match self.rng.below(3) {
                    0 => serde_json::Value::String(self.key()),
                    1 => serde_json::json!(self.rng.below(1000) as f64 / 10.0),
                    _ => serde_json::json!(self.coin()),
                };
                params.insert(k, v);
            }
            Some(FigureSpec { id: format!("fig-{id}"), params })
        } else {
            None
        };
        let image_ok = self.coin();
        let image = if self.coin() {
            let h = self.hex();
            if image_ok { self.visible.push(h.clone()) } else { self.key.push(h.clone()) }
            Some(ImageRef { file: format!("media/{h}.png"), width: 1 + self.rng.below(4096) as u32, height: 1 + self.rng.below(4096) as u32 })
        } else {
            None
        };
        let origin = if self.coin() {
            let (s1, s2) = (self.hex(), self.hex());
            self.key.push(s1.clone());
            self.key.push(s2.clone());
            Some(Origin {
                item_id: "item1".into(),
                qid: "qid1".into(),
                publish_id: "pub1".into(),
                version: 3,
                author_name: "Ms. X".into(),
                license: "CC-BY-4.0".into(),
                form: "full".into(),
                served_sha256: s1,
                match_hash: s2,
                chain: vec![],
            })
        } else {
            None
        };
        let published = if self.coin() {
            Some(Published { item_id: self.key(), qid: self.key(), publish_id: self.key(), version: 1 })
        } else {
            None
        };
        let q = Question {
            id: format!("q{i}"),
            stem,
            kind,
            points: Some(self.rng.below(10) as f64),
            difficulty: None,
            topics,
            image,
            explanation,
            figure,
            created: 5,
            modified: 6,
            keyless: false,
            keep_order: false,
            origin,
            published,
        };
        (q, image_ok)
    }
    fn payload(&mut self) -> (Payload, Vec<usize>) {
        if self.coin() {
            let (q, ok) = self.question(0);
            (Payload { schema_version: 2, body: Body::Question(q) }, if ok { vec![0] } else { vec![] })
        } else {
            let mut qs = Vec::new();
            let mut oks = Vec::new();
            for i in 0..1 + self.upto(4) {
                let (q, ok) = self.question(i);
                qs.push(q);
                if ok {
                    oks.push(i);
                }
            }
            let name = self.vis("BANK");
            let origin = if self.coin() {
                let (s1, s2) = (self.hex(), self.hex());
                self.key.push(s1.clone());
                self.key.push(s2.clone());
                Some(Origin {
                    item_id: "bitem".into(),
                    qid: String::new(),
                    publish_id: "bpub".into(),
                    version: 2,
                    author_name: "Mr. Y".into(),
                    license: "CC0-1.0".into(),
                    form: "full".into(),
                    served_sha256: s1,
                    match_hash: s2,
                    chain: vec![],
                })
            } else {
                None
            };
            let published = if self.coin() { Some(Published { item_id: self.key(), qid: String::new(), publish_id: self.key(), version: 4 }) } else { None };
            let bank = QuestionBank { id: "b1".into(), name, questions: qs, created: 1, modified: 2, details: BankDetails::default(), origin, published };
            (Payload { schema_version: 2, body: Body::Bank(bank) }, oks)
        }
    }
}

fn mc_texts(a: &[Answer]) -> Vec<String> {
    let mut v: Vec<String> = a.iter().map(|x| x.text.clone()).collect();
    v.sort();
    v
}

/// Structural checks of one stripped question against its original.
fn check_question(orig: &Question, s: &Question, case: usize, labels: &QuestionLabels, kept: &dyn Fn(Option<usize>) -> bool) {
    assert!(s.keyless && s.explanation.is_none() && s.figure.is_none() && s.published.is_none(), "case {case}");
    if let Some(o) = &s.origin {
        assert!(o.served_sha256.is_empty() && o.match_hash.is_empty());
    }
    let mc = |a: &[Answer], b: &[Answer], part: Option<usize>| {
        let decision = labels.get(&part).copied();
        assert_eq!(a.len(), b.len());
        assert!(b.iter().all(|x| !x.correct), "case {case}: a correct flag survived");
        // Locked answers keep their slot.
        for (i, x) in a.iter().enumerate() {
            if x.locked {
                assert!(b[i].locked, "case {case}: locked answer moved");
            }
        }
        // Labels: removed exactly when Strip was decided (and allowed), as the preview said.
        let texts: Vec<&str> = a.iter().map(|x| x.text.as_str()).collect();
        let mut aa: Vec<String> = match (share_core::labels::analyze(&texts), decision) {
            (Some(an), Some(LabelChoice::Strip)) => an.previews(&texts).into_iter().map(|c| c.after).collect(),
            _ => texts.iter().map(|t| t.to_string()).collect(),
        };
        if kept(part) {
            // Order preserved (labels kept, sibling references, too few unlocked): nothing moved.
            assert_eq!(aa, b.iter().map(|x| x.text.clone()).collect::<Vec<_>>(), "case {case}: a kept order moved");
        }
        aa.sort();
        assert_eq!(aa, mc_texts(b), "case {case}: label removal");
    };
    match (&orig.kind, &s.kind) {
        (Kind::Mc { answers: a }, Kind::Mc { answers: b }) => mc(a, b, None),
        (Kind::Tf { .. }, Kind::Tf { correct }) => assert!(!correct),
        (Kind::Matching { pairs: p, distractors: d }, Kind::Matching { pairs: sp, distractors: sd }) => {
            assert_eq!(p.iter().map(|x| &x.left).collect::<Vec<_>>(), sp.iter().map(|x| &x.left).collect::<Vec<_>>());
            assert!(sp.iter().all(|x| x.right.is_empty()), "case {case}: a right side survived");
            let want: BTreeSet<&String> = p.iter().map(|x| &x.right).chain(d.iter()).collect();
            let got: BTreeSet<&String> = sd.iter().collect();
            assert_eq!(want, got, "case {case}: word bank");
            assert_eq!(sd.len(), got.len(), "case {case}: duplicates in the word bank");
        }
        (Kind::Blank { accept: a }, Kind::Blank { accept: b }) => {
            assert_eq!(a.len(), b.len());
            assert!(b.iter().all(|x| x.is_empty()));
        }
        (Kind::Short { .. }, Kind::Short { sample }) => assert!(sample.is_empty()),
        (Kind::Multipart { parts: a }, Kind::Multipart { parts: b }) => {
            assert_eq!(a.len(), b.len());
            for (pi, (x, y)) in a.iter().zip(b).enumerate() {
                assert_eq!(x.stem, y.stem);
                match (&x.kind, &y.kind) {
                    (PartKind::Mc { answers: a }, PartKind::Mc { answers: b }) => mc(a, b, Some(pi)),
                    (PartKind::Tf { .. }, PartKind::Tf { correct }) => assert!(!correct),
                    (PartKind::Short { .. }, PartKind::Short { sample }) => assert!(sample.is_empty()),
                    _ => panic!("case {case}: part kind changed"),
                }
            }
        }
        _ => panic!("case {case}: kind changed"),
    }
}

/// The same question with a different key: flipped flags, re-paired rights, other accept lists,
/// samples, explanation and figure.
fn other_key(q: &Question, g: &mut Gen) -> Question {
    let mut k = q.clone();
    let flip = |a: &mut Vec<Answer>, g: &mut Gen| a.iter_mut().for_each(|x| x.correct = g.coin());
    match &mut k.kind {
        Kind::Mc { answers } => flip(answers, g),
        Kind::Tf { correct } => *correct = !*correct,
        Kind::Matching { pairs, .. } => {
            let mut rights: Vec<String> = pairs.iter().map(|p| p.right.clone()).collect();
            g.rng.shuffle(&mut rights);
            pairs.iter_mut().zip(rights).for_each(|(p, r)| p.right = r);
        }
        Kind::Blank { accept } => accept.iter_mut().for_each(|a| *a = vec!["other".into()]),
        Kind::Short { sample } => *sample = "another sample".into(),
        Kind::Multipart { parts } => {
            for p in parts {
                match &mut p.kind {
                    PartKind::Mc { answers } => flip(answers, g),
                    PartKind::Tf { correct } => *correct = !*correct,
                    PartKind::Short { sample } => *sample = "x".into(),
                }
            }
        }
    }
    k.explanation = Some("a different explanation".into());
    k.figure = None;
    k
}

#[test]
fn no_key_marker_survives_strip_and_hashes_are_key_blind() {
    let mut stats = [0usize; 6];
    for case in 0..CASES {
        let mut g = Gen::new(case);
        let (p, image_ok) = g.payload();
        // Through the real entry point: JSON in, validated, stripped.
        let v = validate(&p.to_json(), &Limits::default(), &Structural).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
        assert!(v.dropped.is_empty(), "case {case}: {:?}", v.dropped);
        // The author's decisions: Strip where allowed (half the time), else Keep.
        let labels: Vec<LabelDecision> = label_findings(&v.payload)
            .into_iter()
            .map(|f| LabelDecision { question: f.question, part: f.part, choice: if f.strip_allowed && g.coin() { LabelChoice::Strip } else { LabelChoice::Keep } })
            .collect();
        // Undecided findings are refused.
        if !labels.is_empty() {
            let e = strip_with_seed(v.payload.clone(), &StripOptions { image_ok: image_ok.clone(), labels: vec![] }, seed(1)).unwrap_err();
            assert!(e.iter().all(|e| e.code == "labels_need_decision"), "case {case}: {e:?}");
        }
        let so = StripOptions { image_ok: image_ok.clone(), labels: labels.clone() };
        let s = strip_with_seed(v.payload.clone(), &so, seed(case as u8)).unwrap();
        let out = s.payload.to_json();
        assert_eq!(out, canonical_json(&s.payload.to_value()), "case {case}: canonical writer");
        assert_eq!(v.payload.to_json(), canonical_json(&v.payload.to_value()), "case {case}: canonical writer");

        for m in &g.key {
            assert!(!out.contains(m.as_str()) && !out.contains(&m.to_lowercase()), "case {case}: key marker {m} leaked");
        }
        for m in &g.visible {
            assert!(out.contains(m.as_str()), "case {case}: visible marker {m} missing");
        }
        assert!(!out.contains("\"correct\":true"), "case {case}");

        // The stripped form passes the importer's validation and is stable under it.
        let back = ok(&out);
        assert_eq!(back.payload, s.payload, "case {case}");

        let s2 = strip_with_seed(v.payload.clone(), &so, seed(case as u8 ^ 0xFF)).unwrap();
        for (i, ((o, a), b)) in v.payload.questions().iter().zip(s.payload.questions()).zip(s2.payload.questions()).enumerate() {
            let ql = question_labels(&labels, i);
            let kept = |part: Option<usize>| s.order_reasons.iter().any(|n| n.question == i && n.part == part);
            check_question(o, a, case, &ql, &kept);
            stats[o.kind.tag().len() % 6] += 1;
            let h = match_hash_with(o, &ql);
            assert_eq!(h, match_hash(a), "case {case} q{i}: full (with the decisions) vs stripped");
            assert_eq!(h, match_hash(b), "case {case} q{i}: seed changed the hash");
            assert_eq!(h, match_hash(&strip_without_shuffle(o, &ql)), "case {case} q{i}");
            assert_eq!(match_hash(o), match_hash_with(o, &QuestionLabels::new()), "case {case}: no decision = plain text");
            let k = other_key(o, &mut g);
            assert_eq!(h, match_hash_with(&k, &ql), "case {case} q{i}: a different key changed match_hash");
            assert_eq!(match_skeleton(o), match_skeleton(&k));
            if k != *o {
                assert_ne!(full_hash(o), full_hash(&k), "case {case}: full_hash must see the key");
            }
        }
    }
    assert!(stats.iter().sum::<usize>() > CASES);
}

/// The original position of the (single) correct answer tells nothing about its stripped position:
/// over many seeds every slot is equally likely (chi-square, 5 slots, generous bound).
#[test]
fn correct_answer_position_is_uniform_after_strip() {
    let q = question(
        "Pick one",
        Kind::Mc { answers: vec![answer("Mercury", true), answer("Venus", false), answer("Earth", false), answer("Mars", false), answer("Jupiter", false)] },
    );
    let v = ok(&q_payload(&q));
    let mut counts = [0usize; 5];
    let n = 20_000;
    for i in 0..n {
        let mut s = [0u8; 32];
        s[..8].copy_from_slice(&(i as u64).to_le_bytes());
        let st = strip_with_seed(v.payload.clone(), &StripOptions::default(), s).unwrap();
        let Kind::Mc { answers } = &only_question(&st.payload).kind else { panic!() };
        counts[answers.iter().position(|a| a.text == "Mercury").unwrap()] += 1;
    }
    let e = n as f64 / 5.0;
    let chi: f64 = counts.iter().map(|&c| (c as f64 - e).powi(2) / e).sum();
    assert!(chi < 25.0, "chi-square {chi} for {counts:?}");
}

#[test]
fn two_seeds_same_choices_different_order() {
    let q = question("Pick", Kind::Mc { answers: (0..10).map(|i| answer(&format!("planet {i}"), i == 3)).collect() });
    let v = ok(&q_payload(&q));
    let a = strip_with_seed(v.payload.clone(), &StripOptions::default(), seed(1)).unwrap();
    let b = strip_with_seed(v.payload.clone(), &StripOptions::default(), seed(2)).unwrap();
    let (Kind::Mc { answers: x }, Kind::Mc { answers: y }) = (&only_question(&a.payload).kind, &only_question(&b.payload).kind) else { panic!() };
    assert_eq!(mc_texts(x), mc_texts(y));
    assert_ne!(x, y, "different seeds give different orders");
    // Same seed, same output (deterministic for tests and replays).
    let c = strip_with_seed(v.payload.clone(), &StripOptions::default(), seed(1)).unwrap();
    assert_eq!(a.payload, c.payload);

    let m = question("Match", Kind::Matching {
        pairs: (0..8).map(|i| Pair { left: format!("l{i}"), right: format!("r{i}") }).collect(),
        distractors: vec!["d1".into(), "d2".into()],
    });
    let v = ok(&q_payload(&m));
    let a = strip_with_seed(v.payload.clone(), &StripOptions::default(), seed(1)).unwrap();
    let b = strip_with_seed(v.payload.clone(), &StripOptions::default(), seed(2)).unwrap();
    let (Kind::Matching { distractors: x, .. }, Kind::Matching { distractors: y, .. }) = (&only_question(&a.payload).kind, &only_question(&b.payload).kind) else { panic!() };
    assert_ne!(x, y);
    let (mut x, mut y) = (x.clone(), y.clone());
    x.sort();
    y.sort();
    assert_eq!(x, y);
}

#[test]
fn order_ack_when_fewer_than_two_unlocked() {
    let mut a = vec![answer("Mercury", true), answer("Venus", false), answer("Earth", false)];
    a[0].locked = true;
    a[1].locked = true;
    let multipart = question("M", Kind::Multipart { parts: vec![
        Part { stem: "p0".into(), kind: PartKind::Mc { answers: vec![answer("alpha", true), answer("beta", false)] } },
        Part { stem: "p1".into(), kind: PartKind::Mc { answers: vec![Answer { locked: true, ..answer("x", true) }, answer("y", false)] } },
    ] });
    // Position labels on some choices only: kept, and the order must be acknowledged.
    let partial = question("labels", Kind::Mc { answers: vec![answer("A. x", true), answer("B. y", false), answer("None of these", false)] });
    let bank = QuestionBank {
        id: "b".into(),
        name: "B".into(),
        questions: vec![question("one", Kind::Mc { answers: a.clone() }), multipart, partial],
        created: 0,
        modified: 0,
        details: Default::default(),
        origin: None,
        published: None,
    };
    let v = ok(&Payload { schema_version: 1, body: Body::Bank(bank) }.to_json());
    let opts = StripOptions { labels: decide_all(&v.payload, LabelChoice::Keep), ..Default::default() };
    let s = strip_with_seed(v.payload.clone(), &opts, seed(3)).unwrap();
    assert_eq!(s.needs_order_ack, [Pos { question: 0, part: None }, Pos { question: 1, part: Some(1) }, Pos { question: 2, part: None }]);
    let Body::Bank(b) = &s.payload.body else { panic!() };
    let Kind::Mc { answers } = &b.questions[2].kind else { panic!() };
    let mut t: Vec<&str> = answers.iter().map(|a| a.text.as_str()).collect();
    t.sort();
    assert_eq!(t, ["A. x", "B. y", "None of these"], "partial labels are not removed");
    let Body::Bank(b) = &s.payload.body else { panic!() };
    let Kind::Mc { answers } = &b.questions[0].kind else { panic!() };
    assert_eq!(answers.iter().map(|a| a.text.as_str()).collect::<Vec<_>>(), ["Mercury", "Venus", "Earth"], "nothing moves");
    assert_eq!(s.order_reasons[0].reason, share_core::OrderReason::FewUnlocked);
    assert!(answers.iter().all(|a| !a.correct));
}

#[test]
fn shuffle_rng_is_unbiased_for_small_ranges() {
    let mut r = ShuffleRng::from_seed(seed(9));
    for n in [1usize, 2, 3, 7, 26] {
        let mut c = vec![0usize; n];
        let trials = 26_000;
        for _ in 0..trials {
            c[r.below(n)] += 1;
        }
        let e = trials as f64 / n as f64;
        let chi: f64 = c.iter().map(|&x| (x as f64 - e).powi(2) / e).sum();
        assert!(chi < 3.0 * n as f64 + 20.0, "n={n} chi={chi}");
    }
}

/// Keyless inputs: random keyless questions are accepted only when they carry no key material
/// (no correct flag, no Tf true, no accepted blank, no sample, no explanation but the sentinel, no
/// figure) and no blank right side; each injected piece of key material is refused.
#[test]
fn keyless_inputs_never_carry_key_material() {
    for case in 0..CASES {
        let mut g = Gen::new(case + 1_000_000);
        let (mut q, _) = g.question(0);
        // Make it a clean keyless question first (what strip serves, localised or not).
        let s = strip_question(&q, false, &decide_q(&q), &mut ShuffleRng::from_seed(seed(case as u8))).unwrap();
        q = if g.coin() { share_core::import::localize(&s) } else { s };
        q.image = None;
        let clean = Payload { schema_version: 2, body: Body::Question(q.clone()) }.to_json();
        assert!(validate(&clean, &Limits::default(), &Structural).is_ok(), "case {case}: {clean}");
        // Inject one piece of key material.
        let mut bad = q.clone();
        let injected = match &mut bad.kind {
            Kind::Mc { answers } => {
                answers[0].correct = true;
                "keyless_key_material"
            }
            Kind::Tf { correct } => {
                *correct = true;
                "keyless_key_material"
            }
            Kind::Blank { accept } => {
                accept[0] = vec!["k".into()];
                "keyless_key_material"
            }
            Kind::Short { sample } => {
                *sample = "k".into();
                "keyless_key_material"
            }
            Kind::Matching { pairs, .. } => {
                pairs[0].right = " ".into();
                "blank_text"
            }
            Kind::Multipart { parts } => match &mut parts[0].kind {
                PartKind::Mc { answers } => {
                    answers[0].correct = true;
                    "keyless_key_material"
                }
                PartKind::Tf { correct } => {
                    *correct = true;
                    "keyless_key_material"
                }
                PartKind::Short { sample } => {
                    *sample = "k".into();
                    "keyless_key_material"
                }
            },
        };
        let e = errs(&Payload { schema_version: 2, body: Body::Question(bad) }.to_json());
        assert!(e.iter().any(|e| e.code == injected), "case {case}: {e:?}");
        let mut expl = q.clone();
        expl.explanation = Some("the answer is 4".into());
        assert!(errs(&Payload { schema_version: 2, body: Body::Question(expl) }.to_json()).iter().any(|e| e.code == "keyless_key_material"));
    }
}

/// Keep for every finding of one question.
fn decide_q(q: &Question) -> QuestionLabels {
    let p = Payload { schema_version: 2, body: Body::Question(q.clone()) };
    label_findings(&p).into_iter().map(|f| (f.part, LabelChoice::Keep)).collect()
}

/// Matching with distractors up to (and past) the cap, shared rights included: whatever
/// `validate` accepts, `strip` serves a form that `validate` accepts again.
#[test]
fn stripped_matching_always_revalidates() {
    for case in 0..CASES {
        let mut g = Gen::new(case + 2_000_000);
        let np = 2 + g.upto(48);
        let nd = g.upto(np + 30).min(76);
        let mut pairs: Vec<Pair> = (0..np).map(|i| Pair { left: format!("l{i}"), right: format!("r{i}") }).collect();
        for i in 1..np {
            if g.rng.below(5) == 0 {
                pairs[i].right = pairs[g.rng.below(i)].right.clone();
            }
        }
        let distractors: Vec<String> = (0..nd).map(|i| if g.rng.below(6) == 0 { format!("r{}", g.rng.below(np)) } else { format!("d{i}") }).collect();
        let q = question("Match", Kind::Matching { pairs, distractors });
        let Ok(v) = validate(&q_payload_v1(&q), &Limits::default(), &Structural) else { continue };
        let s = strip_with_seed(v.payload, &StripOptions::default(), seed(case as u8)).unwrap();
        let back = validate(&s.payload.to_json(), &Limits::default(), &Structural);
        assert!(back.is_ok(), "case {case} np {np} nd {nd}: {:?}", back.err());
    }
}

/// Decoration on choices: strip refuses exactly when some correct choice looks unlike every wrong
/// one, so no served form singles out the correct index by formatting.
#[test]
fn no_served_form_carries_the_key_in_formatting() {
    const DECOS: [(&str, &str); 6] = [("", ""), ("**", "**"), ("*", "*"), ("\u{2713} ", ""), ("", " (correct)"), ("<b>", "</b>")];
    let mut refused = 0;
    for case in 0..CASES {
        let mut g = Gen::new(case + 3_000_000);
        let n = 2 + g.upto(5);
        let uniform = g.coin();
        let base = DECOS[g.rng.below(DECOS.len())];
        let answers: Vec<Answer> = (0..n)
            .map(|i| {
                let (a, z) = if uniform || g.rng.below(3) != 0 { base } else { DECOS[g.rng.below(DECOS.len())] };
                Answer { text: format!("{a}planet {i}{z}"), correct: i == 0 || g.rng.below(4) == 0, locked: false }
            })
            .collect();
        let leaky = share_core::labels::key_in_formatting(&answers.iter().map(|a| (a.text.as_str(), a.correct)).collect::<Vec<_>>());
        let q = question("Pick", Kind::Mc { answers });
        let v = validate(&q_payload_v1(&q), &Limits::default(), &Structural).unwrap();
        match strip_with_seed(v.payload, &StripOptions::default(), seed(case as u8)) {
            Ok(_) => assert!(leaky.is_empty(), "case {case}"),
            Err(e) => {
                refused += 1;
                assert!(!leaky.is_empty() && e.iter().all(|e| e.code == "key_in_formatting"), "case {case}: {e:?}");
            }
        }
    }
    assert!(refused > 0);
}
