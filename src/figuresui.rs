//! Figure engine UI: the standalone Figures screen, the shared browser + parameter workspace,
//! the "Insert figure" modal used by the question editor, "Save to question", and the
//! hybrid SVG/PNG import. Rendering and parameter schema live in `figures`; this file only
//! builds controls from `ParamDef`s and shows `figures::to_png` output.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use egui::{
    Align, Button, ComboBox, CornerRadius, Frame, Id as UiId, Layout, Margin, Modal, RichText,
    ScrollArea, Sense, Stroke, TextEdit, TextureHandle, Ui, Vec2,
};

use crate::figures::{self, FigureDef, Kind, ParamDef, Params, Subject, Value};
use crate::model::{FigureSpec, Id, Question};
use crate::theme;

mod composite_canvas;

const DEBOUNCE: Duration = Duration::from_millis(150);
const CARD_W: f32 = 210.0;

// ---------- registry (cached) ----------

fn reg() -> &'static [FigureDef] {
    static R: OnceLock<Vec<FigureDef>> = OnceLock::new();
    R.get_or_init(|| {
        #[allow(unused_mut)]
        let mut v = figures::registry();
        #[cfg(test)]
        v.extend(demo_defs());
        #[cfg(debug_assertions)]
        if std::env::var("QUESHION_DEMO_FIGURES").as_deref() == Ok("1") {
            v.extend(demo_defs());
        }
        v
    })
}

/// Tiny demo figure so the UI can be exercised while subject modules are empty. Only ever
/// registered under `cfg(test)` or a debug build with `QUESHION_DEMO_FIGURES=1`.
#[cfg(any(test, debug_assertions))]
fn demo_defs() -> Vec<FigureDef> {
    fn params() -> Vec<ParamDef> {
        vec![
            ParamDef::float("w", "Width", 20.0, 300.0, 1.0, 160.0),
            ParamDef::int("n", "Count", 1, 6, 3),
            ParamDef::bool("fill", "Filled", true),
            ParamDef::choice("shape", "Shape", &["rect", "circle"], "rect"),
            ParamDef::text("label", "Label", "demo"),
        ]
    }
    fn render(p: &Params) -> String {
        let mut d = figures::svg::Doc::new(400.0, 260.0);
        let fill = if p.b("fill") { "#9f5037" } else { "none" };
        for i in 0..p.i("n") {
            let x = 20.0 + i as f64 * 30.0;
            if p.s("shape") == "circle" {
                d.circle(x + 20.0, 100.0, p.f("w") / 8.0, fill, "#332a24", 2.0);
            } else {
                d.rect(x, 40.0, p.f("w") / 4.0, 80.0, fill, "#332a24", 2.0);
            }
        }
        d.text(200.0, 200.0, 18.0, "middle", p.s("label"));
        d.finish()
    }
    vec![FigureDef {
        id: "demo-shapes",
        subject: Subject::Math,
        title: "Demo shapes",
        blurb: "Test-only demo figure.",
        params,
        render,
    }]
}

fn find_def(id: &str) -> Option<(usize, &'static FigureDef)> {
    reg().iter().enumerate().find(|(_, d)| d.id == id)
}

// ---------- params <-> json / hashing ----------

fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::F(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::I(i) => serde_json::Value::from(*i),
        Value::B(b) => serde_json::Value::Bool(*b),
        Value::S(s) => serde_json::Value::String(s.clone()),
    }
}

/// Reads a stored JSON value using the def's default for the expected type.
fn value_from_json(def: &ParamDef, j: &serde_json::Value) -> Option<Value> {
    match &def.default {
        Value::F(_) => j.as_f64().map(Value::F),
        Value::I(_) => j.as_i64().or_else(|| j.as_f64().map(|f| f.round() as i64)).map(Value::I),
        Value::B(_) => j.as_bool().map(Value::B),
        Value::S(_) => j.as_str().map(|s| Value::S(s.to_string())),
    }
}

pub fn spec_for(id: &str, p: &Params) -> FigureSpec {
    FigureSpec {
        id: id.to_string(),
        params: p.0.iter().map(|(k, v)| (k.clone(), value_to_json(v))).collect::<BTreeMap<_, _>>(),
    }
}

/// Defaults overlaid with whatever the spec stored (unknown keys / wrong types are ignored).
pub fn params_from_spec(def: &FigureDef, spec: &FigureSpec) -> Params {
    let defs = (def.params)();
    let mut p = Params::defaults(&defs);
    for d in &defs {
        if let Some(v) = spec.params.get(d.key).and_then(|j| value_from_json(d, j)) {
            p.0.insert(d.key.to_string(), v);
        }
    }
    figures::migrate(def.id, spec, &mut p);
    p.normalize(&defs, false);
    p
}

fn hash_params(id: &str, p: &Params) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut h);
    format!("{:?}", p.0).hash(&mut h);
    h.finish()
}

// ---------- raster helpers ----------

/// Composites over white and re-encodes as PNG (exports turn transparency black otherwise).
pub(crate) fn flatten_white(png: &[u8]) -> Result<Vec<u8>, String> {
    let mut img = image::load_from_memory(png).map_err(|e| e.to_string())?.to_rgba8();
    for px in img.pixels_mut() {
        let a = px[3] as u32;
        for c in 0..3 {
            px[c] = ((px[c] as u32 * a + 255 * (255 - a)) / 255) as u8;
        }
        px[3] = 255;
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

fn color_image(png: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [img.width().max(1) as usize, img.height().max(1) as usize],
        &img,
    ))
}

fn texture(ctx: &egui::Context, name: String, png: &[u8]) -> Option<TextureHandle> {
    Some(ctx.load_texture(name, color_image(png)?, egui::TextureOptions::LINEAR))
}

thread_local! {
    /// Thumbnail textures keyed by figure id + params hash (defaults only, today).
    static THUMBS: RefCell<HashMap<String, Option<TextureHandle>>> = RefCell::new(HashMap::new());
}

fn thumb(ctx: &egui::Context, def: &FigureDef, budget: &mut u8) -> Option<TextureHandle> {
    let p = Params::defaults(&(def.params)());
    let key = format!("{}:{:x}", def.id, hash_params(def.id, &p));
    if let Some(hit) = THUMBS.with(|t| t.borrow().get(&key).cloned()) {
        return hit;
    }
    if *budget == 0 {
        ctx.request_repaint(); // render a few per frame so the grid never stalls
        return None;
    }
    *budget -= 1;
    let tex = figures::to_png(&(def.render)(&p), 0.6)
        .ok()
        .and_then(|png| texture(ctx, format!("testparrot:thumb:{key}"), &png));
    THUMBS.with(|t| t.borrow_mut().insert(key, tex.clone()));
    tex
}

// ---------- workspace ----------

/// What the user asked for; the host (screen or modal) performs it.
pub enum Act {
    None,
    Close,
    /// Attach into the question being edited.
    Insert(Output),
    /// Pick a bank/question to attach to.
    Save(Output),
    Toast(String, bool),
}

/// A finished figure: 2x PNG (flattened on white), optional SVG, and the spec if generated.
pub struct Output {
    pub png: Vec<u8>,
    pub svg: Option<String>,
    pub spec: Option<FigureSpec>,
}

struct Imported {
    svg: Option<String>,
    png: Vec<u8>,
    tex: TextureHandle,
    name: String,
}

#[derive(Default)]
pub struct Workspace {
    pub modal: bool,
    subject: Option<Subject>,
    search: String,
    sel: Option<usize>,
    params: Params,
    imp: Option<Imported>,
    import_open: bool,
    import_text: String,
    import_err: Option<String>,
    /// Debounce bookkeeping: hash of the params last seen changing, when, and what is shown.
    pending: Option<(u64, Instant)>,
    shown: Option<(u64, TextureHandle)>,
    err: Option<String>,
    /// Canvas size of the shown SVG in its own units (maps pointer <-> figure coordinates).
    canvas: (f64, f64),
    /// "Handles" chip off = plain preview.
    hide_handles: bool,
    /// A handle is being dragged: render without waiting for the debounce.
    live: bool,
    /// Inline label edit: (param key, buffer, just-opened).
    editing: Option<(&'static str, String, bool)>,
    qa_init: bool,
    /// Undo/redo of parameter edits: settled snapshots, plus the edit currently in flight.
    undo: Vec<Params>,
    redo: Vec<Params>,
    hist_hash: u64,
    hist_snap: Params,
    hist_base: Option<Params>,
    hist_t: Option<Instant>,
    preset_name: String,
    spec_text: String,
    spec_err: Option<String>,
    /// Canvas editor of the composite-shape figure.
    comp: composite_canvas::Canvas,
}

/// A saved parameter set for one figure ("Save preset"), kept in `figure-presets.json`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PresetEntry {
    name: String,
    params: BTreeMap<String, serde_json::Value>,
}
type PresetFile = BTreeMap<String, Vec<PresetEntry>>;

fn load_presets() -> PresetFile {
    crate::store::load_json("figure-presets.json").unwrap_or_default()
}

fn save_presets(f: &PresetFile) {
    let _ = crate::store::save_json("figure-presets.json", f);
}

impl Workspace {
    pub fn new(modal: bool) -> Self {
        Workspace { modal, ..Default::default() }
    }

    pub fn open_import(modal: bool) -> Self {
        Workspace { modal, import_open: true, ..Default::default() }
    }

    /// Re-opens a stored figure with its saved parameter values.
    pub fn from_spec(spec: &FigureSpec, modal: bool) -> Option<Self> {
        let (i, def) = find_def(&spec.id)?;
        Some(Workspace { modal, sel: Some(i), params: params_from_spec(def, spec), ..Default::default() })
    }

    fn reset_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.hist_hash = 0;
        self.hist_base = None;
        self.hist_t = None;
    }

    /// Records parameter edits as undo steps: a burst of changes (a slider drag, typing)
    /// settles into one step once input pauses and the pointer is up.
    fn track_history(&mut self, ctx: &egui::Context) {
        let Some(si) = self.sel else { return };
        let h = hash_params(reg()[si].id, &self.params);
        if self.hist_hash == 0 {
            self.hist_hash = h;
            self.hist_snap = self.params.clone();
            return;
        }
        if h != self.hist_hash {
            if self.hist_base.is_none() {
                self.hist_base = Some(self.hist_snap.clone());
            }
            self.hist_hash = h;
            self.hist_snap = self.params.clone();
            self.hist_t = Some(Instant::now());
            ctx.request_repaint_after(Duration::from_millis(450));
        } else if self.hist_base.is_some()
            && self.hist_t.is_some_and(|t| t.elapsed() > Duration::from_millis(400))
            && !ctx.input(|i| i.pointer.any_down())
        {
            self.flush_history();
        } else if self.hist_base.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn flush_history(&mut self) {
        if let Some(base) = self.hist_base.take() {
            self.undo.push(base);
            self.redo.clear();
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
    }

    fn set_params_with_history(&mut self, p: Params) {
        self.flush_history();
        self.hist_base = Some(self.params.clone());
        self.hist_t = Some(Instant::now() - Duration::from_secs(1));
        self.params = p;
        self.flush_history();
        if let Some(si) = self.sel {
            self.hist_hash = hash_params(reg()[si].id, &self.params);
            self.hist_snap = self.params.clone();
        }
    }

    fn step_history(&mut self, back: bool) {
        self.flush_history();
        let next = if back { self.undo.pop() } else { self.redo.pop() };
        let Some(p) = next else { return };
        let cur = std::mem::replace(&mut self.params, p);
        if back { self.redo.push(cur) } else { self.undo.push(cur) }
        if let Some(si) = self.sel {
            self.hist_hash = hash_params(reg()[si].id, &self.params);
            self.hist_snap = self.params.clone();
        }
    }

    /// Spec text others can paste back in: {"queshion":"figure","id":..,"params":{..}}.
    fn spec_json(&self) -> Option<String> {
        let def = &reg()[self.sel?];
        let spec = spec_for(def.id, &self.params);
        serde_json::to_string(&serde_json::json!({"queshion": "figure", "id": spec.id, "params": spec.params})).ok()
    }

    fn apply_spec_text(&mut self, text: &str) -> Result<(), String> {
        let v: serde_json::Value = serde_json::from_str(text.trim()).map_err(|e| format!("not valid JSON ({e})"))?;
        let id = v.get("id").and_then(|x| x.as_str()).ok_or("no \"id\" in the pasted spec")?;
        let (i, def) = find_def(id).ok_or_else(|| format!("unknown figure \"{id}\""))?;
        let spec = FigureSpec {
            id: id.to_string(),
            params: v.get("params").and_then(|p| p.as_object()).map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default(),
        };
        let p = params_from_spec(def, &spec);
        if self.sel != Some(i) {
            self.select(i);
        }
        self.set_params_with_history(p);
        Ok(())
    }

    fn select(&mut self, i: usize) {
        self.reset_history();
        self.sel = Some(i);
        self.params = Params::defaults(&(reg()[i].params)());
        self.comp = Default::default();
        self.editing = None;
        self.shown = None;
        self.pending = None;
        self.err = None;
    }

    fn back(&mut self) {
        self.reset_history();
        self.sel = None;
        self.imp = None;
        self.shown = None;
        self.pending = None;
        self.err = None;
    }

    /// Final artifact (2x PNG) for attaching / saving.
    fn output(&self) -> Result<Output, String> {
        if let Some(imp) = &self.imp {
            return Ok(Output { png: imp.png.clone(), svg: imp.svg.clone(), spec: None });
        }
        let def = &reg()[self.sel.ok_or("no figure selected")?];
        let svg = (def.render)(&self.params);
        let png = flatten_white(&figures::to_png(&svg, 2.0)?)?;
        Ok(Output { png, svg: Some(svg), spec: Some(spec_for(def.id, &self.params)) })
    }

    pub fn show(&mut self, ui: &mut Ui) -> Act {
        if !self.qa_init {
            self.qa_init = true;
            // QA hook: QUESHION_FIG=<figure id> opens that figure straight away.
            if let Ok(id) = std::env::var("QUESHION_FIG")
                && let Some((i, _)) = find_def(&id)
            {
                self.select(i);
                // QUESHION_FIG_PARAMS='{"spec":"rect 0 0 6 4"}' seeds its params (as QUESHION_RENDER_FIG does)
                if let Ok(j) = std::env::var("QUESHION_FIG_PARAMS")
                    && let Ok(over) = serde_json::from_str::<serde_json::Value>(&j)
                {
                    self.params = figures::params_from_json(&reg()[i], &over);
                    self.reset_history();
                }
            }
        }
        if self.sel.is_some() || self.imp.is_some() {
            self.workspace_view(ui)
        } else {
            self.browser_view(ui)
        }
    }

    // ----- browser -----

    fn browser_view(&mut self, ui: &mut Ui) -> Act {
        let ctx = ui.ctx().clone();
        ui.horizontal_wrapped(|ui| {
            if tab(ui, "All", self.subject.is_none()) {
                self.subject = None;
            }
            for s in Subject::ALL {
                if tab(ui, s.name(), self.subject == Some(s)) {
                    self.subject = Some(s);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Import SVG/PNG…").clicked() {
                    self.import_open = !self.import_open;
                }
                ui.add(
                    TextEdit::singleline(&mut self.search)
                        .hint_text("Search figures…")
                        .desired_width(180.0),
                );
            });
        });
        ui.add_space(8.0);

        if self.import_open {
            self.import_panel(ui);
            ui.add_space(8.0);
        }

        let q = self.search.trim().to_lowercase();
        let list: Vec<(usize, &FigureDef)> = reg()
            .iter()
            .enumerate()
            .filter(|(_, d)| self.subject.is_none_or(|s| d.subject == s))
            .filter(|(_, d)| {
                q.is_empty()
                    || d.title.to_lowercase().contains(&q)
                    || d.blurb.to_lowercase().contains(&q)
                    || d.id.contains(&q)
            })
            .collect();

        let max_h = if self.modal {
            (ctx.viewport_rect().height() * 0.55).clamp(220.0, 520.0)
        } else {
            ui.available_height().max(200.0)
        };
        let mut picked = None;
        ScrollArea::vertical()
            .id_salt("figs_browser")
            .max_height(max_h)
            .auto_shrink([!self.modal, true])
            .show(ui, |ui| {
                if list.is_empty() {
                    ui.add_space(20.0);
                    ui.label(theme::muted("No figures match.").size(13.0));
                    return;
                }
                let avail = ui.available_width() - 18.0; // vertical scrollbar allowance
                let cols = ((avail + 12.0) / (CARD_W + 12.0)).floor().max(1.0) as usize;
                let mut budget = 10u8;
                // Cards flex to fill the row (no blank strip at wide sizes).
                let mut cw = ((avail - 12.0 * (cols as f32 - 1.0)) / cols as f32).max(CARD_W);
                if self.modal {
                    cw = cw.min(CARD_W + 60.0);
                }
                for chunk in list.chunks(cols) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        for (i, def) in chunk {
                            if card(ui, &ctx, def, *i, &mut budget, cw) {
                                picked = Some(*i);
                            }
                        }
                    });
                    ui.add_space(12.0);
                }
            });
        if let Some(i) = picked {
            self.select(i);
        }
        if self.modal {
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ghost(ui, "Cancel").clicked() {
                    return Act::Close;
                }
                Act::None
            })
            .inner
        } else {
            Act::None
        }
    }

    fn import_panel(&mut self, ui: &mut Ui) {
        Frame::new()
            .fill(theme::card())
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(crate::kit::R_CARD))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.label(theme::strong("Import your own figure").size(13.0));
                ui.label(
                    theme::muted("Pick an SVG or PNG/JPG file, or paste SVG text. It is attached as a picture; no parameters.")
                        .size(12.0),
                );
                ui.add_space(6.0);
                if ghost(ui, "Choose file…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("SVG / images", &["svg", "png", "jpg", "jpeg", "gif", "webp", "bmp"])
                        .pick_file()
                {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    match std::fs::read(&path) {
                        Ok(bytes) => self.import_bytes(ui.ctx(), &bytes, name),
                        Err(e) => self.import_err = Some(format!("Couldn't read the file: {e}")),
                    }
                }
                ui.add_space(4.0);
                ui.add(
                    TextEdit::multiline(&mut self.import_text)
                        .hint_text("…or paste <svg>…</svg> here")
                        .desired_width(ui.available_width())
                        .desired_rows(4)
                        .code_editor(),
                );
                ui.horizontal(|ui| {
                    let has = !self.import_text.trim().is_empty();
                    if ui
                        .add_enabled(
                            has,
                            Button::new(theme::on_accent("Use pasted SVG"))
                                .fill(theme::accent_col())
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        let t = self.import_text.clone();
                        self.import_bytes(ui.ctx(), t.as_bytes(), "pasted.svg".to_string());
                    }
                    if let Some(e) = &self.import_err {
                        ui.label(RichText::new(e).color(theme::red()).size(12.0));
                    }
                });
            });
    }

    fn import_bytes(&mut self, ctx: &egui::Context, bytes: &[u8], name: String) {
        match import_any(bytes) {
            Ok((svg, png)) => match texture(ctx, format!("testparrot:import:{name}"), &png) {
                Some(tex) => {
                    self.imp = Some(Imported { svg, png, tex, name });
                    self.import_err = None;
                    self.import_open = false;
                    self.sel = None;
                }
                None => self.import_err = Some("Couldn't decode the rendered image.".into()),
            },
            Err(e) => self.import_err = Some(e),
        }
    }

    // ----- workspace -----

    fn workspace_view(&mut self, ui: &mut Ui) -> Act {
        let ctx = ui.ctx().clone();
        let mut act = Act::None;

        ui.horizontal(|ui| {
            if ghost(ui, "← Figures").clicked() {
                self.back();
            }
            let title = match (&self.imp, self.sel) {
                (Some(i), _) => format!("Imported: {}", i.name),
                (None, Some(s)) => reg()[s].title.to_string(),
                _ => String::new(),
            };
            ui.label(theme::serif_strong(title).size(19.0));
        });
        ui.add_space(8.0);

        if self.imp.is_none() && self.sel.is_some() {
            if !ctx.egui_wants_keyboard_input() {
                use egui::{Key, Modifiers};
                if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))
                    || ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Y))
                {
                    self.step_history(false);
                } else if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
                    self.step_history(true);
                }
            }
            self.track_history(&ctx);
        }

        // Resolve the preview texture (debounced re-render for generated figures).
        let mut preview: Option<TextureHandle> = self.imp.as_ref().map(|i| i.tex.clone());
        if self.imp.is_none()
            && let Some(si) = self.sel
        {
            let def = &reg()[si];
            let h = hash_params(def.id, &self.params);
            let stale = self.shown.as_ref().is_none_or(|(sh, _)| *sh != h);
            if stale {
                let first = self.shown.is_none();
                let t0 = match self.pending {
                    Some((ph, t)) if ph == h => t,
                    _ => {
                        let now = Instant::now();
                        self.pending = Some((h, now));
                        now
                    }
                };
                if first || self.live || t0.elapsed() >= DEBOUNCE {
                    let svg = (def.render)(&self.params);
                    self.canvas = figures::svg_size(&svg);
                    match figures::to_png(&svg, 1.5).map_err(|e| e.to_string()).and_then(|p| {
                        texture(&ctx, format!("testparrot:fig:{}", def.id), &p).ok_or("decode failed".to_string())
                    }) {
                        Ok(tex) => {
                            self.shown = Some((h, tex));
                            self.err = None;
                        }
                        Err(e) => {
                            self.err = Some(e);
                            self.shown = None;
                        }
                    }
                    self.pending = None;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(60));
                }
            }
            preview = self.shown.as_ref().map(|(_, t)| t.clone());
        }

        let generated = self.imp.is_none();
        let avail_h = if self.modal {
            (ctx.viewport_rect().height() * 0.55).clamp(260.0, 520.0)
        } else {
            (ui.available_height() - 64.0).max(240.0)
        };
        ui.horizontal_top(|ui| {
            if generated {
                ui.allocate_ui(Vec2::new(290.0, avail_h), |ui| {
                    ui.set_width(290.0);
                    self.controls(ui, avail_h);
                });
                ui.add_space(12.0);
            }
            let w = ui.available_width();
            ui.allocate_ui(Vec2::new(w, avail_h), |ui| {
                Frame::new()
                    .fill(egui::Color32::WHITE)
                    .stroke(Stroke::new(1.0, theme::border()))
                    .corner_radius(CornerRadius::same(crate::kit::R_CARD))
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width((w - 22.0).max(100.0));
                        ui.set_height((avail_h - 22.0).max(100.0));
                        if generated && !self.hide_handles && self.sel.is_some_and(|si| reg()[si].id == "composite-shape") {
                            self.composite_canvas(ui);
                        } else if generated && let Some(t) = &preview {
                            self.preview_interactive(ui, t);
                        } else if let Some(t) = &preview {
                            ui.centered_and_justified(|ui| {
                                ui.add(egui::Image::new(t).shrink_to_fit());
                            });
                        } else {
                            ui.centered_and_justified(|ui| {
                                if let Some(e) = &self.err {
                                    ui.label(RichText::new(e).color(theme::red()).size(12.0));
                                } else {
                                    ui.spinner();
                                }
                            });
                        }
                    });
            });
        });
        ui.add_space(10.0);

        // Action row.
        let have_svg = self.imp.as_ref().map(|i| i.svg.is_some()).unwrap_or(true);
        ui.horizontal_wrapped(|ui| {
            if self.modal {
                if accent_btn(ui, if self.imp.is_some() { "Attach picture" } else { "Insert figure" }).clicked() {
                    act = match self.output() {
                        Ok(o) => Act::Insert(o),
                        Err(e) => Act::Toast(e, true),
                    };
                }
            } else if accent_btn(ui, "Save to question…").clicked() {
                act = match self.output() {
                    Ok(o) => Act::Save(o),
                    Err(e) => Act::Toast(e, true),
                };
            }
            if ghost(ui, "Export PNG").clicked() {
                act = self.export(false);
            }
            if have_svg && ghost(ui, "Export SVG").clicked() {
                act = self.export(true);
            }
            if ghost(ui, "Copy PNG").clicked() {
                act = match self.output().ok().and_then(|o| color_image(&o.png)) {
                    Some(ci) => {
                        ctx.copy_image(ci);
                        Act::Toast("Copied PNG to the clipboard".into(), false)
                    }
                    None => Act::Toast("Couldn't render the figure".into(), true),
                };
            }
            if generated && self.sel.is_some() {
                self.history_and_presets_ui(ui);
            }
            if generated
                && let Some(ls) = self.sel.and_then(|si| (reg()[si].params)().into_iter().find(|d| d.key == "label_size"))
                && matches!(&ls.kind, Kind::Choice(o) if o.len() == 3)
            {
                let cur = self.params.s("label_size").to_string();
                for (name, px) in [("Large", 16.0), ("Medium", 13.0), ("Small", 10.0)] {
                    let sel = cur == name;
                    let b = Button::new(RichText::new("A").size(px).color(if sel { theme::accent_col() } else { theme::muted_col() }))
                        .fill(if sel { theme::accent_faded() } else { egui::Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, if sel { theme::accent_col() } else { theme::border() }));
                    if ui.add(b).on_hover_text(format!("{name} labels")).clicked() {
                        self.params.0.insert("label_size".to_string(), Value::S(name.to_string()));
                    }
                }
            }
            if generated && chip_handles(ui, !self.hide_handles) {
                self.hide_handles = !self.hide_handles;
            }
            if self.modal && ghost(ui, "Cancel").clicked() {
                act = Act::Close;
            }
        });
        act
    }

    /// Undo / Redo, "Examples" and "Presets" (saved parameter sets + copy/paste spec).
    fn history_and_presets_ui(&mut self, ui: &mut Ui) {
        let Some(si) = self.sel else { return };
        let def = &reg()[si];
        let can_undo = !self.undo.is_empty() || self.hist_base.is_some();
        let can_redo = !self.redo.is_empty();
        if ui.add_enabled(can_undo, Button::new(RichText::new("Undo").size(13.0))).on_hover_text("Undo the last change (Ctrl+Z)").clicked() {
            self.step_history(true);
        }
        if ui.add_enabled(can_redo, Button::new(RichText::new("Redo").size(13.0))).on_hover_text("Redo (Ctrl+Shift+Z)").clicked() {
            self.step_history(false);
        }
        ui.menu_button("Examples", |ui| {
            let ex = figures::examples_for(def.id);
            if ex.is_empty() {
                ui.label(theme::muted("No examples for this figure yet."));
            }
            for e in ex {
                if ui.button(e.name).clicked() {
                    let mut p = Params::defaults(&(def.params)());
                    for (k, v) in &e.params {
                        p.0.insert((*k).to_string(), v.clone());
                    }
                    p.normalize(&(def.params)(), false);
                    self.set_params_with_history(p);
                    ui.close();
                }
            }
        });
        ui.menu_button("Presets", |ui| {
            ui.set_min_width(230.0);
            let mut file = load_presets();
            let mut changed = false;
            let mut apply: Option<Params> = None;
            let list = file.entry(def.id.to_string()).or_default();
            if list.is_empty() {
                ui.label(theme::muted("No saved presets for this figure."));
            }
            let mut remove: Option<usize> = None;
            for (i, pr) in list.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button(&pr.name).clicked() {
                        let spec = FigureSpec { id: def.id.to_string(), params: pr.params.clone() };
                        apply = Some(params_from_spec(def, &spec));
                    }
                    if ui.add(Button::new(theme::muted("Delete").size(11.0)).frame(false)).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                list.remove(i);
                changed = true;
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut self.preset_name).hint_text("Name this preset").desired_width(130.0));
                if ui.button("Save preset").clicked() && !self.preset_name.trim().is_empty() {
                    let spec = spec_for(def.id, &self.params);
                    let name = self.preset_name.trim().to_string();
                    list.retain(|p| p.name != name);
                    list.push(PresetEntry { name, params: spec.params });
                    self.preset_name.clear();
                    changed = true;
                }
            });
            ui.label(theme::muted("Presets stay on this computer.").size(11.0));
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Copy spec").on_hover_text("Copy this figure's settings as text to share or paste back later").clicked() {
                    if let Some(t) = self.spec_json() {
                        ui.ctx().copy_text(t);
                    }
                }
            });
            ui.add(TextEdit::singleline(&mut self.spec_text).hint_text("Paste a copied spec here").desired_width(210.0));
            if ui.button("Apply pasted spec").clicked() {
                let t = self.spec_text.clone();
                match self.apply_spec_text(&t) {
                    Ok(()) => {
                        self.spec_text.clear();
                        self.spec_err = None;
                        ui.close();
                    }
                    Err(e) => self.spec_err = Some(e),
                }
            }
            if let Some(e) = &self.spec_err {
                ui.label(RichText::new(e).color(theme::red()).size(11.0));
            }
            if changed {
                save_presets(&file);
            }
            if let Some(p) = apply {
                self.set_params_with_history(p);
                ui.close();
            }
        });
    }

    /// The composite-shape canvas editor; a click-sized edit (add, delete, shade...) becomes its
    /// own undo step, drags and typing settle through `track_history` like any other edit.
    fn composite_canvas(&mut self, ui: &mut Ui) {
        let before = self.params.clone();
        // the preview frame lays out left to right; the canvas stacks its toolbar over the grid
        let ch = ui.vertical(|ui| self.comp.show(ui, &mut self.params)).inner;
        if let Some(si) = self.sel {
            self.params.normalize(&(reg()[si].params)(), false);
        }
        match ch {
            composite_canvas::Change::Step if format!("{:?}", before.0) != format!("{:?}", self.params.0) => {
                let after = std::mem::replace(&mut self.params, before);
                self.set_params_with_history(after);
            }
            composite_canvas::Change::Live => self.live = true,
            _ => {}
        }
    }

    /// The preview with its interaction layer: draws the texture fitted into the frame, then
    /// the figure's handles; pointer input is mapped back into `self.params`.
    fn preview_interactive(&mut self, ui: &mut Ui, tex: &TextureHandle) {
        use figures::interact::{Cursor, Shape};
        let Some(si) = self.sel else { return };
        let def = &reg()[si];
        let space = ui.available_size();
        let (full, _) = ui.allocate_exact_size(space, Sense::hover());
        let area = egui::Rect::from_min_max(full.min, egui::pos2(full.max.x, full.max.y - 16.0));
        let nat = tex.size_vec2();
        let fit = (area.width() / nat.x).min(area.height() / nat.y).min(1.0).max(0.05);
        let img = egui::Rect::from_center_size(area.center(), nat * fit);
        egui::Image::new(tex).fit_to_exact_size(img.size()).paint_at(ui, img);
        let (cw, ch) = if self.canvas.0 > 1.0 { self.canvas } else { ((nat.x / 1.5) as f64, (nat.y / 1.5) as f64) };
        let k = img.width() as f64 / cw; // screen points per SVG unit
        let to_screen = |p: (f64, f64)| egui::pos2(img.left() + (p.0 * k) as f32, img.top() + (p.1 * k) as f32);
        let to_svg = |p: egui::Pos2| ((p.x - img.left()) as f64 / k, (p.y - img.top()) as f64 / k);
        let _ = ch;
        let pdefs = (def.params)();

        self.live = false;
        if self.hide_handles {
            return;
        }
        let mut hs = def.handles(&self.params, (cw, ch));
        // Zones first so knobs on top win the hit test.
        hs.sort_by_key(|h| h.shape != Shape::Zone);
        let painter = ui.painter_at(full.expand(8.0));
        let acc = theme::accent_col();
        let mut changed = false;
        let mut start_edit: Option<(&'static str, String)> = None;
        for (i, h) in hs.iter().enumerate() {
            let c = to_screen(h.pos);
            let hx = (h.half.0 * k).max(9.0) as f32;
            let hy = (h.half.1 * k).max(9.0) as f32;
            let hit = egui::Rect::from_center_size(c, Vec2::new(hx * 2.0, hy * 2.0));
            let sense = if h.drag.is_some() { Sense::click_and_drag() } else { Sense::click() };
            let r = ui.interact(hit, UiId::new(("fig_handle", si, i)), sense);
            let active = r.dragged();
            if r.hovered() || active {
                ui.ctx().set_cursor_icon(if active {
                    egui::CursorIcon::Grabbing
                } else {
                    match h.cursor {
                        Cursor::Grab => egui::CursorIcon::Grab,
                        Cursor::Move => egui::CursorIcon::Move,
                        Cursor::ResizeH => egui::CursorIcon::ResizeHorizontal,
                        Cursor::ResizeV => egui::CursorIcon::ResizeVertical,
                        Cursor::Pointer => egui::CursorIcon::PointingHand,
                        Cursor::Crosshair => egui::CursorIcon::Crosshair,
                    }
                });
            }
            if r.drag_started()
                && let Some(ptr) = r.interact_pointer_pos()
            {
                let t = to_svg(ptr);
                ui.data_mut(|d| d.insert_temp(UiId::new("fig_drag_off"), (t.0 - h.pos.0, t.1 - h.pos.1)));
            }
            if active
                && let (Some(f), Some(ptr)) = (&h.drag, r.interact_pointer_pos())
            {
                let off: (f64, f64) = ui.data(|d| d.get_temp(UiId::new("fig_drag_off"))).unwrap_or((0.0, 0.0));
                let t = to_svg(ptr);
                f(&mut self.params, (t.0 - off.0, t.1 - off.1));
                self.params.normalize(&pdefs, true);
                self.live = true;
                changed = true;
            }
            if r.double_clicked()
                && let Some(key) = h.edit
            {
                start_edit = Some((key, self.params.s(key).to_string()));
            } else if r.clicked()
                && let Some(f) = &h.click
            {
                f(&mut self.params);
                self.params.normalize(&pdefs, true);
                changed = true;
            }
            if r.hovered()
                && let Some(f) = &h.scroll
            {
                let dy = ui.input(|i| i.smooth_scroll_delta.y);
                if dy.abs() > 0.5 {
                    f(&mut self.params, (dy / 40.0) as f64);
                    self.params.normalize(&pdefs, true);
                    changed = true;
                }
            }
            let hot = r.hovered() || active;
            match h.shape {
                Shape::Zone => {
                    if hot {
                        painter.rect(hit, 4.0, theme::accent_faded().gamma_multiply(0.6), Stroke::new(1.5, acc), egui::StrokeKind::Inside);
                    }
                }
                sh => {
                    let rad = if hot { 7.5 } else { 5.5 };
                    let (fill, stroke) = if hot {
                        (acc, Stroke::new(2.0, egui::Color32::WHITE))
                    } else {
                        (egui::Color32::WHITE, Stroke::new(1.8, acc))
                    };
                    match sh {
                        Shape::Circle => {
                            painter.circle(c, rad, fill, stroke);
                        }
                        Shape::Square => {
                            painter.rect(egui::Rect::from_center_size(c, Vec2::splat(rad * 1.7)), 2.0, fill, stroke, egui::StrokeKind::Middle);
                        }
                        _ => {
                            let d = rad * 1.3;
                            let pts = vec![c + Vec2::new(0.0, -d), c + Vec2::new(d, 0.0), c + Vec2::new(0.0, d), c + Vec2::new(-d, 0.0)];
                            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
                        }
                    }
                }
            }
            if r.hovered() && !active && !h.tip.is_empty() {
                r.on_hover_text(&h.tip);
            }
        }
        if changed {
            ui.ctx().request_repaint();
        }
        if start_edit.is_some() {
            self.editing = start_edit.map(|(k, v)| (k, v, true));
        }
        if let Some((key, mut buf, fresh)) = self.editing.take() {
            let at = egui::Rect::from_center_size(area.center_top() + Vec2::new(0.0, 22.0), Vec2::new(220.0, 26.0));
            let resp = ui.put(at, TextEdit::singleline(&mut buf).desired_width(220.0).hint_text("Edit label"));
            if fresh {
                resp.request_focus();
            }
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if enter || (resp.lost_focus() && !esc) {
                self.params.set_s(key, &buf);
            } else if !esc {
                self.editing = Some((key, buf, false));
            }
        }
        if !hs.is_empty() {
            painter.text(
                full.left_bottom() + Vec2::new(4.0, -2.0),
                egui::Align2::LEFT_BOTTOM,
                "Drag the dots to edit; double-click a label to rename it",
                egui::FontId::proportional(10.5),
                theme::muted_col(),
            );
        }
    }

    fn controls(&mut self, ui: &mut Ui, max_h: f32) {
        let Some(si) = self.sel else { return };
        let def = &reg()[si];
        let pdefs = (def.params)();
        ui.vertical(|ui| {
            ui.label(theme::muted(def.blurb).size(12.0));
            ui.add_space(6.0);
            ScrollArea::vertical()
                .id_salt("figs_controls")
                .max_height((max_h - 70.0).max(120.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().slider_width = 170.0;
                    let n_now = pdefs.iter().any(|d| d.key == "n").then(|| self.params.i("n"));
                    if let Some(card) = figures::measures_for(def.id) {
                        self.measures_card(ui, def.id, &pdefs, &card);
                    }
                    if let Some(card) = figures::rows_for(def.id) {
                        self.rows_card(ui, def.id, &pdefs, &card);
                    }
                    if let Some(card) = figures::chips_for(def.id) {
                        self.chips_card(ui, &pdefs, &card);
                    }
                    let secs = figures::layout::sections(def.id, &pdefs, n_now);
                    for (si_, sec) in secs.iter().enumerate() {
                        let shown: Vec<usize> = sec.items.iter().copied().filter(|&i| !is_hidden(&pdefs, &self.params, &pdefs[i])).collect();
                        if shown.is_empty() {
                            continue;
                        }
                        let id = ui.make_persistent_id(("fig_sec", def.id, sec.title.as_str()));
                        let summary = section_summary(&pdefs, &self.params, &sec.items);
                        let mut reset = false;
                        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, si_ < 2)
                            .show_header(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.label(theme::strong(sec.title.as_str()).size(13.0));
                                    if !summary.is_empty() {
                                        ui.label(theme::muted(summary).size(11.0));
                                    }
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let r = ui.add(Button::new(theme::muted("Reset").size(11.0)).frame(false));
                                    reset = r.on_hover_text("Reset this section to its defaults").clicked();
                                });
                            })
                            .body(|ui| {
                                if !sec.help.is_empty() {
                                    ui.label(theme::muted(sec.help.as_str()).size(11.0));
                                    ui.add_space(4.0);
                                }
                                for &i in &shown {
                                    param_control(ui, &pdefs[i], &mut self.params);
                                    ui.add_space(6.0);
                                }
                                ui.add_space(4.0);
                            });
                        if reset {
                            for &i in &sec.items {
                                self.params.0.insert(pdefs[i].key.to_string(), pdefs[i].default.clone());
                            }
                        }
                        ui.separator();
                    }
                });
            ui.add_space(4.0);
            if ghost(ui, "Reset all").clicked() {
                self.params = Params::defaults(&pdefs);
            }
        });
    }

    /// The reference's "Measures" card: one row per side/angle/dimension with its value box and a
    /// label popover. A typed value is "given"; an empty box shows the worked-out value in grey.
    fn measures_card(&mut self, ui: &mut Ui, fig: &str, pdefs: &[ParamDef], card: &figures::MeasureCard) {
        let computed: HashMap<&'static str, String> = figures::computed_for(fig, &self.params).into_iter().collect();
        Frame::new()
            .fill(theme::card())
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    crate::kit::small_caps(ui, card.title);
                    if !card.help.is_empty() {
                        ui.label(theme::muted("(?)").size(11.0)).on_hover_text(card.help);
                    }
                });
                for g in &card.groups {
                    if !figures::card_visible(fig, g.title, None, &self.params) {
                        continue;
                    }
                    if !g.title.is_empty() {
                        ui.add_space(6.0);
                        ui.label(theme::strong(g.title).size(11.0));
                    }
                    for r in &g.rows {
                        if !figures::card_visible(fig, g.title, Some(r.value_key), &self.params) {
                            continue;
                        }
                        self.measure_row(ui, pdefs, r, &computed);
                        ui.add_space(2.0);
                    }
                }
                // A solver message ("These measures don't fix the triangle.") from `computed`.
                if let Some(msg) = computed.get("__msg") {
                    ui.add_space(6.0);
                    ui.label(RichText::new(msg.as_str()).color(theme::red()).size(11.5));
                }
            });
        ui.add_space(8.0);
    }

    /// The reference's list cards: one editable row per equation / data set, with Add, clear and
    /// (where the figure has them) a colour per row.
    fn rows_card(&mut self, ui: &mut Ui, fig: &str, pdefs: &[ParamDef], card: &figures::RowsCard) {
        let extra_id = ui.make_persistent_id(("fig_rows_extra", fig));
        let mut extra: usize = ui.data(|d| d.get_temp(extra_id)).unwrap_or(0);
        Frame::new()
            .fill(theme::card())
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    crate::kit::small_caps(ui, card.title);
                    if !card.help.is_empty() {
                        ui.label(theme::muted("(?)").size(11.0)).on_hover_text(card.help);
                    }
                });
                ui.add_space(4.0);
                let palette = |k: &str| pdefs.iter().find(|d| d.key == k).and_then(|d| if let Kind::Color(p) = &d.kind { Some(p.clone()) } else { None });
                match &card.source {
                    figures::RowsSource::Joined { key, sep, color_keys } => {
                        let cur = self.params.s(key).to_string();
                        let mut rows: Vec<String> = cur.split(*sep).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
                        for _ in 0..extra {
                            rows.push(String::new());
                        }
                        if rows.is_empty() {
                            rows.push(String::new());
                        }
                        let mut remove: Option<usize> = None;
                        for (i, row) in rows.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add(TextEdit::singleline(row).hint_text(card.hint).desired_width(170.0));
                                if let Some(ck) = color_keys.get(i)
                                    && let Some(pal) = palette(ck)
                                {
                                    self.row_colour(ui, ck, &pal);
                                }
                                if ui.add(Button::new(theme::muted("×").size(13.0)).frame(false)).on_hover_text("Remove this row").clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                        if let Some(i) = remove {
                            rows.remove(i);
                        }
                        let joined = rows.iter().map(|r| r.trim()).filter(|r| !r.is_empty()).collect::<Vec<_>>().join(&format!("{sep} "));
                        if joined != cur {
                            self.params.0.insert((*key).to_string(), Value::S(joined));
                        }
                        extra = extra.min(rows.iter().filter(|r| r.trim().is_empty()).count());
                        if ui.button(card.add_label).clicked() && color_keys.len().max(8) > rows.len() {
                            extra += 1;
                        }
                    }
                    figures::RowsSource::Slots(slots) => {
                        let mut shown = 0;
                        for (i, sl) in slots.iter().enumerate() {
                            let filled = !self.params.s(sl.value_key).trim().is_empty();
                            if i == 0 || filled || i <= extra {
                                shown = i + 1;
                            }
                        }
                        for sl in slots.iter().take(shown) {
                            ui.horizontal(|ui| {
                                if let Some(nk) = sl.name_key {
                                    let mut n = self.params.s(nk).to_string();
                                    if ui.add(TextEdit::singleline(&mut n).hint_text("name").desired_width(64.0)).changed() {
                                        self.params.0.insert(nk.to_string(), Value::S(n));
                                    }
                                }
                                let mut v = self.params.s(sl.value_key).to_string();
                                if ui.add(TextEdit::singleline(&mut v).hint_text(card.hint).desired_width(if sl.name_key.is_some() { 120.0 } else { 170.0 })).changed() {
                                    self.params.0.insert(sl.value_key.to_string(), Value::S(v));
                                }
                                if let Some(ck) = sl.color_key
                                    && let Some(pal) = palette(ck)
                                {
                                    self.row_colour(ui, ck, &pal);
                                }
                                if ui.add(Button::new(theme::muted("×").size(13.0)).frame(false)).on_hover_text("Clear this row").clicked() {
                                    self.params.0.insert(sl.value_key.to_string(), Value::S(String::new()));
                                }
                            });
                        }
                        if shown < slots.len() && ui.button(card.add_label).clicked() {
                            extra = shown;
                        }
                    }
                }
            });
        ui.data_mut(|d| d.insert_temp(extra_id, extra));
        ui.add_space(8.0);
    }

    /// "Add a force" chips: each appends one preset item and bumps the count.
    fn chips_card(&mut self, ui: &mut Ui, pdefs: &[ParamDef], card: &figures::ChipsCard) {
        let n = self.params.i(card.count_key);
        Frame::new()
            .fill(theme::card())
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    crate::kit::small_caps(ui, card.title);
                    if !card.count_key.is_empty() {
                        ui.label(theme::muted(format!("{n} of {}", card.max)).size(11.0));
                    }
                });
                ui.add_space(4.0);
                let slotted = !card.count_key.is_empty();
                let full = slotted && n >= card.max;
                ui.horizontal_wrapped(|ui| {
                    for chip in &card.chips {
                        if ui.add_enabled(!full, Button::new(RichText::new(format!("+ {}", chip.label)).size(12.0)).corner_radius(CornerRadius::same(10))).clicked() {
                            apply_chip(&mut self.params, pdefs, card, chip);
                        }
                    }
                    if slotted && n > 1 && ui.add(Button::new(theme::muted("Remove the last").size(12.0)).frame(false)).clicked() {
                        self.params.set_i(card.count_key, n - 1);
                    }
                });
            });
        ui.add_space(8.0);
    }

    /// A colour swatch that opens a palette popover for one param.
    fn row_colour(&mut self, ui: &mut Ui, key: &str, pal: &[&'static str]) {
        let cur = self.params.s(key).to_string();
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
        ui.painter().circle_filled(r.center(), 8.0, swatch(&cur));
        ui.painter().circle_stroke(r.center(), 9.5, Stroke::new(1.0, theme::border()));
        egui::Popup::menu(&resp).show(|ui| {
            ui.horizontal_wrapped(|ui| {
                for hex in pal {
                    let (r, rr) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                    ui.painter().circle_filled(r.center(), 9.0, swatch(hex));
                    if cur.eq_ignore_ascii_case(hex) {
                        ui.painter().circle_stroke(r.center(), 11.0, Stroke::new(2.0, theme::accent_col()));
                    }
                    if rr.clicked() {
                        self.params.0.insert(key.to_string(), Value::S((*hex).to_string()));
                    }
                }
            });
        });
    }

    fn measure_row(&mut self, ui: &mut Ui, pdefs: &[ParamDef], r: &figures::MeasureRow, computed: &HashMap<&'static str, String>) {
        let find = |k: &str| pdefs.iter().find(|d| d.key == k);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            // Name: an editable vertex letter, or a fixed label.
            if let Some(nk) = r.name_key {
                let mut v = self.params.s(nk).to_string();
                if ui.add(TextEdit::singleline(&mut v).desired_width(26.0)).changed() {
                    self.params.0.insert(nk.to_string(), Value::S(v.chars().take(3).collect()));
                }
                ui.label(theme::muted(r.name).size(12.0));
            } else {
                // egui's bundled font has no "∠"; spell it out.
                let nm = if r.name.trim() == "∠" { "angle" } else { r.name };
                ui.add_sized([40.0, 20.0], egui::Label::new(RichText::new(nm).size(12.0).italics()));
            }
            // Value.
            let unit_txt: String = if r.unit == "$unit" { self.params.s("unit").to_string() } else { r.unit.to_string() };
            let mut shown = String::new();
            if let Some(eq) = r.equals {
                ui.add_sized([88.0, 20.0], egui::Label::new(theme::muted(format!("= {eq}")).size(12.0)));
                shown = computed.get(r.value_key).cloned().unwrap_or_default();
            } else {
                match find(r.value_key).map(|d| &d.kind) {
                    Some(Kind::Float { min, max, step }) => {
                        let mut v = self.params.f(r.value_key);
                        if ui.add(egui::DragValue::new(&mut v).speed(*step).range(*min..=*max).max_decimals(3)).changed() {
                            self.params.set_f(r.value_key, v);
                        }
                        shown = fmt_measure(v);
                    }
                    Some(Kind::Int { min, max }) => {
                        let mut v = self.params.i(r.value_key);
                        if ui.add(egui::DragValue::new(&mut v).range(*min..=*max)).changed() {
                            self.params.set_i(r.value_key, v);
                        }
                        shown = v.to_string();
                    }
                    _ => {
                        let mut v = self.params.s(r.value_key).to_string();
                        let hint = computed.get(r.value_key).cloned().unwrap_or_default();
                        let was_empty = self.params.s(r.value_key).trim().is_empty();
                        if ui.add(TextEdit::singleline(&mut v).hint_text(hint.clone()).desired_width(88.0)).changed() {
                            self.params.0.insert(r.value_key.to_string(), Value::S(v.clone()));
                            // Like the reference: typing a value shows its label, clearing it hides it.
                            if !r.label_key.is_empty() {
                                let mode = self.params.s(r.label_key).to_string();
                                if was_empty && !v.trim().is_empty() && mode == "None" {
                                    self.params.0.insert(r.label_key.to_string(), Value::S("Measure".into()));
                                } else if v.trim().is_empty() && mode == "Measure" {
                                    self.params.0.insert(r.label_key.to_string(), Value::S("None".into()));
                                }
                            }
                        }
                        shown = if v.trim().is_empty() { hint } else { v };
                    }
                }
                if !unit_txt.is_empty() {
                    ui.label(theme::muted(unit_txt.as_str()).size(11.0));
                }
            }
            // Label button + popover (a row with no label key has none).
            if r.label_key.is_empty() {
                return;
            }
            let mode = self.params.s(r.label_key).to_string();
            let txt = match mode.as_str() {
                "Text" => self.params.s(r.text_key).to_string(),
                "None" => "None".to_string(),
                _ => format!("{shown}{}", if shown.is_empty() { String::new() } else { if unit_txt == "°" || unit_txt == "%" { unit_txt.clone() } else { format!(" {unit_txt}").trim_end().to_string() } }),
            };
            let btn = ui.add(Button::new(RichText::new(if txt.trim().is_empty() { "—".to_string() } else { txt }).size(12.0)).min_size(Vec2::new(78.0, 24.0)));
            egui::Popup::menu(&btn).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_min_width(190.0);
                ui.label(theme::strong(format!("Label at {}", r.name.trim())).size(12.0));
                if let Some(Kind::Choice(opts)) = find(r.label_key).map(|d| &d.kind) {
                    let mut cur = mode.clone();
                    segmented(ui, opts, &mut cur);
                    if cur != mode {
                        self.params.0.insert(r.label_key.to_string(), Value::S(cur));
                    }
                }
                if self.params.s(r.label_key) == "Text" {
                    let mut t = self.params.s(r.text_key).to_string();
                    if ui.add(TextEdit::singleline(&mut t).desired_width(170.0)).changed() {
                        self.params.0.insert(r.text_key.to_string(), Value::S(t));
                    }
                }
                if let Some(tk) = r.tick_key
                    && let Some(Kind::Choice(opts)) = find(tk).map(|d| &d.kind)
                {
                    ui.add_space(4.0);
                    ui.label(theme::strong(find(tk).map(|d| d.label).unwrap_or("Marks")).size(12.0));
                    let before = self.params.s(tk).to_string();
                    let mut cur = before.clone();
                    segmented(ui, opts, &mut cur);
                    if cur != before {
                        self.params.0.insert(tk.to_string(), Value::S(cur));
                    }
                }
            });
        });
    }

    fn export(&self, svg: bool) -> Act {
        let out = match self.output() {
            Ok(o) => o,
            Err(e) => return Act::Toast(e, true),
        };
        let stem = match (&self.imp, self.sel) {
            (Some(_), _) => "figure".to_string(),
            (None, Some(s)) => reg()[s].id.to_string(),
            _ => "figure".to_string(),
        };
        let (ext, label) = if svg { ("svg", "SVG") } else { ("png", "PNG") };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{stem}.{ext}"))
            .add_filter(label, &[ext])
            .save_file()
        else {
            return Act::None;
        };
        let data: Vec<u8> = if svg { out.svg.unwrap_or_default().into_bytes() } else { out.png };
        match std::fs::write(&path, data) {
            Ok(()) => Act::Toast(format!("Exported {}", path.display()), false),
            Err(e) => Act::Toast(format!("Export failed: {e}"), true),
        }
    }
}

/// SVG text or any raster the `image` crate reads -> (svg, 2x flattened PNG).
fn import_any(bytes: &[u8]) -> Result<(Option<String>, Vec<u8>), String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]).to_lowercase();
    if head.contains("<svg") {
        let svg = String::from_utf8(bytes.to_vec()).map_err(|_| "SVG is not valid UTF-8".to_string())?;
        figures::validate_svg(&svg).map_err(|e| format!("Not a usable SVG: {e}"))?;
        let png = flatten_white(&figures::to_png(&svg, 2.0)?)?;
        Ok((Some(svg), png))
    } else {
        let img = image::load_from_memory(bytes).map_err(|_| "Not an SVG or a supported image (PNG/JPG/GIF/WebP/BMP)".to_string())?;
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok((None, flatten_white(&png)?))
    }
}

// ---------- widgets ----------

fn ghost(ui: &mut Ui, label: &str) -> egui::Response {
    crate::kit::ghost_button(ui, label)
}

fn accent_btn(ui: &mut Ui, label: &str) -> egui::Response {
    crate::kit::primary_button(ui, label)
}

fn tab(ui: &mut Ui, label: &str, active: bool) -> bool {
    crate::kit::chip_toggle(ui, label, active).clicked()
}

/// One browser card; true when clicked.
fn card(ui: &mut Ui, ctx: &egui::Context, def: &FigureDef, idx: usize, budget: &mut u8, cw: f32) -> bool {
    let resp = Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(crate::kit::R_CARD))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(cw - 20.0);
            ui.set_height(218.0);
            // Vertical layout: the parent is horizontal_top, and a Frame
            // inherits its direction — without the wrap the title, subject
            // and blurb all draw side-by-side on one row (overwriting).
            ui.vertical(|ui| {
                Frame::new().fill(egui::Color32::WHITE).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
                    ui.set_width(cw - 20.0);
                    ui.set_height(100.0);
                    ui.centered_and_justified(|ui| match thumb(ctx, def, budget) {
                        Some(t) => {
                            ui.add(egui::Image::new(&t).shrink_to_fit());
                        }
                        None => {
                            ui.label(theme::muted("…"));
                        }
                    });
                });
                ui.add_space(5.0);
                ui.label(theme::strong(def.title).size(13.0));
                ui.label(theme::muted(def.subject.name()).size(11.0));
                ui.vertical(|ui| {
                    ui.set_max_width(cw - 24.0);
                    ui.label(
                        theme::text(def.blurb).size(11.0),
                    );
                });
            });
        })
        .response;
    let r = ui.interact(resp.rect, UiId::new("fig_card").with(idx), Sense::click());
    if r.hovered() {
        ui.painter().rect_stroke(
            resp.rect,
            CornerRadius::same(crate::kit::R_CARD),
            Stroke::new(1.5, theme::accent_col()),
            egui::StrokeKind::Inside,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r.clicked()
}

fn chip_handles(ui: &mut Ui, on: bool) -> bool {
    crate::kit::chip_toggle(ui, "Handles", on).on_hover_text("Show drag handles on the figure").clicked()
}

/// Splits a list value on the separators the generators accept.
fn list_items(s: &str) -> (Vec<String>, char) {
    let sep = if s.contains(';') { ';' } else { ',' };
    let v: Vec<String> = s.split(sep).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
    (v, sep)
}

/// Palette used by Kind::Color swatches.
fn swatch(hex: &str) -> egui::Color32 {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0);
    egui::Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Control auto-built from one ParamDef.
/// Hides a text box whose Text / Blank line / None choice isn't on "Text" (the reference only
/// shows the box when it is needed). Pairs: `k` with `k_mode`, `k_label` with `k_lab`, `k_text` with `k`.
fn is_hidden(defs: &[ParamDef], p: &Params, pd: &ParamDef) -> bool {
    if !matches!(pd.kind, Kind::Text | Kind::Long) {
        return false;
    }
    let k = pd.key;
    let mut cands: Vec<String> = vec![format!("{k}_mode")];
    if let Some(b) = k.strip_suffix("_label") {
        cands.push(format!("{b}_lab"));
        cands.push(format!("{b}_mode"));
    }
    if let Some(b) = k.strip_suffix("_text") {
        cands.push(b.to_string());
        cands.push(format!("{b}_mode"));
    }
    cands.iter().any(|c| {
        defs.iter().any(|d| d.key == c.as_str() && matches!(&d.kind, Kind::Choice(o) if o.contains(&"Text")) && p.s(d.key) != "Text")
    })
}

/// A short live summary under a section header: the first few meaningful values.
fn section_summary(defs: &[ParamDef], p: &Params, items: &[usize]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for &i in items {
        let d = &defs[i];
        if is_hidden(defs, p, d) {
            continue;
        }
        let part = match (&d.kind, p.0.get(d.key)) {
            (Kind::Choice(_), Some(Value::S(v))) if !v.is_empty() => Some(v.clone()),
            (Kind::Text, Some(Value::S(v))) if !v.trim().is_empty() => Some(v.chars().take(18).collect()),
            (Kind::Bool, Some(Value::B(true))) => Some(d.label.split(['(', ':']).next().unwrap_or(d.label).trim().to_lowercase()),
            (Kind::Float { .. }, Some(Value::F(v))) => Some(format!("{} {}", short_label(d.label), (v * 100.0).round() / 100.0)),
            (Kind::Int { .. }, Some(Value::I(v))) => Some(format!("{} {}", short_label(d.label), v)),
            _ => None,
        };
        if let Some(t) = part {
            parts.push(t);
        }
        if parts.len() >= 3 {
            break;
        }
    }
    parts.join(" · ")
}

fn fmt_measure(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Applies a chip: fills slot `count + 1` and bumps the count, or (no `count_key`) just sets the
/// chip's values. `Value::I(i64::MIN)` becomes a fresh random number within the param's range.
fn apply_chip(p: &mut Params, pdefs: &[ParamDef], card: &figures::ChipsCard, chip: &figures::Chip) {
    let slotted = !card.count_key.is_empty();
    let slot = if slotted { p.i(card.count_key) + 1 } else { 0 };
    if slotted && slot > card.max {
        return;
    }
    let letter = ((b'A' + (slot as u8).saturating_sub(1)) as char).to_string();
    for (tmpl, v) in &chip.set {
        let key = tmpl.replace("{n}", &slot.to_string());
        let Some(def) = pdefs.iter().find(|d| d.key == key) else { continue };
        let val = match v {
            Value::S(t) => Value::S(t.replace("{L}", &letter)),
            Value::I(i64::MIN) => {
                let (lo, hi) = match &def.kind { Kind::Int { min, max } => (*min, *max), _ => (1, 1_000_000) };
                let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as i64).unwrap_or(7);
                Value::I(lo + t.rem_euclid((hi - lo).max(1)))
            }
            other => other.clone(),
        };
        p.0.insert(key, val);
    }
    if slotted {
        p.set_i(card.count_key, slot);
    }
}

/// "Vector 1 magnitude" -> "magnitude"; "Number of vectors (1-4)" -> "number of vectors".
fn short_label(l: &str) -> String {
    let base = l.split(['(', ':', ',']).next().unwrap_or(l).trim();
    let words: Vec<&str> = base.split_whitespace().collect();
    let keep = if words.len() > 3 { &words[words.len() - 2..] } else { &words[..] };
    keep.join(" ").to_lowercase()
}

/// Short choices (<= 4 options, short labels) as one row of buttons, like the reference.
fn segmented(ui: &mut Ui, opts: &[&'static str], cur: &mut String) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for o in opts {
            let sel = cur == o;
            let b = Button::new(RichText::new(*o).size(12.0).color(if sel { theme::accent_col() } else { theme::muted_col() }))
                .fill(if sel { theme::accent_faded() } else { egui::Color32::TRANSPARENT })
                .stroke(Stroke::new(1.0, if sel { theme::accent_col() } else { theme::border() }))
                .corner_radius(CornerRadius::same(8));
            if ui.add(b).clicked() {
                *cur = o.to_string();
            }
        }
    });
}

fn param_control(ui: &mut Ui, pd: &ParamDef, p: &mut Params) {
    match &pd.kind {
        Kind::Float { min, max, step } => {
            ui.label(theme::muted(pd.label).size(11.0));
            let mut v = p.f(pd.key);
            let mut s = egui::Slider::new(&mut v, *min..=*max).clamping(egui::SliderClamping::Always);
            // Ranges spanning orders of magnitude are unusable on a linear slider; a log slider
            // can't also snap to a step, so round to 3 significant digits instead.
            let log = *min > 0.0 && *max / *min >= 200.0;
            if log {
                s = s.logarithmic(true).smallest_positive(*min).max_decimals(4);
            } else if *step > 0.0 {
                s = s.step_by(*step);
            }
            if ui.add(s).changed() {
                if log && v > 0.0 {
                    let mag = 10f64.powf(v.log10().floor() - 2.0);
                    v = (v / mag).round() * mag;
                }
                p.0.insert(pd.key.to_string(), Value::F(v.clamp(*min, *max)));
            }
        }
        Kind::Int { min, max } => {
            let mut v = p.i(pd.key);
            if max - min <= 12 {
                ui.horizontal(|ui| {
                    ui.label(theme::muted(pd.label).size(11.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add_enabled(v < *max, Button::new("+").min_size(Vec2::splat(22.0))).clicked() {
                            v += 1;
                        }
                        ui.label(theme::strong(v.to_string()).size(13.0));
                        if ui.add_enabled(v > *min, Button::new("-").min_size(Vec2::splat(22.0))).clicked() {
                            v -= 1;
                        }
                    });
                });
                if v != p.i(pd.key) {
                    p.0.insert(pd.key.to_string(), Value::I(v.clamp(*min, *max)));
                }
            } else {
                ui.label(theme::muted(pd.label).size(11.0));
                if ui.add(egui::Slider::new(&mut v, *min..=*max).clamping(egui::SliderClamping::Always)).changed() {
                    p.0.insert(pd.key.to_string(), Value::I(v));
                }
            }
        }
        Kind::Bool => {
            let mut v = p.b(pd.key);
            if ui.checkbox(&mut v, pd.label).changed() {
                p.0.insert(pd.key.to_string(), Value::B(v));
            }
        }
        Kind::Choice(opts) => {
            ui.label(theme::muted(pd.label).size(11.0));
            let mut cur = p.s(pd.key).to_string();
            if opts.len() <= 4 && opts.iter().map(|o| o.chars().count()).sum::<usize>() <= 30 {
                segmented(ui, opts, &mut cur);
            } else {
                ComboBox::from_id_salt(("fig_param", pd.key))
                    .selected_text(cur.clone())
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for o in opts {
                            ui.selectable_value(&mut cur, o.to_string(), *o);
                        }
                    });
            }
            if cur != p.s(pd.key) {
                p.0.insert(pd.key.to_string(), Value::S(cur));
            }
        }
        Kind::Text => {
            ui.label(theme::muted(pd.label).size(11.0));
            let mut cur = p.s(pd.key).to_string();
            if ui.add(TextEdit::singleline(&mut cur).desired_width(220.0)).changed() {
                p.0.insert(pd.key.to_string(), Value::S(cur));
            }
        }
        Kind::Long => {
            ui.label(theme::muted(pd.label).size(11.0));
            let mut cur = p.s(pd.key).to_string();
            if ui
                .add(TextEdit::multiline(&mut cur).desired_width(240.0).desired_rows(4).font(egui::TextStyle::Monospace))
                .changed()
            {
                p.0.insert(pd.key.to_string(), Value::S(cur));
            }
        }
        Kind::List => {
            ui.label(theme::muted(pd.label).size(11.0));
            let (mut items, sep) = list_items(p.s(pd.key));
            let mut changed = false;
            let mut del = None;
            for (i, it) in items.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    if ui.add(TextEdit::singleline(it).desired_width(176.0)).changed() {
                        changed = true;
                    }
                    if ui.add(Button::new("-").min_size(Vec2::splat(22.0))).on_hover_text("Remove").clicked() {
                        del = Some(i);
                    }
                });
            }
            if let Some(i) = del {
                items.remove(i);
                changed = true;
            }
            if ui.add(Button::new("+ Add").small()).clicked() {
                items.push(items.last().cloned().unwrap_or_else(|| "0".into()));
                changed = true;
            }
            if changed {
                let joined = items.join(if sep == ';' { "; " } else { ", " });
                p.0.insert(pd.key.to_string(), Value::S(joined));
            }
        }
        Kind::Color(pal) => {
            ui.label(theme::muted(pd.label).size(11.0));
            let cur = p.s(pd.key).to_string();
            ui.horizontal_wrapped(|ui| {
                for hex in pal {
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                    ui.painter().circle_filled(r.center(), 8.0, swatch(hex));
                    if cur.eq_ignore_ascii_case(hex) {
                        ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(2.0, theme::accent_col()));
                    }
                    if resp.on_hover_text(*hex).clicked() {
                        p.0.insert(pd.key.to_string(), Value::S((*hex).to_string()));
                    }
                }
            });
        }
    }
}

// ---------- App wiring ----------

/// Figures screen + modal state, owned by `App`.
#[derive(Default)]
pub struct FigUi {
    ws: Workspace,
    save: Option<SaveDlg>,
    /// Insert-figure modal of the question editor.
    picker: Option<Workspace>,
}

struct SaveDlg {
    out: Output,
    bank: Option<Id>,
    /// None = new question stub.
    target: Option<Id>,
}

/// Request flags from the question editor to the picker (stored in egui memory).
pub const REQ_KEY: &str = "testparrot_fig_request";
pub const REQ_INSERT: u8 = 1;
pub const REQ_EDIT: u8 = 2;
pub const REQ_IMPORT: u8 = 3;

pub fn request(ctx: &egui::Context, what: u8) {
    ctx.data_mut(|d| d.insert_temp(UiId::new(REQ_KEY), what));
}

fn snippet(q: &Question) -> String {
    let s: String = q.stem.chars().take(60).collect();
    if s.trim().is_empty() { "(no text)".to_string() } else { s }
}

impl crate::App {
    /// The standalone Figures screen (sidebar already drawn onto `ui`).
    pub fn draw_figures(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut ws = std::mem::take(&mut self.figs.ws);
        let mut act = Act::None;
        crate::screens::desk_panel(ui, 24, |ui| {
            crate::kit::page_header(
                ui,
                &["Home", "Figures"],
                "Figures",
                Some("Physics, biology, chemistry and math diagrams you can tune, export, or attach to a question."),
            );
            ui.add_space(12.0);
            act = ws.show(ui);
        });
        self.figs.ws = ws;
        match act {
            Act::Save(out) => {
                let bank = self.store.banks.first().map(|b| b.id.clone());
                self.figs.save = Some(SaveDlg { out, bank, target: None });
            }
            Act::Toast(t, err) => crate::screens::toast(self, t, err),
            _ => {}
        }
        self.draw_save_dialog(&ctx);
    }

    fn draw_save_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.figs.save.take() else { return };
        let banks: Vec<(Id, String)> = self.store.banks.iter().map(|b| (b.id.clone(), b.name.clone())).collect();
        let mut done = false;
        let mut confirm = false;
        let m = Modal::new(UiId::new("testparrot_fig_save")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(theme::serif_strong("Save to question").size(20.0));
            ui.add_space(8.0);
            if banks.is_empty() {
                ui.label(theme::muted("You have no question banks yet. Create one on the Question Banks screen first."));
                if ghost(ui, "Close").clicked() {
                    done = true;
                }
                return;
            }
            let bank_name = banks.iter().find(|(i, _)| Some(i) == dlg.bank.as_ref()).map(|(_, n)| n.clone()).unwrap_or_default();
            ui.label(theme::muted("Bank").size(11.0));
            ComboBox::from_id_salt("fig_save_bank").selected_text(bank_name).width(380.0).show_ui(ui, |ui| {
                for (id, name) in &banks {
                    if ui.selectable_label(dlg.bank.as_ref() == Some(id), name).clicked() {
                        dlg.bank = Some(id.clone());
                        dlg.target = None;
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(theme::muted("Question").size(11.0));
            let qs: Vec<(Id, String)> = dlg
                .bank
                .as_ref()
                .and_then(|b| self.store.bank(b))
                .map(|b| b.questions.iter().map(|q| (q.id.clone(), snippet(q))).collect())
                .unwrap_or_default();
            let cur = match &dlg.target {
                None => "New question".to_string(),
                Some(t) => qs.iter().find(|(i, _)| i == t).map(|(_, s)| s.clone()).unwrap_or_default(),
            };
            ComboBox::from_id_salt("fig_save_q").selected_text(cur).width(380.0).show_ui(ui, |ui| {
                if ui.selectable_label(dlg.target.is_none(), "New question").clicked() {
                    dlg.target = None;
                }
                for (id, s) in &qs {
                    if ui.selectable_label(dlg.target.as_ref() == Some(id), s).clicked() {
                        dlg.target = Some(id.clone());
                    }
                }
            });
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if accent_btn(ui, "Save").clicked() {
                    confirm = true;
                }
                if ghost(ui, "Cancel").clicked() {
                    done = true;
                }
            });
        });
        if confirm {
            self.save_figure_to_question(dlg);
        } else if !done && !m.should_close() {
            self.figs.save = Some(dlg);
        }
    }

    fn save_figure_to_question(&mut self, dlg: SaveDlg) {
        let Some(bank_id) = dlg.bank else { return };
        let img = match crate::store::put_image(&dlg.out.png) {
            Ok(i) => i,
            Err(e) => return self.toast_err(format!("Couldn't store the image: {e}")),
        };
        let now = crate::screens::now_secs();
        let mut open: Option<Question> = None;
        {
            let Some(bank) = self.store.bank_mut(&bank_id) else { return };
            match &dlg.target {
                Some(qid) => {
                    if let Some(q) = bank.questions.iter_mut().find(|q| q.id == *qid) {
                        q.image = Some(img);
                        q.figure = dlg.out.spec.clone();
                        q.modified = now;
                    }
                }
                None => {
                    let q = Question {
                        id: crate::model::new_id(),
                        stem: "Use the figure to answer the question.".to_string(),
                        kind: crate::model::Kind::Short { sample: String::new() },
                        points: None,
                        difficulty: None,
                        topics: Vec::new(),
                        image: Some(img),
                        explanation: None,
                        figure: dlg.out.spec.clone(),
                        created: now,
                        modified: now,
                    };
                    open = Some(q.clone());
                    bank.questions.push(q);
                }
            }
            bank.modified = now;
        }
        self.store.save_banks();
        match open {
            Some(q) => {
                self.toast("Question created with the figure");
                self.open_bank(&bank_id);
                self.qedit = Some(crate::qedit::Draft::from_question(bank_id, &q));
            }
            None => self.toast("Figure attached to the question"),
        }
    }

    /// Insert/Edit/Import modal for the question editor. Call after the editor modal so it
    /// stacks on top; consumes request flags set by the editor's image row.
    pub fn draw_figure_picker(&mut self, ctx: &egui::Context) {
        if self.qedit.is_none() {
            self.figs.picker = None;
            return;
        }
        let req = ctx.data_mut(|d| d.get_temp::<u8>(UiId::new(REQ_KEY))).unwrap_or(0);
        if req != 0 {
            ctx.data_mut(|d| d.remove::<u8>(UiId::new(REQ_KEY)));
            self.figs.picker = Some(match req {
                REQ_EDIT => self
                    .qedit
                    .as_ref()
                    .and_then(|d| d.figure.as_ref())
                    .and_then(|s| Workspace::from_spec(s, true))
                    .unwrap_or_else(|| Workspace::new(true)),
                REQ_IMPORT => Workspace::open_import(true),
                _ => Workspace::new(true),
            });
        }
        let Some(mut ws) = self.figs.picker.take() else { return };
        let m = Modal::new(UiId::new("testparrot_fig_picker")).show(ctx, |ui| {
            ui.set_min_width(860.0);
            ui.set_max_width((ctx.viewport_rect().width() - 80.0).max(560.0));
            ui.label(theme::serif_strong("Insert figure").size(21.0));
            ui.add_space(8.0);
            ws.show(ui)
        });
        match m.inner {
            Act::Insert(out) => match crate::store::put_image(&out.png) {
                Ok(img) => {
                    if let Some(d) = self.qedit.as_mut() {
                        d.image = Some(img);
                        d.figure = out.spec;
                    }
                }
                Err(e) => {
                    self.toast_err(format!("Couldn't store the image: {e}"));
                    self.figs.picker = Some(ws);
                }
            },
            Act::Close => {}
            Act::Toast(t, err) => {
                crate::screens::toast(self, t, err);
                self.figs.picker = Some(ws);
            }
            _ => {
                if !m.should_close() {
                    self.figs.picker = Some(ws);
                }
            }
        }
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> &'static FigureDef {
        find_def("demo-shapes").expect("demo registered under cfg(test)").1
    }

    #[test]
    fn spec_roundtrip_and_unknown_keys() {
        let def = demo();
        let mut p = Params::defaults(&(def.params)());
        p.0.insert("w".into(), Value::F(88.5));
        p.0.insert("n".into(), Value::I(5));
        p.0.insert("shape".into(), Value::S("circle".into()));
        let spec = spec_for(def.id, &p);
        let json = serde_json::to_string(&spec).unwrap();
        let mut back: FigureSpec = serde_json::from_str(&json).unwrap();
        back.params.insert("bogus".into(), serde_json::json!(1));
        back.params.insert("fill".into(), serde_json::json!("not a bool"));
        let q = params_from_spec(def, &back);
        assert_eq!(q.f("w"), 88.5);
        assert_eq!(q.i("n"), 5);
        assert_eq!(q.s("shape"), "circle");
        assert!(q.b("fill"), "wrong-typed value falls back to default");
        assert!(!q.0.contains_key("bogus"));
    }

    #[test]
    fn hash_changes_with_params() {
        let def = demo();
        let mut p = Params::defaults(&(def.params)());
        let a = hash_params(def.id, &p);
        p.0.insert("n".into(), Value::I(6));
        assert_ne!(a, hash_params(def.id, &p));
    }

    #[test]
    fn output_is_flattened_2x_png() {
        let def = demo();
        let ws = Workspace::from_spec(&spec_for(def.id, &Params::defaults(&(def.params)())), true).unwrap();
        let out = ws.output().unwrap();
        let img = image::load_from_memory(&out.png).unwrap().to_rgba8();
        assert_eq!((img.width(), img.height()), (800, 520));
        assert!(img.pixels().all(|p| p[3] == 255));
        assert!(out.spec.is_some() && out.svg.is_some());
    }

    #[test]
    fn import_svg_and_png_and_garbage() {
        let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='40' height='20'><rect width='40' height='20' fill='red'/></svg>";
        let (s, png) = import_any(svg.as_bytes()).unwrap();
        assert!(s.is_some());
        let (s2, png2) = import_any(&png).unwrap();
        assert!(s2.is_none());
        assert!(image::load_from_memory(&png2).is_ok());
        assert!(import_any(b"hello world").is_err());
        assert!(import_any(b"<svg").is_err());
    }

    #[test]
    fn transparent_svg_flattens_to_white() {
        let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='4' height='4'></svg>";
        let (_, png) = import_any(svg.as_bytes()).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0).0, [255, 255, 255, 255]);
    }

    #[test]
    fn workspace_ui_runs_headless() {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut ws = Workspace::new(false);
        let frame = |ws: &mut Workspace| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let _ = ws.show(ui);
            });
            out.textures_delta.clear();
        };
        frame(&mut ws);
        ws.select(find_def("demo-shapes").unwrap().0);
        frame(&mut ws);
        frame(&mut ws);
        assert!(ws.shown.is_some());
    }

    /// Every real figure opens in the workspace and draws its handle overlay without panicking.
    #[test]
    fn every_figure_opens_with_overlay() {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        for (i, d) in reg().iter().enumerate().filter(|(_, d)| d.id != "demo-shapes") {
            let mut ws = Workspace::new(false);
            ws.select(i);
            for _ in 0..3 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let _ = ws.show(ui);
                });
                out.textures_delta.clear();
            }
            assert!(ws.shown.is_some(), "{} did not render", d.id);
            assert!(ws.canvas.0 > 50.0, "{} canvas unknown", d.id);
        }
    }

    #[test]
    fn undo_redo_round_trips_a_pasted_spec() {
        let mut ws = Workspace::new(false);
        let id = "lewis-structures";
        let (i, _) = find_def(id).unwrap();
        ws.select(i);
        let before = hash_params(id, &ws.params);
        ws.apply_spec_text(&format!(r#"{{"queshion":"figure","id":"{id}","params":{{"formula":"NH3"}}}}"#)).unwrap_or_else(|e| panic!("{e}"));
        let after = hash_params(id, &ws.params);
        assert_ne!(before, after, "the pasted formula must change the params");
        ws.step_history(true);
        assert_eq!(hash_params(id, &ws.params), before, "undo restores the previous params");
        ws.step_history(false);
        assert_eq!(hash_params(id, &ws.params), after, "redo reapplies the pasted params");
    }

    #[test]
    fn pasted_spec_rejects_garbage_and_unknown_figures() {
        let mut ws = Workspace::new(false);
        assert!(ws.apply_spec_text("not json").is_err());
        assert!(ws.apply_spec_text(r#"{"id":"nope"}"#).is_err());
        assert!(ws.apply_spec_text(r#"{"params":{}}"#).is_err());
    }

    #[test]
    fn spec_json_copies_what_apply_reads() {
        let mut ws = Workspace::new(false);
        let (i, _) = find_def("lewis-structures").unwrap();
        ws.select(i);
        ws.params.set_s("formula", "NH3");
        let text = ws.spec_json().unwrap();
        let mut other = Workspace::new(false);
        other.apply_spec_text(&text).unwrap();
        assert_eq!(other.params.s("formula"), "NH3");
        assert_eq!(other.sel, Some(i));
    }

    #[test]
    fn add_a_force_and_vector_chips_fill_the_next_slot() {
        let fbd = figures::registry().into_iter().find(|d| d.id == "free-body-diagram").unwrap();
        let defs = (fbd.params)();
        let mut p = Params::defaults(&defs);
        let card = figures::chips_for("free-body-diagram").unwrap();
        let n0 = p.i("n");
        apply_chip(&mut p, &defs, &card, &card.chips[0]);
        assert_eq!(p.i("n"), n0 + 1);
        assert_eq!(p.s(&format!("f{}_lab", n0 + 1)), "F_g");
        assert_eq!(p.f(&format!("f{}_ang", n0 + 1)), 270.0);
        // Full: further chips do nothing.
        for _ in 0..10 {
            apply_chip(&mut p, &defs, &card, &card.chips[1]);
        }
        assert_eq!(p.i("n"), card.max);

        let vd = figures::registry().into_iter().find(|d| d.id == "vector-diagram").unwrap();
        let vdefs = (vd.params)();
        let mut q = Params::defaults(&vdefs);
        let vcard = figures::chips_for("vector-diagram").unwrap();
        let before = q.i("n");
        apply_chip(&mut q, &vdefs, &vcard, &vcard.chips[0]);
        assert_eq!(q.i("n"), before + 1);
        assert_eq!(q.s(&format!("v{}_lab", before + 1)), ((b'A' + before as u8) as char).to_string());
    }

    // ---------- composite-shape canvas (driven with synthetic pointer events) ----------

    fn comp_ws(spec: &str) -> (egui::Context, Workspace) {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut ws = Workspace::new(false);
        ws.select(find_def("composite-shape").unwrap().0);
        ws.params.set_s("preset", figures::math::COMPOSITE_CUSTOM);
        ws.params.set_s("spec", spec);
        for _ in 0..3 {
            comp_frame(&ctx, &mut ws, vec![]);
        }
        (ctx, ws)
    }

    fn comp_frame(ctx: &egui::Context, ws: &mut Workspace, events: Vec<egui::Event>) {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1400.0, 900.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| {
            let _ = ws.show(ui);
        });
        out.textures_delta.clear();
    }

    fn btn(p: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![egui::Event::PointerMoved(p), egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }]
    }

    /// Press at model point `a`, move through to `b`, release there.
    fn comp_drag(ctx: &egui::Context, ws: &mut Workspace, a: (f64, f64), b: (f64, f64)) {
        let (pa, pb) = (ws.comp.to_screen(a), ws.comp.to_screen(b));
        comp_frame(ctx, ws, btn(pa, true));
        for k in 1..=4 {
            comp_frame(ctx, ws, vec![egui::Event::PointerMoved(pa + (pb - pa) * (k as f32 / 4.0))]);
        }
        comp_frame(ctx, ws, btn(pb, false));
        comp_frame(ctx, ws, vec![]);
    }

    fn comp_click(ctx: &egui::Context, ws: &mut Workspace, a: (f64, f64)) {
        let p = ws.comp.to_screen(a);
        let mut ev = btn(p, true);
        ev.extend(btn(p, false));
        comp_frame(ctx, ws, ev);
        comp_frame(ctx, ws, vec![]);
    }

    #[test]
    fn composite_canvas_moves_a_piece_and_undo_restores_it() {
        let (ctx, mut ws) = comp_ws("rect 0 0 4 3; dim 0 0 4 0");
        assert!(ws.comp.rect.width() > 300.0, "canvas laid out: {:?}", ws.comp.rect);
        comp_drag(&ctx, &mut ws, (2.0, 1.5), (4.1, 1.4));
        assert_eq!(ws.params.s("spec"), "rect 2 0 4 3; dim 2 0 6 0", "moved on the half-unit snap, dim along");
        assert_eq!(ws.comp.sel, Some(0));
        ws.step_history(true);
        assert_eq!(ws.params.s("spec"), "rect 0 0 4 3; dim 0 0 4 0");
        ws.step_history(false);
        assert_eq!(ws.params.s("spec"), "rect 2 0 4 3; dim 2 0 6 0");
    }

    #[test]
    fn composite_canvas_resizes_with_a_corner_grip() {
        let (ctx, mut ws) = comp_ws("rect 0 0 4 3");
        ws.comp.sel = Some(0);
        comp_frame(&ctx, &mut ws, vec![]);
        comp_drag(&ctx, &mut ws, (4.0, 3.0), (6.1, 5.2));
        assert_eq!(ws.params.s("spec"), "rect 0 0 6 5");
    }

    #[test]
    fn composite_canvas_draws_new_pieces_with_tools() {
        let (ctx, mut ws) = comp_ws("rect 0 0 4 3");
        ws.comp.tool = composite_canvas::Tool::Rect;
        comp_drag(&ctx, &mut ws, (4.0, 0.0), (6.0, 2.0));
        assert_eq!(ws.params.s("spec"), "rect 0 0 4 3; rect 4 0 2 2");
        assert_eq!((ws.comp.sel, ws.comp.tool), (Some(1), composite_canvas::Tool::Select));
        // a discrete add is its own undo step
        ws.step_history(true);
        assert_eq!(ws.params.s("spec"), "rect 0 0 4 3");
        ws.step_history(false);
        ws.comp.tool = composite_canvas::Tool::Circle;
        comp_frame(&ctx, &mut ws, vec![]); // the selection's panel closes with the tool change
        comp_drag(&ctx, &mut ws, (1.0, 1.5), (2.0, 1.5));
        assert!(ws.params.s("spec").ends_with("; circle 1 1.5 1"), "{}", ws.params.s("spec"));
        // free polygon: click the corners, then the first one again
        ws.comp.tool = composite_canvas::Tool::Poly;
        comp_frame(&ctx, &mut ws, vec![]);
        for q in [(0.5, 0.5), (2.5, 0.5), (2.5, 2.5), (0.5, 0.5)] {
            comp_click(&ctx, &mut ws, q);
        }
        assert!(ws.params.s("spec").ends_with("; tri 0.5 0.5 2.5 0.5 2.5 2.5"), "{}", ws.params.s("spec"));
        assert!(ws.comp.poly.is_empty());
        // the figure still renders for export
        let out = ws.output().unwrap();
        assert!(out.svg.unwrap().contains("<polygon"));
    }

    #[test]
    fn composite_canvas_deletes_and_labels_corners() {
        let (ctx, mut ws) = comp_ws("rect 0 0 4 3; rect 4 0 2 2");
        ws.comp.tool = composite_canvas::Tool::Label;
        comp_click(&ctx, &mut ws, (0.0, 0.0));
        assert_eq!(ws.params.s("spec"), "rect 0 0 4 3; rect 4 0 2 2; ra 0 0 NE");
        comp_click(&ctx, &mut ws, (0.0, 0.0));
        assert_eq!(ws.params.s("spec"), "rect 0 0 4 3; rect 4 0 2 2");
        ws.comp.tool = composite_canvas::Tool::Select;
        comp_click(&ctx, &mut ws, (5.0, 1.0));
        assert_eq!(ws.comp.sel, Some(1));
        let key = |pressed| egui::Event::Key { key: egui::Key::Delete, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
        comp_frame(&ctx, &mut ws, vec![key(true), key(false)]);
        assert_eq!(ws.params.s("spec"), "rect 0 0 4 3");
        assert_eq!(ws.comp.sel, None);
    }


    #[test]
    fn composite_canvas_pans_on_empty_space() {
        let (ctx, mut ws) = comp_ws("rect 0 0 4 3");
        let before = ws.comp.to_screen((0.0, 0.0));
        let a = ws.comp.rect.left_top() + Vec2::new(40.0, 120.0);
        let b = a + Vec2::new(60.0, -30.0);
        comp_frame(&ctx, &mut ws, btn(a, true));
        for k in 1..=3 {
            comp_frame(&ctx, &mut ws, vec![egui::Event::PointerMoved(a + (b - a) * (k as f32 / 3.0))]);
        }
        comp_frame(&ctx, &mut ws, btn(b, false));
        let moved = ws.comp.to_screen((0.0, 0.0)) - before;
        assert!((moved - (b - a)).length() < 1.0, "the shape follows the pointer: {moved:?}");
        assert_eq!(ws.params.s("spec"), "rect 0 0 4 3", "panning edits nothing");
    }

}
