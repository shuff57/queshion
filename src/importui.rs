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
use crate::model::{self, Id, Import, Question, QuestionBank};
use crate::examui::{paper_question, ptext, PAGE_H, PAGE_W, PAPER_RULE, PAPER_SOFT, PT};
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
    /// Read by the AI this session (tagged in the list).
    fresh: bool,
    /// The file format's own version, when it states one.
    version: Option<String>,
    /// Which of the doc's tests (exams) to import.
    exam_sel: Vec<bool>,
    /// Preview texture of a photo / PDF page waiting for the AI.
    tex: Option<egui::TextureHandle>,
}

/// What the centre pane previews.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Pick {
    Source(usize),
    /// (source, exam index in its doc)
    Exam(usize, usize),
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
    /// Item shown in the page preview.
    pick: Option<Pick>,
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

/// What "Skip duplicates" compares: the whole question — stem AND its kind's content
/// (answers and which are correct, pairs, accepted blanks, every multipart part with its
/// own stem and table) — whitespace- and case-insensitively. Two multiparts sharing a
/// lead-in but with different parts are not duplicates.
fn dup_key(q: &crate::model::Question) -> String {
    fn norm_all(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::String(s) => *s = norm(s),
            serde_json::Value::Array(a) => a.iter_mut().for_each(norm_all),
            serde_json::Value::Object(o) => o.values_mut().for_each(norm_all),
            _ => {}
        }
    }
    let mut kind = serde_json::to_value(&q.kind).unwrap_or_default();
    norm_all(&mut kind);
    format!("{}\u{1}{}", norm(&q.stem), kind)
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
    fn push_doc(&mut self, name: &str, kind: &str, how: &'static str, icon: Ic, doc: ImportDoc, version: Option<String>) {
        let n = doc.questions.len();
        let ne = doc.exams.len();
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
            version,
            exam_sel: vec![true; ne],
            tex: None,
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
            version: None,
            exam_sel: Vec::new(),
            tex: None,
        });
    }
}

/// Detects what `bytes` is and adds it as a source (or toasts why not).
fn add_bytes(app: &mut crate::App, st: &mut State, name: &str, bytes: Vec<u8>, how: &'static str) {
    let before = st.sources.len();
    add_bytes_inner(app, st, name, bytes, how);
    if st.sources.len() > before {
        st.pick = Some(Pick::Source(before));
    }
}

fn add_bytes_inner(app: &mut crate::App, st: &mut State, name: &str, bytes: Vec<u8>, how: &'static str) {
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
        "xls" => {
            app.toast_err(format!("{name}: old .xls files can't be read. Save it as .xlsx (or CSV) and add it again."));
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
        // Zips (.xlsx, QTI/Canvas exports) and the newer text formats name themselves;
        // the rest keep the original Blackboard/Aiken/CSV/Test Parrot labels.
        let k = crate::formats::format_label(&bytes, name).map(str::to_string).unwrap_or_else(|| text_kind(name, &ext, &head).to_string());
        (bytes, k, if how == "paste" { Ic::Clip } else { Ic::File })
    };
    match crate::formats::detect_and_parse(&text_bytes, name) {
        Ok(doc) if doc.questions.is_empty() => {
            app.toast_err(format!("Couldn't find any questions in {name}."))
        }
        Ok(doc) => {
            let version = crate::formats::format_version(&text_bytes);
            st.push_doc(name, &kind, how, icon, doc, version)
        }
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
                    s.exam_sel = vec![true; doc.exams.len()];
                    s.doc = Some(doc);
                    s.status = Status::Ready;
                    s.fresh = true;
                    s.kind = "Photo, read by AI".to_string();
                    s.image = None;
                    s.tex = None;
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
            // Headless test hook: QUESHION_IMPORT_FILES=<path>[:<path>...] stages files once.
            if let Ok(v) = std::env::var("QUESHION_IMPORT_FILES") {
                add_paths(self, &mut st, std::env::split_paths(&v).collect());
            }
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
    fix_pick(st);
    let view_h = ui.available_height();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let full = ui.available_width();
        let w = if st.sources.is_empty() { full.min(1040.0) } else { (full - 14.0).min(1500.0) };
        ui.allocate_ui_with_layout(Vec2::new(w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(w);
            kit::page_header(
                ui,
                &["Home", "Import"],
                "Import questions",
                Some("One place for everything: pasted text, files, spreadsheets or a photo of a worksheet. We work out the format for you."),
            );
            ui.add_space(14.0);
            drop_area(app, ui, st);
            ui.add_space(14.0);
            if st.sources.is_empty() {
                privacy_note(app, ui);
            } else {
                let flags = dup_flags(app, st);
                workspace(app, ui, ctx, st, &flags, w, view_h);
            }
            ui.add_space(26.0);
            history(app, ui);
            ui.add_space(20.0);
        });
    });
}

/// Keeps the previewed item valid (first source when nothing / something removed).
fn fix_pick(st: &mut State) {
    let ok = match st.pick {
        Some(Pick::Source(i)) => i < st.sources.len(),
        Some(Pick::Exam(i, e)) => st.sources.get(i).and_then(|s| s.doc.as_ref()).is_some_and(|d| e < d.exams.len()),
        None => false,
    };
    if !ok {
        st.pick = (!st.sources.is_empty()).then_some(Pick::Source(0));
    }
}

fn dup_flags(app: &crate::App, st: &State) -> Vec<Vec<bool>> {
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(id) = &st.dest {
        if let Some(b) = app.store.bank(id) {
            for q in &b.questions {
                seen.insert(dup_key(q));
            }
        }
    }
    st.sources
        .iter()
        .map(|s| match &s.doc {
            Some(d) => d.questions.iter().map(|q| !seen.insert(dup_key(q))).collect(),
            None => Vec::new(),
        })
        .collect()
}

// ----- drop area -----

fn drop_area(app: &mut crate::App, ui: &mut egui::Ui, st: &mut State) {
    let compact = !st.sources.is_empty();
    let hovering = ctx_hovering(ui.ctx());
    let fill = if hovering { theme::accent_faded().gamma_multiply(1.4) } else { theme::accent_faded() };
    let inner = egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(if compact { Margin::symmetric(16, 10) } else { Margin { left: 16, right: 16, top: 18, bottom: 14 } })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                if !compact {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(14), theme::card());
                    paint_icon(ui.painter(), Ic::Upload, r.center(), theme::accent_col(), 1.25);
                    ui.add_space(4.0);
                    ui.label(theme::serif("Drop anything here").size(19.0).color(theme::ink()));
                    ui.label(theme::text("Files, photos, or paste text straight in.").size(12.5));
                    ui.add_space(8.0);
                }
                ui.horizontal(|ui| {
                    let lead = if compact { 520.0 } else { 350.0 };
                    ui.add_space(((ui.available_width() - lead) / 2.0).max(0.0));
                    if compact {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                        paint_icon(ui.painter(), Ic::Upload, r.center(), theme::accent_col(), 0.9);
                        ui.label(theme::serif("Drop more here, or").size(16.0).color(theme::ink()));
                    }
                    if kit::primary_button(ui, if compact { "Add files" } else { "Choose files" }).clicked() {
                        if let Some(paths) = rfd::FileDialog::new()
                            .add_filter(
                                "Question files",
                                &[
                                    "txt", "csv", "tsv", "json", "docx", "xlsx", "gift", "xml", "qti", "zip", "imscc", "md", "pdf", "png", "jpg",
                                    "jpeg", "gif", "webp",
                                ],
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
                if !compact {
                    ui.add_space(8.0);
                    ui.label(
                        theme::muted(
                            "Blackboard · Aiken · CSV · Excel · GIFT · Moodle XML · QTI/Canvas · Respondus · Kahoot · Quizlet · .parrot.json · PDF · Word · photos",
                        )
                        .size(11.0),
                    );
                }
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

/// Privacy note, truthful about the configured provider.
fn privacy_note(app: &crate::App, ui: &mut egui::Ui) {
    let (dest, leaves) = provider_label(&app.settings);
    let msg = if leaves {
        format!(
            "Text and spreadsheets are read on this computer. Nothing leaves it until you press Convert, and then only the photo goes to {dest}."
        )
    } else {
        format!("Everything is read on this computer. When you press Convert, the photo goes to {dest}, which runs here too.")
    };
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
        paint_icon(ui.painter(), Ic::Lock, r.center() + Vec2::new(0.0, 2.0), theme::accent_col(), 0.8);
        ui.add(egui::Label::new(theme::muted(msg).size(12.0)).wrap());
    });
}

// ----- the three-pane workspace: import list | page preview | details -----

fn included(st_skip: bool, s: &Source, dup: &[bool], i: usize) -> bool {
    s.sel.get(i).copied().unwrap_or(false) && !(st_skip && dup.get(i).copied().unwrap_or(false))
}

/// A pane: card of `w` (and fixed `h` when Some) whose content scrolls on its own.
fn pane(ui: &mut egui::Ui, w: f32, h: Option<f32>, salt: &str, fill: Option<Color32>, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(Vec2::new(w, h.unwrap_or(0.0)), Layout::top_down(Align::Min), |ui| {
        let mut frame = kit::card().inner_margin(Margin::symmetric(14, 12));
        if let Some(f) = fill {
            frame = frame.fill(f);
        }
        frame.show(ui, |ui| {
            ui.set_width(w - 30.0);
            match h {
                Some(h) => {
                    ui.set_height(h - 26.0);
                    egui::ScrollArea::vertical().id_salt(salt).max_height(h - 26.0).auto_shrink([false, false]).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        add(ui);
                    });
                }
                // Stacked (narrow) panes just grow; the page scrolls.
                None => add(ui),
            }
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn workspace(app: &mut crate::App, ui: &mut egui::Ui, ctx: &egui::Context, st: &mut State, flags: &[Vec<bool>], w: f32, view_h: f32) {
    let h = (view_h - 120.0).clamp(460.0, 900.0);
    let desk = Some(theme::field());
    if w >= 940.0 {
        let (lw, rw, gap) = (250.0, 272.0, 12.0);
        let cw = (w - lw - rw - gap * 2.0).floor();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            pane(ui, lw, Some(h), "imp_list", None, |ui| list_pane(app, ui, ctx, st, flags));
            pane(ui, cw, Some(h), "imp_page", desk, |ui| preview_pane(app, ui, ctx, st, flags));
            pane(ui, rw, Some(h), "imp_detail", None, |ui| detail_pane(app, ui, st, flags));
        });
    } else {
        pane(ui, w, None, "imp_list", None, |ui| list_pane(app, ui, ctx, st, flags));
        ui.add_space(12.0);
        pane(ui, w, Some(h), "imp_page", desk, |ui| preview_pane(app, ui, ctx, st, flags));
        ui.add_space(12.0);
        pane(ui, w, None, "imp_detail", None, |ui| detail_pane(app, ui, st, flags));
    }
}

/// One-line truncated galley.
fn line_galley(ui: &egui::Ui, text: &str, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap.max_width = width.max(10.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    ui.painter().layout_job(job)
}

/// Section head of the import list: icon-less small caps + count badge.
fn group_head(ui: &mut egui::Ui, label: &str, n: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
    let p = ui.painter();
    let lg = p.layout_no_wrap(kit::small_caps_text(label).text().to_string(), FontId::new(10.5, theme::bold_family()), theme::muted_col());
    p.galley(Pos2::new(rect.left(), rect.center().y - lg.size().y / 2.0), lg, theme::muted_col());
    let ng = p.layout_no_wrap(n.to_string(), FontId::new(10.5, theme::bold_family()), theme::accent_col());
    let w = (ng.size().x + 14.0).max(22.0);
    let badge = Rect::from_center_size(Pos2::new(rect.right() - w / 2.0, rect.center().y), Vec2::new(w, 18.0));
    p.rect_filled(badge, CornerRadius::same(9), theme::accent_faded());
    p.galley(badge.center() - ng.size() / 2.0, ng, theme::accent_col());
    ui.add_space(4.0);
}

enum RowHit {
    None,
    Check,
    Pick,
    Remove,
}

/// A painter-drawn import-list row: tick box (its own click target), title, sub line,
/// optional tag, hover "Remove". Returns what was clicked.
fn list_row(ui: &mut egui::Ui, id: egui::Id, picked: bool, on: bool, title: &str, sub: &str, tag: Option<(&str, bool)>, removable: bool) -> RowHit {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
    let p = ui.painter();
    if picked {
        p.rect_filled(rect, CornerRadius::same(10), theme::accent_faded());
        p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, theme::accent_col().gamma_multiply(0.5)), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(10), theme::hover_bg());
    }
    let cb_rect = Rect::from_center_size(Pos2::new(rect.left() + 16.0, rect.center().y), Vec2::splat(16.0));
    let hover_pos = resp.hover_pos();
    let over_cb = hover_pos.is_some_and(|h| cb_rect.expand(4.0).contains(h));
    draw_cb(p, cb_rect, on, over_cb);
    let x0 = rect.left() + 34.0;
    let mut right = rect.right() - 8.0;
    let rm_rect = Rect::from_center_size(Pos2::new(right - 6.0, rect.center().y), Vec2::splat(16.0));
    let show_rm = removable && resp.hovered();
    if show_rm {
        let over = hover_pos.is_some_and(|h| rm_rect.contains(h));
        let s = Stroke::new(1.6, if over { theme::bad() } else { theme::muted_col() });
        let c = rm_rect.center();
        p.line_segment([c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0)], s);
        p.line_segment([c + Vec2::new(-4.0, 4.0), c + Vec2::new(4.0, -4.0)], s);
        right -= 18.0;
    }
    if let Some((t, hot)) = tag {
        let g = p.layout_no_wrap(t.to_string(), FontId::new(9.5, theme::bold_family()), if hot { theme::on_highlighter() } else { theme::accent_col() });
        let pr = Rect::from_min_size(Pos2::new(right - g.size().x - 12.0, rect.top() + 7.0), g.size() + Vec2::new(12.0, 4.0));
        p.rect_filled(pr, CornerRadius::same(8), if hot { theme::highlighter() } else { theme::accent_faded() });
        p.galley(pr.min + Vec2::new(6.0, 2.0), g, theme::ink());
        right = pr.left() - 4.0;
    }
    let tg = line_galley(ui, title, FontId::new(13.0, theme::bold_family()), theme::ink(), right - x0);
    let sg = line_galley(ui, sub, FontId::new(11.5, egui::FontFamily::Proportional), theme::muted_col(), rect.right() - 8.0 - x0);
    let p = ui.painter();
    p.galley(Pos2::new(x0, rect.top() + 6.0), tg, theme::ink());
    p.galley(Pos2::new(x0, rect.top() + 25.0), sg, theme::muted_col());
    let _ = id;
    if resp.clicked() {
        let at = resp.interact_pointer_pos().unwrap_or(rect.center());
        if cb_rect.expand(5.0).contains(at) {
            return RowHit::Check;
        }
        if show_rm && rm_rect.expand(3.0).contains(at) {
            return RowHit::Remove;
        }
        return RowHit::Pick;
    }
    RowHit::None
}

fn list_pane(app: &mut crate::App, ui: &mut egui::Ui, ctx: &egui::Context, st: &mut State, flags: &[Vec<bool>]) {
    ui.label(theme::serif("Import list").size(19.0).color(theme::ink()));
    ui.label(theme::muted("What you're about to import. Untick anything you don't want.").size(12.0));
    ui.add_space(10.0);
    let skip = st.skip_dups;
    group_head(ui, "Questions", st.sources.len());
    let mut remove: Option<usize> = None;
    for (i, s) in st.sources.iter_mut().enumerate() {
        let nq = s.doc.as_ref().map(|d| d.questions.len()).unwrap_or(0);
        let inc = (0..nq).filter(|&k| included(skip, s, &flags[i], k)).count();
        let on = nq > 0 && s.sel.iter().all(|b| *b);
        let sub = match (&s.doc, &s.status) {
            (Some(_), _) if inc == nq => format!("{} · {}", plural(nq, "question"), s.kind),
            (Some(_), _) => format!("{inc} of {nq} questions · {}", s.kind),
            (None, Status::NeedsAi(Some(e))) => format!("{} · {e}", s.kind),
            _ => s.kind.clone(),
        };
        let tag = match s.status {
            Status::NeedsAi(_) => Some(("Needs AI", true)),
            Status::Converting => Some(("Reading…", true)),
            Status::Ready if s.fresh => Some(("Read by AI", false)),
            Status::Ready => None,
        };
        let picked = st.pick == Some(Pick::Source(i));
        match list_row(ui, egui::Id::new(("imp_src", i)), picked, on, &s.name, &sub, tag, s.status != Status::Converting) {
            RowHit::Check if nq > 0 => {
                for b in s.sel.iter_mut() {
                    *b = !on;
                }
            }
            RowHit::Check | RowHit::Pick => st.pick = Some(Pick::Source(i)),
            RowHit::Remove => remove = Some(i),
            RowHit::None => {}
        }
        ui.add_space(4.0);
    }
    if let Some(i) = remove {
        st.sources.remove(i);
        fix_pick(st);
    }

    // Tests (exams) that came inside the files.
    let n_exams: usize = st.sources.iter().filter_map(|s| s.doc.as_ref()).map(|d| d.exams.len()).sum();
    if n_exams > 0 {
        ui.add_space(10.0);
        group_head(ui, "Tests", n_exams);
        for (i, s) in st.sources.iter_mut().enumerate() {
            let Some(doc) = &s.doc else { continue };
            for (e, ex) in doc.exams.iter().enumerate() {
                let on = s.exam_sel.get(e).copied().unwrap_or(true);
                let sub = format!("{} · from {}", plural(ex.items.len(), "question"), s.name);
                let picked = st.pick == Some(Pick::Exam(i, e));
                match list_row(ui, egui::Id::new(("imp_exam", i, e)), picked, on, &ex.name, &sub, None, false) {
                    RowHit::Check => {
                        if let Some(b) = s.exam_sel.get_mut(e) {
                            *b = !on;
                        }
                    }
                    RowHit::Pick => st.pick = Some(Pick::Exam(i, e)),
                    _ => {}
                }
                ui.add_space(4.0);
            }
        }
    }

    ai_block(app, ui, ctx, st);
    ui.add_space(12.0);
    privacy_note(app, ui);
}

/// Photos (and PDF pages) are the only sources that go to a model.
fn ai_block(app: &mut crate::App, ui: &mut egui::Ui, ctx: &egui::Context, st: &mut State) {
    let busy = app.ai_busy;
    let pending = st.sources.iter().filter(|s| matches!(s.status, Status::NeedsAi(_))).count();
    let converting = st.sources.iter().any(|s| s.status == Status::Converting);
    let converted = st.sources.iter().any(|s| s.fresh);
    if pending == 0 && !converting && !converted {
        return;
    }
    ui.add_space(8.0);
    let y = ui.cursor().top();
    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::border()));
    ui.add_space(10.0);
    if converting || busy {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.add(egui::Label::new(theme::strong(format!("Asking the AI to read {}…", app.import_name))).wrap());
        });
    } else if pending > 0 {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
            paint_icon(ui.painter(), Ic::Sparkle, r.center(), theme::accent_col(), 0.75);
            ui.label(theme::strong("Photos"));
        });
        ui.label(theme::strong(format!(
            "{} {} AI to read {}",
            pending,
            if pending == 1 { "photo needs" } else { "photos need" },
            if pending == 1 { "it" } else { "them" }
        )));
        ui.label(theme::muted("Typed text and files need no AI. Handwriting and photos do.").size(12.0));
        ui.add_space(6.0);
        ui.add(
            egui::TextEdit::singleline(&mut app.ai_hint)
                .desired_width(ui.available_width())
                .hint_text("Anything the AI should know? (optional)"),
        );
        ui.add_space(6.0);
        if kit::primary_button(ui, "Convert with AI").clicked() {
            st.ai_all = true;
            start_next_ai(app, st, ctx);
        }
    } else {
        ui.horizontal(|ui| {
            ui.label(theme::strong("Photo converted"));
            kit::pill(ui, "Converted", theme::accent_faded(), theme::ink());
        });
        ui.label(theme::muted("Check the new questions in the preview.").size(12.0));
    }
}

// ----- centre: the page as it will print -----

/// Question numbers as they print: matching takes one number per left item.
fn print_numbers<'a>(qs: impl IntoIterator<Item = Option<&'a Question>>) -> Vec<u32> {
    let mut run = 1u32;
    qs.into_iter()
        .map(|q| {
            let n = run;
            run += match q.map(|q| &q.kind) {
                Some(model::Kind::Matching { pairs, .. }) => (pairs.len() as u32).max(1),
                _ => 1,
            };
            n
        })
        .collect()
}

fn preview_pane(app: &mut crate::App, ui: &mut egui::Ui, ctx: &egui::Context, st: &mut State, flags: &[Vec<bool>]) {
    let Some(pick) = st.pick else { return };
    let skip = st.skip_dups;
    let (si, exam_ix) = match pick {
        Pick::Source(i) => (i, None),
        Pick::Exam(i, e) => (i, Some(e)),
    };
    let Some(s) = st.sources.get_mut(si) else { return };
    let dup = &flags[si];

    // The questions on this page (index into the source's doc), in print order.
    let order: Vec<Option<usize>> = match (&s.doc, exam_ix) {
        (Some(d), None) => (0..d.questions.len()).map(Some).collect(),
        (Some(d), Some(e)) => d.exams[e]
            .items
            .iter()
            .map(|it| d.questions.iter().position(|q| q.id == it.question_id))
            .collect(),
        (None, _) => Vec::new(),
    };
    let pictures: usize = s
        .doc
        .as_ref()
        .map(|d| order.iter().flatten().map(|&k| crate::bankops::pictures_needed(&d.questions[k])).sum())
        .unwrap_or(0);
    if pictures > 0 {
        egui::Frame::new()
            .fill(theme::card())
            .stroke(Stroke::new(1.0, theme::accent_col().gamma_multiply(0.45)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                    let k = Stroke::new(1.4, theme::accent_col());
                    let b = Rect::from_center_size(r.center(), Vec2::new(14.0, 11.0));
                    ui.painter().rect_stroke(b, CornerRadius::same(2), k, StrokeKind::Middle);
                    ui.painter().add(Shape::line(
                        vec![b.left_bottom() + Vec2::new(2.0, -2.0), b.center() + Vec2::new(-1.0, 1.0), b.right_bottom() + Vec2::new(-2.0, -2.0)],
                        k,
                    ));
                    ui.add(
                        egui::Label::new(
                            theme::text(format!(
                                "{} needed. Add {} in the question editor after importing.",
                                plural(pictures, "picture"),
                                if pictures == 1 { "it" } else { "them" }
                            ))
                            .size(12.5),
                        )
                        .wrap(),
                    );
                });
            });
        ui.add_space(10.0);
    }

    let aw = ui.available_width();
    let paper_w = PAGE_W.min((aw - 16.0).max(260.0));
    let inset = ((aw - paper_w) / 2.0).max(0.0);
    let bp = 11.0 * PT;
    let side = (19.05 / 25.4 * 72.0 * PT).clamp(24.0, (paper_w - 180.0) / 2.0);
    let cw = paper_w - side * 2.0;
    let bg = ui.painter().add(Shape::Noop);
    let mut paper_rect = Rect::NOTHING;
    let mut pages = 1usize;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.add_space(inset);
        let r = ui.vertical(|ui| {
            ui.set_width(paper_w);
            let y0 = ui.cursor().top();
            ui.add_space(side * 1.05);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.add_space(side);
                ui.vertical(|ui| {
                    ui.set_width(cw);
                    sheet(app, ui, ctx, s, dup, skip, exam_ix, &order, cw, bp, side, si, y0);
                });
            });
            ui.add_space(side * 0.8);
            let used = ui.cursor().top() - y0;
            pages = (used / PAGE_H).ceil().max(1.0) as usize;
            ui.allocate_space(Vec2::new(1.0, pages as f32 * PAGE_H - used));
        });
        paper_rect = r.response.rect;
    });
    ui.add_space(10.0);
    let mut shapes: Vec<Shape> = vec![
        egui::Shadow { offset: [0, 10], blur: 24, spread: 0, color: Color32::from_black_alpha(50) }
            .as_shape(paper_rect, CornerRadius::same(2))
            .into(),
        Shape::rect_filled(paper_rect, CornerRadius::same(2), theme::paper()),
        Shape::rect_stroke(paper_rect, CornerRadius::same(2), Stroke::new(1.0, theme::paper_edge()), StrokeKind::Inside),
    ];
    for k in 1..pages {
        let y = paper_rect.top() + k as f32 * PAGE_H;
        shapes.extend(Shape::dashed_line(
            &[Pos2::new(paper_rect.left(), y), Pos2::new(paper_rect.right(), y)],
            Stroke::new(1.0, PAPER_RULE.gamma_multiply(0.7)),
            5.0,
            4.0,
        ));
    }
    ui.painter().set(bg, Shape::Vec(shapes));
    for k in 1..=pages {
        ui.painter().text(
            Pos2::new(paper_rect.center().x, paper_rect.top() + k as f32 * PAGE_H - 12.0),
            egui::Align2::CENTER_CENTER,
            k.to_string(),
            FontId::new(9.0, theme::serif_family()),
            PAPER_SOFT.gamma_multiply(0.8),
        );
    }
}

/// The printed content of the previewed item: header line, title, numbered questions
/// (each with an include box in the left margin), or the photo while it waits for the AI.
#[allow(clippy::too_many_arguments)]
fn sheet(
    app: &mut crate::App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    s: &mut Source,
    dup: &[bool],
    skip: bool,
    exam_ix: Option<usize>,
    order: &[Option<usize>],
    cw: f32,
    bp: f32,
    side: f32,
    si: usize,
    paper_top: f32,
) {
    let Some(doc) = s.doc.as_ref() else {
        // Photo / PDF page not read yet: show the page itself.
        match s.status {
            Status::Converting => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(ptext("Reading this page…", bp, false));
                });
                ui.add_space(8.0);
            }
            _ => {
                ui.label(ptext("Press Convert with AI to turn this page into questions.", bp, false).color(PAPER_SOFT));
                ui.add_space(8.0);
            }
        }
        if s.tex.is_none() {
            if let Some(img) = s.image.as_ref().and_then(|f| image::load_from_memory(&f.bytes).ok()) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                s.tex = Some(ctx.load_texture(format!("imp_photo_{si}_{}", s.name), ci, egui::TextureOptions::LINEAR));
            }
        }
        if let Some(t) = &s.tex {
            ui.add(egui::Image::new(t).max_width(cw).maintain_aspect_ratio(true));
        }
        return;
    };

    let (header, title) = match exam_ix {
        Some(e) => {
            let ex = &doc.exams[e];
            (ex.header.clone(), ex.title.clone().unwrap_or_else(|| ex.name.clone()))
        }
        None => (
            crate::formats::DEFAULT_HEADER.to_string(),
            if doc.name.trim().is_empty() { s.name.clone() } else { doc.name.trim().to_string() },
        ),
    };
    ui.label(ptext(header, bp * 0.96, false).color(Color32::from_rgb(0x33, 0x33, 0x33)));
    let ly = ui.cursor().top() + 1.0;
    ui.painter().line_segment([Pos2::new(ui.min_rect().left(), ly), Pos2::new(ui.min_rect().left() + cw, ly)], Stroke::new(1.0, PAPER_RULE));
    ui.add_space(14.0);
    let pts: f64 = order.iter().flatten().filter_map(|&k| doc.questions[k].points).sum();
    ui.vertical_centered(|ui| {
        ui.add(egui::Label::new(ptext(title, 16.0 * PT, true)).wrap());
        ui.label(
            ptext(
                format!(
                    "{}{}",
                    plural(order.len(), "question"),
                    if pts > 0.0 { format!(" · {} points", if pts.fract() == 0.0 { format!("{}", pts as i64) } else { format!("{pts:.1}") }) } else { String::new() }
                ),
                bp * 0.92,
                false,
            )
            .color(PAPER_SOFT),
        );
    });
    ui.add_space(18.0);

    let numbers = print_numbers(order.iter().map(|k| k.map(|k| &doc.questions[k])));
    let mut toggle: Option<usize> = None;
    for (n, k) in order.iter().enumerate() {
        let q = k.map(|k| &doc.questions[k]);
        // Like the print (and the exam page): a question that would cross a page break,
        // judging by last frame's height, starts on the next page.
        let h_id = egui::Id::new(("imp_q_h", si, exam_ix, n));
        if let Some(h) = ctx.data(|d| d.get_temp::<f32>(h_id)) {
            let top = ui.cursor().top();
            let page = ((top - paper_top) / PAGE_H).floor().max(0.0);
            let page_end = paper_top + (page + 1.0) * PAGE_H - side * 0.8;
            if top + h > page_end && h < PAGE_H - side * 1.85 {
                ui.add_space(page_end + side * 0.8 + side * 1.05 - top);
            }
        }
        let rect = paper_question(app, ui, ctx, q, numbers[n], cw, bp, crate::model::PaperStyle::Standard);
        ctx.data_mut(|d| d.insert_temp(h_id, rect.height()));
        if let Some(k) = *k {
            let is_dup = dup.get(k).copied().unwrap_or(false);
            let inc = included(skip, s, dup, k);
            let p = ui.painter();
            if !inc {
                p.rect_filled(rect.expand2(Vec2::new(4.0, 3.0)), CornerRadius::same(2), theme::paper().gamma_multiply(0.62));
            }
            let cb_rect = Rect::from_center_size(Pos2::new(rect.left() - side * 0.5, rect.top() + bp * 0.75), Vec2::splat(14.0));
            let r = ui.interact(cb_rect.expand(3.0), ui.id().with(("imp_q_cb", si, k)), Sense::click());
            draw_cb(ui.painter(), cb_rect, inc, r.hovered());
            if r.clicked() {
                toggle = Some(k);
            }
            r.on_hover_text(if inc { "Included. Click to leave this question out." } else { "Left out. Click to include it." });
            if is_dup {
                ui.painter().text(
                    Pos2::new(rect.right(), rect.top() - 3.0),
                    egui::Align2::RIGHT_BOTTOM,
                    if skip { "Duplicate · skipped" } else { "Duplicate" },
                    FontId::new(9.5, theme::bold_family()),
                    theme::bad(),
                );
            }
        }
        ui.add_space(bp * 1.3);
    }
    if let Some(k) = toggle {
        let shown = included(skip, s, dup, k);
        if let Some(b) = s.sel.get_mut(k) {
            *b = !shown;
        }
    }
}

// ----- right: what's in it + where it goes -----

fn detail_row(ui: &mut egui::Ui, label: &str, value: &str, strong: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
    let vf = if strong { FontId::new(15.0, theme::bold_family()) } else { FontId::new(12.5, egui::FontFamily::Proportional) };
    let vg = ui.painter().layout_no_wrap(value.to_string(), vf, theme::ink());
    let vw = vg.size().x.min(rect.width() * 0.6);
    let lg = line_galley(ui, label, FontId::new(12.5, egui::FontFamily::Proportional), theme::text_col(), rect.width() - vw - 10.0);
    let p = ui.painter();
    p.galley(Pos2::new(rect.left(), rect.center().y - lg.size().y / 2.0), lg, theme::text_col());
    let vg = line_galley(ui, value, if strong { FontId::new(15.0, theme::bold_family()) } else { FontId::new(12.5, egui::FontFamily::Proportional) }, theme::ink(), rect.width() * 0.6);
    p.galley(Pos2::new(rect.right() - vg.size().x, rect.center().y - vg.size().y / 2.0), vg, theme::ink());
    p.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, theme::border()));
}

fn detail_pane(app: &mut crate::App, ui: &mut egui::Ui, st: &mut State, flags: &[Vec<bool>]) {
    if let Some(pick) = st.pick {
        let (si, e) = match pick {
            Pick::Source(i) => (i, None),
            Pick::Exam(i, e) => (i, Some(e)),
        };
        if let Some(s) = st.sources.get(si) {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                paint_icon(ui.painter(), if e.is_some() { Ic::File } else { s.icon }, r.center(), theme::accent_col(), 0.7);
                kit::small_caps(ui, if e.is_some() { "Test" } else { "Questions" });
            });
            let name = match (e, &s.doc) {
                (Some(e), Some(d)) => d.exams[e].name.clone(),
                _ => s.name.clone(),
            };
            ui.add(egui::Label::new(theme::serif(name).size(18.0).color(theme::ink())).wrap());
            ui.add_space(8.0);
            if let Some(d) = &s.doc {
                let qs: Vec<&Question> = match e {
                    Some(e) => d.exams[e].items.iter().filter_map(|it| d.questions.iter().find(|q| q.id == it.question_id)).collect(),
                    None => d.questions.iter().collect(),
                };
                let counts = crate::bankops::type_counts(qs.iter().copied());
                for (k, n) in crate::bankops::KINDS.iter().zip(counts) {
                    detail_row(ui, k.label(), &n.to_string(), true);
                }
                let pics: usize = qs.iter().map(|q| crate::bankops::pictures_needed(q)).sum();
                if pics > 0 {
                    detail_row(ui, "Pictures needed", &pics.to_string(), true);
                }
            }
            detail_row(ui, "Format", &s.kind, false);
            if let Some(v) = &s.version {
                detail_row(ui, "Format version", v, true);
            }
            if e.is_some() {
                ui.add_space(8.0);
                kit::small_caps(ui, "Questions from");
                ui.add_space(4.0);
                let nq = s.doc.as_ref().map(|d| d.questions.len()).unwrap_or(0);
                egui::Frame::new()
                    .stroke(Stroke::new(1.0, theme::border()))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(theme::strong(truncate_chars(&s.name, 22)).size(12.5));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(theme::muted(plural(nq, "question")).size(11.5));
                            });
                        });
                    });
            }
            ui.add_space(14.0);
        }
    }
    options_and_finish(app, ui, st, flags);
}

fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n).chain(std::iter::once('…')).collect() }
}

fn options_and_finish(app: &mut crate::App, ui: &mut egui::Ui, st: &mut State, flags: &[Vec<bool>]) {
    let skip = st.skip_dups;
    let dups: usize = flags.iter().map(|f| f.iter().filter(|d| **d).count()).sum();
    let n: usize = st
        .sources
        .iter()
        .enumerate()
        .map(|(si, s)| (0..s.sel.len()).filter(|&i| included(skip, s, &flags[si], i)).count())
        .sum();
    let tests: usize = st
        .sources
        .iter()
        .filter_map(|s| s.doc.as_ref().map(|d| (0..d.exams.len()).filter(|&e| s.exam_sel.get(e).copied().unwrap_or(true)).count()))
        .sum();

    let y = ui.cursor().top();
    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::rule()));
    ui.add_space(10.0);
    kit::small_caps(ui, "Add to bank");
    let current = match &st.dest {
        Some(id) => app.store.bank(id).map(|b| b.name.clone()).unwrap_or_else(|| "New bank".to_string()),
        None => "New bank".to_string(),
    };
    egui::ComboBox::from_id_salt("imp_dest")
        .width(ui.available_width() - 4.0)
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
        ui.add(egui::TextEdit::singleline(&mut st.new_name).desired_width(ui.available_width() - 4.0).hint_text(hint));
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        kit::small_caps(ui, "Default points");
        ui.add(egui::DragValue::new(&mut st.points).range(0.0..=100.0).speed(0.25));
    });
    ui.label(theme::muted("for questions without any").size(10.5));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if cb(ui, st.skip_dups) {
            st.skip_dups = !st.skip_dups;
        }
        ui.label(theme::text(format!("Skip duplicates ({dups} found)")));
    });

    ui.add_space(12.0);
    let tests_note = if tests > 0 { format!(" and {}", plural(tests, "test")) } else { String::new() };
    let note = match st.dest.as_ref().and_then(|id| app.store.bank(id)) {
        Some(b) => {
            let cur = b.questions.len();
            format!("Adds {} to “{}” ({cur} + {n} = {}){tests_note}.", plural(n, "question"), b.name, cur + n)
        }
        None => format!("Will create the new bank “{}” with {}{tests_note}.", bank_name_for(st), plural(n, "question")),
    };
    ui.add(egui::Label::new(theme::muted(note).size(12.0)).wrap());
    ui.add_space(8.0);
    let label = format!("Import {}", plural(n, "question"));
    if ui.add_enabled_ui(n > 0, |ui| kit::primary_button(ui, &label)).inner.clicked() {
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
    // Named before the sources are taken (the default name comes from the first file).
    let new_bank_name = bank_name_for(st);
    struct Done {
        name: String,
        how: &'static str,
        count: usize,
    }
    let mut picked: Vec<Question> = Vec::new();
    let mut exams: Vec<model::Exam> = Vec::new();
    let mut done: Vec<Done> = Vec::new();
    let mut keep: Vec<Source> = Vec::new();
    // Normalized stem -> the question that will hold it, so a test that places a skipped
    // duplicate points at the copy that is kept (or already in the destination bank).
    let mut by_stem: std::collections::HashMap<String, Id> = std::collections::HashMap::new();
    if let Some(b) = st.dest.as_ref().and_then(|id| app.store.bank(id)) {
        for q in &b.questions {
            by_stem.entry(norm(&q.stem)).or_insert_with(|| q.id.clone());
        }
    }
    for (si, mut s) in std::mem::take(&mut st.sources).into_iter().enumerate() {
        let Some(doc) = s.doc.take() else {
            keep.push(s);
            continue;
        };
        let dup = &flags[si];
        let inc: Vec<bool> = (0..doc.questions.len()).map(|i| included(skip, &s, dup, i)).collect();
        let taken: HashSet<Id> = doc.questions.iter().zip(&inc).filter(|(_, t)| **t).map(|(q, _)| q.id.clone()).collect();
        let stems: std::collections::HashMap<Id, String> = doc.questions.iter().map(|q| (q.id.clone(), norm(&q.stem))).collect();
        for q in doc.questions.iter().filter(|q| taken.contains(&q.id)) {
            by_stem.entry(norm(&q.stem)).or_insert_with(|| q.id.clone());
        }
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
        // Ticked tests come along, keeping only the questions that are imported.
        for (ei, mut ex) in doc.exams.into_iter().enumerate() {
            if !s.exam_sel.get(ei).copied().unwrap_or(true) {
                continue;
            }
            ex.items = ex
                .items
                .into_iter()
                .filter_map(|mut it| {
                    if !it.bank_id.is_empty() || taken.contains(&it.question_id) {
                        return Some(it);
                    }
                    let kept = stems.get(&it.question_id).and_then(|n| by_stem.get(n))?;
                    it.question_id = kept.clone();
                    Some(it)
                })
                .collect();
            if !ex.items.is_empty() {
                exams.push(ex);
            }
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
        let name = new_bank_name;
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
    fn dup_key_compares_full_content_not_lead_in() {
        use crate::formats::make_question;
        use crate::model::{Kind, Part, PartKind};
        let multi = |a: &str| {
            make_question(
                "Use the table.\n| a | b |".into(),
                Kind::Multipart { parts: vec![Part { stem: a.into(), kind: PartKind::Short { sample: String::new() } }] },
            )
        };
        assert_ne!(dup_key(&multi("NH3")), dup_key(&multi("CO2")), "same lead-in, different parts");
        assert_eq!(dup_key(&multi("CO2")), dup_key(&multi("  co2 ")), "whitespace/case only");
        let tf = |s: &str, c: bool| make_question(s.into(), Kind::Tf { correct: c });
        assert_eq!(dup_key(&tf("The sun is a star.", true)), dup_key(&tf("The  Sun is a star.", true)));
        assert_ne!(dup_key(&tf("The sun is a star.", true)), dup_key(&tf("The sun is a star.", false)));
        assert_ne!(dup_key(&tf("Same stem", true)), dup_key(&make_question("Same stem".into(), Kind::Short { sample: String::new() })));
    }

    #[test]
    fn norm_collapses_space_and_case() {
        assert_eq!(norm("  Hello   World "), "hello world");
    }
}
