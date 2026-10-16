//! Test Parrot clone — app entry and shared shell state. Screen bodies live in the screen
//! modules (`screens`, `bankui`/`qedit`, `examui`). Persistence is `store`; AI is `ai`.
//!
//! Shell-owned extra fields (added by the shell worker, per NOTES contract extension —
//! not serialized anywhere, so no `#[serde]` applies to `App`):
//! `toast_times` (toasts vec parallel timestamps), `help_open`, `confirm_restore`,
//! `ai_busy`, `import_file`, `ai_hint`.

use eframe::egui;

pub mod ai;
pub mod bankui;
pub mod examui;
pub mod export;
pub mod figures;
pub mod figuresui;
pub mod formats;
pub mod layout;
pub mod model;
pub mod qedit;
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
    pub import_open: bool,
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
        let mut app = App {
            store: Store::load(),
            screen: Screen::Welcome,
            bank: None,
            exam: None,
            qedit: None,
            import_open: false,
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
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
        let text = ai::transcribe(&bytes, mime, &cfg, &ai::prompt_for(&hint))
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("queShion")
            .with_decorations(true)
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("queShion", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}