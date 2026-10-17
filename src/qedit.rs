//! Question editor (V4): the `Draft` state the bank screen opens, validation, and the
//! in-place two-column editor that a question card expands into (left: stem + answers
//! with tappable correct-circles; right: type, points, difficulty, topics, explanation,
//! image). Supports all six kinds (MC/MA, True/False, Matching, Fill in the blank,
//! Short answer, Multipart). `bankui` owns the card/grid; this module owns the body.

use std::collections::HashMap;

use egui::{
    Align, Button, Color32, CornerRadius, Frame, Key, Layout, Margin, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};
// egui's widget id type vs the store's question id (a String).
use egui::Id as UiId;

use crate::model::{
    Answer, Difficulty, Id, ImageRef, Kind, Pair, Part, PartKind, QKind, Question,
};
use crate::{kit, screens, theme, App};

/// Type selector order and the short tags the V4 board uses.
const KINDS: [(QKind, &str); 6] = [
    (QKind::Mc, "MC"),
    (QKind::Tf, "TF"),
    (QKind::Multipart, "MULTI"),
    (QKind::Matching, "MATCH"),
    (QKind::Blank, "FILL"),
    (QKind::Short, "SHORT"),
];

/// Short tag for a question type ("MC", "TF", "MULTI", "MATCH", "FILL", "SHORT").
pub fn type_tag(k: QKind) -> &'static str {
    KINDS.iter().find(|(q, _)| *q == k).map(|(_, t)| *t).unwrap_or("?")
}

fn empty_answer() -> Answer {
    Answer {
        text: String::new(),
        correct: false,
        locked: false,
    }
}

fn empty_answers(n: usize) -> Vec<Answer> {
    (0..n).map(|_| empty_answer()).collect()
}

/// Number of "___" blanks in a Fill-in-the-blank stem.
fn blank_count(stem: &str) -> usize {
    stem.matches("___").count()
}

/// Splits one editor row ("red OR dark red") into accepted alternatives.
fn split_alts(s: &str) -> Vec<String> {
    s.split(" OR ")
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect()
}

/// Small red label.
fn red(ui: &mut Ui, msg: &str) {
    ui.label(RichText::new(msg).color(theme::bad()).size(12.0));
}

/// Small ghost pill used inside the editor ("+ Add answer", "Attach image...").
fn ghost_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).family(theme::bold_family()).size(12.5).color(theme::accent_col()))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(255))
            .min_size(Vec2::new(0.0, 28.0)),
    )
}

/// What the editor body reports each frame.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum QAction {
    None,
    /// Done pressed (draft validated; caller commits it).
    Done,
    /// Cancel pressed (caller drops the draft).
    Cancel,
    /// Delete pressed (caller removes the stored question, if any).
    Delete,
}

/// Context memory for one editing session.
#[derive(Clone, Default)]
struct Temp {
    /// Raw text of each "accepted answers" row, alternatives joined with " OR ".
    /// The frozen `Kind::Blank` shape can't hold raw text, so this mirrors it and
    /// is synced into `Kind::Blank` each frame the Blank editor draws/edits.
    blank_texts: Option<Vec<String>>,
    /// Draft text of the "add topic" field.
    topics_text: String,
    /// The open equation request (set by a sigma button, the word-initial '/' key or a click on
    /// a rendered equation chip); the equation modal reads and clears it.
    eq_req: Option<EqReq>,
}

/// Which text field an equation is being spliced into.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum EqField {
    Stem,
    Mc(usize),
    MatchL(usize),
    MatchR(usize),
    Distractor(usize),
    Blank(usize),
    Short,
    PartStem(usize),
    PartMc(usize, usize),
    PartShort(usize),
}

/// An equation edit in progress: where it goes and what it starts as.
#[derive(Clone, Debug)]
struct EqReq {
    field: EqField,
    /// Char index to insert at (new equation).
    caret: usize,
    /// Char range of the `$..$` being replaced (editing an existing one).
    span: Option<std::ops::Range<usize>>,
    /// Linear content to prefill.
    initial: String,
}

/// Context-memory key for the per-draft editor state.
fn temp_key(bank_id: &str, qid: Option<&Id>) -> UiId {
    UiId::new("testparrot_qedit_temp")
        .with(bank_id)
        .with(qid.map_or("", |s| s.as_str()))
}

/// Everything a field needs to ask for the equation modal.
#[derive(Clone)]
struct EqCtx {
    ctx: egui::Context,
    key: UiId,
}

impl EqCtx {
    fn request(&self, field: EqField, caret: usize, span: Option<std::ops::Range<usize>>, initial: String) {
        self.ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Temp>(self.key).eq_req = Some(EqReq { field, caret, span, initial });
        });
    }
}

// ---------- the draft ----------


#[derive(Clone, Debug)]
pub struct Draft {
    pub bank_id: Id,
    pub qid: Option<Id>,
    pub stem: String,
    pub kind: Kind,
    pub points: Option<f64>,
    pub difficulty: Option<Difficulty>,
    pub topics: Vec<String>,
    pub image: Option<ImageRef>,
    pub explanation: Option<String>,
    /// Figure-engine origin of `image` (None for attached/imported pictures).
    pub figure: Option<crate::model::FigureSpec>,
}

impl Draft {
    /// A new unsaved question: empty stem, multiple choice with four empty choices.
    pub fn new(bank_id: Id) -> Draft {
        Draft {
            bank_id,
            qid: None,
            stem: String::new(),
            kind: Kind::Mc {
                answers: empty_answers(4),
            },
            points: None,
            difficulty: None,
            topics: Vec::new(),
            image: None,
            explanation: None,
            figure: None,
        }
    }

    /// A deep copy of a stored question for opening in the editor.
    pub fn from_question(bank_id: Id, q: &Question) -> Draft {
        Draft {
            bank_id,
            qid: Some(q.id.clone()),
            stem: q.stem.clone(),
            kind: q.kind.clone(),
            points: q.points,
            difficulty: q.difficulty,
            topics: q.topics.clone(),
            image: q.image.clone(),
            explanation: q.explanation.clone(),
            figure: q.figure.clone(),
        }
    }

    pub fn is_edit(&self) -> bool {
        self.qid.is_some()
    }

    /// The kind as one of the six combobox values.
    pub fn qkind(&self) -> QKind {
        self.kind.qkind()
    }

    /// Switches question type; the old kind's data is replaced with sane defaults.
    pub fn set_kind(&mut self, k: QKind) {
        if self.kind.qkind() == k {
            return;
        }
        self.kind = match k {
            QKind::Mc => Kind::Mc {
                answers: empty_answers(4),
            },
            QKind::Tf => Kind::Tf { correct: true },
            QKind::Matching => Kind::Matching {
                pairs: (0..2)
                    .map(|_| Pair {
                        left: String::new(),
                        right: String::new(),
                    })
                    .collect(),
                distractors: vec![String::new()],
            },
            QKind::Blank => Kind::Blank {
                accept: vec![vec![String::new()]],
            },
            QKind::Short => Kind::Short {
                sample: String::new(),
            },
            QKind::Multipart => Kind::Multipart {
                parts: vec![
                    Part {
                        stem: String::new(),
                        kind: PartKind::Tf { correct: true },
                    },
                    Part {
                        stem: String::new(),
                        kind: PartKind::Mc {
                            answers: empty_answers(3),
                        },
                    },
                ],
            },
        };
    }

    /// How the points edit field shows the current points ("3", "3.5", empty for none).
    pub fn points_text(&self) -> String {
        match self.points {
            Some(p) if p.fract() == 0.0 => format!("{}", p as i64),
            Some(p) => p.to_string(),
            None => String::new(),
        }
    }

    pub fn set_points_text(&mut self, s: &str) {
        let t = s.trim();
        self.points = if t.is_empty() { None } else { t.parse().ok() };
    }

    /// Everything the Save button refuses to write without.
    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        if self.stem.trim().is_empty() {
            errs.push("Add a question stem.".to_string());
        }
        match &self.kind {
            Kind::Mc { answers } => {
                let filled = answers.iter().filter(|a| !a.text.trim().is_empty()).count();
                if filled < 2 {
                    errs.push("Add at least two answer choices.".to_string());
                }
                if !answers
                    .iter()
                    .any(|a| a.correct && !a.text.trim().is_empty())
                {
                    errs.push("Mark at least one correct answer.".to_string());
                }
            }
            Kind::Tf { .. } => {}
            Kind::Matching { pairs, .. } => {
                let filled = pairs
                    .iter()
                    .filter(|p| !p.left.trim().is_empty() && !p.right.trim().is_empty())
                    .count();
                if filled < 2 {
                    errs.push("Add at least two matching pairs (both sides filled).".to_string());
                }
            }
            Kind::Blank { accept } => {
                if blank_count(&self.stem) == 0 {
                    errs.push("Write ___ in the stem where the blank goes.".to_string());
                }
                for (i, row) in accept.iter().enumerate().take(blank_count(&self.stem)) {
                    if row.iter().all(|a| a.trim().is_empty()) {
                        errs.push(format!("Blank {} needs at least one accepted answer.", i + 1));
                    }
                }
            }
            Kind::Short { .. } => {}
            Kind::Multipart { parts } => {
                if parts.is_empty() {
                    errs.push("Add at least one part.".to_string());
                }
                for (i, p) in parts.iter().enumerate() {
                    if p.stem.trim().is_empty() {
                        errs.push(format!("Part {} needs its own stem.", i + 1));
                    }
                    if let PartKind::Mc { answers } = &p.kind {
                        let filled = answers
                            .iter()
                            .filter(|a| !a.text.trim().is_empty())
                            .count();
                        if filled >= 2
                            && !answers
                                .iter()
                                .any(|a| a.correct && !a.text.trim().is_empty())
                        {
                            errs.push(format!(
                                "Part {} needs at least one correct answer.",
                                i + 1
                            ));
                        }
                    }
                }
            }
        }
        errs
    }

    /// Builds the stored `Question`: strings trimmed, empties dropped, order kept.
    pub fn build_question(&self, existing: Option<&Question>) -> Question {
        let clean_mc = |answers: &[Answer]| -> Vec<Answer> {
            answers
                .iter()
                .filter(|a| !a.text.trim().is_empty())
                .map(|a| Answer {
                    text: a.text.trim().to_string(),
                    correct: a.correct,
                    locked: a.locked,
                })
                .collect()
        };
        let kind = match &self.kind {
            Kind::Mc { answers } => Kind::Mc {
                answers: clean_mc(answers),
            },
            Kind::Tf { correct } => Kind::Tf { correct: *correct },
            Kind::Matching { pairs, distractors } => Kind::Matching {
                pairs: pairs
                    .iter()
                    .filter(|p| !p.left.trim().is_empty() && !p.right.trim().is_empty())
                    .map(|p| Pair {
                        left: p.left.trim().to_string(),
                        right: p.right.trim().to_string(),
                    })
                    .collect(),
                distractors: distractors
                    .iter()
                    .map(|d| d.trim().to_string())
                    .filter(|d| !d.is_empty())
                    .collect(),
            },
            Kind::Blank { accept } => Kind::Blank {
                accept: accept
                    .iter()
                    .take(blank_count(&self.stem))
                    .map(|row| {
                        row.iter()
                            .map(|a| a.trim().to_string())
                            .filter(|a| !a.is_empty())
                            .collect()
                    })
                    .collect(),
            },
            Kind::Short { sample } => Kind::Short {
                sample: sample.trim().to_string(),
            },
            Kind::Multipart { parts } => Kind::Multipart {
                parts: parts
                    .iter()
                    .map(|p| Part {
                        stem: p.stem.trim().to_string(),
                        kind: match &p.kind {
                            PartKind::Mc { answers } => PartKind::Mc {
                                answers: clean_mc(answers),
                            },
                            PartKind::Tf { correct } => PartKind::Tf { correct: *correct },
                            PartKind::Short { sample } => PartKind::Short {
                                sample: sample.trim().to_string(),
                            },
                        },
                    })
                    .collect(),
            },
        };

        let mut topics: Vec<String> = Vec::new();
        for t in &self.topics {
            let t = t.trim().to_string();
            if !t.is_empty() && !topics.contains(&t) {
                topics.push(t);
            }
        }

        // An existing question keeps its id and created time; a new one is born now.
        let id = existing
            .map(|q| q.id.clone())
            .or_else(|| self.qid.clone())
            .unwrap_or_else(crate::model::new_id);
        let created = existing
            .map(|q| q.created)
            .unwrap_or_else(|| screens::now_secs());
        let t = screens::now_secs();
        Question {
            id,
            stem: self.stem.trim().to_string(),
            kind,
            points: self.points,
            difficulty: self.difficulty,
            topics,
            image: self.image.clone(),
            explanation: match self.explanation.as_deref() {
                Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                _ => None,
            },
            figure: if self.image.is_some() { self.figure.clone() } else { None },
            created,
            modified: t,
        }
    }
}

// ---------- small widget helpers ----------

#[derive(Clone, Copy, PartialEq)]
enum Ico {
    Up,
    Down,
    Lock,
    Close,
}

/// A 24 px square painter-drawn icon button (reorder / lock / remove).
fn icon_btn(ui: &mut Ui, ico: Ico, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, CornerRadius::same(7), theme::accent_faded());
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(7), theme::hover_bg());
    }
    let col = if on {
        theme::accent_col()
    } else if resp.hovered() {
        theme::ink()
    } else {
        theme::muted_col()
    };
    let s = Stroke::new(1.6, col);
    let c = rect.center();
    let d = |x: f32, y: f32| c + Vec2::new(x, y);
    match ico {
        Ico::Up => {
            p.line_segment([d(-4.0, 2.0), d(0.0, -2.0)], s);
            p.line_segment([d(0.0, -2.0), d(4.0, 2.0)], s);
        }
        Ico::Down => {
            p.line_segment([d(-4.0, -2.0), d(0.0, 2.0)], s);
            p.line_segment([d(0.0, 2.0), d(4.0, -2.0)], s);
        }
        Ico::Close => {
            p.line_segment([d(-3.5, -3.5), d(3.5, 3.5)], s);
            p.line_segment([d(-3.5, 3.5), d(3.5, -3.5)], s);
        }
        Ico::Lock => {
            p.rect_stroke(
                Rect::from_min_size(d(-4.5, -1.0), Vec2::new(9.0, 6.5)),
                CornerRadius::same(2),
                s,
                StrokeKind::Inside,
            );
            p.line_segment([d(-2.5, -1.0), d(-2.5, -3.5)], s);
            p.line_segment([d(-2.5, -3.5), d(2.5, -3.5)], s);
            p.line_segment([d(2.5, -3.5), d(2.5, -1.0)], s);
        }
    }
    resp
}

/// Rounded field surface (radius 12, field fill, hairline border).
fn field_frame(margin: Margin) -> Frame {
    Frame::new()
        .fill(theme::field())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(kit::R_FIELD))
        .inner_margin(margin)
}

/// Single-line field exactly `w` wide.
fn field_line(ui: &mut Ui, text: &mut String, w: f32, hint: &str) -> egui::Response {
    let inner = (w - 24.0 - 2.0).max(30.0);
    field_frame(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.add(
                TextEdit::singleline(text)
                    .frame(Frame::new())
                    .hint_text(hint)
                    .desired_width(inner),
            )
        })
        .inner
}

/// Multi-line field exactly `w` wide; `serif` uses the house serif at `size`.
fn field_multi(
    ui: &mut Ui,
    text: &mut String,
    w: f32,
    rows: usize,
    serif: Option<f32>,
    pad_right: i8,
    hint: &str,
) -> (egui::Rect, egui::text_edit::TextEditOutput) {
    let inner = (w - 12.0 - pad_right as f32 - 2.0).max(30.0);
    let fr = field_frame(Margin { left: 12, right: pad_right, top: 8, bottom: 8 }).show(ui, |ui| {
        let mut te = TextEdit::multiline(text)
            .frame(Frame::new())
            .hint_text(hint)
            .desired_rows(rows)
            .desired_width(inner);
        if let Some(sz) = serif {
            te = te.font(egui::FontId::new(sz, theme::serif_family()));
        }
        te.show(ui)
    });
    (fr.response.rect, fr.inner)
}

/// Painter-drawn sigma button (opens the equation modal).
fn sigma_button(ui: &mut Ui, eq: &EqCtx, field: &EqField, center: Pos2, size: f32) -> egui::Response {
    let btn = Rect::from_center_size(center, Vec2::splat(size));
    let br = ui.interact(btn, eq.key.with(("eq_sigma", field)), Sense::click());
    ui.painter().rect(
        btn,
        CornerRadius::same(8),
        if br.hovered() { theme::accent_faded() } else { theme::card() },
        Stroke::new(1.0, theme::border()),
        StrokeKind::Inside,
    );
    let k = size / 26.0;
    let s = Stroke::new(1.5, theme::accent_col());
    let d = |x: f32, y: f32| center + Vec2::new(x, y) * k;
    let pts = [d(4.0, -5.0), d(-4.0, -5.0), d(1.0, 0.0), d(-4.0, 5.0), d(4.0, 5.0)];
    for w2 in pts.windows(2) {
        ui.painter().line_segment([w2[0], w2[1]], s);
    }
    br.on_hover_text("Insert equation (or type / at the start of a word)")
}

/// A '/' typed at the start of a word opens the equation editor and eats the slash. Only a
/// slash that starts a word counts, so "1/2" and "and/or" type normally.
fn slash_hook(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &mut String, out: &egui::text_edit::TextEditOutput) {
    if !(out.response.has_focus() && ui.input(|i| i.key_pressed(Key::Slash))) {
        return;
    }
    let Some(cr) = out.cursor_range else { return };
    let idx: usize = cr.primary.index.into();
    let mut chars: Vec<char> = text.chars().collect();
    let starts_word = idx >= 1
        && chars.get(idx - 1) == Some(&'/')
        && (idx == 1 || chars.get(idx - 2).is_some_and(|c| c.is_whitespace()));
    if !starts_word {
        return;
    }
    eq.request(field, idx - 1, None, String::new());
    chars.remove(idx - 1);
    *text = chars.into_iter().collect();
    let mut st = out.state.clone();
    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(egui::text::CharIndex::from(idx - 1)))));
    st.store(&eq.ctx, out.response.id);
}

/// True when a field's text renders differently from its raw form (stem markup, `$math$`
/// or a `[picture]` placeholder an importer left).
fn renders_rich(text: &str) -> bool {
    crate::richtext::has_markup(text) || text.contains('*') || has_picture(text)
}

/// Does `text` hold a real `[picture]` / `[picture: alt]` marker?
fn has_picture(text: &str) -> bool {
    text.contains("[picture") && crate::bankops::split_pictures(text).iter().any(|p| matches!(p, crate::bankops::StemPart::Picture(_)))
}

/// `text` cut into the runs a preview draws: (char offset of the run in `text`, run) for
/// text, `None` offset for a picture placeholder (its alt in the run).
fn preview_runs(text: &str) -> Vec<(Option<usize>, String)> {
    use crate::bankops::StemPart;
    let mut byte = 0usize;
    crate::bankops::split_pictures(text)
        .into_iter()
        .map(|p| match p {
            StemPart::Text(t) => {
                let at = text[byte..].find(&t).map_or(byte, |i| byte + i);
                byte = at + t.len();
                (Some(text[..at].chars().count()), t)
            }
            StemPart::Picture(alt) => (None, alt.unwrap_or_default()),
        })
        .collect()
}

/// The editor's stand-in for a `[picture]` marker: a dashed "Picture needed" box with its
/// description (the picture itself is added with Attach image after import).
fn picture_box(ui: &mut Ui, alt: &str, size: f32) {
    let w = ui.available_width().clamp(120.0, 420.0);
    let h = if alt.is_empty() { size * 2.6 } else { size * 3.6 };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter();
    let red = theme::red();
    p.rect_filled(rect, CornerRadius::same(6), red.gamma_multiply(0.08));
    p.extend(egui::Shape::dashed_line(
        &[rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()],
        Stroke::new(1.0, red),
        4.0,
        3.0,
    ));
    let cy = if alt.is_empty() { rect.center().y } else { rect.center().y - size * 0.6 };
    p.text(Pos2::new(rect.center().x, cy), egui::Align2::CENTER_CENTER, "Picture needed", egui::FontId::new(size, theme::bold_family()), red);
    if !alt.is_empty() {
        let a: String = if alt.chars().count() > 60 { alt.chars().take(60).chain(std::iter::once('…')).collect() } else { alt.to_string() };
        p.text(Pos2::new(rect.center().x, cy + size * 1.25), egui::Align2::CENTER_CENTER, a, egui::FontId::new(size * 0.85, theme::serif_family()), theme::muted_col());
    }
    ui.add_space(4.0);
}

/// Rendered preview of a field that holds markup or equations; clicking an equation opens it
/// for editing.
fn eq_preview(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &str, indent: f32, size: f32) {
    if !renders_rich(text) {
        return;
    }
    let mut hit = None;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.vertical(|ui| {
            let o = crate::equation::Rich::new(size, theme::text_col()).editable(true);
            if !has_picture(text) {
                hit = crate::equation::rich_ui(ui, text, &o).1;
                return;
            }
            for (at, run) in preview_runs(text) {
                match at {
                    Some(at) => {
                        if let Some(mut h) = crate::equation::rich_ui(ui, &run, &o).1 {
                            h.span = h.span.start + at..h.span.end + at;
                            hit = Some(h);
                        }
                    }
                    None => picture_box(ui, &run, size),
                }
            }
        });
    });
    if let Some(h) = hit {
        eq.request(field, 0, Some(h.span), h.content);
    }
    ui.add_space(2.0);
}

/// The stem's rendered preview: how the question prints (tables, lists, bold, scripts, math),
/// in a soft card under the field. Shown once the stem has any markup or math.
fn stem_preview(ui: &mut Ui, eq: &EqCtx, text: &str, w: f32) {
    if !renders_rich(text) {
        return;
    }
    ui.add_space(6.0);
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(kit::R_FIELD))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width((w - 26.0).max(40.0));
            ui.label(theme::muted("Preview").size(11.5));
            ui.add_space(2.0);
            eq_preview(ui, eq, EqField::Stem, text, 0.0, 14.0);
        });
}

/// The stem's insert helpers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Markup {
    Bold,
    Italic,
    Bullets,
    Table,
}

const TABLE_TEMPLATE: &str = "Column 1 | Column 2 | Column 3\n---|---|---\n___ | ___ | ___\n___ | ___ | ___";

/// Apply an insert helper to `text` with the char selection `sel`; returns the new text and the
/// new char selection. Bold/italic wrap the selection (or insert a selected placeholder); the
/// list helper toggles "• " on every line the selection touches; the table helper inserts a
/// header + two blank rows on lines of their own with "Column 1" selected.
fn apply_markup(text: &str, sel: std::ops::Range<usize>, m: Markup) -> (String, std::ops::Range<usize>) {
    let ch: Vec<char> = text.chars().collect();
    let a = sel.start.min(sel.end).min(ch.len());
    let b = sel.start.max(sel.end).min(ch.len());
    let collect = |v: &[char]| v.iter().collect::<String>();
    match m {
        Markup::Bold | Markup::Italic => {
            let mark = if m == Markup::Bold { "**" } else { "*" };
            let ml = mark.chars().count();
            // Keep surrounding spaces outside the markers.
            let (mut s, mut e) = (a, b);
            while s < e && ch[s].is_whitespace() {
                s += 1;
            }
            while e > s && ch[e - 1].is_whitespace() {
                e -= 1;
            }
            let inner = if s == e { if m == Markup::Bold { "bold text".to_string() } else { "italic text".to_string() } } else { collect(&ch[s..e]) };
            let n = inner.chars().count();
            let out = format!("{}{mark}{inner}{mark}{}", collect(&ch[..s]), collect(&ch[e..]));
            (out, s + ml..s + ml + n)
        }
        Markup::Bullets => {
            let ls = ch[..a].iter().rposition(|c| *c == '\n').map_or(0, |i| i + 1);
            let le = ch[b..].iter().position(|c| *c == '\n').map_or(ch.len(), |i| b + i);
            let block = collect(&ch[ls..le]);
            let lines: Vec<&str> = block.split('\n').collect();
            let is_b = |l: &str| l.trim_start().starts_with("• ");
            let all = lines.iter().filter(|l| !l.trim().is_empty()).all(|l| is_b(l)) && lines.iter().any(|l| is_b(l));
            let new: Vec<String> = lines
                .iter()
                .map(|l| {
                    if all {
                        l.replacen("• ", "", 1)
                    } else if is_b(l) || (l.trim().is_empty() && lines.len() > 1) {
                        l.to_string()
                    } else {
                        format!("• {}", l.trim_start())
                    }
                })
                .collect();
            let nb = new.join("\n");
            let end = ls + nb.chars().count();
            (format!("{}{nb}{}", collect(&ch[..ls]), collect(&ch[le..])), end..end)
        }
        Markup::Table => {
            let pre = if a > 0 && ch[a - 1] != '\n' { "\n" } else { "" };
            let post = if b < ch.len() && ch[b] != '\n' { "\n" } else { "" };
            let start = a + pre.chars().count();
            let out = format!("{}{pre}{TABLE_TEMPLATE}{post}{}", collect(&ch[..a]), collect(&ch[b..]));
            (out, start..start + "Column 1".chars().count())
        }
    }
}

/// Small text button for the stem toolbar.
fn tool_button(ui: &mut Ui, label: RichText, tip: &str) -> egui::Response {
    ui.add(
        Button::new(label.size(12.5).color(theme::text_col()))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(8))
            .min_size(Vec2::new(30.0, 24.0)),
    )
    .on_hover_text(tip)
}

/// The stem toolbar (bold, italic, bullet list, table). Edits `text` around the field's
/// selection and puts the caret/selection back into the field.
fn stem_toolbar(ui: &mut Ui, text: &mut String, out: &egui::text_edit::TextEditOutput) {
    let mut pick = None;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if tool_button(ui, RichText::new("B").family(theme::bold_family()), "Bold (**text**)").clicked() {
            pick = Some(Markup::Bold);
        }
        if tool_button(ui, RichText::new("I").italics(), "Italic (*text*)").clicked() {
            pick = Some(Markup::Italic);
        }
        if tool_button(ui, RichText::new("• List"), "Bullet list (lines starting with \"• \")").clicked() {
            pick = Some(Markup::Bullets);
        }
        if tool_button(ui, RichText::new("Table"), "Table (cells separated by \" | \"; ___ is a blank)").clicked() {
            pick = Some(Markup::Table);
        }
        ui.add_space(6.0);
        ui.label(theme::muted("x_{2} subscript · x^{2} superscript · ___ blank").size(11.0));
    });
    let Some(m) = pick else { return };
    let n = text.chars().count();
    let sel = out.state.cursor.char_range().map_or(n..n, |r| {
        let [x, y] = r.sorted_cursors();
        usize::from(x.index)..usize::from(y.index)
    });
    let (t, ns) = apply_markup(text, sel, m);
    *text = t;
    let mut st = out.state.clone();
    st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
        egui::text::CCursor::new(egui::text::CharIndex::from(ns.start)),
        egui::text::CCursor::new(egui::text::CharIndex::from(ns.end)),
    )));
    st.store(ui.ctx(), out.response.id);
    ui.memory_mut(|mem| mem.request_focus(out.response.id));
}

/// Single-line field with the sigma affordance at its right edge.
fn eq_line(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &mut String, w: f32, hint: &str) -> egui::Response {
    let inner = (w - 24.0 - 2.0 - 24.0).max(30.0);
    let fr = field_frame(Margin::symmetric(12, 6)).show(ui, |ui| {
        TextEdit::singleline(text).frame(Frame::new()).hint_text(hint).desired_width(inner).show(ui)
    });
    let out = fr.inner;
    let r = fr.response.rect;
    let idx = out.cursor_range.map_or(text.chars().count(), |cr| cr.primary.index.into());
    if sigma_button(ui, eq, &field, Pos2::new(r.right() - 17.0, r.center().y), 22.0).clicked() {
        eq.request(field.clone(), idx, None, String::new());
    }
    slash_hook(ui, eq, field, text, &out);
    out.response.response
}

/// Like `eq_line`, but the text may hold line breaks (a multipart part's stem: its
/// sub-lines, bullets and table rows). A single-line field would draw each `\n` as a
/// missing-glyph box; this one grows a row per line, and Enter adds a line.
fn eq_line_multi(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &mut String, w: f32, hint: &str) -> egui::Response {
    let inner = (w - 24.0 - 2.0 - 24.0).max(30.0);
    let fr = field_frame(Margin::symmetric(12, 6)).show(ui, |ui| {
        TextEdit::multiline(text).frame(Frame::new()).hint_text(hint).desired_rows(1).desired_width(inner).show(ui)
    });
    let out = fr.inner;
    let r = fr.response.rect;
    let idx = out.cursor_range.map_or(text.chars().count(), |cr| cr.primary.index.into());
    if sigma_button(ui, eq, &field, Pos2::new(r.right() - 17.0, r.top() + 17.0), 22.0).clicked() {
        eq.request(field.clone(), idx, None, String::new());
    }
    slash_hook(ui, eq, field, text, &out);
    out.response.response
}

/// Multi-line field with the sigma affordance and a typeset preview.
fn eq_multi(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &mut String, w: f32, rows: usize, hint: &str) {
    let (frect, out) = field_multi(ui, text, w, rows, None, 34, hint);
    let idx = out.cursor_range.map_or(text.chars().count(), |cr| cr.primary.index.into());
    if sigma_button(ui, eq, &field, Pos2::new(frect.right() - 20.0, frect.top() + 20.0), 26.0).clicked() {
        eq.request(field.clone(), idx, None, String::new());
    }
    slash_hook(ui, eq, field.clone(), text, &out);
    eq_preview(ui, eq, field, text, 0.0, 13.0);
}

fn label(ui: &mut Ui, text: &str) {
    kit::small_caps(ui, text);
    ui.add_space(3.0);
}

// ---------- the in-place editor ----------

impl App {
    /// Writes a validated draft into the store (new or edit) and toasts.
    pub(crate) fn commit_draft(&mut self, draft: Draft) {
        let existing = draft
            .qid
            .as_ref()
            .and_then(|qid| self.store.question(&draft.bank_id, qid))
            .cloned();
        let q = draft.build_question(existing.as_ref());
        let t = q.modified;
        if let Some(bank) = self.store.bank_mut(&draft.bank_id) {
            match draft.qid.as_ref() {
                Some(qid) => {
                    if let Some(slot) = bank.questions.iter_mut().find(|x| x.id == *qid) {
                        *slot = q;
                    } else {
                        bank.questions.push(q);
                    }
                }
                None => {
                    bank.questions.push(q);
                }
            }
            bank.modified = t;
        }
        self.store.save_banks();
        self.toast("Question saved");
    }
}

/// Draws the expanded editor body (two columns + footer) for `draft`. Call inside the
/// open question card; the caller applies the returned action. `topic_pool` is the
/// bank's existing topics (offered as one-tap suggestions).
pub fn draw_inline(
    ui: &mut Ui,
    draft: &mut Draft,
    topic_pool: &[String],
    textures: &mut HashMap<String, egui::TextureHandle>,
) -> QAction {
    let ctx = ui.ctx().clone();
    let key = temp_key(&draft.bank_id, draft.qid.as_ref());
    let mut action = QAction::None;

    let total = ui.available_width();
    let gap = 24.0;
    let narrow = total < 640.0;
    let (left_w, right_w) = if narrow {
        (total, total)
    } else {
        let l = ((total - gap) * 0.6).floor();
        (l, total - gap - l)
    };

    if narrow {
        ui.vertical(|ui| {
            left_col(ui, &ctx, draft, key, left_w);
            ui.add_space(10.0);
            right_col(ui, &ctx, draft, key, right_w, topic_pool, textures);
        });
    } else {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(Vec2::new(left_w, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(left_w);
                left_col(ui, &ctx, draft, key, left_w);
            });
            ui.add_space(gap);
            ui.allocate_ui_with_layout(Vec2::new(right_w, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(right_w);
                right_col(ui, &ctx, draft, key, right_w, topic_pool, textures);
            });
        });
    }

    // Footer: Delete (danger text), save note, Cancel, Done.
    ui.add_space(6.0);
    let y = ui.cursor().top();
    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::rule()));
    ui.add_space(8.0);
    let errs = draft.validate();
    if !errs.is_empty() {
        for e in &errs {
            red(ui, e);
        }
        ui.add_space(4.0);
    }
    ui.horizontal(|ui| {
        if kit::danger_text_button(ui, "Delete").clicked() {
            action = QAction::Delete;
        }
        ui.add_space(8.0);
        ui.label(theme::muted("Changes save when you press Done.").size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let done = ui.add_enabled_ui(errs.is_empty(), |ui| kit::primary_button(ui, "Done")).inner;
            if done.clicked() {
                action = QAction::Done;
            }
            let cancel = ui.add(
                Button::new(RichText::new("Cancel").family(theme::bold_family()).color(theme::muted_col()))
                    .frame(false),
            );
            if cancel.clicked() {
                action = QAction::Cancel;
            }
        });
    });
    action
}

fn left_col(ui: &mut Ui, ctx: &egui::Context, draft: &mut Draft, key: UiId, w: f32) {
    // Stem with the equation hook.
    let eq = EqCtx { ctx: ctx.clone(), key };
    label(ui, "Question");
    let (frect, out) = field_multi(ui, &mut draft.stem, w, 3, Some(14.5), 34, "Write the question...");
    let idx = out.cursor_range.map_or(draft.stem.chars().count(), |cr| cr.primary.index.into());
    if sigma_button(ui, &eq, &EqField::Stem, Pos2::new(frect.right() - 20.0, frect.top() + 20.0), 26.0).clicked() {
        eq.request(EqField::Stem, idx, None, String::new());
    }
    slash_hook(ui, &eq, EqField::Stem, &mut draft.stem, &out);
    stem_toolbar(ui, &mut draft.stem, &out);
    stem_preview(ui, &eq, &draft.stem, w);
    equation_panel(ctx, draft, key);
    ui.add_space(12.0);

    // Per-kind answers.
    match &mut draft.kind {
        Kind::Mc { answers } => {
            label(ui, "Answers  -  click the circle to mark correct");
            mc_editor(ui, &eq, answers, usize::MAX, w, &|i| EqField::Mc(i));
        }
        Kind::Tf { correct } => {
            label(ui, "Correct answer  -  click the circle");
            for (val, name) in [(true, "True"), (false, "False")] {
                ui.horizontal(|ui| {
                    if kit::check_circle(ui, *correct == val).clicked() {
                        *correct = val;
                    }
                    ui.add_space(4.0);
                    ui.label(theme::text(name).size(13.5));
                });
            }
        }
        Kind::Matching { pairs, distractors } => matching_editor(ui, &eq, pairs, distractors, w),
        Kind::Blank { .. } => blank_editor(ui, &eq, draft, w),
        Kind::Short { sample } => {
            label(ui, "Model answer");
            eq_multi(ui, &eq, EqField::Short, sample, w, 3, "What a good answer looks like...");
        }
        Kind::Multipart { parts } => {
            label(ui, "Parts");
            multipart_editor(ui, &eq, parts, w);
        }
    }
}

fn right_col(
    ui: &mut Ui,
    ctx: &egui::Context,
    draft: &mut Draft,
    key: UiId,
    w: f32,
    topic_pool: &[String],
    textures: &mut HashMap<String, egui::TextureHandle>,
) {
    // Type.
    label(ui, "Type");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        for (k, tag) in KINDS {
            if kit::chip_toggle(ui, tag, draft.qkind() == k).clicked() && draft.qkind() != k {
                draft.set_kind(k);
                ctx.data_mut(|d| d.remove_temp::<Temp>(key));
            }
        }
    });
    ui.add_space(12.0);

    // Points + difficulty.
    let half = ((w - 10.0) / 2.0).floor();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);
            label(ui, "Points");
            let mut pt = draft.points_text();
            if field_line(ui, &mut pt, half, "none").changed() {
                draft.set_points_text(&pt);
            }
        });
        ui.allocate_ui_with_layout(Vec2::new(w - half - 10.0, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(w - half - 10.0);
            label(ui, "Difficulty");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                for (d, name) in [
                    (Difficulty::Low, "Easy"),
                    (Difficulty::Medium, "Medium"),
                    (Difficulty::High, "Hard"),
                ] {
                    let on = draft.difficulty == Some(d);
                    if kit::chip_toggle(ui, name, on).clicked() {
                        draft.difficulty = if on { None } else { Some(d) };
                    }
                }
            });
        });
    });
    ui.add_space(12.0);

    // Topics: removable chips, add field, suggestions.
    label(ui, "Topics");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        for t in draft.topics.clone() {
            if kit::chip_toggle(ui, &format!("{t}  x"), true)
                .on_hover_text("Remove topic")
                .clicked()
            {
                draft.topics.retain(|x| x != &t);
            }
        }
    });
    let mut txt = ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).topics_text.clone());
    let r = field_line(ui, &mut txt, w, "+ add topic (press Enter)");
    // Autocomplete: the bank's topics matching what is typed (prefix first); Enter takes
    // the existing spelling when the typed text names a topic already in the bank.
    let sugg = crate::bankops::suggest_topics(topic_pool, &txt, &draft.topics, 8);
    let commit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !txt.trim().is_empty();
    if commit {
        let t = crate::bankops::canonical_topic(topic_pool, &txt);
        if !draft.topics.contains(&t) {
            draft.topics.push(t);
        }
        txt.clear();
        r.request_focus();
    }
    let typed = txt.trim().to_string();
    let mut picked: Option<String> = None;
    if !sugg.is_empty() || !typed.is_empty() {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            if !typed.is_empty() {
                let exists = topic_pool.iter().any(|p| p.trim().eq_ignore_ascii_case(&typed));
                ui.label(
                    theme::muted(if exists { "Enter adds this topic." } else if sugg.is_empty() { "Enter adds a new topic." } else { "Matching:" })
                        .size(11.0),
                );
            }
            for t in &sugg {
                if kit::chip_toggle(ui, t, false).clicked() {
                    picked = Some(t.clone());
                }
            }
        });
    }
    if let Some(t) = picked {
        if !draft.topics.contains(&t) {
            draft.topics.push(t);
        }
        txt.clear();
    }
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).topics_text = txt);
    ui.add_space(12.0);

    // Explanation.
    label(ui, "Explanation (optional)");
    let mut explanation = draft.explanation.clone().unwrap_or_default();
    field_multi(ui, &mut explanation, w, 2, None, 12, "Shown with the answer key");
    draft.explanation = match explanation.trim() {
        "" => None,
        _ => Some(explanation),
    };
    ui.add_space(12.0);

    label(ui, "Image");
    draw_image_row(ui, draft, textures);
}

/// Multiple choice / multiple answer. Each row: tappable correct circle (one ticked =
/// single choice; several = multiple answer), text, reorder up/down, lock, remove.
fn mc_editor(ui: &mut Ui, eq: &EqCtx, answers: &mut Vec<Answer>, max: usize, w: f32, fld: &dyn Fn(usize) -> EqField) {
    let mut to_remove: Option<usize> = None;
    let mut move_up: Option<usize> = None;
    let n = answers.len();
    let field_w = (w - 22.0 - 4.0 * 24.0 - 5.0 * 6.0).max(120.0);
    for i in 0..n {
        let locked = answers[i].locked;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if kit::check_circle(ui, answers[i].correct).clicked() {
                answers[i].correct = !answers[i].correct;
            }
            eq_line(ui, eq, fld(i), &mut answers[i].text, field_w, "Answer");
            if icon_btn(ui, Ico::Up, false).on_hover_text("Move up").clicked() && i > 0 {
                move_up = Some(i);
            }
            if icon_btn(ui, Ico::Down, false).on_hover_text("Move down").clicked() && i + 1 < n {
                move_up = Some(i + 1);
            }
            if icon_btn(ui, Ico::Lock, locked)
                .on_hover_text("Lock answer position (never shuffled)")
                .clicked()
            {
                answers[i].locked = !locked;
            }
            if n > 2 {
                if icon_btn(ui, Ico::Close, false).on_hover_text("Remove").clicked() {
                    to_remove = Some(i);
                }
            } else {
                ui.add_space(24.0);
            }
        });
        eq_preview(ui, eq, fld(i), &answers[i].text, 28.0, 13.0);
        ui.add_space(2.0);
    }
    if let Some(i) = to_remove {
        answers.remove(i);
    }
    if let Some(i) = move_up {
        answers.swap(i, i - 1);
    }
    ui.add_space(2.0);
    // Parts are capped at six answer choices; top-level questions are not.
    if answers.len() < max && ghost_button(ui, "+ Add answer").clicked() {
        answers.push(empty_answer());
    }
}

/// Matching pairs and extra distractors (the "model answer" for MATCH).
fn matching_editor(ui: &mut Ui, eq: &EqCtx, pairs: &mut Vec<Pair>, distractors: &mut Vec<String>, w: f32) {
    let mut remove_pair: Option<usize> = None;
    label(ui, "Pairs  -  left item, right item");
    let fw = ((w - 20.0 - 24.0 - 3.0 * 6.0) / 2.0).max(80.0);
    for i in 0..pairs.len() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            eq_line(ui, eq, EqField::MatchL(i), &mut pairs[i].left, fw, "Left");
            // painter arrow
            let (r, _) = ui.allocate_exact_size(Vec2::new(20.0, 24.0), Sense::hover());
            let s = Stroke::new(1.5, theme::muted_col());
            let c = r.center();
            ui.painter().line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(5.0, 0.0)], s);
            ui.painter().line_segment([c + Vec2::new(5.0, 0.0), c + Vec2::new(2.0, -3.0)], s);
            ui.painter().line_segment([c + Vec2::new(5.0, 0.0), c + Vec2::new(2.0, 3.0)], s);
            eq_line(ui, eq, EqField::MatchR(i), &mut pairs[i].right, fw, "Right");
            if icon_btn(ui, Ico::Close, false).on_hover_text("Remove pair").clicked() {
                remove_pair = Some(i);
            }
        });
        eq_preview(ui, eq, EqField::MatchL(i), &pairs[i].left, 0.0, 13.0);
        eq_preview(ui, eq, EqField::MatchR(i), &pairs[i].right, 0.0, 13.0);
        ui.add_space(2.0);
    }
    if let Some(i) = remove_pair {
        pairs.remove(i);
    }
    if ghost_button(ui, "+ Add pair").clicked() {
        pairs.push(Pair {
            left: String::new(),
            right: String::new(),
        });
    }

    ui.add_space(8.0);
    label(ui, "Distractors  -  extra right-side items");
    let mut remove_dis: Option<usize> = None;
    for i in 0..distractors.len() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            eq_line(ui, eq, EqField::Distractor(i), &mut distractors[i], (w - 30.0).max(120.0), "Distractor");
            if icon_btn(ui, Ico::Close, false).on_hover_text("Remove").clicked() {
                remove_dis = Some(i);
            }
        });
        eq_preview(ui, eq, EqField::Distractor(i), &distractors[i], 0.0, 13.0);
        ui.add_space(2.0);
    }
    if let Some(i) = remove_dis {
        distractors.remove(i);
    }
    if ghost_button(ui, "+ Add distractor").clicked() {
        distractors.push(String::new());
    }
}

/// Fill in the blank: one accepted-alternatives row per "___" in the stem.
fn blank_editor(ui: &mut Ui, eq: &EqCtx, draft: &mut Draft, w: f32) {
    let key = eq.key;
    let ctx = &eq.ctx;
    let blanks = blank_count(&draft.stem).max(1);

    let mut texts = ctx.data_mut(|d| {
        let st = d.get_temp_mut_or_default::<Temp>(key);
        if st.blank_texts.is_none() {
            if let Kind::Blank { accept } = &draft.kind {
                st.blank_texts = Some(accept.iter().map(|v| v.join(" OR ")).collect());
            }
        }
        let texts = st.blank_texts.get_or_insert_with(Vec::new);
        while texts.len() < blanks {
            texts.push(String::new());
        }
        texts.clone()
    });
    for i in 0..blanks {
        label(ui, &format!("Blank {}  -  accepted answers, separate alternatives with OR", i + 1));
        eq_line(ui, eq, EqField::Blank(i), &mut texts[i], w, "e.g. 1 OR one");
        eq_preview(ui, eq, EqField::Blank(i), &texts[i], 0.0, 13.0);
        ui.add_space(4.0);
    }
    // Mirror -> draft; the text the user types stays in the mirror.
    let accept = texts.iter().map(|t| split_alts(t)).collect::<Vec<Vec<String>>>();
    draft.kind = Kind::Blank { accept };
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).blank_texts = Some(texts));
}

/// Multipart: each part has a stem, a kind selector and an expanded editor below.
fn multipart_editor(ui: &mut Ui, eq: &EqCtx, parts: &mut Vec<Part>, w: f32) {
    let mut remove: Option<usize> = None;
    for i in 0..parts.len() {
        let cur = match &parts[i].kind {
            PartKind::Mc { .. } => 0,
            PartKind::Tf { .. } => 1,
            PartKind::Short { .. } => 2,
        };
        let mut sel = cur;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(theme::muted(format!("{}.", i + 1)).size(12.0));
            eq_line_multi(ui, eq, EqField::PartStem(i), &mut parts[i].stem, (w - 24.0 - 3.0 * 52.0 - 30.0).max(120.0), "Part stem");
            for (v, tag) in [(0, "MC"), (1, "TF"), (2, "Short")] {
                if kit::chip_toggle(ui, tag, sel == v).clicked() {
                    sel = v;
                }
            }
            if parts.len() > 1 && icon_btn(ui, Ico::Close, false).on_hover_text("Remove part").clicked() {
                remove = Some(i);
            }
        });
        eq_preview(ui, eq, EqField::PartStem(i), &parts[i].stem, 20.0, 13.0);
        if sel != cur {
            parts[i].kind = match sel {
                0 => PartKind::Mc {
                    answers: empty_answers(3),
                },
                1 => PartKind::Tf { correct: true },
                _ => PartKind::Short {
                    sample: String::new(),
                },
            };
        }
        ui.push_id(i, |ui| {
            ui.add_space(2.0);
            Frame::new().inner_margin(Margin::symmetric(18, 6)).show(ui, |ui| {
                let iw = w - 36.0;
                match &mut parts[i].kind {
                    PartKind::Mc { answers } => mc_editor(ui, eq, answers, 6, iw, &|j| EqField::PartMc(i, j)),
                    PartKind::Tf { correct } => {
                        for (val, name) in [(true, "True"), (false, "False")] {
                            ui.horizontal(|ui| {
                                if kit::check_circle(ui, *correct == val).clicked() {
                                    *correct = val;
                                }
                                ui.add_space(4.0);
                                ui.label(theme::text(name).size(13.0));
                            });
                        }
                    }
                    PartKind::Short { sample } => {
                        eq_multi(ui, eq, EqField::PartShort(i), sample, iw, 2, "Model answer");
                    }
                }
            });
        });
    }
    if let Some(i) = remove {
        parts.remove(i);
    }
    ui.add_space(2.0);
    if ghost_button(ui, "+ Add part").clicked() {
        parts.push(Part {
            stem: String::new(),
            kind: PartKind::Tf { correct: true },
        });
    }
}

// ---------- images ----------

/// Loads (and caches in the shared media map) the texture for a stored image.
fn media_texture(
    textures: &mut HashMap<String, egui::TextureHandle>,
    ctx: &egui::Context,
    file: &str,
) -> Option<egui::TextureHandle> {
    if let Some(t) = textures.get(file) {
        return Some(t.clone());
    }
    let bytes = std::fs::read(crate::store::store_dir().join(file)).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let tex = ctx.load_texture(
        format!("testparrot:{file}"),
        egui::ColorImage::from_rgba_unmultiplied(
            [img.width().max(1) as usize, img.height().max(1) as usize],
            &img,
        ),
        egui::TextureOptions::LINEAR,
    );
    textures.insert(file.to_string(), tex.clone());
    Some(tex)
}

/// Attach / preview / remove image row.
fn draw_image_row(
    ui: &mut Ui,
    draft: &mut Draft,
    textures: &mut HashMap<String, egui::TextureHandle>,
) {
    let key = UiId::new("testparrot_qedit_media_status");

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        if ghost_button(ui, "Attach image...").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp"])
                .pick_file()
        {
            match std::fs::read(&path) {
                Ok(bytes) => match crate::store::put_image(&bytes) {
                    Ok(img) => {
                        media_texture(textures, ui.ctx(), &img.file);
                        draft.image = Some(img);
                        draft.figure = None;
                        ui.ctx().data_mut(|d| d.insert_temp(key, None::<(String, bool)>));
                    }
                    Err(e) => {
                        ui.ctx().data_mut(|d| d.insert_temp(key, Some((e, true))));
                    }
                },
                Err(e) => ui.ctx().data_mut(|d| {
                    d.insert_temp(key, Some((format!("Couldn't read the file: {e}"), true)));
                }),
            }
        }
        if draft.image.is_some() && ghost_button(ui, "Remove").clicked() {
            draft.image = None;
            draft.figure = None;
        }
        if ghost_button(ui, "Insert figure...").clicked() {
            crate::figuresui::request(ui.ctx(), crate::figuresui::REQ_INSERT);
        }
        if draft.figure.is_some() && ghost_button(ui, "Edit figure").clicked() {
            crate::figuresui::request(ui.ctx(), crate::figuresui::REQ_EDIT);
        }
        if ghost_button(ui, "Import SVG/PNG...").clicked() {
            crate::figuresui::request(ui.ctx(), crate::figuresui::REQ_IMPORT);
        }
    });
    if let Some(f) = &draft.figure {
        ui.label(theme::muted(format!("Generated figure: {}", f.id)).size(11.0));
    }

    if let Some(status) = ui.ctx().data_mut(|d| d.get_temp::<(String, bool)>(key)) {
        if !status.0.is_empty() {
            red(ui, &status.0);
        }
    }

    if let Some(img) = draft.image.clone()
        && let Some(tex) = media_texture(textures, ui.ctx(), &img.file)
    {
        ui.add_space(4.0);
        Frame::new()
            .fill(theme::field())
            .corner_radius(CornerRadius::same(kit::R_FIELD))
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.add(egui::Image::new(&tex).max_height(110.0));
            });
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    #[test]
    fn markup_helpers_wrap_toggle_and_insert() {
        // Bold wraps the selection (spaces stay outside) and keeps it selected.
        let (t, s) = apply_markup("the most one", 3..9, Markup::Bold);
        assert_eq!(t, "the **most** one");
        assert_eq!(&t.chars().collect::<Vec<_>>()[s.clone()].iter().collect::<String>(), "most");
        // Italic with no selection inserts a selected placeholder.
        let (t, s) = apply_markup("ab", 1..1, Markup::Italic);
        assert_eq!(t, "a*italic text*b");
        assert_eq!(s, 2..13);
        // Bullets toggle on every touched line.
        let (t, _) = apply_markup("Do:\nDraw\nCount", 5..12, Markup::Bullets);
        assert_eq!(t, "Do:\n• Draw\n• Count");
        let (t, _) = apply_markup(&t, 5..14, Markup::Bullets);
        assert_eq!(t, "Do:\nDraw\nCount");
        let (t, _) = apply_markup("", 0..0, Markup::Bullets);
        assert_eq!(t, "• ");
        // Table goes on its own lines and parses as a table with a header.
        let (t, s) = apply_markup("Name these.", 11..11, Markup::Table);
        assert!(t.starts_with("Name these.\nColumn 1 | "), "{t}");
        assert_eq!(t.chars().skip(s.start).take(s.end - s.start).collect::<String>(), "Column 1");
        let blocks = crate::richtext::parse(&t);
        assert!(matches!(&blocks[1], crate::richtext::Block::Table { header: true, rows } if rows.len() == 3));
        // Selection past the end is clamped.
        let (t, _) = apply_markup("x", 5..9, Markup::Bold);
        assert_eq!(t, "x**bold text**");
    }

    #[test]
    fn rich_preview_only_for_markup_or_math() {
        assert!(!renders_rich("What is 2+2?"));
        assert!(renders_rich("a\nb"));
        assert!(renders_rich("$x$"));
        assert!(renders_rich("**b**"));
    }

    use super::*;

    fn sample_question() -> Question {
        Question {
            id: "q1".to_string(),
            stem: "What is 2+2?".to_string(),
            kind: Kind::Mc {
                answers: vec![
                    Answer {
                        text: "4".to_string(),
                        correct: true,
                        locked: false,
                    },
                    Answer {
                        text: "5".to_string(),
                        correct: false,
                        locked: false,
                    },
                ],
            },
            points: Some(2.0),
            difficulty: Some(Difficulty::Medium),
            topics: vec!["arith".to_string(), "  arith ".to_string()],
            image: None,
            explanation: Some(" 2+2 makes four ".to_string()),
            figure: None,
            created: 1_700_000_000,
            modified: 1_700_000_100,
        }
    }

    #[test]
    fn new_draft_defaults_to_four_mc_answers() {
        let d = Draft::new("bank".to_string());
        assert!(d.qid.is_none());
        assert_eq!(d.stem, "");
        match &d.kind {
            Kind::Mc { answers } => {
                assert_eq!(answers.len(), 4);
                assert!(answers.iter().all(|a| a.text.is_empty() && !a.correct));
            }
            other => panic!("wrong default kind: {other:?}"),
        }
        assert_eq!(d.points, None);
        assert_eq!(d.difficulty, None);
        assert!(d.topics.is_empty());
    }

    #[test]
    fn empty_draft_fails_validation() {
        let d = Draft::new("bank".to_string());
        let errs = d.validate();
        assert!(errs.iter().any(|e| e.contains("stem")));
        assert!(errs.iter().any(|e| e.contains("choices")));
        assert!(errs.iter().any(|e| e.contains("correct")));
    }

    #[test]
    fn valid_mc_draft_passes() {
        let mut d = Draft::new("bank".to_string());
        d.stem = "Pick one".to_string();
        if let Kind::Mc { answers } = &mut d.kind {
            answers[0].text = "A".to_string();
            answers[1].text = "B".to_string();
            answers[0].correct = true;
        }
        assert!(d.validate().is_empty());
    }

    #[test]
    fn set_kind_resets_data_to_defaults() {
        let mut d = Draft::new("bank".to_string());
        d.stem = "x".to_string();
        d.set_kind(QKind::Tf);
        assert!(matches!(d.kind, Kind::Tf { correct: true }));
        d.set_kind(QKind::Matching);
        match &d.kind {
            Kind::Matching { pairs, distractors } => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(distractors.len(), 1);
            }
            other => panic!("{other:?}"),
        }
        d.set_kind(QKind::Blank);
        assert!(matches!(&d.kind, Kind::Blank { accept } if accept.len() == 1));
        d.set_kind(QKind::Short);
        assert!(matches!(&d.kind, Kind::Short { sample } if sample.is_empty()));
        d.set_kind(QKind::Multipart);
        match &d.kind {
            Kind::Multipart { parts } => assert_eq!(parts.len(), 2),
            other => panic!("{other:?}"),
        }
        d.set_kind(QKind::Mc);
        assert!(matches!(&d.kind, Kind::Mc { answers } if answers.len() == 4));
    }

    #[test]
    fn points_parsing() {
        let mut d = Draft::new("bank".to_string());
        d.set_points_text("3");
        assert_eq!(d.points, Some(3.0));
        assert_eq!(d.points_text(), "3");
        d.set_points_text("3.5");
        assert_eq!(d.points, Some(3.5));
        d.set_points_text("");
        assert_eq!(d.points, None);
        d.set_points_text("junk");
        assert_eq!(d.points, None);
    }

    #[test]
    fn from_question_and_build_roundtrip() {
        let q = sample_question();
        let d = Draft::from_question("bank".to_string(), &q);
        assert!(d.is_edit());
        let rebuilt = d.build_question(Some(&q));
        assert_eq!(rebuilt.id, q.id);
        assert_eq!(rebuilt.stem, q.stem);
        assert_eq!(rebuilt.topics, vec!["arith".to_string()]);
        assert_eq!(rebuilt.explanation.as_deref(), Some("2+2 makes four"));
        assert_eq!(rebuilt.points, Some(2.0));
        assert_eq!(rebuilt.created, 1_700_000_000);
    }

    #[test]
    fn blank_validation_matches_blanks() {
        let mut d = Draft::new("bank".to_string());
        d.kind = Kind::Blank {
            accept: vec![vec!["one".to_string()], vec![]],
        };
        d.stem = "Fill ___ and ___ here.".to_string();
        let errs = d.validate();
        assert!(errs.iter().any(|e| e.contains("Blank 2")));
        d.kind = Kind::Blank {
            accept: vec![vec!["one".to_string()], vec!["two".to_string()]],
        };
        assert!(d.validate().is_empty());
    }

    #[test]
    fn picture_markers_preview_as_placeholders() {
        let t = "[picture: Handwritten key for NH3]\nNumber of charge clouds: 4\nShape: $x^2$";
        assert!(has_picture(t));
        assert!(renders_rich(t), "a picture marker gets a preview even without markup");
        let runs = preview_runs(t);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0], (None, "Handwritten key for NH3".to_string()));
        let (Some(at), run) = &runs[1] else { panic!("text run expected") };
        // The run's char offset maps a click on its equation back into the full text.
        assert_eq!(t.chars().skip(*at).take(run.chars().count()).collect::<String>(), *run);
        assert!(!has_picture("a [pictures] b"));
        assert!(!renders_rich("Number of charge clouds: 4"));
    }
}

// ---------- equation panel (opened by the sigma button, a word-initial '/', or a click on a chip) ----------

thread_local! {
    /// The one equation editor on screen (created when a request appears, dropped when it closes).
    static EQ_PANEL: std::cell::RefCell<Option<crate::equation::EquationEditor>> = const { std::cell::RefCell::new(None) };
}

/// The text of `f` inside the draft (the Blank rows live in the context-memory mirror instead).
fn field_text<'a>(draft: &'a mut Draft, f: &EqField) -> Option<&'a mut String> {
    match f {
        EqField::Stem => Some(&mut draft.stem),
        EqField::Mc(i) => match &mut draft.kind {
            Kind::Mc { answers } => answers.get_mut(*i).map(|a| &mut a.text),
            _ => None,
        },
        EqField::MatchL(i) => match &mut draft.kind {
            Kind::Matching { pairs, .. } => pairs.get_mut(*i).map(|p| &mut p.left),
            _ => None,
        },
        EqField::MatchR(i) => match &mut draft.kind {
            Kind::Matching { pairs, .. } => pairs.get_mut(*i).map(|p| &mut p.right),
            _ => None,
        },
        EqField::Distractor(i) => match &mut draft.kind {
            Kind::Matching { distractors, .. } => distractors.get_mut(*i),
            _ => None,
        },
        EqField::Short => match &mut draft.kind {
            Kind::Short { sample } => Some(sample),
            _ => None,
        },
        EqField::PartStem(i) => match &mut draft.kind {
            Kind::Multipart { parts } => parts.get_mut(*i).map(|p| &mut p.stem),
            _ => None,
        },
        EqField::PartMc(i, j) => match &mut draft.kind {
            Kind::Multipart { parts } => match parts.get_mut(*i).map(|p| &mut p.kind) {
                Some(PartKind::Mc { answers }) => answers.get_mut(*j).map(|a| &mut a.text),
                _ => None,
            },
            _ => None,
        },
        EqField::PartShort(i) => match &mut draft.kind {
            Kind::Multipart { parts } => match parts.get_mut(*i).map(|p| &mut p.kind) {
                Some(PartKind::Short { sample }) => Some(sample),
                _ => None,
            },
            _ => None,
        },
        EqField::Blank(_) => None,
    }
}

/// Splice `$linear$` into the requested field (replacing the clicked equation, if any).
fn apply_equation(ctx: &egui::Context, draft: &mut Draft, key: UiId, req: &EqReq, linear: &str) {
    let wrapped = crate::equation::wrap_math(linear);
    if let EqField::Blank(i) = req.field {
        ctx.data_mut(|d| {
            if let Some(t) = d.get_temp_mut_or_default::<Temp>(key).blank_texts.as_mut().and_then(|v| v.get_mut(i)) {
                *t = crate::equation::splice(t, req.span.clone(), req.caret, &wrapped);
            }
        });
    } else if let Some(t) = field_text(draft, &req.field) {
        *t = crate::equation::splice(t, req.span.clone(), req.caret, &wrapped);
    }
}

/// Shows the equaSHion editor as a modal while a field asked for it, and splices the finished
/// `$linear$` into that field at the remembered caret (or over the equation being edited).
fn equation_panel(ctx: &egui::Context, draft: &mut Draft, key: UiId) {
    let Some(req) = ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).eq_req.clone()) else {
        return;
    };
    let mut verdict: Option<bool> = None; // Some(true) = insert, Some(false) = cancel
    let linear = EQ_PANEL.with(|p| {
        let mut slot = p.borrow_mut();
        let ed = slot.get_or_insert_with(|| {
            crate::equation::EquationEditor::with_linear(&req.initial)
                .id_salt("queshion_equation_modal")
                .font_size(26.0)
                .canvas_height(110.0)
                .auto_focus(true)
        });
        let editing = req.span.is_some();
        egui::Modal::new(UiId::new("qedit_equation_modal")).show(ctx, |ui| {
            ui.set_width(640.0);
            kit::small_caps(ui, if editing { "Edit equation" } else { "Insert equation" });
            ui.add_space(2.0);
            ui.label(
                theme::muted("Type it the way you would say it: 1/2, sqrt(x), x^2, pi. Enter inserts, Esc cancels.").size(12.0),
            );
            ui.add_space(8.0);
            let r = ed.show(ui);
            if r.committed {
                verdict = Some(true);
            } else if r.cancelled {
                verdict = Some(false);
            }
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if kit::primary_button(ui, if editing { "Update equation" } else { "Insert equation" }).clicked() {
                    verdict = Some(true);
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    verdict = Some(false);
                }
            });
        });
        ed.linear()
    });
    if let Some(insert) = verdict {
        if insert && !linear.trim().is_empty() {
            apply_equation(ctx, draft, key, &req, linear.trim());
        }
        EQ_PANEL.with(|p| *p.borrow_mut() = None);
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).eq_req = None);
    }
}
