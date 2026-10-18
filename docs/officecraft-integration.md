# verSHion in OfficeCraft: integration plan

Status: plan only (2026-10-18). Nothing in verSHion, OfficeCraft or any sibling repo has been
changed for it. Every claim below was read from source; file:line references are against the
working trees on that date.

## 0. Which OfficeCraft

There are two checkouts with the same remote (`github.com/shuff57/officecraft`):

| Path | State | Deps | Notes |
|---|---|---|---|
| `~/officecraft` | 3 commits, last 2026-10-10 (`6a938b9`), dirty `Cargo.toml`/`Cargo.lock` | **path** deps `../wordcraft` ... and `../Documents/GitHub/viSHual/engine/*` | The tree the memory note `officecraft-shell.md` and `~/officecraft/HANDOFF.md` describe. `Cargo.lock` is stale (`cargo tree --locked` refuses). Graph tab is its own `src/graph.rs` over `math_playground`. |
| `~/Documents/GitHub/officecraft` | 63 commits, last 2026-10-15 (`0611351`), clean | **git** deps (`shuff57/wordcraft@equashion-editor`, `storytold/gridcraft`, `shuff57/deckcraft@equashion-editor`, `storytold/pdfcraft`, `shuff57/viSHual`, `shuff57/equaSHion@3d3b840`) | Tiling, Equations tab, `*-mcp` tool sets, updater, wasm/Trunk web build, `install.sh`. |

**This plan targets `~/Documents/GitHub/officecraft`** (called "the shell" below). The older tree
is referenced only where it documents the same contract. The memory note is partly out of date:
WordCraft's `context_changed()` is now committed (`6b1b63b` in `~/wordcraft`), and the newer
shell builds from git, so "uncommitted sibling patches" and "repeat pdfcraft's `[patch]` as path
entries" apply only to `~/officecraft`. (Decision D1 below.)

---

## 1. How OfficeCraft hosts a Craft

All in `~/Documents/GitHub/officecraft/src/main.rs` (3102 lines) unless noted.

### 1.1 The app "trait" (by convention; there is no Rust trait)

Each Craft exposes an inherent-method surface on its UI struct, and the shell wraps them in
`enum Tab` (main.rs:58) and dispatches with `match`:

| Shell call (`impl Tab`) | WordCraft / DeckCraft / GridCraft | viSHual (`VishualApp`) | PdfCraft |
|---|---|---|---|
| `Tab::create(kind)` (:157) | `WordApp::new(Session, Services)` | `VishualApp::new(Services)` | `PdfCraftApp::new()` |
| `logic(ctx, frame)` (:175) | `a.logic(ctx)` | (none) | `eframe::App::logic` |
| `raw_input_hook(ctx, raw)` (:185) | `a.raw_input_hook(raw)` | (none) | `eframe::App::raw_input_hook` |
| `ui(ui, frame)` (:195) | `a.ui(ui)` | `a.ui(ui)` | `eframe::App::ui` |
| `is_dirty()` (:266) | `session.dirty` / `documents().any(is_dirty)` | `a.changed` | `first_dirty()` |
| `context_changed(ctx)` (:279) | `a.context_changed()` | `a.context_changed(ctx)` | shell restores a captured `Look` |
| `open(path)` (:309) | `run("file.open")` / `open_path` | `open_path` | `open_path` |
| `deliver(name, bytes)` (wasm) | `services.inbox` | - | `startup_inbox` |

Only the **active** tab gets `logic`/`raw_input_hook`/`ui` (or, when tiled, the front tile gets
input and the others run under `with_events_withheld`, main.rs:514). The tab body receives the
`Ui` left over after the shell's top tab strip and optional right assistant panel.

`Services` structs are the host-injection seam: `wordcraft_ui_egui::Services`
(`~/wordcraft/crates/ui-egui/src/lib.rs:41`: `pick_open`, `pick_save`, `open_async`, `inbox`,
`download`) and `vishual_ui_egui::Services`
(`~/Documents/GitHub/viSHual/crates/ui-egui/src/services.rs`: `pick_open`, `pick_save`,
`open_url`, `gallery_store: Option<PathBuf>`). The shell builds them in `word_services()`,
`grid_services()`, `deck_services()`, `graph_services()` (main.rs:68-155) with `rfd` dialogs
(native) and `None` (wasm). **The UI crates have no dialog crate of their own.** verSHion must
follow that.

### 1.2 Shared egui, fonts and theming

- One eframe 0.36.2 window, one `egui::Context`. Each app replaces the whole `FontDefinitions`
  and style when it comes to the front. Switching without reinstalling panicked
  (`FontFamily::Name("semibold") is not bound`).
- Fix in the shell: `ctx_owner: Option<Kind>` (:987); on a switch the shell calls
  `Tab::context_changed` (:2627-2635). For apps that cannot reinstall themselves
  (`restores_look`, :529: Grid, Pdf) it captures and replays a `Look { fonts, light, dark }`
  (:334). `Look::apply_fonts` installs fonts under a **plain style first** and the real style a
  frame later, because fonts activate on the next pass but a style naming an unbound family
  panics at once. WordCraft does the same internally (`fonts_frames < 2` early return in
  `WordApp::ui`).
- Tiling: `merge_fonts`/`fonts_cover` (:483, :507) keep a union of tiled apps' font sets. Apps
  that "bring their own fonts and layout" are `!tileable()` (:38): Graph, Equation.
- Fonts: WordCraft embeds Inter/JetBrains Mono (`include_bytes!` in `crates/ui-egui/src/theme.rs`);
  viSHual embeds Noto Sans (`control.rs:53`); CJK faces come from `~/craft-fonts`
  (`CRAFT_FONTS_DIR`, `install.sh:33`). Nothing reads fonts relative to the cwd.

### 1.3 Storage, settings, env hooks

- Shell config: `$XDG_CONFIG_HOME/officecraft/` (`session.txt`, `recent.txt`, `folders.txt`,
  `ai.json`, `ai-keys.json` (0600), `sources`, tile prefs, `vishual-gallery.json` via
  `graph_services().gallery_store`).
- Env hooks: `OFFICECRAFT_SELFTEST`, `_SELFTEST_AI`, `_SELFTEST_AI_APP`, `_SELFTEST_AI_FILE`,
  `_SELFTEST_GRAPH`, `_SHOT`, `_SHOT_TAB`, `_SHOT_VISHUAL`, `_SHOT_EQUATION(_STATIC)`,
  `_SHOT_CHAT`, `_TILES`, `_AUTOSEND`, `_AI_URL/_MODEL/_KEY`. All read in the shell's `main`,
  never inside the Craft crates.

### 1.4 Menus, shortcuts, files, assistant

- No suite menu bar: each app draws its own ribbon/menus inside its `ui`. The shell owns the tab
  strip, `+` menu, Home, close confirmation, drop overlay, Settings and Updates windows.
- Files route by extension in `kind_for` (:239). Dropped files of a known kind become tabs in
  `raw_input_hook`; others stay for the active app.
- Assistant (`src/ai.rs`, 2324 lines): `enum App { Docs, Sheets, Slides, Pdf, Graph }` (:430);
  tools come from each Craft's `*-mcp` crate (`tool_definitions()`, `call_tool(&mut dyn Backend,
  name, args)`, ai.rs:1415-1734) over a `Bridge` implementing `Backend::call(method, params)`
  onto the app's control surface. Providers: local endpoint, Ollama Cloud, OpenRouter: the same
  three verSHion has.
- No secondary viewports anywhere in the shell (`grep show_viewport` finds nothing). The
  HANDOFF lists tear-off windows as "not attempted".
- wasm: 42 `cfg(target_arch = "wasm32")` sites; the web build is real (Trunk, `src/web.rs`).

---

## 2. verSHion today

Single **bin** crate, 72.7 k lines in `src/`, 480 `#[test]`s, plus `tests/import_formats.rs`
(drives the binary through `VERSHION_PARSE`). 40 files carry uncommitted changes (about 6.7 k
lines, `git diff --stat`). License MIT (shell is Apache-2.0; compatible).

### 2.1 Modules and what they touch

| Module | Lines | egui? | Internal deps that matter for a split |
|---|---|---|---|
| `model.rs` | 457 | no | none (the data contract) |
| `store.rs` | 182 | no | **`crate::screens::civil_from_days`** (store.rs:58) |
| `formats.rs` | 6376 | no | ditto, fonts, richtext, store |
| `ditto.rs` | 641 | no | formats, richtext, store |
| `richtext.rs` | 1376 | no | none |
| `layout.rs` | 114 | no | export, richtext |
| `export.rs` | 3607 | no | **`crate::equation::stem_to_plain` / `heading_scale`** (:476, :767, :2190) |
| `fonts.rs` | 535 | no | `equashion_font::bundled()` (STIX bytes) |
| `figures/` | ~25 k | 1 doc ref | **`crate::figuresui::params_from_spec/spec_for`** (figures/math.rs:4064, :4119) |
| `tidy.rs`, `bankops.rs` | 698, 419 | no | ditto, formats, model, store |
| `figfit.rs` | 142 | no | ai, ditto, figures, **figuresui** (`params_from_spec`, `flatten_white`, `spec_for`, :64-67) |
| `ai.rs` | 652 | `send_image(.., ctx)` only | ditto, figfit, formats, store |
| `equation.rs` | 1284 | yes (39) | theme, fonts, richtext; wraps `equashion_egui::EquationEditor` |
| `theme.rs`, `kit.rs` | 553, 186 | yes | - |
| `screens.rs`, `bankui.rs`, `qedit.rs`, `examui.rs`, `importui.rs`, `figuresui*`, `wysiwyg*` | ~26 k | yes | the UI |

So the "core" set is already almost egui-free; five back-edges (bold above) are the whole cost of
a clean split.

### 2.2 Global state

- **Store path**: `store::store_dir()` (store.rs:7) reads `XDG_DATA_HOME`/`HOME` on every call;
  used directly by `ai.rs:178,186` (`ai-keys.json`), `qedit.rs:1242`, `bankui.rs:1447`,
  `screens.rs:533,1240,1249,2044,2090`, `examui.rs:4578`, `main.rs`. `save_json`/`load_json`/
  `put_image` take a file name and resolve the dir themselves.
- **Settings**: `crate::Settings` in `main.rs:39`, loaded from `settings.json` in the store dir
  (`App::new_from`, :126), saved from `set_dark_mode` (:121).
- **Dark mode**: `static DARK_MODE: AtomicBool` (theme.rs:114) read by every colour accessor via
  `pal()`. Process-wide.
- **Fonts**: `static FONTS: OnceLock<Option<FontDefinitions>>` (theme.rs:309). The four Source
  Serif 4 faces are `include_bytes!` (fonts.rs:13-16) but **Source Sans 3 (4 faces) and
  Serif Semibold are read from disk** via `theme::fonts_dir()` (theme.rs:313): `$CWD/assets/fonts`
  or `<exe>/../../assets/fonts`. If neither exists `load_fonts` returns `None` and no named
  families are registered, yet 98 call sites use `serif_family()`, `bold_family()`,
  `serif_bold_family()` unguarded (theme.rs:170-200) → **"FontFamily::Name("serif") is not
  bound" panic** whenever verSHion runs from another cwd or an installed path, including inside
  the shell. Family names (`serif`, `serif-bold`, `sans-bold`, `serif-italic`, ...) are generic
  and would collide with other apps in the shell's tiling font union.
- **resvg fontdb**: `figures::fontdb()` (figures/mod.rs:421) `OnceLock` with
  `load_system_fonts()` + the serif faces; first figure render stalls on the system scan.
- **thread_locals**: `importui.rs:135` (`ST`, `MST` import dialog state), `qedit.rs:1501`
  (`EQ_PANEL`), `equation.rs:315` (`CACHE`), `figures/biology/punnett.rs:17,118,123`. One
  verSHion instance per process is assumed.
- **Id seq**: `model.rs:9` `static SEQ: AtomicU64` (fine).
- **Env hooks**: 45 distinct `VERSHION_*` (+ `PHYS_SVG`, `PHYS_EX_DIR`, `FIG_OVERLAY_DIR`) spread
  over 16 files, not only `main.rs`: `bankui.rs` (`VERSHION_POPOVER`, `_POPOVER_FILTERS`,
  `_EMBED_VIEWPORTS`), `examui.rs`, `importui.rs` (`_IMPORT_MODAL`, `_IMPORT_FILES`, `_ATTACH`,
  `_PASTE`), `figuresui.rs` (`_FIG`, `_FIG_LINK`, `_FIG_FAQ`, ...), `wysiwyg.rs`
  (`_WYS_DEBUG`), `figuresui/composite_canvas.rs` (`_COMP`), `figuresui/pedigree_panel.rs`
  (`_PED_SEL`, `_PED_DO`), `formats.rs`, `figfit.rs`, `figures/layout.rs`, `figures/census.rs`.
  `main.rs` one-shots call `std::process::exit` (5 sites) and `VERSHION_SHOT` exits from
  `raw_input_hook` (main.rs:216). In a host these would kill the whole suite.
- **Test env mutation**: `ditto.rs:514` `unsafe { set_var("XDG_DATA_HOME", ..) }` in tests.

### 2.3 Window ownership assumptions

- `eframe::App for App` in `main.rs:205`; `ui()` ignores the passed `Ui` and calls
  `draw_screen(&ctx)` (screens.rs:374), which builds its own full-window root with
  `screens::root_ui(ctx)` (screens.rs:102: `Ui::new(.., max_rect(ctx.viewport_rect()))`).
  12 call sites (`screens.rs` 9, `examui.rs` 2, `importui.rs` 1) via `root_ui`/`shell_frame`.
  Embedded, that would paint over the shell's tab strip and assistant panel.
- `ctx.viewport_rect()` / `content_rect()` used for layout decisions (screens.rs:555 sidebar
  collapse < 900 px, figuresui.rs:745,948,1065, importui.rs:601, bankui.rs:2185, wysiwyg.rs:3572,
  3657). They must measure the tab, not the window.
- Context-level overlays (`egui::Modal`, `Window`, `Area` `.show(ctx)`) in bankui, importui,
  qedit, figuresui, examui, screens toasts. These are fine embedded (centred on the window) but
  share the window-wide modal stack with the shell's dialogs.
- **Bank Pop-over** is a real secondary viewport: `bankui.rs:2645`
  `ctx.show_viewport_immediate(ViewportId::from_hash_of("qs_bank_popover_window"), ..)` with
  an `EmbeddedWindow` fallback path already written (`class == ViewportClass::EmbeddedWindow`,
  :2659) and `send_viewport_cmd_to(ROOT, Focus)` (:2842). `VERSHION_EMBED_VIEWPORTS=1` forces
  the embedded path.
- Screenshots: `ViewportCommand::Screenshot` (main.rs:316) handled in `raw_input_hook`.
- Native-only: `rfd` (qedit 1, screens 2, importui 3, figuresui 3, bankui 1, examui 1),
  `std::process::Command` (`xdg-open` screens.rs:1249, examui.rs:4765; `pdftoppm` in
  `screens::pdf_pages_png` screens.rs:1992; `pdftotext` in export tests), `std::thread` (AI
  jobs), `ureq`, direct `std::fs` in 20 modules.
- Shortcuts consumed directly: `examui.rs:791-794` (Cmd+Z/Shift+Z/Y/E), `figuresui.rs:887-891`,
  `composite_canvas.rs:732` (Cmd+D). Fine embedded (only the active tab runs) but Cmd+E/Cmd+D
  must not be suite shortcuts.
- Generic egui ids: `"sidebar"`, `"toasts"`, `"__tp_ui"` (keyed by viewport id). Low risk while
  only the active tab draws; real collision risk once tiled.

### 2.4 Dependencies vs the shell (cargo tree comparison)

`cargo tree -e normal` for verSHion vs `~/officecraft` (resolved in a scratch copy) and the
newer shell's `Cargo.lock`:

| Crate | verSHion | Shell | Verdict |
|---|---|---|---|
| egui / eframe / epaint / egui-winit | 0.36.2 | 0.36.2 | same. verSHion adds eframe feature `links` (additive). |
| egui-wgpu | 0.36.2 (crates.io) | 0.36.2 **patched** (storytold/pdfcraft vendor: `surface_fit`) | patch applies to verSHion inside the shell; harmless. |
| winit | 0.30.13 | 0.30.13 **patched** + 0.29.15 (viSHual engine) | fine. |
| wgpu | 30.0.1 | 30.0.1 + 0.19.4 (viSHual headless) | fine; verSHion uses no wgpu directly. |
| resvg / usvg / tiny-skia | 0.45.1 (`text`,`system-fonts`) | 0.45.1 via `egui_extras` (pdfcraft) | unify; verSHion adds `fontdb` 0.23 + `fontconfig-parser`. |
| ttf-parser | 0.25.1 | 0.25.1 (rustybuzz/krilla) | same. |
| zip | 3.0.0 | 3.0.0 (deckcraft/gridcraft/wordcraft) | same. |
| roxmltree | 0.20.0 | 0.20.0 (pdfcraft-xfa, usvg) | same. |
| image | 0.25.10 | 0.25.10 | same (features union). |
| flate2, ureq, rfd | 1.1.10, 3.4.2, 0.17.2 | same | same. |
| sha2 | **0.10.9** | **0.11.0** (gridcraft-model, lopdf, ed25519) | two copies (+digest/block-buffer 0.10). Bump verSHion to 0.11 (`Sha256::digest` API unchanged; `format!("{hash:x}")` needs checking). |
| equashion-* | **path** `../equaSHion` (HEAD `3d3b840`) | **git** `shuff57/equaSHion?rev=3d3b840` **and** a second copy from `shuff57/viSHual` (its vendored `equashion/`) | Same revision. Path source ≠ git source, so as-is verSHion would add a **third** copy. Switch verSHion to the same git rev (or let the shell `[patch]` it). |
| verSHion-only | `fontdb`, `fontconfig-parser`, `slotmap`, `unicode-vo`, `generic-array` | - | small. |

Patch-level drifts (`serde_json` 1.0.151/154, `syn`, `uuid`, ...) resolve to one version in the
shell's lock. No hard conflicts.

---

## 3. Target architecture

### 3.1 Crates (mirrors the Craft layout: `*-engine`/`*-ui-egui`/`*-mcp`)

```
vershion/                       (cargo workspace, same repo)
  crates/core      vershion-core      model, store (StoreDir), formats, ditto, richtext,
                                      layout, export (PDF/DOCX), fonts (all faces embedded),
                                      figures (engine + params/spec helpers), bankops, tidy,
                                      figfit (blocking), ai (blocking provider calls, keys IO
                                      behind a path), equation_text (stem_to_plain, heading_scale)
  crates/ui-egui   vershion-ui-egui   VershionApp + screens, bankui, qedit, examui, importui,
                                      figuresui*, wysiwyg*, equation (editor), theme, kit,
                                      ai job runner (thread + repaint)
  crates/mcp       vershion-mcp       tool_definitions(), call_tool(&mut dyn Backend, ..)
  apps/vershion    vershion (bin)     eframe main, VERSHION_* hooks parsed into a QaHooks
                                      struct, headless one-shots, rfd Services
```

`vershion-core` depends on no egui (enforce with a CI `cargo tree -p vershion-core | grep -c egui`
== 0). It keeps `resvg`, `zip`, `roxmltree`, `ttf-parser`, `flate2`, `image`, `sha2`,
`equashion-math/-layout/-font`; `ureq` behind a `net` feature (default on). `vershion-ui-egui`
adds `egui` only (no `eframe`, no `rfd`), `equashion-egui`, `equashion-render`.

### 3.2 Host surface (what the shell calls)

```rust
pub struct Services {
    pub pick_open: Option<Box<dyn Fn(&str /*purpose*/, &[(&str, &[&str])]) -> Option<Vec<PathBuf>>>>,
    pub pick_save: Option<Box<dyn Fn(&str /*suggested*/, &[(&str, &[&str])]) -> Option<PathBuf>>>,
    pub pick_folder: Option<Box<dyn Fn() -> Option<PathBuf>>>,
    pub open_path: Option<Box<dyn Fn(&Path) -> bool>>,          // replaces xdg-open
    pub open_in_suite: Option<Box<dyn Fn(&Path) -> bool>>,      // exported PDF/DOCX -> Pdf/Docs tab
    pub ai: Option<Box<dyn Fn() -> vershion_core::ai::AiCfg>>,  // host-owned provider + key
    pub viewports: bool,                                        // may open secondary windows
}
pub struct Paths { pub store: PathBuf /* banks.json, media/, backups/, settings.json */ }

impl VershionApp {
    pub fn new(ctx: &egui::Context, paths: Paths, services: Services) -> Self;
    pub fn logic(&mut self, ctx: &egui::Context);          // AI polling, toasts clock, drops
    pub fn raw_input_hook(&mut self, raw: &mut egui::RawInput); // QA script only, if enabled
    pub fn ui(&mut self, ui: &mut egui::Ui);               // draws inside ui.max_rect()
    pub fn context_changed(&mut self, ctx: &egui::Context);// reinstall fonts, then style next frame
    pub fn is_dirty(&self) -> bool;                        // unsaved draft / export dialog open
    pub fn open_path(&mut self, path: &str) -> Result<(), String>; // stage an import
    pub fn control(&mut self, method: &str, params: Value) -> Result<Value, String>; // for mcp
    pub fn set_dark(&mut self, dark: bool, ctx: &egui::Context);
}
```

The standalone bin becomes `struct Standalone(VershionApp, QaHooks)` with `impl eframe::App`
forwarding to these methods, `rfd` services, `viewports: true`, and the screenshot/exit hooks.

### 3.3 Embedded mode vs standalone

| Concern | Standalone | Embedded in the shell |
|---|---|---|
| Store dir | `$XDG_DATA_HOME/vershion` | Same dir by default, so a teacher sees one library (D3). The shell passes `Paths`; verSHion never reads env. |
| Concurrent writers | one process | standalone + shell could both write `banks.json` (atomic rename, last writer wins). Add a `store.lock` (pid file) and open read-only with a banner if held. |
| Settings | `settings.json` in store | `dark` follows the shell's look when the host sets it; `ai_*` ignored when `Services.ai` is set; view-mode toggles stay. |
| AI keys | `<store>/ai-keys.json` + `OLLAMA_API_KEY` + `~/.config/ollama-cloud/credentials.json` | `Services.ai` returns the suite's provider/model/key (`~/.config/officecraft/ai-keys.json`). One key store. |
| Fonts | installed in `new` | installed by `context_changed`, fonts first under a plain style, verSHion style a frame later (same protocol as `Look::apply_fonts`). Embedded bytes only; family names prefixed `qs-`. |
| Window | owns root viewport | draws inside the tab `Ui`; `root_ui` replaced by `ui.new_child(UiBuilder::new().max_rect(ui.max_rect()))`; "narrow" measured on that rect. |
| Pop-over | `show_viewport_immediate` | `Services.viewports == false` → the existing `EmbeddedWindow` branch (an `egui::Window`). Turn on later when the shell supports viewports. |
| Dialogs | `rfd` | `Services` closures (shell passes `rfd`; wasm passes none). |
| Shortcuts | consumed directly | unchanged (only the active tab runs). Avoid suite chords; the shell has none today. |
| Instances | one | **singleton tab** ("Exams"): thread_locals and the shared store make two instances unsafe. `+ New Exams` focuses the existing tab. |
| Exit/QA hooks | env | none in the library; bin-only. |
| Tiling | n/a | `tileable() == false` at first (like Graph/Equation); later true once fonts are namespaced and `merge_fonts` can union them. |
| Web build | n/a | `cfg(not(target_arch = "wasm32"))` on the Kind (store, threads, ureq, resvg system fonts are native). |

---

## 4. Shared pieces and de-duplication

| # | Overlap | Where | Recommendation | Value / cost |
|---|---|---|---|---|
| 1 | equaSHion | verSHion path dep; shell git rev `3d3b840`; viSHual vendors its own copy | verSHion pins `git = shuff57/equaSHion, rev = 3d3b840` (bump with WordCraft/DeckCraft, per the shell's Cargo.toml comment). Separately ask viSHual to depend on the git repo instead of `viSHual/equashion/` (removes the existing duplicate). | high / trivial |
| 2 | STIX Two Math bytes | `equashion_font::bundled()`; verSHion `fonts::fallback_bytes` already reuses them | keep; nothing to do. | done |
| 3 | AI providers + key storage | verSHion `ai.rs` (cloud/ollama/openrouter/custom, `ai-keys.json` 0600); shell `ai.rs` (same three, `~/.config/officecraft/ai-keys.json` 0600) | `Services.ai` in embedded mode; later extract a tiny `craft-ai-config` crate both use. Vision calls stay in verSHion (shell chat is text tools). | high / small |
| 4 | Assistant tools | shell uses `*-mcp` crates | new `vershion-mcp` with `Backend::call` over `VershionApp::control` (`bank.list`, `bank.inspect`, `question.add/update`, `exam.create`, `exam.add_items`, `exam.export`, `import.text`). | high / medium |
| 5 | PDF page rasterising | verSHion shells out to `pdftoppm` (`screens::pdf_pages_png`) | use pdfcraft's renderer (hayro, already in the shell) behind a `Services.render_pdf_pages` hook; standalone keeps `pdftoppm` or adopts the same crate. Removes a poppler runtime dependency. | medium / medium |
| 6 | Exported files | verSHion writes PDF/DOCX itself | `Services.open_in_suite`: after export, open the PDF in a PdfCraft tab or the DOCX in WordCraft. No writer change. | high / small |
| 7 | PDF writer | verSHion hand-rolled (`export.rs`, subsets, Type0/Type3, byte-identical tests); WordCraft/DeckCraft use krilla via `storytold/craft-libs` | **keep verSHion's**. It is tuned to Test Parrot output and tested; krilla brings a patched fork. Revisit only if tagging/accessibility is required. | low / large |
| 8 | DOCX writer | verSHion `export.rs`; `wordcraft-docx` | keep; consider round-trip test: export DOCX → open in WordCraft engine → assert paragraph count (cheap integration test in the shell). | low |
| 9 | XLSX import | verSHion own zip+roxmltree reader (`formats.rs`); `gridcraft-xlsx` | keep (small, read-only, no formulas needed). | low |
| 10 | Figures vs viSHual | verSHion `figures/math_plot.rs` (553), `math.rs` (5742) function plots and geometry, SVG via resvg; viSHual `math_core` (parser, evaluators, meshing) + GPU headless renderer + `VishualApp::render_png` | Do not merge engines (SVG print-vector vs GPU raster; different goals). Add bridges: (a) "Graph from viSHual" figure source in verSHion that calls `render_png` through a host service; (b) shell "Insert figure" in Docs/Slides from the verSHion figure registry. | medium / medium |
| 11 | "Picture that remembers its source" | shell `src/graphpic.rs` (viSHual doc in PNG `tEXt` chunk `vishual-graph`); verSHion `figures/share.rs` (`vershion://figure/<id>?p=<b64url>`) | Use the same mechanism: PNG `tEXt` key `vershion-figure` carrying the share payload, so a verSHion figure pasted into a document reopens in the Figures workspace. Move base64 helpers (3 copies: graphpic, share.rs, viSHual `base64` crate) to one place. | medium / small |
| 12 | Equation editor chrome | shell `src/equation.rs` (`EquationApp`); verSHion `equation.rs` (editor modal + typeset cache) | both thin wrappers over `equashion-egui`; no shared code needed. Keep verSHion's `stem_to_plain`/`heading_scale` in core. | none |
| 13 | Fonts on disk | Source Sans 3 / Source Serif 4 (verSHion `assets/fonts`, 2.7 MB); `~/craft-fonts` holds only CJK/Arabic/Thai | embed in `vershion-core::fonts`; no sharing (no other app uses Source*). | needed for correctness |

---

## 5. Migration plan

Each milestone leaves both apps building, testable and shippable. Sizes are focused working
days for one agent/person. Verification per milestone: `cargo test --workspace` (480+ tests),
`tests/import_formats.rs`, `VERSHION_SELFTEST=1`, a `VERSHION_SHOT` per screen compared to the
previous build, and from M4 `OFFICECRAFT_SELFTEST=1` plus `OFFICECRAFT_SHOT_TAB`.

### M0. Baseline (0.5 d)
- Commit the 40 dirty files in verSHion as they are (or on a branch); tag `pre-officecraft`.
  Several files the build needs are **untracked**: `src/fonts.rs`, `src/tidy.rs`,
  `src/figures/{cards,census,items}.rs`, `src/figuresui/cards_panel.rs`,
  `assets/fonts/SourceSerif4-{It,BoldIt}.ttf` (both `include_bytes!`-ed), the OFL texts,
  `docs/fonts.md`, `tests/fixtures/`. A fresh clone of the GitHub repo does not build until
  they are added, and M4 builds the shell from that clone.
- Capture baseline screenshots of every `VERSHION_SCREEN` (with an `XDG_DATA_HOME` temp store
  seeded by `VERSHION_SEED=1`) for visual diffing. Record test counts.
- Ship: nothing changes.

### M1. Make verSHion host-agnostic inside the one crate (2-3 d)
Checklist items C1-C9 (section 6). Main moves: `StoreDir` instead of `store_dir()`; all fonts
embedded and families prefixed; `QaHooks` struct read once in `main`; dark mode into a
per-context value; `Services` for dialogs/`xdg-open`; `root_ui` takes a rect.
- Test: all unit tests; run the binary from `/` (cwd without `assets/`) and confirm no font
  panic; screenshot diff against M0 is pixel-identical (same faces, now embedded).
- Ship: standalone verSHion, behaviour unchanged.

### M2. Workspace split (2 d)
- Create `crates/core`, `crates/ui-egui`, `apps/vershion`; `git mv` files (keep history).
- Break back-edges: `civil_from_days` → `core::time`; `stem_to_plain`/`heading_scale` →
  `core::equation_text`; `params_from_spec`/`spec_for`/`flatten_white` → `core::figures::spec`;
  `figures/math.rs` tests use the moved helpers; `ai::Ai::send_image` (egui `ctx`) stays in
  ui-egui as `AiJob`, blocking calls (`transcribe_checked`, `convert_page`) in core.
- `use eframe::egui` → `use egui` in screens/importui/examui.
- Move `tests/import_formats.rs` to `apps/vershion/tests/` (it drives `CARGO_BIN_EXE_vershion`).
- Test: `cargo test --workspace`; `cargo tree -p vershion-core | grep egui` empty.
- Ship: same binary.

### M3. `VershionApp` host surface (2 d)
- Introduce `vershion_ui_egui::VershionApp` with the section 3.2 methods; the bin wraps it.
- `context_changed` implements fonts-then-style over two frames (copy WordCraft's
  `fonts_frames` gate) so a host switch never draws with unbound `qs-*` families.
- `Services.viewports` drives the Pop-over branch (`bankui.rs:2645`), replacing
  `VERSHION_EMBED_VIEWPORTS`.
- Add a tiny host harness in `apps/vershion/examples/hosted.rs`: an eframe app with a 32 px top
  panel and a right panel, drawing `VershionApp::ui` in the middle and swapping fonts every N
  frames to an egui default set and back (reproduces the shell's font crash locally).
- Test: harness runs 600 frames headless without panic; screenshots show nothing drawn outside
  the central rect.
- Ship: standalone unchanged; the library is now embeddable.

### M4. Exams tab in the shell, native only (1.5 d, in the officecraft repo)
- Publish verSHion to `github.com/shuff57/vershion` (remote already set) and add to the shell:
  `vershion-ui-egui = { git = "https://github.com/shuff57/vershion", branch = "main" }` under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`.
- `Kind::Exams` ("Exams"), `Tab::Exams(Box<VershionApp>)`, entries in `create`, `logic`,
  `raw_input_hook`, `ui`, `is_dirty`, `context_changed`, `open`; `tileable() == false`;
  `has_assistant() == false` until M5; `kind_color`, `kind_abbr`, Home quick-start card.
  Singleton: `new_tab(Kind::Exams)` focuses an open one.
- `kind_for`: only unambiguous extensions (`.parrot.json`, `.ditto.json`, `.gift`, `.qti`).
  Never `.csv/.xlsx/.zip/.txt` (Sheets/Docs own them); those reach verSHion through its own
  Import dialog.
- `exams_services()` beside `graph_services()`: rfd dialogs, `open_path` via the shell's
  platform helper, `open_in_suite` → `self.pending.push(path)`.
- Extend `OFFICECRAFT_SELFTEST` cycle to include Exams; add `OFFICECRAFT_SHOT_TAB=exams`.
- Test: shell selftest cycles Word → Exams → Graph → Exams → Pdf without panic; screenshot.
- Ship: OfficeCraft with an Exams tab.

### M5. Assistant and shared AI config (2 d)
- `vershion-mcp` crate (tool defs + dispatcher) and `VershionApp::control` methods.
- Shell: `ai::App::Exams`, `impl vershion_mcp::Backend for Bridge`, `has_assistant() == true`.
- `Services.ai` hands the suite's provider config to verSHion's image conversion.
- Test: `OFFICECRAFT_SELFTEST_AI_APP=exams OFFICECRAFT_SELFTEST_AI="list my banks"`.

### M6. Cross-app bridges (2-3 d, optional, any order)
- Export → open in PdfCraft/WordCraft tab (`open_in_suite`).
- PDF import pages through pdfcraft's renderer (`Services.render_pdf_pages`), dropping the
  `pdftoppm` requirement in the suite.
- verSHion figure PNGs carry a `vershion-figure` `tEXt` chunk; shell double-click handler (like
  `open_double_clicked_graphs`) opens them in Exams → Figures.
- "Insert graph from viSHual" figure source via a host render service.

### M7. Tileable (1 d, optional)
- With `qs-*` families and embedded faces, verify `merge_fonts` covers verSHion; set
  `tileable() == true`; rename generic ids (`"sidebar"` → `"qs_sidebar"`, `"toasts"` →
  `"qs_toasts"`). Test with `OFFICECRAFT_TILES`.

### M8. Web (not recommended now)
Needs a `wasm` store backend, fetch-based AI, no threads, no resvg system fonts. Weeks, not days.

**Total M0-M5: about 10-11 days; M6-M7 another 3-4.**

### Risks and mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Font-family panic on tab switch (`qs-serif` not bound because style/text used before fonts land) | high without care | M3 two-frame `context_changed`; harness that swaps fonts; never call `theme::serif_family()` before `ctx.fonts` has it (add a debug assert). |
| Fonts missing when not run from repo root (exists today) | certain today | C3: embed every face. |
| Third copy of equaSHion, or a mismatched rev between verSHion and the shell | medium | Pin the same rev; shell CI greps `Cargo.lock` for one `shuff57/equaSHion?rev=` source. |
| Store clobbering between standalone app and shell tab | medium | `store.lock`; read-only banner; or D3 = separate dir. |
| `process::exit`/env hooks firing inside the suite | high if left in lib | C2: hooks only in the bin. |
| thread_local state shared by two instances | medium | singleton tab; later move into `VershionApp`. |
| Secondary viewport misbehaving under the shell (Wayland, focus to ROOT) | medium | `viewports: false` default in the shell. |
| Build time/binary size (+73 k lines, fontdb) | certain | release `lto = "thin"` already; acceptable. |
| Upstream drift: 40 uncommitted verSHion files | high | M0 first. |
| Two concurrent agent sessions in the shell repo (memory note) | medium | `git status` before editing `src/main.rs`; small, additive diffs. |
| `pdfcraft` `[patch]` (egui-wgpu `surface_fit`, winit) changes egui-wgpu under verSHion | low | verSHion has no wgpu code; visual check in M4. |

### Decisions needed from the user

- **D1. Target shell**: `~/Documents/GitHub/officecraft` (git deps, current) rather than the
  stale `~/officecraft`? (Recommended: yes; retire or archive `~/officecraft`.)
- **D2. Repo layout and names**: workspace inside the verSHion repo with Craft-style crate
  names `vershion-core`, `vershion-ui-egui`, `vershion-mcp`, bin `vershion` (recommended), or
  move the crates into the officecraft repo.
- **D3. Data location in the suite**: share `~/.local/share/vershion` with the standalone app
  (recommended, plus a lock), or namespace it (`~/.local/share/officecraft/vershion`).
- **D4. Tab model**: one singleton "Exams" workspace tab (recommended) vs one tab per bank/exam.
- **D5. Look inside the suite**: keep verSHion's cream/terracotta Source Serif/Sans theme in its
  tab (recommended; it previews the printed sheet) or adopt the suite's light/dark tokens.
- **D6. AI settings**: verSHion uses the suite's provider and key in embedded mode
  (recommended) or keeps its own.
- **D7. Figures engine sharing with viSHual**: bridges only (recommended), extract a standalone
  `vershion-figures` crate so Docs/Slides can insert figures, or no sharing.
- **D8. Web**: exclude verSHion from the web build (recommended) or plan a wasm port.

---

## 6. Checklist: changes in verSHion first (before touching the shell)

Ordered; C1-C9 are M1, C10-C14 are M2/M3.

- [ ] **C1 Injectable store path.** `pub struct StoreDir(PathBuf)` with `StoreDir::from_env()`
  (the current `store_dir()` logic, bin only). Change `save_json`, `load_json`, `put_image`,
  `backup_dir`, `backup_banks` and `Store::load` to take `&StoreDir` (or make them methods on
  `Store` which holds it). Replace the direct `store_dir()` calls in `ai.rs:178,186`,
  `qedit.rs:1242`, `bankui.rs:1447`, `screens.rs:533,1240,1249,2044,2090`,
  `examui.rs:4578` with `app.store.dir()`. Remove `ditto.rs:514` `set_var` (tests pass a temp
  `StoreDir`).
- [ ] **C2 Env hooks out of the library.** One `QaHooks` struct parsed in `main.rs` holding every
  `VERSHION_*` value; screens receive `Option<&QaHooks>` (or `App.qa`). Covers `bankui.rs`
  (`POPOVER*`, `EMBED_VIEWPORTS`), `importui.rs` (`IMPORT_MODAL`, `IMPORT_FILES`, `ATTACH`,
  `PASTE`), `figuresui.rs` / `bankui.rs` / `examui.rs` (`FIG*`, `DEMO_FIGURES`, `THUMB_SVG`,
  `OPEN_Q`, `DUMP`, `TIDY`), `wysiwyg.rs`
  (`WYS_DEBUG`), `composite_canvas.rs` (`COMP`), `pedigree_panel.rs` (`PED_*`), `examui.rs`,
  `formats.rs`, `figfit.rs` (`FIT_DEBUG`), `figures/layout.rs`, `figures/census.rs`
  (`CENSUS_DIR`), `figures/physics*.rs` (`PHYS_*`, test-only: gate with `#[cfg(test)]`).
  `process::exit` stays in `main.rs` only; `VERSHION_SHOT` handling moves to the bin wrapper.
- [ ] **C3 Embed all fonts.** `include_bytes!` Source Sans 3 Regular/It/Semibold/Bold and Source
  Serif 4 Semibold in `fonts.rs`; delete `theme::fonts_dir()` and the disk reads in
  `theme::load_fonts`; `load_fonts` returns `FontDefinitions` (not `Option`). Fixes the panic
  when run outside the repo. (+1.9 MB binary.)
- [ ] **C4 Namespace font names.** Families `serif`, `serif-bold`, `serif-italic`,
  `serif-bold-italic`, `sans-bold`, `sans-italic` → `qs-*`; font-data keys already specific
  (`SourceSerif4-*`). Update `theme::italic_family` and the font test (theme.rs ~520).
  Keep putting Source Sans 3 first in `FontFamily::Proportional`: the tab owns the context
  while it is in front, and `context_changed` reinstalls it.
- [ ] **C5 Dark mode per app, not per process.** Replace `static DARK_MODE` (theme.rs:114) with a
  value stored in `ctx.data` (`Id::new("qs_dark")`) or passed palette; `pal()` takes `&Context`
  or a `Palette` cached on `App`. The 30 colour accessors keep their names.
- [ ] **C6 Settings ownership.** `Settings` moves to core; `App` saves it through `Store`; add
  `Settings::ai_overridden` semantics for `Services.ai`.
- [ ] **C7 Host services for native calls.** Route `rfd` (11 sites), `xdg-open` (2) and
  `pdftoppm` (`screens::pdf_pages_png`) through a `Services` struct on `App`; the bin supplies
  rfd/xdg-open/pdftoppm implementations.
- [ ] **C8 Draw inside a given rect.** `screens::root_ui(ctx)` → `root_ui(ui: &mut Ui)` (child
  over `ui.max_rect()`); `shell_frame`, `draw_sidebar_into` and the 12 call sites follow;
  replace `viewport_rect()`/`content_rect()` width tests (screens.rs:555, figuresui.rs:745, 948,
  1065, importui.rs:601, bankui.rs:2185, wysiwyg.rs:3572, 3657) with the tab rect stored on
  `App` each frame (`self.area`).
- [ ] **C9 Viewport switch.** `App.allow_viewports: bool` decides the Pop-over path
  (`bankui.rs:2645-2660`); `send_viewport_cmd_to(ROOT, Focus)` only when true.
- [ ] **C10 Break core→UI edges.** `store.rs:58` (`screens::civil_from_days`), `export.rs:476,
  767, 2190` (`equation::stem_to_plain`, `heading_scale`), `figfit.rs:64-67` and
  `figures/math.rs:4064, 4119` (`figuresui::params_from_spec`, `spec_for`, `flatten_white`).
- [ ] **C11 Split `ai.rs`.** Blocking provider code and key IO (path injected) in core;
  `Ai`/`send_image` with `egui::Context` repaint in ui-egui.
- [ ] **C12 Dependency hygiene.** `sha2` 0.10 → 0.11; equaSHion path deps → git
  `rev = "3d3b840c65d7b6e0e7933c6cd95e171384fdfb54"` (keep a git-ignored
  `.cargo/config.toml` `[patch]` for local hacking, like the shell's
  `scripts/use-local-siblings.sh`); `eframe` only in the bin.
- [ ] **C13 Instance state.** Move `importui.rs:135` `ST`/`MST` and `qedit.rs:1501` `EQ_PANEL`
  into `App` (they are UI state); `equation.rs:315` `CACHE` and the `punnett.rs` thread_locals
  can stay (pure caches) but get a size cap.
- [ ] **C14 Unique ids.** Prefix generic egui ids (`"sidebar"`, `"toasts"`, `"__tp_ui"`) with
  `qs_`.

### Done-when for the checklist
`cd / && vershion` starts with the house fonts; `VERSHION_SELFTEST=1` passes with no env other
than `XDG_DATA_HOME`; `grep -rn 'env::var' crates/` returns only `StoreDir::from_env` and
`#[cfg(test)]` code; `grep -rn 'rfd::\|process::Command\|process::exit' crates/` is empty; the
hosted example survives font swaps; workspace tests green.
