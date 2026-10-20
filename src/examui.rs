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
use crate::layout::KeyLayout;
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

fn list_id() -> egui::Id { egui::Id::NULL.with("examui_list") }
fn edit_id() -> egui::Id { egui::Id::NULL.with("examui_edit") }
fn export_id() -> egui::Id { egui::Id::NULL.with("examui_export") }

fn now() -> i64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `model::ExamItem` doesn't derive PartialEq; equality of item lists is pairwise
/// equality of every field (a question may legally appear twice — this compares
/// positions, as reorder semantics need; section headings, work space and answer
/// order count as changes too).
fn same_items(a: &[ExamItem], b: &[ExamItem]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(x, y)| {
            x.bank_id == y.bank_id
                && x.question_id == y.question_id
                && x.section == y.section
                && x.work_lines == y.work_lines
                && x.answer_order == y.answer_order
                && x.part_work == y.part_work
                && x.answer_cols() == y.answer_cols()
        })
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
    /// A card's "…" menu chose Open.
    open: Option<Id>,
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
    /// Shelf Question Type filter (empty = all types).
    kinds: Vec<QKind>,
    /// Shelf sort (index into `SORTS`).
    sort: u8,
    /// Banks open as shelf tabs (the selected one is `App.panel_bank`).
    open_banks: Vec<Id>,
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
    /// File > Discard changes confirmation open.
    confirm_discard: bool,
    save_as_open: bool,
    save_as_name: String,
    /// Margins dialog open (the page draws live guide lines at the margins).
    margins_open: bool,
    /// Margins dialog: editing each side separately.
    margins_each: bool,
    /// Last laid-out page geometry (ml, mr, mt, mb, content width) in px.
    geo: Option<(f32, f32, f32, f32, f32)>,
    /// Work-space handle being dragged: (item index, live line count, pointer y and line
    /// count when the drag began).
    ws_drag: Option<(usize, u8, f32, u8)>,
    /// A multipart part's work-space handle being dragged: (item index, part index, live
    /// line count, pointer y and line count when the drag began).
    pws_drag: Option<(usize, usize, u8, f32, u8)>,
}

#[derive(Clone)]
struct ExportUi {
    docx: bool,
    student: bool,
    key: bool,
    shuffle_q: bool,
    shuffle_a: bool,
    versions: u32,
    /// Export although questions still have `[picture]` placeholders (reset when the dialog closes).
    anyway: bool,
    /// The answer key's layout (Test Parrot's answer section by default).
    key_layout: KeyLayout,
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
            anyway: false,
            key_layout: KeyLayout::Section,
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
#[cfg(test)]
fn part_line(pi: usize, stem: &str) -> String {
    part_line_styled(pi, stem, model::PaperStyle::Standard)
}

/// [`part_line`] in a paper style ("(a)" labels on Exam Board).
fn part_line_styled(pi: usize, stem: &str, style: model::PaperStyle) -> String {
    if own_part_label(stem).is_some() {
        stem.trim_start().to_string()
    } else {
        format!("{} {}", style.part_label(pi), stem)
    }
}

/// A part split into its printed label ("a.") and the rest of its stem, for a hanging indent.
#[cfg(test)]
fn split_part_label(pi: usize, stem: &str) -> (String, String) {
    split_part_label_styled(pi, stem, model::PaperStyle::Standard)
}

/// [`split_part_label`] in a paper style ("(a)" on Exam Board).
fn split_part_label_styled(pi: usize, stem: &str, style: model::PaperStyle) -> (String, String) {
    let t = stem.trim_start();
    if own_part_label(t).is_some() {
        let end = t.find(['.', ')']).map_or(0, |i| i + 1);
        return (t[..end].to_string(), t[end..].trim_start().to_string());
    }
    (style.part_label(pi), stem.to_string())
}

/// Ruled answer lines under a fill-in-the-blank item: one per `___` outside tables (a blank in a
/// table cell is written in the cell), at least one when the stem has no blank at all.
fn ruled_blanks(stem: &str) -> usize {
    let total = stem.matches("___").count();
    if total == 0 {
        return 1;
    }
    let mut in_tables = 0usize;
    crate::richtext::walk(&crate::richtext::parse(stem), &mut |b| {
        if let crate::richtext::Block::Table { rows, .. } = b {
            in_tables += rows.iter().flatten().map(|c| crate::richtext::line_text(c).matches("___").count()).sum::<usize>();
        }
    });
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
    let exam = model::Exam { paper_style: Default::default(), margins: None,
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
                        kit::view_toggle(ui, &mut st.mode, "exam_view");
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

        // "Open" from a card's "…" menu.
        if let Some(id) = st.open.take() {
            open_exam_screen(self, &id);
        }
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
            if ui.button("Open").clicked() {
                st.open = Some(row.id.clone());
            }
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

/// The page thumbnail area of an exam card (points): the Exams screen and the Home screen's
/// recent-exam cards draw the first page at this same size, so both show the same detail
/// (and share one cached rasterisation).
pub(crate) const CARD_THUMB: Vec2 = Vec2::new(112.0, 152.0);

fn draw_exam_cards(app: &mut crate::App, ui: &mut egui::Ui, rows: &[ExamRow], st: &mut ListUi) {
    let gap = 14.0;
    let avail = ui.available_width();
    let n = (((avail + gap) / (300.0 + gap)).floor() as usize).max(1);
    let card_w = ((avail - gap * (n as f32 - 1.0)) / n as f32).floor();
    let card_h = CARD_THUMB.y + 24.0;
    let thumb_w = CARD_THUMB.x;
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
            let thumb_area = Rect::from_min_size(rect.left_top() + Vec2::new(14.0, 12.0), Vec2::new(thumb_w, card_h - 24.0));
            let ctx = ui.ctx().clone();
            let tp = ui.painter().clone();
            if paint_exam_thumb(app, &ctx, &tp, thumb_area, &row.id).is_none() {
                paint_thumb(&tp, rect.left_top() + Vec2::new(14.0 + thumb_w / 2.0, 14.0 + 60.0), -0.05, row.q.max(2));
            }
            let tx = rect.left() + 14.0 + thumb_w + 12.0;
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

        // File-menu Discard changes (confirm) modal.
        if st.confirm_discard {
            let mut open = true;
            egui::Window::new(theme::strong("Discard changes"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(420.0)
                .show(ctx, |ui| {
                    ui.add(egui::Label::new("Go back to this exam as it was at your last Save (or when you opened it)? Edits since then are thrown away.").wrap());
                    ui.add_space(12.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if danger_button(ui, "Discard").clicked() {
                            discard_changes(self, ctx, &exam_id);
                            st.confirm_discard = false;
                        }
                        if kit::ghost_button(ui, "Cancel").clicked() {
                            st.confirm_discard = false;
                        }
                    });
                });
            if !open {
                st.confirm_discard = false;
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

        if st.margins_open {
            draw_margins_dialog(self, ctx, &exam_id, &mut st);
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
    let glyph = match ico {
        Ico::Undo => egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
        Ico::Redo => egui_phosphor::regular::ARROW_CLOCKWISE,
    };
    p.text(rect.center(), egui::Align2::CENTER_CENTER, glyph, theme::icon_font(17.0), col);
    resp
}

// ----- working copy: Save / Discard changes ------------------------------------

/// Every edit is written to disk at once (nothing is ever lost); on top of that the editor
/// keeps Test Parrot's Working Copy checkpoint in memory: File > Save marks the current
/// state, File > Discard changes goes back to it. The checkpoint starts as the exam when
/// the editor opened (it does not survive an app restart).
fn baseline_id(exam_id: &Id) -> egui::Id {
    egui::Id::new(("qs_exam_baseline", exam_id))
}

/// Exam content for checkpoint comparison (the edit timestamp does not count).
fn exam_sig(e: &model::Exam) -> String {
    let mut e = e.clone();
    e.modified = 0;
    serde_json::to_string(&e).unwrap_or_default()
}

/// True when the exam differs from its Save checkpoint (creating the checkpoint if missing).
fn working_copy_dirty(app: &crate::App, ctx: &egui::Context, exam_id: &Id) -> bool {
    let Some(cur) = app.store.exam(exam_id) else { return false };
    let id = baseline_id(exam_id);
    match ctx.data(|d| d.get_temp::<model::Exam>(id)) {
        Some(b) => exam_sig(&b) != exam_sig(cur),
        None => {
            ctx.data_mut(|d| d.insert_temp(id, cur.clone()));
            false
        }
    }
}

fn save_checkpoint(app: &mut crate::App, ctx: &egui::Context, exam_id: &Id) {
    if let Some(cur) = app.store.exam(exam_id).cloned() {
        ctx.data_mut(|d| d.insert_temp(baseline_id(exam_id), cur));
        screens::toast(app, "Saved", false);
    }
}

fn discard_changes(app: &mut crate::App, ctx: &egui::Context, exam_id: &Id) {
    let Some(b) = ctx.data(|d| d.get_temp::<model::Exam>(baseline_id(exam_id))) else { return };
    if let Some(e) = app.store.exam_mut(exam_id) {
        *e = b;
        e.modified = now();
    }
    app.store.save_exams();
    app.undo.clear();
    app.redo.clear();
    screens::toast(app, "Changes discarded", false);
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

                // Saved indicator: every edit is written straight through; "Edited" means the
                // exam differs from its File > Save checkpoint (Discard changes goes back).
                let dirty = working_copy_dirty(app, ui.ctx(), exam_id);
                let (sr, sresp) = ui.allocate_exact_size(Vec2::new(13.0, 16.0), Sense::hover());
                let sc = sr.center();
                if dirty {
                    ui.painter().circle_stroke(sc, 4.5, Stroke::new(1.6, theme::accent_col()));
                    ui.painter().circle_filled(sc, 1.6, theme::accent_col());
                } else {
                    let w = Stroke::new(1.8, theme::accent_col());
                    ui.painter().line_segment([sc + Vec2::new(-4.5, 0.5), sc + Vec2::new(-1.5, 3.5)], w);
                    ui.painter().line_segment([sc + Vec2::new(-1.5, 3.5), sc + Vec2::new(4.5, -3.5)], w);
                }
                let tip = if dirty {
                    "Edited since your last Save. Every change is already on disk; File > Discard changes goes back to the last Save."
                } else {
                    "Everything in this working copy matches your last Save."
                };
                sresp.on_hover_text(tip);
                if !compact {
                    ui.label(RichText::new(if dirty { "Edited" } else { "Saved" }).family(theme::bold_family()).size(12.0).color(theme::accent_col()))
                        .on_hover_text(tip);
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
                    let dirty = working_copy_dirty(app, ui.ctx(), exam_id);
                    if ui
                        .add_enabled_ui(dirty, |ui| kit::ghost_button(ui, "Save"))
                        .inner
                        .on_hover_text("Save a checkpoint of this exam (File > Discard changes returns to it)")
                        .clicked()
                    {
                        save_checkpoint(app, ui.ctx(), exam_id);
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
        let dirty = working_copy_dirty(app, ui.ctx(), exam_id);
        if ui.add_enabled(dirty, egui::Button::new("Save")).clicked() {
            save_checkpoint(app, ui.ctx(), exam_id);
        }
        if ui.add_enabled(dirty, egui::Button::new("Discard changes")).clicked() {
            st.confirm_discard = true;
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
        ui.separator();
        ui.label(kit::small_caps_text("Vary"));
        if ui.button("Shuffle question order").on_hover_text("Shuffles the questions within each section").clicked() {
            let mut rnd = editor_rng();
            apply_items_fn(app, exam_id, |it| with_all_sections_shuffled(it, &mut rnd));
        }
        if ui.button("Shuffle answer order").on_hover_text("Shuffles every multiple-choice question's answers on this exam").clicked() {
            shuffle_all_answers(app, exam_id);
        }
        if ui.button("Restore bank answer order").clicked() {
            apply_items_fn(app, exam_id, |mut it| {
                it.iter_mut().for_each(|x| x.answer_order.clear());
                it
            });
        }
    });
    ui.menu_button("Format", |ui| {
        ui.set_min_width(300.0);
        kit::small_caps(ui, "Page format");
        ui.add_space(6.0);
        let mut changed = false;
        if let Some(ex) = app.store.exam(exam_id) {
            let mut f = ex.clone();
            for (label, which) in [("Heading", 0), ("Text size", 1), ("Answer blanks", 3)] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).family(theme::bold_family()).color(theme::text_col()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        changed |= format_control(ui, which, &mut f, "menu", &mut st.margins_open);
                    });
                });
                ui.add_space(4.0);
            }
            ui.separator();
            kit::small_caps(ui, "Paper style");
            for ps in model::PaperStyle::ALL {
                let on = f.paper_style == ps;
                let r = ui.add(
                    egui::Button::new(
                        RichText::new(format!("{}\n{}", ps.label(), ps.blurb()))
                            .size(12.0)
                            .color(if on { theme::ink() } else { theme::text_col() }),
                    )
                    .selected(on)
                    .min_size(Vec2::new(ui.available_width(), 0.0)),
                );
                if r.clicked() && !on {
                    f.paper_style = ps;
                    changed = true;
                }
            }
            ui.separator();
            if ui.button("Margins…").clicked() {
                st.margins_open = true;
            }
            if changed {
                write_format(app, exam_id, &f);
            }
        }
    });
}

// ----- format bar -----------------------------------------------------------

/// Inches with up to two decimals ("0.75", "1").
fn fmt_inches(mm: f32) -> String {
    let v = (mm / 25.4 * 100.0).round() / 100.0;
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Shuffles every multiple-choice question's answers on this exam (one undo step).
fn shuffle_all_answers(app: &mut crate::App, exam_id: &Id) {
    let Some(ex) = app.store.exam(exam_id) else { return };
    let mut rnd = editor_rng();
    let orders: Vec<Option<Vec<usize>>> = ex
        .items
        .iter()
        .map(|it| app.store.question(&it.bank_id, &it.question_id).and_then(|q| shuffled_answer_order(q, &it.answer_order, &mut rnd)))
        .collect();
    apply_items_fn(app, exam_id, |mut it| {
        for (x, o) in it.iter_mut().zip(orders) {
            if let Some(o) = o {
                x.answer_order = o;
            }
        }
        it
    });
}

/// The Margins dialog (Format > Margins…): all sides, or each side, in inches; the page
/// draws live guide lines while it is open.
fn draw_margins_dialog(app: &mut crate::App, ctx: &egui::Context, exam_id: &Id, st: &mut EditUi) {
    let Some(ex) = app.store.exam(exam_id) else { return };
    let mut f = ex.clone();
    let mut m = f.page_margins();
    if !m.uniform() {
        st.margins_each = true;
    }
    let mut open = true;
    let mut changed = false;
    let pos = egui::pos2(150.0, 160.0);
    egui::Window::new(kit::small_caps_text("Margins"))
        .id(egui::Id::new("qs_margins_dialog"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_pos(pos)
        .default_width(250.0)
        .show(ctx, |ui| {
            let inch = |ui: &mut egui::Ui, label: &str, v: &mut f32| -> bool {
                let mut i = *v / 25.4;
                let mut ch = false;
                ui.horizontal(|ui| {
                    ui.add_sized([70.0, 20.0], egui::Label::new(label));
                    let r = ui.add(egui::DragValue::new(&mut i).range(0.25..=2.5).speed(0.01).fixed_decimals(2).suffix(" in"));
                    if r.changed() {
                        *v = (i * 25.4 * 100.0).round() / 100.0;
                        ch = true;
                    }
                });
                ch
            };
            if st.margins_each {
                changed |= inch(ui, "Top", &mut m.top);
                changed |= inch(ui, "Right", &mut m.right);
                changed |= inch(ui, "Bottom", &mut m.bottom);
                changed |= inch(ui, "Left", &mut m.left);
            } else {
                let mut all = m.top;
                if inch(ui, "All sides", &mut all) {
                    m = model::Margins::all(all);
                    changed = true;
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (lab, inch_v) in [("0.5", 0.5_f32), ("0.75", 0.75), ("1", 1.0), ("1.25", 1.25)] {
                    let mm = inch_v * 25.4;
                    if kit::chip_toggle(ui, lab, m.uniform() && (m.top - mm).abs() < 0.05).clicked() {
                        m = model::Margins::all(mm);
                        st.margins_each = false;
                        changed = true;
                    }
                }
            });
            ui.add_space(4.0);
            if ui.checkbox(&mut st.margins_each, "Set each side").changed() && !st.margins_each {
                m = model::Margins::all(m.top);
                changed = true;
            }
        });
    if changed {
        if m.uniform() {
            f.margin_mm = m.top;
            f.margins = None;
        } else {
            f.margins = Some(m);
            f.margin_mm = m.top;
        }
        write_format(app, exam_id, &f);
    }
    if !open {
        st.margins_open = false;
    }
}

fn head_idx(pt: f32) -> usize {
    if pt < 16.5 { 0 } else if pt < 19.5 { 1 } else { 2 }
}

fn write_format(app: &mut crate::App, exam_id: &Id, f: &model::Exam) {
    if let Some(ex) = app.store.exam_mut(exam_id) {
        ex.heading_pt = f.heading_pt;
        ex.body_pt = f.body_pt;
        ex.margin_mm = f.margin_mm;
        ex.margins = f.margins;
        ex.paper_style = f.paper_style;
        ex.ruled_blanks = f.ruled_blanks;
        ex.modified = now();
    }
    app.store.save_exams();
}

/// One format control (0 heading, 1 text size, 2 margins, 3 answer blanks, 4 paper style).
/// Shared by the Format bar, the Format menu and the popover alternative. The margins
/// control is a button that opens the Margins dialog (`margins_open`).
fn format_control(ui: &mut egui::Ui, which: u8, f: &mut model::Exam, salt: &str, margins_open: &mut bool) -> bool {
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
            let m = f.page_margins();
            let label = if m.uniform() { format!("{} in", fmt_inches(m.top)) } else { "Each side".to_string() };
            let r = ui.add(egui::Button::new(label).selected(*margins_open)).on_hover_text("Margins (all sides or each side)");
            if r.clicked() {
                *margins_open = !*margins_open;
            }
        }
        4 => {
            let cur = f.paper_style;
            let mut sel = cur;
            egui::ComboBox::from_id_salt(("fmt_style", salt))
                .selected_text(cur.label())
                .width(110.0)
                .show_ui(ui, |ui| {
                    for ps in model::PaperStyle::ALL {
                        ui.selectable_value(&mut sel, ps, ps.label()).on_hover_text(ps.blurb());
                    }
                });
            if sel != cur {
                f.paper_style = sel;
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

fn draw_format_bar(app: &mut crate::App, ui: &mut egui::Ui, exam_id: &Id, st: &mut EditUi) {
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
            let wide = ui.available_width() > 1180.0;
            let mut body = |ui: &mut egui::Ui, range: std::ops::Range<usize>| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (label, which) in [("Paper", 4u8), ("Heading", 0), ("Text size", 1), ("Margins", 2), ("Answer blanks", 3)]
                    .into_iter()
                    .skip(range.start)
                    .take(range.len())
                {
                    ui.label(muted(RichText::new(label).size(11.5)));
                    changed |= format_control(ui, which, &mut f, "bar", &mut st.margins_open);
                    ui.add_space(10.0);
                }
            };
            if wide {
                ui.horizontal(|ui| {
                    body(ui, 0..5);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(muted(RichText::new(&summary).size(12.0)));
                    });
                });
            } else {
                ui.horizontal(|ui| body(ui, 0..3));
                ui.horizontal(|ui| {
                    body(ui, 3..5);
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

/// One printed-looking answer line (ruled, or dotted on Exam Board), one style pitch tall.
fn ruled_line(ui: &mut egui::Ui, width: f32, bp: f32, style: model::PaperStyle) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width.min(ui.available_width()), line_pitch_px(bp, style)),
        egui::Sense::hover(),
    );
    answer_rule(ui.painter(), egui::pos2(rect.left(), rect.bottom() - 2.0), rect.right(), style);
}

/// True when a stem (or option) needs the structured renderer: markup (`crate::richtext`) or math.
/// Everything else keeps the original single-label layout exactly.
fn needs_rich(s: &str) -> bool {
    // `___` blanks draw as writing lines (the rich renderer does that), as they print.
    crate::richtext::has_markup(s) || s.contains('*') || s.contains("___")
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

/// Preview page geometry (paper top on screen, page height, top/bottom margins), shared with
/// [`keep_on_page`] through temp memory while the page lays out.
#[derive(Clone, Copy)]
struct PageGeom {
    p0: f32,
    page_h: f32,
    mt: f32,
    mb: f32,
}

fn geom_id() -> egui::Id {
    egui::Id::new("qs_paper_geom")
}

/// Preview size (px) of one ruled / work-space line in a style.
fn line_pitch_px(bp: f32, style: model::PaperStyle) -> f32 {
    bp * style.rule_pitch() * 0.85
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
    let style = exam.paper_style;
    let (pw, ph) = style.page_pt();
    let (page_w, page_h) = (pw * PT, ph * PT);
    let m = exam.page_margins();
    let mm_px = |mm: f32| mm / 25.4 * 72.0 * PT;
    let mut geo = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32); // ml, mr, mt, mb, cw
    egui::ScrollArea::vertical()
        .id_salt("page_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let aw = ui.available_width();
            let paper_w = page_w.min((aw - 40.0).max(260.0));
            let inset = ((aw - paper_w) / 2.0).max(0.0);
            let bp = exam.body_pt * PT;
            let side_max = ((paper_w - 180.0) / 2.0).max(18.0);
            let ml = mm_px(m.left).clamp(18.0, side_max);
            let mr = mm_px(m.right).clamp(18.0, side_max);
            let mt = mm_px(m.top).max(12.0);
            let mb = mm_px(m.bottom).max(12.0);
            let cw = paper_w - ml - mr;
            geo = (ml, mr, mt, mb, cw);

            ui.add_space(22.0);
            let bg = ui.painter().add(Shape::Noop);
            let mut paper_rect = Rect::NOTHING;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.add_space(inset);
                let r = ui.vertical(|ui| {
                    ui.set_width(paper_w);
                    let y0 = ui.cursor().top();
                    ui.ctx().data_mut(|d| d.insert_temp(geom_id(), PageGeom { p0: y0, page_h, mt, mb }));
                    ui.add_space(mt);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        ui.add_space(ml);
                        ui.vertical(|ui| {
                            ui.set_width(cw);
                            draw_page_article(app, ui, ctx, exam_id, st, &exam, cw, bp, ml, mr);
                        });
                    });
                    ui.add_space(mb);
                    let used = ui.cursor().top() - y0;
                    let pages = (used / page_h).ceil().max(1.0);
                    ui.allocate_space(Vec2::new(1.0, pages * page_h - used));
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
                let y = paper_rect.top() + k as f32 * page_h;
                shapes.extend(Shape::dashed_line(
                    &[Pos2::new(paper_rect.left(), y), Pos2::new(paper_rect.right(), y)],
                    Stroke::new(1.0, PAPER_RULE.gamma_multiply(0.7)),
                    5.0,
                    4.0,
                ));
            }
            // Margins dialog open: live guide lines at the margins of every page.
            if st.margins_open {
                let red = Color32::from_rgb(0xc0, 0x40, 0x30).gamma_multiply(0.75);
                for k in 0..st.pages {
                    let top = paper_rect.top() + k as f32 * page_h;
                    let g = Rect::from_min_max(
                        Pos2::new(paper_rect.left() + ml, top + mt),
                        Pos2::new(paper_rect.right() - mr, top + page_h - mb),
                    );
                    shapes.extend(Shape::dashed_line(
                        &[g.left_top(), g.right_top(), g.right_bottom(), g.left_bottom(), g.left_top()],
                        Stroke::new(1.0, red),
                        4.0,
                        3.0,
                    ));
                }
            }
            ui.painter().set(bg, Shape::Vec(shapes));
            // The header line repeats at the top of every later page (print does the same).
            if !exam.header.trim().is_empty() {
                let x = paper_rect.left() + ml;
                for k in 1..st.pages {
                    let y = paper_rect.top() + k as f32 * page_h + mt;
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
            // Page numbers at the foot of each page; Exam Board adds "Turn over".
            for k in 1..=st.pages {
                let foot = paper_rect.top() + k as f32 * page_h;
                ui.painter().text(
                    Pos2::new(paper_rect.center().x, foot - 12.0),
                    Align2::CENTER_CENTER,
                    k.to_string(),
                    FontId::new(9.0, theme::serif_family()),
                    PAPER_SOFT.gamma_multiply(0.8),
                );
                if style == model::PaperStyle::ExamBoard && k < st.pages {
                    ui.painter().text(
                        Pos2::new(paper_rect.right() - mr, foot - mb * 0.55),
                        Align2::RIGHT_CENTER,
                        "Turn over",
                        FontId::new(bp, theme::serif_bold_family()),
                        PAPER_INK,
                    );
                }
            }
        });
    st.geo = Some(geo);
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
    ml: f32,
    mr: f32,
) {
    let items = &exam.items;
    let style = exam.paper_style;
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
                if pts > 0.0 { format!(" · {} point{}", fmt_number(pts), if pts == 1.0 { "" } else { "s" }) } else { String::new() }
            ))
            .family(theme::serif_family())
            .size(bp * 0.92)
            .color(PAPER_SOFT),
        );
    });
    ui.add_space(8.0);

    // Extra ruled blank lines after the title area.
    for _ in 0..exam.ruled_blanks {
        ruled_line(ui, cw, bp, style);
    }
    ui.add_space(10.0);

    // Pages 2+ repeat the header line at their top; content starts below it.
    let hdr_h = running_header_h(&exam.header, bp);

    // Numbers: matching consumes one number per left item; everything else one.
    // Keeps the preview numbering identical to the printed exam (it continues across sections).
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
    let ranges = section_ranges(items);

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
        if let Some(sec) = &item.section {
            let s_idx = ranges.iter().position(|r| r.start == i).unwrap_or(0);
            keep_on_page(ui, bp * 6.0, hdr_h);
            section_block(app, ui, exam_id, st, i, s_idx, ranges.len(), sec, cw, bp, ml);
        }
        let q = item_question(&app.store, item);
        let number = numbers.get(i).copied().unwrap_or(i as u32 + 1);
        // Keep a question off the dashed page-break line: when last frame's height says it
        // would cross the break, start it at the top of the next page (as the print does).
        let h_id = egui::Id::new(("qs_item_h", i, &item.question_id));
        if let Some(h) = ctx.data(|d| d.get_temp::<f32>(h_id)) {
            keep_on_page(ui, h, hdr_h);
        }
        let rect = draw_item(app, ui, ctx, exam_id, st, i, number, q.as_ref(), cw, bp, ml, mr, style);
        ctx.data_mut(|d| d.insert_temp(h_id, rect.height()));
        st.rows.push((i, rect.translate(Vec2::new(0.0, -slot_shift))));
        ui.add_space(bp * style.q_gap() * 1.6);
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
    keep_on_page(ui, bp * 1.6 + 6.0 + bp * 2.1, hdr_h);
    ui.add(egui::Label::new(ptext("Answer Section", bp + 1.0, true)).wrap());
    ui.add_space(6.0);
    let cell_w = (cw / 4.0).min(130.0);
    for chunk in cells.chunks(4) {
        keep_on_page(ui, bp * 2.1, hdr_h);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for n in chunk {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(cell_w, bp * 2.1), Sense::hover());
                ui.painter().text(
                    Pos2::new(rect.left(), rect.center().y),
                    Align2::LEFT_CENTER,
                    format!("{}  ______", style.number_label(*n)),
                    FontId::new(bp, theme::serif_family()),
                    PAPER_INK,
                );
            }
        });
    }
}

/// A section heading on the page: dashed divider, editable title and directions (written
/// straight through, like the header), and a hover rail in the left margin: move section
/// up / down, merge with the section above, delete the section with its questions.
#[allow(clippy::too_many_arguments)]
fn section_block(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    exam_id: &Id,
    st: &mut EditUi,
    i: usize,
    s_idx: usize,
    n_sections: usize,
    sec: &model::ExamSection,
    cw: f32,
    bp: f32,
    ml: f32,
) {
    let top = ui.cursor().top();
    let x0 = ui.cursor().left();
    ui.add_space(bp * 1.4);
    let line_y = ui.cursor().top();
    let p = ui.painter().clone();
    p.extend(Shape::dashed_line(
        &[Pos2::new(x0, line_y), Pos2::new(x0 + cw, line_y)],
        Stroke::new(1.0, PAPER_RULE.gamma_multiply(0.8)),
        4.0,
        3.0,
    ));
    let label = if sec.is_blank() { format!("Untitled section {}", s_idx + 1) } else { format!("Section {}", s_idx + 1) };
    p.text(
        Pos2::new(x0 + cw, line_y - 2.0),
        Align2::RIGHT_BOTTOM,
        label,
        FontId::new(9.5, theme::serif_family()),
        PAPER_SOFT.gamma_multiply(0.85),
    );
    ui.add_space(bp * 0.45);
    let mut title = sec.title.clone();
    let mut dirs = sec.directions.clone();
    ui.add(
        egui::TextEdit::singleline(&mut title)
            .id_salt(("sec_title", i))
            .hint_text(RichText::new("Section title").font(FontId::new(bp * 1.1, theme::serif_bold_family())).color(PAPER_SOFT.gamma_multiply(0.6)))
            .font(FontId::new(bp * 1.1, theme::serif_bold_family()))
            .text_color(PAPER_INK)
            .frame(egui::Frame::NONE)
            .desired_width(cw),
    );
    ui.add(
        egui::TextEdit::multiline(&mut dirs)
            .id_salt(("sec_dirs", i))
            .hint_text(RichText::new("Directions").font(FontId::new(bp, theme::serif_family())).color(PAPER_SOFT.gamma_multiply(0.6)))
            .font(FontId::new(bp, theme::serif_family()))
            .text_color(PAPER_SOFT)
            .frame(egui::Frame::NONE)
            .desired_rows(1)
            .desired_width(cw),
    );
    ui.add_space(bp * 0.5);
    if title != sec.title || dirs != sec.directions {
        if let Some(it) = app.store.exam_mut(exam_id).and_then(|e| e.items.get_mut(i)) {
            it.section = Some(model::ExamSection { title, directions: dirs });
        }
        if let Some(e) = app.store.exam_mut(exam_id) {
            e.modified = now();
        }
        app.store.save_exams();
    }

    let rect = Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x0 + cw, ui.cursor().top()));
    let show = st.drag.is_none() && ui.rect_contains_pointer(rect.expand2(Vec2::new(ml - 2.0, 2.0)));
    let gx = rect.left() - (ml * 0.5).min(24.0) - 2.0;
    let y = rect.top() + bp * 1.4;
    let mut act: Option<u8> = None;
    // Only the actions that apply, stacked without gaps: up, down, merge up, delete.
    let avail = [(1u8, 1u8, s_idx > 0), (2, 2, s_idx + 1 < n_sections), (3, 5, s_idx > 0), (4, 6, true)];
    let mut k = 0.0;
    for (a, kind, ok) in avail {
        if !ok {
            continue;
        }
        if margin_button(ui, ui.id().with(("sec_btn", i, a)), Pos2::new(gx, y + k * 20.0), kind, show) {
            act = Some(a);
        }
        k += 1.0;
    }
    match act {
        Some(1) => apply_items_fn(app, exam_id, |it| with_section_moved(it, s_idx, -1)),
        Some(2) => apply_items_fn(app, exam_id, |it| with_section_moved(it, s_idx, 1)),
        Some(3) => apply_items_fn(app, exam_id, |it| with_section_merged_up(it, s_idx)),
        Some(4) => {
            apply_items_fn(app, exam_id, |it| with_section_deleted(it, s_idx));
            screens::toast(app, "Section deleted with its questions (Ctrl+Z undoes)", false);
        }
        _ => {}
    }
}

/// Height the repeated header line (Name / Class / Date) takes at the top of pages 2+;
/// 0 when the exam has no header.
fn running_header_h(header: &str, bp: f32) -> f32 {
    if header.trim().is_empty() { 0.0 } else { bp * 1.45 + bp * 0.6 }
}

/// Where page `k` (0-based) of the preview starts its content, relative to the paper top:
/// the top margin, plus the repeated header on pages after the first.
fn page_content_top(k: f32, page_h: f32, mt: f32, hdr_h: f32) -> f32 {
    k * page_h + mt + if k >= 1.0 { hdr_h } else { 0.0 }
}

/// Keeps a block `h` tall off the dashed page-break line and out of the repeated header:
/// when it would cross into the bottom margin it starts at the next page's content top
/// (as the print does); when the cursor sits in a page's header band it moves below it.
fn keep_on_page(ui: &mut egui::Ui, h: f32, hdr_h: f32) {
    let Some(g) = ui.ctx().data(|d| d.get_temp::<PageGeom>(geom_id())) else { return };
    let top = ui.cursor().top();
    let page = ((top - g.p0) / g.page_h).floor().max(0.0);
    let start = g.p0 + page_content_top(page, g.page_h, g.mt, hdr_h);
    if top < start {
        ui.add_space(start - top);
        return;
    }
    let page_end = g.p0 + (page + 1.0) * g.page_h - g.mb;
    if top + h > page_end && h < g.page_h - g.mt - g.mb - hdr_h {
        ui.add_space(g.p0 + page_content_top(page + 1.0, g.page_h, g.mt, hdr_h) - top);
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
/// Kinds: 0 remove, 1 up, 2 down, 3 duplicate, 4 actions (three dots), 5 merge up, 6 delete section.
fn margin_button(ui: &mut egui::Ui, id: egui::Id, center: Pos2, kind: u8, show: bool) -> bool {
    margin_button_resp(ui, id, center, kind, show).is_some_and(|r| r.clicked())
}

/// [`margin_button`] returning its response (None while hidden).
fn margin_button_resp(ui: &mut egui::Ui, id: egui::Id, center: Pos2, kind: u8, show: bool) -> Option<egui::Response> {
    let rect = Rect::from_center_size(center, Vec2::splat(18.0));
    let resp = ui.interact(rect, id, if show { Sense::click() } else { Sense::hover() });
    if !show {
        return None;
    }
    paint_margin_glyph(ui, rect, kind, resp.hovered());
    let tip = match kind {
        0 => "Remove from exam",
        1 => "Move up",
        3 => "Duplicate question",
        4 => "Question actions",
        5 => "Merge with section above",
        6 => "Delete section and its questions",
        _ => "Move down",
    };
    Some(resp.on_hover_text(tip))
}

/// Paints a margin button's glyph (see [`margin_button`] for the kinds).
fn paint_margin_glyph(ui: &egui::Ui, rect: Rect, kind: u8, hovered: bool) {
    let p = ui.painter();
    if hovered {
        p.rect_filled(rect, CornerRadius::same(5), theme::accent_faded());
    }
    let col = if hovered { theme::accent_col() } else { PAPER_SOFT };
    let s = Stroke::new(1.6, col);
    let c = rect.center();
    match kind {
        0 | 6 => {
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
        4 => {
            for dx in [-4.5, 0.0, 4.5] {
                p.circle_filled(c + Vec2::new(dx, 0.0), 1.5, col);
            }
        }
        5 => {
            // Merge up: arrow into a bar.
            p.line_segment([c + Vec2::new(-4.5, -4.0), c + Vec2::new(4.5, -4.0)], s);
            p.line_segment([c + Vec2::new(0.0, 4.5), c + Vec2::new(0.0, -1.5)], s);
            p.line_segment([c + Vec2::new(-3.0, 1.0), c + Vec2::new(0.0, -1.5)], s);
            p.line_segment([c + Vec2::new(3.0, 1.0), c + Vec2::new(0.0, -1.5)], s);
        }
        _ => {
            p.line_segment([c + Vec2::new(-4.0, -1.5), c + Vec2::new(0.0, 2.5)], s);
            p.line_segment([c + Vec2::new(0.0, 2.5), c + Vec2::new(4.0, -1.5)], s);
        }
    }
}

/// One question on the page, with its work space. Returns the item's screen rect (for drop
/// targeting).
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
    ml: f32,
    mr: f32,
    style: model::PaperStyle,
) -> Rect {
    let wash = ui.painter().add(Shape::Noop);
    let qrect = paper_question_ws(app, ui, ctx, q, number, cw, bp, style, Some((exam_id, i, &mut *st)));
    let work = app.store.exam(exam_id).and_then(|e| e.items.get(i)).map(|it| it.work_lines).unwrap_or(0);
    let wrect = work_space_area(app, ui, exam_id, st, i, work, qrect, cw, bp, style);
    let rect = qrect.union(wrect);

    // Highlighter wash for a just-added question.
    if let Some((idx, t0)) = st.just_added {
        let age = (ctx.input(|i| i.time) - t0) as f32;
        if idx == i && age < 4.0 {
            let a = (1.0 - age / 4.0).clamp(0.0, 1.0) * 0.55;
            let hl = theme::highlighter().gamma_multiply(a);
            let r = qrect.expand2(Vec2::new(4.0, 3.0));
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

    // Margin gutters: grip and the actions menu on the left, remove / move on the right.
    let dragging_this = matches!(&st.drag, Some(DragState { src: DragSrc::Page(f), .. }) if *f == i);
    let any_drag = st.drag.is_some() || st.ws_drag.is_some() || st.pws_drag.is_some();
    let menu_id = ui.id().with(("it_menu", i));
    let menu_open = egui::Popup::is_id_open(ctx, menu_id.with("popup"));
    let hover = !any_drag && ui.rect_contains_pointer(rect.expand2(Vec2::new(ml.max(mr) - 2.0, 4.0)));
    let show = hover || dragging_this || menu_open;
    if let Some(q) = q {
        let gc = Pos2::new(rect.left() - (ml * 0.5).min(24.0) - 2.0, rect.top() + bp * 0.75);
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
        let rx = rect.right() + (mr * 0.5).min(24.0) + 2.0;
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
        // The per-question actions menu sits in the left gutter under the grip, inside even a
        // one-line question's height.
        let mrect = Rect::from_center_size(gc + Vec2::new(0.0, 24.0), Vec2::splat(18.0));
        let mresp = ui.interact(mrect, menu_id, Sense::click());
        if show {
            paint_margin_glyph(ui, mrect, 4, mresp.hovered() || menu_open);
        }
        let mresp = mresp.on_hover_text("Question actions");
        let n_items = app.store.exam(exam_id).map(|e| e.items.len()).unwrap_or(0);
        let next_starts = app.store.exam(exam_id).and_then(|e| e.items.get(i + 1)).is_some_and(|it| it.section.is_some());
        let is_mc = matches!(q.kind, Kind::Mc { .. });
        let mut act: Option<QAct> = None;
        egui::Popup::menu(&mresp).id(menu_id.with("popup")).show(|ui| {
            ui.set_min_width(220.0);
            if ui.button("Edit question").clicked() {
                act = Some(QAct::Edit);
            }
            if ui.button("Duplicate").clicked() {
                act = Some(QAct::Duplicate);
            }
            ui.separator();
            let can_section = i + 1 < n_items && !next_starts;
            if ui
                .add_enabled(can_section, egui::Button::new("Insert new section below"))
                .on_disabled_hover_text(if i + 1 >= n_items { "Add a question below this one first" } else { "A section already starts below" })
                .clicked()
            {
                act = Some(QAct::SectionBelow);
            }
            if work == 0 {
                if ui.button("Add work space").clicked() {
                    act = Some(QAct::Work(3));
                }
            } else if ui.button("Remove work space").clicked() {
                act = Some(QAct::Work(0));
            }
            if let Kind::Multipart { parts } = &q.kind {
                if !parts.is_empty() {
                    let pw = app.store.exam(exam_id).and_then(|e| e.items.get(i)).map(|it| it.part_work.clone()).unwrap_or_default();
                    ui.menu_button("Work space per part", |ui| {
                        ui.set_min_width(200.0);
                        for (pi, part) in parts.iter().enumerate() {
                            let label = split_part_label_styled(pi, &part.stem, style).0;
                            let n = pw.get(pi).copied().unwrap_or(0);
                            let text = if n == 0 { format!("{label}  Add work space") } else { format!("{label}  Remove work space ({n} lines)") };
                            if ui.button(text).clicked() {
                                act = Some(QAct::PartWork(pi, if n == 0 { 3 } else { 0 }));
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text("Room to work under each part; drag the handle under a part to size it");
                }
            }
            ui.separator();
            if is_mc {
                let cur = app.store.exam(exam_id).and_then(|e| e.items.get(i)).map(|it| it.answer_cols()).unwrap_or(1);
                ui.label(kit::small_caps_text("Answer format"));
                // Test Parrot's 1 / 2 / 4 column switch, right in the menu.
                ui.horizontal(|ui| {
                    ui.label("Answer columns");
                    ui.add_space(8.0);
                    let mut n = cur;
                    if answer_columns_switch(ui, &mut n).changed() {
                        act = Some(QAct::Columns(n));
                    }
                });
                ui.separator();
            }
            ui.label(kit::small_caps_text("Vary"));
            if ui.button("Shuffle question order").on_hover_text("Shuffles the questions in this section").clicked() {
                act = Some(QAct::ShuffleQ);
            }
            if ui
                .add_enabled(is_mc, egui::Button::new("Shuffle answer order"))
                .on_hover_text("Shuffles this question's answers on this exam (locked answers stay put)")
                .clicked()
            {
                act = Some(QAct::ShuffleA);
            }
            ui.separator();
            if ui.add(egui::Button::new(RichText::new("Remove").color(theme::bad()))).clicked() {
                act = Some(QAct::Remove);
            }
        });
        if let Some(a) = act {
            question_action(app, ctx, exam_id, st, i, q, a);
        }
    }
    if dragging_this {
        ui.painter().rect_filled(rect.expand(3.0), CornerRadius::same(3), theme::paper().gamma_multiply(0.72));
        ctx.request_repaint();
    }
    rect
}

/// The per-question menu's actions.
#[derive(Clone, Copy, PartialEq, Debug)]
enum QAct {
    Edit,
    Duplicate,
    SectionBelow,
    Work(u8),
    /// Work space under one part of a multipart question: (part index, lines).
    PartWork(usize, u8),
    ShuffleQ,
    ShuffleA,
    /// Multiple choice: answer columns (1 / 2 / 4).
    Columns(u8),
    Remove,
}

fn question_action(app: &mut crate::App, ctx: &egui::Context, exam_id: &Id, st: &mut EditUi, i: usize, q: &model::Question, a: QAct) {
    match a {
        QAct::Edit => {
            let Some(it) = app.store.exam(exam_id).and_then(|e| e.items.get(i)).cloned() else { return };
            let Some(bq) = app.store.question(&it.bank_id, &it.question_id).cloned() else { return };
            app.qedit = Some(crate::qedit::Draft::from_question(it.bank_id.clone(), &bq));
            app.bank = Some(it.bank_id.clone());
            app.screen = crate::Screen::BankEdit(it.bank_id.clone());
        }
        QAct::Duplicate => {
            if let Some(at) = duplicate_item(app, exam_id, i) {
                st.just_added = Some((at, ctx.input(|t| t.time)));
            }
        }
        QAct::SectionBelow => apply_items_fn(app, exam_id, |it| with_section_below(it, i)),
        QAct::Work(n) => apply_items_fn(app, exam_id, |mut it| {
            if let Some(x) = it.get_mut(i) {
                x.work_lines = n;
            }
            it
        }),
        QAct::PartWork(pi, n) => apply_items_fn(app, exam_id, |it| with_part_work(it, i, pi, n)),
        QAct::ShuffleQ => {
            let mut rnd = editor_rng();
            apply_items_fn(app, exam_id, |it| with_section_shuffled(it, i, &mut rnd));
        }
        QAct::ShuffleA => {
            let mut rnd = editor_rng();
            let Some(bq) = app.store.exam(exam_id).and_then(|e| e.items.get(i)).and_then(|it| app.store.question(&it.bank_id, &it.question_id)).cloned() else { return };
            let cur = app.store.exam(exam_id).and_then(|e| e.items.get(i)).map(|it| it.answer_order.clone()).unwrap_or_default();
            let _ = q;
            if let Some(order) = shuffled_answer_order(&bq, &cur, &mut rnd) {
                apply_items_fn(app, exam_id, |mut it| {
                    if let Some(x) = it.get_mut(i) {
                        x.answer_order = order;
                    }
                    it
                });
            } else {
                screens::toast(app, "Nothing to shuffle: fewer than two unlocked answers", true);
            }
        }
        QAct::Columns(n) => apply_items_fn(app, exam_id, |mut it| {
            if let Some(x) = it.get_mut(i) {
                x.answer_columns = n.clamp(1, 4);
            }
            it
        }),
        QAct::Remove => remove_item(app, exam_id, i),
    }
}

/// Test Parrot's answer-columns switch for a multiple-choice card: three segments showing
/// 1, 2 and 4 columns of choices. Clicking one sets `cols` and marks the response changed.
/// Shared with the bank's Pop-over (Worker Y), so both look the same.
pub fn answer_columns_switch(ui: &mut egui::Ui, cols: &mut u8) -> egui::Response {
    let seg = Vec2::new(22.0, 18.0);
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(seg.x * 3.0 + 4.0, seg.y + 4.0), Sense::click());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(4), theme::paper(), Stroke::new(1.0, PAPER_RULE), StrokeKind::Inside);
    let names = ["1 column", "2 columns", "4 columns"];
    let mut hovered_k = None;
    for (k, n) in model::ANSWER_COLUMNS.into_iter().enumerate() {
        let r = Rect::from_min_size(rect.min + Vec2::new(2.0 + k as f32 * seg.x, 2.0), seg);
        let on = *cols == n || (k == 0 && !(1..=4).contains(cols));
        let hov = resp.hovered() && ui.ctx().pointer_hover_pos().is_some_and(|q| r.contains(q));
        if hov {
            hovered_k = Some(k);
        }
        if on {
            p.rect_filled(r, CornerRadius::same(3), theme::accent_col());
        } else if hov {
            p.rect_filled(r, CornerRadius::same(3), theme::accent_col().gamma_multiply(0.15));
        }
        let ink = if on { theme::paper() } else { PAPER_SOFT };
        let inner = r.shrink2(Vec2::new(5.0, 5.0));
        let per = n as usize;
        let gap = 2.0;
        let cw = (inner.width() - gap * (per as f32 - 1.0)) / per as f32;
        let rows = if per == 4 { 1 } else { 4 / per };
        for c in 0..per {
            for rr in 0..rows {
                let y = if rows == 1 { inner.center().y } else { inner.top() + rr as f32 * inner.height() / (rows as f32 - 1.0) };
                let x0 = inner.left() + c as f32 * (cw + gap);
                p.line_segment([Pos2::new(x0, y), Pos2::new(x0 + cw, y)], Stroke::new(1.3, ink));
            }
        }
        if resp.clicked() && hov && *cols != n {
            *cols = n;
            resp.mark_changed();
        }
    }
    let tip = hovered_k.map(|k| names[k]).unwrap_or("Answer columns");
    resp.on_hover_text(tip)
}

/// The question's work space under it: `lines` printed lines (blank room, or ruled /
/// dotted lines in the Classic / Exam Board styles) and a drag handle at its bottom edge
/// that sets the line count (0..=23). Returns the area's rect.
#[allow(clippy::too_many_arguments)]
fn work_space_area(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    exam_id: &Id,
    st: &mut EditUi,
    i: usize,
    lines: u8,
    qrect: Rect,
    cw: f32,
    bp: f32,
    style: model::PaperStyle,
) -> Rect {
    let pitch = line_pitch_px(bp, style);
    // While dragging this handle the live count previews (written once on release).
    let live = match st.ws_drag {
        Some((k, n, _, _)) if k == i => n,
        _ => lines,
    };
    let top = ui.cursor().top();
    let x0 = qrect.left();
    let num_w = (bp * 1.9).max(16.0);
    if live > 0 {
        if style.work_space_lined() {
            for _ in 0..live {
                let (r, _) = ui.allocate_exact_size(Vec2::new(cw, pitch), Sense::hover());
                answer_rule(ui.painter(), Pos2::new(x0 + num_w, r.bottom() - 2.0), r.right(), style);
            }
        } else {
            ui.allocate_exact_size(Vec2::new(cw, live as f32 * pitch), Sense::hover());
        }
    }
    let area = Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x0 + cw, ui.cursor().top()));
    // Handle: a thin strip at the bottom edge of question + work space.
    let hy = area.bottom().max(qrect.bottom()) + 1.0;
    let hrect = Rect::from_center_size(Pos2::new(x0 + cw / 2.0, hy), Vec2::new(cw, 8.0));
    let hid = ui.id().with(("ws_handle", i));
    let resp = ui.interact(hrect, hid, Sense::drag());
    let near = ui.rect_contains_pointer(hrect.expand2(Vec2::new(0.0, 3.0)));
    let dragging = st.ws_drag.is_some_and(|(k, ..)| k == i);
    if (near && st.drag.is_none()) || dragging {
        let p = ui.painter();
        if live > 0 || dragging {
            p.rect_stroke(
                Rect::from_min_max(Pos2::new(x0 + num_w, top), Pos2::new(x0 + cw, hy)),
                CornerRadius::ZERO,
                Stroke::new(1.0, Color32::from_rgb(0xb0, 0x3a, 0x2e).gamma_multiply(0.7)),
                StrokeKind::Inside,
            );
        }
        let c = Pos2::new(x0 + cw / 2.0, hy);
        p.rect_filled(Rect::from_center_size(c, Vec2::new(36.0, 5.0)), CornerRadius::same(3), theme::accent_col().gamma_multiply(0.85));
        let txt = if live == 1 { "1 line of work space".to_string() } else { format!("{live} lines of work space") };
        p.text(c + Vec2::new(26.0, 0.0), Align2::LEFT_CENTER, txt, FontId::new(9.5, theme::bold_family()), theme::accent_col());
        resp.clone().on_hover_cursor(egui::CursorIcon::ResizeVertical);
    }
    if resp.drag_started() {
        let y0 = ui.ctx().pointer_interact_pos().map_or(hy, |p| p.y);
        st.ws_drag = Some((i, lines, y0, lines));
    }
    if dragging {
        if let Some(p) = ui.ctx().pointer_latest_pos() {
            // Relative to where the drag began: the question may hop to the next page as it
            // grows, so absolute positions are not stable.
            if let Some((_, _, y0, n0)) = st.ws_drag {
                let n = (n0 as f32 + ((p.y - y0) / pitch).round()).clamp(0.0, 23.0) as u8;
                st.ws_drag = Some((i, n, y0, n0));
            }
        }
        ui.ctx().request_repaint();
    }
    if resp.drag_stopped() || (dragging && !ui.ctx().input(|i| i.pointer.primary_down())) {
        if let Some((k, n, _, _)) = st.ws_drag.take() {
            if k == i && n != lines {
                apply_items_fn(app, exam_id, |mut it| {
                    if let Some(x) = it.get_mut(i) {
                        x.work_lines = n;
                    }
                    it
                });
            }
        }
    }
    area
}

/// Items with part `pi` of item `i` given `n` lines of work space (trailing zeros dropped,
/// so an exam with none stores none).
fn with_part_work(mut items: Vec<ExamItem>, i: usize, pi: usize, n: u8) -> Vec<ExamItem> {
    if let Some(x) = items.get_mut(i) {
        if x.part_work.len() <= pi {
            x.part_work.resize(pi + 1, 0);
        }
        x.part_work[pi] = n.min(23);
        while x.part_work.last() == Some(&0) {
            x.part_work.pop();
        }
    }
    items
}

/// The work space under one part of a multipart question on the exam page: `lines` printed
/// lines and a drag handle at its bottom edge (0..=23), like the question's own handle.
#[allow(clippy::too_many_arguments)]
fn part_space_area(app: &mut crate::App, ui: &mut egui::Ui, exam_id: &Id, st: &mut EditUi, i: usize, pi: usize, label: &str, x0: f32, w: f32, bp: f32, style: model::PaperStyle) {
    let lines = app.store.exam(exam_id).and_then(|e| e.items.get(i)).and_then(|it| it.part_work.get(pi).copied()).unwrap_or(0);
    let pitch = line_pitch_px(bp, style);
    let live = match st.pws_drag {
        Some((k, p, n, _, _)) if k == i && p == pi => n,
        _ => lines,
    };
    let top = ui.cursor().top();
    if live > 0 {
        if style.work_space_lined() {
            for _ in 0..live {
                let (r, _) = ui.allocate_exact_size(Vec2::new(w, pitch), Sense::hover());
                answer_rule(ui.painter(), Pos2::new(x0, r.bottom() - 2.0), x0 + w, style);
            }
        } else {
            ui.allocate_exact_size(Vec2::new(w, live as f32 * pitch), Sense::hover());
        }
    }
    let hy = ui.cursor().top().max(top) + 1.0;
    let hrect = Rect::from_center_size(Pos2::new(x0 + w / 2.0, hy), Vec2::new(w, 6.0));
    let hid = ui.id().with(("pws_handle", i, pi));
    let resp = ui.interact(hrect, hid, Sense::drag());
    let near = ui.rect_contains_pointer(hrect.expand2(Vec2::new(0.0, 2.0)));
    let dragging = st.pws_drag.is_some_and(|(k, p, ..)| k == i && p == pi);
    if (near && st.drag.is_none() && st.ws_drag.is_none()) || dragging {
        let p = ui.painter();
        if live > 0 || dragging {
            p.rect_stroke(
                Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x0 + w, hy)),
                CornerRadius::ZERO,
                Stroke::new(1.0, Color32::from_rgb(0xb0, 0x3a, 0x2e).gamma_multiply(0.7)),
                StrokeKind::Inside,
            );
        }
        let c = Pos2::new(x0 + w / 2.0, hy);
        p.rect_filled(Rect::from_center_size(c, Vec2::new(28.0, 4.0)), CornerRadius::same(2), theme::accent_col().gamma_multiply(0.85));
        let txt = if live == 1 { format!("{label} 1 line of work space") } else { format!("{label} {live} lines of work space") };
        p.text(c + Vec2::new(20.0, 0.0), Align2::LEFT_CENTER, txt, FontId::new(9.0, theme::bold_family()), theme::accent_col());
        resp.clone().on_hover_cursor(egui::CursorIcon::ResizeVertical);
    }
    if resp.drag_started() {
        let y0 = ui.ctx().pointer_interact_pos().map_or(hy, |p| p.y);
        st.pws_drag = Some((i, pi, lines, y0, lines));
    }
    if dragging {
        if let (Some(p), Some((_, _, _, y0, n0))) = (ui.ctx().pointer_latest_pos(), st.pws_drag) {
            let n = (n0 as f32 + ((p.y - y0) / pitch).round()).clamp(0.0, 23.0) as u8;
            st.pws_drag = Some((i, pi, n, y0, n0));
        }
        ui.ctx().request_repaint();
    }
    if resp.drag_stopped() || (dragging && !ui.ctx().input(|i| i.pointer.primary_down())) {
        if let Some((k, p, n, _, _)) = st.pws_drag.take() {
            if k == i && p == pi && n != lines {
                apply_items_fn(app, exam_id, |it| with_part_work(it, i, pi, n));
            }
        }
    }
}

/// One printed answer line from `a` to x `x1` (dotted on Exam Board).
fn answer_rule(p: &egui::Painter, a: Pos2, x1: f32, style: model::PaperStyle) {
    let s = egui::Stroke::new(0.9, PAPER_RULE);
    if style.dotted() {
        p.extend(Shape::dotted_line(&[a, Pos2::new(x1, a.y)], PAPER_RULE, 3.0, 0.7));
    } else {
        p.line_segment([a, Pos2::new(x1, a.y)], s);
    }
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
    style: model::PaperStyle,
) -> Rect {
    paper_question_ws(app, ui, ctx, q, number, cw, bp, style, None)
}

/// [`paper_question`] on the exam page: `pws` (exam, item index, editor state) adds each
/// multipart part's work space with its drag handle.
#[allow(clippy::too_many_arguments)]
fn paper_question_ws(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    q: Option<&model::Question>,
    number: u32,
    cw: f32,
    bp: f32,
    style: model::PaperStyle,
    mut pws: Option<(&Id, usize, &mut EditUi)>,
) -> Rect {
    let objective = q.is_some_and(|q| matches!(q.kind, Kind::Mc { .. } | Kind::Tf { .. }));
    let blank_w = if style.objective_blank() && objective { bp * 3.6 } else { 0.0 };
    let num_w = (bp * 1.9).max(16.0) + blank_w;
    let body_w = cw - num_w;
    let board_points = style == model::PaperStyle::ExamBoard;
    let inner = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let (nr, _) = ui.allocate_exact_size(Vec2::new(num_w, bp * 1.45), Sense::hover());
        if blank_w > 0.0 {
            let y = nr.top() + bp * 1.15;
            ui.painter().line_segment([Pos2::new(nr.left(), y), Pos2::new(nr.left() + blank_w - bp * 0.6, y)], Stroke::new(0.9, PAPER_INK));
        }
        ui.painter().text(
            nr.left_top() + Vec2::new(blank_w, 0.0),
            Align2::LEFT_TOP,
            style.number_label(number),
            FontId::new(bp, theme::serif_bold_family()),
            PAPER_INK,
        );
        ui.vertical(|ui| {
            ui.set_width(body_w);
            let Some(q) = q else {
                ui.label(RichText::new("(question missing from its bank)").size(bp).color(PAPER_SOFT));
                return;
            };
            let pts = if board_points { None } else { q.points };
            // An attached picture sits at the stem's first `[picture]` marker (as print does);
            // without a marker it goes under the stem.
            let at_marker = q.image.as_ref().and_then(|_| crate::layout::split_at_picture(&q.stem));
            if let Some((before, after)) = at_marker {
                render_stem(ui, &before, pts, bp, false);
                draw_image(app, ui, ctx, q.image.as_ref());
                if !after.trim().is_empty() {
                    render_stem(ui, &after, None, bp, false);
                }
                let mut rest = q.clone();
                rest.image = None;
                draw_item_body(app, ui, ctx, &rest, number, body_w, bp, style, pws.as_mut().map(|(e, i, st)| (*e, *i, &mut **st)));
            } else {
                render_stem(ui, &q.stem, pts, bp, q.image.is_some());
                draw_item_body(app, ui, ctx, q, number, body_w, bp, style, pws.as_mut().map(|(e, i, st)| (*e, *i, &mut **st)));
            }
        });
    });
    let r = Rect::from_min_size(inner.response.rect.min, Vec2::new(cw, inner.response.rect.height().max(bp * 1.45)));
    if board_points {
        if let Some(p) = q.and_then(|q| q.points) {
            ui.painter().text(
                Pos2::new(r.right(), r.bottom() - 2.0),
                Align2::RIGHT_BOTTOM,
                style.points_label(p),
                FontId::new(bp, theme::serif_family()),
                PAPER_INK,
            );
        }
    }
    r
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

/// Choice label as printed in `style` ("A. Ge", "a. Ge", "A Ge").
fn styled_choice(style: model::PaperStyle, ci: usize, text: &str) -> String {
    style.choice_label(&format!("{}. {}", letter(ci), text))
}

#[allow(clippy::too_many_arguments)]
fn draw_item_body(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    q: &model::Question,
    number: u32,
    body_w: f32,
    bp: f32,
    style: model::PaperStyle,
    mut pws: Option<(&Id, usize, &mut EditUi)>,
) {
    if model::is_keyless(q) {
        // Imported from the pool without its key: say so where the teacher looks.
        ui.label(egui::RichText::new("No answer key").size(bp * 0.8).color(egui::Color32::from_rgb(170, 60, 40)));
    }
    match &q.kind {
        Kind::Mc { answers } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            ui.add_space(bp * 0.25);
            let labels: Vec<String> = answers.iter().enumerate().map(|(ci, a)| styled_choice(style, ci, &a.text)).collect();
            // The exam item's answer columns (2 / 4): equal columns, filled column-major as print does.
            let fixed = pws
                .as_ref()
                .and_then(|(e, i, _)| app.store.exam(e).and_then(|x| x.items.get(*i)).map(|it| it.answer_cols()))
                .filter(|&c| c > 1 && labels.len() > 1);
            if let Some(n) = fixed {
                for row in crate::layout::choice_grid(labels.len(), n) {
                    ui.columns(n as usize, |cs| {
                        for (k, &ci) in row.iter().enumerate() {
                            popt(&mut cs[k], labels[ci].clone(), bp);
                        }
                    });
                }
                return;
            }
            // Condensed prints the choices across when every one fits its column.
            let across = style.choices_across() && labels.len() > 1 && !labels.iter().any(|l| needs_rich(l) || l.contains("[picture"));
            let cols = if across {
                let widest = labels.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32 * bp * 0.5 + 12.0;
                [labels.len(), 4, 3, 2].into_iter().filter(|&c| c >= 2 && c <= labels.len()).find(|&c| widest <= body_w / c as f32)
            } else {
                None
            };
            match cols {
                Some(n) => {
                    for row in labels.chunks(n) {
                        ui.columns(n, |cs| {
                            for (k, l) in row.iter().enumerate() {
                                popt(&mut cs[k], l.clone(), bp);
                            }
                        });
                    }
                }
                None => {
                    for l in labels {
                        popt(ui, l, bp);
                    }
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
            let rights = model::right_column(pairs, distractors);
            ui.columns(2, |cols| {
                for (li, p) in pairs.iter().enumerate() {
                    popt(&mut cols[0], format!("{} {}", style.number_label(number + li as u32), p.left), bp);
                }
                for (ri, r) in rights.iter().enumerate() {
                    popt(&mut cols[1], styled_choice(style, ri, r), bp);
                }
            });
        }
        Kind::Blank { .. } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            for _ in 0..ruled_blanks(&q.stem) {
                ruled_line(ui, body_w, bp, style);
            }
        }
        Kind::Short { .. } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            for _ in 0..3 {
                ruled_line(ui, body_w, bp, style);
            }
        }
        Kind::Multipart { parts } => {
            draw_image(app, ui, ctx, q.image.as_ref());
            let mut rules = 0u32;
            for (pi, part) in parts.iter().enumerate() {
                ui.add_space(bp * 0.35);
                let (label, rest) = split_part_label_styled(pi, &part.stem, style);
                let row = ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let lw = bp * 1.9;
                    let (lr, _) = ui.allocate_exact_size(Vec2::new(lw, bp * 1.2), Sense::hover());
                    ui.painter().text(lr.left_top(), Align2::LEFT_TOP, &label, FontId::new(bp, theme::serif_family()), PAPER_INK);
                    ui.vertical(|ui| {
                        ui.set_width((body_w - lw).max(40.0));
                        match &part.kind {
                            PartKind::Mc { answers } => {
                                popt(ui, rest.clone(), bp);
                                for (ci, a) in answers.iter().enumerate() {
                                    popt(ui, styled_choice(style, ci, &a.text), bp);
                                }
                            }
                            PartKind::Tf { .. } => popt(ui, format!("{rest} (True / False)"), bp),
                            PartKind::Short { .. } => popt(ui, rest.clone(), bp),
                        }
                    });
                });
                if let Some((exam_id, i, st)) = pws.as_mut() {
                    let x0 = row.response.rect.left() + bp * 1.9;
                    part_space_area(app, ui, exam_id, st, *i, pi, &label, x0, (body_w - bp * 1.9).max(40.0), bp, style);
                }
                if matches!(part.kind, PartKind::Short { .. }) {
                    rules += 2;
                }
            }
            for _ in 0..rules.max(2) {
                ruled_line(ui, body_w, bp, style);
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
        // The printed size (`layout::picture_size_pt` in this column), in page pixels.
        let [tw, th] = tex.size();
        let (w_px, h_px) = if img.width > 0 && img.height > 0 { (img.width, img.height) } else { (tw as u32, th as u32) };
        let (w, h) = crate::layout::picture_size_pt(w_px, h_px, (ui.available_width() / PT) as f64);
        ui.add_space(6.0 * PT);
        ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(w as f32 * PT, h as f32 * PT)));
        ui.add_space(6.0 * PT);
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
    apply_items_fn(app, exam_id, |items| without_item(items, i));
}

// ----- sections (positional: an item with `section` starts one) -------------

/// Contiguous item ranges of the exam's sections, in order: one starts at item 0 and at
/// every item that carries a section heading. Empty exam: no ranges.
pub(crate) fn section_ranges(items: &[ExamItem]) -> Vec<std::ops::Range<usize>> {
    let starts: Vec<usize> = (0..items.len()).filter(|&i| i == 0 || items[i].section.is_some()).collect();
    starts
        .iter()
        .enumerate()
        .map(|(k, &s)| s..starts.get(k + 1).copied().unwrap_or(items.len()))
        .collect()
}

/// `items` without item `i`; a section heading on it moves to the next item (unless that
/// one already starts a section), so removing a question never deletes its section.
fn without_item(mut items: Vec<ExamItem>, i: usize) -> Vec<ExamItem> {
    if i >= items.len() {
        return items;
    }
    let gone = items.remove(i);
    if let (Some(h), Some(next)) = (gone.section, items.get_mut(i)) {
        if next.section.is_none() {
            next.section = Some(h);
        }
    }
    items
}

/// Applies a reorder to the items' contents while section headings stay at their positions
/// (a question moved across a boundary changes section, as on Test Parrot).
fn with_headings_fixed(items: Vec<ExamItem>, f: impl FnOnce(Vec<ExamItem>) -> Vec<ExamItem>) -> Vec<ExamItem> {
    let heads: Vec<Option<model::ExamSection>> = items.iter().map(|i| i.section.clone()).collect();
    let bare: Vec<ExamItem> = items.into_iter().map(|mut i| { i.section = None; i }).collect();
    let mut out = f(bare);
    for (i, h) in heads.into_iter().enumerate() {
        if let Some(it) = out.get_mut(i) {
            it.section = h;
        }
    }
    out
}

/// "Insert new section below" question `i`: the next question starts a new (blank) section.
/// No-op on the last question or when the next one already starts a section.
fn with_section_below(mut items: Vec<ExamItem>, i: usize) -> Vec<ExamItem> {
    if let Some(next) = items.get_mut(i + 1) {
        if next.section.is_none() {
            next.section = Some(model::ExamSection::default());
        }
    }
    items
}

/// Shuffles the questions of the section holding item `i` (headings stay put).
fn with_section_shuffled(items: Vec<ExamItem>, i: usize, rnd: &mut impl FnMut() -> u64) -> Vec<ExamItem> {
    let Some(r) = section_ranges(&items).into_iter().find(|r| r.contains(&i)) else { return items };
    let order = shuffled_indices(r.len(), rnd);
    with_headings_fixed(items, |bare| {
        let mut out = bare.clone();
        for (k, o) in order.iter().enumerate() {
            out[r.start + k] = bare[r.start + o].clone();
        }
        out
    })
}

/// Shuffles every section's questions (Edit > Shuffle question order).
fn with_all_sections_shuffled(items: Vec<ExamItem>, rnd: &mut impl FnMut() -> u64) -> Vec<ExamItem> {
    let starts: Vec<usize> = section_ranges(&items).iter().map(|r| r.start).collect();
    starts.into_iter().fold(items, |acc, s| with_section_shuffled(acc, s, rnd))
}

/// Swaps section `s` (heading + its questions) with its neighbour `s + dir` (-1 / +1).
/// A block that ends up after another but has no heading gets a blank one, so sections
/// never silently merge.
fn with_section_moved(items: Vec<ExamItem>, s: usize, dir: i32) -> Vec<ExamItem> {
    let ranges = section_ranges(&items);
    let t = s as i64 + dir as i64;
    if s >= ranges.len() || t < 0 || t as usize >= ranges.len() {
        return items;
    }
    let (a, b) = if dir < 0 { (t as usize, s) } else { (s, t as usize) };
    let mut blocks: Vec<Vec<ExamItem>> = ranges.iter().map(|r| items[r.clone()].to_vec()).collect();
    blocks.swap(a, b);
    let mut out: Vec<ExamItem> = Vec::with_capacity(items.len());
    for (k, mut blk) in blocks.into_iter().enumerate() {
        if k > 0 {
            if let Some(first) = blk.first_mut() {
                if first.section.is_none() {
                    first.section = Some(model::ExamSection::default());
                }
            }
        }
        out.extend(blk);
    }
    out
}

/// Removes section `s`'s heading: its questions join the section above.
fn with_section_merged_up(mut items: Vec<ExamItem>, s: usize) -> Vec<ExamItem> {
    if let Some(r) = section_ranges(&items).get(s) {
        items[r.start].section = None;
    }
    items
}

/// Deletes section `s` with its questions.
fn with_section_deleted(mut items: Vec<ExamItem>, s: usize) -> Vec<ExamItem> {
    if let Some(r) = section_ranges(&items).get(s).cloned() {
        items.drain(r); // the next section keeps its own heading
    }
    items
}

/// A new answer order for item `i`'s multiple-choice question: the current order with its
/// unlocked answers shuffled (locked ones keep their slot). None when nothing can move.
fn shuffled_answer_order(q: &model::Question, current: &[usize], rnd: &mut impl FnMut() -> u64) -> Option<Vec<usize>> {
    let Kind::Mc { answers } = &q.kind else { return None };
    let n = answers.len();
    let cur: Vec<usize> = if is_permutation(current, n) { current.to_vec() } else { (0..n).collect() };
    let movable: Vec<usize> = (0..n).filter(|&p| !answers[cur[p]].locked).collect();
    if movable.len() < 2 {
        return None;
    }
    for _ in 0..8 {
        let perm = shuffled_indices(movable.len(), rnd);
        let mut next = cur.clone();
        for (k, &p) in movable.iter().enumerate() {
            next[p] = cur[movable[perm[k]]];
        }
        if next != cur {
            return Some(next);
        }
    }
    None
}

/// A time-seeded LCG for the editor's one-off shuffles.
fn editor_rng() -> impl FnMut() -> u64 {
    let t = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(7);
    lcg(t)
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
    items.insert(at, ExamItem::new(bank_id.clone(), question_id.clone()));
    items
}

fn shift_item(app: &mut crate::App, exam_id: &Id, i: usize, dir: i32) {
    apply_items_fn(app, exam_id, |mut items| {
        if items.is_empty() {
            return items;
        }
        let j = (i as i64 + dir as i64).clamp(0, items.len() as i64 - 1) as usize;
        if j != i {
            items = with_headings_fixed(items, |mut b| {
                b.swap(i, j);
                b
            });
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
        items.insert(k, ExamItem::new(bank_id.clone(), question_id.clone()));
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
        if from >= items.len() {
            return items;
        }
        with_headings_fixed(items, |mut next| {
            let moved = next.remove(from);
            let k = k.min(next.len());
            next.insert(k, moved);
            next
        })
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

/// Shelf sort orders (index into [`SORTS`]).
const SORTS: [&str; 5] = ["Newest", "Oldest", "Bank order", "Question type", "A to Z"];

/// A shelf filter button that opens a popup (highlighted while a filter is active).
fn filter_button(ui: &mut egui::Ui, label: &str, active: usize) -> egui::Response {
    let text = if active > 0 { format!("{label} · {active}") } else { label.to_string() };
    ui.add(egui::Button::new(RichText::new(text).size(12.0)).selected(active > 0))
}

fn draw_shelf_inner(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
) {
    // ---- bank tabs (+ opens another bank) ----
    if let Some(b) = app.panel_bank.clone() {
        if !st.open_banks.contains(&b) {
            st.open_banks.insert(0, b);
        }
    }
    st.open_banks.retain(|b| app.store.bank(b).is_some());
    let mut close: Option<Id> = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for b in st.open_banks.clone() {
            let name = app.store.bank(&b).map(|x| x.name.clone()).unwrap_or_default();
            let on = app.panel_bank.as_ref() == Some(&b);
            egui::Frame::new()
                .fill(if on { theme::card() } else { theme::field() })
                .stroke(Stroke::new(1.0, if on { theme::accent_col() } else { theme::border() }))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let r = ui
                        .add(egui::Label::new(RichText::new(truncate_str(&name, 22)).size(12.5).color(if on { theme::ink() } else { theme::muted_col() })).sense(Sense::click()))
                        .on_hover_text(&name);
                    if r.clicked() {
                        app.panel_bank = Some(b.clone());
                    }
                    if ui.add(egui::Button::new(RichText::new(egui_phosphor::regular::X).size(12.0)).frame(false)).on_hover_text("Close this bank").clicked() {
                        close = Some(b.clone());
                    }
                });
        }
        let plus = ui.add(egui::Button::new(RichText::new("+").size(14.0)).frame(false)).on_hover_text("Open another question bank");
        egui::Popup::menu(&plus).show(|ui| {
            ui.set_min_width(220.0);
            ui.label(kit::small_caps_text("Open question bank"));
            let others: Vec<(Id, String, usize)> = app
                .store
                .banks
                .iter()
                .filter(|b| !st.open_banks.contains(&b.id))
                .map(|b| (b.id.clone(), b.name.clone(), b.questions.len()))
                .collect();
            if others.is_empty() {
                ui.label(muted("Every bank is open."));
            }
            for (id, name, n) in others {
                if ui.button(format!("{name}   ({n})")).clicked() {
                    st.open_banks.push(id.clone());
                    app.panel_bank = Some(id);
                }
            }
        });
    });
    if let Some(c) = close {
        st.open_banks.retain(|b| *b != c);
        if app.panel_bank.as_ref() == Some(&c) {
            app.panel_bank = st.open_banks.first().cloned();
        }
    }

    let Some(bank_id) = app.panel_bank.clone() else {
        ui.add_space(12.0);
        ui.label(theme::serif("Question bank").size(17.0));
        ui.add(egui::Label::new(muted("Open a bank with + above (create one on the Banks screen first).")));
        return;
    };
    let Some(bank) = app.store.bank(&bank_id).cloned() else {
        app.panel_bank = None;
        return;
    };
    let in_exam: Vec<ExamItem> = app.store.exam(exam_id).map(|e| e.items.clone()).unwrap_or_default();

    ui.add_space(4.0);
    ui.label(theme::serif("Question bank").size(17.0));
    ui.add_space(4.0);
    ui.add(
        egui::TextEdit::singleline(&mut st.search)
            .hint_text("Search questions")
            .margin(Margin::symmetric(10, 5))
            .desired_width(ui.available_width()),
    );
    ui.add_space(4.0);

    // ---- filters: Question Type / Difficulty / Topic ----
    let mut topics: Vec<String> = bank
        .questions
        .iter()
        .flat_map(|q| q.topics.clone())
        .filter(|t| !t.trim().is_empty())
        .collect();
    topics.sort();
    topics.dedup();
    st.topics.retain(|t| topics.contains(t));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
        let close_outside = egui::PopupCloseBehavior::CloseOnClickOutside;
        let r = filter_button(ui, "Question Type", st.kinds.len());
        egui::Popup::menu(&r).close_behavior(close_outside).show(|ui| {
            for k in [QKind::Mc, QKind::Tf, QKind::Matching, QKind::Blank, QKind::Short, QKind::Multipart] {
                let n = bank.questions.iter().filter(|q| q.kind.qkind() == k).count();
                let mut on = st.kinds.contains(&k);
                if ui.checkbox(&mut on, format!("{}  ({n})", k.label())).changed() {
                    if on {
                        st.kinds.push(k);
                    } else {
                        st.kinds.retain(|x| *x != k);
                    }
                }
            }
            if !st.kinds.is_empty() && ui.button("Clear").clicked() {
                st.kinds.clear();
            }
        });
        let r = filter_button(ui, "Difficulty", usize::from(st.diff != 0));
        egui::Popup::menu(&r).close_behavior(close_outside).show(|ui| {
            for (di, label) in [(0u8, "Any difficulty"), (1, "Easy"), (2, "Medium"), (3, "Hard")] {
                ui.radio_value(&mut st.diff, di, label);
            }
        });
        let r = filter_button(ui, "Topic", st.topics.len());
        egui::Popup::menu(&r).close_behavior(close_outside).show(|ui| {
            if topics.is_empty() {
                ui.label(muted("No topics in this bank yet."));
            }
            for t in &topics {
                let mut on = st.topics.contains(t);
                if ui.checkbox(&mut on, t).changed() {
                    if on {
                        st.topics.push(t.clone());
                    } else {
                        st.topics.retain(|x| x != t);
                    }
                }
            }
            if !st.topics.is_empty() && ui.button("Clear").clicked() {
                st.topics.clear();
            }
        });
    });
    ui.add_space(4.0);

    let needle = st.search.trim().to_lowercase();
    let mut qs: Vec<(usize, model::Question)> = bank
        .questions
        .iter()
        .enumerate()
        .filter(|(_, q)| needle.is_empty() || q.stem.to_lowercase().contains(&needle) || crate::equation::stem_preview(&q.stem).to_lowercase().contains(&needle))
        .filter(|(_, q)| st.kinds.is_empty() || st.kinds.contains(&q.kind.qkind()))
        .filter(|(_, q)| st.topics.is_empty() || st.topics.iter().any(|t| q.topics.contains(t)))
        .filter(|(_, q)| match (st.diff, &q.difficulty) {
            (0, _) => true,
            (1, Some(model::Difficulty::Low))
            | (2, Some(model::Difficulty::Medium))
            | (3, Some(model::Difficulty::High)) => true,
            _ => false,
        })
        .map(|(i, q)| (i, q.clone()))
        .collect();
    match st.sort {
        0 => qs.sort_by_key(|(i, q)| (-q.created, -(*i as i64))),
        1 => qs.sort_by_key(|(i, q)| (q.created, *i)),
        3 => qs.sort_by_key(|(i, q)| (q.kind.qkind() as u8, *i)),
        4 => qs.sort_by_key(|(_, q)| crate::equation::stem_preview(&q.stem).to_lowercase()),
        _ => qs.sort_by_key(|(i, _)| *i),
    }
    let not_in: Vec<Id> = qs
        .iter()
        .filter(|(_, q)| !in_exam.iter().any(|it| it.bank_id == bank_id && it.question_id == q.id))
        .map(|(_, q)| q.id.clone())
        .collect();

    // ---- sort + Add all ----
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        egui::ComboBox::from_id_salt("shelf_sort")
            .selected_text(SORTS[st.sort.min(SORTS.len() as u8 - 1) as usize])
            .width(130.0)
            .show_ui(ui, |ui| {
                for (k, l) in SORTS.iter().enumerate() {
                    ui.selectable_value(&mut st.sort, k as u8, *l);
                }
            });
        let add_all = ui
            .add_enabled(!not_in.is_empty(), egui::Button::new(RichText::new(format!("+ Add all ({})", not_in.len())).family(theme::bold_family()).size(12.5)))
            .on_hover_text("Adds every question shown here that is not on the exam yet");
        if add_all.clicked() {
            let len = app.store.exam(exam_id).map(|e| e.items.len()).unwrap_or(0);
            let b = bank_id.clone();
            let add = not_in.clone();
            apply_items_fn(app, exam_id, |mut items| {
                items.extend(add.iter().map(|q| ExamItem::new(b.clone(), q.clone())));
                items
            });
            st.just_added = Some((len, ctx.input(|i| i.time)));
            screens::toast(app, &format!("Added {} question{}", not_in.len(), if not_in.len() == 1 { "" } else { "s" }), false);
        }
    });
    ui.add_space(6.0);

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
            for (_, q) in &qs {
                let count = in_exam
                    .iter()
                    .filter(|it| it.bank_id == bank_id && it.question_id == q.id)
                    .count();
                shelf_row(app, ui, st, ctx, &bank_id, exam_id, q, count);
                ui.add_space(6.0);
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

/// One shelf card (Test Parrot's layout): TYPE label with "In exam" / "Image" badges, the
/// stem on two lines, and edit / copy / add (or in-exam check) buttons. The card drags
/// onto the page.
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
    let btn_w = 3.0 * 26.0 + 2.0 * 4.0;
    let stem_w = w - 12.0 - btn_w - 18.0;
    let already = in_count > 0;
    let p = ui.painter().clone();
    let stem_txt = crate::equation::stem_preview(&q.stem);
    let galley = clamped_galley(&p, &stem_txt, FontId::proportional(12.5), theme::ink(), stem_w, 2);
    let h = galley.size().y + 34.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());
    let drag_this = matches!(&st.drag, Some(DragState { src: DragSrc::Shelf { question_id, .. }, .. }) if *question_id == q.id);

    let fill = if already { theme::hover_bg() } else { theme::card() };
    p.rect(
        rect,
        CornerRadius::same(9),
        if resp.hovered() || drag_this { theme::accent_faded() } else { fill },
        Stroke::new(1.0, if resp.hovered() { theme::accent_col() } else { theme::border() }),
        StrokeKind::Inside,
    );
    // TYPE label (+ part count) and badges.
    let mut x = rect.left() + 12.0;
    let ty = match &q.kind {
        Kind::Multipart { parts } => format!("{} · {} PARTS", q.kind.qkind().label().to_uppercase(), parts.len()),
        _ => q.kind.qkind().label().to_uppercase(),
    };
    let tg = p.layout_no_wrap(ty, FontId::new(9.5, theme::bold_family()), theme::muted_col());
    let ty_y = rect.top() + 8.0;
    let tw = tg.size().x;
    p.galley(Pos2::new(x, ty_y), tg, theme::muted_col());
    x += tw + 8.0;
    let mut badge = |label: &str, strong: bool| {
        let g = p.layout_no_wrap(label.to_string(), FontId::new(9.5, theme::bold_family()), if strong { theme::ink() } else { theme::muted_col() });
        let r = Rect::from_min_size(Pos2::new(x, ty_y - 1.5), g.size() + Vec2::new(10.0, 3.0));
        p.rect_filled(r, CornerRadius::same(6), if strong { theme::highlighter().gamma_multiply(0.55) } else { theme::field() });
        x = r.right() + 6.0;
        p.galley(r.min + Vec2::new(5.0, 1.5), g, Color32::PLACEHOLDER);
    };
    if already {
        badge(if in_count > 1 { "In exam ×2+" } else { "In exam" }, true);
    }
    if q.image.is_some() {
        badge("Image", false);
    }
    if let Some(pts) = q.points {
        let g = p.layout_no_wrap(format!("{} pt", fmt_number(pts)), FontId::proportional(10.5), theme::muted_col());
        p.galley(Pos2::new(rect.right() - btn_w - 14.0 - g.size().x, ty_y), g, Color32::PLACEHOLDER);
    }
    // Stem.
    p.galley(Pos2::new(rect.left() + 12.0, rect.top() + 24.0), galley, theme::ink());

    // Buttons: edit, copy, add / in-exam check.
    let cy = rect.center().y;
    let mut bx = rect.right() - 10.0 - 13.0 - 2.0 * 30.0;
    let mut small = |id: &str, tip: &str| -> (Rect, egui::Response) {
        let br = Rect::from_center_size(Pos2::new(bx, cy), Vec2::splat(26.0));
        bx += 30.0;
        let r = ui.interact(br, ui.id().with((id, &q.id)), Sense::click()).on_hover_text(tip);
        if r.hovered() {
            ui.painter().rect_filled(br, CornerRadius::same(7), theme::card());
        }
        (br, r)
    };
    let (er, eresp) = small("shelf_edit", "Edit question");
    let (cr, cresp) = small("shelf_copy", "Copy: duplicate this question in the bank");
    let (br, bresp) = small("shelf_add", if already { "Already in this exam. Drag the card to add it again." } else { "Add to the exam" });
    let ink = Stroke::new(1.5, theme::muted_col());
    {
        // Pencil.
        let c = er.center();
        p.line_segment([c + Vec2::new(-4.5, 4.5), c + Vec2::new(3.5, -3.5)], ink);
        p.line_segment([c + Vec2::new(-4.5, 4.5), c + Vec2::new(-5.0, 2.0)], ink);
        p.line_segment([c + Vec2::new(2.0, -5.0), c + Vec2::new(5.0, -2.0)], ink);
        // Copy: two sheets.
        let c = cr.center();
        p.rect_stroke(Rect::from_center_size(c + Vec2::new(-1.5, -1.5), Vec2::new(7.5, 8.5)), CornerRadius::same(1), ink, StrokeKind::Middle);
        p.rect_filled(Rect::from_center_size(c + Vec2::new(1.8, 1.8), Vec2::new(7.5, 8.5)), CornerRadius::same(1), fill);
        p.rect_stroke(Rect::from_center_size(c + Vec2::new(1.8, 1.8), Vec2::new(7.5, 8.5)), CornerRadius::same(1), ink, StrokeKind::Middle);
        let c = br.center();
        let s = Stroke::new(2.0, theme::accent_col());
        if already {
            p.line_segment([c + Vec2::new(-4.5, 0.5), c + Vec2::new(-1.5, 3.5)], s);
            p.line_segment([c + Vec2::new(-1.5, 3.5), c + Vec2::new(4.5, -3.5)], s);
        } else {
            p.line_segment([c + Vec2::new(-4.5, 0.0), c + Vec2::new(4.5, 0.0)], s);
            p.line_segment([c + Vec2::new(0.0, -4.5), c + Vec2::new(0.0, 4.5)], s);
        }
    }
    if eresp.clicked() {
        app.qedit = Some(crate::qedit::Draft::from_question(bank_id.clone(), q));
        app.bank = Some(bank_id.clone());
        app.screen = crate::Screen::BankEdit(bank_id.clone());
    }
    if cresp.clicked() {
        if let Some(b) = app.store.bank_mut(bank_id) {
            if crate::bankui::duplicate_in_bank(b, &q.id).is_some() {
                app.store.save_banks();
                screens::toast(app, "Question copied in the bank", false);
            }
        }
    }
    if bresp.clicked() && !already {
        let len = app.store.exam(exam_id).map(|e| e.items.len()).unwrap_or(0);
        let idx = add_item(app, exam_id, bank_id, &q.id, len);
        st.just_added = Some((idx, ctx.input(|i| i.time)));
    }

    let on_button = eresp.hovered() || cresp.hovered() || bresp.hovered();
    if resp.drag_started() && !on_button {
        st.drag = Some(DragState {
            src: DragSrc::Shelf { bank_id: bank_id.clone(), question_id: q.id.clone() },
            stem: q.stem.clone(),
            label: kind_pill(q.kind.qkind()),
        });
    }
    if resp.hovered() && !on_button {
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
    /// Multipart: the stem text cut at each part (piece 0 = stem + part a, piece k = part k),
    /// joined with `\n` it is `dq.text[0]`. Used when the exam sets per-part work space.
    pieces: Vec<String>,
}

/// Reads one attached image's bytes from the store media dir.
fn read_img(img_ref: &model::ImageRef) -> Option<DocImg> {
    let bytes = store::read_media(&img_ref.file)?;
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
#[cfg(test)]
fn map_question(number: u32, q: &model::Question) -> Mapped {
    map_question_style(number, q, model::PaperStyle::Standard)
}

/// [`map_question`] in a paper style (multipart labels follow the style).
fn map_question_style(number: u32, q: &model::Question, style: model::PaperStyle) -> Mapped {
    let mut m = map_question_raw_style(number, q, style);
    // Exports have no typesetter yet: degrade `$latex$` to its Unicode approximation.
    let plain = |s: &mut String| *s = crate::equation_text::stem_to_plain(s);
    m.dq.text.iter_mut().for_each(plain);
    m.pieces.iter_mut().for_each(plain);
    m.dq.choices.iter_mut().for_each(plain);
    plain(&mut m.dq.key_text);
    m
}

#[cfg(test)]
fn map_question_raw(number: u32, q: &model::Question) -> Mapped {
    map_question_raw_style(number, q, model::PaperStyle::Standard)
}

fn map_question_raw_style(number: u32, q: &model::Question, style: model::PaperStyle) -> Mapped {
    let mut dq = DocQ::default();
    dq.number = number;
    dq.points = q.points;
    dq.image = q.image.as_ref().and_then(read_img);
    dq.rule_lines = 0;
    let mut ktext: String;
    let mut pieces: Vec<String> = Vec::new();

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
            if !ks.is_empty() {
                dq.key_marks = vec![ktext.clone()];
            }
            dq.correct = ks;
        }
        Kind::Tf { correct } => {
            dq.text = vec![q.stem.clone()];
            dq.choices = vec!["True".to_string(), "False".to_string()];
            ktext = if *correct { "True" } else { "False" }.to_string();
            dq.correct = vec![if *correct { 0 } else { 1 }];
            dq.key_marks = vec![ktext.clone()];
        }
        Kind::Matching { pairs, distractors } => {
            // Every text entry is a left-column item (no stem line); the
            // exporters print them with continuing numbers from `number`.
            dq.text = pairs.iter().map(|p| p.left.clone()).collect();
            // The same column the sheet prints (no blank or repeated choice).
            let rights = model::right_column(pairs, distractors);
            dq.choices = rights
                .iter()
                .enumerate()
                .map(|(i, r)| format!("{}. {}", letter(i), r))
                .collect();
            dq.key_marks = pairs
                .iter()
                .map(|p| rights.iter().position(|r| *r == p.right).map(letter).unwrap_or_else(|| "?".to_string()))
                .collect();
            ktext = pairs
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let li = rights.iter().position(|r| *r == p.right);
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
            let mut cuts: Vec<usize> = Vec::new();
            for (pi, part) in parts.iter().enumerate() {
                cuts.push(s.len());
                match &part.kind {
                    PartKind::Mc { answers } => {
                        s.push_str(&format!("\n{}", part_line_styled(pi, &part.stem, style)));
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
                        s.push_str(&format!("\n{} (True / False)", part_line_styled(pi, &part.stem, style)));
                        kparts.push(format!(
                            "{}) {}",
                            part_key_label(pi, &part.stem),
                            if *correct { "True" } else { "False" }
                        ));
                    }
                    PartKind::Short { sample } => {
                        s.push_str(&format!("\n{}", part_line_styled(pi, &part.stem, style)));
                        rules += 3;
                        kparts.push(format!(
                            "{}) {}",
                            part_key_label(pi, &part.stem),
                            sample.trim()
                        ));
                    }
                }
            }
            // Piece k runs from part k's line to part k+1's (the first piece keeps the stem).
            cuts.push(s.len());
            pieces = (0..parts.len())
                .map(|k| {
                    let from = if k == 0 { 0 } else { cuts[k] + 1 };
                    s[from.min(cuts[k + 1])..cuts[k + 1]].to_string()
                })
                .collect();
            dq.text = vec![s];
            dq.rule_lines = rules;
            // Multi-line part answers each start on their own line; short ones stay on one line.
            ktext = kparts.join(if kparts.iter().any(|k| k.contains('\n')) { "\n" } else { "  " });
        }
    }

    // Imported without its key: the placeholder flags (all false, a layout pairing) are not a key.
    if model::is_keyless(q) {
        dq.correct.clear();
        dq.key_marks.clear();
        dq.key_text = "Key not provided".to_string();
        return Mapped { dq, img: q.image.clone(), pieces };
    }
    // The teacher's explanation travels inside key_text (prints only on the key).
    if let Some(ex) = &q.explanation {
        let ex = ex.trim();
        if !ex.is_empty() {
            ktext = format!("{ktext}\n\nExplanation: {ex}");
        }
    }
    dq.key_text = ktext;
    Mapped { dq, img: q.image.clone(), pieces }
}

/// Headless export (QA hook VERSHION_EXPORT): writes `exam.pdf`, `exam.docx` and `key.pdf` for the
/// exam at `index` of the store into `dir`, with the real export's mapping (single copy, no
/// shuffling; questions missing from their bank are skipped).
pub fn headless_export(index: usize, dir: &std::path::Path) -> Result<(), String> {
    let st = store::Store::load();
    let exam = st
        .exams
        .get(index)
        .ok_or_else(|| format!("no exam at index {index} ({} exams in the store)", st.exams.len()))?;
    let placed = resolve_items(&st, exam);
    if placed.is_empty() {
        return Err("the exam has no resolvable questions".into());
    }
    let mapped = map_placed(&placed, exam.paper_style);
    let student = build_doc(exam, &mapped, None, false);
    let key = build_key(exam, &mapped, None, env_key_layout());
    let write = |name: &str, bytes: Vec<u8>| std::fs::write(dir.join(name), bytes).map_err(|e| format!("{name}: {e}"));
    write("exam.pdf", crate::export::render_pdf(&student)?)?;
    write("exam.docx", crate::export::render_docx(&student)?)?;
    write("key.pdf", crate::export::render_pdf(&key)?)?;
    write("key.docx", crate::export::render_docx(&key)?)?;
    Ok(())
}

/// The bank question as this exam prints it: the item's own answer order applied.
pub(crate) fn item_question(store: &store::Store, it: &ExamItem) -> Option<model::Question> {
    let mut q = store.question(&it.bank_id, &it.question_id)?.clone();
    apply_answer_order(&mut q, &it.answer_order);
    Some(q)
}

/// Reorders a multiple-choice question's answers by `order` (indices into the bank order);
/// an empty or stale order (not a permutation of the answers) leaves the bank order.
fn apply_answer_order(q: &mut model::Question, order: &[usize]) {
    let Kind::Mc { answers } = &mut q.kind else { return };
    if order.is_empty() || !is_permutation(order, answers.len()) {
        return;
    }
    *answers = order.iter().map(|&i| answers[i].clone()).collect();
}

fn is_permutation(order: &[usize], n: usize) -> bool {
    let mut seen = vec![false; n];
    order.len() == n && order.iter().all(|&i| i < n && !std::mem::replace(&mut seen[i], true))
}

/// One printable placed question: the exam item (section heading, work space) and the
/// resolved question (answer order applied).
#[derive(Clone)]
struct Placed {
    item: ExamItem,
    q: model::Question,
}

/// The exam's printable questions in order; items whose question is missing from its bank
/// are skipped (a skipped item's section heading moves to the next printed question).
/// Numbers (as printed, 1-based) of the exam's questions that still have unresolved
/// `[picture]` placeholders (no picture attached for them).
pub(crate) fn picture_gaps(st: &store::Store, exam: &model::Exam) -> Vec<usize> {
    resolve_items(st, exam)
        .iter()
        .enumerate()
        .filter(|(_, p)| crate::bankops::pictures_needed(&p.q) > 0)
        .map(|(i, _)| i + 1)
        .collect()
}

/// The export gate's message for `nums` (empty when nothing is missing).
pub(crate) fn picture_gate_text(nums: &[usize]) -> String {
    let list = match nums {
        [] => return String::new(),
        [a] => return format!("Question {a} still needs a picture. Attach it in the bank first, or export anyway with the placeholder."),
        [rest @ .., last] => format!("{} and {last}", rest.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")),
    };
    format!("Questions {list} still need pictures. Attach them in the bank first, or export anyway with the placeholders.")
}

fn resolve_items(st: &store::Store, exam: &model::Exam) -> Vec<Placed> {
    let mut out: Vec<Placed> = Vec::new();
    let mut pending: Option<model::ExamSection> = None;
    for it in &exam.items {
        if it.section.is_some() {
            pending = it.section.clone();
        }
        if let Some(q) = item_question(st, it) {
            let mut item = it.clone();
            item.section = pending.take();
            out.push(Placed { item, q });
        }
    }
    out
}

/// Maps placed questions (numbering continues across sections) in a paper style.
fn map_placed(placed: &[Placed], style: model::PaperStyle) -> Vec<Mapped> {
    let qs: Vec<model::Question> = placed.iter().map(|p| p.q.clone()).collect();
    let numbers = doc_numbers(&qs);
    placed
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut m = map_question_style(numbers[i], &p.q, style);
            m.dq.section = p.item.section.as_ref().map(|s| crate::layout::DocSection { title: s.title.clone(), directions: s.directions.clone() });
            m.dq.work_lines = p.item.work_lines;
            if matches!(p.q.kind, Kind::Mc { .. }) && p.item.answer_cols() > 1 {
                m.dq.answer_columns = p.item.answer_cols();
            }
            apply_part_work(&mut m, &p.item.part_work);
            m
        })
        .collect()
}

/// Per-part work space: when any part of a multipart question has some, its text is split
/// into one entry per part and `space_after` carries each part's lines.
fn apply_part_work(m: &mut Mapped, part_work: &[u8]) {
    if m.pieces.is_empty() || !part_work.iter().take(m.pieces.len()).any(|&n| n > 0) {
        return;
    }
    m.dq.text = m.pieces.clone();
    m.dq.space_after = (0..m.pieces.len()).map(|k| part_work.get(k).copied().unwrap_or(0).min(23)).collect();
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
        style: exam.paper_style,
        margins: exam.margins,
        key_layout: KeyLayout::List,
        questions: mapped
            .iter()
            .map(|m| {
                let mut dq = m.dq.clone();
                if is_key {
                    dq.choices.clear();
                    dq.image = None;
                    dq.rule_lines = 0;
                    dq.text = vec![];
                    dq.work_lines = 0;
                    dq.space_after.clear();
                    dq.section = None;
                }
                dq
            })
            .collect(),
    }
}

/// The answer-key document in `layout`: the compact list ([`build_doc`] with `is_key`), Test
/// Parrot's answer section (the list plus section headings and written answers under
/// blanks), or the whole test with the correct answers marked.
fn build_key(exam: &model::Exam, mapped: &[Mapped], label: Option<String>, layout: KeyLayout) -> ExamDoc {
    let mut doc = match layout {
        KeyLayout::Marked => {
            let mut d = build_doc(exam, mapped, label, false);
            d.is_key = true;
            d.title = build_doc(exam, &[], None, true).title;
            d
        }
        _ => build_doc(exam, mapped, label, true),
    };
    if layout == KeyLayout::Section {
        for (dq, m) in doc.questions.iter_mut().zip(mapped) {
            dq.section = m.dq.section.clone();
        }
    }
    doc.key_layout = layout;
    doc
}

/// `VERSHION_KEY_LAYOUT=list|section|marked` (headless export; default: the dialog's default).
fn env_key_layout() -> KeyLayout {
    match crate::env_var("VERSHION_KEY_LAYOUT").unwrap_or_default().trim() {
        "list" => KeyLayout::List,
        "marked" => KeyLayout::Marked,
        _ => KeyLayout::Section,
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

/// Shuffles placed questions within each section; section headings keep their positions.
fn shuffle_placed_in_sections(placed: Vec<Placed>, rnd: &mut impl FnMut() -> u64) -> Vec<Placed> {
    let items: Vec<ExamItem> = placed.iter().map(|p| p.item.clone()).collect();
    let mut out: Vec<Placed> = Vec::with_capacity(placed.len());
    for r in section_ranges(&items) {
        let order = shuffled_indices(r.len(), rnd);
        for (k, o) in order.into_iter().enumerate() {
            let mut p = placed[r.start + o].clone();
            p.item.section = placed[r.start + k].item.section.clone();
            out.push(p);
        }
    }
    out
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
    #[serde(default)]
    pub style: model::PaperStyle,
    #[serde(default)]
    pub margins: Option<model::Margins>,
    /// Answer-key layout; snapshots before R7 have none (= the compact list they printed).
    #[serde(default, skip_serializing_if = "is_list_key")]
    pub key_layout: KeyLayout,
}

fn is_list_key(k: &KeyLayout) -> bool {
    *k == KeyLayout::List
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
    #[serde(default)]
    pub section: Option<model::ExamSection>,
    #[serde(default)]
    pub work_lines: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub space_after: Vec<u8>,
    /// Answer columns (0 = the style's own layout); absent in snapshots before R7.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub answer_columns: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub correct: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_marks: Vec<String>,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
struct SnapImage {
    pub file: String,
    pub width: u32,
    pub height: u32,
}

impl SnapDoc {
    #[cfg(test)]
    fn from_parts(
        exam: &model::Exam,
        mapped: &[Mapped],
        label: Option<String>,
        is_key: bool,
        out_file: String,
    ) -> SnapDoc {
        SnapDoc::from_doc(&build_doc(exam, mapped, label.clone(), is_key), mapped, label, out_file)
    }

    /// The snapshot of an already built document (`mapped` supplies the image files).
    fn from_doc(doc: &ExamDoc, mapped: &[Mapped], label: Option<String>, out_file: String) -> SnapDoc {
        let doc = doc.clone();
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
                    section: doc.questions[i].section.as_ref().map(|s| model::ExamSection { title: s.title.clone(), directions: s.directions.clone() }),
                    work_lines: doc.questions[i].work_lines,
                    space_after: doc.questions[i].space_after.clone(),
                    answer_columns: doc.questions[i].answer_columns,
                    correct: doc.questions[i].correct.clone(),
                    key_marks: doc.questions[i].key_marks.clone(),
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
            style: doc.style,
            margins: doc.margins,
            key_layout: doc.key_layout,
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
            style: self.style,
            margins: self.margins,
            key_layout: self.key_layout,
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
                    section: q.section.as_ref().map(|s| crate::layout::DocSection { title: s.title.clone(), directions: s.directions.clone() }),
                    work_lines: q.work_lines,
                    space_after: q.space_after.clone(),
                    answer_columns: q.answer_columns,
                    correct: q.correct.clone(),
                    key_marks: q.key_marks.clone(),
                    ..Default::default()
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Export preview: real pages, painted from the PDF's own content streams
// ---------------------------------------------------------------------------

/// One token of a (verSHion-written) PDF content stream.
#[derive(Clone, Debug, PartialEq)]
enum PdfTok {
    Num(f64),
    Name(String),
    Str(Vec<u8>),
    /// A `<hex>` string (glyph codes of the CID fonts F5..F8 and the fallback font F9).
    Hex(Vec<u8>),
    Arr(Vec<f64>),
    Op(String),
}

/// Tokenizes the small operator subset `export::render_pdf` writes.
fn pdf_tokens(ops: &[u8]) -> Vec<PdfTok> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        let c = ops[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'(' {
            let mut depth = 1;
            let mut s = Vec::new();
            i += 1;
            while i < ops.len() {
                let b = ops[i];
                if b == b'\\' && i + 1 < ops.len() {
                    s.push(match ops[i + 1] {
                        b'n' => b'\n',
                        b'r' => b'\r',
                        b't' => b'\t',
                        o => o,
                    });
                    i += 2;
                    continue;
                }
                if b == b'(' {
                    depth += 1;
                } else if b == b')' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                s.push(b);
                i += 1;
            }
            out.push(PdfTok::Str(s));
        } else if c == b'<' {
            let end = ops[i..].iter().position(|&b| b == b'>').map_or(ops.len(), |p| i + p);
            let digits: Vec<u8> = ops[i + 1..end].iter().copied().filter(|b| b.is_ascii_hexdigit()).collect();
            let v = |d: u8| (d as char).to_digit(16).unwrap_or(0) as u8;
            out.push(PdfTok::Hex(digits.chunks(2).map(|p| v(p[0]) * 16 + p.get(1).map_or(0, |&d| v(d))).collect()));
            i = end + 1;
        } else if c == b'[' {
            let end = ops[i..].iter().position(|&b| b == b']').map_or(ops.len(), |p| i + p);
            let nums = String::from_utf8_lossy(&ops[i + 1..end]).split_whitespace().filter_map(|t| t.parse().ok()).collect();
            out.push(PdfTok::Arr(nums));
            i = end + 1;
        } else {
            let end = ops[i..].iter().position(|b| b.is_ascii_whitespace() || *b == b'(' || *b == b'[' || *b == b'<').map_or(ops.len(), |p| i + p).max(i + 1);
            let t = String::from_utf8_lossy(&ops[i..end]).to_string();
            out.push(if let Some(n) = t.strip_prefix('/') {
                PdfTok::Name(n.to_string())
            } else if let Ok(v) = t.parse::<f64>() {
                PdfTok::Num(v)
            } else {
                PdfTok::Op(t)
            });
            i = end;
        }
    }
    out
}

/// WinAnsi bytes back to text (the inverse of the exporter's mapping, near enough to read).
fn winansi_text(b: &[u8]) -> String {
    b.iter().map(|&c| crate::fonts::winansi_char(c).unwrap_or(c as char)).collect()
}

/// The text a `Tj` operand shows in font resource `font`: WinAnsi bytes for F1..F4, glyph ids
/// of the same faces for the CID fonts F5..F8, codes of `fallback` for F9.
fn shown_text(tok: &PdfTok, font: &str, fallback: &[char]) -> Option<String> {
    match tok {
        PdfTok::Str(b) => Some(winansi_text(b)),
        PdfTok::Hex(b) if font == "F9" => Some(b.iter().map(|&k| fallback.get(k as usize).copied().unwrap_or('?')).collect()),
        PdfTok::Hex(b) => {
            let style = match font {
                "F6" => crate::fonts::Style::Bold,
                "F7" => crate::fonts::Style::Italic,
                "F8" => crate::fonts::Style::BoldItalic,
                _ => crate::fonts::Style::Regular,
            };
            let cm = crate::fonts::charmap(style);
            Some(b.chunks(2).map(|p| cm.char_of(u16::from_be_bytes([p[0], *p.get(1).unwrap_or(&0)])).unwrap_or('?')).collect())
        }
        _ => None,
    }
}

/// The simple font (F1..F4) a font resource shares its face with (F5..F8 are their CID
/// fonts; the fallback F9 previews in the regular face).
fn base_font(name: &str) -> &str {
    match name {
        "F5" | "F9" => "F1",
        "F6" => "F2",
        "F7" => "F3",
        "F8" => "F4",
        n => n,
    }
}

/// The preview family of a PDF font resource: F1..F4 are the print serif's regular, bold,
/// italic and bold italic faces (real italics when the house fonts are loaded).
fn pdf_font_family(ctx: &egui::Context, name: &str) -> egui::FontFamily {
    let name = base_font(name);
    let bold = name == "F2" || name == "F4";
    let base = if bold { theme::serif_bold_family() } else { theme::serif_family() };
    let base = if ctx.fonts(|f| f.definitions().families.contains_key(&base)) {
        base
    } else if bold {
        theme::bold_family()
    } else {
        egui::FontFamily::Proportional
    };
    if name == "F3" || name == "F4" {
        theme::italic_family(ctx, &base).unwrap_or(base)
    } else {
        base
    }
}

/// Paints one PDF page (its content stream) into `rect`, scaled from `page_w` x `page_h` pt.
/// `images[k]` is the texture for `/Im{k+1}` (None paints a grey box).
fn paint_pdf_page(p: &egui::Painter, rect: Rect, page_w: f64, page_h: f64, ops: &[u8], images: &[Option<egui::TextureId>], fallback: &[char]) {
    let s = rect.width() / page_w as f32;
    let to = |x: f64, y: f64| Pos2::new(rect.left() + x as f32 * s, rect.top() + (page_h - y) as f32 * s);
    let gray = |v: f64| {
        let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
        Color32::from_rgb(g, g, g)
    };
    let rgb = |r: f64, g: f64, b: f64| Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
    p.rect_filled(rect, CornerRadius::same(1), Color32::WHITE);
    let mut stack: Vec<PdfTok> = Vec::new();
    let (mut fill, mut stroke) = (Color32::BLACK, Color32::BLACK);
    let mut lw = 1.0_f64;
    let mut dash = false;
    let mut font = (String::from("F1"), 11.0_f64);
    let mut tpos = (0.0_f64, 0.0_f64);
    let mut path: Vec<Vec<Pos2>> = Vec::new();
    let mut rects: Vec<Rect> = Vec::new();
    let mut cm = [1.0_f64, 0.0, 0.0, 1.0, 0.0, 0.0];
    let num = |st: &[PdfTok], k: usize| -> f64 {
        match st.get(st.len().wrapping_sub(k)) {
            Some(PdfTok::Num(v)) => *v,
            _ => 0.0,
        }
    };
    for t in pdf_tokens(ops) {
        let PdfTok::Op(op) = &t else {
            stack.push(t);
            continue;
        };
        match op.as_str() {
            "g" => fill = gray(num(&stack, 1)),
            "G" => stroke = gray(num(&stack, 1)),
            "rg" => fill = rgb(num(&stack, 3), num(&stack, 2), num(&stack, 1)),
            "RG" => stroke = rgb(num(&stack, 3), num(&stack, 2), num(&stack, 1)),
            "w" => lw = num(&stack, 1),
            "d" => dash = matches!(stack.get(stack.len().wrapping_sub(2)), Some(PdfTok::Arr(a)) if !a.is_empty()),
            "m" => path.push(vec![to(num(&stack, 2), num(&stack, 1))]),
            "l" => {
                let pt = to(num(&stack, 2), num(&stack, 1));
                if let Some(last) = path.last_mut() {
                    last.push(pt);
                }
            }
            "re" => {
                let (x, y, w, h) = (num(&stack, 4), num(&stack, 3), num(&stack, 2), num(&stack, 1));
                rects.push(Rect::from_two_pos(to(x, y), to(x + w, y + h)));
            }
            "S" | "f" => {
                let st = Stroke::new((lw as f32 * s).max(0.6), stroke);
                if op == "f" {
                    for r in rects.drain(..) {
                        p.rect_filled(r, CornerRadius::ZERO, fill);
                    }
                } else {
                    for r in rects.drain(..) {
                        p.rect_stroke(r, CornerRadius::ZERO, st, StrokeKind::Middle);
                    }
                }
                for seg in path.drain(..) {
                    if dash {
                        p.extend(Shape::dotted_line(&seg, stroke, 2.5, 0.5));
                    } else {
                        p.add(Shape::line(seg, st));
                    }
                }
            }
            "Tf" => {
                if let Some(PdfTok::Name(n)) = stack.get(stack.len().wrapping_sub(2)) {
                    font = (n.clone(), num(&stack, 1));
                }
            }
            "Td" => tpos = (num(&stack, 2), num(&stack, 1)),
            "Tj" => {
                if let Some(shown) = stack.last().and_then(|t| shown_text(t, &font.0, fallback)) {
                    let size = (font.1 as f32 * s).max(1.0);
                    let fam = pdf_font_family(p.ctx(), &font.0);
                    let base = to(tpos.0, tpos.1);
                    if size < 3.5 {
                        // Card thumbnails: glyphs this small vanish, so text reads as a grey bar
                        // of the run's real width (measured at 10 px and scaled down).
                        let t = shown.clone();
                        if !t.trim().is_empty() {
                            let w = p.layout_no_wrap(t, FontId::new(10.0, fam), fill).size().x * size / 10.0;
                            let bar = Rect::from_min_size(Pos2::new(base.x, base.y - size * 0.62), Vec2::new(w, (size * 0.5).max(0.8)));
                            p.rect_filled(bar, CornerRadius::ZERO, fill.gamma_multiply(0.45));
                        }
                    } else {
                        p.text(Pos2::new(base.x, base.y - size * 0.78), Align2::LEFT_TOP, shown, FontId::new(size, fam), fill);
                    }
                }
            }
            "cm" => {
                cm = [num(&stack, 6), num(&stack, 5), num(&stack, 4), num(&stack, 3), num(&stack, 2), num(&stack, 1)];
            }
            "Do" => {
                if let Some(PdfTok::Name(n)) = stack.last() {
                    let k: usize = n.trim_start_matches("Im").parse().unwrap_or(1);
                    let r = Rect::from_two_pos(to(cm[4], cm[5]), to(cm[4] + cm[0], cm[5] + cm[3]));
                    match images.get(k.saturating_sub(1)).copied().flatten() {
                        Some(tex) => {
                            p.image(tex, r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }
                        None => {
                            p.rect_filled(r, CornerRadius::ZERO, Color32::from_gray(225));
                        }
                    }
                }
            }
            "Q" => cm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            _ => {}
        }
        stack.clear();
    }
}

/// The export preview's pages, cached in temp memory per exam content + what is exported.
fn export_preview(app: &mut crate::App, ctx: &egui::Context, exam_id: &Id, key: Option<KeyLayout>) -> Option<(crate::export::PdfPreview, Vec<Option<egui::TextureId>>)> {
    let exam = app.store.exam(exam_id)?.clone();
    let placed = resolve_items(&app.store, &exam);
    if placed.is_empty() {
        return None;
    }
    let sig = {
        let mut h = fnv1a(serde_json::to_string(&exam).unwrap_or_default().as_bytes());
        for p in &placed {
            h = h.rotate_left(7) ^ fnv1a(serde_json::to_string(&p.q).unwrap_or_default().as_bytes());
        }
        h ^ key.map_or(0, |k| 1 + k as u64)
    };
    let id = egui::Id::new(("qs_export_preview", exam_id));
    let cached: Option<(u64, crate::export::PdfPreview, Vec<String>)> = ctx.data(|d| d.get_temp(id));
    let (pv, files) = match cached {
        Some((s, pv, files)) if s == sig => (pv, files),
        _ => {
            let mapped = map_placed(&placed, exam.paper_style);
            let doc = match key {
                Some(k) => build_key(&exam, &mapped, None, k),
                None => build_doc(&exam, &mapped, None, false),
            };
            let pv = crate::export::preview_pages(&doc).ok()?;
            let files: Vec<String> = mapped
                .iter()
                .zip(doc.questions.iter())
                .filter(|(_, dq)| dq.image.is_some())
                .map(|(m, _)| m.img.as_ref().map(|r| r.file.clone()).unwrap_or_default())
                .collect();
            ctx.data_mut(|d| d.insert_temp(id, (sig, pv.clone(), files.clone())));
            (pv, files)
        }
    };
    let texs = files.iter().map(|f| app.image_texture(ctx, f).map(|t| t.id())).collect();
    Some((pv, texs))
}

/// First-page thumbnail of an exam (Home and Exams cards), painted from the same page
/// layout the export uses, fitted into `area` (centered, drop shadow, thin edge). Returns
/// the page rect, or None when the exam has no questions yet (callers draw their blank sheet).
pub(crate) fn paint_exam_thumb(app: &mut crate::App, ctx: &egui::Context, p: &egui::Painter, area: Rect, exam_id: &Id) -> Option<Rect> {
    let (pv, texs) = export_preview(app, ctx, exam_id, None)?;
    let ops = pv.pages.first()?;
    let aspect = (pv.page_w / pv.page_h) as f32;
    let h = area.height().min(area.width() / aspect);
    let r = Rect::from_center_size(area.center(), Vec2::new(h * aspect, h));
    p.add(egui::Shadow { offset: [0, 4], blur: 10, spread: 0, color: Color32::from_black_alpha(45) }.as_shape(r, CornerRadius::same(1)));
    // A real rasterisation of the page (text glyphs, table rules, pictures downscaled) at the
    // card's pixel size; the vector painter is the fallback.
    let px_w = (r.width() * ctx.pixels_per_point()).round().max(8.0) as u32;
    match page_thumb_texture(app, ctx, exam_id, &pv, ops, px_w) {
        Some(tex) => {
            p.image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => paint_pdf_page(&p.with_clip_rect(r.intersect(p.clip_rect())), r, pv.page_w, pv.page_h, ops, &texs, &pv.fallback),
    }
    p.rect_stroke(r, CornerRadius::same(1), Stroke::new(1.0, Color32::from_rgb(0xcf, 0xc9, 0xb6)), StrokeKind::Inside);
    Some(r)
}

/// The first page rasterised for a card thumbnail, cached per page content and pixel width.
fn page_thumb_texture(app: &mut crate::App, ctx: &egui::Context, exam_id: &Id, pv: &crate::export::PdfPreview, ops: &[u8], px_w: u32) -> Option<egui::TextureHandle> {
    let files: Vec<String> = ctx.data(|d| d.get_temp::<(u64, crate::export::PdfPreview, Vec<String>)>(egui::Id::new(("qs_export_preview", exam_id)))).map(|c| c.2).unwrap_or_default();
    let mut sig = fnv1a(ops) ^ (px_w as u64).rotate_left(32);
    for f in &files {
        sig = sig.rotate_left(5) ^ fnv1a(f.as_bytes());
    }
    let key = egui::Id::new(("qs_thumb_tex", exam_id));
    if let Some((s, tex)) = ctx.data(|d| d.get_temp::<(u64, Option<egui::TextureHandle>)>(key))
        && s == sig
    {
        return tex;
    }
    let _ = app;
    let (svg, placed) = page_svg(pv.page_w, pv.page_h, ops, &pv.fallback);
    // QA hook: VERSHION_THUMB_SVG=<file> writes the thumbnail's SVG (and the page ops beside it).
    if let Ok(path) = crate::env_var("VERSHION_THUMB_SVG") {
        let _ = std::fs::write(&path, &svg);
        let _ = std::fs::write(format!("{path}.ops"), ops);
    }
    let scale = px_w as f32 / pv.page_w as f32;
    let tex = crate::figures::to_rgba(&svg, scale).ok().map(|(w, h, mut px)| {
        // pictures: decoded and downscaled into their boxes (smooth, not nearest-pixel)
        for (k, [x, y, bw, bh]) in placed {
            let Some(file) = files.get(k).filter(|f| !f.is_empty()) else { continue };
            let Some(img) = crate::store::read_media(file).and_then(|b| image::load_from_memory(&b).ok()) else { continue };
            let (pw, ph) = (((bw as f32) * scale).round().max(1.0) as u32, ((bh as f32) * scale).round().max(1.0) as u32);
            let small = image::imageops::resize(&img.to_rgba8(), pw, ph, image::imageops::FilterType::Triangle);
            let (x0, y0) = ((x as f32 * scale).round() as i64, (y as f32 * scale).round() as i64);
            blend_premul(&mut px, w, h, &small, x0, y0);
        }
        let img = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &px);
        ctx.load_texture(format!("qs_thumb_{exam_id}"), img, egui::TextureOptions::LINEAR)
    });
    ctx.data_mut(|d| d.insert_temp(key, (sig, tex.clone())));
    tex
}

/// Alpha-blend straight-alpha `src` onto premultiplied RGBA `dst` (`w` x `h`) at `x0`, `y0`.
fn blend_premul(dst: &mut [u8], w: u32, h: u32, src: &image::RgbaImage, x0: i64, y0: i64) {
    for (sx, sy, p) in src.enumerate_pixels() {
        let (x, y) = (x0 + sx as i64, y0 + sy as i64);
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            continue;
        }
        let i = ((y as u32 * w + x as u32) * 4) as usize;
        let a = p[3] as f32 / 255.0;
        for c in 0..3 {
            dst[i + c] = (p[c] as f32 * a + dst[i + c] as f32 * (1.0 - a)).round() as u8;
        }
        dst[i + 3] = (p[3] as f32 + dst[i + 3] as f32 * (1.0 - a)).round() as u8;
    }
}

fn xml_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// One PDF page (the content stream the export writes) as an SVG document in points with y
/// down: text runs in the print serif's four faces, filled / stroked rectangles and lines (dashed ones
/// too). Pictures are left to the caller: their (image index, [x, y, w, h] in points) come back
/// beside the SVG (which shows a pale box there).
pub(crate) fn page_svg(page_w: f64, page_h: f64, ops: &[u8], fallback: &[char]) -> (String, Vec<(usize, [f64; 4])>) {
    let mut placed = Vec::new();
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{page_w}\" height=\"{page_h}\" viewBox=\"0 0 {page_w} {page_h}\"><rect width=\"{page_w}\" height=\"{page_h}\" fill=\"#fff\"/>"
    );
    let fy = |y: f64| page_h - y;
    let col = |c: [f64; 3]| format!("#{:02x}{:02x}{:02x}", (c[0].clamp(0.0, 1.0) * 255.0) as u8, (c[1].clamp(0.0, 1.0) * 255.0) as u8, (c[2].clamp(0.0, 1.0) * 255.0) as u8);
    let mut stack: Vec<PdfTok> = Vec::new();
    let (mut fill, mut stroke) = ([0.0; 3], [0.0; 3]);
    let mut lw = 1.0_f64;
    let mut dash = false;
    let mut font = (String::from("F1"), 11.0_f64);
    let mut tpos = (0.0_f64, 0.0_f64);
    let mut path: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut rects: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut cm = [1.0_f64, 0.0, 0.0, 1.0, 0.0, 0.0];
    let num = |st: &[PdfTok], k: usize| -> f64 {
        match st.get(st.len().wrapping_sub(k)) {
            Some(PdfTok::Num(v)) => *v,
            _ => 0.0,
        }
    };
    for t in pdf_tokens(ops) {
        let PdfTok::Op(op) = &t else {
            stack.push(t);
            continue;
        };
        match op.as_str() {
            "g" => fill = [num(&stack, 1); 3],
            "G" => stroke = [num(&stack, 1); 3],
            "rg" => fill = [num(&stack, 3), num(&stack, 2), num(&stack, 1)],
            "RG" => stroke = [num(&stack, 3), num(&stack, 2), num(&stack, 1)],
            "w" => lw = num(&stack, 1),
            "d" => dash = matches!(stack.get(stack.len().wrapping_sub(2)), Some(PdfTok::Arr(a)) if !a.is_empty()),
            "m" => path.push(vec![(num(&stack, 2), fy(num(&stack, 1)))]),
            "l" => {
                let pt = (num(&stack, 2), fy(num(&stack, 1)));
                if let Some(last) = path.last_mut() {
                    last.push(pt);
                }
            }
            "re" => {
                let (x, y, w, h) = (num(&stack, 4), num(&stack, 3), num(&stack, 2), num(&stack, 1));
                rects.push((x, fy(y + h.max(0.0)).min(fy(y)), w.abs(), h.abs()));
            }
            "S" | "f" => {
                for (x, y, w, h) in rects.drain(..) {
                    if op == "f" {
                        out.push_str(&format!("<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"{}\"/>", col(fill)));
                    } else {
                        out.push_str(&format!("<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\"/>", col(stroke), lw.max(0.3)));
                    }
                }
                for seg in path.drain(..) {
                    let pts: Vec<String> = seg.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
                    let da = if dash { " stroke-dasharray=\"2.5 2\"" } else { "" };
                    out.push_str(&format!("<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\"{da}/>", pts.join(" "), col(stroke), lw.max(0.3)));
                }
            }
            "Tf" => {
                if let Some(PdfTok::Name(n)) = stack.get(stack.len().wrapping_sub(2)) {
                    font = (n.clone(), num(&stack, 1));
                }
            }
            "Td" => tpos = (num(&stack, 2), num(&stack, 1)),
            "Tj" => {
                if let Some(text) = stack.last().and_then(|t| shown_text(t, &font.0, fallback)) {
                    if !text.trim().is_empty() {
                        let f = base_font(&font.0);
                        let bold = f == "F2" || f == "F4";
                        let ital = f == "F3" || f == "F4";
                        out.push_str(&format!(
                            "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"'Source Serif 4', 'Liberation Serif', 'DejaVu Serif', serif\" font-size=\"{:.2}\"{}{} fill=\"{}\" xml:space=\"preserve\">{}</text>",
                            tpos.0,
                            fy(tpos.1),
                            font.1,
                            if bold { " font-weight=\"bold\"" } else { "" },
                            if ital { " font-style=\"italic\"" } else { "" },
                            col(fill),
                            xml_text(&text)
                        ));
                    }
                }
            }
            "cm" => cm = [num(&stack, 6), num(&stack, 5), num(&stack, 4), num(&stack, 3), num(&stack, 2), num(&stack, 1)],
            "Do" => {
                if let Some(PdfTok::Name(n)) = stack.last() {
                    let k: usize = n.trim_start_matches("Im").parse().unwrap_or(1);
                    let (x, y, w, h) = (cm[4], fy(cm[5] + cm[3]), cm[0], cm[3]);
                    out.push_str(&format!("<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#efefef\"/>"));
                    placed.push((k.saturating_sub(1), [x, y, w, h]));
                }
            }
            "Q" => cm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            _ => {}
        }
        stack.clear();
    }
    out.push_str("</svg>");
    (out, placed)
}

/// The export dialog's preview strip: page thumbnails of the document that will export.
fn draw_export_preview(app: &mut crate::App, ui: &mut egui::Ui, exam_id: &Id, key: Option<KeyLayout>) {
    let ctx = ui.ctx().clone();
    let Some((pv, texs)) = export_preview(app, &ctx, exam_id, key) else {
        ui.add(egui::Label::new(muted("Add questions to see the pages.")));
        return;
    };
    let n = pv.pages.len();
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(muted(RichText::new(if key.is_some() { "Answer key preview" } else { "Preview" }).size(11.5))));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(muted(RichText::new(format!("{n} page{}", if n == 1 { "" } else { "s" })).size(11.5))));
        });
    });
    let thumb_w = 178.0_f32;
    let thumb_h = thumb_w * (pv.page_h / pv.page_w) as f32;
    egui::ScrollArea::horizontal()
        .id_salt("export_preview_strip")
        .max_height(thumb_h + 26.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for (k, ops) in pv.pages.iter().enumerate() {
                    ui.vertical(|ui| {
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(thumb_w, thumb_h), Sense::hover());
                        let p = ui.painter().with_clip_rect(r);
                        ui.painter().add(egui::Shadow { offset: [0, 4], blur: 10, spread: 0, color: Color32::from_black_alpha(40) }.as_shape(r, CornerRadius::same(1)));
                        paint_pdf_page(&p, r, pv.page_w, pv.page_h, ops, &texs, &pv.fallback);
                        ui.painter().rect_stroke(r, CornerRadius::same(1), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
                        resp.on_hover_ui(|ui| {
                            // A larger look at the hovered page.
                            let w = 420.0_f32;
                            let h = w * (pv.page_h / pv.page_w) as f32;
                            let (big, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
                            paint_pdf_page(&ui.painter().with_clip_rect(big), big, pv.page_w, pv.page_h, ops, &texs, &pv.fallback);
                        });
                        ui.add(egui::Label::new(muted(RichText::new(format!("Page {}", k + 1)).size(10.5))));
                    });
                }
            });
        });
}

// ---------------------------------------------------------------------------
// Export pipeline
// ---------------------------------------------------------------------------

fn xdg_open(path: &std::path::Path) {
    let _ = crate::platform::open_path(path);
}

impl crate::App {
    fn draw_export_dialog(&mut self, ctx: &egui::Context, exam_id: &Id) {
        let mut exu: ExportUi = mem_load(ctx, export_id());
        let mut open = self.export_open;
        let mut cancel = false;

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
            .default_width(640.0)
            .show(ctx, |ui| {
                card_frame(theme::cream()).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(theme::serif_strong(name.clone()).size(16.0).color(theme::ink())).truncate());
                        ui.add(egui::Label::new(muted(format!(
                            "{n_items} question{} · {} total points",
                            if n_items == 1 { "" } else { "s" },
                            fmt_number(n_points),
                        ))));
                    });
                    ui.add_space(4.0);
                    // What will export: the student test, or the key when only the key is ticked.
                    draw_export_preview(self, ui, exam_id, (exu.key && !exu.student).then_some(exu.key_layout));
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
                ui.add_enabled_ui(exu.key, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(22.0);
                        ui.add(egui::Label::new(muted(egui::RichText::new("Key layout").size(11.5))));
                        for k in KeyLayout::ALL {
                            let tip = match k {
                                KeyLayout::Section => "Test Parrot's answer section: each number with its answer written on a blank",
                                KeyLayout::Marked => "The whole test with the correct answers marked",
                                KeyLayout::List => "One short line per question",
                            };
                            if kit::chip_toggle(ui, k.label(), exu.key_layout == k).on_hover_text(tip).clicked() {
                                exu.key_layout = k;
                            }
                        }
                    });
                });
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
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(muted(
                        "Export History is kept only on this computer, in verSHion's data folder. It is useful for re-exporting an earlier copy, but it is not an archival backup."
                            .to_string(),
                    ).size(11.5))
                    .wrap(),
                );
                // Gate: questions with unresolved [picture] placeholders block the download
                // unless the teacher chooses to export anyway.
                let gaps = self.store.exam(exam_id).map(|e| picture_gaps(&self.store, e)).unwrap_or_default();
                if !gaps.is_empty() {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(RichText::new(picture_gate_text(&gaps)).size(12.5).color(theme::bad())).wrap());
                    ui.checkbox(&mut exu.anyway, "Export anyway (the placeholders print as [picture])");
                }
                ui.add_space(10.0);
                let label = if exu.docx { "Download DOCX" } else { "Download PDF" };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ready = n_items > 0 && (exu.student || exu.key) && (gaps.is_empty() || exu.anyway);
                    let clicked = ui
                        .add_enabled_ui(ready, |ui| kit::primary_button(ui, label).clicked())
                        .inner;
                    if clicked {
                        do_export(self, exam_id, exu.clone());
                    }
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if cancel {
            open = false;
        }
        self.export_open = open;
        if !open {
            exu.anyway = false;
        }
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

    let Some(path) = crate::platform::save_path(&format!("{}.{}", exam.name, ext), filter, &[ext])
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
    let placed = resolve_items(&app.store, &exam);
    let n_missing = exam.items.len() - placed.len();
    if placed.is_empty() {
        return Err(
            "none of the exam's questions resolve (bank questions deleted?)".to_string(),
        );
    }

    // Deterministic per-run base seed: exam-id hash mixed with wall time.
    let t = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
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
        // This version's questions: shuffled within each section (headings stay put), answers
        // shuffled inside each question.
        let mut vqs = placed.clone();
        if exu.shuffle_q {
            vqs = shuffle_placed_in_sections(vqs, &mut rnd);
        }
        if exu.shuffle_a {
            for p in vqs.iter_mut() {
                shuffle_answers(&mut p.q, &mut rnd);
            }
        }
        let label = if n_versions > 1 {
            Some(format!("Version {}", version_letter(v)))
        } else {
            None
        };
        let mapped: Vec<Mapped> = map_placed(&vqs, exam.paper_style);

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
            crate::platform::write(&out_path, &bytes).map_err(|e| e.to_string())?;
            files.push(out_path.to_string_lossy().to_string());
            wrote += 1;
            snap_write(
                &doc,
                &mapped,
                label.clone(),
                &dir,
                &format!("{stem}{suffix}.doc.json"),
                &out_name,
                &mut files,
                &mut wrote,
            );
        }
        if exu.key {
            let doc = build_key(&exam, &mapped, label.clone(), exu.key_layout);
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
            crate::platform::write(&out_path, &bytes).map_err(|e| e.to_string())?;
            files.push(out_path.to_string_lossy().to_string());
            wrote += 1;
            let snap_suffix = if n_versions > 1 {
                suffix.clone()
            } else {
                String::new()
            };
            snap_write(
                &doc,
                &mapped,
                label,
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
    doc: &ExamDoc,
    mapped: &[Mapped],
    label: Option<String>,
    dir: &std::path::Path,
    json_name: &str,
    out_file: &str,
    files: &mut Vec<String>,
    wrote: &mut usize,
) {
    let snap = SnapDoc::from_doc(doc, mapped, label, out_file.to_string());
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

    #[test]
    fn thumbnail_svg_keeps_text_tables_and_picture_boxes() {
        use crate::layout::{DocImage, DocQ, ExamDoc};
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 0, 0, 255])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let doc = ExamDoc {
            title: "Quiz & <Test>".into(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            questions: vec![
                DocQ { number: 1, text: vec!["Name **these**:\nFormula | Name\nKI | ___".into()], image: Some(DocImage { bytes: png, width_px: 40, height_px: 30 }), ..Default::default() },
            ],
            ..Default::default()
        };
        let pv = crate::export::preview_pages(&doc).unwrap();
        let (svg, placed) = page_svg(pv.page_w, pv.page_h, &pv.pages[0], &pv.fallback);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        assert!(svg.contains("Quiz &amp; &lt;Test&gt;"), "title text, escaped");
        assert!(svg.contains("font-weight=\"bold\""), "bold runs stay bold");
        assert!(svg.contains(">Formula<") || svg.contains(">Formula "), "table text");
        assert!(svg.matches("<rect").count() >= 3, "table cell borders");
        assert_eq!(placed.len(), 1, "one picture box");
        assert!(placed[0].1[2] > 10.0 && placed[0].1[3] > 10.0);
        // it rasterises (the thumbnail path)
        let (w, h, px) = crate::figures::to_rgba(&svg, 0.25).unwrap();
        assert!(w > 100 && h > 100 && px.len() == (w * h * 4) as usize);
        // Thumbnail text is drawn (in each of the four serif faces) -- a font list resvg cannot
        // parse renders no text at all.
        for f in ["F1", "F2", "F3", "F4"] {
            let ops = format!("BT /{f} 20 Tf 10 30 Td (Hamburgefonstiv) Tj ET\n");
            let (svg, _) = page_svg(220.0, 50.0, ops.as_bytes(), &[]);
            assert!(svg.contains("'Source Serif 4'"), "{svg}");
            let (_, _, px) = crate::figures::to_rgba(&svg, 1.0).unwrap();
            let ink = px.chunks(4).filter(|p| p[0] < 110).count();
            assert!(ink > 150, "{f}: only {ink} dark pixels");
        }
        // pictures blend into the raster
        let mut buf = vec![255u8; 4 * 4 * 4];
        blend_premul(&mut buf, 4, 4, &image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 255])), 1, 1);
        assert_eq!(&buf[(4 + 1) * 4..(4 + 1) * 4 + 4], &[0, 0, 0, 255]);
        assert_eq!(&buf[0..4], &[255, 255, 255, 255]);
    }
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
        Exam { paper_style: Default::default(), margins: None,
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
        ExamItem::new(bank.to_string(), qid.to_string())
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

    #[test]
    fn answer_columns_are_stored_and_reach_the_print_for_mc_only() {
        // Old exam items (no field) read as 1 and save without it.
        let old: ExamItem = serde_json::from_str(r#"{"bank_id":"b","question_id":"q"}"#).unwrap();
        assert_eq!((old.answer_columns, old.answer_cols()), (1, 1));
        assert!(!serde_json::to_string(&old).unwrap().contains("answer_columns"));
        let mut two = old.clone();
        two.answer_columns = 2;
        let js = serde_json::to_string(&two).unwrap();
        assert!(js.contains(r#""answer_columns":2"#));
        assert_eq!(serde_json::from_str::<ExamItem>(&js).unwrap().answer_cols(), 2);
        let mut junk = old.clone();
        junk.answer_columns = 9;
        assert_eq!(junk.answer_cols(), 1);
        assert_eq!(model::ANSWER_COLUMNS, [1, 2, 4]);
        assert!(!same_items(&[old.clone()], &[two.clone()]), "a column change is an edit (undo, dirty)");
        // Mapping: MC questions carry the columns, other kinds never do.
        let mcq = q("q1", "Pick.", mc(&[("a", false, false), ("b", true, false)]));
        let tfq = q("q2", "True?", Kind::Tf { correct: true });
        let placed = vec![Placed { item: two.clone(), q: mcq }, Placed { item: two, q: tfq }];
        let m = map_placed(&placed, model::PaperStyle::Standard);
        assert_eq!(m[0].dq.answer_columns, 2);
        assert_eq!(m[1].dq.answer_columns, 0);
        // The snapshot keeps it (and old snapshots read as 0).
        let e = exam(vec![item("b", "q1")]);
        let snap = SnapDoc::from_parts(&e, &m[..1], None, false, "x.pdf".into());
        assert_eq!(serde_json::from_str::<SnapDoc>(&serde_json::to_string(&snap).unwrap()).unwrap().to_doc().questions[0].answer_columns, 2);
    }

    #[test]
    fn key_marks_and_correct_choices_are_mapped() {
        let m = map_question(1, &q("q1", "Pick.", mc(&[("a", false, false), ("b", true, false), ("c", true, false)])));
        assert_eq!(m.dq.correct, vec![1, 2]);
        assert_eq!(m.dq.key_marks, vec!["B, C".to_string()]);
        let m = map_question(1, &q("q2", "T?", Kind::Tf { correct: false }));
        assert_eq!((m.dq.correct.clone(), m.dq.key_marks.clone()), (vec![1], vec!["False".to_string()]));
        let kind = Kind::Matching {
            pairs: vec![Pair { left: "Dog".into(), right: "Bark".into() }, Pair { left: "Cat".into(), right: "Meow".into() }],
            distractors: vec![],
        };
        let m = map_question(1, &q("q3", "", kind));
        assert_eq!(m.dq.key_marks, vec!["A".to_string(), "B".to_string()]);
        let m = map_question(1, &q("q4", "Why?", Kind::Short { sample: "Because.".into() }));
        assert!(m.dq.key_marks.is_empty() && m.dq.correct.is_empty());
    }

    #[test]
    fn key_layouts_build_and_old_snapshots_stay_lists() {
        let mut e = exam(vec![item("b", "q1"), item("b", "q2")]);
        e.items[0].section = Some(model::ExamSection { title: "Part A".into(), directions: String::new() });
        let mut st = store::Store::default();
        let _ = &mut st;
        let mapped = vec![
            map_question(1, &q("q1", "Pick.", mc(&[("a", false, false), ("b", true, false)]))),
            map_question(2, &q("q2", "Why?", Kind::Short { sample: "Because.".into() })),
        ];
        let mut mapped = mapped;
        mapped[0].dq.section = Some(crate::layout::DocSection { title: "Part A".into(), directions: String::new() });
        let list = build_key(&e, &mapped, None, KeyLayout::List);
        assert_eq!(list.key_layout, KeyLayout::List);
        assert!(list.questions[0].section.is_none() && list.questions[0].choices.is_empty());
        let sec = build_key(&e, &mapped, None, KeyLayout::Section);
        assert!(sec.is_key && sec.questions[0].section.is_some() && sec.questions[0].choices.is_empty());
        let marked = build_key(&e, &mapped, None, KeyLayout::Marked);
        assert!(marked.is_key && !marked.questions[0].choices.is_empty() && marked.title.ends_with("Answer Key"));
        assert!(crate::export::render_pdf(&marked).is_ok() && crate::export::render_docx(&sec).is_ok());
        // Snapshots: the layout round-trips; one written before R7 reads as the list.
        let snap = SnapDoc::from_doc(&sec, &mapped, None, "k.pdf".into());
        let back: SnapDoc = serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        assert_eq!(back.to_doc().key_layout, KeyLayout::Section);
        assert_eq!(back.to_doc().questions[0].key_marks, vec!["B".to_string()]);
        let old = r#"{"title":"T","header_line":"","heading_pt":16,"body_pt":11,"margin_mm":18,"answers_inline":true,"is_key":true,"version_label":null,"ruled_blanks":0,"questions":[{"number":1,"text":[],"choices":[],"image":null,"key_text":"B","rule_lines":0,"points":null}],"out_file":"o.pdf"}"#;
        let old: SnapDoc = serde_json::from_str(old).unwrap();
        assert_eq!(old.to_doc().key_layout, KeyLayout::List);
        assert!(!serde_json::to_string(&old).unwrap().contains("key_layout"));
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

    fn plain_q(kind: Kind) -> model::Question {
        model::Question {
            stem: "Match".into(),
            kind,
            id: "q".into(),
            points: Some(1.0),
            difficulty: None,
            topics: vec![],
            image: None,
            explanation: None,
            figure: None,
            created: 0,
            modified: 0,
        }
    }

    #[test]
    fn matching_column_has_no_blank_or_repeated_choice_and_the_key_agrees() {
        let p = |l: &str, r: &str| model::Pair { left: l.into(), right: r.into() };
        // Two lefts share a right item.
        let q = plain_q(Kind::Matching { pairs: vec![p("Dog", "Mammal"), p("Cat", "Mammal"), p("Frog", "Amphibian")], distractors: vec![] });
        let m = map_question_raw(1, &q);
        assert_eq!(m.dq.choices, ["A. Mammal", "B. Amphibian"]);
        assert_eq!(m.dq.key_text, "1-A 2-A 3-B");
        assert_eq!(m.dq.key_marks, ["A", "A", "B"]);
        // A distractor equal to a right item, and a blank one.
        let q = plain_q(Kind::Matching { pairs: vec![p("a", "X"), p("b", "Y")], distractors: vec!["X".into(), " ".into(), "Z".into()] });
        let m = map_question_raw(1, &q);
        assert_eq!(m.dq.choices, ["A. X", "B. Y", "C. Z"]);
        assert_eq!(m.dq.key_text, "1-A 2-B");
    }

    #[test]
    fn keyless_questions_print_no_key() {
        let p = |l: &str, r: &str| model::Pair { left: l.into(), right: r.into() };
        for kind in [
            Kind::Tf { correct: false },
            Kind::Mc { answers: vec![model::Answer { text: "x".into(), correct: false, locked: false }, model::Answer { text: "y".into(), correct: false, locked: false }] },
            Kind::Matching { pairs: vec![p("Dog", "Mammal"), p("Frog", "Amphibian")], distractors: vec!["Bird".into()] },
            Kind::Blank { accept: vec![vec![]] },
            Kind::Short { sample: String::new() },
        ] {
            let mut q = plain_q(kind);
            q.explanation = Some(model::KEYLESS_SENTINEL.into());
            let m = map_question_raw(1, &q);
            assert_eq!(m.dq.key_text, "Key not provided");
            assert!(m.dq.key_marks.is_empty() && m.dq.correct.is_empty(), "{:?}", m.dq.key_marks);
            // The student side is untouched.
            if let Kind::Matching { .. } = q.kind {
                assert_eq!(m.dq.choices, ["A. Mammal", "B. Amphibian", "C. Bird"]);
            }
        }
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
        assert_eq!(page_content_top(0.0, PAGE_H, 30.0, h), 30.0, "page 1: header is part of the body");
        assert_eq!(page_content_top(1.0, PAGE_H, 30.0, h), PAGE_H + 30.0 + h);
        assert_eq!(page_content_top(2.0, PAGE_H, 30.0, 0.0), 2.0 * PAGE_H + 30.0);
    }

    #[test]
    fn duplicate_puts_the_copy_right_after_the_item() {
        let it = |q: &str| ExamItem::new("b", q);
        let out = with_copy_after(vec![it("a"), it("b"), it("c")], 1, &"b".to_string(), &"b2".to_string());
        let ids: Vec<&str> = out.iter().map(|x| x.question_id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "b2", "c"]);
        let out = with_copy_after(vec![it("a")], 0, &"b".to_string(), &"a2".to_string());
        assert_eq!(out[1].question_id, "a2");
        let out = with_copy_after(Vec::new(), 3, &"b".to_string(), &"z".to_string());
        assert_eq!(out.len(), 1, "an out-of-range index appends");
    }
}

#[cfg(test)]
mod examed_tests {
    use super::*;
    use crate::model::{Answer, ExamSection, PaperStyle, Question};

    fn it(q: &str) -> ExamItem {
        ExamItem::new("b", q)
    }
    fn sec(t: &str) -> Option<ExamSection> {
        Some(ExamSection { title: t.into(), directions: String::new() })
    }
    fn ids(items: &[ExamItem]) -> Vec<String> {
        items.iter().map(|i| i.question_id.clone()).collect()
    }
    fn heads(items: &[ExamItem]) -> Vec<Option<String>> {
        items.iter().map(|i| i.section.as_ref().map(|s| s.title.clone())).collect()
    }
    /// q1 q2 | B: q3 q4 | C: q5
    fn sample() -> Vec<ExamItem> {
        let mut v: Vec<ExamItem> = ["q1", "q2", "q3", "q4", "q5"].iter().map(|q| it(q)).collect();
        v[2].section = sec("B");
        v[4].section = sec("C");
        v
    }
    fn mcq(id: &str, answers: &[(&str, bool, bool)]) -> Question {
        Question {
            id: id.into(),
            stem: format!("Stem {id}"),
            kind: Kind::Mc { answers: answers.iter().map(|(t, c, l)| Answer { text: t.to_string(), correct: *c, locked: *l }).collect() },
            points: Some(1.0),
            difficulty: None,
            topics: vec![],
            image: None,
            explanation: None,
            figure: None,
            created: 1,
            modified: 1,
        }
    }

    #[test]
    fn section_ranges_start_at_zero_and_every_heading() {
        assert!(section_ranges(&[]).is_empty());
        assert_eq!(section_ranges(&sample()), vec![0..2, 2..4, 4..5]);
        let mut v = sample();
        v[0].section = sec("A");
        assert_eq!(section_ranges(&v), vec![0..2, 2..4, 4..5], "a heading on item 0 adds no empty range");
    }

    #[test]
    fn insert_section_below_marks_the_next_question() {
        let v = with_section_below(sample(), 0);
        assert_eq!(heads(&v), vec![None, Some("".into()), Some("B".into()), None, Some("C".into())]);
        // Already a boundary, or the last question: no-op.
        assert_eq!(heads(&with_section_below(sample(), 1)), heads(&sample()));
        assert_eq!(heads(&with_section_below(sample(), 4)), heads(&sample()));
    }

    #[test]
    fn removing_a_sections_first_question_keeps_the_section() {
        let v = without_item(sample(), 2);
        assert_eq!(ids(&v), vec!["q1", "q2", "q4", "q5"]);
        assert_eq!(heads(&v), vec![None, None, Some("B".into()), Some("C".into())]);
        // Removing the only question of a section whose next item already starts one: the
        // later heading wins (the emptied section disappears).
        let v = without_item(sample(), 4);
        assert_eq!(heads(&v), vec![None, None, Some("B".into()), None]);
    }

    #[test]
    fn moving_questions_keeps_headings_in_place() {
        // Swap q2 (end of section 1) with q3 (start of B): q3 joins section 1, q2 opens B.
        let v = with_headings_fixed(sample(), |mut b| {
            b.swap(1, 2);
            b
        });
        assert_eq!(ids(&v), vec!["q1", "q3", "q2", "q4", "q5"]);
        assert_eq!(heads(&v), heads(&sample()));
    }

    #[test]
    fn section_move_merge_and_delete() {
        let v = with_section_moved(sample(), 1, -1);
        assert_eq!(ids(&v), vec!["q3", "q4", "q1", "q2", "q5"]);
        assert_eq!(heads(&v), vec![Some("B".into()), None, Some("".into()), None, Some("C".into())], "the headless first block gains a blank heading");
        let v = with_section_moved(sample(), 2, 1);
        assert_eq!(ids(&v), ids(&sample()), "moving the last section down is a no-op");
        let v = with_section_merged_up(sample(), 1);
        assert_eq!(section_ranges(&v), vec![0..4, 4..5]);
        let v = with_section_deleted(sample(), 1);
        assert_eq!(ids(&v), vec!["q1", "q2", "q5"]);
        assert_eq!(heads(&v), vec![None, None, Some("C".into())]);
    }

    #[test]
    fn shuffling_a_section_stays_inside_it() {
        let mut v: Vec<ExamItem> = (0..12).map(|k| it(&format!("q{k}"))).collect();
        v[4].section = sec("B");
        v[9].section = sec("C");
        for seed in 0..20u64 {
            let s = with_section_shuffled(v.clone(), 5, &mut lcg(seed));
            assert_eq!(heads(&s), heads(&v));
            assert_eq!(ids(&s)[..4], ids(&v)[..4]);
            assert_eq!(ids(&s)[9..], ids(&v)[9..]);
            let mut mid = ids(&s)[4..9].to_vec();
            mid.sort();
            let mut want = ids(&v)[4..9].to_vec();
            want.sort();
            assert_eq!(mid, want);
        }
        let all = with_all_sections_shuffled(v.clone(), &mut lcg(3));
        assert_eq!(heads(&all), heads(&v));
    }

    #[test]
    fn answer_order_shuffle_keeps_locked_answers() {
        let q = mcq("m", &[("A", false, false), ("B", true, false), ("C", false, true), ("D", false, false)]);
        for seed in 0..30u64 {
            let o = shuffled_answer_order(&q, &[], &mut lcg(seed)).expect("movable answers");
            assert!(is_permutation(&o, 4));
            assert_eq!(o[2], 2, "a locked answer keeps its slot");
            assert_ne!(o, vec![0, 1, 2, 3]);
            let mut q2 = q.clone();
            apply_answer_order(&mut q2, &o);
            let Kind::Mc { answers } = &q2.kind else { unreachable!() };
            assert_eq!(answers.iter().filter(|a| a.correct).count(), 1);
        }
        let one = mcq("o", &[("A", true, false), ("B", false, true)]);
        assert!(shuffled_answer_order(&one, &[], &mut lcg(1)).is_none());
        // A stale order (bank question edited since) is ignored.
        let mut q3 = q.clone();
        apply_answer_order(&mut q3, &[1, 0]);
        let Kind::Mc { answers } = &q3.kind else { unreachable!() };
        assert_eq!(answers[0].text, "A");
    }

    fn multipart_q(id: &str) -> model::Question {
        let mut q = mcq(id, &[("x", true, false)]);
        q.stem = "Answer the parts about $f(x)=x^2$.".into();
        q.kind = Kind::Multipart {
            parts: vec![
                model::Part { stem: "What is $f(3)$?".into(), kind: PartKind::Short { sample: "9".into() } },
                model::Part { stem: "$f$ is even.".into(), kind: PartKind::Tf { correct: true } },
                model::Part {
                    stem: "Which is $f'(x)$?".into(),
                    kind: PartKind::Mc { answers: vec![model::Answer { text: "$2x$".into(), correct: true, locked: false }, model::Answer { text: "$x$".into(), correct: false, locked: false }] },
                },
            ],
        };
        q
    }

    /// Per-part work space: stored on the exam item per part, printed under each part.
    #[test]
    fn multipart_parts_carry_their_own_work_space() {
        let q = multipart_q("m");
        let mut st = store::Store::default();
        st.banks.push(model::QuestionBank { id: "b".into(), name: "B".into(), questions: vec![q.clone()], created: 0, modified: 0, details: Default::default() });
        let mut ex = model::Exam { items: vec![it("m")], ..Default::default() };
        // No per-part space: one text entry exactly as before.
        let plain = map_placed(&resolve_items(&st, &ex), ex.paper_style);
        assert_eq!(plain[0].dq.text.len(), 1);
        assert!(plain[0].dq.space_after.is_empty());
        assert_eq!(plain[0].pieces.join("\n"), plain[0].dq.text[0], "the pieces rebuild the stem");
        // Space under parts a and c.
        ex.items = with_part_work(ex.items, 0, 2, 5);
        ex.items = with_part_work(ex.items, 0, 0, 2);
        assert_eq!(ex.items[0].part_work, vec![2, 0, 5]);
        let m = map_placed(&resolve_items(&st, &ex), ex.paper_style);
        assert_eq!(m[0].dq.text.len(), 3, "{:?}", m[0].dq.text);
        assert!(m[0].dq.text[0].starts_with("Answer the parts") && m[0].dq.text[0].ends_with("a. What is f(3)?"), "{:?}", m[0].dq.text[0]);
        assert!(m[0].dq.text[1].starts_with("b. f is even.") && m[0].dq.text[2].starts_with("c. Which is f'(x)?"), "{:?}", m[0].dq.text);
        assert_eq!(m[0].dq.space_after, vec![2, 0, 5]);
        assert_eq!(m[0].dq.text.join("\n"), plain[0].dq.text[0], "same printed text, cut at the parts");
        // Removing the space again leaves nothing stored.
        ex.items = with_part_work(ex.items, 0, 2, 0);
        ex.items = with_part_work(ex.items, 0, 0, 0);
        assert!(ex.items[0].part_work.is_empty());
        ex.items = with_part_work(ex.items, 0, 1, 30);
        assert_eq!(ex.items[0].part_work, vec![0, 23], "clamped to 23 lines");
        // The key drops it; the snapshot keeps it.
        let m = map_placed(&resolve_items(&st, &ex), ex.paper_style);
        let key = build_doc(&ex, &m, None, true);
        assert!(key.questions[0].space_after.is_empty());
        let snap = SnapDoc::from_parts(&ex, &m, None, false, "x.pdf".into());
        let back: SnapDoc = serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        assert_eq!(back.to_doc().questions[0].space_after, vec![0, 23, 0]);
        // Old snapshots load without it.
        let old: SnapQ = serde_json::from_str(r#"{"number":1,"text":["x"],"choices":[],"image":null,"key_text":"","rule_lines":0,"points":null}"#).unwrap();
        assert!(old.space_after.is_empty());
        // Item equality sees it (so the working copy notices the change).
        let mut a = vec![it("m")];
        let b = a.clone();
        a[0].part_work = vec![1];
        assert!(!same_items(&a, &b));
    }

    #[test]
    fn printed_doc_carries_sections_work_space_and_answer_order() {
        let mut st = store::Store::default();
        st.banks.push(model::QuestionBank {
            id: "b".into(),
            name: "B".into(),
            questions: vec![
                mcq("q1", &[("Ge", false, false), ("O", true, false)]),
                mcq("q2", &[("x", true, false), ("y", false, false)]),
                mcq("q3", &[("u", true, false), ("v", false, false)]),
            ],
            created: 0,
            modified: 0,
            details: Default::default(),
        });
        let mut ex = model::Exam { items: vec![it("q1"), it("gone"), it("q2"), it("q3")], ..Default::default() };
        ex.items[0].answer_order = vec![1, 0];
        ex.items[1].section = sec("Part II");
        ex.items[3].work_lines = 5;
        ex.paper_style = PaperStyle::Classic;
        let placed = resolve_items(&st, &ex);
        assert_eq!(placed.len(), 3, "the missing question is skipped");
        assert_eq!(placed[1].item.section.as_ref().map(|s| s.title.as_str()), Some("Part II"), "its heading moves to the next printed question");
        let mapped = map_placed(&placed, ex.paper_style);
        assert_eq!(mapped.iter().map(|m| m.dq.number).collect::<Vec<_>>(), vec![1, 2, 3], "numbering continues across sections");
        assert_eq!(mapped[0].dq.choices, vec!["A. O", "B. Ge"]);
        assert_eq!(mapped[0].dq.key_text, "A");
        assert_eq!(mapped[2].dq.work_lines, 5);
        let doc = build_doc(&ex, &mapped, None, false);
        assert_eq!(doc.style, PaperStyle::Classic);
        assert_eq!(doc.questions[1].section.as_ref().map(|s| s.title.as_str()), Some("Part II"));
        let key = build_doc(&ex, &mapped, None, true);
        assert!(key.questions.iter().all(|q| q.section.is_none() && q.work_lines == 0));
        // The snapshot round-trips the new fields.
        let snap = SnapDoc::from_parts(&ex, &mapped, None, false, "x.pdf".into());
        let back: SnapDoc = serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        let d2 = back.to_doc();
        assert_eq!(d2.style, PaperStyle::Classic);
        assert_eq!(d2.questions[2].work_lines, 5);
        assert_eq!(d2.questions[1].section.as_ref().map(|s| s.title.as_str()), Some("Part II"));
        // Old snapshots (no style / section fields) still load.
        let old = r#"{"title":"T","header_line":"","heading_pt":16,"body_pt":11,"margin_mm":18,"answers_inline":true,"is_key":false,"version_label":null,"ruled_blanks":0,"questions":[{"number":1,"text":["a"],"choices":[],"image":null,"key_text":"","rule_lines":0,"points":null}],"out_file":"o.pdf"}"#;
        let s: SnapDoc = serde_json::from_str(old).unwrap();
        assert_eq!(s.style, PaperStyle::Standard);
    }

    #[test]
    fn export_gate_lists_questions_still_needing_pictures() {
        let mut st = store::Store::default();
        let mut q2 = mcq("q2", &[("x", true, false), ("y", false, false)]);
        q2.stem = "Look at the graph. [picture: page 1]".into();
        let mut q4 = mcq("q4", &[("x", true, false), ("y", false, false)]);
        q4.stem = "Which arrow? [picture]".into();
        let mut q3 = mcq("q3", &[("x", true, false), ("y", false, false)]);
        q3.stem = "Attached already [picture]".into();
        q3.image = Some(model::ImageRef { file: "media/a.png".into(), width: 10, height: 10 });
        st.banks.push(model::QuestionBank {
            id: "b".into(),
            name: "B".into(),
            questions: vec![mcq("q1", &[("a", true, false), ("b", false, false)]), q2, q3, q4],
            created: 0,
            modified: 0,
            details: Default::default(),
        });
        let ex = model::Exam { items: vec![it("q1"), it("gone"), it("q2"), it("q3"), it("q4")], ..Default::default() };
        // printed numbers: the missing question is skipped, an attached picture resolves its marker
        assert_eq!(picture_gaps(&st, &ex), vec![2, 4]);
        assert_eq!(picture_gate_text(&[2, 4]), "Questions 2 and 4 still need pictures. Attach them in the bank first, or export anyway with the placeholders.");
        assert!(picture_gate_text(&[1, 3, 5]).starts_with("Questions 1, 3 and 5 still need"));
        assert!(picture_gate_text(&[3]).starts_with("Question 3 still needs a picture."));
        assert_eq!(picture_gate_text(&[]), "");
    }

    #[test]
    fn version_shuffle_stays_within_sections() {
        let q = |id: &str| Placed { item: it(id), q: mcq(id, &[("a", true, false), ("b", false, false)]) };
        let mut placed: Vec<Placed> = (0..8).map(|k| q(&format!("q{k}"))).collect();
        placed[3].item.section = sec("B");
        for seed in 0..10u64 {
            let out = shuffle_placed_in_sections(placed.clone(), &mut lcg(seed));
            let first: Vec<&str> = out[..3].iter().map(|p| p.q.id.as_str()).collect();
            assert!(first.iter().all(|id| ["q0", "q1", "q2"].contains(id)));
            assert_eq!(out[3].item.section.as_ref().map(|s| s.title.as_str()), Some("B"));
            assert!(out.iter().enumerate().all(|(k, p)| (k == 3) == p.item.section.is_some()));
        }
    }

    #[test]
    fn work_space_and_margins_layout_math() {
        // Condensed lines are closer than Standard ones; the preview pitch follows the style.
        let bp = 11.0 * PT;
        assert!(line_pitch_px(bp, PaperStyle::Condensed) < line_pitch_px(bp, PaperStyle::Standard));
        // Content on page 2 starts below the top margin and the repeated header.
        let (_, ph) = PaperStyle::ExamBoard.page_pt();
        assert_eq!(page_content_top(1.0, ph * PT, 40.0, 12.0), ph * PT + 52.0);
        assert_eq!(fmt_inches(19.05), "0.75");
        assert_eq!(fmt_inches(25.4), "1");
        assert_eq!(part_line_styled(0, "NH3", PaperStyle::ExamBoard), "(a) NH3");
        assert_eq!(split_part_label_styled(1, "CO2", PaperStyle::ExamBoard).0, "(b)");
        assert_eq!(styled_choice(PaperStyle::Classic, 2, "O"), "c. O");
        assert_eq!(styled_choice(PaperStyle::ExamBoard, 0, "Ge"), "A Ge");
    }

    #[test]
    fn export_preview_pages_are_the_pdfs_pages() {
        let mk = |n: u32| DocQ { number: n, text: vec![format!("Question {n} (with parens)")], rule_lines: 3, ..Default::default() };
        let doc = ExamDoc { title: "T".into(), body_pt: 11.0, heading_pt: 16.0, margin_mm: 18.0, questions: (1..=25).map(mk).collect(), ..Default::default() };
        let pv = crate::export::preview_pages(&doc).unwrap();
        let pdf = crate::export::render_pdf(&doc).unwrap();
        assert_eq!(pv.pages.len(), String::from_utf8_lossy(&pdf).matches("/Type /Page ").count());
        assert_eq!((pv.page_w, pv.page_h), (612.0, 792.0));
        let toks = pdf_tokens(&pv.pages[0]);
        let strs: Vec<String> = toks.iter().filter_map(|t| if let PdfTok::Str(b) = t { Some(winansi_text(b)) } else { None }).collect();
        assert!(strs.iter().any(|s| s == "Question 1 (with parens)"), "escaped parens decode: {strs:?}");
        assert!(toks.contains(&PdfTok::Op("Tj".into())) && toks.contains(&PdfTok::Name("F1".into())));
        assert_eq!(pdf_tokens(b"[0.8 2.2] 0 d"), vec![PdfTok::Arr(vec![0.8, 2.2]), PdfTok::Num(0.0), PdfTok::Op("d".into())]);
    }

    /// The export preview and the card thumbnails read the CID (F5..F8) and fallback (F9)
    /// strings back to the characters they print.
    #[test]
    fn preview_reads_cid_and_fallback_text() {
        let stem = "Find \u{3c0} \u{2212} \u{221a}2 \u{2264} *\u{3b8}* so x \u{2208} A \u{21d2} done";
        let doc = ExamDoc { title: "T".into(), body_pt: 11.0, heading_pt: 16.0, margin_mm: 18.0, questions: vec![DocQ { number: 1, text: vec![stem.into()], ..Default::default() }], ..Default::default() };
        let pv = crate::export::preview_pages(&doc).unwrap();
        assert_eq!(pv.fallback, vec!['\u{2208}', '\u{21d2}']);
        let mut font = String::new();
        let mut text = String::new();
        let toks = pdf_tokens(&pv.pages[0]);
        for (k, t) in toks.iter().enumerate() {
            match t {
                PdfTok::Name(n) if matches!(toks.get(k + 2), Some(PdfTok::Op(o)) if o == "Tf") => font = n.clone(),
                PdfTok::Op(o) if o == "Tj" => {
                    text.push_str(&shown_text(&toks[k - 1], &font, &pv.fallback).unwrap());
                }
                _ => {}
            }
        }
        for c in "\u{3c0}\u{2212}\u{221a}\u{2264}\u{3b8}\u{2208}\u{21d2}".chars() {
            assert!(text.contains(c), "{c} in {text}");
        }
        assert!(!text.contains('?'), "{text}");
        assert!(toks.iter().any(|t| matches!(t, PdfTok::Name(n) if n == "F7")), "italic theta via the italic CID font");
        assert_eq!(base_font("F7"), "F3");
        let (svg, _) = page_svg(pv.page_w, pv.page_h, &pv.pages[0], &pv.fallback);
        assert!(svg.contains('\u{3c0}') && svg.contains('\u{21d2}') && svg.contains("font-style=\"italic\""));
    }

    #[test]
    fn working_copy_signature_ignores_the_edit_time() {
        let mut a = model::Exam { id: "e".into(), ..Default::default() };
        let mut b = a.clone();
        b.modified = 99;
        assert_eq!(exam_sig(&a), exam_sig(&b));
        a.items.push(it("q1"));
        assert_ne!(exam_sig(&a), exam_sig(&b));
    }

    #[test]
    fn same_items_sees_section_work_and_order_changes() {
        let a = sample();
        let mut b = sample();
        assert!(same_items(&a, &b));
        b[0].work_lines = 2;
        assert!(!same_items(&a, &b));
        let mut c = sample();
        c[1].answer_order = vec![1, 0];
        assert!(!same_items(&a, &c));
        let mut d = sample();
        d[2].section = None;
        assert!(!same_items(&a, &d));
    }
}
