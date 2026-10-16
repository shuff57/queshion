//! Question editor: the `Draft` state the bank screens open, validation, and the
//! centered modal editor supporting all six question kinds (MC/MA, True/False,
//! Matching, Fill in the blank, Short answer, Multipart).
//! Bank-worker owned; `bankui::draw_bank_edit` triggers it via `draw_question_editor`.

use std::collections::HashMap;

use egui::{
    Align, Button, Color32, ComboBox, CornerRadius, Key, Layout, Margin, Modal,
    RichText, ScrollArea, TextEdit, Ui, Vec2,
};
// egui's widget id type vs the store's question id (a String).
use egui::Id as UiId;

use crate::model::{
    Answer, Difficulty, Id, ImageRef, Kind, Pair, Part, PartKind, QKind, Question,
};
use crate::{screens, theme, App};

const KINDS: [QKind; 6] = [
    QKind::Mc,
    QKind::Tf,
    QKind::Matching,
    QKind::Blank,
    QKind::Short,
    QKind::Multipart,
];

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
    ui.label(RichText::new(msg).color(theme::RED).size(12.0));
}

/// Card-ish secondary button (card fill, border, brown text).
fn ghost_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        Button::new(theme::text(label))
            .fill(theme::CARD)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .corner_radius(CornerRadius::same(6)),
    )
}

/// One click result the modal body reports.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum QAction {
    None,
    Save,
    Cancel,
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
    /// Is the topics sub-dialog open?
    topics_open: bool,
}

/// Context-memory key for the per-draft editor state.
fn temp_key(bank_id: &str, qid: Option<&Id>) -> UiId {
    UiId::new("testparrot_qedit_temp")
        .with(bank_id)
        .with(qid.map_or("", |s| s.as_str()))
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

/// A small square icon button (reorder / lock / remove / ✕).
fn icon_button(ui: &mut Ui, glyph: &str, fill: Option<Color32>) -> egui::Response {
    let mut b = Button::new(RichText::new(glyph).size(12.0))
        .min_size(Vec2::new(24.0, 24.0))
        .corner_radius(CornerRadius::same(6));
    if let Some(f) = fill {
        b = b.fill(f);
    }
    ui.add(b)
}

// ---------- the modal ----------

impl App {
    /// Draws the question editor modal when `qedit` is open. Save writes the draft
    /// into the bank; backdrop click, Esc, Cancel and ✕ all discard it.
    pub fn draw_question_editor(&mut self, ui: &mut egui::Ui) {
        if self.qedit.is_none() {
            return;
        }
        let ctx = ui.ctx();
        let bank_name = self
            .qedit
            .as_ref()
            .and_then(|d| self.store.bank(&d.bank_id))
            .map(|b| b.name.clone())
            .unwrap_or_else(|| "—".to_string());

        // Hand the media cache to the modal body (can't reach `self` inside it).
        let mut textures: HashMap<String, egui::TextureHandle> = std::mem::take(&mut self.textures);

        let mut draft = self.qedit.take().unwrap();

        let modal = Modal::new(UiId::new("testparrot_qedit_modal"))
            .show(ctx, |ui| editor_body(ui, ctx, &mut draft, &bank_name, &mut textures));

        let saved = modal.inner == QAction::Save;
        let keep = match modal.inner {
            QAction::Save => false,
            QAction::Cancel => false,
            QAction::None => !modal.should_close(), // backdrop click or Esc = cancel
        };
        if keep {
            self.qedit = Some(draft);
        } else if saved {
            self.commit_draft(draft);
        }
        self.textures = textures;
        // Insert-figure modal stacks on top of the editor.
        self.draw_figure_picker(ctx);
    }

    /// Writes a validated draft into the store and closes the editor.
    fn commit_draft(&mut self, draft: Draft) {
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

/// The whole modal body; returns what the user clicked.
fn editor_body(
    ui: &mut Ui,
    ctx: &egui::Context,
    draft: &mut Draft,
    bank_name: &str,
    textures: &mut HashMap<String, egui::TextureHandle>,
) -> QAction {
    let mut action = QAction::None;
    let key = temp_key(&draft.bank_id, draft.qid.as_ref());

    ui.set_min_width(620.0);
    ui.add_space(4.0);

    // Header.
    ui.horizontal(|ui| {
        ui.label(
            theme::serif_strong(if draft.qid.is_some() {
                "Edit question"
            } else {
                "Add question"
            })
            .size(21.0),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(Button::new(RichText::new("✕").size(14.0)).frame(false))
                .clicked()
            {
                action = QAction::Cancel;
            }
            ui.label(theme::muted(format!("in {bank_name}")).size(12.0));
        });
    });
    ui.add_space(8.0);

    // Scrollable editor body.
    let scroll_max = (ctx.viewport_rect().height() * 0.62).clamp(250.0, 480.0);
    ScrollArea::vertical()
        .max_height(scroll_max)
        .id_salt("qedit_scroll")
        .show(ui, |ui| {
            draw_editor(ui, ctx, draft, key, textures);
        });
    ui.add_space(6.0);

    // Validation errors (live; Save refuses to work while these show).
    let errs = draft.validate();
    if !errs.is_empty() {
        ui.vertical(|ui| {
            for e in &errs {
                red(ui, e);
            }
        });
        ui.add_space(4.0);
    }

    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if ui
            .add(
                Button::new(theme::on_accent("Save question"))
                    .fill(theme::ACCENT)
                    .corner_radius(CornerRadius::same(8)),
            )
            .clicked()
        {
            action = QAction::Save;
        }
        if ui.add(Button::new(theme::text("Cancel"))).clicked() {
            action = QAction::Cancel;
        }
    });
    action
}

// ---------- editor sections ----------

fn draw_editor(
    ui: &mut Ui,
    ctx: &egui::Context,
    draft: &mut Draft,
    key: UiId,
    textures: &mut HashMap<String, egui::TextureHandle>,
) {
    draw_meta_row(ui, ctx, draft, key);
    ui.add_space(8.0);

    ui.label(theme::muted("Question").size(11.0));
    ui.add(
        TextEdit::multiline(&mut draft.stem)
            .hint_text("Write the question…")
            .desired_width(ui.available_width())
            .desired_rows(3),
    );
    ui.add_space(8.0);

    // Per-kind editing.
    match &mut draft.kind {
        Kind::Mc { answers } => mc_editor(ui, answers, usize::MAX),
        Kind::Tf { correct } => {
            ui.horizontal(|ui| {
                ui.label(theme::muted("Correct answer:").size(12.0));
                ui.radio_value(correct, true, "True");
                ui.radio_value(correct, false, "False");
            });
        }
        Kind::Matching { pairs, distractors } => matching_editor(ui, pairs, distractors),
        Kind::Blank { .. } => blank_editor(ui, ctx, draft),
        Kind::Short { sample } => {
            ui.label(theme::muted("Sample answer").size(11.0));
            ui.add(
                TextEdit::multiline(sample)
                    .hint_text("What a good answer looks like…")
                    .desired_width(ui.available_width())
                    .desired_rows(2),
            );
        }
        Kind::Multipart { parts } => multipart_editor(ui, parts),
    }
    ui.add_space(8.0);

    draw_image_row(ui, draft, textures);
    ui.add_space(8.0);

    ui.label(theme::muted("Explanation").size(11.0));
    let mut explanation = draft.explanation.clone().unwrap_or_default();
    ui.add(
        TextEdit::multiline(&mut explanation)
            .hint_text("Optional explanation — shown with the answer key")
            .desired_width(ui.available_width())
            .desired_rows(2),
    );
    draft.explanation = match explanation.trim() {
        "" => None,
        s => Some(s.to_string()),
    };
}

/// Type / points / difficulty / topics row.
fn draw_meta_row(ui: &mut Ui, ctx: &egui::Context, draft: &mut Draft, key: UiId) {
    ui.horizontal(|ui| {
        ui.label(theme::muted("Type").size(11.0));
        let mut kind_sel = draft.qkind();
        ComboBox::from_id_salt("qedit_type")
            .selected_text(theme::text(kind_sel.label()).size(13.0))
            .show_ui(ui, |ui| {
                for k in KINDS {
                    ui.selectable_value(&mut kind_sel, k, k.label());
                }
            });
        if kind_sel != draft.qkind() {
            draft.set_kind(kind_sel);
            ctx.data_mut(|d| d.remove_temp::<Temp>(key));
        }

        ui.add_space(10.0);
        ui.label(theme::muted("Points").size(11.0));
        let mut pt = draft.points_text();
        let pt_resp = ui.add_sized([60.0, 24.0], TextEdit::singleline(&mut pt));
        if pt_resp.changed() {
            draft.set_points_text(&pt);
        }
        ui.label(theme::muted("pt").size(11.0));

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.menu_button(theme::muted("Difficulty ▾").size(12.0), |ui| {
                let options: [(Option<Difficulty>, &str); 4] = [
                    (None, "All (none)"),
                    (Some(Difficulty::Low), "Low"),
                    (Some(Difficulty::Medium), "Medium"),
                    (Some(Difficulty::High), "High"),
                ];
                for (d, label) in options {
                    let marked = draft.difficulty == d;
                    if ui
                        .add(
                            Button::new(if marked {
                                RichText::new(format!("✓ {label}")).color(theme::ACCENT)
                            } else {
                                RichText::new(label)
                            })
                            .frame(false),
                        )
                        .clicked()
                    {
                        draft.difficulty = d;
                        ui.close();
                    }
                }
            });
        });
    });

    // Topics row: committed topic chips + the editor button.
    ui.horizontal(|ui| {
        ui.label(theme::muted("Topics").size(11.0));
        for t in draft.topics.clone() {
            if ui.add(
                Button::new(RichText::new(format!("{t} ×")).size(11.0))
                    .fill(theme::ACCENT_FADED)
                    .corner_radius(CornerRadius::same(6)),
            ) .clicked()
            {
                draft.topics.retain(|x| x != &t);
            }
            ui.add_space(2.0);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let opened = ctx.data_mut(|d| {
                let t = d.get_temp_mut_or_default::<Temp>(key);
                if ui.button("Edit topics…").clicked() {
                    t.topics_open = true;
                }
                t.topics_open
            });
            if opened {
                draw_topics_dialog(ctx, draft, key);
            }
        });
    });
}

/// The topics popup: chips to remove, a text field committed on Enter.
fn draw_topics_dialog(ctx: &egui::Context, draft: &mut Draft, key: UiId) {
    let mut close = false;
    egui::Modal::new(UiId::new("testparrot_qedit_topics"))
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.horizontal(|ui| {
                ui.label(theme::serif_strong("Topics").size(17.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(Button::new(RichText::new("✕").size(13.0)).frame(false))
                        .clicked()
                    {
                        close = true;
                    }
                });
            });
            ui.add_space(4.0);

            if draft.topics.is_empty() {
                ui.label(theme::muted("No topics yet.").size(12.0));
            }
            let mut remove = None;
            for (i, t) in draft.topics.clone().into_iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(theme::strong(t).size(12.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(Button::new(RichText::new("✕").size(11.0)).frame(false))
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                draft.topics.remove(i);
            }
            ui.add_space(6.0);
            let committed = ctx.data_mut(|d| {
                let t = d.get_temp_mut_or_default::<Temp>(key);
                let r = ui.text_edit_singleline(&mut t.topics_text);
                let mut commit = false;
                if r.lost_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter))
                    && !t.topics_text.trim().is_empty()
                {
                    commit = true;
                }
                (commit, t.topics_text.clone())
            });
            if committed.0 {
                draft.topics.push(committed.1.trim().to_string());
                ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).topics_text.clear());
            }
            ui.label(
                theme::muted("Type a topic and press Enter. Click ✕ to remove.").size(11.0),
            );
            ui.add_space(8.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        Button::new(theme::on_accent("Done"))
                            .fill(theme::ACCENT)
                            .corner_radius(CornerRadius::same(8)),
                    )
                    .clicked()
                {
                    close = true;
                }
            });
        });
    if close {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).topics_open = false);
    }
}

/// Multiple choice / multiple answer. Every row has a correct checkbox (one tick
/// = single-choice behavior; two ticks make it a multiple-answer question),
/// a text field, reorder buttons and a lock toggle.
fn mc_editor(ui: &mut Ui, answers: &mut Vec<Answer>, max: usize) {
    let mut to_remove: Option<usize> = None;
    let mut move_up: Option<usize> = None;
    let n = answers.len();
    for i in 0..n {
        let locked = answers[i].locked;
        ui.horizontal_top(|ui| {
            ui.checkbox(&mut answers[i].correct, "");
            let width = (ui.available_width() - 136.0).max(160.0);
            ui.add(
                TextEdit::singleline(&mut answers[i].text)
                    .desired_width(width),
            );
            if icon_button(ui, "↑", None).clicked() && i > 0 {
                move_up = Some(i);
            }
            if icon_button(ui, "↓", None).clicked() && i + 1 < n {
                move_up = Some(i + 1);
            }
            let fill = locked.then_some(theme::ACCENT_FADED);
            if icon_button(ui, "🔒", fill).clicked() {
                answers[i].locked = !locked;
            }
            if n > 2
                && ui
                    .add(Button::new(RichText::new("✕").size(12.0)).frame(false))
                    .clicked()
            {
                to_remove = Some(i);
            }
        });
    }
    if let Some(i) = to_remove {
        answers.remove(i);
    }
    if let Some(i) = move_up {
        answers.swap(i, i - 1);
    }
    ui.add_space(2.0);
    // Parts are capped at six answer choices; top-level questions are not.
    if answers.len() < max && ghost_button(ui, "＋ Add answer").clicked() {
        answers.push(empty_answer());
    }
}

/// Matching pairs and extra distractors.
fn matching_editor(ui: &mut Ui, pairs: &mut Vec<Pair>, distractors: &mut Vec<String>) {
    let mut remove_pair: Option<usize> = None;
    ui.label(theme::muted("Pairs (left item — right item)").size(11.0));
    for i in 0..pairs.len() {
        ui.horizontal_top(|ui| {
            let width = (ui.available_width() - 92.0).max(120.0);
            ui.add_sized([width, 24.0], TextEdit::singleline(&mut pairs[i].left));
            ui.label(theme::muted("→").size(13.0));
            ui.add_sized([width, 24.0], TextEdit::singleline(&mut pairs[i].right));
            if ui
                .add(Button::new(RichText::new("✕").size(12.0)).frame(false))
                .clicked()
            {
                remove_pair = Some(i);
            }
        });
    }
    if let Some(i) = remove_pair {
        pairs.remove(i);
    }
    if ghost_button(ui, "＋ Add pair").clicked() {
        pairs.push(Pair {
            left: String::new(),
            right: String::new(),
        });
    }

    ui.add_space(6.0);
    ui.label(theme::muted("Distractors (extra right-side items)").size(11.0));
    let mut remove_dis: Option<usize> = None;
    for i in 0..distractors.len() {
        ui.horizontal_top(|ui| {
            ui.add(
                TextEdit::singleline(&mut distractors[i])
                    .desired_width((ui.available_width() - 40.0).max(160.0)),
            );
            if ui
                .add(Button::new(RichText::new("✕").size(12.0)).frame(false))
                .clicked()
            {
                remove_dis = Some(i);
            }
        });
    }
    if let Some(i) = remove_dis {
        distractors.remove(i);
    }
    if ghost_button(ui, "＋ Add distractor").clicked() {
        distractors.push(String::new());
    }
}

/// Fill in the blank: one accepted-alternatives row per "___" in the stem.
fn blank_editor(ui: &mut Ui, ctx: &egui::Context, draft: &mut Draft) {
    let key = temp_key(&draft.bank_id, draft.qid.as_ref());
    let blanks = blank_count(&draft.stem).max(1);

    ctx.data_mut(|d| {
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
        for i in 0..blanks {
            ui.label(theme::muted(format!("Blank {}", i + 1)).size(11.0));
            ui.add(
                TextEdit::singleline(&mut texts[i])
                    .hint_text("Accepted answer — separate alternatives with OR")
                    .desired_width(ui.available_width()),
            );
        }
        // Mirror → draft; the text the user types stays in the mirror.
        let accept = texts
            .iter()
            .map(|t| split_alts(t))
            .collect::<Vec<Vec<String>>>();
        draft.kind = Kind::Blank { accept };
    });
}

/// Multipart: each part has a stem, a kind selector and an expanded editor below.
fn multipart_editor(ui: &mut Ui, parts: &mut Vec<Part>) {
    let mut remove: Option<usize> = None;
    for i in 0..parts.len() {
        ui.horizontal_top(|ui| {
            ui.label(theme::muted(format!("{}.", i + 1)).size(12.0));
            ui.add(
                TextEdit::singleline(&mut parts[i].stem)
                    .hint_text("Part stem")
                    .desired_width((ui.available_width() - 150.0).max(160.0)),
            );
            let mut sel = match &parts[i].kind {
                PartKind::Mc { .. } => 0,
                PartKind::Tf { .. } => 1,
                PartKind::Short { .. } => 2,
            };
            ComboBox::from_id_salt(format!("qedit_part_kind_{i}"))
                .selected_text(theme::text(["MC", "T/F", "Short"][sel]))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut sel, 0, "MC");
                    ui.selectable_value(&mut sel, 1, "T/F");
                    ui.selectable_value(&mut sel, 2, "Short");
                });
            if sel
                != match &parts[i].kind {
                    PartKind::Mc { .. } => 0,
                    PartKind::Tf { .. } => 1,
                    PartKind::Short { .. } => 2,
                }
            {
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
            if parts.len() > 1
                && ui
                    .add(Button::new(RichText::new("✕").size(12.0)).frame(false))
                    .clicked()
            {
                remove = Some(i);
            }
        });
        ui.push_id(i, |ui| {
            ui.add_space(2.0);
            egui::Frame::new()
                .inner_margin(Margin::symmetric(18, 6))
                .show(ui, |ui| {
                    match &mut parts[i].kind {
                        PartKind::Mc { answers } => mc_editor(ui, answers, 6),
                        PartKind::Tf { correct } => {
                            ui.horizontal(|ui| {
                                ui.label(theme::muted("Correct:").size(11.0));
                                ui.radio_value(correct, true, "True");
                                ui.radio_value(correct, false, "False");
                            });
                        }
                        PartKind::Short { sample } => {
                            ui.add(
                                TextEdit::multiline(sample)
                                    .hint_text("Sample answer")
                                    .desired_width((ui.available_width() - 60.0).max(160.0))
                                    .desired_rows(1),
                            );
                        }
                    }
                });
        });
    }
    if let Some(i) = remove {
        parts.remove(i);
    }
    ui.add_space(2.0);
    if ghost_button(ui, "＋ Add part").clicked() {
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

    ui.horizontal(|ui| {
        ui.label(theme::muted("Image").size(11.0));
        if ghost_button(ui, "Attach image…").clicked()
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
        if draft.image.is_some() && ui.add(Button::new(theme::text("Remove"))).clicked() {
            draft.image = None;
            draft.figure = None;
        }
        if ghost_button(ui, "Insert figure…").clicked() {
            crate::figuresui::request(ui.ctx(), crate::figuresui::REQ_INSERT);
        }
        if draft.figure.is_some() && ghost_button(ui, "Edit figure").clicked() {
            crate::figuresui::request(ui.ctx(), crate::figuresui::REQ_EDIT);
        }
        if ghost_button(ui, "Import SVG/PNG…").clicked() {
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
        egui::Frame::new()
            .fill(theme::CREAM)
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.add(egui::Image::new(&tex).max_height(110.0));
            });
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
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
}