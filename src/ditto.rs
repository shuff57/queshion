//! verSHion Ditto package (`.ditto.json`, see `docs/ditto-format.md`): what the AI conversion
//! prompt (`prompts/ditto-convert.md`) produces. Import is tolerant: each question is checked
//! alone, a bad one is skipped with a warning, and figure boxes are cropped out of the page
//! image the model was shown.
use crate::formats::ImportDoc;
use crate::model::{Answer, Difficulty, Exam, ExamItem, Kind, Pair, Part, PartKind, Question};
use serde_json::Value;
use std::collections::HashMap;

/// Stable data-format identifier, deliberately unchanged by the verSHion rename: existing
/// `.ditto.json` files and the AI conversion prompt all carry `queshion/ditto`.
pub const FORMAT: &str = "queshion/ditto";

#[derive(Debug)]
pub struct DittoImport {
    pub doc: ImportDoc,
    /// One human sentence per skipped question, dropped figure or repaired field.
    pub warnings: Vec<String>,
    /// How many figures were cropped and attached.
    pub figures: usize,
}

/// The JSON object inside a model reply, tolerating a code fence or chatter around it.
pub fn json_object(text: &str) -> Option<Value> {
    let (a, b) = (text.find('{')?, text.rfind('}')?);
    if b <= a {
        return None;
    }
    serde_json::from_str(&text[a..=b]).ok()
}

/// Why a reply isn't usable JSON, or None when it parses (used to ask the model to retry).
pub fn json_error(text: &str) -> Option<String> {
    let (Some(a), Some(b)) = (text.find('{'), text.rfind('}')) else {
        return Some("there was no JSON object in the reply".into());
    };
    if b <= a {
        return Some("the JSON object was not closed".into());
    }
    serde_json::from_str::<Value>(&text[a..=b]).err().map(|e| e.to_string())
}

pub fn is_ditto(text: &str) -> bool {
    json_object(text).is_some_and(|v| v.get("format").and_then(Value::as_str) == Some(FORMAT))
}

/// Parses ditto JSON. `page_png` is the page raster the figure boxes refer to (`page_no`
/// is its 1-based number); with no raster, figures are dropped with a warning.
pub fn import_text(text: &str, page_png: Option<&[u8]>, page_no: usize, fallback_name: &str) -> Result<DittoImport, String> {
    let v = json_object(text).ok_or("The reply was not a JSON object.")?;
    if v.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(format!("Not a verSHion Ditto file (format should be \"{FORMAT}\")."));
    }
    let bank = v.get("bank").ok_or("The file has no \"bank\".")?;
    let name = bank.get("name").and_then(Value::as_str).filter(|n| !n.trim().is_empty()).unwrap_or(fallback_name).to_string();
    let raw = bank.get("questions").and_then(Value::as_array).ok_or("The bank has no \"questions\" list.")?;

    let page = page_png.and_then(|b| image::load_from_memory(b).ok());
    let mut warnings = Vec::new();
    let mut questions = Vec::new();
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut figures = 0;
    for (i, item) in raw.iter().enumerate() {
        let label = item.get("label").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("#{}", i + 1));
        match question(item) {
            Ok(mut q) => {
                if let Some(fig) = item.get("figure").filter(|f| !f.is_null()) {
                    match crop(fig, page.as_ref(), page_no) {
                        Ok(png) => match crate::store::put_image(&png) {
                            Ok(img) => {
                                q.image = Some(img);
                                figures += 1;
                            }
                            Err(e) => warnings.push(format!("Question {label}: couldn't save its figure ({e}).")),
                        },
                        Err(e) => warnings.push(format!("Question {label}: figure dropped ({e}).")),
                    }
                }
                if let Some(id) = item.get("id").and_then(Value::as_str) {
                    ids.insert(id.to_string(), q.id.clone());
                }
                questions.push(q);
            }
            Err(e) => warnings.push(format!("Question {label} skipped: {e}")),
        }
    }
    if let Some(rep) = v.pointer("/report/unconverted").and_then(Value::as_array) {
        for u in rep {
            let l = u.get("label").map(|l| l.as_str().map(str::to_string).unwrap_or_else(|| l.to_string())).unwrap_or_default();
            let why = u.get("reason").and_then(Value::as_str).unwrap_or("unreadable");
            warnings.push(format!("The model left question {l} out: {why}."));
        }
    }
    let exams = exams(&v, &ids);
    Ok(DittoImport { doc: ImportDoc { name, questions, exams }, warnings, figures })
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("").trim()
}

fn question(item: &Value) -> Result<Question, String> {
    let stem = text(item, "stem").to_string();
    if stem.is_empty() {
        return Err("it has no stem.".into());
    }
    let ty = text(item, "type");
    // Tables of blanks are cleaned for every type (blank questions also realign their answers).
    let stem = if ty == "fill-in-the-blank" { stem } else { normalize_tables(&stem, &[]).stem };
    let (stem, kind) = match ty {
        "multiple-choice" => (stem, Kind::Mc { answers: choices(item)? }),
        "true-false" => (stem, Kind::Tf { correct: item.get("answer").and_then(Value::as_bool).unwrap_or(true) }),
        "matching" => {
            let pairs: Vec<Pair> = item
                .get("pairs")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|p| Pair { left: text(p, "left").into(), right: text(p, "right").into() }).filter(|p| !p.left.is_empty() && !p.right.is_empty()).collect())
                .unwrap_or_default();
            if pairs.len() < 2 {
                return Err("a matching question needs at least two pairs.".into());
            }
            let distractors = strings(item.get("distractors"));
            (stem, Kind::Matching { pairs, distractors })
        }
        "fill-in-the-blank" => blank(stem, item),
        "short-answer" => (stem, Kind::Short { sample: text(item, "suggestedAnswer").to_string() }),
        "multipart" => {
            let parts: Vec<Part> = item.get("parts").and_then(Value::as_array).map(|a| a.iter().filter_map(part).collect()).unwrap_or_default();
            if parts.is_empty() {
                return Err("a multipart question needs at least one part.".into());
            }
            (stem, Kind::Multipart { parts })
        }
        other => return Err(format!("unknown type \"{other}\".")),
    };
    let mut q = crate::formats::make_question(stem, kind);
    q.points = item.get("points").and_then(Value::as_f64).filter(|p| *p > 0.0);
    q.difficulty = match text(item, "difficulty") {
        "easy" => Some(Difficulty::Low),
        "medium" => Some(Difficulty::Medium),
        "hard" => Some(Difficulty::High),
        _ => None,
    };
    q.topics = strings(item.get("topics"));
    q.explanation = Some(text(item, "explanation").to_string()).filter(|e| !e.is_empty());
    Ok(q)
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
}

fn choices(item: &Value) -> Result<Vec<Answer>, String> {
    let answers: Vec<Answer> = item
        .get("choices")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|c| Answer { text: text(c, "text").to_string(), correct: c.get("correct").and_then(Value::as_bool).unwrap_or(false), locked: false }).filter(|a| !a.text.is_empty()).collect())
        .unwrap_or_default();
    if answers.len() < 2 {
        return Err("a multiple-choice question needs at least two choices.".into());
    }
    Ok(answers)
}

/// A blank question: one accepted-answer list per `___` in the stem. A stem with no marker
/// can't be a blank question, so it degrades to short answer with the answers as the sample.
fn blank(stem: String, item: &Value) -> (String, Kind) {
    let mut accept: Vec<Vec<String>> = item.get("blanks").and_then(Value::as_array).map(|a| a.iter().map(|b| match b {
        Value::String(s) => vec![s.trim().to_string()],
        other => strings(Some(other)),
    }).collect()).unwrap_or_default();
    let before = stem.matches("___").count();
    let fix = normalize_tables(&stem, &[accept.iter().filter(|a| !a.is_empty()).count(), accept.iter().map(Vec::len).sum()]);
    let stem = fix.stem;
    let markers = stem.matches("___").count();
    if markers == 0 {
        let sample = accept.iter().filter(|a| !a.is_empty()).map(|a| a.join(" OR ")).collect::<Vec<_>>().join("; ");
        return (stem, Kind::Short { sample });
    }
    let basic = |mut acc: Vec<Vec<String>>| {
        // Trailing empty entries are padding, not answers (resize pads them back).
        while acc.last().is_some_and(Vec::is_empty) {
            acc.pop();
        }
        // Repair: the model grouped each row's answers into one entry ([[a, b], [c, d]] for
        // four blanks). When flattening them gives exactly one answer per blank, that was it.
        if acc.len() < markers && acc.iter().map(Vec::len).sum::<usize>() == markers {
            acc = acc.into_iter().flatten().map(|a| vec![a]).collect();
        }
        acc
    };
    let fitted = basic(accept.clone());
    accept = if fitted.len() == markers || accept.len() != before || fix.dropped.is_empty() {
        fitted
    } else {
        // Repair: the header row lost its blanks and the model gave one entry per ORIGINAL
        // blank (the header's ones empty or junk): drop exactly those entries.
        basic(accept.into_iter().enumerate().filter(|(i, _)| !fix.dropped.contains(i)).map(|(_, a)| a).collect())
    };
    accept.resize(markers, Vec::new());
    // A model sometimes repeats an alternative ([["Ionic", "Ionic"]]): keep each once.
    for alts in &mut accept {
        let mut seen: Vec<String> = Vec::new();
        alts.retain(|a| !seen.contains(a) && {
            seen.push(a.clone());
            true
        });
    }
    (stem, Kind::Blank { accept })
}

/// A stem after [`normalize_tables`], and which `___` markers of the original stem (0-based,
/// reading order) were removed because they sat in a header row.
#[derive(Debug, PartialEq)]
pub(crate) struct TableFix {
    pub stem: String,
    pub dropped: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
enum Cell {
    Blank,
    Labeled(String),
    Other(String),
}

fn classify(c: &str) -> Cell {
    let t = c.trim();
    let core = t.trim_end_matches('_');
    if t.len() - core.len() >= 3 {
        let label = core.trim().trim_end_matches(':').trim();
        if label.is_empty() {
            return Cell::Blank;
        }
        if !label.contains("___") {
            return Cell::Labeled(label.to_string());
        }
    }
    Cell::Other(t.to_string())
}

/// Cells of a pipe row (the same reading as `richtext`): None for a line with < 2 cells.
fn pipe_cells(line: &str) -> Option<Vec<String>> {
    if !line.contains('|') {
        return None;
    }
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    let c: Vec<String> = t.split('|').map(|x| x.trim().to_string()).collect();
    (c.len() >= 2).then_some(c)
}

fn is_separator(c: &[String]) -> bool {
    c.iter().all(|x| !x.is_empty() && x.contains('-') && x.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

fn pipe_row(cells: &[String]) -> String {
    if cells.first().is_some_and(String::is_empty) || cells.last().is_some_and(String::is_empty) {
        format!("| {} |", cells.join(" | "))
    } else {
        cells.join(" | ")
    }
}

/// Words a first column's header cell typically says (used only when nothing else decides
/// whether the first row of a label-repeating table is a header).
fn looks_like_title(s: &str) -> bool {
    const TITLES: [&str; 24] = [
        "formula", "compound", "name", "symbol", "element", "term", "word", "item", "molecule", "substance", "ion", "question",
        "problem", "number", "#", "equation", "reaction", "sample", "object", "material", "quantity", "value", "unit", "chemical formula",
    ];
    let l = s.trim().trim_end_matches(':').to_lowercase();
    let l = l.strip_suffix('s').filter(|x| TITLES.contains(x)).map(str::to_string).unwrap_or(l);
    TITLES.contains(&l.as_str())
}

/// The common model slip in a pipe table of blanks: every row repeats the column's header
/// label in front of its blank (`KI | Ionic or Covalent? ___ | Name ___`), and the header row
/// itself may carry blanks (`Formula | Ionic or Covalent? ___ | Name ___`). Rewrites the table
/// to the clean shape — a header row WITHOUT blanks, body rows with a bare `___` per answer
/// cell — synthesising the header (first cells empty) when the source rows have none.
/// `answer_counts` are the model's answer counts (entries, flattened answers) used to tell a
/// header row from a data row when the rows look alike. Stems without such a table come back
/// unchanged.
pub(crate) fn normalize_tables(stem: &str, answer_counts: &[usize]) -> TableFix {
    let lines: Vec<&str> = stem.split('\n').collect();
    let total = stem.matches("___").count();
    let mut out: Vec<String> = Vec::new();
    let mut dropped: Vec<usize> = Vec::new();
    let mut markers_before = 0usize;
    let mut i = 0;
    while i < lines.len() {
        let Some(cols) = pipe_cells(lines[i]).map(|c| c.len()) else {
            markers_before += lines[i].matches("___").count();
            out.push(lines[i].to_string());
            i += 1;
            continue;
        };
        let mut j = i;
        while j < lines.len() && pipe_cells(lines[j]).is_some_and(|c| c.len() == cols || is_separator(&c)) {
            j += 1;
        }
        let run = &lines[i..j];
        let base = markers_before;
        markers_before += run.iter().map(|l| l.matches("___").count()).sum::<usize>();
        i = j;
        match fix_table(run, cols, base, total, answer_counts) {
            Some((rows, gone)) => {
                out.extend(rows);
                dropped.extend(gone);
            }
            None => out.extend(run.iter().map(|l| l.to_string())),
        }
    }
    TableFix { stem: out.join("\n"), dropped }
}

/// One table run → its clean lines and the dropped header markers, or None when the table is
/// not the label-repeating shape (left exactly as written).
fn fix_table(run: &[&str], cols: usize, base: usize, total: usize, answer_counts: &[usize]) -> Option<(Vec<String>, Vec<usize>)> {
    let parsed: Vec<Vec<String>> = run.iter().filter_map(|l| pipe_cells(l)).collect();
    let data_idx: Vec<usize> = (0..parsed.len()).filter(|&r| !is_separator(&parsed[r])).collect();
    if data_idx.len() < 2 || data_idx[0] != 0 {
        return None;
    }
    let sep_after_first = parsed.get(1).is_some_and(|r| is_separator(r));
    let grid: Vec<Vec<Cell>> = data_idx.iter().map(|&r| parsed[r].iter().map(|c| classify(c)).collect()).collect();
    // Label columns: every row after the first is a blank (bare or labelled), the labels agree.
    let mut labels: Vec<Option<String>> = vec![None; cols];
    let mut any_repeat = false;
    for (c, slot) in labels.iter_mut().enumerate() {
        let rest: Vec<&Cell> = grid[1..].iter().map(|r| &r[c]).collect();
        if !rest.iter().all(|x| matches!(x, Cell::Blank | Cell::Labeled(_))) {
            continue;
        }
        let mut seen: Option<String> = None;
        let mut agree = true;
        for x in &rest {
            if let Cell::Labeled(l) = x {
                match &seen {
                    Some(s) if !s.eq_ignore_ascii_case(l) => agree = false,
                    Some(_) => {}
                    None => seen = Some(l.clone()),
                }
            }
        }
        if !agree {
            continue;
        }
        if seen.is_some() {
            any_repeat = true;
        }
        let first = match &grid[0][c] {
            Cell::Labeled(l) => Some(l.clone()),
            _ => None,
        };
        *slot = seen.or(first);
    }
    let label_cols: Vec<usize> = (0..cols).filter(|&c| labels[c].is_some()).collect();
    if label_cols.is_empty() {
        return None;
    }
    let first_labeled = label_cols.iter().any(|&c| matches!(grid[0][c], Cell::Labeled(_)));
    if !any_repeat && !first_labeled {
        return None;
    }
    // Is the first row the header?
    let k = label_cols.iter().filter(|&&c| matches!(grid[0][c], Cell::Labeled(_) | Cell::Blank)).count();
    let header = if sep_after_first || label_cols.iter().any(|&c| matches!(grid[0][c], Cell::Other(_))) || !any_repeat {
        true
    } else {
        let as_header = answer_counts.iter().any(|&a| a > 0 && a == total - k);
        let as_data = answer_counts.iter().any(|&a| a > 0 && a == total);
        match (as_header, as_data) {
            (true, false) => true,
            (false, true) => false,
            _ => {
                let others: Vec<&Cell> = (0..cols).filter(|c| !label_cols.contains(c)).map(|c| &grid[0][c]).collect();
                !others.is_empty() && others.iter().all(|x| matches!(x, Cell::Other(t) if looks_like_title(t)))
            }
        }
    };
    let mut lines: Vec<String> = Vec::new();
    let mut gone: Vec<usize> = Vec::new();
    let mut seen_markers = base;
    for (r, raw) in parsed.iter().enumerate() {
        if is_separator(raw) {
            lines.push(run[r].to_string());
            continue;
        }
        let is_head = header && r == 0;
        let mut cells: Vec<String> = Vec::new();
        for (c, text) in raw.iter().enumerate() {
            let n = text.matches("___").count();
            if let Some(label) = labels[c].as_ref() {
                if is_head {
                    gone.extend(seen_markers..seen_markers + n);
                    cells.push(match &grid[0][c] {
                        Cell::Other(t) => t.clone(),
                        _ => label.clone(),
                    });
                } else {
                    cells.push("___".to_string());
                }
            } else {
                cells.push(text.clone());
            }
            seen_markers += n;
        }
        if r == 0 && !header {
            let head: Vec<String> = (0..cols).map(|c| labels[c].clone().unwrap_or_default()).collect();
            lines.push(pipe_row(&head));
        }
        lines.push(pipe_row(&cells));
    }
    Some((lines, gone))
}

fn part(p: &Value) -> Option<Part> {
    let stem = normalize_tables(text(p, "stem"), &[]).stem;
    if stem.is_empty() {
        return None;
    }
    let kind = match text(p, "type") {
        "multiple-choice" => PartKind::Mc { answers: choices(p).ok()? },
        "true-false" => PartKind::Tf { correct: p.get("answer").and_then(Value::as_bool).unwrap_or(true) },
        // A part can't hold blanks; keep the key as its sample answer.
        "fill-in-the-blank" => {
            let key: Vec<String> = p.get("blanks").and_then(Value::as_array).map(|a| a.iter().map(|b| match b {
                Value::String(s) => s.trim().to_string(),
                other => strings(Some(other)).join(" OR "),
            }).collect()).unwrap_or_default();
            PartKind::Short { sample: key.join("; ") }
        }
        _ => PartKind::Short { sample: text(p, "suggestedAnswer").to_string() },
    };
    Some(Part { stem, kind })
}

/// Crops `figure.box` (fractions of the page, top-left origin) out of the page raster.
fn crop(fig: &Value, page: Option<&image::DynamicImage>, page_no: usize) -> Result<Vec<u8>, String> {
    let page = page.ok_or("no page image to crop from")?;
    if let Some(p) = fig.get("page").and_then(Value::as_u64)
        && p as usize != page_no
    {
        return Err(format!("it points at page {p}, this is page {page_no}"));
    }
    let b: Vec<f64> = fig.get("box").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if b.len() != 4 {
        return Err("its box needs four numbers".into());
    }
    let (w, h) = (page.width() as f64, page.height() as f64);
    let pad = 0.012;
    let (x0, y0) = ((b[0] - pad).clamp(0.0, 1.0), (b[1] - pad).clamp(0.0, 1.0));
    let (x1, y1) = ((b[2] + pad).clamp(0.0, 1.0), (b[3] + pad).clamp(0.0, 1.0));
    let (px, py) = ((x0 * w) as u32, (y0 * h) as u32);
    let (pw, ph) = (((x1 - x0) * w) as u32, ((y1 - y0) * h) as u32);
    if pw < 24 || ph < 24 {
        return Err("its box is empty or off the page".into());
    }
    if pw as f64 > w * 0.98 && ph as f64 > h * 0.9 {
        return Err("its box covers the whole page".into());
    }
    let mut out = Vec::new();
    page.crop_imm(px, py, pw, ph)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

fn exams(v: &Value, ids: &HashMap<String, String>) -> Vec<Exam> {
    let mut out = Vec::new();
    for e in v.get("exams").and_then(Value::as_array).into_iter().flatten() {
        let name = text(e, "name");
        let items: Vec<ExamItem> = e
            .get("positions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p.get("question").and_then(Value::as_str).and_then(|q| ids.get(q)))
            .map(|q| ExamItem::new(String::new(), q.clone()))
            .collect();
        if name.is_empty() || items.is_empty() {
            continue;
        }
        out.push(Exam { paper_style: Default::default(), margins: None,
            id: crate::model::new_id(),
            name: name.to_string(),
            header: crate::formats::DEFAULT_HEADER.to_string(),
            title: None,
            items,
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 0,
            modified: 0,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(200, 300, image::Rgba([255, 255, 255, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    fn store() {
        let dir = std::env::temp_dir().join(format!("ditto-test-{}", std::process::id()));
        unsafe { std::env::set_var("XDG_DATA_HOME", dir) };
    }

    #[test]
    fn bad_question_never_costs_the_page() {
        store();
        let t = r#"{"format":"queshion/ditto","formatVersion":"0.1.0","bank":{"name":"B","questions":[
          {"id":"q1","type":"multiple-choice","stem":"Pick","choices":[{"text":"a","correct":true},{"text":"b"}]},
          {"id":"q2","type":"mystery","stem":"?"},
          {"id":"q3","type":"fill-in-the-blank","stem":"A ___ and ___","blanks":[["x"],["y","z"]]}]}}"#;
        let r = import_text(t, None, 1, "n").unwrap();
        assert_eq!(r.doc.questions.len(), 2);
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn grouped_row_answers_are_flattened_one_per_blank() {
        store();
        let t = r#"{"format":"queshion/ditto","bank":{"questions":[{"type":"fill-in-the-blank","stem":"KI ___ ___\nSO3 ___ ___","blanks":[["Ionic","potassium iodide"],["Covalent","sulfur trioxide"]]}]}}"#;
        let r = import_text(t, None, 1, "n").unwrap();
        match &r.doc.questions[0].kind {
            Kind::Blank { accept } => assert_eq!(accept, &vec![vec!["Ionic".to_string()], vec!["potassium iodide".into()], vec!["Covalent".into()], vec!["sulfur trioxide".into()]]),
            k => panic!("{k:?}"),
        }
    }

    const LEAD: &str = "Name the following compounds. Be sure to determine whether each is ionic or covalent so that you use the correct rules.";
    const CLEAN: &str = "Formula | Ionic or Covalent? | Name\nMg(NO₂)₂ | ___ | ___\nSO₃ | ___ | ___\nNO | ___ | ___\nKI | ___ | ___\nP₂S₅ | ___ | ___";
    const KEY: [&str; 10] = ["Ionic", "magnesium nitrite", "Covalent", "sulfur trioxide", "Covalent", "nitrogen monoxide", "Ionic", "potassium iodide", "Covalent", "diphosphorus pentasulfide"];

    fn import_one(stem: &str, blanks: Value) -> Question {
        store();
        let v = serde_json::json!({"format": FORMAT, "bank": {"questions": [{"type": "fill-in-the-blank", "stem": stem, "blanks": blanks}]}});
        import_text(&v.to_string(), None, 1, "n").unwrap().doc.questions.remove(0)
    }

    fn key_per_blank() -> Vec<Vec<String>> {
        KEY.iter().map(|k| vec![k.to_string()]).collect()
    }

    #[test]
    fn ch4_every_row_repeats_the_header_label_real_model_output() {
        // Real gemma4 output (trial run, page 1): header row AND every row carry "label ___",
        // answers grouped by row and padded with empties to the 12 markers it wrote.
        let stem = format!(
            "{LEAD}\nFormula | Ionic or Covalent? ___ | Name ___\nMg(NO₂)₂ | Ionic or Covalent? ___ | Name ___\nSO₃ | Ionic or Covalent? ___ | Name ___\nNO | Ionic or Covalent? ___ | Name ___\nKI | Ionic or Covalent? ___ | Name ___\nP₂S₅ | Ionic or Covalent? ___ | Name ___"
        );
        let grouped = serde_json::json!([["Ionic", "magnesium nitrite"], ["Covalent", "sulfur trioxide"], ["Covalent", "nitrogen monoxide"], ["Ionic", "potassium iodide"], ["Covalent", "diphosphorus pentasulfide"], [], [], [], [], [], [], []]);
        let q = import_one(&stem, grouped);
        assert_eq!(q.stem, format!("{LEAD}\n{CLEAN}"));
        assert!(matches!(&q.kind, Kind::Blank { accept } if *accept == key_per_blank()), "{:?}", q.kind);
        // The same shape with one entry per ORIGINAL marker (header ones empty) lines up too.
        let mut per_marker = vec![serde_json::json!([]), serde_json::json!([])];
        per_marker.extend(KEY.iter().map(|k| serde_json::json!([k])));
        let q = import_one(&stem, Value::Array(per_marker));
        assert!(matches!(&q.kind, Kind::Blank { accept } if *accept == key_per_blank()), "{:?}", q.kind);
        // No key at all: the title-like first cell still marks the header.
        let q = import_one(&stem, serde_json::json!([]));
        assert_eq!(q.stem, format!("{LEAD}\n{CLEAN}"));
        assert!(matches!(&q.kind, Kind::Blank { accept } if accept.len() == 10));
    }

    #[test]
    fn ch4_header_row_with_blanks_over_clean_rows() {
        // Real battery output: only the header row carries the stray blanks.
        let stem = format!("{LEAD}\nFormula | Ionic or Covalent? ___ | Name ___\nMg(NO₂)₂ | ___ | ___\nSO₃ | ___ | ___\nNO | ___ | ___\nKI | ___ | ___\nP₂S₅ | ___ | ___");
        let q = import_one(&stem, Value::Array(KEY.iter().map(|k| serde_json::json!([k])).collect()));
        assert_eq!(q.stem, format!("{LEAD}\n{CLEAN}"));
        assert!(matches!(&q.kind, Kind::Blank { accept } if *accept == key_per_blank()), "{:?}", q.kind);
        // Real battery output (rotated page): each alternative written twice.
        let q = import_one(&stem, Value::Array(KEY.iter().map(|k| serde_json::json!([k, k])).collect()));
        assert!(matches!(&q.kind, Kind::Blank { accept } if *accept == key_per_blank()), "{:?}", q.kind);
    }

    #[test]
    fn ch4_rows_repeat_labels_without_a_header_row() {
        // Real battery output (5B): no header row at all; one header is synthesised.
        let stem = format!(
            "{LEAD}\nN₂O | Ionic or Covalent? ___ | Name ___\nKI | Ionic or Covalent? ___ | Name ___\nCa(NO₂)₂ | Ionic or Covalent? ___ | Name ___\nP₂S₅ | Ionic or Covalent? ___ | Name ___\nSO₂ | Ionic or Covalent? ___ | Name ___"
        );
        let key = ["Covalent", "dinitrogen monoxide", "Ionic", "potassium iodide", "Ionic", "calcium nitrite", "Covalent", "diphosphorus pentasulfide", "Covalent", "sulfur dioxide"];
        let q = import_one(&stem, Value::Array(key.iter().map(|k| serde_json::json!([k])).collect()));
        assert_eq!(q.stem, format!("{LEAD}\n|  | Ionic or Covalent? | Name |\nN₂O | ___ | ___\nKI | ___ | ___\nCa(NO₂)₂ | ___ | ___\nP₂S₅ | ___ | ___\nSO₂ | ___ | ___"));
        assert!(matches!(&q.kind, Kind::Blank { accept } if accept.len() == 10 && accept[1] == vec!["dinitrogen monoxide".to_string()]));
        match &crate::richtext::parse(&q.stem)[1] {
            crate::richtext::Block::Table { header, rows } => {
                assert!(*header);
                assert_eq!(rows.len(), 6);
                assert!(rows.iter().all(|r| r.len() == 3));
            }
            b => panic!("{b:?}"),
        }
    }

    #[test]
    fn clean_tables_and_plain_stems_are_left_alone() {
        let stem = format!("{LEAD}\n{CLEAN}");
        assert_eq!(normalize_tables(&stem, &[10]), TableFix { stem: stem.clone(), dropped: vec![] });
        let q = import_one(&stem, Value::Array(KEY.iter().map(|k| serde_json::json!([k])).collect()));
        assert_eq!(q.stem, stem);
        for s in ["A ___ and ___", "x | y", "Bonds | Lone pairs\n___ | ___", "a | b\n1 | 2", "• Draw it.\n• Name it."] {
            assert_eq!(normalize_tables(s, &[]).stem, s, "{s}");
        }
        // Other types get the same cleaning, without answers to align.
        store();
        let v = serde_json::json!({"format": FORMAT, "bank": {"questions": [{"type": "short-answer", "stem": "Fill in:\nKI | Name ___\nNO | Name ___"}]}});
        let q = import_text(&v.to_string(), None, 1, "n").unwrap().doc.questions.remove(0);
        assert_eq!(q.stem, "Fill in:\n|  | Name |\nKI | ___\nNO | ___");
    }

    #[test]
    fn blank_without_markers_becomes_short() {
        store();
        let t = r#"{"format":"queshion/ditto","bank":{"questions":[{"type":"fill-in-the-blank","stem":"Bonding: 14","blanks":[["14"],["10"]]}]}}"#;
        let r = import_text(t, None, 1, "n").unwrap();
        assert!(matches!(&r.doc.questions[0].kind, Kind::Short { sample } if sample == "14; 10"));
    }

    #[test]
    fn figure_box_is_cropped_and_tolerates_fences() {
        store();
        let t = "```json\n{\"format\":\"queshion/ditto\",\"bank\":{\"questions\":[{\"type\":\"short-answer\",\"stem\":\"See\",\"figure\":{\"box\":[0.2,0.2,0.6,0.5]}},{\"type\":\"short-answer\",\"stem\":\"Bad\",\"figure\":{\"box\":[0,0,1,1]}}]}}\n```";
        let r = import_text(t, Some(&page()), 1, "n").unwrap();
        assert_eq!(r.figures, 1);
        assert!(r.doc.questions[0].image.is_some() && r.doc.questions[1].image.is_none());
        assert_eq!(r.warnings.len(), 1);
    }
}
