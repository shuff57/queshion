//! Exam list, exam working-copy editor, export dialog and export history
//! (exam worker file; public signatures frozen in NOTES.md).
//!
//! - [`App::draw_exams`]: exams list (card grid) with New / Open / Rename / Delete
//!   and an empty state.
//! - [`App::draw_exam_edit`]: the printable working copy. Toolbar rows (breadcrumb,
//!   editable name, File/Edit/Format menus, Saved status, Export History, Export),
//!   then a page preview (left) and the open Question Bank panel (right) split by a
//!   draggable divider. Questions are added to the working copy from the panel,
//!   reorder by grip-drag (floating ghost + drop indicator) or the up/down buttons,
//!   remove with `×`; every item-list change is undo/redo-snapshotted.
//! - [`App::draw_export_dialog`] / [`App::draw_history`]: PDF/DOCX export with
//!   shuffled named versions (deterministic LCG, no `rand`), answer-key docs, and
//!   regeneration from the stored `.doc.json` snapshots.
//!
//! Local UI state lives in egui temp memory keyed by stable `egui::Id`s; persistent
//! state is `App` / `store` (saved exclusively through `store::save_*`).
//!
//! Tier gate: I am the delegated cheaper-tier builder; big inline writes are mine
//! and I announce that here once.

use eframe::egui;
use crate::model::{self, ExamItem, Id, Kind, PartKind, QKind};
use crate::{bankui, screens, store, theme};
type DocImg = crate::layout::DocImage;
type DocQ = crate::layout::DocQ;
type ExamDoc = crate::layout::ExamDoc;

/// Page-paper color for the preview article (the shared theme's CARD is the
/// card-list tone; "paper" is a touch warmer, matching the site).
const PAPER: egui::Color32 = egui::Color32::from_rgb(0xff, 0xfd, 0xf8);

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
    confirm_delete: Option<Id>,
    rename: Option<(Id, String)>,
}

/// Drag state for the page reorder: which page index started the drag + the first
/// stem line (for the floating ghost card).
#[derive(Clone)]
struct DragState {
    from: usize,
    stem: String,
}

#[derive(Clone, Default)]
struct EditUi {
    search: String,
    topics: Vec<String>,
    diff: u8, // 0 any, 1 low, 2 medium, 3 high
    sort_newest: bool,
    /// Width of the right (bank) pane; default mirrors the spec's SidePanel width.
    splitter: f32,
    drag: Option<DragState>,
    /// Screen-space rects of the page item cards as laid out last frame.
    rows: Vec<(usize, egui::Rect)>,
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

/// Accent (`theme::ACCENT`) button; hover uses `theme::ACCENT_HOVER`; disabled uses
/// `theme::ACCENT_FADED`. Hand-painted so the colors are exact.
fn accent_button_opt(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let font = egui::FontId::proportional(13.5);
    let gsize = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), egui::Color32::WHITE)
        .size();
    let size = gsize + egui::vec2(20.0, 12.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let mut fill = theme::ACCENT;
    if resp.hovered() {
        fill = theme::ACCENT_HOVER;
    }
    if !enabled {
        fill = theme::ACCENT_FADED;
    }
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        if enabled { egui::Color32::WHITE } else { theme::CREAM },
    );
    resp
}

fn accent_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    accent_button_opt(ui, label, true)
}

/// Small icon-ish button for hover toolbars.
fn icon_btn(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label).size(11.5).color(theme::INK),
        )
        .fill(theme::CARD)
        .corner_radius(4.0),
    )
}

/// A toggle chip; selected chips fill with the accent color.
fn chip(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let font = egui::FontId::proportional(11.0);
    let gsize = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), egui::Color32::WHITE)
        .size();
    let size = gsize + egui::vec2(14.0, 9.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if selected { theme::ACCENT } else { theme::CARD };
    ui.painter().rect_filled(rect, 9.0, fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        if selected { egui::Color32::WHITE } else { theme::INK },
    );
    resp
}

fn card_frame(fill: egui::Color32) -> egui::Frame {
    egui::Frame::default()
        .fill(fill)
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(10))
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
}

fn muted(rt: impl Into<egui::RichText>) -> egui::RichText {
    rt.into().color(theme::MUTED)
}

/// One printed-looking ruled line.
fn ruled_line(ui: &mut egui::Ui, width_hint: f32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_hint.min(ui.available_width()), 15.0),
        egui::Sense::hover(),
    );
    let y = rect.bottom() - 3.0;
    ui.painter().line_segment(
        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
        egui::Stroke::new(0.8, theme::RULE),
    );
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

fn roman(i: usize) -> String {
    const R: [&str; 12] = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii",
    ];
    R.get(i)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("({})", i + 1))
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
        heading_pt: 16.0,
        body_pt: 11.0,
        margin_mm: 18.0,
        ruled_blanks: 0,
        modified: now(),
    };
    app.store.exams.push(exam);
    app.store.save_exams();
    app.undo.clear();
    app.redo.clear();
    id
}

impl crate::App {
    pub fn draw_exams(&mut self, ctx: &egui::Context) {
        let mut st: ListUi = mem_load(ctx, list_id());

        screens::shell_frame(self, ctx, true, |app, ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(
                    theme::strong("Exams").size(22.0).color(theme::INK),
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("New").clicked() {
                        let id = new_exam(app);
                        app.exam = Some(id.clone());
                        app.screen = crate::Screen::ExamEdit(id);
                    }
                });
            });
            ui.add_space(2.0);
            ui.add(
                egui::TextEdit::singleline(&mut st.search)
                    .hint_text("Search Exams")
                    .desired_width(ui.available_width().min(320.0)),
            );
            ui.add_space(2.0);

            let mut exams: Vec<(i64, Id, String, usize)> = app
                .store
                .exams
                .iter()
                .filter(|e| {
                    let needle = st.search.trim().to_lowercase();
                    needle.is_empty() || e.name.to_lowercase().contains(&needle)
                })
                .map(|e| (e.modified, e.id.clone(), e.name.clone(), e.items.len()))
                .collect();
            exams.sort_by(|a, b| b.0.cmp(&a.0));
            let count = exams.len();
            ui.add(egui::Label::new(muted(format!(
                "{count} Exam{}",
                if count == 1 { "" } else { "s" }
            ))));
            ui.add_space(8.0);

            if count == 0 {
                card_frame(theme::CARD).show(ui, |ui| {
                    ui.allocate_ui(egui::vec2(ui.available_width(), 170.0), |ui| {
                        ui.add_space(30.0);
                        ui.vertical_centered(|ui| {
                            ui.add(egui::Label::new(
                                theme::strong("Create your first Exam")
                                    .size(16.0)
                                    .color(theme::INK),
                            ));
                            ui.add_space(4.0);
                            ui.add(egui::Label::new(muted(
                                "Pull questions into a printable working copy.",
                            )));
                            ui.add_space(10.0);
                            if accent_button(ui, "Create your first Exam").clicked() {
                                let id = new_exam(app);
                                app.exam = Some(id.clone());
                                app.screen = crate::Screen::ExamEdit(id);
                            }
                        });
                        ui.add_space(30.0);
                    });
                });
            } else {
                egui::Grid::new("exam_grid")
                    .spacing([10.0, 10.0])
                    .min_col_width(250.0)
                    .max_col_width(380.0)
                    .show(ui, |ui| {
                        for (_, id, _, _) in &exams {
                            draw_exam_card(app, ui, id, &mut st);
                            ui.end_row();
                        }
                    });
            }
        });

        // Delete-confirm modal (raised from a card's "…" menu above).
        if let Some(id) = st.confirm_delete.clone() {
            match self.exams_name(&id) {
                None => st.confirm_delete = None,
                Some(name) => {
                    let mut open = true;
                    egui::Window::new(theme::strong("Delete Exam"))
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
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                theme::strong("Delete")
                                                    .color(egui::Color32::WHITE),
                                            )
                                            .fill(theme::BAD),
                                        )
                                        .clicked()
                                    {
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
                                    if ui.button("Cancel").clicked() {
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
                    egui::Window::new(theme::strong("Rename Exam"))
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
                                    if ui
                                        .add(egui::Button::new(theme::strong("Rename")))
                                        .clicked()
                                    {
                                        done = true;
                                    }
                                    if ui.button("Cancel").clicked() {
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

fn draw_exam_card(app: &mut crate::App, ui: &mut egui::Ui, id: &Id, st: &mut ListUi) {
    let Some((name, nq, date)) = app
        .store
        .exam(id)
        .map(|e| (e.name.clone(), e.items.len(), bankui::fmt_date(e.modified)))
    else {
        return;
    };
    card_frame(theme::CARD).show(ui, |ui| {
        // Name row doubles as the open affordance.
        if ui
            .add(
                egui::Button::new(theme::strong(name.clone()).size(15.0).color(theme::INK))
                    .fill(egui::Color32::TRANSPARENT)
                    .min_size(egui::vec2(ui.available_width(), 22.0)),
            )
            .on_hover_text("Open")
            .clicked()
        {
            app.screen = crate::Screen::ExamEdit(id.clone());
            app.exam = Some(id.clone());
        }
        ui.add(egui::Label::new(muted(format!(
            "{nq} question{}",
            if nq == 1 { "" } else { "s" }
        ))));
        ui.add(egui::Label::new(muted(date)));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if accent_button(ui, "Open").clicked() {
                app.screen = crate::Screen::ExamEdit(id.clone());
                app.exam = Some(id.clone());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("…", |ui| {
                    if ui.button("Rename").clicked() {
                        st.rename = Some((id.clone(), name.clone()));
                    }
                    ui.add_space(4.0);
                    if ui
                        .add(egui::Button::new(egui::RichText::new("Delete").color(theme::BAD)))
                        .clicked()
                    {
                        st.confirm_delete = Some(id.clone());
                    }
                });
            });
        });
    });
}

// ---------------------------------------------------------------------------
// Exam editor screen
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
        if st.splitter <= 0.0 {
            st.splitter = 320.0;
        }

        screens::shell_frame(self, ctx, true, |app, ui| {
            draw_editor_chrome(app, ui, ctx, &exam_id, &mut st);
        });

        // File-menu Delete (confirm) modal on `self`.
        if st.confirm_delete {
            let name = self.exams_name(&exam_id).unwrap_or_default();
            let mut open = true;
            egui::Window::new(theme::strong("Delete Exam"))
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
                        if ui
                            .add(
                                egui::Button::new(
                                    theme::strong("Delete")
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(theme::BAD),
                            )
                            .clicked()
                        {
                            self.store.exams.retain(|e| e.id != exam_id);
                            self.store.save_exams();
                            self.exam = None;
                            self.screen = crate::Screen::Exams;
                            self.undo.clear();
                            self.redo.clear();
                            screens::toast(self, "Exam deleted", false);
                            st.confirm_delete = false;
                        }
                        if ui.button("Cancel").clicked() {
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
            egui::Window::new(theme::strong("Save As…"))
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
                        if ui.add(egui::Button::new(theme::strong("Save"))).clicked() {
                            done = true;
                        }
                        if ui.button("Cancel").clicked() {
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

fn crumb_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(muted(egui::RichText::new(label).size(12.5)))
            .fill(egui::Color32::TRANSPARENT),
    )
}

fn draw_editor_chrome(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
) {
    ui.add_space(5.0);
    // Row 1: breadcrumb | Save status.
    ui.horizontal(|ui| {
        if crumb_button(ui, "Home").clicked() {
            app.screen = crate::Screen::Home;
        }
        ui.add(egui::Label::new(muted(" / ")));
        if crumb_button(ui, "Exams").clicked() {
            app.screen = crate::Screen::Exams;
        }
        if let Some(exam) = app.store.exam(exam_id) {
            let shown = if exam.name.is_empty() {
                "Untitled Exam".to_string()
            } else {
                exam.name.clone()
            };
            ui.add(egui::Label::new(muted(
                egui::RichText::new(format!(" / {shown}")).size(12.5),
            )));
        }
    });

    // Name edit + right controls on their own line keeps the widths sane.
    ui.horizontal(|ui| {
        let mut name = app.store.exam(exam_id).map(|e| e.name.clone()).unwrap_or_default();
        let before = name.clone();
        ui.add(
            egui::TextEdit::singleline(&mut name)
                .hint_text("Untitled Exam")
                .font(egui::FontId::proportional(18.0))
                .text_color(theme::INK)
                .frame(egui::Frame::NONE)
                .desired_width(ui.available_width() * 0.45),
        );
        if name != before {
            if let Some(e) = app.store.exam_mut(exam_id) {
                e.name = name;
                e.modified = now();
            }
            app.store.save_exams();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if accent_button(ui, "Export").clicked() {
                app.export_open = true;
            }
            if ui.button("Export History").clicked() {
                app.history_open = true;
            }
            ui.add(egui::Label::new(muted("Saved")));
        });
    });

    // Row 3 (menus): File / Edit / Format.
    ui.horizontal(|ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Open Exam").clicked() {
                app.screen = crate::Screen::Exams;
            }
            if ui.button("Save As…").clicked() {
                st.save_as_open = true;
                st.save_as_name.clear();
            }
            if ui.button("Export").clicked() {
                app.export_open = true;
            }
            if ui.button("Export History").clicked() {
                app.history_open = true;
            }
            ui.add_space(4.0);
            if ui
                .add(egui::Button::new(egui::RichText::new("Delete Exam").color(theme::BAD)))
                .clicked()
            {
                st.confirm_delete = true;
            }
        });
        ui.menu_button("Edit", |ui| {
            if ui
                .add_enabled(
                    !app.undo.is_empty(),
                    egui::Button::new(egui::RichText::new("Undo")),
                )
                .clicked()
            {
                do_undo(app, exam_id);
            }
            if ui
                .add_enabled(
                    !app.redo.is_empty(),
                    egui::Button::new(egui::RichText::new("Redo")),
                )
                .clicked()
            {
                do_redo(app, exam_id);
            }
        });
        ui.menu_button("Format", |ui| {
            let mut changed = false;
            if let Some(ex) = app.store.exam_mut(exam_id) {
                let r1 = ui.add(egui::Slider::new(&mut ex.heading_pt, 10.0..=28.0).text("Heading size"));
                let r2 = ui.add(egui::Slider::new(&mut ex.body_pt, 8.0..=14.0).text("Text size"));
                let r3 = ui.add(egui::Slider::new(&mut ex.margin_mm, 10.0..=40.0).text("Margins (mm)"));
                let mut blanks = ex.ruled_blanks as i32;
                let r4 = ui.add(egui::Slider::new(&mut blanks, 0..=6).text("Answer blanks"));
                ex.ruled_blanks = blanks.clamp(0, 6) as u8;
                changed = r1.changed() || r2.changed() || r3.changed() || r4.changed();
            }
            if changed {
                if let Some(ex) = app.store.exam_mut(exam_id) {
                    ex.modified = now();
                }
                app.store.save_exams();
            }
        });
    });
    ui.add_space(4.0);

    // Body: page (left) | draggable separator | open bank panel (right).
    let total = ui.available_width();
    let height = ui.available_height();
    let mut right = st.splitter.clamp(240.0, 640.0);
    let left = (total - right - 8.0).max(340.0);
    right = (total - left - 8.0).max(200.0);

    ui.horizontal(|ui| {
        // LEFT: page preview.
        let page_h = height;
        ui.allocate_ui(egui::vec2(left, page_h), |ui| {
            egui::Frame::default()
                .fill(PAPER)
                .corner_radius(10.0)
                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    draw_page(app, ui, ctx, exam_id, st);
                });
        });
        // Separator.
        let (sep, sep_resp) =
            ui.allocate_exact_size(egui::vec2(8.0, height), egui::Sense::drag());
        let hovered = sep_resp.hovered() || sep_resp.dragged();
        let cx = sep.center().x;
        ui.painter().line_segment(
            [
                egui::pos2(cx, sep.top() + 8.0),
                egui::pos2(cx, sep.bottom() - 8.0),
            ],
            egui::Stroke::new(
                if hovered { 2.5 } else { 1.5 },
                if hovered { theme::BROWN } else { theme::BORDER },
            ),
        );
        if sep_resp.dragged() {
            st.splitter -= sep_resp.drag_delta().x;
        }
        // RIGHT: open bank panel.
        ui.allocate_ui(egui::vec2(right, height), |ui| {
            egui::Frame::default()
                .fill(theme::CREAM)
                .corner_radius(10.0)
                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    draw_bank_panel(app, ui, ctx, exam_id, st);
                });
        });
    });
}

// ---------------------------------------------------------------------------
// Page preview (left pane)
// ---------------------------------------------------------------------------

fn draw_page(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
) {
    st.rows.clear();
    egui::ScrollArea::vertical()
        .id_salt("page_scroll")
        .show(ui, |ui| {
            let aw = ui.available_width().max(200.0);
            let art_w = 620.0_f32.min(aw - 24.0).max(240.0);
            let inset = ((aw - art_w) / 2.0).max(4.0);

            let items: Vec<ExamItem> = app
                .store
                .exam(exam_id)
                .map(|e| e.items.clone())
                .unwrap_or_default();

            ui.horizontal(|ui| {
                ui.add_space(inset);
                ui.vertical(|ui| {
                    ui.set_max_width(art_w);
                    draw_page_article(app, ui, ctx, exam_id, st, &items, art_w);
                });
            });

            // Post-pass: a released pointer off the grip still means "drop".
            // A grip's own drag_stopped cleared `st.drag` already.
            if let Some(drag) = st.drag.clone() {
                let held = ctx.input(|i| i.pointer.primary_down());
                let released = ctx.input(|i| i.pointer.any_released());
                if released || !held {
                    let y = ctx
                        .pointer_interact_pos()
                        .or_else(|| ctx.pointer_hover_pos())
                        .map(|p| p.y)
                        .unwrap_or(-1.0);
                    if y >= 0.0 {
                        finish_drag(app, exam_id, st, &drag, y);
                    } else {
                        st.drag = None;
                    }
                } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    st.drag = None;
                }
            }

            // Ghost + insertion indicator while dragging.
            if let Some(drag) = st.drag.clone() {
                if let Some(pos) =
                    ctx.pointer_hover_pos().or_else(|| ctx.pointer_interact_pos())
                {
                    let k = drop_index(&st.rows, pos.y, drag.from);
                    if let Some(yl) = insertion_y(&st.rows, k, drag.from) {
                        ui.painter().line_segment(
                            [
                                egui::pos2(ui.clip_rect().left(), yl),
                                egui::pos2(ui.clip_rect().right(), yl),
                            ],
                            egui::Stroke::new(2.0, theme::ACCENT),
                        );
                    }
                    let font = egui::FontId::proportional(12.0);
                    let g_rect = egui::Rect::from_min_size(
                        egui::pos2(pos.x + 12.0, pos.y - 14.0),
                        egui::vec2(320.0, 30.0),
                    );
                    let p = ui
                        .painter()
                        .clone()
                        .with_layer_id(egui::LayerId::new(
                            egui::Order::Tooltip,
                            egui::Id::NULL.with("examui_ghost"),
                        ));
                    p.rect_filled(g_rect, 8.0, theme::CARD);
                    p.rect_stroke(g_rect, 8.0, egui::Stroke::new(1.2, theme::ACCENT), egui::StrokeKind::Inside);
                    p.text(
                        egui::pos2(g_rect.left() + 8.0, g_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        truncate_str(&drag.stem, 46),
                        font,
                        theme::INK,
                    );
                }
            }
        });
}

fn draw_page_article(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
    items: &[ExamItem],
    art_w: f32,
) {
    let Some(exam) = app.store.exam(exam_id) else { return };
    let exam = exam.clone();
    let title = exam
        .title
        .clone()
        .unwrap_or_else(|| exam.name.clone());
    let body_pt = exam.body_pt;

    ui.add_space(2.0);
    ui.add(
        egui::Label::new(
            theme::serif_strong(title)
                .size(exam.heading_pt)
                .color(theme::INK),
        )
        .wrap(),
    );
    ui.add_space(2.0);
    // The printed header line is editable directly on the page.
    let mut header = exam.header.clone();
    let before = header.clone();
    ui.add(
        egui::TextEdit::singleline(&mut header)
            .font(egui::FontId::proportional(body_pt * 0.92))
            .text_color(theme::MUTED)
            .frame(egui::Frame::NONE)
            .desired_width(art_w),
    );
    if header != before {
        if let Some(e) = app.store.exam_mut(exam_id) {
            e.header = header;
            e.modified = now();
        }
        app.store.save_exams();
    }

    // Ruled blank lines after the title area.
    for _ in 0..exam.ruled_blanks {
        ruled_line(ui, art_w);
    }
    ui.add_space(10.0);

    if items.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.add(egui::Label::new(muted(
                "Drag or add a Question from an open Question Bank",
            )));
        });
        ui.add_space(40.0);
        return;
    }

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

    for (i, item) in items.iter().enumerate() {
        let q = app.store.question(&item.bank_id, &item.question_id).cloned();
        let number = numbers.get(i).copied().unwrap_or(i as u32 + 1);
        let rect = draw_item(app, ui, ctx, exam_id, st, i, number, q.as_ref(), art_w);
        st.rows.push((i, rect));
        ui.add_space(8.0);
    }

    // Answer-section grid for MC/TF items ("N. ______" cells, 4 per print row).
    ui.add_space(16.0);
    ui.add(
        egui::Label::new(
            theme::serif_strong("Answer Section")
                .size(body_pt + 1.0)
                .color(theme::INK),
        )
        .wrap(),
    );
    ui.add_space(6.0);
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
    let cell_w = ((art_w - 40.0) / 4.0).min(130.0);
    for chunk in cells.chunks(4) {
        ui.horizontal(|ui| {
            for n in chunk {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(cell_w, body_pt * 2.1),
                    egui::Sense::hover(),
                );
                ui.painter().text(
                    egui::pos2(rect.left(), rect.center().y - body_pt * 0.3),
                    egui::Align2::LEFT_CENTER,
                    format!("{n}. ______"),
                    egui::FontId::proportional(body_pt),
                    theme::INK,
                );
            }
        });
    }
}

/// One question on the page. Returns the card's screen rect (for drop targeting).
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
    art_w: f32,
) -> egui::Rect {
    let body_pt = 11.0_f32;
    let frame = egui::Frame::default().show(ui, |ui| {
        ui.set_max_width(art_w);
        let Some(q) = q else {
            ui.add(
                egui::Label::new(muted(
                    egui::RichText::new(format!(
                        "{number}. (question missing from its bank)"
                    ))
                    .size(body_pt),
                )),
            );
            return;
        };

        let dragging = matches!(&st.drag, Some(DragState { from, .. }) if *from == i);

        // Header line: grip | number | stem | points badge | toolbar.
        ui.horizontal(|ui| {
            let (grect, gresp) = ui.allocate_exact_size(
                egui::vec2(14.0, 18.0),
                egui::Sense::click_and_drag(),
            );
            let grip_color = if dragging || gresp.hovered() {
                theme::BROWN
            } else {
                theme::MUTED
            };
            {
                let p = ui.painter();
                let gx = grect.center().x;
                let cy = grect.center().y;
                for yoff in [-4.0, 0.0, 4.0] {
                    p.line_segment(
                        [
                            egui::pos2(gx - 5.0, cy + yoff),
                            egui::pos2(gx + 5.0, cy + yoff),
                        ],
                        egui::Stroke::new(1.4, grip_color),
                    );
                }
            }
            if gresp.drag_started() {
                st.drag = Some(DragState { from: i, stem: q.stem.clone() });
            }
            if gresp.drag_stopped() {
                if let Some(drag) = st.drag.clone() {
                    match gresp.interact_pointer_pos() {
                        Some(pos) => finish_drag(app, exam_id, st, &drag, pos.y),
                        None => st.drag = None, // unknown drop position: cancel, never guess
                    }
                }
            }
            if gresp.dragged() {
                ctx.request_repaint();
            }

            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("{number}."))
                        .size(12.0)
                        .color(theme::BROWN),
                ),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(q.stem.replace('\n', " — "))
                        .size(body_pt)
                        .color(theme::INK),
                )
                .wrap(),
            );
            if let Some(p) = q.points {
                ui.add(
                    egui::Label::new(muted(
                        egui::RichText::new(format!(
                            " ({} point{})",
                            fmt_number(p),
                            if p == 1.0 { "" } else { "s" }
                        ))
                        .size(10.5),
                    )),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_btn(ui, "×").clicked() {
                    remove_item(app, exam_id, i);
                }
                if icon_btn(ui, "↓").clicked() {
                    shift_item(app, exam_id, i, 1);
                }
                if icon_btn(ui, "↑").clicked() {
                    shift_item(app, exam_id, i, -1);
                }
            });
        });

        // Body per kind.
        match &q.kind {
            Kind::Mc { answers } => {
                draw_image(app, ui, ctx, q.image.as_ref());
                for (ci, a) in answers.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("{}. ", letter(ci)))
                                    .size(body_pt - 1.0)
                                    .color(theme::BROWN),
                            ),
                        );
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(a.text.clone())
                                    .size(body_pt)
                                    .color(theme::INK),
                            ),
                        );
                    });
                }
            }
            Kind::Tf { .. } => {
                draw_image(app, ui, ctx, q.image.as_ref());
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("True / False")
                            .size(body_pt - 1.0)
                            .color(theme::MUTED),
                    ),
                );
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
                        cols[0].horizontal(|ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "{}. ",
                                        number + li as u32
                                    ))
                                    .size(12.0)
                                    .color(theme::BROWN),
                                ),
                            );
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(p.left.clone())
                                        .size(body_pt)
                                        .color(theme::INK),
                                ),
                            );
                        });
                    }
                    for (ri, r) in rights.iter().enumerate() {
                        cols[1].horizontal(|ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!("{}. {}", letter(ri), r))
                                        .size(body_pt)
                                        .color(theme::INK),
                                ),
                            );
                        });
                    }
                });
            }
            Kind::Blank { accept } => {
                draw_image(app, ui, ctx, q.image.as_ref());
                let markers = q.stem.matches("___").count().max(1);
                for _ in 0..markers {
                    ruled_line(ui, art_w - 40.0);
                }
                let _ = accept;
            }
            Kind::Short { .. } => {
                draw_image(app, ui, ctx, q.image.as_ref());
                for _ in 0..3 {
                    ruled_line(ui, art_w - 40.0);
                }
            }
            Kind::Multipart { parts } => {
                draw_image(app, ui, ctx, q.image.as_ref());
                let mut rules = 0u32;
                for (pi, part) in parts.iter().enumerate() {
                    match &part.kind {
                        PartKind::Mc { answers } => {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("{}. ", roman(pi)))
                                            .size(12.0)
                                            .color(theme::BROWN),
                                    ),
                                );
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(part.stem.clone())
                                            .size(body_pt)
                                            .color(theme::INK),
                                    ),
                                );
                            });
                            for (ci, a) in answers.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(format!("{}. ", letter(ci)))
                                                .size(body_pt - 1.0)
                                                .color(theme::MUTED),
                                        ),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(a.text.clone())
                                                .size(body_pt)
                                                .color(theme::INK),
                                        ),
                                    );
                                });
                            }
                        }
                        PartKind::Tf { .. } => {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "{}. {} (True / False)",
                                        roman(pi),
                                        part.stem
                                    ))
                                    .size(body_pt)
                                    .color(theme::INK),
                                ),
                            );
                        }
                        PartKind::Short { .. } => {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "{}. {}",
                                        roman(pi),
                                        part.stem
                                    ))
                                    .size(body_pt)
                                    .color(theme::INK),
                                ),
                            );
                            rules += 2;
                        }
                    }
                }
                for _ in 0..rules.max(2) {
                    ruled_line(ui, art_w - 60.0);
                }
            }
        }
    });
    frame.response.rect
}

fn remove_item(app: &mut crate::App, exam_id: &Id, i: usize) {
    apply_items_fn(app, exam_id, |mut items| {
        if i < items.len() {
            items.remove(i);
        }
        items
    });
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

fn push_undo(app: &mut crate::App, exam_id: &Id) {
    if let Some(ex) = app.store.exam(exam_id) {
        app.undo.push(ex.items.clone());
        app.redo.clear();
    }
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

/// Finish a page drag: move the item from `drag.from` to where the pointer sits.
/// Called only after a real drag; a same-position drop is a no-op.
fn finish_drag(
    app: &mut crate::App,
    exam_id: &Id,
    st: &mut EditUi,
    drag: &DragState,
    y: f32,
) {
    let k = drop_index(&st.rows, y, drag.from);
    st.drag = None;
    apply_items_fn(app, exam_id, |items| {
        let mut next = items;
        if drag.from >= next.len() {
            return next;
        }
        let moved = next.remove(drag.from);
        let k = k.min(next.len());
        next.insert(k, moved);
        next
    });
}

/// Drop index (into the list WITHOUT row `from`) for pointer y.
fn drop_index(rows: &[(usize, egui::Rect)], y: f32, from: usize) -> usize {
    let mut count = 0usize;
    for (_, r) in rows {
        if r.center().y < y {
            count += 1;
        }
    }
    if from < count {
        count - 1
    } else {
        count
    }
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

// ---------------------------------------------------------------------------
// Right pane: question bank panel
// ---------------------------------------------------------------------------

fn draw_bank_panel(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    exam_id: &Id,
    st: &mut EditUi,
) {
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(
                theme::serif_strong("Question Bank")
                    .size(15.0)
                    .color(theme::INK),
            )
            .wrap(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.panel_bank.is_some() && icon_btn(ui, "×").clicked() {
                app.panel_bank = None;
            }
        });
    });
    ui.add_space(4.0);
    egui::ComboBox::from_id_salt("exam_panel_bank")
        .selected_text(
            app.panel_bank
                .as_ref()
                .and_then(|id| app.store.bank(id))
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "Open bank…".to_string()),
        )
        .width(ui.available_width().min(280.0))
        .show_ui(ui, |ui| {
            for b in app.store.banks.clone() {
                ui.selectable_value(&mut app.panel_bank, Some(b.id.clone()), b.name.clone());
            }
        });

    let Some(bank_id) = app.panel_bank.clone() else {
        ui.add_space(12.0);
        ui.add(egui::Label::new(muted(
            "Choose a bank above, or open one from the Banks screen.",
        )));
        return;
    };
    let Some(bank) = app.store.bank(&bank_id).cloned() else {
        app.panel_bank = None;
        return;
    };

    ui.add_space(4.0);
    ui.add(
        egui::TextEdit::singleline(&mut st.search)
            .hint_text("Search question stems")
            .desired_width(ui.available_width().min(280.0)),
    );
    ui.add_space(4.0);

    // Topic chips.
    let mut topics: Vec<String> = bank
        .questions
        .iter()
        .flat_map(|q| q.topics.clone())
        .filter(|t| !t.trim().is_empty())
        .collect();
    topics.sort();
    topics.dedup();
    if !topics.is_empty() {
        ui.add(egui::Label::new(muted(
            egui::RichText::new("Topics").size(10.5),
        )));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
            for t in &topics {
                let sel = st.topics.contains(t);
                if chip(ui, t, sel).clicked() {
                    if sel {
                        st.topics.retain(|x| x != t);
                    } else {
                        st.topics.push(t.clone());
                    }
                }
            }
        });
        ui.add_space(2.0);
    }

    // Difficulty filter + sort toggle.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
        for (di, label) in [(0u8, "All"), (1, "Low"), (2, "Medium"), (3, "High")] {
            if chip(ui, label, st.diff == di).clicked() {
                st.diff = di;
            }
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui
            .button(if st.sort_newest { "Newest first" } else { "Oldest first" })
            .clicked()
        {
            st.sort_newest = !st.sort_newest;
        }
    });
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .id_salt("bank_panel_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let needle = st.search.trim().to_lowercase();
            let mut qs: Vec<model::Question> = bank
                .questions
                .iter()
                .filter(|q| needle.is_empty() || q.stem.to_lowercase().contains(&needle))
                .filter(|q| {
                    st.topics.is_empty() || st.topics.iter().any(|t| q.topics.contains(t))
                })
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

            if qs.is_empty() {
                ui.add(egui::Label::new(muted("No questions match.")));
                return;
            }
            for q in &qs {
                draw_bank_card(app, ui, ctx, &bank_id, exam_id, q);
                ui.add_space(6.0);
            }
        });
}

fn draw_bank_card(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    bank_id: &Id,
    exam_id: &Id,
    q: &model::Question,
) {
    card_frame(theme::CARD).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add(
                egui::Label::new(
                    theme::strong(q.kind.qkind().label())
                        .size(10.5)
                        .color(theme::BROWN),
                ),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if accent_button(ui, "Add").clicked() {
                    push_undo(app, exam_id);
                    if let Some(ex) = app.store.exam_mut(exam_id) {
                        ex.items.push(ExamItem {
                            bank_id: bank_id.clone(),
                            question_id: q.id.clone(),
                        });
                        ex.modified = now();
                    }
                    app.store.save_exams();
                }
            });
        });
        ui.add(
            egui::Label::new(muted(
                egui::RichText::new(format!(
                    "{}{}{}",
                    match q.difficulty {
                        Some(model::Difficulty::Low) => "low · ",
                        Some(model::Difficulty::Medium) => "medium · ",
                        Some(model::Difficulty::High) => "high · ",
                        None => "",
                    },
                    match q.points {
                        Some(p) => format!("{} pt · ", fmt_number(p)),
                        None => String::new(),
                    },
                    answers_preview(q),
                ))
                .size(10.5),
            )),
        );
        if let Some(img) = &q.image {
            if let Some(tex) = app.image_texture(ctx, &img.file) {
                ui.add(
                    egui::Image::new(&tex)
                        .max_size(egui::vec2(120.0, 80.0))
                        .maintain_aspect_ratio(true),
                );
            }
        }
        ui.add(
            egui::Label::new(
                egui::RichText::new(truncate_str(&q.stem, 150))
                    .size(12.5)
                    .color(theme::INK),
            ),
        );
    });
}

/// Short human summary of a question's right answer(s).
fn answers_preview(q: &model::Question) -> String {
    match &q.kind {
        Kind::Mc { answers } => {
            let good: Vec<&String> =
                answers.iter().filter(|a| a.correct).map(|a| &a.text).collect();
            if good.is_empty() {
                "(no answer marked)".to_string()
            } else {
                good.iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<&str>>()
                    .join("  ·  ")
            }
        }
        Kind::Tf { correct } => {
            if *correct {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Kind::Matching { pairs, .. } => match (pairs.first(), pairs.get(1)) {
            (Some(a), Some(b)) => format!("{}, {}, … → {}", a.left, b.left, a.right),
            (Some(a), _) => format!("{} → {}", a.left, a.right),
            _ => String::new(),
        },
        Kind::Blank { accept } => accept.first().map(|g| g.join(" / ")).unwrap_or_default(),
        Kind::Short { sample } => truncate_str(sample, 48),
        Kind::Multipart { parts } => format!("{} parts", parts.len()),
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
            dq.rule_lines = q.stem.matches("___").count() as u8;
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
                        s.push_str(&format!("\n{}. {}", letter(pi).to_lowercase(), part.stem));
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
                        kparts.push(format!("{}) {}", letter(pi).to_lowercase(), label));
                    }
                    PartKind::Tf { correct } => {
                        s.push_str(&format!(
                            "\n{}. {} (True / False)",
                            letter(pi).to_lowercase(),
                            part.stem
                        ));
                        kparts.push(format!(
                            "{}) {}",
                            letter(pi).to_lowercase(),
                            if *correct { "True" } else { "False" }
                        ));
                    }
                    PartKind::Short { sample } => {
                        s.push_str(&format!("\n{}. {}", letter(pi).to_lowercase(), part.stem));
                        rules += 3;
                        kparts.push(format!(
                            "{}) {}",
                            letter(pi).to_lowercase(),
                            sample.trim()
                        ));
                    }
                }
            }
            dq.text = vec![s];
            dq.rule_lines = rules;
            ktext = kparts.join("  ");
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
                card_frame(theme::CREAM).show(ui, |ui| {
                    ui.add(egui::Label::new(muted("Preview")));
                    ui.add(
                        egui::Label::new(
                            theme::serif_strong(name.clone())
                                .size(18.0)
                                .color(theme::INK),
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
                    if chip(ui, "PDF", !exu.docx).clicked() {
                        exu.docx = false;
                    }
                    if chip(ui, "DOCX", exu.docx).clicked() {
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
                    if accent_button_opt(ui, &label, n_items > 0 && (exu.student || exu.key))
                        .clicked()
                    {
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
        egui::Window::new(theme::strong("Export History"))
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
                            egui::Frame::default()
                                .fill(theme::CARD)
                                .corner_radius(8.0)
                                .inner_margin(egui::Margin::same(10))
                                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Label::new(bankui::fmt_date(rec.time)));
                                        ui.add(
                                            egui::Label::new(
                                                theme::strong(rec.exam_name.clone())
                                                    .size(14.0)
                                                    .color(theme::INK),
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
                                        if ui.button("Show files").clicked() {
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
                                        if ui.button("Regenerate").clicked() {
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
}