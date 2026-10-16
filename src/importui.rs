//! V4 unified Import screen: one drop area for pasted text, files and photos; the
//! format is detected automatically (`formats::detect_and_parse`, image sniffing,
//! Word text extraction). Photos/PDFs are the only thing that needs the AI, and only
//! when the user presses Convert. A review panel groups the questions by source and
//! files them into an existing or new bank. The import history stays below.

use std::cell::RefCell;
use std::collections::HashSet;

use eframe::egui;
use egui::{Align, Color32, CornerRadius, FontId, Layout, Margin, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};

use crate::formats::ImportDoc;
use crate::kit;
use crate::model::{self, Id, Import, QKind, Question, QuestionBank};
use crate::screens::{fmt_time, import_how_label, mime_of, now_secs, pdf_pages_png, remap_empty_bank_ids, shell_frame, ImportFile};
use crate::theme;

// ---------- screen state (UI-only, never persisted) ----------

#[derive(Clone, PartialEq)]
enum Status {
    Ready,
    /// Photo / PDF page waiting for the AI; carries the last failure, if any.
    NeedsAi(Option<String>),
    Converting,
}

#[derive(Clone, Copy, PartialEq)]
enum Ic {
    Upload,
    File,
    Clip,
    Camera,
    Sparkle,
    Lock,
}

struct Source {
    name: String,
    kind: String,
    how: &'static str,
    icon: Ic,
    status: Status,
    doc: Option<ImportDoc>,
    sel: Vec<bool>,
    image: Option<ImportFile>,
    /// New AI rows get the highlighter edge.
    fresh: bool,
    expanded: bool,
}

#[derive(Default)]
struct State {
    sources: Vec<Source>,
    inited: bool,
    paste_open: bool,
    /// None = a new bank; Some = append to this bank.
    dest: Option<Id>,
    new_name: String,
    points: f64,
    skip_dups: bool,
    /// "Convert with AI" pressed: keep feeding photos to the model one by one.
    ai_all: bool,
}

thread_local! {
    static ST: RefCell<State> = RefCell::new(State::default());
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    ST.with(|s| f(&mut s.borrow_mut()))
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn short_kind(q: &Question) -> &'static str {
    match q.kind.qkind() {
        QKind::Mc => "MC",
        QKind::Tf => "T/F",
        QKind::Matching => "MATCH",
        QKind::Blank => "FILL",
        QKind::Short => "SHORT",
        QKind::Multipart => "MULTI",
    }
}

fn fmt_pts(p: f64) -> String {
    if p.fract() == 0.0 {
        format!("{} pt{}", p as i64, if p as i64 == 1 { "" } else { "s" })
    } else {
        format!("{p:.1} pts")
    }
}

// ---------- reading sources ----------

/// Plain text of a .docx: paragraphs become lines, empty paragraphs keep the blank
/// lines the Blackboard grammar uses between questions.
fn docx_text(bytes: &[u8]) -> Result<String, String> {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "that Word file couldn't be opened (is it a .docx?)".to_string())?;
    let mut xml = String::new();
    z.by_name("word/document.xml")
        .map_err(|_| "that Word file has no document body".to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;
    let xml = xml
        .replace("</w:p>", "\n")
        .replace("<w:br/>", "\n")
        .replace("<w:tab/>", " ");
    let mut out = String::with_capacity(xml.len() / 2);
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    Ok(out
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&"))
}

fn text_kind(name: &str, ext: &str, text: &str) -> &'static str {
    if ext == "csv" {
        "Spreadsheet (CSV)"
    } else if ext == "json" || text.trim_start().starts_with('{') {
        "Test Parrot file"
    } else if text.lines().any(|l| l.trim_start().to_ascii_uppercase().starts_with("ANSWER:"))
        && !name.is_empty()
    {
        "Aiken format"
    } else {
        "Blackboard test-generator format"
    }
}

impl State {
    fn push_doc(&mut self, name: &str, kind: &str, how: &'static str, icon: Ic, doc: ImportDoc) {
        let n = doc.questions.len();
        self.sources.push(Source {
            name: name.to_string(),
            kind: kind.to_string(),
            how,
            icon,
            status: Status::Ready,
            doc: Some(doc),
            sel: vec![true; n],
            image: None,
            fresh: false,
            expanded: false,
        });
    }

    fn push_photo(&mut self, name: &str, kind: &str, mime: &str, bytes: Vec<u8>) {
        self.sources.push(Source {
            name: name.to_string(),
            kind: kind.to_string(),
            how: "ai",
            icon: Ic::Camera,
            status: Status::NeedsAi(None),
            doc: None,
            sel: Vec::new(),
            image: Some(ImportFile { name: name.to_string(), mime: mime.to_string(), bytes }),
            fresh: false,
            expanded: false,
        });
    }
}

/// Detects what `bytes` is and adds it as a source (or toasts why not).
fn add_bytes(app: &mut crate::App, st: &mut State, name: &str, bytes: Vec<u8>, how: &'static str) {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" => {
            st.push_photo(name, "Photo", mime_of(&ext), bytes);
            return;
        }
        "pdf" => {
            match pdf_pages_png(&bytes) {
                Ok(pages) => {
                    let n = pages.len();
                    for (i, png) in pages.into_iter().enumerate() {
                        let label = if n == 1 { name.to_string() } else { format!("{name} · page {}", i + 1) };
                        st.push_photo(&label, "PDF page", "image/png", png);
                    }
                }
                Err(e) => app.toast_err(e),
            }
            return;
        }
        "doc" => {
            app.toast_err(format!("{name}: old .doc files can't be read. Save it as .docx and add it again."));
            return;
        }
        _ => {}
    }
    let (text_bytes, kind, icon): (Vec<u8>, String, Ic) = if ext == "docx" {
        match docx_text(&bytes) {
            Ok(t) => (t.into_bytes(), "Word document".to_string(), Ic::File),
            Err(e) => {
                app.toast_err(format!("Couldn't read {name}: {e}"));
                return;
            }
        }
    } else if ext.is_empty() && image::guess_format(&bytes).is_ok() {
        st.push_photo(name, "Photo", "image/png", bytes);
        return;
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4000)]).to_string();
        let k = text_kind(name, &ext, &head).to_string();
        (bytes, k, if how == "paste" { Ic::Clip } else { Ic::File })
    };
    match crate::formats::detect_and_parse(&text_bytes, name) {
        Ok(doc) if doc.questions.is_empty() => {
            app.toast_err(format!("Couldn't find any questions in {name}."))
        }
        Ok(doc) => st.push_doc(name, &kind, how, icon, doc),
        Err(e) => app.toast_err(format!("Couldn't read {name}: {e}")),
    }
}

fn add_paths(app: &mut crate::App, st: &mut State, paths: Vec<std::path::PathBuf>) {
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        match std::fs::read(&path) {
            Ok(b) => add_bytes(app, st, &name, b, "file"),
            Err(e) => app.toast_err(format!("Couldn't read {name}: {e}")),
        }
    }
}

// ---------- AI hand-off ----------

/// Truthful description of where a photo goes: (destination text, leaves_computer).
fn provider_label(s: &crate::Settings) -> (String, bool) {
    let cfg = crate::ai::AiCfg::from(s);
    match cfg.provider.as_str() {
        "cloud" => (format!("Ollama Cloud · {}", cfg.model), true),
        "openrouter" => (format!("OpenRouter · {}", cfg.model), true),
        "custom" => (format!("{} · {}", cfg.base, cfg.model), true),
        _ => (format!("Ollama on this computer · {}", cfg.model), false),
    }
}

impl crate::App {
    /// Kicks off the AI conversion of the staged file (background thread in ai.rs).
    pub fn begin_ai_convert(&mut self, ctx: &egui::Context) {
        let Some(f) = self.import_file.clone() else {
            self.toast_err("Choose a file first.");
            return;
        };
        let cfg = crate::ai::AiCfg::from(&self.settings.clone());
        self.ai_busy = true;
        let page_no = f.name.rsplit_once("page ").and_then(|(_, n)| n.trim().parse().ok()).unwrap_or(1);
        self.ai.send_image(f.bytes, &f.mime, self.ai_hint.clone(), page_no, &cfg, ctx);
    }

    /// Drains the background AI job into the matching source (never auto-imports).
    pub fn poll_ai(&mut self) {
        if !self.ai_busy {
            return;
        }
        let Some(res) = self.ai.poll() else { return };
        self.ai_busy = false;
        let name = self.import_file.take().map(|f| f.name).unwrap_or_default();
        let mut notes: Vec<String> = Vec::new();
        let outcome: Result<ImportDoc, String> = match res {
            Ok(c) => {
                notes = c.warnings;
                if c.cropped > 0 {
                    notes.insert(0, format!("{} figure{} cropped from the page, {} redrawn as editable Figures-tab figures.", c.cropped, if c.cropped == 1 { "" } else { "s" }, c.redrawn));
                }
                if c.doc.questions.is_empty() { Err("The AI found no questions in it.".to_string()) } else { Ok(c.doc) }
            }
            Err(e) => Err(format!("The AI couldn't read that: {e}")),
        };
        let mut err = None;
        with_state(|st| {
            let Some(s) = st.sources.iter_mut().find(|s| s.status == Status::Converting && s.name == name)
            else {
                return;
            };
            match outcome {
                Ok(doc) => {
                    s.sel = vec![true; doc.questions.len()];
                    s.doc = Some(doc);
                    s.status = Status::Ready;
                    s.fresh = true;
                    s.kind = "Photo, read by AI".to_string();
                    s.image = None;
                }
                Err(e) => {
                    s.status = Status::NeedsAi(Some(e.clone()));
                    st.ai_all = false;
                    err = Some(e);
                }
            }
        });
        if let Some(e) = err {
            self.toast_err(e);
        } else if let Some(n) = notes.iter().find(|n| !n.contains("cropped from the page")) {
            self.toast_err(format!("{} (+{} more notes)", n, notes.len().saturating_sub(1)));
        }
    }
}

fn start_next_ai(app: &mut crate::App, st: &mut State, ctx: &egui::Context) {
    let Some(s) = st.sources.iter_mut().find(|s| matches!(s.status, Status::NeedsAi(_))) else {
        st.ai_all = false;
        return;
    };
    let cfg = crate::ai::AiCfg::from(&app.settings);
    if !cfg.has_key_or_needs_none() {
        st.ai_all = false;
        app.toast_err("Add your API key in Settings, then AI, before converting.");
        app.screen = crate::Screen::Settings;
        return;
    }
    s.status = Status::Converting;
    app.import_file = s.image.clone();
    app.import_name = s.name.clone();
    app.begin_ai_convert(ctx);
}

// ---------- painter icons ----------

fn paint_icon(p: &Painter, ic: Ic, c: Pos2, col: Color32, k: f32) {
    let w = Stroke::new(1.6, col);
    let pt = |x: f32, y: f32| c + Vec2::new(x, y) * k;
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| pt(x, y)).collect(), w));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(Shape::closed_line(pts.iter().map(|&(x, y)| pt(x, y)).collect(), w));
    };
    let rrect = |x0: f32, y0: f32, x1: f32, y1: f32, r: u8| {
        p.rect_stroke(Rect::from_min_max(pt(x0, y0), pt(x1, y1)), CornerRadius::same(r), w, StrokeKind::Middle);
    };
    match ic {
        Ic::Upload => {
            line(&[(0.0, 5.0), (0.0, -7.0)]);
            line(&[(-4.0, -3.0), (0.0, -7.0), (4.0, -3.0)]);
            line(&[(-7.0, 3.0), (-7.0, 8.0), (7.0, 8.0), (7.0, 3.0)]);
        }
        Ic::File => {
            line(&[(2.0, -8.0), (-5.0, -8.0), (-5.0, 8.0), (6.0, 8.0), (6.0, -4.0), (2.0, -8.0), (2.0, -4.0), (6.0, -4.0)]);
        }
        Ic::Clip => {
            rrect(-6.0, -5.0, 6.0, 8.0, 2);
            rrect(-3.0, -8.0, 3.0, -4.0, 1);
        }
        Ic::Camera => {
            line(&[(-8.0, -3.0), (-8.0, 7.0), (8.0, 7.0), (8.0, -3.0), (4.0, -3.0), (2.5, -6.0), (-2.5, -6.0), (-4.0, -3.0), (-8.0, -3.0)]);
            p.circle_stroke(pt(0.0, 2.0), 3.5 * k, w);
        }
        Ic::Sparkle => closed(&[
            (0.0, -8.0), (1.8, -1.8), (8.0, 0.0), (1.8, 1.8), (0.0, 8.0), (-1.8, 1.8), (-8.0, 0.0), (-1.8, -1.8),
        ]),
        Ic::Lock => {
            rrect(-6.0, -1.0, 6.0, 7.0, 2);
            line(&[(-3.5, -1.0), (-3.5, -3.5), (-1.8, -5.5), (1.8, -5.5), (3.5, -3.5), (3.5, -1.0)]);
        }
    }
}

fn draw_cb(p: &Painter, rect: Rect, on: bool, hot: bool) {
    if on {
        p.rect_filled(rect, CornerRadius::same(6), theme::accent_col());
        let s = Stroke::new(2.0, theme::on_accent_col());
        let c = rect.center();
        p.line_segment([c + Vec2::new(-3.5, 0.5), c + Vec2::new(-1.0, 3.0)], s);
        p.line_segment([c + Vec2::new(-1.0, 3.0), c + Vec2::new(3.5, -2.5)], s);
    } else {
        let col = if hot { theme::accent_col() } else { theme::rule() };
        p.rect_stroke(rect.shrink(0.75), CornerRadius::same(6), Stroke::new(1.5, col), StrokeKind::Middle);
    }
}

/// 17 px tick box; true when clicked.
fn cb(ui: &mut egui::Ui, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(17.0), Sense::click());
    draw_cb(ui.painter(), rect, on, resp.hovered());
    resp.clicked()
}

// ---------- the screen ----------

impl crate::App {
    pub fn draw_imports(&mut self, ctx: &egui::Context) {
        let mut st = ST.with(|s| std::mem::take(&mut *s.borrow_mut()));
        if !st.inited {
            st.inited = true;
            st.skip_dups = true;
            st.points = 1.0;
        }
        intake(self, ctx, &mut st);
        if st.ai_all && !self.ai_busy {
            start_next_ai(self, &mut st, ctx);
        }
        shell_frame(self, ctx, true, |app, ui| page(app, ui, ctx, &mut st));
        ST.with(|s| *s.borrow_mut() = st);
    }
}

/// OS drag-and-drop and Ctrl+V of plain text.
fn intake(app: &mut crate::App, ctx: &egui::Context, st: &mut State) {
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    for f in dropped {
        let path = f.path().to_path_buf();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "dropped file".to_string());
        match f.bytes() {
            Ok(b) => add_bytes(app, st, &name, b, "file"),
            Err(e) => app.toast_err(format!("Couldn't read {name}: {e}")),
        }
    }
    if !st.paste_open && !ctx.egui_wants_keyboard_input() {
        let pasted: Vec<String> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| if let egui::Event::Paste(t) = e { Some(t.clone()) } else { None })
                .collect()
        });
        for t in pasted {
            if !t.trim().is_empty() {
                add_bytes(app, st, "Pasted text", t.into_bytes(), "paste");
            }
        }
    }
}

fn page(app: &mut crate::App, ui: &mut egui::Ui, ctx: &egui::Context, st: &mut State) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let w = ui.available_width().min(1040.0);
        ui.allocate_ui_with_layout(Vec2::new(w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(w);
            kit::page_header(
                ui,
                &["Home", "Import"],
                "Import questions",
                Some("One place for everything: pasted text, files, spreadsheets or a photo of a worksheet. We work out the format for you."),
            );
            ui.add_space(14.0);

            let flags = dup_flags(app, st);
            if w >= 760.0 {
                ui.columns(2, |cols| {
                    left(app, &mut cols[0], ctx, st);
                    right(app, &mut cols[1], st, &flags);
                });
            } else {
                left(app, ui, ctx, st);
                ui.add_space(14.0);
                right(app, ui, st, &flags);
            }

            ui.add_space(26.0);
            history(app, ui);
            ui.add_space(20.0);
        });
    });
}

fn dup_flags(app: &crate::App, st: &State) -> Vec<Vec<bool>> {
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(id) = &st.dest {
        if let Some(b) = app.store.bank(id) {
            for q in &b.questions {
                seen.insert(norm(&q.stem));
            }
        }
    }
    st.sources
        .iter()
        .map(|s| match &s.doc {
            Some(d) => d.questions.iter().map(|q| !seen.insert(norm(&q.stem))).collect(),
            None => Vec::new(),
        })
        .collect()
}

// ----- left column: drop area + sources -----

fn left(app: &mut crate::App, ui: &mut egui::Ui, ctx: &egui::Context, st: &mut State) {
    drop_area(app, ui, st);
    ui.add_space(14.0);
    sources_card(app, ui, ctx, st);
}

fn drop_area(app: &mut crate::App, ui: &mut egui::Ui, st: &mut State) {
    let hovering = ctx_hovering(ui.ctx());
    let fill = if hovering { theme::accent_faded().gamma_multiply(1.4) } else { theme::accent_faded() };
    let inner = egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin { left: 16, right: 16, top: 18, bottom: 14 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(14), theme::card());
                paint_icon(ui.painter(), Ic::Upload, r.center(), theme::accent_col(), 1.25);
                ui.add_space(4.0);
                ui.label(theme::serif("Drop anything here").size(19.0).color(theme::ink()));
                ui.label(theme::text("Files, photos, or paste text straight in.").size(12.5));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - 350.0) / 2.0).max(0.0));
                    if kit::primary_button(ui, "Choose files").clicked() {
                        if let Some(paths) = rfd::FileDialog::new()
                            .add_filter(
                                "Question files",
                                &["txt", "csv", "json", "docx", "pdf", "png", "jpg", "jpeg", "gif", "webp"],
                            )
                            .pick_files()
                        {
                            add_paths(app, st, paths);
                        }
                    }
                    if kit::ghost_button(ui, "Paste text").clicked() {
                        st.paste_open = !st.paste_open;
                    }
                    if kit::ghost_button(ui, "Add photo").clicked() {
                        if let Some(paths) = rfd::FileDialog::new()
                            .add_filter("Photos & PDFs", &["png", "jpg", "jpeg", "gif", "webp", "pdf"])
                            .pick_files()
                        {
                            add_paths(app, st, paths);
                        }
                    }
                });
                if st.paste_open {
                    ui.add_space(8.0);
                    ui.add(
                        egui::TextEdit::multiline(&mut app.paste)
                            .desired_rows(6)
                            .desired_width(ui.available_width())
                            .hint_text(
                                "MC\nWhich planet is closest to the Sun?\nVenus\n*Mercury\nMars\n\nTF\nThe Sun is a star.\nT",
                            ),
                    );
                    ui.label(theme::muted("* marks the correct answer. One blank line between questions.").size(11.5));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - 250.0) / 2.0).max(0.0));
                        let ready = !app.paste.trim().is_empty();
                        if ui.add_enabled_ui(ready, |ui| kit::primary_button(ui, "Add pasted text")).inner.clicked() {
                            let t = std::mem::take(&mut app.paste);
                            add_bytes(app, st, "Pasted text", t.into_bytes(), "paste");
                            st.paste_open = false;
                        }
                        if kit::ghost_button(ui, "Cancel").clicked() {
                            st.paste_open = false;
                        }
                    });
                }
                ui.add_space(8.0);
                ui.label(
                    theme::muted("Blackboard text · Aiken · CSV · .parrot.json · PDF · Word · JPG · PNG").size(11.0),
                );
            });
        });
    // Dashed accent outline (the frame itself has no stroke).
    let r = inner.response.rect.shrink(0.75);
    let stroke = Stroke::new(1.5, theme::accent_col());
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    ui.painter().extend(Shape::dashed_line(&pts, stroke, 7.0, 5.0));
}

fn ctx_hovering(ctx: &egui::Context) -> bool {
    ctx.input(|i| !i.raw.hovered_files.is_empty())
}

fn tag(ui: &mut egui::Ui, status: &Status, s: &Source) {
    match status {
        Status::Ready if s.fresh => {
            let n = s.doc.as_ref().map(|d| d.questions.len()).unwrap_or(0);
            kit::pill(ui, &format!("Read · {}", plural(n, "question")), theme::accent_faded(), theme::ink());
        }
        Status::Ready => {
            kit::pill(ui, "Ready", theme::accent_faded(), theme::ink());
        }
        Status::NeedsAi(_) => kit::marker_pill(ui, "Needs AI"),
        Status::Converting => {
            kit::pill(ui, "Reading…", theme::highlighter(), theme::on_highlighter());
        }
    }
}

fn sources_card(app: &mut crate::App, ui: &mut egui::Ui, ctx: &egui::Context, st: &mut State) {
    kit::card().inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        kit::small_caps(ui, &format!("Added · {}", plural(st.sources.len(), "source")));
        ui.add_space(6.0);
        if st.sources.is_empty() {
            ui.label(theme::muted("Nothing added yet. Drop a file above, or paste some text."));
        }
        let busy = app.ai_busy;
        let mut remove: Option<usize> = None;
        for (i, s) in st.sources.iter().enumerate() {
            if i > 0 {
                let y = ui.cursor().top();
                ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::border()));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let (g, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                ui.painter().rect_filled(g, CornerRadius::same(10), theme::accent_faded());
                paint_icon(ui.painter(), s.icon, g.center(), theme::accent_col(), 0.95);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(theme::strong(s.name.clone()));
                    let sub = match (&s.doc, &s.status) {
                        (Some(d), _) => format!("{} · {}", s.kind, plural(d.questions.len(), "question")),
                        (None, Status::NeedsAi(Some(e))) => format!("{} · {e}", s.kind),
                        _ => s.kind.clone(),
                    };
                    ui.label(theme::muted(sub).size(11.5));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if s.status != Status::Converting {
                        let r = ui.add(egui::Button::new(theme::muted("Remove").size(11.5)).frame(false));
                        if r.clicked() {
                            remove = Some(i);
                        }
                    }
                    tag(ui, &s.status, s);
                });
            });
            ui.add_space(8.0);
        }
        if let Some(i) = remove {
            st.sources.remove(i);
        }

        // AI block: only photos (and PDF pages) ever go to a model.
        let pending = st.sources.iter().filter(|s| matches!(s.status, Status::NeedsAi(_))).count();
        let converting = st.sources.iter().any(|s| s.status == Status::Converting);
        let converted = st.sources.iter().any(|s| s.fresh);
        if pending > 0 || converting || converted {
            ui.add_space(4.0);
            let y = ui.cursor().top();
            ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::border()));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if converting || busy {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(theme::strong(format!(
                                "Asking the AI to read {}…",
                                app.import_name
                            )));
                        });
                    } else if pending > 0 {
                        ui.label(theme::strong(format!(
                            "{} {} AI to read {}",
                            pending,
                            if pending == 1 { "photo needs" } else { "photos need" },
                            if pending == 1 { "it" } else { "them" }
                        )));
                        ui.label(theme::muted("Typed text and files need no AI. Handwriting and photos do.").size(12.0));
                    } else {
                        ui.label(theme::strong("Photo converted"));
                        ui.label(theme::muted("Check the new questions, marked with a yellow edge.").size(12.0));
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pending > 0 && !converting && !busy {
                        if kit::primary_button(ui, "Convert with AI").clicked() {
                            st.ai_all = true;
                            start_next_ai(app, st, ctx);
                        }
                    } else if pending == 0 && !converting && converted {
                        kit::pill(ui, "Converted", theme::accent_faded(), theme::ink());
                    }
                });
            });
            if pending > 0 && !converting && !busy {
                ui.add_space(6.0);
                ui.add(
                    egui::TextEdit::singleline(&mut app.ai_hint)
                        .desired_width(ui.available_width())
                        .hint_text("Anything the AI should know about this photo? (optional)"),
                );
            }
        }

        // Privacy note, truthful about the configured provider.
        ui.add_space(10.0);
        let (dest, leaves) = provider_label(&app.settings);
        let msg = if leaves {
            format!(
                "Text and spreadsheets are read on this computer. Nothing leaves it until you press Convert, and then only the photo goes to {dest}."
            )
        } else {
            format!(
                "Everything is read on this computer. When you press Convert, the photo goes to {dest}, which runs here too."
            )
        };
        ui.horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
            paint_icon(ui.painter(), Ic::Lock, r.center() + Vec2::new(0.0, 2.0), theme::accent_col(), 0.8);
            ui.add(egui::Label::new(theme::muted(msg).size(12.0)).wrap());
        });
    });
}

// ----- right column: review -----

fn included(st_skip: bool, s: &Source, dup: &[bool], i: usize) -> bool {
    s.sel.get(i).copied().unwrap_or(false) && !(st_skip && dup.get(i).copied().unwrap_or(false))
}

fn right(app: &mut crate::App, ui: &mut egui::Ui, st: &mut State, flags: &[Vec<bool>]) {
    let skip = st.skip_dups;
    let total: usize = st.sources.iter().map(|s| s.doc.as_ref().map(|d| d.questions.len()).unwrap_or(0)).sum();
    let dups: usize = flags.iter().map(|f| f.iter().filter(|d| **d).count()).sum();
    let n: usize = st
        .sources
        .iter()
        .enumerate()
        .map(|(si, s)| (0..s.sel.len()).filter(|&i| included(skip, s, &flags[si], i)).count())
        .sum();

    kit::card().inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(theme::serif("Review").size(22.0).color(theme::ink()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(theme::muted(format!("{} found · you can untick any", plural(total, "question"))).size(12.0));
            });
        });
        ui.add_space(8.0);
        if st.sources.is_empty() {
            ui.label(theme::muted("Add something on the left and the questions inside it will show up here."));
            ui.add_space(8.0);
        }
        let skip_now = skip;
        for (si, s) in st.sources.iter_mut().enumerate() {
            group(ui, s, &flags[si], skip_now, app.settings_points(), si);
            ui.add_space(8.0);
        }
        options_and_finish(app, ui, st, flags, dups, n);
    });
}

impl crate::App {
    fn settings_points(&self) -> f64 {
        with_default_points()
    }
}

// Default points live in the screen state, which is checked out while drawing; the
// review rows read it through this thread-local mirror instead.
thread_local! { static DEFAULT_PTS: RefCell<f64> = const { RefCell::new(1.0) }; }
fn with_default_points() -> f64 {
    DEFAULT_PTS.with(|p| *p.borrow())
}

fn group(ui: &mut egui::Ui, s: &mut Source, dup: &[bool], skip: bool, default_pts: f64, si: usize) {
    egui::Frame::new()
        .fill(theme::field())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            let nq = s.doc.as_ref().map(|d| d.questions.len()).unwrap_or(0);
            // header
            let head = egui::Frame::new()
                .fill(theme::card())
                .corner_radius(CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 })
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let all = nq > 0 && s.sel.iter().all(|b| *b);
                        if cb(ui, all) && nq > 0 {
                            for b in s.sel.iter_mut() {
                                *b = !all;
                            }
                        }
                        ui.add_space(4.0);
                        ui.label(theme::strong(s.name.clone()));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(theme::muted(plural(nq, "question")).size(12.0));
                        });
                    });
                });
            let y = head.response.rect.bottom();
            ui.painter().hline(head.response.rect.x_range(), y, Stroke::new(1.0, theme::border()));
            match &s.status {
                Status::NeedsAi(_) => {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.add_space(0.0);
                        let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 36.0), Sense::hover());
                        paint_icon(ui.painter(), Ic::Sparkle, r.center(), theme::muted_col(), 0.8);
                        ui.label(theme::muted("Press Convert with AI to read this photo.").size(12.5));
                    });
                    return;
                }
                Status::Converting => {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.spinner();
                        ui.label(theme::muted("Reading this photo…").size(12.5));
                    });
                    ui.add_space(8.0);
                    return;
                }
                Status::Ready => {}
            }
            let shown = if s.expanded { nq } else { nq.min(3) };
            let Some(doc) = &s.doc else { return };
            for i in 0..shown {
                let q = &doc.questions[i];
                let is_dup = dup.get(i).copied().unwrap_or(false);
                let on = s.sel[i];
                let skipped = skip && is_dup;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
                let p = ui.painter();
                if i > 0 {
                    p.hline(rect.x_range(), rect.top(), Stroke::new(1.0, theme::border()));
                }
                if s.fresh {
                    p.rect_filled(
                        Rect::from_min_size(rect.left_top(), Vec2::new(3.0, rect.height())),
                        CornerRadius::ZERO,
                        theme::highlighter(),
                    );
                }
                let cb_rect = Rect::from_center_size(rect.left_center() + Vec2::new(21.0, 0.0), Vec2::splat(15.0));
                draw_cb(p, cb_rect, on && !skipped, resp.hovered());
                let pill = Rect::from_center_size(rect.left_center() + Vec2::new(63.0, 0.0), Vec2::new(42.0, 18.0));
                p.rect_filled(pill, CornerRadius::same(9), theme::accent_faded());
                p.text(pill.center(), egui::Align2::CENTER_CENTER, short_kind(q), FontId::new(9.5, theme::bold_family()), theme::ink());
                let right_txt = if is_dup {
                    "Duplicate".to_string()
                } else {
                    fmt_pts(q.points.unwrap_or(default_pts))
                };
                let (rc, rf) = if is_dup {
                    (theme::bad(), FontId::new(11.5, theme::bold_family()))
                } else {
                    (theme::muted_col(), FontId::new(11.5, egui::FontFamily::Proportional))
                };
                let rg = p.layout_no_wrap(right_txt, rf, rc);
                let rw = rg.size().x;
                p.galley(Pos2::new(rect.right() - 12.0 - rw, rect.center().y - rg.size().y / 2.0), rg, rc);
                let stem_rect = Rect::from_min_max(
                    Pos2::new(rect.left() + 92.0, rect.top()),
                    Pos2::new(rect.right() - 22.0 - rw, rect.bottom()),
                );
                let stem_col = if on && !skipped { theme::ink() } else { theme::muted_col() };
                let stem = crate::equation::stem_to_plain(&q.stem.replace('\n', " "));
                ui.put(
                    stem_rect,
                    egui::Label::new(egui::RichText::new(stem).size(12.5).color(stem_col)).truncate(),
                );
                if resp.clicked() {
                    s.sel[i] = !on;
                }
            }
            if nq > 3 {
                let label = if s.expanded { "Show fewer".to_string() } else { format!("+ {} more", nq - 3) };
                ui.horizontal(|ui| {
                    ui.add_space(63.0);
                    if ui.add(egui::Button::new(theme::muted(label).size(12.0)).frame(false)).clicked() {
                        s.expanded = !s.expanded;
                    }
                });
                ui.add_space(3.0);
            }
            let _ = si;
        });
}

fn options_and_finish(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    st: &mut State,
    flags: &[Vec<bool>],
    dups: usize,
    n: usize,
) {
    DEFAULT_PTS.with(|p| *p.borrow_mut() = st.points);
    ui.add_space(4.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() * 0.62).max(120.0));
            kit::small_caps(ui, "Add to bank");
            let current = match &st.dest {
                Some(id) => app.store.bank(id).map(|b| b.name.clone()).unwrap_or_else(|| "New bank".to_string()),
                None => "New bank".to_string(),
            };
            egui::ComboBox::from_id_salt("imp_dest")
                .width(ui.available_width() - 8.0)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut st.dest, None, "New bank");
                    for b in &app.store.banks {
                        ui.selectable_value(&mut st.dest, Some(b.id.clone()), b.name.clone());
                    }
                });
            let hint = default_bank_name(st);
            if st.dest.is_none() {
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut st.new_name)
                        .desired_width(ui.available_width() - 8.0)
                        .hint_text(hint),
                );
            }
        });
        ui.vertical(|ui| {
            kit::small_caps(ui, "Default points");
            ui.add(egui::DragValue::new(&mut st.points).range(0.0..=100.0).speed(0.25));
            ui.label(theme::muted("for questions without any").size(10.5));
        });
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if cb(ui, st.skip_dups) {
            st.skip_dups = !st.skip_dups;
        }
        ui.label(theme::text(format!("Skip duplicates ({dups} found)")));
    });

    ui.add_space(10.0);
    let y = ui.cursor().top();
    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::border()));
    ui.add_space(10.0);
    let note = match st.dest.as_ref().and_then(|id| app.store.bank(id)) {
        Some(b) => {
            let cur = b.questions.len();
            format!("Goes to {}, {cur} + {n} = {} questions after import.", b.name, cur + n)
        }
        None => format!("Creates a new bank, {}.", bank_name_for(st)),
    };
    let mut go = false;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = format!("Import {}", plural(n, "question"));
            if ui.add_enabled_ui(n > 0, |ui| kit::primary_button(ui, &label)).inner.clicked() {
                go = true;
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(egui::Label::new(theme::muted(note).size(12.0)).wrap());
            });
        });
    });
    if go {
        commit(app, st, flags);
    }
}

fn default_bank_name(st: &State) -> String {
    st.sources
        .iter()
        .find_map(|s| s.doc.as_ref().map(|d| d.name.trim().to_string()).filter(|n| !n.is_empty()))
        .unwrap_or_else(|| "Imported questions".to_string())
}

fn bank_name_for(st: &State) -> String {
    let n = st.new_name.trim();
    if n.is_empty() { default_bank_name(st) } else { n.to_string() }
}

/// Files the ticked questions into the destination bank and records each source.
fn commit(app: &mut crate::App, st: &mut State, flags: &[Vec<bool>]) {
    let t = now_secs();
    let skip = st.skip_dups;
    let default_pts = st.points;
    struct Done {
        name: String,
        how: &'static str,
        count: usize,
    }
    let mut picked: Vec<Question> = Vec::new();
    let mut exams: Vec<model::Exam> = Vec::new();
    let mut done: Vec<Done> = Vec::new();
    let mut keep: Vec<Source> = Vec::new();
    for (si, mut s) in std::mem::take(&mut st.sources).into_iter().enumerate() {
        let Some(doc) = s.doc.take() else {
            keep.push(s);
            continue;
        };
        let dup = &flags[si];
        let inc: Vec<bool> = (0..doc.questions.len()).map(|i| included(skip, &s, dup, i)).collect();
        let all = inc.iter().all(|b| *b);
        let mut count = 0;
        for (q, take) in doc.questions.into_iter().zip(inc) {
            if take {
                let mut q = q;
                if q.points.is_none() {
                    q.points = Some(default_pts);
                }
                picked.push(q);
                count += 1;
            }
        }
        if all {
            exams.extend(doc.exams);
        }
        if count > 0 {
            done.push(Done { name: s.name.clone(), how: s.how, count });
        }
    }
    st.sources = keep;
    if picked.is_empty() {
        return;
    }
    let total = picked.len();
    let existing = st.dest.clone().filter(|id| app.store.bank(id).is_some());
    let (bank_id, bank_name) = if let Some(id) = existing {
        let b = app.store.banks.iter_mut().find(|b| b.id == id).expect("checked");
        b.questions.extend(picked);
        b.modified = t;
        (id, b.name.clone())
    } else {
        let name = bank_name_for(st);
        let bank = QuestionBank {
            id: model::new_id(),
            name: name.clone(),
            questions: picked,
            created: t,
            modified: t,
        };
        let id = bank.id.clone();
        app.store.banks.push(bank);
        (id, name)
    };
    app.store.save_banks();
    if !exams.is_empty() {
        remap_empty_bank_ids(&mut exams, &bank_id);
        app.store.exams.extend(exams);
        app.store.save_exams();
    }
    let model_note = crate::ai::AiCfg::from(&app.settings).model;
    for d in done {
        app.store.imports.push(Import {
            id: model::new_id(),
            time: t,
            source: d.name,
            how: d.how.to_string(),
            count: d.count,
            bank_ids: vec![bank_id.clone()],
            note: if d.how == "ai" { Some(model_note.clone()) } else { None },
        });
    }
    app.store.save_imports();
    st.new_name.clear();
    app.toast(format!("Imported {} into “{}”", plural(total, "question"), bank_name));
    app.open_bank(&bank_id);
}

// ----- import history (secondary) -----

fn history(app: &mut crate::App, ui: &mut egui::Ui) {
    ui.label(theme::serif("Import history").size(22.0).color(theme::ink()));
    ui.label(theme::muted("Everything queShion has read: pasted, converted by the AI, or read from a file.").size(12.5));
    ui.add_space(8.0);
    if app.store.imports.is_empty() {
        ui.label(theme::muted("No imports yet."));
        return;
    }
    let mut sorted = app.store.imports.clone();
    sorted.sort_by(|a, b| b.time.cmp(&a.time));
    let mut open: Option<Id> = None;
    for imp in &sorted {
        kit::card().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(theme::strong(imp.source.clone()));
            let mut line = format!(
                "{} · {} · {}",
                fmt_time(imp.time),
                import_how_label(&imp.how),
                plural(imp.count, "question")
            );
            if let Some(note) = &imp.note {
                if !note.trim().is_empty() {
                    line.push_str(&format!(" · {note}"));
                }
            }
            ui.label(theme::muted(line).size(11.5));
            if !imp.bank_ids.is_empty() {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::muted("Questions in:").size(11.5));
                    for id in &imp.bank_ids {
                        match app.store.bank(id) {
                            Some(b) => {
                                if kit::chip_toggle(ui, &b.name, false).clicked() {
                                    open = Some(id.clone());
                                }
                            }
                            None => {
                                ui.label(theme::muted("deleted bank").size(11.5));
                            }
                        }
                    }
                });
            }
        });
        ui.add_space(4.0);
    }
    if let Some(id) = open {
        app.open_bank(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn docx_paragraphs_become_lines() {
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            z.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"<w:document><w:p><w:r><w:t>TF</w:t></w:r></w:p><w:p><w:r><w:t>A &amp; B is true</w:t></w:r></w:p><w:p><w:r><w:t>T</w:t></w:r></w:p></w:document>").unwrap();
            z.finish().unwrap();
        }
        let t = docx_text(&buf).unwrap();
        assert_eq!(t, "TF\nA & B is true\nT\n");
        let doc = crate::formats::detect_and_parse(t.as_bytes(), "x.docx").unwrap();
        assert_eq!(doc.questions.len(), 1);
    }

    #[test]
    fn norm_collapses_space_and_case() {
        assert_eq!(norm("  Hello   World "), "hello world");
    }
}
