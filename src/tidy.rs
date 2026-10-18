//! "Tidy stems": rewrites stems written by older imports into the current stem markup
//! (`docs/stem-format.md`). Pure logic, no egui; the bank editor's "Tidy stems…" dialog
//! (`bankui`) previews what this returns and applies only the questions the teacher picks.
//!
//! What it fixes, in this order, repeated until nothing changes (so tidying twice is the
//! same as tidying once):
//!
//! 1. **Joiners.** Old exports flattened line breaks into ` — `. A stem with no line break
//!    whose ` — ` pieces look like separate lines (a list item, a table row, a `Label: ___`
//!    line, or a sentence ending before a capitalised one) gets real line breaks back.
//! 2. **Spacing.** Windows line endings, trailing spaces, runs of empty lines and empty lines
//!    at the start or end.
//! 3. **Hard wraps.** A long prose line that stops mid-sentence (no closing punctuation) and is
//!    followed by a prose line starting in lowercase is one line that a PDF wrapped: rejoined.
//! 4. **Tables.** The label-repeating table of blanks (`KI | Ionic or Covalent? ___ | Name ___`,
//!    header rows carrying blanks): [`crate::ditto::normalize_tables`], the import's own fix.
//!
//! A stem that is already clean comes back byte for byte (property-tested below). Fill-in-the-
//! blank answers stay one per `___`: when a table fix removes a header row's stray blanks the
//! matching answer entries go too (or the import's trailing padding, when that is what lined
//! up), and when that can't be decided the table is left as it was.

use crate::model::{Id, Kind, Question};
use crate::richtext;

/// One kind of change [`tidy_stem`] made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fix {
    Joiners,
    Spacing,
    Wraps,
    Tables,
}

impl Fix {
    pub fn label(self) -> &'static str {
        match self {
            Fix::Joiners => "Line breaks",
            Fix::Spacing => "Spacing",
            Fix::Wraps => "Wrapped lines",
            Fix::Tables => "Table",
        }
    }
}

/// A tidied stem, the original `___` markers (0-based, reading order) a table fix removed,
/// and what changed.
#[derive(Clone, Debug, PartialEq)]
pub struct StemTidy {
    pub stem: String,
    pub dropped: Vec<usize>,
    pub fixes: Vec<Fix>,
}

const JOINER: &str = " — ";

/// Tidies one stem. `answer_counts` help the table fix tell a header row from a data row
/// (see [`crate::ditto::normalize_tables`]); pass `&[]` when there are no answers to go by.
pub fn tidy_stem(stem: &str, answer_counts: &[usize]) -> StemTidy {
    tidy_text(stem, true, answer_counts)
}

fn tidy_text(stem: &str, tables: bool, counts: &[usize]) -> StemTidy {
    let unchanged = || StemTidy { stem: stem.to_string(), dropped: Vec::new(), fixes: Vec::new() };
    let mut s = stem.to_string();
    // Original marker index of every `___` still in `s`.
    let mut alive: Vec<usize> = (0..s.matches("___").count()).collect();
    let mut fixes: Vec<Fix> = Vec::new();
    let note = |f: Fix, fixes: &mut Vec<Fix>| {
        if !fixes.contains(&f) {
            fixes.push(f);
        }
    };
    for _ in 0..6 {
        let start = s.clone();
        let steps: [(Fix, fn(&str) -> String); 3] = [(Fix::Joiners, split_joiners), (Fix::Spacing, fix_spacing), (Fix::Wraps, unwrap_lines)];
        for (fix, step) in steps {
            let next = step(&s);
            if next != s {
                // These steps never add or remove blanks; if one did, touch nothing.
                if next.matches("___").count() != alive.len() {
                    return unchanged();
                }
                note(fix, &mut fixes);
                s = next;
            }
        }
        if tables {
            let t = crate::ditto::normalize_tables(&s, counts);
            if t.stem != s {
                let mut gone: Vec<usize> = t.dropped.iter().filter_map(|&d| alive.get(d).copied()).collect();
                gone.sort_unstable();
                gone.dedup();
                if gone.len() != t.dropped.len() {
                    return unchanged();
                }
                alive.retain(|i| !gone.contains(i));
                if t.stem.matches("___").count() != alive.len() {
                    // A long `______` cell became one `___`: the answers can't follow it.
                    return unchanged();
                }
                note(Fix::Tables, &mut fixes);
                s = t.stem;
            }
        }
        if s == start {
            break;
        }
    }
    if s == stem {
        return unchanged();
    }
    let total = stem.matches("___").count();
    let dropped = (0..total).filter(|i| !alive.contains(i)).collect();
    StemTidy { stem: s, dropped, fixes }
}

// ---------- 1. joiners ----------

/// True for a ` — ` piece that reads as its own line.
fn line_like(seg: &str) -> bool {
    let t = seg.trim();
    t.contains("___")
        || richtext::bullet(t).is_some()
        || richtext::numbered(t).is_some()
        || richtext::heading(t).is_some()
        || t.starts_with("> ")
        || t.matches(" | ").count() >= 1 && richtext::cells(t).is_some_and(|c| c.len() >= 2)
}

fn ends_sentence(seg: &str) -> bool {
    let t = seg.trim_end().trim_end_matches([')', '"', '”', '’', '\'']);
    t.ends_with(['.', '?', '!', ':'])
}

fn starts_line(seg: &str) -> bool {
    seg.trim_start().chars().next().is_some_and(|c| c.is_uppercase() || c.is_ascii_digit() || c == '(')
}

fn split_joiners(s: &str) -> String {
    if s.contains('\n') || !s.contains(JOINER) {
        return s.to_string();
    }
    let segs: Vec<&str> = s.split(JOINER).collect();
    let mut out = segs[0].to_string();
    for w in segs.windows(2) {
        let (a, b) = (w[0], w[1]);
        let split = !a.trim().is_empty()
            && !b.trim().is_empty()
            && (line_like(b) || (line_like(a) || ends_sentence(a)) && starts_line(b));
        if split {
            // A numbered list after a paragraph line needs a blank line above it.
            let para_then_list = richtext::numbered(b.trim()).is_some() && richtext::numbered(a.trim()).is_none();
            out = out.trim_end().to_string();
            out.push_str(if para_then_list { "\n\n" } else { "\n" });
            out.push_str(b.trim_start());
        } else {
            out.push_str(JOINER);
            out.push_str(b);
        }
    }
    out
}

// ---------- 2. spacing ----------

fn fix_spacing(s: &str) -> String {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    let mut out: Vec<&str> = Vec::new();
    for l in s.split('\n') {
        let l = l.trim_end_matches([' ', '\t']);
        if l.trim().is_empty() {
            if out.last().is_some_and(|p| !p.is_empty()) {
                out.push("");
            }
            continue;
        }
        out.push(l);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

// ---------- 3. hard wraps ----------

/// The text of a line that may take a continuation: prose, or a list / quote item's text.
fn joinable_text(l: &str) -> Option<&str> {
    if l.trim().is_empty() || richtext::heading(l).is_some() || richtext::divider(l) || richtext::fence(l).is_some() || richtext::cells(l).is_some() || l.contains("___") {
        return None;
    }
    Some(richtext::bullet(l).or_else(|| richtext::numbered(l)).or_else(|| richtext::quote(l)).unwrap_or(l))
}

/// A plain prose line (no list marker, table, heading, ...).
fn prose(l: &str) -> bool {
    joinable_text(l).is_some_and(|t| t.len() == l.len()) && richtext::bullet(l).is_none() && richtext::numbered(l).is_none() && richtext::quote(l).is_none()
}

fn unwrap_lines(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for l in s.split('\n') {
        let join = out.last().is_some_and(|p| {
            joinable_text(p).is_some_and(|t| {
                let t = t.trim_end();
                t.chars().count() >= 40 && !t.ends_with(['.', '?', '!', ':', ';', ')', ']', '"', '”', '’', '\'', '-', '—', '–', ','])
            }) && prose(l)
                && l.trim_start().chars().next().is_some_and(char::is_lowercase)
        });
        if join {
            let p = out.last_mut().unwrap();
            let keep = p.trim_end().len();
            p.truncate(keep);
            p.push(' ');
            p.push_str(l.trim_start());
        } else {
            out.push(l.to_string());
        }
    }
    out.join("\n")
}

// ---------- questions ----------

/// One changed text of a question: "Stem" or "Part a", before and after.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub label: String,
    pub before: String,
    pub after: String,
}

/// A question [`tidy_question`] would change: the question as it is (`before`) and tidied.
#[derive(Clone, Debug)]
pub struct QuestionTidy {
    pub id: Id,
    /// Position in the bank (0-based).
    pub index: usize,
    pub fields: Vec<Field>,
    pub fixes: Vec<Fix>,
    pub before: Question,
    pub after: Question,
    /// Something the teacher should know (e.g. a table left alone).
    pub note: Option<String>,
}

/// A blank question's answers after a table fix removed the original markers `dropped`
/// (`after` markers remain). None when it can't be told which answers belonged to them.
fn align_blanks(accept: &[Vec<String>], before: usize, after: usize, dropped: &[usize]) -> Option<Vec<Vec<String>>> {
    if dropped.is_empty() {
        return Some(accept.to_vec());
    }
    let mut core = accept.to_vec();
    while core.last().is_some_and(Vec::is_empty) {
        core.pop();
    }
    let mut out = if core.len() == after {
        // One answer per remaining blank, padded at the end: the padding was the header's.
        core
    } else if accept.len() == before && dropped.iter().all(|&d| accept[d].is_empty()) {
        // One entry per original blank, the header's empty: drop exactly those.
        accept.iter().enumerate().filter(|(i, _)| !dropped.contains(i)).map(|(_, a)| a.clone()).collect()
    } else if core.len() < after && core.iter().map(Vec::len).sum::<usize>() == after {
        // Answers grouped by row ([[a, b], [c, d]]): one per blank once flattened.
        core.into_iter().flatten().map(|a| vec![a]).collect()
    } else {
        return None;
    };
    if out.len() > after {
        return None;
    }
    out.resize(after, Vec::new());
    Some(out)
}

/// The answer counts the table fix uses to tell a header row from data (entries without the
/// trailing padding, flattened answers).
fn counts(accept: &[Vec<String>]) -> [usize; 2] {
    let core = accept.len() - accept.iter().rev().take_while(|a| a.is_empty()).count();
    [core, accept.iter().map(Vec::len).sum()]
}

/// What tidying `q` would change, or None when it is already tidy.
pub fn tidy_question(q: &Question, index: usize) -> Option<QuestionTidy> {
    let mut after = q.clone();
    let mut fields: Vec<Field> = Vec::new();
    let mut fixes: Vec<Fix> = Vec::new();
    let mut note = None;
    let add = |fields: &mut Vec<Field>, fixes: &mut Vec<Fix>, label: String, before: &str, t: &StemTidy| {
        if t.stem != before {
            fields.push(Field { label, before: before.to_string(), after: t.stem.clone() });
            for f in &t.fixes {
                if !fixes.contains(f) {
                    fixes.push(*f);
                }
            }
        }
    };
    match &q.kind {
        Kind::Blank { accept } => {
            let before = q.stem.matches("___").count();
            let mut t = tidy_stem(&q.stem, &counts(accept));
            let after_n = t.stem.matches("___").count();
            let aligned = if t.dropped.is_empty() {
                Some(accept.clone())
            } else if after_n == 0 {
                None
            } else {
                align_blanks(accept, before, after_n, &t.dropped)
            };
            match aligned {
                Some(acc) => after.kind = Kind::Blank { accept: acc },
                None => {
                    // Keep every blank (and so every answer) where it is.
                    t = tidy_text(&q.stem, false, &[]);
                    note = Some("Its table is left as it is: the answers don't say which blanks were the header's.".to_string());
                }
            }
            add(&mut fields, &mut fixes, "Stem".into(), &q.stem, &t);
            after.stem = t.stem;
        }
        Kind::Multipart { parts } => {
            let t = tidy_stem(&q.stem, &[]);
            add(&mut fields, &mut fixes, "Stem".into(), &q.stem, &t);
            after.stem = t.stem;
            let mut new_parts = parts.clone();
            for (i, p) in new_parts.iter_mut().enumerate() {
                let t = tidy_stem(&p.stem, &[]);
                add(&mut fields, &mut fixes, format!("Part {}", char::from(b'a' + (i % 26) as u8)), &p.stem, &t);
                p.stem = t.stem;
            }
            after.kind = Kind::Multipart { parts: new_parts };
        }
        _ => {
            let t = tidy_stem(&q.stem, &[]);
            add(&mut fields, &mut fixes, "Stem".into(), &q.stem, &t);
            after.stem = t.stem;
        }
    }
    if fields.is_empty() {
        return None;
    }
    Some(QuestionTidy { id: q.id.clone(), index, fields, fixes, before: q.clone(), after, note })
}

/// Every question of a bank that tidying would change, in bank order.
pub fn tidy_bank(qs: &[Question]) -> Vec<QuestionTidy> {
    qs.iter().enumerate().filter_map(|(i, q)| tidy_question(q, i)).collect()
}

fn same(a: &Question, b: &Question) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}

/// Applies the picked tidies to `qs` (by id, and only to a question still exactly as it was
/// previewed). Returns the questions as they were, for undo.
pub fn apply(qs: &mut [Question], picked: &[&QuestionTidy], now: i64) -> Vec<Question> {
    let mut undo = Vec::new();
    for t in picked {
        if let Some(q) = qs.iter_mut().find(|q| q.id == t.id)
            && same(q, &t.before)
        {
            undo.push(q.clone());
            q.stem = t.after.stem.clone();
            q.kind = t.after.kind.clone();
            q.modified = now;
        }
    }
    undo
}

/// Puts back the questions [`apply`] returned (by id). Returns how many were restored.
pub fn undo(qs: &mut [Question], before: &[Question]) -> usize {
    let mut n = 0;
    for b in before {
        if let Some(q) = qs.iter_mut().find(|q| q.id == b.id) {
            *q = b.clone();
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PartKind, QuestionBank};

    fn bank(json: &str) -> QuestionBank {
        serde_json::from_str(json).expect("fixture bank")
    }
    fn old() -> QuestionBank {
        bank(include_str!("../tests/fixtures/ch4_old_bank.json"))
    }
    fn clean() -> QuestionBank {
        bank(include_str!("../tests/fixtures/ch4_bank.json"))
    }
    fn q<'a>(b: &'a QuestionBank, id: &str) -> &'a Question {
        b.questions.iter().find(|q| q.id == id).unwrap()
    }
    fn accept(q: &Question) -> &Vec<Vec<String>> {
        match &q.kind {
            Kind::Blank { accept } => accept,
            k => panic!("{k:?}"),
        }
    }

    /// The old Ch4 bank tidies into the bank as it is stored today, question by question.
    #[test]
    fn ch4_old_bank_tidies_into_the_clean_bank() {
        let (old, clean) = (old(), clean());
        let tidies = tidy_bank(&old.questions);
        // Two multiple-choice questions per quiz were never broken.
        assert_eq!(tidies.len(), old.questions.len() - 2, "{:?}", tidies.iter().map(|t| &t.id).collect::<Vec<_>>());
        for t in &tidies {
            let want = q(&clean, &t.id);
            assert_eq!(t.after.stem, want.stem, "{}", t.id);
            assert_eq!(serde_json::to_value(&t.after.kind).unwrap(), serde_json::to_value(&want.kind).unwrap(), "{}", t.id);
            assert!(t.note.is_none());
        }
        let fixes = |id: &str| tidies.iter().find(|t| t.id == id).unwrap().fixes.clone();
        assert_eq!(fixes("0041a146fa9f3c"), vec![Fix::Tables]);
        assert!(fixes("0031a146fa9f3b").contains(&Fix::Joiners));
        assert!(fixes("00c1a146fab514").contains(&Fix::Wraps) && fixes("00c1a146fab514").contains(&Fix::Spacing));
        // Applying them all gives the clean bank; undo gives the old one back.
        let mut qs = old.questions.clone();
        let picked: Vec<&QuestionTidy> = tidies.iter().collect();
        let before = apply(&mut qs, &picked, 99);
        assert_eq!(before.len(), tidies.len());
        for (a, b) in qs.iter().zip(&clean.questions) {
            assert_eq!(a.stem, b.stem);
        }
        assert!(tidy_bank(&qs).is_empty(), "tidying is idempotent");
        assert_eq!(undo(&mut qs, &before), before.len());
        for (a, b) in qs.iter().zip(&old.questions) {
            assert!(same(a, b));
        }
    }

    #[test]
    fn the_real_ch4_bank_is_already_tidy() {
        assert!(tidy_bank(&clean().questions).is_empty());
    }

    #[test]
    fn apply_skips_questions_edited_since_the_preview() {
        let old = old();
        let tidies = tidy_bank(&old.questions);
        let mut qs = old.questions.clone();
        qs[1].stem.push_str(" (edited)");
        let picked: Vec<&QuestionTidy> = tidies.iter().filter(|t| t.index <= 1).collect();
        let undo = apply(&mut qs, &picked, 1);
        assert_eq!(undo.len(), picked.len() - 1);
        assert!(qs[1].stem.ends_with("(edited)"));
    }

    // ---------- alignment invariants ----------

    const LEAD: &str = "Name the following compounds.";
    const KEY: [&str; 4] = ["Ionic", "potassium iodide", "Covalent", "sulfur trioxide"];

    fn blank_q(stem: &str, accept: Vec<Vec<String>>) -> Question {
        let mut q = crate::formats::make_question(stem.to_string(), Kind::Blank { accept });
        q.id = "q".into();
        q
    }
    fn key() -> Vec<Vec<String>> {
        KEY.iter().map(|k| vec![k.to_string()]).collect()
    }
    fn invariant(q: &Question) {
        if let Kind::Blank { accept } = &q.kind {
            assert_eq!(accept.len(), q.stem.matches("___").count(), "{:?}", q.stem);
        }
    }

    #[test]
    fn header_blanks_drop_their_padding_or_their_empty_entries() {
        let stem = format!("{LEAD}\nFormula | Ionic or Covalent? ___ | Name ___\nKI | ___ | ___\nSO₃ | ___ | ___");
        let clean = format!("{LEAD}\nFormula | Ionic or Covalent? | Name\nKI | ___ | ___\nSO₃ | ___ | ___");
        // Old import: answers for the data blanks, padded at the end to the six markers.
        let mut padded = key();
        padded.extend([vec![], vec![]]);
        let t = tidy_question(&blank_q(&stem, padded), 0).unwrap();
        assert_eq!(t.after.stem, clean);
        assert_eq!(accept(&t.after), &key());
        invariant(&t.after);
        // One entry per original marker, the header's empty.
        let mut per_marker = vec![vec![], vec![]];
        per_marker.extend(key());
        let t = tidy_question(&blank_q(&stem, per_marker), 0).unwrap();
        assert_eq!(accept(&t.after), &key());
        // Grouped by row.
        let grouped = vec![vec!["Ionic".to_string(), "potassium iodide".into()], vec!["Covalent".into(), "sulfur trioxide".into()], vec![], vec![], vec![], vec![]];
        let t = tidy_question(&blank_q(&stem, grouped), 0).unwrap();
        assert_eq!(accept(&t.after), &key());
    }

    #[test]
    fn undecidable_answers_keep_the_table_and_every_blank() {
        // Six real answers for six markers: which two were the header's is unknowable.
        let stem = format!("{LEAD}\nFormula | Ionic or Covalent? ___ | Name ___\nKI | ___ | ___\nSO₃ | ___ | ___  ");
        let six: Vec<Vec<String>> = ["a", "b", "c", "d", "e", "f"].iter().map(|k| vec![k.to_string()]).collect();
        let q = blank_q(&stem, six.clone());
        let t = tidy_question(&q, 0).unwrap();
        assert_eq!(t.after.stem, stem.trim_end(), "only the spacing is fixed");
        assert_eq!(accept(&t.after), &six);
        assert!(t.note.is_some());
        invariant(&t.after);
        // Nothing else to fix: no change at all.
        assert!(tidy_question(&blank_q(stem.trim_end(), six), 0).is_none());
    }

    #[test]
    fn labelled_rows_without_a_header_keep_their_answers() {
        let stem = format!("{LEAD}\nKI | Ionic or Covalent? ___ | Name ___\nSO₃ | Ionic or Covalent? ___ | Name ___");
        let t = tidy_question(&blank_q(&stem, key()), 0).unwrap();
        assert_eq!(t.after.stem, format!("{LEAD}\n|  | Ionic or Covalent? | Name |\nKI | ___ | ___\nSO₃ | ___ | ___"));
        assert_eq!(accept(&t.after), &key());
        invariant(&t.after);
    }

    #[test]
    fn multipart_parts_stay_in_order_and_kind() {
        let old = old();
        let mp = q(&old, "00b1a146fab514");
        let t = tidy_question(mp, 3).unwrap();
        let (Kind::Multipart { parts: a }, Kind::Multipart { parts: b }) = (&mp.kind, &t.after.kind) else { panic!() };
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b) {
            assert_eq!(x.stem.matches("___").count(), y.stem.matches("___").count());
            assert!(matches!((&x.kind, &y.kind), (PartKind::Short { sample: s1 }, PartKind::Short { sample: s2 }) if s1 == s2));
        }
        assert!(t.fields.iter().any(|f| f.label == "Part a"));
    }

    // ---------- rules ----------

    #[test]
    fn joiners_split_only_where_lines_were() {
        let cases = [
            ("Do all: — • Draw it. — • Name it.", "Do all:\n• Draw it.\n• Name it."),
            ("Give these: — Bonds: ___  Lone pairs: ___", "Give these:\nBonds: ___  Lone pairs: ___"),
            ("One ounce is 29.57 mL.) — Show your work. — Use units.", "One ounce is 29.57 mL.)\nShow your work.\nUse units."),
            ("Steps — 1. mix — 2. heat", "Steps\n\n1. mix\n2. heat"),
            ("Name — Formula | Name — KI | ___", "Name\nFormula | Name\nKI | ___"),
        ];
        for (a, b) in cases {
            assert_eq!(tidy_stem(a, &[]).stem, b, "{a}");
        }
        // Real dashes stay.
        for s in [
            "The answer — if there is one — is short.",
            "Water — a polar molecule — boils at 100 °C.",
            "Pick one — or none.",
            "Already\nsplit — keep this dash.",
        ] {
            assert_eq!(tidy_stem(s, &[]).stem, s, "{s}");
        }
    }

    #[test]
    fn spacing_and_wraps() {
        assert_eq!(tidy_stem("A.  \r\n\r\n\r\n\nB.\n\n", &[]).stem, "A.\n\nB.");
        let wrapped = "A milliliter of cow's milk contains 0.026 g of casein\nprotein. How many grams are in a cup?";
        assert_eq!(tidy_stem(wrapped, &[]).stem, "A milliliter of cow's milk contains 0.026 g of casein protein. How many grams are in a cup?");
        let bullet = "• Draw a valid Lewis structure for each of the\nmolecules below.";
        assert_eq!(tidy_stem(bullet, &[]).stem, "• Draw a valid Lewis structure for each of the molecules below.");
        // Short lines, finished sentences, capitals, blanks and tables are never joined.
        for s in ["a. NH₃\nnumber of clouds", "Show each step explicitly, with units included.\nanswer with units", "Number of charge clouds: ___\nmolecule shape", "This line is long enough to be a wrapped one\nBut the next starts with a capital", "Formula | Name\nKI | potassium iodide"] {
            assert_eq!(tidy_stem(s, &[]).stem, s, "{s}");
        }
    }

    // ---------- properties ----------

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
            v[(self.next() % v.len() as u64) as usize]
        }
    }

    /// A random stem built only from well-formed blocks of the current markup.
    fn clean_stem(r: &mut Rng) -> String {
        const PARA: &[&str] = &[
            "Name the following compounds.",
            "Give the number of bonding and nonbonding electrons for the structure below:",
            "Which of the following elements is the most electronegative?",
            "A milliliter of milk contains 0.026 g of casein — a protein.",
            "Solve $x^2 = 4$ for **x**.",
            "Water is H_{2}O and the ion is Ca^{2+}.",
            "BONDING ELECTRONS: ___  NONBONDING ELECTRONS: ___",
            "Short line",
            "Polarity of molecule (circle one): polar nonpolar",
            "The answer — if there is one — is short.",
        ];
        const BLOCK: &[&str] = &[
            "• Draw a valid Lewis structure.\n• Count the number of electron clouds.",
            "1. Mix the solution.\n2. Heat it gently.",
            "> A quoted line.",
            "Formula | Ionic or Covalent? | Name\nKI | ___ | ___\nSO₃ | ___ | ___",
            "|  | Ionic or Covalent? | Name |\nN₂O | ___ | ___\nKI | ___ | ___",
            "a | b\n---|---\n1 | 2",
            "## Part A",
            "---",
            "::: side\nLeft panel.\n::: next\nRight panel.\n:::",
            "Number of charge clouds: ___\nMolecule shape: ___",
        ];
        let n = 1 + (r.next() % 5) as usize;
        let mut out = String::new();
        for k in 0..n {
            if k > 0 {
                out.push_str(if r.next() % 3 == 0 { "\n\n" } else { "\n" });
            }
            out.push_str(if r.next() % 2 == 0 { r.pick(PARA) } else { r.pick(BLOCK) });
        }
        out
    }

    #[test]
    fn property_clean_stems_are_never_changed() {
        let mut r = Rng(0x5eed_1234_abcd_0001);
        for _ in 0..20_000 {
            let s = clean_stem(&mut r);
            let t = tidy_stem(&s, &[s.matches("___").count()]);
            assert_eq!(t.stem, s, "clean stem changed: {s:?}");
            assert!(t.fixes.is_empty() && t.dropped.is_empty());
        }
    }

    /// Random messy stems: the old shapes, stray whitespace and joiners mixed with clean text.
    fn messy_stem(r: &mut Rng) -> String {
        const TOK: &[&str] = &[
            "Formula | Ionic or Covalent? ___ | Name ___", "KI | Ionic or Covalent? ___ | Name ___", "NO | ___ | ___", "Name ___", " | ",
            "___", "______", " — ", " — ", "\n", "\n", "\n\n\n", "  ", "\r\n", "• ", "1. ", "> ", "# ", "---", "::: side", "::: next", ":::",
            "this is a fairly long line of prose that wraps", "continues here", "A long wrapped sentence of prose that keeps going on\nand on until", "\nand more", "Capital", ".", ":", "?", "word", "a", " ", "|", "$x$", "**b**",
        ];
        let n = (r.next() % 30) as usize;
        (0..n).map(|_| r.pick(TOK)).collect()
    }

    #[test]
    fn property_tidy_is_idempotent_and_keeps_blank_answers_aligned() {
        let mut r = Rng(0x0dd_ba11);
        let mut hits = [0usize; 4];
        for _ in 0..20_000 {
            let s = messy_stem(&mut r);
            let once = tidy_stem(&s, &[]);
            for f in &once.fixes {
                hits[*f as usize] += 1;
            }
            let twice = tidy_stem(&once.stem, &[]);
            assert_eq!(twice.stem, once.stem, "not idempotent: {s:?}");
            assert!(twice.fixes.is_empty(), "{s:?}");
            // Only table fixes remove blanks, and exactly the ones reported.
            assert_eq!(once.stem.matches("___").count() + once.dropped.len(), s.matches("___").count(), "{s:?}");
            // As a fill-in-the-blank question with one answer per blank, the answers follow.
            let n = s.matches("___").count();
            if n > 0 {
                let acc: Vec<Vec<String>> = (0..n).map(|i| vec![format!("a{i}")]).collect();
                let q = blank_q(&s, acc);
                if let Some(t) = tidy_question(&q, 0) {
                    invariant(&t.after);
                    let again = tidy_question(&t.after, 0);
                    assert!(again.is_none(), "question not idempotent: {s:?} -> {:?}", again.map(|a| a.after.stem));
                }
            }
        }
        // Every rule actually ran on a good share of the inputs.
        assert!(hits.iter().all(|&h| h > 200), "{hits:?}");
    }

    #[test]
    fn backup_copies_banks_json_without_overwriting() {
        let root = std::env::temp_dir().join(format!("tidy-backup-{}", std::process::id()));
        let (store, backups) = (root.join("queshion"), root.join("queshion/backups"));
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("banks.json"), "[1]").unwrap();
        let a = crate::store::backup_banks_in(&store, &backups, &[], 1_792_192_201).unwrap();
        let b = crate::store::backup_banks_in(&store, &backups, &[], 1_792_192_201).unwrap();
        assert_ne!(a, b);
        assert!(a.file_name().unwrap().to_string_lossy().starts_with("banks-2026"), "{a:?}");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "[1]");
        // No banks.json yet: the banks in memory are written.
        std::fs::remove_file(store.join("banks.json")).unwrap();
        let c = crate::store::backup_banks_in(&store, &backups, &[clean()], 5).unwrap();
        let back: Vec<QuestionBank> = serde_json::from_slice(&std::fs::read(c).unwrap()).unwrap();
        assert_eq!(back[0].questions.len(), 10);
        let _ = std::fs::remove_dir_all(root);
    }
}
