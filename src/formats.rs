//! Import format parsers — turn pasted or uploaded text into bank questions.
//!
//! Five formats are recognized, tried in this order by [`detect_and_parse`]:
//!
//! 1. **Test Parrot package / real question-bank record** (`.parrot.json`, also any JSON
//!    whose bytes mention `test-parrot/package` or `test-parrot/question-bank`): the real
//!    export format of testparrot.com —
//!    `{"format":"test-parrot/package","formatVersion":"0.1.0","generator":{..},"requiredFeatures":[..],"questionBanks":[{"id":"bank","record":{ <question-bank record> }}],"exams":[..]}`
//!    with a question bank record of
//!    `{"format":"test-parrot/question-bank","formatVersion":"0.10.0","generator":{..},"requiredFeatures":[..],"bank":{"name":"..","questions":[..]},"media":[..]}`
//!    (a bare record parses the same way; schemas at testparrot.com/formats). All
//!    questions of all banks land in one `ImportDoc`; ids are regenerated; the map to
//!    `model::Kind` is best effort: multiple-choice → Mc (choice texts, correct and locked
//!    kept); true-false → Tf (the first choice is True; exactly one carries `correct`);
//!    matching → Matching (each prompt whose `answer` names a `wordBank` id pairs prompt
//!    text with that answer text; wordBank answers no prompt references are distractors);
//!    fill-in-the-blank → Blank (each `blank` node in the stem prints as `___`, in order,
//!    and its answer text is that blank's accepted string — the real format has no
//!    alternatives concept, so one string per blank); short-answer → Short
//!    (`suggestedAnswer` text); multipart → Multipart (each `part` is a `model::Part`; a
//!    part carrying subparts — which `PartKind` cannot nest — flattens to its lead-in stem
//!    plus one part per subpart); `easy`/`medium`/`hard` → Low/Medium/High, `topics` and
//!    integer `points` are kept. Stem media become `[picture]`/`[picture: alt]`
//!    placeholders — the assets normally travel as files beside the JSON inside the
//!    package zip, so their bytes are not in the JSON; a media entry that also carries
//!    inline base64 (`data`/`bytes`/`base64` property) is attached through
//!    `store::put_image` instead. A package exam record becomes a best-effort
//!    `model::Exam` in `doc.exams`: items in `positions` order referencing the regenerated
//!    question ids, with `bank_id` left empty for the shell to fill when it creates the
//!    bank from this ImportDoc; sections, columns, answerOrder, hiddenAnswers, paper style
//!    and margins are ignored.
//! 2. **Legacy Test Parrot JSON** (any JSON whose `kind` is `test-parrot-question-bank`):
//!    an earlier placeholder shape, `{"kind":"test-parrot-question-bank", ..., "name":"..",
//!    "questions":[model::Question..]}` — kept working as an alias; ids regenerate; the
//!    doc name comes from the JSON, else the filename stem.
//! 3. **CSV** (`.csv`, or any text whose first line looks like a header naming `type` and
//!    `stem`): single-line comma-separated cells with double quotes (`""` inside a quoted
//!    cell for a literal quote — no newlines inside cells). Header columns, in any order:
//!    `type`, `stem`, `points`, `topic` (split into topics on `;`, `,`, `|`), answer columns
//!    named `a`…`h`, `correct` (letters like `b` or `a,c`; digits `1`… also accepted) and
//!    `correct_text`. An empty `type` cell defaults to `mc`. Types: `mc`/`ma` (answers from
//!    the letter columns; correct marks from `correct` and/or a `correct_text` cell that
//!    matches one answer exactly), `tf` (first char of `correct_text`/`correct`: T/Y →
//!    true, F/N → false; A/B also mean true/false), `short`/`ess`/`essay` and
//!    `blank`/`fib`/`num` (the `correct_text` cell is the accepted string / sample answer).
//!    An unparseable `points` cell is ignored (`None`); structural problems (no stem, no
//!    correct mark, unknown type) fail the whole import naming the row. A `.csv` file
//!    whose first line is not such a header is not CSV — the text parsers get their try.
//! 3. **CSV** (`.csv` — header required; a `.csv` whose first line is not such a header
//!    is not CSV — the text parsers get their try. Single-line comma-separated cells with
//!    double quotes (`""` inside a quoted cell for a literal quote — no newlines inside
//!    cells). Header columns, in any order: `type`, `stem`, `points`, `topic` (split into
//!    topics on `;`, `,`, `|`), answer columns named `a`…`h`, `correct` (letters like `b`
//!    or `a,c`; digits `1`… also accepted) and `correct_text`. An empty `type` cell
//!    defaults to `mc`. Types: `mc`/`ma` (answers from the letter columns; correct marks
//!    from `correct` and/or a `correct_text` cell that matches one answer exactly), `tf`
//!    (first char of `correct_text`/`correct`: T/Y → true, F/N → false; A/B also mean
//!    true/false), `short`/`ess`/`essay` and `blank`/`fib`/`num` (the `correct_text` cell
//!    is the accepted string / sample answer). An unparseable `points` cell is ignored
//!    (`None`); structural problems (no stem, no correct mark, unknown type) fail the
//!    whole import naming the row.
//! 4. **Question-block text**: Blackboard test-generator text (also what the AI converter
//!    returns and what "Paste your questions" takes) plus untagged Aiken blocks. See
//!    [`parse_blackboard`] for the grammar. The doc name is the filename stem, or
//!    "Pasted questions" for an untitled paste. Two hardenings from a live e2e corpus:
//!    a TF statement may have its answer glued onto the stem line's tail ("…liquid iron.
//!    False" — a bare, case-insensitive t/f/true/false word after its statement is the
//!    answer, stripped from the stem), and in a FIB stem a run of three-or-more
//!    underscores is exactly ONE blank (collapsed back to the canonical `___`).
//! 5. Text in any of the above arrives through one decoder: UTF-8 first (BOM stripped),
//!    then Latin-1 byte-for-byte, so accented files import instead of failing.
//!
//! Every imported question gets fresh ids, `created`/`modified` timestamps, and
//! `points: None` (CSV/package may fill points), `difficulty: None` (package may fill),
//! `topics: []` (CSV/package may fill), `explanation: None`, `image: None` (only a
//! package with inline media fills it).

use crate::model::{Answer, Difficulty, Exam, ExamItem, Kind, Pair, Part, PartKind, Question};

/// One successfully parsed import: a suggested bank name, its questions — and, for Test
/// Parrot packages that carry exams, their best-effort `model::Exam`s (empty otherwise;
/// the shell fills each item's empty `bank_id` with the bank it creates from this doc).
#[derive(Debug)]
pub struct ImportDoc {
    pub name: String,
    pub questions: Vec<Question>,
    pub exams: Vec<Exam>,
}

/// The exact not-recognized fallback error the build contract asks for.
const GENERIC: &str =
    "Not a recognized question file — expected Blackboard test text, Aiken, CSV, or a Test Parrot question-bank JSON";

const PACKAGE_FORMAT: &str = "test-parrot/package";
const RECORD_FORMAT: &str = "test-parrot/question-bank";
const LEGACY_KIND: &str = "test-parrot-question-bank";
/// model::Exam's default printed header (matches the model's documented default).
const DEFAULT_HEADER: &str = "Name: __________________ Class: ___________ Date: ___________";
/// Block type tags, matched case-insensitively as the whole first line (trailing `. : )` trimmed).
const TAGS: [&str; 8] = ["mc", "ma", "tf", "ess", "num", "mat", "fib", "ord"];
/// Default stem for a MAT block that has no stem text of its own.
const MAT_DEFAULT_STEM: &str = "Match each item on the left with its correct match on the right.";
/// Alternative separator inside one FIB answer line (exact, case-sensitive).
const OR: &str = " OR ";
/// Fill-in-the-blank marker a FIB stem must contain.
const BLANK: &str = "___";
/// Arrow separator accepted inside MAT pair lines: `left -> right`.
const ARROW: &str = " -> ";

pub fn detect_and_parse(data: &[u8], filename: &str) -> Result<ImportDoc, String> {
    let text = decode_text(data);
    let lower = filename.to_ascii_lowercase();
    let name = doc_name(filename);
    if let Some(result) = try_json(&text, &name, lower.ends_with(".json")) {
        return result;
    }
    if let Some(result) = try_csv(&text) {
        return result.map(|questions| ImportDoc { name, questions, exams: Vec::new() });
    }
    parse_blocks(&text, &name)
}

/// The Blackboard test-generator format (plus untagged Aiken blocks), used for a paste.
/// The full grammar is documented in the module docs: question blocks separated by blank
/// lines, a type tag on a block's own first line, `*` marks the correct choice, and the
/// same stems/stems-wrap and pairing conventions described there.
pub fn parse_blackboard(text: &str) -> Result<ImportDoc, String> {
    parse_blocks(text, "Pasted questions")
}

fn parse_blocks(text: &str, name: &str) -> Result<ImportDoc, String> {
    let mut questions: Vec<Question> = Vec::new();
    let mut failed: Option<String> = None;
    let mut claimed = 0usize; // count only blocks we tried as questions, for messages
    for block in split_blocks(text) {
        let Some((first, rest)) = block.split_first() else { unreachable!("split_blocks skips empties") };
        if let Some(tag) = tag_of(first) {
            claimed += 1;
            match parse_tagged(tag, rest).map(|(stem, kind)| make_question(stem, kind)) {
                Ok(q) => questions.push(q),
                Err(e) => failed = failed.or_else(|| Some(format!("question {claimed}: {e}"))),
            }
        } else if rest.iter().filter(|l| is_choice(l)).count() >= 2 {
            claimed += 1;
            match parse_aiken(&block) {
                Ok(q) => questions.push(q),
                Err(e) => failed = failed.or_else(|| Some(format!("question {claimed}: {e}"))),
            }
        }
        // Anything else is prose (a title line, section headers): skipped.
    }
    if let Some(e) = failed {
        return Err(e);
    }
    if questions.is_empty() {
        return Err(GENERIC.to_string());
    }
    Ok(ImportDoc { name: name.to_string(), questions, exams: Vec::new() })
}

/// The whole first line, case-insensitively, optionally ending with `. : )`, is the tag.
fn tag_of(line: &str) -> Option<&'static str> {
    let core = line.trim().trim_end_matches(['.', ')', ':']).trim().to_ascii_lowercase();
    TAGS.iter().find(|t| **t == core).copied()
}

// ---------- tagged blocks ----------

fn parse_tagged(tag: &str, lines: &[String]) -> Result<(String, Kind), String> {
    match tag {
        "mc" | "ma" => {
            let (stem, answers) = parse_mc(lines)?;
            Ok((stem, Kind::Mc { answers }))
        }
        "tf" => {
            let mut stem_lines: Vec<String> = Vec::new();
            let mut correct: Option<bool> = None;
            for line in lines {
                let low = line.to_ascii_lowercase();
                if correct.is_none() {
                    // Primary rule: a line of its own whose (punctuation-stripped, once)
                    // whole text is the answer word: T, true, false…
                    match low.trim_end_matches([',', ';', ':', '.', '!', '?']) {
                        "t" | "true" => {
                            correct = Some(true);
                            continue;
                        }
                        "f" | "false" => {
                            correct = Some(false);
                            continue;
                        }
                        _ => {}
                    }
                }
                stem_lines.push(line.clone());
            }
            // Hardening (live corpus): the model may append the answer INLINE at the end
            // of the stem line ("…made of liquid iron. False"); a bare t/f/true/false
            // word there is stripped and taken as the answer.
            if correct.is_none()
                && let Some(last) = stem_lines.last_mut()
            {
                correct = inline_tf(last).map(|(prefix, flag)| {
                    *last = prefix;
                    flag
                });
            }
            let stem = stem_lines.join(" ");
            if stem.is_empty() {
                return Err("no stem — write the statement on the line after the TF tag, and T or F on a later line".into());
            }
            let correct = correct.ok_or("no answer — add a T or F (its own line, or after the statement)")?;
            Ok((stem, Kind::Tf { correct }))
        }
        "ess" => {
            let (stem, sample) = first_and_rest(lines, "short-answer")?;
            Ok((stem, Kind::Short { sample: sample.join("\n") }))
        }
        "num" => {
            let mut stem = String::new();
            let mut accept: Option<Vec<String>> = None;
            for line in lines {
                if accept.is_none()
                    && let Some(variants) = parse_num_answer(line)
                {
                    accept = Some(variants);
                } else if stem.is_empty() {
                    stem = line.clone();
                } else {
                    stem.push(' ');
                    stem.push_str(line);
                }
            }
            if stem.is_empty() {
                return Err("no stem — the first line after the NUM tag must be the question text".into());
            }
            let accept = accept.ok_or("no numeric answer — add a line with just the number (like 4.5 or 1.5±0.02)")?;
            Ok((stem, Kind::Blank { accept: vec![accept] }))
        }
        "fib" => {
            let (raw_stem, answers) = first_and_rest(lines, "fill-in-the-blank")?;
            // Harden: any run of 3+ underscores is ONE blank; collapse runs so the stem
            // prints the canonical `___` marker the rest of the app counts.
            let stem = normalize_blanks(&raw_stem);
            let blanks = blank_count(&stem);
            if blanks == 0 {
                return Err("the stem has no ___ blank markers — write ___ where the answer goes".into());
            }
            if answers.len() < blanks {
                return Err(format!(
                    "the stem has {blanks} {BLANK} markers but only {} answer lines — one line per blank; alternatives on a line are separated by {OR}",
                    answers.len()
                ));
            }
            if answers.len() > blanks {
                return Err(format!("the stem has {blanks} {BLANK} markers but {} answer lines — one line per blank", answers.len()));
            }
            let accept = answers
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let alts: Vec<String> = line.split(OR).map(str::trim).filter(|a| !a.is_empty()).map(str::to_string).collect();
                    if alts.is_empty() { Err(format!("no accepted answer for blank {}", i + 1)) } else { Ok(alts) }
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((stem, Kind::Blank { accept }))
        }
        "mat" => {
            let split_at = lines.iter().position(|l| l.trim() == "-");
            let (pair_lines, distractor_lines): (&[String], &[String]) = match split_at {
                Some(i) => (&lines[..i], &lines[(i + 1)..]),
                None => (lines, &[]),
            };
            if pair_lines.is_empty() {
                return Err(
                    "no matching pairs — write pairs of lines (left item, right item), then a line containing only `-`, then any distractor lines".into(),
                );
            }
            let pairs: Vec<Pair> = if pair_lines.iter().all(|l| l.contains(ARROW) || l.contains('→')) {
                pair_lines
                    .iter()
                    .map(|l| {
                        let (left, right) = l
                            .split_once(ARROW)
                            .or_else(|| l.split_once('→'))
                            .ok_or_else(|| "no arrow".to_string())?;
                        Ok(Pair { left: left.trim().to_string(), right: right.trim().to_string() })
                    })
                    .collect::<Result<Vec<_>, String>>()?
            } else if pair_lines.len() % 2 == 0 {
                pair_lines.chunks(2).map(|c| Pair { left: c[0].clone(), right: c[1].clone() }).collect()
            } else {
                return Err(
                    "an odd number of pair lines — write pairs as alternating left/right lines, or one `left -> right` line per pair".into(),
                );
            };
            let distractors = distractor_lines.iter().map(|l| l.trim().to_string()).collect();
            Ok((MAT_DEFAULT_STEM.to_string(), Kind::Matching { pairs, distractors }))
        }
        "ord" => {
            let (stem, answers) = first_and_rest(lines, "ordering")?;
            if answers.is_empty() {
                return Err("no answer lines — write the answers one per line, in their correct order".into());
            }
            // Storing choice: one fill-in-the-blank accepting the comma-joined ordered list.
            let ordered = answers.join(", ");
            Ok((stem, Kind::Blank { accept: vec![vec![ordered]] }))
        }
        _ => Err("unknown type tag".into()), // unreachable: tags come from TAGS
    }
}

/// First line = stem; the rest = that kind's content lines.
fn first_and_rest<'a>(lines: &'a [String], kind: &str) -> Result<(String, &'a [String]), String> {
    match lines.split_first() {
        Some((stem, rest)) => Ok((stem.clone(), rest)),
        None => Err(format!("no stem — write the {kind} text on the line after the tag")),
    }
}

/// The TF hardening: a stem line that ENDS in the answer ("…liquid iron. False") — the
/// last word (case-insensitive, trailing punctuation allowed) is a bare t/f/true/false
/// word after whitespace; returns the line cut back to its stem (stem punctuation kept)
/// and the answer. `None` when the tail is not a bare answer word.
fn inline_tf(line: &str) -> Option<(String, bool)> {
    let t = line.trim_end();
    let (prefix, word) = t.rsplit_once(char::is_whitespace)?;
    let word = word.trim_end_matches([',', ';', ':', '.', '!', '?']);
    let flag = if word.eq_ignore_ascii_case("t") || word.eq_ignore_ascii_case("true") {
        true
    } else if word.eq_ignore_ascii_case("f") || word.eq_ignore_ascii_case("false") {
        false
    } else {
        return None;
    };
    let prefix = prefix.trim_end();
    if prefix.is_empty() {
        return None; // a lone word is not an inline tail on a stem
    }
    Some((prefix.to_string(), flag))
}

/// Counts fill-in-the-blank markers: a run of THREE OR MORE underscores is one blank.
/// (The AI prompt says `___`, but live models frequently type a run like `__________`;
/// a literal `___` match count would read one run as several blanks.)
fn blank_count(stem: &str) -> usize {
    let mut count = 0usize;
    let mut run = 0usize;
    for b in stem.bytes() {
        if b == b'_' {
            run += 1;
        } else {
            count += usize::from(run >= 3);
            run = 0;
        }
    }
    count + usize::from(run >= 3)
}

/// Collapses every run of 3+ underscores into exactly one `___` marker, so downstream
/// code that counts literal `___` markers sees the blanks this parser counted.
fn normalize_blanks(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    let mut run = 0usize;
    for c in stem.chars() {
        if c == '_' {
            run += 1;
            continue;
        }
        out.push_str(&if run >= 3 { BLANK.to_string() } else { "_".repeat(run) });
        run = 0;
        out.push(c);
    }
    out.push_str(&if run >= 3 { BLANK.to_string() } else { "_".repeat(run) });
    out
}

/// MC/MA lines: choices (correct-star or letter-dot) collected in order; everything else
/// is stem — the first such line, with later ones space-appended so long stems may wrap.
fn parse_mc(lines: &[String]) -> Result<(String, Vec<Answer>), String> {
    let mut stem = String::new();
    let mut answers: Vec<Answer> = Vec::new();
    for line in lines {
        match parse_choice(line) {
            Some((correct, text)) if !text.is_empty() => answers.push(Answer { text, correct, locked: false }),
            Some((..)) => {} // a bare `a.` or lone `*`: dropped as an empty choice
            None => {
                if stem.is_empty() {
                    stem = line.clone();
                } else {
                    stem.push(' ');
                    stem.push_str(line);
                }
            }
        }
    }
    if stem.is_empty() {
        return Err("no stem — put the question text after the tag (choice lines start with * or a letter like `a.`)".into());
    }
    if answers.is_empty() {
        return Err("no answer choices — put each choice on its own line and mark the correct one with a leading *".into());
    }
    Ok((stem, answers))
}

/// Is this line a choice in an MC/MA or Aiken block (with a non-empty choice text)?
fn is_choice(line: &str) -> bool {
    parse_choice(line).is_some_and(|(_, text)| !text.is_empty())
}

/// A choice line starts with `*` (optional space after; the star marks it correct and stays
/// off the text) or with a letter-dot/letter-paren pair like `a.` / `B)` (letters A…I).
/// A plain stem line returns `None`. Deliberately empty choices (`a.`, lone `*`) come back
/// with empty text for the caller to drop — this also keeps them out of the stem.
fn parse_choice(line: &str) -> Option<(bool, String)> {
    let mut work = line.trim();
    if work.is_empty() {
        return None;
    }
    let mut correct = false;
    if let Some(rest) = work.strip_prefix('*') {
        correct = true;
        work = rest.trim_start();
        if work.is_empty() {
            return None;
        }
    }
    let bytes = work.as_bytes();
    let lower = bytes[0].to_ascii_lowercase();
    let letter_dot = bytes.len() >= 2 && (b'a'..=b'j').contains(&lower) && (bytes[1] == b'.' || bytes[1] == b')');
    if letter_dot {
        Some((correct, work[2..].trim().to_string()))
    } else if correct {
        Some((true, work.to_string()))
    } else {
        None
    }
}

/// An untagged Aiken block: stem lines, then lettered choices; the correct one starts with `*`.
fn parse_aiken(block: &[String]) -> Result<Question, String> {
    let choice_at = block.iter().position(|l| is_choice(l));
    match choice_at {
        Some(0) => Err("an Aiken question needs stem lines before its choices".into()),
        Some(i) => {
            let stem = block[..i].join(" ");
            let mut answers: Vec<Answer> = Vec::new();
            for line in &block[i..] {
                match parse_choice(line) {
                    Some((correct, text)) if !text.is_empty() => answers.push(Answer { text, correct, locked: false }),
                    _ => return Err(format!("not a choice line after the stem: {line:?}")),
                }
            }
            if !answers.iter().any(|a| a.correct) {
                return Err("no correct choice — mark it by putting a * in front of the choice line".into());
            }
            Ok(make_question(stem, Kind::Mc { answers }))
        }
        None => Err("no choice lines found".into()),
    }
}

// ---------- NUM answers ----------

/// Accept-string variants for one NUM answer line: a plain number (`3.5`), a fraction
/// (`3/8` — kept as printed plus its decimal value), or a range `value±tolerance`
/// (also `+/-`/`+-`), recorded as center, center−tol, center+tol; the expanded pair is
/// formatted to the tolerance's decimals so `1.5±0.02` yields `1.5`, `1.48`, `1.52`.
fn parse_num_answer(line: &str) -> Option<Vec<String>> {
    let w = line.trim();
    if w.is_empty() {
        return None;
    }
    // Fraction like 3/8 (exactly one slash, integer parts).
    if w.split('/').count() == 2 {
        let Some((n, d)) = w.split_once('/') else { unreachable!("count of one slash") };
        let parts = (n.trim().parse::<i64>(), d.trim().parse::<i64>());
        if let (Ok(a), Ok(b)) = parts
            && b != 0
        {
            return Some(vec![format!("{a}/{b}"), fmt_num(a as f64 / b as f64)]);
        }
        return None;
    }
    let (base, tol) = split_range(w);
    let Some(v) = parse_f64(base) else { return None };
    let mut out = vec![fmt_num(v)];
    if let Some(tol) = tol
        && let Some(t) = parse_f64(&tol)
    {
        let dp = decimals(base).max(decimals(&tol));
        out.push(fmt_dp(v - t.abs(), dp));
        out.push(fmt_dp(v + t.abs(), dp));
    }
    Some(out)
}

/// `1.5±0.02`, `1.5 +/- 0.02`, `1.5+-0.02` → (value text, tolerance text).
fn split_range(w: &str) -> (&str, Option<String>) {
    for sep in ["±", "+/-", "+-"] {
        if let Some(i) = w.find(sep) {
            let (left, right) = (&w[..i], &w[(i + sep.len())..]);
            if parse_f64(left).is_some() {
                return (left.trim(), Some(right.trim().to_string()));
            }
        }
    }
    (w, None)
}

/// Plain finite number only (no `inf`/`NaN`/mixed text).
fn parse_f64(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() { return None; }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn fmt_num(v: f64) -> String {
    format!("{v}") // Rust's shortest display drops the fraction of integral values
}

fn fmt_dp(v: f64, dp: usize) -> String {
    if dp == 0 { format!("{}", v.round() as i64) } else { format!("{v:.dp$}") }
}

/// Decimal places in a plain decimal literal: `1.48` → 2.
fn decimals(s: &str) -> usize {
    match s.split_once('.') {
        Some((_, frac)) => frac.len(),
        None => 0,
    }
}

// ---------- CSV ----------

/// Header-driven CSV only; returns `None` when the first line does not read as a header
/// naming both `type` and `stem` (so a `.csv` that is really Blackboard text still parses).
fn try_csv(text: &str) -> Option<Result<Vec<Question>, String>> {
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).filter(|l| !l.trim().is_empty()).collect();
    let Some(first) = lines.first() else { return None };
    let Some(cols) = csv_cols(&split_csv_line(first)) else { return None };

    let mut questions: Vec<Question> = Vec::new();
    for (n, line) in lines.iter().enumerate().skip(1) {
        let row = n; // 1-based data-row number, for messages
        let cells = split_csv_line(line);
        let get = |col: Option<usize>| col.and_then(|i| cells.get(i)).map(|s| s.trim().to_string()).unwrap_or_default();
        let stem = get(Some(cols.stem));
        if stem.is_empty() {
            return Some(Err(format!("CSV row {row}: the stem is empty")));
        }
        let ty = {
            let t = get(Some(cols.ty));
            let norm = t.to_ascii_lowercase().chars().filter(char::is_ascii_alphanumeric).collect::<String>();
            if norm.is_empty() { "mc".to_string() } else { norm }
        };
        let kind = match ty.as_str() {
            "mc" | "ma" | "multiplechoice" | "multipleanswers" | "multipleanswer" => {
                let mut answers: Vec<Answer> = cols
                    .answers
                    .iter()
                    .filter_map(|c| *c)
                    .map(|i| get(Some(i)))
                    .filter(|t| !t.is_empty())
                    .map(|text| Answer { text, correct: false, locked: false })
                    .collect();
                if answers.is_empty() {
                    return Some(Err(format!("CSV row {row}: an MC row has no answers — fill the a…h columns")));
                }
                let letters = get(cols.correct);
                let text_mark = get(cols.correct_text);
                if !letters.is_empty() {
                    for i in correct_letters(&letters) {
                        match answers.get_mut(i) {
                            Some(a) => a.correct = true,
                            None => {
                                return Some(Err(format!(
                                    "CSV row {row}: the `correct` column ({letters:?}) names answer {} but only {} answers are given",
                                    i + 1,
                                    answers.len()
                                )));
                            }
                        }
                    }
                }
                if !text_mark.is_empty() {
                    match answers.iter_mut().find(|a| a.text.eq_ignore_ascii_case(&text_mark)) {
                        Some(a) => a.correct = true,
                        None => {
                            return Some(Err(format!(
                                "CSV row {row}: `correct_text` {text_mark:?} does not match any of the answers exactly"
                            )));
                        }
                    }
                }
                if !answers.iter().any(|a| a.correct) {
                    return Some(Err(format!(
                        "CSV row {row}: no correct answer marked — fill `correct` (like `b` or `a,c`) or `correct_text`"
                    )));
                }
                Kind::Mc { answers }
            }
            "tf" | "truefalse" | "t" => {
                let mark = first_non_empty(get(cols.correct_text), get(cols.correct));
                let correct = match mark.chars().next() {
                    Some(c) => match c.to_ascii_lowercase() {
                        't' | 'y' | 'a' => true,
                        'f' | 'n' | 'b' => false,
                        _ => return Some(Err(format!("CSV row {row}: a TF row needs T/F (or Y/N, A/B); got {mark:?}"))),
                    },
                    None => return Some(Err(format!("CSV row {row}: a TF row needs its answer in `correct` or `correct_text` (T or F)"))),
                };
                Kind::Tf { correct }
            }
            "short" | "ess" | "essay" | "shortanswer" | "essays" => {
                let sample = first_non_empty(get(cols.correct_text), get(cols.correct));
                if sample.is_empty() {
                    return Some(Err(format!("CSV row {row}: a short-answer row needs `correct_text` (a sample answer)")));
                }
                Kind::Short { sample }
            }
            "blank" | "fib" | "num" | "numeric" | "fillintheblank" | "numericanswer" => {
                let text = first_non_empty(get(cols.correct_text), get(cols.correct));
                if text.is_empty() {
                    return Some(Err(format!("CSV row {row}: a blank row needs `correct_text` (the accepted answer)")));
                }
                Kind::Blank { accept: vec![vec![text]] }
            }
            other => {
                return Some(Err(format!(
                    "CSV row {row}: unsupported type {other:?} — supported: mc, ma, tf, short/ess, blank/fib/num; matching must be imported as Blackboard text"
                )));
            }
        };
        let raw_points = get(cols.points);
        let points = if raw_points.is_empty() { None } else { raw_points.parse::<f64>().ok().filter(|p| p.is_finite()) };
        let topics: Vec<String> = get(cols.topic).split([';', ',', '|']).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect();
        let mut q = make_question(stem, kind);
        q.points = points;
        q.topics = topics;
        questions.push(q);
    }
    if questions.is_empty() {
        return Some(Err("The CSV header names questions but there are no data rows.".into()));
    }
    Some(Ok(questions))
}

/// Reads the header row into column positions. `None` unless `type` and `stem` both appear.
fn csv_cols(header: &[String]) -> Option<CsvCols> {
    let mut ty: Option<usize> = None;
    let mut stem: Option<usize> = None;
    let mut points: Option<usize> = None;
    let mut topic: Option<usize> = None;
    let mut correct: Option<usize> = None;
    let mut correct_text: Option<usize> = None;
    let mut answers = [None::<usize>; 8];
    for (i, cell) in header.iter().enumerate() {
        let key = cell.trim().to_ascii_lowercase().replace([' ', '-', '_'], "");
        match key.as_str() {
            "type" | "kind" | "qtype" => ty = ty.or(Some(i)),
            "stem" | "question" | "questiontext" => stem = stem.or(Some(i)),
            "points" | "pointvalue" | "value" => points = points.or(Some(i)),
            "topic" | "topics" | "categories" => topic = topic.or(Some(i)),
            "correct" | "answer" | "key" | "answers" => correct = correct.or(Some(i)),
            "correcttext" | "answertext" | "keytext" => correct_text = correct_text.or(Some(i)),
            "a" | "b" | "c" | "d" | "e" | "f" | "g" | "h" => {
                let idx = key.as_bytes()[0] - b'a';
                answers[idx as usize] = answers[idx as usize].or(Some(i));
            }
            _ => {}
        }
    }
    match (ty, stem) {
        (Some(ty), Some(stem)) => Some(CsvCols { ty, stem, points, topic, answers, correct, correct_text }),
        _ => None,
    }
}

struct CsvCols {
    ty: usize,
    stem: usize,
    points: Option<usize>,
    topic: Option<usize>,
    answers: [Option<usize>; 8],
    correct: Option<usize>,
    correct_text: Option<usize>,
}

/// `correct` cell letters/digits → answer indexes (`b` → 1, `a,c` → [0, 2], `1` → 0).
fn correct_letters(s: &str) -> Vec<usize> {
    s.chars()
        .filter_map(|c| {
            if c.is_whitespace() || c == ',' || c == ';' {
                return None;
            }
            if let Some(d) = c.to_digit(10) {
                if d >= 1 { Some(d as usize - 1) } else { None }
            } else {
                let lower = c.to_ascii_lowercase();
                if ('a'..='h').contains(&lower) { Some(lower as usize - 'a' as usize) } else { None }
            }
        })
        .collect()
}

/// First non-empty of two option-like cells.
fn first_non_empty(a: String, b: String) -> String {
    if a.is_empty() { b } else { a }
}

/// Splits one CSV line, honoring double quotes and doubled `""` inside them.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => cells.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    cells.push(cur);
    cells
}

// ---------- JSON: Test Parrot package, record, and legacy bank ----------

/// The Test Parrot JSON entry point. A `.json` file is claimed even when it fails;
/// otherwise only content that really looks Test Parrot (`test-parrot/package` or
/// `test-parrot/question-bank` in the bytes) or a legacy `"kind"` bank is claimed.
fn try_json(text: &str, fallback_name: &str, forced: bool) -> Option<Result<ImportDoc, String>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let claimed = forced || trimmed.contains("test-parrot/");
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(v) => {
            match v.get("format").and_then(serde_json::Value::as_str).unwrap_or("") {
                PACKAGE_FORMAT => return Some(parse_package(&v, fallback_name)),
                RECORD_FORMAT => return Some(record_import(&v, fallback_name)),
                _ => {}
            }
            if v.get("kind").and_then(serde_json::Value::as_str) == Some(LEGACY_KIND) {
                return Some(legacy_bank(&v, fallback_name));
            }
            if claimed {
                return Some(Err(
                    "This JSON mentions Test Parrot formats but carries none: expected \"format\": \"test-parrot/package\" or \"test-parrot/question-bank\".".into(),
                ));
            }
            None
        }
        Err(e) => {
            if claimed {
                Some(Err(format!(
                    "That file is not valid JSON ({e}). A Test Parrot package looks like {{\"format\": \"test-parrot/package\", \"questionBanks\": [..], \"exams\": [..]}}"
                )))
            } else {
                None // not a JSON file we claim — let the text parsers take a look
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct JsonBank {
    #[serde(default)]
    name: String,
    #[serde(default)]
    questions: Vec<Question>,
}

/// The legacy invented shape (`"kind": "test-parrot-question-bank"`): a full bank.
fn legacy_bank(v: &serde_json::Value, fallback_name: &str) -> Result<ImportDoc, String> {
    let bank: JsonBank = match serde_json::from_value(v.clone()) {
        Ok(b) => b,
        Err(e) => return Err(format!("The Test Parrot question-bank JSON is malformed: {e}")),
    };
    if bank.questions.is_empty() {
        return Err("The question bank holds no questions — its \"questions\" list is empty or missing.".into());
    }
    let questions = bank.questions.into_iter().map(|mut q| { q.id = crate::model::new_id(); q }).collect();
    let name = if bank.name.trim().is_empty() { fallback_name.to_string() } else { bank.name };
    Ok(ImportDoc { name, questions, exams: Vec::new() })
}

/// A bare Question Bank Record: `{"format": .., "bank": {"name", "questions"}, "media"}`.
fn record_import(v: &serde_json::Value, fallback_name: &str) -> Result<ImportDoc, String> {
    let media = build_media(v);
    let bank = v.get("bank").ok_or("The Question Bank Record has no \"bank\" object")?;
    let (name, pairs) = tp_bank_questions(bank, &media)?;
    let name = if name.trim().is_empty() { fallback_name.to_string() } else { name };
    Ok(ImportDoc { name, questions: pairs.into_iter().map(|(_, q)| q).collect(), exams: Vec::new() })
}

/// Media assets of one record, keyed by their digest (`sha256:…` without the prefix).
fn build_media(record: &serde_json::Value) -> std::collections::HashMap<String, serde_json::Value> {
    let mut map = std::collections::HashMap::new();
    if let Some(list) = record.get("media").and_then(serde_json::Value::as_array) {
        for entry in list {
            if let Some(id) = entry.get("id").and_then(serde_json::Value::as_str).and_then(|a| a.strip_prefix("sha256:")) {
                map.insert(id.to_string(), entry.clone());
            }
        }
    }
    map
}

/// The inline base64 an exporter may embed beside a media entry (`data`, `bytes`, or
/// `base64` key); the 0.10.0 schema names none of these keys, so attachment is a bonus.
fn media_bytes(entry: &serde_json::Value) -> Option<Vec<u8>> {
    ["data", "bytes", "base64"]
        .iter()
        .find_map(|k| entry.get(*k).and_then(serde_json::Value::as_str))
        .and_then(base64_decode)
}

/// A stem's first resolvable picture becomes the question's image; the rest stay text.
fn attach_first_image(images: &[String], media: &std::collections::HashMap<String, serde_json::Value>, q: &mut Question) {
    for hex in images {
        let Some(entry) = media.get(hex.as_str()) else { continue };
        if let Some(bytes) = media_bytes(entry)
            && let Ok(img) = crate::store::put_image(&bytes)
        {
            q.image = Some(img);
            return;
        }
    }
}

// ---------- Test Parrot question mapping ----------

/// One whole package: every bank's questions into one doc, plus best-effort exams.
fn parse_package(v: &serde_json::Value, fallback_name: &str) -> Result<ImportDoc, String> {
    let banks = arr(v, "questionBanks");
    if banks.is_empty() {
        return Err("The Test Parrot package has no \"questionBanks\".".into());
    }
    let mut name = String::new();
    let mut questions: Vec<Question> = Vec::new();
    // "bankId/packageQuestionId" from the package → this import's fresh question id.
    let mut lookup: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut first_err: Option<String> = None;
    for entry in banks {
        let record = entry.get("record").ok_or("A package \"questionBanks\" entry has no embedded \"record\"")?;
        let bank = record.get("bank").ok_or("An embedded question-bank record has no \"bank\" object")?;
        let media = build_media(record);
        match tp_bank_questions(bank, &media) {
            Ok((bank_name, pairs)) => {
                if name.is_empty() {
                    name = bank_name;
                }
                let bank_id = entry.get("id").and_then(serde_json::Value::as_str).unwrap_or("");
                for (pkg_qid, q) in pairs {
                    lookup.insert(format!("{bank_id}/{pkg_qid}"), q.id.clone());
                    questions.push(q);
                }
            }
            Err(e) => first_err = first_err.or_else(|| Some(e)),
        }
    }
    if questions.is_empty() {
        return Err(first_err.unwrap_or_else(|| "the package's question banks hold no importable questions".into()));
    }
    let name = if name.trim().is_empty() { fallback_name.to_string() } else { name };
    Ok(ImportDoc { name, questions, exams: package_exams(v, &lookup) })
}

/// A bank object's questions, in authored order, as (package id, fresh Question) pairs.
/// All-or-nothing per bank: a question that fails names itself in the error.
fn tp_bank_questions(bank: &serde_json::Value, media: &std::collections::HashMap<String, serde_json::Value>) -> Result<(String, Vec<(String, Question)>), String> {
    let raw = bank.get("questions").and_then(serde_json::Value::as_array).ok_or("The bank has no \"questions\" array")?;
    let mut pairs: Vec<(String, Question)> = Vec::new();
    let mut first_err: Option<String> = None;
    for item in raw {
        match tp_question(item, media) {
            Ok(q) => pairs.push(q),
            Err(e) => first_err = first_err.or_else(|| Some(e)),
        }
    }
    if pairs.is_empty() {
        return Err(first_err.unwrap_or_else(|| "the bank holds no questions".into()));
    }
    Ok((field_str(bank, "name").unwrap_or("").to_string(), pairs))
}

fn field_str<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(serde_json::Value::as_str)
}

/// One record question → (package id, model::Question); Kind mapping is best effort.
fn tp_question(item: &serde_json::Value, media: &std::collections::HashMap<String, serde_json::Value>) -> Result<(String, Question), String> {
    let pkg_id = field_str(item, "id").filter(|s| !s.is_empty()).ok_or("every question needs an id")?.to_string();
    let ty = field_str(item, "type").ok_or_else(|| format!("question {pkg_id} has no type"))?.to_string();
    let stem_node = item.get("stem").ok_or_else(|| format!("question {pkg_id} has no stem document"))?;
    let flat = flatten_doc(stem_node);
    let kind = match ty.as_str() {
        "multiple-choice" => tp_choice_answers(item).map(|answers| Kind::Mc { answers }),
        "true-false" => tp_true_false(item),
        "matching" => tp_matching(item),
        "fill-in-the-blank" if flat.blanks.is_empty() => {
            Err(format!("question {pkg_id} has no ___ blanks in its stem"))
        }
        "fill-in-the-blank" => Ok(Kind::Blank { accept: flat.blanks.iter().map(|b| vec![b.clone()]).collect() }),
        "short-answer" => Ok(Kind::Short { sample: item.get("suggestedAnswer").map(doc_text).unwrap_or_default() }),
        "multipart" => tp_parts(item).map(|parts| Kind::Multipart { parts }),
        other => Err(format!("question {pkg_id}: unsupported Test Parrot type {other:?}")),
    }
    .map_err(|e| format!("question {pkg_id}: {e}"))?;
    let mut q = make_question(flat.text, kind);
    attach_first_image(&flat.images, media, &mut q);
    q.difficulty = match field_str(item, "difficulty") {
        Some("easy") => Some(Difficulty::Low),
        Some("medium") => Some(Difficulty::Medium),
        Some("hard") => Some(Difficulty::High),
        _ => None,
    };
    q.topics = arr(item, "topics").iter().filter_map(serde_json::Value::as_str).filter(|t| !t.is_empty()).map(str::to_string).collect();
    q.points = item.get("points").and_then(serde_json::Value::as_f64).filter(|p| p.is_finite()).filter(|p| *p > 0.0);
    Ok((pkg_id, q))
}

/// The `choices` array of a question, part or subpart → model answers (all three share
/// the same choice shape: `content` document, `correct`, optional `locked`).
fn tp_choice_answers(item: &serde_json::Value) -> Result<Vec<Answer>, String> {
    let Some(choices) = item.get("choices").and_then(serde_json::Value::as_array) else {
        return Err("the question has no \"choices\" array".into());
    };
    let answers: Vec<Answer> = choices
        .iter()
        .map(|c| Answer {
            text: doc_text(c),
            correct: c.get("correct").and_then(serde_json::Value::as_bool).unwrap_or(false),
            locked: c.get("locked").and_then(serde_json::Value::as_bool).unwrap_or(false),
        })
        .collect();
    if answers.is_empty() { Err("the question has no choices".into()) } else { Ok(answers) }
}

fn tp_true_false(item: &serde_json::Value) -> Result<Kind, String> {
    let choices = item
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .ok_or("a True/False question needs exactly two choices (True, False)")?;
    if choices.len() != 2 {
        return Err(format!(
            "a True/False question needs exactly two choices (True, False); this record has {}",
            choices.len()
        ));
    }
    match choices.iter().position(|c| c.get("correct").and_then(serde_json::Value::as_bool) == Some(true)) {
        Some(0) => Ok(Kind::Tf { correct: true }),
        Some(1) => Ok(Kind::Tf { correct: false }),
        _ => Err("a True/False question needs exactly one of its two choices marked correct".into()),
    }
}

fn tp_matching(item: &serde_json::Value) -> Result<Kind, String> {
    let prompts = item.get("prompts").and_then(serde_json::Value::as_array).ok_or("a matching question has no \"prompts\"")?;
    let word_bank = item.get("wordBank").and_then(serde_json::Value::as_array).ok_or("a matching question has no \"wordBank\"")?;
    if prompts.is_empty() {
        return Err("a matching question needs at least one prompt".into());
    }
    let mut pairs: Vec<Pair> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    for p in prompts {
        // Best effort: a prompt that names no wordBank answer is skipped.
        let Some(ans) = p.get("answer").and_then(serde_json::Value::as_str) else { continue };
        let Some(right) = word_bank.iter().find(|w| field_str(w, "id") == Some(ans)) else { continue };
        pairs.push(Pair { left: doc_text(p), right: doc_text(right) });
        used.push(ans.to_string());
    }
    if pairs.is_empty() {
        return Err("no matching pair resolves — every prompt's \"answer\" must name a wordBank id".into());
    }
    let distractors: Vec<String> = word_bank
        .iter()
        .filter(|w| field_str(w, "id").is_none_or(|id| !used.iter().any(|u| u == id)))
        .map(doc_text)
        .collect();
    Ok(Kind::Matching { pairs, distractors })
}

fn tp_parts(item: &serde_json::Value) -> Result<Vec<Part>, String> {
    let raw = item
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .filter(|p| !p.is_empty())
        .ok_or("a multipart question needs at least one part")?;
    let mut out: Vec<Part> = Vec::new();
    for p in raw {
        tp_one_part(p, &mut out)?;
    }
    Ok(out)
}

/// One part → one `model::Part`; a part carrying subparts (a lead-in stem, no answer of
/// its own) becomes its lead-in stem plus one part per subpart, since `PartKind` cannot
/// nest. Subparts' own points are not representable in `model::Part` and are dropped.
fn tp_one_part(p: &serde_json::Value, out: &mut Vec<Part>) -> Result<(), String> {
    let part_id = field_str(p, "id").unwrap_or("?").to_string();
    let stem = p.get("stem").map(doc_text).unwrap_or_default();
    let subparts = arr(p, "subparts");
    if subparts.is_empty() {
        let kind = tp_answer_kind(p).map_err(|e| format!("part {part_id}: {e}"))?;
        out.push(Part { stem, kind });
        return Ok(());
    }
    out.push(Part { stem, kind: PartKind::Short { sample: String::new() } });
    for sp in subparts {
        let sp_id = field_str(sp, "id").unwrap_or("?").to_string();
        match tp_answer_kind(sp) {
            Ok(kind) => out.push(Part { stem: sp.get("stem").map(doc_text).unwrap_or_default(), kind }),
            Err(e) => return Err(format!("subpart {sp_id}: {e}")),
        }
    }
    Ok(())
}

/// A part's or subpart's own answer: multiple-choice choices, else a suggested answer.
fn tp_answer_kind(item: &serde_json::Value) -> Result<PartKind, String> {
    if item.get("choices").is_some() {
        return Ok(PartKind::Mc { answers: tp_choice_answers(item)? });
    }
    if let Some(sa) = item.get("suggestedAnswer") {
        return Ok(PartKind::Short { sample: doc_text(sa) });
    }
    Err("the part answers nothing (needs choices or a suggested answer)".into())
}

/// Package exam records → best-effort `model::Exam`s: items in `positions` order, resolved
/// against this import's fresh question ids, with `bank_id` empty for the shell to fill.
/// Sections, columns, answerOrder, hiddenAnswers, margins and paper style are ignored, and
/// exams with no resolvable position are skipped.
fn package_exams(v: &serde_json::Value, lookup: &std::collections::HashMap<String, String>) -> Vec<Exam> {
    let mut out = Vec::new();
    let Some(exams) = v.get("exams").and_then(serde_json::Value::as_array) else { return out };
    for record in exams {
        let Some(name) = field_str(record, "name").filter(|n| !n.is_empty()) else { continue };
        let mut items: Vec<ExamItem> = Vec::new();
        for pos in arr(record, "positions") {
            let Some(bank) = pos.pointer("/question/bank").and_then(serde_json::Value::as_str) else { continue };
            let Some(qid) = pos.pointer("/question/question").and_then(serde_json::Value::as_str) else { continue };
            let Some(q) = lookup.get(&format!("{bank}/{qid}")) else { continue };
            items.push(ExamItem { bank_id: String::new(), question_id: q.clone() });
        }
        if items.is_empty() {
            continue;
        }
        let header = record
            .get("header")
            .and_then(|h| h.get("first"))
            .and_then(serde_json::Value::as_str)
            .filter(|h| !h.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| DEFAULT_HEADER.to_string());
        out.push(Exam {
            id: crate::model::new_id(),
            name: name.to_string(),
            header,
            title: None,
            items,
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 0,
            // Fresh import: zero is "never edited since import".
            modified: 0,
        });
    }
    out
}

// ---------- Test Parrot document flattening ----------

/// One flattened stem/suggested-answer document: newline-joined block text, the `___`
/// answers in stem order, and the image assets the document shows, in order.
#[derive(Default)]
struct Flat {
    text: String,
    blanks: Vec<String>,
    images: Vec<String>,
}

fn flatten_doc(document: &serde_json::Value) -> Flat {
    let mut lines: Vec<String> = Vec::new();
    let mut blanks: Vec<String> = Vec::new();
    let mut images: Vec<String> = Vec::new();
    for block in children(document) {
        flatten_block(block, &mut lines, &mut blanks, &mut images);
    }
    Flat { text: lines.join("\n"), blanks, images }
}

/// A node with `content` flattened to plain text — choices, prompts, answers; a document
/// keeps its blocks as lines, anything else flattens inline.
fn doc_text(v: &serde_json::Value) -> String {
    // Choice/prompt/wordBank/part entries wrap their document in `content`; the value may
    // also already be the document itself (stray inline node).
    let document = if node_type(v) == "document" { v } else { v.get("content").unwrap_or(&serde_json::Value::Null) };
    if !document.is_object() {
        return String::new();
    }
    flatten_doc(document).text
}

fn children(node: &serde_json::Value) -> &[serde_json::Value] {
    node.get("content").and_then(serde_json::Value::as_array).map_or(&[], |c| c.as_slice())
}

/// A JSON array field, or an empty slice when it is absent or not an array.
fn arr<'a>(v: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    v.get(key).and_then(serde_json::Value::as_array).map_or(&[], |c| c.as_slice())
}

fn node_type<'a>(node: &'a serde_json::Value) -> &'a str {
    node.get("type").and_then(serde_json::Value::as_str).unwrap_or("")
}

fn asset_hex(node: &serde_json::Value) -> Option<&str> {
    node.get("asset").and_then(serde_json::Value::as_str).and_then(|a| a.strip_prefix("sha256:"))
}

/// One top-level block → one or more lines; unknown or stray inline nodes at block level
/// recurse as a paragraph when they say anything.
fn flatten_block(block: &serde_json::Value, lines: &mut Vec<String>, blanks: &mut Vec<String>, images: &mut Vec<String>) {
    match node_type(block) {
        "paragraph" | "heading" => {
            let mut line = String::new();
            for n in children(block) {
                flatten_inline(n, &mut line, blanks, images);
            }
            lines.push(line.trim_end().to_string());
        }
        "blockquote" | "panel" | "document" => {
            for c in children(block) {
                flatten_block(c, lines, blanks, images);
            }
        }
        "bullet-list" => {
            for item in children(block) {
                let mut line = String::from("- ");
                flatten_inline(item, &mut line, blanks, images);
                lines.push(line.trim_end().to_string());
            }
        }
        "ordered-list" => {
            let start = block.get("start").and_then(serde_json::Value::as_u64).unwrap_or(1);
            for (i, item) in children(block).iter().enumerate() {
                let mut line = format!("{}. ", start as usize + i);
                flatten_inline(item, &mut line, blanks, images);
                lines.push(line.trim_end().to_string());
            }
        }
        "code-block" => {
            let mut line = String::new();
            flatten_inline(block, &mut line, blanks, images);
            lines.push(line);
        }
        "rule" => lines.push("—".to_string()),
        "table" => {
            for row in children(block) {
                let mut line = String::new();
                for (i, cell) in children(row).iter().enumerate() {
                    if i > 0 {
                        line.push_str(" | ");
                    }
                    flatten_inline(cell, &mut line, blanks, images);
                }
                lines.push(line.trim_end().to_string());
            }
        }
        "side-by-side" => {
            for panel in children(block) {
                flatten_block(panel, lines, blanks, images);
            }
        }
        "inline-image" | "block-image" => {
            if let Some(hex) = asset_hex(block) {
                images.push(hex.to_string());
            }
            lines.push(picture_text(node_alt(block)));
        }
        _ => {
            let mut line = String::new();
            flatten_inline(block, &mut line, blanks, images);
            if !line.trim().is_empty() {
                lines.push(line);
            }
        }
    }
}

fn node_alt(node: &serde_json::Value) -> &str {
    node.get("alt").and_then(serde_json::Value::as_str).or_else(|| node.get("caption").and_then(serde_json::Value::as_str)).unwrap_or("")
}

fn picture_text(alt: &str) -> String {
    if alt.is_empty() { "[picture]".to_string() } else { format!("[picture: {alt}]") }
}

/// One node's inline text: text, hard breaks, `___` blanks (each collects its answer),
/// math in `$…$`, pictures as placeholders (asset collected); other types recurse into
/// their `content`, so paragraphs, links and marks flatten transparently.
fn flatten_inline(node: &serde_json::Value, out: &mut String, blanks: &mut Vec<String>, images: &mut Vec<String>) {
    match node_type(node) {
        "text" => out.push_str(node.get("text").and_then(serde_json::Value::as_str).unwrap_or("")),
        "hard-break" => out.push('\n'),
        "blank" => {
            out.push_str(BLANK);
            let mut answer = String::new();
            for c in children(node) {
                flatten_inline(c, &mut answer, blanks, images);
            }
            blanks.push(answer);
        }
        "inline-math" | "display-math" => {
            out.push('$');
            out.push_str(node.get("source").and_then(serde_json::Value::as_str).unwrap_or(""));
            out.push('$');
        }
        "block-image" | "inline-image" => {
            if let Some(hex) = asset_hex(node) {
                images.push(hex.to_string());
            }
            out.push_str(&picture_text(node_alt(node)));
        }
        _ => {
            for c in children(node) {
                flatten_inline(c, out, blanks, images);
            }
        }
    }
}

/// RFC 4648 base64, standard alphabet. Whitespace ignored, trailing padding optional;
/// any other byte, or a stray 1-symbol tail chunk, fails.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let dec = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let body: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let body = trim_eq_pad(&body);
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for chunk in body.chunks(4) {
        let mut digits: Vec<u8> = Vec::with_capacity(4);
        for &c in chunk {
            match dec(c) {
                Some(x) => digits.push(x),
                None => return None, // a stray '=' or real garbage inside the body
            }
        }
        let mut n = 0u32;
        for &x in &digits {
            n = (n << 6) | u32::from(x);
        }
        match digits.len() {
            4 => {
                out.push((n >> 16) as u8);
                out.push((n >> 8) as u8);
                out.push(n as u8);
            }
            3 => {
                out.push((n >> 10) as u8);
                out.push((n >> 2) as u8);
            }
            2 => out.push(((n >> 4) & 0xFF) as u8),
            0 => continue, // nothing but whitespace in this chunk
            _ => return None, // a stranded single symbol decodes to nothing
        }
    }
    Some(out)
}

/// Strips trailing '=' padding symbols (after whitespace removal).
fn trim_eq_pad(body: &[u8]) -> &[u8] {
    let mut end = body.len();
    while end > 0 && body[end - 1] == b'=' {
        end -= 1;
    }
    &body[..end]
}
// ---------- text decoding + helpers ----------

/// UTF-8 first (BOM stripped), else Latin-1 byte-for-byte: every byte is its own code point.
fn decode_text(data: &[u8]) -> String {
    let body = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    match std::str::from_utf8(body) {
        Ok(s) => s.to_string(),
        Err(_) => body.iter().map(|&b| b as char).collect(),
    }
}

/// Filename stem for the suggested bank name; no filename means pasted questions.
fn doc_name(filename: &str) -> String {
    if filename.trim().is_empty() {
        return "Pasted questions".to_string();
    }
    let file = filename.rsplit(['/', '\\']).next().unwrap_or(filename).trim();
    let stem = match file.rsplit_once('.') {
        Some((s, _ext)) if !s.is_empty() => s,
        _ => file,
    };
    let stem = stem.trim();
    if stem.is_empty() { "Imported questions".to_string() } else { stem.to_string() }
}

/// Blocks are runs of non-blank lines; blank lines separate the questions.
fn split_blocks(text: &str) -> Vec<Vec<String>> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            if !cur.is_empty() {
                blocks.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(line.to_string());
        }
    }
    if !cur.is_empty() {
        blocks.push(cur);
    }
    blocks
}

fn make_question(stem: String, kind: Kind) -> Question {
    let now = now_secs();
    Question {
        id: crate::model::new_id(),
        stem,
        kind,
        points: None,
        difficulty: None,
        topics: Vec::new(),
        image: None,
        explanation: None,
        figure: None,
        created: now,
        modified: now,
    }
}

/// `store::now()` is private, so parsers timestamp with `SystemTime` directly.
fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Kind;

    fn parsed(text: &str) -> Vec<Question> {
        parse_blackboard(text).unwrap().questions
    }

    #[test]
    fn mc_star_marks_correct_and_choices_keep_letter_order() {
        let doc = parse_blackboard("MC\nWhat is 2+2?\n*b. 4\na. 3\nc. 5\nd. 6\n").unwrap();
        assert_eq!(doc.name, "Pasted questions");
        let q = &doc.questions[0];
        assert_eq!(q.stem, "What is 2+2?");
        let Kind::Mc { answers } = &q.kind else { panic!("not MC: {:?}", q.kind) };
        assert_eq!(answers.len(), 4);
        assert_eq!(answers[0].text, "4");
        assert!(answers[0].correct);
        assert!(!answers[1].correct);
        // All imported questions carry the default metadata.
        assert_eq!(q.points, None);
        assert_eq!(q.difficulty, None);
        assert!(q.topics.is_empty());
        assert!(q.explanation.is_none());
        assert_eq!(q.created, q.modified);
    }

    #[test]
    fn ma_two_correct() {
        let qs = parsed("MA\nPick every even number.\na. 2\n*b. 4\nc. 5\n*d. 6\n");
        let Kind::Mc { answers } = &qs[0].kind else { panic!("not MC") };
        assert_eq!(answers.iter().filter(|a| a.correct).count(), 2);
        assert_eq!(answers.len(), 4);
        assert_eq!(answers[1].text, "4");
        assert_eq!(answers[3].text, "6");
    }

    #[test]
    fn tf_accepts_word_and_letter_forms() {
        let qs = parsed("TF\nThe sky is blue.\nTRUE\n\nTF\nThe moon is made of cheese.\nFalse\n");
        assert!(matches!(&qs[0].kind, Kind::Tf { correct: true }));
        assert!(matches!(&qs[1].kind, Kind::Tf { correct: false }));
        assert_eq!(qs[1].stem, "The moon is made of cheese.");
    }

    #[test]
    fn ess_collects_sample_answer_lines() {
        let qs = parsed("ESS\nExplain gravity in one sentence.\nMasses attract each other.\nThat is about it.\n");
        let Kind::Short { sample } = &qs[0].kind else { panic!("not Short") };
        assert_eq!(sample, "Masses attract each other.\nThat is about it.");
        // A bare ESS with no sample is fine too.
        parsed("ESS\nExplain gravity.\n");
    }

    #[test]
    fn num_plain_fraction_and_range() {
        let qs = parsed("NUM\nHow many days are in a week?\n7\n\nNUM\nPi to 2 dp\n3.14±0.001\n\nNUM\nWhat is 3/8?\n3/8\n");
        let Kind::Blank { accept } = &qs[0].kind else { panic!("not Blank") };
        assert_eq!(accept, &[vec!["7".to_string()]]);
        let accept = { let Kind::Blank { accept } = &qs[1].kind else { panic!("not Blank") }; accept };
        assert_eq!(accept, &[vec!["3.14".to_string(), "3.139".to_string(), "3.141".to_string()]]);
        let Kind::Blank { accept } = &qs[2].kind else { panic!("not Blank") };
        assert_eq!(accept, &[vec!["3/8".to_string(), "0.375".to_string()]]);
    }

    #[test]
    fn fib_two_blanks_with_alts() {
        let qs = parsed("FIB\nThe capital of ___ sits on a ___ river.\nParis OR paris\nnile\n");
        let Kind::Blank { accept } = &qs[0].kind else { panic!("not Blank") };
        assert_eq!(accept, &[vec!["Paris".to_string(), "paris".to_string()], vec!["nile".to_string()]]);
    }

    #[test]
    fn fib_requires_one_line_per_blank() {
        let err = parse_blackboard("FIB\nTwo ___ here. And ___ too.\nParis\n").expect_err("missing blank");
        assert!(err.contains("___ markers"), "{err}");
    }

    #[test]
    fn mat_alternating_pairs_with_distractors() {
        let qs = parsed("MAT\nCat\nKitten\nDog\nPuppy\n-\nEgg\nBasket\n");
        let Kind::Matching { pairs, distractors } = &qs[0].kind else { panic!("not Matching") };
        assert_eq!(
            serde_json::to_value(pairs).unwrap(),
            serde_json::json!([{"left": "Cat", "right": "Kitten"}, {"left": "Dog", "right": "Puppy"}])
        );
        assert_eq!(distractors, &["Egg".to_string(), "Basket".to_string()]);
        assert_eq!(qs[0].stem, MAT_DEFAULT_STEM);
    }

    #[test]
    fn mat_arrow_lines_are_accepted() {
        let qs = parsed("MAT\nParis -> France\nTokyo -> Japan\n");
        let Kind::Matching { pairs, distractors } = &qs[0].kind else { panic!("not Matching") };
        assert_eq!(pairs.len(), 2);
        assert!(distractors.is_empty());
    }

    #[test]
    fn ord_joins_the_ordered_answers_into_one_blank() {
        let qs = parsed("ORD\nPut these in order by size.\nElephant\nDog\nAnt\n");
        let Kind::Blank { accept } = &qs[0].kind else { panic!("not Blank") };
        assert_eq!(accept, &[vec!["Elephant, Dog, Ant".to_string()]]);
        assert_eq!(qs[0].stem, "Put these in order by size.");
    }

    #[test]
    fn aiken_blocks_without_tags_parse_too() {
        let doc = parse_blackboard("What is the capital of France?\nA. London\n*B. Paris\nC. Berlin\n\nBigger question?\nA. yes\nB. no\n*A. yes\n").unwrap();
        assert_eq!(doc.questions.len(), 2);
        let Kind::Mc { answers } = &doc.questions[0].kind else { panic!("not MC") };
        assert_eq!(answers[1].text, "Paris");
        assert!(answers[1].correct);
        assert_eq!(doc.questions[1].stem, "Bigger question?");
    }

    #[test]
    fn prose_blocks_and_mixed_documents_parse() {
        let qs = parsed("Chapter 3 review\n\nMC\nWhich is odd?\na. 1\nb. 3\nd. 4\n*c. 5\n");
        assert_eq!(qs.len(), 1);
    }

    /// The exact corpus of a live e2e run (Ollama gemma4:31b) that hardened two rules:
    /// the TF answer glued onto the stem line's tail, and an underscore RUN as one blank.
    const CORPUS: &str = "MC\nWhich layer of Earth is directly beneath the crust?\n\
        *A. Mantle\nB. Inner core\nC. Outer core\nD. Lithosphere\n\n\
        TF\nThe Earth's inner core is made of liquid iron. False\n\n\
        FIB\nThe liquid layer of the core is the __________ core.\nouter\n\n\
        MC\nTectonic plates float on the:\nA. Inner core\n*B. Asthenosphere\nC. Mesosphere\nD. Continental crust\n";

    #[test]
    fn corpus_tf_answer_glued_to_the_stem_tail_is_extracted() {
        let qs = parsed(CORPUS);
        assert_eq!(qs.len(), 4);
        let q = &qs[1];
        assert_eq!(q.stem, "The Earth's inner core is made of liquid iron.");
        assert!(matches!(&q.kind, Kind::Tf { correct: false }), "{:?}", q.kind);
    }

    #[test]
    fn corpus_fib_underscore_run_is_exactly_one_blank() {
        let qs = parsed(CORPUS);
        let q = &qs[2];
        // The run of 10 underscores collapses to the canonical single `___` marker.
        assert_eq!(q.stem, "The liquid layer of the core is the ___ core.");
        let Kind::Blank { accept } = &q.kind else { panic!("not Blank: {:?}", q.kind) };
        assert_eq!(accept, &[vec!["outer".to_string()]]);
        // The neighboring questions keep parsing from the same corpus, too.
        let Kind::Mc { answers } = &qs[3].kind else { panic!("not MC") };
        assert!(answers[1].correct);
        assert_eq!(answers[1].text, "Asthenosphere");
    }

    #[test]
    fn blank_or_failure_reports_the_question_number() {
        let err = parse_blackboard("MC\nQ1?\na. 1\n*b. 2\n\nTF\nA broken question with no answer\n\nMC\nQ3?\n*a. 1\n").expect_err("missing T/F");
        assert!(err.starts_with("question 2:"), "{err}");
        assert!(err.contains("T or F"), "{err}");
    }

    #[test]
    fn not_recognized_gives_the_helpful_generic_error() {
        for bad in ["", "hello world", "just some notes\nover two lines"] {
            let err = parse_blackboard(bad).expect_err("garbage rejected");
            assert_eq!(err, GENERIC);
        }
        let err = detect_and_parse(b"hello", "junk.txt").expect_err("garbage rejected");
        assert_eq!(err, GENERIC);
    }

    // ---------- detect_and_parse ----------

    #[test]
    fn detect_runs_blackboard_and_names_the_stem() {
        let doc = detect_and_parse("TF\nThe sky is blue.\nTRUE\n".as_bytes(), "unit-3-test.txt").unwrap();
        assert_eq!(doc.name, "unit-3-test");
        assert!(matches!(&doc.questions[0].kind, Kind::Tf { correct: true }));
    }

    #[test]
    fn detect_decodes_latin1_and_utf8_bom() {
        let doc = detect_and_parse(b"ESS\nWhat is d\xe9j\xe0 vu?\nSeen before.\n", "accents.txt").unwrap();
        assert!(doc.questions[0].stem.contains("déjà"), "{:?}", doc.questions[0].stem);
        let mut bom = b"\xEF\xBB\xBF".to_vec();
        bom.extend(b"TF\nTwo plus two is four.\nTRUE\n");
        assert!(detect_and_parse(&bom, "bom.txt").is_ok());
    }

    #[test]
    fn csv_rows_with_quotes_and_markings() {
        let csv = "\n\
type,stem,points,topic,a,b,c,d,correct
MC,\"What is \"\"2+2\"\"?\",1,math,4,5,6,7,a
,Pick the evens,,math,2,3,4,5,\"a,c\"
TF,The sky is blue,,science,,,,,TRUE
";
        let doc = detect_and_parse(csv.as_bytes(), "quiz-1.csv").unwrap();
        assert_eq!(doc.questions.len(), 3);
        let q = &doc.questions[0];
        assert_eq!(q.stem, "What is \"2+2\"?");
        assert_eq!(q.points, Some(1.0));
        assert_eq!(q.topics, &["math"]);
        let Kind::Mc { answers } = &q.kind else { panic!("not MC") };
        assert_eq!(answers[0].text, "4");
        assert!(answers[0].correct);
        // Empty type defaults to MC; `a,c` marks two correct.
        let Kind::Mc { answers } = &doc.questions[1].kind else { panic!("not MC") };
        assert_eq!(answers.iter().filter(|a| a.correct).map(|a| a.text.clone()).collect::<Vec<_>>(), &["2", "4"]);
        assert!(matches!(&doc.questions[2].kind, Kind::Tf { correct: true }));
    }

    #[test]
    fn csv_correct_text_marks_or_builds_blanks() {
        let csv = "type,stem,correct_text\nTF,Fire is hot.,true\nblank,The Sun rises in the ___.,east\nESS,Explain tides.,Moon gravity\n";
        let doc = detect_and_parse(csv.as_bytes(), "quiz-2.csv").unwrap();
        assert!(matches!(&doc.questions[0].kind, Kind::Tf { correct: true }));
        assert_eq!(doc.questions[1].topics, Vec::<String>::new());
        assert!(matches!(&doc.questions[1].kind, Kind::Blank { accept: a } if a == &vec![vec!["east".to_string()]]));
        assert!(matches!(&doc.questions[2].kind, Kind::Short { sample } if sample == "Moon gravity"));
    }

    #[test]
    fn json_bank_round_trips() {
        let q = make_question("Stem".into(), Kind::Tf { correct: true });
        let bank = serde_json::json!({
            "kind": LEGACY_KIND, "version": 1, "name": "Chapter 1", "questions": [q],
        });
        let data = serde_json::to_vec(&bank).unwrap();
        let doc = detect_and_parse(&data, "chapter-1.json").unwrap();
        assert_eq!(doc.name, "Chapter 1");
        assert!(matches!(&doc.questions[0].kind, Kind::Tf { correct: true }));
        assert_ne!(doc.questions[0].id, "regenerated-ids-are-required"); // ids are fresh
        // Pasted JSON with no filename keeps the bank's own name.
        let pasted = detect_and_parse(&data, "").unwrap();
        assert_eq!(pasted.name, "Chapter 1");
    }

    #[test]
    fn json_claimed_even_with_a_paste_filename_when_kind_matches() {
        let bank = serde_json::json!({"kind": LEGACY_KIND, "questions": []});
        let err = detect_and_parse(serde_json::to_vec(&bank).unwrap().as_slice(), "notes.txt").expect_err("empty bank");
        assert!(err.contains("no questions"), "{err}");
    }

    // ---------- Test Parrot package / record (.parrot.json) ----------

    /// A minimal but schema-shaped package: one bank (MC + fill-in-blank), one exam whose
    /// positions land q2 then q1 and skip an unresolvable q9.
    fn package_json() -> serde_json::Value {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let para = |t: &str| serde_json::json!({"type": "paragraph", "content": [text(t)]});
        let doc = move |t: &str| serde_json::json!({"type": "document", "content": [para(t)]});
        serde_json::json!({
            "format": "test-parrot/package", "formatVersion": "0.1.0",
            "generator": {"name": "testparrot-test", "version": "1"}, "requiredFeatures": [],
            "questionBanks": [{"id": "b1", "record": {
                "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
                "generator": {"name": "testparrot-test", "version": "1"}, "requiredFeatures": [],
                "bank": {"name": "Periodic table", "questions": [
                    {"id": "q1", "type": "multiple-choice", "stem": doc("Which gas is noble?"), "choices": [
                        {"id": "q1-c1", "content": doc("Helium"), "correct": true},
                        {"id": "q1-c2", "content": doc("Oxygen"), "correct": false},
                    ]},
                    {"id": "q2", "type": "fill-in-the-blank", "difficulty": "hard", "topics": ["atoms"], "points": 3,
                     "stem": {"type": "document", "content": [{"type": "paragraph", "content": [
                        {"type": "text", "text": "Helium has "}, {"type": "blank", "content": [text("4")]},
                        {"type": "text", "text": " neutrons; hydrogen "}, {"type": "blank", "content": [text("0")]}, {"type": "text", "text": "."}
                     ]}]},
                    }
                ]},
                "media": []
            }}],
            "exams": [{"format": "test-parrot/exam", "formatVersion": "0.5.0", "name": "Atoms test",
                       "sections": [{"title": "Part A", "instructions": "Answer all."}],
                       "positions": [
                           {"question": {"bank": "b1", "question": "q2"}, "section": 0, "columns": 2},
                           {"question": {"bank": "b1", "question": "q1"}, "section": 0},
                           {"question": {"bank": "b1", "question": "q9"}, "section": 0},
                       ]}]
        })
    }

    fn bytes_of(v: &serde_json::Value) -> Vec<u8> { serde_json::to_vec(v).unwrap() }

    #[test]
    fn package_maps_questions_and_best_effort_exam() {
        let data = bytes_of(&package_json());
        let doc = detect_and_parse(&data, "atoms.parrot.json").unwrap();
        assert_eq!(doc.name, "Periodic table");
        assert_eq!(doc.questions.len(), 2);
        let q1 = &doc.questions[0];
        let Kind::Mc { answers } = &q1.kind else { panic!("not MC: {:?}", q1.kind) };
        assert_eq!(answers[0].text, "Helium");
        assert!(answers[0].correct);
        assert!(!answers[1].correct);
        let q2 = &doc.questions[1];
        assert_eq!(q2.stem, "Helium has ___ neutrons; hydrogen ___.");
        let Kind::Blank { accept } = &q2.kind else { panic!("not Blank: {:?}", q2.kind) };
        assert_eq!(accept, &[vec!["4".to_string()], vec!["0".to_string()]]);
        assert_eq!(q2.points, Some(3.0));
        assert_eq!(q2.difficulty, Some(Difficulty::High));
        assert_eq!(q2.topics, &["atoms"]);
        // Exam positions resolve in their own order, unresolvable ones skipped.
        assert_eq!(doc.exams.len(), 1);
        let e = &doc.exams[0];
        assert_eq!(e.name, "Atoms test");
        assert_eq!(e.items.len(), 2);
        assert_eq!(e.items[0].question_id, q2.id);
        assert_eq!(e.items[1].question_id, q1.id);
        assert!(e.items.iter().all(|it| it.bank_id.is_empty()));
        assert_eq!(e.header, DEFAULT_HEADER);
    }

    #[test]
    fn record_matching_pairs_and_distractors() {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let doc = move |t: &str| serde_json::json!({"type": "document", "content": [{"type": "paragraph", "content": [text(t)]}]});
        let rec = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Match practice", "questions": [{"id": "q1", "type": "matching",
                "stem": doc("Match the animals."),
                "prompts": [
                    {"id": "q1-p1", "content": doc("Cat"), "answer": "q1-a2"},
                    {"id": "q1-p2", "content": doc("Dog"), "answer": "q1-a1"},
                ],
                "wordBank": [
                    {"id": "q1-a1", "content": doc("Puppy")},
                    {"id": "q1-a2", "content": doc("Kitten")},
                    {"id": "q1-a3", "content": doc("Egg")},
                ]}]},
            "media": [],
        });
        let parsed_doc = detect_and_parse(&bytes_of(&rec), "match.json").unwrap();
        assert_eq!(parsed_doc.name, "Match practice");
        let q = &parsed_doc.questions[0];
        let Kind::Matching { pairs, distractors } = &q.kind else { panic!("not Matching: {:?}", q.kind) };
        assert_eq!(pairs[0].left, "Cat");
        assert_eq!(pairs[0].right, "Kitten");
        assert_eq!(pairs[1].left, "Dog");
        assert_eq!(pairs[1].right, "Puppy");
        assert_eq!(distractors, &["Egg".to_string()]);
        // Unlike Blackboard text, a record keeps the authored stem.
        assert_eq!(q.stem, "Match the animals.");
    }

    #[test]
    fn record_multipart_flattens_subparts() {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let para_doc = move |t: &str| serde_json::json!({"type": "document", "content": [{"type": "paragraph", "content": [text(t)]}]});
        let rec = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Big questions", "questions": [{"id": "q1", "type": "multipart",
                "stem": para_doc("Solve these."),
                "parts": [
                    {"id": "q1-s1", "type": "multiple-choice", "stem": para_doc("(a) Pick one."), "choices": [
                        {"id": "q1-s1-c1", "content": para_doc("Red"), "correct": false},
                        {"id": "q1-s1-c2", "content": para_doc("Blue"), "correct": true},
                    ]},
                    {"id": "q1-s2", "stem": para_doc("(b) Do both parts."),
                     "subparts": [
                        {"id": "q1-s2-s1", "type": "multiple-choice", "stem": para_doc("(i) Yes or no."), "choices": [
                            {"id": "q1-s2-s1-c1", "content": para_doc("Yes"), "correct": true},
                            {"id": "q1-s2-s1-c2", "content": para_doc("No"), "correct": false},
                        ]},
                        {"id": "q1-s2-s2", "type": "short-answer", "stem": para_doc("(ii) Why?"), "suggestedAnswer": para_doc("Because physics.")},
                     ]},
                ]}]},
            "media": [],
        });
        let parsed_doc = detect_and_parse(&bytes_of(&rec), "multi.json").unwrap();
        let q = &parsed_doc.questions[0];
        let Kind::Multipart { parts } = &q.kind else { panic!("not Multipart: {:?}", q.kind) };
        assert_eq!(parts.len(), 4); // (a), lead-in, (i), (ii)
        assert!(matches!(&parts[0].kind, PartKind::Mc { .. }));
        assert!(matches!(&parts[1].kind, PartKind::Short { .. }));
        assert!(matches!(&parts[2].kind, PartKind::Mc { .. }));
        assert_eq!(parts[0].stem, "(a) Pick one.");
        assert_eq!(parts[1].stem, "(b) Do both parts.");
        assert_eq!(parts[3].stem, "(ii) Why?");
        if let PartKind::Short { sample } = &parts[3].kind {
            assert_eq!(sample, "Because physics.");
        } else {
            panic!("not Short");
        }
    }

    #[test]
    fn record_short_answer_tf_and_metadata() {
        let text = |t: &str| serde_json::json!({"type": "text", "text": t});
        let doc = move |t: &str| serde_json::json!({"type": "document", "content": [{"type": "paragraph", "content": [text(t)]}]});
        let rec = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Space", "questions": [
                {"id": "q1", "type": "true-false", "stem": doc("Space is empty."), "choices": [
                    {"id": "q1-c1", "content": doc("True"), "correct": true},
                    {"id": "q1-c2", "content": doc("False"), "correct": false},
                ]},
                {"id": "q2", "type": "short-answer", "difficulty": "easy", "points": 2, "topics": ["gravity"], "stem": doc("What pulls tides?"), "suggestedAnswer": doc("Moon gravity")},
            ]},
            "media": [],
        });
        let parsed_doc = detect_and_parse(&bytes_of(&rec), "space.json").unwrap();
        assert!(matches!(&parsed_doc.questions[0].kind, Kind::Tf { correct: true }));
        let q = &parsed_doc.questions[1];
        assert!(matches!(&q.kind, Kind::Short { sample } if sample == "Moon gravity"));
        assert_eq!(q.difficulty, Some(Difficulty::Low));
        assert_eq!(q.points, Some(2.0));
        assert_eq!(q.topics, &["gravity"]);
    }

    #[test]
    fn record_errors_name_the_question() {
        let no_flag = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Space", "questions": [
                {"id": "q1", "type": "true-false", "stem": {"type": "document", "content": []}, "choices": [
                    {"id": "q1-c1", "content": {"type": "document", "content": []}, "correct": false},
                    {"id": "q1-c2", "content": {"type": "document", "content": []}, "correct": false},
                ]}
            ]},
            "media": [],
        });
        let err = detect_and_parse(&bytes_of(&no_flag), "space.json").expect_err("unmarked pair");
        assert!(err.starts_with("question q1:"), "{err}");
        // Unsupported types name themselves too.
        let bad = serde_json::json!({
            "format": "test-parrot/question-bank", "formatVersion": "0.10.0",
            "generator": {"name": "g", "version": "1"}, "requiredFeatures": [],
            "bank": {"name": "Space", "questions": [{"id": "q2", "type": "poll", "stem": {"type": "document", "content": []}}]},
            "media": [],
        });
        let err = detect_and_parse(&bytes_of(&bad), "space.json").expect_err("bad type");
        assert!(err.starts_with("question q2:"), "{err}");
    }

    #[test]
    fn package_claimed_even_when_the_json_is_broken() {
        let err = detect_and_parse(b"{ \"format\": \"test-parrot/package\", \"questionBanks\":", "b.parrot.json").expect_err("malformed");
        assert!(err.contains("not valid JSON"), "{err}");
        // A non-json file that merely says the format name in prose is not test-parrot.
        // Blackboard text that merely mentions packages still parses; 1 question.
        let doc = detect_and_parse(b"plain text about packages\n\nMC\nQ?\na. 1\n*b. 2\n", "notes.txt").unwrap();
        assert_eq!(doc.questions.len(), 1);
        let err = detect_and_parse(b"notes\n\"test-parrot/package\" in prose but broken json", "r.txt").expect_err("marker but junk");
        assert!(err.contains("not valid JSON"), "{err}");
    }

    #[test]
    fn base64_decode_matches_vectors() {
        assert_eq!(base64_decode(""), Some(Vec::<u8>::new()));
        assert_eq!(base64_decode("Zg=="), Some(b"f".to_vec()));
        assert_eq!(base64_decode("Zm8="), Some(b"fo".to_vec()));
        assert_eq!(base64_decode("aGVsbG8="), Some(b"hello".to_vec()));
        assert_eq!(base64_decode("Zm9vYmFy"), Some(b"foobar".to_vec()));
        assert_eq!(base64_decode("Z m 9 v"), Some(b"foo".to_vec())); // interior whitespace tolerated
        assert_eq!(base64_decode("Z!!="), None);
        assert_eq!(base64_decode("Z"), None); // a stranded single symbol
    }
}