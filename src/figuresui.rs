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
fn flatten_white(png: &[u8]) -> Result<Vec<u8>, String> {
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

    fn select(&mut self, i: usize) {
        self.sel = Some(i);
        self.params = Params::defaults(&(reg()[i].params)());
        self.shown = None;
        self.pending = None;
        self.err = None;
    }

    fn back(&mut self) {
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
                let avail = ui.available_width();
                let cols = ((avail + 12.0) / (CARD_W + 12.0)).floor().max(1.0) as usize;
                let mut budget = 2u8;
                for chunk in list.chunks(cols) {
                    ui.horizontal_top(|ui| {
                        for (i, def) in chunk {
                            if card(ui, &ctx, def, *i, &mut budget) {
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
            .fill(theme::CARD)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(CornerRadius::same(10))
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
                                .fill(theme::ACCENT)
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        let t = self.import_text.clone();
                        self.import_bytes(ui.ctx(), t.as_bytes(), "pasted.svg".to_string());
                    }
                    if let Some(e) = &self.import_err {
                        ui.label(RichText::new(e).color(theme::RED).size(12.0));
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
                if first || t0.elapsed() >= DEBOUNCE {
                    let svg = (def.render)(&self.params);
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
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width((w - 22.0).max(100.0));
                        ui.set_height((avail_h - 22.0).max(100.0));
                        ui.centered_and_justified(|ui| {
                            if let Some(t) = &preview {
                                ui.add(egui::Image::new(t).shrink_to_fit());
                            } else if let Some(e) = &self.err {
                                ui.label(RichText::new(e).color(theme::RED).size(12.0));
                            } else {
                                ui.spinner();
                            }
                        });
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
            if self.modal && ghost(ui, "Cancel").clicked() {
                act = Act::Close;
            }
        });
        act
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
                    for pd in &pdefs {
                        param_control(ui, pd, &mut self.params);
                        ui.add_space(6.0);
                    }
                });
            ui.add_space(4.0);
            if ghost(ui, "Reset").clicked() {
                self.params = Params::defaults(&pdefs);
            }
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
    ui.add(
        Button::new(theme::text(label))
            .fill(theme::CARD)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(CornerRadius::same(6)),
    )
}

fn accent_btn(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(Button::new(theme::on_accent(label)).fill(theme::ACCENT).corner_radius(CornerRadius::same(8)))
}

fn tab(ui: &mut Ui, label: &str, active: bool) -> bool {
    let b = Button::new(if active { theme::accent(label) } else { theme::text(label) })
        .fill(if active { theme::ACCENT_FADED } else { egui::Color32::TRANSPARENT })
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(8));
    ui.add(b).clicked()
}

/// One browser card; true when clicked.
fn card(ui: &mut Ui, ctx: &egui::Context, def: &FigureDef, idx: usize, budget: &mut u8) -> bool {
    let resp = Frame::new()
        .fill(theme::CARD)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(CARD_W - 20.0);
            ui.set_height(190.0);
            Frame::new().fill(egui::Color32::WHITE).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
                ui.set_width(CARD_W - 20.0);
                ui.set_height(110.0);
                ui.centered_and_justified(|ui| match thumb(ctx, def, budget) {
                    Some(t) => {
                        ui.add(egui::Image::new(&t).shrink_to_fit());
                    }
                    None => {
                        ui.label(theme::muted("…"));
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(theme::strong(def.title).size(13.0));
            ui.label(theme::muted(def.subject.name()).size(11.0));
            ui.label(theme::text(def.blurb).size(11.0));
        })
        .response;
    let r = ui.interact(resp.rect, UiId::new("fig_card").with(idx), Sense::click());
    if r.hovered() {
        ui.painter().rect_stroke(
            resp.rect,
            CornerRadius::same(10),
            Stroke::new(1.5, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r.clicked()
}

/// Control auto-built from one ParamDef.
fn param_control(ui: &mut Ui, pd: &ParamDef, p: &mut Params) {
    match &pd.kind {
        Kind::Float { min, max, step } => {
            ui.label(theme::muted(pd.label).size(11.0));
            let mut v = p.f(pd.key);
            let mut s = egui::Slider::new(&mut v, *min..=*max);
            if *step > 0.0 {
                s = s.step_by(*step);
            }
            if ui.add(s).changed() {
                p.0.insert(pd.key.to_string(), Value::F(v));
            }
        }
        Kind::Int { min, max } => {
            ui.label(theme::muted(pd.label).size(11.0));
            let mut v = p.i(pd.key);
            if ui.add(egui::Slider::new(&mut v, *min..=*max)).changed() {
                p.0.insert(pd.key.to_string(), Value::I(v));
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
            ComboBox::from_id_salt(("fig_param", pd.key))
                .selected_text(cur.clone())
                .width(200.0)
                .show_ui(ui, |ui| {
                    for o in opts {
                        ui.selectable_value(&mut cur, o.to_string(), *o);
                    }
                });
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
        egui::CentralPanel::default()
            .frame(Frame::new().fill(theme::CREAM).inner_margin(Margin::same(24)))
            .show(ui, |ui| {
                ui.label(theme::serif_strong("Figures").size(28.0));
                ui.label(
                    theme::muted("Physics, biology, chemistry and math diagrams you can tune, export, or attach to a question.")
                        .size(13.0),
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
        let mut frame = |ws: &mut Workspace| {
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
}
