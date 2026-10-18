//! Print-layout model the exporters render (signatures frozen in NOTES.md).
//!
//! The exam worker maps `model::Exam` + resolved bank questions into an [`ExamDoc`]
//! (shuffling, version generation and the answer-key doc are built there) and calls
//! `crate::export::render_pdf` / `crate::export::render_docx`, which are pure.
//!
//! # Conventions (they decide the printed layout — `DocQ` carries no kind)
//!
//! * Ordinary questions (MC/MA, Short, Blank, Multipart): `text[0]` is the stem — the
//!   exporters wrap it. Every `text` entry may carry stem markup (`docs/stem-format.md`)
//!   and is parsed with `crate::richtext::parse`: `\n` hard breaks, blank-line paragraphs,
//!   bullet / numbered lists, quotes, ` | ` tables, `**bold**`, `*italic*`, `_{sub}`,
//!   `^{sup}`; `$math$` is degraded to Unicode (`equation::stem_to_plain`). A stem
//!   without markup prints exactly as before. Extra `text` entries print as further blocks
//!   (put explicit breaks as `\n` inside an entry, not as extra entries, or a stem will
//!   be mistaken for a matching left-item list). `choices` are pre-lettered ("A. Venus"),
//!   honour inline markup, and print each on its own line under the stem.
//! * True/False: `choices` is exactly `["True", "False"]` (any case).
//! * Matching: EVERY `text` entry is a left-column item — no stem/instructions line —
//!   printed with continuing numbers `number`, `number+1`, ...; `choices` is the
//!   shuffled, pre-lettered right column ("A. ...", at least two of them).
//! * Short answer / essays: leave `choices` empty and set `rule_lines` (3 is the
//!   default when it is 0 and the stem has no `___` marker). Questions whose stem
//!   contains `___` with no choices fill-in blanks: they get one ruled line after the
//!   stem when `rule_lines` is 0 — unless every blank sits in a table cell (then the
//!   cell, printed empty and roomy, is the answer space and no line is added).
//! * `key_text` may carry the same markup (lines, inline styles); the key prints it.

/// A raster image resolved (with `store::media_path`) by the exam worker; the exporters
/// re-encode to JPEG for the PDF and embed it as an inline drawing in the DOCX.
#[derive(Clone, Debug)]
pub struct DocImage {
    pub bytes: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Debug, Default)]
pub struct DocQ {
    /// Printed question number. Matching: the number of the FIRST left item; later
    /// items continue `number+1`, `number+2`, ...
    pub number: u32,
    /// Stem entries (may carry stem markup, parsed with `richtext`) — see the module-level
    /// conventions.
    pub text: Vec<String>,
    /// Pre-lettered choices ("A. Venus"); `["True", "False"]` for TF.
    pub choices: Vec<String>,
    pub image: Option<DocImage>,
    /// Answer-key text printed on the answer-key document ("B", "B, D", a worked
    /// answer, or the teacher's explanation).
    pub key_text: String,
    /// Ruled answer lines after the stem.
    pub rule_lines: u8,
    pub points: Option<f64>,
    /// A section starts at this question: its heading prints right before it.
    pub section: Option<DocSection>,
    /// Extra work space under the question in printed lines (blank room, or ruled /
    /// dotted lines in the Classic / Exam Board styles).
    pub work_lines: u8,
    /// Multipart questions with per-part work space: printed lines of work space after
    /// `text` entry k (the parts are then split into one entry each, entry 0 holding the
    /// stem and part a). Empty (the default) for every other question.
    pub space_after: Vec<u8>,
    /// Multiple choice: columns the choices print across in (Test Parrot's 1 / 2 / 4 switch,
    /// `model::ExamItem::answer_columns`). 0 or 1 (the default) = the paper style's own
    /// layout (one per line; Condensed's automatic across-layout). 2..=4 = that many equal
    /// columns, filled column-major ([`choice_grid`]), each choice wrapping inside its column.
    pub answer_columns: u8,
    /// Indices of the correct `choices` (multiple choice / true-false), for the marked answer
    /// key ([`KeyLayout::Marked`]). Empty = none known.
    pub correct: Vec<usize>,
    /// The short answers an answer section writes on its blanks, one per printed number:
    /// `["C"]`, `["True"]`, matching `["B", "A", ...]` (one per left item). Empty for written
    /// answers (the key then prints `key_text` under an empty blank).
    pub key_marks: Vec<String>,
    /// No answer room at all: no default ruled lines (short answer, fill-in), no work space.
    /// The bank-file PDF sets it; exams leave it false.
    pub no_answer_space: bool,
}

/// How the answer-key document lays out (the export dialog's choice).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyLayout {
    /// "N. answer" lines (the original key; also what a snapshot without the field means).
    #[default]
    List,
    /// Test Parrot's printed key: the title, "Answer Section", section headings, and per
    /// question its number and a blank with the answer written on it (letters, True/False,
    /// matching letters); written answers print under an empty blank.
    Section,
    /// The whole test with the correct choices marked (check mark, bold, underlined) and the
    /// expected answers printed in place of the answer lines.
    Marked,
}

impl KeyLayout {
    pub const ALL: [KeyLayout; 3] = [KeyLayout::Section, KeyLayout::Marked, KeyLayout::List];
    pub fn label(self) -> &'static str {
        match self {
            KeyLayout::Section => "Answer section",
            KeyLayout::Marked => "Marked test",
            KeyLayout::List => "Compact list",
        }
    }
}

/// Printed size (pt) of a picture `w_px` x `h_px` in a column `avail_w` pt wide: its natural
/// size at 96 dpi (0.75 pt per pixel), scaled down to the column width and to at most
/// [`PICTURE_MAX_H_PT`] tall, never enlarged. The exam page preview draws the same box (times
/// its pt-to-px factor) and the PDF prints it at the stem's left edge, so the two agree.
pub fn picture_size_pt(w_px: u32, h_px: u32, avail_w: f64) -> (f64, f64) {
    let (w, h) = (w_px.max(1) as f64, h_px.max(1) as f64);
    let s = 0.75_f64.min(avail_w.max(36.0) / w).min(PICTURE_MAX_H_PT / h);
    (w * s, h * s)
}

/// Tallest a question's picture prints (3 inches).
pub const PICTURE_MAX_H_PT: f64 = 216.0;

/// Splits a stem at its first `[picture]` / `[picture: ...]` marker: (text before, text after,
/// with any later markers kept as written). None when the stem has no marker. The attached
/// picture prints at that spot (Test Parrot puts it where the marker is).
pub fn split_at_picture(stem: &str) -> Option<(String, String)> {
    use crate::bankops::StemPart;
    let parts = crate::bankops::split_pictures(stem);
    let first = parts.iter().position(|p| matches!(p, StemPart::Picture(_)))?;
    let text = |p: &StemPart| match p {
        StemPart::Text(t) => t.clone(),
        StemPart::Picture(None) => "[picture]".to_string(),
        StemPart::Picture(Some(a)) => format!("[picture: {a}]"),
    };
    let before: Vec<String> = parts[..first].iter().map(text).collect();
    let after: Vec<String> = parts[first + 1..].iter().map(text).collect();
    Some((before.join("\n"), after.join("\n")))
}

/// Rows of choice indices for `n` choices in `cols` columns, filled column-major as Test
/// Parrot does (4 choices in 2 columns: rows `[0, 2]`, `[1, 3]`, i.e. A C / B D). `cols` 0 or
/// 1 gives one choice per row. A short last column leaves the trailing rows short.
pub fn choice_grid(n: usize, cols: u8) -> Vec<Vec<usize>> {
    let cols = (cols.max(1) as usize).min(n.max(1));
    let rows = n.div_ceil(cols);
    (0..rows).map(|r| (0..cols).map(|c| c * rows + r).filter(|&i| i < n).collect()).collect()
}

/// A section heading (title and directions; either may be empty).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocSection {
    pub title: String,
    pub directions: String,
}

impl DocSection {
    pub fn is_blank_doc(&self) -> bool {
        self.title.trim().is_empty() && self.directions.trim().is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExamDoc {
    pub title: String,
    /// The printed header, usually "Name: ____ Class: ____ Date: ____".
    pub header_line: String,
    /// Heading size in points for the printed title.
    pub heading_pt: f32,
    /// Body text size in points.
    pub body_pt: f32,
    /// Page margins in mm (all four).
    pub margin_mm: f32,
    /// true = choices print inline under each stem (the normal exam look). Both
    /// exporters currently treat false the same as true: a sectioned answers sheet
    /// would need per-choice correctness, which `DocQ` does not carry.
    pub answers_inline: bool,
    pub questions: Vec<DocQ>,
    /// Extra ruled blank lines after the title/header area.
    pub ruled_blanks: u8,
    /// true on the ANSWER-KEY document: renderers print `N. key_text` per question and
    /// skip stems, choices, rules and images. Default false.
    pub is_key: bool,
    /// Added by the exam worker (authorized by the orchestrator): "Version B" etc.
    /// for a shuffled named version. Renderers may print it as a small muted line
    /// under the header line; `None` (the default) means plain single-copy output.
    pub version_label: Option<String>,
    /// Format > Paper style (numbering, blanks, rules, points placement, spacing, page size).
    pub style: crate::model::PaperStyle,
    /// Per-side margins in mm; None = `margin_mm` on all four sides.
    pub margins: Option<crate::model::Margins>,
    /// Answer-key documents only (`is_key`): the key's layout. Default `List`.
    pub key_layout: KeyLayout,
}

impl ExamDoc {
    /// Effective margins in mm (top, right, bottom, left).
    pub fn margins_mm(&self) -> crate::model::Margins {
        self.margins.unwrap_or_else(|| crate::model::Margins::all(self.margin_mm))
    }
}