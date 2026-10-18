//! Test Parrot clone — app entry and shared shell state. Screen bodies live in the screen
//! modules (`screens`, `bankui`/`qedit`, `examui`). Persistence is `store`; AI is `ai`.
//!
//! Shell-owned extra fields (added by the shell worker, per NOTES contract extension —
//! not serialized anywhere, so no `#[serde]` applies to `App`):
//! `toast_times` (toasts vec parallel timestamps), `help_open`, `confirm_restore`,
//! `ai_busy`, `import_file`, `ai_hint`.

use eframe::egui;

/// Reads `VERSHION_*`, falling back to the pre-rename `QUESHION_*` for one release so
/// existing QA scripts keep working.
pub(crate) fn env_var(name: &str) -> Result<String, std::env::VarError> {
    std::env::var(name)
        .or_else(|first| std::env::var(name.replacen("VERSHION_", "QUESHION_", 1)).map_err(|_| first))
}

pub(crate) fn env_var_os(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name)
        .or_else(|| std::env::var_os(name.replacen("VERSHION_", "QUESHION_", 1)))
}

pub mod ai;
pub mod bankexport;
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
pub mod fonts;
pub mod formats;
pub mod importui;
pub mod layout;
pub mod model;
pub mod platform;
pub mod qedit;
pub mod richtext;
pub mod screens;
pub mod store;
pub mod theme;
pub mod tidy;
pub mod wysiwyg;

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
    /// Whether secondary viewports (the bank pop-over) are allowed. False when embedded in a
    /// host that owns the window.
    pub allow_viewports: bool,
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
    /// QA hook: scripted input (VERSHION_SCRIPT), see `raw_input_hook`.
    pub qa_steps: Vec<String>,
    /// QA hook: VERSHION_SHOT=<file.png> saves a screenshot of this window's own contents and exits.
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
            allow_viewports: true,
            import_file: None,
            ai_hint: String::new(),
            import_name: String::new(),
            figs: Default::default(),
            qa_shot: crate::env_var("VERSHION_SHOT").ok().map(|p| (p, 0)),
            qa_steps: crate::env_var("VERSHION_SCRIPT")
                .map(|v| v.split(';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
                .unwrap_or_default(),
            qa_frame: 0,
        };
        let mut app = app; // non-mut unless the capture hook below needs it
        // Screen-capture/test hook: VERSHION_SCREEN opens a given screen on startup.
        if let Ok(s) = crate::env_var("VERSHION_SCREEN") {
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
    /// Test hook: VERSHION_SCRIPT="click:x,y;text:abc;key:Enter;wait" injects one input step
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
            if let Some(model::Kind::Blank { accept }) = self.qedit.as_ref().map(|d| &d.kind) {
                eprintln!("qa draft blanks = {accept:?}");
            }
            if let Some(model::Kind::Mc { answers }) = self.qedit.as_ref().map(|d| &d.kind) {
                eprintln!("qa draft answers = {:?}", answers.iter().map(|a| a.text.clone()).collect::<Vec<_>>());
            }
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
            // A real pointer click: hover first, press 10 frames later, release 10 after that
            // (a one-frame "click" teleports the pointer, which no mouse does).
            "tap" => {
                raw.events.push(egui::Event::PointerMoved(pt(arg)));
                self.qa_steps.insert(0, format!("down:{arg}"));
                self.qa_steps.insert(1, format!("up:{arg}"));
            }
            // drag:x1,y1,x2,y2 = hover, press, move in 4 steps, release (one step per 10 frames).
            "drag" => {
                let v: Vec<f32> = arg.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                if let [x1, y1, x2, y2] = v[..] {
                    raw.events.push(egui::Event::PointerMoved(egui::pos2(x1, y1)));
                    let mut steps = vec![format!("down:{x1},{y1}")];
                    steps.extend((1..=4).map(|k| {
                        let t = k as f32 / 4.0;
                        format!("move:{},{}", x1 + (x2 - x1) * t, y1 + (y2 - y1) * t)
                    }));
                    steps.push(format!("up:{x2},{y2}"));
                    for (i, s) in steps.into_iter().enumerate() {
                        self.qa_steps.insert(i, s);
                    }
                }
            }
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
            // key:Enter, key:Ctrl+B, key:Shift+ArrowLeft, key:Ctrl+Shift+Num8 ...
            "key" => {
                let mut kmods = egui::Modifiers::NONE;
                let mut name = arg;
                loop {
                    if let Some(r) = name.strip_prefix("Ctrl+") {
                        kmods.ctrl = true;
                        kmods.command = true;
                        name = r;
                    } else if let Some(r) = name.strip_prefix("Shift+") {
                        kmods.shift = true;
                        name = r;
                    } else if let Some(r) = name.strip_prefix("Alt+") {
                        kmods.alt = true;
                        name = r;
                    } else {
                        break;
                    }
                }
                let key = match name {
                    "Num7" => Some(egui::Key::Num7),
                    "Num8" => Some(egui::Key::Num8),
                    "Num9" => Some(egui::Key::Num9),
                    "Comma" => Some(egui::Key::Comma),
                    "Period" => Some(egui::Key::Period),
                    n => egui::Key::from_name(n),
                };
                if let Some(key) = key {
                    for pressed in [true, false] {
                        raw.events.push(egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: kmods });
                    }
                }
            }
            "paste" => raw.events.push(egui::Event::Paste(arg.replace("\\n", "\n"))),
            _ => {}
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::begin_frame(self.settings.dark);
        theme::set_area(ui.max_rect());
        let ctx = ui.ctx().clone();
        if !self.qa_steps.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16)); // keep the VERSHION_SCRIPT clock advancing
        }
        if let Some((_, frames)) = &mut self.qa_shot {
            *frames += 1;
            // VERSHION_SHOT_FRAME=<n> delays the capture (lets a VERSHION_SCRIPT finish first).
            let at = crate::env_var("VERSHION_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(60u32);
            if *frames == at {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        // Sidebar + active screen + shell overlays (import dialog, help, toasts).
        self.draw_screen(&ctx);
    }
}

/// Headless pre-window handling: VERSHION_TESTING=1 clears the store dir
/// (double-guarded), VERSHION_SELFTEST=1 / VERSHION_SEED=1 build the app,
/// seed optionally, and print the count line with no window. Returns true if
/// the process should exit after printing.
fn handle_headless() -> bool {
    let xdg_set = std::env::var_os("XDG_DATA_HOME")
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    if crate::env_var("VERSHION_TESTING").as_deref() == Ok("1") {
        if !xdg_set {
            eprintln!("VERSHION_TESTING=1 refused: XDG_DATA_HOME must be set explicitly");
            std::process::exit(1);
        }
        let dir = store::store_dir();
        if dir.file_name().map(|n| n == "vershion").unwrap_or(false) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    let selftest =
        crate::env_var("VERSHION_SELFTEST").as_deref() == Ok("1")
            || crate::env_var("VERSHION_SEED").as_deref() == Ok("1");
    if !selftest {
        return false;
    }
    let ctx = egui::Context::default();
    let mut app = App::new_from(&ctx);
    if crate::env_var("VERSHION_SEED").as_deref() == Ok("1") {
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
    // Copy the pre-rename data dir once, then install the filesystem backend.
    store::migrate_legacy_dir();
    store::init(Box::new(store::FsBackend::from_env()));
    // Headless one-shot: VERSHION_RENDER_FIG=<figure id> [VERSHION_FIG_PARAMS='{"formula":"NH3"}']
    // VERSHION_OUT=<file.png> renders a Figures-tab figure to a PNG (strings/bools/numbers).
    if let Ok(id) = crate::env_var("VERSHION_RENDER_FIG") {
        let def = figures::registry().into_iter().find(|d| d.id == id).unwrap_or_else(|| panic!("no figure {id}"));
        let over: serde_json::Value = crate::env_var("VERSHION_FIG_PARAMS").ok().and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
        let p = figures::params_from_json(&def, &over);
        let png = figures::to_png(&(def.render)(&p), 1.5).expect("render");
        std::fs::write(crate::env_var("VERSHION_OUT").expect("VERSHION_OUT"), png).expect("write");
        return Ok(());
    }
    // Headless one-shot: VERSHION_EXPORT=<exam index> VERSHION_OUT=<dir> writes exam.pdf, exam.docx
    // and key.pdf for that exam of the store (the stem-rendering checks read these).
    if let Ok(i) = crate::env_var("VERSHION_EXPORT") {
        let dir = std::path::PathBuf::from(crate::env_var("VERSHION_OUT").expect("VERSHION_OUT"));
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
    // Headless one-shot: VERSHION_FIT=<crop.png> runs the figure-fitting loop on one picture.
    if let Ok(png) = crate::env_var("VERSHION_FIT") {
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
    // Headless one-shot: VERSHION_IMPORT_PDF=<file.pdf> [VERSHION_BANK_NAME=..]
    // runs the Import screen's whole AI path (pages -> model -> Ditto parse -> figure crops)
    // and saves the result as a new bank in the store.
    if let Ok(pdf) = crate::env_var("VERSHION_IMPORT_PDF") {
        import_pdf_headless(&pdf);
        return Ok(());
    }
    // Headless one-shot: VERSHION_PARSE=<reply text> [VERSHION_PAGE=<page.png> VERSHION_PAGE_NO=<n>]
    // runs the AI-import parser (figures cropped from the page image) and prints
    // {questions, warnings, figures} as JSON; the import test battery scores this.
    if let Ok(path) = crate::env_var("VERSHION_PARSE") {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let text = String::from_utf8_lossy(&bytes).to_string();
        let page = crate::env_var("VERSHION_PAGE").ok().and_then(|p| std::fs::read(p).ok());
        let page_no = crate::env_var("VERSHION_PAGE_NO").ok().and_then(|n| n.parse().ok()).unwrap_or(1);
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
    // Headless one-shot: VERSHION_CONVERT=<file> [VERSHION_HINT="..."] transcribes a
    // photo/scan through the configured provider and prints the Blackboard import text.
    if let Ok(path) = crate::env_var("VERSHION_CONVERT") {
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
        let hint = crate::env_var("VERSHION_HINT").unwrap_or_default();
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
    // Test hook: VERSHION_WINSIZE=WxH forces the initial inner size (e.g. 860x700).
    let size = crate::env_var("VERSHION_WINSIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.trim().split_once('x')?;
            Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?])
        })
        .unwrap_or([1180.0, 780.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("verSHion")
            .with_decorations(true)
            .with_inner_size(size)
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("verSHion", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
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
    let name = crate::env_var("VERSHION_BANK_NAME").unwrap_or_else(|_| "Imported questions".into());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut st = store::Store::load();
    let n = questions.len();
    st.banks.push(model::QuestionBank { id: model::new_id(), name: name.clone(), questions, created: now, modified: now, details: Default::default() });
    st.save_banks();
    println!("saved bank \"{name}\" with {n} questions to {}", store::store_dir_string());
}
