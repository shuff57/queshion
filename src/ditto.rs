//! queShion Ditto package (`.ditto.json`, see `docs/ditto-format.md`): what the AI conversion
//! prompt (`prompts/ditto-convert.md`) produces. Import is tolerant: each question is checked
//! alone, a bad one is skipped with a warning, and figure boxes are cropped out of the page
//! image the model was shown.
use crate::formats::ImportDoc;
use crate::model::{Answer, Difficulty, Exam, ExamItem, Kind, Pair, Part, PartKind, Question};
use serde_json::Value;
use std::collections::HashMap;

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
        return Err(format!("Not a queShion Ditto file (format should be \"{FORMAT}\")."));
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
    let markers = stem.matches("___").count();
    if markers == 0 {
        let sample = accept.iter().map(|a| a.join(" OR ")).collect::<Vec<_>>().join("; ");
        return (stem, Kind::Short { sample });
    }
    // Repair: the model grouped each row's answers into one entry ([[a, b], [c, d]] for four
    // blanks). When flattening them gives exactly one answer per blank, that was the intent.
    if accept.len() < markers && accept.iter().map(Vec::len).sum::<usize>() == markers {
        accept = accept.into_iter().flatten().map(|a| vec![a]).collect();
    }
    accept.resize(markers, Vec::new());
    (stem, Kind::Blank { accept })
}

fn part(p: &Value) -> Option<Part> {
    let stem = text(p, "stem").to_string();
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
            .map(|q| ExamItem { bank_id: String::new(), question_id: q.clone() })
            .collect();
        if name.is_empty() || items.is_empty() {
            continue;
        }
        out.push(Exam {
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
