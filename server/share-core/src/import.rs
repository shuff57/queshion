//! The import contract: how a client turns a received keyless (stripped) question into its own
//! local model, and how the teacher's re-entered key turns it back into a full question.
//!
//! # The served stripped Matching form
//! `pairs[i].left` in the author's order; every `pairs[i].right` is `""`; `distractors` holds the
//! **word bank**: all right-hand items (the answers and the true distractors together),
//! de-duplicated by exact text, in one random order fixed at publish time. `keyless` is `true`.
//! The word bank is not split into "answers" and "distractors" because that split *is* the key.
//!
//! Why no separate `bank` field (and no schema v3): the app's model has no such field, so an older
//! build would drop it and lose the word bank entirely; and the served form is only ever read by
//! the importer below, which must convert it anyway. `keyless: true` plus all-empty rights is
//! unambiguous, and `validate` refuses an empty right side on a question that is not keyless.
//!
//! # Conversion on import ([`localize`])
//! For every question with `keyless: true`:
//! 1. `explanation` becomes [`KEYLESS_SENTINEL`] (older builds drop `keyless` on re-save but keep
//!    and print the explanation, so the answer key always says it has no key).
//! 2. Nothing else changes. In particular the copy carries **no placeholder key**: Matching keeps
//!    its empty right sides with the word bank in `distractors`, Mc keeps every `correct` false,
//!    Blank keeps empty accept lists. The app prints the right column with [`right_column`]
//!    (`model::right_column`, empties skipped), so the student sheet shows exactly the word bank.
//!    A keyless **Tf** still holds `correct: false` because the model has no "unset" value: that
//!    `false` is not a key, and the `keyless` field (with the sentinel for older builds) is what
//!    says so. Every printing and export path must check `keyless` and print no key.
//! 3. The copy must be re-keyed ([`rekey`]) before it is used as a keyed question; until then
//!    `validate_for_publish` refuses it without `hide_key`, and the app prints "Key not provided".
//!
//! # Re-entering the key ([`rekey`], every kind)
//! The teacher picks, for each left item, one item of the right column (`pairs[].right` then
//! `distractors`, empties skipped, as the sheet shows it). The picks become `pairs[].right`; every
//! column item not picked becomes a distractor, once; `keyless` is cleared and the sentinel removed.
//! No item is ever duplicated or lost. Picks are indices into the column the teacher was shown,
//! checked against it (out of range is refused). The other kinds take their key directly
//! ([`Key`]): the correct choice indices (Mc, at least one), true/false (Tf), one non-empty accept
//! list per blank (Blank), the sample (Short), and one part key per part of the same kinds
//! (Multipart). `rekey` is the only way `keyless` is cleared.
//!
//! # Publishing an imported copy
//! `validate_for_publish` refuses a question that is still keyless (or still carries the sentinel)
//! unless the publish has `hide_key`, so the placeholder layout is never served as a real key.
use crate::wire::*;

/// The importer's conversion of one served question (see the module docs). A question without
/// `keyless` is returned unchanged.
pub fn localize(q: &Question) -> Question {
    let mut l = q.clone();
    if l.keyless {
        l.explanation = Some(KEYLESS_SENTINEL.to_string());
    }
    l
}

/// `localize` over a whole payload.
pub fn localize_payload(p: &Payload) -> Payload {
    let mut p = p.clone();
    for q in p.questions_mut() {
        *q = localize(q);
    }
    p
}

/// The right column as the app shows it: `pairs[].right` then `distractors`, empties skipped,
/// first occurrence of each text only.
pub fn right_column(pairs: &[Pair], distractors: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in pairs.iter().map(|p| &p.right).chain(distractors) {
        if crate::text::has_visible(s) && !out.contains(s) {
            out.push(s.clone());
        }
    }
    out
}

/// The teacher's key for a keyless question (see the module docs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// Indices of the correct choices (at least one).
    Mc(Vec<usize>),
    Tf(bool),
    /// For each left item, its pick in [`right_column`].
    Matching(Vec<usize>),
    /// One non-empty accept list per blank.
    Blank(Vec<Vec<String>>),
    Short(String),
    Multipart(Vec<PartKey>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartKey {
    Mc(Vec<usize>),
    Tf(bool),
    Short(String),
}

fn mc_key(answers: &mut [Answer], picks: &[usize]) -> Result<(), &'static str> {
    if picks.is_empty() || picks.iter().any(|&i| i >= answers.len()) {
        return Err("bad_key");
    }
    for (i, a) in answers.iter_mut().enumerate() {
        a.correct = picks.contains(&i);
    }
    Ok(())
}

/// Applies the teacher's key: the only way `keyless` is cleared. Refused (`bad_key`) when the key
/// does not fit the question (other kind, wrong count, an index out of range, an empty accept
/// list, no correct Mc choice).
pub fn rekey(q: &Question, key: &Key) -> Result<Question, &'static str> {
    let mut out = q.clone();
    match (&mut out.kind, key) {
        (Kind::Mc { answers }, Key::Mc(picks)) => mc_key(answers, picks)?,
        (Kind::Tf { correct }, Key::Tf(v)) => *correct = *v,
        (Kind::Matching { pairs, distractors }, Key::Matching(picks)) => {
            let column = right_column(pairs, distractors);
            if picks.len() != pairs.len() || picks.iter().any(|&i| i >= column.len()) {
                return Err("bad_key");
            }
            let picked: std::collections::BTreeSet<usize> = picks.iter().copied().collect();
            for (p, &i) in pairs.iter_mut().zip(picks) {
                p.right = column[i].clone();
            }
            *distractors = column.iter().enumerate().filter(|(i, _)| !picked.contains(i)).map(|(_, s)| s.clone()).collect();
        }
        (Kind::Blank { accept }, Key::Blank(lists)) => {
            if lists.len() != accept.len() || lists.iter().any(|l| l.iter().all(|s| s.trim().is_empty())) {
                return Err("bad_key");
            }
            *accept = lists.clone();
        }
        (Kind::Short { sample }, Key::Short(s)) => *sample = s.clone(),
        (Kind::Multipart { parts }, Key::Multipart(keys)) => {
            if keys.len() != parts.len() {
                return Err("bad_key");
            }
            for (p, k) in parts.iter_mut().zip(keys) {
                match (&mut p.kind, k) {
                    (PartKind::Mc { answers }, PartKey::Mc(picks)) => mc_key(answers, picks)?,
                    (PartKind::Tf { correct }, PartKey::Tf(v)) => *correct = *v,
                    (PartKind::Short { sample }, PartKey::Short(s)) => *sample = s.clone(),
                    _ => return Err("bad_key"),
                }
            }
        }
        _ => return Err("bad_key"),
    }
    out.keyless = false;
    if out.explanation.as_deref() == Some(KEYLESS_SENTINEL) {
        out.explanation = None;
    }
    Ok(out)
}

/// [`rekey`] for Matching.
pub fn rekey_matching(q: &Question, picks: &[usize]) -> Option<Question> {
    rekey(q, &Key::Matching(picks.to_vec())).ok()
}
