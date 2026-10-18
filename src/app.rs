//! Test Parrot clone — app entry and shared shell state. Screen bodies live in the screen
//! modules (`screens`, `bankui`/`qedit`, `examui`). Persistence is `store`; AI is `ai`.
//!
//! Shell-owned extra fields (added by the shell worker, per NOTES contract extension —
//! not serialized anywhere, so no `#[serde]` applies to `App`):
//! `toast_times` (toasts vec parallel timestamps), `help_open`, `confirm_restore`,
//! `ai_busy`, `import_file`, `ai_hint`.

use egui;
use crate::{ai, figuresui, model, qedit, screens, store, theme};
use crate::model::{ExamItem, Id};
use crate::store::Store;

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

    /// Re-installs the app's fonts and palette after a host tab switch in a shared context.
    pub fn context_changed(&mut self, ctx: &egui::Context) {
        theme::setup(ctx);
        theme::set_dark(self.settings.dark, Some(ctx));
        ctx.request_repaint();
    }

    /// Opens a file from a path (native host: officecraft, command line).
    pub fn open_path(&mut self, path: &std::path::Path) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        match crate::platform::read(path) {
            Ok(bytes) => self.open_import_bytes(&name, bytes),
            Err(e) => self.toast_err(format!("Couldn't read {name}: {e}")),
        }
    }

    /// Opens a file already in memory (browser drop/picker, officecraft `deliver`).
    pub fn open_import_bytes(&mut self, name: &str, bytes: Vec<u8>) {
        crate::importui::open_external(self, name, bytes);
    }

    pub fn new_from(ctx: &egui::Context) -> Self {
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

}

impl App {
    /// Test hook: VERSHION_SCRIPT="click:x,y;text:abc;key:Enter;wait" injects one input step
    /// every 10 frames after a 60 frame warm-up. Coordinates are egui points.
    pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
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

    pub fn ui(&mut self, ui: &mut egui::Ui) {
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
