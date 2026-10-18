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
    /// Optional sharing details (Test Parrot's "Question Bank details"). Older stores
    /// have none; only what the teacher enters travels with an exported bank.
    #[serde(default, skip_serializing_if = "BankDetails::is_empty")]
    pub details: BankDetails,
}

/// A bank's description, declared author and license (all optional free text).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BankDetails {
    #[serde(default, skip_serializing_if = "String::is_empty")] pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub license_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub license_url: String,
}

impl BankDetails {
    pub fn is_empty(&self) -> bool {
        [&self.description, &self.author, &self.license_name, &self.license_url].iter().all(|s| s.trim().is_empty())
    }
    /// The same details with surrounding whitespace removed (what gets saved).
    pub fn trimmed(&self) -> BankDetails {
        BankDetails {
            description: self.description.trim().to_string(),
            author: self.author.trim().to_string(),
            license_name: self.license_name.trim().to_string(),
            license_url: self.license_url.trim().to_string(),
        }
    }
}

/// One question placed on the exam working copy. Points are the bank question's own.
///
/// Sections are positional: an item whose `section` is set starts a new section (its
/// heading prints right before the question); every following item belongs to it until
/// the next item that starts one. Item 0 may carry the first section's heading.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExamItem {
    pub bank_id: Id,
    pub question_id: Id,
    /// A new section starts at this item (title + directions printed before it).
    #[serde(default, skip_serializing_if = "Option::is_none")] pub section: Option<ExamSection>,
    /// Extra work space under the question, in printed lines (0..=23, Test Parrot's range).
    #[serde(default, skip_serializing_if = "is_zero_u8")] pub work_lines: u8,
    /// This exam's own answer order for a multiple-choice question: indices into the bank
    /// question's answers. Empty (or stale after a bank edit) = the bank's order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub answer_order: Vec<usize>,
}

fn is_zero_u8(v: &u8) -> bool { *v == 0 }

impl ExamItem {
    pub fn new(bank_id: impl Into<Id>, question_id: impl Into<Id>) -> ExamItem {
        ExamItem { bank_id: bank_id.into(), question_id: question_id.into(), section: None, work_lines: 0, answer_order: Vec::new() }
    }
}

/// A section heading on the exam: printed title and directions (either may be empty).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExamSection {
    #[serde(default)] pub title: String,
    #[serde(default)] pub directions: String,
}

impl ExamSection {
    pub fn is_blank(&self) -> bool { self.title.trim().is_empty() && self.directions.trim().is_empty() }
}

/// Printed look of the exam (Test Parrot's Format > Paper style). See docs/exam-styles.md.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperStyle {
    /// "1." numbers, "A." choices, points in parentheses, ruled lines for written answers.
    #[default]
    Standard,
    /// A blank before each objective number, lowercase "a." choices, ruled work space.
    Classic,
    /// Tighter spacing, choices across where they fit, closer-ruled lines.
    Condensed,
    /// A4, "1" / "(a)" labels, dotted lines, points in brackets at the right, "Turn over".
    ExamBoard,
}

impl PaperStyle {
    pub const ALL: [PaperStyle; 4] = [PaperStyle::Standard, PaperStyle::Classic, PaperStyle::Condensed, PaperStyle::ExamBoard];
    pub fn label(self) -> &'static str {
        match self {
            PaperStyle::Standard => "Standard",
            PaperStyle::Classic => "Classic",
            PaperStyle::Condensed => "Condensed",
            PaperStyle::ExamBoard => "Exam Board",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            PaperStyle::Standard => "Circle the letter or T/F; no blanks before numbers.",
            PaperStyle::Classic => "A blank before each objective number; ruled lines for written answers.",
            PaperStyle::Condensed => "Saves paper: tighter spacing, answers across where they fit, closer-ruled lines.",
            PaperStyle::ExamBoard => "A4: 1 (a) (i) labels, dotted lines, points in brackets at the right.",
        }
    }
    /// Page size in PDF points (Letter, or A4 for Exam Board).
    pub fn page_pt(self) -> (f32, f32) {
        match self {
            PaperStyle::ExamBoard => (595.28, 841.89),
            _ => (612.0, 792.0),
        }
    }
    /// Printed question number label: "3." (or "3" on Exam Board).
    pub fn number_label(self, n: u32) -> String {
        match self {
            PaperStyle::ExamBoard => n.to_string(),
            _ => format!("{n}."),
        }
    }
    /// A multipart part label for part index `pi`: "a." (or "(a)" on Exam Board).
    pub fn part_label(self, pi: usize) -> String {
        let l = char::from(b'a' + (pi % 26) as u8);
        match self {
            PaperStyle::ExamBoard => format!("({l})"),
            _ => format!("{l}."),
        }
    }
    /// Restyles a pre-lettered choice ("A. Venus"): "a. Venus" (Classic), "A Venus" (Exam Board).
    pub fn choice_label(self, choice: &str) -> String {
        let b = choice.as_bytes();
        let lettered = b.len() >= 2 && b[0].is_ascii_uppercase() && b[1] == b'.';
        if !lettered {
            return choice.to_string();
        }
        match self {
            PaperStyle::Classic => format!("{}{}", (b[0] as char).to_ascii_lowercase(), &choice[1..]),
            PaperStyle::ExamBoard => format!("{}{}", b[0] as char, &choice[2..]),
            _ => choice.to_string(),
        }
    }
    /// Points label: "(2 points)" or "[2]" (Exam Board).
    pub fn points_label(self, p: f64) -> String {
        let n = if p.fract() == 0.0 { format!("{}", p as i64) } else { format!("{p}") };
        match self {
            PaperStyle::ExamBoard => format!("[{n}]"),
            _ if p == 1.0 => "(1 point)".to_string(),
            _ => format!("({n} points)"),
        }
    }
    /// Line leading as a multiple of the body size.
    pub fn lead(self) -> f32 {
        match self { PaperStyle::Condensed => 1.25, _ => 1.42 }
    }
    /// Height of one ruled / work-space line as a multiple of the body size.
    pub fn rule_pitch(self) -> f32 {
        match self { PaperStyle::Condensed => 1.55, _ => 2.0 }
    }
    /// Gap after each question as a multiple of the body size.
    pub fn q_gap(self) -> f32 {
        match self { PaperStyle::Condensed => 0.3, _ => 0.6 }
    }
    /// Work space prints as lines (Classic ruled, Exam Board dotted) instead of blank room.
    pub fn work_space_lined(self) -> bool {
        matches!(self, PaperStyle::Classic | PaperStyle::ExamBoard)
    }
    /// Answer lines are dotted (Exam Board).
    pub fn dotted(self) -> bool { self == PaperStyle::ExamBoard }
    /// A "______" answer blank before objective (MC/TF/matching) question numbers.
    pub fn objective_blank(self) -> bool { self == PaperStyle::Classic }
    /// MC choices print across in columns when they fit.
    pub fn choices_across(self) -> bool { self == PaperStyle::Condensed }
}

/// Page margins in mm, per side.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Margins { pub top: f32, pub right: f32, pub bottom: f32, pub left: f32 }

impl Margins {
    pub fn all(mm: f32) -> Margins { Margins { top: mm, right: mm, bottom: mm, left: mm } }
    pub fn uniform(&self) -> bool { self.top == self.right && self.right == self.bottom && self.bottom == self.left }
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
    /// Format > Paper style.
    #[serde(default)] pub paper_style: PaperStyle,
    /// Per-side margins (Margins dialog "Set each side"); None = `margin_mm` on all sides.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub margins: Option<Margins>,
}

impl Exam {
    /// The effective page margins in mm.
    pub fn page_margins(&self) -> Margins {
        self.margins.unwrap_or_else(|| Margins::all(self.margin_mm))
    }
}

impl Default for Exam {
    fn default() -> Exam {
        Exam {
            id: String::new(),
            name: String::new(),
            header: "Name: __________________ Class: ___________ Date: ___________".to_string(),
            title: None,
            items: Vec::new(),
            heading_pt: def_heading_pt(),
            body_pt: def_body_pt(),
            margin_mm: def_margin_mm(),
            ruled_blanks: 0,
            modified: 0,
            paper_style: PaperStyle::Standard,
            margins: None,
        }
    }
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_details_default_for_old_stores_and_stay_out_when_empty() {
        let old = r#"{"id":"b1","name":"Old","questions":[],"created":1,"modified":2}"#;
        let b: QuestionBank = serde_json::from_str(old).expect("pre-details bank loads");
        assert!(b.details.is_empty());
        let out = serde_json::to_string(&b).unwrap();
        assert!(!out.contains("details"), "empty details are not written: {out}");
        let mut b2 = b.clone();
        b2.details.author = "A. Teacher".into();
        let back: QuestionBank = serde_json::from_str(&serde_json::to_string(&b2).unwrap()).unwrap();
        assert_eq!(back.details.author, "A. Teacher");
        assert_eq!(back.details.description, "");
    }

    #[test]
    fn old_exams_load_with_defaults() {
        let old = r#"{"id":"e","name":"Quiz","header":"Name:","title":null,"items":[{"bank_id":"b","question_id":"q"}]}"#;
        let e: Exam = serde_json::from_str(old).unwrap();
        assert_eq!(e.paper_style, PaperStyle::Standard);
        assert!(e.margins.is_none());
        assert_eq!(e.page_margins(), Margins::all(18.0));
        let it = &e.items[0];
        assert!(it.section.is_none() && it.work_lines == 0 && it.answer_order.is_empty());
        // Default fields stay out of the file, so the JSON of an untouched exam is unchanged.
        let json = serde_json::to_string(&e.items[0]).unwrap();
        assert_eq!(json, r#"{"bank_id":"b","question_id":"q"}"#);
    }

    #[test]
    fn new_fields_round_trip() {
        let mut e = Exam { id: "e".into(), name: "n".into(), ..Default::default() };
        e.paper_style = PaperStyle::ExamBoard;
        e.margins = Some(Margins { top: 10.0, right: 20.0, bottom: 30.0, left: 40.0 });
        let mut it = ExamItem::new("b", "q");
        it.section = Some(ExamSection { title: "Part I".into(), directions: "Circle one.".into() });
        it.work_lines = 7;
        it.answer_order = vec![2, 0, 1];
        e.items.push(it);
        let back: Exam = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back.paper_style, PaperStyle::ExamBoard);
        assert_eq!(back.page_margins().left, 40.0);
        assert_eq!(back.items[0].section.as_ref().unwrap().title, "Part I");
        assert_eq!(back.items[0].work_lines, 7);
        assert_eq!(back.items[0].answer_order, vec![2, 0, 1]);
    }

    #[test]
    fn paper_style_labels() {
        use PaperStyle::*;
        assert_eq!(Standard.number_label(3), "3.");
        assert_eq!(ExamBoard.number_label(3), "3");
        assert_eq!(Standard.choice_label("B. Venus"), "B. Venus");
        assert_eq!(Classic.choice_label("B. Venus"), "b. Venus");
        assert_eq!(ExamBoard.choice_label("B. Venus"), "B Venus");
        assert_eq!(Classic.choice_label("True"), "True");
        assert_eq!(Standard.points_label(1.0), "(1 point)");
        assert_eq!(Condensed.points_label(2.5), "(2.5 points)");
        assert_eq!(ExamBoard.points_label(4.0), "[4]");
        assert_eq!(ExamBoard.part_label(0), "(a)");
        assert_eq!(Standard.part_label(1), "b.");
        assert_eq!(ExamBoard.page_pt(), (595.28, 841.89));
        assert!(Condensed.lead() < Standard.lead() && Condensed.rule_pitch() < Standard.rule_pitch());
        assert!(Margins::all(5.0).uniform() && !Margins { top: 1.0, right: 1.0, bottom: 1.0, left: 2.0 }.uniform());
    }
}
