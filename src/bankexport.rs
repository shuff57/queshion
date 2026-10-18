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
    let mut text = q.stem.trim_end().to_string();
    let mut parts: Vec<String> = vec![meta_line(q)];
    let mut choices: Vec<String> = Vec::new();
    let mut key = String::new();
    match &q.kind {
        Kind::Mc { answers } => {
            for (i, a) in answers.iter().enumerate() {
                let t = a.text.trim();
                choices.push(if a.correct { format!("{}. {t}  **(Correct answer)**", letter(i)) } else { format!("{}. {t}", letter(i)) });
            }
            key = answers.iter().enumerate().filter(|(_, a)| a.correct).map(|(i, _)| letter(i).to_string()).collect::<Vec<_>>().join(", ");
        }
        Kind::Tf { correct } => {
            key = if *correct { "True" } else { "False" }.to_string();
            parts.push(format!("**Answer:** {key}"));
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
                        let k = if *correct { "True" } else { "False" };
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
    if let Some(e) = q.explanation.as_deref().map(str::trim).filter(|e| !e.is_empty()) {
        parts.push(block("Explanation", e));
    }
    for p in parts {
        text.push_str("\n\n");
        text.push_str(&p);
    }
    DocQ { number, text: vec![text], choices, image, key_text: key, rule_lines: 0, no_answer_space: true, points: q.points, ..Default::default() }
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
