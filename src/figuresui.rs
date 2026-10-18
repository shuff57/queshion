//! Figure engine UI: the standalone Figures screen, the shared browser + parameter workspace,
//! the "Insert figure" modal used by the question editor, "Save to question", and the
//! hybrid SVG/PNG import. Rendering and parameter schema live in `figures`; this file only
//! builds controls from `ParamDef`s and shows `figures::to_png` output.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use std::time::Duration;
use web_time::Instant;

use egui::{
    Align, Button, ComboBox, CornerRadius, Frame, Id as UiId, Layout, Margin, Modal, RichText,
    ScrollArea, Sense, Stroke, TextEdit, TextureHandle, Ui, Vec2,
};

use crate::figures::{self, FigureDef, Kind, ParamDef, Params, Subject, Value};
use crate::model::{FigureSpec, Id, Question};
use crate::theme;

mod cards_panel;
mod composite_canvas;
mod pedigree_panel;

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
        if crate::env_var("VERSHION_DEMO_FIGURES").as_deref() == Ok("1") {
            v.extend(demo_defs());
        }
        v
    })
}

/// Tiny demo figure so the UI can be exercised while subject modules are empty. Only ever
/// registered under `cfg(test)` or a debug build with `VERSHION_DEMO_FIGURES=1`.
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

/// Most screen points per SVG unit the preview zooms a figure to.
const PREVIEW_MAX_ZOOM: f64 = 4.0;

/// Bounding box `[x0, y0, x1, y1]` (pixels) of the inked pixels of a premultiplied RGBA
/// raster: anything not (near) white and not transparent. `None` for a blank figure.
fn ink_box(px: &[u8], w: u32, h: u32) -> Option<[u32; 4]> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for y in 0..h {
        let row = &px[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
        for x in 0..w {
            let p = &row[(x * 4) as usize..(x * 4 + 4) as usize];
            let a = p[3] as u32;
            if a > 24 && (p[0] as u32).min(p[1] as u32).min(p[2] as u32) * 100 < a * 92 {
                x0 = x0.min(x);
                x1 = x1.max(x + 1);
                y0 = y0.min(y);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1, y1])
}

/// The part of the canvas the preview shows: the inked box plus a margin, kept on the canvas
/// (the whole canvas when nothing is inked).
fn view_box(content: Option<[f64; 4]>, (cw, ch): (f64, f64)) -> [f64; 4] {
    let Some(b) = content else { return [0.0, 0.0, cw, ch] };
    let pad = 0.05 * (b[2] - b[0]).max(b[3] - b[1]) + 8.0;
    [(b[0] - pad).max(0.0), (b[1] - pad).max(0.0), (b[2] + pad).min(cw), (b[3] + pad).min(ch)]
}

/// Whether the preview keeps its current view `old` for a drawing whose own view is `new`:
/// the drawing still fits inside, and the view isn't much bigger than it (no jumpy refits on
/// every small edit).
fn keep_view(old: [f64; 4], new: [f64; 4]) -> bool {
    let inside = new[0] >= old[0] - 0.5 && new[1] >= old[1] - 0.5 && new[2] <= old[2] + 0.5 && new[3] <= old[3] + 0.5;
    let area = |b: [f64; 4]| (b[2] - b[0]).max(1.0) * (b[3] - b[1]).max(1.0);
    inside && area(old) <= area(new) * 1.3
}

/// Raster scale for a preview shown at `px_per_unit` device pixels per SVG unit: rounded to a
/// quarter so small zoom changes don't re-render, and capped so the texture stays modest.
fn raster_for(px_per_unit: f64, (cw, ch): (f64, f64)) -> f32 {
    let cap = (4096.0 / cw.max(ch).max(1.0)).min((12.0e6 / (cw * ch).max(1.0)).sqrt());
    ((px_per_unit.min(cap) * 4.0).ceil() / 4.0).clamp(1.0, 6.0) as f32
}

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

thread_local! {
    /// How many thumbnails were actually rendered (tests check the cache holds across frames).
    static THUMB_RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn thumb(ctx: &egui::Context, def: &FigureDef, budget: &mut u8) -> Option<TextureHandle> {
    thumb_for(ctx, def, &figures::start_params(def), 0.6, budget)
}

/// Cached preview texture of `def` drawn with `p` (keyed by figure id + params hash, so the
/// browser card of a figure and an example with the same params share one texture). Renders at
/// most `budget` misses per frame and asks for another frame while any are missing.
fn thumb_for(ctx: &egui::Context, def: &FigureDef, p: &Params, scale: f32, budget: &mut u8) -> Option<TextureHandle> {
    let key = format!("{}:{:x}:{scale}", def.id, hash_params(def.id, p));
    if let Some(hit) = THUMBS.with(|t| t.borrow().get(&key).cloned()) {
        return hit;
    }
    if *budget == 0 {
        ctx.request_repaint(); // render a few per frame so the grid never stalls
        return None;
    }
    *budget -= 1;
    THUMB_RENDERS.with(|c| c.set(c.get() + 1));
    let tex = figures::to_png(&(def.render)(p), scale)
        .ok()
        .and_then(|png| texture(ctx, format!("testparrot:thumb:{key}"), &png));
    THUMBS.with(|t| t.borrow_mut().insert(key, tex.clone()));
    tex
}

const EX_CARD_W: f32 = 270.0;
/// One gallery row: card (preview 170 + name + buttons + margins) plus the gap under it.
const EX_CARD_H: f32 = 304.0;

/// One gallery card: preview, name, Edit and PNG. Returns (edit clicked, png clicked).
fn example_card(ui: &mut Ui, ctx: &egui::Context, def: &FigureDef, e: &figures::Example, k: usize, budget: &mut u8, cw: f32) -> (bool, bool) {
    let p = example_params(def, e);
    let (mut edit, mut png) = (false, false);
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(crate::kit::R_CARD))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(cw - 22.0);
            ui.vertical(|ui| {
                Frame::new().fill(egui::Color32::WHITE).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
                    ui.set_width(cw - 22.0);
                    ui.set_height(170.0);
                    ui.centered_and_justified(|ui| match thumb_for(ctx, def, &p, 0.75, budget) {
                        Some(t) => {
                            ui.add(egui::Image::new(&t).shrink_to_fit());
                        }
                        None => {
                            ui.spinner();
                        }
                    });
                });
                ui.add_space(6.0);
                ui.allocate_ui(Vec2::new(cw - 22.0, 36.0), |ui| {
                    ui.set_min_height(36.0);
                    ui.label(theme::strong(e.name).size(13.0));
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    edit = ui.push_id(("ex_edit", k), |ui| accent_btn(ui, "Edit")).inner.on_hover_text("Load this example to change it").clicked();
                    png = ui.push_id(("ex_png", k), |ui| ghost(ui, "PNG")).inner.on_hover_text("Save this example as a PNG as it is").clicked();
                });
            });
        });
    (edit, png)
}

/// The gallery's PNG button: the example as a 2x PNG on white, through a save dialog.
fn export_example_png(def: &FigureDef, e: &figures::Example) -> Act {
    let p = example_params(def, e);
    let png = match figures::to_png(&(def.render)(&p), 2.0).and_then(|b| flatten_white(&b)) {
        Ok(b) => b,
        Err(err) => return Act::Toast(format!("Couldn't render the example: {err}"), true),
    };
    let slug: String = e.name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let slug = slug.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    let Some(path) = crate::platform::save_path(&format!("{}-{slug}.png", def.id), "PNG", &["png"]) else {
        return Act::None;
    };
    match crate::platform::write(&path, &png) {
        Ok(()) => Act::Toast(format!("Exported {}", path.display()), false),
        Err(err) => Act::Toast(format!("Export failed: {err}"), true),
    }
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
    /// Where the last preview drew the figure: top-left on screen and points per SVG unit
    /// (lets tests aim synthetic pointer events at a label).
    preview_map: Option<(egui::Pos2, f64)>,
    /// Share link: the paste box ("Open from link") and its error, in the toolbar menu and on
    /// the browser ("Open link…" toggles `link_open`).
    link_text: String,
    link_err: Option<String>,
    link_open: bool,
    /// A message for the host to show (set inside menus that can't return an `Act`).
    toast: Option<(String, bool)>,
    /// "Start from an example" gallery modal is open.
    examples_open: bool,
    /// Pedigree: the person selected for the Family card's person panel (index in drawing order).
    ped_sel: Option<usize>,
    /// Particle diagram: kinds whose card stays on show although they draw nothing (just added,
    /// or set to none) until removed.
    kinds_keep: [bool; 4],
    /// Preview zoom: the scale the shown texture was rasterized at, the scale the last layout
    /// asked for, the inked part of the figure (SVG units) and the part of the canvas on show.
    raster: f32,
    want_raster: f32,
    content: Option<[f64; 4]>,
    view: Option<[f64; 4]>,
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

    /// Spec text others can paste back in: {"vershion":"figure","id":..,"params":{..}}.
    fn spec_json(&self) -> Option<String> {
        let def = &reg()[self.sel?];
        let spec = spec_for(def.id, &self.params);
        serde_json::to_string(&serde_json::json!({"vershion": "figure", "id": spec.id, "params": spec.params})).ok()
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

    /// The offline share link of the figure on screen: `vershion://figure/<id>?p=<base64url>`
    /// holding only the params that differ from the figure's opening state.
    fn share_link(&self) -> Option<String> {
        let def = &reg()[self.sel?];
        Some(figures::share::encode(def.id, &self.params, &figures::start_params(def)))
    }

    /// Opens a pasted share link: switches to its figure and applies its params as one undo step.
    /// Params go through the same path as a saved spec (type checks, migrations, clamping).
    fn apply_link(&mut self, text: &str) -> Result<(), String> {
        let (id, changed) = figures::share::decode(text)?;
        let (i, def) = find_def(&id).ok_or_else(|| format!("this link is for a figure this version doesn't have (\"{id}\")"))?;
        let mut params: BTreeMap<String, serde_json::Value> =
            figures::start_params(def).0.iter().map(|(k, v)| (k.clone(), value_to_json(v))).collect();
        // an older link was made against an older opening state: its newer keys read as defaults
        let defs = (def.params)();
        for (k, was) in figures::start::older_values(&id, figures::share::version(text)) {
            if let Some(d) = defs.iter().find(|d| d.key == k) {
                params.insert(k.to_string(), value_to_json(&was.unwrap_or_else(|| d.default.clone())));
            }
        }
        params.extend(changed);
        let p = params_from_spec(def, &FigureSpec { id: id.clone(), params });
        self.imp = None;
        if self.sel != Some(i) {
            self.select(i);
        }
        self.set_params_with_history(p);
        Ok(())
    }

    /// Applies example `idx` of the open figure (the gallery's Edit button) as one undo step.
    fn use_example(&mut self, idx: usize) -> bool {
        let Some(si) = self.sel else { return false };
        let def = &reg()[si];
        let Some(e) = figures::examples_for(def.id).into_iter().nth(idx) else { return false };
        self.set_params_with_history(example_params(def, &e));
        true
    }

    fn select(&mut self, i: usize) {
        self.reset_history();
        self.examples_open = false;
        self.sel = Some(i);
        self.params = figures::start_params(&reg()[i]);
        // the composite shape opens on a blank canvas with its "Start with" screen, as the
        // reference does. Its opening state (share links, Reset all) keeps the arch, so links
        // made before stay the same; links and saved figures set their own spec over this.
        if reg()[i].id == "composite-shape" {
            self.params.set_s("spec", "");
        }
        self.comp = Default::default();
        self.ped_sel = None;
        self.kinds_keep = [false; 4];
        self.editing = None;
        self.shown = None;
        self.view = None;
        self.content = None;
        self.pending = None;
        self.err = None;
    }

    fn back(&mut self) {
        self.reset_history();
        self.ped_sel = None;
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
            // QA hook: VERSHION_FIG=<figure id> opens that figure straight away.
            if let Ok(id) = crate::env_var("VERSHION_FIG")
                && let Some((i, _)) = find_def(&id)
            {
                self.select(i);
                // VERSHION_FIG_PARAMS='{"spec":"rect 0 0 6 4"}' seeds its params (as VERSHION_RENDER_FIG does)
                if let Ok(j) = crate::env_var("VERSHION_FIG_PARAMS")
                    && let Ok(over) = serde_json::from_str::<serde_json::Value>(&j)
                {
                    self.params = figures::params_from_json(&reg()[i], &over);
                    self.reset_history();
                }
            }
            // QA hooks: VERSHION_FIG_LINK=<share link> opens a figure from a link (as "Open from
            // link" does); VERSHION_FIG_EXAMPLES=1 opens the example gallery; VERSHION_FIG_OPENLINK=1
            // shows the browser's "Open link" box.
            if let Ok(l) = crate::env_var("VERSHION_FIG_LINK")
                && let Err(e) = self.apply_link(&l)
            {
                eprintln!("VERSHION_FIG_LINK: {e}");
            }
            // VERSHION_PED_SEL / VERSHION_PED_DO: select a pedigree person and run panel actions
            self.ped_qa_hooks();
            if crate::env_var("VERSHION_FIG_EXAMPLES").as_deref() == Ok("1") {
                self.examples_open = true;
            }
            if crate::env_var("VERSHION_FIG_OPENLINK").as_deref() == Ok("1") {
                self.link_open = true;
            }
        }
        let act = if self.sel.is_some() || self.imp.is_some() {
            self.workspace_view(ui)
        } else {
            self.browser_view(ui)
        };
        match (act, self.toast.take()) {
            (Act::None, Some((t, err))) => Act::Toast(t, err),
            (a, _) => a,
        }
    }

    /// The paste box that reopens a share link (browser and toolbar menu).
    fn link_box(&mut self, ui: &mut Ui, width: f32) -> bool {
        let mut opened = false;
        ui.horizontal(|ui| {
            let r = ui.add(TextEdit::singleline(&mut self.link_text).hint_text("Paste a vershion://figure/… link").desired_width(width));
            let go = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.add_enabled(!self.link_text.trim().is_empty(), Button::new(RichText::new("Open from link").size(13.0))).clicked() || go {
                let t = self.link_text.clone();
                match self.apply_link(&t) {
                    Ok(()) => {
                        self.link_text.clear();
                        self.link_err = None;
                        self.link_open = false;
                        opened = true;
                    }
                    Err(e) => self.link_err = Some(e),
                }
            }
        });
        if let Some(e) = &self.link_err {
            ui.label(RichText::new(e).color(theme::red()).size(11.0));
        }
        opened
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
                if ghost(ui, "Open link…").on_hover_text("Reopen a figure from a share link someone copied").clicked() {
                    self.link_open = !self.link_open;
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
        if self.link_open {
            Frame::new()
                .fill(theme::card())
                .stroke(Stroke::new(1.0, theme::border()))
                .corner_radius(CornerRadius::same(crate::kit::R_CARD))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.label(theme::strong("Open a shared figure").size(13.0));
                    ui.label(theme::muted("Paste a link made with Share link → Copy link. It opens offline; the link holds the settings.").size(12.0));
                    ui.add_space(6.0);
                    let w = (ui.available_width() - 140.0).clamp(200.0, 560.0);
                    self.link_box(ui, w);
                });
            ui.add_space(8.0);
            if self.sel.is_some() {
                return Act::None;
            }
        }

        let q = self.search.trim().to_lowercase();
        let mut list: Vec<(usize, &FigureDef)> = reg()
            .iter()
            .enumerate()
            .filter(|(_, d)| self.subject.is_none_or(|s| figures::listed_under(d, s)))
            .filter(|(_, d)| {
                q.is_empty()
                    || d.title.to_lowercase().contains(&q)
                    || d.blurb.to_lowercase().contains(&q)
                    || d.id.contains(&q)
            })
            .collect();
        // cross-listed figures come after the subject's own, as on the reference home pages
        if let Some(s) = self.subject {
            list.sort_by_key(|(_, d)| d.subject != s);
        }

        let max_h = if self.modal {
            (crate::theme::area_or(ctx.viewport_rect()).height() * 0.55).clamp(220.0, 520.0)
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
                    && let Some(path) = crate::platform::pick_file("SVG / images", &["svg", "png", "jpg", "jpeg", "gif", "webp", "bmp"])
                {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    match crate::platform::read(&path) {
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
            // the preview is rasterized at the zoom it is shown at (see `preview_interactive`),
            // so a bigger window or a small figure stays crisp; exports keep their own scale
            let max_side = ctx.input(|i| i.max_texture_side) as f64;
            let fit = |want: f32, canvas: (f64, f64)| {
                let side = canvas.0.max(canvas.1);
                if side > 0.0 { want.min((max_side / side) as f32) } else { want }
            };
            let want = fit(if self.want_raster > 0.0 { self.want_raster } else { 1.5 }, self.canvas);
            let rescale = self.shown.is_some() && (want - self.raster).abs() > 0.01;
            let stale = rescale || self.shown.as_ref().is_none_or(|(sh, _)| *sh != h);
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
                if first || rescale || self.live || t0.elapsed() >= DEBOUNCE {
                    let svg = (def.render)(&self.params);
                    self.canvas = figures::svg_size(&svg);
                    let scale = fit(want, self.canvas);
                    match figures::to_rgba(&svg, scale) {
                        Ok((w, hh, px)) => {
                            let img = egui::ColorImage::from_rgba_premultiplied([w as usize, hh as usize], &px);
                            self.content = ink_box(&px, w, hh).map(|b| b.map(|v| v as f64 / scale as f64));
                            self.raster = scale;
                            let tex = ctx.load_texture(format!("testparrot:fig:{}", def.id), img, egui::TextureOptions::LINEAR);
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
            (crate::theme::area_or(ctx.viewport_rect()).height() * 0.55).clamp(260.0, 520.0)
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
        if self.examples_open && self.imp.is_none() && self.sel.is_some() {
            let a = self.examples_gallery(&ctx);
            if !matches!(a, Act::None) {
                act = a;
            }
        }
        act
    }

    /// "Start from an example": a modal grid of the figure's examples, each drawn by the figure's
    /// own renderer (textures cached by figure + params, a few rendered per frame), with Edit
    /// (load it here, one undo step) and PNG (save it as it is).
    fn examples_gallery(&mut self, ctx: &egui::Context) -> Act {
        let Some(si) = self.sel else { return Act::None };
        let def = &reg()[si];
        let ex = figures::examples_for(def.id);
        let (mut edit, mut png, mut close) = (None, None, false);
        let vw = crate::theme::area_or(ctx.viewport_rect()).width();
        let vh = crate::theme::area_or(ctx.viewport_rect()).height();
        let w = (vw - 120.0).clamp(300.0, 1000.0);
        let m = Modal::new(UiId::new(("testparrot_fig_examples", self.modal))).show(ctx, |ui| {
            ui.set_width(w);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(theme::serif_strong(format!("{} examples", def.title)).size(20.0));
                    ui.label(theme::muted("Pick one to edit it here, or save it as a PNG as it is.").size(12.0));
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    if ghost(ui, "Close").clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(10.0);
            if ex.is_empty() {
                ui.label(theme::muted("No examples for this figure yet."));
                return;
            }
            // the modal sizes itself from its content, so ask for the grid's height up front
            let cols_for = |avail: f32| ((avail + 12.0) / (EX_CARD_W + 12.0)).floor().clamp(1.0, 3.0) as usize;
            let rows = ex.len().div_ceil(cols_for(w - 14.0)) as f32;
            let max_h = (vh - 200.0).max(200.0);
            ScrollArea::vertical().id_salt("fig_examples_grid").max_height(max_h).min_scrolled_height((rows * EX_CARD_H).min(max_h)).auto_shrink([false, true]).show(ui, |ui| {
                let avail = ui.available_width() - 14.0;
                let cols = cols_for(avail);
                let cw = (avail - 12.0 * (cols as f32 - 1.0)) / cols as f32;
                let mut budget = 3u8;
                for (r, chunk) in ex.chunks(cols).enumerate() {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        for (j, e) in chunk.iter().enumerate() {
                            let k = r * cols + j;
                            let (ed, pn) = example_card(ui, ctx, def, e, k, &mut budget, cw);
                            if ed {
                                edit = Some(k);
                            }
                            if pn {
                                png = Some(k);
                            }
                        }
                    });
                    ui.add_space(12.0);
                }
            });
        });
        if close || m.should_close() {
            self.examples_open = false;
        }
        if let Some(k) = edit {
            self.use_example(k);
            self.examples_open = false;
        }
        if let Some(k) = png {
            return export_example_png(def, &ex[k]);
        }
        Act::None
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
        let n_ex = figures::examples_for(def.id).len();
        if ui
            .add_enabled(n_ex > 0, Button::new(RichText::new("Examples").size(13.0)))
            .on_hover_text("Start from an example: a gallery of ready-made figures")
            .on_disabled_hover_text("No examples for this figure yet")
            .clicked()
        {
            self.examples_open = true;
        }
        ui.menu_button("Share link", |ui| {
            ui.set_min_width(300.0);
            if ui.button("Copy link").on_hover_text("Copy a link that reopens this exact figure").clicked()
                && let Some(l) = self.share_link()
            {
                ui.ctx().copy_text(l);
                self.toast = Some(("Link copied. Share it, and anyone who opens it in verSHion sees this exact figure.".into(), false));
                ui.close();
            }
            ui.label(theme::muted("Works offline: the link itself holds the settings.").size(11.0));
            ui.separator();
            if self.link_box(ui, 190.0) {
                ui.close();
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
        let rs = if self.raster > 0.0 { self.raster } else { 1.5 };
        let (cw, ch) = if self.canvas.0 > 1.0 { self.canvas } else { ((nat.x / rs) as f64, (nat.y / rs) as f64) };
        // Zoom to the drawing, not the canvas: the inked box (plus a margin) fills the frame, as
        // the reference draws its figures. The view holds still while the pointer is down (a
        // drag or a slider must not rescale under the hand) and only refits when the drawing
        // leaves it or shrinks well inside it.
        let fresh = view_box(self.content, (cw, ch));
        let pointer_down = ui.input(|i| i.pointer.any_down());
        match self.view {
            Some(v) if pointer_down || keep_view(v, fresh) => {}
            _ => self.view = Some(fresh),
        }
        let v = self.view.unwrap_or(fresh);
        let (vw, vh) = ((v[2] - v[0]).max(1.0), (v[3] - v[1]).max(1.0));
        let k = (area.width() as f64 / vw).min(area.height() as f64 / vh).clamp(0.05, PREVIEW_MAX_ZOOM); // screen points per SVG unit
        let origin = area.center() - Vec2::new(((v[0] + vw / 2.0) * k) as f32, ((v[1] + vh / 2.0) * k) as f32);
        let img = egui::Rect::from_min_size(origin, Vec2::new((cw * k) as f32, (ch * k) as f32));
        ui.painter_at(full).image(tex.id(), img, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        if !pointer_down {
            let want = raster_for(k * ui.ctx().pixels_per_point() as f64, (cw, ch));
            if (want - rs).abs() / rs > 0.15 {
                self.want_raster = want;
                ui.ctx().request_repaint();
            }
        }
        let to_screen = |p: (f64, f64)| egui::pos2(img.left() + (p.0 * k) as f32, img.top() + (p.1 * k) as f32);
        let to_svg = |p: egui::Pos2| ((p.x - img.left()) as f64 / k, (p.y - img.top()) as f64 / k);
        let _ = ch;
        self.preview_map = Some((img.min, k));
        let pdefs = (def.params)();

        self.live = false;
        // pedigree people are selected by clicking them (with or without the handles shown)
        if def.id == "pedigree" {
            self.pedigree_canvas(ui, img, k, si);
        }
        if self.hide_handles {
            return;
        }
        let mut hs = def.handles(&self.params, (cw, ch));
        // Zones first so knobs on top win the hit test; movable labels last of all, so a press on
        // a label's own text moves the label (their hit box is the text, not the 9 pt minimum).
        hs.sort_by_key(|h| (h.label_drag, h.shape != Shape::Zone));
        let has_labels = hs.iter().any(|h| h.label_drag);
        let painter = ui.painter_at(full.expand(8.0));
        let acc = theme::accent_col();
        let mut changed = false;
        let mut start_edit: Option<(&'static str, String)> = None;
        for (i, h) in hs.iter().enumerate() {
            let c = to_screen(h.pos);
            let min_half = if h.label_drag { 5.0 } else { 9.0 };
            let hx = (h.half.0 * k).max(min_half) as f32;
            let hy = (h.half.1 * k).max(min_half) as f32;
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
            // grab offset from where the press began (not where the drag threshold was crossed),
            // so the dragged thing follows the pointer exactly
            if r.drag_started()
                && let Some(ptr) = ui.input(|i| i.pointer.press_origin()).or(r.interact_pointer_pos())
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
                        // a zone over much of the figure (a plot area, a particle box) gets only a
                        // light outline: a fill there would wash the whole figure out
                        let (fill, stroke) = zone_hover_style(hit.area(), img.area(), acc);
                        painter.rect(hit, 4.0, fill, stroke, egui::StrokeKind::Inside);
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
                if def.id == "pedigree" {
                    "Click someone to change them; double-click the title to rename it"
                } else if has_labels {
                    "Drag the dots to edit; drag a label to move it"
                } else {
                    "Drag the dots to edit; double-click a label to rename it"
                },
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
                    // the composite shape is built on its canvas: its typed rows sit behind Advanced
                    let rows_advanced = def.id == "composite-shape";
                    if let Some(card) = figures::rows_for(def.id).filter(|_| !rows_advanced) {
                        self.rows_card(ui, def.id, &pdefs, &card);
                    }
                    // the Classic setups dropdown goes in the figure's first card: an action-chips
                    // card ("Family") when there is one, else the first section
                    // free-body / vector / particle chips sit inside their item cards
                    let chips = figures::chips_for(def.id).filter(|_| !figures::cards::embeds_chips(def.id));
                    let classic_in_chips = chips.as_ref().is_some_and(|c| c.count_key.is_empty());
                    if let Some(card) = chips {
                        self.chips_card(ui, def.id, &pdefs, &card);
                    }
                    // the pedigree's Family card: setup, New pedigree and the person editor
                    let classic_in_chips = classic_in_chips || def.id == "pedigree";
                    if def.id == "pedigree" {
                        self.pedigree_card(ui);
                    }
                    if rows_advanced && let Some(card) = figures::rows_for(def.id) {
                        egui::CollapsingHeader::new(RichText::new("Advanced: spec").size(12.0))
                            .id_salt(("fig_rows_adv", def.id))
                            .default_open(false)
                            .show(ui, |ui| self.rows_card(ui, def.id, &pdefs, &card));
                        ui.add_space(6.0);
                    }
                    let secs = figures::layout::sections(def.id, &pdefs, n_now);
                    for (si_, sec) in secs.iter().enumerate() {
                        let shown: Vec<usize> = sec.items.iter().copied().filter(|&i| !is_hidden(&pdefs, &self.params, &pdefs[i]) && figures::param_relevant(def.id, pdefs[i].key, &self.params)).collect();
                        if shown.is_empty() {
                            continue;
                        }
                        let id = ui.make_persistent_id(("fig_sec", def.id, sec.title.as_str()));
                        let relevant: Vec<usize> = sec.items.iter().copied().filter(|&i| figures::param_relevant(def.id, pdefs[i].key, &self.params)).collect();
                        let summary = sec.items.iter().find_map(|&i| figures::cards::summary(def.id, pdefs[i].key, &self.params)).unwrap_or_else(|| section_summary(&pdefs, &self.params, &relevant));
                        let mut reset = false;
                        let (mut reset_labels, mut straighten) = (false, false);
                        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, si_ < open_sections(def.id))
                            .show_header(ui, |ui| {
                                ui.vertical(|ui| {
                                    // leave room for Reset on the right: a long summary wraps
                                    // short of it instead of running underneath
                                    ui.set_max_width((ui.available_width() - SECTION_RESET_W).max(80.0));
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
                                let help = figures::sections::live_help(def.id, &sec.title, &sec.help, &self.params);
                                if !help.is_empty() {
                                    ui.label(theme::muted(help).size(11.0));
                                    ui.add_space(4.0);
                                }
                                if si_ == 0 && !classic_in_chips {
                                    self.classic_combo(ui, def.id);
                                }
                                for &i in &shown {
                                    if pdefs[i].key == figures::labels::KEY {
                                        // the offsets param is never typed: dragging sets it, this clears it. Always
                                        // shown (as the reference does), greyed out until a label has moved.
                                        let moved = figures::labels::any_moved(&self.params);
                                        let b = ui.add_enabled(moved, egui::Button::new(RichText::new("Reset label positions").size(12.0)));
                                        let b = if moved { b.on_hover_text("Put every dragged label back in its own place") } else { b.on_disabled_hover_text("Drag a label on the figure to move it; this puts them back") };
                                        if b.clicked() {
                                            reset_labels = true;
                                        }
                                        continue;
                                    }
                                    if self.card_control(ui, def.id, pdefs[i].key, &pdefs) {
                                        ui.add_space(6.0);
                                        continue;
                                    }
                                    let old = self.params.0.get(pdefs[i].key).cloned();
                                    param_control(ui, &pdefs[i], &mut self.params);
                                    if let Some(old) = old.filter(|o| self.params.0.get(pdefs[i].key) != Some(o)) {
                                        figures::after_panel_change(def.id, pdefs[i].key, &old, &mut self.params);
                                    }
                                    self.card_after(ui, def.id, pdefs[i].key, &pdefs);
                                    if pdefs[i].key == "rotate"
                                        && figures::labels::turnable(def.id)
                                        && ui.add_enabled(self.params.f("rotate") != 0.0, Button::new(RichText::new("Straighten").size(12.0))).on_hover_text("Turn the figure back to 0°").clicked()
                                    {
                                        straighten = true;
                                    }
                                    if figures::random_key(def.id) == Some(pdefs[i].key)
                                        && ui.button(RichText::new("Random").size(12.0)).on_hover_text("Set a random reading").clicked()
                                    {
                                        figures::random_reading(def.id, &mut self.params, random_seed());
                                    }
                                    ui.add_space(6.0);
                                }
                                ui.add_space(4.0);
                            });
                        if reset_labels {
                            let mut p = self.params.clone();
                            figures::labels::reset(&mut p);
                            self.set_params_with_history(p);
                        }
                        if straighten {
                            let mut p = self.params.clone();
                            p.set_f("rotate", 0.0);
                            self.set_params_with_history(p);
                        }
                        if reset {
                            let start = figures::start_params(def);
                            for &i in &sec.items {
                                let k = pdefs[i].key;
                                self.params.0.insert(k.to_string(), start.0.get(k).cloned().unwrap_or_else(|| pdefs[i].default.clone()));
                            }
                            let anchors: Vec<&str> = sec.items.iter().map(|&i| pdefs[i].key).collect();
                            self.reset_card_keys(def.id, &anchors, &pdefs, &start);
                        }
                        ui.separator();
                    }
                    faq_section(ui, def.id);
                });
            ui.add_space(4.0);
            if ghost(ui, "Reset all").clicked() {
                self.params = figures::start_params(def);
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
    fn chips_card(&mut self, ui: &mut Ui, fig: &str, pdefs: &[ParamDef], card: &figures::ChipsCard) {
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
                if !slotted {
                    ui.add_space(6.0);
                    self.classic_combo(ui, fig);
                }
            });
        ui.add_space(8.0);
    }

    /// The reference's "Classic setups" dropdown (population growth, Punnett square, pedigree):
    /// picking one starts the figure over from that setup, as one undo step.
    fn classic_combo(&mut self, ui: &mut Ui, fig: &str) {
        let setups = figures::classic_setups(fig);
        let Some(def) = self.sel.map(|si| &reg()[si]) else { return };
        if setups.is_empty() {
            return;
        }
        ui.label(theme::muted("Classic setups").size(11.0));
        let mut pick = None;
        egui::ComboBox::from_id_salt(("fig_classic", fig))
            .selected_text("Choose one…")
            .width(ui.available_width().min(260.0))
            .show_ui(ui, |ui| {
                for (i, e) in setups.iter().enumerate() {
                    if ui.selectable_label(false, e.name).clicked() {
                        pick = Some(i);
                    }
                }
            });
        if let Some(i) = pick {
            self.set_params_with_history(example_params(def, &setups[i]));
        }
        ui.add_space(6.0);
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
        let Some(path) = crate::platform::save_path(&format!("{stem}.{ext}"), label, &[ext])
        else {
            return Act::None;
        };
        let data: Vec<u8> = if svg { out.svg.unwrap_or_default().into_bytes() } else { out.png };
        match crate::platform::write(&path, &data) {
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

/// Width kept free for a section header's Reset button.
const SECTION_RESET_W: f32 = 44.0;

/// How many of a figure's generic sections open expanded. The first two, except under the
/// pedigree's Family card, where the reference keeps Symbols, Labels and Chart title collapsed.
fn open_sections(fig: &str) -> usize {
    match fig {
        "pedigree" => 0,
        _ => 2,
    }
}

/// Hover look of a click/scroll zone covering `zone` of the preview's `img` area (both in
/// points²): small zones are tinted, large ones (over a quarter of the figure) only outlined.
fn zone_hover_style(zone: f32, img: f32, acc: egui::Color32) -> (egui::Color32, Stroke) {
    if img > 0.0 && zone > img * 0.25 {
        (egui::Color32::TRANSPARENT, Stroke::new(1.0, acc.gamma_multiply(0.55)))
    } else {
        (theme::accent_faded().gamma_multiply(0.6), Stroke::new(1.5, acc))
    }
}

/// The figure's FAQ at the bottom of its panel: one collapsed "FAQ" header holding a collapsed
/// header per question, its answer underneath. Nothing for a figure without one.
fn faq_section(ui: &mut Ui, fig: &str) {
    let faq = figures::faq::faq_for(fig);
    if faq.is_empty() {
        return;
    }
    let id = ui.make_persistent_id(("fig_faq", fig));
    // QA hook: VERSHION_FIG_FAQ=1 opens the FAQ and its first answer and scrolls them into view
    let qa = crate::env_var("VERSHION_FIG_FAQ").is_ok_and(|v| v == "1");
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, qa)
        .show_header(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(theme::strong("FAQ").size(13.0));
                ui.label(theme::muted(format!("{} questions about this figure", faq.len())).size(11.0));
            });
        })
        .body(|ui| {
            // questions wrap inside the panel (a collapsing header's title would widen it)
            for (i, e) in faq.iter().enumerate() {
                let qid = ui.make_persistent_id(("fig_faq_q", fig, i));
                let open = ui.data(|d| d.get_temp::<bool>(qid)).unwrap_or(qa && i == 0);
                let q = format!("{} {}", if open { "\u{25be}" } else { "\u{25b8}" }, e.q);
                let r = ui.add(egui::Label::new(RichText::new(q).size(12.0).strong()).wrap().sense(Sense::click()));
                if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    ui.data_mut(|d| d.insert_temp(qid, !open));
                }
                if open {
                    ui.indent(qid, |ui| {
                        ui.add(egui::Label::new(RichText::new(e.a).size(12.0).color(theme::muted_col())).wrap());
                    });
                }
                ui.add_space(4.0);
            }
            ui.add_space(4.0);
            if qa {
                ui.scroll_to_cursor(Some(Align::BOTTOM));
            }
        });
}

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
        if is_hidden(defs, p, d) || d.key == figures::labels::KEY {
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
    if items.iter().any(|&i| defs[i].key == figures::labels::KEY) && figures::labels::any_moved(p) {
        parts.push("labels moved".into());
    }
    parts.join(" · ")
}

fn fmt_measure(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A named starting point (example or classic setup): the figure's defaults with its overrides.
fn example_params(def: &FigureDef, e: &figures::Example) -> Params {
    let defs = (def.params)();
    let mut p = Params::defaults(&defs);
    for (k, v) in &e.params {
        p.0.insert((*k).to_string(), v.clone());
    }
    p.normalize(&defs, false);
    p
}

/// A fresh seed for the Random reading buttons.
fn random_seed() -> u64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7)
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
                let t = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.subsec_nanos() as i64).unwrap_or(7);
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
            ui.set_max_width((crate::theme::area_or(ctx.viewport_rect()).width() - 80.0).max(560.0));
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
        ws.apply_spec_text(&format!(r#"{{"vershion":"figure","id":"{id}","params":{{"formula":"NH3"}}}}"#)).unwrap_or_else(|e| panic!("{e}"));
        let after = hash_params(id, &ws.params);
        assert_ne!(before, after, "the pasted formula must change the params");
        ws.step_history(true);
        assert_eq!(hash_params(id, &ws.params), before, "undo restores the previous params");
        ws.step_history(false);
        assert_eq!(hash_params(id, &ws.params), after, "redo reapplies the pasted params");
    }

    /// Pointer drag on the figure preview from SVG point `a` to `b` (one press, four moves, release).
    fn preview_drag(ctx: &egui::Context, ws: &mut Workspace, a: (f64, f64), b: (f64, f64)) {
        let (o, k) = ws.preview_map.expect("preview drawn");
        let sc = |q: (f64, f64)| egui::pos2(o.x + (q.0 * k) as f32, o.y + (q.1 * k) as f32);
        let (pa, pb) = (sc(a), sc(b));
        comp_frame(ctx, ws, vec![egui::Event::PointerMoved(pa)]);
        comp_frame(ctx, ws, btn(pa, true));
        for i in 1..=4 {
            comp_frame(ctx, ws, vec![egui::Event::PointerMoved(pa + (pb - pa) * (i as f32 / 4.0))]);
        }
        comp_frame(ctx, ws, btn(pb, false));
        comp_frame(ctx, ws, vec![]);
    }

    fn open_fig(id: &str) -> (egui::Context, Workspace) {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut ws = Workspace::new(false);
        ws.select(find_def(id).unwrap().0);
        for _ in 0..3 {
            comp_frame(&ctx, &mut ws, vec![]);
        }
        (ctx, ws)
    }

    /// Dragging a label on the preview moves only that label (offset param), as one undo step,
    /// on a flat shape and on a solid; vertex knobs keep working next to a moved label.
    #[test]
    fn dragging_a_label_moves_it_as_one_undo_step() {
        for (id, lbl) in [("triangle", "s0"), ("rectangle", "v2"), ("cone", "h")] {
            let (ctx, mut ws) = open_fig(id);
            let def = &reg()[ws.sel.unwrap()];
            let before = ws.params.clone();
            let b = figures::labels::boxes(&(def.render)(&ws.params)).into_iter().find(|b| b.id == lbl).unwrap_or_else(|| panic!("{id}: no label {lbl}"));
            let c = b.centre();
            preview_drag(&ctx, &mut ws, c, (c.0 + 30.0, c.1 + 20.0));
            let off = figures::labels::get(&ws.params, lbl);
            assert!((off.0 - 30.0).abs() < 1.5 && (off.1 - 20.0).abs() < 1.5, "{id}: offset {off:?} ({})", ws.params.s(figures::labels::KEY));
            for (k, v) in &before.0 {
                if k != figures::labels::KEY {
                    assert_eq!(ws.params.0.get(k), Some(v), "{id}: {k} changed by a label drag");
                }
            }
            // the moved label is drawn (and hit-tested) at its new place
            let moved = figures::labels::boxes(&(def.render)(&ws.params)).into_iter().find(|b| b.id == lbl).unwrap();
            assert!((moved.centre().0 - c.0 - off.0).abs() < 1e-6);
            ws.step_history(true);
            assert_eq!(ws.params.s(figures::labels::KEY), "", "{id}: undo puts the label back");
            assert!(ws.undo.is_empty(), "{id}: the whole drag is one undo step");
            ws.step_history(false);
            assert_eq!(figures::labels::get(&ws.params, lbl), off, "{id}: redo moves it again");
        }
        // a vertex knob next to a moved label still drags the vertex
        let (ctx, mut ws) = open_fig("rectangle");
        ws.params.set_s(figures::labels::KEY, "v2:20,-20");
        for _ in 0..2 {
            comp_frame(&ctx, &mut ws, vec![]);
        }
        let def = &reg()[ws.sel.unwrap()];
        let hs = def.handles(&ws.params, ws.canvas);
        let knob = hs.iter().find(|h| !h.label_drag && h.tip.contains("width")).expect("width knob").pos;
        let ab = ws.params.s("m_ab").to_string();
        preview_drag(&ctx, &mut ws, knob, (knob.0 - 60.0, knob.1));
        assert_ne!(ws.params.s("m_ab"), ab, "the width knob still works");
        assert_eq!(ws.params.s(figures::labels::KEY), "v2:20,-20", "and leaves the label alone");
    }

    /// Offsets survive a saved spec / preset round trip and reach every output (PNG and SVG).
    #[test]
    fn label_offsets_round_trip_and_reach_exports() {
        for id in figures::labels::FIGURES {
            let (i, def) = find_def(id).unwrap();
            let mut p = figures::start_params(def);
            let boxes = figures::labels::boxes(&(def.render)(&p));
            assert!(!boxes.is_empty(), "{id}: no movable labels by default");
            let first = boxes[0].id.clone();
            figures::labels::set(&mut p, &first, (25.0, -15.0));
            let spec = spec_for(id, &p);
            let json = serde_json::to_string(&spec).unwrap();
            let back: FigureSpec = serde_json::from_str(&json).unwrap();
            let q = params_from_spec(def, &back);
            assert_eq!(figures::labels::get(&q, &first), (25.0, -15.0), "{id}");
            let ws = Workspace { sel: Some(i), params: q, ..Default::default() };
            let out = ws.output().unwrap();
            assert!(out.svg.as_ref().unwrap().contains(&format!(r#"data-lbl="{first}""#)), "{id}");
            assert!(out.svg.as_ref().unwrap().contains("translate("), "{id}: svg export honours the offset");
            let plain = Workspace { sel: Some(i), params: figures::start_params(def), ..Default::default() }.output().unwrap();
            assert_ne!(out.png, plain.png, "{id}: png differs when a label is moved");
        }
        // an old spec without the key opens with no offsets
        let def = find_def("triangle").unwrap().1;
        let q = params_from_spec(def, &FigureSpec { id: "triangle".into(), params: Default::default() });
        assert_eq!(q.s(figures::labels::KEY), "");
    }

    /// Copy link -> Open from link reproduces the exact params of every figure's opening state,
    /// every example and a saved spec (incl. dragged label offsets), switching figures as needed.
    #[test]
    fn share_link_round_trips_every_figure_and_example() {
        for (i, def) in reg().iter().enumerate() {
            let mut cases = vec![figures::start_params(def)];
            cases.extend(figures::examples_for(def.id).iter().map(|e| example_params(def, e)));
            for p in cases {
                let src = Workspace { sel: Some(i), params: p.clone(), ..Default::default() };
                let link = src.share_link().unwrap();
                assert!(link.starts_with(&format!("vershion://figure/{}", def.id)), "{link}");
                let mut dst = Workspace::new(false);
                dst.select(find_def("waves").unwrap().0);
                dst.apply_link(&link).unwrap_or_else(|e| panic!("{}: {e} ({link})", def.id));
                assert_eq!(dst.sel, Some(i));
                assert_eq!(format!("{:?}", dst.params.0), format!("{:?}", p.0), "{} via {link}", def.id);
            }
        }
        // a moved label and typed text survive too; the link is one undo step
        let (i, def) = find_def("triangle").unwrap();
        let mut p = figures::start_params(def);
        figures::labels::set(&mut p, "s0", (12.5, -7.0));
        p.set_s("name_a", "Ω");
        let link = Workspace { sel: Some(i), params: p.clone(), ..Default::default() }.share_link().unwrap();
        let mut ws = Workspace::new(false);
        ws.select(i);
        let before = ws.params.clone();
        ws.apply_link(&link).unwrap();
        assert_eq!(figures::labels::get(&ws.params, "s0"), (12.5, -7.0));
        assert_eq!(ws.params.s("name_a"), "Ω");
        ws.step_history(true);
        assert_eq!(format!("{:?}", ws.params.0), format!("{:?}", before.0), "undo goes back");
    }

    #[test]
    fn share_link_errors_are_friendly() {
        let mut ws = Workspace::new(false);
        assert!(ws.apply_link("").is_err());
        assert!(ws.apply_link("hello").is_err());
        let e = ws.apply_link("vershion://figure/no-such-figure").unwrap_err();
        assert!(e.contains("no-such-figure"), "{e}");
        assert!(ws.sel.is_none(), "a bad link leaves the browser alone");
        // wrong-typed or out-of-range values fall back / clamp like a saved spec
        let (i, def) = find_def("waves").unwrap();
        let json = r#"{"amp":"loud","cycles":1e9}"#;
        let link = format!("vershion://figure/waves?p={}", figures::share::b64url_encode(json.as_bytes()));
        ws.apply_link(&link).unwrap();
        assert_eq!(ws.sel, Some(i));
        let start = figures::start_params(def);
        assert_eq!(ws.params.0.get("amp"), start.0.get("amp"));
        assert!(ws.params.f("cycles") < 1e9);
    }

    /// The example gallery draws each example once (cached textures, nothing re-rendered on later
    /// frames), and Edit loads an example as one undo step.
    #[test]
    fn example_gallery_caches_previews_and_edit_applies() {
        let (ctx, mut ws) = open_fig("motion-graphs");
        let def = &reg()[ws.sel.unwrap()];
        let ex = figures::examples_for(def.id);
        assert!(ex.len() >= 5);
        ws.examples_open = true;
        for _ in 0..(ex.len() + 4) {
            comp_frame(&ctx, &mut ws, vec![]);
        }
        let keys = |ws: &Workspace| {
            let _ = ws;
            ex.iter().map(|e| format!("{}:{:x}:0.75", def.id, hash_params(def.id, &example_params(def, e)))).collect::<Vec<_>>()
        };
        for k in keys(&ws) {
            assert!(THUMBS.with(|t| t.borrow().get(&k).is_some_and(|x| x.is_some())), "missing preview {k}");
        }
        let n = THUMB_RENDERS.with(|c| c.get());
        for _ in 0..4 {
            comp_frame(&ctx, &mut ws, vec![]);
        }
        assert_eq!(THUMB_RENDERS.with(|c| c.get()), n, "no re-render once cached");
        assert!(ws.examples_open);
        let before = ws.params.clone();
        assert!(ws.use_example(2));
        assert_eq!(format!("{:?}", ws.params.0), format!("{:?}", example_params(def, &ex[2]).0));
        ws.step_history(true);
        assert_eq!(format!("{:?}", ws.params.0), format!("{:?}", before.0));
        // a figure without examples has no gallery to fill
        let mut none = Workspace::new(false);
        none.select(find_def("demo-shapes").unwrap().0);
        assert!(!none.use_example(0));
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

    /// Label tool popovers keep clear of the Turn and zoom bars at the bottom right, and an
    /// interior angle's popover opens outside the shape instead of over it.
    #[test]
    fn composite_popovers_keep_clear_of_the_bars_and_the_shape() {
        let house = "rect 0 0 6 4; tri 0 4 6 4 3 7";
        let (ctx, mut ws) = comp_ws(house);
        ws.comp.tool = composite_canvas::Tool::Label;
        comp_frame(&ctx, &mut ws, vec![]);
        let area = |ctx: &egui::Context, id: &str| ctx.memory(|m| m.area_rect(egui::Id::new(id)));
        for corner in [(6.0, 0.0), (0.0, 0.0)] {
            ws.comp.angle = None;
            comp_click(&ctx, &mut ws, corner);
            assert_eq!(ws.comp.corner, Some(corner), "corner popover open");
            for _ in 0..3 {
                comp_frame(&ctx, &mut ws, vec![]);
            }
            let pop = area(&ctx, "comp_corner").expect("corner popover");
            for bar in ["comp_turn", "comp_zoom"] {
                let b = area(&ctx, bar).unwrap_or_else(|| panic!("{bar} drawn"));
                assert!(!pop.intersects(b), "{corner:?}: popover {pop:?} covers {bar} {b:?}");
            }
            ws.comp.corner = None;
        }
        // the right shoulder's 135° interior angle: popover outside the shape
        ws.comp.angle = Some(((6.0, 4.0), false));
        for _ in 0..3 {
            comp_frame(&ctx, &mut ws, vec![]);
        }
        let pop = area(&ctx, "comp_angle").expect("angle popover");
        let shoulder = ws.comp.to_screen((6.0, 4.0));
        let inside = ws.comp.to_screen((3.0, 3.0));
        assert!(pop.left() >= shoulder.x - 2.0 || !pop.contains(inside), "angle popover {pop:?} over the shape (corner {shoulder:?})");
        assert!(!pop.contains(inside));
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
        // a corner opens its Point popover, inside it the angle, outside it the exterior angle
        comp_click(&ctx, &mut ws, (0.0, 0.0));
        assert_eq!(ws.comp.corner, Some((0.0, 0.0)));
        let at = |ws: &Workspace, dx: f32, dy: f32| ws.comp.to_screen((0.0, 0.0)) + Vec2::new(dx, dy);
        let click_px = |ctx: &egui::Context, ws: &mut Workspace, p: egui::Pos2| {
            let mut ev = btn(p, true);
            ev.extend(btn(p, false));
            comp_frame(ctx, ws, ev);
            comp_frame(ctx, ws, vec![]);
        };
        ws.comp.corner = None; // closed (its popover may sit over the angle)
        comp_frame(&ctx, &mut ws, vec![]);
        let p = at(&ws, 22.0, -22.0);
        click_px(&ctx, &mut ws, p);
        assert_eq!((ws.comp.angle, ws.comp.corner), (Some(((0.0, 0.0), false)), None));
        ws.comp.angle = None;
        comp_frame(&ctx, &mut ws, vec![]);
        let p = at(&ws, -22.0, -22.0);
        click_px(&ctx, &mut ws, p);
        assert_eq!(ws.comp.angle, Some(((0.0, 0.0), true)));
        // a side opens the side popover
        ws.comp.angle = None;
        comp_frame(&ctx, &mut ws, vec![]);
        comp_click(&ctx, &mut ws, (2.0, 3.0));
        assert!(ws.comp.side.is_some_and(|s| (s.a.1 - 3.0).abs() < 1e-9 && (s.b.1 - 3.0).abs() < 1e-9), "{:?}", ws.comp.side);
        assert_eq!(ws.params.s("spec"), "rect 0 0 4 3; rect 4 0 2 2", "opening popovers edits nothing");
        ws.comp.tool = composite_canvas::Tool::Select;
        comp_click(&ctx, &mut ws, (5.0, 1.0));
        assert_eq!(ws.comp.sel, Some(1));
        let key = |pressed| egui::Event::Key { key: egui::Key::Delete, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
        comp_frame(&ctx, &mut ws, vec![key(true), key(false)]);
        assert_eq!(ws.params.s("spec"), "rect 0 0 4 3");
        assert_eq!(ws.comp.sel, None);
    }

    /// The Label tool shows the figure turned: clicks land on the turned corners, and the
    /// drawing tools show it straight again.
    #[test]
    fn composite_canvas_label_tool_turns_with_the_figure() {
        let (ctx, mut ws) = comp_ws("rect 0 0 4 2");
        ws.params.set_f("rotate", 90.0);
        ws.comp.tool = composite_canvas::Tool::Label;
        comp_frame(&ctx, &mut ws, vec![]);
        comp_frame(&ctx, &mut ws, vec![]);
        // the rectangle turns a quarter about its middle (2, 1): (4, 0) goes to (3, 3)
        let s = ws.comp.to_screen((4.0, 0.0));
        let rect_mid = ws.comp.to_screen((2.0, 1.0));
        assert!((s.x - rect_mid.x) > 0.0 && (s.y - rect_mid.y) < -10.0, "turned up: {s:?} vs {rect_mid:?}");
        let mut ev = btn(s, true);
        ev.extend(btn(s, false));
        comp_frame(&ctx, &mut ws, ev);
        comp_frame(&ctx, &mut ws, vec![]);
        assert_eq!(ws.comp.corner, Some((4.0, 0.0)));
        ws.comp.tool = composite_canvas::Tool::Select;
        comp_frame(&ctx, &mut ws, vec![]);
        let s2 = ws.comp.to_screen((4.0, 0.0));
        assert!((s2.y - ws.comp.to_screen((2.0, 1.0)).y) > 10.0, "straight again outside the Label tool");
        assert_eq!(ws.params.f("rotate"), 90.0, "the turn is kept");
    }

    /// A dimension label on the composite canvas drags like a math label: the row gains an
    /// `@DX,DY` offset (one undo step), the export draws it there, and a plain click on it in the
    /// Label tool still opens its side's popover.
    #[test]
    fn composite_canvas_drags_a_dimension_label() {
        let (ctx, mut ws) = comp_ws("rect 0 0 6 4; dim 0 0 6 0 6 cm");
        ws.comp.tool = composite_canvas::Tool::Label;
        comp_frame(&ctx, &mut ws, vec![]);
        let before = (reg()[ws.sel.unwrap()].render)(&ws.params);
        let (i, at) = ws.comp.label_centres(&ws.params)[0];
        assert_eq!(i, 1, "the dim row's label");
        let to = at + Vec2::new(40.0, -60.0);
        comp_frame(&ctx, &mut ws, btn(at, true));
        for k in 1..=4 {
            comp_frame(&ctx, &mut ws, vec![egui::Event::PointerMoved(at + (to - at) * (k as f32 / 4.0))]);
        }
        comp_frame(&ctx, &mut ws, btn(to, false));
        comp_frame(&ctx, &mut ws, vec![]);
        let spec = ws.params.s("spec").to_string();
        assert!(spec.contains("dim 0 0 6 0 6 cm @") || spec.contains("dim 0 0 6 0 @"), "{spec}");
        let it = &figures::composite::parse(&spec)[1];
        let o = it.off.expect("offset stored");
        assert!(o.0 > 0.0 && o.1 < 0.0, "moved right and up: {o:?}");
        let now = ws.comp.label_centres(&ws.params)[0].1;
        assert!(now.distance(to) < 3.0, "the label follows the pointer: {now:?} vs {to:?}");
        assert_ne!((reg()[ws.sel.unwrap()].render)(&ws.params), before, "exports draw the moved label");
        assert!(!ws.undo.is_empty() || ws.hist_base.is_some(), "an undo step");
        ws.step_history(true);
        assert!(!ws.params.s("spec").contains('@'), "undo puts it back");
        // a click (no move) on the label in the Label tool opens the side's popover
        comp_frame(&ctx, &mut ws, vec![]);
        let at = ws.comp.label_centres(&ws.params)[0].1;
        let mut ev = btn(at, true);
        ev.extend(btn(at, false));
        comp_frame(&ctx, &mut ws, ev);
        comp_frame(&ctx, &mut ws, vec![]);
        let sd = ws.comp.side.expect("the side's popover opens");
        assert_eq!((sd.a, sd.b), ((0.0, 0.0), (6.0, 0.0)));
        assert!(!ws.params.s("spec").contains('@'), "a click moves nothing");
    }

    /// The preview zooms to the drawing: a small molecule fills the frame well past the old
    /// "natural size" cap, the zoom is crisp (re-rasterized), and the view holds while dragging.
    #[test]
    fn preview_zooms_to_the_drawing() {
        let (ctx, mut ws) = open_fig("lewis-structures");
        for _ in 0..3 {
            comp_frame(&ctx, &mut ws, vec![]);
        }
        let (_, k) = ws.preview_map.expect("preview drawn");
        assert!(k > 1.6, "zoomed past the old 1.5x cap: {k}");
        assert!(ws.raster > 1.6, "re-rasterized at the zoom: {}", ws.raster);
        let c = ws.content.expect("inked box");
        let v = ws.view.expect("view");
        assert!(v[0] <= c[0] && v[2] >= c[2] && v[1] <= c[1] && v[3] >= c[3], "view holds the drawing");
    }

    #[test]
    fn preview_zoom_helpers() {
        // a 10x10 white raster with a 2x3 black block at (4, 5)
        let (w, h) = (10u32, 10u32);
        let mut px = vec![255u8; (w * h * 4) as usize];
        assert_eq!(ink_box(&px, w, h), None);
        for y in 5..8 {
            for x in 4..6 {
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        assert_eq!(ink_box(&px, w, h), Some([4, 5, 6, 8]));
        // the view pads the drawing and stays on the canvas
        let v = view_box(Some([100.0, 100.0, 200.0, 150.0]), (400.0, 300.0));
        assert!(v[0] < 100.0 && v[2] > 200.0 && v[0] >= 0.0 && v[3] <= 300.0);
        assert_eq!(view_box(None, (400.0, 300.0)), [0.0, 0.0, 400.0, 300.0]);
        // small edits keep the view; a drawing that leaves it or shrinks well inside refits
        let old = [0.0, 0.0, 100.0, 100.0];
        assert!(keep_view(old, [2.0, 2.0, 98.0, 98.0]));
        assert!(!keep_view(old, [2.0, 2.0, 120.0, 98.0]));
        assert!(!keep_view(old, [40.0, 40.0, 60.0, 60.0]));
        // raster scale: quarter steps, at least 1, capped by the texture size
        assert_eq!(raster_for(2.1, (400.0, 300.0)), 2.25);
        assert_eq!(raster_for(0.3, (400.0, 300.0)), 1.0);
        assert!(raster_for(50.0, (2000.0, 2000.0)) <= 2.25);
    }

    /// The FAQ sits at the bottom of every figure's panel (and the panel stays its width).
    #[test]
    fn every_figure_panel_has_a_faq() {
        for d in figures::registry() {
            assert!(!figures::faq::faq_for(d.id).is_empty(), "{}", d.id);
        }
        // the panel builds with the FAQ in it, without growing past its 290 pt column
        let (ctx, mut ws) = open_fig("pulley");
        comp_frame(&ctx, &mut ws, vec![]);
        let (_, k) = ws.preview_map.expect("preview drawn beside the panel");
        assert!(k > 0.0);
    }

    /// The pulley shows only its setup's controls; switching setups keeps hidden values.
    #[test]
    fn pulley_controls_follow_the_setup() {
        let d = figures::registry().into_iter().find(|d| d.id == "pulley").unwrap();
        let mut p = figures::start_params(&d);
        let rel = |p: &Params, k: &str| figures::param_relevant("pulley", k, p);
        assert!(rel(&p, "lower") && rel(&p, "third") && !rel(&p, "angle") && !rel(&p, "surface") && !rel(&p, "strands") && !rel(&p, "t_lab"));
        p.set_b("third", true);
        assert!(rel(&p, "t_lab") && rel(&p, "third_side"));
        p.set_s("setup", "Ramp");
        assert!(rel(&p, "angle") && rel(&p, "surface") && rel(&p, "friction") && rel(&p, "l_kind") && !rel(&p, "third") && !rel(&p, "t_lab"));
        p.set_s("setup", "Table");
        assert!(!rel(&p, "angle") && rel(&p, "normal") && !rel(&p, "s_lab"));
        p.set_b("second", true);
        assert!(rel(&p, "s_lab"));
        p.set_s("setup", "Block & tackle");
        assert!(rel(&p, "strands") && !rel(&p, "r_lab") && !rel(&p, "surface") && rel(&p, "tension") && rel(&p, "l_lab"));
        assert!(p.b("third"), "hidden values are kept");
        assert!(figures::param_relevant("waves", "anything", &p));
    }

    /// Share links are v2 now; a v1 link (no `v=`) of a vector diagram keeps the solid resultant
    /// it was made with, while a new one opens dashed like the reference.
    #[test]
    fn share_links_v1_keep_their_opening_state() {
        let mut ws = Workspace::new(false);
        let (i, def) = find_def("vector-diagram").unwrap();
        ws.select(i);
        assert_eq!(ws.params.s("res_style"), "Dashed", "opens dashed");
        let link = ws.share_link().unwrap();
        assert!(link.ends_with(&format!("v={}", figures::share::VERSION)), "{link}");
        ws.apply_link(&link).unwrap();
        assert_eq!(ws.params.s("res_style"), "Dashed");
        // a v1 link: bare, or with only other params
        ws.apply_link("vershion://figure/vector-diagram").unwrap();
        assert_eq!(ws.params.s("res_style"), "Drawn", "v1 links keep the old opening state");
        let v1 = format!("vershion://figure/vector-diagram?p={}", figures::share::b64url_encode(br#"{"n":3}"#));
        ws.apply_link(&v1).unwrap();
        assert_eq!((ws.params.s("res_style"), ws.params.i("n")), ("Drawn", 3));
        let v1d = format!("vershion://figure/vector-diagram?p={}", figures::share::b64url_encode(br#"{"res_style":"Left off"}"#));
        ws.apply_link(&v1d).unwrap();
        assert_eq!(ws.params.s("res_style"), "Left off", "what the sender set still wins");
        let _ = def;
    }

    /// Links written before the P4 opening states (v2, and v1) open exactly as they did: the new
    /// opening keys read as the plain defaults. Links made now (v3) keep the new opening state.
    #[test]
    fn share_links_v2_keep_their_opening_state() {
        let cases: [(&str, &str); 8] = [
            ("length-reading", "mag_show"),
            ("orbital-diagram", "config"),
            ("angles", "letters"),
            ("parallel-lines", "marked"),
            ("parallel-lines", "unknown"),
            ("volume-by-displacement", "arrow"),
            ("ph-reading", "meter"),
            ("kite", "flip"),
        ];
        let mut ws = Workspace::new(false);
        for (id, key) in cases {
            let (i, def) = find_def(id).unwrap();
            let mut plain = Params::defaults(&(def.params)());
            for (k, was) in figures::start::older_values(id, 2) {
                if let Some(v) = was {
                    plain.0.insert(k.to_string(), v);
                }
            }
            let start = figures::start_params(def);
            assert_ne!(plain.0.get(key), start.0.get(key), "{id}.{key} is a new opening state");
            ws.select(i);
            let now = ws.share_link().unwrap();
            ws.apply_link(&now).unwrap();
            assert_eq!(ws.params.0.get(key), start.0.get(key), "{id}: a v3 link keeps the opening {key}");
            for old in [format!("vershion://figure/{id}?v=2"), format!("vershion://figure/{id}")] {
                ws.apply_link(&old).unwrap();
                assert_eq!(ws.params.0.get(key), plain.0.get(key), "{id}: {old} keeps the old {key}");
                // the opening state of round 6: today's, with the P4 keys at their plain defaults
                let mut was = start.clone();
                for k in figures::start::newer_than(id, 2) {
                    was.0.insert(k.to_string(), plain.0[k].clone());
                }
                assert_eq!(format!("{:?}", ws.params.0), format!("{:?}", was.0), "{id}: {old} opens as before");
            }
            // what a v2 sender changed still wins
            let v2 = format!("vershion://figure/{id}?p={}&v=2", figures::share::b64url_encode(format!("{{\"title\":\"T\",\"{key}\":{}}}", value_to_json(start.0.get(key).unwrap())).as_bytes()));
            ws.apply_link(&v2).unwrap();
            assert_eq!(ws.params.0.get(key), start.0.get(key), "{id}: sender's {key} wins");
        }
    }

    /// The composite shape opens on a blank canvas (its Start with screen), while its opening
    /// state, so every share link made before, keeps the arch.
    #[test]
    fn composite_opens_blank_but_old_links_keep_their_shape() {
        let (i, def) = find_def("composite-shape").unwrap();
        let mut ws = Workspace::new(false);
        ws.select(i);
        assert_eq!(ws.params.s("spec"), "");
        let arch = figures::start_params(def).s("spec").to_string();
        assert!(arch.starts_with("rect 0 0 6 4; semi 3 4 3 u"), "{arch}");
        // a bare link (made from an untouched figure before) still opens the arch
        ws.apply_link("vershion://figure/composite-shape").unwrap();
        assert_eq!(ws.params.s("spec"), arch);
        // a link that changed only the unit keeps the arch too, and the new settings default
        let mut p = figures::start_params(def);
        p.set_s("unit", "m");
        let link = figures::share::encode("composite-shape", &p, &figures::start_params(def));
        let mut ws2 = Workspace::new(false);
        ws2.select(i);
        ws2.apply_link(&link).unwrap();
        assert_eq!((ws2.params.s("spec"), ws2.params.s("unit"), ws2.params.f("rotate"), ws2.params.i("angle_dec")), (arch.as_str(), "m", 0.0, 0));
        // a blank canvas round-trips through its own link
        let mut bp = figures::start_params(def);
        bp.set_s("spec", "");
        let blank = figures::share::encode("composite-shape", &bp, &figures::start_params(def));
        let mut ws3 = Workspace::new(false);
        ws3.apply_link(&blank).unwrap();
        assert_eq!(ws3.params.s("spec"), "");
    }

    /// Runs a frame with AccessKit on and clicks the widget labelled `label` (the canvas'
    /// popovers and Start with chips are found by their names, as a screen reader would).
    fn click_named(ctx: &egui::Context, ws: &mut Workspace, label: &str) {
        ctx.enable_accesskit();
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1400.0, 900.0))), ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| {
            let _ = ws.show(ui);
        });
        out.textures_delta.clear();
        let tree = out.platform_output.accesskit_update.expect("accesskit tree");
        // the canvas' own widget when the side panel has one of the same name (rightmost)
        let b = tree
            .nodes
            .iter()
            .filter(|(_, n)| n.label().is_some_and(|l| l == label))
            .filter_map(|(_, n)| n.bounds())
            .max_by(|a, b| a.x0.total_cmp(&b.x0))
            .unwrap_or_else(|| panic!("no widget named {label:?}"));
        let p = egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
        comp_frame(ctx, ws, vec![egui::Event::PointerMoved(p)]);
        let mut ev = btn(p, true);
        ev.extend(btn(p, false));
        comp_frame(ctx, ws, ev);
        comp_frame(ctx, ws, vec![]);
    }

    /// The Label tool's popovers write their rows into the spec, each click one undo step.
    #[test]
    fn composite_canvas_popovers_write_the_spec_with_undo() {
        let (ctx, mut ws) = comp_ws("rect 0 0 6 4");
        ws.comp.tool = composite_canvas::Tool::Label;
        comp_click(&ctx, &mut ws, (3.0, 0.0));
        assert!(ws.comp.side.is_some());
        click_named(&ctx, &mut ws, "Dashed");
        assert_eq!(ws.params.s("spec"), "rect 0 0 6 4; side 0 0 6 0 dashed");
        click_named(&ctx, &mut ws, "Thick");
        assert_eq!(ws.params.s("spec"), "rect 0 0 6 4; side 0 0 6 0 dashed thick");
        ws.step_history(true);
        assert_eq!(ws.params.s("spec"), "rect 0 0 6 4; side 0 0 6 0 dashed");
        ws.step_history(true);
        assert_eq!(ws.params.s("spec"), "rect 0 0 6 4");
        ws.step_history(false);
        ws.step_history(false);
        assert_eq!(ws.params.s("spec"), "rect 0 0 6 4; side 0 0 6 0 dashed thick");
        // the angle inside the bottom-left corner: its measure, then its right-angle square
        ws.comp.side = None;
        comp_frame(&ctx, &mut ws, vec![]);
        let p = ws.comp.to_screen((0.0, 0.0)) + Vec2::new(24.0, -24.0);
        let mut ev = btn(p, true);
        ev.extend(btn(p, false));
        comp_frame(&ctx, &mut ws, ev);
        comp_frame(&ctx, &mut ws, vec![]);
        assert_eq!(ws.comp.angle, Some(((0.0, 0.0), false)));
        click_named(&ctx, &mut ws, "Measure");
        assert!(ws.params.s("spec").ends_with("; angle 0 0 measure"), "{}", ws.params.s("spec"));
        click_named(&ctx, &mut ws, "\u{221f} Square");
        assert!(ws.params.s("spec").ends_with("; angle 0 0 measure; ra 0 0 NE"), "{}", ws.params.s("spec"));
        ws.step_history(true);
        assert!(!ws.params.s("spec").contains("ra 0 0"));
        ws.step_history(false);
        // the corner's point
        ws.comp.angle = None;
        comp_frame(&ctx, &mut ws, vec![]);
        comp_click(&ctx, &mut ws, (6.0, 4.0));
        assert_eq!(ws.comp.corner, Some((6.0, 4.0)));
        click_named(&ctx, &mut ws, "\u{25cf} Show");
        assert!(ws.params.s("spec").ends_with("; point 6 4"), "{}", ws.params.s("spec"));
        // the turn and Straighten
        ws.params.set_f("rotate", 30.0);
        comp_frame(&ctx, &mut ws, vec![]);
        click_named(&ctx, &mut ws, "Straighten");
        assert_eq!(ws.params.f("rotate"), 0.0);
        ws.step_history(true);
        assert_eq!(ws.params.f("rotate"), 30.0);
    }

    /// An empty canvas shows the Start with chips; a chip builds that shape as one undo step.
    #[test]
    fn composite_start_screen_chips_build_a_shape() {
        let (ctx, mut ws) = comp_ws("");
        click_named(&ctx, &mut ws, "House");
        assert_eq!(ws.params.s("spec"), figures::math::preset_spec("House").unwrap());
        ws.step_history(true);
        assert_eq!(ws.params.s("spec"), "");
        ws.step_history(false);
        assert_eq!(ws.params.s("spec"), figures::math::preset_spec("House").unwrap());
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

    /// Pedigree: clicking a person on the preview selects them; each panel edit is one undo
    /// step stored in the `family` param (so exports and share links carry it); old typed
    /// overrides fold into the family on the first edit; Remove clears the selection.
    #[test]
    fn pedigree_click_selects_and_panel_edits_are_undo_steps() {
        use crate::figures::biology::pedigree as ped;
        let (ctx, mut ws) = open_fig("pedigree");
        ws.params.set_s("changes", "I-1=deceased");
        ws.reset_history();
        for _ in 0..3 {
            comp_frame(&ctx, &mut ws, vec![]);
        }
        let (_, _, whos) = ped::people_of(&ws.params);
        let target = whos.iter().position(|w| w.gi == 2).unwrap();
        let (o, k) = ws.preview_map.expect("preview drawn");
        let at = egui::pos2(o.x + (whos[target].x * k) as f32, o.y + (whos[target].y * k) as f32);
        let mut ev = btn(at, true);
        ev.extend(btn(at, false));
        comp_frame(&ctx, &mut ws, ev);
        comp_frame(&ctx, &mut ws, vec![]);
        assert_eq!(ws.ped_sel, Some(target), "a click selects");
        assert!(ws.params.s("family").is_empty(), "selecting edits nothing");
        let start = ws.params.clone();
        let n0 = whos.len();
        ws.ped_action("affected");
        assert!(ped::hand_changed(&ws.params) && ws.params.s("changes").is_empty(), "overrides folded into the family");
        assert!(ped::family_of(&ws.params).person(0).unwrap().dead, "the folded override survives");
        ws.ped_action("child");
        let kid = ws.ped_sel.unwrap();
        assert_eq!(ped::family_of(&ws.params).len(), n0 + 2, "partner + child");
        assert_eq!(ped::people_of(&ws.params).2[kid].gi, 3);
        ws.ped_action("sibling");
        ws.ped_action("unknown");
        assert_eq!(ped::family_of(&ws.params).person(ws.ped_sel.unwrap()).unwrap().sex, 2);
        assert_eq!(ws.undo.len(), 4, "one undo step per edit");
        // exports carry the edits
        let out = ws.output().unwrap();
        assert!(out.svg.as_deref().unwrap().contains("Sex unknown"));
        assert_eq!(out.spec.unwrap().params.get("family").and_then(|v| v.as_str()), Some(ws.params.s("family")));
        let link = ws.share_link().unwrap();
        let mut dst = Workspace::new(false);
        dst.apply_link(&link).unwrap();
        assert_eq!(dst.params.s("family"), ws.params.s("family"), "share link round trip");
        // remove takes the selection away; undo all the way back restores the start
        ws.ped_action("remove");
        assert_eq!(ws.ped_sel, None);
        for _ in 0..5 {
            ws.step_history(true);
        }
        assert_eq!(format!("{:?}", ws.params.0), format!("{:?}", start.0));
        // the panel draws for every kind of person without panicking
        for i in [0usize, 1, 2, target] {
            ws.ped_sel = Some(i);
            comp_frame(&ctx, &mut ws, vec![]);
        }
    }

    // ---------- figure-specific cards (figures::cards / cards_panel) ----------

    /// A frame on a tall screen, so the whole side panel is laid out and clickable.
    fn tall_frame(ctx: &egui::Context, ws: &mut Workspace, events: Vec<egui::Event>) -> egui::FullOutput {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1400.0, 3600.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| {
            let _ = ws.show(ui);
        });
        out.textures_delta.clear();
        out
    }

    fn open_tall(id: &str) -> (egui::Context, Workspace) {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        ctx.enable_accesskit();
        let mut ws = Workspace::new(false);
        ws.select(find_def(id).unwrap().0);
        for _ in 0..3 {
            tall_frame(&ctx, &mut ws, vec![]);
        }
        (ctx, ws)
    }

    /// Clicks the `n`th (top to bottom) side-panel widget named `label`, as a screen reader finds it.
    fn click_nth(ctx: &egui::Context, ws: &mut Workspace, label: &str, n: usize) {
        let out = tall_frame(ctx, ws, vec![]);
        let tree = out.platform_output.accesskit_update.expect("accesskit tree");
        let mut hits: Vec<_> = tree.nodes.iter().filter(|(_, nd)| nd.label().is_some_and(|l| l == label)).filter_map(|(_, nd)| nd.bounds()).filter(|b| b.x0 < 700.0).collect();
        hits.sort_by(|a, b| a.y0.total_cmp(&b.y0));
        let b = hits.get(n).unwrap_or_else(|| panic!("no widget #{n} named {label:?} ({} found)", hits.len()));
        let p = egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
        tall_frame(ctx, ws, vec![egui::Event::PointerMoved(p)]);
        let mut ev = btn(p, true);
        ev.extend(btn(p, false));
        tall_frame(ctx, ws, ev);
        tall_frame(ctx, ws, vec![]);
    }

    /// Side-panel text labels (section titles, control captions) reading exactly `text`.
    fn count_text(ctx: &egui::Context, ws: &mut Workspace, text: &str) -> usize {
        let out = tall_frame(ctx, ws, vec![]);
        let tree = out.platform_output.accesskit_update.expect("accesskit tree");
        tree.nodes.iter().filter(|(_, nd)| nd.role() == egui::accesskit::Role::Label && nd.value().is_some_and(|v| v == text)).filter(|(_, nd)| nd.bounds().is_some_and(|b| b.x0 < 700.0)).count()
    }

    /// Opens the side-panel section titled `title` by clicking its arrow (left of the title).
    fn open_section(ctx: &egui::Context, ws: &mut Workspace, title: &str) {
        let out = tall_frame(ctx, ws, vec![]);
        let tree = out.platform_output.accesskit_update.expect("accesskit tree");
        let b = tree.nodes.iter().filter(|(_, nd)| nd.role() == egui::accesskit::Role::Label && nd.value().is_some_and(|v| v == title)).filter_map(|(_, nd)| nd.bounds()).find(|b| b.x0 < 700.0).unwrap_or_else(|| panic!("no section {title:?}"));
        // the arrow: the unlabelled widget just left of the title, a little lower
        let icon = tree.nodes.iter().filter(|(_, nd)| nd.role() == egui::accesskit::Role::Unknown).filter_map(|(_, nd)| nd.bounds()).filter(|r| r.x1 <= b.x0 + 1.0 && r.y0 >= b.y0 - 2.0 && r.y0 < b.y1 + 20.0).min_by(|a, c| a.y0.total_cmp(&c.y0)).expect("section arrow");
        let p = egui::pos2(((icon.x0 + icon.x1) / 2.0) as f32, ((icon.y0 + icon.y1) / 2.0) as f32);
        tall_frame(ctx, ws, vec![egui::Event::PointerMoved(p)]);
        let mut ev = btn(p, true);
        ev.extend(btn(p, false));
        tall_frame(ctx, ws, ev);
        for _ in 0..12 {
            tall_frame(ctx, ws, vec![]);
        }
    }

    fn count_named(ctx: &egui::Context, ws: &mut Workspace, label: &str) -> usize {
        let out = tall_frame(ctx, ws, vec![]);
        let tree = out.platform_output.accesskit_update.expect("accesskit tree");
        tree.nodes.iter().filter(|(_, nd)| nd.label().is_some_and(|l| l == label)).filter(|(_, nd)| nd.bounds().is_some_and(|b| b.x0 < 700.0)).count()
    }

    /// "Reset label positions" is always in the Labels section, as on the reference: greyed out
    /// until a label has been dragged, then it puts the labels back.
    #[test]
    fn reset_label_positions_is_always_shown() {
        let (ctx, mut ws) = open_tall("triangle");
        open_section(&ctx, &mut ws, "Labels");
        let state = |ctx: &egui::Context, ws: &mut Workspace| {
            let out = tall_frame(ctx, ws, vec![]);
            let tree = out.platform_output.accesskit_update.expect("accesskit tree");
            let n: Vec<bool> = tree.nodes.iter().filter(|(_, nd)| nd.label().is_some_and(|l| l == "Reset label positions")).map(|(_, nd)| nd.is_disabled()).collect();
            n
        };
        assert_eq!(state(&ctx, &mut ws), vec![true], "shown and disabled with nothing moved");
        let mut p = ws.params.clone();
        p.set_s(figures::labels::KEY, "s0:10,5");
        assert!(figures::labels::any_moved(&p));
        ws.set_params_with_history(p);
        assert_eq!(state(&ctx, &mut ws), vec![false], "enabled once a label moved");
        click_nth(&ctx, &mut ws, "Reset label positions", 0);
        assert!(!figures::labels::any_moved(&ws.params), "labels put back");
    }

    /// Under the pedigree's Family card the generic sections start collapsed (reference);
    /// other figures still open their first two sections.
    #[test]
    fn pedigree_sections_start_collapsed() {
        let (ctx, mut ws) = open_tall("pedigree");
        assert_eq!(count_text(&ctx, &mut ws, "Symbols"), 1, "the header is there");
        assert_eq!(count_text(&ctx, &mut ws, "Carriers"), 0, "Symbols starts collapsed");
        assert_eq!(count_text(&ctx, &mut ws, "Key"), 0, "Labels starts collapsed");
        open_section(&ctx, &mut ws, "Symbols");
        assert!(count_text(&ctx, &mut ws, "Carriers") >= 1, "opens on a click");
        assert_eq!((open_sections("pedigree"), open_sections("pulley")), (0, 2));
    }

    /// A section header's summary wraps short of its Reset button instead of running under it
    /// (motion-graphs segment summaries are long).
    #[test]
    fn section_summaries_never_run_under_reset() {
        for id in ["motion-graphs", "pulley", "free-body-diagram"] {
            let (ctx, mut ws) = open_tall(id);
            let out = tall_frame(&ctx, &mut ws, vec![]);
            let tree = out.platform_output.accesskit_update.expect("accesskit tree");
            let resets: Vec<_> = tree.nodes.iter().filter(|(_, nd)| nd.label().is_some_and(|l| l == "Reset")).filter_map(|(_, nd)| nd.bounds()).filter(|b| b.x0 < 700.0).collect();
            assert!(!resets.is_empty(), "{id}");
            let texts: Vec<_> = tree.nodes.iter().filter(|(_, nd)| nd.role() == egui::accesskit::Role::Label).filter_map(|(_, nd)| nd.bounds()).filter(|b| b.x0 < 700.0).collect();
            for r in &resets {
                for t in &texts {
                    let overlap = t.x1 > r.x0 + 1.0 && t.x0 < r.x1 - 1.0 && t.y1 > r.y0 + 1.0 && t.y0 < r.y1 - 1.0;
                    assert!(!overlap, "{id}: text {t:?} under Reset {r:?}");
                }
            }
        }
    }

    /// Hovering a zone that covers much of the figure (PES plot, particle box) only outlines it;
    /// small zones keep the tint.
    #[test]
    fn large_zones_are_outlined_not_tinted() {
        let acc = theme::accent_col();
        let (fill, _) = zone_hover_style(900.0, 1000.0, acc);
        assert_eq!(fill, egui::Color32::TRANSPARENT);
        let (fill, stroke) = zone_hover_style(50.0, 1000.0, acc);
        assert_ne!(fill, egui::Color32::TRANSPARENT);
        assert_eq!(stroke.width, 1.5);
    }

    /// Gel: the Lanes card shows one card per lane; adding, moving and removing lanes rewrite the
    /// lanes text (a classic setup is copied in first), each as one undo step.
    #[test]
    fn gel_lane_cards_in_the_panel() {
        use crate::figures::biology::gel;
        let (ctx, mut ws) = open_tall("gel-electrophoresis");
        assert_eq!(gel::current_setup(&ws.params), Some("Crime scene"));
        assert_eq!(count_named(&ctx, &mut ws, "←"), 5, "one card per lane");
        let before = ws.params.clone();
        click_nth(&ctx, &mut ws, "+ Ladder", 0);
        let lanes = gel::lane_cards(&ws.params);
        assert_eq!(lanes.len(), 6);
        assert_eq!(lanes[5], gel::LaneCard::ladder());
        assert_eq!(gel::current_setup(&ws.params), None, "editing a setup's lanes copies them into the text");
        click_nth(&ctx, &mut ws, "→", 0); // Ladder moves right of Crime scene
        assert_eq!(gel::lane_cards(&ws.params)[1].label, "Ladder");
        click_nth(&ctx, &mut ws, "×", 0); // remove the first lane (now Crime scene)
        let lanes = gel::lane_cards(&ws.params);
        assert_eq!(lanes.iter().map(|l| l.label.as_str()).collect::<Vec<_>>(), ["Ladder", "Suspect 1", "Suspect 2", "Suspect 3", "Ladder"]);
        for _ in 0..3 {
            ws.step_history(true);
        }
        assert_eq!(format!("{:?}", ws.params.0), format!("{:?}", before.0), "three undo steps restore the setup");
    }

    /// Typing into a lane's label keeps spaces as typed although the lanes text is tidied on
    /// every keystroke (the box keeps its own buffer while focused).
    #[test]
    fn lane_label_typing_keeps_spaces() {
        use crate::figures::biology::gel;
        let (ctx, mut ws) = open_tall("gel-electrophoresis");
        let mut p = ws.params.clone();
        p.set_s("lanes", "Ladder: ladder 1kb | Mom: 800, 500");
        ws.set_params_with_history(p);
        let out = tall_frame(&ctx, &mut ws, vec![]);
        let tree = out.platform_output.accesskit_update.expect("accesskit tree");
        let b = tree.nodes.iter().filter(|(_, nd)| nd.value().is_some_and(|v| v == "Mom")).filter_map(|(_, nd)| nd.bounds()).next().expect("Mom's label box");
        let at = egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
        tall_frame(&ctx, &mut ws, vec![egui::Event::PointerMoved(at)]);
        let mut ev = btn(at, true);
        ev.extend(btn(at, false));
        tall_frame(&ctx, &mut ws, ev);
        tall_frame(&ctx, &mut ws, vec![egui::Event::Key { key: egui::Key::End, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
        for t in [" ", "B", " ", "2"] {
            tall_frame(&ctx, &mut ws, vec![egui::Event::Text(t.into())]);
        }
        assert_eq!(gel::lane_cards(&ws.params)[1].label, "Mom B 2");
    }

    /// Free-body diagram: per-force cards with quick directions and delete-any; the Motion card
    /// turns velocity and acceleration on with their own directions.
    #[test]
    fn force_cards_in_the_panel() {
        let (ctx, mut ws) = open_tall("free-body-diagram");
        assert_eq!(ws.params.i("n"), 2);
        assert_eq!(count_named(&ctx, &mut ws, "Up"), 2);
        click_nth(&ctx, &mut ws, "Left", 0);
        assert_eq!(ws.params.f("f1_ang"), 180.0);
        click_nth(&ctx, &mut ws, "+ Friction", 0);
        assert_eq!((ws.params.i("n"), ws.params.s("f3_lab")), (3, "F_f"));
        click_nth(&ctx, &mut ws, "×", 0); // delete force 1: the others move up
        assert_eq!((ws.params.i("n"), ws.params.s("f1_lab"), ws.params.s("f2_lab")), (2, "F_N", "F_f"));
        ws.step_history(true);
        assert_eq!((ws.params.i("n"), ws.params.s("f1_lab")), (3, "F_g"));
        // Motion is the third section (collapsed): open it and tick both
        let id = egui::Id::new("fig_sec_probe");
        let _ = id;
        let secs = figures::layout::sections("free-body-diagram", &(reg()[ws.sel.unwrap()].params)(), Some(3));
        assert_eq!(secs.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["Body", "Forces", "Motion", "Figure"]);
        let mut p = ws.params.clone();
        p.set_s("motion", "Both");
        ws.set_params_with_history(p);
        tall_frame(&ctx, &mut ws, vec![]);
        assert_eq!(figures::cards::summary("free-body-diagram", "motion", &ws.params).as_deref(), Some("velocity and acceleration"));
    }

    /// Vector diagram: cards with remove, reorder and "Add a vector".
    #[test]
    fn vector_cards_in_the_panel() {
        let (ctx, mut ws) = open_tall("vector-diagram");
        click_nth(&ctx, &mut ws, "+ Add a vector", 0);
        assert_eq!(ws.params.i("n"), 3);
        click_nth(&ctx, &mut ws, "↓", 0); // A after B
        assert_eq!((ws.params.s("v1_lab"), ws.params.s("v2_lab")), ("B", "A"));
        click_nth(&ctx, &mut ws, "×", 1); // remove the second (A)
        assert_eq!((ws.params.i("n"), ws.params.s("v1_lab"), ws.params.s("v2_lab")), (2, "B", "C"));
    }

    /// Population growth: readouts under the Population card, "Fit now" catches the axes up and
    /// unticking "Fit the axes" keeps them where they are.
    #[test]
    fn growth_readout_and_fit_now_in_the_panel() {
        let (ctx, mut ws) = open_tall("population-growth");
        let def = &reg()[ws.sel.unwrap()];
        let fitted = (def.render)(&ws.params);
        click_nth(&ctx, &mut ws, "Fit the axes as these change", 0);
        assert!(!ws.params.b("fit"));
        assert_eq!((def.render)(&ws.params), fitted, "unticking keeps the axes");
        let mut p = ws.params.clone();
        p.set_f("k", 3000.0);
        ws.set_params_with_history(p);
        click_nth(&ctx, &mut ws, "Fit now", 0);
        assert_eq!((ws.params.f("y_max"), ws.params.f("y_step")), (3500.0, 500.0));
        assert!(!ws.params.b("fit"));
        let lines = figures::cards::readout("population-growth", &ws.params);
        assert!(lines[0].starts_with("Inflection point: N = K/2 = 1500"), "{lines:?}");
    }

    /// Line spectrum: strip cards with element select, Edit lines, move and "+ Mixture"; an old
    /// figure (ticked elements) moves into the strip list on the first edit.
    #[test]
    fn strip_cards_in_the_panel() {
        use crate::figures::chemistry::spectra::{strip_cards, StripCard as C};
        let (ctx, mut ws) = open_tall("line-spectrum");
        assert_eq!(count_named(&ctx, &mut ws, "Edit lines"), 3);
        click_nth(&ctx, &mut ws, "Edit lines", 0);
        assert!(matches!(&strip_cards(&ws.params)[0], C::Own { name, lines } if name == "Hydrogen" && lines.starts_with("410.2")));
        click_nth(&ctx, &mut ws, "+ Mixture", 0);
        assert_eq!(strip_cards(&ws.params).len(), 5);
        click_nth(&ctx, &mut ws, "↑", 1); // Helium above the edited hydrogen (strip 1's own ↑)
        assert_eq!(strip_cards(&ws.params)[0], C::Element("Helium".into()));
        // an old figure
        let def = &reg()[ws.sel.unwrap()];
        let mut old = Params::defaults(&(def.params)());
        old.set_b("neon", true);
        let before = (def.render)(&old);
        ws.set_params_with_history(old);
        tall_frame(&ctx, &mut ws, vec![]);
        assert_eq!((def.render)(&ws.params), before, "showing the cards changes nothing");
        assert!(ws.params.s("strips").is_empty());
        click_nth(&ctx, &mut ws, "+ Element", 0);
        assert_eq!(ws.params.s("strips"), "Hydrogen; Neon; Helium");
    }

    /// Particle diagram: one card per kind, "+ Add a kind", remove moves later kinds up.
    #[test]
    fn particle_kind_cards_in_the_panel() {
        use crate::figures::chemistry::atoms;
        let (ctx, mut ws) = open_tall("particle-diagram");
        assert_eq!(count_named(&ctx, &mut ws, "XL"), 2);
        click_nth(&ctx, &mut ws, "+ Add a kind", 0);
        assert!(atoms::kind_used(&ws.params, 2));
        assert_eq!(count_named(&ctx, &mut ws, "XL"), 3);
        click_nth(&ctx, &mut ws, "Pair", 2);
        assert_eq!(ws.params.s("c_shape"), "Pair");
        assert_eq!(atoms::kind_noun(&ws.params, 2), "Molecule");
        assert_eq!(count_named(&ctx, &mut ws, "XL"), 4, "a molecule has Center and Outer sizes");
        click_nth(&ctx, &mut ws, "×", 0);
        assert_eq!(ws.params.s("b_shape"), "Pair");
        assert!(!atoms::kind_used(&ws.params, 2));
        // a kind set to none keeps its card until removed
        let mut p = ws.params.clone();
        p.set_i("a_count", 0);
        ws.params = p;
        assert_eq!(count_named(&ctx, &mut ws, "XL"), 3);
    }

    /// Predator-prey: the readout follows the Predator and prey card.
    #[test]
    fn pp_readout_in_the_panel() {
        let (ctx, mut ws) = open_tall("predator-prey");
        let out = tall_frame(&ctx, &mut ws, vec![]);
        let tree = out.platform_output.accesskit_update.expect("accesskit tree");
        assert!(tree.nodes.iter().any(|(_, nd)| nd.label().is_some_and(|l| l.starts_with("A cycle every 10 years"))) || tree.nodes.iter().any(|(_, nd)| nd.value().is_some_and(|l| l.starts_with("A cycle every 10 years"))));
    }
}
