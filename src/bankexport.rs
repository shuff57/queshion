//! Bank export (Test Parrot's "Export Question Bank"): a printable teacher's bank file as
//! PDF, answers included, that also carries the whole bank as data so it imports back
//! losslessly; plus the plain JSON file. The PDF is laid out by the exam exporter
//! (`export::render_pdf` on an [`ExamDoc`], read-only use of `layout` / `export`); the data
//! rides along as an embedded file attachment (`queshion-bank.json`), appended to the
//! finished PDF as an incremental update so the page content is exactly what the exporter
//! drew. [`embedded_data`] reads it back (and Test Parrot's own `parrot.zip` attachment), so
//! the import screen can take a bank-file PDF without the AI.
//!
//! Pure functions; the dialog lives in `bankui`.

use std::collections::BTreeMap;

use crate::layout::{DocImage, DocQ, DocSection, ExamDoc};
use crate::model::{Kind, PartKind, QKind, Question, QuestionBank};

/// The attachment's file name inside the PDF.
/// Stable attachment name, deliberately unchanged by the verSHion rename: bank PDFs exported
/// before the rename embed `queshion-bank.json` and must still import.
pub const ATTACHMENT: &str = "queshion-bank.json";

// ---------- the printable bank file ----------

fn kind_title(k: QKind) -> &'static str {
    match k {
        QKind::Mc => "Multiple Choice",
        QKind::Tf => "True / False",
        QKind::Matching => "Matching",
        QKind::Blank => "Fill in the Blank",
        QKind::Short => "Short Answer",
        QKind::Multipart => "Multipart",
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn letter(i: usize) -> char {
    char::from(b'A' + (i % 26) as u8)
}

fn diff_word(q: &Question) -> &'static str {
    match q.difficulty {
        Some(crate::model::Difficulty::Low) => "Easy",
        Some(crate::model::Difficulty::Medium) => "Medium",
        Some(crate::model::Difficulty::High) => "Hard",
        None => "Unspecified",
    }
}

/// "*Difficulty:* Easy · *Topics:* Atoms, Bonds" — the line under each question heading.
fn meta_line(q: &Question) -> String {
    let topics: Vec<&str> = q.topics.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
    let topics = if topics.is_empty() { "None".to_string() } else { topics.join(", ") };
    format!("*Difficulty:* {} · *Topics:* {topics}", diff_word(q))
}

/// "Answers: 14; 10" text of a fill-in-the-blank key.
fn blank_key(accept: &[Vec<String>]) -> String {
    accept
        .iter()
        .map(|alts| {
            let a: Vec<&str> = alts.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if a.is_empty() { "—".to_string() } else { a.join(" / ") }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// A heading line in stem markup ("**Suggested Answer**") followed by `body` on new lines.
fn block(title: &str, body: &str) -> String {
    let body = body.trim_end();
    if body.is_empty() { format!("**{title}**") } else { format!("**{title}**\n{body}") }
}

/// One question as it prints in the bank file: everything goes into ONE text entry (more
/// entries would read as a matching list to the exporter), MC choices keep their letters with
/// the correct ones marked, every other kind prints its key under the stem.
fn bank_q(q: &Question, number: u32, image: Option<DocImage>) -> DocQ {
    // Imported from the pool without its key: the placeholder values (all-false choices, empty
    // matches, a `false` True/False) are not a key and are never printed as one.
    let keyless = crate::model::is_keyless(q);
    let none = crate::model::KEY_NOT_PROVIDED;
    let mut text = q.stem.trim_end().to_string();
    let mut parts: Vec<String> = vec![meta_line(q)];
    let mut choices: Vec<String> = Vec::new();
    let mut key;
    match &q.kind {
        Kind::Mc { answers } => {
            for (i, a) in answers.iter().enumerate() {
                let t = a.text.trim();
                choices.push(if a.correct { format!("{}. {t}  **(Correct answer)**", letter(i)) } else { format!("{}. {t}", letter(i)) });
            }
            key = answers.iter().enumerate().filter(|(_, a)| a.correct).map(|(i, _)| letter(i).to_string()).collect::<Vec<_>>().join(", ");
        }
        Kind::Tf { correct } => {
            key = if keyless { none } else if *correct { "True" } else { "False" }.to_string();
            parts.push(format!("**Answer:** {key}"));
        }
        Kind::Matching { pairs, distractors } if keyless => {
            let mut lines: Vec<String> = pairs.iter().enumerate().map(|(i, p)| format!("{}. {}", i + 1, p.left.trim())).collect();
            lines.push(format!("Choices: {}", crate::model::right_column(pairs, distractors).join(", ")));
            parts.push(block("Matches", &lines.join("\n")));
            key = none.to_string();
        }
        Kind::Matching { pairs, distractors } => {
            let mut lines: Vec<String> = pairs.iter().enumerate().map(|(i, p)| format!("{}. {} → {}", i + 1, p.left.trim(), p.right.trim())).collect();
            if !distractors.is_empty() {
                lines.push(format!("Also listed: {}", distractors.iter().map(|d| d.trim()).collect::<Vec<_>>().join(", ")));
            }
            key = lines.join("\n");
            parts.push(block("Matches", &key));
        }
        Kind::Blank { accept } => {
            key = blank_key(accept);
            parts.push(block("Answers", &key));
        }
        Kind::Short { sample } => {
            key = sample.trim().to_string();
            if !key.is_empty() {
                parts.push(block("Suggested Answer", &key));
            }
        }
        Kind::Multipart { parts: ps } => {
            let mut keys = Vec::new();
            for (pi, p) in ps.iter().enumerate() {
                let l = char::from(b'a' + (pi % 26) as u8);
                let stem = p.stem.trim();
                let own = stem.len() > 1 && stem.as_bytes()[0].is_ascii_alphabetic() && matches!(stem.as_bytes()[1], b'.' | b')');
                let mut s = if own { stem.to_string() } else { format!("{l}. {stem}") };
                match &p.kind {
                    PartKind::Mc { answers } => {
                        for (i, a) in answers.iter().enumerate() {
                            let mark = if a.correct { "  **(Correct answer)**" } else { "" };
                            s.push_str(&format!("\n{}. {}{mark}", letter(i), a.text.trim()));
                        }
                        let k: Vec<String> = answers.iter().enumerate().filter(|(_, a)| a.correct).map(|(i, _)| letter(i).to_string()).collect();
                        keys.push(format!("{l}. {}", k.join(", ")));
                    }
                    PartKind::Tf { correct } => {
                        let k = if keyless { none } else if *correct { "True" } else { "False" };
                        s.push_str(&format!("\n**Answer:** {k}"));
                        keys.push(format!("{l}. {k}"));
                    }
                    PartKind::Short { sample } => {
                        if !sample.trim().is_empty() {
                            s.push_str(&format!("\n\n{}", block("Suggested Answer", sample.trim())));
                            keys.push(format!("{l}. {}", sample.trim()));
                        }
                    }
                }
                parts.push(s);
            }
            key = keys.join("\n");
        }
    }
    if keyless {
        key = none.to_string();
    }
    if let Some(e) = q.explanation.as_deref().map(str::trim).filter(|e| !e.is_empty()) {
        parts.push(block("Explanation", e));
    }
    for p in parts {
        text.push_str("\n\n");
        text.push_str(&p);
    }
    DocQ { number, text: vec![text], choices, image, key_text: key, rule_lines: 0, no_answer_space: true, points: q.points, ..Default::default() }
}

/// The bank as CSV (RFC 4180, CRLF): one header row and one row per question, for Excel,
/// Sheets or Numbers. Math stays as authored (`$...$`); answers are plain text.
pub fn bank_csv(bank: &QuestionBank) -> String {
    let mut out = String::from("#,type,points,question,answer,choices,explanation,topics,difficulty\r\n");
    for (i, q) in bank.questions.iter().enumerate() {
        let cells = [
            (i + 1).to_string(),
            kind_title(q.kind.qkind()).to_string(),
            q.points.map(num_str).unwrap_or_default(),
            question_cell(q),
            if crate::model::is_keyless(q) { crate::model::KEY_NOT_PROVIDED.to_string() } else { answer_cell(&q.kind) },
            choices_cell(&q.kind),
            q.explanation.as_deref().unwrap_or("").trim().to_string(),
            q.topics.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(", "),
            diff_word(q).to_string(),
        ];
        out.push_str(&cells.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    out
}

/// `12` for whole numbers, `2.5` otherwise (points read better without a trailing `.0`).
fn num_str(p: f64) -> String {
    if p.fract() == 0.0 { format!("{}", p as i64) } else { format!("{p}") }
}

/// One CSV field, quoted and with doubled quotes only when it needs it.
fn csv_field(s: &str) -> String {
    if s.chars().any(|c| matches!(c, ',' | '"' | '\n' | '\r')) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Lower-case part label (`a.`), as multipart parts are conventionally numbered.
fn part_letter(i: usize) -> char {
    char::from(b'a' + (i % 26) as u8)
}

/// A part stem that already starts with `a.` / `a)` keeps its own label.
fn own_label(s: &str) -> bool {
    s.len() > 1 && s.as_bytes()[0].is_ascii_alphabetic() && matches!(s.as_bytes()[1], b'.' | b')')
}

/// The stem, with multipart part stems appended as `a. ...` lines.
fn question_cell(q: &Question) -> String {
    let mut s = q.stem.trim().to_string();
    if let Kind::Multipart { parts } = &q.kind {
        for (i, p) in parts.iter().enumerate() {
            let stem = p.stem.trim();
            if own_label(stem) {
                s.push_str(&format!("\n{stem}"));
            } else {
                s.push_str(&format!("\n{}. {stem}", part_letter(i)));
            }
        }
    }
    s
}

/// The key: correct choice texts, `True`/`False`, pairs, accepted blanks, the sample answer,
/// or each multipart part's key.
fn answer_cell(kind: &Kind) -> String {
    match kind {
        Kind::Mc { answers } => answers.iter().filter(|a| a.correct).map(|a| a.text.trim()).collect::<Vec<_>>().join(" | "),
        Kind::Tf { correct } => (if *correct { "True" } else { "False" }).to_string(),
        Kind::Matching { pairs, .. } => pairs.iter().map(|p| format!("{} → {}", p.left.trim(), p.right.trim())).collect::<Vec<_>>().join(" | "),
        Kind::Blank { accept } => blank_key(accept),
        Kind::Short { sample } => sample.trim().to_string(),
        Kind::Multipart { parts } => parts.iter().enumerate().map(|(i, p)| format!("{}. {}", part_letter(i), part_answer(&p.kind))).collect::<Vec<_>>().join("; "),
    }
}

fn part_answer(kind: &PartKind) -> String {
    match kind {
        PartKind::Mc { answers } => answers.iter().filter(|a| a.correct).map(|a| a.text.trim()).collect::<Vec<_>>().join(", "),
        PartKind::Tf { correct } => (if *correct { "True" } else { "False" }).to_string(),
        PartKind::Short { sample } => sample.trim().to_string(),
    }
}

/// Choice texts with their letters (`A. ... | B. ...`), including multipart choice parts.
fn choices_cell(kind: &Kind) -> String {
    match kind {
        Kind::Mc { answers } => answers.iter().enumerate().map(|(i, a)| format!("{}. {}", letter(i), a.text.trim())).collect::<Vec<_>>().join(" | "),
        Kind::Multipart { parts } => parts
            .iter()
            .enumerate()
            .filter_map(|(pi, p)| match &p.kind {
                PartKind::Mc { answers } => Some(format!(
                    "({}) {}",
                    part_letter(pi),
                    answers.iter().enumerate().map(|(i, a)| format!("{}. {}", letter(i), a.text.trim())).collect::<Vec<_>>().join(" | ")
                )),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("; "),
        _ => String::new(),
    }
}

/// Aiken format: only single-answer multiple choice fits, as `question`, lettered choices and
/// `ANSWER: C`, with a blank line between questions. Returns the text and the number of
/// questions the format cannot hold.
pub fn bank_aiken(bank: &QuestionBank) -> (String, usize) {
    let mut blocks = Vec::new();
    let mut skipped = 0;
    for q in &bank.questions {
        let Kind::Mc { answers } = &q.kind else {
            skipped += 1;
            continue;
        };
        if crate::model::is_keyless(q) {
            // The format is a key: a question with none cannot be written.
            skipped += 1;
            continue;
        }
        let correct: Vec<usize> = answers.iter().enumerate().filter(|(_, a)| a.correct).map(|(i, _)| i).collect();
        if correct.len() != 1 || answers.len() < 2 || answers.len() > 26 {
            skipped += 1;
            continue;
        }
        let mut b = one_line(&q.stem);
        for (i, a) in answers.iter().enumerate() {
            b.push_str(&format!("\n{}. {}", letter(i), one_line(&a.text)));
        }
        b.push_str(&format!("\nANSWER: {}", letter(correct[0])));
        blocks.push(b);
    }
    let mut out = blocks.join("\n\n");
    if !out.is_empty() {
        out.push('\n');
    }
    (out, skipped)
}

/// Blackboard Learn 7.3+ "Upload Questions" tab-delimited text. Holds single-answer multiple
/// choice (`MC`), true/false (`TF`), fill in the blank (`FIB`) and short answer (`ESS`);
/// matching, multipart and multi-answer MC are skipped and counted. Every row pads to the same
/// number of incorrect-answer columns so the positional Categories/Topics/Feedback fields line
/// up, and embedded tabs/newlines become spaces.
pub fn bank_blackboard(bank: &QuestionBank) -> (String, usize) {
    let mut pad = 0;
    let mut skipped = 0;
    for q in &bank.questions {
        if crate::model::is_keyless(q) {
            skipped += 1;
            continue;
        }
        match &q.kind {
            Kind::Mc { answers } => {
                let correct = answers.iter().filter(|a| a.correct).count();
                let wrong = answers.len().saturating_sub(correct);
                if correct != 1 || wrong > 10 {
                    skipped += 1;
                } else {
                    pad = pad.max(wrong);
                }
            }
            Kind::Tf { .. } | Kind::Blank { .. } | Kind::Short { .. } => {}
            Kind::Matching { .. } | Kind::Multipart { .. } => skipped += 1,
        }
    }
    let mut rows = Vec::new();
    for q in &bank.questions {
        if crate::model::is_keyless(q) {
            continue; // counted in the first pass
        }
        let (kind, correct, wrong): (&str, String, Vec<String>) = match &q.kind {
            Kind::Mc { answers } => {
                let good: Vec<&str> = answers.iter().filter(|a| a.correct).map(|a| a.text.trim()).collect();
                let bad: Vec<String> = answers.iter().filter(|a| !a.correct).map(|a| one_line(&a.text)).collect();
                if good.len() != 1 || bad.len() > 10 {
                    continue;
                }
                ("MC", one_line(good[0]), bad)
            }
            Kind::Tf { correct } => ("TF", (if *correct { "True" } else { "False" }).to_string(), Vec::new()),
            Kind::Blank { accept } => (
                "FIB",
                accept
                    .iter()
                    .map(|alts| alts.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" / "))
                    .collect::<Vec<_>>()
                    .join("; "),
                Vec::new(),
            ),
            Kind::Short { sample } => ("ESS", one_line(sample), Vec::new()),
            _ => continue,
        };
        let mut fields = vec![one_line(&q.stem), kind.to_string(), q.points.map(num_str).unwrap_or_default(), correct];
        fields.extend(wrong);
        fields.resize(4 + pad, String::new()); // incorrect-answer columns, aligned across rows
        fields.push(String::new()); // Categories
        fields.push(q.topics.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(", "));
        fields.push(q.explanation.as_deref().map(one_line).unwrap_or_default());
        rows.push(fields.join("\t"));
    }
    let mut out = rows.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    (out, skipped)
}

/// One line of plain text: tabs and newlines become single spaces (tab-delimited formats).
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Questions per type in print order (Test Parrot's: MC, T/F, Matching, Fill, Short, Multipart),
/// keeping the bank's own order inside each type.
pub fn grouped(bank: &QuestionBank) -> Vec<(QKind, Vec<&Question>)> {
    crate::bankops::KINDS
        .iter()
        .map(|k| (*k, bank.questions.iter().filter(|q| q.kind.qkind() == *k).collect::<Vec<_>>()))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

/// The cover note under the title: question count, what the file is, the types it holds.
/// Plain text (section directions print without markup).
pub fn cover_lines(bank: &QuestionBank) -> Vec<String> {
    let mut out = vec![
        plural(bank.questions.len(), "question"),
        "A digital file for importing into verSHion: it holds the whole question bank, answers included. Import works only from this original file: printing it, scanning it, or saving it again as a PDF removes the data verSHion reads.".to_string(),
    ];
    let types: Vec<String> = grouped(bank).iter().map(|(k, v)| format!("{} {}", kind_title(*k), v.len())).collect();
    out.push(format!("Question types: {}", types.join(" · ")));
    let d = &bank.details;
    let mut who: Vec<String> = Vec::new();
    if !d.author.trim().is_empty() {
        who.push(format!("By {}", d.author.trim()));
    }
    if !d.license_name.trim().is_empty() {
        who.push(d.license_name.trim().to_string());
    }
    if !who.is_empty() {
        out.push(who.join(" · "));
    }
    if !d.description.trim().is_empty() {
        out.push(d.description.trim().to_string());
    }
    out
}

/// The bank file as an exam document: one section per question type ("Multiple Choice",
/// "2 questions"), every question with its difficulty / topics line and its answers.
/// `image` resolves a question's picture (the caller reads the store's media).
pub fn bank_doc(bank: &QuestionBank, image: &dyn Fn(&crate::model::ImageRef) -> Option<DocImage>) -> ExamDoc {
    let mut questions = Vec::new();
    let mut n = 0u32;
    for (gi, (k, qs)) in grouped(bank).into_iter().enumerate() {
        for (i, q) in qs.into_iter().enumerate() {
            n += 1;
            let mut dq = bank_q(q, n, q.image.as_ref().and_then(image));
            if i == 0 {
                let mut directions = plural(dq_count(bank, k), "question");
                if gi == 0 {
                    directions = format!("{}\n\n{directions}", cover_lines(bank).join("\n\n"));
                }
                dq.section = Some(DocSection { title: kind_title(k).to_string(), directions });
            }
            questions.push(dq);
        }
    }
    ExamDoc {
        title: bank.name.clone(),
        header_line: "verSHion · Question bank file · for teachers: answers included".to_string(),
        heading_pt: 18.0,
        body_pt: 10.5,
        margin_mm: 18.0,
        answers_inline: true,
        questions,
        ..Default::default()
    }
}

fn dq_count(bank: &QuestionBank, k: QKind) -> usize {
    bank.questions.iter().filter(|q| q.kind.qkind() == k).count()
}

/// Reads a question's picture from the store for [`bank_doc`].
pub fn store_image(img: &crate::model::ImageRef) -> Option<DocImage> {
    let bytes = crate::store::read_media(&img.file)?;
    Some(DocImage { bytes, width_px: img.width, height_px: img.height })
}

/// Suggested file name: "chemistry-chapter-4-quiz-5a-and-5b.question-bank.pdf".
pub fn file_stem(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "question-bank".to_string() } else { format!("{out}.question-bank") }
}

// ---------- the data that rides along ----------

/// The bank as the JSON file (`bankui::bank_export_json`) plus every picture it uses, so the
/// file imports on another computer with its pictures: `"media": {"media/<hash>.png": base64}`.
pub fn bank_data(bank: &QuestionBank) -> serde_json::Value {
    let mut doc = crate::bankui::bank_export_json(bank);
    let mut media = BTreeMap::new();
    for q in &bank.questions {
        if let Some(img) = &q.image {
            if let Some(b) = crate::store::read_media(&img.file) {
                media.insert(img.file.clone(), serde_json::Value::String(b64_encode(&b)));
            }
        }
    }
    if !media.is_empty() {
        doc["media"] = serde_json::Value::Object(media.into_iter().collect());
    }
    doc
}

/// Writes the pictures a bank file carries into the store (only those missing there).
/// Returns how many were restored.
pub fn restore_media(json: &[u8]) -> usize {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(json) else { return 0 };
    let Some(m) = v.get("media").and_then(|m| m.as_object()) else { return 0 };
    let mut n = 0;
    for (file, data) in m {
        let Some(bytes) = data.as_str().and_then(b64_decode) else { continue };
        let want = crate::model::ImageRef { file: file.clone(), width: 0, height: 0 };
        if crate::store::media_exists(&want) {
            continue;
        }
        // put_image names the file by its content, the same name it had when exported.
        if crate::store::put_image(&bytes).is_ok_and(|img| img.file == *file) {
            n += 1;
        }
    }
    n
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= c.len() {
                out.push(B64[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ---------- PDF attachment ----------

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn rfind(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).rposition(|w| w == n)
}

/// A PDF text string as UTF-16BE hex (`<FEFF…>`), safe for any title.
fn pdf_text(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        out.push_str(&format!("{u:04X}"));
    }
    out.push('>');
    out
}

/// Appends `data` to a finished PDF as an embedded file attachment (`name`), plus document
/// info (title, subject), as an incremental update: the original bytes are untouched, new
/// objects, a new catalog revision, an xref section and a trailer pointing back at the old one.
pub fn attach_file(pdf: &[u8], name: &str, data: &[u8], title: &str, subject: &str) -> Result<Vec<u8>, String> {
    let tail = &pdf[pdf.len().saturating_sub(2048)..];
    let sx = rfind(tail, b"startxref").ok_or("not a PDF (no startxref)")?;
    let num: String = String::from_utf8_lossy(&tail[sx + 9..]).split_whitespace().next().unwrap_or("").to_string();
    let prev: usize = num.parse().map_err(|_| "unreadable startxref")?;
    let tr = rfind(pdf, b"trailer").ok_or("no trailer (xref streams are not supported)")?;
    let trailer = String::from_utf8_lossy(&pdf[tr..]).to_string();
    let int_after = |key: &str| -> Option<usize> {
        let at = trailer.find(key)? + key.len();
        trailer[at..].split_whitespace().next()?.trim_end_matches(">>").parse().ok()
    };
    let size = int_after("/Size").ok_or("trailer without /Size")?;
    let root = int_after("/Root").ok_or("trailer without /Root")?;
    // The catalog's current dictionary, to re-issue it with the attachment added.
    // At a line start: "1 0 obj" must not match the tail of "31 0 obj".
    let head = format!("\n{root} 0 obj");
    let at = rfind(pdf, head.as_bytes()).ok_or("catalog object not found")?;
    let body = &pdf[at + head.len()..];
    let end = find(body, b"endobj").ok_or("catalog object not closed")?;
    let dict = String::from_utf8_lossy(&body[..end]).trim().to_string();
    let close = dict.rfind(">>").ok_or("catalog is not a dictionary")?;
    if dict.contains("/Names") {
        return Err("the PDF already has a name tree".into());
    }
    let (ef, fs, info) = (size, size + 1, size + 2);
    let mut out = pdf.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let mut offs: Vec<(usize, usize)> = Vec::new();
    let z = {
        use std::io::Write;
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).map_err(|e| e.to_string())?;
        e.finish().map_err(|e| e.to_string())?
    };
    offs.push((ef, out.len()));
    out.extend_from_slice(format!("{ef} 0 obj\n<< /Type /EmbeddedFile /Subtype /application#2Fjson /Params << /Size {} >> /Filter /FlateDecode /Length {} >>\nstream\n", data.len(), z.len()).as_bytes());
    out.extend_from_slice(&z);
    out.extend_from_slice(b"\nendstream\nendobj\n");
    offs.push((fs, out.len()));
    out.extend_from_slice(
        format!("{fs} 0 obj\n<< /Type /Filespec /F {0} /UF {0} /Desc {1} /AFRelationship /Data /EF << /F {ef} 0 R /UF {ef} 0 R >> >>\nendobj\n", pdf_text(name), pdf_text("The whole question bank as data, for importing")).as_bytes(),
    );
    offs.push((root, out.len()));
    out.extend_from_slice(
        format!("{root} 0 obj\n{} /Names << /EmbeddedFiles << /Names [{} {fs} 0 R] >> >> /AF [{fs} 0 R] >>\nendobj\n", dict[..close].trim_end(), pdf_text(name)).as_bytes(),
    );
    offs.push((info, out.len()));
    out.extend_from_slice(format!("{info} 0 obj\n<< /Title {} /Subject {} /Creator (verSHion) /Producer (verSHion) >>\nendobj\n", pdf_text(title), pdf_text(subject)).as_bytes());
    offs.sort();
    let xref = out.len();
    let mut x = String::from("xref\n");
    // One subsection per run of consecutive object numbers.
    let mut i = 0;
    while i < offs.len() {
        let mut j = i;
        while j + 1 < offs.len() && offs[j + 1].0 == offs[j].0 + 1 {
            j += 1;
        }
        x.push_str(&format!("{} {}\n", offs[i].0, j - i + 1));
        for (_, o) in &offs[i..=j] {
            x.push_str(&format!("{o:010} 00000 n \n"));
        }
        i = j + 1;
    }
    x.push_str(&format!("trailer\n<< /Size {} /Root {root} 0 R /Info {info} 0 R /Prev {prev} >>\nstartxref\n{xref}\n%%EOF\n", info + 1));
    out.extend_from_slice(x.as_bytes());
    Ok(out)
}

/// The finished bank-file PDF: the exporter's pages plus the bank data attached.
pub fn bank_pdf(bank: &QuestionBank) -> Result<Vec<u8>, String> {
    let doc = bank_doc(bank, &store_image);
    let pdf = crate::export::render_pdf(&doc)?;
    let data = serde_json::to_vec_pretty(&bank_data(bank)).map_err(|e| e.to_string())?;
    attach_file(&pdf, ATTACHMENT, &data, &bank.name, "Teacher question bank containing answers; verSHion bank file")
}

/// Question data embedded in a PDF: a verSHion bank file's `queshion-bank.json`, or the
/// `parrot.json` inside Test Parrot's `parrot.zip` attachment. Scans the file's
/// `/Type /EmbeddedFile` streams (direct `/Length`, optional `/FlateDecode`).
pub fn embedded_data(pdf: &[u8]) -> Option<Vec<u8>> {
    if !pdf.starts_with(b"%PDF") {
        return None;
    }
    let mut from = 0;
    while let Some(i) = find(&pdf[from..], b"/EmbeddedFile") {
        let at = from + i;
        from = at + 13;
        // only the stream's own dictionary ("/Type /EmbeddedFile"), not "/EmbeddedFiles" name trees
        if pdf.get(at + 13).is_some_and(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        let dict_start = rfind(&pdf[..at], b"<<")?;
        let Some(s) = find(&pdf[at..], b"stream") else { continue };
        let dict = String::from_utf8_lossy(&pdf[dict_start..at + s]).to_string();
        let Some(len) = dict.split("/Length").nth(1).and_then(|t| {
            let mut w = t.split_whitespace();
            let n: usize = w.next()?.trim_end_matches(">>").parse().ok()?;
            // an indirect length ("12 0 R") is not followed here
            if w.next() == Some("0") && w.next().is_some_and(|r| r.starts_with('R')) { None } else { Some(n) }
        }) else {
            continue;
        };
        let mut p = at + s + 6;
        if pdf.get(p) == Some(&b'\r') {
            p += 1;
        }
        if pdf.get(p) == Some(&b'\n') {
            p += 1;
        }
        let Some(raw) = pdf.get(p..p + len) else { continue };
        let bytes = if dict.contains("/FlateDecode") {
            use std::io::Read;
            let mut out = Vec::new();
            if flate2::read::ZlibDecoder::new(raw).read_to_end(&mut out).is_err() {
                continue;
            }
            out
        } else {
            raw.to_vec()
        };
        if bytes.starts_with(b"PK") {
            use std::io::Read;
            let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(&bytes)) else { continue };
            for k in 0..z.len() {
                let Ok(mut f) = z.by_index(k) else { continue };
                if f.name().to_ascii_lowercase().ends_with(".json") {
                    let mut out = Vec::new();
                    if f.read_to_end(&mut out).is_ok() {
                        return Some(out);
                    }
                }
            }
            continue;
        }
        if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
            return Some(bytes);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::make_question;

    /// A keyless (pool-imported) question: its placeholder values never print as a key.
    #[test]
    fn keyless_questions_export_no_key() {
        let sentinel = Some(crate::model::KEYLESS_SENTINEL.to_string());
        let mut tf = make_question("The sun is a star.".into(), Kind::Tf { correct: false });
        tf.explanation = sentinel.clone();
        let mut m = make_question(
            "Match".into(),
            Kind::Matching { pairs: vec![crate::model::Pair { left: "Dog".into(), right: String::new() }, crate::model::Pair { left: "Frog".into(), right: String::new() }], distractors: vec!["Mammal".into(), "Amphibian".into()] },
        );
        m.explanation = sentinel.clone();
        let mut mp = make_question("Parts".into(), Kind::Multipart { parts: vec![Part { stem: "a. x".into(), kind: PartKind::Tf { correct: false } }] });
        mp.explanation = sentinel;
        let bank = QuestionBank { id: "b".into(), name: "n".into(), questions: vec![tf.clone(), m.clone(), mp.clone()], created: 0, modified: 0, details: Default::default() };
        for q in &bank.questions {
            let d = bank_q(q, 1, None);
            assert_eq!(d.key_text, crate::model::KEY_NOT_PROVIDED);
            let t = d.text.join("\n");
            assert!(!t.contains("**Answer:** False") && !t.contains("→"), "{t}");
        }
        let m_text = bank_q(&m, 1, None).text.join("\n");
        assert!(m_text.contains("Choices: Mammal, Amphibian"), "{m_text}");
        let csv = bank_csv(&bank);
        assert_eq!(csv.matches(crate::model::KEY_NOT_PROVIDED).count(), 3, "{csv}");
        assert!(!csv.contains(",False,") && !csv.contains(",True,"), "{csv}");
        let (bb, skipped) = bank_blackboard(&bank);
        assert_eq!((bb.trim(), skipped), ("", 3));
        let mut mc = make_question("Pick".into(), Kind::Mc { answers: vec![Answer { text: "x".into(), correct: false, locked: false }, Answer { text: "y".into(), correct: false, locked: false }] });
        mc.explanation = Some(crate::model::KEYLESS_SENTINEL.to_string());
        let (aiken, skipped) = bank_aiken(&QuestionBank { questions: vec![mc], ..bank });
        assert_eq!((aiken.as_str(), skipped), ("", 1));
    }
    use crate::model::{Answer, Part};

    fn bank() -> QuestionBank {
        let mut mc = make_question(
            "Which of the following elements is the **most** electronegative?".into(),
            Kind::Mc { answers: ["Ge", "Ca", "O", "C"].iter().map(|t| Answer { text: t.to_string(), correct: *t == "O", locked: false }).collect() },
        );
        mc.topics = vec!["Electronegativity".into()];
        mc.difficulty = Some(crate::model::Difficulty::Low);
        let fill = make_question("BONDING ELECTRONS: ___ NONBONDING ELECTRONS: ___".into(), Kind::Blank { accept: vec![vec!["14".into()], vec!["10".into()]] });
        let short = make_question("How many grams?".into(), Kind::Short { sample: "3.1 g".into() });
        let multi = make_question(
            "For each compound:".into(),
            Kind::Multipart { parts: vec![Part { stem: "CO₂".into(), kind: PartKind::Short { sample: "linear".into() } }, Part { stem: "b. Is it polar?".into(), kind: PartKind::Tf { correct: false } }] },
        );
        QuestionBank { id: "b".into(), name: "Chemistry Chapter 4 Quiz (5A and 5B)".into(), questions: vec![short, mc, fill, multi], created: 1, modified: 1, details: Default::default() }
    }

    #[test]
    fn bank_doc_groups_by_type_with_answers_marked() {
        let b = bank();
        let d = bank_doc(&b, &|_| None);
        assert_eq!(d.title, "Chemistry Chapter 4 Quiz (5A and 5B)");
        assert_eq!(d.questions.len(), 4);
        let numbers: Vec<u32> = d.questions.iter().map(|q| q.number).collect();
        assert_eq!(numbers, vec![1, 2, 3, 4]);
        let secs: Vec<String> = d.questions.iter().filter_map(|q| q.section.as_ref().map(|s| s.title.clone())).collect();
        assert_eq!(secs, vec!["Multiple Choice", "Fill in the Blank", "Short Answer", "Multipart"]);
        let first = d.questions[0].section.as_ref().unwrap();
        assert!(first.directions.contains("4 questions") && first.directions.contains("A digital file for importing"), "{}", first.directions);
        assert!(first.directions.ends_with("1 question"), "{}", first.directions);
        let mc = &d.questions[0];
        assert_eq!(mc.choices, vec!["A. Ge", "B. Ca", "C. O  **(Correct answer)**", "D. C"]);
        assert_eq!(mc.text.len(), 1, "one text entry, or it would print as matching");
        assert!(mc.text[0].contains("*Difficulty:* Easy · *Topics:* Electronegativity"));
        assert_eq!(mc.key_text, "C");
        assert!(d.questions[1].text[0].ends_with("**Answers**\n14; 10"), "{}", d.questions[1].text[0]);
        assert!(d.questions[2].text[0].ends_with("**Suggested Answer**\n3.1 g"));
        let m = &d.questions[3].text[0];
        assert!(m.contains("\n\na. CO₂\n\n**Suggested Answer**\nlinear") && m.contains("\n\nb. Is it polar?\n**Answer:** False"), "{m}");
        // the exporter lays it out
        let pdf = crate::export::render_pdf(&d).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn bank_csv_has_one_row_per_question_with_answers_and_choices() {
        let csv = bank_csv(&bank());
        // Records end in CRLF; the multipart question's embedded LF stays inside its field.
        let rows: Vec<&str> = csv.split("\r\n").collect();
        assert_eq!(rows.len(), 6, "{csv}");
        assert_eq!(rows[5], "");
        assert_eq!(rows[0], "#,type,points,question,answer,choices,explanation,topics,difficulty");
        assert!(rows[1].contains(",Short Answer,"), "{}", rows[1]);
        assert!(rows[2].contains(",Multiple Choice,"), "{}", rows[2]);
        assert!(rows[2].contains("A. Ge | B. Ca | C. O | D. C"), "{}", rows[2]);
        assert!(rows[2].contains(",O,"), "{}", rows[2]);
        assert!(rows[3].contains("14; 10"), "{}", rows[3]);
        assert!(rows[4].contains("a. linear; b. False"), "{}", rows[4]);
    }

    #[test]
    fn bank_csv_quotes_commas_quotes_and_newlines() {
        let mut q = make_question(
            "Pick, \"one\".\nNext line".into(),
            Kind::Mc { answers: vec![Answer { text: "x, y".into(), correct: true, locked: false }] },
        );
        q.explanation = Some("Line one\nLine two".into());
        let b = QuestionBank { id: "b2".into(), name: "Quotes".into(), questions: vec![q], created: 0, modified: 0, details: Default::default() };
        let csv = bank_csv(&b);
        assert!(csv.contains("\"Pick, \"\"one\"\".\nNext line\""), "{csv}");
        assert!(csv.contains("\"Line one\nLine two\""), "{csv}");
        assert!(csv.contains("\"x, y\""), "{csv}");
    }

    #[test]
    fn empty_bank_csv_is_the_header_only() {
        let b = QuestionBank { id: "b3".into(), name: "Empty".into(), questions: Vec::new(), created: 0, modified: 0, details: Default::default() };
        assert_eq!(bank_csv(&b), "#,type,points,question,answer,choices,explanation,topics,difficulty\r\n");
    }

    #[test]
    fn aiken_keeps_single_answer_mc_and_counts_the_rest() {
        let (text, skipped) = bank_aiken(&bank());
        assert_eq!(skipped, 3, "short, fill and multipart are skipped");
        assert_eq!(
            text,
            "Which of the following elements is the **most** electronegative?\nA. Ge\nB. Ca\nC. O\nD. C\nANSWER: C\n"
        );
    }

    #[test]
    fn aiken_skips_multi_answer_mc() {
        let q = make_question(
            "Pick both.".into(),
            Kind::Mc {
                answers: vec![
                    Answer { text: "one".into(), correct: true, locked: false },
                    Answer { text: "two".into(), correct: true, locked: false },
                    Answer { text: "three".into(), correct: false, locked: false },
                ],
            },
        );
        let b = QuestionBank { id: "b4".into(), name: "MA".into(), questions: vec![q], created: 0, modified: 0, details: Default::default() };
        assert_eq!(bank_aiken(&b), (String::new(), 1));
    }

    #[test]
    fn blackboard_rows_are_aligned_tab_delimited_and_skip_unsupported() {
        let (text, skipped) = bank_blackboard(&bank());
        assert_eq!(skipped, 1, "multipart is skipped");
        let rows: Vec<&str> = text.lines().collect();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r.split('\t').count() == rows[0].split('\t').count()), "{text}");
        let short = rows[0].split('\t').collect::<Vec<_>>();
        assert_eq!(short[1], "ESS");
        let mc = rows[1].split('\t').collect::<Vec<_>>();
        assert_eq!(mc[1], "MC");
        assert_eq!(mc[3], "O");
        assert_eq!(&mc[4..7], &["Ge", "Ca", "C"]);
        let fill = rows[2].split('\t').collect::<Vec<_>>();
        assert_eq!(fill[1], "FIB");
        assert_eq!(fill[3], "14; 10");
    }

    #[test]
    fn blackboard_flattens_tabs_and_newlines() {
        let mut q = make_question("Line one\nline\ttwo".into(), Kind::Short { sample: "a\nb".into() });
        q.explanation = Some("why\nnot".into());
        let b = QuestionBank { id: "b5".into(), name: "Flatten".into(), questions: vec![q], created: 0, modified: 0, details: Default::default() };
        let (text, skipped) = bank_blackboard(&b);
        assert_eq!(skipped, 0);
        assert_eq!(text, "Line one line two\tESS\t\ta b\t\t\twhy not\n");
    }

    #[test]
    fn attachment_round_trips_through_the_pdf() {
        let b = bank();
        let mut b = b;
        // enough pages that object numbers like 11, 21, 31 exist ("1 0 obj" must not match them)
        let more: Vec<Question> = (0..12).flat_map(|_| b.questions.clone()).collect();
        b.questions.extend(more);
        let d = bank_doc(&b, &|_| None);
        let pdf = crate::export::render_pdf(&d).unwrap();
        assert!(String::from_utf8_lossy(&pdf).contains("\n31 0 obj"));
        let data = serde_json::to_vec(&crate::bankui::bank_export_json(&b)).unwrap();
        let out = attach_file(&pdf, ATTACHMENT, &data, &b.name, "subject").unwrap();
        assert!(out.starts_with(&pdf), "incremental update keeps the original bytes");
        assert_eq!(embedded_data(&out).as_deref(), Some(&data[..]));
        assert!(embedded_data(&pdf).is_none());
        // and it imports as the same bank
        let doc = crate::formats::detect_and_parse(&embedded_data(&out).unwrap(), "x.json").unwrap();
        assert_eq!(doc.name, b.name);
        assert_eq!(doc.questions.len(), b.questions.len());
        assert_eq!(serde_json::to_value(&doc.questions[1].kind).unwrap(), serde_json::to_value(&b.questions[1].kind).unwrap());
        // the new xref section points at the objects it lists
        // (byte offsets: work on the bytes, a lossy string would shift them)
        let tail = String::from_utf8_lossy(&out[out.len() - 400..]).to_string();
        let sx: usize = tail.rsplit("startxref").next().unwrap().split_whitespace().next().unwrap().parse().unwrap();
        assert!(out[sx..].starts_with(b"xref"));
        let xref = String::from_utf8_lossy(&out[sx..]).to_string();
        for line in xref.lines().filter(|l| l.ends_with(" n ")) {
            let off: usize = line[..10].parse().unwrap();
            let at = String::from_utf8_lossy(&out[off..off + 20]).to_string();
            assert!(at.split_whitespace().nth(2) == Some("obj"), "{line} -> {at:?}");
        }
        assert!(xref.contains("/Prev "));
        // pdfinfo / pdfdetach agree, when poppler is installed
        let dir = std::env::temp_dir().join(format!("bankexport-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("b.pdf");
        std::fs::write(&f, &out).unwrap();
        if let Ok(o) = std::process::Command::new("pdfdetach").arg("-list").arg(&f).output() {
            let t = String::from_utf8_lossy(&o.stdout);
            assert!(t.contains(ATTACHMENT), "{t}");
        }
        if let Ok(o) = std::process::Command::new("pdfinfo").arg(&f).output() {
            let t = String::from_utf8_lossy(&o.stdout);
            assert!(t.contains("Chemistry Chapter 4 Quiz (5A and 5B)"), "{t}");
            let pages = String::from_utf8_lossy(&std::process::Command::new("pdfinfo").arg(&pdf_path(&dir, &pdf)).output().unwrap().stdout).to_string();
            let n = |t: &str| t.lines().find(|l| l.starts_with("Pages:")).map(|l| l.to_string());
            assert_eq!(n(&t), n(&pages), "the update keeps every page");
            assert!(String::from_utf8_lossy(&o.stderr).trim().is_empty(), "{}", String::from_utf8_lossy(&o.stderr));
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    fn pdf_path(dir: &std::path::Path, pdf: &[u8]) -> std::path::PathBuf {
        let p = dir.join("plain.pdf");
        std::fs::write(&p, pdf).unwrap();
        p
    }

    #[test]
    fn test_parrot_zip_attachments_are_read_too() {
        use std::io::Write;
        let json = br#"{"format":"test-parrot/package"}"#;
        let mut zbuf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut zbuf));
            z.start_file("parrot.json", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(json).unwrap();
            z.finish().unwrap();
        }
        let mut pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /EmbeddedFiles >>\nendobj\n61 0 obj\n<<\n/Type /EmbeddedFile\n/Subtype /application#2Fzip\n/Length ".to_vec();
        pdf.extend_from_slice(format!("{}\n>>\nstream\n", zbuf.len()).as_bytes());
        pdf.extend_from_slice(&zbuf);
        pdf.extend_from_slice(b"\nendstream\nendobj\n%%EOF\n");
        assert_eq!(embedded_data(&pdf).as_deref(), Some(&json[..]));
    }

    #[test]
    fn base64_and_file_names() {
        for s in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar", &[0u8, 255, 128, 7][..]] {
            assert_eq!(b64_decode(&b64_encode(s)).unwrap(), s);
        }
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64_encode(b"fo"), "Zm8=");
        assert_eq!(file_stem("Chemistry Chapter 4 Quiz (5A and 5B)"), "chemistry-chapter-4-quiz-5a-and-5b.question-bank");
        assert_eq!(file_stem("  ¿?  "), "question-bank");
    }
}
