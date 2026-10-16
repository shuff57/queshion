# Test Parrot clone — build contract

Desktop clone of https://www.testparrot.com/ in pure Rust: egui + wgpu (eframe), everything local.
You are the delegated worker: write files whole per your assignment; you may run `cargo check`/`cargo test`
in `/home/shuff57/queshion` (block on the shared target lock if another worker holds it).
The AGENTS.md tier gate fires on big inline writes — you ARE the delegated cheaper-tier builder; announce
that and continue. Never touch files outside your ownership list (except adding tests in the same file you own).

## Shared source of truth

`src/model.rs` (already present) is THE data contract. Do not change its public shapes; if you must add
something, add it as a new field with `#[serde(default)]`.

Storage: data at `${XDG_DATA_HOME:-~/.local/share}/queshion/` —
`banks.json`, `exams.json`, `imports.json`, `exports.json`, `settings.json`, `media/` (images by sha256).
`src/store.rs` (already present) owns all persistence; never `std::fs` from UI code.

`src/theme.rs` and `src/main.rs` are owned by the shell worker. `src/layout.rs` (ExportDoc) and
`src/export/` by the exporters worker. `src/formats.rs` by the parsers worker. `src/ai.rs` by the llm worker.

## Interfaces (each worker implements their own file; shell worker wires together)

- Shell (`src/main.rs`, `src/theme.rs`, `src/shell.rs`): owns `enum Screen`, central `AppState`.
- Parsers (`src/formats.rs`): `pub fn detect_and_parse(data: &[u8], filename: &str) -> Result<ImportDoc, String>`;
  supports Blackboard test-generator text (MC/MA/TF/ESS/MAT/FIB/ORD, `*` marks correct, blank line between
  questions), Aiken, simple CSV (header `type,stem,a,b,c,d,answer`), and Test Parrot JSON file
  (`.json`: `{"kind":"test-parrot-question-bank","version":1,"name":..., "questions":[...model::Question...]}`).
  `pub struct ImportDoc { pub name: String, pub questions: Vec<model::Question> }`.
- Exporters (`src/export.rs`): `pub fn render(doc: &layout::ExportDoc, opts: &layout::Options) -> layout::Output`
  where `layout::Output { pdf: Vec<u8>, docx: Vec<u8> }`; pure, no filesystem access.
  `src/layout.rs` also defines `ExportDoc`/`Options` per the struct definitions below.
- LLM (`src/ai.rs`): `struct Ai { cfg: AiCfg, job: Option<Receiver<Result<String, String>>> }` —
  copy the pattern from `~/officecraft/src/ai.rs` (ureq 3, background thread + mpsc + `ctx.request_repaint()`).
  Providers: Ollama (native `/api/chat`, `images: [b64]`, default base `http://127.0.0.1:11434`, model `qwen3-vl:8b`)
  and OpenAI-compatible (OpenRouter `https://openrouter.ai/api/v1`, key from `OPENROUTER_API_KEY` or stored settings).
  `convert_image_to_questions(data, mime, cfg, ctx) -> Receiver<...>` returns the model text, which must be the
  Blackboard import format; shell parses it with the parsers and stores the ImportDoc + Import record.
- Bank UI (`src/bankui.rs`): `pub fn bank_screen(ui, state)` — bank list screen + bank editor screen
  (outline topics panel, cards list, toolbar Import/Export/New question, name edit, search/difficulty/sort),
  question editor modal (all 6 types, correct marks, answer reorderup/down + lock, + Add answer, points,
  difficulty popup, topics chips, image attach via rfd, explain text).
- Exam UI (`src/examui.rs`): `pub fn exam_screen(ui, state)` — exam list + editor:
  top bar (home, name, File/Edit/Format menus, Saved status, Export History, Export), page-like article
  (header line, title, numbered questions, Answer Section), right resizable open-bank panel with search/topi
  filter and click/drag to add, item move/remove, undo/redo of item list snapshots, Format: heading size /
  text size / margins / answer blank ruling; export dialog (PDF/DOCX, student test + answer key checkboxes,
  shuffle q/a order, versions count) producing Output via `src/export.rs` + Export record.

## Verification bar

`cargo check` then `cargo test` must pass at integration. The crate is bin-only: run plain `cargo test` (not `--lib`).

## Frozen types on disk

`src/model.rs` and `src/store.rs` already exist and compile — treat them as truth; read them.
Each parallel worker REPLACES their stub file with the same public signatures (the shell worker first
writes the stub files exactly as listed there, so the crate keeps compiling). If a stub signature is
wrong in a way you cannot work around, fix your own file and note it in your report — do not edit
another worker's file.

## Cross-worker state (defined by the shell worker in main.rs; everyone writes against it)

```rust
pub enum Screen { Welcome, Home, Banks, Exams, Imports, Settings, BankEdit(Id), ExamEdit(Id) }

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default)] pub ai_provider: String, // "ollama" | "openrouter" | "custom"
    #[serde(default)] pub ai_model: String,
    #[serde(default)] pub ai_base: String,     // custom OpenAI-compatible base URL
}

pub struct App {
    pub store: Store,
    pub screen: Screen,
    pub bank: Option<Id>,                 // open bank editor
    pub exam: Option<Id>,                 // open exam editor
    pub qedit: Option<qedit::Draft>,      // open question editor modal
    pub import_open: bool,
    pub paste: String,
    pub ai: ai::Ai,
    pub panel_bank: Option<Id>,           // bank tab open in the exam editor side panel
    pub export_open: bool,
    pub history_open: bool,
    pub toasts: Vec<(String, bool)>,      // (text, is_error)
    pub settings: Settings,
    pub textures: std::collections::HashMap<String, egui::TextureHandle>, // media cache, key = model::ImageRef.file
    pub undo: Vec<Vec<model::ExamItem>>,  // exam editor snapshots (push before each change)
    pub redo: Vec<Vec<model::ExamItem>>,
}
```

Screen-drawing methods live in the owning worker's file as inherent `impl App` methods:
- `screens.rs` (shell worker): `draw_welcome(ctx)`, `draw_home(ctx)`, `draw_imports(ctx)`, `draw_settings(ctx)`, `draw_sidebar(ctx)`, `draw_toasts(ctx)`.
- `bankui.rs` (bank worker): `draw_banks(ctx)`, `draw_bank_edit(ctx)`, `draw_question_editor(ctx)`.
- `examui.rs` (exam worker): `draw_exams(ctx)`, `draw_exam_edit(ctx)`, plus private `draw_export_dialog(ctx)`, `draw_history(ctx)`.
`qedit.rs` (bank worker) defines `pub struct Draft { pub bank_id: Id, pub qid: Option<Id>, pub stem: String, pub kind: model::Kind, pub points: Option<f64>, pub difficulty: Option<model::Difficulty>, pub topics: Vec<String>, pub image: Option<model::ImageRef>, pub explanation: Option<String> }` and any of its own impls; nobody outside bankui/qedit touches Draft internals.

## Fiddly bits, spelled out once

- Export pipeline (exam worker → exporters): exam worker maps `Exam` + resolved bank questions into
  `layout::ExamDoc` per the exporters worker's definition in `src/layout.rs`; shuffling (question order;
  answer order skipping `locked` answers, remapping the printed key letters) happens THERE in examui,
  producing one `ExamDoc` per named version plus an optional answer-key `ExamDoc`. Then
  `export::render_pdf(doc) -> Result<Vec<u8>, String>` / `export::render_docx(doc)`; files saved under
  `store::store_dir()/exports/` and recorded as `model::ExportRecord`. DOCX may replace embedded pictures
  with a `[picture]` line; PDF must draw them. Every export snapshot is ALSO saved as JSON
  (`exports/<id>.doc.json`) so re-export-from-history is reproducible; that is the clone's "the version
  you handed out is always there".
- AI conversion (imports screen + convert path): read image/PDF bytes → if pdf and `pdftoppm` exists,
  rasterize first page(s) (poppler is optional at runtime, never a build dep) → `ai` sends the image with
  a prompt that instructs a vision model to transcribe the test into EXACTLY the Blackboard import text
  (question blocks `MC|MA|TF|ESS|MAT|FIB|ORD`, `*` marks correct answers, blank line between questions,
  don't number, preserve answer order, wrap `___` blanks as-is, emit nothing but the import text).
  The returned text is parsed by `formats` into an `ImportDoc`, questions become a NEW Question Bank,
  and an `Import` record with `how: "ai"` + model name is added. Show progress in the dialog; keep it
  on the background thread (never block egui).