//! Shell screens: frame router, sidebar, welcome, home, imports list, settings,
//! toasts, plus the shell-owned overlays: AI import dialog, help, restore confirm.
//! Owned by the shell worker; also hosts `pub` helpers other screens reuse:
//! `shell_frame`, `toast`, `accent_button`, `card_frame`, `new_bank`, `new_exam`,
//! `open_bank`, `open_exam`, `finish_import`, `image_texture`, `fmt_time`, `seed_demo`.
//!
//! NOTE (egui 0.36): side/central panels draw on a `&mut Ui`; eframe 0.36 passes the
//! root `Ui` into `eframe::App::ui`, and `Ui::new(ctx, …)` is public, so every
//! `draw_*` method keeps the NOTES `ctx: &egui::Context` signature and obtains its
//! root `Ui` here. `shell_frame(app, ctx, show_sidebar, f)` is the shared chrome:
//! it draws the sidebar (when asked) and the cream central panel, then hands the
//! body `(&mut App, &mut Ui)`.

use eframe::egui;
use egui::{
    Align2, Color32, Context, CornerRadius, Frame, LayerId, Margin, Pos2, Rect, RichText, Stroke,
    StrokeKind, UiBuilder, Vec2,
};

use crate::model;
use crate::model::{Answer, Difficulty, Exam, ExamItem, Id, Import, Kind, Question, QuestionBank};
use crate::store;
use crate::theme;

/// Sidebar width; matches Test Parrot's narrow rail.
pub const SIDEBAR_WIDTH: f32 = 200.0;

/// Auto-dismiss time for toasts.
pub const TOAST_MS: i64 = 5000;

/// A staged import file picked in the import dialog (in-RAM, not persisted).
#[derive(Clone, Debug)]
pub struct ImportFile {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

// ---------- time ----------

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Days-since-epoch to (year, month, day) — Howard Hinnant's `civil_from_days`.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d)
}

/// Naive timestamp "2026-10-15 15:42"; store times are naive UTC-era seconds.
pub fn fmt_time(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", tod / 3600, (tod % 3600) / 60)
}

/// Short date for backup filenames: "2026-10-15".
fn fmt_date(secs: i64) -> String {
    fmt_time(secs)[..10].to_string()
}

/// "file" / "paste" / "ai" -> human label.
pub fn import_how_label(how: &str) -> &'static str {
    match how {
        "file" => "Read from file",
        "paste" => "Pasted",
        "ai" => "Converted by AI",
        _ => "Imported",
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

// ---------- root ui + shared chrome ----------

/// A fresh top-level `Ui` for the pass (same recipe as egui's own context code).
pub fn root_ui(ctx: &Context) -> egui::Ui {
    egui::Ui::new(
        ctx.clone(),
        egui::Id::new((ctx.viewport_id(), "__tp_ui")),
        UiBuilder::new()
            .layer_id(LayerId::background())
            .max_rect(ctx.viewport_rect()),
    )
}

/// Shared chrome used by every screen body: optional sidebar, then a cream
/// full-height central panel; the closure receives `(&mut App, &mut Ui)`.
pub fn shell_frame<R>(
    app: &mut crate::App,
    ctx: &egui::Context,
    show_sidebar: bool,
    f: impl FnOnce(&mut crate::App, &mut egui::Ui) -> R,
) {
    let mut root = root_ui(ctx);
    if show_sidebar {
        app.draw_sidebar_into(&mut root);
    }
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::cream()).inner_margin(Margin::same(20)))
        .show(&mut root, |ui| f(app, ui));
}

// ---------- small ui helpers ----------

/// One screen's top bar: a lighter strip with breadcrumb text on the left and
/// an optional status on the right (Test Parrot parity: every screen has one).
pub struct Crumb(pub String, pub Option<crate::Screen>);

pub fn topbar(
    ui: &mut egui::Ui,
    app: &mut crate::App,
    crumbs: Vec<Crumb>,
    right: Option<(String, bool)>,
) {
    Frame::new()
        .fill(theme::card())
        .inner_margin(Margin::symmetric(16, 4))
        .corner_radius(CornerRadius::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Left: breadcrumbs, plain text with slash separators (TP parity).
                let n = crumbs.len();
                for (i, crumb) in crumbs.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(2.0);
                        ui.label(theme::muted("/").size(12.5));
                        ui.add_space(2.0);
                    }
                    let last = i + 1 == n;
                    let tint = if last { theme::ink() } else { theme::muted_col() };
                    let label = RichText::new(crumb.0.clone())
                        .family(theme::bold_family())
                        .size(13.0)
                        .color(tint);
                    match (&crumb.1, last) {
                        (Some(target), false) => {
                            if ui.add(egui::Button::new(label).frame(false)).clicked() {
                                app.screen = target.clone();
                                app.bank = None;
                                app.exam = None;
                            }
                        }
                        _ => {
                            ui.label(label);
                        }
                    }
                }
                // Right: optional status text, optional storage-info button,
                // pencil glyph at the very end (RTL order: first = rightmost).
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((status, info)) = right {
                        // pencil glyph, rightmost; painted after the space is reserved
                        let pr = ui.allocate_exact_size(Vec2::new(18.0, 20.0), egui::Sense::hover());
                        draw_nav_icon(
                            ui.painter(),
                            NavIcon::Pencil,
                            pr.0.center(),
                            theme::muted_col(),
                        );
                        if info {
                            let (irect, iresp) =
                                ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::click());
                            if iresp.hovered() {
                                ui.painter()
                                    .circle_filled(irect.center(), 9.0, theme::hover_bg());
                            }
                            draw_nav_icon(
                                ui.painter(),
                                NavIcon::Info,
                                irect.center(),
                                theme::muted_col(),
                            );
                            if iresp.clicked() {
                                app.storage_info = true;
                            }
                        }
                        ui.label(theme::muted(status).size(11.5));
                    }
                });
            });
        });
}

/// Filled accent (brown) call-to-action button. Uniform 34 px height via style.
pub fn accent_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(theme::on_accent(label))
            .fill(theme::accent_col())
            .corner_radius(CornerRadius::same(8)),
    )
}

/// A card: near-white fill, brown hairline, rounded corners.
pub fn card_frame() -> Frame {
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(10))
}

/// Small solid chip with a bold colored label (step numbers, B/E badges).
pub fn chip(ui: &mut egui::Ui, label: &str, color: Color32) {
    Frame::new()
        .fill(theme::accent_faded())
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .family(theme::bold_family())
                    .color(color)
                    .size(11.0),
            );
        });
}

/// Tiny badge at the left of doc rows: "B" (bank) or "E" (exam).
fn doc_chip(ui: &mut egui::Ui, is_bank: bool) {
    chip(
        ui,
        if is_bank { "B" } else { "E" },
        if is_bank { theme::accent_col() } else { theme::green() },
    );
}

// ---------- drawn icons (no glyph-font dependency: every shape is painted) ----------

/// Sidebar icons and small chrome glyphs, drawn as vector strokes.
#[derive(Clone, Copy)]
pub enum NavIcon {
    Home,
    Banks,
    Exams,
    Import,
    Figures,
    Help,
    Gear,
    Pencil,
    Plus,
    Info,
}

/// Paint one 16px icon centered at `c`, in color `c_col` — `c` shadows egui's
/// Color32 name, so the paint calls use `c_col` throughout.
pub fn draw_nav_icon(
    painter: &egui::Painter,
    icon: NavIcon,
    c: egui::Pos2,
    c_col: Color32,
) {
    let w = Stroke::new(1.6, c_col);
    let p = |dx: f32, dy: f32| c + Vec2::new(dx, dy);
    match icon {
        NavIcon::Home => {
            // roof + walls + door
            painter.line_segment([p(-6.5, 0.0), p(0.0, -5.5)], w);
            painter.line_segment([p(0.0, -5.5), p(6.5, 0.0)], w);
            painter.line_segment([p(-5.0, -1.0), p(-5.0, 6.0)], w);
            painter.line_segment([p(5.0, -1.0), p(5.0, 6.0)], w);
            painter.line_segment([p(-5.0, 6.0), p(5.0, 6.0)], w);
            painter.line_segment([p(-1.8, 6.0), p(-1.8, 1.5)], w);
            painter.line_segment([p(1.8, 6.0), p(1.8, 1.5)], w);
        }
        NavIcon::Banks => {
            // document with lines
            painter.add(egui::Shape::rect_filled(
                Rect::from_min_max(p(-4.5, -6.5), p(4.5, 6.5)),
                CornerRadius::same(2),
                c_col.gamma_multiply(0.25),
            ));
            painter.rect_stroke(Rect::from_min_max(p(-4.5, -6.5), p(4.5, 6.5)), CornerRadius::same(2), w, StrokeKind::Middle);
            for dy in [-3.0, -0.5, 2.0, 4.5] {
                painter.line_segment([p(-2.5, dy), p(2.5, dy)], w);
            }
        }
        NavIcon::Exams => {
            // stacked sheets
            painter.rect_stroke(Rect::from_min_max(p(-5.5, -5.5), p(3.0, 4.5)), CornerRadius::same(2), w, StrokeKind::Middle);
            painter.rect_stroke(Rect::from_min_max(p(-2.5, -3.5), p(5.5, 6.5)), CornerRadius::same(2), Stroke::new(1.6, c_col), StrokeKind::Middle);
        }
        NavIcon::Import => {
            // arrow down into a tray
            painter.line_segment([p(0.0, -6.0), p(0.0, 2.0)], w);
            painter.line_segment([p(0.0, 2.0), p(-3.0, -1.0)], w);
            painter.line_segment([p(0.0, 2.0), p(3.0, -1.0)], w);
            painter.line_segment([p(-5.5, 3.0), p(-5.5, 5.5)], w);
            painter.line_segment([p(-5.5, 5.5), p(5.5, 5.5)], w);
            painter.line_segment([p(5.5, 5.5), p(5.5, 3.0)], w);
        }
        NavIcon::Figures => {
            // 2x2 grid of tiles
            for (dx, dy) in [(-3.2, -3.2), (3.2, -3.2), (-3.2, 3.2), (3.2, 3.2)] {
                painter.add(egui::Shape::rect_filled(
                    Rect::from_min_size(p(dx - 2.2, dy - 2.2), Vec2::new(4.4, 4.4)),
                    CornerRadius::same(1),
                    c_col.gamma_multiply(0.35),
                ));
            }
        }
        NavIcon::Help => {
            painter.circle_stroke(c, 5.5, Stroke::new(1.4, c_col));
            painter.text(c, egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(8.5), c_col);
        }
        NavIcon::Gear => {
            // circle + spokes (reads as a gear at this size)
            painter.circle_stroke(c, 3.2, w);
            for k in 0..4usize {
                let a = (k as f32) * std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_8;
                let d = Vec2::new(a.cos(), a.sin());
                painter.line_segment([c + d * 4.6, c + d * 6.4], w);
            }
        }
        NavIcon::Pencil => {
            // diagonal pencil: shaft + tip
            painter.line_segment([p(-4.5, 4.5), p(3.5, -3.5)], w);
            painter.line_segment([p(3.5, -3.5), p(5.5, -5.5)], w);
            painter.line_segment([p(-4.5, 4.5), p(-5.5, 5.5)], Stroke::new(2.2, c_col));
        }
        NavIcon::Plus => {
            painter.line_segment([c + Vec2::new(-4.5, 0.0), c + Vec2::new(4.5, 0.0)], Stroke::new(1.8, c_col));
            painter.line_segment([c + Vec2::new(0.0, -4.5), c + Vec2::new(0.0, 4.5)], Stroke::new(1.8, c_col));
        }
        NavIcon::Info => {
            painter.circle_stroke(c, 5.5, Stroke::new(1.4, c_col));
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "i",
                egui::FontId::proportional(8.5),
                c_col,
            );
        }
    }
}

/// The four how-it-works steps, shared by the welcome page and the help overlay.
pub const STEPS: [(&str, &str); 4] = [
    (
        "Gather your questions",
        "Write them in the editor, or hand a test you already have — a PDF, a scan, a \
         screenshot — to the built-in AI, which transcribes it into a question file the \
         import reads.",
    ),
    (
        "Keep them in a bank",
        "Every question gets a topic and a difficulty, so the one you want is a filter \
         away. A bank is yours to reuse in as many exams as you like.",
    ),
    (
        "Lay out the exam",
        "Pull questions from any bank onto the page. Reorder them, shuffle the questions \
         or the answers, and make a second version in a click.",
    ),
    (
        "Print the real thing",
        "Export a clean, printable exam — PDF or Word — complete with an answer key. \
         Every copy you hand out is saved, ready to re-download years from now.",
    ),
];

/// One numbered how-it-works card.
pub fn step_card(ui: &mut egui::Ui, i: usize) {
    let Some((title, body)) = STEPS.get(i) else { return };
    card_frame()
        .outer_margin(Margin::same(6))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_min_width(150.0);
            chip(ui, &format!("{}", i + 1), theme::accent_col());
            ui.add_space(6.0);
            ui.label(theme::strong(*title).size(13.5));
            ui.add_space(4.0);
            ui.label(theme::muted(*body).size(11.5));
        });
}

/// Free-function toast used by other screen modules: `screens::toast(app, msg, err?)`.
pub fn toast(app: &mut crate::App, text: impl Into<String>, is_error: bool) {
    if is_error {
        app.toast_err(text);
    } else {
        app.toast(text);
    }
}

// ---------- frame router ----------

impl crate::App {
    /// Draws the whole pass: the AI poll, the active screen, then the shell
    /// overlays (import dialog, help, restore confirm, toasts).
    ///
    /// Frame plumbing (egui 0.36: all panels of one pass must draw on a single
    /// root `Ui`): the bank-worker screens draw their own `CentralPanel`, so the
    /// router draws the sidebar onto one root `Ui` and hands it to them; the
    /// exam-worker screens call [`shell_frame`] (`ctx`-based) themselves; the
    /// shell screens draw through `shell_frame` too.
    pub fn draw_screen(&mut self, ctx: &egui::Context) {
        self.poll_ai();
        match &self.screen {
            crate::Screen::Welcome => self.draw_welcome(ctx),
            crate::Screen::Home => self.draw_home(ctx),
            crate::Screen::Banks | crate::Screen::BankEdit(_) => {
                let mut root = root_ui(ctx);
                self.draw_sidebar_into(&mut root);
                if matches!(self.screen, crate::Screen::BankEdit(_)) {
                    self.draw_bank_edit(&mut root);
                } else {
                    self.draw_banks(&mut root);
                }
            }
            crate::Screen::Exams => self.draw_exams(ctx),
            crate::Screen::ExamEdit(_) => self.draw_exam_edit(ctx),
            crate::Screen::Imports => self.draw_imports(ctx),
            crate::Screen::Figures => {
                let mut root = root_ui(ctx);
                self.draw_sidebar_into(&mut root);
                self.draw_figures(&mut root);
            }
            crate::Screen::Settings => self.draw_settings(ctx),
        }
        if self.ai_busy {
            ctx.request_repaint();
        }
        if matches!(self.screen, crate::Screen::Welcome) {
            // Nothing may open it there, but stay consistent if it somehow did.
            self.ai_busy = false;
            self.import_file = None;
        } else {
            self.draw_import_dialog(ctx);
        }
        self.draw_help_overlay(ctx);
        self.draw_restore_confirm(ctx);
        self.draw_toasts(ctx);
    }

    /// Drains the background AI job the import dialog started.
    pub fn poll_ai(&mut self) {
        if !self.ai_busy {
            return;
        }
        match self.ai.poll() {
            None => {}
            Some(Ok(transcription)) => {
                self.ai_busy = false;
                let source = self
                    .import_file
                    .as_ref()
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| "AI conversion".to_string());
                let model = if self.settings.ai_model.trim().is_empty() {
                    "the configured model".to_string()
                } else {
                    self.settings.ai_model.clone()
                };
                self.import_file = None;
                match crate::formats::detect_and_parse(transcription.as_bytes(), "converted") {
                    Ok(doc) => {
                        self.finish_import(doc, &source, "ai", Some(model));
                    }
                    Err(e) => {
                        self.toast_err(format!("Couldn't parse the AI transcription: {e}"));
                    }
                }
            }
            Some(Err(e)) => {
                self.ai_busy = false;
                self.import_file = None;
                self.toast_err(format!("The AI couldn't read that: {e}"));
            }
        }
    }

    // ---------- shared mutations (pub: other screens reuse) ----------

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push((text.into(), false));
    }

    pub fn toast_err(&mut self, text: impl Into<String>) {
        self.toasts.push((text.into(), true));
    }

    pub fn open_bank(&mut self, id: &Id) {
        self.screen = crate::Screen::BankEdit(id.clone());
        self.bank = Some(id.clone());
    }

    pub fn open_exam(&mut self, id: &Id) {
        self.screen = crate::Screen::ExamEdit(id.clone());
        self.exam = Some(id.clone());
    }

    pub fn new_bank(&mut self, name: &str) -> Id {
        let t = now_secs();
        let bank = QuestionBank {
            id: model::new_id(),
            name: name.to_string(),
            questions: Vec::new(),
            created: t,
            modified: t,
        };
        let id = bank.id.clone();
        self.store.banks.push(bank);
        self.store.save_banks();
        id
    }

    pub fn new_exam(&mut self, name: &str) -> Id {
        let t = now_secs();
        let exam = Exam {
            id: model::new_id(),
            name: name.to_string(),
            header: "Name: __________________ Class: ___________ Date: ___________".to_string(),
            title: None,
            items: Vec::new(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 0,
            modified: t,
        };
        let id = exam.id.clone();
        self.store.exams.push(exam);
        self.store.save_exams();
        id
    }

    /// Stores a parsed import: new Question Bank + Import record, then navigates
    /// there. `how`: "file" | "paste" | "ai"; `note`: model name (AI), etc.
    pub fn finish_import(
        &mut self,
        doc: crate::formats::ImportDoc,
        source: &str,
        how: &str,
        note: Option<String>,
    ) -> Id {
        let t = now_secs();
        let name =
            if doc.name.trim().is_empty() { "Imported questions" } else { doc.name.trim() };
        let bank = QuestionBank {
            id: model::new_id(),
            name: name.to_string(),
            questions: doc.questions,
            created: t,
            modified: t,
        };
        let id = bank.id.clone();
        let count = bank.questions.len();
        self.store.banks.push(bank);
        self.store.save_banks();
        // A Test Parrot file may also carry whole exams; keep them out if empty
        // to avoid needless disk churn. The parser can't know the new bank's id,
        // so package-import exam items arrive with EMPTY bank_ids — remap those
        // to the new bank (their question_ids already point at the regenerated
        // questions).
        if !doc.exams.is_empty() {
            let mut exams = doc.exams;
            remap_empty_bank_ids(&mut exams, &id);
            self.store.exams.extend(exams);
            self.store.save_exams();
        }
        self.store.imports.push(Import {
            id: model::new_id(),
            time: t,
            source: source.to_string(),
            how: how.to_string(),
            count,
            bank_ids: vec![id.clone()],
            note,
        });
        self.store.save_imports();
        self.import_open = false;
        self.import_file = None;
        self.import_name.clear();
        self.toast(format!(
            "Imported {} into “{}”",
            plural(count, "question"),
            name
        ));
        self.open_bank(&id);
        id
    }

    /// Loads a store image (`media/<sha>.png`) into a cached texture. Note: this is
    /// read-only content ingestion, the deliberate exception to "no std::fs from UI".
    pub fn image_texture(&mut self, ctx: &Context, file: &str) -> Option<egui::TextureHandle> {
        if let Some(t) = self.textures.get(file) {
            return Some(t.clone());
        }
        let bytes = std::fs::read(store::store_dir().join(file)).ok()?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let (w, h) = (img.width().max(1), img.height().max(1));
        let tex = ctx.load_texture(
            format!("testparrot:{file}"),
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &img),
            egui::TextureOptions::LINEAR,
        );
        self.textures.insert(file.to_string(), tex.clone());
        Some(tex)
    }

    // ---------- sidebar ----------

    pub fn draw_sidebar(&mut self, ctx: &egui::Context) {
        let mut root = root_ui(ctx);
        self.draw_sidebar_into(&mut root);
    }

    fn draw_sidebar_into(&mut self, root: &mut egui::Ui) {
        egui::Panel::left(egui::Id::new("sidebar"))
            .exact_size(SIDEBAR_WIDTH)
            .show_separator_line(false)
            .frame(Frame::new().fill(theme::card()).inner_margin(Margin::same(14)))
            .show(root, |ui| {
                // Monogram row (the logo mark): serif Q on an accent chip.
                ui.horizontal(|ui| {
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            ui.cursor().min + Vec2::new(0.0, 8.0),
                            Vec2::new(30.0, 30.0),
                        ),
                        CornerRadius::same(9),
                        theme::accent_col(),
                    );
                    let _ = ui.allocate_ui(Vec2::new(30.0, 30.0), |ui| {
                        ui.set_min_size(Vec2::new(30.0, 30.0));
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                theme::serif_strong("Q")
                                    .size(16.0)
                                    .color(theme::on_accent_col()),
                            );
                        });
                    });
                    if ui
                        .add(
                            egui::Button::new(
                                theme::serif_strong("queShion")
                                    .color(theme::ink())
                                    .size(16.0),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.screen = crate::Screen::Home;
                    }
                });
                ui.add_space(14.0);

                self.nav_item(
                    ui,
                    NavIcon::Home,
                    "Home",
                    matches!(self.screen, crate::Screen::Home),
                    |a| a.screen = crate::Screen::Home,
                );
                self.nav_item(
                    ui,
                    NavIcon::Banks,
                    "Question Banks",
                    matches!(self.screen, crate::Screen::Banks | crate::Screen::BankEdit(_)),
                    |a| a.screen = crate::Screen::Banks,
                );
                self.nav_item(
                    ui,
                    NavIcon::Exams,
                    "Exams",
                    matches!(self.screen, crate::Screen::Exams | crate::Screen::ExamEdit(_)),
                    |a| a.screen = crate::Screen::Exams,
                );
                self.nav_item(
                    ui,
                    NavIcon::Import,
                    "Imports",
                    matches!(self.screen, crate::Screen::Imports),
                    |a| a.screen = crate::Screen::Imports,
                );
                self.nav_item(
                    ui,
                    NavIcon::Figures,
                    "Figures",
                    matches!(self.screen, crate::Screen::Figures),
                    |a| a.screen = crate::Screen::Figures,
                );

                // Pin help, settings and the light/dark switch to the floor of the panel.
                let remaining = ui.available_height() - 128.0;
                ui.allocate_at_least(Vec2::new(0.0, remaining.max(0.0)), egui::Sense::hover());

                self.nav_item(
                    ui,
                    NavIcon::Help,
                    "How it works",
                    false,
                    |a| a.help_open = true,
                );
                self.nav_item(
                    ui,
                    NavIcon::Gear,
                    "Settings",
                    matches!(self.screen, crate::Screen::Settings),
                    |a| a.screen = crate::Screen::Settings,
                );
                self.theme_switch(ui);
            });
    }

    /// Light/dark toggle row: a small drawn sun or moon beside the current mode's
    /// name. Persists via settings (the App writes it when clicked).
    fn theme_switch(&mut self, ui: &mut egui::Ui) {
        let dark = theme::is_dark();
        let w = ui.available_width();
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(w, 34.0), egui::Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(8), theme::hover_bg());
        }
        let icon_c = theme::muted_col();
        let icon_center = Pos2::new(rect.left() + 15.0, rect.center().y);
        if dark {
            // moon: filled disc with an offset cutout disc
            ui.painter().circle_filled(icon_center, 6.5, icon_c);
            ui.painter()
                .circle_filled(icon_center + Vec2::new(3.5, -1.5), 5.5, theme::card());
        } else {
            // sun: core disc + 8 rays
            ui.painter().circle_filled(icon_center, 4.0, icon_c);
            for k in 0..8usize {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin()) * 8.5;
                ui.painter().line_segment(
                    [icon_center + d * 0.55, icon_center + d],
                    Stroke::new(1.4, icon_c),
                );
            }
        }
        let label = if dark { "Dark" } else { "Light" };
        ui.painter().text(
            Pos2::new(rect.left() + 30.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.5),
            theme::text_col(),
        );
        if resp.clicked() {
            let ctx = ui.ctx().clone();
            self.set_dark_mode(!dark, &ctx);
        }
    }

    fn nav_item(
        &mut self,
        ui: &mut egui::Ui,
        icon: NavIcon,
        label: &str,
        active: bool,
        goto: impl FnOnce(&mut Self) + 'static,
    ) {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 36.0), egui::Sense::click());
        if active {
            ui.painter().rect_filled(rect, CornerRadius::same(8), theme::accent_faded());
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(8), theme::hover_bg());
        }
        let c = if active { theme::accent_col() } else { theme::muted_col() };
        let center = Pos2::new(rect.left() + 17.0, rect.center().y);
        draw_nav_icon(ui.painter(), icon, center, c);
        let text_c = if active {
            theme::ink()
        } else {
            theme::text_col()
        };
        ui.painter().text(
            Pos2::new(rect.left() + 42.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.5),
            text_c,
        );
        if resp.clicked() {
            goto(self);
        }
    }

    // ---------- welcome ----------

    pub fn draw_welcome(&mut self, ctx: &egui::Context) {
        let fresh = self.store.banks.is_empty() && self.store.exams.is_empty();
        shell_frame(self, ctx, false, |app, ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(if fresh { 30.0 } else { 70.0 });
                ui.label(theme::serif("Turn anything into an exam.").size(34.0));
                ui.add_space(12.0);
                ui.set_max_width(620.0);
                ui.label(
                    theme::text(
                        "PDFs, scans, screenshots, old Word documents, questions you type \
                         yourself — queShion gathers them into Question Banks and lays \
                         them out as a clean, printable exam.",
                    )
                    .size(14.0),
                );
                ui.add_space(22.0);
                if accent_button(ui, "Get started — it's free").clicked() {
                    app.screen = crate::Screen::Home;
                }
                ui.add_space(6.0);
                ui.label(
                    theme::muted(
                        "Runs on your machine. Nothing to install, no account to make.",
                    )
                    .size(12.0),
                );
                ui.add_space(48.0);
            });

            ui.set_max_width(980.0);
            ui.vertical_centered(|ui| {
                ui.label(theme::serif_strong("How it works").size(20.0));
            });
            ui.add_space(10.0);
            if ui.available_width() >= 900.0 {
                ui.columns(4, |cols| {
                    for (i, col) in cols.iter_mut().enumerate() {
                        step_card(col, i);
                    }
                });
            } else {
                egui::Grid::new("welcome-steps-grid").num_columns(2).show(ui, |ui| {
                    step_card(ui, 0);
                    step_card(ui, 1);
                    ui.end_row();
                    step_card(ui, 2);
                    step_card(ui, 3);
                    ui.end_row();
                });
            }
        });
    }

    // ---------- home ----------

    pub fn draw_home(&mut self, ctx: &egui::Context) {
        let fresh = self.store.banks.is_empty() && self.store.exams.is_empty();
        shell_frame(self, ctx, true, |app, ui| {
            topbar(ui, app, vec![Crumb("Home".into(), None)], None);
            if fresh {
                app.draw_home_fresh(ui);
            } else {
                app.draw_home_recent(ui);
            }
        });
    }

    /// Fresh install: the get-started chooser.
    fn draw_home_fresh(&mut self, ui: &mut egui::Ui) {
        ui.add_space(24.0);
        ui.label(theme::serif_strong("Let's make your first Question Bank").size(24.0));
        ui.add_space(6.0);
        ui.label(
            theme::text(
                "A bank is a bag of questions: reuse the same questions in as many exams \
                 as you like.",
            )
            .size(13.0),
        );
        ui.add_space(20.0);

        ui.columns(2, |cols| {
            let [left, right] = cols else { return };
            card_frame().inner_margin(Margin::same(16)).show(left, |ui| {
                ui.label(theme::serif_strong("Write it myself").size(16.0));
                ui.add_space(4.0);
                ui.set_max_width(340.0);
                ui.label(theme::muted(
                    "Create a Question Bank and add questions one at a time — a clean \
                     little editor with all six question types.",
                ));
                ui.add_space(10.0);
                if accent_button(ui, "New question bank").clicked() {
                    self.first_bank();
                }
            });
            card_frame().inner_margin(Margin::same(16)).show(right, |ui| {
                ui.label(theme::serif_strong("Convert what I already have").size(16.0));
                ui.add_space(4.0);
                ui.set_max_width(340.0);
                ui.label(theme::muted(
                    "Have a test as a PDF, a scan, or screenshots? Hand it to the built-in \
                     AI and it comes back as a Question Bank.",
                ));
                ui.add_space(10.0);
                if accent_button(ui, "Import a test").clicked() {
                    self.import_open = true;
                }
            });
        });
        ui.add_space(10.0);
        if ui
            .add(
                egui::Button::new(theme::text("Or skip ahead — start with a blank Exam"))
                    .fill(Color32::TRANSPARENT)
                    .corner_radius(CornerRadius::same(8)),
            )
            .clicked()
        {
            self.first_exam();
        }
    }

    fn first_bank(&mut self) {
        let id = self.new_bank("Untitled Question Bank");
        self.open_bank(&id);
    }

    fn first_exam(&mut self) {
        let id = self.new_exam("Untitled Exam");
        self.open_exam(&id);
    }

    fn draw_home_recent(&mut self, ui: &mut egui::Ui) {
        ui.label(theme::serif_strong("Welcome back").size(20.0));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if accent_button(ui, "New question bank").clicked() {
                let id = self.new_bank("Untitled Question Bank");
                self.open_bank(&id);
            }
            if ui
                .add(
                    egui::Button::new(theme::strong("New exam"))
                        .corner_radius(CornerRadius::same(8)),
                )
                .clicked()
            {
                let id = self.new_exam("Untitled Exam");
                self.open_exam(&id);
            }
            if ui
                .add(
                    egui::Button::new(theme::text("Convert a test"))
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(CornerRadius::same(8)),
                )
                .clicked()
            {
                self.import_open = true;
            }
        });
        ui.add_space(14.0);
        ui.separator();
        ui.add_space(4.0);

        // Most-recently-modified banks first; exams carry no timestamp in the
        // model contract, so they follow in their natural order.
        let mut recent: Vec<(bool, Id, String, String, i64)> = Vec::new();
        for b in &self.store.banks {
            recent.push((
                true,
                b.id.clone(),
                b.name.clone(),
                format!("Question bank · {}", plural(b.questions.len(), "question")),
                b.modified,
            ));
        }
        for e in &self.store.exams {
            recent.push((
                false,
                e.id.clone(),
                e.name.clone(),
                format!("Exam · {}", plural(e.items.len(), "question")),
                i64::MIN,
            ));
        }
        recent.sort_by(|a, b| b.4.cmp(&a.4));
        recent.truncate(12);

        egui::ScrollArea::vertical().show(ui, |ui| {
            if recent.is_empty() {
                ui.label(theme::muted("Nothing here yet."));
                return;
            }
            let rows = recent;
            for (is_bank, id, name, meta, modified) in &rows {
                let clicked = card_frame()
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().max(0.0));
                        ui.horizontal(|ui| {
                            doc_chip(ui, *is_bank);
                            ui.vertical(|ui| {
                                ui.label(theme::strong(name.clone()));
                                ui.label(theme::muted(meta.clone()).size(11.0));
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(theme::accent("Open").size(12.0));
                                    if *modified > i64::MIN / 2 {
                                        ui.label(theme::muted(fmt_time(*modified)).size(11.0));
                                    }
                                },
                            );
                        });
                    })
                    .response
                    .clicked();
                if clicked {
                    if *is_bank {
                        self.open_bank(id);
                    } else {
                        self.open_exam(id);
                    }
                }
                ui.add_space(2.0);
            }
        });
    }

    // ---------- imports list ----------

    pub fn draw_imports(&mut self, ctx: &egui::Context) {
        shell_frame(self, ctx, true, |app, ui| {
            topbar(
                ui,
                app,
                vec![
                    Crumb("Home".into(), Some(crate::Screen::Home)),
                    Crumb("Imports".into(), None),
                ],
                None,
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(theme::serif_strong("Imports").size(22.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if accent_button(ui, "Import").clicked() {
                        app.import_open = true;
                    }
                });
            });
            ui.add_space(6.0);
            ui.label(theme::muted(
                "Everything queShion has read — pasted, converted by the AI, or read \
                 from a file.",
            ));
            ui.add_space(10.0);

            if app.store.imports.is_empty() {
                ui.label(theme::muted("No imports yet."));
            } else {
                let mut sorted = app.store.imports.clone();
                sorted.sort_by(|a, b| b.time.cmp(&a.time));
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for imp in &sorted {
                        let bank_ids = imp.bank_ids.clone();
                        card_frame()
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width().max(0.0));
                                ui.vertical(|ui| {
                                    ui.label(theme::strong(imp.source.clone()));
                                    ui.horizontal(|ui| {
                                        ui.label(theme::muted(fmt_time(imp.time)).size(11.0));
                                        ui.label(theme::muted("·").size(11.0));
                                        ui.label(
                                            theme::muted(import_how_label(&imp.how)).size(11.0),
                                        );
                                        ui.label(theme::muted("·").size(11.0));
                                        ui.label(
                                            theme::muted(plural(imp.count, "question")).size(11.0),
                                        );
                                        if let Some(note) = &imp.note {
                                            if !note.trim().is_empty() {
                                                ui.label(theme::muted("·").size(11.0));
                                                ui.label(theme::muted(note.clone()).size(11.0));
                                            }
                                        }
                                    });
                                    if !bank_ids.is_empty() {
                                        ui.horizontal(|ui| {
                                            ui.label(theme::muted("Questions in:").size(11.0));
                                            for bank_id in bank_ids {
                                                let name = app
                                                    .store
                                                    .bank(&bank_id)
                                                    .map(|b| b.name.clone())
                                                    .unwrap_or_else(|| {
                                                        "deleted bank".to_string()
                                                    });
                                                if ui
                                                    .add(
                                                        egui::Button::new(theme::accent(name.clone()))
                                                            .fill(theme::accent_faded())
                                                            .corner_radius(CornerRadius::same(6)),
                                                    )
                                                    .clicked()
                                                {
                                                    app.open_bank(&bank_id);
                                                }
                                            }
                                        });
                                    }
                                });
                            });
                        ui.add_space(2.0);
                    }
                });
            }
        });
    }

    // ---------- settings ----------

    fn save_settings(&mut self) {
        let _ = store::save_json("settings.json", &self.settings);
    }

    fn settings_changed(&self, before: &crate::Settings) -> bool {
        before.ai_provider != self.settings.ai_provider
            || before.ai_model != self.settings.ai_model
            || before.ai_base != self.settings.ai_base
    }

    /// Help line under the AI settings: where the key comes from for the provider.
    pub fn ai_help_line(&self) -> String {
        match self.settings.ai_provider.as_str() {
            "cloud" => {
                "Ollama Cloud runs gemma4:31b on Ollama's servers; queShion reads the \
                 key from the OLLAMA_API_KEY environment variable, or from ~/.config/ollama-cloud/credentials.json."
                    .to_string()
            }
            "openrouter" => {
                "OpenRouter is a keyed service; queShion reads the key from the \
                 OPENROUTER_API_KEY environment variable."
                    .to_string()
            }
            "custom" => {
                "A custom endpoint is any OpenAI-compatible chat API; it optionally reads \
                 a key from the QUESHION_AI_KEY environment variable."
                    .to_string()
            }
            _ => "Ollama runs on your machine: install it, pull a vision model (the \
                  default is gemma4:31b), and queShion finds it at 127.0.0.1:11434."
                .to_string(),
        }
    }

    pub fn draw_settings(&mut self, ctx: &egui::Context) {
        shell_frame(self, ctx, true, |app, ui| {
            topbar(
                ui,
                app,
                vec![
                    Crumb("Home".into(), Some(crate::Screen::Home)),
                    Crumb("Settings".into(), None),
                ],
                None,
            );
            ui.add_space(8.0);
            ui.label(theme::serif_strong("Settings").size(22.0));
            ui.add_space(6.0);
            ui.label(theme::text(
                "Your work is saved on this machine. Keep a copy by exporting it.",
            ));
            ui.add_space(14.0);

            // --- Back up / restore ---
            card_frame().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_min_width(ui.available_width().max(0.0));
                ui.label(theme::strong("Back up"));
                ui.add_space(4.0);
                ui.label(theme::muted(
                    "Zips up every bank, exam, import, export, setting and picture into \
                     one file.",
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if accent_button(ui, "Download backup").clicked() {
                        app.backup_now();
                    }
                    if ui
                        .add(
                            egui::Button::new(theme::strong("Restore from backup…"))
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        app.start_restore();
                    }
                });
            });
            ui.add_space(10.0);

            // --- Storage location ---
            card_frame().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_min_width(ui.available_width().max(0.0));
                ui.label(theme::strong("Where your work is stored"));
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(store::store_dir().to_string_lossy().to_string())
                            .color(theme::text_col())
                            .size(12.0)
                            .family(egui::FontFamily::Monospace),
                    );
                    if ui
                        .add(
                            egui::Button::new(theme::strong("Open folder"))
                                .fill(theme::accent_faded())
                                .corner_radius(CornerRadius::same(6)),
                        )
                        .clicked()
                    {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(store::store_dir())
                            .spawn();
                    }
                });
            });
            ui.add_space(10.0);

            // --- AI conversion ---
            card_frame().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_min_width(ui.available_width().max(0.0));
                ui.label(theme::strong("AI conversion"));
                ui.add_space(4.0);
                ui.label(theme::muted(
                    "Used when a test is converted for you. Runs where you point it — by \
                     default, entirely on your machine.",
                ));
                ui.add_space(8.0);
                let before = app.settings.clone();
                ui.horizontal_wrapped(|ui| {
                    ui.radio_value(
                        &mut app.settings.ai_provider,
                        "cloud".to_string(),
                        "Ollama Cloud",
                    );
                    ui.radio_value(
                        &mut app.settings.ai_provider,
                        "ollama".to_string(),
                        "Ollama (local)",
                    );
                    ui.radio_value(
                        &mut app.settings.ai_provider,
                        "openrouter".to_string(),
                        "OpenRouter",
                    );
                    ui.radio_value(
                        &mut app.settings.ai_provider,
                        "custom".to_string(),
                        "Custom endpoint",
                    );
                });
                if app.settings.ai_provider == "custom" {
                    ui.horizontal(|ui| {
                        ui.label(theme::text("Base URL"));
                        ui.add(
                            egui::TextEdit::singleline(&mut app.settings.ai_base)
                                .desired_width(360.0)
                                .hint_text("http://127.0.0.1:8000/v1"),
                        );
                    });
                }
                ui.horizontal(|ui| {
                    ui.label(theme::text("Model"));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.settings.ai_model)
                            .desired_width(360.0)
                            .hint_text("gemma4:31b"),
                    );
                });
                ui.add_space(6.0);
                ui.label(theme::muted(app.ai_help_line()).size(11.5));
                if app.settings_changed(&before) {
                    app.save_settings();
                }
            });
        });
    }

    // ---------- toasts ----------

    pub fn draw_toasts(&mut self, ctx: &egui::Context) {
        let now = now_ms();
        // Keep toast_times parallel to toasts; other screens may push without times.
        while self.toast_times.len() < self.toasts.len() {
            self.toast_times.push(now);
        }
        self.toast_times.truncate(self.toasts.len());
        // Expire old entries (any order; front entries are the oldest).
        let mut i = self.toasts.len();
        let mut expired: Vec<usize> = Vec::new();
        while i > 0 {
            i -= 1;
            if now - self.toast_times[i] >= TOAST_MS {
                expired.push(i);
            }
        }
        for i in expired {
            self.toasts.remove(i);
            self.toast_times.remove(i);
        }
        if self.toasts.is_empty() {
            return;
        }
        let items = self.toasts.clone();
        let mut dismissed: Vec<usize> = Vec::new();
        egui::Area::new(egui::Id::new("toasts"))
            .order(egui::Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for (i, (text, is_error)) in items.iter().enumerate() {
                    let tint = if *is_error {
                        theme::error_wash()
                    } else {
                        theme::card()
                    };
                    let stroke = if *is_error { theme::red() } else { theme::border() };
                    Frame::new()
                        .fill(tint)
                        .stroke(Stroke::new(1.0, stroke))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(if *is_error {
                                    RichText::new(text.clone()).color(theme::red())
                                } else {
                                    theme::text(text.clone())
                                });
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new("×").size(11.0))
                                            .fill(Color32::TRANSPARENT)
                                            .corner_radius(CornerRadius::same(4)),
                                    )
                                    .clicked()
                                {
                                    dismissed.push(i);
                                }
                            });
                        });
                }
            });
        // Click-to-dismiss, descending order so earlier indices stay valid.
        dismissed.sort_unstable_by(|a, b| b.cmp(a));
        for i in dismissed {
            self.toasts.remove(i);
            self.toast_times.remove(i);
        }
    }

    // ---------- import dialog (overlay) ----------

    pub fn draw_import_dialog(&mut self, ctx: &egui::Context) {
        if !self.import_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Import")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .min_width(620.0)
            .show(ctx, |ui| {
                ui.label(theme::serif_strong("Import").size(18.0));
                ui.add_space(2.0);
                ui.label(theme::muted(
                    "Type the questions in, or hand a test to the AI and it becomes a \
                     Question Bank.",
                ));
                ui.add_space(10.0);

                ui.label(theme::strong("Read it for me"));
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(theme::strong("Choose a file…"))
                                .fill(theme::accent_faded())
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        self.pick_import_file();
                    }
                    if let Some(f) = self.import_file.as_ref() {
                        ui.label(theme::text(f.name.clone()).size(12.0));
                        ui.label(theme::muted(f.mime.clone()).size(10.5));
                    }
                });
                if self.ai_busy {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(theme::text("Asking the AI to read your test…"));
                    });
                    return;
                }
                if self.import_file.is_some() {
                    ui.add_space(6.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ai_hint)
                            .desired_width(ui.available_width().max(0.0))
                            .hint_text(
                                "Anything the AI should know about this test? (optional)",
                            ),
                    );
                    ui.add_space(8.0);
                    if accent_button(ui, "Convert with AI").clicked() {
                        self.begin_ai_convert(ctx);
                    }
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label(theme::strong("Or paste the questions"));
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::multiline(&mut self.paste)
                        .desired_rows(6)
                        .desired_width(ui.available_width().max(0.0))
                        .hint_text(
                            "MC\nWhich planet is closest to the Sun?\nVenus\n*Mercury\nMars\n\n\
                             TF\nThe Sun is a star.\nT",
                        ),
                );
                ui.add_space(4.0);
                ui.label(theme::muted(
                    "* marks the correct answer. One blank line between questions.",
                ));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let ready = !self.paste.trim().is_empty();
                    if ui
                        .add_enabled(
                            ready,
                            egui::Button::new(theme::on_accent("Read my questions"))
                                .fill(if ready { theme::accent_col() } else { theme::muted_col() })
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        let text = self.paste.clone();
                        match crate::formats::detect_and_parse(
                            text.as_bytes(),
                            "Pasted questions",
                        ) {
                            Ok(doc) => {
                                self.finish_import(doc, "Pasted questions", "paste", None);
                            }
                            Err(e) => {
                                self.toast_err(format!("Couldn't read those questions: {e}"))
                            }
                        }
                    }
                    if ui
                        .add(
                            egui::Button::new(theme::text("Cancel"))
                                .fill(Color32::TRANSPARENT)
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        self.import_open = false;
                    }
                });
            });
        self.import_open = open;
    }

    /// File picker for the AI flow: images & PDFs; PDFs are rasterized to a PNG
    /// first if `pdftoppm` (poppler, optional at runtime) is on the PATH. This is
    /// read-only content ingestion — the deliberate exception to "no std::fs in UI".
    pub fn pick_import_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images & PDFs", &["png", "jpg", "jpeg", "gif", "webp", "pdf"])
            .pick_file()
        else {
            return;
        };
        let mut data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                self.toast_err(format!("Couldn't read that file: {e}"));
                return;
            }
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if ext == "pdf" {
            match pdf_first_page_png(&data) {
                Ok(png) => data = png,
                Err(e) => {
                    self.toast_err(e);
                    return;
                }
            }
        }
        let mime = mime_of(&ext).to_string();
        self.import_file =
            Some(ImportFile { name: name.clone(), mime, bytes: data });
        self.import_name = name.clone();
        self.toast(format!("Selected {name} — the AI will read it"));
    }

    /// Kicks off the AI conversion of the staged file (background thread in ai.rs).
    pub fn begin_ai_convert(&mut self, ctx: &egui::Context) {
        let Some(f) = self.import_file.clone() else {
            self.toast_err("Choose a file first.");
            return;
        };
        let settings = self.settings.clone();
        let cfg = crate::ai::AiCfg::from(&settings);
        self.ai_busy = true;
        self.toast("Reading your test…");
        self.ai.send_image(f.bytes, &f.mime, self.ai_hint.clone(), &cfg, ctx);
    }

    // ---------- help overlay ----------

    pub fn draw_help_overlay(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        let mut open = true;
        egui::Window::new("How queShion works")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .min_width(560.0)
            .show(ctx, |ui| {
                ui.label(theme::text(
                    "queShion is built around two ideas: Question Banks, which hold \
                     questions, and Exams, which pick and arrange them for printing.",
                ));
                ui.add_space(10.0);
                for (i, (title, body)) in STEPS.iter().enumerate() {
                    ui.horizontal_top(|ui| {
                        chip(ui, &format!("{}", i + 1), theme::accent_col());
                        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                            ui.set_max_width(470.0);
                            ui.label(theme::strong(*title).size(13.0));
                            ui.label(theme::muted(*body).size(11.5));
                        });
                    });
                    ui.add_space(6.0);
                }
                ui.add_space(4.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(theme::strong("Close"))
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        self.help_open = false;
                    }
                });
            });
        self.help_open = open;
    }

    // ---------- backup / restore ----------

    pub fn backup_now(&mut self) {
        let dest = rfd::FileDialog::new()
            .set_file_name(format!("queshion-backup-{}.zip", fmt_date(now_secs())))
            .save_file();
        let Some(dest) = dest else { return };
        match write_backup(&dest) {
            Ok(()) => self.toast(format!("Backup saved: {}", dest.display())),
            Err(e) => self.toast_err(format!("Backup failed: {e}")),
        }
    }

    pub fn start_restore(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("queShion backups", &["zip"])
            .pick_file()
        else {
            return;
        };
        self.confirm_restore = Some(path);
    }

    pub fn draw_restore_confirm(&mut self, ctx: &egui::Context) {
        let Some(path) = self.confirm_restore.clone() else { return };
        let mut open = true;
        let mut action: u8 = 0; // 1 = dismissed, 2 = replace
        egui::Window::new("Restore from backup?")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .min_width(460.0)
            .show(ctx, |ui| {
                ui.label(theme::text(
                    "This replaces ALL your banks, exams, imports and exports with the \
                     backup. Continue?",
                ));
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(theme::on_accent("Replace"))
                                .fill(theme::red())
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        action = 2;
                    }
                    if ui
                        .add(
                            egui::Button::new(theme::strong("Cancel"))
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        action = 1;
                    }
                });
            });
        if !open || action != 0 {
            self.confirm_restore = None;
            if open && action == 2 {
                self.restore_from(&path);
            }
        }
    }

    fn restore_from(&mut self, path: &std::path::Path) {
        match read_backup(path) {
            Ok(count) => {
                self.store = store::Store::load();
                if let Some(s) = store::load_json::<crate::Settings>("settings.json") {
                    self.settings = s;
                }
                self.toast(format!("Restored {count} files from the backup"));
            }
            Err(e) => self.toast_err(format!("Restore failed: {e}")),
        }
    }

    // ---------- demo data (QUESHION_SEED) ----------

    /// One demo bank (MC / TF / FIB) and a two-item demo exam; persisted to disk.
    pub fn seed_demo(&mut self) {
        let t = now_secs();
        let q1 = Question {
            id: model::new_id(),
            stem: "Which planet is closest to the Sun?".to_string(),
            kind: Kind::Mc {
                answers: vec![
                    Answer { text: "Mercury".into(), correct: true, locked: false },
                    Answer { text: "Venus".into(), correct: false, locked: false },
                    Answer { text: "Mars".into(), correct: false, locked: false },
                    Answer { text: "Earth".into(), correct: false, locked: false },
                ],
            },
            points: Some(1.0),
            difficulty: Some(Difficulty::Medium),
            topics: vec!["Solar system".into()],
            image: None,
            explanation: Some("Mercury is the innermost planet of the eight.".into()),
            figure: None,
            created: t,
            modified: t,
        };
        let q2 = Question {
            id: model::new_id(),
            stem: "The Sun is a star.".to_string(),
            kind: Kind::Tf { correct: true },
            points: Some(1.0),
            difficulty: Some(Difficulty::Low),
            topics: vec!["Solar system".into()],
            image: None,
            explanation: None,
            figure: None,
            created: t,
            modified: t,
        };
        let q3 = Question {
            id: model::new_id(),
            stem: "The largest planet is ___ in our solar system.".to_string(),
            kind: Kind::Blank {
                accept: vec![vec!["Jupiter".to_string()]],
            },
            points: Some(2.0),
            difficulty: Some(Difficulty::High),
            topics: vec!["Solar system".into()],
            image: None,
            explanation: None,
            figure: None,
            created: t,
            modified: t,
        };
        let (q1_id, q3_id) = (q1.id.clone(), q3.id.clone());
        let bank = QuestionBank {
            id: model::new_id(),
            name: "Solar System — Practice".into(),
            questions: vec![q1, q2, q3],
            created: t,
            modified: t,
        };
        let bank_id = bank.id.clone();
        self.store.banks.push(bank);
        let exam = Exam {
            id: model::new_id(),
            name: "Solar System — Quiz".into(),
            header: "Name: __________________ Class: ___________ Date: ___________".into(),
            title: Some("Solar System Quiz".into()),
            items: vec![
                ExamItem { bank_id: bank_id.clone(), question_id: q1_id },
                ExamItem { bank_id, question_id: q3_id },
            ],
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 0,
            modified: t,
        };
        self.store.exams.push(exam);
        self.store.save_banks();
        self.store.save_exams();
    }
} // impl crate::App

// ---------- files: import mime, pdf, backup ----------

/// Rewires parsed-exam items whose bank_id is blank (the parser regenerates ids
/// and cannot know the bank id being created right now) onto `bank_id`.
/// Non-empty bank_ids pass through untouched.
fn remap_empty_bank_ids(exams: &mut [Exam], bank_id: &str) {
    for exam in exams.iter_mut() {
        for item in exam.items.iter_mut() {
            if item.bank_id.is_empty() {
                item.bank_id = bank_id.to_string();
            }
        }
    }
}

fn mime_of(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => "image/png",
    }
}

/// Rasterizes page 1 of a PDF via `pdftoppm` (optional runtime dependency).
fn pdf_first_page_png(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::process::{Command, Stdio};

    let dir = std::env::temp_dir().join(format!("queshion-pdf-{}", now_ms()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src = dir.join("in.pdf");
    std::fs::write(&src, data).map_err(|e| e.to_string())?;
    let out = Command::new("pdftoppm")
        .args(["-png", "-singlefile", "-f", "1", "-l", "1", "-r", "150"])
        .arg(&src)
        .arg("-") // the image goes to stdout
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output();
    let result = match out {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => Ok(o.stdout),
        Ok(o) => Err(format!(
            "pdftoppm failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(
            "PDF rasterizer (pdftoppm) not found — export the pages as images instead"
                .to_string(),
        ),
        Err(e) => Err(e.to_string()),
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Guard for restore entries: only the five JSON docs plus exports/ and media/.
fn sanitize_backup_entry(name: &str) -> bool {
    if name.contains("..") || name.starts_with('/') || name.contains('\\') {
        return false;
    }
    matches!(
        name,
        "banks.json" | "exams.json" | "imports.json" | "exports.json" | "settings.json"
    ) || name.starts_with("exports/")
        || name.starts_with("media/")
}

/// Zips the whole store dir into `dest`.
fn write_backup(dest: &std::path::Path) -> Result<(), String> {
    use std::io::Write as _;

    let dir = store::store_dir();
    if !dir.exists() {
        return Err(format!("{} does not exist", dir.display()));
    }
    let file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let writer = std::io::BufWriter::new(file);
    let mut zip = zip::ZipWriter::new(writer);
    let options = zip::write::SimpleFileOptions::default();
    let mut wrote_any = false;
    for name in
        ["banks.json", "exams.json", "imports.json", "exports.json", "settings.json"]
    {
        if let Ok(data) = std::fs::read(dir.join(name)) {
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(&data).map_err(|e| e.to_string())?;
            wrote_any = true;
        }
    }
    for sub in ["exports", "media"] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_file() {
                if let Ok(data) = std::fs::read(&path) {
                    zip.start_file(format!("{sub}/{name}"), options).map_err(|e| e.to_string())?;
                    zip.write_all(&data).map_err(|e| e.to_string())?;
                    wrote_any = true;
                }
            }
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    if !wrote_any {
        return Err("the store is empty — nothing to back up".to_string());
    }
    Ok(())
}

/// Replaces the store dir contents with a backup zip's files; returns file count.
fn read_backup(path: &std::path::Path) -> Result<usize, String> {
    use std::io::Read as _;

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
    let dir = store::store_dir();
    let mut count = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if entry.is_dir() || !sanitize_backup_entry(&name) {
            continue;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        let dest = dir.join(&name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(dest, data).map_err(|e| e.to_string())?;
        count += 1;
    }
    if count == 0 {
        return Err("nothing usable found in the backup".to_string());
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_start_formats() {
        assert_eq!(fmt_time(0), "1970-01-01 00:00");
    }

    #[test]
    fn known_timestamps() {
        // 1,000,000,001 s epoch — the famous billion-second bash.
        assert_eq!(fmt_time(1_000_000_001), "2001-09-09 01:46");
        // 2009-02-13 23:31:30 UTC.
        assert_eq!(fmt_time(1_234_567_890), "2009-02-13 23:31");
        // Leap day 2024.
        assert_eq!(fmt_time(1_709_164_800), "2024-02-29 00:00");
    }

    #[test]
    fn backup_entry_guard() {
        assert!(sanitize_backup_entry("banks.json"));
        assert!(sanitize_backup_entry("exports/abc.json"));
        assert!(sanitize_backup_entry("media/12ab.png"));
        assert!(!sanitize_backup_entry("../etc/passwd"));
        assert!(!sanitize_backup_entry("/etc/passwd"));
        assert!(!sanitize_backup_entry("C:\\Windows"));
        assert!(!sanitize_backup_entry("random.txt"));
    }

    #[test]
    fn package_exam_items_remap_onto_the_new_bank() {
        let bank_id = "b123".to_string();
        let mut exams = vec![
            Exam {
                id: "e1".into(),
                name: "Quiz".into(),
                header: String::new(),
                title: None,
                items: vec![
                    ExamItem { bank_id: String::new(), question_id: "q1".into() },
                    ExamItem { bank_id: "other-bank".into(), question_id: "q2".into() },
                ],
                heading_pt: 16.0,
                body_pt: 11.0,
                margin_mm: 18.0,
                ruled_blanks: 0,
                modified: 0,
            },
        ];
        remap_empty_bank_ids(&mut exams, &bank_id);
        assert_eq!(exams[0].items[0].bank_id, "b123");
        // A non-empty bank_id is never touched.
        assert_eq!(exams[0].items[1].bank_id, "other-bank");
        assert_eq!(exams[0].items[1].question_id, "q2"); // ids pass through verbatim
    }
}