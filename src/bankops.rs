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

/// Picture placeholders still needing a picture in one question: markers in its stem,
/// answers and multipart parts, less the one its attached picture already covers.
pub fn pictures_needed(q: &Question) -> usize {
    let n = |s: &str| split_pictures(s).iter().filter(|p| matches!(p, StemPart::Picture(_))).count();
    let mut c = n(&q.stem);
    if let Kind::Multipart { parts } = &q.kind {
        c += parts.iter().map(|p| n(&p.stem)).sum::<usize>();
        for p in parts {
            if let PartKind::Mc { answers } = &p.kind {
                c += answers.iter().map(|a| n(&a.text)).sum::<usize>();
            }
        }
    }
    if let Kind::Mc { answers } = &q.kind {
        c += answers.iter().map(|a| n(&a.text)).sum::<usize>();
    }
    if q.image.is_some() { c.saturating_sub(1) } else { c }
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
        Exam {
            id: id.to_string(),
            name: format!("Exam {id}"),
            header: String::new(),
            title: None,
            items: items.iter().map(|(b, q)| ExamItem { bank_id: b.to_string(), question_id: q.to_string() }).collect(),
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
    }
}
