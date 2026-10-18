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
pub const SIDEBAR_WIDTH: f32 = 88.0;
/// Gap between the dock and the content.
pub const DOCK_GAP: f32 = 16.0;

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

/// Naive timestamp "2026-10-15 15:42"; store times are naive UTC-era seconds.
pub fn fmt_time(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400);
    let (y, m, d) = crate::time::civil_from_days(days);
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
        egui::Id::new((ctx.viewport_id(), "vs_ui")),
        UiBuilder::new()
            .layer_id(LayerId::background())
            .max_rect(theme::area_or(ctx.viewport_rect())),
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
    desk_panel(&mut root, 20, |ui| f(app, ui));
}

/// The V4 desk: cream central panel with the faint 24 px graph-paper grid.
/// Every screen body goes through this so the chrome is uniform.
pub fn desk_panel<R>(
    root: &mut egui::Ui,
    margin: i8,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::cream()).inner_margin(Margin {
            left: margin,
            top: margin,
            bottom: margin,
            // leave room for the floating scrollbar so content never sits under it
            right: if margin >= 16 { margin.saturating_add(14) } else { margin },
        }))
        .show(root, |ui| {
            theme::paint_grid(ui.painter(), ui.max_rect());
            f(ui)
        })
        .inner
}

// ---------- small ui helpers ----------

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
    draw_nav_icon_k(painter, icon, c, c_col, 1.0);
}

/// Same icons at `k` times the 16 px design size (the dock draws them at 21 px).
pub fn draw_nav_icon_k(
    painter: &egui::Painter,
    icon: NavIcon,
    c: egui::Pos2,
    c_col: Color32,
    k: f32,
) {
    let w = Stroke::new(1.6, c_col);
    let p = |dx: f32, dy: f32| c + Vec2::new(dx, dy) * k;
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
                    Rect::from_min_size(p(dx - 2.2, dy - 2.2), Vec2::new(4.4, 4.4) * k),
                    CornerRadius::same(1),
                    c_col.gamma_multiply(0.35),
                ));
            }
        }
        NavIcon::Help => {
            painter.circle_stroke(c, 5.5 * k, Stroke::new(1.4, c_col));
            painter.text(c, egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(8.5 * k), c_col);
        }
        NavIcon::Gear => {
            // circle + spokes (reads as a gear at this size)
            painter.circle_stroke(c, 3.2 * k, w);
            for i in 0..4usize {
                let a = (i as f32) * std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_8;
                let d = Vec2::new(a.cos(), a.sin());
                painter.line_segment([c + d * 4.6 * k, c + d * 6.4 * k], w);
            }
        }
        NavIcon::Pencil => {
            // diagonal pencil: shaft + tip
            painter.line_segment([p(-4.5, 4.5), p(3.5, -3.5)], w);
            painter.line_segment([p(3.5, -3.5), p(5.5, -5.5)], w);
            painter.line_segment([p(-4.5, 4.5), p(-5.5, 5.5)], Stroke::new(2.2, c_col));
        }
        NavIcon::Plus => {
            painter.line_segment([c + Vec2::new(-4.5, 0.0) * k, c + Vec2::new(4.5, 0.0) * k], Stroke::new(1.8, c_col));
            painter.line_segment([c + Vec2::new(0.0, -4.5) * k, c + Vec2::new(0.0, 4.5) * k], Stroke::new(1.8, c_col));
        }
        NavIcon::Info => {
            painter.circle_stroke(c, 5.5 * k, Stroke::new(1.4, c_col));
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "i",
                egui::FontId::proportional(8.5 * k),
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
    crate::kit::card()
        .outer_margin(Margin::same(6))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_min_width(150.0);
            let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::hover());
            ui.painter().circle_filled(r.center(), 13.0, theme::accent_faded());
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                format!("{}", i + 1),
                egui::FontId::new(12.0, theme::bold_family()),
                theme::accent_col(),
            );
            ui.add_space(6.0);
            ui.label(theme::serif(*title).size(16.0));
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
        }
        if self.storage_info {
            self.storage_info_modal(ctx);
        }
        crate::bankui::draw_popover(self, ctx);
        self.draw_help_overlay(ctx);
        self.draw_restore_confirm(ctx);
        self.draw_toasts(ctx);
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
            details: Default::default(),
        };
        let id = bank.id.clone();
        self.store.banks.push(bank);
        self.store.save_banks();
        id
    }

    pub fn new_exam(&mut self, name: &str) -> Id {
        let t = now_secs();
        let exam = Exam { paper_style: Default::default(), margins: None,
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
            details: Default::default(),
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
        let bytes = store::read_media(file)?;
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
        // V4 dock: 88 px rounded rail inside a 10 px app gutter; below ~900 px it
        // collapses to a 64 px icon rail (labels hidden).
        let narrow = root.max_rect().width() < 900.0;
        let dock_w = if narrow { 64.0 } else { 88.0 };
        egui::Panel::left(egui::Id::new("sidebar"))
            .exact_size(dock_w + 10.0 + DOCK_GAP)
            .show_separator_line(false)
            .frame(Frame::new().fill(theme::cream()).inner_margin(Margin {
                left: 10,
                right: DOCK_GAP as i8,
                top: 10,
                bottom: 10,
            }))
            .show(root, |ui| {
                theme::paint_grid(ui.painter(), ui.max_rect());
                let rect = ui.max_rect();
                ui.painter().rect(
                    rect,
                    CornerRadius::same(18),
                    theme::rail(),
                    if theme::is_dark() { Stroke::new(1.0, theme::border()) } else { Stroke::NONE },
                    StrokeKind::Inside,
                );
                let inner = rect.shrink2(Vec2::new((dock_w - if narrow { 48.0 } else { 70.0 }) / 2.0, 12.0));
                let mut ui = ui.new_child(UiBuilder::new().max_rect(inner));
                let ui = &mut ui;
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
                let item_w = inner.width();

                // Logo tile: serif Q on the highlighter-free accent chip.
                let (lr, lresp) =
                    ui.allocate_exact_size(Vec2::new(item_w, 40.0), egui::Sense::click());
                let chip = Rect::from_center_size(lr.center(), Vec2::splat(38.0));
                ui.painter().rect_filled(chip, CornerRadius::same(12), theme::accent_col());
                ui.painter().text(
                    chip.center(),
                    egui::Align2::CENTER_CENTER,
                    "Q",
                    egui::FontId::new(21.0, theme::serif_strong_family()),
                    theme::on_accent_col(),
                );
                if lresp.clicked() {
                    self.screen = crate::Screen::Home;
                }
                ui.add_space(14.0);

                let nav = [
                    (NavIcon::Home, "Home", crate::Screen::Home),
                    (NavIcon::Banks, "Banks", crate::Screen::Banks),
                    (NavIcon::Exams, "Exams", crate::Screen::Exams),
                    (NavIcon::Import, "Import", crate::Screen::Imports),
                    (NavIcon::Figures, "Figures", crate::Screen::Figures),
                    (NavIcon::Gear, "Settings", crate::Screen::Settings),
                ];
                for (icon, label, target) in nav {
                    let active = match (&self.screen, &target) {
                        (crate::Screen::BankEdit(_), crate::Screen::Banks) => true,
                        (crate::Screen::ExamEdit(_), crate::Screen::Exams) => true,
                        (cur, t) => std::mem::discriminant(cur) == std::mem::discriminant(t),
                    };
                    if self.dock_item(ui, icon, label, active, item_w, narrow) {
                        self.screen = target;
                    }
                }

                // Floor of the dock: lock note, then the vertical light/dark switch.
                let floor = (ui.available_height() - 28.0 - 62.0 - 12.0).max(0.0);
                ui.allocate_at_least(Vec2::new(item_w, floor), egui::Sense::hover());
                let (lock, lresp) = ui.allocate_exact_size(Vec2::new(item_w, 28.0), egui::Sense::click());
                let lock_col = if lresp.hovered() { theme::dock_text() } else { theme::dock_text().gamma_multiply(0.7) };
                draw_lock(ui.painter(), lock.center(), lock_col);
                if lresp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if lresp.clone().on_hover_text("Saved on this computer. Click to see where.").clicked() {
                    self.storage_info = true;
                }
                ui.add_space(4.0);
                self.theme_switch(ui, item_w);
            });
    }

    /// Vertical light/dark switch: a pill with a sun above a moon; the active
    /// mode sits on the highlighter.
    fn theme_switch(&mut self, ui: &mut egui::Ui, w: f32) {
        let dark = theme::is_dark();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 62.0), egui::Sense::click());
        let pill = Rect::from_center_size(rect.center(), Vec2::new(30.0_f32.min(w), 62.0));
        ui.painter().rect_filled(pill, CornerRadius::same(15), theme::dock_text().gamma_multiply(0.12));
        let sun_c = Pos2::new(pill.center().x, pill.top() + 15.0);
        let moon_c = Pos2::new(pill.center().x, pill.bottom() - 15.0);
        let active_c = if dark { moon_c } else { sun_c };
        ui.painter().circle_filled(active_c, 12.5, theme::highlighter());
        let ink = theme::on_highlighter();
        let off = theme::dock_text().gamma_multiply(0.75);
        // sun: core disc + 8 rays
        let sc = if dark { off } else { ink };
        ui.painter().circle_filled(sun_c, 3.6, sc);
        for k in 0..8usize {
            let a = k as f32 * std::f32::consts::TAU / 8.0;
            let d = Vec2::new(a.cos(), a.sin());
            ui.painter().line_segment([sun_c + d * 5.6, sun_c + d * 7.6], Stroke::new(1.3, sc));
        }
        // moon: disc with an offset cutout in the active-slot colour
        let mc = if dark { ink } else { off };
        ui.painter().circle_filled(moon_c, 6.0, mc);
        let cut = if dark { theme::highlighter() } else { theme::rail() };
        ui.painter().circle_filled(moon_c + Vec2::new(3.2, -1.8), 5.2, cut);
        if resp.clicked() {
            let ctx = ui.ctx().clone();
            self.set_dark_mode(!dark, &ctx);
        }
    }

    /// One dock nav item: icon over a small label; active = highlighter fill, dark ink.
    fn dock_item(
        &mut self,
        ui: &mut egui::Ui,
        icon: NavIcon,
        label: &str,
        active: bool,
        w: f32,
        narrow: bool,
    ) -> bool {
        let h = if narrow { 48.0 } else { 56.0 };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
        if active {
            ui.painter().rect_filled(rect, CornerRadius::same(13), theme::highlighter());
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(13), theme::dock_text().gamma_multiply(0.12));
        }
        let (icon_c, text_c) = if active {
            (theme::on_highlighter(), theme::on_highlighter())
        } else {
            (theme::dock_text(), theme::dock_text())
        };
        let cy = if narrow { rect.center().y } else { rect.top() + 20.0 };
        draw_nav_icon_k(ui.painter(), icon, Pos2::new(rect.center().x, cy), icon_c, 21.0 / 16.0);
        if !narrow {
            ui.painter().text(
                Pos2::new(rect.center().x, rect.bottom() - 11.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::new(10.5, theme::bold_family()),
                text_c,
            );
        } else {
            resp.clone().on_hover_text(label);
        }
        resp.clicked()
    }

    // ---------- welcome ----------

    pub fn draw_welcome(&mut self, ctx: &egui::Context) {
        let fresh = self.store.banks.is_empty() && self.store.exams.is_empty();
        shell_frame(self, ctx, false, |app, ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(if fresh { 30.0 } else { 70.0 });
                    ui.label(theme::serif("Turn anything into an exam.").size(40.0));
                    ui.add_space(12.0);
                    ui.set_max_width(620.0);
                    ui.label(
                        theme::text(
                            "PDFs, scans, screenshots, old Word documents, questions you type \
                             yourself: verSHion gathers them into Question Banks and lays \
                             them out as a clean, printable exam.",
                        )
                        .size(14.0),
                    );
                    ui.add_space(22.0);
                    if crate::kit::primary_button(ui, "Get started, it's free").clicked() {
                        app.screen = crate::Screen::Home;
                    }
                    ui.add_space(6.0);
                    ui.label(
                        theme::muted("Runs on your machine. Nothing to install, no account to make.")
                            .size(12.0),
                    );
                    ui.add_space(48.0);
                });

                ui.set_max_width(ui.available_width().min(980.0));
                ui.vertical_centered(|ui| {
                    crate::kit::small_caps(ui, "How it works");
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
        });
    }

    // ---------- home ----------

    pub fn draw_home(&mut self, ctx: &egui::Context) {
        shell_frame(self, ctx, true, |app, ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(1040.0));
                app.draw_home_body(ui);
            });
        });
    }

    fn first_bank(&mut self) {
        let id = self.new_bank("Untitled Question Bank");
        self.open_bank(&id);
    }

    fn first_exam(&mut self) {
        let id = self.new_exam("Untitled Exam");
        self.open_exam(&id);
    }

    fn save_view_prefs(&self) {
        let _ = store::save_json("settings.json", &self.settings);
    }

    fn draw_home_body(&mut self, ui: &mut egui::Ui) {
        use crate::kit;
        let fresh = self.store.banks.is_empty() && self.store.exams.is_empty();
        let n_banks = self.store.banks.len();
        let n_exams = self.store.exams.len();
        let n_q: usize = self.store.banks.iter().map(|b| b.questions.len()).sum();
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                kit::page_header(
                    ui,
                    &["Home"],
                    if fresh { "Welcome." } else { "Welcome back." },
                    None,
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (n, label) in [(n_q, "questions"), (n_exams, "exams"), (n_banks, "banks")] {
                    // Fixed-size slot owns the direction (the parent row is right-to-left).
                    ui.allocate_ui_with_layout(
                        Vec2::new(92.0, 58.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            Frame::new()
                                .fill(theme::card())
                                .stroke(Stroke::new(1.0, theme::border()))
                                .corner_radius(CornerRadius::same(14))
                                .inner_margin(Margin::symmetric(14, 7))
                                .show(ui, |ui| {
                                    ui.set_min_width(62.0);
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.label(theme::serif(n.to_string()).size(22.0));
                                    ui.label(theme::muted(label).size(11.5));
                                });
                        },
                    );
                }
            });
        });
        ui.add_space(14.0);

        // Action tiles.
        let gap = 12.0;
        let avail = ui.available_width();
        let stacked = avail < 620.0;
        let tiles: [(NavIcon, &str, &str, bool); 3] = [
            (NavIcon::Plus, "New exam", "Start from a blank page", true),
            (NavIcon::Banks, "New bank", "A shelf for questions", false),
            (NavIcon::Import, "Import", "Anything: text, files, photos", false),
        ];
        let mut tile_hit: Option<usize> = None;
        let widths: Vec<f32> = if stacked {
            vec![avail; 3]
        } else {
            let unit = (avail - 2.0 * gap) / 3.25;
            vec![unit * 1.25, unit, unit]
        };
        let mut tile_row = |ui: &mut egui::Ui, range: std::ops::Range<usize>| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for i in range {
                    let (icon, title, sub, pri) = tiles[i];
                    if home_tile(ui, widths[i], icon, title, sub, pri) {
                        tile_hit = Some(i);
                    }
                }
            });
        };
        if stacked {
            for i in 0..3 {
                tile_row(ui, i..i + 1);
            }
        } else {
            tile_row(ui, 0..3);
        }
        match tile_hit {
            Some(0) => self.first_exam(),
            Some(1) => self.first_bank(),
            Some(2) => self.screen = crate::Screen::Imports,
            _ => {}
        }
        ui.add_space(18.0);

        if fresh {
            kit::card().inner_margin(Margin::same(18)).show(ui, |ui| {
                ui.set_min_width(ui.available_width().max(0.0));
                ui.label(theme::serif("Let's make your first Question Bank").size(20.0));
                ui.add_space(4.0);
                ui.label(theme::text(
                    "A bank is a bag of questions: reuse the same questions in as many exams as you \
                     like. Write them yourself, or hand a PDF, a scan or screenshots to the built-in \
                     AI and it comes back as a bank.",
                ));
            });
            return;
        }

        // ---- Recent exams ----
        let mut exams: Vec<ExamRow> = self
            .store
            .exams
            .iter()
            .map(|e| {
                let mut pts = 0.0;
                for it in &e.items {
                    if let Some(q) = self
                        .store
                        .banks
                        .iter()
                        .find(|b| b.id == it.bank_id)
                        .and_then(|b| b.questions.iter().find(|q| q.id == it.question_id))
                    {
                        pts += q.points.unwrap_or(1.0);
                    }
                }
                let bank = e
                    .items
                    .first()
                    .and_then(|it| self.store.banks.iter().find(|b| b.id == it.bank_id))
                    .map(|b| b.name.clone())
                    .unwrap_or_else(|| "No bank yet".to_string());
                ExamRow {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    qs: e.items.len(),
                    pts,
                    bank,
                    edited: e.modified,
                }
            })
            .collect();
        exams.sort_by(|a, b| b.edited.cmp(&a.edited));

        let mut exam_mode = if self.settings.home_exams_list { kit::ViewMode::List } else { kit::ViewMode::Cards };
        let mut link_exams = false;
        ui.horizontal(|ui| {
            kit::small_caps(ui, "Recent exams");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if kit::view_toggle(ui, &mut exam_mode) {
                    self.settings.home_exams_list = exam_mode == kit::ViewMode::List;
                    self.save_view_prefs();
                }
                if ui.add(egui::Button::new(theme::accent("All exams").family(theme::bold_family()).size(12.5)).frame(false)).clicked() {
                    link_exams = true;
                }
            });
        });
        ui.add_space(6.0);
        if link_exams {
            self.screen = crate::Screen::Exams;
        }
        let mut open_ex: Option<Id> = None;
        if exams.is_empty() {
            ui.label(theme::muted("No exams yet. Start one with New exam above."));
        } else if exam_mode == kit::ViewMode::Cards {
            let cols = grid_cols(ui.available_width(), 230.0, gap, 3);
            let cw = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
            for chunk in exams.iter().take(cols * 2).collect::<Vec<_>>().chunks(cols) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (k, e) in chunk.iter().enumerate() {
                        if exam_card(ui, self, cw, e, k % 2 == 1) {
                            open_ex = Some(e.id.clone());
                        }
                    }
                });
                ui.add_space(gap - 4.0);
            }
        } else {
            let rows: Vec<&ExamRow> = exams.iter().take(8).collect();
            if let Some(id) = exam_list(ui, &rows) {
                open_ex = Some(id);
            }
        }
        if let Some(id) = open_ex {
            self.open_exam(&id);
        }
        ui.add_space(22.0);

        // ---- Your banks ----
        let mut banks: Vec<BankRow> = self
            .store
            .banks
            .iter()
            .map(|b| {
                let mut topics: Vec<&String> = b.questions.iter().flat_map(|q| q.topics.iter()).collect();
                topics.sort();
                topics.dedup();
                let mut counts = [0usize; 6];
                for q in &b.questions {
                    counts[q.kind.qkind() as usize] += 1;
                }
                let total = b.questions.len().max(1) as f32;
                let mut mix: Vec<f32> = counts.iter().filter(|c| **c > 0).map(|c| *c as f32 / total).collect();
                mix.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
                BankRow {
                    id: b.id.clone(),
                    name: b.name.clone(),
                    qs: b.questions.len(),
                    topics: topics.len(),
                    edited: b.modified,
                    mix,
                }
            })
            .collect();
        banks.sort_by(|a, b| b.edited.cmp(&a.edited));
        let mut bank_mode = if self.settings.home_banks_list { kit::ViewMode::List } else { kit::ViewMode::Cards };
        let mut link_banks = false;
        ui.horizontal(|ui| {
            kit::small_caps(ui, "Your banks");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if kit::view_toggle(ui, &mut bank_mode) {
                    self.settings.home_banks_list = bank_mode == kit::ViewMode::List;
                    self.save_view_prefs();
                }
                if ui.add(egui::Button::new(theme::accent("All banks").family(theme::bold_family()).size(12.5)).frame(false)).clicked() {
                    link_banks = true;
                }
            });
        });
        ui.add_space(6.0);
        if link_banks {
            self.screen = crate::Screen::Banks;
        }
        let mut open_bk: Option<Id> = None;
        if banks.is_empty() {
            ui.label(theme::muted("No banks yet. Start one with New bank above."));
        } else if bank_mode == kit::ViewMode::Cards {
            let cols = grid_cols(ui.available_width(), 226.0, gap, 4);
            let cw = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
            for chunk in banks.iter().take(cols * 2).collect::<Vec<_>>().chunks(cols) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for b in chunk {
                        if bank_card(ui, cw, b) {
                            open_bk = Some(b.id.clone());
                        }
                    }
                });
                ui.add_space(gap - 4.0);
            }
        } else {
            let rows: Vec<&BankRow> = banks.iter().take(10).collect();
            if let Some(id) = bank_list(ui, &rows) {
                open_bk = Some(id);
            }
        }
        if let Some(id) = open_bk {
            self.open_bank(&id);
        }

        // ---- Last export strip ----
        if let Some(rec) = self.store.exports.iter().max_by_key(|r| r.time).cloned() {
            ui.add_space(20.0);
            let mut go = false;
            Frame::new()
                .fill(theme::card())
                .stroke(Stroke::new(1.0, theme::border()))
                .corner_radius(CornerRadius::same(255))
                .inner_margin(Margin::symmetric(16, 9))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().max(0.0));
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
                        draw_printer_icon(ui.painter(), r.center(), theme::accent_col());
                        let what = if rec.what.is_empty() { String::new() } else { format!(" with {}", rec.what.iter().skip(1).cloned().collect::<Vec<_>>().join(", ")) };
                        let what = if what == " with " { String::new() } else { what };
                        ui.label(theme::strong("Last export").size(12.5));
                        ui.label(
                            theme::text(format!(
                                "{} · {} · {}{}, {}",
                                fmt_when(rec.time),
                                rec.exam_name,
                                rec.format.to_uppercase(),
                                what,
                                plural(rec.versions.max(1) as usize, "version")
                            ))
                            .size(12.5),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(theme::accent("Export history").family(theme::bold_family()).size(12.5)).frame(false)).clicked() {
                                go = true;
                            }
                        });
                    });
                });
            if go {
                if self.store.exams.iter().any(|e| e.id == rec.exam_id) {
                    self.history_open = true;
                    self.open_exam(&rec.exam_id);
                } else {
                    self.toast("That exam no longer exists, so its history is not available.");
                }
            }
        }
        ui.add_space(16.0);
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
        let cfg = crate::ai::AiCfg::from(&self.settings);
        match cfg.provider.as_str() {
            "cloud" => format!("Ollama Cloud runs gemma4:31b on Ollama's servers. {}", cfg.key_source_help()),
            "ollama" => "Ollama runs on your machine: install it, pull a vision model (the \
                  default is gemma4:31b), and verSHion finds it at 127.0.0.1:11434."
                .to_string(),
            _ => cfg.key_source_help(),
        }
    }

    /// Settings > AI: masked key entry, save/remove, status and a connection test for the
    /// current keyed provider. The key lives only in the text buffer until saved (then it is
    /// cleared), and in `ai-keys.json`; it is never shown, logged or put in a toast.
    fn draw_ai_key_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use crate::kit;
        let cfg = crate::ai::AiCfg::from(&self.settings);
        if !cfg.uses_key() {
            return;
        }
        type Probe = std::sync::Arc<std::sync::Mutex<Option<Result<(), String>>>>;
        let id_buf = egui::Id::new("ai_key_buf");
        let id_probe = egui::Id::new("ai_key_probe");
        let id_msg = egui::Id::new("ai_key_msg");
        let mut buf: String = ctx.data_mut(|d| d.get_temp(id_buf)).unwrap_or_default();
        let probe: Probe = ctx.data_mut(|d| d.get_temp(id_probe)).unwrap_or_default();
        let mut msg: Option<(String, bool)> = ctx.data_mut(|d| d.get_temp(id_msg));
        if let Some(r) = probe.lock().ok().and_then(|mut g| g.take()) {
            msg = Some(match r {
                Ok(()) => ("Connection OK".to_string(), false),
                Err(e) => (e, true),
            });
        }
        let testing = msg.as_ref().is_some_and(|m| m.0 == "Testing…");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(theme::text("API key"));
            ui.add(
                egui::TextEdit::singleline(&mut buf)
                    .password(true)
                    .desired_width(300.0)
                    .hint_text("paste a key"),
            );
            if kit::primary_button(ui, "Save key").clicked() && !buf.trim().is_empty() {
                match crate::ai::set_key(&cfg.provider, Some(&buf)) {
                    Ok(()) => msg = Some(("Key saved on this computer".to_string(), false)),
                    Err(e) => msg = Some((e, true)),
                }
                buf.clear();
            }
        });
        ui.horizontal(|ui| {
            let status = cfg.key_status();
            ui.label(theme::muted(status.describe()).size(12.0));
            if crate::ai::load_keys().get(&cfg.provider).is_some() && kit::danger_text_button(ui, "Remove key").clicked() {
                match crate::ai::set_key(&cfg.provider, None) {
                    Ok(()) => msg = Some(("Key removed".to_string(), false)),
                    Err(e) => msg = Some((e, true)),
                }
            }
            if !testing && kit::ghost_button(ui, "Test connection").clicked() {
                msg = Some(("Testing…".to_string(), false));
                let probe2 = probe.clone();
                let ctx2 = ctx.clone();
                let cfg2 = cfg.clone();
                std::thread::spawn(move || {
                    let r = crate::ai::test_connection(&cfg2);
                    if let Ok(mut g) = probe2.lock() {
                        *g = Some(r);
                    }
                    ctx2.request_repaint();
                });
            }
        });
        if let Some((m, bad)) = &msg {
            let t = if *bad { theme::muted(m.clone()).color(theme::bad()) } else { theme::muted(m.clone()) };
            ui.label(t.size(12.0));
        }
        ctx.data_mut(|d| {
            d.insert_temp(id_buf, buf);
            d.insert_temp(id_probe, probe);
            match msg {
                Some(m) => {
                    d.insert_temp(id_msg, m);
                }
                None => {
                    d.remove::<(String, bool)>(id_msg);
                }
            }
        });
    }

    pub fn draw_settings(&mut self, ctx: &egui::Context) {
        use crate::kit;
        shell_frame(self, ctx, true, |app, ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(820.0));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        kit::page_header(
                            ui,
                            &["Home", "Settings"],
                            "Settings",
                            Some("Your work is saved on this machine. Keep a copy by exporting it."),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                        if kit::ghost_button(ui, "How it works").clicked() {
                            app.help_open = true;
                        }
                    });
                });
                ui.add_space(14.0);

                // --- Back up / restore ---
                kit::card().inner_margin(Margin::same(18)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width().max(0.0));
                    ui.label(theme::serif("Back up").size(18.0));
                    ui.add_space(4.0);
                    ui.label(theme::muted(
                        "Zips up every bank, exam, import, export, setting and picture into one file.",
                    ));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if kit::primary_button(ui, "Download backup").clicked() {
                            app.backup_now();
                        }
                        if kit::ghost_button(ui, "Restore from backup…").clicked() {
                            app.start_restore();
                        }
                    });
                });
                ui.add_space(12.0);

                // --- Storage location ---
                kit::card().inner_margin(Margin::same(18)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width().max(0.0));
                    ui.label(theme::serif("Where your work is stored").size(18.0));
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(store::backend_label())
                                .color(theme::text_col())
                                .size(12.0)
                                .family(egui::FontFamily::Monospace),
                        )
                        .wrap(),
                    );
                    ui.add_space(8.0);
                    if kit::ghost_button(ui, "Open folder").clicked() {
                        let _ = crate::platform::open_path(&store::store_dir());
                    }
                });
                ui.add_space(12.0);

                // --- AI conversion ---
                kit::card().inner_margin(Margin::same(18)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width().max(0.0));
                    ui.label(theme::serif("AI conversion").size(18.0));
                    ui.add_space(4.0);
                    ui.label(theme::muted(
                        "Used when a test is converted for you. Runs where you point it, by \
                         default entirely on your machine.",
                    ));
                    ui.add_space(10.0);
                    let before = app.settings.clone();
                    ui.horizontal_wrapped(|ui| {
                        for (val, label) in [
                            ("cloud", "Ollama Cloud"),
                            ("ollama", "Ollama (local)"),
                            ("openrouter", "OpenRouter"),
                            ("custom", "Custom endpoint"),
                        ] {
                            let on = app.settings.ai_provider == val;
                            if kit::chip_toggle(ui, label, on).clicked() {
                                app.settings.ai_provider = val.to_string();
                            }
                        }
                    });
                    ui.add_space(6.0);
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
                    app.draw_ai_key_ui(ui, ctx);
                    ui.add_space(6.0);
                    ui.label(theme::muted(app.ai_help_line()).size(11.5));
                    if app.settings_changed(&before) {
                        app.save_settings();
                    }
                });
                ui.add_space(16.0);
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
                        .corner_radius(CornerRadius::same(crate::kit::R_CARD))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 14,
                            spread: 0,
                            color: Color32::from_black_alpha(40),
                        })
                        .inner_margin(Margin::symmetric(16, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(if *is_error {
                                    RichText::new(text.clone()).color(theme::red())
                                } else {
                                    theme::text(text.clone())
                                });
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new("×").size(12.0).color(theme::muted_col()))
                                            .fill(Color32::TRANSPARENT)
                                            .corner_radius(CornerRadius::same(255)),
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

    // ---------- help overlay ----------

    pub fn draw_help_overlay(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("vershion_help"))
            .frame(
                crate::kit::card()
                    .inner_margin(Margin::same(22))
                    .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(60) }),
            )
            .show(ctx, |ui| {
                ui.set_width(540.0);
                ui.label(theme::serif("How verSHion works").size(26.0));
                ui.add_space(4.0);
                ui.label(theme::muted(
                    "Two ideas: Question Banks hold questions; Exams pick and arrange them for printing.",
                ));
                ui.add_space(14.0);
                for (i, (title, body)) in STEPS.iter().enumerate() {
                    ui.horizontal_top(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::hover());
                        ui.painter().circle_filled(r.center(), 13.0, theme::accent_faded());
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            format!("{}", i + 1),
                            egui::FontId::new(12.0, theme::bold_family()),
                            theme::accent_col(),
                        );
                        ui.vertical(|ui| {
                            ui.set_max_width(480.0);
                            ui.label(theme::strong(*title).size(13.5));
                            ui.label(theme::muted(*body).size(12.0));
                        });
                    });
                    ui.add_space(8.0);
                }
                ui.add_space(4.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::kit::primary_button(ui, "Got it").clicked() {
                        close = true;
                    }
                });
            });
        if close || modal.should_close() {
            self.help_open = false;
        }
    }

    // ---------- backup / restore ----------

    pub fn backup_now(&mut self) {
        let dest = crate::platform::save_path(
            &format!("vershion-backup-{}.zip", fmt_date(now_secs())),
            "verSHion backups",
            &["zip"],
        );
        let Some(dest) = dest else { return };
        match write_backup(&dest) {
            Ok(()) => self.toast(format!("Backup saved: {}", dest.display())),
            Err(e) => self.toast_err(format!("Backup failed: {e}")),
        }
    }

    pub fn start_restore(&mut self) {
        let Some(path) = crate::platform::pick_file("verSHion backups", &["zip"])
        else {
            return;
        };
        self.confirm_restore = Some(path);
    }

    pub fn draw_restore_confirm(&mut self, ctx: &egui::Context) {
        let Some(path) = self.confirm_restore.clone() else { return };
        let mut action: u8 = 0; // 1 = dismissed, 2 = replace
        let modal = egui::Modal::new(egui::Id::new("vershion_restore"))
            .frame(crate::kit::card().inner_margin(Margin::same(22)))
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.label(theme::serif("Restore from backup?").size(22.0));
                ui.add_space(6.0);
                ui.label(theme::text(
                    "This replaces ALL your banks, exams, imports and exports with the backup. Continue?",
                ));
                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(theme::on_accent("Replace").family(theme::bold_family()))
                                .fill(theme::red())
                                .corner_radius(CornerRadius::same(255))
                                .min_size(Vec2::new(0.0, 34.0)),
                        )
                        .clicked()
                    {
                        action = 2;
                    }
                    if crate::kit::ghost_button(ui, "Cancel").clicked() {
                        action = 1;
                    }
                });
            });
        if modal.should_close() && action == 0 {
            action = 1;
        }
        if action != 0 {
            self.confirm_restore = None;
            if action == 2 {
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

    // ---------- demo data (VERSHION_SEED) ----------

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
            details: Default::default(),
        };
        let bank_id = bank.id.clone();
        self.store.banks.push(bank);
        let exam = Exam { paper_style: Default::default(), margins: None,
            id: model::new_id(),
            name: "Solar System — Quiz".into(),
            header: "Name: __________________ Class: ___________ Date: ___________".into(),
            title: Some("Solar System Quiz".into()),
            items: vec![
                ExamItem::new(bank_id.clone(), q1_id),
                ExamItem::new(bank_id, q3_id),
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
// ---------- V4 home helpers (painter-drawn cards, tiles, lists) ----------

struct ExamRow {
    id: Id,
    name: String,
    qs: usize,
    pts: f64,
    bank: String,
    edited: i64,
}

struct BankRow {
    id: Id,
    name: String,
    qs: usize,
    topics: usize,
    edited: i64,
    mix: Vec<f32>,
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"];

/// "2 hours ago" / "Yesterday" / "Oct 9".
fn rel_time(secs: i64) -> String {
    if secs <= 0 {
        return "Not yet".to_string();
    }
    let d = (now_secs() - secs).max(0);
    if d < 60 {
        "Just now".to_string()
    } else if d < 3600 {
        format!("{} min ago", d / 60)
    } else if d < 86400 {
        plural((d / 3600) as usize, "hour") + " ago"
    } else if d < 2 * 86400 {
        "Yesterday".to_string()
    } else {
        let (_, m, day) = crate::time::civil_from_days(secs.div_euclid(86400));
        format!("{} {}", MONTHS[(m as usize).clamp(1, 12) - 1], day)
    }
}

/// "Tuesday, 4:12 pm" within the last week, else "Oct 9, 4:12 pm".
fn fmt_when(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400);
    let (h, mi) = (tod / 3600, (tod % 3600) / 60);
    let (h12, ap) = (if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "am" } else { "pm" });
    let clock = format!("{h12}:{mi:02} {ap}");
    if now_secs() - secs < 7 * 86400 {
        format!("{}, {clock}", WEEKDAYS[days.rem_euclid(7) as usize])
    } else {
        let (_, m, d) = crate::time::civil_from_days(days);
        format!("{} {d}, {clock}", MONTHS[(m as usize).clamp(1, 12) - 1])
    }
}

fn grid_cols(avail: f32, min_w: f32, gap: f32, max: usize) -> usize {
    (((avail + gap) / (min_w + gap)).floor() as usize).clamp(1, max)
}

fn fmt_pts(p: f64) -> String {
    if (p - p.round()).abs() < 0.05 { format!("{}", p.round() as i64) } else { format!("{p:.1}") }
}

/// Single-row, ellipsised text painted with its top-left at `pos`.
fn paint_fit(p: &egui::Painter, pos: Pos2, text: &str, font: egui::FontId, col: Color32, width: f32) {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::text::TextFormat { font_id: font, color: col, ..Default::default() },
    );
    job.wrap.max_width = width.max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let g = p.layout_job(job);
    p.galley(pos, g, col);
}

fn sans(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Proportional)
}
fn bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, theme::bold_family())
}
fn serif_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, theme::serif_family())
}

fn hover_border(resp: &egui::Response) -> Stroke {
    if resp.hovered() { Stroke::new(1.0, theme::accent_col()) } else { Stroke::new(1.0, theme::border()) }
}

/// One of the three big action tiles. Returns true when clicked.
fn home_tile(ui: &mut egui::Ui, w: f32, icon: NavIcon, title: &str, sub: &str, primary: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 72.0), egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let p = ui.painter();
    let (fill, stroke, fg, glyph_bg, glyph_fg) = if primary {
        let gbg = if theme::is_dark() { Color32::from_black_alpha(36) } else { Color32::from_white_alpha(46) };
        (theme::accent_col(), Stroke::new(1.0, theme::accent_col()), theme::on_accent_col(), gbg, theme::on_accent_col())
    } else {
        (theme::card(), hover_border(&resp), theme::ink(), theme::accent_faded(), theme::accent_col())
    };
    p.rect(rect, CornerRadius::same(18), fill, stroke, StrokeKind::Inside);
    let g = Rect::from_min_size(rect.min + Vec2::new(18.0, 15.0), Vec2::splat(42.0));
    p.rect_filled(g, CornerRadius::same(13), glyph_bg);
    draw_nav_icon_k(p, icon, g.center(), glyph_fg, 1.3);
    let tx = rect.min.x + 18.0 + 42.0 + 14.0;
    let tw = rect.right() - tx - 10.0;
    paint_fit(p, Pos2::new(tx, rect.min.y + 15.0), title, serif_font(19.0), fg, tw);
    let subc = if primary { fg.gamma_multiply(0.9) } else { theme::muted_col() };
    paint_fit(p, Pos2::new(tx, rect.min.y + 40.0), sub, sans(12.0), subc, tw);
    resp.clicked()
}

fn rot(c: Pos2, v: Vec2, a: f32) -> Pos2 {
    let (s, co) = a.sin_cos();
    c + Vec2::new(v.x * co - v.y * s, v.x * s + v.y * co)
}

/// Small tilted sheet of paper with faint lines (and an optional highlighter bar).
fn paper_thumb(p: &egui::Painter, c: Pos2, size: Vec2, deg: f32, hl: bool, shadow: bool) {
    let a = deg.to_radians();
    let h = size / 2.0;
    let corners = |off: Vec2| -> Vec<Pos2> {
        [Vec2::new(-h.x, -h.y), Vec2::new(h.x, -h.y), Vec2::new(h.x, h.y), Vec2::new(-h.x, h.y)]
            .iter()
            .map(|v| rot(c + off, *v, a))
            .collect()
    };
    let s = size.y / 90.0;
    if shadow {
        p.add(egui::Shape::convex_polygon(corners(Vec2::new(0.0, 6.0 * s.max(0.5))), Color32::from_black_alpha(34), Stroke::NONE));
    }
    p.add(egui::Shape::convex_polygon(
        corners(Vec2::ZERO),
        Color32::from_rgb(0xff, 0xfd, 0xf8),
        Stroke::new(1.0, Color32::from_rgb(0xcf, 0xc9, 0xb6)),
    ));
    let pad = 8.0 * s.max(0.5);
    let inner_w = size.x - pad * 1.6;
    let step = 8.0 * s;
    let thin = Stroke::new((2.0 * s).max(1.0), Color32::from_rgb(0xd3, 0xcb, 0xb8));
    for i in 0..7usize {
        let y = -h.y + pad + 2.0 + i as f32 * step;
        let (w, stroke) = match i {
            0 => (inner_w * 0.58, Stroke::new((2.0 * s).max(1.0), Color32::from_rgb(0x9d, 0x8c, 0x7b))),
            2 if hl => (inner_w * 0.78, Stroke::new((4.0 * s).max(1.5), theme::highlighter())),
            _ => (inner_w, thin),
        };
        let x0 = if i == 0 { -w / 2.0 } else { -inner_w / 2.0 };
        p.line_segment([rot(c, Vec2::new(x0, y), a), rot(c, Vec2::new(x0 + w, y), a)], stroke);
    }
}

fn paint_preview_bg(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, CornerRadius { nw: 16, ne: 16, sw: 0, se: 0 }, theme::accent_faded());
    let clip = p.with_clip_rect(r);
    let mut x = r.left() + 8.0;
    while x < r.right() {
        clip.line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], Stroke::new(1.0, theme::grid_line()));
        x += 16.0;
    }
    let mut y = r.top() + 8.0;
    while y < r.bottom() {
        clip.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], Stroke::new(1.0, theme::grid_line()));
        y += 16.0;
    }
    p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, theme::border()));
}

/// Small-caps caption over a value, painted; returns nothing.
fn paint_fact(p: &egui::Painter, pos: Pos2, w: f32, label: &str, value: &str) {
    paint_fit(p, pos, &label.to_uppercase(), bold(9.5), theme::muted_col(), w);
    paint_fit(p, pos + Vec2::new(0.0, 12.0), value, bold(13.0), theme::ink(), w);
}

/// Where a Home exam card's page thumbnail goes in its preview band `pv`: the Exams screen's
/// thumbnail size, centred (smaller only when the card is too narrow).
fn home_thumb_area(pv: Rect) -> Rect {
    Rect::from_center_size(pv.center(), crate::examui::CARD_THUMB.min(pv.shrink2(Vec2::new(10.0, 9.0)).size()))
}

fn exam_card(ui: &mut egui::Ui, app: &mut crate::App, w: f32, e: &ExamRow, tilt_right: bool) -> bool {
    // The preview band holds the first page at the Exams screen's thumbnail size.
    let band_h = crate::examui::CARD_THUMB.y + 18.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, band_h + 124.0), egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(16), theme::card(), hover_border(&resp), StrokeKind::Inside);
    let pv = Rect::from_min_size(rect.min + Vec2::splat(1.0), Vec2::new(w - 2.0, band_h));
    paint_preview_bg(p, pv);
    // The exam's real first page; the generic sheet only while it has no questions.
    let ctx = ui.ctx().clone();
    let page = crate::examui::paint_exam_thumb(app, &ctx, p, home_thumb_area(pv), &e.id);
    let p = ui.painter();
    if page.is_none() {
        paper_thumb(p, pv.center(), Vec2::new(70.0, 90.0), if tilt_right { 3.0 } else { -3.0 }, e.id.len() % 2 == 0, true);
    }
    let x = rect.min.x + 14.0;
    let iw = w - 28.0;
    paint_fit(p, Pos2::new(x, pv.bottom() + 11.0), &e.name, serif_font(16.0), theme::ink(), iw);
    let y1 = pv.bottom() + 42.0;
    let half = (iw - 10.0) / 2.0;
    paint_fact(p, Pos2::new(x, y1), half, "Questions", &e.qs.to_string());
    paint_fact(p, Pos2::new(x + half + 10.0, y1), half, "Points", &fmt_pts(e.pts));
    paint_fact(p, Pos2::new(x, y1 + 36.0), half, "Bank", &e.bank);
    paint_fact(p, Pos2::new(x + half + 10.0, y1 + 36.0), half, "Edited", &rel_time(e.edited));
    resp.clicked()
}

fn list_columns(w: f32, fr: &[f32]) -> Vec<(f32, f32)> {
    // First column is the fixed 56 px thumb/icon column; the rest share the space.
    let gap = 12.0;
    let fixed_end = 8.0 + 56.0;
    let total: f32 = fr.iter().sum();
    let avail = (w - fixed_end - 14.0 - gap * fr.len() as f32).max(50.0);
    let mut x = fixed_end + gap;
    let mut out = vec![(8.0, 56.0)];
    for f in fr {
        let cw = avail * f / total;
        out.push((x, cw));
        x += cw + gap;
    }
    out
}

fn list_header(p: &egui::Painter, rect: Rect, cols: &[(f32, f32)], names: &[&str]) {
    for ((x, w), n) in cols.iter().skip(1).zip(names) {
        paint_fit(p, Pos2::new(rect.min.x + x, rect.min.y + 8.0), &n.to_uppercase(), bold(10.0), theme::muted_col(), *w);
    }
    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, theme::border()));
}

fn exam_list(ui: &mut egui::Ui, rows: &[&ExamRow]) -> Option<Id> {
    let w = ui.available_width();
    let cols = list_columns(w, &[2.2, 0.8, 0.8, 1.4, 1.0]);
    let (outer, _) = ui.allocate_exact_size(Vec2::new(w, 30.0 + 54.0 * rows.len() as f32), egui::Sense::hover());
    ui.painter().rect(outer, CornerRadius::same(16), theme::card(), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
    let head = Rect::from_min_size(outer.min, Vec2::new(w, 30.0));
    list_header(ui.painter(), head, &cols, &["Name", "Questions", "Points", "Bank", "Edited"]);
    let mut hit = None;
    for (i, e) in rows.iter().enumerate() {
        let r = Rect::from_min_size(outer.min + Vec2::new(1.0, 30.0 + 54.0 * i as f32), Vec2::new(w - 2.0, 54.0));
        let resp = ui.interact(r, ui.id().with(("exl", i)), egui::Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(r, CornerRadius::ZERO, theme::hover_bg());
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if i + 1 < rows.len() {
            p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, theme::border()));
        }
        paper_thumb(p, Pos2::new(r.min.x + 8.0 + 28.0, r.center().y), Vec2::new(30.0, 38.0), 0.0, false, true);
        let cy = r.center().y;
        let at = |c: usize| Pos2::new(outer.min.x + cols[c].0, cy);
        paint_fit(p, at(1) - Vec2::new(0.0, 9.0), &e.name, serif_font(15.0), theme::ink(), cols[1].1);
        paint_fit(p, at(2) - Vec2::new(0.0, 8.0), &e.qs.to_string(), bold(13.0), theme::ink(), cols[2].1);
        paint_fit(p, at(3) - Vec2::new(0.0, 8.0), &fmt_pts(e.pts), bold(13.0), theme::ink(), cols[3].1);
        paint_fit(p, at(4) - Vec2::new(0.0, 8.0), &e.bank, sans(13.0), theme::text_col(), cols[4].1);
        paint_fit(p, at(5) - Vec2::new(0.0, 8.0), &rel_time(e.edited), sans(13.0), theme::text_col(), cols[5].1);
        if resp.clicked() {
            hit = Some(e.id.clone());
        }
    }
    hit
}

/// Thin segmented question-type bar; shares are fractions, shades fade out.
fn mix_bar(p: &egui::Painter, r: Rect, mix: &[f32]) {
    p.rect_filled(r, CornerRadius::same(3), theme::accent_faded());
    let total: f32 = mix.iter().sum::<f32>().max(0.001);
    let n = mix.len();
    let mut x = r.left();
    for (i, f) in mix.iter().enumerate() {
        let seg_w = r.width() * f / total;
        let alpha = match i {
            0 => 1.0,
            1 => 0.6,
            2 => 0.35,
            _ => 0.2,
        };
        let gap = if i + 1 < n { 2.0 } else { 0.0 };
        let seg = Rect::from_min_size(Pos2::new(x, r.top()), Vec2::new((seg_w - gap).max(1.0), r.height()));
        p.rect_filled(seg, CornerRadius::same(3), theme::accent_col().gamma_multiply(alpha));
        x += seg_w;
    }
}

fn paint_bank_glyph(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, CornerRadius::same(10), theme::accent_faded());
    draw_nav_icon_k(p, NavIcon::Banks, r.center(), theme::accent_col(), 1.05);
}

fn bank_card(ui: &mut egui::Ui, w: f32, b: &BankRow) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 148.0), egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(16), theme::card(), hover_border(&resp), StrokeKind::Inside);
    let g = Rect::from_min_size(rect.min + Vec2::new(14.0, 13.0), Vec2::splat(34.0));
    paint_bank_glyph(p, g);
    let x = rect.min.x + 14.0 + 34.0 + 11.0;
    let iw = rect.right() - x - 14.0;
    paint_fit(p, Pos2::new(x, rect.min.y + 13.0), &b.name, serif_font(15.0), theme::ink(), iw);
    let half = (iw - 10.0) / 2.0;
    paint_fact(p, Pos2::new(x, rect.min.y + 42.0), half, "Questions", &b.qs.to_string());
    paint_fact(p, Pos2::new(x + half + 10.0, rect.min.y + 42.0), half, "Topics", &b.topics.to_string());
    paint_fact(p, Pos2::new(x, rect.min.y + 78.0), iw, "Edited", &rel_time(b.edited));
    mix_bar(p, Rect::from_min_size(Pos2::new(x, rect.min.y + 122.0), Vec2::new(iw, 5.0)), &b.mix);
    resp.clicked()
}

fn bank_list(ui: &mut egui::Ui, rows: &[&BankRow]) -> Option<Id> {
    let w = ui.available_width();
    let mut cols = list_columns(w - 90.0 - 12.0, &[2.2, 0.9, 0.8, 1.0]);
    cols.push((w - 14.0 - 90.0 - 0.0, 90.0));
    let (outer, _) = ui.allocate_exact_size(Vec2::new(w, 30.0 + 54.0 * rows.len() as f32), egui::Sense::hover());
    ui.painter().rect(outer, CornerRadius::same(16), theme::card(), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
    let head = Rect::from_min_size(outer.min, Vec2::new(w, 30.0));
    list_header(ui.painter(), head, &cols, &["Name", "Questions", "Topics", "Edited", "Types"]);
    let mut hit = None;
    for (i, b) in rows.iter().enumerate() {
        let r = Rect::from_min_size(outer.min + Vec2::new(1.0, 30.0 + 54.0 * i as f32), Vec2::new(w - 2.0, 54.0));
        let resp = ui.interact(r, ui.id().with(("bkl", i)), egui::Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(r, CornerRadius::ZERO, theme::hover_bg());
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if i + 1 < rows.len() {
            p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, theme::border()));
        }
        let cy = r.center().y;
        paint_bank_glyph(p, Rect::from_center_size(Pos2::new(r.min.x + 8.0 + 28.0, cy), Vec2::splat(34.0)));
        let at = |c: usize| Pos2::new(outer.min.x + cols[c].0, cy);
        paint_fit(p, at(1) - Vec2::new(0.0, 9.0), &b.name, serif_font(15.0), theme::ink(), cols[1].1);
        paint_fit(p, at(2) - Vec2::new(0.0, 8.0), &b.qs.to_string(), bold(13.0), theme::ink(), cols[2].1);
        paint_fit(p, at(3) - Vec2::new(0.0, 8.0), &b.topics.to_string(), sans(13.0), theme::text_col(), cols[3].1);
        paint_fit(p, at(4) - Vec2::new(0.0, 8.0), &rel_time(b.edited), sans(13.0), theme::text_col(), cols[4].1);
        mix_bar(p, Rect::from_min_size(Pos2::new(outer.min.x + cols[5].0, cy - 2.5), Vec2::new(cols[5].1, 5.0)), &b.mix);
        if resp.clicked() {
            hit = Some(b.id.clone());
        }
    }
    hit
}

/// Printer glyph for the last-export strip.
fn draw_printer_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    let s = Stroke::new(1.7, col);
    p.rect_stroke(Rect::from_center_size(c + Vec2::new(0.0, 1.0), Vec2::new(14.0, 7.0)), CornerRadius::same(2), s, StrokeKind::Middle);
    p.rect_stroke(Rect::from_min_max(c + Vec2::new(-4.0, -7.0), c + Vec2::new(4.0, -2.5)), CornerRadius::same(1), s, StrokeKind::Middle);
    p.rect_stroke(Rect::from_min_max(c + Vec2::new(-4.0, 2.0), c + Vec2::new(4.0, 8.0)), CornerRadius::same(1), s, StrokeKind::Middle);
}

pub(crate) fn remap_empty_bank_ids(exams: &mut [Exam], bank_id: &str) {
    for exam in exams.iter_mut() {
        for item in exam.items.iter_mut() {
            if item.bank_id.is_empty() {
                item.bank_id = bank_id.to_string();
            }
        }
    }
}

pub(crate) fn mime_of(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => "image/png",
    }
}

/// Rasterizes every page of a PDF (up to `MAX_PDF_PAGES`) to PNG with `pdftoppm`.
pub(crate) fn pdf_pages_png(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    use std::process::{Command, Stdio};
    const MAX_PDF_PAGES: usize = 30;

    let dir = std::env::temp_dir().join(format!("vershion-pdf-{}", now_ms()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src = dir.join("in.pdf");
    std::fs::write(&src, data).map_err(|e| e.to_string())?;
    let out = Command::new("pdftoppm")
        .args(["-png", "-r", "150", "-l", &MAX_PDF_PAGES.to_string()])
        .arg(&src)
        .arg(dir.join("page"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output();
    let result = match out {
        Ok(o) if o.status.success() => {
            let mut files: Vec<_> = std::fs::read_dir(&dir)
                .map(|d| d.flatten().map(|e| e.path()).collect())
                .unwrap_or_default();
            files.retain(|f| f.extension().is_some_and(|e| e == "png"));
            files.sort(); // pdftoppm zero-pads the page number, so name order is page order
            let pages: Vec<Vec<u8>> = files.iter().filter_map(|f| std::fs::read(f).ok()).collect();
            if pages.is_empty() { Err("pdftoppm produced no pages".to_string()) } else { Ok(pages) }
        }
        Ok(o) => Err(format!("pdftoppm failed: {}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(
            "PDF rasterizer (pdftoppm) not found — export the pages as images instead".to_string(),
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

    /// Home's recent-exam cards show the first page as large as the Exams screen does.
    #[test]
    fn home_exam_thumbnail_matches_the_exams_screen() {
        let band = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(300.0, crate::examui::CARD_THUMB.y + 18.0));
        let a = home_thumb_area(band);
        assert_eq!(a.size(), crate::examui::CARD_THUMB);
        assert!((a.center() - band.center()).length() < 1e-3);
        // a very narrow card shrinks it rather than overflowing
        let narrow = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, band.height()));
        assert!(home_thumb_area(narrow).width() <= 80.0 + 1e-3);
    }

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
    fn backup_never_includes_or_restores_the_key_file() {
        // Restore guard refuses it, so a malicious/old zip can't clobber saved keys.
        assert!(!sanitize_backup_entry("ai-keys.json"));
        assert!(!sanitize_backup_entry("ai-keys.json.tmp"));
        // Backup writer works from a fixed file list that must not name it.
        let src = include_str!("screens.rs");
        let a = src.find("fn write_backup").unwrap();
        let body = &src[a..a + 900];
        assert!(!body.contains(concat!("ai-", "keys")));
    }

    #[test]
    fn package_exam_items_remap_onto_the_new_bank() {
        let bank_id = "b123".to_string();
        let mut exams = vec![
            Exam { paper_style: Default::default(), margins: None,
                id: "e1".into(),
                name: "Quiz".into(),
                header: String::new(),
                title: None,
                items: vec![
                    ExamItem::new("", "q1"),
                    ExamItem::new("other-bank", "q2"),
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
/// Padlock glyph for the dock's "Saved on this computer" note.
fn draw_lock(painter: &egui::Painter, c: Pos2, col: Color32) {
    let body = Rect::from_center_size(c + Vec2::new(0.0, 3.0), Vec2::new(14.0, 10.0));
    painter.rect_stroke(body, CornerRadius::same(2), Stroke::new(1.5, col), StrokeKind::Middle);
    let sc = Pos2::new(c.x, body.top());
    painter.line_segment([sc + Vec2::new(-3.5, 0.0), sc + Vec2::new(-3.5, -3.0)], Stroke::new(1.5, col));
    painter.line_segment([sc + Vec2::new(3.5, 0.0), sc + Vec2::new(3.5, -3.0)], Stroke::new(1.5, col));
    painter.line_segment([sc + Vec2::new(-3.5, -3.0), sc + Vec2::new(3.5, -3.0)], Stroke::new(1.5, col));
}
