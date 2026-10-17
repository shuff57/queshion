//! Test Parrot clone — app entry and shared shell state. Screen bodies live in the screen
//! modules (`screens`, `bankui`/`qedit`, `examui`). Persistence is `store`; AI is `ai`.
//!
//! Shell-owned extra fields (added by the shell worker, per NOTES contract extension —
//! not serialized anywhere, so no `#[serde]` applies to `App`):
//! `toast_times` (toasts vec parallel timestamps), `help_open`, `confirm_restore`,
//! `ai_busy`, `import_file`, `ai_hint`.

use eframe::egui;

pub mod ai;
pub mod bankops;
pub mod bankui;
pub mod ditto;
pub mod figfit;
pub mod examui;
pub mod equation;
pub mod export;
pub mod figures;
pub mod figuresui;
pub mod kit;
pub mod formats;
pub mod importui;
pub mod layout;
pub mod model;
pub mod qedit;
pub mod richtext;
pub mod screens;
pub mod store;
pub mod theme;

use model::{ExamItem, Id};
use store::Store;

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub ai_provider: String, // "ollama" | "openrouter" | "custom"
    #[serde(default)]
    pub ai_model: String,
    #[serde(default)]
    pub ai_base: String, // custom OpenAI-compatible base URL
    #[serde(default)]
    pub dark: bool, // dark palette; applied via theme::set_dark
    /// Home screen view modes (false = Cards, true = List).
    #[serde(default)]
    pub home_exams_list: bool,
    #[serde(default)]
    pub home_banks_list: bool,
    /// Banks list / Bank editor view modes (false = Cards, true = List), bank worker.
    #[serde(default)]
    pub banks_list_view: bool,
    #[serde(default)]
    pub bank_edit_list_view: bool,
}

#[derive(Clone)]
pub enum Screen {
    Welcome,
    Home,
    Banks,
    Exams,
    Imports,
    Figures,
    Settings,
    BankEdit(Id),
    ExamEdit(Id),
}

pub struct App {
    pub store: Store,
    pub screen: Screen,
    pub bank: Option<Id>,                // open bank editor
    pub exam: Option<Id>,                // open exam editor
    pub qedit: Option<qedit::Draft>,     // open question editor modal
    pub paste: String,
    pub ai: ai::Ai,
    pub storage_info: bool, // topbar ⓘ: "where your work is stored" popup
    pub panel_bank: Option<Id>,          // bank tab open in the exam editor side panel
    pub export_open: bool,
    pub history_open: bool,
    pub toasts: Vec<(String, bool)>,     // (text, is_error)
    pub settings: Settings,
    pub textures: std::collections::HashMap<String, egui::TextureHandle>, // media cache, key = model::ImageRef.file
    pub undo: Vec<Vec<ExamItem>>,        // exam editor snapshots (push before each change)
    pub redo: Vec<Vec<ExamItem>>,
    // Shell-owned extensions:
    /// Per-toast wall-clock timestamps (ms), parallel to `toasts` (5s expiry).
    pub toast_times: Vec<i64>,
    /// "How it works" overlay open.
    pub help_open: bool,
    /// Backup path awaiting the replace-everything confirmation modal.
    pub confirm_restore: Option<std::path::PathBuf>,
    /// An AI conversion job is running (import dialog spinner).
    pub ai_busy: bool,
    /// Staged import file chosen in the dialog (name, mime, bytes).
    pub import_file: Option<screens::ImportFile>,
    /// Optional hint sent with the AI conversion.
    pub ai_hint: String,
    /// Name of the file currently staged for AI conversion (shown while busy).
    pub import_name: String,
    /// Figures screen + insert-figure modal state.
    pub figs: figuresui::FigUi,
    /// QA hook: scripted input (QUESHION_SCRIPT), see `raw_input_hook`.
    pub qa_steps: Vec<String>,
    /// QA hook: QUESHION_SHOT=<file.png> saves a screenshot of this window's own contents and exits.
    pub qa_shot: Option<(String, u32)>,
    pub qa_frame: u64,
}

/// Shared constructor so the window path and the headless selftest path build the
/// same App. `ctx` is the egui context (theme/fonts setup happens here too).
impl App {
    /// Flip the palette and remember it (the sidebar switch calls this).
    pub fn set_dark_mode(&mut self, dark: bool, ctx: &egui::Context) {
        self.settings.dark = dark;
        theme::set_dark(dark, Some(ctx));
        let _ = store::save_json("settings.json", &self.settings);
    }

    fn new_from(ctx: &egui::Context) -> Self {
        theme::setup(ctx);
        let mut settings: Settings = store::load_json("settings.json").unwrap_or_default();
        if settings.ai_provider.trim().is_empty() {
            settings.ai_provider = "cloud".to_string();
        }
        if settings.ai_model.trim().is_empty() {
            settings.ai_model = "gemma4:31b".to_string();
        }
        theme::set_dark(settings.dark, Some(ctx));
        let app = App {
            store: Store::load(),
            screen: Screen::Welcome,
            bank: None,
            exam: None,
            qedit: None,
            paste: String::new(),
            ai: ai::Ai::default(),
            panel_bank: None,
            export_open: false,
            history_open: false,
            toasts: Vec::new(),
            settings,
            textures: Default::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            toast_times: Vec::new(),
            help_open: false,
            confirm_restore: None,
            ai_busy: false,
            storage_info: false,
            import_file: None,
            ai_hint: String::new(),
            import_name: String::new(),
            figs: Default::default(),
            qa_shot: std::env::var("QUESHION_SHOT").ok().map(|p| (p, 0)),
            qa_steps: std::env::var("QUESHION_SCRIPT")
                .map(|v| v.split(';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
                .unwrap_or_default(),
            qa_frame: 0,
        };
        let mut app = app; // non-mut unless the capture hook below needs it
        // Screen-capture/test hook: QUESHION_SCREEN opens a given screen on startup.
        if let Ok(s) = std::env::var("QUESHION_SCREEN") {
            app.screen = match s.trim() {
                "home" => Screen::Home,
                "banks" => Screen::Banks,
                "exams" => Screen::Exams,
                "imports" => Screen::Imports,
                "figures" => Screen::Figures,
                "settings" => Screen::Settings,
                "bank" => app
                    .store
                    .banks
                    .first()
                    .map(|b| Screen::BankEdit(b.id.clone()))
                    .unwrap_or(app.screen),
                "exam" => app
                    .store
                    .exams
                    .first()
                    .map(|e| Screen::ExamEdit(e.id.clone()))
                    .unwrap_or(app.screen),
                // "exam:N" opens the exam at index N of the store.
                s if s.starts_with("exam:") => s[5..]
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| app.store.exams.get(i))
                    .map(|e| Screen::ExamEdit(e.id.clone()))
                    .unwrap_or(app.screen),
                _ => app.screen,
            };
        }
        app
    }

    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        App::new_from(&cc.egui_ctx)
    }
}

impl eframe::App for App {
    /// Test hook: QUESHION_SCRIPT="click:x,y;text:abc;key:Enter;wait" injects one input step
    /// every 10 frames after a 60 frame warm-up. Coordinates are egui points.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some((path, _)) = &self.qa_shot {
            for ev in &raw.events {
                if let egui::Event::Screenshot { image, .. } = ev {
                    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    if let Some(img) = image::RgbaImage::from_raw(image.width() as u32, image.height() as u32, bytes) {
                        let _ = img.save(path);
                    }
                    std::process::exit(0);
                }
            }
        }
        if self.qa_steps.is_empty() {
            return;
        }
        ctx.request_repaint();
        self.qa_frame += 1;
        if self.qa_frame < 60 || self.qa_frame % 10 != 0 {
            return;
        }
        let step = self.qa_steps.remove(0);
        eprintln!("qa step {} at frame {}", step, self.qa_frame);
        if step == "dump" {
            eprintln!("qa draft stem = {:?}", self.qedit.as_ref().map(|d| d.stem.clone()));
        }
        let (cmd, arg) = step.split_once(':').unwrap_or((step.as_str(), ""));
        let mods = egui::Modifiers::NONE;
        let pt = |a: &str| {
            let (x, y) = a.split_once(',').unwrap_or(("0", "0"));
            egui::pos2(x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0))
        };
        match cmd {
            "click" => {
                let p = pt(arg);
                raw.events.push(egui::Event::PointerMoved(p));
                raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
                raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: mods });
            }
            "move" => raw.events.push(egui::Event::PointerMoved(pt(arg))),
            // Drag support: down:x,y then move:x,y ... then up:x,y (one step per 10 frames).
            "down" | "up" => {
                let p = pt(arg);
                raw.events.push(egui::Event::PointerMoved(p));
                raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: cmd == "down", modifiers: mods });
            }
            "dclick" => {
                let p = pt(arg);
                raw.events.push(egui::Event::PointerMoved(p));
                for _ in 0..2 {
                    raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
                    raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: mods });
                }
            }
            "text" => raw.events.push(egui::Event::Text(arg.to_string())),
            "key" => {
                let key = match arg {
                    "Enter" => Some(egui::Key::Enter),
                    "Escape" => Some(egui::Key::Escape),
                    "End" => Some(egui::Key::End),
                    "Home" => Some(egui::Key::Home),
                    "Tab" => Some(egui::Key::Tab),
                    "ArrowRight" => Some(egui::Key::ArrowRight),
                    "ArrowLeft" => Some(egui::Key::ArrowLeft),
                    "Backspace" => Some(egui::Key::Backspace),
                    _ => None,
                };
                if let Some(key) = key {
                    for pressed in [true, false] {
                        raw.events.push(egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: mods });
                    }
                }
            }
            _ => {}
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.qa_steps.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16)); // keep the QUESHION_SCRIPT clock advancing
        }
        if let Some((_, frames)) = &mut self.qa_shot {
            *frames += 1;
            // QUESHION_SHOT_FRAME=<n> delays the capture (lets a QUESHION_SCRIPT finish first).
            let at = std::env::var("QUESHION_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(60u32);
            if *frames == at {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        // Sidebar + active screen + shell overlays (import dialog, help, toasts).
        self.draw_screen(&ctx);
    }
}

/// Headless pre-window handling: QUESHION_TESTING=1 clears the store dir
/// (double-guarded), QUESHION_SELFTEST=1 / QUESHION_SEED=1 build the app,
/// seed optionally, and print the count line with no window. Returns true if
/// the process should exit after printing.
fn handle_headless() -> bool {
    let xdg_set = std::env::var_os("XDG_DATA_HOME")
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    if std::env::var("QUESHION_TESTING").as_deref() == Ok("1") {
        if !xdg_set {
            eprintln!("QUESHION_TESTING=1 refused: XDG_DATA_HOME must be set explicitly");
            std::process::exit(1);
        }
        let dir = store::store_dir();
        if dir.file_name().map(|n| n == "queshion").unwrap_or(false) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    let selftest =
        std::env::var("QUESHION_SELFTEST").as_deref() == Ok("1")
            || std::env::var("QUESHION_SEED").as_deref() == Ok("1");
    if !selftest {
        return false;
    }
    let ctx = egui::Context::default();
    let mut app = App::new_from(&ctx);
    if std::env::var("QUESHION_SEED").as_deref() == Ok("1") {
        app.seed_demo();
    }
    println!(
        "SELFTEST OK banks={} exams={} imports={} exports={}",
        app.store.banks.len(),
        app.store.exams.len(),
        app.store.imports.len(),
        app.store.exports.len()
    );
    true
}

fn main() -> Result<(), eframe::Error> {
    // Headless one-shot: QUESHION_RENDER_FIG=<figure id> [QUESHION_FIG_PARAMS='{"formula":"NH3"}']
    // QUESHION_OUT=<file.png> renders a Figures-tab figure to a PNG (strings/bools/numbers).
    if let Ok(id) = std::env::var("QUESHION_RENDER_FIG") {
        let def = figures::registry().into_iter().find(|d| d.id == id).unwrap_or_else(|| panic!("no figure {id}"));
        let over: serde_json::Value = std::env::var("QUESHION_FIG_PARAMS").ok().and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
        let p = figures::params_from_json(&def, &over);
        let png = figures::to_png(&(def.render)(&p), 1.5).expect("render");
        std::fs::write(std::env::var("QUESHION_OUT").expect("QUESHION_OUT"), png).expect("write");
        return Ok(());
    }
    // Headless one-shot: QUESHION_EXPORT=<exam index> QUESHION_OUT=<dir> writes exam.pdf, exam.docx
    // and key.pdf for that exam of the store (the stem-rendering checks read these).
    if let Ok(i) = std::env::var("QUESHION_EXPORT") {
        let dir = std::path::PathBuf::from(std::env::var("QUESHION_OUT").expect("QUESHION_OUT"));
        std::fs::create_dir_all(&dir).expect("out dir");
        match examui::headless_export(i.parse().unwrap_or(0), &dir) {
            Ok(()) => println!("exported to {}", dir.display()),
            Err(e) => {
                eprintln!("export failed: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // Headless one-shot: QUESHION_FIT=<crop.png> runs the figure-fitting loop on one picture.
    if let Ok(png) = std::env::var("QUESHION_FIT") {
        let bytes = std::fs::read(&png).unwrap_or_else(|e| panic!("cannot read {png}: {e}"));
        let settings: Settings = store::load_json("settings.json").unwrap_or_default();
        let cfg = ai::AiCfg::from(&settings);
        match figfit::fit(&bytes, "Give the number of bonding and nonbonding electrons for the structure below:", &cfg) {
            Ok(Some(f)) => println!("fit: {} {:?}", f.spec.id, f.spec.params.iter().filter(|(k, _)| *k == "formula" || *k == "preset").collect::<Vec<_>>()),
            Ok(None) => println!("no fit"),
            Err(e) => println!("error: {e}"),
        }
        return Ok(());
    }
    // Headless one-shot: QUESHION_IMPORT_PDF=<file.pdf> [QUESHION_BANK_NAME=..]
    // runs the Import screen's whole AI path (pages -> model -> Ditto parse -> figure crops)
    // and saves the result as a new bank in the store.
    if let Ok(pdf) = std::env::var("QUESHION_IMPORT_PDF") {
        import_pdf_headless(&pdf);
        return Ok(());
    }
    // Headless one-shot: QUESHION_PARSE=<reply text> [QUESHION_PAGE=<page.png> QUESHION_PAGE_NO=<n>]
    // runs the AI-import parser (figures cropped from the page image) and prints
    // {questions, warnings, figures} as JSON; the import test battery scores this.
    if let Ok(path) = std::env::var("QUESHION_PARSE") {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let text = String::from_utf8_lossy(&bytes).to_string();
        let page = std::env::var("QUESHION_PAGE").ok().and_then(|p| std::fs::read(p).ok());
        let page_no = std::env::var("QUESHION_PAGE_NO").ok().and_then(|n| n.parse().ok()).unwrap_or(1);
        let res = if ditto::is_ditto(&text) {
            ditto::import_text(&text, page.as_deref(), page_no, "converted")
        } else {
            formats::detect_and_parse(&bytes, &path).map(|doc| ditto::DittoImport { doc, warnings: Vec::new(), figures: 0 })
        };
        match res {
            Ok(r) => println!(
                "{}",
                serde_json::json!({"questions": r.doc.questions, "warnings": r.warnings, "figures": r.figures, "store": store::store_dir_string()})
            ),
            Err(e) => {
                eprintln!("parse failed: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // Headless one-shot: QUESHION_CONVERT=<file> [QUESHION_HINT="..."] transcribes a
    // photo/scan through the configured provider and prints the Blackboard import text.
    if let Ok(path) = std::env::var("QUESHION_CONVERT") {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let mime = match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "pdf" => "application/pdf",
            _ => "",
        };
        let settings: Settings = store::load_json("settings.json").unwrap_or_default();
        let cfg = ai::AiCfg::from(&settings);
        let hint = std::env::var("QUESHION_HINT").unwrap_or_default();
        let text = ai::transcribe_checked(&bytes, mime, &cfg, &ai::prompt_for(&hint))
            .map(|raw| ai::extract_import_text(&raw));
        match text {
            Ok(t) => println!("{t}"),
            Err(e) => {
                eprintln!("conversion failed: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // Headless modes...
    if handle_headless() {
        return Ok(());
    }
    // Test hook: QUESHION_WINSIZE=WxH forces the initial inner size (e.g. 860x700).
    let size = std::env::var("QUESHION_WINSIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.trim().split_once('x')?;
            Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?])
        })
        .unwrap_or([1180.0, 780.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("queShion")
            .with_decorations(true)
            .with_inner_size(size)
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("queShion", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
fn import_pdf_headless(pdf: &str) {
    let bytes = std::fs::read(pdf).unwrap_or_else(|e| panic!("cannot read {pdf}: {e}"));
    let pages = screens::pdf_pages_png(&bytes).unwrap_or_else(|e| panic!("{e}"));
    let settings: Settings = store::load_json("settings.json").unwrap_or_default();
    let cfg = ai::AiCfg::from(&settings);
    let converted: Vec<Result<ai::Converted, String>> = std::thread::scope(|sc| {
        let hs: Vec<_> = pages.iter().enumerate().map(|(i, png)| sc.spawn({ let cfg = &cfg; move || ai::convert_page(png, "image/png", "", cfg, i + 1) })).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("thread panicked".into()))).collect()
    });
    let mut questions = Vec::new();
    for (i, c) in converted.into_iter().enumerate() {
        let c = c.unwrap_or_else(|e| panic!("page {} failed: {e}", i + 1));
        eprintln!("page {}: {} questions, {} cropped, {} redrawn, notes {:?}", i + 1, c.doc.questions.len(), c.cropped, c.redrawn, c.warnings);
        questions.extend(c.doc.questions);
    }
    for q in &mut questions {
        q.points.get_or_insert(1.0);
    }
    let name = std::env::var("QUESHION_BANK_NAME").unwrap_or_else(|_| "Imported questions".into());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut st = store::Store::load();
    let n = questions.len();
    st.banks.push(model::QuestionBank { id: model::new_id(), name: name.clone(), questions, created: now, modified: now });
    st.save_banks();
    println!("saved bank \"{name}\" with {n} questions to {}", store::store_dir_string());
}
