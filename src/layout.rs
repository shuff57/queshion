//! Print-layout model the exporters render (signatures frozen in NOTES.md).
//!
//! The exam worker maps `model::Exam` + resolved bank questions into an [`ExamDoc`]
//! (shuffling, version generation and the answer-key doc are built there) and calls
//! `crate::export::render_pdf` / `crate::export::render_docx`, which are pure.
//!
//! # Conventions (they decide the printed layout — `DocQ` carries no kind)
//!
//! * Ordinary questions (MC/MA, Short, Blank, Multipart): `text[0]` is the stem on ONE
//!   logical line — the exporters wrap it. Extra `text` entries print as further plain
//!   lines (put explicit breaks as `\n` inside an entry, not as extra entries, or a
//!   stem will be mistaken for a matching left-item list). `choices` are pre-lettered
//!   ("A. Venus") and print each on its own line under the stem.
//! * True/False: `choices` is exactly `["True", "False"]` (any case).
//! * Matching: EVERY `text` entry is a left-column item — no stem/instructions line —
//!   printed with continuing numbers `number`, `number+1`, ...; `choices` is the
//!   shuffled, pre-lettered right column ("A. ...", at least two of them).
//! * Short answer / essays: leave `choices` empty and set `rule_lines` (3 is the
//!   default when it is 0 and the stem has no `___` marker). Questions whose stem
//!   contains `___` with no choices fill-in blanks: they get one ruled line after the
//!   stem when `rule_lines` is 0.

/// A raster image resolved (with `store::media_path`) by the exam worker; the exporters
/// re-encode to JPEG for the PDF and print `[picture]` in the DOCX.
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
    /// Stem lines — see the module-level conventions.
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
}