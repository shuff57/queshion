//! `strip`: the stripped (keyless) form of docs/shared-pool.md ("The stripped (keyless) form").
//!
//! Applied by the server only, once per `hide_key` publish. **The shuffle seed is drawn inside
//! this crate** ([`strip`]: `getrandom`, which is `crypto.getRandomValues` in the Worker's wasm and
//! the OS generator natively); the caller cannot choose it, so a caller cannot replay a chosen
//! order. One ChaCha20 stream per payload: the questions of a bank are shuffled one after another
//! from it, so each gets an independent permutation. [`strip_with_seed`] takes an explicit seed
//! for tests and fixtures only; the wasm build exports it only with the `test-seed` feature.
//!
//! | Kind | Stripped form |
//! |---|---|
//! | `Mc` (single or multiple answer) | every `correct` set to `false`; labels removed **only** where the author chose `Strip` (`labels`); the unlocked answers shuffled once (uniform permutation; `locked` answers keep their slot) **unless** the question keeps order-revealing labels (`Keep`), names sibling choices ("Both A and B", an unlocked "All of the above"), or has fewer than two unlocked answers: then **nothing in it moves** and its position is in `needs_order_ack` with the reason in `order_reasons` |
//! | `Tf` | `correct = false` |
//! | `Matching` | `pairs[].left` kept in order; `pairs[].right = ""`; `distractors` = the **word bank**: every non-empty right item and distractor, de-duplicated by exact text (as Moodle's match question does), in one random order. See `import` for how a client turns this into its own model. |
//! | `Blank` | every `accept` list emptied; the number of blanks is kept |
//! | `Short` | `sample = ""` |
//! | `Multipart` | the rules above for each part (`Mc`, `Tf`, `Short`) |
//! | every question | `explanation` removed; `figure` **always** removed; `image` removed unless its position is in `image_ok`; `published` removed (author-local state); `origin.served_sha256` and `origin.match_hash` removed; `keyless = true` |
//! | the bank | `published` removed; `origin.served_sha256` and `origin.match_hash` removed |
//!
//! Kept: `id`, `stem`, part stems, answer and left-item texts, `locked`, `topics`, `points`,
//! `difficulty`, `created`, `modified`, the rest of `origin`, and the bank's name and details.
//! The payload is written as schema v2 (`SCHEMA_STRIPPED`), the first version with `keyless`.
use crate::labels::{self, Analysis, LabelChoice, Preview, SiblingRef};
use crate::text;
use crate::validate::{MAX_ERRORS, ValidationError};
use crate::wire::*;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The shuffle source: ChaCha20 keyed by a 32-byte seed.
pub struct ShuffleRng(ChaCha20Rng);

impl ShuffleRng {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        ShuffleRng(ChaCha20Rng::from_seed(seed))
    }
    /// Seeded from the platform's cryptographic generator.
    pub fn from_entropy() -> Result<Self, getrandom::Error> {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed)?;
        Ok(Self::from_seed(seed))
    }
    /// The next raw 64-bit output (tests compare `below` against a reference built on it).
    pub fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }
    /// Uniform in `0..n` (`n >= 1`), without modulo bias.
    pub fn below(&mut self, n: usize) -> usize {
        let n = n.max(1) as u64;
        // 2^64 mod n: values below it would make the low residues more likely.
        let threshold = n.wrapping_neg() % n;
        loop {
            let x = self.0.next_u64();
            if x >= threshold {
                return (x % n) as usize;
            }
        }
    }
    /// Fisher-Yates.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

/// A question (and optionally a part of a multipart question) by position in the payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub question: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
}

/// The author's label decision for one question or part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelDecision {
    pub question: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    pub choice: LabelChoice,
}

/// The decisions of one question, by part (`None` = the question's own choices).
pub type QuestionLabels = BTreeMap<Option<usize>, LabelChoice>;

/// A label finding for the author's preview (see `labels`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LabelFinding {
    pub question: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    pub order_revealing: bool,
    pub strip_allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<LabelChoice>,
    /// The before/after preview (empty in strip results; capped in `label_findings`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Preview>,
    /// The preview was left out to stay within the preview budget.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

/// Why a question's order is preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderReason {
    /// Fewer than two unlocked answers.
    FewUnlocked,
    /// Order-revealing labels the author chose to keep.
    LabelsKept,
    /// A choice names its siblings.
    SiblingRefs,
    /// The author set `keep_order` on the question.
    AuthorKeepOrder,
}

/// The served order of one shuffled question, part or word bank.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ServedOrder {
    pub question: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    /// A Matching word bank (indices into the author's right column), not Mc choices.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub matching: bool,
    pub order: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct OrderNote {
    pub question: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    pub reason: OrderReason,
}

#[derive(Clone, Debug, Default)]
pub struct StripOptions {
    /// Question positions whose `image` the author ticked "does not reveal the answer".
    /// A position outside the payload is an error.
    pub image_ok: Vec<usize>,
    /// The author's label decisions (required for every finding).
    pub labels: Vec<LabelDecision>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stripped {
    pub payload: Payload,
    /// Mc questions or parts whose order is preserved: the publish must carry the author's
    /// acknowledgement for each, or be refused. The reasons are in `order_reasons`.
    pub needs_order_ack: Vec<Pos>,
    pub order_reasons: Vec<OrderNote>,
    /// Positions where labels were removed (decision `Strip`).
    pub labels_removed: Vec<Pos>,
    /// Every label finding with its decision: store these with the publish (`acks_json`), they
    /// are what `match_hash_with` needs to hash the author's full body consistently.
    pub label_findings: Vec<LabelFinding>,
    /// Every Mc question or part, and every Matching word bank, that **was shuffled**, with its
    /// served order (`order[k]` = the author's index now at position `k`), for the author's review.
    pub served_orders: Vec<ServedOrder>,
    pub images_kept: Vec<usize>,
    pub images_removed: Vec<usize>,
    pub figures_removed: Vec<usize>,
}

enum Order<'a> {
    Shuffle(&'a mut ShuffleRng),
    Sort,
}

struct Ctx<'a, 'r> {
    order: Order<'r>,
    labels: &'a BTreeMap<(usize, Option<usize>), LabelChoice>,
    out: Stripped,
    errors: Vec<ValidationError>,
    /// The current question's `keep_order`.
    keep_order: bool,
}

fn label_path(at: Pos) -> String {
    match at.part {
        Some(p) => format!("labels[question={},part={p}]", at.question),
        None => format!("labels[question={}]", at.question),
    }
}

fn answers_path(at: Pos) -> String {
    match at.part {
        Some(p) => format!("question[{}].kind.Multipart.parts[{p}].kind.Mc.answers", at.question),
        None => format!("question[{}].kind.Mc.answers", at.question),
    }
}

fn texts(answers: &[Answer]) -> Vec<&str> {
    answers.iter().map(|a| a.text.as_str()).collect()
}

/// What may happen to one Mc's choices given its labels, its decision and its sibling refs.
struct McPlan {
    analysis: Option<Analysis>,
    strip: bool,
    keep_order: Option<OrderReason>,
}

fn plan_mc(answers: &[Answer], decision: Option<LabelChoice>, author_keep: bool) -> Result<McPlan, &'static str> {
    let analysis = labels::analyze(&texts(answers));
    let mut strip = false;
    let mut keep_order = None;
    if let Some(a) = &analysis {
        match decision {
            None => return Err("labels_need_decision"),
            Some(LabelChoice::Strip) if !a.strip_allowed => return Err("labels_strip_refused"),
            Some(LabelChoice::Strip) => strip = true,
            Some(LabelChoice::Keep) => {
                if a.order_revealing {
                    keep_order = Some(OrderReason::LabelsKept);
                }
            }
        }
    }
    let sibling = answers.iter().any(|a| match labels::sibling_ref(&a.text) {
        Some(SiblingRef::Labels) => true,
        Some(SiblingRef::Above) => !a.locked,
        None => false,
    });
    if keep_order.is_none() && sibling {
        keep_order = Some(OrderReason::SiblingRefs);
    }
    if author_keep {
        keep_order = Some(OrderReason::AuthorKeepOrder);
    }
    Ok(McPlan { analysis, strip, keep_order })
}

fn strip_mc(answers: &mut [Answer], at: Pos, cx: &mut Ctx) {
    let decision = cx.labels.get(&(at.question, at.part)).copied();
    // A key carried by formatting ("**Venus**", "Venus (correct)") survives every shuffle.
    let fmt = labels::key_in_formatting(&answers.iter().map(|a| (a.text.as_str(), a.correct)).collect::<Vec<_>>());
    for i in fmt {
        if cx.errors.len() < MAX_ERRORS {
            cx.errors.push(ValidationError::new("key_in_formatting", &format!("{}[{i}]", answers_path(at))));
        }
    }
    let plan = match plan_mc(answers, decision, cx.keep_order) {
        Ok(p) => p,
        Err(code) => {
            if cx.errors.len() < MAX_ERRORS {
                cx.errors.push(ValidationError::new(code, &label_path(at)));
            }
            return;
        }
    };
    if let Some(a) = &plan.analysis {
        // Decisions only: the previews were the author's, and copying them here is what made a
        // large bank's strip use too much memory.
        cx.out.label_findings.push(LabelFinding {
            question: at.question,
            part: at.part,
            order_revealing: a.order_revealing,
            strip_allowed: a.strip_allowed,
            decision,
            choices: Vec::new(),
            truncated: false,
        });
        if plan.strip {
            for (ans, &c) in answers.iter_mut().zip(&a.cut) {
                ans.text = labels::apply(&ans.text, c);
            }
            cx.out.labels_removed.push(at);
        }
    }
    for a in answers.iter_mut() {
        a.correct = false;
    }
    let slots: Vec<usize> = answers.iter().enumerate().filter(|(_, a)| !a.locked).map(|(i, _)| i).collect();
    let reason = plan.keep_order.or((slots.len() < 2 && !answers.is_empty()).then_some(OrderReason::FewUnlocked));
    if let Some(r) = reason {
        cx.out.needs_order_ack.push(at);
        cx.out.order_reasons.push(OrderNote { question: at.question, part: at.part, reason: r });
        if matches!(cx.order, Order::Shuffle(_)) {
            return;
        }
    }
    match &mut cx.order {
        Order::Shuffle(rng) => {
            let mut moving: Vec<(usize, Answer)> = slots.iter().map(|&i| (i, answers[i].clone())).collect();
            rng.shuffle(&mut moving);
            let mut order: Vec<usize> = (0..answers.len()).collect();
            for (slot, (from, a)) in slots.into_iter().zip(moving) {
                order[slot] = from;
                answers[slot] = a;
            }
            cx.out.served_orders.push(ServedOrder { question: at.question, part: at.part, matching: false, order });
        }
        Order::Sort => answers.sort_by_cached_key(|a| text::hash_form(&a.text)),
    }
}

fn word_bank(pairs: &mut [Pair], distractors: &mut Vec<String>, at: usize, cx: &mut Ctx) {
    let mut bank: Vec<(usize, String)> = Vec::with_capacity(pairs.len() + distractors.len());
    let mut seen = BTreeSet::new();
    for s in pairs.iter_mut().map(|p| std::mem::take(&mut p.right)).chain(distractors.drain(..)) {
        if text::has_visible(&s) && seen.insert(s.clone()) {
            bank.push((bank.len(), s));
        }
    }
    match &mut cx.order {
        Order::Shuffle(rng) => {
            rng.shuffle(&mut bank);
            // `order[k]` indexes the author's right column (`import::right_column` of the full question).
            cx.out.served_orders.push(ServedOrder { question: at, part: None, matching: true, order: bank.iter().map(|b| b.0).collect() });
        }
        Order::Sort => bank.sort_by_cached_key(|s| text::hash_form(&s.1)),
    }
    *distractors = bank.into_iter().map(|b| b.1).collect();
}

fn clear_origin(o: &mut Option<Origin>) {
    if let Some(o) = o.as_mut() {
        o.served_sha256.clear();
        o.match_hash.clear();
    }
}

fn strip_one(mut s: Question, image_ok: bool, pos: usize, cx: &mut Ctx) -> Question {
    s.explanation = None;
    if s.figure.take().is_some() {
        cx.out.figures_removed.push(pos);
    }
    if s.image.is_some() {
        if image_ok {
            cx.out.images_kept.push(pos);
        } else {
            s.image = None;
            cx.out.images_removed.push(pos);
        }
    }
    s.published = None;
    clear_origin(&mut s.origin);
    s.keyless = true;
    cx.keep_order = s.keep_order;
    let at = |part| Pos { question: pos, part };
    match &mut s.kind {
        Kind::Mc { answers } => strip_mc(answers, at(None), cx),
        Kind::Tf { correct } => *correct = false,
        Kind::Matching { pairs, distractors } => word_bank(pairs, distractors, pos, cx),
        Kind::Blank { accept } => {
            for a in accept.iter_mut() {
                a.clear();
            }
        }
        Kind::Short { sample } => sample.clear(),
        Kind::Multipart { parts } => {
            for (pi, p) in parts.iter_mut().enumerate() {
                match &mut p.kind {
                    PartKind::Mc { answers } => strip_mc(answers, at(Some(pi)), cx),
                    PartKind::Tf { correct } => *correct = false,
                    PartKind::Short { sample } => sample.clear(),
                }
            }
        }
    }
    s
}

fn empty_report(payload: Payload) -> Stripped {
    Stripped {
        payload,
        needs_order_ack: Vec::new(),
        order_reasons: Vec::new(),
        labels_removed: Vec::new(),
        label_findings: Vec::new(),
        served_orders: Vec::new(),
        images_kept: Vec::new(),
        images_removed: Vec::new(),
        figures_removed: Vec::new(),
    }
}

fn check_options(p: &Payload, opts: &StripOptions) -> Result<(), Vec<ValidationError>> {
    let n = p.questions().len();
    let mut bad: Vec<ValidationError> = Vec::new();
    for (k, i) in opts.image_ok.iter().enumerate() {
        if *i >= n {
            if bad.len() >= MAX_ERRORS {
                break;
            }
            bad.push(ValidationError { limit: Some(n as u64), ..ValidationError::new("image_ok_out_of_range", &format!("image_ok[{k}]")) });
        }
    }
    for (k, d) in opts.labels.iter().enumerate() {
        if d.question >= n && bad.len() < MAX_ERRORS {
            bad.push(ValidationError { limit: Some(n as u64), ..ValidationError::new("labels_out_of_range", &format!("labels[{k}]")) });
        }
    }
    if bad.is_empty() { Ok(()) } else { Err(bad) }
}

fn decisions(opts: &StripOptions) -> BTreeMap<(usize, Option<usize>), LabelChoice> {
    opts.labels.iter().map(|d| ((d.question, d.part), d.choice)).collect()
}

/// Strips `p` (consumed: no second copy of a large bank is made).
fn strip_owned(p: Payload, opts: &StripOptions, order: Order) -> Result<Stripped, Vec<ValidationError>> {
    check_options(&p, opts)?;
    let de = decision_errors(&p, &opts.labels);
    if !de.is_empty() {
        return Err(de);
    }
    let image_ok: BTreeSet<usize> = opts.image_ok.iter().copied().collect();
    let labels = decisions(opts);
    let schema_version = p.schema_version.max(SCHEMA_STRIPPED);
    let mut cx = Ctx { order, labels: &labels, out: empty_report(Payload { schema_version, body: Body::Question(placeholder()) }), errors: Vec::new(), keep_order: false };
    let body = match p.body {
        Body::Bank(mut b) => {
            let qs = std::mem::take(&mut b.questions);
            b.questions = qs.into_iter().enumerate().map(|(i, q)| strip_one(q, image_ok.contains(&i), i, &mut cx)).collect();
            b.published = None;
            clear_origin(&mut b.origin);
            Body::Bank(b)
        }
        Body::Question(q) => Body::Question(strip_one(q, image_ok.contains(&0), 0, &mut cx)),
    };
    if !cx.errors.is_empty() {
        return Err(cx.errors);
    }
    let mut out = cx.out;
    out.payload.body = body;
    out.needs_order_ack.sort();
    out.needs_order_ack.dedup();
    out.order_reasons.sort();
    Ok(out)
}

fn placeholder() -> Question {
    Question {
        id: String::new(),
        stem: String::new(),
        kind: Kind::Tf { correct: false },
        points: None,
        difficulty: None,
        topics: vec![],
        image: None,
        explanation: None,
        figure: None,
        created: 0,
        modified: 0,
        keyless: true,
        keep_order: false,
        origin: None,
        published: None,
    }
}

/// The stripped form of a validated payload, shuffled with a seed drawn from the platform's
/// cryptographic generator. This is the production entry point. Refused with
/// `labels_need_decision` when a label finding has no decision (see [`label_findings`]).
pub fn strip(p: Payload, opts: &StripOptions) -> Result<Stripped, Vec<ValidationError>> {
    let mut rng = ShuffleRng::from_entropy().map_err(|_| vec![ValidationError::new("rng_unavailable", "")])?;
    strip_owned(p, opts, Order::Shuffle(&mut rng))
}

/// `strip` with an explicit seed: **tests and fixtures only** (a caller-chosen seed is a
/// caller-chosen order). Not exported to wasm unless built with the `test-seed` feature.
pub fn strip_with_seed(p: Payload, opts: &StripOptions, seed: [u8; 32]) -> Result<Stripped, Vec<ValidationError>> {
    let mut rng = ShuffleRng::from_seed(seed);
    strip_owned(p, opts, Order::Shuffle(&mut rng))
}

/// Total preview text `label_findings` returns before it leaves the rest out (`truncated`).
pub const PREVIEW_BUDGET: usize = 256 * 1024;

/// Every label finding of a payload, undecided, for the author's preview before publishing.
/// Previews stop after [`PREVIEW_BUDGET`] bytes of text; later findings are listed with
/// `truncated: true` and no preview (the app computes the full preview locally with this crate).
pub fn label_findings(p: &Payload) -> Vec<LabelFinding> {
    findings(p, Some(PREVIEW_BUDGET))
}

/// The findings without any preview: what the publish checks need.
pub fn label_finding_flags(p: &Payload) -> Vec<LabelFinding> {
    findings(p, None)
}

fn findings(p: &Payload, mut budget: Option<usize>) -> Vec<LabelFinding> {
    let mut out = Vec::new();
    let mut push = |q: usize, part: Option<usize>, answers: &[Answer]| {
        let t = texts(answers);
        if let Some(a) = labels::analyze(&t) {
            let size: usize = t.iter().map(|x| 2 * x.len()).sum();
            let (choices, truncated) = match budget.as_mut() {
                None => (Vec::new(), false),
                Some(b) if size <= *b => {
                    *b -= size;
                    (a.previews(&t), false)
                }
                Some(_) => (Vec::new(), true),
            };
            out.push(LabelFinding { question: q, part, order_revealing: a.order_revealing, strip_allowed: a.strip_allowed, decision: None, choices, truncated });
        }
    };
    for (i, q) in p.questions().iter().enumerate() {
        match &q.kind {
            Kind::Mc { answers } => push(i, None, answers),
            Kind::Multipart { parts } => {
                for (pi, part) in parts.iter().enumerate() {
                    if let PartKind::Mc { answers } = &part.kind {
                        push(i, Some(pi), answers);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The label decisions checked against the findings (without previews): a finding with no
/// decision (`labels_need_decision`), two decisions for one question or part
/// (`labels_duplicate`), a decision with no finding (`labels_unused`), or Strip where it is not
/// allowed (`labels_strip_refused`). `validate_for_publish` and `strip` both use this.
pub fn decision_errors(p: &Payload, decisions: &[LabelDecision]) -> Vec<ValidationError> {
    let found = label_finding_flags(p);
    let mut errors = Vec::new();
    let mut push = |code, at: Pos| {
        if errors.len() < MAX_ERRORS {
            errors.push(ValidationError::new(code, &label_path(at)));
        }
    };
    let mut seen = BTreeSet::new();
    for d in decisions {
        let at = Pos { question: d.question, part: d.part };
        if !seen.insert((d.question, d.part)) {
            push("labels_duplicate", at);
        } else if !found.iter().any(|f| f.question == d.question && f.part == d.part) {
            push("labels_unused", at);
        }
    }
    for f in &found {
        let at = Pos { question: f.question, part: f.part };
        match decisions.iter().find(|d| d.question == f.question && d.part == f.part).map(|d| d.choice) {
            None => push("labels_need_decision", at),
            Some(LabelChoice::Strip) if !f.strip_allowed => push("labels_strip_refused", at),
            _ => {}
        }
    }
    errors
}

/// One question's decisions out of a payload's.
pub fn question_labels(all: &[LabelDecision], question: usize) -> QuestionLabels {
    all.iter().filter(|d| d.question == question).map(|d| (d.part, d.choice)).collect()
}

/// One question's stripped form with a given generator and decisions (tests).
pub fn strip_question(q: &Question, image_ok: bool, labels: &QuestionLabels, rng: &mut ShuffleRng) -> Result<Question, Vec<ValidationError>> {
    let map: BTreeMap<(usize, Option<usize>), LabelChoice> = labels.iter().map(|(p, c)| ((0, *p), *c)).collect();
    let mut cx = Ctx { order: Order::Shuffle(rng), labels: &map, out: empty_report(Payload { schema_version: SCHEMA_STRIPPED, body: Body::Question(placeholder()) }), errors: Vec::new(), keep_order: false };
    let s = strip_one(q.clone(), image_ok, 0, &mut cx);
    if cx.errors.is_empty() { Ok(s) } else { Err(cx.errors) }
}

/// The spec's `strip_without_shuffle`: the stripped form with every unordered list **sorted**
/// instead of shuffled, labels removed only where `labels` says Strip (and Strip is allowed).
/// `match_hash` covers exactly what survives this (see `hash`). Undecided findings keep their text.
pub fn strip_without_shuffle(q: &Question, labels: &QuestionLabels) -> Question {
    // Undecided findings are treated as Keep here (hashing never fails).
    let mut map: BTreeMap<(usize, Option<usize>), LabelChoice> = BTreeMap::new();
    let mut fill = |part: Option<usize>, answers: &[Answer]| {
        if let Some(a) = labels::analyze(&texts(answers)) {
            let c = match labels.get(&part) {
                Some(LabelChoice::Strip) if a.strip_allowed => LabelChoice::Strip,
                _ => LabelChoice::Keep,
            };
            map.insert((0, part), c);
        }
    };
    match &q.kind {
        Kind::Mc { answers } => fill(None, answers),
        Kind::Multipart { parts } => {
            for (pi, p) in parts.iter().enumerate() {
                if let PartKind::Mc { answers } = &p.kind {
                    fill(Some(pi), answers);
                }
            }
        }
        _ => {}
    }
    let mut cx = Ctx { order: Order::Sort, labels: &map, out: empty_report(Payload { schema_version: SCHEMA_STRIPPED, body: Body::Question(placeholder()) }), errors: Vec::new(), keep_order: false };
    strip_one(q.clone(), false, 0, &mut cx)
}
