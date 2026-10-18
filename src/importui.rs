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
use crate::screens::{fmt_time, import_how_label, mime_of, now_secs, remap_empty_bank_ids, shell_frame, ImportFile};
use crate::theme;

/// Bytes read asynchronously from browser file drops, drained on the next frame.
#[cfg(target_arch = "wasm32")]
thread_local! {
    static DROPPED_BYTES: RefCell<Vec<Result<(String, Vec<u8>), String>>> = const { RefCell::new(Vec::new()) };
    static RESOLVE_BYTES: RefCell<Vec<Result<(String, Vec<u8>), String>>> = const { RefCell::new(Vec::new()) };
}

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
    /// The file format's own version, when it states one (a package's question-bank record
    /// version, which is what the bank row shows).
    version: Option<String>,
    /// Each test's own format version, as the source states it (a package's exams).
    exam_versions: Vec<Option<String>>,
    /// Which of the doc's tests (exams) to import.
    exam_sel: Vec<bool>,
    /// Preview texture of a photo / PDF page waiting for the AI.
    tex: Option<egui::TextureHandle>,
    /// The page image a photo / PDF page source was read from, kept after the AI reads it
    /// so a picture placeholder can be cropped from it.
    page: Option<Vec<u8>>,
}

/// The "Add a picture" dialog for one question of the import preview.
#[derive(Clone, Default)]
struct Attach {
    /// Source and question index in its doc.
    si: usize,
    k: usize,
    /// Which crop page is shown (index into `crop_pages`).
    page: usize,
    /// Selection on that page, normalized 0..1 (min x, min y, max x, max y).
    sel: Option<[f32; 4]>,
    drag_from: Option<Pos2>,
    tex: Option<(usize, egui::TextureHandle)>,
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
    /// Open "Add a picture" dialog.
    attach: Option<Attach>,
    /// Live read of the paste box: (text it was computed for, result).
    paste_read: Option<(String, Result<PasteRead, String>)>,
    /// Drawn inside the bank's Import modal (which lists the formats itself).
    in_modal: bool,
}

/// What the paste box's text reads as, live while typing.
#[derive(Clone, Debug, PartialEq)]
struct PasteRead {
    /// "Read as <format>".
    format: String,
    found: usize,
    /// Found minus duplicates (of each other, or of the destination bank).
    coming: usize,
}

/// Reads pasted text the way "Add pasted text" will: its format label, how many questions
/// it holds and how many would come in once duplicates are skipped.
fn read_paste(text: &str, existing: &HashSet<String>, skip_dups: bool) -> Result<PasteRead, String> {
    let name = "Pasted text";
    let bytes = text.as_bytes();
    let head: String = text.chars().take(4000).collect();
    let format = crate::formats::format_label(bytes, name).map(str::to_string).unwrap_or_else(|| text_kind(name, "", &head).to_string());
    let doc = crate::formats::detect_and_parse(bytes, name)?;
    let found = doc.questions.len();
    let mut seen = existing.clone();
    let coming = if skip_dups { doc.questions.iter().filter(|q| seen.insert(dup_key(q))).count() } else { found };
    Ok(PasteRead { format, found, coming })
}

/// "3 questions found · 3 coming in".
fn paste_counts_line(r: &PasteRead) -> (String, String) {
    (format!("{} found · ", plural(r.found, "question")), format!("{} coming in", r.coming))
}

thread_local! {
    static ST: RefCell<State> = RefCell::new(State::default());
    /// The bank Import modal's own staging (the Imports page keeps what it has staged).
    static MST: RefCell<State> = RefCell::new(State::default());
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

/// Format versions a source states: the question bank's own (for a Test Parrot package that
/// is the embedded question-bank record's `formatVersion`, e.g. 0.10.0, not the package's
/// 0.1.0) and each test's (`exams[i].formatVersion`, e.g. 0.5.0) — as Test Parrot's import
/// preview shows them for the picked item.
#[derive(Clone, Debug, Default, PartialEq)]
struct SourceVersions {
    bank: Option<String>,
    exams: Vec<Option<String>>,
}

fn source_versions(data: &[u8]) -> SourceVersions {
    fn ver(v: &serde_json::Value) -> Option<String> {
        match v.get("formatVersion").or_else(|| v.get("version"))? {
            serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }
    let whole = crate::formats::format_version(data);
    let head = String::from_utf8_lossy(&data[..data.len().min(64)]).to_string();
    if !head.trim_start().starts_with('{') {
        return SourceVersions { bank: whole, exams: Vec::new() };
    }
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(data) else {
        return SourceVersions { bank: whole, exams: Vec::new() };
    };
    let bank = v
        .get("questionBanks")
        .and_then(|b| b.as_array())
        .and_then(|b| b.first())
        .and_then(|e| e.get("record"))
        .and_then(ver)
        .or(whole);
    let exams = v.get("exams").and_then(|e| e.as_array()).map(|a| a.iter().map(ver).collect()).unwrap_or_default();
    SourceVersions { bank, exams }
}

/// The name an import source goes by in the preview: the bank / package title the file states
/// (Test Parrot shows "Chemistry Chapter 4 Quiz (5A and 5B)", not the file name), else the
/// file name.
fn source_title(s: &Source) -> String {
    s.doc.as_ref().map(|d| d.name.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| s.name.clone())
}

/// The footer line of the import review: what Import will do, tests included (Test Parrot:
/// "Will create the new Question Bank “X” and 2 Tests." / "Will create 2 Tests and merge “X”
/// into “Y”.").
fn import_footer(n: usize, tests: usize, source: Option<&str>, dest: Option<&str>, new_name: &str) -> String {
    let qs = plural(n, "question");
    match (dest, tests) {
        (Some(d), 0) => format!("Will add {qs} to “{d}”."),
        (Some(d), t) => match source {
            Some(src) if src != d => format!("Will create {} and merge “{src}” ({qs}) into “{d}”.", plural(t, "test")),
            _ => format!("Will create {} and add {qs} to “{d}”.", plural(t, "test")),
        },
        (None, 0) => format!("Will create the new bank “{new_name}” with {qs}."),
        (None, t) => format!("Will create the new bank “{new_name}” ({qs}) and {}.", plural(t, "test")),
    }
}

impl State {
    fn push_doc(&mut self, name: &str, kind: &str, how: &'static str, icon: Ic, doc: ImportDoc, versions: SourceVersions) {
        let n = doc.questions.len();
        let ne = doc.exams.len();
        let SourceVersions { bank: version, exams: exam_versions } = versions;
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
            exam_versions,
            exam_sel: vec![true; ne],
            tex: None,
            page: None,
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
            exam_versions: Vec::new(),
            exam_sel: Vec::new(),
            tex: None,
            page: None,
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
            // A bank-file PDF (verSHion's, or Test Parrot's) carries its questions as data:
            // read those instead of sending page pictures to the AI.
            if let Some(data) = crate::bankexport::embedded_data(&bytes) {
                let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
                match crate::formats::detect_and_parse(&data, &format!("{stem}.json")) {
                    Ok(doc) if !doc.questions.is_empty() => {
                        crate::bankexport::restore_media(&data);
                        let versions = source_versions(&data);
                        st.push_doc(name, "Question bank file (PDF)", how, Ic::File, doc, versions);
                        return;
                    }
                    _ => {} // not readable as questions: fall back to the pages
                }
            }
            match crate::platform::render_pdf_pages(&bytes) {
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
            if text_bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
                // a bank file's pictures travel inside it
                crate::bankexport::restore_media(&text_bytes);
            }
            let versions = source_versions(&text_bytes);
            st.push_doc(name, &kind, how, icon, doc, versions)
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
        match crate::platform::read(&path) {
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
                    s.page = s.image.take().map(|f| f.bytes);
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
            // Headless test hook: VERSHION_IMPORT_FILES=<path>[:<path>...] stages files once.
            if let Ok(v) = crate::env_var("VERSHION_IMPORT_FILES") {
                add_paths(self, &mut st, std::env::split_paths(&v).collect());
            }
            // QA hook: VERSHION_PASTE=<file> opens the paste box holding that file's text.
            if let Ok(p) = crate::env_var("VERSHION_PASTE") {
                if let Ok(t) = std::fs::read_to_string(p) {
                    self.paste = t;
                    st.paste_open = true;
                }
            }
        }
        intake(self, ctx, &mut st);
        if st.ai_all && !self.ai_busy {
            start_next_ai(self, &mut st, ctx);
        }
        shell_frame(self, ctx, true, |app, ui| page(app, ui, ctx, &mut st));
        attach_dialog(self, ctx, &mut st);
        ST.with(|s| *s.borrow_mut() = st);
    }
}

// ----- the bank's Import modal (the same import, in place over the bank) -----

fn modal_key() -> egui::Id {
    egui::Id::new("qs_import_modal")
}

/// Formats named under the drop zone of the Import modal.
pub const SUPPORTS: &[&str] = &[
    "PDFs", "Word documents", "Photos", "Blackboard", "Canvas", "Brightspace (D2L)", "Moodle (XML, GIFT, Aiken)", "QTI", "Respondus", "text2qti", "Kahoot",
    "Quizlet and Anki", "CSV and Excel spreadsheets", "Test Parrot files",
];

/// Opens the Import modal over bank `bank_id`: a fresh import whose questions go into that bank.
pub fn open_import_modal(ctx: &egui::Context, bank_id: &Id) {
    ctx.data_mut(|d| d.insert_temp(modal_key(), Some(bank_id.clone())));
    MST.with(|s| {
        let mut st = s.borrow_mut();
        st.sources.clear();
        st.pick = None;
        st.attach = None;
        st.paste_open = false;
        st.paste_read = None;
        st.dest = Some(bank_id.clone());
    });
}

pub fn import_modal_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Option<Id>>(modal_key())).flatten().is_some()
}

fn close_import_modal(ctx: &egui::Context, st: &mut State) {
    ctx.data_mut(|d| d.insert_temp::<Option<Id>>(modal_key(), None));
    st.sources.clear();
    st.pick = None;
    st.attach = None;
    st.paste_open = false;
}

impl crate::App {
    /// The bank editor's Import modal (call every frame on the bank screen): drop zone, file
    /// picker and paste box first; once something is read, the full review (sources, page
    /// preview, details, Import) grows inside the same modal. Importing lands in the bank.
    pub fn draw_import_modal(&mut self, ctx: &egui::Context) {
        let Some(bank_id) = ctx.data(|d| d.get_temp::<Option<Id>>(modal_key())).flatten() else { return };
        if self.store.bank(&bank_id).is_none() {
            ctx.data_mut(|d| d.insert_temp::<Option<Id>>(modal_key(), None));
            return;
        }
        let mut st = MST.with(|s| std::mem::take(&mut *s.borrow_mut()));
        if !st.inited {
            st.inited = true;
            st.skip_dups = true;
            st.points = 1.0;
        }
        if st.dest.is_none() && st.sources.is_empty() {
            st.dest = Some(bank_id.clone());
        }
        st.in_modal = true;
        // Headless test hook (same as the Imports page): stage files once.
        if let Ok(v) = crate::env_var("VERSHION_IMPORT_FILES") {
            let hid = egui::Id::new("qs_import_modal_files_hook");
            if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
                ctx.data_mut(|d| d.insert_temp(hid, true));
                add_paths(self, &mut st, std::env::split_paths(&v).collect());
            }
        }
        intake(self, ctx, &mut st);
        if st.ai_all && !self.ai_busy {
            start_next_ai(self, &mut st, ctx);
        }
        fix_pick(&mut st);
        let imports_before = self.store.imports.len();
        let screen = crate::theme::area_or(ctx.content_rect());
        let big = !st.sources.is_empty();
        let w = if big { (screen.width() - 80.0).clamp(520.0, 1500.0) } else { 600.0_f32.min(screen.width() - 40.0) };
        let view_h = (screen.height() - 150.0).max(420.0);
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("qs_import_modal_window")).show(ctx, |ui| {
            ui.set_width(w);
            ui.horizontal(|ui| {
                ui.label(theme::serif("Import").size(22.0).color(theme::ink()));
                if let Some(b) = self.store.bank(&bank_id) {
                    ui.label(theme::muted(format!("into “{}”", b.name)).size(13.0));
                }
            });
            ui.add_space(8.0);
            let body = |app: &mut crate::App, ui: &mut egui::Ui, st: &mut State| {
                drop_area(app, ui, st);
                if st.sources.is_empty() {
                    ui.add_space(10.0);
                    ui.vertical_centered(|ui| {
                        ui.label(theme::strong("Supports").size(13.0));
                    });
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                        for s in SUPPORTS {
                            let chip = egui::Button::new(theme::text(*s).size(12.0))
                                .wrap_mode(egui::TextWrapMode::Extend)
                                .fill(theme::accent_faded())
                                .stroke(Stroke::new(1.0, theme::border()))
                                .corner_radius(CornerRadius::same(10))
                                .sense(Sense::hover());
                            ui.add(chip);
                        }
                    });
                    ui.add_space(8.0);
                    privacy_note(app, ui);
                } else {
                    ui.add_space(10.0);
                    let flags = dup_flags(app, st);
                    workspace(app, ui, ctx, st, &flags, w, view_h);
                }
            };
            egui::ScrollArea::vertical().id_salt("imp_modal_scroll").max_height(screen.height() - 140.0).auto_shrink([false, true]).show(ui, |ui| {
                ui.allocate_ui_with_layout(Vec2::new(w, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(w);
                    body(self, ui, &mut st);
                });
            });
            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.set_width(w);
                let mut go = false;
                if big {
                    let flags = dup_flags(self, &st);
                    let n: usize = st.sources.iter().enumerate().map(|(si, s)| (0..s.sel.len()).filter(|&i| included(st.skip_dups, s, &flags[si], i)).count()).sum();
                    let dest = st.dest.as_ref().and_then(|id| self.store.bank(id)).map(|b| b.name.clone());
                    let tests = tests_ticked(&st);
                    let source = (st.sources.len() == 1).then(|| source_title(&st.sources[0]));
                    ui.label(theme::muted(import_footer(n, tests, source.as_deref(), dest.as_deref(), &bank_name_for(&st))).size(12.5));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add_enabled_ui(n > 0, |ui| kit::primary_button(ui, "Import")).inner.clicked() {
                            go = true;
                        }
                        if kit::ghost_button(ui, "Cancel").clicked() {
                            cancel = true;
                        }
                    });
                    if go {
                        commit(self, &mut st, &flags);
                    }
                } else {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if kit::ghost_button(ui, "Cancel").clicked() {
                            cancel = true;
                        }
                    });
                }
            });
        });
        let imported = self.store.imports.len() > imports_before;
        if cancel || imported || (modal.should_close() && st.attach.is_none()) {
            // (an import already opened the bank it went into)
            close_import_modal(ctx, &mut st);
        }
        attach_dialog(self, ctx, &mut st);
        MST.with(|s| *s.borrow_mut() = st);
    }
}

/// OS drag-and-drop and Ctrl+V of plain text.
fn intake(app: &mut crate::App, ctx: &egui::Context, st: &mut State) {
    #[cfg(target_arch = "wasm32")]
    for item in DROPPED_BYTES.with(|q| std::mem::take(&mut *q.borrow_mut())) {
        match item {
            Ok((name, b)) => add_bytes(app, st, &name, b, "file"),
            Err(e) => app.toast_err(e),
        }
    }
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    for f in dropped {
        #[cfg(not(target_arch = "wasm32"))]
        {
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
        #[cfg(target_arch = "wasm32")]
        {
            let name = f.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped file".to_string());
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let item = match f.bytes_async().await {
                    Ok(b) => Ok((name.clone(), b)),
                    Err(e) => Err(format!("Couldn't read {name}: {e}")),
                };
                DROPPED_BYTES.with(|q| q.borrow_mut().push(item));
                ctx.request_repaint();
            });
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
                        if let Some(paths) = crate::platform::pick_files(
                            "Question files",
                            &[
                                "txt", "csv", "tsv", "json", "docx", "xlsx", "gift", "xml", "qti", "zip", "imscc", "md", "pdf", "png", "jpg",
                                "jpeg", "gif", "webp",
                            ],
                        ) {
                            add_paths(app, st, paths);
                        }
                    }
                    if kit::ghost_button(ui, "Paste text").clicked() {
                        st.paste_open = !st.paste_open;
                    }
                    if kit::ghost_button(ui, "Add photo").clicked() {
                        if let Some(paths) = crate::platform::pick_files("Photos & PDFs", &["png", "jpg", "jpeg", "gif", "webp", "pdf"]) {
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
                    paste_live_line(app, ui, st);
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
                if !compact && !st.in_modal {
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

/// "Read as <format>" and "N questions found · N coming in" under the paste box, live.
fn paste_live_line(app: &crate::App, ui: &mut egui::Ui, st: &mut State) {
    let text = app.paste.trim();
    if text.is_empty() {
        st.paste_read = None;
        return;
    }
    if st.paste_read.as_ref().is_none_or(|(t, _)| t != text) {
        let mut existing: HashSet<String> = HashSet::new();
        if let Some(b) = st.dest.as_ref().and_then(|id| app.store.bank(id)) {
            existing.extend(b.questions.iter().map(dup_key));
        }
        st.paste_read = Some((text.to_string(), read_paste(text, &existing, st.skip_dups)));
    }
    let Some((_, res)) = &st.paste_read else { return };
    ui.add_space(4.0);
    match res {
        Ok(r) => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(theme::text(format!("Read as {}", r.format)).size(12.5));
                ui.label(theme::muted("  ·  ").size(12.5));
                let (a, b) = paste_counts_line(r);
                ui.label(theme::text(a).size(12.5));
                ui.label(theme::strong(b).size(12.5));
            });
        }
        Err(e) => {
            let e: String = e.chars().take(140).collect();
            ui.add(egui::Label::new(theme::muted(format!("Not readable yet: {e}")).size(12.0)).wrap());
        }
    }
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
        let title = source_title(s);
        match list_row(ui, egui::Id::new(("imp_src", i)), picked, on, &title, &sub, tag, s.status != Status::Converting) {
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
            let from = truncate_chars(&source_title(s), 28);
            for (e, ex) in doc.exams.iter().enumerate() {
                let on = s.exam_sel.get(e).copied().unwrap_or(true);
                let sub = format!("{} · from {from}", plural(ex.items.len(), "question"));
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
    if s.how == "paste" {
        if let Some(d) = &s.doc {
            let coming = (0..d.questions.len()).filter(|&k| included(skip, s, dup, k)).count();
            let r = PasteRead { format: s.kind.clone(), found: d.questions.len(), coming };
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(theme::text(format!("Read as {}", r.format)).size(12.5));
                ui.label(theme::muted("  ·  ").size(12.5));
                let (a, b) = paste_counts_line(&r);
                ui.label(theme::text(a).size(12.5));
                ui.label(theme::strong(b).size(12.5));
            });
            ui.add_space(8.0);
        }
    }
    let clickable: usize = s
        .doc
        .as_ref()
        .map(|d| order.iter().flatten().filter(|&&k| stem_picture_open(&d.questions[k])).count())
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
                            theme::text(picture_banner(pictures, clickable))
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
    let mut open_attach: Option<usize> = None;
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
        if let (Some(k), Some(q)) = (*k, q) {
            if stem_picture_open(q) {
                // The question's picture box: click anywhere on it (or the margin badge) to add one.
                let hit = ui.interact(rect, ui.id().with(("imp_q_pic", si, k)), Sense::click());
                let badge = Rect::from_center_size(Pos2::new(rect.right() + side * 0.5, rect.top() + bp * 0.75), Vec2::splat(18.0));
                let bhit = ui.interact(badge.expand(3.0), ui.id().with(("imp_q_pic_b", si, k)), Sense::click());
                let hot = hit.hovered() || bhit.hovered();
                let p = ui.painter();
                p.circle_filled(badge.center(), 9.0, if hot { theme::accent_col() } else { Color32::from_rgb(0xb0, 0x3a, 0x2e) });
                let ic = Rect::from_center_size(badge.center(), Vec2::new(10.0, 8.0));
                let w = Stroke::new(1.2, Color32::WHITE);
                p.rect_stroke(ic, CornerRadius::same(1), w, StrokeKind::Middle);
                p.line_segment([ic.left_bottom() + Vec2::new(1.5, -1.5), ic.center() + Vec2::new(0.0, 0.5)], w);
                p.line_segment([ic.center() + Vec2::new(0.0, 0.5), ic.right_bottom() + Vec2::new(-1.5, -1.5)], w);
                if hot {
                    p.rect_stroke(rect.expand2(Vec2::new(5.0, 4.0)), CornerRadius::same(3), Stroke::new(1.2, theme::accent_col()), StrokeKind::Outside);
                    let g = p.layout_no_wrap("Add a picture".into(), FontId::new(10.5, theme::bold_family()), theme::on_accent_col());
                    let pr = Rect::from_min_size(Pos2::new(rect.left(), rect.top() - g.size().y - 8.0), g.size() + Vec2::new(14.0, 5.0));
                    p.rect_filled(pr, CornerRadius::same(8), theme::accent_col());
                    p.galley(pr.min + Vec2::new(7.0, 2.5), g, theme::on_accent_col());
                }
                if hit.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() || bhit.on_hover_text("Add a picture").clicked() {
                    open_attach = Some(k);
                }
            }
        }
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
    if let Some(k) = open_attach {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("imp_open_attach"), Some((si, k))));
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
                kit::small_caps(ui, if e.is_some() { "Test" } else if s.doc.is_some() { "Question bank" } else { "Questions" });
            });
            let name = match (e, &s.doc) {
                (Some(e), Some(d)) => d.exams[e].name.clone(),
                _ => source_title(s),
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
            if source_title(s) != s.name {
                detail_row(ui, "File", &truncate_chars(&s.name, 34), false);
            }
            let version = match e {
                Some(e) => s.exam_versions.get(e).cloned().flatten(),
                None => s.version.clone(),
            };
            if let Some(v) = &version {
                detail_row(ui, "Format version", v, true);
            }
            // A bank that comes with tests: which of them use it (Test Parrot's "Used by").
            if let (None, Some(d)) = (e, &s.doc) {
                if !d.exams.is_empty() {
                    ui.add_space(8.0);
                    kit::small_caps(ui, "Used by");
                    ui.add_space(4.0);
                    for ex in &d.exams {
                        egui::Frame::new()
                            .stroke(Stroke::new(1.0, theme::border()))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(10, 5))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                                    paint_icon(ui.painter(), Ic::File, r.center(), theme::accent_col(), 0.6);
                                    ui.label(theme::strong(truncate_chars(&ex.name, 30)).size(12.5));
                                });
                            });
                        ui.add_space(3.0);
                    }
                }
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
                            ui.label(theme::strong(truncate_chars(&source_title(s), 22)).size(12.5));
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
    let tests = tests_ticked(st);

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

/// Tests (exams) ticked for import across the sources.
fn tests_ticked(st: &State) -> usize {
    st.sources
        .iter()
        .filter_map(|s| s.doc.as_ref().map(|d| (0..d.exams.len()).filter(|&e| s.exam_sel.get(e).copied().unwrap_or(true)).count()))
        .sum()
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
            details: Default::default(),
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

// ----- adding a picture to a placeholder before importing -----

/// True when the question's stem shows a "Picture needed" box that its own picture could
/// fill (a stem marker and no attached picture yet). Markers in answer keys are counted but
/// are not printed on the page.
fn stem_picture_open(q: &Question) -> bool {
    q.image.is_none()
        && crate::bankops::split_pictures(&q.stem).iter().any(|p| matches!(p, crate::bankops::StemPart::Picture(_)))
}

/// The preview banner: how many pictures are needed and how to add the ones on the page.
fn picture_banner(pictures: usize, clickable: usize) -> String {
    let head = format!("{} needed.", plural(pictures, "picture"));
    let keys = pictures.saturating_sub(clickable);
    match (clickable, keys) {
        (0, _) => format!("{head} They are in answer keys; edit them in the question editor after importing."),
        (_, 0) if pictures == 1 => format!("{head} Click it to add it, or add it after importing."),
        (_, 0) => format!("{head} Click one to add it, or add them after importing."),
        (c, k) => format!(
            "{head} Click {} on the page to add {}; {} in answer keys can be edited after importing.",
            if c == 1 { "the box" } else { "a box" },
            if c == 1 { "it" } else { "one" },
            if k == 1 { "the one".to_string() } else { format!("the {k}") }
        ),
    }
}

/// Crops `sel` (normalized min x, min y, max x, max y) out of an image and returns PNG bytes.
fn crop_png(bytes: &[u8], sel: [f32; 4]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("couldn't read that page: {e}"))?;
    let (w, h) = (img.width() as f32, img.height() as f32);
    let x0 = (sel[0].min(sel[2]).clamp(0.0, 1.0) * w).floor() as u32;
    let y0 = (sel[1].min(sel[3]).clamp(0.0, 1.0) * h).floor() as u32;
    let x1 = (sel[0].max(sel[2]).clamp(0.0, 1.0) * w).ceil() as u32;
    let y1 = (sel[1].max(sel[3]).clamp(0.0, 1.0) * h).ceil() as u32;
    if x1 <= x0 + 2 || y1 <= y0 + 2 {
        return Err("select a larger area".to_string());
    }
    let part = img.crop_imm(x0, y0, x1 - x0, y1 - y0);
    let mut out = std::io::Cursor::new(Vec::new());
    part.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// Gives the question its picture (it fills the first stem placeholder).
fn attach_picture(q: &mut Question, img: model::ImageRef) {
    q.image = Some(img);
    q.modified = now_secs();
}

/// Pages a picture can be cropped from: the question's own page (a photo the AI read),
/// then every photo / PDF page in the import list.
fn crop_pages(st: &State, si: usize) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    if let Some(p) = st.sources.get(si).and_then(|s| s.page.clone()) {
        out.push((st.sources[si].name.clone(), p));
    }
    for (i, s) in st.sources.iter().enumerate() {
        if i == si {
            continue;
        }
        if let Some(b) = s.page.clone().or_else(|| s.image.as_ref().map(|f| f.bytes.clone())) {
            out.push((s.name.clone(), b));
        }
    }
    out
}

fn attach_dialog(app: &mut crate::App, ctx: &egui::Context, st: &mut State) {
    let oid = egui::Id::new("imp_open_attach");
    if let Some((si, k)) = ctx.data_mut(|d| d.remove_temp::<Option<(usize, usize)>>(oid)).flatten() {
        st.attach = Some(Attach { si, k, ..Default::default() });
    }
    // QA hook: VERSHION_ATTACH=<question n> opens the dialog for the first source's n-th question.
    if let Ok(v) = crate::env_var("VERSHION_ATTACH") {
        let hid = egui::Id::new("imp_attach_hook");
        if !st.sources.is_empty() && !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
            ctx.data_mut(|d| d.insert_temp(hid, true));
            let n: usize = v.trim().parse().unwrap_or(1);
            st.attach = Some(Attach { si: 0, k: n.saturating_sub(1), ..Default::default() });
        }
    }
    let Some(mut at) = st.attach.take() else { return };
    let Some(q) = st.sources.get(at.si).and_then(|s| s.doc.as_ref()).and_then(|d| d.questions.get(at.k)).cloned() else {
        return;
    };
    let pages = crop_pages(st, at.si);
    at.page = at.page.min(pages.len().saturating_sub(1));
    let alt = crate::bankops::split_pictures(&q.stem).into_iter().find_map(|p| match p {
        crate::bankops::StemPart::Picture(a) => Some(a),
        _ => None,
    });
    let mut close = false;
    let mut picked: Option<Vec<u8>> = None;
    let modal = egui::Modal::new(egui::Id::new("imp_attach_modal")).show(ctx, |ui| {
      egui::Frame::new().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
        ui.set_width(if pages.is_empty() { 460.0 } else { 640.0 });
        ui.label(theme::serif("Add a picture").size(22.0).color(theme::ink()));
        ui.add_space(2.0);
        let text_only: String = crate::bankops::split_pictures(&q.stem)
            .into_iter()
            .filter_map(|p| match p {
                crate::bankops::StemPart::Text(t) => Some(t),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
        let preview: String = crate::equation::stem_preview(&text_only).chars().take(140).collect();
        ui.add(egui::Label::new(theme::muted(preview).size(12.0)).wrap());
        if let Some(Some(a)) = &alt {
            ui.add_space(4.0);
            ui.add(egui::Label::new(theme::text(format!("Placeholder: {a}")).size(12.5)).wrap());
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if kit::primary_button(ui, "Choose image file…").clicked() {
                if let Some(path) = crate::platform::pick_file("Pictures", &["png", "jpg", "jpeg", "gif", "webp"]) {
                    match crate::platform::read(&path) {
                        Ok(b) => picked = Some(b),
                        Err(e) => app.toast_err(format!("Couldn't read that file: {e}")),
                    }
                }
            }
            if pages.is_empty() {
                ui.label(theme::muted("Add the PDF or photo it came from to crop from its page.").size(11.5));
            }
        });
        if !pages.is_empty() {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(theme::strong("Or crop it from a page").size(13.0));
                if pages.len() > 1 {
                    let mut pg = at.page;
                    egui::ComboBox::from_id_salt("imp_attach_page")
                        .selected_text(truncate_chars(&pages[pg].0, 34))
                        .show_ui(ui, |ui| {
                            for (i, (name, _)) in pages.iter().enumerate() {
                                ui.selectable_value(&mut pg, i, name.clone());
                            }
                        });
                    if pg != at.page {
                        at.page = pg;
                        at.sel = None;
                    }
                }
            });
            ui.label(theme::muted("Drag a box around the picture.").size(11.5));
            ui.add_space(4.0);
            if at.tex.as_ref().is_none_or(|(i, _)| *i != at.page) {
                at.tex = image::load_from_memory(&pages[at.page].1).ok().map(|img| {
                    let rgba = img.to_rgba8();
                    let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
                    (at.page, ctx.load_texture(format!("imp_attach_page_{}", at.page), ci, egui::TextureOptions::LINEAR))
                });
            }
            if let Some((_, tex)) = &at.tex {
                let ts = tex.size_vec2();
                let scale = (ui.available_width() / ts.x).min(520.0 / ts.y).min(1.0);
                let (r, resp) = ui.allocate_exact_size(ts * scale, Sense::drag());
                ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                ui.painter().rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, theme::border()), StrokeKind::Outside);
                let norm = |p: Pos2| Pos2::new(((p.x - r.left()) / r.width()).clamp(0.0, 1.0), ((p.y - r.top()) / r.height()).clamp(0.0, 1.0));
                if resp.drag_started() {
                    at.drag_from = resp.interact_pointer_pos().map(norm);
                }
                if let (Some(a), Some(p)) = (at.drag_from, resp.interact_pointer_pos()) {
                    if resp.dragged() || resp.drag_stopped() {
                        let b = norm(p);
                        at.sel = Some([a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]);
                    }
                }
                if resp.drag_stopped() {
                    at.drag_from = None;
                }
                if let Some(s) = at.sel {
                    let sr = Rect::from_min_max(
                        Pos2::new(r.left() + s[0] * r.width(), r.top() + s[1] * r.height()),
                        Pos2::new(r.left() + s[2] * r.width(), r.top() + s[3] * r.height()),
                    );
                    // Dim everything outside the selection.
                    let dim = Color32::from_black_alpha(90);
                    let p = ui.painter();
                    p.rect_filled(Rect::from_min_max(r.min, Pos2::new(r.right(), sr.top())), CornerRadius::ZERO, dim);
                    p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), sr.bottom()), r.max), CornerRadius::ZERO, dim);
                    p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), sr.top()), Pos2::new(sr.left(), sr.bottom())), CornerRadius::ZERO, dim);
                    p.rect_filled(Rect::from_min_max(Pos2::new(sr.right(), sr.top()), Pos2::new(r.right(), sr.bottom())), CornerRadius::ZERO, dim);
                    p.rect_stroke(sr, CornerRadius::ZERO, Stroke::new(1.5, theme::accent_col()), StrokeKind::Outside);
                }
            }
        }
        ui.add_space(10.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !pages.is_empty() {
                let ready = at.sel.is_some();
                if ui.add_enabled_ui(ready, |ui| kit::primary_button(ui, "Use selected area")).inner.clicked() {
                    if let Some(sel) = at.sel {
                        match crop_png(&pages[at.page].1, sel) {
                            Ok(b) => picked = Some(b),
                            Err(e) => app.toast_err(format!("Couldn't crop: {e}")),
                        }
                    }
                }
            }
            if kit::ghost_button(ui, "Cancel").clicked() {
                close = true;
            }
        });
      });
    });
    let mut done = false;
    if let Some(bytes) = picked {
        match crate::store::put_image(&bytes) {
            Ok(img) => {
                if let Some(q) = st.sources.get_mut(at.si).and_then(|s| s.doc.as_mut()).and_then(|d| d.questions.get_mut(at.k)) {
                    attach_picture(q, img);
                }
                app.toast("Picture added");
                done = true;
            }
            Err(e) => app.toast_err(format!("Couldn't use that picture: {e}")),
        }
    }
    if !(done || close || modal.should_close()) {
        st.attach = Some(at);
    }
}

// ----- Resolve Images (the bank page's "N pictures needed" link) -----

/// The bank's Resolve Images dialog: every picture placeholder of the bank, with an upload,
/// a crop from the original PDF / photo, a picture from the original Word file, or the
/// placeholder's description in words; "Use these pictures" applies them all at once.
#[derive(Default)]
struct Resolve {
    bank: Id,
    /// Placeholders when the dialog opened (re-checked before applying).
    slots: Vec<crate::bankops::PictureSlot>,
    /// Pages a picture can be cropped from (PDF pages, photos): (name, PNG/JPEG bytes).
    pages: Vec<(String, Vec<u8>)>,
    /// Pictures found inside a dropped Word document.
    doc_images: Vec<(String, Vec<u8>)>,
    /// Slot index -> the picture chosen for it.
    staged: std::collections::HashMap<usize, Vec<u8>>,
    /// Slots resolved by writing their description instead.
    described: HashSet<usize>,
    /// Slot whose crop / Word-picture panel is open.
    panel: Option<usize>,
    page: usize,
    sel: Option<[f32; 4]>,
    drag_from: Option<Pos2>,
    /// Texture cache: key -> texture (pages, staged pictures, Word pictures).
    tex: std::collections::HashMap<String, egui::TextureHandle>,
}

thread_local! {
    static RESOLVE: RefCell<Option<Resolve>> = const { RefCell::new(None) };
}

/// Opens Resolve Images for a bank.
pub fn open_resolve_images(app: &crate::App, bank_id: &Id) {
    let slots = app.store.bank(bank_id).map(|b| crate::bankops::picture_slots(&b.questions)).unwrap_or_default();
    RESOLVE.with(|r| *r.borrow_mut() = Some(Resolve { bank: bank_id.clone(), slots, ..Default::default() }));
}

/// "k of N pictures ready. …" (Test Parrot's dialog subtitle).
fn resolve_subtitle(ready: usize, total: usize) -> String {
    format!("{ready} of {} ready. Any left unresolved stay as “picture needed” and can be added later.", plural(total, "picture"))
}

/// Applies the chosen pictures and descriptions to a bank's questions: an attached picture
/// becomes the question's picture (it fills the first stem placeholder); a described
/// placeholder is replaced by its words. Returns how many placeholders were resolved.
fn apply_resolution(
    qs: &mut [Question],
    slots: &[crate::bankops::PictureSlot],
    pictures: &std::collections::HashMap<usize, model::ImageRef>,
    described: &HashSet<usize>,
    now: i64,
) -> usize {
    let mut n = 0;
    for (k, slot) in slots.iter().enumerate() {
        if let (true, Some(img)) = (slot.attachable, pictures.get(&k)) {
            if let Some(q) = qs.get_mut(slot.qi) {
                q.image = Some(img.clone());
                q.modified = now;
                n += 1;
            }
        }
    }
    // Later markers of a field first, so earlier ones keep their position.
    let mut desc: Vec<&crate::bankops::PictureSlot> = described.iter().filter(|k| !pictures.contains_key(k)).filter_map(|&k| slots.get(k)).collect();
    desc.sort_by(|a, b| (a.qi, b.nth).cmp(&(b.qi, a.nth)));
    for slot in desc {
        let Some(q) = qs.get_mut(slot.qi) else { continue };
        // An attached picture covers the first stem marker: describing it would be lost.
        if let Some(t) = crate::bankops::field_text_mut(q, slot.field) {
            let next = crate::bankops::describe_marker(t, slot.nth);
            if next != *t {
                *t = next;
                q.modified = now;
                n += 1;
            }
        }
    }
    n
}

/// Pictures inside a .docx (word/media), in archive order.
fn docx_images(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    use std::io::Read;
    let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else { return Vec::new() };
    let mut out = Vec::new();
    for i in 0..z.len() {
        let Ok(mut f) = z.by_index(i) else { continue };
        let name = f.name().to_string();
        let lower = name.to_ascii_lowercase();
        if !lower.starts_with("word/media/") || !(lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".gif")) {
            continue;
        }
        let mut b = Vec::new();
        if f.read_to_end(&mut b).is_ok() {
            out.push((name.rsplit('/').next().unwrap_or(&name).to_string(), b));
        }
    }
    out
}

/// Adds a dropped / chosen original: PDF pages and photos to crop from, a Word file's pictures.
fn resolve_add_file(app: &mut crate::App, rs: &mut Resolve, name: &str, bytes: Vec<u8>) {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "pdf" => match crate::platform::render_pdf_pages(&bytes) {
            Ok(pages) => {
                let n = pages.len();
                for (i, png) in pages.into_iter().enumerate() {
                    rs.pages.push((if n == 1 { name.to_string() } else { format!("{name} · page {}", i + 1) }, png));
                }
            }
            Err(e) => app.toast_err(e),
        },
        "docx" => {
            let imgs = docx_images(&bytes);
            if imgs.is_empty() {
                app.toast_err(format!("{name} has no pictures in it."));
            }
            rs.doc_images.extend(imgs.into_iter().map(|(n, b)| (format!("{name} · {n}"), b)));
        }
        _ if image::guess_format(&bytes).is_ok() => rs.pages.push((name.to_string(), bytes)),
        _ => app.toast_err(format!("{name}: drop the original PDF, Word document or a photo of it.")),
    }
}

fn tex_of(ctx: &egui::Context, cache: &mut std::collections::HashMap<String, egui::TextureHandle>, key: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    if let Some(t) = cache.get(key) {
        return Some(t.clone());
    }
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
    let t = ctx.load_texture(format!("resolve_{key}"), ci, egui::TextureOptions::LINEAR);
    cache.insert(key.to_string(), t.clone());
    Some(t)
}

/// Red dashed "Picture needed" box (Test Parrot's placeholder thumbnail).
fn needed_box(ui: &mut egui::Ui, size: Vec2, alt: Option<&str>) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    let red = theme::red();
    p.rect_filled(rect, CornerRadius::same(4), red.gamma_multiply(0.10));
    p.extend(Shape::dashed_line(&[rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()], Stroke::new(1.0, red), 4.0, 3.0));
    let has_alt = alt.is_some_and(|a| !a.trim().is_empty());
    let cy = if has_alt { rect.center().y - 7.0 } else { rect.center().y };
    p.text(Pos2::new(rect.center().x, cy), egui::Align2::CENTER_CENTER, "Picture needed", FontId::new(12.5, theme::bold_family()), red);
    if let Some(a) = alt.filter(|a| !a.trim().is_empty()) {
        p.text(Pos2::new(rect.center().x, cy + 15.0), egui::Align2::CENTER_CENTER, truncate_chars(a.trim(), 26), FontId::new(10.0, egui::FontFamily::Proportional), theme::muted_col());
    }
}

/// A page with a drag-to-select box; returns nothing, updates `sel` / `drag_from`.
fn crop_canvas(ui: &mut egui::Ui, tex: &egui::TextureHandle, max_h: f32, sel: &mut Option<[f32; 4]>, drag_from: &mut Option<Pos2>) {
    let ts = tex.size_vec2();
    let scale = (ui.available_width() / ts.x).min(max_h / ts.y).min(1.0);
    let (r, resp) = ui.allocate_exact_size(ts * scale, Sense::drag());
    ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    ui.painter().rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, theme::border()), StrokeKind::Outside);
    let norm = |p: Pos2| Pos2::new(((p.x - r.left()) / r.width()).clamp(0.0, 1.0), ((p.y - r.top()) / r.height()).clamp(0.0, 1.0));
    if resp.drag_started() {
        *drag_from = resp.interact_pointer_pos().map(norm);
    }
    if let (Some(a), Some(p)) = (*drag_from, resp.interact_pointer_pos()) {
        if resp.dragged() || resp.drag_stopped() {
            let b = norm(p);
            *sel = Some([a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]);
        }
    }
    if resp.drag_stopped() {
        *drag_from = None;
    }
    if let Some(s) = *sel {
        let sr = Rect::from_min_max(Pos2::new(r.left() + s[0] * r.width(), r.top() + s[1] * r.height()), Pos2::new(r.left() + s[2] * r.width(), r.top() + s[3] * r.height()));
        let dim = Color32::from_black_alpha(90);
        let p = ui.painter();
        p.rect_filled(Rect::from_min_max(r.min, Pos2::new(r.right(), sr.top())), CornerRadius::ZERO, dim);
        p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), sr.bottom()), r.max), CornerRadius::ZERO, dim);
        p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), sr.top()), Pos2::new(sr.left(), sr.bottom())), CornerRadius::ZERO, dim);
        p.rect_filled(Rect::from_min_max(Pos2::new(sr.right(), sr.top()), Pos2::new(r.right(), sr.bottom())), CornerRadius::ZERO, dim);
        p.rect_stroke(sr, CornerRadius::ZERO, Stroke::new(1.5, theme::accent_col()), StrokeKind::Outside);
    }
}

impl crate::App {
    /// Draws the Resolve Images dialog when it is open (call every frame on the bank screen).
    pub fn draw_resolve_images(&mut self, ctx: &egui::Context, on_screen: &Id) {
        // QA hooks: VERSHION_RESOLVE=1 opens it for the bank on screen; VERSHION_RESOLVE_FILES=<paths>
        // adds originals; VERSHION_RESOLVE_CROP=<slot n> opens slot n's crop panel with a selection.
        if crate::env_var("VERSHION_RESOLVE").is_ok() {
            let hid = egui::Id::new("qs_resolve_hook");
            if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
                ctx.data_mut(|d| d.insert_temp(hid, true));
                {
                    open_resolve_images(self, on_screen);
                    if let Ok(v) = crate::env_var("VERSHION_RESOLVE_FILES") {
                        let mut rs = RESOLVE.with(|r| r.borrow_mut().take()).unwrap();
                        for p in std::env::split_paths(&v) {
                            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            if let Ok(b) = std::fs::read(&p) {
                                resolve_add_file(self, &mut rs, &name, b);
                            }
                        }
                        if let Ok(n) = crate::env_var("VERSHION_RESOLVE_CROP").map(|v| v.trim().parse::<usize>().unwrap_or(1)) {
                            rs.panel = Some(n.saturating_sub(1));
                            rs.sel = Some([0.1, 0.2, 0.6, 0.45]);
                        }
                        RESOLVE.with(|r| *r.borrow_mut() = Some(rs));
                    }
                }
            }
        }
        let Some(mut rs) = RESOLVE.with(|r| r.borrow_mut().take()) else { return };
        let Some(bank) = self.store.bank(&rs.bank).cloned() else { return };
        // Originals dropped onto the window while the dialog is open.
        #[cfg(target_arch = "wasm32")]
        for item in RESOLVE_BYTES.with(|q| std::mem::take(&mut *q.borrow_mut())) {
            match item {
                Ok((name, b)) => resolve_add_file(self, &mut rs, &name, b),
                Err(e) => self.toast_err(e),
            }
        }
        for f in ctx.input(|i| i.raw.dropped_files.clone()) {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let name = f.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped file".into());
                match f.bytes() {
                    Ok(b) => resolve_add_file(self, &mut rs, &name, b),
                    Err(e) => self.toast_err(format!("Couldn't read {name}: {e}")),
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                let name = f.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped file".into());
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let item = match f.bytes_async().await {
                        Ok(b) => Ok((name.clone(), b)),
                        Err(e) => Err(format!("Couldn't read {name}: {e}")),
                    };
                    RESOLVE_BYTES.with(|q| q.borrow_mut().push(item));
                    ctx.request_repaint();
                });
            }
        }
        let total = rs.slots.len();
        let ready = rs.staged.len() + rs.described.iter().filter(|k| !rs.staged.contains_key(k)).count();
        let screen = crate::theme::area_or(ctx.content_rect());
        let w = 620.0_f32.min(screen.width() - 40.0);
        let mut close = false;
        let mut apply = false;
        let mut open_q: Option<usize> = None;
        let modal = egui::Modal::new(egui::Id::new("qs_resolve_modal")).show(ctx, |ui| {
            ui.set_width(w);
            ui.label(theme::serif("Resolve Images").size(22.0).color(theme::ink()));
            ui.label(theme::muted(resolve_subtitle(ready, total)).size(12.5));
            ui.add_space(8.0);
            // Drop zone for the original document.
            let dz = egui::Frame::new()
                .stroke(Stroke::new(1.0, theme::red().gamma_multiply(0.7)))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                        paint_icon(ui.painter(), Ic::Upload, r.center(), theme::red(), 0.75);
                        let have = rs.pages.len() + rs.doc_images.len();
                        let msg = if have == 0 {
                            "Drop your original PDF or Word document here to take pictures from it, or upload each picture below.".to_string()
                        } else {
                            format!("{} from your original{} to take pictures from. Drop more here.", plural(rs.pages.len(), "page"), if rs.doc_images.is_empty() { String::new() } else { format!(" and {}", plural(rs.doc_images.len(), "picture")) })
                        };
                        ui.add(egui::Label::new(theme::text(msg).size(12.5)).wrap());
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        if kit::ghost_button(ui, "Choose original…").clicked() {
                            if let Some(paths) = crate::platform::pick_files("PDF, Word or photo", &["pdf", "docx", "png", "jpg", "jpeg", "gif", "webp"]) {
                                for p in paths {
                                    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                    match std::fs::read(&p) {
                                        Ok(b) => resolve_add_file(self, &mut rs, &name, b),
                                        Err(e) => self.toast_err(format!("Couldn't read {name}: {e}")),
                                    }
                                }
                            }
                        }
                    });
                });
            if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
                ui.painter().rect_stroke(dz.response.rect, CornerRadius::same(8), Stroke::new(2.0, theme::accent_col()), StrokeKind::Inside);
            }
            ui.add_space(8.0);
            let list_h = (screen.height() - 300.0).max(240.0);
            egui::ScrollArea::vertical().id_salt("resolve_list").max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
                if rs.slots.is_empty() {
                    ui.label(theme::muted("Every picture is in place.").size(13.0));
                }
                for k in 0..rs.slots.len() {
                    let slot = rs.slots[k].clone();
                    let n = slot.qi + 1;
                    kit::card().inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_top(|ui| {
                            let thumb = Vec2::new(170.0, 74.0);
                            if let Some(bytes) = rs.staged.get(&k).cloned() {
                                match tex_of(ctx, &mut rs.tex, &format!("staged_{k}_{}", bytes.len()), &bytes) {
                                    Some(t) => {
                                        let sz = t.size_vec2();
                                        let s = (thumb.x / sz.x).min(thumb.y * 1.4 / sz.y).min(1.0);
                                        ui.allocate_ui(Vec2::new(thumb.x, thumb.y), |ui| {
                                            ui.add(egui::Image::new(&t).fit_to_exact_size(sz * s));
                                        });
                                    }
                                    None => needed_box(ui, thumb, Some("unreadable picture")),
                                }
                            } else if rs.described.contains(&k) {
                                let (r, _) = ui.allocate_exact_size(thumb, Sense::hover());
                                ui.painter().rect_filled(r, CornerRadius::same(4), theme::field());
                                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "Described in words", FontId::new(12.0, theme::bold_family()), theme::text_col());
                            } else {
                                needed_box(ui, thumb, None);
                            }
                            ui.add_space(8.0);
                            ui.vertical(|ui| {
                                ui.label(theme::strong(slot.label(n)).size(13.5));
                                if let Some(a) = &slot.alt {
                                    ui.add(egui::Label::new(theme::text(a).size(12.0)).wrap());
                                }
                                if !slot.attachable {
                                    ui.add(egui::Label::new(theme::muted("In an answer key or part: these have no picture of their own. Describe it in words, or edit it in the question.").size(11.0)).wrap());
                                }
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    if slot.attachable {
                                        let up = if rs.staged.contains_key(&k) { "Change file" } else { "Upload a file" };
                                        if kit::ghost_button(ui, up).clicked() {
                                            if let Some(path) = crate::platform::pick_file("Pictures", &["png", "jpg", "jpeg", "gif", "webp"]) {
                                                match crate::platform::read(&path) {
                                                    Ok(b) if image::guess_format(&b).is_ok() => {
                                                        rs.staged.insert(k, b);
                                                        rs.described.remove(&k);
                                                    }
                                                    Ok(_) => self.toast_err("That file is not a picture."),
                                                    Err(e) => self.toast_err(format!("Couldn't read that file: {e}")),
                                                }
                                            }
                                        }
                                        if !rs.pages.is_empty() && kit::ghost_button(ui, "Crop from original").clicked() {
                                            rs.panel = if rs.panel == Some(k) { None } else { Some(k) };
                                            rs.sel = None;
                                        }
                                        if !rs.doc_images.is_empty() && rs.pages.is_empty() && kit::ghost_button(ui, "Pick from document").clicked() {
                                            rs.panel = if rs.panel == Some(k) { None } else { Some(k) };
                                        }
                                    }
                                    let described = rs.described.contains(&k);
                                    let dl = if described { "Undo description" } else { "Describe in words" };
                                    if !rs.staged.contains_key(&k) && ui.add(egui::Button::new(theme::accent(dl).size(12.0)).frame(false)).on_hover_text("Replace the placeholder with its description, e.g. “(Picture: Lewis structure of NH3)”").clicked() {
                                        if described {
                                            rs.described.remove(&k);
                                        } else {
                                            rs.described.insert(k);
                                        }
                                    }
                                    if rs.staged.contains_key(&k) && ui.add(egui::Button::new(theme::accent("Remove").size(12.0)).frame(false)).clicked() {
                                        rs.staged.remove(&k);
                                    }
                                    if ui.add(egui::Button::new(theme::accent("Open question").size(12.0)).frame(false)).clicked() {
                                        open_q = Some(slot.qi);
                                    }
                                });
                            });
                        });
                        // Crop / Word-picture panel for this placeholder.
                        if rs.panel == Some(k) && slot.attachable {
                            ui.add_space(8.0);
                            if !rs.doc_images.is_empty() {
                                ui.label(theme::strong("Pictures in your document").size(12.5));
                                ui.horizontal_wrapped(|ui| {
                                    for (i, (name, bytes)) in rs.doc_images.clone().iter().enumerate() {
                                        if let Some(t) = tex_of(ctx, &mut rs.tex, &format!("doc_{i}"), bytes) {
                                            let sz = t.size_vec2();
                                            let s = (110.0 / sz.x).min(80.0 / sz.y).min(1.0);
                                            let resp = ui.add(egui::Button::image(egui::Image::new(&t).fit_to_exact_size(sz * s))).on_hover_text(name);
                                            if resp.clicked() {
                                                rs.staged.insert(k, bytes.clone());
                                                rs.described.remove(&k);
                                                rs.panel = None;
                                            }
                                        }
                                    }
                                });
                            }
                            if !rs.pages.is_empty() {
                                rs.page = rs.page.min(rs.pages.len() - 1);
                                ui.horizontal(|ui| {
                                    ui.label(theme::strong("Drag a box around the picture").size(12.5));
                                    if rs.pages.len() > 1 {
                                        let mut pg = rs.page;
                                        egui::ComboBox::from_id_salt(("resolve_page", k)).selected_text(truncate_chars(&rs.pages[pg].0, 30)).show_ui(ui, |ui| {
                                            for (i, (name, _)) in rs.pages.iter().enumerate() {
                                                ui.selectable_value(&mut pg, i, name.clone());
                                            }
                                        });
                                        if pg != rs.page {
                                            rs.page = pg;
                                            rs.sel = None;
                                        }
                                    }
                                });
                                let (name, bytes) = rs.pages[rs.page].clone();
                                if let Some(t) = tex_of(ctx, &mut rs.tex, &format!("page_{}_{name}", rs.page), &bytes) {
                                    let (mut sel, mut from) = (rs.sel, rs.drag_from);
                                    crop_canvas(ui, &t, 460.0, &mut sel, &mut from);
                                    rs.sel = sel;
                                    rs.drag_from = from;
                                }
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    if ui.add_enabled_ui(rs.sel.is_some(), |ui| kit::primary_button(ui, "Use selected area")).inner.clicked() {
                                        if let Some(sel) = rs.sel {
                                            match crop_png(&bytes, sel) {
                                                Ok(b) => {
                                                    rs.staged.insert(k, b);
                                                    rs.described.remove(&k);
                                                    rs.panel = None;
                                                    rs.sel = None;
                                                }
                                                Err(e) => self.toast_err(format!("Couldn't crop: {e}")),
                                            }
                                        }
                                    }
                                    if kit::ghost_button(ui, "Close").clicked() {
                                        rs.panel = None;
                                    }
                                });
                            }
                        }
                    });
                    ui.add_space(6.0);
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add_enabled_ui(ready > 0, |ui| kit::primary_button(ui, "Use these pictures")).inner.clicked() {
                        apply = true;
                    }
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            });
        });
        if apply {
            let fresh = crate::bankops::picture_slots(&bank.questions);
            if fresh != rs.slots {
                self.toast_err("The bank changed while this was open. Open Resolve Images again.");
                return;
            }
            let mut pics = std::collections::HashMap::new();
            for (k, b) in &rs.staged {
                match crate::store::put_image(b) {
                    Ok(img) => {
                        pics.insert(*k, img);
                    }
                    Err(e) => self.toast_err(format!("Couldn't use a picture: {e}")),
                }
            }
            let t = now_secs();
            let mut n = 0;
            if let Some(b) = self.store.bank_mut(&rs.bank) {
                n = apply_resolution(&mut b.questions, &rs.slots, &pics, &rs.described, t);
                if n > 0 {
                    b.modified = t;
                }
            }
            if n > 0 {
                self.store.save_banks();
                self.toast(format!("Resolved {}", plural(n, "picture")));
            }
            return;
        }
        if let Some(qi) = open_q {
            if let Some(q) = bank.questions.get(qi) {
                self.qedit = Some(crate::qedit::Draft::from_question(rs.bank.clone(), q));
            }
            return;
        }
        if close || modal.should_close() {
            return;
        }
        RESOLVE.with(|r| *r.borrow_mut() = Some(rs));
    }
}

// ----- import history (secondary) -----

fn history(app: &mut crate::App, ui: &mut egui::Ui) {
    ui.label(theme::serif("Import history").size(22.0).color(theme::ink()));
    ui.label(theme::muted("Everything verSHion has read: pasted, converted by the AI, or read from a file.").size(12.5));
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
    fn paste_reads_format_and_counts_live() {
        let text = "term1\tdef1\nterm2\tdef2\nterm3\tdef3\n";
        let r = read_paste(text, &HashSet::new(), true).expect("term list reads");
        assert!(r.format.contains("Flashcard"), "{}", r.format);
        let n = crate::formats::detect_and_parse(text.as_bytes(), "Pasted text").unwrap().questions.len();
        assert_eq!((r.found, r.coming), (n, n));
        // Duplicates of the destination bank, and repeats inside the paste, stay out.
        let one = "MC\nWhich planet is closest to the Sun?\nVenus\n*Mercury\n\nTF\nThe Sun is a star.\nT\n";
        let r = read_paste(one, &HashSet::new(), true).unwrap();
        assert_eq!((r.found, r.coming), (2, 2));
        assert_eq!(paste_counts_line(&r), ("2 questions found · ".to_string(), "2 coming in".to_string()));
        let doc = crate::formats::detect_and_parse(one.as_bytes(), "Pasted text").unwrap();
        let existing: HashSet<String> = std::iter::once(dup_key(&doc.questions[0])).collect();
        let twice = format!("{one}\nTF\nThe Sun is a star.\nT\n");
        let r = read_paste(&twice, &existing, true).unwrap();
        assert_eq!((r.found, r.coming), (3, 1));
        let r = read_paste(&twice, &existing, false).unwrap();
        assert_eq!(r.coming, 3, "with Skip duplicates off everything comes in");
        assert!(read_paste("   \n", &HashSet::new(), true).is_err() || read_paste("   \n", &HashSet::new(), true).unwrap().found == 0);
    }

    #[test]
    fn picture_banner_wording() {
        assert_eq!(picture_banner(1, 1), "1 picture needed. Click it to add it, or add it after importing.");
        assert_eq!(picture_banner(3, 3), "3 pictures needed. Click one to add it, or add them after importing.");
        assert!(picture_banner(5, 1).contains("the 4 in answer keys"), "{}", picture_banner(5, 1));
        assert!(picture_banner(2, 0).contains("answer keys"));
    }

    #[test]
    fn crop_and_attach_fill_the_stem_placeholder() {
        let img = image::RgbaImage::from_fn(100, 50, |x, _| if x < 50 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) });
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let png = png.into_inner();
        let out = crop_png(&png, [0.5, 0.0, 1.0, 1.0]).expect("crop");
        let c = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!((c.width(), c.height()), (50, 50));
        assert_eq!(c.get_pixel(10, 10).0, [0, 0, 255, 255], "right half kept");
        // Reversed drag corners work; a sliver is refused.
        assert!(crop_png(&png, [1.0, 1.0, 0.5, 0.0]).is_ok());
        assert!(crop_png(&png, [0.2, 0.2, 0.2, 0.9]).is_err());

        let mut q = crate::formats::detect_and_parse(b"term\tdef\n", "p").unwrap().questions.remove(0);
        q.stem = "Look:\n[picture: Lewis structure]\nBONDING: ___".into();
        assert!(stem_picture_open(&q));
        assert_eq!(crate::bankops::pictures_needed(&q), 1);
        attach_picture(&mut q, model::ImageRef { file: "media/x.png".into(), width: 50, height: 50 });
        assert!(!stem_picture_open(&q));
        assert_eq!(crate::bankops::pictures_needed(&q), 0);
    }

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

    #[test]
    fn source_versions_read_the_bank_record_and_each_test() {
        let pkg = br#"{"format":"test-parrot/package","formatVersion":"0.1.0","questionBanks":[{"id":"bank","record":{"format":"test-parrot/question-bank","formatVersion":"0.10.0","bank":{"name":"B","questions":[]}}}],
            "exams":[{"format":"test-parrot/exam","formatVersion":"0.5.0","name":"A"},{"name":"B"}]}"#;
        let v = source_versions(pkg);
        assert_eq!(v.bank.as_deref(), Some("0.10.0"));
        assert_eq!(v.exams, vec![Some("0.5.0".to_string()), None]);
        let rec = br#"{"format":"test-parrot/question-bank","formatVersion":"0.10.0","bank":{"name":"B","questions":[]}}"#;
        assert_eq!(source_versions(rec), SourceVersions { bank: Some("0.10.0".into()), exams: vec![] });
        assert_eq!(source_versions(b"MC\nWhat?\n*a\nb\n"), SourceVersions::default());
        assert_eq!(source_versions(b"<questestinterop><item/></questestinterop>").bank.as_deref(), Some("QTI 1.2"));
    }

    #[test]
    fn source_title_prefers_the_stated_bank_name() {
        let mut st = State::default();
        let doc = ImportDoc { name: "Chemistry Chapter 4 Quiz (5A and 5B)".into(), questions: Vec::new(), exams: Vec::new() };
        st.push_doc("ch4-quiz-5a-5b.parrot.json", "Test Parrot file", "file", Ic::File, doc, SourceVersions::default());
        assert_eq!(source_title(&st.sources[0]), "Chemistry Chapter 4 Quiz (5A and 5B)");
        st.sources[0].doc.as_mut().unwrap().name = "  ".into();
        assert_eq!(source_title(&st.sources[0]), "ch4-quiz-5a-5b.parrot.json");
    }

    #[test]
    fn import_footer_mentions_tests_like_the_reference() {
        assert_eq!(import_footer(9, 0, None, Some("Ch4"), "x"), "Will add 9 questions to “Ch4”.");
        assert_eq!(
            import_footer(9, 2, Some("Chemistry Chapter 4 Quiz (5A and 5B)"), Some("Untitled Question Bank"), "x"),
            "Will create 2 tests and merge “Chemistry Chapter 4 Quiz (5A and 5B)” (9 questions) into “Untitled Question Bank”."
        );
        assert_eq!(import_footer(1, 1, Some("B"), Some("B"), "x"), "Will create 1 test and add 1 question to “B”.");
        assert_eq!(import_footer(9, 2, None, None, "Chem"), "Will create the new bank “Chem” (9 questions) and 2 tests.");
        assert_eq!(import_footer(3, 0, None, None, "Chem"), "Will create the new bank “Chem” with 3 questions.");
    }

    #[test]
    fn resolution_attaches_and_describes_without_shifting_markers() {
        use crate::bankops::{picture_slots, pictures_needed};
        use crate::formats::make_question;
        use crate::model::{Kind, Part, PartKind};
        let mut qs = vec![
            make_question("Bonds: [picture: acetyl chloride] ___".into(), Kind::Blank { accept: vec![vec!["14".into()]] }),
            make_question(
                "Do all: [picture: a] and [picture: b]".into(),
                Kind::Multipart { parts: vec![Part { stem: "a. NH3".into(), kind: PartKind::Short { sample: "[picture: key]".into() } }] },
            ),
        ];
        let slots = picture_slots(&qs);
        assert_eq!(slots.len(), 4);
        let img = model::ImageRef { file: "media/x.png".into(), width: 2, height: 2 };
        let pics: std::collections::HashMap<usize, model::ImageRef> = [(0, img.clone())].into_iter().collect();
        // describe both stem markers of question 2 and the part's key; slot 0 gets a picture
        let described: HashSet<usize> = [0, 1, 2, 3].into_iter().collect();
        let n = apply_resolution(&mut qs, &slots, &pics, &described, 7);
        assert_eq!(n, 4);
        assert_eq!(qs[0].image.as_ref().map(|i| i.file.as_str()), Some("media/x.png"));
        assert_eq!(qs[0].stem, "Bonds: [picture: acetyl chloride] ___", "the picture fills the marker; the text stays");
        assert_eq!(qs[1].stem, "Do all: (Picture: a) and (Picture: b)");
        assert!(matches!(&qs[1].kind, Kind::Multipart { parts } if matches!(&parts[0].kind, PartKind::Short { sample } if sample == "(Picture: key)")));
        assert_eq!(qs.iter().map(pictures_needed).sum::<usize>(), 0);
        assert_eq!(qs[1].modified, 7);
        assert_eq!(resolve_subtitle(0, 5), "0 of 5 pictures ready. Any left unresolved stay as “picture needed” and can be added later.");
    }

    #[test]
    fn docx_pictures_are_found_in_word_media() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("word/document.xml", o).unwrap();
            z.write_all(b"<w:document/>").unwrap();
            z.start_file("word/media/image1.png", o).unwrap();
            z.write_all(b"\x89PNG").unwrap();
            z.start_file("word/media/notes.txt", o).unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        let imgs = docx_images(&buf);
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].0, "image1.png");
        assert!(docx_images(b"not a zip").is_empty());
    }
}
