//! Shared data contract. Question banks own questions; exams reference bank questions
//! into an ordered working copy; imports and exports are records. Entirely local.
use serde::{Deserialize, Serialize};
pub type Id = String;

/// URL-safe base62-ish id: 4 hex of a per-process counter + 7 hex of unix millis.
pub fn new_id() -> Id {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    format!("{:03x}{:07x}", n, t)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty { Low, Medium, High }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageRef {
    /// Path relative to the store dir, e.g. "media/<sha256>.png".
    pub file: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QKind { Mc, Tf, Matching, Blank, Short, Multipart }

impl QKind {
    pub fn label(self) -> &'static str {
        match self {
            QKind::Mc => "Multiple choice",
            QKind::Tf => "True/False",
            QKind::Matching => "Matching",
            QKind::Blank => "Fill in the blank",
            QKind::Short => "Short answer",
            QKind::Multipart => "Multipart",
        }
    }
}

/// One answer choice. `locked` means exporting/shuffling must not move it (Test Parrot's
/// "Lock answer position" button). `correct` marks the right answer(s).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Answer {
    pub text: String,
    #[serde(default)] pub correct: bool,
    #[serde(default)] pub locked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pair {
    pub left: String,
    pub right: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Part {
    pub stem: String,
    pub kind: PartKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PartKind {
    Mc { answers: Vec<Answer> },
    Tf { correct: bool },
    Short { sample: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Kind {
    /// Multiple choice / multiple-answer (several `correct` = MA).
    Mc { answers: Vec<Answer> },
    /// True/False; correct holds the right one.
    Tf { correct: bool },
    /// Matching: left column items paired with right column; distractors are extra right items.
    Matching { pairs: Vec<Pair>, distractors: Vec<String> },
    /// Fill in the blank; stem contains "___" markers, answers in order.
    Blank { accept: Vec<Vec<String>> },
    /// Short answer with a sample answer(s).
    Short { sample: String },
    /// Multipart with independent parts.
    Multipart { parts: Vec<Part> },
}

impl Kind {
    pub fn qkind(&self) -> QKind { match self { Kind::Mc {..} => QKind::Mc, Kind::Tf {..} => QKind::Tf, Kind::Matching {..} => QKind::Matching, Kind::Blank {..} => QKind::Blank, Kind::Short {..} => QKind::Short, Kind::Multipart {..} => QKind::Multipart } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Question {
    pub id: Id,
    pub stem: String,
    pub kind: Kind,
    pub points: Option<f64>,
    pub difficulty: Option<Difficulty>,
    #[serde(default)] pub topics: Vec<String>,
    #[serde(default)] pub image: Option<ImageRef>,
    /// Teacher's explanation, shown on the answer key.
    #[serde(default)] pub explanation: Option<String>,
    /// Figure-engine origin of `image` (id + params) so the figure can be re-opened and edited.
    #[serde(default)] pub figure: Option<FigureSpec>,
    pub created: i64,
    pub modified: i64,
}

/// Which generated figure (figures registry id) and parameter values produced a question's image.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FigureSpec {
    pub id: String,
    #[serde(default)] pub params: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestionBank {
    pub id: Id,
    pub name: String,
    pub questions: Vec<Question>,
    pub created: i64,
    pub modified: i64,
}

/// One question placed on the exam working copy. Points are the bank question's own.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExamItem {
    pub bank_id: Id,
    pub question_id: Id,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Exam {
    pub id: Id,
    pub name: String,
    /// The printed header, default "Name: __________________ Class: ___________ Date: ___________".
    pub header: String,
    /// Printed title; when None the exam name prints.
    pub title: Option<String>,
    pub items: Vec<ExamItem>,
    /// Heading size in points for the printed title (10-28).
    #[serde(default = "def_heading_pt")] pub heading_pt: f32,
    /// Body text size in points (8-14).
    #[serde(default = "def_body_pt")] pub body_pt: f32,
    /// Page margins in mm (all four).
    #[serde(default = "def_margin_mm")] pub margin_mm: f32,
    /// Extra ruled blank lines after the exam title area.
    #[serde(default)] pub ruled_blanks: u8,
    /// Last-edit timestamp, added by the exam-UI worker (NOTES.md permits new
    /// fields with #[serde(default)]); backs the exam list's "modified" column.
    #[serde(default)] pub modified: i64,
}

fn def_heading_pt() -> f32 { 16.0 }
fn def_body_pt() -> f32 { 11.0 }
fn def_margin_mm() -> f32 { 18.0 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Import {
    pub id: Id,
    pub time: i64,
    /// Original file/paste label, e.g. "unit-3-test.pdf".
    pub source: String,
    /// How it was converted: "file", "paste", "ai".
    pub how: String,
    /// Number of questions parsed.
    pub count: usize,
    /// Bank (or banks) the questions landed in.
    pub bank_ids: Vec<Id>,
    /// Any note, e.g. errors, model name for AI imports.
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportRecord {
    pub id: Id,
    pub time: i64,
    pub exam_id: Id,
    pub exam_name: String,
    pub format: String, // "pdf" | "docx"
    pub what: Vec<String>, // ["student test", "answer key"]
    pub versions: u32,
    /// Absolute or relative paths of produced files, in order: [student test v1, v2..., key].
    pub files: Vec<String>,
}