//! Pure bank logic shared by the bank editor, the question editor and the import preview:
//! topic counts / rename / merge / delete across a bank's questions, topic autocomplete,
//! "used in N exams" lookups, per-type counts and `[picture]` placeholders in stems.
//! No egui here, so everything is unit-tested below.

use std::collections::HashMap;

use crate::model::{Exam, Id, Kind, PartKind, QKind, Question};

// ---------- topics ----------

/// Every topic in `qs` with the number of questions that carry it, sorted by name
/// (case-insensitive). Topics are trimmed; a question counts once per topic.
pub fn topic_counts(qs: &[Question]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for q in qs {
        let mut seen: Vec<&str> = Vec::new();
        for t in q.topics.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
            if seen.contains(&t) {
                continue;
            }
            seen.push(t);
            match out.iter_mut().find(|(n, _)| n == t) {
                Some((_, c)) => *c += 1,
                None => out.push((t.to_string(), 1)),
            }
        }
    }
    out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then_with(|| a.0.cmp(&b.0)));
    out
}

/// The existing spelling of `typed` in `pool` when one matches ignoring case and outer
/// space, else `typed` trimmed. Keeps "algebra" from becoming a second "Algebra".
pub fn canonical_topic(pool: &[String], typed: &str) -> String {
    let t = typed.trim();
    pool.iter()
        .find(|p| p.trim().eq_ignore_ascii_case(t) || p.trim().to_lowercase() == t.to_lowercase())
        .map(|p| p.trim().to_string())
        .unwrap_or_else(|| t.to_string())
}

/// Renames topic `from` to `to` on every question. When a question already has `to`
/// the two merge into one. Returns how many questions changed. An empty `to` is a no-op.
pub fn rename_topic(qs: &mut [Question], from: &str, to: &str) -> usize {
    let (from, to) = (from.trim(), to.trim());
    if to.is_empty() || from.is_empty() || from == to {
        return 0;
    }
    let mut changed = 0;
    for q in qs.iter_mut() {
        if !q.topics.iter().any(|t| t.trim() == from) {
            continue;
        }
        let mut next: Vec<String> = Vec::with_capacity(q.topics.len());
        for t in &q.topics {
            let t = if t.trim() == from { to } else { t.trim() };
            if !t.is_empty() && !next.iter().any(|x| x == t) {
                next.push(t.to_string());
            }
        }
        q.topics = next;
        changed += 1;
    }
    changed
}

/// Merges every topic in `from` into `into` (rename of each). Returns questions changed.
pub fn merge_topics(qs: &mut [Question], from: &[String], into: &str) -> usize {
    let mut touched: Vec<usize> = Vec::new();
    for f in from {
        for (i, q) in qs.iter().enumerate() {
            if q.topics.iter().any(|t| t.trim() == f.trim()) && !touched.contains(&i) {
                touched.push(i);
            }
        }
        rename_topic(qs, f, into);
    }
    touched.len()
}

/// Removes `name` from every question. Returns how many questions changed.
pub fn delete_topic(qs: &mut [Question], name: &str) -> usize {
    let name = name.trim();
    let mut changed = 0;
    for q in qs.iter_mut() {
        let before = q.topics.len();
        q.topics.retain(|t| t.trim() != name);
        if q.topics.len() != before {
            changed += 1;
        }
    }
    changed
}

/// Autocomplete for a topic field: pool entries not in `exclude` that start with `typed`
/// (ignoring case) first, then those containing it; at most `limit`. Empty `typed` lists
/// the pool in order.
pub fn suggest_topics(pool: &[String], typed: &str, exclude: &[String], limit: usize) -> Vec<String> {
    let t = typed.trim().to_lowercase();
    let free = pool.iter().filter(|p| !exclude.iter().any(|e| e.trim() == p.trim()));
    if t.is_empty() {
        return free.take(limit).cloned().collect();
    }
    let (mut pre, mut mid): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for p in free {
        let l = p.to_lowercase();
        if l == t {
            continue;
        }
        if l.starts_with(&t) {
            pre.push(p.clone());
        } else if l.contains(&t) {
            mid.push(p.clone());
        }
    }
    pre.extend(mid);
    pre.truncate(limit);
    pre
}

// ---------- usage in exams ----------

/// Exams that place at least one question of `bank_id`, in store order.
pub fn exams_using_bank<'a>(exams: &'a [Exam], bank_id: &str) -> Vec<&'a Exam> {
    exams.iter().filter(|e| e.items.iter().any(|it| it.bank_id == bank_id)).collect()
}

/// Exams that place question `qid` of `bank_id`, in store order.
pub fn exams_using_question<'a>(exams: &'a [Exam], bank_id: &str, qid: &str) -> Vec<&'a Exam> {
    exams
        .iter()
        .filter(|e| e.items.iter().any(|it| it.bank_id == bank_id && it.question_id == qid))
        .collect()
}

/// question id -> number of distinct exams using it, for one bank (one pass per frame).
pub fn question_usage(exams: &[Exam], bank_id: &str) -> HashMap<Id, usize> {
    let mut m: HashMap<Id, usize> = HashMap::new();
    for e in exams {
        let mut seen: Vec<&str> = Vec::new();
        for it in e.items.iter().filter(|it| it.bank_id == bank_id) {
            if !seen.contains(&it.question_id.as_str()) {
                seen.push(&it.question_id);
                *m.entry(it.question_id.clone()).or_default() += 1;
            }
        }
    }
    m
}

/// "Used in 2 exams" / "Not in any exam".
pub fn usage_label(n: usize) -> String {
    match n {
        0 => "Not in any exam".to_string(),
        1 => "Used in 1 exam".to_string(),
        n => format!("Used in {n} exams"),
    }
}

// ---------- types ----------

/// Display order of question types (Test Parrot's: MC, T/F, Matching, Fill, Short, Multipart).
pub const KINDS: [QKind; 6] = [QKind::Mc, QKind::Tf, QKind::Matching, QKind::Blank, QKind::Short, QKind::Multipart];

/// Questions per type, indexed like `KINDS`.
pub fn type_counts<'a>(qs: impl IntoIterator<Item = &'a Question>) -> [usize; 6] {
    let mut c = [0usize; 6];
    for q in qs {
        let k = q.kind.qkind();
        if let Some(i) = KINDS.iter().position(|x| *x == k) {
            c[i] += 1;
        }
    }
    c
}

// ---------- picture placeholders ----------

/// A stem split around `[picture]` / `[picture: alt]` markers (what importers write when
/// the image itself did not come along).
#[derive(Clone, Debug, PartialEq)]
pub enum StemPart {
    Text(String),
    Picture(Option<String>),
}

/// Splits `stem` into text runs and picture placeholders, in order. Text runs are kept
/// verbatim except that the line breaks hugging a marker are dropped; empty runs vanish.
pub fn split_pictures(stem: &str) -> Vec<StemPart> {
    let mut out = Vec::new();
    let mut rest = stem;
    while let Some(at) = find_marker(rest) {
        let (before, after) = rest.split_at(at);
        let close = after.find(']').expect("find_marker checks the bracket");
        let inner = &after[1..close];
        let alt = inner.strip_prefix("picture").unwrap_or("").trim_start_matches(':').trim();
        push_text(&mut out, before);
        out.push(StemPart::Picture((!alt.is_empty()).then(|| alt.to_string())));
        rest = &after[close + 1..];
    }
    push_text(&mut out, rest);
    out
}

fn find_marker(s: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = s[from..].find("[picture") {
        let at = from + i;
        let tail = &s[at + "[picture".len()..];
        let ok = tail.starts_with(']') || (tail.starts_with(':') && tail.find(']').is_some_and(|j| !tail[..j].contains('\n')));
        if ok {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

fn push_text(out: &mut Vec<StemPart>, s: &str) {
    let t = s.trim_matches(|c| c == '\n' || c == '\r');
    if !t.trim().is_empty() {
        out.push(StemPart::Text(t.to_string()));
    }
}

/// Picture placeholders still needing a picture in one question: every marker in its
/// stem, answers, matching pairs, short-answer model answers and multipart parts (stems,
/// choices and model answers), less the one its attached picture already covers.
pub fn pictures_needed(q: &Question) -> usize {
    let c = picture_markers(q);
    if q.image.is_some() { c.saturating_sub(1) } else { c }
}

/// Every `[picture]` marker anywhere in a question, before the attached picture is counted.
pub fn picture_markers(q: &Question) -> usize {
    let n = |s: &str| split_pictures(s).iter().filter(|p| matches!(p, StemPart::Picture(_))).count();
    let mut c = n(&q.stem);
    match &q.kind {
        Kind::Mc { answers } => c += answers.iter().map(|a| n(&a.text)).sum::<usize>(),
        Kind::Short { sample } => c += n(sample),
        Kind::Matching { pairs, distractors } => {
            c += pairs.iter().map(|p| n(&p.left) + n(&p.right)).sum::<usize>();
            c += distractors.iter().map(|d| n(d)).sum::<usize>();
        }
        Kind::Multipart { parts } => {
            for p in parts {
                c += n(&p.stem);
                match &p.kind {
                    PartKind::Mc { answers } => c += answers.iter().map(|a| n(&a.text)).sum::<usize>(),
                    PartKind::Short { sample } => c += n(sample),
                    PartKind::Tf { .. } => {}
                }
            }
        }
        Kind::Tf { .. } | Kind::Blank { .. } => {}
    }
    c
}

// ---------- resolving pictures ----------

/// Where a `[picture]` marker sits in a question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicField {
    Stem,
    Choice(usize),
    Sample,
    MatchLeft(usize),
    MatchRight(usize),
    Distractor(usize),
    PartStem(usize),
    PartChoice(usize, usize),
    PartSample(usize),
}

/// One "picture needed" placeholder of a bank: question index, field, which marker of that
/// field (0-based), its description, and whether the question's own picture can fill it
/// (the first stem marker of a question with no picture yet). Others live in answer keys and
/// parts, which have no picture of their own: they can be described in words instead.
#[derive(Clone, Debug, PartialEq)]
pub struct PictureSlot {
    pub qi: usize,
    pub field: PicField,
    pub nth: usize,
    pub alt: Option<String>,
    pub attachable: bool,
}

fn letter(i: usize) -> char {
    char::from(b'a' + (i % 26) as u8)
}

impl PictureSlot {
    /// "Question 4, Part a, Suggested Answer" (Test Parrot's wording); `n` = the question's
    /// number on the bank page.
    pub fn label(&self, n: usize) -> String {
        let tail = match self.field {
            PicField::Stem => String::new(),
            PicField::Choice(i) => format!(", Choice {}", letter(i).to_ascii_uppercase()),
            PicField::Sample => ", Suggested Answer".to_string(),
            PicField::MatchLeft(i) => format!(", Match {}", i + 1),
            PicField::MatchRight(i) => format!(", Match {} (right)", i + 1),
            PicField::Distractor(i) => format!(", Extra choice {}", i + 1),
            PicField::PartStem(p) => format!(", Part {}", letter(p)),
            PicField::PartChoice(p, i) => format!(", Part {}, Choice {}", letter(p), letter(i).to_ascii_uppercase()),
            PicField::PartSample(p) => format!(", Part {}, Suggested Answer", letter(p)),
        };
        format!("Question {n}{tail}")
    }
}

/// The text of one field of a question (None when the field is not there).
pub fn field_text(q: &Question, f: PicField) -> Option<&String> {
    match (f, &q.kind) {
        (PicField::Stem, _) => Some(&q.stem),
        (PicField::Choice(i), Kind::Mc { answers }) => answers.get(i).map(|a| &a.text),
        (PicField::Sample, Kind::Short { sample }) => Some(sample),
        (PicField::MatchLeft(i), Kind::Matching { pairs, .. }) => pairs.get(i).map(|p| &p.left),
        (PicField::MatchRight(i), Kind::Matching { pairs, .. }) => pairs.get(i).map(|p| &p.right),
        (PicField::Distractor(i), Kind::Matching { distractors, .. }) => distractors.get(i),
        (PicField::PartStem(p), Kind::Multipart { parts }) => parts.get(p).map(|x| &x.stem),
        (PicField::PartChoice(p, i), Kind::Multipart { parts }) => match parts.get(p).map(|x| &x.kind) {
            Some(PartKind::Mc { answers }) => answers.get(i).map(|a| &a.text),
            _ => None,
        },
        (PicField::PartSample(p), Kind::Multipart { parts }) => match parts.get(p).map(|x| &x.kind) {
            Some(PartKind::Short { sample }) => Some(sample),
            _ => None,
        },
        _ => None,
    }
}

/// Mutable access to one field's text.
pub fn field_text_mut(q: &mut Question, f: PicField) -> Option<&mut String> {
    match (f, &mut q.kind) {
        (PicField::Stem, _) => Some(&mut q.stem),
        (PicField::Choice(i), Kind::Mc { answers }) => answers.get_mut(i).map(|a| &mut a.text),
        (PicField::Sample, Kind::Short { sample }) => Some(sample),
        (PicField::MatchLeft(i), Kind::Matching { pairs, .. }) => pairs.get_mut(i).map(|p| &mut p.left),
        (PicField::MatchRight(i), Kind::Matching { pairs, .. }) => pairs.get_mut(i).map(|p| &mut p.right),
        (PicField::Distractor(i), Kind::Matching { distractors, .. }) => distractors.get_mut(i),
        (PicField::PartStem(p), Kind::Multipart { parts }) => parts.get_mut(p).map(|x| &mut x.stem),
        (PicField::PartChoice(p, i), Kind::Multipart { parts }) => match parts.get_mut(p).map(|x| &mut x.kind) {
            Some(PartKind::Mc { answers }) => answers.get_mut(i).map(|a| &mut a.text),
            _ => None,
        },
        (PicField::PartSample(p), Kind::Multipart { parts }) => match parts.get_mut(p).map(|x| &mut x.kind) {
            Some(PartKind::Short { sample }) => Some(sample),
            _ => None,
        },
        _ => None,
    }
}

/// Every field of a question that can hold markers, in reading order.
fn fields(q: &Question) -> Vec<PicField> {
    let mut f = vec![PicField::Stem];
    match &q.kind {
        Kind::Mc { answers } => f.extend((0..answers.len()).map(PicField::Choice)),
        Kind::Short { .. } => f.push(PicField::Sample),
        Kind::Matching { pairs, distractors } => {
            for i in 0..pairs.len() {
                f.push(PicField::MatchLeft(i));
                f.push(PicField::MatchRight(i));
            }
            f.extend((0..distractors.len()).map(PicField::Distractor));
        }
        Kind::Multipart { parts } => {
            for (p, part) in parts.iter().enumerate() {
                f.push(PicField::PartStem(p));
                match &part.kind {
                    PartKind::Mc { answers } => f.extend((0..answers.len()).map(|i| PicField::PartChoice(p, i))),
                    PartKind::Short { .. } => f.push(PicField::PartSample(p)),
                    PartKind::Tf { .. } => {}
                }
            }
        }
        Kind::Tf { .. } | Kind::Blank { .. } => {}
    }
    f
}

/// The pictures a bank still needs, in question order then reading order. Agrees with
/// [`pictures_needed`]: a question's attached picture covers its first marker.
pub fn picture_slots(qs: &[Question]) -> Vec<PictureSlot> {
    let mut out = Vec::new();
    for (qi, q) in qs.iter().enumerate() {
        let mut covered = q.image.is_some();
        for f in fields(q) {
            let Some(text) = field_text(q, f) else { continue };
            let alts: Vec<Option<String>> = split_pictures(text)
                .into_iter()
                .filter_map(|p| match p {
                    StemPart::Picture(a) => Some(a),
                    StemPart::Text(_) => None,
                })
                .collect();
            for (nth, alt) in alts.into_iter().enumerate() {
                if covered {
                    covered = false;
                    continue;
                }
                let attachable = q.image.is_none() && f == PicField::Stem && nth == 0;
                out.push(PictureSlot { qi, field: f, nth, alt, attachable });
            }
        }
    }
    out
}

/// `text` with its `nth` `[picture]` marker replaced by the words that describe it:
/// "[picture: Lewis structure of NH3]" -> "(Picture: Lewis structure of NH3)". Unchanged when
/// there is no such marker.
pub fn describe_marker(text: &str, nth: usize) -> String {
    let mut from = 0;
    let mut k = 0;
    while let Some(i) = find_marker(&text[from..]) {
        let at = from + i;
        let close = at + text[at..].find(']').expect("find_marker checks the bracket");
        if k == nth {
            let inner = &text[at + 1..close];
            let alt = inner.strip_prefix("picture").unwrap_or("").trim_start_matches(':').trim();
            let words = if alt.is_empty() { "(Picture)".to_string() } else { format!("(Picture: {alt})") };
            return format!("{}{}{}", &text[..at], words, &text[close + 1..]);
        }
        k += 1;
        from = close + 1;
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Answer, ExamItem, Part};

    fn q(id: &str, topics: &[&str]) -> Question {
        Question {
            id: id.to_string(),
            stem: format!("stem {id}"),
            kind: Kind::Short { sample: String::new() },
            points: None,
            difficulty: None,
            topics: topics.iter().map(|s| s.to_string()).collect(),
            image: None,
            explanation: None,
            figure: None,
            created: 0,
            modified: 0,
        }
    }

    fn exam(id: &str, items: &[(&str, &str)]) -> Exam {
        Exam { paper_style: Default::default(), margins: None,
            id: id.to_string(),
            name: format!("Exam {id}"),
            header: String::new(),
            title: None,
            items: items.iter().map(|(b, q)| ExamItem::new(b.to_string(), q.to_string())).collect(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 0,
            modified: 0,
        }
    }

    #[test]
    fn counts_trim_dedupe_and_sort() {
        let qs = vec![q("1", &["Ions", " bonds"]), q("2", &["bonds", "bonds "]), q("3", &["", "acids"])];
        assert_eq!(
            topic_counts(&qs),
            vec![("acids".to_string(), 1), ("bonds".to_string(), 2), ("Ions".to_string(), 1)]
        );
        assert!(topic_counts(&[q("x", &[])]).is_empty());
    }

    #[test]
    fn rename_changes_every_question_and_merges() {
        let mut qs = vec![q("1", &["ions", "bonds"]), q("2", &["ions"]), q("3", &["acids"])];
        assert_eq!(rename_topic(&mut qs, "ions", "bonds"), 2);
        assert_eq!(qs[0].topics, vec!["bonds"]);
        assert_eq!(qs[1].topics, vec!["bonds"]);
        assert_eq!(qs[2].topics, vec!["acids"]);
        assert_eq!(rename_topic(&mut qs, "bonds", "  "), 0, "empty target is a no-op");
        assert_eq!(rename_topic(&mut qs, "nope", "x"), 0);
        assert_eq!(topic_counts(&qs), vec![("acids".to_string(), 1), ("bonds".to_string(), 2)]);
    }

    #[test]
    fn merge_and_delete() {
        let mut qs = vec![q("1", &["a", "b"]), q("2", &["b"]), q("3", &["c"]), q("4", &["d"])];
        assert_eq!(merge_topics(&mut qs, &["a".into(), "b".into(), "c".into()], "abc"), 3);
        assert_eq!(qs[0].topics, vec!["abc"]);
        assert_eq!(qs[2].topics, vec!["abc"]);
        assert_eq!(delete_topic(&mut qs, "abc"), 3);
        assert!(qs[0].topics.is_empty());
        assert_eq!(qs[3].topics, vec!["d"]);
        assert_eq!(delete_topic(&mut qs, "abc"), 0);
    }

    #[test]
    fn canonical_keeps_existing_spelling() {
        let pool = vec!["Algebra".to_string(), "Ions".to_string()];
        assert_eq!(canonical_topic(&pool, " algebra "), "Algebra");
        assert_eq!(canonical_topic(&pool, "Geometry"), "Geometry");
    }

    #[test]
    fn suggestions_prefix_then_contains() {
        let pool: Vec<String> = ["Ionic bonds", "Ions", "Covalent bonds", "Acids"].iter().map(|s| s.to_string()).collect();
        assert_eq!(suggest_topics(&pool, "io", &[], 8), vec!["Ionic bonds", "Ions"]);
        assert_eq!(suggest_topics(&pool, "bond", &[], 8), vec!["Ionic bonds", "Covalent bonds"]);
        assert_eq!(suggest_topics(&pool, "bond", &["Ionic bonds".into()], 8), vec!["Covalent bonds"]);
        assert_eq!(suggest_topics(&pool, "", &["Ions".into()], 2), vec!["Ionic bonds", "Covalent bonds"]);
        assert!(suggest_topics(&pool, "ions", &[], 8).is_empty(), "exact match is not a suggestion");
    }

    #[test]
    fn usage_lookups() {
        let exams = vec![
            exam("e1", &[("b1", "q1"), ("b1", "q2"), ("b2", "q9")]),
            exam("e2", &[("b1", "q1"), ("b1", "q1")]),
            exam("e3", &[("b2", "q9")]),
        ];
        let ids = |v: Vec<&Exam>| v.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(exams_using_bank(&exams, "b1")), vec!["e1", "e2"]);
        assert_eq!(ids(exams_using_bank(&exams, "b2")), vec!["e1", "e3"]);
        assert!(exams_using_bank(&exams, "b3").is_empty());
        assert_eq!(ids(exams_using_question(&exams, "b1", "q1")), vec!["e1", "e2"]);
        assert_eq!(ids(exams_using_question(&exams, "b1", "q2")), vec!["e1"]);
        assert!(exams_using_question(&exams, "b2", "q1").is_empty(), "same id, other bank");
        let m = question_usage(&exams, "b1");
        assert_eq!(m.get("q1"), Some(&2));
        assert_eq!(m.get("q2"), Some(&1));
        assert_eq!(m.get("q9"), None);
        assert_eq!(usage_label(0), "Not in any exam");
        assert_eq!(usage_label(1), "Used in 1 exam");
        assert_eq!(usage_label(2), "Used in 2 exams");
    }

    #[test]
    fn type_counts_follow_kinds_order() {
        let mut a = q("1", &[]);
        a.kind = Kind::Mc { answers: vec![] };
        let mut b = q("2", &[]);
        b.kind = Kind::Tf { correct: true };
        let c = q("3", &[]);
        assert_eq!(type_counts(&[a, b, c]), [1, 1, 0, 0, 1, 0]);
    }

    #[test]
    fn pictures_split_and_count() {
        assert_eq!(split_pictures("plain"), vec![StemPart::Text("plain".into())]);
        assert_eq!(
            split_pictures("Look:\n[picture]\nBONDING: ___"),
            vec![StemPart::Text("Look:".into()), StemPart::Picture(None), StemPart::Text("BONDING: ___".into())]
        );
        assert_eq!(
            split_pictures("[picture: Lewis structure of NH3]"),
            vec![StemPart::Picture(Some("Lewis structure of NH3".into()))]
        );
        assert_eq!(split_pictures("a [pictures] b"), vec![StemPart::Text("a [pictures] b".into())]);
        assert_eq!(split_pictures("[picture]$\\frac{1}{2}$").len(), 2);
        let mut m = q("m", &[]);
        m.stem = "[picture] and [picture: x]".into();
        m.kind = Kind::Multipart {
            parts: vec![Part { stem: "a) [picture]".into(), kind: PartKind::Mc { answers: vec![Answer { text: "[picture]".into(), correct: true, locked: false }] } }],
        };
        assert_eq!(pictures_needed(&m), 4);
        m.image = Some(crate::model::ImageRef { file: "media/x.png".into(), width: 1, height: 1 });
        assert_eq!(pictures_needed(&m), 3, "the attached picture covers one marker");
        assert_eq!(pictures_needed(&q("p", &[])), 0);
        // Short-answer model answers count too (multipart parts and plain short answers).
        let mut sa = q("s", &[]);
        sa.kind = Kind::Short { sample: "[picture: key sketch]\nlinear".into() };
        assert_eq!(pictures_needed(&sa), 1);
        let mut mp = q("m", &[]);
        mp.kind = Kind::Multipart {
            parts: vec![
                Part { stem: "NH3".into(), kind: PartKind::Short { sample: "[picture: NH3 key]\npyramidal".into() } },
                Part { stem: "CO2".into(), kind: PartKind::Short { sample: "[picture: CO2 key]".into() } },
            ],
        };
        assert_eq!(pictures_needed(&mp), 2);
        let mut mt = q("t", &[]);
        mt.kind = Kind::Matching { pairs: vec![crate::model::Pair { left: "[picture]".into(), right: "x".into() }], distractors: vec!["[picture: y]".into()] };
        assert_eq!(pictures_needed(&mt), 2);
    }

    // ---------- resolving pictures ----------

    fn pq(stem: &str, kind: Kind) -> Question {
        crate::formats::make_question(stem.into(), kind)
    }

    #[test]
    fn picture_slots_follow_pictures_needed_and_name_their_place() {
        use crate::model::{Part, PartKind};
        let multi = pq(
            "For each compound: [picture: CO2 sketch]",
            Kind::Multipart {
                parts: vec![
                    Part { stem: "a. NH3".into(), kind: PartKind::Short { sample: "[picture: NH3 key] Clouds: 4".into() } },
                    Part { stem: "b. CO2".into(), kind: PartKind::Short { sample: "[picture]".into() } },
                ],
            },
        );
        let fill = pq("Bonds [picture: acetyl chloride] ___", Kind::Blank { accept: vec![vec!["14".into()]] });
        let mut done = fill.clone();
        done.image = Some(crate::model::ImageRef { file: "media/x.png".into(), width: 1, height: 1 });
        let qs = vec![fill, multi, done];
        let slots = picture_slots(&qs);
        assert_eq!(slots.len(), qs.iter().map(pictures_needed).sum::<usize>());
        assert_eq!(slots.len(), 4);
        assert!(slots[0].attachable && slots[0].qi == 0 && slots[0].alt.as_deref() == Some("acetyl chloride"));
        assert_eq!(slots[0].label(1), "Question 1");
        assert!(slots[1].attachable && slots[1].field == PicField::Stem);
        assert_eq!((slots[2].field, slots[2].attachable), (PicField::PartSample(0), false));
        assert_eq!(slots[2].label(2), "Question 2, Part a, Suggested Answer");
        assert_eq!(slots[3].label(2), "Question 2, Part b, Suggested Answer");
        assert_eq!(slots[3].alt, None);
    }

    #[test]
    fn describing_a_marker_resolves_exactly_that_one() {
        let t = "Draw [picture: NH3] and [picture] then [picture: CO2].";
        assert_eq!(describe_marker(t, 0), "Draw (Picture: NH3) and [picture] then [picture: CO2].");
        assert_eq!(describe_marker(t, 1), "Draw [picture: NH3] and (Picture) then [picture: CO2].");
        assert_eq!(describe_marker(t, 2), "Draw [picture: NH3] and [picture] then (Picture: CO2).");
        assert_eq!(describe_marker(t, 3), t);
        // the described marker no longer counts
        let mut q = pq(t, Kind::Short { sample: String::new() });
        q.stem = describe_marker(&q.stem, 1);
        assert_eq!(pictures_needed(&q), 2);
        let f = field_text_mut(&mut q, PicField::Sample).unwrap();
        f.push_str("[picture]");
        assert_eq!(pictures_needed(&q), 3);
        assert!(field_text_mut(&mut q, PicField::Choice(0)).is_none());
    }
}
