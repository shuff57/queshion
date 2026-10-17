//! Exam list, exam working-copy editor, export dialog and export history
//! (V4 "Study Studio in paper tones" restyle).
//!
//! - [`App::draw_exams`]: exam list, Cards/List toggle, New exam, rename/delete.
//! - [`App::draw_exam_edit`]: the editor. Top card (crumb, serif name, Saved, File /
//!   Edit / Format menus, undo, redo, Export history, Export), the always-visible
//!   Format bar, then the desk (table colour, near-white paper at 0.8 px per pt)
//!   and the 340 px question shelf. Shelf rows add with `+` or drag onto the page
//!   (dashed drop slot); page questions reorder by grip-drag or the margin buttons;
//!   every item-list change is undo/redo-snapshotted.
//! - [`App::draw_export_dialog`] / [`App::draw_history`]: PDF/DOCX export with
//!   shuffled named versions (deterministic LCG, no `rand`), answer-key docs, and
//!   regeneration from the stored `.doc.json` snapshots.
//!
//! Seams for other workers:
//! - Stem rendering lives in [`render_stem`] (equation worker swaps its body).
//! - The Format controls are shared between the always-visible bar and the popover
//!   alternative: flip [`FORMAT_POPOVER`] to move them into the Format menu.
//!
//! Local UI state lives in egui temp memory keyed by stable `egui::Id`s; persistent
//! state is `App` / `store` (saved exclusively through `store::save_*`).

use eframe::egui;
use egui::epaint::{Mesh, TextShape};
use egui::text::LayoutJob;
use egui::{
    Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Shape, Stroke,
    StrokeKind, UiBuilder, Vec2,
};
use crate::kit::{self, ViewMode};
use crate::model::{self, ExamItem, Id, Kind, PartKind, QKind};
use crate::{bankui, screens, store, theme};
type DocImg = crate::layout::DocImage;
type DocQ = crate::layout::DocQ;
type ExamDoc = crate::layout::ExamDoc;

/// `false`: the Format bar is always visible under the top card (main design).
/// `true`: the bar is hidden and the Format menu opens the same controls as a
/// popover (see `reference/v4-exam-popover-alt.dc.html`).
const FORMAT_POPOVER: bool = false;

/// Paper ink colours: the sheet stays near-white in both modes.
pub(crate) const PAPER_INK: Color32 = Color32::from_rgb(0x22, 0x22, 0x22);
pub(crate) const PAPER_SOFT: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
pub(crate) const PAPER_RULE: Color32 = Color32::from_rgb(0x9d, 0x8c, 0x7b);

/// Preview scale: px = pt * 0.8; US Letter page.
pub(crate) const PT: f32 = 0.8;
pub(crate) const PAGE_W: f32 = 612.0 * PT;
pub(crate) const PAGE_H: f32 = 792.0 * PT;
const SHELF_W: f32 = 340.0;
const HEAD_PT: [f32; 3] = [15.0, 18.0, 21.0]; // prints at 12 / 14.4 / 16.8 px on screen
const HEAD_LABELS: [&str; 3] = ["Small", "Medium", "Large"];
const MARGIN_MM: [f32; 3] = [19.05, 25.4, 31.75];
const MARGIN_LABELS: [&str; 3] = ["Narrow  0.75 in", "Normal  1 in", "Wide  1.25 in"];

fn list_id() -> egui::Id { egui::Id::NULL.with("examui_list") }
fn edit_id() -> egui::Id { egui::Id::NULL.with("examui_edit") }
fn export_id() -> egui::Id { egui::Id::NULL.with("examui_export") }

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `model::ExamItem` doesn't derive PartialEq; equality of item lists is just
/// pairwise id equality (a question may legally appear twice — this compares
/// positions, as reorder semantics need).
fn same_items(a: &[ExamItem], b: &[ExamItem]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(x, y)| x.bank_id == y.bank_id && x.question_id == y.question_id)
}

// ---------------------------------------------------------------------------
// Local UI state (egui temp memory)
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct ListUi {
    search: String,
    mode: ViewMode,
    confirm_delete: Option<Id>,
    rename: Option<(Id, String)>,
}

/// Where a drag started: a page question (reorder) or a shelf row (add).
#[derive(Clone)]
enum DragSrc {
    Page(usize),
    Shelf { bank_id: Id, question_id: Id },
}

#[derive(Clone)]
struct DragState {
    src: DragSrc,
    stem: String,
    label: &'static str,
}

#[derive(Clone, Default)]
struct EditUi {
    search: String,
    topics: Vec<String>,
    diff: u8, // 0 any, 1 low, 2 medium, 3 high
    sort_newest: bool,
    drag: Option<DragState>,
    /// Screen-space rects of the page items as laid out this frame (drop slot
    /// space removed), for drop targeting.
    rows: Vec<(usize, Rect)>,
    desk_rect: Option<Rect>,
    /// Insert position of the dashed drop slot while a shelf row is dragged.
    slot_k: Option<usize>,
    /// (item index, egui time) of the question that just landed (highlighter wash).
    just_added: Option<(usize, f64)>,
    /// Page count measured on the last frame.
    pages: usize,
    confirm_delete: bool,
    save_as_open: bool,
    save_as_name: String,
}

#[derive(Clone)]
struct ExportUi {
    docx: bool,
    student: bool,
    key: bool,
    shuffle_q: bool,
    shuffle_a: bool,
    versions: u32,
}
impl Default for ExportUi {
    fn default() -> Self {
        ExportUi {
            docx: false,
            student: true,
            key: true,
            shuffle_q: false,
            shuffle_a: false,
            versions: 2,
        }
    }
}

fn mem_load<T: Clone + Default + Send + Sync + 'static>(ctx: &egui::Context, id: egui::Id) -> T {
    ctx.data_mut(|d| d.get_temp::<T>(id)).unwrap_or_default()
}
fn mem_store<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, id: egui::Id, v: &T) {
    ctx.data_mut(|d| d.insert_temp::<T>(id, v.clone()));
}

// ---------------------------------------------------------------------------
// Small drawing helpers
// ---------------------------------------------------------------------------

fn card_frame(fill: egui::Color32) -> egui::Frame {
    egui::Frame::default()
        .fill(fill)
        .corner_radius(kit::R_FIELD)
        .inner_margin(egui::Margin::same(12))
        .stroke(egui::Stroke::new(1.0, theme::border()))
}

fn muted(rt: impl Into<egui::RichText>) -> egui::RichText {
    rt.into().color(theme::muted_col())
}

/// Destructive pill (confirm delete).
fn danger_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).family(theme::bold_family()).color(Color32::WHITE))
            .fill(theme::bad())
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(17))
            .min_size(Vec2::new(0.0, 34.0)),
    )
}

/// A/B/.. letters (wraps past Z as AA, AB, …).
fn letter(i: usize) -> String {
    if i < 26 {
        char::from(b'A' + i as u8).to_string()
    } else {
        format!("{}{}", letter((i / 26) - 1), letter(i % 26))
    }
}

fn version_letter(v: usize) -> String {
    letter(v)
}

/// The label a part stem already starts with ("a. NH3", "(b) ...", "ii) ...", "3. ..."),
/// without its punctuation. Imports often keep the source's own part letters in the stem.
fn own_part_label(stem: &str) -> Option<&str> {
    let t = stem.trim_start();
    let inner = t.strip_prefix('(').unwrap_or(t);
    let n = inner.chars().take_while(|c| c.is_ascii_alphanumeric()).count();
    if n == 0 || n > 4 {
        return None;
    }
    let core = &inner[..n];
    let ok = (n == 1 && core.chars().all(|c| c.is_ascii_alphabetic()))
        || core.chars().all(|c| matches!(c, 'i' | 'v' | 'x'))
        || (n <= 2 && core.chars().all(|c| c.is_ascii_digit()));
    let rest = &inner[n..];
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    (ok && (rest.is_empty() || rest.starts_with(char::is_whitespace))).then_some(core)
}

/// A multipart part as printed: labelled once (its own label when the stem has one, else a., b., ...).
fn part_line(pi: usize, stem: &str) -> String {
    if own_part_label(stem).is_some() {
        stem.trim_start().to_string()
    } else {
        format!("{}. {}", letter(pi).to_lowercase(), stem)
    }
}

/// A part split into its printed label ("a.") and the rest of its stem, for a hanging indent.
fn split_part_label(pi: usize, stem: &str) -> (String, String) {
    let t = stem.trim_start();
    if own_part_label(t).is_some() {
        let end = t.find(['.', ')']).map_or(0, |i| i + 1);
        return (t[..end].to_string(), t[end..].trim_start().to_string());
    }
    (format!("{}.", letter(pi).to_lowercase()), stem.to_string())
}

/// Ruled answer lines under a fill-in-the-blank item: one per `___` outside tables (a blank in a
/// table cell is written in the cell), at least one when the stem has no blank at all.
fn ruled_blanks(stem: &str) -> usize {
    let total = stem.matches("___").count();
    if total == 0 {
        return 1;
    }
    let in_tables: usize = crate::richtext::parse(stem)
        .iter()
        .map(|b| match b {
            crate::richtext::Block::Table { rows, .. } => rows
                .iter()
                .flatten()
                .map(|c| crate::richtext::line_text(c).matches("___").count())
                .sum(),
            _ => 0,
        })
        .sum();
    total.saturating_sub(in_tables)
}

/// The part's label for the answer key ("a" in "a) True").
fn part_key_label(pi: usize, stem: &str) -> String {
    own_part_label(stem).map(str::to_string).unwrap_or_else(|| letter(pi).to_lowercase())
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars).collect();
        out.push('…');
        out
    }
}

fn fmt_number(p: f64) -> String {
    if p.fract() == 0.0 {
        format!("{}", p as i64)
    } else {
        format!("{p:.1}")
    }
}

/// Galley clamped to `rows` rows with an ellipsis.
fn clamped_galley(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
    rows: usize,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::simple(text.to_string(), font, color, width.max(10.0));
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = false;
    job.wrap.overflow_character = Some('…');
    painter.layout_job(job)
}

fn rot(a: f32, v: Vec2) -> Vec2 {
    egui::emath::Rot2::from_angle(a) * v
}

/// A card tilted by `angle` radians around its centre with a text line: the
/// "picked up" look for drags and the exam-card thumbnails.
fn paint_tilted_card(
    painter: &egui::Painter,
    rect: Rect,
    angle: f32,
    label: &str,
    pill: Option<&str>,
) {
    let c = rect.center();
    let half = rect.size() / 2.0;
    let corners = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(half.x, half.y),
        Vec2::new(-half.x, half.y),
    ];
    let shadow: Vec<Pos2> = corners.iter().map(|d| c + rot(angle, *d) + Vec2::new(0.0, 9.0)).collect();
    painter.add(Shape::convex_polygon(shadow, Color32::from_black_alpha(38), Stroke::NONE));
    let pts: Vec<Pos2> = corners.iter().map(|d| c + rot(angle, *d)).collect();
    painter.add(Shape::convex_polygon(pts, theme::card(), Stroke::new(1.5, theme::accent_col())));
    let mut x = -half.x + 12.0;
    if let Some(p) = pill {
        let g = painter.layout_no_wrap(p.to_string(), FontId::new(9.5, theme::bold_family()), theme::accent_col());
        let pos = c + rot(angle, Vec2::new(x, -g.size().y / 2.0));
        painter.add(TextShape::new(pos, g.clone(), theme::accent_col()).with_angle(angle));
        x += g.size().x + 10.0;
    }
    let g = clamped_galley(painter, label, FontId::proportional(12.5), theme::ink(), rect.width() - (x + half.x) - 12.0, 1);
    let pos = c + rot(angle, Vec2::new(x, -g.size().y / 2.0));
    painter.add(TextShape::new(pos, g, theme::ink()).with_angle(angle));
}

/// Tiny tilted page thumbnail for exam cards.
fn paint_thumb(painter: &egui::Painter, c: Pos2, angle: f32, lines: usize) {
    let (w, h) = (62.0_f32, 82.0_f32);
    let corners = [
        Vec2::new(-w / 2.0, -h / 2.0),
        Vec2::new(w / 2.0, -h / 2.0),
        Vec2::new(w / 2.0, h / 2.0),
        Vec2::new(-w / 2.0, h / 2.0),
    ];
    let shadow: Vec<Pos2> = corners.iter().map(|d| c + rot(angle, *d) + Vec2::new(1.0, 5.0)).collect();
    painter.add(Shape::convex_polygon(shadow, Color32::from_black_alpha(34), Stroke::NONE));
    let pts: Vec<Pos2> = corners.iter().map(|d| c + rot(angle, *d)).collect();
    painter.add(Shape::convex_polygon(pts, theme::paper(), Stroke::new(1.0, theme::paper_edge())));
    let seg = |a: Vec2, b: Vec2, s: Stroke| {
        painter.line_segment([c + rot(angle, a), c + rot(angle, b)], s);
    };
    seg(Vec2::new(-14.0, -h / 2.0 + 12.0), Vec2::new(14.0, -h / 2.0 + 12.0), Stroke::new(2.0, PAPER_SOFT));
    for i in 0..lines.clamp(2, 7) {
        let y = -h / 2.0 + 24.0 + i as f32 * 8.0;
        let len = if i % 3 == 2 { 16.0 } else { 23.0 };
        seg(Vec2::new(-23.0, y), Vec2::new(-23.0 + len * 2.0 - 8.0, y), Stroke::new(1.4, PAPER_RULE.gamma_multiply(0.6)));
    }
}

// ---------------------------------------------------------------------------
// Exam list screen
// ---------------------------------------------------------------------------

fn new_exam(app: &mut crate::App) -> Id {
    let id = model::new_id();
    let exam = model::Exam {
        id: id.clone(),
        name: "Untitled Exam".to_string(),
        header: "Name: __________________ Class: ___________ Date: ___________"
            .to_string(),
        title: None,
        items: vec![],
        heading_pt: HEAD_PT[1],
        body_pt: 12.0,
        margin_mm: MARGIN_MM[1],
        ruled_blanks: 0,
        modified: now(),
    };
    app.store.exams.push(exam);
    app.store.save_exams();
    app.undo.clear();
    app.redo.clear();
    id
}

/// One exam's list facts.
struct ExamRow {
    id: Id,
    name: String,
    q: usize,
    pts: f64,
    bank: String,
    modified: i64,
}

impl crate::App {
    fn exam_rows(&self, needle: &str) -> Vec<ExamRow> {
        let needle = needle.trim().to_lowercase();
        let mut out: Vec<ExamRow> = self
            .store
            .exams
            .iter()
            .filter(|e| needle.is_empty() || e.name.to_lowercase().contains(&needle))
            .map(|e| {
                let pts: f64 = e
                    .items
                    .iter()
                    .filter_map(|it| self.store.question(&it.bank_id, &it.question_id).and_then(|q| q.points))
                    .sum();
                let mut banks: Vec<&Id> = Vec::new();
                for it in &e.items {
                    if !banks.contains(&&it.bank_id) {
                        banks.push(&it.bank_id);
                    }
                }
                let bank = match banks.len() {
                    0 => "None yet".to_string(),
                    1 => self.store.bank(banks[0]).map(|b| b.name.clone()).unwrap_or_else(|| "Missing bank".into()),
                    n => format!("{n} banks"),
                };
                ExamRow { id: e.id.clone(), name: e.name.clone(), q: e.items.len(), pts, bank, modified: e.modified }
            })
            .collect();
        out.sort_by(|a, b| b.modified.cmp(&a.modified));
        out
    }

    pub fn draw_exams(&mut self, ctx: &egui::Context) {
        let mut st: ListUi = mem_load(ctx, list_id());

        screens::shell_frame(self, ctx, true, |app, ui| {
            let total = app.store.exams.len();
            let total_q: usize = app.store.exams.iter().map(|e| e.items.len()).sum();
            let rows = app.exam_rows(&st.search);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(1040.0));
                let sub = format!(
                    "{total} exam{} · {total_q} question{} placed",
                    if total == 1 { "" } else { "s" },
                    if total_q == 1 { "" } else { "s" },
                );
                kit::page_header(ui, &["Home", "Exams"], "Exams", Some(&sub));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut st.search)
                            .hint_text("Search exams")
                            .desired_width(260.0)
                            .margin(Margin::symmetric(10, 6)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if kit::primary_button(ui, "New exam").clicked() {
                            let id = new_exam(app);
                            app.exam = Some(id.clone());
                            app.screen = crate::Screen::ExamEdit(id);
                        }
                        kit::view_toggle(ui, &mut st.mode);
                    });
                });
                ui.add_space(14.0);

                if rows.is_empty() {
                    kit::card().inner_margin(Margin::symmetric(24, 30)).show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.vertical_centered(|ui| {
                            if total == 0 {
                                ui.label(theme::serif("Create your first exam").size(20.0));
                                ui.add_space(4.0);
                                ui.label(muted("Pull questions into a printable working copy."));
                                ui.add_space(12.0);
                                if kit::primary_button(ui, "New exam").clicked() {
                                    let id = new_exam(app);
                                    app.exam = Some(id.clone());
                                    app.screen = crate::Screen::ExamEdit(id);
                                }
                            } else {
                                ui.label(muted("No exams match that search."));
                            }
                        });
                    });
                    return;
                }
                match st.mode {
                    ViewMode::Cards => draw_exam_cards(app, ui, &rows, &mut st),
                    ViewMode::List => draw_exam_list(app, ui, &rows, &mut st),
                }
            });
        });

        // Delete-confirm modal (raised from a card's "…" menu above).
        if let Some(id) = st.confirm_delete.clone() {
            match self.exams_name(&id) {
                None => st.confirm_delete = None,
                Some(name) => {
                    let mut open = true;
                    egui::Window::new(theme::strong("Delete exam"))
                        .open(&mut open)
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                        .default_width(420.0)
                        .show(ctx, |ui| {
                            ui.add(egui::Label::new(format!(
                                "Delete \"{name}\"?\nIts questions stay in their bank."
                            )));
                            ui.add_space(12.0);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if danger_button(ui, "Delete").clicked() {
                                        self.store.exams.retain(|e| e.id != id);
                                        self.store.save_exams();
                                        if self.exam.as_deref() == Some(&id) {
                                            self.exam = None;
                                        }
                                        self.undo.clear();
                                        self.redo.clear();
                                        screens::toast(self, "Exam deleted", false);
                                        st.confirm_delete = None;
                                    }
                                    if kit::ghost_button(ui, "Cancel").clicked() {
                                        st.confirm_delete = None;
                                    }
                                },
                            );
                        });
                    if !open {
                        st.confirm_delete = None;
                    }
                }
            }
        }

        // Rename modal.
        if let Some((id, rename_buf)) = st.rename.clone() {
            match self.exams_name(&id) {
                None => st.rename = None,
                Some(name) => {
                    let mut buf = rename_buf;
                    let mut open = true;
                    let mut done = false;
                    let mut cancel = false;
                    egui::Window::new(theme::strong("Rename exam"))
                        .open(&mut open)
                        .collapsible(false)
                        .resizable(false)
                        .default_width(400.0)
                        .show(ctx, |ui| {
                            ui.add(egui::Label::new(muted(
                                egui::RichText::new(format!("Rename \"{name}\"")).size(12.0),
                            )));
                            ui.add(egui::TextEdit::singleline(&mut buf).desired_width(340.0));
                            ui.add_space(10.0);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if kit::primary_button(ui, "Rename").clicked() {
                                        done = true;
                                    }
                                    if kit::ghost_button(ui, "Cancel").clicked() {
                                        cancel = true;
                                    }
                                },
                            );
                        });
                    if done || cancel || !open {
                        let new_name = buf.trim().to_string();
                        if done && !new_name.is_empty() {
                            if let Some(e) = self.store.exam_mut(&id) {
                                e.name = new_name;
                                e.modified = now();
                            }
                            self.store.save_exams();
                        }
                        st.rename = None;
                    } else {
                        st.rename = Some((id, buf));
                    }
                }
            }
        }

        mem_store(ctx, list_id(), &st);
    }

    fn exams_name(&self, id: &Id) -> Option<String> {
        self.store
            .exams
            .iter()
            .find(|e| e.id == *id)
            .map(|e| e.name.clone())
    }
}

fn open_exam_screen(app: &mut crate::App, id: &Id) {
    app.screen = crate::Screen::ExamEdit(id.clone());
    app.exam = Some(id.clone());
}

/// The "…" menu on a card/row.
fn exam_menu(ui: &mut egui::Ui, rect: Rect, row: &ExamRow, st: &mut ListUi) {
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().button_padding = Vec2::new(6.0, 2.0);
        ui.menu_button("…", |ui| {
            if ui.button("Rename").clicked() {
                st.rename = Some((row.id.clone(), row.name.clone()));
            }
            ui.add_space(4.0);
            if ui
                .add(egui::Button::new(egui::RichText::new("Delete").color(theme::bad())))
                .clicked()
            {
                st.confirm_delete = Some(row.id.clone());
            }
        });
    });
}

fn draw_exam_cards(app: &mut crate::App, ui: &mut egui::Ui, rows: &[ExamRow], st: &mut ListUi) {
    let gap = 14.0;
    let avail = ui.available_width();
    let n = (((avail + gap) / (300.0 + gap)).floor() as usize).max(1);
    let card_w = ((avail - gap * (n as f32 - 1.0)) / n as f32).floor();
    let card_h = 142.0;
    ui.spacing_mut().item_spacing = Vec2::splat(gap);
    for chunk in rows.chunks(n) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(gap);
        for row in chunk {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::click());
            let p = ui.painter();
            p.rect(
                rect,
                CornerRadius::same(kit::R_CARD),
                theme::card(),
                Stroke::new(1.0, if resp.hovered() { theme::accent_col() } else { theme::border() }),
                StrokeKind::Inside,
            );
            paint_thumb(p, rect.left_top() + Vec2::new(14.0 + 36.0, 14.0 + 46.0), -0.05, row.q.max(2));
            let tx = rect.left() + 14.0 + 78.0 + 12.0;
            let tw = rect.right() - 36.0 - tx;
            let g = clamped_galley(p, &row.name, FontId::new(16.0, theme::serif_family()), theme::ink(), tw, 2);
            let gh = g.size().y;
            p.galley(Pos2::new(tx, rect.top() + 14.0), g, theme::ink());
            let facts = [
                ("Questions", row.q.to_string()),
                ("Points", fmt_number(row.pts)),
                ("Bank", truncate_str(&row.bank, 18)),
                ("Edited", bankui::fmt_date(row.modified)),
            ];
            let mut y = rect.top() + 14.0 + gh + 6.0;
            for (k, v) in facts {
                p.text(Pos2::new(tx, y), Align2::LEFT_TOP, k, FontId::proportional(11.5), theme::muted_col());
                p.text(Pos2::new(tx + 66.0, y), Align2::LEFT_TOP, v, FontId::new(11.5, theme::bold_family()), theme::ink());
                y += 15.0;
            }
            if resp.clicked() {
                open_exam_screen(app, &row.id);
            }
            exam_menu(
                ui,
                Rect::from_min_size(Pos2::new(rect.right() - 36.0, rect.top() + 8.0), Vec2::new(30.0, 22.0)),
                row,
                st,
            );
        }
    });
    }
}

fn draw_exam_list(app: &mut crate::App, ui: &mut egui::Ui, rows: &[ExamRow], st: &mut ListUi) {
    kit::card().inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
        let w = ui.available_width();
        // Column x offsets (fractions of the row width).
        let cols = [0.0, 0.42, 0.55, 0.65, 0.84];
        let head = ["Exam", "Questions", "Points", "Bank", "Edited"];
        let (hr, _) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::hover());
        for (i, h) in head.iter().enumerate() {
            let g = kit::small_caps_text(h);
            ui.painter().text(
                Pos2::new(hr.left() + 10.0 + w * cols[i], hr.center().y),
                Align2::LEFT_CENTER,
                h.to_uppercase(),
                FontId::new(10.0, theme::bold_family()),
                theme::muted_col(),
            );
            let _ = g;
        }
        ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], Stroke::new(1.0, theme::border()));
        for row in rows {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 46.0), Sense::click());
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(rect, CornerRadius::same(10), theme::hover_bg());
            }
            let cy = rect.center().y;
            let x = |i: usize| rect.left() + 10.0 + w * cols[i];
            let name_w = w * (cols[1] - cols[0]) - 20.0;
            let g = clamped_galley(p, &row.name, FontId::new(15.0, theme::serif_family()), theme::ink(), name_w, 1);
            p.galley(Pos2::new(x(0), cy - g.size().y / 2.0), g, theme::ink());
            let f = FontId::proportional(12.5);
            p.text(Pos2::new(x(1), cy), Align2::LEFT_CENTER, row.q.to_string(), f.clone(), theme::text_col());
            p.text(Pos2::new(x(2), cy), Align2::LEFT_CENTER, fmt_number(row.pts), f.clone(), theme::text_col());
            p.text(Pos2::new(x(3), cy), Align2::LEFT_CENTER, truncate_str(&row.bank, 22), f.clone(), theme::text_col());
            p.text(Pos2::new(x(4), cy), Align2::LEFT_CENTER, bankui::fmt_date(row.modified), f, theme::muted_col());
            if resp.clicked() {
                open_exam_screen(app, &row.id);
            }
            exam_menu(
                ui,
                Rect::from_min_size(Pos2::new(rect.right() - 36.0, cy - 11.0), Vec2::new(30.0, 22.0)),
                row,
                st,
            );
        }
    });
}

// ---------------------------------------------------------------------------
// Exam editor
// ---------------------------------------------------------------------------

impl crate::App {
    pub fn draw_exam_edit(&mut self, ctx: &egui::Context) {
        let exam_id: Option<Id> = match &self.screen {
            crate::Screen::ExamEdit(id) => Some(id.clone()),
            _ => None,
        };
        let Some(exam_id) = exam_id else { return };
        if self.store.exam(&exam_id).is_none() {
            // Deleted out from under the editor.
            self.screen = crate::Screen::Exams;
            self.exam = None;
            return;
        }

        let mut st: EditUi = mem_load(ctx, edit_id());

        // Default the shelf to a bank this exam already draws from.
        if self.panel_bank.is_none() || self.panel_bank.as_ref().and_then(|b| self.store.bank(b)).is_none() {
            let from_exam = self
                .store
                .exam(&exam_id)
                .and_then(|e| e.items.first().map(|i| i.bank_id.clone()))
                .filter(|b| self.store.bank(b).is_some());
            self.panel_bank = from_exam.or_else(|| self.store.banks.first().map(|b| b.id.clone()));
        }

        // Keyboard shortcuts (never while a text field or the equation widget has focus).
        if !ctx.egui_wants_keyboard_input() && !self.export_open {
            use egui::{Key, KeyboardShortcut, Modifiers};
            let (redo, undo, export) = ctx.input_mut(|i| {
                let redo = i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))
                    || i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Y));
                let undo = i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z));
                let export = i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::E));
                (redo, undo, export)
            });
            if redo {
                do_redo(self, &exam_id);
            } else if undo {
                do_undo(self, &exam_id);
            }
            if export {
                self.export_open = true;
            }
        }

        screens::shell_frame(self, ctx, true, |app, ui| {
            draw_editor(app, ui, ctx, &exam_id, &mut st);
        });

        // File-menu Delete (confirm) modal on `self`.
        if st.confirm_delete {
            let name = self.exams_name(&exam_id).unwrap_or_default();
            let mut open = true;
            egui::Window::new(theme::strong("Delete exam"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(420.0)
                .show(ctx, |ui| {
                    ui.add(egui::Label::new(format!(
                        "Delete \"{name}\"? Its questions stay in their bank."
                    )));
                    ui.add_space(12.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if danger_button(ui, "Delete").clicked() {
                            self.store.exams.retain(|e| e.id != exam_id);
                            self.store.save_exams();
                            self.exam = None;
                            self.screen = crate::Screen::Exams;
                            self.undo.clear();
                            self.redo.clear();
                            screens::toast(self, "Exam deleted", false);
                            st.confirm_delete = false;
                        }
                        if kit::ghost_button(ui, "Cancel").clicked() {
                            st.confirm_delete = false;
                        }
                    });
                });
            if !open {
                st.confirm_delete = false;
            }
        }

        // File-menu Save As… modal.
        if st.save_as_open {
            let cur = self.exams_name(&exam_id).unwrap_or_default();
            if st.save_as_name.is_empty() {
                st.save_as_name = cur;
            }
            let mut buf = st.save_as_name.clone();
            let mut open = true;
            let mut done = false;
            let mut cancel = false;
            egui::Window::new(theme::strong("Save as…"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.add(egui::Label::new(muted(
                        "Saves a copy of this exam under a new name.".to_string(),
                    )));
                    ui.add(egui::TextEdit::singleline(&mut buf).desired_width(340.0));
                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if kit::primary_button(ui, "Save").clicked() {
                            done = true;
                        }
                        if kit::ghost_button(ui, "Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            st.save_as_name = buf;
            if done {
                let name = st.save_as_name.trim().to_string();
                if !name.is_empty() {
                    if let Some(src) = self.store.exam(&exam_id) {
                        let mut copy = src.clone();
                        copy.id = model::new_id();
                        copy.name = name;
                        copy.modified = now();
                        let new_id = copy.id.clone();
                        self.store.exams.push(copy);
                        self.store.save_exams();
                        self.screen = crate::Screen::ExamEdit(new_id.clone());
                        self.exam = Some(new_id);
                        self.undo.clear();
                        self.redo.clear();
                    }
                    st.save_as_open = false;
                }
            }
            if !open || cancel {
                st.save_as_open = false;
            }
        }

        if self.export_open {
            self.draw_export_dialog(ctx, &exam_id);
        }
        if self.history_open {
            self.draw_history(ctx);
        }

        mem_store(ctx, edit_id(), &st);
    }
}

fn draw_editor(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
) {
    draw_top_card(app, ui, exam_id, st);
    ui.add_space(10.0);
    if !FORMAT_POPOVER {
        draw_format_bar(app, ui, exam_id, st);
        ui.add_space(10.0);
    }

    let rest = ui.available_rect_before_wrap();
    let shelf_w = SHELF_W.min((rest.width() * 0.4).max(260.0));
    let shelf_rect = Rect::from_min_max(Pos2::new(rest.right() - shelf_w, rest.top()), rest.right_bottom());
    let desk_rect = Rect::from_min_max(rest.left_top(), Pos2::new(shelf_rect.left() - 12.0, rest.bottom()));
    st.desk_rect = Some(desk_rect);
    ui.allocate_rect(rest, Sense::hover());

    // Desk: the table colour (with grid) behind the paper.
    {
        let p = ui.painter();
        p.rect_filled(desk_rect, CornerRadius::same(18), theme::table());
        theme::paint_grid(&p.with_clip_rect(desk_rect.shrink(6.0)), desk_rect);
        p.rect_stroke(desk_rect, CornerRadius::same(18), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
    }
    ui.scope_builder(UiBuilder::new().max_rect(desk_rect.shrink(1.0)), |ui| {
        ui.set_clip_rect(desk_rect.shrink(1.0));
        draw_page(app, ui, ctx, exam_id, st);
    });
    ui.scope_builder(UiBuilder::new().max_rect(shelf_rect), |ui| {
        draw_shelf(app, ui, ctx, exam_id, st, shelf_rect);
    });

    drag_overlay(app, ctx, exam_id, st);
}

// ----- top card -------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Ico {
    Undo,
    Redo,
}

/// 32 px round outline button with a painted curved arrow.
fn icon_circle(ui: &mut egui::Ui, ico: Ico, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    let hov = enabled && resp.hovered();
    let col = if !enabled {
        theme::muted_col().gamma_multiply(0.5)
    } else if hov {
        theme::accent_col()
    } else {
        theme::text_col()
    };
    let p = ui.painter();
    p.circle(
        rect.center(),
        15.5,
        if hov { theme::hover_bg() } else { theme::card() },
        Stroke::new(1.0, if hov { theme::accent_col() } else { theme::border() }),
    );
    let s = if ico == Ico::Undo { 1.0 } else { -1.0 };
    let c = rect.center();
    let pt = |x: f32, y: f32| c + Vec2::new(x * s, y);
    let w = Stroke::new(1.7, col);
    p.line_segment([pt(-3.0, -5.5), pt(-7.0, -1.5)], w);
    p.line_segment([pt(-7.0, -1.5), pt(-3.0, 2.5)], w);
    p.add(Shape::line(
        vec![pt(-7.0, -1.5), pt(1.0, -1.5), pt(5.0, 0.5), pt(5.5, 3.0), pt(3.5, 5.5), pt(0.5, 6.0), pt(-2.0, 6.0)],
        w,
    ));
    resp
}

fn draw_top_card(app: &mut crate::App, ui: &mut egui::Ui, exam_id: &Id, st: &mut EditUi) {
    kit::card()
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            // Narrow windows: drop "Saved" text, Export history (it lives in File) and shrink the name.
            let compact = ui.available_width() < 980.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                if ui
                    .add(egui::Button::new(muted(RichText::new("Exams").size(12.0))).frame(false))
                    .clicked()
                {
                    app.screen = crate::Screen::Exams;
                    app.exam = None;
                }
                ui.label(muted(RichText::new("/").size(12.0)));

                let mut name = app.store.exam(exam_id).map(|e| e.name.clone()).unwrap_or_default();
                let before = name.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut name)
                        .hint_text("Untitled Exam")
                        .font(FontId::new(19.0, theme::serif_family()))
                        .text_color(theme::ink())
                        .frame(egui::Frame::NONE)
                        .desired_width(if compact { 190.0 } else { 260.0 }),
                );
                if name != before {
                    if let Some(e) = app.store.exam_mut(exam_id) {
                        e.name = name;
                        e.modified = now();
                    }
                    app.store.save_exams();
                }

                // Saved indicator (every edit is written straight through).
                let (sr, _) = ui.allocate_exact_size(Vec2::new(13.0, 16.0), Sense::hover());
                let sc = sr.center();
                let w = Stroke::new(1.8, theme::accent_col());
                ui.painter().line_segment([sc + Vec2::new(-4.5, 0.5), sc + Vec2::new(-1.5, 3.5)], w);
                ui.painter().line_segment([sc + Vec2::new(-1.5, 3.5), sc + Vec2::new(4.5, -3.5)], w);
                if !compact {
                    ui.label(RichText::new("Saved").family(theme::bold_family()).size(12.0).color(theme::accent_col()));
                }
                ui.add_space(4.0);

                draw_menus(app, ui, exam_id, st);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if kit::primary_button(ui, "Export").clicked() {
                        app.export_open = true;
                    }
                    if !compact && kit::ghost_button(ui, "Export history").clicked() {
                        app.history_open = true;
                    }
                    let redo = icon_circle(ui, Ico::Redo, !app.redo.is_empty()).on_hover_text("Redo (Ctrl+Shift+Z)");
                    if redo.clicked() && !app.redo.is_empty() {
                        do_redo(app, exam_id);
                    }
                    let undo = icon_circle(ui, Ico::Undo, !app.undo.is_empty()).on_hover_text("Undo (Ctrl+Z)");
                    if undo.clicked() && !app.undo.is_empty() {
                        do_undo(app, exam_id);
                    }
                });
            });
        });
}

fn draw_menus(app: &mut crate::App, ui: &mut egui::Ui, exam_id: &Id, st: &mut EditUi) {
    ui.spacing_mut().item_spacing.x = 2.0;
    ui.menu_button("File", |ui| {
        if ui.button("Open exam").clicked() {
            app.screen = crate::Screen::Exams;
        }
        if ui.button("Save as…").clicked() {
            st.save_as_open = true;
            st.save_as_name.clear();
        }
        if ui.button("Export").clicked() {
            app.export_open = true;
        }
        if ui.button("Export history").clicked() {
            app.history_open = true;
        }
        ui.add_space(4.0);
        if ui
            .add(egui::Button::new(egui::RichText::new("Delete exam").color(theme::bad())))
            .clicked()
        {
            st.confirm_delete = true;
        }
    });
    ui.menu_button("Edit", |ui| {
        if ui
            .add_enabled(!app.undo.is_empty(), egui::Button::new("Undo"))
            .clicked()
        {
            do_undo(app, exam_id);
        }
        if ui
            .add_enabled(!app.redo.is_empty(), egui::Button::new("Redo"))
            .clicked()
        {
            do_redo(app, exam_id);
        }
    });
    ui.menu_button("Format", |ui| {
        ui.set_min_width(300.0);
        kit::small_caps(ui, "Page format");
        ui.add_space(6.0);
        let mut changed = false;
        if let Some(ex) = app.store.exam(exam_id) {
            let mut f = ex.clone();
            for (label, which) in [("Heading", 0), ("Text size", 1), ("Margins", 2), ("Answer blanks", 3)] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).family(theme::bold_family()).color(theme::text_col()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        changed |= format_control(ui, which, &mut f, "menu");
                    });
                });
                ui.add_space(4.0);
            }
            if changed {
                write_format(app, exam_id, &f);
            }
        }
    });
}

// ----- format bar -----------------------------------------------------------

fn head_idx(pt: f32) -> usize {
    if pt < 16.5 { 0 } else if pt < 19.5 { 1 } else { 2 }
}

fn margin_idx(mm: f32) -> usize {
    if mm < 22.0 { 0 } else if mm < 28.5 { 1 } else { 2 }
}

fn write_format(app: &mut crate::App, exam_id: &Id, f: &model::Exam) {
    if let Some(ex) = app.store.exam_mut(exam_id) {
        ex.heading_pt = f.heading_pt;
        ex.body_pt = f.body_pt;
        ex.margin_mm = f.margin_mm;
        ex.ruled_blanks = f.ruled_blanks;
        ex.modified = now();
    }
    app.store.save_exams();
}

/// One format control (0 heading, 1 text size, 2 margins, 3 answer blanks).
/// Shared by the Format bar, the Format menu and the popover alternative.
fn format_control(ui: &mut egui::Ui, which: u8, f: &mut model::Exam, salt: &str) -> bool {
    let mut changed = false;
    match which {
        0 => {
            let cur = head_idx(f.heading_pt);
            let mut sel = cur;
            egui::ComboBox::from_id_salt(("fmt_heading", salt))
                .selected_text(HEAD_LABELS[cur])
                .width(96.0)
                .show_ui(ui, |ui| {
                    for (i, l) in HEAD_LABELS.iter().enumerate() {
                        ui.selectable_value(&mut sel, i, *l);
                    }
                });
            if sel != cur {
                f.heading_pt = HEAD_PT[sel];
                changed = true;
            }
        }
        1 => {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(104.0, 28.0), Sense::hover());
            let minus = Rect::from_min_size(rect.min, Vec2::new(30.0, 28.0));
            let plus = Rect::from_min_size(rect.right_top() - Vec2::new(30.0, 0.0), Vec2::new(30.0, 28.0));
            let r_minus = ui.interact(minus, ui.id().with(("fmt_minus", salt)), Sense::click());
            let r_plus = ui.interact(plus, ui.id().with(("fmt_plus", salt)), Sense::click());
            let p = ui.painter();
            p.rect(rect, CornerRadius::same(14), theme::card(), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
            for (r, resp, is_plus) in [(minus, &r_minus, false), (plus, &r_plus, true)] {
                if resp.hovered() {
                    p.rect_filled(r.shrink(2.0), CornerRadius::same(12), theme::hover_bg());
                }
                let c = r.center();
                let s = Stroke::new(1.8, theme::ink());
                p.line_segment([c + Vec2::new(-4.0, 0.0), c + Vec2::new(4.0, 0.0)], s);
                if is_plus {
                    p.line_segment([c + Vec2::new(0.0, -4.0), c + Vec2::new(0.0, 4.0)], s);
                }
            }
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                format!("{} pt", f.body_pt.round() as i32),
                FontId::new(12.5, theme::bold_family()),
                theme::ink(),
            );
            if r_minus.clicked() && f.body_pt > 9.0 {
                f.body_pt = (f.body_pt.round() - 1.0).max(9.0);
                changed = true;
            }
            if r_plus.clicked() && f.body_pt < 14.0 {
                f.body_pt = (f.body_pt.round() + 1.0).min(14.0);
                changed = true;
            }
        }
        2 => {
            let cur = margin_idx(f.margin_mm);
            let mut sel = cur;
            egui::ComboBox::from_id_salt(("fmt_margin", salt))
                .selected_text(MARGIN_LABELS[cur])
                .width(130.0)
                .show_ui(ui, |ui| {
                    for (i, l) in MARGIN_LABELS.iter().enumerate() {
                        ui.selectable_value(&mut sel, i, *l);
                    }
                });
            if sel != cur {
                f.margin_mm = MARGIN_MM[sel];
                changed = true;
            }
        }
        _ => {
            let cur = f.ruled_blanks.min(6) as usize;
            let mut sel = cur;
            let label = |n: usize| match n {
                0 => "None".to_string(),
                1 => "1 ruled line".to_string(),
                n => format!("{n} ruled lines"),
            };
            egui::ComboBox::from_id_salt(("fmt_blanks", salt))
                .selected_text(label(cur))
                .width(120.0)
                .show_ui(ui, |ui| {
                    for n in 0..=6usize {
                        ui.selectable_value(&mut sel, n, label(n));
                    }
                });
            if sel != cur {
                f.ruled_blanks = sel as u8;
                changed = true;
            }
        }
    }
    changed
}

fn draw_format_bar(app: &mut crate::App, ui: &mut egui::Ui, exam_id: &Id, st: &EditUi) {
    let Some(ex) = app.store.exam(exam_id) else { return };
    let mut f = ex.clone();
    let n_q = f.items.len();
    let pts: f64 = f
        .items
        .iter()
        .filter_map(|it| app.store.question(&it.bank_id, &it.question_id).and_then(|q| q.points))
        .sum();
    let mut changed = false;
    egui::Frame::new()
        .fill(theme::field())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::symmetric(14, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let summary = format!(
                "Page 1 of {} · {n_q} question{} · {} pts",
                st.pages.max(1),
                if n_q == 1 { "" } else { "s" },
                fmt_number(pts)
            );
            // Wide: one row with the summary pushed right. Narrow: controls wrap, summary last.
            let wide = ui.available_width() > 1000.0;
            let mut body = |ui: &mut egui::Ui, range: std::ops::Range<usize>| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (label, which) in [("Heading", 0u8), ("Text size", 1), ("Margins", 2), ("Answer blanks", 3)]
                    .into_iter()
                    .skip(range.start)
                    .take(range.len())
                {
                    ui.label(muted(RichText::new(label).size(11.5)));
                    changed |= format_control(ui, which, &mut f, "bar");
                    ui.add_space(10.0);
                }
            };
            if wide {
                ui.horizontal(|ui| {
                    body(ui, 0..4);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(muted(RichText::new(&summary).size(12.0)));
                    });
                });
            } else {
                ui.horizontal(|ui| body(ui, 0..3));
                ui.horizontal(|ui| {
                    body(ui, 3..4);
                    ui.label(muted(RichText::new(&summary).size(12.0)));
                });
            }
        });
    if changed {
        write_format(app, exam_id, &f);
    }
}

// ---------------------------------------------------------------------------
// Desk: paper preview
// ---------------------------------------------------------------------------

pub(crate) fn ptext(s: impl Into<String>, px: f32, bold: bool) -> RichText {
    RichText::new(s.into())
        .family(if bold { theme::serif_bold_family() } else { theme::serif_family() })
        .size(px)
        .color(PAPER_INK)
        .line_height(Some(px * 1.45))
}

/// One printed-looking ruled line (1.7 em tall).
fn ruled_line(ui: &mut egui::Ui, width: f32, bp: f32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width.min(ui.available_width()), bp * 1.7),
        egui::Sense::hover(),
    );
    let y = rect.bottom() - 2.0;
    ui.painter().line_segment(
        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
        egui::Stroke::new(0.9, PAPER_RULE),
    );
}

/// True when a stem (or option) needs the structured renderer: markup (`crate::richtext`) or math.
/// Everything else keeps the original single-label layout exactly.
fn needs_rich(s: &str) -> bool {
    crate::richtext::has_markup(s) || s.contains('*')
}

/// THE stem renderer. Everything that prints a question stem on the preview page goes through
/// here: structured stems (tables, lists, line breaks, bold/italic, sub/sup, `$math$`) via
/// `equation::rich_ui`; plain one-line stems exactly as before.
/// `has_image`: the question's attached picture stands in for its first `[picture]` marker
/// (it is drawn under the stem), so that marker gets no "Picture needed" box.
fn render_stem(ui: &mut egui::Ui, stem: &str, points: Option<f64>, bp: f32, has_image: bool) {
    if stem.contains("[picture") {
        use crate::bankops::StemPart;
        let parts = crate::bankops::split_pictures(stem);
        if parts.iter().any(|p| matches!(p, StemPart::Picture(_))) {
            let mut pts = points;
            let mut covered = has_image;
            for (i, part) in parts.iter().enumerate() {
                match part {
                    StemPart::Text(t) => render_stem_one(ui, t, pts.take(), bp),
                    StemPart::Picture(alt) => {
                        if i == 0 {
                            if let Some(p) = pts.take() {
                                render_stem_one(ui, "", Some(p), bp);
                            }
                        }
                        if std::mem::take(&mut covered) {
                            continue;
                        }
                        ui.add_space(bp * 0.35);
                        picture_needed_box(ui, bp, alt.as_deref());
                        ui.add_space(bp * 0.35);
                    }
                }
            }
            return;
        }
    }
    render_stem_one(ui, stem, points, bp);
}

/// Dashed "Picture needed" box standing in for a `[picture]` marker an importer left in a
/// stem (the image itself did not come along). Add the picture in the question editor.
pub(crate) fn picture_needed_box(ui: &mut egui::Ui, bp: f32, alt: Option<&str>) {
    let w = ui.available_width();
    let h = (bp * 3.6).max(36.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter();
    let red = Color32::from_rgb(0xb0, 0x3a, 0x2e);
    p.rect_filled(rect, CornerRadius::same(3), Color32::from_rgba_unmultiplied(0xc0, 0x40, 0x30, 22));
    p.extend(Shape::dashed_line(
        &[rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()],
        Stroke::new(1.0, red),
        4.0,
        3.0,
    ));
    let cy = if alt.is_some() { rect.center().y - bp * 0.45 } else { rect.center().y };
    p.text(Pos2::new(rect.center().x, cy), Align2::CENTER_CENTER, "Picture needed", FontId::new(bp * 0.95, theme::serif_bold_family()), red);
    if let Some(a) = alt {
        let a: String = if a.chars().count() > 70 { a.chars().take(70).chain(std::iter::once('…')).collect() } else { a.to_string() };
        p.text(Pos2::new(rect.center().x, cy + bp * 1.05), Align2::CENTER_CENTER, a, FontId::new(bp * 0.78, theme::serif_family()), PAPER_SOFT);
    }
}

fn render_stem_one(ui: &mut egui::Ui, stem: &str, points: Option<f64>, bp: f32) {
    if needs_rich(stem) {
        let mut o = crate::equation::Rich::new(bp, PAPER_INK).leading(0.32);
        if let Some(p) = points {
            o = o.lead(format!("({} pt{}) ", fmt_number(p), if p == 1.0 { "" } else { "s" }), bp * 0.92, PAPER_SOFT);
        }
        crate::equation::rich_ui(ui, stem, &o);
        return;
    }
    let mut job = LayoutJob::default();
    let lh = Some(bp * 1.45);
    if let Some(p) = points {
        job.append(
            &format!("({} pt{}) ", fmt_number(p), if p == 1.0 { "" } else { "s" }),
            0.0,
            egui::TextFormat {
                font_id: FontId::new(bp * 0.92, theme::serif_family()),
                color: PAPER_SOFT,
                line_height: lh,
                ..Default::default()
            },
        );
    }
    job.append(
        stem,
        0.0,
        egui::TextFormat {
            font_id: FontId::new(bp, theme::serif_family()),
            color: PAPER_INK,
            line_height: lh,
            ..Default::default()
        },
    );
    ui.add(egui::Label::new(job).wrap());
}

fn draw_page(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
) {
    st.rows.clear();
    let Some(exam) = app.store.exam(exam_id).cloned() else { return };
    egui::ScrollArea::vertical()
        .id_salt("page_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let aw = ui.available_width();
            let paper_w = PAGE_W.min((aw - 40.0).max(260.0));
            let inset = ((aw - paper_w) / 2.0).max(0.0);
            let bp = exam.body_pt * PT;
            let side = (exam.margin_mm / 25.4 * 72.0 * PT).clamp(24.0, (paper_w - 180.0) / 2.0);
            let cw = paper_w - side * 2.0;

            ui.add_space(22.0);
            let bg = ui.painter().add(Shape::Noop);
            let mut paper_rect = Rect::NOTHING;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.add_space(inset);
                let r = ui.vertical(|ui| {
                    ui.set_width(paper_w);
                    let y0 = ui.cursor().top();
                    // Paper top for the item loop's page-break avoidance (draw_page_article).
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("qs_paper_top"), y0));
                    ui.add_space(side * 1.05);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        ui.add_space(side);
                        ui.vertical(|ui| {
                            ui.set_width(cw);
                            draw_page_article(app, ui, ctx, exam_id, st, &exam, cw, bp, side);
                        });
                    });
                    ui.add_space(side * 0.8);
                    let used = ui.cursor().top() - y0;
                    let pages = (used / PAGE_H).ceil().max(1.0);
                    ui.allocate_space(Vec2::new(1.0, pages * PAGE_H - used));
                    st.pages = pages as usize;
                });
                paper_rect = r.response.rect;
            });
            ui.add_space(26.0);

            // Paper sheet (shadow, fill, edge) and page-break guides, behind the text.
            let mut shapes: Vec<Shape> = Vec::new();
            shapes.push(
                egui::Shadow { offset: [0, 18], blur: 34, spread: 0, color: Color32::from_black_alpha(70) }
                    .as_shape(paper_rect, CornerRadius::same(2))
                    .into(),
            );
            shapes.push(Shape::rect_filled(paper_rect, CornerRadius::same(2), theme::paper()));
            shapes.push(Shape::rect_stroke(
                paper_rect,
                CornerRadius::same(2),
                Stroke::new(1.0, theme::paper_edge()),
                StrokeKind::Inside,
            ));
            for k in 1..st.pages {
                let y = paper_rect.top() + k as f32 * PAGE_H;
                shapes.extend(Shape::dashed_line(
                    &[Pos2::new(paper_rect.left(), y), Pos2::new(paper_rect.right(), y)],
                    Stroke::new(1.0, PAPER_RULE.gamma_multiply(0.7)),
                    5.0,
                    4.0,
                ));
            }
            ui.painter().set(bg, Shape::Vec(shapes));
            // The header line repeats at the top of every later page (print does the same).
            if !exam.header.trim().is_empty() {
                let x = paper_rect.left() + side;
                for k in 1..st.pages {
                    let y = paper_rect.top() + k as f32 * PAGE_H + side * 1.05;
                    ui.painter().text(
                        Pos2::new(x, y + bp * 0.72),
                        Align2::LEFT_CENTER,
                        exam.header.trim(),
                        FontId::new(bp * 0.96, theme::serif_family()),
                        Color32::from_rgb(0x33, 0x33, 0x33),
                    );
                    let ly = y + bp * 1.45 + 1.0;
                    ui.painter().line_segment([Pos2::new(x, ly), Pos2::new(x + cw, ly)], Stroke::new(1.0, PAPER_RULE));
                }
            }
            // Page numbers at the foot of each page.
            for k in 1..=st.pages {
                ui.painter().text(
                    Pos2::new(paper_rect.center().x, paper_rect.top() + k as f32 * PAGE_H - 12.0),
                    Align2::CENTER_CENTER,
                    k.to_string(),
                    FontId::new(9.0, theme::serif_family()),
                    PAPER_SOFT.gamma_multiply(0.8),
                );
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn draw_page_article(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
    exam: &model::Exam,
    cw: f32,
    bp: f32,
    side: f32,
) {
    let items = &exam.items;
    let title = exam.title.clone().unwrap_or_else(|| exam.name.clone());
    let head_px = exam.heading_pt * PT;

    // The printed header line is editable directly on the page.
    let mut header = exam.header.clone();
    let before = header.clone();
    ui.add(
        egui::TextEdit::singleline(&mut header)
            .font(FontId::new(bp * 0.96, theme::serif_family()))
            .text_color(Color32::from_rgb(0x33, 0x33, 0x33))
            .frame(egui::Frame::NONE)
            .desired_width(cw),
    );
    if header != before {
        if let Some(e) = app.store.exam_mut(exam_id) {
            e.header = header;
            e.modified = now();
        }
        app.store.save_exams();
    }
    let ly = ui.cursor().top() + 1.0;
    ui.painter().line_segment(
        [Pos2::new(ui.min_rect().left(), ly), Pos2::new(ui.min_rect().left() + cw, ly)],
        Stroke::new(1.0, PAPER_RULE),
    );
    ui.add_space(14.0);

    ui.vertical_centered(|ui| {
        ui.add(egui::Label::new(ptext(title, head_px, true)).wrap());
        let pts: f64 = items
            .iter()
            .filter_map(|it| app.store.question(&it.bank_id, &it.question_id).and_then(|q| q.points))
            .sum();
        ui.label(
            RichText::new(format!(
                "{} question{}{}",
                items.len(),
                if items.len() == 1 { "" } else { "s" },
                if pts > 0.0 { format!(" · {} points", fmt_number(pts)) } else { String::new() }
            ))
            .family(theme::serif_family())
            .size(bp * 0.92)
            .color(PAPER_SOFT),
        );
    });
    ui.add_space(8.0);

    // Extra ruled blank lines after the title area.
    for _ in 0..exam.ruled_blanks {
        ruled_line(ui, cw, bp);
    }
    ui.add_space(10.0);

    // Pages 2+ repeat the header line at their top; content starts below it.
    let hdr_h = running_header_h(&exam.header, bp);

    // Numbers: matching consumes one number per left item; everything else one.
    // Keeps the preview numbering identical to the printed exam.
    let mut numbers: Vec<u32> = Vec::with_capacity(items.len());
    let mut run = 1u32;
    for it in items {
        let extra = app
            .store
            .question(&it.bank_id, &it.question_id)
            .map(|q| match &q.kind {
                Kind::Matching { pairs, .. } => pairs.len() as u32,
                _ => 1,
            })
            .unwrap_or(1);
        numbers.push(run);
        run += extra;
    }

    let shelf_drag = matches!(&st.drag, Some(DragState { src: DragSrc::Shelf { .. }, .. }));
    let slot_k = if shelf_drag { st.slot_k } else { None };
    let slot_no = |k: usize| numbers.get(k).copied().unwrap_or(run);
    let mut slot_shift = 0.0_f32;

    if items.is_empty() {
        if slot_k.is_some() {
            draw_drop_slot(ui, cw, bp, slot_no(0));
        } else {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Drag a question here, or press + on the shelf")
                        .family(theme::serif_family())
                        .size(bp)
                        .color(PAPER_SOFT),
                );
            });
            ui.add_space(30.0);
        }
    }

    for (i, item) in items.iter().enumerate() {
        if slot_k == Some(i) {
            slot_shift = draw_drop_slot(ui, cw, bp, slot_no(i)) + 8.0;
        }
        let q = app.store.question(&item.bank_id, &item.question_id).cloned();
        let number = numbers.get(i).copied().unwrap_or(i as u32 + 1);
        // Keep a question off the dashed page-break line: when last frame's height says it
        // would cross the break, start it at the top of the next page (as the print does).
        let h_id = egui::Id::new(("qs_item_h", &item.question_id));
        if let Some(h) = ctx.data(|d| d.get_temp::<f32>(h_id)) {
            keep_on_page(ui, h, side, hdr_h);
        }
        let rect = draw_item(app, ui, ctx, exam_id, st, i, number, q.as_ref(), cw, bp, side);
        ctx.data_mut(|d| d.insert_temp(h_id, rect.height()));
        st.rows.push((i, rect.translate(Vec2::new(0.0, -slot_shift))));
        ui.add_space(bp * 1.0);
    }
    if !items.is_empty() && slot_k == Some(items.len()) {
        draw_drop_slot(ui, cw, bp, slot_no(items.len()));
    }

    // Answer-section grid for MC/TF items ("N. ______" cells, 4 per print row).
    let cells: Vec<u32> = items
        .iter()
        .zip(numbers.iter())
        .filter(|(it, _)| {
            app.store
                .question(&it.bank_id, &it.question_id)
                .map(|q| matches!(q.kind.qkind(), QKind::Mc | QKind::Tf))
                .unwrap_or(false)
        })
        .map(|(_, n)| *n)
        .collect();
    if cells.is_empty() {
        return;
    }
    ui.add_space(bp * 1.2);
    keep_on_page(ui, bp * 1.6 + 6.0 + bp * 2.1, side, hdr_h);
    ui.add(egui::Label::new(ptext("Answer Section", bp + 1.0, true)).wrap());
    ui.add_space(6.0);
    let cell_w = (cw / 4.0).min(130.0);
    for chunk in cells.chunks(4) {
        keep_on_page(ui, bp * 2.1, side, hdr_h);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for n in chunk {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(cell_w, bp * 2.1), Sense::hover());
                ui.painter().text(
                    Pos2::new(rect.left(), rect.center().y),
                    Align2::LEFT_CENTER,
                    format!("{n}. ______"),
                    FontId::new(bp, theme::serif_family()),
                    PAPER_INK,
                );
            }
        });
    }
}

/// Height the repeated header line (Name / Class / Date) takes at the top of pages 2+;
/// 0 when the exam has no header.
fn running_header_h(header: &str, bp: f32) -> f32 {
    if header.trim().is_empty() { 0.0 } else { bp * 1.45 + bp * 0.6 }
}

/// Where page `k` (0-based) of the preview starts its content, relative to the paper top:
/// the top margin, plus the repeated header on pages after the first.
fn page_content_top(k: f32, side: f32, hdr_h: f32) -> f32 {
    k * PAGE_H + side * 1.05 + if k >= 1.0 { hdr_h } else { 0.0 }
}

/// Keeps a block `h` tall off the dashed page-break line and out of the repeated header:
/// when it would cross into the bottom margin it starts at the next page's content top
/// (as the print does); when the cursor sits in a page's header band it moves below it.
fn keep_on_page(ui: &mut egui::Ui, h: f32, side: f32, hdr_h: f32) {
    let Some(p0) = ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new("qs_paper_top"))) else { return };
    let top = ui.cursor().top();
    let page = ((top - p0) / PAGE_H).floor().max(0.0);
    let start = p0 + page_content_top(page, side, hdr_h);
    if top < start {
        ui.add_space(start - top);
        return;
    }
    let page_end = p0 + (page + 1.0) * PAGE_H - side * 0.8;
    if top + h > page_end && h < PAGE_H - side * 1.85 - hdr_h {
        ui.add_space(p0 + page_content_top(page + 1.0, side, hdr_h) - top);
    }
}

/// Dashed accent slot ("Drop here to add as question N"). Returns its height.
fn draw_drop_slot(ui: &mut egui::Ui, cw: f32, bp: f32, number: u32) -> f32 {
    let h = (bp * 3.0).max(28.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(cw, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(6), theme::accent_col().gamma_multiply(0.09));
    let s = Stroke::new(1.5, theme::accent_col());
    p.extend(Shape::dashed_line(
        &[rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()],
        s,
        5.0,
        3.5,
    ));
    p.text(
        rect.left_center() + Vec2::new(10.0, 0.0),
        Align2::LEFT_CENTER,
        format!("Drop here to add as question {number}"),
        FontId::new(10.5, theme::bold_family()),
        theme::accent_col(),
    );
    h
}

/// Small painted glyph buttons in the paper's margin.
fn margin_button(ui: &mut egui::Ui, id: egui::Id, center: Pos2, kind: u8, show: bool) -> bool {
    let rect = Rect::from_center_size(center, Vec2::splat(18.0));
    let resp = ui.interact(rect, id, Sense::click());
    if show {
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect, CornerRadius::same(5), theme::accent_faded());
        }
        let col = if resp.hovered() { theme::accent_col() } else { PAPER_SOFT };
        let s = Stroke::new(1.6, col);
        let c = rect.center();
        match kind {
            0 => {
                p.line_segment([c + Vec2::new(-3.5, -3.5), c + Vec2::new(3.5, 3.5)], s);
                p.line_segment([c + Vec2::new(-3.5, 3.5), c + Vec2::new(3.5, -3.5)], s);
            }
            1 => {
                p.line_segment([c + Vec2::new(-4.0, 1.5), c + Vec2::new(0.0, -2.5)], s);
                p.line_segment([c + Vec2::new(0.0, -2.5), c + Vec2::new(4.0, 1.5)], s);
            }
            3 => {
                // Duplicate: two overlapping sheets.
                let back = Rect::from_center_size(c + Vec2::new(-1.6, -1.6), Vec2::new(7.0, 8.5));
                let front = Rect::from_center_size(c + Vec2::new(1.6, 1.6), Vec2::new(7.0, 8.5));
                p.rect_stroke(back, CornerRadius::same(1), Stroke::new(1.3, col), StrokeKind::Middle);
                p.rect_filled(front, CornerRadius::same(1), theme::paper());
                p.rect_stroke(front, CornerRadius::same(1), Stroke::new(1.3, col), StrokeKind::Middle);
            }
            _ => {
                p.line_segment([c + Vec2::new(-4.0, -1.5), c + Vec2::new(0.0, 2.5)], s);
                p.line_segment([c + Vec2::new(0.0, 2.5), c + Vec2::new(4.0, -1.5)], s);
            }
        }
        let tip = match kind {
            0 => "Remove from exam",
            1 => "Move up",
            3 => "Duplicate question",
            _ => "Move down",
        };
        resp.clone().on_hover_text(tip);
    }
    show && resp.clicked()
}

/// One question on the page. Returns the item's screen rect (for drop targeting).
#[allow(clippy::too_many_arguments)]
fn draw_item(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
    i: usize,
    number: u32,
    q: Option<&model::Question>,
    cw: f32,
    bp: f32,
    side: f32,
) -> Rect {
    let wash = ui.painter().add(Shape::Noop);
    let rect = paper_question(app, ui, ctx, q, number, cw, bp);

    // Highlighter wash for a just-added question.
    if let Some((idx, t0)) = st.just_added {
        let age = (ctx.input(|i| i.time) - t0) as f32;
        if idx == i && age < 4.0 {
            let a = (1.0 - age / 4.0).clamp(0.0, 1.0) * 0.55;
            let hl = theme::highlighter().gamma_multiply(a);
            let r = rect.expand2(Vec2::new(4.0, 3.0));
            let mut mesh = Mesh::default();
            mesh.colored_vertex(r.left_top(), hl);
            mesh.colored_vertex(r.right_top(), Color32::TRANSPARENT);
            mesh.colored_vertex(r.right_bottom(), Color32::TRANSPARENT);
            mesh.colored_vertex(r.left_bottom(), hl);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            ui.painter().set(
                wash,
                Shape::Vec(vec![
                    Shape::mesh(mesh),
                    Shape::rect_filled(Rect::from_min_max(r.left_top() - Vec2::new(9.0, 0.0), r.left_bottom()), CornerRadius::ZERO, hl),
                ]),
            );
            ctx.request_repaint();
        }
    }

    // Margin gutters: grip on the left, remove / move buttons on the right.
    let dragging_this = matches!(&st.drag, Some(DragState { src: DragSrc::Page(f), .. }) if *f == i);
    let any_drag = st.drag.is_some();
    let hover = !any_drag && ui.rect_contains_pointer(rect.expand2(Vec2::new(side - 2.0, 4.0)));
    let show = hover || dragging_this;
    if let Some(q) = q {
        let gc = Pos2::new(rect.left() - (side * 0.5).min(24.0) - 2.0, rect.top() + bp * 0.75);
        let grect = Rect::from_center_size(gc, Vec2::new(18.0, 24.0));
        let gresp = ui.interact(grect, ui.id().with(("it_grip", i)), Sense::click_and_drag());
        if show {
            let col = if dragging_this || gresp.hovered() { theme::accent_col() } else { PAPER_SOFT };
            for dx in [-3.0, 3.0] {
                for dy in [-5.5, 0.0, 5.5] {
                    ui.painter().circle_filled(gc + Vec2::new(dx, dy), 1.5, col);
                }
            }
            gresp.clone().on_hover_cursor(egui::CursorIcon::Grab);
        }
        if gresp.drag_started() {
            st.drag = Some(DragState {
                src: DragSrc::Page(i),
                stem: q.stem.clone(),
                label: kind_pill(q.kind.qkind()),
            });
        }
        let rx = rect.right() + (side * 0.5).min(24.0) + 2.0;
        let y0 = rect.top() + 9.0;
        if margin_button(ui, ui.id().with(("it_x", i)), Pos2::new(rx, y0), 0, show) {
            remove_item(app, exam_id, i);
        }
        if margin_button(ui, ui.id().with(("it_up", i)), Pos2::new(rx, y0 + 20.0), 1, show) {
            shift_item(app, exam_id, i, -1);
        }
        if margin_button(ui, ui.id().with(("it_dn", i)), Pos2::new(rx, y0 + 40.0), 2, show) {
            shift_item(app, exam_id, i, 1);
        }
        // Duplicate sits in the left gutter under the grip, inside even a one-line
        // question's height (the right rail is already three buttons tall).
        if margin_button(ui, ui.id().with(("it_dup", i)), gc + Vec2::new(0.0, 24.0), 3, show) {
            if let Some(at) = duplicate_item(app, exam_id, i) {
                st.just_added = Some((at, ctx.input(|t| t.time)));
            }
        }
    }
    if dragging_this {
        ui.painter().rect_filled(rect.expand(3.0), CornerRadius::same(3), theme::paper().gamma_multiply(0.72));
        ctx.request_repaint();
    }
    rect
}

/// One numbered question as it prints (number, stem with picture placeholders, answer
/// area). Shared by the exam page and the import preview. Returns its rect.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paper_question(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    q: Option<&model::Question>,
    number: u32,
    cw: f32,
    bp: f32,
) -> Rect {
    let num_w = (bp * 1.9).max(16.0);
    let body_w = cw - num_w;
    let inner = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let (nr, _) = ui.allocate_exact_size(Vec2::new(num_w, bp * 1.45), Sense::hover());
        ui.painter().text(
            nr.left_top(),
            Align2::LEFT_TOP,
            format!("{number}."),
            FontId::new(bp, theme::serif_bold_family()),
            PAPER_INK,
        );
        ui.vertical(|ui| {
            ui.set_width(body_w);
            let Some(q) = q else {
                ui.label(RichText::new("(question missing from its bank)").size(bp).color(PAPER_SOFT));
                return;
            };
            render_stem(ui, &q.stem, q.points, bp, q.image.is_some());
            draw_item_body(app, ui, ctx, q, number, body_w, bp);
        });
    });
    Rect::from_min_size(inner.response.rect.min, Vec2::new(cw, inner.response.rect.height().max(bp * 1.45)))
}

/// One answer/option line on the paper: plain label, or the structured renderer when it holds
/// markup or `$..$`.
fn popt(ui: &mut egui::Ui, s: String, bp: f32) {
    if s.contains("[picture") {
        use crate::bankops::StemPart;
        let parts = crate::bankops::split_pictures(&s);
        if parts.iter().any(|p| matches!(p, StemPart::Picture(_))) {
            for part in parts {
                match part {
                    StemPart::Text(t) => popt(ui, t, bp),
                    StemPart::Picture(alt) => picture_needed_box(ui, bp, alt.as_deref()),
                }
            }
            return;
        }
    }
    if needs_rich(&s) {
        crate::equation::rich_ui(ui, &s, &crate::equation::Rich::new(bp, PAPER_INK).leading(0.32));
    } else {
        ui.add(egui::Label::new(ptext(s, bp, false)).wrap());
    }
}

fn draw_item_body(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    q: &model::Question,
    number: u32,
    body_w: f32,
    bp: f32,
) {
    match &q.kind {
        Kind::Mc { answers } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            ui.add_space(bp * 0.25);
            let short = answers.iter().all(|a| a.text.chars().count() <= 26);
            if short && answers.len() > 1 {
                ui.columns(2, |cols| {
                    for (ci, a) in answers.iter().enumerate() {
                        popt(&mut cols[ci % 2], format!("{}. {}", letter(ci), a.text), bp);
                    }
                });
            } else {
                for (ci, a) in answers.iter().enumerate() {
                    popt(ui, format!("{}. {}", letter(ci), a.text), bp);
                }
            }
        }
        Kind::Tf { .. } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for lab in ["True", "False"] {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(bp * 1.1), Sense::hover());
                    ui.painter().circle_stroke(r.center(), bp * 0.4, Stroke::new(1.0, PAPER_SOFT));
                    ui.label(ptext(lab, bp, false));
                    ui.add_space(12.0);
                }
            });
        }
        Kind::Matching { pairs, distractors } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            let rights: Vec<String> = pairs
                .iter()
                .map(|p| p.right.clone())
                .chain(distractors.iter().cloned())
                .collect();
            ui.columns(2, |cols| {
                for (li, p) in pairs.iter().enumerate() {
                    popt(&mut cols[0], format!("{}. {}", number + li as u32, p.left), bp);
                }
                for (ri, r) in rights.iter().enumerate() {
                    popt(&mut cols[1], format!("{}. {}", letter(ri), r), bp);
                }
            });
        }
        Kind::Blank { .. } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            for _ in 0..ruled_blanks(&q.stem) {
                ruled_line(ui, body_w, bp);
            }
        }
        Kind::Short { .. } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            for _ in 0..3 {
                ruled_line(ui, body_w, bp);
            }
        }
        Kind::Multipart { parts } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            let mut rules = 0u32;
            for (pi, part) in parts.iter().enumerate() {
                ui.add_space(bp * 0.35);
                let (label, rest) = split_part_label(pi, &part.stem);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let lw = bp * 1.6;
                    let (lr, _) = ui.allocate_exact_size(Vec2::new(lw, bp * 1.2), Sense::hover());
                    ui.painter().text(lr.left_top(), Align2::LEFT_TOP, &label, FontId::new(bp, theme::serif_family()), PAPER_INK);
                    ui.vertical(|ui| {
                        ui.set_width((body_w - lw).max(40.0));
                        match &part.kind {
                            PartKind::Mc { answers } => {
                                popt(ui, rest.clone(), bp);
                                for (ci, a) in answers.iter().enumerate() {
                                    popt(ui, format!("{}. {}", letter(ci), a.text), bp);
                                }
                            }
                            PartKind::Tf { .. } => popt(ui, format!("{rest} (True / False)"), bp),
                            PartKind::Short { .. } => popt(ui, rest.clone(), bp),
                        }
                    });
                });
                if matches!(part.kind, PartKind::Short { .. }) {
                    rules += 2;
                }
            }
            for _ in 0..rules.max(2) {
                ruled_line(ui, body_w, bp);
            }
        }
    }
}

fn draw_image(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    img: Option<&model::ImageRef>,
) {
    let Some(img) = img else { return };
    if let Some(tex) = app.image_texture(ctx, &img.file) {
        ui.add(
            egui::Image::new(&tex)
                .max_size(egui::vec2(170.0, 120.0))
                .maintain_aspect_ratio(true),
        );
    }
}

fn kind_pill(k: QKind) -> &'static str {
    match k {
        QKind::Mc => "MC",
        QKind::Tf => "TF",
        QKind::Matching => "MATCH",
        QKind::Blank => "FILL",
        QKind::Short => "SHORT",
        QKind::Multipart => "MULTI",
    }
}

// ---------------------------------------------------------------------------
// Item list operations
// ---------------------------------------------------------------------------

fn remove_item(app: &mut crate::App, exam_id: &Id, i: usize) {
    apply_items_fn(app, exam_id, |mut items| {
        if i < items.len() {
            items.remove(i);
        }
        items
    });
}

/// Duplicate (Test Parrot's per-question menu): copies item `i`'s question in its bank
/// (right after the original) and puts the copy on the page right after item `i`, as one
/// undo step for the page. Returns the copy's item index.
fn duplicate_item(app: &mut crate::App, exam_id: &Id, i: usize) -> Option<usize> {
    let item = app.store.exam(exam_id)?.items.get(i)?.clone();
    let bank = app.store.bank_mut(&item.bank_id)?;
    let new_id = crate::bankui::duplicate_in_bank(bank, &item.question_id)?;
    app.store.save_banks();
    let bank_id = item.bank_id.clone();
    apply_items_fn(app, exam_id, |items| with_copy_after(items, i, &bank_id, &new_id));
    Some(i + 1)
}

/// `items` with a new item for (`bank_id`, `question_id`) inserted after index `i`.
fn with_copy_after(mut items: Vec<ExamItem>, i: usize, bank_id: &Id, question_id: &Id) -> Vec<ExamItem> {
    let at = (i + 1).min(items.len());
    items.insert(at, ExamItem { bank_id: bank_id.clone(), question_id: question_id.clone() });
    items
}

fn shift_item(app: &mut crate::App, exam_id: &Id, i: usize, dir: i32) {
    apply_items_fn(app, exam_id, |mut items| {
        if items.is_empty() {
            return items;
        }
        let j = (i as i64 + dir as i64).clamp(0, items.len() as i64 - 1) as usize;
        if j != i {
            items.swap(i, j);
        }
        items
    });
}

/// Applies an item-list transform with an undo snapshot (no-ops don't touch the
/// undo stacks).
fn apply_items_fn(
    app: &mut crate::App,
    exam_id: &Id,
    f: impl FnOnce(Vec<ExamItem>) -> Vec<ExamItem>,
) {
    let before: Vec<ExamItem> = app
        .store
        .exam(exam_id)
        .map(|e| e.items.clone())
        .unwrap_or_default();
    let after = f(before.clone());
    if same_items(&after, &before) {
        return;
    }
    app.undo.push(before);
    app.redo.clear();
    if let Some(ex) = app.store.exam_mut(exam_id) {
        ex.items = after;
        ex.modified = now();
    }
    app.store.save_exams();
}

/// Inserts a bank question at `at` (clamped); returns the index it landed on.
fn add_item(app: &mut crate::App, exam_id: &Id, bank_id: &Id, question_id: &Id, at: usize) -> usize {
    let len = app.store.exam(exam_id).map(|e| e.items.len()).unwrap_or(0);
    let k = at.min(len);
    apply_items_fn(app, exam_id, |mut items| {
        items.insert(k, ExamItem { bank_id: bank_id.clone(), question_id: question_id.clone() });
        items
    });
    k
}

fn do_undo(app: &mut crate::App, exam_id: &Id) {
    let Some(prev) = app.undo.pop() else { return };
    if let Some(ex) = app.store.exam_mut(exam_id) {
        app.redo.push(ex.items.clone());
        ex.items = prev;
        ex.modified = now();
    }
    app.store.save_exams();
}

fn do_redo(app: &mut crate::App, exam_id: &Id) {
    let Some(prev) = app.redo.pop() else { return };
    if let Some(ex) = app.store.exam_mut(exam_id) {
        app.undo.push(ex.items.clone());
        ex.items = prev;
        ex.modified = now();
    }
    app.store.save_exams();
}

/// Finish a page drag: move the item from `from` to where the pointer sits.
/// A same-position drop is a no-op.
fn finish_drag(app: &mut crate::App, exam_id: &Id, st: &mut EditUi, from: usize, y: f32) {
    let k = drop_index(&st.rows, y, from);
    st.drag = None;
    apply_items_fn(app, exam_id, |items| {
        let mut next = items;
        if from >= next.len() {
            return next;
        }
        let moved = next.remove(from);
        let k = k.min(next.len());
        next.insert(k, moved);
        next
    });
}

/// Drop index (into the list WITHOUT row `from`) for pointer y.
fn drop_index(rows: &[(usize, egui::Rect)], y: f32, from: usize) -> usize {
    let count = rows.iter().filter(|(_, r)| r.center().y < y).count();
    if from < count {
        count - 1
    } else {
        count
    }
}

/// Insert position among all rows for a new (shelf) question at pointer y.
fn insert_index(rows: &[(usize, egui::Rect)], y: f32) -> usize {
    rows.iter().filter(|(_, r)| r.center().y < y).count()
}

/// The screen-space y of the drop boundary for target index `k` (list without
/// row `from`): the top of the k-th remaining row, or the bottom of the last row
/// when dropping at the end.
fn insertion_y(rows: &[(usize, egui::Rect)], k: usize, from: usize) -> Option<f32> {
    let mut idx = 0usize;
    for (orig, rect) in rows {
        if *orig == from {
            continue;
        }
        if k == idx {
            return Some(rect.top() - 3.0);
        }
        idx += 1;
    }
    rows.last().map(|(_, r)| r.bottom() + 3.0)
}

/// Drag post-pass, run once per frame after desk and shelf are laid out: finish or
/// cancel the drop, track the dashed slot for shelf drags, paint the tilted ghost.
fn drag_overlay(app: &mut crate::App, ctx: &egui::Context, exam_id: &Id, st: &mut EditUi) {
    let Some(drag) = st.drag.clone() else {
        st.slot_k = None;
        return;
    };
    let pos = ctx.input(|i| i.pointer.latest_pos());
    let held = ctx.input(|i| i.pointer.primary_down());
    let released = ctx.input(|i| i.pointer.any_released()) || !held;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        st.drag = None;
        st.slot_k = None;
        return;
    }
    let desk = st.desk_rect.unwrap_or(Rect::NOTHING);
    let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::NULL.with("examui_ghost"));
    let painter = ctx.layer_painter(layer);

    match &drag.src {
        DragSrc::Page(from) => {
            if released {
                match pos {
                    Some(p) => finish_drag(app, exam_id, st, *from, p.y),
                    None => st.drag = None,
                }
                return;
            }
            if let Some(p) = pos {
                let k = drop_index(&st.rows, p.y, *from);
                if let Some(yl) = insertion_y(&st.rows, k, *from) {
                    let clip = desk.shrink(2.0);
                    painter
                        .with_clip_rect(clip)
                        .line_segment([Pos2::new(clip.left(), yl), Pos2::new(clip.right(), yl)], Stroke::new(2.0, theme::accent_col()));
                }
            }
        }
        DragSrc::Shelf { bank_id, question_id } => {
            let target = pos.filter(|p| desk.contains(*p)).map(|p| insert_index(&st.rows, p.y));
            if released {
                if let Some(k) = target {
                    let idx = add_item(app, exam_id, bank_id, question_id, k);
                    st.just_added = Some((idx, ctx.input(|i| i.time)));
                }
                st.drag = None;
                st.slot_k = None;
                return;
            }
            st.slot_k = target;
        }
    }

    if let Some(p) = pos {
        let r = Rect::from_min_size(Pos2::new(p.x + 12.0, p.y - 18.0), Vec2::new(290.0, 38.0));
        paint_tilted_card(&painter, r, -0.021, &truncate_str(&crate::equation::stem_preview(&drag.stem), 40), Some(drag.label));
    }
    ctx.request_repaint();
}

// ---------------------------------------------------------------------------
// Shelf (right pane): bank dropdown, search, topics, draggable rows
// ---------------------------------------------------------------------------

fn draw_shelf(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
    rect: Rect,
) {
    kit::card()
        .inner_margin(Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_min_size(rect.size() - Vec2::new(30.0, 26.0));
            ui.set_max_width(rect.width() - 30.0);
            draw_shelf_inner(app, ui, ctx, exam_id, st);
        });
}

fn draw_shelf_inner(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
) {
    ui.label(theme::serif("Question shelf").size(17.0));
    ui.add_space(6.0);
    egui::ComboBox::from_id_salt("exam_panel_bank")
        .selected_text(
            app.panel_bank
                .as_ref()
                .and_then(|id| app.store.bank(id))
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "Open bank…".to_string()),
        )
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for b in app.store.banks.clone() {
                ui.selectable_value(&mut app.panel_bank, Some(b.id.clone()), b.name.clone());
            }
        });

    let Some(bank_id) = app.panel_bank.clone() else {
        ui.add_space(12.0);
        ui.add(egui::Label::new(muted("Create a bank on the Banks screen, then pick it here.")));
        return;
    };
    let Some(bank) = app.store.bank(&bank_id).cloned() else {
        app.panel_bank = None;
        return;
    };
    let in_exam: Vec<ExamItem> = app.store.exam(exam_id).map(|e| e.items.clone()).unwrap_or_default();

    ui.add_space(6.0);
    ui.add(
        egui::TextEdit::singleline(&mut st.search)
            .hint_text("Search this bank")
            .margin(Margin::symmetric(10, 6))
            .desired_width(ui.available_width()),
    );
    ui.add_space(6.0);

    // Topic chips.
    let mut topics: Vec<String> = bank
        .questions
        .iter()
        .flat_map(|q| q.topics.clone())
        .filter(|t| !t.trim().is_empty())
        .collect();
    topics.sort();
    topics.dedup();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
        if kit::chip_toggle(ui, "All", st.topics.is_empty()).clicked() {
            st.topics.clear();
        }
        for t in &topics {
            let sel = st.topics.contains(t);
            if kit::chip_toggle(ui, t, sel).clicked() {
                if sel {
                    st.topics.retain(|x| x != t);
                } else {
                    st.topics.push(t.clone());
                }
            }
        }
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
        for (di, label) in [(0u8, "Any level"), (1, "Low"), (2, "Medium"), (3, "High")] {
            if kit::chip_toggle(ui, label, st.diff == di).clicked() {
                st.diff = di;
            }
        }
        if kit::chip_toggle(ui, "Newest first", st.sort_newest).clicked() {
            st.sort_newest = !st.sort_newest;
        }
    });
    ui.add_space(6.0);

    let needle = st.search.trim().to_lowercase();
    let mut qs: Vec<model::Question> = bank
        .questions
        .iter()
        .filter(|q| needle.is_empty() || q.stem.to_lowercase().contains(&needle) || crate::equation::stem_preview(&q.stem).to_lowercase().contains(&needle))
        .filter(|q| st.topics.is_empty() || st.topics.iter().any(|t| q.topics.contains(t)))
        .filter(|q| match (st.diff, &q.difficulty) {
            (0, _) => true,
            (1, Some(model::Difficulty::Low))
            | (2, Some(model::Difficulty::Medium))
            | (3, Some(model::Difficulty::High)) => true,
            _ => false,
        })
        .cloned()
        .collect();
    if st.sort_newest {
        qs.sort_by_key(|q| -q.created);
    } else {
        qs.sort_by_key(|q| q.created);
    }

    let used = bank
        .questions
        .iter()
        .filter(|q| in_exam.iter().any(|it| it.bank_id == bank_id && it.question_id == q.id))
        .count();

    let list_h = (ui.available_height() - 34.0).max(60.0);
    egui::ScrollArea::vertical()
        .id_salt("bank_panel_scroll")
        .max_height(list_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if qs.is_empty() {
                ui.add(egui::Label::new(muted("No questions match.")));
                return;
            }
            for q in &qs {
                let count = in_exam
                    .iter()
                    .filter(|it| it.bank_id == bank_id && it.question_id == q.id)
                    .count();
                shelf_row(app, ui, st, ctx, &bank_id, exam_id, q, count);
            }
        });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(muted(RichText::new("Click + or drag onto the page").size(11.5)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(muted(RichText::new(format!("of {} used", bank.questions.len())).size(11.5)));
            ui.label(RichText::new(used.to_string()).family(theme::bold_family()).size(11.5).color(theme::ink()));
        });
    });
}

/// One shelf row: grip | type pill | 2-line stem | points | add / in-exam mark.
#[allow(clippy::too_many_arguments)]
fn shelf_row(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    st: &mut EditUi,
    ctx: &egui::Context,
    bank_id: &Id,
    exam_id: &Id,
    q: &model::Question,
    in_count: usize,
) {
    let w = ui.available_width();
    let stem_w = w - (6.0 + 14.0 + 8.0 + 46.0 + 8.0 + 38.0 + 8.0 + 28.0 + 6.0);
    let already = in_count > 0;
    let stem_col = if already { theme::muted_col() } else { theme::ink() };
    let p = ui.painter().clone();
    let stem_txt = crate::equation::stem_preview(&q.stem);
    let galley = clamped_galley(&p, &stem_txt, FontId::proportional(12.5), stem_col, stem_w, 2);
    let h = (galley.size().y + 18.0).max(46.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());
    let drag_this = matches!(&st.drag, Some(DragState { src: DragSrc::Shelf { question_id, .. }, .. }) if *question_id == q.id);

    if resp.hovered() || drag_this {
        p.rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), CornerRadius::same(9), theme::hover_bg());
    }
    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, theme::border()));
    let cy = rect.center().y;

    // Grip.
    let gx = rect.left() + 6.0 + 7.0;
    let gcol = if resp.hovered() { theme::accent_col() } else { theme::rule() };
    for dx in [-2.8, 2.8] {
        for dy in [-5.0, 0.0, 5.0] {
            p.circle_filled(Pos2::new(gx + dx, cy + dy), 1.3, gcol);
        }
    }
    // Type pill (same look as kit::type_pill, painted to fit the grid).
    let px = rect.left() + 6.0 + 14.0 + 8.0;
    let pr = Rect::from_min_size(Pos2::new(px, cy - 9.0), Vec2::new(46.0, 18.0));
    p.rect_filled(pr, CornerRadius::same(9), theme::accent_faded());
    p.text(pr.center(), Align2::CENTER_CENTER, kind_pill(q.kind.qkind()), FontId::new(9.5, theme::bold_family()), theme::accent_col());
    // Stem.
    let sx = pr.right() + 8.0;
    p.galley(Pos2::new(sx, cy - galley.size().y / 2.0), galley, stem_col);
    // Points.
    let pts = q.points.map(|v| format!("{} pt", fmt_number(v))).unwrap_or_default();
    p.text(
        Pos2::new(rect.right() - 6.0 - 28.0 - 8.0, cy),
        Align2::RIGHT_CENTER,
        pts,
        FontId::proportional(11.5),
        theme::muted_col(),
    );
    // Add button / in-exam check.
    let br = Rect::from_center_size(Pos2::new(rect.right() - 6.0 - 13.0, cy), Vec2::splat(26.0));
    let bresp = ui.interact(br, ui.id().with(("shelf_add", &q.id)), Sense::click());
    let c = br.center();
    if already {
        let s = Stroke::new(2.0, theme::accent_col());
        p.line_segment([c + Vec2::new(-4.5, 0.5), c + Vec2::new(-1.5, 3.5)], s);
        p.line_segment([c + Vec2::new(-1.5, 3.5), c + Vec2::new(4.5, -3.5)], s);
        bresp.clone().on_hover_text("Already in this exam. Drag the row to add it again.");
    } else {
        p.rect(
            br,
            CornerRadius::same(8),
            if bresp.hovered() { theme::accent_faded() } else { theme::card() },
            Stroke::new(1.0, if bresp.hovered() { theme::accent_col() } else { theme::rule() }),
            StrokeKind::Inside,
        );
        let s = Stroke::new(2.0, theme::accent_col());
        p.line_segment([c + Vec2::new(-4.5, 0.0), c + Vec2::new(4.5, 0.0)], s);
        p.line_segment([c + Vec2::new(0.0, -4.5), c + Vec2::new(0.0, 4.5)], s);
        if bresp.clicked() {
            let len = app.store.exam(exam_id).map(|e| e.items.len()).unwrap_or(0);
            let idx = add_item(app, exam_id, bank_id, &q.id, len);
            st.just_added = Some((idx, ctx.input(|i| i.time)));
        }
    }
    if drag_this {
        p.rect_filled(rect, CornerRadius::same(9), theme::card().gamma_multiply(0.6));
    }

    if resp.drag_started() && !bresp.hovered() {
        st.drag = Some(DragState {
            src: DragSrc::Shelf { bank_id: bank_id.clone(), question_id: q.id.clone() },
            stem: q.stem.clone(),
            label: kind_pill(q.kind.qkind()),
        });
    }
    if resp.hovered() {
        ctx.set_cursor_icon(egui::CursorIcon::Grab);
    }
}

// ---------------------------------------------------------------------------
// Export / print mapping
// ---------------------------------------------------------------------------

/// The exam worker's internal question mapping: the exporters' `DocQ` (with image
/// bytes) plus the media ref so the snapshot keeps the image by reference.
struct Mapped {
    dq: DocQ,
    img: Option<model::ImageRef>,
}

/// Reads one attached image's bytes from the store media dir.
fn read_img(img_ref: &model::ImageRef) -> Option<DocImg> {
    let bytes = std::fs::read(store::media_path(img_ref)).ok()?;
    Some(DocImg { bytes, width_px: img_ref.width, height_px: img_ref.height })
}

/// Print numbers for a resolved question list: matching consumes one number per
/// left item (exporters continue numbering per left item), everything else one.
fn doc_numbers(qs: &[model::Question]) -> Vec<u32> {
    let mut out = Vec::with_capacity(qs.len());
    let mut n = 1u32;
    for q in qs {
        out.push(n);
        n += match &q.kind {
            Kind::Matching { pairs, .. } => pairs.len() as u32,
            _ => 1,
        };
    }
    out
}

/// Maps one `model::Question` into the exporters' print conventions.
fn map_question(number: u32, q: &model::Question) -> Mapped {
    let mut m = map_question_raw(number, q);
    // Exports have no typesetter yet: degrade `$latex$` to its Unicode approximation.
    let plain = |s: &mut String| *s = crate::equation::stem_to_plain(s);
    m.dq.text.iter_mut().for_each(plain);
    m.dq.choices.iter_mut().for_each(plain);
    plain(&mut m.dq.key_text);
    m
}

fn map_question_raw(number: u32, q: &model::Question) -> Mapped {
    let mut dq = DocQ::default();
    dq.number = number;
    dq.points = q.points;
    dq.image = q.image.as_ref().and_then(read_img);
    dq.rule_lines = 0;
    let mut ktext: String;

    match &q.kind {
        Kind::Mc { answers } => {
            dq.text = vec![q.stem.clone()];
            dq.choices = answers
                .iter()
                .enumerate()
                .map(|(i, a)| format!("{}. {}", letter(i), a.text))
                .collect();
            let ks: Vec<usize> = answers
                .iter()
                .enumerate()
                .filter(|(_, a)| a.correct)
                .map(|(i, _)| i)
                .collect();
            ktext = if ks.is_empty() {
                "(no answer marked)".to_string()
            } else {
                ks.iter().map(|i| letter(*i)).collect::<Vec<_>>().join(", ")
            };
        }
        Kind::Tf { correct } => {
            dq.text = vec![q.stem.clone()];
            dq.choices = vec!["True".to_string(), "False".to_string()];
            ktext = if *correct { "True" } else { "False" }.to_string();
        }
        Kind::Matching { pairs, distractors } => {
            // Every text entry is a left-column item (no stem line); the
            // exporters print them with continuing numbers from `number`.
            dq.text = pairs.iter().map(|p| p.left.clone()).collect();
            let rights: Vec<&String> = pairs
                .iter()
                .map(|p| &p.right)
                .chain(distractors.iter())
                .collect();
            dq.choices = rights
                .iter()
                .enumerate()
                .map(|(i, r)| format!("{}. {}", letter(i), r))
                .collect();
            ktext = pairs
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let li = rights.iter().position(|r| **r == p.right);
                    format!(
                        "{}-{}",
                        i + 1,
                        li.map(letter).unwrap_or_else(|| "?".to_string())
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
        }
        Kind::Blank { accept } => {
            dq.text = vec![q.stem.clone()];
            // Blanks inside table cells are answer cells: no ruled line for them.
            dq.rule_lines = crate::export::blanks_outside_tables(&q.stem) as u8;
            ktext = accept
                .iter()
                .map(|group| group.join(" / "))
                .collect::<Vec<_>>()
                .join(", ");
        }
        Kind::Short { sample } => {
            dq.text = vec![q.stem.clone()];
            dq.rule_lines = 3;
            ktext = sample.clone();
        }
        Kind::Multipart { parts } => {
            let mut s = q.stem.clone();
            let mut rules = 0u8;
            let mut kparts: Vec<String> = Vec::new();
            for (pi, part) in parts.iter().enumerate() {
                match &part.kind {
                    PartKind::Mc { answers } => {
                        s.push_str(&format!("\n{}", part_line(pi, &part.stem)));
                        for (ci, a) in answers.iter().enumerate() {
                            s.push_str(&format!("\n      {}. {}", letter(ci), a.text));
                        }
                        let ks: Vec<usize> = answers
                            .iter()
                            .enumerate()
                            .filter(|(_, a)| a.correct)
                            .map(|(i, _)| i)
                            .collect();
                        let label = if ks.is_empty() {
                            "(no answer marked)".to_string()
                        } else {
                            ks.iter().map(|i| letter(*i)).collect::<Vec<_>>().join(", ")
                        };
                        kparts.push(format!("{}) {}", part_key_label(pi, &part.stem), label));
                    }
                    PartKind::Tf { correct } => {
                        s.push_str(&format!("\n{} (True / False)", part_line(pi, &part.stem)));
                        kparts.push(format!(
                            "{}) {}",
                            part_key_label(pi, &part.stem),
                            if *correct { "True" } else { "False" }
                        ));
                    }
                    PartKind::Short { sample } => {
                        s.push_str(&format!("\n{}", part_line(pi, &part.stem)));
                        rules += 3;
                        kparts.push(format!(
                            "{}) {}",
                            part_key_label(pi, &part.stem),
                            sample.trim()
                        ));
                    }
                }
            }
            dq.text = vec![s];
            dq.rule_lines = rules;
            // Multi-line part answers each start on their own line; short ones stay on one line.
            ktext = kparts.join(if kparts.iter().any(|k| k.contains('\n')) { "\n" } else { "  " });
        }
    }

    // The teacher's explanation travels inside key_text (prints only on the key).
    if let Some(ex) = &q.explanation {
        let ex = ex.trim();
        if !ex.is_empty() {
            ktext = format!("{ktext}\n\nExplanation: {ex}");
        }
    }
    dq.key_text = ktext;
    Mapped { dq, img: q.image.clone() }
}

/// Headless export (QA hook QUESHION_EXPORT): writes `exam.pdf`, `exam.docx` and `key.pdf` for the
/// exam at `index` of the store into `dir`, with the real export's mapping (single copy, no
/// shuffling; questions missing from their bank are skipped).
pub fn headless_export(index: usize, dir: &std::path::Path) -> Result<(), String> {
    let st = store::Store::load();
    let exam = st
        .exams
        .get(index)
        .ok_or_else(|| format!("no exam at index {index} ({} exams in the store)", st.exams.len()))?;
    let qs: Vec<model::Question> = exam
        .items
        .iter()
        .filter_map(|it| st.question(&it.bank_id, &it.question_id).cloned())
        .collect();
    if qs.is_empty() {
        return Err("the exam has no resolvable questions".into());
    }
    let numbers = doc_numbers(&qs);
    let mapped: Vec<Mapped> = qs.iter().enumerate().map(|(i, q)| map_question(numbers[i], q)).collect();
    let student = build_doc(exam, &mapped, None, false);
    let key = build_doc(exam, &mapped, None, true);
    let write = |name: &str, bytes: Vec<u8>| std::fs::write(dir.join(name), bytes).map_err(|e| format!("{name}: {e}"));
    write("exam.pdf", crate::export::render_pdf(&student)?)?;
    write("exam.docx", crate::export::render_docx(&student)?)?;
    write("key.pdf", crate::export::render_pdf(&key)?)?;
    Ok(())
}

/// Builds a printable exam doc: student test (`is_key` false) or answer key.
fn build_doc(
    exam: &model::Exam,
    mapped: &[Mapped],
    label: Option<String>,
    is_key: bool,
) -> ExamDoc {
    let base_title = exam.title.clone().unwrap_or_else(|| exam.name.clone());
    ExamDoc {
        title: if is_key {
            format!("{base_title} — Answer Key")
        } else {
            base_title
        },
        header_line: exam.header.clone(),
        heading_pt: exam.heading_pt,
        body_pt: exam.body_pt,
        margin_mm: exam.margin_mm,
        answers_inline: true,
        is_key,
        version_label: label,
        ruled_blanks: exam.ruled_blanks,
        questions: mapped
            .iter()
            .map(|m| {
                let mut dq = m.dq.clone();
                if is_key {
                    dq.choices.clear();
                    dq.image = None;
                    dq.rule_lines = 0;
                    dq.text = vec![];
                }
                dq
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Deterministic shuffle (no rand crate)
// ---------------------------------------------------------------------------

/// Small LCG: successive raw states from a seed. Same seed ⇒ same output stream.
fn lcg(seed: u64) -> impl FnMut() -> u64 {
    let mut st = seed ^ 0x853c49e6748fea9b;
    move || {
        st = st
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        st
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Fisher–Yates of indices 0..n from an LCG stream.
fn shuffled_indices(n: usize, rnd: &mut impl FnMut() -> u64) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    if n <= 1 {
        return v;
    }
    for i in (1..n).rev() {
        let j = ((rnd() >> 33) % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
    v
}

/// Moves only the non-locked MC answers around (locked answers keep their slot);
/// the key letters recompute afterwards from the new order in `map_question`.
fn shuffle_answers(q: &mut model::Question, rnd: &mut impl FnMut() -> u64) {
    let Kind::Mc { answers } = &mut q.kind else { return };
    let movable: Vec<usize> = answers
        .iter()
        .enumerate()
        .filter(|(_, a)| !a.locked)
        .map(|(i, _)| i)
        .collect();
    if movable.len() <= 1 {
        return;
    }
    let order = shuffled_indices(movable.len(), rnd);
    let mvals: Vec<model::Answer> = movable
        .iter()
        .map(|i| answers[*i].clone())
        .collect();
    let mut next = answers.clone();
    let mut k = 0usize;
    for (i, a) in answers.iter().enumerate() {
        if a.locked {
            next[i] = a.clone();
        } else {
            next[i] = mvals[order[k]].clone();
            k += 1;
        }
    }
    *answers = next;
}

// ---------------------------------------------------------------------------
// Snapshot serialization (the reproducible `.doc.json` next to every export)
// ---------------------------------------------------------------------------

/// Mirrors `layout::ExamDoc` for the `.doc.json` snapshot: the image is kept as a
/// media-file reference (bytes re-read from the store on regeneration) and the
/// produced file name is recorded, so "Regenerate" re-renders the exact copy.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
struct SnapDoc {
    pub title: String,
    pub header_line: String,
    pub heading_pt: f32,
    pub body_pt: f32,
    pub margin_mm: f32,
    pub answers_inline: bool,
    pub is_key: bool,
    pub version_label: Option<String>,
    pub ruled_blanks: u8,
    pub questions: Vec<SnapQ>,
    /// Name of the produced PDF/DOCX file sitting next to this snapshot.
    pub out_file: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
struct SnapQ {
    pub number: u32,
    pub text: Vec<String>,
    pub choices: Vec<String>,
    pub image: Option<SnapImage>,
    pub key_text: String,
    pub rule_lines: u8,
    pub points: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
struct SnapImage {
    pub file: String,
    pub width: u32,
    pub height: u32,
}

impl SnapDoc {
    fn from_parts(
        exam: &model::Exam,
        mapped: &[Mapped],
        label: Option<String>,
        is_key: bool,
        out_file: String,
    ) -> SnapDoc {
        let doc = build_doc(exam, mapped, label.clone(), is_key);
            let questions = mapped
                .iter()
                .enumerate()
                .map(|(i, m)| SnapQ {
                    number: doc.questions[i].number,
                    text: doc.questions[i].text.clone(),
                    choices: doc.questions[i].choices.clone(),
                    image: m.img.clone().map(|r| SnapImage {
                        file: r.file,
                        width: r.width,
                        height: r.height,
                    }),
                    key_text: doc.questions[i].key_text.clone(),
                    rule_lines: doc.questions[i].rule_lines,
                    points: doc.questions[i].points,
                })
                .collect::<Vec<_>>();
        SnapDoc {
            title: doc.title,
            header_line: doc.header_line,
            heading_pt: doc.heading_pt,
            body_pt: doc.body_pt,
            margin_mm: doc.margin_mm,
            answers_inline: doc.answers_inline,
            is_key: doc.is_key,
            version_label: label,
            ruled_blanks: doc.ruled_blanks,
            questions,
            out_file,
        }
    }

    fn to_doc(&self) -> ExamDoc {
        ExamDoc {
            title: self.title.clone(),
            header_line: self.header_line.clone(),
            heading_pt: self.heading_pt,
            body_pt: self.body_pt,
            margin_mm: self.margin_mm,
            answers_inline: self.answers_inline,
            is_key: self.is_key,
            version_label: self.version_label.clone(),
            ruled_blanks: self.ruled_blanks,
            questions: self
                .questions
                .iter()
                .map(|q| DocQ {
                    number: q.number,
                    text: q.text.clone(),
                    choices: q.choices.clone(),
                    image: q.image.as_ref().and_then(|s| {
                        let img_ref = model::ImageRef {
                            file: s.file.clone(),
                            width: s.width,
                            height: s.height,
                        };
                        read_img(&img_ref)
                    }),
                    key_text: q.key_text.clone(),
                    rule_lines: q.rule_lines,
                    points: q.points,
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Export pipeline
// ---------------------------------------------------------------------------

fn xdg_open(path: &std::path::Path) {
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

impl crate::App {
    fn draw_export_dialog(&mut self, ctx: &egui::Context, exam_id: &Id) {
        let mut exu: ExportUi = mem_load(ctx, export_id());
        let mut open = self.export_open;

        let name = self.exams_name(exam_id).unwrap_or_default();
        let n_items = self.store.exam(exam_id).map(|e| e.items.len()).unwrap_or(0);
        let n_points = self
            .store
            .exam(exam_id)
            .map(|e| {
                e.items
                    .iter()
                    .filter_map(|it| {
                        self.store
                            .question(&it.bank_id, &it.question_id)
                            .and_then(|q| q.points)
                    })
                    .sum::<f64>()
            })
            .unwrap_or(0.0);

        egui::Window::new(theme::strong("Export"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(560.0)
            .show(ctx, |ui| {
                card_frame(theme::cream()).show(ui, |ui| {
                    ui.add(egui::Label::new(muted("Preview")));
                    ui.add(
                        egui::Label::new(
                            theme::serif_strong(name.clone())
                                .size(18.0)
                                .color(theme::ink()),
                        )
                        .wrap(),
                    );
                    ui.add(egui::Label::new(muted(format!(
                        "{n_items} question{} · {} total points",
                        if n_items == 1 { "" } else { "s" },
                        fmt_number(n_points),
                    ))));
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(muted(
                        egui::RichText::new("Format").size(11.5),
                    )));
                    if kit::chip_toggle(ui, "PDF", !exu.docx).clicked() {
                        exu.docx = false;
                    }
                    if kit::chip_toggle(ui, "DOCX", exu.docx).clicked() {
                        exu.docx = true;
                    }
                });
                ui.add_space(2.0);
                ui.checkbox(&mut exu.student, "Student test");
                ui.checkbox(&mut exu.key, "Answer key");
                ui.add_space(6.0);
                ui.checkbox(&mut exu.shuffle_q, "Shuffle question order");
                ui.checkbox(&mut exu.shuffle_a, "Shuffle answer order");
                let any_shuffle = exu.shuffle_q || exu.shuffle_a;
                ui.horizontal(|ui| {
                    let mut versions = exu.versions as i32;
                    ui.add_enabled(
                        any_shuffle,
                        egui::Label::new(muted(egui::RichText::new("Versions").size(11.5))),
                    );
                    let r = ui.add_enabled(
                        any_shuffle,
                        egui::DragValue::new(&mut versions)
                            .range(2..=8)
                            .speed(0.2),
                    );
                    if r.changed() {
                        exu.versions = versions.clamp(2, 8) as u32;
                    }
                });
                ui.add(
                    egui::Label::new(muted(
                        "Turn on a shuffle to print named Versions. Otherwise the Exam prints as you see it."
                            .to_string(),
                    ).size(11.5)),
                );
                ui.add_space(10.0);
                let label = if exu.docx { "Download DOCX" } else { "Download PDF" };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ready = n_items > 0 && (exu.student || exu.key);
                    let clicked = ui
                        .add_enabled_ui(ready, |ui| kit::primary_button(ui, label).clicked())
                        .inner;
                    if clicked {
                        do_export(self, exam_id, exu.clone());
                    }
                });
            });

        self.export_open = open;
        mem_store(ctx, export_id(), &exu);
    }
}

fn do_export(app: &mut crate::App, exam_id: &Id, exu: ExportUi) {
    match do_export_inner(app, exam_id, &exu) {
        Ok(Some(dir)) => xdg_open(&dir),
        Ok(None) => {} // canceled / no-op
        Err(e) => screens::toast(app, e.as_str(), true),
    }
}

/// Ok(Some(dir)): exported (the caller opens the folder). Ok(None): silent no-op
/// (canceled save dialog, nothing selected). Err(msg): toast the failure.
fn do_export_inner(
    app: &mut crate::App,
    exam_id: &Id,
    exu: &ExportUi,
) -> Result<Option<std::path::PathBuf>, String> {
    let Some(exam) = app.store.exam(exam_id).cloned() else {
        return Err("exam not found".to_string());
    };
    if !(exu.student || exu.key) {
        screens::toast(app, "Pick Student test or Answer key first", true);
        return Ok(None);
    }
    if exam.items.is_empty() {
        screens::toast(app, "Add questions before exporting", true);
        return Ok(None);
    }

    let ext = if exu.docx { "docx" } else { "pdf" };
    let filter = if exu.docx { "DOCX" } else { "PDF" };

    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{}.{}", exam.name, ext))
        .add_filter(filter, &[ext])
        .save_file()
    else {
        return Ok(None); // user canceled
    };

    let dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| exam.name.clone());

    // Resolve placed questions once; missing ones are skipped (and reported).
    let resolved: Vec<Option<model::Question>> = exam
        .items
        .iter()
        .map(|it| app.store.question(&it.bank_id, &it.question_id).cloned())
        .collect();
    let n_missing = resolved.iter().filter(|q| q.is_none()).count();
    if n_missing == resolved.len() {
        return Err(
            "none of the exam's questions resolve (bank questions deleted?)".to_string(),
        );
    }

    // Deterministic per-run base seed: exam-id hash mixed with wall time.
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let base_seed = fnv1a(exam.id.as_bytes()) ^ t;
    let shuffle = exu.shuffle_q || exu.shuffle_a;
    let n_versions: usize = if shuffle {
        exu.versions.clamp(2, 8) as usize
    } else {
        1
    };

    let mut files: Vec<String> = Vec::new();
    let mut wrote = 0usize;
    for v in 0..n_versions {
        let mut rnd = lcg(base_seed ^ ((v as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        // Question order for this version.
        let order: Vec<usize> = if exu.shuffle_q {
            shuffled_indices(resolved.len(), &mut rnd)
        } else {
            (0..resolved.len()).collect()
        };
        // This version's questions (fresh clones; shuffle answers inside).
        let mut qs: Vec<model::Question> = Vec::with_capacity(resolved.len());
        for i in &order {
            if let Some(q) = &resolved[*i] {
                let mut q = q.clone();
                if exu.shuffle_a {
                    shuffle_answers(&mut q, &mut rnd);
                }
                qs.push(q);
            }
        }
        let numbers = doc_numbers(&qs);
        let label = if n_versions > 1 {
            Some(format!("Version {}", version_letter(v)))
        } else {
            None
        };
        let mapped: Vec<Mapped> = qs
            .iter()
            .enumerate()
            .map(|(i, q)| map_question(numbers[i], q))
            .collect();

        let suffix = if n_versions > 1 {
            format!("-Version {}", version_letter(v))
        } else {
            String::new()
        };

        if exu.student {
            let doc = build_doc(&exam, &mapped, label.clone(), false);
            let out_name = format!("{stem}{suffix}.{ext}");
            let out_path = dir.join(&out_name);
            let bytes = if exu.docx {
                crate::export::render_docx(&doc)?
            } else {
                crate::export::render_pdf(&doc)?
            };
            std::fs::write(&out_path, bytes).map_err(|e| e.to_string())?;
            files.push(out_path.to_string_lossy().to_string());
            wrote += 1;
            snap_write(
                &exam,
                &mapped,
                label.clone(),
                false,
                &dir,
                &format!("{stem}{suffix}.doc.json"),
                &out_name,
                &mut files,
                &mut wrote,
            );
        }
        if exu.key {
            let doc = build_doc(&exam, &mapped, label.clone(), true);
            let out_name = if n_versions > 1 {
                format!("{stem}{suffix}-Answer Key.{ext}")
            } else {
                format!("{stem}-Answer Key.{ext}")
            };
            let out_path = dir.join(&out_name);
            let bytes = if exu.docx {
                crate::export::render_docx(&doc)?
            } else {
                crate::export::render_pdf(&doc)?
            };
            std::fs::write(&out_path, bytes).map_err(|e| e.to_string())?;
            files.push(out_path.to_string_lossy().to_string());
            wrote += 1;
            let snap_suffix = if n_versions > 1 {
                suffix.clone()
            } else {
                String::new()
            };
            snap_write(
                &exam,
                &mapped,
                label,
                true,
                &dir,
                &format!("{stem}{snap_suffix}-key.doc.json"),
                &out_name,
                &mut files,
                &mut wrote,
            );
        }
    }
    if wrote == 0 {
        return Err("nothing could be written".to_string());
    }

    // Record + open the folder + toast.
    let mut what: Vec<String> = vec![];
    if exu.student {
        what.push("student test".to_string());
    }
    if exu.key {
        what.push("answer key".to_string());
    }
    let rec = model::ExportRecord {
        id: model::new_id(),
        time: now(),
        exam_id: exam.id.clone(),
        exam_name: exam.name.clone(),
        format: ext.to_string(),
        what,
        versions: n_versions as u32,
        files,
    };
    app.store.exports.push(rec);
    app.store.save_exports();

    let msg = if n_missing == 0 {
        format!("Exported {wrote} files to {}", dir.display())
    } else {
        format!(
            "Exported {wrote} files ({n_missing} questions are missing from their bank and were skipped)"
        )
    };
    screens::toast(app, msg.as_str(), n_missing > 0);
    Ok(Some(dir))
}

#[allow(clippy::too_many_arguments)]
fn snap_write(
    exam: &model::Exam,
    mapped: &[Mapped],
    label: Option<String>,
    is_key: bool,
    dir: &std::path::Path,
    json_name: &str,
    out_file: &str,
    files: &mut Vec<String>,
    wrote: &mut usize,
) {
    let snap = SnapDoc::from_parts(exam, mapped, label, is_key, out_file.to_string());
    match serde_json::to_string_pretty(&snap) {
        Ok(json) => {
            let json_path = dir.join(json_name);
            if std::fs::write(&json_path, json).is_ok() {
                files.push(json_path.to_string_lossy().to_string());
                *wrote += 1;
            }
        }
        Err(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Export history dialog
// ---------------------------------------------------------------------------

impl crate::App {
    fn draw_history(&mut self, ctx: &egui::Context) {
        let mut open = self.history_open;
        let mut note: Option<(String, bool)> = None;
        egui::Window::new(theme::strong("Export history"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(640.0)
            .default_height(420.0)
            .show(ctx, |ui| {
                if self.store.exports.is_empty() {
                    ui.add(egui::Label::new(muted(
                        "No exports yet. Every export is recorded here with its .doc.json snapshot, so any handed-out version can be regenerated exactly.",
                    )));
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_salt("history_scroll")
                    .show(ui, |ui| {
                        let recs: Vec<model::ExportRecord> = self.store.exports.clone();
                        for rec in recs.iter().rev() {
                            kit::card()
                                .inner_margin(egui::Margin::same(12))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Label::new(bankui::fmt_date(rec.time)));
                                        ui.add(
                                            egui::Label::new(
                                                theme::strong(rec.exam_name.clone())
                                                    .size(14.0)
                                                    .color(theme::ink()),
                                            )
                                            .wrap(),
                                        );
                                    });
                                    ui.add(egui::Label::new(muted(
                                        format!(
                                            "{} · {} · {} version{}",
                                            rec.what.join(" + "),
                                            rec.format.to_uppercase(),
                                            rec.versions,
                                            if rec.versions == 1 { "" } else { "s" },
                                        ),
                                    )));
                                    let printed: Vec<String> = rec
                                        .files
                                        .iter()
                                        .filter(|f| !f.ends_with(".doc.json"))
                                        .map(|f| truncate_str(f, 70))
                                        .collect();
                                    if !printed.is_empty() {
                                        ui.add(
                                            egui::Label::new(muted(
                                                egui::RichText::new(printed.join("\n")).size(10.5),
                                            ))
                                            .wrap(),
                                        );
                                    }
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        if kit::ghost_button(ui, "Show files").clicked() {
                                            let dir = rec
                                                .files
                                                .first()
                                                .map(|f| {
                                                    std::path::Path::new(f)
                                                        .parent()
                                                        .map(|p| p.to_path_buf())
                                                        .unwrap_or_default()
                                                })
                                                .unwrap_or_default();
                                            xdg_open(&dir);
                                        }
                                        if kit::ghost_button(ui, "Regenerate").clicked() {
                                            match regenerate(rec) {
                                                Ok(n) => {
                                                    note = Some((
                                                        format!("Regenerated {n} files",),
                                                        false,
                                                    ));
                                                }
                                                Err(e) => {
                                                    note = Some((e, true));
                                                }
                                            }
                                        }
                                    });
                                });
                            ui.add_space(6.0);
                        }
                    });
            });
        self.history_open = open;
        if let Some((msg, is_error)) = note.clone() {
            screens::toast(self, msg.as_str(), is_error);
        }
    }
}

/// Re-renders every `.doc.json` snapshot in a record's files, overwriting the
/// corresponding PDF/DOCX next to it.
fn regenerate(rec: &model::ExportRecord) -> Result<usize, String> {
    let docx = rec.format == "docx";
    let mut done = 0usize;
    for p in &rec.files {
        if !p.ends_with(".doc.json") {
            continue;
        }
        let raw = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
        let snap: SnapDoc = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let dir = std::path::Path::new(p)
            .parent()
            .map(|d| d.to_path_buf())
            .unwrap_or_default();
        let doc = snap.to_doc();
        let bytes = if docx {
            crate::export::render_docx(&doc)?
        } else {
            crate::export::render_pdf(&doc)?
        };
        std::fs::write(dir.join(&snap.out_file), bytes).map_err(|e| e.to_string())?;
        done += 1;
    }
    if done == 0 {
        return Err("no .doc.json snapshots in this record".to_string());
    }
    Ok(done)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Answer, Difficulty, Exam, ExamItem, Pair, Part, PartKind, Question};

    fn mc(answers: &[(&str, bool, bool)]) -> Kind {
        Kind::Mc {
            answers: answers
                .iter()
                .map(|(t, c, l)| Answer { text: t.to_string(), correct: *c, locked: *l })
                .collect(),
        }
    }

    fn q(id: &str, stem: &str, kind: Kind) -> Question {
        Question {
            id: id.to_string(),
            stem: stem.to_string(),
            kind,
            points: Some(2.0),
            difficulty: Some(Difficulty::Medium),
            topics: vec![],
            image: None,
            explanation: None,
            figure: None,
            created: 100,
            modified: 100,
        }
    }

    fn exam(items: Vec<ExamItem>) -> Exam {
        Exam {
            id: "exam1".to_string(),
            name: "Quiz".to_string(),
            header: "Name: ______ Class:".to_string(),
            title: None,
            items,
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 2,
            modified: 0,
        }
    }

    fn item(bank: &str, qid: &str) -> ExamItem {
        ExamItem { bank_id: bank.to_string(), question_id: qid.to_string() }
    }

    #[test]
    fn lcg_shuffles_are_deterministic() {
        let a = shuffled_indices(30, &mut lcg(1234));
        let a2 = shuffled_indices(30, &mut lcg(1234));
        let b = shuffled_indices(30, &mut lcg(5678));
        assert_eq!(a, a2, "same seed must produce the same permutation");
        assert_ne!(a, b, "different seeds should differ for n=30");
        // And it really is a permutation.
        let mut sorted_a = a.clone();
        sorted_a.sort();
        assert_eq!(sorted_a, (0..30).collect::<Vec<_>>());
    }

    #[test]
    fn version_labels_increment() {
        assert_eq!(version_letter(0), "A");
        assert_eq!(version_letter(1), "B");
        assert_eq!(version_letter(7), "H");
        assert_eq!(version_letter(25), "Z");
        assert_eq!(version_letter(26), "AA");
    }

    #[test]
    fn mc_maps_key_and_lettered_choices() {
        let question = q(
            "q1",
            "Which planet?",
            mc(&[("Venus", false, false), ("Mars", true, false), ("Sun", false, false)]),
        );
        let m = map_question(1, &question);
        assert_eq!(m.dq.text, vec!["Which planet?"]);
        assert_eq!(m.dq.choices, vec!["A. Venus", "B. Mars", "C. Sun"]);
        assert_eq!(m.dq.key_text, "B");
        assert_eq!(m.dq.rule_lines, 0);
        assert_eq!(m.dq.points, Some(2.0));
    }

    #[test]
    fn tf_blank_short_map_key_text() {
        let tf = map_question(1, &q("q1", "The sky is green.", Kind::Tf { correct: false }));
        assert_eq!(tf.dq.choices, vec!["True", "False"]);
        assert_eq!(tf.dq.key_text, "False");

        let blank = map_question(
            1,
            &q(
                "q2",
                "Water is ___ at sea level.",
                Kind::Blank { accept: vec![vec!["liquid".into(), "a liquid".into()]] },
            ),
        );
        assert_eq!(blank.dq.rule_lines, 1);
        assert_eq!(blank.dq.key_text, "liquid / a liquid");

        let short = map_question(
            1,
            &q("q3", "Explain photosynthesis.", Kind::Short { sample: "Plants convert light…".into() }),
        );
        assert_eq!(short.dq.rule_lines, 3);
        assert_eq!(short.dq.key_text, "Plants convert light…");
    }

    #[test]
    fn matching_maps_left_items_and_numbered_key() {
        let kind = Kind::Matching {
            pairs: vec![
                Pair { left: "Dog".into(), right: "Bark".into() },
                Pair { left: "Cat".into(), right: "Meow".into() },
                Pair { left: "Cow".into(), right: "Moo".into() },
            ],
            distractors: vec!["Roar".into(), "Quack".into()],
        };
        let m = map_question(3, &q("q1", "Match the parts.", kind));
        // Left items only — no stem line, per the exporters' convention.
        assert_eq!(m.dq.text, vec!["Dog", "Cat", "Cow"]);
        assert_eq!(
            m.dq.choices,
            vec!["A. Bark", "B. Meow", "C. Moo", "D. Roar", "E. Quack"]
        );
        assert_eq!(m.dq.key_text, "1-A 2-B 3-C");
        assert_eq!(m.dq.number, 3);
    }

    #[test]
    fn multipart_is_one_text_entry_with_key_parts() {
        let kind = Kind::Multipart {
            parts: vec![
                Part {
                    stem: "2 + 2 = ?".into(),
                    kind: PartKind::Mc {
                        answers: vec![Answer { text: "4".into(), correct: true, locked: false }],
                    },
                },
                Part {
                    stem: "Sky color?".into(),
                    kind: PartKind::Short { sample: "Blue".into() },
                },
            ],
        };
        let m = map_question(1, &q("q1", "Answer the parts.", kind));
        assert_eq!(m.dq.text.len(), 1);
        let s = &m.dq.text[0];
        assert!(s.starts_with("Answer the parts."), "{s}");
        assert!(s.contains("a. 2 + 2 = ?"), "{s}");
        assert!(s.contains("A. 4"), "{s}");
        assert!(s.contains("b. Sky color?"), "{s}");
        assert_eq!(m.dq.rule_lines, 3); // one short part → 3 ruled lines
        assert!(m.dq.key_text.contains("a) A"), "{}", m.dq.key_text);
        assert!(m.dq.key_text.contains("b) Blue"), "{}", m.dq.key_text);
    }

    #[test]
    fn matching_consumes_multiple_print_numbers() {
        let qs = vec![
            q("q1", "plain", mc(&[("A", true, false)])),
            q(
                "q2",
                "match",
                Kind::Matching {
                    pairs: vec![
                        Pair { left: "1".into(), right: "x".into() },
                        Pair { left: "2".into(), right: "y".into() },
                        Pair { left: "3".into(), right: "z".into() },
                    ],
                    distractors: vec![],
                },
            ),
            q("q3", "plain2", Kind::Tf { correct: true }),
        ];
        assert_eq!(doc_numbers(&qs), vec![1, 2, 5]);
    }

    #[test]
    fn build_doc_maps_exam_fields_and_key_variant() {
        let e = exam(vec![item("b", "q1")]);
        let mapped = vec![map_question(
            1,
            &q(
                "q1",
                "Which planet?",
                mc(&[("Venus", false, false), ("Mars", true, false)]),
            ),
        )];
        let student = build_doc(&e, &mapped, None, false);
        assert_eq!(student.title, "Quiz");
        assert_eq!(student.header_line, "Name: ______ Class:");
        assert_eq!(student.heading_pt, 16.0);
        assert_eq!(student.body_pt, 11.0);
        assert_eq!(student.margin_mm, 18.0);
        assert_eq!(student.ruled_blanks, 2);
        assert!(student.answers_inline);
        assert!(!student.is_key);
        assert!(student.version_label.is_none());

        let key = build_doc(&e, &mapped, Some("Version B".into()), true);
        assert_eq!(key.title, "Quiz — Answer Key");
        assert!(key.is_key);
        assert_eq!(key.version_label.as_deref(), Some("Version B"));
        assert!(key.questions.iter().all(|dq| dq.choices.is_empty()));
        assert_eq!(key.questions[0].key_text, "B");
    }

    #[test]
    fn shuffle_answers_skips_locked_and_reindexes_key() {
        // Locked correct answer at slot 0 must stay there; movable answer is
        // permuted within the movable set; the key still reads "A".
        let mut question = q(
            "q1",
            "Pick.",
            mc(&[
                ("Always first", true, true),     // locked
                ("One", false, false),
                ("Two", false, false),
                ("Three", false, false),
                ("Four", false, false),
            ]),
        );
        let before: Vec<String> = match &question.kind {
            Kind::Mc { answers } => answers.iter().map(|a| a.text.clone()).collect(),
            _ => panic!(),
        };
        shuffle_answers(&mut question, &mut lcg(7));
        let answers = match &question.kind {
            Kind::Mc { answers } => answers.clone(),
            _ => panic!(),
        };
        // Multiset preserved (sorted on both sides — `before` is in bank order).
        let mut got: Vec<String> = answers.iter().map(|a| a.text.clone()).collect();
        got.sort();
        let mut want = before.clone();
        want.sort();
        assert_eq!(got, want);
        // Locked answer kept its slot and its correctness.
        assert_eq!(answers[0].text, "Always first");
        assert!(answers[0].correct);
        assert_eq!(map_question(1, &question).dq.key_text, "A");

        // And with a seed that actually moves things, letters reindex correctly.
        let mut moved_any = false;
        for seed in [1u64, 2, 3, 4, 5, 6, 7, 9, 15, 22] {
            let mut q3 = q(
                "q3",
                "Pick3.",
                mc(&[("1", false, false), ("2", true, false), ("3", false, false), ("4", false, false)]),
            );
            let texts_before: Vec<String> = {
                match &q3.kind {
                    Kind::Mc { answers } => answers.iter().map(|a| a.text.clone()).collect(),
                    _ => panic!(),
                }
            };
            shuffle_answers(&mut q3, &mut lcg(seed));
            let out_texts: Vec<String> = match &q3.kind {
                Kind::Mc { answers } => answers.iter().map(|a| a.text.clone()).collect(),
                _ => panic!(),
            };
            // Multiset preserved always.
            let mut sorted_out = out_texts.clone();
            sorted_out.sort();
            let mut sorted_before = texts_before.clone();
            sorted_before.sort();
            assert_eq!(sorted_out, sorted_before);
            if out_texts != texts_before {
                moved_any = true;
                let idx = out_texts.iter().position(|t| t == "2").unwrap();
                let key = map_question(1, &q3).dq.key_text;
                assert_eq!(key, letter(idx));
            }
        }
        assert!(moved_any, "some seed among the chosen set should move the answers");
    }

    #[test]
    fn math_degrades_to_unicode_in_exports() {
        let stem = r"Evaluate $\frac{1}{2}+\sqrt{x}$ now.";
        let ans = &[(r"$\frac{1}{2}$", true, false), ("2", false, false)];
        let m = map_question(1, &q("q1", stem, mc(ans)));
        let joined = format!("{:?} {:?}", m.dq.text, m.dq.choices);
        assert!(!joined.contains('$'), "dollar signs leaked: {joined}");
        assert!(!joined.contains("frac") && !joined.contains("sqrt"), "latex leaked: {joined}");
        assert!(joined.contains('\u{221a}'), "expected a square-root sign: {joined}");
        // And the real renderers accept it.
        let e = exam(vec![item("b", "q1")]);
        let snap = SnapDoc::from_parts(&e, &[m], None, false, "x.pdf".to_string());
        let doc = snap.to_doc();
        assert!(crate::export::render_pdf(&doc).unwrap().starts_with(b"%PDF"));
        assert!(crate::export::render_docx(&doc).unwrap().starts_with(b"PK"));
    }

    #[test]
    fn snapshot_roundtrips_without_images() {
        let e = exam(vec![item("b", "q1")]);
        let mapped = vec![map_question(
            1,
            &q("q1", "Stem?", mc(&[("A", false, false), ("B", true, false)])),
        )];
        let snap = SnapDoc::from_parts(
            &e,
            &mapped,
            Some("Version A".into()),
            false,
            "Quiz-Version A.pdf".to_string(),
        );
        assert_eq!(snap.out_file, "Quiz-Version A.pdf");

        let json = serde_json::to_string(&snap).unwrap();
        let back: SnapDoc = serde_json::from_str(&json).unwrap();
        let doc = back.to_doc();
        assert_eq!(doc.title, "Quiz");
        assert_eq!(doc.version_label.as_deref(), Some("Version A"));
        assert_eq!(doc.questions[0].choices, vec!["A. A", "B. B"]);
        assert_eq!(doc.questions[0].key_text, "B");
        assert!(!doc.is_key);
    }

    #[test]
    fn key_doc_carries_explanation() {
        let mut question = q("q1", "Why?", mc(&[("Yes", true, false)]));
        question.explanation = Some("Because light is energy.".to_string());
        let m = map_question(1, &question);
        // MC keys answer with the letter; the explanation rides along on a new line.
        assert!(m.dq.key_text.starts_with("A"), "{}", m.dq.key_text);
        assert!(m.dq.key_text.contains("Explanation: Because light is energy."));
    }
    #[test]
    fn export_mapping_keeps_stem_markup_and_skips_rules_for_table_blanks() {
        let stem = "Name them.\nFormula | Name\nKI | ___\nSO\u{2083} | ___";
        let m = map_question(1, &q("q1", stem, Kind::Blank { accept: vec![] }));
        assert_eq!(m.dq.text, vec![stem.to_string()], "raw markup reaches the exporters");
        assert_eq!(m.dq.rule_lines, 0, "table blanks are answer cells");
        let m = map_question(1, &q("q2", "A: ___  B: ___", Kind::Blank { accept: vec![] }));
        assert_eq!(m.dq.rule_lines, 2);
    }
}

#[cfg(test)]
mod stem_ui_tests {
    use super::*;

    #[test]
    fn part_labels_print_once() {
        assert_eq!(own_part_label("a. NH₃\nNumber of charge clouds: ___"), Some("a"));
        assert_eq!(own_part_label("(b) CO₂"), Some("b"));
        assert_eq!(own_part_label("ii) why"), Some("ii"));
        assert_eq!(own_part_label("3. third"), Some("3"));
        assert_eq!(own_part_label("NH₃ is polar"), None);
        assert_eq!(own_part_label("Ca. 5 g"), None);
        assert_eq!(own_part_label("e.g. this"), None);
        assert_eq!(part_line(0, "a. NH₃"), "a. NH₃");
        assert_eq!(part_line(1, "CO₂"), "b. CO₂");
        assert_eq!(part_key_label(0, "a. NH₃"), "a");
        assert_eq!(part_key_label(2, "Why?"), "c");
        assert_eq!(split_part_label(0, "a. NH₃\nshape: ___"), ("a.".to_string(), "NH₃\nshape: ___".to_string()));
        assert_eq!(split_part_label(1, "CO₂"), ("b.".to_string(), "CO₂".to_string()));
    }

    #[test]
    fn multipart_export_text_is_labelled_once() {
        let q = model::Question {
            stem: "Do all:\n• Draw it.".into(),
            kind: Kind::Multipart {
                parts: vec![
                    model::Part { stem: "a. NH₃".into(), kind: PartKind::Short { sample: "pyramidal".into() } },
                    model::Part { stem: "CO₂".into(), kind: PartKind::Tf { correct: true } },
                ],
            },
            id: "q".into(),
            points: Some(1.0),
            difficulty: None,
            topics: vec![],
            image: None,
            explanation: None,
            figure: None,
            created: 0,
            modified: 0,
        };
        let m = map_question_raw(4, &q);
        let t = &m.dq.text[0];
        assert!(t.contains("\na. NH₃") && !t.contains("a. a."), "{t}");
        assert!(t.contains("\nb. CO₂ (True / False)"), "{t}");
        assert!(m.dq.key_text.contains("a) pyramidal") && m.dq.key_text.contains("b) True"), "{}", m.dq.key_text);
    }

    #[test]
    fn blanks_in_table_cells_get_no_ruled_lines() {
        assert_eq!(ruled_blanks("Name:\nFormula | Name\nKI | ___\nNO | ___"), 0);
        assert_eq!(ruled_blanks("A: ___ B: ___"), 2);
        assert_eq!(ruled_blanks("Fill:\nx | y\n1 | ___\n2 | ___\nand ___"), 1);
        assert_eq!(ruled_blanks("no blanks"), 1);
    }

    #[test]
    fn rich_detection_keeps_plain_stems_on_the_old_path() {
        assert!(!needs_rich("Which element is the most electronegative?"));
        assert!(needs_rich("a\nb"));
        assert!(needs_rich("the *most* one"));
        assert!(needs_rich("H_{2}O"));
    }

    #[test]
    fn later_pages_reserve_room_for_the_repeated_header() {
        let bp = 11.0 * PT;
        assert_eq!(running_header_h("", bp), 0.0);
        assert_eq!(running_header_h("   ", bp), 0.0);
        let h = running_header_h("Name: ____", bp);
        assert!(h > bp, "the header band is at least a line tall");
        assert_eq!(page_content_top(0.0, 30.0, h), 30.0 * 1.05, "page 1: header is part of the body");
        assert_eq!(page_content_top(1.0, 30.0, h), PAGE_H + 30.0 * 1.05 + h);
        assert_eq!(page_content_top(2.0, 30.0, 0.0), 2.0 * PAGE_H + 30.0 * 1.05);
    }

    #[test]
    fn duplicate_puts_the_copy_right_after_the_item() {
        let it = |q: &str| ExamItem { bank_id: "b".into(), question_id: q.into() };
        let out = with_copy_after(vec![it("a"), it("b"), it("c")], 1, &"b".to_string(), &"b2".to_string());
        let ids: Vec<&str> = out.iter().map(|x| x.question_id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "b2", "c"]);
        let out = with_copy_after(vec![it("a")], 0, &"b".to_string(), &"a2".to_string());
        assert_eq!(out[1].question_id, "a2");
        let out = with_copy_after(Vec::new(), 3, &"b".to_string(), &"z".to_string());
        assert_eq!(out.len(), 1, "an out-of-range index appends");
    }
}
