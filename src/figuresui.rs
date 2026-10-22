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

use crate::figures::spec::{flatten_white, params_from_spec, spec_for, value_to_json};
use crate::figures::{self, FigureDef, Kind, ParamDef, Params, Subject, Value};
use crate::model::{FigureSpec, Id, Question};
use crate::theme;

mod cards_panel;
mod chem_shell;
mod lucide;
mod composite_canvas;
mod graph_canvas;
mod graph_edit;
mod tree_edit;
mod logic_canvas;
mod data_entry;
#[cfg(test)]
mod ui_test;
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
    // a Computing figure on the shared pieces: Kind::Data, the style switch and show_answers
    fn data_params() -> Vec<ParamDef> {
        vec![ParamDef::data("data", "Data", 1, "3, 5, 8"), ParamDef::data("pts", "Points", 2, "1, 2\n3, 4"), figures::svg::style_param(), figures::answers_param()]
    }
    fn data_render(p: &Params) -> String {
        let fills = figures::svg::Fills::new(p);
        let mut d = figures::svg::Doc::new(300.0, 200.0);
        d.defs(&fills.defs());
        for (i, v) in figures::data::numbers(p.s("data")).iter().enumerate() {
            d.rect(20.0 + i as f64 * 24.0, 180.0 - v * 8.0, 20.0, v * 8.0, &fills.series(i), "#000", 1.0);
        }
        if p.b(figures::SHOW_ANSWERS) {
            d.text(150.0, 20.0, 14.0, "middle", "ANSWER");
        }
        d.finish()
    }
    vec![
        FigureDef { id: "demo-shapes", subject: Subject::Math, title: "Demo shapes", blurb: "Test-only demo figure.", params, render },
        FigureDef { id: "demo-data", subject: Subject::Computing, title: "Demo data", blurb: "Test-only data figure.", params: data_params, render: data_render },
    ]
}

fn find_def(id: &str) -> Option<(usize, &'static FigureDef)> {
    reg().iter().enumerate().find(|(_, d)| d.id == id)
}

// ---------- params hashing ----------

fn hash_params(id: &str, p: &Params) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut h);
    format!("{:?}", p.0).hash(&mut h);
    h.finish()
}

// ---------- raster helpers ----------

/// Most screen points per SVG unit the preview zooms a figure to.
const PREVIEW_MAX_ZOOM: f64 = 4.0;

/// Padding (SVG units) the chemistry preview keeps around the drawing: the reference's 16 px.
const CHEM_PAD: f64 = 16.0;
/// Padding (SVG units) the math preview keeps around the inked box: the reference's viewBox is the
/// drawing's bounds grown by 12 on every side, then fitted into the sheet.
const MATH_PAD: f64 = 12.0;
/// How far (points) the math preview's frame starts below the reference's sheet: ours sits about 9 px
/// under the toolbar rule, the reference's sheet starts at it, with its svg 16 px in.
const MATH_TOP_GAP: f32 = 8.8;

/// The math preview's padding in our SVG units for figure `id`: the reference's 12 units scaled by
/// how large our drawing is next to its (ours draws on a 480x360 canvas, the reference on its own
/// ink-sized one), measured on each figure's opening state.
fn math_pad(id: &str) -> f64 {
    MATH_PAD * math_unit(id)
}

/// Our drawing's size against the reference's for figure `id` (1.0 = same units).
fn math_unit(id: &str) -> f64 {
    match id {
        "triangle" | "parallelogram" | "trapezoid" => 0.875,
        "rectangle" => 0.87,
        "kite" => 0.90,
        "regular-polygon" => 0.91,
        "angles" => 0.55,
        "prism" | "pyramid" => 1.0,
                _ => 1.0,
    }
}

/// The part of a math figure's canvas the preview shows: the whole canvas for the plots, else
/// the inked box plus the reference's padding.
fn math_view(content: Option<[f64; 4]>, id: &str, (cw, ch): (f64, f64)) -> [f64; 4] {
    if math_whole(id) { return [0.0, 0.0, cw, ch]; }
    // the reference grows prism and pyramid by 12 around the drawing's lines and label origins,
    // not around their ink: less above (the stroke is inked), more below (the label's descent)
    if let (Some(b), "prism" | "pyramid" | "cylinder" | "cone" | "sphere") = (content, id) {
        // round solids carry no label below the drawing: 10.8 (the grown bounds less the stroke) on
        // every side, 12 to 12.5 past a label or a dashed line's end on the right; the cone's bounds
        // are taken from its arcs' end points, so they stop short of the ellipse's extremes
        let (l, t, r, bt) = match id { "cylinder" => (10.8, 10.8, 12.5, 10.8), "cone" => (4.4, 10.8, 12.0, -0.7), "sphere" => (10.8, 10.8, 10.8, 10.8), _ => (12.0, 10.8, 12.0, 16.0) };
        return [(b[0] - l).max(0.0), (b[1] - t).max(0.0), (b[2] + r).min(cw), (b[3] + bt).min(ch)];
    }
    view_box_padded(content, (cw, ch), Some(math_pad(id)))
}
/// Physics previews: the drawing fills about 92% of the frame's width, as the reference's sheet does.
const PHYS_PAD: f64 = 0.028;
/// The reference's svg box on a desktop sheet, and its centre relative to ours.
/// The reference draws a 36 px toolbar above its sheet, so its figure sits 48 px lower than ours.
/// (width, centre offset): waves and projectile sit in a 962 px svg box; motion graphs' reference page
/// has a wider controls column, so its box is 930 px and starts 15 px further right.
fn phys_whole_box(id: &str) -> (f32, f32) {
    // motion graphs now has the reference's own wide column, so its box is centred on the card
    if matches!(id, "motion-graphs" | "ray-diagram" | "motion-map") { (930.0, 0.0) } else { (962.0, -3.0) }
}

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
#[cfg(test)]
fn view_box(content: Option<[f64; 4]>, (cw, ch): (f64, f64)) -> [f64; 4] {
    view_box_padded(content, (cw, ch), None)
}

/// `view_box` with a fixed `pad` (SVG units) around the drawing when given. The chemistry
/// figures use the reference's 16 px page padding, so the figure fills the frame as it does there.
fn view_box_padded(content: Option<[f64; 4]>, (cw, ch): (f64, f64), fixed: Option<f64>) -> [f64; 4] {
    let Some(b) = content else { return [0.0, 0.0, cw, ch] };
    let pad = fixed.unwrap_or(0.05 * (b[2] - b[0]).max(b[3] - b[1]) + 8.0);
    [(b[0] - pad).max(0.0), (b[1] - pad).max(0.0), (b[2] + pad).min(cw), (b[3] + pad).min(ch)]
}

/// How the physics reference lays a figure on its sheet: its svg canvas is the drawing plus a margin,
/// fitted whole into the sheet (never cropped to the ink). Per figure, at the default drawing:
/// the ink's share of the canvas (x, y), the ink's left and top margin as a share of the canvas, and
/// the reference's pixels per unit of ours (so a phone, where the reference never enlarges a figure
/// past its natural size, can show the same size).
#[derive(Clone, Copy)]
struct PhysRef { fx: f64, fy: f64, fl: f64, ft: f64, u: f64 }

/// Figures whose phone svg box keeps the canvas aspect (it shrinks in height with its width); the
/// rest keep their natural height while the width shrinks.
fn phys_scales_height(id: &str) -> bool {
    id == "motion-graphs"
}

/// Physics figures whose canvas is the reference's own viewBox (waves 597.8 x 378.8, motion graphs
/// 570 x 304.8, projectile 640 x 400): on a desktop the reference fits that whole canvas into a 962 px
/// wide svg box, so we do too instead of zooming to our ink box.
fn phys_whole(id: &str) -> bool {
    matches!(id, "waves" | "motion-graphs" | "projectile-motion" | "ray-diagram" | "motion-map" | "inclined-plane")
}

fn phys_ref(id: &str) -> Option<PhysRef> {
    let (fx, fy, fl, ft, u) = match id {
        "circuit-diagram" => (0.928, 0.841, 0.0365, 0.0853, 0.987),
        "free-body-diagram" => (0.156, 0.850, 0.425, 0.086, 1.162),
        "inclined-plane" => (0.936, 0.800, 0.0333, 0.0990, 1.431),
        "motion-graphs" => (0.940, 0.909, 0.0344, 0.0315, 1.0),
        "projectile-motion" => (0.948, 0.366, 0.0260, 0.3370, 1.0),
        "pulley" => (0.349, 0.678, 0.3250, 0.0550, 0.990),
        "spring-scale" => (0.948, 0.959, 0.0270, 0.0238, 1.300),
        "vector-diagram" => (0.995, 0.995, 0.0024, 0.0027, 0.582),
        "waves" => (0.949, 0.927, 0.0323, 0.0259, 1.0),
        _ => return None,
    };
    Some(PhysRef { fx, fy, fl, ft, u })
}

/// The reference's canvas around our ink box `b`, in our units.
fn phys_view(pr: PhysRef, b: [f64; 4], id: &str) -> [f64; 4] {
    let (w, h) = ((b[2] - b[0]).max(1.0), (b[3] - b[1]).max(1.0));
    let (mut vw, vh) = (w / pr.fx, h / pr.fy);
    if id == "free-body-diagram" {
        // the free-body canvas keeps its 160 x 308 shape while the arrows are narrow; motion arrows beside the
        // body widen it (the reference keeps the scale and centres the drawing)
        let base = vh * 160.0 / 308.0;
        if w / pr.fx > base * 1.001 {
            vw = base.max(w + 2.0 * pr.ft * vh);
            let x0 = (b[0] + b[2]) / 2.0 - vw / 2.0;
            let y0 = b[1] - pr.ft * vh;
            return [x0, y0, x0 + vw, y0 + vh];
        }
    }
    let (x0, y0) = (b[0] - pr.fl * vw, b[1] - pr.ft * vh);
    [x0, y0, x0 + vw, y0 + vh]
}

/// The box the reference's sheet gives that canvas: the sheet less 16 px of padding on a desktop; on a
/// phone as wide as the sheet allows but never past the natural size, and as tall as the natural
/// height up to the window less 219 px, centred (the figure is fitted inside, as the svg does).
fn phys_area(full: egui::Rect, id: &str, win_h: f32) -> egui::Rect {
    let inner = full.shrink(16.0);
    if full.width() >= 600.0 {
        // our card already stops 15 px inside the preview box; the reference's svg sits 16 px in
        // from its sheet, so the figure gets the whole frame
        // the vector diagram's svg sits inset in the reference's sheet: 19 px under the toolbar, 20 above the card's edge
        if id == "vector-diagram" {
            return egui::Rect::from_min_max(egui::pos2(full.left(), full.top() + 8.0), egui::pos2(full.right(), full.bottom() - 16.0));
        }
        return full;
    }
    let (nw, nh) = phys_vb(id).unwrap_or((inner.width(), 300.0));
    let bw = nw.min(inner.width());
    let bh = if phys_scales_height(id) { nh * bw / nw } else { nh }.min((win_h - 219.0).max(100.0));
    egui::Rect::from_min_size(egui::pos2(inner.center().x - bw / 2.0, inner.top()), Vec2::new(bw, bh))
}

/// The reference's own svg viewBox per physics figure: on a phone its svg box is that wide (never
/// wider than the sheet allows) and that tall (never past the window less 219 px).
fn phys_vb(id: &str) -> Option<(f32, f32)> {
    Some(match id {
        "motion-graphs" => (570.0, 304.8),
        "waves" => (597.8, 378.8),
        "motion-map" => (859.38, 92.04),
        "ray-diagram" => (820.0, 299.0),
        "free-body-diagram" => (160.0, 308.0),
        "vector-diagram" => (212.0, 182.0),
        "inclined-plane" | "projectile-motion" => (640.0, 400.0),
        "pulley" => (640.0, 420.0),
        "spring-scale" => (545.0, 820.0),
        "circuit-diagram" => (433.0, 204.0),
        _ => return None,
    })
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
    let cap = (2048.0 / cw.max(ch).max(1.0)).min((12.0e6 / (cw * ch).max(1.0)).sqrt());
    let r = ((px_per_unit.min(cap) * 4.0).ceil() / 4.0).clamp(1.0, 6.0);
    // rounding up must not push the texture past the cap
    (if r as f64 > cap && cap >= 1.0 { (cap * 4.0).floor() / 4.0 } else { r }) as f32
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

/// A thumbnail rasterized to fit a `bw` x `bh` box (points) at the screen's pixel density, so it
/// is drawn 1:1 instead of being shrunk from a bigger bitmap (which aliases thin hairlines).
pub(crate) fn thumb_fit(ctx: &egui::Context, def: &FigureDef, p: &Params, bw: f32, bh: f32, budget: &mut u8) -> Option<(TextureHandle, f32)> {
    let ppp = ctx.pixels_per_point();
    let key = format!("{}:{:x}:fit{bw:.0}x{bh:.0}@{ppp}", def.id, hash_params(def.id, p));
    if let Some(hit) = THUMBS.with(|t| t.borrow().get(&key).cloned()) {
        return hit.map(|t| (t, ppp));
    }
    if *budget == 0 {
        ctx.request_repaint();
        return None;
    }
    *budget -= 1;
    THUMB_RENDERS.with(|c| c.set(c.get() + 1));
    // crisp hairlines turn into solid 1 px bars at thumbnail scale; the browser smooths them to fine grey
    let svg = (def.render)(p).replace(r#"shape-rendering="crispEdges""#, "");
    let tex = (|| {
        let mut opt = resvg::usvg::Options::default();
        opt.fontdb = figures::fontdb();
        let tree = resvg::usvg::Tree::from_str(&figures::serif_stack(&svg), &opt).ok()?;
        let sz = tree.size();
        let k = (bw / sz.width()).min(bh / sz.height()) * ppp;
        let png = figures::to_png(&svg, k).ok()?;
        texture(ctx, format!("testparrot:thumb:{key}"), &png)
    })();
    THUMBS.with(|t| t.borrow_mut().insert(key, tex.clone()));
    tex.map(|t| (t, ppp))
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

/// The reference's floating circled "?" at the bottom-left of the page; opens a short tips card.
fn help_fab(ctx: &egui::Context, left: bool) {
    let sr = ctx.content_rect();
    let id = egui::Id::new("fig_help_fab_open");
    let open = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    // The reference puts it bottom-left of the page; the skinned pages hide our rail, so they use it.
    // The unskinned pages keep it bottom-right, clear of the rail.
    let x = if sr.width() < 500.0 || left { sr.left() + 17.0 } else { sr.right() - 53.0 };
    let pos = egui::Pos2::new(x, sr.bottom() - 54.0);
    let r = egui::Area::new(egui::Id::new("fig_help_fab")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
        let p = ui.painter();
        p.add(egui::epaint::RectShape::filled(rect.translate(Vec2::new(0.0, 1.5)), CornerRadius::same(18), egui::Color32::from_black_alpha(if theme::is_dark() { 70 } else { 28 })));
        p.circle(rect.center(), 18.0, theme::card(), Stroke::new(1.0, theme::border()));
        p.text(rect.center(), egui::Align2::CENTER_CENTER, egui_phosphor::regular::QUESTION, theme::icon_font(20.0), if resp.hovered() { theme::accent_col() } else { theme::ink() });
        resp.on_hover_text("Help")
    });
    if r.inner.clicked() {
        ctx.data_mut(|d| d.insert_temp(id, !open));
    }
    if open {
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let full = ctx.content_rect();
        // Dimmed backdrop (the reference also blurs the page, which egui cannot do).
        let bd = egui::Area::new(egui::Id::new("fig_help_backdrop")).order(egui::Order::Foreground).fixed_pos(full.min).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(full.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgba_unmultiplied(17, 24, 39, 107));
            resp
        });
        if bd.inner.clicked() {
            close = true;
        }
        let w = (full.width() - 32.0).min(440.0);
        egui::Area::new(egui::Id::new("fig_help_card")).order(egui::Order::Tooltip).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            Frame::new()
                .fill(theme::card())
                .corner_radius(CornerRadius::same(12))
                .shadow(egui::epaint::Shadow { offset: [0, 18], blur: 60, spread: 0, color: egui::Color32::from_rgba_unmultiplied(17, 24, 39, 61) })
                .inner_margin(Margin::same(20))
                .show(ui, |ui| {
                    ui.set_width(w - 40.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.allocate_ui_with_layout(Vec2::new(w - 40.0, 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Need a hand?").family(theme::bold_family()).size(20.0).color(theme::ink()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let x = egui::Button::new(RichText::new(egui_phosphor::regular::X).font(theme::icon_font(16.0)).color(theme::muted_col())).frame(false).min_size(Vec2::splat(28.0));
                            if ui.add(x).on_hover_text("Close").clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(10.0);
                    let para = |ui: &mut egui::Ui, t: &str| {
                        let mut job = egui::text::LayoutJob::default();
                        job.wrap.max_width = w - 40.0;
                        job.append(t, 0.0, egui::TextFormat { font_id: egui::FontId::new(15.0, egui::TextStyle::Body.resolve(ui.style()).family), color: theme::ink(), line_height: Some(24.0), ..Default::default() });
                        ui.add(egui::Label::new(job));
                    };
                    para(ui, "If you\u{2019}re running into trouble or have suggestions, email us and we\u{2019}ll help.");
                    ui.add_space(10.0);
                    para(ui, "Want a figure we don\u{2019}t make yet? Email us to request a generator, and we\u{2019}ll build it.");
                    ui.add_space(16.0);
                    Frame::new().stroke(Stroke::new(1.0, theme::border())).corner_radius(CornerRadius::same(10)).inner_margin(Margin { left: 13, right: 6, top: 4, bottom: 4 }).show(ui, |ui| {
                        ui.set_width(w - 40.0 - 20.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("support@teacher.dev").family(theme::bold_family()).size(16.0).color(theme::ink()));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let chip = egui::Button::new(RichText::new(format!("{} Copy", egui_phosphor::regular::COPY)).family(theme::bold_family()).size(14.0).color(theme::accent_hover()))
                                    .fill(theme::accent_faded())
                                    .corner_radius(CornerRadius::same(7))
                                    .min_size(Vec2::new(74.0, 32.0));
                                if ui.add(chip).clicked() {
                                    ui.ctx().copy_text("support@teacher.dev".to_string());
                                }
                            });
                        });
                    });
                    ui.add_space(10.0);
                    let btn = egui::Button::new(RichText::new(format!("{} Open in email", egui_phosphor::regular::ENVELOPE_SIMPLE)).family(theme::bold_family()).size(16.0).color(egui::Color32::WHITE))
                        .fill(theme::accent_col())
                        .corner_radius(CornerRadius::same(12))
                        .min_size(Vec2::new(w - 40.0, 41.0));
                    if ui.add_sized(Vec2::new(w - 40.0, 41.0), btn).clicked() {
                        crate::platform::open_url("mailto:support@teacher.dev");
                    }
                });
        });
        if close {
            ctx.data_mut(|d| d.insert_temp(id, false));
        }
    }
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
    /// The major topic whose subject tabs are showing (None: the All view).
    group: Option<figures::Group>,
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
    /// An action chosen on the preview toolbar this frame (chemistry skin), handed up by `workspace_view`.
    tool_act: Option<Act>,
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
    /// Canvas editor of the logic-circuit figure, and whether the preview (not the canvas) shows.
    logic: logic_canvas::Canvas,
    logic_preview: bool,
    /// Canvas editor of the graph figure, and whether the preview shows instead.
    graph: graph_edit::Canvas,
    graph_preview: bool,
    tree: tree_edit::Canvas,
    tree_preview: bool,
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
    /// Sub-pixel offset the shown chemistry texture was rasterized with, and the one wanted now.
    phase: (f32, f32),
    want_phase: (f32, f32),
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

/// Chemistry figures drawn whole on the reference's own canvas.
/// The box the reference's phone sheet gives a chemistry figure's svg: its natural width capped to the
/// sheet, and a height that follows the width for the charts (no `preserveAspectRatio`) but stays at
/// the natural height, up to the window less 219 px, for the rest.
fn phone_box(id: &str, (cw, ch): (f64, f64), card_w: f64, win_h: f64) -> (f64, f64) {
    let bw = cw.min(card_w - 34.0);
    let bh = if proportional_box(id) { ch * bw / cw } else { ch.min(win_h - 219.0) };
    (bw, bh)
}

fn proportional_box(id: &str) -> bool {
    matches!(id, "titration-curve" | "heating-cooling-curve" | "mass-spectrum" | "population-growth" | "predator-prey")
}

/// Math figures the reference fits whole (its canvas is the plot, not the ink): the plots and the mapping diagram.
fn math_whole(id: &str) -> bool {
    matches!(id, "coordinate-grid" | "number-line" | "box-plot" | "mapping-diagram")
}

fn chem_whole(id: &str) -> bool {
    matches!(id, "titration-curve" | "heating-cooling-curve" | "mass-spectrum" | "line-spectrum" | "photoelectron-spectrum" | "bohr-model" | "particle-diagram" | "lewis-structures" | "orbital-diagram" | "gas-syringe" | "length-reading" | "mass-reading" | "ph-reading" | "temperature-reading" | "volume-by-displacement" | "volume-reading" | "population-growth" | "predator-prey" | "punnett-square" | "pedigree" | "gel-electrophoresis" | "micropipette-reading")
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
        // a link that opens a figure fresh starts its history clean (Undo greyed, as on the
        // reference's first screen); a link pasted over an open figure is one undo step
        let fresh = self.sel != Some(i);
        if fresh {
            self.select(i);
        }
        if fresh || hash_params(def.id, &p) == hash_params(def.id, &self.params) {
            self.params = p;
            self.reset_history();
        } else {
            self.set_params_with_history(p);
        }
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
        // a link that opens a figure fresh starts its history clean (Undo greyed, as on the
        // reference's first screen); a link pasted over an open figure is one undo step
        let fresh = self.sel != Some(i);
        if fresh {
            self.select(i);
        }
        if fresh || hash_params(def.id, &p) == hash_params(def.id, &self.params) {
            self.params = p;
            self.reset_history();
        } else {
            self.set_params_with_history(p);
        }
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
        // the drag knobs show on every figure but the physics ones, whose reference draws a bare
        // figure; the Handles chip toggles them
        self.hide_handles = matches!(reg()[i].subject, figures::Subject::Physics | figures::Subject::Chemistry);
        self.params = figures::start_params(&reg()[i]);
        // the composite shape opens on a blank canvas with its "Start with" screen, as the
        // reference does. Its opening state (share links, Reset all) keeps the arch, so links
        // made before stay the same; links and saved figures set their own spec over this.
        if reg()[i].id == "composite-shape" {
            self.params.set_s("spec", "");
        }
        self.comp = Default::default();
        self.logic = Default::default();
        self.logic_preview = false;
        self.graph = Default::default();
        self.graph_preview = false;
        self.tree = Default::default();
        self.tree_preview = false;
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
        if theme::chem_skin() && self.chem_list() {
            let a = self.chem_browser(ui);
            help_fab(ui.ctx(), true);
            return a;
        }
        let ctx = ui.ctx().clone();
        let narrow = ui.available_width() < 640.0;
        ui.horizontal_wrapped(|ui| {
            if tab(ui, "All", self.subject.is_none() && self.group.is_none()) {
                self.subject = None;
                self.group = None;
            }
            // a topic with no figures yet shows no tab
            let has = |s: Subject| reg().iter().any(|d| figures::listed_under(d, s));
            for g in figures::Group::ALL.into_iter().filter(|g| g.subjects().iter().any(|s| has(*s))) {
                if tab(ui, g.name(), self.group == Some(g)) {
                    self.group = Some(g);
                    // a topic with a single subject goes straight to it
                    let own: Vec<Subject> = g.subjects().iter().copied().filter(|s| has(*s)).collect();
                    self.subject = if own.len() == 1 { Some(own[0]) } else { None };
                }
            }
            // a phone: the tabs keep their row and search, Open link and Import wrap onto the next one
            // (right-to-left inside the wrapped tabs row let them ride over the tab chips)
            if !narrow {
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
            }
        });
        if let Some(g) = self.group {
            let has = |s: Subject| reg().iter().any(|d| figures::listed_under(d, s));
            let own: Vec<Subject> = g.subjects().iter().copied().filter(|s| has(*s)).collect();
            if own.len() > 1 {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if tab(ui, &format!("All {}", g.name().to_lowercase()), self.subject.is_none()) {
                        self.subject = None;
                    }
                    for s in own {
                        if tab(ui, s.name(), self.subject == Some(s)) {
                            self.subject = Some(s);
                        }
                    }
                });
            }
        }
        if narrow {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(TextEdit::singleline(&mut self.search).hint_text("Search figures…").desired_width(150.0));
                if ghost(ui, "Open link…").on_hover_text("Reopen a figure from a share link someone copied").clicked() {
                    self.link_open = !self.link_open;
                }
                if ghost(ui, "Import SVG/PNG…").clicked() {
                    self.import_open = !self.import_open;
                }
            });
        }
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
            .filter(|(_, d)| match (self.subject, self.group) {
                (Some(s), _) => figures::listed_under(d, s),
                (None, Some(g)) => g.subjects().iter().any(|s| figures::listed_under(d, *s)),
                (None, None) => true,
            })
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
        } else if let Some(g) = self.group {
            let order = |d: &FigureDef| g.subjects().iter().position(|s| *s == d.subject).unwrap_or(usize::MAX);
            list.sort_by_key(|(_, d)| order(d));
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
        // the chemistry page skin carries the title in its top header and the actions in the
        // preview toolbar, so this view drops its own title row and action row
        let skin = theme::chem_skin() && self.chem_open();

        if !skin {
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
        }

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
            let rescale = self.shown.is_some() && ((want - self.raster).abs() > 0.01 || (self.want_phase.0 - self.phase.0).abs() > 0.02 || (self.want_phase.1 - self.phase.1).abs() > 0.02);
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
                    match figures::to_rgba_at(&svg, scale, self.want_phase) {
                        Ok((w, hh, px)) => {
                            let img = egui::ColorImage::from_rgba_premultiplied([w as usize, hh as usize], &px);
                            self.content = ink_box(&px, w, hh).map(|b| b.map(|v| v as f64 / scale as f64));
                            self.raster = scale;
                            self.phase = self.want_phase;
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
        } else if skin {
            ui.available_height().max(240.0)
        } else {
            (ui.available_height() - 64.0).max(240.0)
        };
        // A phone-width frame stacks the figure above its controls, as the reference does, and
        // scrolls as a whole; a wide one puts the controls beside the figure.
        // the reference stacks its chemistry page below 880 px of window, other pages below 640 px of frame
        let chem_stack = skin && self.skin_subject() == figures::Subject::Chemistry && ctx.content_rect().width() < 880.0;
        let narrow = generated && !self.modal && (ui.available_width() < 640.0 || chem_stack);
        if narrow {
            if skin && self.skin_subject() == figures::Subject::Physics {
                // the physics page's rows, icons and toolbar read these flags (chem_wide sets them on a wide window)
                let fig_id = self.sel.map(|i| reg()[i].id).unwrap_or("");
                ctx.data_mut(|d| {
                    d.insert_temp(egui::Id::new("math_skin_page"), false);
                    d.insert_temp(egui::Id::new("phys_skin_page"), true);
                    d.insert_temp(egui::Id::new("phys_fig_id"), fig_id);
                    d.insert_temp(egui::Id::new("chem_skin_page"), false);
                    d.insert_temp(egui::Id::new("bio_skin_page"), false);
                });
            }
            // the math reference's phone page keeps 16 px of side padding; ours is 12
            let side = if skin && self.sel.is_some_and(|i| reg()[i].subject == figures::Subject::Math) { 4.0 } else if self.sel.is_some_and(|i| reg()[i].subject == figures::Subject::Physics) { 5.0 } else { 0.0 };
            let w = ui.available_width() - 2.0 * side;
            let (cw, ch) = if self.canvas.0 > 1.0 { self.canvas } else { (1.0, 1.0) };
            let whole_chem = self.sel.is_some_and(|i| matches!(reg()[i].subject, figures::Subject::Chemistry | figures::Subject::Biology) && chem_whole(reg()[i].id));
            // the reference's sheet sits 17 px from each edge of a phone; ours 8
            let inset = if whole_chem && !skin { 9.0 } else { 0.0 };
            let cw_card = w - 2.0 * inset;
            let mut card_h = ((w as f64 - 22.0) * ch / cw + 22.0).clamp(180.0, (avail_h as f64 * 0.7).max(180.0)) as f32;
            // the reference opens a chemistry figure alone in a tall white canvas that ends about
            // 156 px above the window bottom, the controls card sitting below the fold
            if let Some(i) = self.sel.filter(|&i| reg()[i].subject == figures::Subject::Math && skin) {
                // the reference's sheet hugs the figure: its drawing shrunk to the width (never grown) plus
                // 16 px of padding, under the toolbar
                let id = reg()[i].id;
                let v = math_view(self.content, id, (cw, ch));
                let narrow = ctx.content_rect().width() < 500.0;
                // phone: the reference's svg is the card's width less 16 px a side, its sheet the svg plus
                // 32 under a 48.6 px toolbar
                let k = ((cw_card as f64 - if narrow && !math_whole(id) { 32.0 } else { 34.0 }) / (v[2] - v[0]).max(1.0)).min(1.0 / math_unit(id));
                card_h = ((v[3] - v[1]) * k + 32.0 + if !narrow { 54.0 + 2.0 } else if math_whole(id) { 47.5 + 2.0 } else { 48.6 }) as f32;
            }
            if let Some(i) = self.sel.filter(|&i| matches!(reg()[i].subject, figures::Subject::Chemistry | figures::Subject::Biology)) {
                if chem_whole(reg()[i].id) && skin {
                    // the reference's sheet hugs the figure: natural size (shrunk to the width, never
                    // grown) plus 16 px of padding, under a 47 px toolbar
                    let (_, bh) = phone_box(reg()[i].id, (cw, ch), cw_card as f64, ui.ctx().content_rect().height() as f64);
                    card_h = (bh + 32.0 + if ctx.content_rect().width() < 500.0 { 47.0 } else { 54.0 } + 2.0 + if proportional_box(reg()[i].id) || matches!(reg()[i].id, "photoelectron-spectrum" | "punnett-square") { 2.0 } else { 0.0 }) as f32;
                } else if chem_whole(reg()[i].id) && !skin {
                    // the reference's sheet hugs the figure: its canvas at natural size (shrunk to
                    // the width, never grown) plus 16 px of padding on every side
                    let sc = ((cw_card as f64 - 32.0) / cw).min(1.0);
                    card_h = (ch * sc + 32.0) as f32;
                } else {
                    card_h = card_h.max(ui.available_height() - if skin { 144.0 } else { 156.0 });
                }
            }
            if let (Some(i), Some(b)) = (self.sel, self.content)
                && reg()[i].subject == figures::Subject::Physics
                && phys_vb(reg()[i].id).is_some()
            {
                let _ = b;
                // the reference's phone sheet hugs its svg box (natural height up to the window less 219 px)
                // plus 16 px of padding; ours adds the strip of help text under it
                let a = phys_area(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(cw_card, 100.0)), reg()[i].id, ctx.content_rect().height());
                card_h = a.height() + 34.0;
            }
            let math_hug = skin && self.sel.is_some_and(|i| reg()[i].subject == figures::Subject::Math);
            if skin && !whole_chem && !math_hug {
                // the icon toolbar sits above the figure inside the card (a physics phone's is 46 px)
                card_h += if self.sel.is_some_and(|i| reg()[i].subject == figures::Subject::Physics) { 46.0 } else { 54.0 };
            } else if !skin && generated && self.shell_panel() {
                // the shell's icon toolbar (48 px on a phone, 53 wider, plus its rule) sits above the figure
                card_h += if ctx.content_rect().width() < 500.0 { 50.0 } else { 55.0 };
            }
            // the math reference leaves 16 px under its header on a phone; ours 11
            let math_top = if skin && self.sel.is_some_and(|i| matches!(reg()[i].subject, figures::Subject::Math | figures::Subject::Physics)) { 5.0 } else { 0.0 };
            ui.add_space(math_top);
            ScrollArea::vertical().id_salt("figs_narrow").max_height(avail_h - math_top).auto_shrink([false, false]).scroll_bar_visibility(if theme::chem_skin() { egui::scroll_area::ScrollBarVisibility::AlwaysHidden } else { egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded }).show(ui, |ui| {
                ui.scope_builder(egui::UiBuilder::new().max_rect(ui.available_rect_before_wrap().shrink2(Vec2::new(side, 0.0))), |ui| {
                if inset > 0.0 {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_space(inset);
                        self.canvas_card(ui, cw_card, card_h, generated, preview.as_ref());
                    });
                } else {
                    self.canvas_card(ui, w, card_h, generated, preview.as_ref());
                }
                ui.add_space(if skin && self.skin_subject() == figures::Subject::Biology { 5.0 } else if skin { 8.0 } else { 10.0 });
                ui.set_width(w);
                self.controls(ui, 1.0e5);
                });
            });
        } else if skin {
            self.chem_wide(ui, avail_h, preview.as_ref());
        } else {
            // the reference's chemistry sheet is 994 px wide at 1440 (its canvas starts 425 px from
            // the left and ends 21 px from the right); ours borrows the scrollbar gutter and a
            // slimmer controls column so the whole-canvas figures fit at the same scale
            let shell = generated && self.shell_panel();
            let chem_wide = !shell && generated && !self.modal && self.sel.is_some_and(|i| reg()[i].subject == figures::Subject::Chemistry);
            let math_fig = generated && !self.modal && self.sel.is_some_and(|i| reg()[i].subject == figures::Subject::Math);
            // the math reference's controls column is 377 px; ours gives up part of that to the rail
            let ctrl_w = if shell { 366.0 } else if chem_wide { 274.0 } else if math_fig && self.sel.is_some_and(|i| reg()[i].id == "composite-shape") { 340.0 } else { 290.0 };
            ui.horizontal_top(|ui| {
                if generated {
                    ui.allocate_ui(Vec2::new(ctrl_w, avail_h), |ui| {
                        ui.set_width(ctrl_w);
                        // the reference sets its controls in white rounded cards on the page
                        if shell {
                            // the math reference's cards sit straight on the page, no outer card
                            ui.set_width(ctrl_w);
                            self.controls(ui, avail_h);
                        } else {
                        Frame::new()
                            .fill(theme::card())
                            .stroke(Stroke::new(1.0, theme::border()))
                            .corner_radius(CornerRadius::same(crate::kit::R_CARD))
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.set_width(ctrl_w - 22.0);
                                self.controls(ui, avail_h - 40.0);
                            });
                        }
                    });
                    ui.add_space(12.0);
                }
                let w = ui.available_width() + if chem_wide { 8.0 } else { 0.0 };
                self.canvas_card(ui, w, avail_h, generated, preview.as_ref());
            });
        }
        if skin {
            help_fab(&ctx, true);
            if let Some(a) = self.tool_act.take() {
                act = a;
            }
            if self.examples_open && self.imp.is_none() && self.sel.is_some() {
                let a = self.examples_gallery(&ctx);
                if !matches!(a, Act::None) {
                    act = a;
                }
            }
            return act;
        }
        if !self.modal && self.imp.is_none() && self.sel.is_some() {
            help_fab(&ctx, false);
        }
        if let Some(a) = self.tool_act.take() {
            act = a;
        }
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
            let mt = generated && self.shell_panel();
            if !mt && ghost(ui, "Export PNG").clicked() {
                act = self.export(false);
            }
            if !mt && have_svg && ghost(ui, "Export SVG").clicked() {
                act = self.export(true);
            }
            if self.has_answers() && ghost(ui, "Student + key").on_hover_text("Save the student copy (answers hidden) and the answer key side by side").clicked() {
                act = self.export_pair(false);
            }
            if !mt && ghost(ui, "Copy PNG").clicked() {
                act = match self.output().ok().and_then(|o| color_image(&o.png)) {
                    Some(ci) => {
                        ctx.copy_image(ci);
                        Act::Toast("Copied PNG to the clipboard".into(), false)
                    }
                    None => Act::Toast("Couldn't render the figure".into(), true),
                };
            }
            if generated && self.sel.is_some() {
                self.history_and_presets_ui(ui, mt);
            }
            if generated && !mt {
                self.label_size_buttons(ui);
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
    /// The A / A / A label-size buttons of figures that have a three-way `label_size`.
    fn label_size_buttons(&mut self, ui: &mut Ui) {
        if let Some(ls) = self.sel.and_then(|si| (reg()[si].params)().into_iter().find(|d| d.key == "label_size"))
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
    }

    fn history_and_presets_ui(&mut self, ui: &mut Ui, toolbar_has_history: bool) {
        let Some(si) = self.sel else { return };
        let def = &reg()[si];
        let can_undo = !self.undo.is_empty() || self.hist_base.is_some();
        let can_redo = !self.redo.is_empty();
        if !toolbar_has_history && ui.add_enabled(can_undo, Button::new(RichText::new("Undo").size(13.0))).on_hover_text("Undo the last change (Ctrl+Z)").clicked() {
            self.step_history(true);
        }
        if !toolbar_has_history && ui.add_enabled(can_redo, Button::new(RichText::new("Redo").size(13.0))).on_hover_text("Redo (Ctrl+Shift+Z)").clicked() {
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
        if !toolbar_has_history {
            ui.menu_button("Share link", |ui| {
                ui.set_min_width(300.0);
                if ui.button("Copy link").on_hover_text("Copy a link that reopens this exact figure").clicked()
                    && let Some(l) = self.share_link()
                {
                    ui.ctx().copy_text(l);
                    self.toast = Some(("Link copied. Share it, and anyone who opens it sees this exact figure.".into(), false));
                    ui.close();
                }
                ui.label(theme::muted("Works offline: the link itself holds the settings.").size(11.0));
                ui.separator();
                if self.link_box(ui, 190.0) {
                    ui.close();
                }
            });
        }
        self.presets_menu(ui);
    }

    /// The "Presets" menu: saved presets, copy / paste of a spec.
    fn presets_menu(&mut self, ui: &mut Ui) {
        let Some(si) = self.sel else { return };
        let def = &reg()[si];
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
            ui.label(theme::muted("Saved presets stay in this browser.").size(11.0));
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

    /// The logic-circuit canvas editor; like the composite canvas, a click-sized edit is its own
    /// undo step and drags settle through `track_history`.
    fn logic_canvas(&mut self, ui: &mut Ui) {
        let before = self.params.clone();
        let ch = ui.vertical(|ui| self.logic.show(ui, &mut self.params)).inner;
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

    /// The graph canvas editor; same undo handling as the logic canvas.
    fn graph_canvas(&mut self, ui: &mut Ui) {
        let before = self.params.clone();
        let ch = ui.vertical(|ui| self.graph.show(ui, &mut self.params)).inner;
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

    /// The tree canvas editor; same undo handling as the graph canvas.
    fn tree_canvas(&mut self, ui: &mut Ui) {
        let before = self.params.clone();
        let ch = ui.vertical(|ui| self.tree.show(ui, &mut self.params)).inner;
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
        // the chemistry canvas has no hint strip (the reference has none); its help is on the
        // Handles chip, so the figure gets the whole frame
        let chem = def.subject == figures::Subject::Chemistry;
        let phys = def.subject == figures::Subject::Physics;
        let math = def.subject == figures::Subject::Math;
        // these chemistry figures draw on the reference's own canvas, which the reference fits whole
        // into its sheet with 16 px of padding (not zoomed to the ink); the rest are zoomed to the drawing
        let bio = def.subject == figures::Subject::Biology;
        let whole = (chem && chem_whole(def.id)) || (bio && theme::chem_skin() && self.chem_open());
        // the reference sizes the instrument figures to a slightly narrower box than its charts
        let reading = matches!(def.id, "gas-syringe" | "length-reading" | "mass-reading" | "ph-reading" | "temperature-reading" | "volume-by-displacement" | "volume-reading");
        let skin_wide = theme::chem_skin() && self.chem_open() && ui.ctx().content_rect().width() >= 880.0;
        // the reference's chart sheet is 994 px wide at 1440 (ours lays out 993), so its figure box gets the missing pixel
        let wide_chart = matches!(def.id, "titration-curve" | "heating-cooling-curve" | "mass-spectrum");
        let phys_whole_on = phys && phys_whole(def.id) && full.width() >= 600.0;
        let area = if phys_whole_on { let (bw, dx) = phys_whole_box(def.id); egui::Rect::from_center_size(full.center() + Vec2::new(dx, if theme::chem_skin() && self.chem_open() { -4.0 } else { 48.0 }), Vec2::new(bw, full.height())) } else if math { if theme::chem_skin() { egui::Rect::from_min_max(egui::pos2(full.left() + 16.0, full.top() + 16.0 - MATH_TOP_GAP), egui::pos2(full.right() - 16.0, full.bottom() - 16.0)) } else { full.shrink(4.0) } } else if whole && skin_wide { let a = full.shrink2(Vec2::new(16.0, 11.5)).translate(Vec2::new(0.0, if def.id == "predator-prey" { -5.0 } else { -4.0 })); if wide_chart { egui::Rect::from_min_max(a.min, egui::pos2(a.max.x + 0.0, a.max.y)) } else { a } } else if whole && theme::chem_skin() && self.chem_open() {
            // a phone: the reference's svg is as wide as the sheet allows (never wider than natural) and
            // as tall as the figure's own height, the drawing centred in that box
            let (cw0, ch0) = if self.canvas.0 > 1.0 { self.canvas } else { (1.0, 1.0) };
            let (bw, bh) = phone_box(def.id, (cw0, ch0), full.width() as f64 + 2.0, ui.ctx().content_rect().height() as f64);
            egui::Rect::from_min_size(egui::pos2(full.center().x - bw as f32 / 2.0, full.top() + 7.0 + if proportional_box(def.id) || matches!(def.id, "photoelectron-spectrum" | "punnett-square") { 2.0 } else { 0.0 }), Vec2::new(bw as f32, bh as f32))
        } else if whole { full.shrink2(Vec2::new(if reading { 14.5 } else { 4.0 }, 5.0)) } else if phys && phys_vb(def.id).is_some() { phys_area(if full.width() >= 600.0 { full } else { full.expand(11.0) }, def.id, ui.ctx().content_rect().height()) } else if chem || phys { full } else { egui::Rect::from_min_max(full.min, egui::pos2(full.max.x, full.max.y - 16.0)) };
        let nat = tex.size_vec2();
        let rs = if self.raster > 0.0 { self.raster } else { 1.5 };
        let (cw, ch) = if self.canvas.0 > 1.0 { self.canvas } else { ((nat.x / rs) as f64, (nat.y / rs) as f64) };
        // Zoom to the drawing, not the canvas: the inked box (plus a margin) fills the frame, as
        // the reference draws its figures. The view holds still while the pointer is down (a
        // drag or a slider must not rescale under the hand) and only refits when the drawing
        // leaves it or shrinks well inside it.
        let fresh = if phys_whole_on || (phys && phys_ref(def.id).is_none() && phys_vb(def.id).is_some()) { [0.0, 0.0, cw, ch] } else if math { math_view(self.content, def.id, (cw, ch)) } else if whole { [0.0, 0.0, cw, ch] } else if let (true, Some(pr), Some(b)) = (phys, phys_ref(def.id), self.content) { phys_view(pr, b, def.id) } else { view_box_padded(self.content, (cw, ch), if chem { Some(CHEM_PAD) } else if phys { self.content.map(|b| PHYS_PAD * (b[2] - b[0]).max(b[3] - b[1])) } else { None }) };
        let pointer_down = ui.input(|i| i.pointer.any_down());
        match self.view {
            Some(v) if pointer_down || keep_view(v, fresh) => {}
            _ => self.view = Some(fresh),
        }
        let v = self.view.unwrap_or(fresh);
        let (vw, vh) = ((v[2] - v[0]).max(1.0), (v[3] - v[1]).max(1.0));
        // screen points per SVG unit. The chemistry figures zoom up to 6x on a wide frame (the reference
        // has no cap there) and, on a narrow one (a phone), only shrink to fit and never grow past
        // natural size, as the reference does.
        let max_zoom = if math { if area.width() < 600.0 { 1.0 / math_unit(def.id) } else { 12.0 } } else if !chem { PREVIEW_MAX_ZOOM } else if area.width() < 600.0 && !skin_wide { 1.0 } else { 6.0 };
        let k = (area.width() as f64 / vw).min(area.height() as f64 / vh).clamp(0.05, max_zoom);
        let origin = area.center() - Vec2::new(((v[0] + vw / 2.0) * k) as f32, ((v[1] + vh / 2.0) * k) as f32);
        let ppp = ui.ctx().pixels_per_point();
        // chemistry previews are rasterized at exactly the device scale and drawn on whole pixels,
        // so one-pixel grid lines stay solid instead of smearing across two pixels
        // The reference's sheet sits on whole x pixels and 0.35 px below a pixel row in y at its desktop
        // size, which decides which hairlines of the titration grid land on two pixels (pH 8, 10 mL, 44 mL).
        let snap = chem || (bio && theme::chem_skin() && self.chem_open()) || (math && def.id == "coordinate-grid" && theme::chem_skin()) || (phys && matches!(def.id, "motion-graphs" | "waves"));
        let (origin, phase) = if snap {
            let o = egui::pos2((origin.x * ppp).round() / ppp, (origin.y * ppp).floor() / ppp);
            (o, (0.0, if phys { PHYS_PH } else if def.id == "titration-curve" { 0.33 } else if bio { 0.16 } else { 0.0 }))
        } else { (origin, (0.0, 0.0)) };
        if !pointer_down { self.want_phase = phase; }
        let img = if snap && (rs as f64 - k * ppp as f64).abs() < 0.01 * rs as f64 {
            // 1:1 texels to device pixels
            egui::Rect::from_min_size(origin, nat / ppp)
        } else {
            egui::Rect::from_min_size(origin, Vec2::new((cw * k) as f32, (ch * k) as f32))
        };
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        if phys && full.width() >= 600.0 {
            // the sheet's bottom corners stay round: the white raster must not square them off
            let r = crate::kit::R_CARD as f32;
            let top = egui::Rect::from_min_max(full.min, egui::pos2(full.right(), full.bottom() - r));
            let bot = egui::Rect::from_min_max(egui::pos2(full.left() + r, full.bottom() - r), egui::pos2(full.right() - r, full.bottom() - 1.0));
            ui.painter_at(top).image(tex.id(), img, uv, egui::Color32::WHITE);
            ui.painter_at(bot).image(tex.id(), img, uv, egui::Color32::WHITE);
        } else {
            ui.painter_at(full).image(tex.id(), img, uv, egui::Color32::WHITE);
        }
        if !pointer_down {
            let want = if snap {
                let cap = (2040.0 / cw.max(ch).max(1.0)).min((12.0e6 / (cw * ch).max(1.0)).sqrt());
                (k * ppp as f64).min(cap).clamp(0.2, 6.0) as f32
            } else {
                raster_for(k * ppp as f64, (cw, ch))
            };
            if (want - rs).abs() / rs > if snap { 0.003 } else { 0.15 } {
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
        let math_quiet = (def.subject == figures::Subject::Math && def.id != "composite-shape" && theme::chem_skin()) || def.subject == figures::Subject::Biology;
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
                // the math reference draws no resting knobs; they show on hover or drag only
                _ if !hot && math_quiet => {}
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
        if !hs.is_empty() && !chem && !theme::chem_skin() && !matches!(def.subject, figures::Subject::Math | figures::Subject::Physics) {
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

    /// The white frame holding the figure preview (or its spinner / error).
    fn canvas_card(&mut self, ui: &mut Ui, w: f32, h: f32, generated: bool, preview: Option<&TextureHandle>) {
        let shell = generated && self.shell_panel();
        let skin = (theme::chem_skin() && generated && self.chem_open()) || shell;
        let rr = ui.allocate_ui_with_layout(Vec2::new(w, h), if skin { Layout::top_down(Align::Min) } else { *ui.layout() }, |ui| {
            let fr = Frame::new()
                .fill(if skin && !shell { theme::card() } else { egui::Color32::WHITE })
                .stroke(Stroke::new(1.0, theme::border()))
                .corner_radius(CornerRadius::same(crate::kit::R_CARD))
                .inner_margin(if skin { Margin::ZERO } else { Margin::same(10) })
                .show(ui, |ui| {
                    ui.set_width((w - if skin { 2.0 } else { 22.0 }).max(100.0));
                    ui.set_height((h - if skin { 2.0 } else { 22.0 }).max(100.0));
                    if skin {
                        let a = self.chem_toolbar(ui);
                        if !matches!(a, Act::None) {
                            self.tool_act = Some(a);
                        }
                        ui.add_space(2.0);
                    }
                    let logic = generated && !self.hide_handles && self.sel.is_some_and(|si| reg()[si].id == "logic-circuit") && self.params.s("source") != "Boolean expression";
                    if logic {
                        // the mode row stacks over the canvas (the card itself lays out left to right)
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(3.0, 4.0);
                                if ui.add(graph_canvas::seg_btn("Edit circuit", !self.logic_preview)).on_hover_text("Drag gates, wire pins, click inputs to try them").clicked() {
                                    self.logic_preview = false;
                                }
                                if ui.add(graph_canvas::seg_btn("Preview", self.logic_preview)).on_hover_text("The figure as it will be exported").clicked() {
                                    self.logic_preview = true;
                                }
                            });
                            ui.add_space(2.0);
                            match (self.logic_preview, preview) {
                                (false, _) => self.logic_canvas(ui),
                                (true, Some(t)) => self.preview_interactive(ui, t),
                                _ => {}
                            }
                        });
                    } else if generated && !self.hide_handles && self.sel.is_some_and(|si| reg()[si].id == "graph-diagram") {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(3.0, 4.0);
                                if ui.add(graph_canvas::seg_btn("Edit graph", !self.graph_preview)).on_hover_text("Drag nodes, add nodes, draw edges and arrows").clicked() {
                                    self.graph_preview = false;
                                }
                                if ui.add(graph_canvas::seg_btn("Preview", self.graph_preview)).on_hover_text("The figure as it will be exported").clicked() {
                                    self.graph_preview = true;
                                }
                            });
                            ui.add_space(2.0);
                            match (self.graph_preview, preview) {
                                (false, _) => self.graph_canvas(ui),
                                (true, Some(t)) => self.preview_interactive(ui, t),
                                _ => {}
                            }
                        });
                    } else if generated && !self.hide_handles && self.sel.is_some_and(|si| reg()[si].id == "tree-diagram") {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(3.0, 4.0);
                                if ui.add(graph_canvas::seg_btn("Edit tree", !self.tree_preview)).on_hover_text("Add and delete nodes, rename them").clicked() {
                                    self.tree_preview = false;
                                }
                                if ui.add(graph_canvas::seg_btn("Preview", self.tree_preview)).on_hover_text("The figure as it will be exported").clicked() {
                                    self.tree_preview = true;
                                }
                            });
                            ui.add_space(2.0);
                            match (self.tree_preview, preview) {
                                (false, _) => self.tree_canvas(ui),
                                (true, Some(t)) => self.preview_interactive(ui, t),
                                _ => {}
                            }
                        });
                    } else if generated && !self.hide_handles && self.sel.is_some_and(|si| reg()[si].id == "composite-shape") {
                        self.composite_canvas(ui);
                    } else if generated && let Some(t) = preview {
                        self.preview_interactive(ui, t);
                    } else if let Some(t) = preview {
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
            if skin {
                let f = ui.ctx().cumulative_frame_nr();
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("share_stage"), (fr.response.rect, f)));
            }
        });
        let _ = rr;
    }

    /// Every generated figure in the standalone editor (all subjects; the composite shape keeps its
    /// own canvas layout) wears the page shell layout in the app's own theme: white cards for the
    /// controls column, section rows with icon tiles, the icon toolbar above the preview and a
    /// slimmer bottom row. The insert-figure dialog and imported pictures keep the plain layout.
    /// Checks `chem_open` rather than `theme::chem_skin`, so it reads the same inside the panel's
    /// `with_shell_layout` scope (the page skins themselves are off, see `chem_shell::SITE_SKINS`).
    pub(super) fn shell_panel(&self) -> bool {
        !self.chem_open() && !self.modal && self.imp.is_none() && self.sel.is_some_and(|i| reg()[i].id != "composite-shape")
    }

    fn controls(&mut self, ui: &mut Ui, max_h: f32) {
        if self.shell_panel() {
            // the shell's section rows take the math reference's offsets and chevron (one look for every subject)
            let flag = egui::Id::new("math_skin_page");
            ui.ctx().data_mut(|d| d.insert_temp(flag, true));
            theme::with_shell_layout(ui, |ui| self.controls_inner(ui, max_h));
            ui.ctx().data_mut(|d| d.insert_temp(flag, false));
        } else {
            self.controls_inner(ui, max_h);
        }
    }

    fn controls_inner(&mut self, ui: &mut Ui, max_h: f32) {
        let Some(si) = self.sel else { return };
        let def = &reg()[si];
        let pdefs = (def.params)();
        let skin = theme::chem_skin() && (self.chem_open() || self.shell_panel());
        // the app's own shell (one look for every subject), not a reference page skin
        let shell = skin && self.shell_panel();
        ui.vertical(|ui| {
            if !skin {
                ui.label(theme::muted(if def.subject == figures::Subject::Math { figures::chemistry::skin_intro(def.id).unwrap_or(def.blurb) } else { def.blurb }).size(12.0));
                ui.add_space(6.0);
            }
            let skin_phys = self.skin_subject() == figures::Subject::Physics;
            let params_now = self.params.clone();
            let mut content = |ui: &mut Ui| {
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
                    let chips = figures::chips_for(def.id).filter(|_| !figures::cards::embeds_chips(def.id) && def.id != "inclined-plane");
                    let classic_in_chips = chips.as_ref().is_some_and(|c| c.count_key.is_empty());
                    // the shell's first section row opens the card unless a card of controls sits above it
                    let mut first_row = figures::measures_for(def.id).is_none() && chips.is_none() && def.id != "pedigree";
                    if let Some(card) = chips {
                        self.chips_card(ui, def.id, &pdefs, &card);
                    }
                    // the pedigree's Family card: setup, New pedigree and the person editor
                    let classic_in_chips = classic_in_chips || def.id == "pedigree";
                    // the Punnett page opens on its setup block (Classic setups, Cross, Dominance, Parents), no card header
                    let top_keys: &[&str] = if skin && def.id == "punnett-square" { &["cross", "dominance", "parents"] } else { &[] };
                    let classic_in_chips = classic_in_chips || !top_keys.is_empty();
                    if !top_keys.is_empty() {
                        ui.add_space(15.5);
                        Frame::new().inner_margin(Margin::symmetric(18, 0)).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 8.0;
                            self.classic_combo(ui, def.id);
                            for k in top_keys {
                                if let Some(pd) = pdefs.iter().find(|d| d.key == *k) {
                                    param_control(ui, def.id, pd, &mut self.params);
                                    chem_note(ui, def.id, k, &self.params);
                                    if *k != "parents" {
                                        ui.add_space(7.0);
                                    }
                                }
                            }
                            ui.add_space(0.0);
                        });
                        // the biology page closes this setup card and opens a second one for the sections
                        chem_shell::mark_split(ui);
                    }
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
                    let secs = figures::layout::sections(if skin_phys && def.id == "length-reading" { "length-reading@physics" } else { def.id }, &pdefs, n_now);
                    if skin && def.id == "mass-spectrum" {
                        self.isotopes_card(ui, &pdefs);
                    }
                    for (si_, sec) in secs.iter().enumerate() {
                        if skin && def.id == "mass-spectrum" && sec.title == "Isotopes" {
                            continue;
                        }
                        let shown: Vec<usize> = sec.items.iter().copied().filter(|&i| !top_keys.contains(&pdefs[i].key) && !is_hidden(&pdefs, &self.params, &pdefs[i]) && !(skin && (def.subject == figures::Subject::Chemistry || matches!(def.id, "coordinate-grid" | "number-line")) && pdefs[i].key == "label_size") && !(skin && def.id == "micropipette-reading" && pdefs[i].key == "shape") && !(def.id == "gel-electrophoresis" && pdefs[i].key == "lane_names") && !(def.id == "inclined-plane" && (matches!(pdefs[i].key, "two" | "n") || (pdefs[i].key == "show_angle" && self.params.b("show_angle")))) && figures::param_relevant(def.id, pdefs[i].key, &self.params)).collect();
                        if shown.is_empty() {
                            continue;
                        }
                        let id = ui.make_persistent_id(("fig_sec", def.id, sec.title.as_str()));
                        let relevant: Vec<usize> = sec.items.iter().copied().filter(|&i| figures::param_relevant(def.id, pdefs[i].key, &self.params)).collect();
                        let summary = (skin || def.subject == figures::Subject::Physics || matches!(def.id, "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon")).then(|| figures::chemistry::skin_summary_in(def.id, &sec.title, &self.params, skin_phys)).flatten().or_else(|| sec.items.iter().find_map(|&i| figures::cards::summary(def.id, pdefs[i].key, &self.params))).or_else(|| if def.id == "population-growth" { figures::biology::popdyn::growth_section_summary(&sec.title, &self.params) } else { None }).or_else(|| if def.id == "pedigree" { figures::biology::pedigree::section_summary(&sec.title, &self.params) } else { None }).or_else(|| if def.id == "titration-curve" { figures::chemistry::tit_section_summary(&sec.title, &self.params) } else { None }).unwrap_or_else(|| section_summary(&pdefs, &self.params, &relevant));
                        let mut reset = false;
                        let (mut reset_labels, mut straighten) = (false, false);
                        let mut st = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, if skin { (def.id == "population-growth" && sec.title == "Graphs") || (def.id == "gel-electrophoresis" && si_ == 1) || (si_ == 0 && def.subject != figures::Subject::Physics && def.id != "pedigree" && !((shell || def.subject == figures::Subject::Math) && (figures::measures_for(def.id).is_some() || figures::rows_for(def.id).is_some()))) || sec.title.to_lowercase().contains("reading") } else { si_ < open_sections(def.id) });
                        let mut body = |ui: &mut Ui| {
                                // the chemistry page's blocks sit 12 px apart (5.2 px item spacing plus this), number rows 9.6
                                let chem_pg = def.subject == figures::Subject::Chemistry;
                                let (gap, pair_gap) = if chem_pg { (6.0, 3.3) } else if skin && def.id == "population-growth" { (6.0, 0.0) } else { (6.0, 6.0) };
                                let help = figures::sections::live_help(def.id, &sec.title, &sec.help, &self.params);
                                // the chemistry page sets the note under the first control, not above it
                                let mut help_left = !help.is_empty();
                                // the biology page sets a section's note above its first control, in the page's 13 px grey
                                if help_left && skin && matches!(def.id, "punnett-square" | "free-body-diagram" | "vector-diagram") {
                                    if def.id != "punnett-square" {
                                        ui.add_space(if def.id == "vector-diagram" { 1.0 } else { 2.0 });
                                    }
                                    ui.add(egui::Label::new(theme::muted(help.as_str()).size(13.6)).wrap());
                                    ui.add_space(gap);
                                    help_left = false;
                                }
                                if help_left && !skin {
                                    ui.label(theme::muted(help.as_str()).size(11.0));
                                    ui.add_space(4.0);
                                    help_left = false;
                                }
                                if si_ == 0 && !classic_in_chips {
                                    self.classic_combo(ui, def.id);
                                }
                                let pair_ok = |i: usize| skin && (matches!(def.id, "titration-curve" | "heating-cooling-curve") || (def.id == "population-growth" && matches!(pdefs[i].key, "n0" | "r" | "k" | "tspan"))) && matches!(pdefs[i].kind, Kind::Float { .. });
                                // the biology page's "Time in" select takes one half-width column of its own
                                let half_alone_of = |i: usize| skin && ((def.id == "population-growth" && pdefs[i].key == "unit") || (def.id == "predator-prey" && pdefs[i].key == "cycle"));
                                // the predator-prey card sets prey | predators side by side in each row
                                let pp_pair = |i: usize, j: usize| skin && def.id == "predator-prey" && matches!((pdefs[i].key, pdefs[j].key), ("prey", "pred") | ("prey0", "pred0") | ("prey_avg", "pred_avg") | ("tshow", "unit"));
                                let mut skip_pair: Option<usize> = None;
                                for (pos, &i) in shown.iter().enumerate() {
                                    if skip_pair == Some(i) {
                                        continue;
                                    }
                                    if skin && def.id == "lewis-structures" && matches!(pdefs[i].key, "central" | "tweak") {
                                        // the Changes section: its note above, then central atom, atoms and bonds
                                        if pdefs[i].key == "central" {
                                            self.lewis_changes(ui, &help);
                                        }
                                        help_left = false;
                                        continue;
                                    }
                                    let half_alone = half_alone_of(i);
                                    let partner = if half_alone { None } else { shown.get(pos + 1).copied().filter(|&j| (pair_ok(i) && pair_ok(j)) || pp_pair(i, j)) };
                                    if half_alone || partner.is_some() {
                                        // the chemistry page sets number boxes two to a row
                                        let ks: Vec<usize> = std::iter::once(i).chain(partner).collect();
                                        skip_pair = partner;
                                        let w = (ui.available_width() - 10.0) / 2.0;
                                        let (mut olds, mut news) = (Vec::new(), Vec::new());
                                        ui.horizontal_top(|ui| {
                                            ui.spacing_mut().item_spacing.x = 10.0;
                                            for &k in &ks {
                                                ui.vertical(|ui| {
                                                    ui.set_width(w);
                                                    if chem_pg {
                                                        ui.spacing_mut().item_spacing.y = 5.0;
                                                    }
                                                    olds.push(self.params.0.get(pdefs[k].key).cloned());
                                                    param_control(ui, def.id, &pdefs[k], &mut self.params);
                                                    news.push(self.params.0.get(pdefs[k].key).cloned());
                                                });
                                            }
                                        });
                                        for (n, &k) in ks.iter().enumerate() {
                                            self.card_after(ui, def.id, pdefs[k].key, &pdefs);
                                            if let Some(old) = olds[n].clone().filter(|o| news[n].as_ref() != Some(o)) {
                                                figures::after_panel_change(def.id, pdefs[k].key, &old, &mut self.params);
                                            }
                                        }
                                        ui.add_space(if skin && def.id == "population-growth" { match pdefs[i].key { "n0" => 1.0, "k" => 1.0, "unit" => 3.0, _ => 0.0 } } else if skin && def.id == "predator-prey" { match pdefs[i].key { "prey" => 4.0, "prey0" => 2.0, "prey_avg" => 3.0, "cycle" => 5.0, "tshow" => 8.0, _ => pair_gap } } else { pair_gap });
                                        continue;
                                    }
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
                                    if skin && def.id == "orbital-diagram" && pdefs[i].key == "charge" {
                                        continue;
                                    }
                                    if skin && def.id == "orbital-diagram" && pdefs[i].key == "element" {
                                        let ci = pdefs.iter().position(|d| d.key == "charge");
                                        let old = self.params.0.get("element").cloned();
                                        let w = ui.available_width();
                                        ui.horizontal_top(|ui| {
                                            ui.spacing_mut().item_spacing.x = 12.0;
                                            ui.vertical(|ui| {
                                                ui.set_width((w - 92.0).max(120.0));
                                                param_control(ui, def.id, &pdefs[i], &mut self.params);
                                            });
                                            if let Some(ci) = ci {
                                                ui.vertical(|ui| {
                                                    ui.set_width(80.0);
                                                    ui.label(theme::strong("Charge").size(13.5));
                                                    let (lo, hi) = match &pdefs[ci].kind { Kind::Int { min, max } => (*min, *max), _ => (-4, 8) };
                                                    let tid = egui::Id::new(("orb_charge_txt", def.id));
                                                    let cur = self.params.i("charge");
                                                    let mut txt = ui.ctx().data(|d| d.get_temp::<String>(tid)).filter(|t| t.parse::<i64>().ok().map(|v| v.clamp(lo, hi)) == Some(cur) || t == "-" || t.is_empty()).unwrap_or_else(|| cur.to_string());
                                                    let te = TextEdit::singleline(&mut txt).desired_width(f32::INFINITY).margin(Margin::symmetric(14, 12)).font(egui::FontId::new(15.0, theme::sans_family()));
                                                    if ui.add(te).changed() {
                                                        if let Ok(v) = txt.trim().parse::<i64>() {
                                                            self.params.0.insert("charge".to_string(), Value::I(v.clamp(lo, hi)));
                                                        }
                                                    }
                                                    ui.ctx().data_mut(|d| d.insert_temp(tid, txt));
                                                });
                                            }
                                        });
                                        if let Some(old) = old.filter(|o| self.params.0.get("element") != Some(o)) {
                                            figures::after_panel_change(def.id, "element", &old, &mut self.params);
                                        }
                                        ui.add_space(gap);
                                        continue;
                                    }
                                    if self.card_control(ui, def.id, pdefs[i].key, &pdefs) {
                                        if skin {
                                            chem_note(ui, def.id, pdefs[i].key, &self.params);
                                        }
                                        ui.add_space(gap);
                                        continue;
                                    }
                                    let old = self.params.0.get(pdefs[i].key).cloned();
                                    let tip_top = ui.cursor().top();
                                    param_control(ui, def.id, &pdefs[i], &mut self.params);
                                    if skin && ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false) {
                                        if let Some(t) = figures::sections::phys_tip(def.id, pdefs[i].key, &self.params) {
                                            help_tip(ui, tip_top, t);
                                        }
                                    }
                                    if let Some(old) = old.filter(|o| self.params.0.get(pdefs[i].key) != Some(o)) {
                                        figures::after_panel_change(def.id, pdefs[i].key, &old, &mut self.params);
                                    }
                                    self.card_after(ui, def.id, pdefs[i].key, &pdefs);
                                    if skin {
                                        chem_note(ui, def.id, pdefs[i].key, &self.params);
                                    }
                                    if pdefs[i].key == "rotate"
                                        && figures::labels::turnable(def.id)
                                        && ui.add_enabled(self.params.f("rotate") != 0.0, Button::new(RichText::new("Straighten").size(12.0))).on_hover_text("Turn the figure back to 0°").clicked()
                                    {
                                        straighten = true;
                                    }
                                    if skin && chem_pg && pdefs[i].label.contains('|') {
                                        ui.add_space(0.9);
                                    }
                                    if help_left {
                                        if def.subject == figures::Subject::Biology && skin {
                                            ui.add_space(3.0);
                                            ui.add(egui::Label::new(theme::muted(help.as_str()).size(13.6).line_height(Some(19.72))).wrap());
                                        } else {
                                            ui.label(theme::muted(help.as_str()).size(if chem_pg { 12.8 } else { 13.0 }));
                                        }
                                        // the reference tucks a checkbox 2 px closer to the note above it
                                        let next_bool = chem_pg && shown.get(pos + 1).is_some_and(|&j| matches!(pdefs[j].kind, Kind::Bool));
                                        ui.add_space(if def.subject == figures::Subject::Biology && skin { gap - 7.0 } else if next_bool { gap - 2.0 } else { gap });
                                        help_left = false;
                                    }
                                    if figures::random_key(def.id) == Some(pdefs[i].key)
                                        && !skin
                                        && ui.button(RichText::new("Random").size(12.0)).on_hover_text("Set a random reading").clicked()
                                    {
                                        figures::random_reading(def.id, &mut self.params, random_seed());
                                    }
                                    let pop_adj = if skin && def.id == "population-growth" { match pdefs[i].key { "growth" => 12.0, "organism" => 9.0, _ => 0.0 } } else if skin && def.id == "predator-prey" { match pdefs[i].key { "pair" => 12.0, "source" => -2.0, _ => 0.0 } } else if skin && def.id == "micropipette-reading" && pdefs[i].key == "volume" { -6.0 } else { 0.0 };
                                    ui.add_space(if chem_pg && pos + 1 == shown.len() && matches!(pdefs[i].kind, Kind::Choice(_)) { gap - 5.0 } else { gap + pop_adj });
                                }
                                ui.add_space(if chem_pg { 1.8 } else if skin && def.id == "punnett-square" { 0.0 } else if skin && def.subject == figures::Subject::Biology { -3.0 } else { 4.0 });
                            };
                        if skin && si_ == 0 && matches!((def.id, sec.title.as_str()), ("titration-curve", "Titration") | ("heating-cooling-curve", "Curve") | ("population-growth", "Population") | ("predator-prey", "Predator and prey")) {
                            // these two pages open on a flat, always-open section with a small caps title
                            chem_shell::flat_header(ui, &sec.title, &sec.help);
                            Frame::new().inner_margin(Margin::symmetric(18, 0)).show(ui, |ui| body(ui));
                            // the chemistry card closes 17 px under its last box
                            if def.subject == figures::Subject::Chemistry {
                                chem_shell::mark_split(ui);
                            } else {
                                ui.add_space(14.0);
                            }
                        } else if skin {
                            let open = st.is_open();
                            // the chemistry rows butt against each other
                            let gap_y = ui.spacing().item_spacing.y;
                            if !shell && matches!(def.subject, figures::Subject::Chemistry | figures::Subject::Physics) {
                                ui.spacing_mut().item_spacing.y = 0.0;
                            }
                            // a row that opens its card sits flush on the card's top edge (no rule of its own)
                            if (!shell && matches!(def.subject, figures::Subject::Biology | figures::Subject::Physics) && si_ == 0) || (shell && first_row) {
                                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("chem_first_row"), true));
                            }
                            first_row = false;
                            let resp = chem_shell::section_header(ui, &sec.title, &summary, open);
                            ui.spacing_mut().item_spacing.y = gap_y;
                            if resp.clicked() {
                                st.toggle(ui);
                            }
                            st.show_body_unindented(ui, |ui| {
                                Frame::new().inner_margin(Margin::symmetric(18, 0)).show(ui, |ui| {
                                    ui.add_space(if shell && def.id != "punnett-square" { 2.0 } else if def.subject == figures::Subject::Chemistry { 3.0 } else if def.id == "punnett-square" { 5.5 } else if def.subject == figures::Subject::Biology { if si_ == 0 { -5.0 } else { -1.0 } } else { 2.0 });
                                    body(ui);
                                });
                            });
                            st.store(ui.ctx());
                        } else {
                            let header = |ui: &mut Ui| {
                                ui.vertical(|ui| {
                                    // leave room for Reset on the right: a long summary wraps
                                    // short of it instead of running underneath
                                    ui.set_max_width((ui.available_width() - SECTION_RESET_W).max(80.0));
                                    ui.label(theme::strong(sec.title.as_str()).size(13.0));
                                    if !summary.is_empty() {
                                        // one line, as the reference keeps its captions
                                        ui.add(egui::Label::new(theme::muted(summary).size(11.0)).truncate());
                                    }
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let r = ui.add(Button::new(theme::muted("Reset").size(11.0)).frame(false));
                                    reset = r.on_hover_text("Reset this section to its defaults").clicked();
                                });
                            };
                            st.show_header(ui, header).body(body);
                        }
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
                        if !skin {
                            ui.separator();
                        }
                    }
                    if !skin {
                        faq_section(ui, def.id);
                    }
            };
            if skin {
                let mut start_example = false;
                let mut reset_all = false;
                let mut preset_apply: Option<Params> = None;

                ui.spacing_mut().scroll.fade.strength = 0.0;
                ScrollArea::vertical().id_salt("figs_controls").max_height(max_h).auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    // the shell sets every subject's intro card and gaps as the math page does; the
                    // reference page skins keep their own
                    let chem_page = !shell && def.subject == figures::Subject::Chemistry;
                    if chem_page && ui.ctx().content_rect().width() >= 880.0 {
                        // the reference's side panel starts 2 px lower than ours did
                        ui.add_space(2.0);
                    }
                    let bio_page = !shell && def.subject == figures::Subject::Biology;
                    let phys_page = !shell && def.subject == figures::Subject::Physics;
                    if bio_page {
                        ui.add_space(2.0);
                    }
                    chem_shell::panel_card_m(ui, if chem_page { Margin { left: 18, right: 18, top: 17, bottom: 16 } } else if bio_page || phys_page { Margin { left: 18, right: 18, top: 17, bottom: 16 } } else { Margin::same(18) }, |ui| {
                        let intro = RichText::new(figures::chemistry::skin_intro_in(def.id, skin_phys).unwrap_or(def.blurb)).size(if shell || def.subject == figures::Subject::Math { 14.5 } else if chem_page || bio_page || phys_page { 14.72 } else { 15.0 }).color(theme::ink());
                        // the chemistry page sets its lead at 1.5 line height
                        ui.label(if chem_page { intro.line_height(Some(22.0)) } else if bio_page { intro.line_height(Some(22.08)) } else if phys_page { intro.line_height(Some(22.0)) } else { intro });
                        ui.add_space(if chem_page { 2.0 } else if bio_page { 1.9 } else if phys_page { 2.0 } else { 4.0 });
                        let has_ex = !figures::examples_for(def.id).is_empty();
                        if chem_shell::outline_button(ui, &format!("{}  Start from an example", egui_phosphor::regular::IMAGES), has_ex).clicked() {
                            start_example = true;
                        }
                    });
                    ui.add_space(if chem_page && !matches!(def.id, "titration-curve" | "heating-cooling-curve" | "mass-spectrum") { 5.0 } else if chem_page || bio_page || phys_page { 6.0 } else { 5.0 });
                    // (these flat first sections close their card with `mark_split`, whatever the skin)
                    let chem_fig = def.subject == figures::Subject::Chemistry;
                    if (chem_fig && matches!(def.id, "titration-curve" | "heating-cooling-curve" | "mass-spectrum")) || (skin && def.id == "punnett-square") || (skin && def.id != "composite-shape" && figures::rows_for(def.id).is_some()) {
                        // the reference's flat first section is a card of its own, the collapsible ones a second
                        chem_shell::split_cards(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            content(ui);
                        });
                    } else {
                        chem_shell::panel_card(ui, 0, |ui| {
                            ui.spacing_mut().item_spacing.y = if chem_page { 6.0 } else { 8.0 };
                            content(ui);
                        });
                    }
                    if shell || matches!(def.subject, figures::Subject::Physics | figures::Subject::Chemistry | figures::Subject::Biology | figures::Subject::Math) {
                        ui.add_space(if chem_page || phys_page { 6.0 } else { 7.0 });
                        preset_apply = preset_card(ui, def, &params_now);
                    }
                    if shell {
                        // the FAQ closes the column in a card of its own
                        ui.add_space(7.0);
                        faq_card(ui, def.id);
                    } else if !matches!(def.subject, figures::Subject::Chemistry | figures::Subject::Biology | figures::Subject::Math | figures::Subject::Physics) {
                        ui.add_space(4.0);
                        if ui.add(Button::new(theme::muted("Reset all").size(13.0)).frame(false)).clicked() {
                            reset_all = true;
                        }
                        ui.add_space(12.0);
                        faq_section(ui, def.id);
                    }
                    ui.add_space(24.0);
                });
                if let Some(p) = preset_apply {
                    self.set_params_with_history(p);
                }
                if start_example {
                    self.examples_open = true;
                }
                if reset_all {
                    self.params = figures::start_params(def);
                }
            } else {
                ScrollArea::vertical()
                    .id_salt("figs_controls")
                    .max_height((max_h - 70.0).max(120.0))
                    .auto_shrink([false, true])
                    .show(ui, content);
                ui.add_space(4.0);
                if ghost(ui, "Reset all").clicked() {
                    self.params = figures::start_params(def);
                }
            }
        });
    }

    /// The mass spectrum's Isotopes card: Element / Custom isotopes, the element (or the typed
    /// isotopes) and the relative atomic mass worked out beneath.
    fn isotopes_card(&mut self, ui: &mut Ui, pdefs: &[ParamDef]) {
        // the reference's Isotopes block is the first card of the panel: no card of its own here
        ui.add_space(15.0);
        Frame::new().inner_margin(Margin::symmetric(18, 0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 10.0;
            ui.label(RichText::new("ISOTOPES".chars().flat_map(|c| [c, '\u{200A}']).collect::<String>()).family(theme::bold_family()).size(12.5).color(theme::muted_col()));
            let mut src = if self.params.s("source") == "Custom isotopes" { "Custom isotopes".to_string() } else { "Element".to_string() };
            chem_shell::segmented_track(ui, &["Element", "Custom isotopes"], &mut src);
            if src != self.params.s("source") {
                self.params.0.insert("source".to_string(), Value::S(src.clone()));
            }
            let custom = src == "Custom isotopes";
            for k in if custom { ["custom_name", "custom"] } else { ["element", ""] } {
                if let Some(pd) = pdefs.iter().find(|d| d.key == k) {
                    param_control(ui, "mass-spectrum", pd, &mut self.params);
                }
            }
            if let Some((head, working)) = figures::chemistry::mass_spectrum_info(&self.params) {
                Frame::new().fill(if theme::keep_theme() { theme::accent_faded() } else if theme::is_dark() { egui::Color32::from_rgb(0x1a, 0x25, 0x3d) } else { egui::Color32::from_rgb(0xe8, 0xf1, 0xfd) }).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 5.0;
                    ui.label(theme::strong(head.as_str()).size(13.5));
                    if !working.is_empty() {
                        ui.label(RichText::new(working).size(12.5).color(theme::ink()));
                    }
                });
            }
        });
        ui.add_space(12.0);
        chem_shell::mark_split(ui);
    }

    /// The reference's "Measures" card: one row per side/angle/dimension with its value box and a
    /// label popover. A typed value is "given"; an empty box shows the worked-out value in grey.
    fn measures_card(&mut self, ui: &mut Ui, fig: &str, pdefs: &[ParamDef], card: &figures::MeasureCard) {
        let computed: HashMap<&'static str, String> = figures::computed_for(fig, &self.params).into_iter().collect();
        let skin = theme::chem_skin();
        Frame::new()
            .fill(theme::card())
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(if skin { Margin { left: 18, right: 18, top: 11, bottom: 9 } } else { Margin::same(10) })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if skin {
                    ui.spacing_mut().item_spacing.y = 7.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(card.title.to_uppercase().chars().flat_map(|c| [c, '\u{200A}']).collect::<String>()).family(theme::bold_family()).size(12.5).color(theme::muted_col()));
                        if !card.help.is_empty() {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add_space(4.0);
                                ui.label(RichText::new(egui_phosphor::regular::QUESTION).size(18.0).color(theme::muted_col())).on_hover_text(card.help);
                            });
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(card.title.to_uppercase().chars().flat_map(|c| [c, '\u{200A}']).collect::<String>()).family(theme::bold_family()).size(12.5).color(theme::muted_col()));
                        if !card.help.is_empty() {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(egui_phosphor::regular::QUESTION).size(17.0).color(theme::muted_col())).on_hover_text(card.help);
                            });
                        }
                    });
                }
                for (gi, g) in card.groups.iter().enumerate() {
                    if !figures::card_visible(fig, g.title, None, &self.params) {
                        continue;
                    }
                    if !g.title.is_empty() {
                        ui.add_space(if skin { if gi == 0 { 0.0 } else { 3.0 } } else { 6.0 });
                        ui.label(theme::strong(g.title).size(if skin { 13.0 } else { 12.5 }));
                    }
                    for r in &g.rows {
                        if !figures::card_visible(fig, g.title, Some(r.value_key), &self.params) {
                            continue;
                        }
                        self.measure_row(ui, pdefs, r, &computed);
                        ui.add_space(if skin { 0.0 } else { 2.0 });
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
        // the math pages draw this flat inside the first card (letter-spaced title, a "?" circle,
        // 44 px rows, a dashed Add pill); the card is closed under it by `mark_split`
        let skin = theme::chem_skin() && (self.chem_open() || self.shell_panel());
        let add_text = format!("{}  {}", egui_phosphor::regular::PLUS, card.add_label);
        let fr = if skin { Frame::new().inner_margin(Margin::symmetric(18, 0)) } else { Frame::new().fill(theme::card()).stroke(Stroke::new(1.0, theme::border())).corner_radius(CornerRadius::same(12)).inner_margin(Margin::same(10)) };
        if skin {
            chem_shell::flat_header(ui, card.title, card.help);
        }
        fr
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if skin {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                } else {
                    ui.horizontal(|ui| {
                        crate::kit::small_caps(ui, card.title);
                        if !card.help.is_empty() {
                            ui.label(theme::muted("(?)").size(11.0)).on_hover_text(card.help);
                        }
                    });
                    ui.add_space(4.0);
                }
                let edit_w = if skin { ui.available_width() - 46.0 - 40.0 - 16.0 } else { 170.0 };
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
                                let ck = color_keys.get(i).filter(|_| true);
                                if skin && let Some(ck) = ck && let Some(pal) = palette(ck) {
                                    self.row_colour(ui, ck, &pal, skin);
                                }
                                ui.add(row_edit(row, card.hint, edit_w, skin));
                                if !skin && let Some(ck) = ck && let Some(pal) = palette(ck) {
                                    self.row_colour(ui, ck, &pal, skin);
                                }
                                if ui.add(Button::new(theme::muted(egui_phosphor::regular::X).size(if skin { 17.0 } else { 13.0 })).frame(false)).on_hover_text("Remove this row").clicked() {
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
                        if (if skin { chem_shell::dashed_chip(ui, &add_text) } else { ui.button(card.add_label) }).clicked() && color_keys.len().max(8) > rows.len() {
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
                                if skin && let Some(ck) = sl.color_key && let Some(pal) = palette(ck) {
                                    self.row_colour(ui, ck, &pal, skin);
                                }
                                let mut v = self.params.s(sl.value_key).to_string();
                                if ui.add(row_edit(&mut v, card.hint, if sl.name_key.is_some() { edit_w - 76.0 } else { edit_w }, skin)).changed() {
                                    self.params.0.insert(sl.value_key.to_string(), Value::S(v));
                                }
                                if !skin && let Some(ck) = sl.color_key && let Some(pal) = palette(ck) {
                                    self.row_colour(ui, ck, &pal, skin);
                                }
                                if ui.add(Button::new(theme::muted(egui_phosphor::regular::X).size(if skin { 17.0 } else { 13.0 })).frame(false)).on_hover_text("Clear this row").clicked() {
                                    self.params.0.insert(sl.value_key.to_string(), Value::S(String::new()));
                                }
                            });
                        }
                        if shown < slots.len() && (if skin { chem_shell::dashed_chip(ui, &add_text) } else { ui.button(card.add_label) }).clicked() {
                            extra = shown;
                        }
                    }
                }
            });
        ui.data_mut(|d| d.insert_temp(extra_id, extra));
        if skin {
            ui.add_space(15.0);
            chem_shell::mark_split(ui);
        } else {
            ui.add_space(8.0);
        }
    }

    /// "Add a force" chips: each appends one preset item and bumps the count.
    fn chips_card(&mut self, ui: &mut Ui, fig: &str, pdefs: &[ParamDef], card: &figures::ChipsCard) {
        let n = self.params.i(card.count_key);
        // the shell sets it as the Measures card: letter-spaced title, outlined chips
        let skin = theme::chem_skin();
        Frame::new()
            .fill(theme::card())
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(if skin { Margin { left: 18, right: 18, top: 13, bottom: 13 } } else { Margin::same(10) })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if skin {
                        ui.label(RichText::new(card.title.to_uppercase().chars().flat_map(|c| [c, '\u{200A}']).collect::<String>()).family(theme::bold_family()).size(12.5).color(theme::muted_col()));
                    } else {
                        crate::kit::small_caps(ui, card.title);
                    }
                    if !card.count_key.is_empty() {
                        ui.label(theme::muted(format!("{n} of {}", card.max)).size(if skin { 13.0 } else { 11.0 }));
                    }
                });
                ui.add_space(if skin { 6.0 } else { 4.0 });
                let slotted = !card.count_key.is_empty();
                let full = slotted && n >= card.max;
                ui.horizontal_wrapped(|ui| {
                    if skin {
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                    }
                    for chip in &card.chips {
                        let label = format!("+ {}", chip.label);
                        let r = if skin { ui.add_enabled_ui(!full, |ui| chem_shell::outline_chip(ui, &label)).inner } else { ui.add_enabled(!full, Button::new(RichText::new(label).size(12.0)).corner_radius(CornerRadius::same(10))) };
                        if r.clicked() {
                            apply_chip(&mut self.params, pdefs, card, chip);
                        }
                    }
                    if slotted && n > 1 && ui.add(Button::new(theme::muted("Remove the last").size(if skin { 13.0 } else { 12.0 })).frame(false)).clicked() {
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
        if theme::chem_skin() {
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("pop_label_tight"), if fig == "population-growth" { 2.0_f32 } else { 0.0_f32 }));
            param_label(ui, "Classic setups", "");
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("pop_label_tight"), 0.0_f32));
            if fig == "punnett-square" {
                ui.add_space(1.0);
            }
        } else {
            ui.label(theme::muted("Classic setups").size(11.0));
        }
        // the dropdown shows the setup the figure is on, as the reference does ("Choose one…" once edited away)
        let touched: std::collections::BTreeSet<&str> = setups.iter().flat_map(|e| e.params.iter().map(|(k, _)| *k)).collect();
        let current = setups.iter().position(|e| {
            let q = example_params(def, e);
            touched.iter().all(|k| q.0.get(*k) == self.params.0.get(*k))
        });
        let mut pick = None;
        let saved_pad = ui.spacing().button_padding.x;
        if theme::chem_skin() {
            ui.spacing_mut().button_padding.x = 15.0;
        }
        let bio = theme::chem_skin() && def.subject == figures::Subject::Biology;
        let saved_pad_y = ui.spacing().button_padding.y;
        let saved_h = ui.spacing().interact_size.y;
        if bio {
            // the biology page's select: 40 px tall, 15.2 px text
            ui.spacing_mut().button_padding.y = 9.5;
            ui.spacing_mut().interact_size.y = 39.5;
        }
        let cb = egui::ComboBox::from_id_salt(("fig_classic", fig));
        let cb = if theme::chem_skin() { cb.icon(chem_shell::select_chevron) } else { cb };
        let shown = current.map(|i| setups[i].name).unwrap_or("Choose one…");
        cb
            .selected_text(if bio { RichText::new(shown).size(15.2).family(theme::sans_family()) } else { RichText::new(shown) })
            .width(if theme::chem_skin() { ui.available_width() } else { ui.available_width().min(260.0) })
            .show_ui(ui, |ui| {
                for (i, e) in setups.iter().enumerate() {
                    if ui.selectable_label(current == Some(i), e.name).clicked() {
                        pick = Some(i);
                    }
                }
            });
        ui.spacing_mut().button_padding.x = saved_pad;
        ui.spacing_mut().button_padding.y = saved_pad_y;
        ui.spacing_mut().interact_size.y = saved_h;
        if let Some(i) = pick {
            self.set_params_with_history(example_params(def, &setups[i]));
        }
        ui.add_space(if fig == "punnett-square" { 6.0 } else if bio && fig == "population-growth" { 17.0 } else if bio { 0.16 } else { 6.0 });
    }

    /// A colour swatch that opens a palette popover for one param.
    fn row_colour(&mut self, ui: &mut Ui, key: &str, pal: &[&'static str], skin: bool) {
        let cur = self.params.s(key).to_string();
        let (r, resp) = ui.allocate_exact_size(if skin { Vec2::new(46.0, 45.0) } else { Vec2::splat(22.0) }, Sense::click());
        if skin {
            ui.painter().rect(r, CornerRadius::same(12), if resp.hovered() { theme::hover_bg() } else { theme::card() }, Stroke::new(1.0, chem_shell::pill_border()), egui::StrokeKind::Inside);
            if key.starts_with("eq") && key.ends_with("_color") {
                // the reference's equation row shows a line-style icon (a diagonal with arrowheads),
                // drawn in the equation's colour
                let col = egui::Color32::from_rgb(0x11, 0x18, 0x27);
                let c = r.center();
                let (a, b) = (c + Vec2::new(-8.0, 8.0), c + Vec2::new(8.0, -8.0));
                ui.painter().line_segment([a, b], Stroke::new(2.4, col));
                let d = Vec2::new(1.0, -1.0) / 2f32.sqrt();
                let n = Vec2::new(d.y, -d.x);
                for (tip, dir) in [(b + d * 3.0, d), (a - d * 3.0, -d)] {
                    let base = tip - dir * 7.0;
                    ui.painter().add(egui::Shape::convex_polygon(vec![tip, base + n * 4.0, base - n * 4.0], col, Stroke::NONE));
                }
            } else {
                ui.painter().circle_filled(r.center(), 7.0, swatch(&cur));
            }
        } else {
            ui.painter().circle_filled(r.center(), 8.0, swatch(&cur));
            ui.painter().circle_stroke(r.center(), 9.5, Stroke::new(1.0, theme::border()));
        }
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
        let skin = theme::chem_skin();
        // the reference's rows are 46 px tall: boxed 42 px letter tiles, 152 px inputs, 83 px label buttons
        let rh = if skin { 46.0 } else { 32.0 };
        let serif = theme::serif_family();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = if skin { 8.0 } else { 5.0 };
            ui.spacing_mut().interact_size.y = rh;
            // the reference's inputs are white boxes, not tinted fields
            if !skin && !theme::is_dark() {
                ui.visuals_mut().extreme_bg_color = egui::Color32::WHITE;
                ui.visuals_mut().text_edit_bg_color = Some(egui::Color32::WHITE);
            }
            // Name: an editable vertex letter, or a fixed label.
            if let Some(nk) = r.name_key {
                let mut v = self.params.s(nk).to_string();
                let te = if skin {
                    TextEdit::singleline(&mut v)
                        .desired_width(26.0)
                        .horizontal_align(Align::Center)
                        .vertical_align(Align::Center)
                        .min_size(Vec2::new(42.0, rh))
                        .font(egui::FontId::new(18.0, theme::italic_family(ui.ctx(), &serif).unwrap_or(serif.clone())))
                } else {
                    // a serif-italic letter in a square tile, as the reference sets its vertex names
                    TextEdit::singleline(&mut v)
                        .desired_width(16.0)
                        .horizontal_align(Align::Center)
                        .vertical_align(Align::Center)
                        .min_size(Vec2::new(34.0, rh))
                        .font(egui::FontId::new(16.0, theme::italic_family(ui.ctx(), &serif).unwrap_or(serif.clone())))
                };
                if ui.add(te).changed() {
                    self.params.0.insert(nk.to_string(), Value::S(v.chars().take(3).collect()));
                }
                ui.label(theme::muted(r.name).size(if skin { 17.0 } else { 14.0 }));
            } else if skin {
                // a side name (AB, BC, CA) sits bare in an angle-row-wide slot
                let nm = r.name.trim();
                let fam = theme::italic_family(ui.ctx(), &serif).unwrap_or(serif.clone());
                ui.add_sized([if nm == "∠" { 50.0 } else { 50.0 }, rh], egui::Label::new(RichText::new(nm).family(fam).size(18.0).color(theme::text_col())));
            } else {
                // egui's bundled font has no "∠"; spell it out.
                let nm = if r.name.trim() == "∠" { "angle" } else { r.name };
                let fam = theme::italic_family(ui.ctx(), &serif).unwrap_or(serif.clone());
                ui.add_sized([49.0, rh], egui::Label::new(RichText::new(nm).family(fam).size(16.0).color(theme::text_col())));
            }
            // Value.
            let unit_txt: String = if r.unit == "$unit" { self.params.s("unit").to_string() } else { r.unit.to_string() };
            let shown;
            if let Some(eq) = r.equals {
                ui.add_sized([if skin { 88.0 } else { 96.0 }, if skin { 20.0 } else { rh }], egui::Label::new(theme::muted(format!("= {eq}")).size(if skin { 12.0 } else { 14.0 })));
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
                        let te = if skin {
                            TextEdit::singleline(&mut v)
                                .hint_text(RichText::new(hint.clone()).size(17.0))
                                .font(egui::FontId::new(17.0, theme::sans_family()))
                                .vertical_align(Align::Center)
                                .min_size(Vec2::new(if r.name_key.is_some() { 152.0 } else { 160.0 }, rh))
                                .desired_width(if r.name_key.is_some() { 128.0 } else { 136.0 })
                                .margin(Margin::symmetric(12, 0))
                        } else {
                            TextEdit::singleline(&mut v)
                                .hint_text(RichText::new(hint.clone()).size(14.0))
                                .font(egui::FontId::new(16.0, egui::FontFamily::Proportional))
                                .vertical_align(Align::Center)
                                .min_size(Vec2::new(110.0, rh))
                                .desired_width(110.0)
                        };
                        if ui.add(te).changed() {
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
                    ui.label(theme::muted(unit_txt.as_str()).size(if skin { 11.0 } else { 13.0 }));}
                else if !skin && r.name_key.is_none() && !r.label_key.is_empty() {
                    ui.add_space(4.0);
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
            let btn = if skin {
                // the label button hugs the panel's right edge, as the reference's does
                ui.add_space((ui.available_width() - 83.0).max(0.0));
                ui.add(
                    Button::new(RichText::new(if txt.trim().is_empty() { "—".to_string() } else { txt }).family(if txt_is_math(&mode) { serif.clone() } else { theme::sans_family() }).size(if txt_is_math(&mode) { 17.0 } else { 13.0 }).color(theme::text_col()))
                        .min_size(Vec2::new(83.0, rh))
                        .corner_radius(CornerRadius::same(10)),
                )
            } else {
                ui.add(Button::new(RichText::new(if txt.trim().is_empty() { "—".to_string() } else { txt }).size(13.0)).min_size(Vec2::new(78.0, rh)))
            };
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

    /// Whether the open figure has a `show_answers` switch (so "Student + key" applies).
    fn has_answers(&self) -> bool {
        self.sel.is_some_and(|i| figures::hides_answers(&(reg()[i].params)()))
    }

    /// Saves the student copy where the user chooses and the answer key beside it
    /// (`name-student.png` / `name-key.png`).
    fn export_pair(&self, svg: bool) -> Act {
        let Some(si) = self.sel else { return Act::None };
        let def = &reg()[si];
        let Some((student, key)) = figures::answer_pair(def, &self.params) else {
            return Act::Toast("This figure has no answers to hide".into(), true);
        };
        let (ext, label) = if svg { ("svg", "SVG") } else { ("png", "PNG") };
        let bytes = |s: &str| -> Result<Vec<u8>, String> {
            if svg { Ok(s.as_bytes().to_vec()) } else { flatten_white(&figures::to_png(s, 2.0)?) }
        };
        let (sb, kb) = match (bytes(&student), bytes(&key)) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(e), _) | (_, Err(e)) => return Act::Toast(format!("Couldn't render the figure: {e}"), true),
        };
        let Some(spath) = crate::platform::save_path(&format!("{}-student.{ext}", def.id), label, &[ext]) else {
            return Act::None;
        };
        let kpath = figures::key_path(&spath);
        match crate::platform::write(&spath, &sb).and_then(|()| crate::platform::write(&kpath, &kb)) {
            Ok(()) => Act::Toast(format!("Exported {} and {}", spath.display(), kpath.display()), false),
            Err(e) => Act::Toast(format!("Export failed: {e}"), true),
        }
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
/// Whether a label button shows a value (set in the serif) rather than the word "None".
fn txt_is_math(mode: &str) -> bool {
    mode != "None"
}

fn open_sections(fig: &str) -> usize {
    match fig {
        "pedigree" => 0,
        // the reference's five sections all start collapsed, each with its summary line
        "inclined-plane" => 0,
        // physicsfigures.com opens every card collapsed with its summary line, except the spring
        // scale's first two (Spring scale, Reading)
        "free-body-diagram" | "vector-diagram" | "pulley" | "projectile-motion" | "motion-graphs" | "waves" | "circuit-diagram" => 0,
        // the reference opens its polygons on the Measures card with Heights, Labels and Position collapsed
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon" => 0,
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
        .body(|ui| faq_list(ui, fig, None, qa));
}

/// The FAQ's questions, each opening its answer; they wrap inside the panel (a collapsing
/// header's title would widen it). `shell` is the shell card's (question, answer) text size.
fn faq_list(ui: &mut Ui, fig: &str, shell: Option<(f32, f32)>, qa: bool) {
    let size = shell.unwrap_or((12.0, 12.0));
    let faq = figures::faq::faq_for(fig);
    for (i, e) in faq.iter().enumerate() {
        let qid = ui.make_persistent_id(("fig_faq_q", fig, i));
        let open = ui.data(|d| d.get_temp::<bool>(qid)).unwrap_or(qa && i == 0);
        let q = format!("{} {}", if open { "\u{25be}" } else { "\u{25b8}" }, e.q);
        let q = if shell.is_some() { RichText::new(q).size(size.0).family(theme::bold_family()).color(theme::ink()) } else { RichText::new(q).size(size.0).strong() };
        let r = ui.add(egui::Label::new(q).wrap().sense(Sense::click()));
        if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            ui.data_mut(|d| d.insert_temp(qid, !open));
        }
        if open {
            ui.indent(qid, |ui| {
                ui.add(egui::Label::new(RichText::new(e.a).size(size.1).color(theme::muted_col())).wrap());
            });
        }
        ui.add_space(4.0);
    }
    ui.add_space(4.0);
    if qa {
        ui.scroll_to_cursor(Some(Align::BOTTOM));
    }
}

/// The shell's FAQ: a card of its own at the foot of the controls column, one section row
/// ("FAQ", how many questions) that opens the questions.
fn faq_card(ui: &mut Ui, fig: &str) {
    let n = figures::faq::faq_for(fig).len();
    if n == 0 {
        return;
    }
    let id = ui.make_persistent_id(("fig_faq", fig));
    // QA hook: VERSHION_FIG_FAQ=1 opens the FAQ and its first answer and scrolls them into view
    let qa = crate::env_var("VERSHION_FIG_FAQ").is_ok_and(|v| v == "1");
    let mut st = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, qa);
    chem_shell::panel_card(ui, 0, |ui| {
        let gap_y = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("chem_first_row"), true));
        let resp = chem_shell::section_header(ui, "FAQ", &format!("{n} questions about this figure"), st.is_open());
        ui.spacing_mut().item_spacing.y = gap_y;
        if resp.clicked() {
            st.toggle(ui);
        }
        st.show_body_unindented(ui, |ui| {
            Frame::new().inner_margin(Margin { left: 18, right: 18, top: 2, bottom: 10 }).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                faq_list(ui, fig, Some((14.0, 13.5)), qa);
            });
        });
        st.store(ui.ctx());
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
    crate::kit::chip_toggle(ui, "Handles", on).on_hover_text("Show drag handles on the figure. Drag the dots to edit; drag or double-click a label to move or rename it").clicked()
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

/// The chemistry page's inline note under a control (muted, or amber when it warns).
fn chem_note(ui: &mut Ui, fig: &str, key: &str, p: &Params) {
    // the physics page keeps these as a hover tip beside the label (see `help_tip`)
    if ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false) && figures::sections::phys_tip(fig, key, p).is_some() {
        return;
    }
    if let Some(n) = figures::chemistry::notes::key_note(fig, key, p) {
        let sz = if matches!(fig, "punnett-square" | "gel-electrophoresis" | "pedigree") { 13.1 } else { 13.0 };
        let t = if n.warn { RichText::new(n.text).size(13.0).color(egui::Color32::from_rgb(0xb4, 0x53, 0x09)) } else { theme::muted(n.text).size(sz).line_height(if sz == 13.1 { Some(19.0) } else { None }) };
        ui.add(egui::Label::new(t).wrap());
        ui.add_space(4.0);
    }
}

/// The physics page's circled "?" at the right end of a control's first line, its text on hover.
fn help_tip(ui: &mut Ui, top: f32, text: String) {
    let c = egui::Pos2::new(ui.max_rect().right() - 13.0, top + 10.0);
    let col = theme::muted_col();
    let id = ui.id().with(("help_tip", text.len(), top as i32));
    let rect = egui::Rect::from_center_size(c, Vec2::splat(18.0));
    let r = ui.interact(rect, id, egui::Sense::hover());
    let col = if r.hovered() { theme::accent_col() } else { col };
    ui.painter().circle_stroke(c, 7.6, egui::Stroke::new(1.5, col));
    ui.painter().text(c + Vec2::new(0.0, 0.3), egui::Align2::CENTER_CENTER, "?", egui::FontId::new(11.0, theme::bold_family()), col);
    r.on_hover_text(text);
}

/// A control's label, with the param's hint on hover (the reference keeps help out of the label).
fn param_label(ui: &mut Ui, label: &str, hint: &str) {
    // the chemistry labels are 14.4 px, which sits them a pixel low in the row
    let lift = !label.is_empty() && chem_shell::chem_page(ui.ctx());
    if lift {
        ui.add_space(-1.0);
    }
    param_label_inner(ui, label, hint);
    if lift {
        ui.add_space(1.0);
    }
    // the biology population page's labels sit a little closer to their boxes than ours did
    let tight = ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new("pop_label_tight"))).unwrap_or(0.0);
    if tight != 0.0 && !label.is_empty() {
        ui.add_space(-tight);
    }
}

fn param_label_inner(ui: &mut Ui, label: &str, hint: &str) {
    if label.is_empty() {
        return;
    }
    // "Name|note" and "Name (unit)": the reference sets the note / unit in lighter grey after the name
    let mut piped = false;
    let (name, note) = match label.split_once('|') {
        Some((n, t)) => {
            piped = true;
            (n.trim_end(), Some(t.to_string()))
        }
        None => match label.rfind(" (").filter(|_| label.ends_with(')')) {
            Some(i) => (&label[..i], Some(label[i + 1..].to_string())),
            None => (label, None),
        },
    };
    let r = if let Some(note) = note {
        ui.scope(|ui| {
            if theme::chem_skin() {
                ui.spacing_mut().interact_size.y = if ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("chem_skin_page"))).unwrap_or(false) { 20.4 } else { 20.0 };
                ui.spacing_mut().item_spacing.y = 0.0;
            }
            ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let r = ui.label(if theme::chem_skin() { chem_shell::param_name(ui.ctx(), name) } else { theme::strong(name).size(12.0) });
            if theme::chem_skin() {
                // the chemistry page sets the unit and the note in the label's own 13.5 px, lighter grey
                let _ = piped;
                let fade = 1.0;
                ui.label(RichText::new(note).size(14.08).family(theme::sans_family()).color(theme::muted_col().gamma_multiply(fade)));
            } else {
                ui.label(theme::muted(note).size(11.0).color(theme::muted_col().gamma_multiply(0.7)));
            }
            r
        })
        .inner
        })
        .inner
    } else if theme::chem_skin() {
        // same row height as a label with a unit, so paired boxes line up
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = if ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("chem_skin_page"))).unwrap_or(false) { 20.0 } else { 20.0 };
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.horizontal_wrapped(|ui| ui.label(chem_shell::param_name(ui.ctx(), label))).inner
        })
        .inner
    } else {
        ui.label(theme::strong(label).size(12.0))
    };
    if !hint.is_empty() {
        r.on_hover_text(hint);
    }
    if piped && theme::chem_skin() {
        // the reference's two-line note label runs on a 20.4 px line
        ui.add_space(0.9);
    }
}

/// The Punnett page's Phenotypes card: one labelled field per allele class ("T_" tall, "tt" short),
/// kept in the comma / semicolon `phenotypes` string the generator reads.
fn phenotype_rows(ui: &mut Ui, pd: &ParamDef, p: &mut Params, genes: &[char]) {
    let mut names: Vec<Vec<String>> = p.s(pd.key).split(';').map(|g| g.split(',').map(|t| t.trim().to_string()).collect()).collect();
    names.resize(genes.len(), Vec::new());
    for g in names.iter_mut() {
        g.resize(2, String::new());
    }
    let ctx = ui.ctx().clone();
    let serif = theme::italic_family(&ctx, &theme::serif_family()).unwrap_or_else(theme::serif_family);
    let mut changed = false;
    for (gi, &c) in genes.iter().enumerate() {
        let lower = c.to_lowercase().next().unwrap_or(c);
        for (k, lab) in [format!("{c}_"), format!("{lower}{lower}")].into_iter().enumerate() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (r, _) = ui.allocate_exact_size(Vec2::new(72.0, 40.0), egui::Sense::hover());
                ui.painter().text(r.right_center(), egui::Align2::RIGHT_CENTER, lab, egui::FontId::new(17.6, serif.clone()), ui.visuals().text_color());
                let te = TextEdit::singleline(&mut names[gi][k])
                    .desired_width(f32::INFINITY)
                    .margin(Margin::symmetric(10, 10))
                    .font(egui::FontId::new(15.2, theme::sans_family()));
                if ui.add(te).changed() {
                    changed = true;
                }
            });
            if !(gi + 1 == genes.len() && k == 1) {
                ui.add_space(1.2);
            }
        }
    }
    if changed {
        let s = names.iter().map(|g| g.join(", ")).collect::<Vec<_>>().join("; ");
        p.0.insert(pd.key.to_string(), Value::S(s));
    }
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
    if k == "title" {
        // the chemistry pages' chart-title switch (Text / Blank line / None)
        cands.push("title_style".to_string());
    }
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

/// Short choices as one wrapped row of pills, like the reference: the chosen one is solid
/// accent with white text, the others white with a hairline border.
fn segmented(ui: &mut Ui, opts: &[&'static str], cur: &mut String) {
    let skin = theme::chem_skin();
    if skin && (opts.len() <= 3 || opts == ["0.8%", "1%", "1.5%", "2%"]) && opts.iter().all(|o| o.chars().count() <= 22) {
        chem_shell::segmented_track(ui, opts, cur);
        ui.add_space(0.0);
        return;
    }
    pills(ui, opts, cur);
}

/// The reference's pill buttons: the chosen one filled blue, the rest outlined.
fn pills(ui: &mut Ui, opts: &[&'static str], cur: &mut String) {
    let skin = theme::chem_skin();
    let chem = skin && (ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("chem_skin_page"))).unwrap_or(false) || ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false));
    let bio = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false);
    if chem && !bio {
        ui.add_space(2.0);
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = if chem { Vec2::new(8.0, 8.0) } else if skin { Vec2::new(8.0, 7.0) } else { Vec2::new(6.0, 6.0) };
        if chem {
            ui.spacing_mut().button_padding = Vec2::new(16.0, 3.5);
        }
        let small = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("pills_small"))).unwrap_or(false);
        if small {
            ui.spacing_mut().button_padding = Vec2::new(12.0, 3.0);
        }
        for o in opts {
            let sel = cur == o;
            let small = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("pills_small"))).unwrap_or(false);
            let size = if small { 14.4 } else if chem && !bio { 15.3 } else if skin { 15.5 } else { 12.5 };
            let mut rt = RichText::new(*o).size(size).color(if sel { egui::Color32::WHITE } else if chem { theme::ink() } else { theme::text_col() });
            if chem {
                rt = rt.family(theme::semibold_family());
            }
            let b = Button::new(rt)
                .fill(if sel { theme::accent_col() } else { theme::card() })
                .stroke(Stroke::new(1.0, if sel { theme::accent_col() } else { theme::border() }))
                .corner_radius(CornerRadius::same(255))
                .min_size(Vec2::new(0.0, if small { 31.0 } else if chem { 38.0 } else if skin { 38.0 } else { 28.0 }));
            if ui.add(b).clicked() {
                *cur = o.to_string();
            }
        }
    });
    ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::new("pills_small")));
    if chem && !bio {
        ui.add_space(3.0);
    }
}

impl Workspace {
    /// The Lewis page's Changes section: the note, the central atom, each atom's lone electrons
    /// and each bond's order, then what differs from the correct structure and what is wrong.
    fn lewis_changes(&mut self, ui: &mut Ui, help: &str) {
        let noto = || theme::sans_family();
        ui.add(egui::Label::new(theme::muted(help).size(13.0)).wrap());
        ui.add_space(6.0);
        if self.params.s("question") != "Full structure" && !figures::chemistry::lewis_has_changes(&self.params) {
            return;
        }
        let mut q = self.params.clone();
        if let Some((opts, right)) = figures::chemistry::lewis_central_choices(&self.params) {
            let cur = if q.s("central").trim().is_empty() { right.clone() } else { q.s("central").trim().to_string() };
            let name = |o: &str| if *o == *right { format!("{o} (correct)") } else { o.to_string() };
            let mut pick = cur.clone();
            ui.horizontal(|ui| {
                ui.label(chem_shell::label_name(ui.ctx(), "Central atom", 14.08));
                ui.scope(|ui| {
                    ui.spacing_mut().button_padding = Vec2::new(12.0, 10.0);
                    ComboBox::from_id_salt("lewis_central")
                        .icon(chem_shell::select_chevron)
                        .selected_text(RichText::new(name(&cur)).size(14.4).family(noto()))
                        .show_ui(ui, |ui| {
                            for o in &opts {
                                ui.selectable_value(&mut pick, o.clone(), name(o));
                            }
                        });
                });
            });
            if pick != cur {
                q.set_s("central", if pick == right { "" } else { &pick });
                q.set_s("tweak", "");
            }
            ui.add_space(8.0);
        }
        if let Some((atoms, bonds)) = figures::chemistry::lewis_edit_rows(&q) {
            let caps = |ui: &mut Ui, t: &str| {
                ui.label(RichText::new(t).size(12.0).family(theme::bold_family()).color(theme::muted_col()));
            };
            caps(ui, "ATOMS");
            ui.add_space(4.0);
            for (i, name, nb) in atoms.iter().map(|(i, n, b)| (*i, n, b)) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let (r, _) = ui.allocate_exact_size(Vec2::new(78.0, 28.0), Sense::hover());
                    ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, name.as_str(), egui::FontId::new(14.4, theme::bold_family()), theme::ink());
                    let step = |ui: &mut Ui, glyph: &str, on: bool| ui.add_enabled(on, Button::new(RichText::new(glyph).size(18.0).color(theme::ink())).frame(false).min_size(Vec2::splat(28.0))).clicked();
                    let mut n = *nb;
                    if step(ui, "\u{2212}", *nb > 0) {
                        n -= 1;
                    }
                    ui.add_sized([112.0, 28.0], egui::Label::new(RichText::new(format!("{nb} lone electron{}", if *nb == 1 { "" } else { "s" })).size(14.4).family(noto())));
                    if step(ui, "+", *nb < 8) {
                        n += 1;
                    }
                    if n != *nb {
                        figures::chemistry::lewis_set_edit(&mut q, true, i, n);
                    }
                });
                ui.add_space(13.0);
            }
            ui.add_space(8.0);
            caps(ui, "BONDS");
            ui.add_space(4.0);
            for (i, (name, order)) in bonds.iter().enumerate() {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let (r, _) = ui.allocate_exact_size(Vec2::new(80.0, 39.0), Sense::hover());
                    ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, name.as_str(), egui::FontId::new(14.4, theme::bold_family()), theme::ink());
                    let mut cur = ["None", "Single", "Double", "Triple"][(*order).clamp(0, 3) as usize].to_string();
                    let before = cur.clone();
                    ui.allocate_ui(Vec2::new(ui.available_width(), 0.0), |ui| chem_shell::segmented_track(ui, &["None", "Single", "Double", "Triple"], &mut cur));
                    if cur != before {
                        let o = ["None", "Single", "Double", "Triple"].iter().position(|x| *x == cur).unwrap_or(1) as i32;
                        figures::chemistry::lewis_set_edit(&mut q, false, i, o);
                    }
                });
                ui.add_space(8.0);
            }
        }
        let n = figures::chemistry::lewis_changes_count(&q);
        if n > 0 {
            ui.add(egui::Label::new(theme::muted(format!("{n} change{} from the correct structure", if n == 1 { "" } else { "s" })).size(13.0)).wrap());
            ui.add_space(4.0);
            if chem_shell::outline_button(ui, "Reset to correct", true).clicked() {
                q.set_s("tweak", "");
                q.set_s("central", "");
            }
            for m in figures::chemistry::lewis_mistakes(&q) {
                ui.add(egui::Label::new(theme::muted(m).size(13.0)).wrap());
            }
        }
        self.params = q;
    }
}

/// Extra width the reference's cross spaces carry beyond the font's own space, in px.
const PUNNETT_SPACE_EXTRA: f32 = 3.3;

fn param_control(ui: &mut Ui, fig: &str, pd: &ParamDef, p: &mut Params) {
    param_control_inner(ui, fig, pd, p);
    // the reference's grey note under a control (ranges, what the answer key prints, typing tips)
    if !matches!(pd.kind, Kind::Bool) && theme::chem_skin() {
        if let Some(note) = figures::sections::note_after(fig, pd.key, p) {
            for line in note.split('\n') {
                if fig == "free-body-diagram" {
                    // the physics page sets its notes in 13.6 px grey on a 19.72 px line
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(theme::muted(line).size(13.6).line_height(Some(19.72))).wrap());
                } else {
                    ui.add(egui::Label::new(theme::muted(line).size(13.0)).wrap());
                }
            }
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("pop_label_tight"), 0.0_f32));
}

/// The reference's Labels card sets the unit and the rounding with two selects (Unit: None, cm, m,
/// mm, in, ft, yd, units, Other...; Round to: Whole numbers, Tenths, Hundredths), where the
/// parameters themselves are a free text and a count of decimal places.
fn math_unit_dec(ui: &mut Ui, fig: &str, pd: &ParamDef, p: &mut Params, label: &str) {
    const UNITS: [&str; 7] = ["cm", "m", "mm", "in", "ft", "yd", "units"];
    param_label(ui, label, "");
    if pd.key == "dec" {
        let names = ["Whole numbers", "Tenths", "Hundredths", "Thousandths"];
        let mut cur = p.i("dec").clamp(0, 3);
        ComboBox::from_id_salt(("math_dec", fig)).selected_text(names[cur as usize]).width(200.0).show_ui(ui, |ui| {
            for (i, n) in names.iter().enumerate().take(3) {
                ui.selectable_value(&mut cur, i as i64, *n);
            }
        });
        if cur != p.i("dec") {
            p.set_i("dec", cur);
        }
        return;
    }
    let other_id = egui::Id::new(("math_unit_other", fig));
    let unit = p.s("unit").trim().to_string();
    let other = ui.ctx().data(|d| d.get_temp::<bool>(other_id)).unwrap_or(false) || (!unit.is_empty() && !UNITS.contains(&unit.as_str()));
    let shown = if other { "Other\u{2026}".to_string() } else if unit.is_empty() { "None".to_string() } else { unit.clone() };
    let mut pick: Option<String> = None;
    ComboBox::from_id_salt(("math_unit", fig)).selected_text(shown.clone()).width(200.0).show_ui(ui, |ui| {
        if ui.selectable_label(shown == "None", "None").clicked() {
            pick = Some(String::new());
        }
        for u in UNITS {
            if ui.selectable_label(shown == u, u).clicked() {
                pick = Some(u.to_string());
            }
        }
        if ui.selectable_label(other, "Other\u{2026}").clicked() {
            pick = Some("\u{0}".to_string());
        }
    });
    match pick.as_deref() {
        Some("\u{0}") => {
            ui.ctx().data_mut(|d| d.insert_temp(other_id, true));
        }
        Some(u) => {
            ui.ctx().data_mut(|d| d.insert_temp(other_id, false));
            p.set_s("unit", u);
        }
        None => {}
    }
    if other {
        let mut cur = p.s("unit").to_string();
        if ui.add(TextEdit::singleline(&mut cur).desired_width(200.0).hint_text("e.g. km")).changed() {
            p.set_s("unit", &cur);
        }
    }
}

fn param_control_inner(ui: &mut Ui, fig: &str, pd: &ParamDef, p: &mut Params) {
    let tight: f32 = if fig == "population-growth" && theme::chem_skin() {
        match pd.key {
            "growth" | "organism" => 2.0,
            "n0" | "r" | "k" | "tspan" => 3.0,
            "unit" => 1.0,
            _ => 0.0,
        }
    } else if fig == "predator-prey" && theme::chem_skin() {
        match pd.key {
            "pair" => 2.0,
            "prey" | "pred" => 4.0,
            "prey0" | "pred0" | "prey_avg" | "pred_avg" | "cycle" | "tshow" | "unit" => 3.0,
            _ => 0.0,
        }
    } else {
        0.0
    };
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("pop_label_tight"), tight));
    let dyn_label = figures::dynamic_label(fig, pd.key, p);
    let mut label = dyn_label.as_deref().or_else(|| figures::label_for(fig, pd.key, p)).unwrap_or(pd.label);
    if fig == "length-reading" && ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false) {
        // the physics page has no "Units" caption over its Metric / Imperial toggle, and calls the ruler's size "Length"
        label = match pd.key { "units" => "", "ruler" => "Length", _ => label };
    }
    if theme::chem_skin() && fig == "punnett-square" && pd.key == "phenotypes" && p.s("dominance") == "Complete" {
        let genes = figures::biology::punnett::gene_letters(p);
        if !genes.is_empty() {
            phenotype_rows(ui, pd, p, &genes);
            return;
        }
    }
    if matches!(fig, "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon") && matches!(pd.key, "unit" | "dec") {
        math_unit_dec(ui, fig, pd, p, label);
        return;
    }
    match &pd.kind {
        Kind::Float { min, max, .. } if theme::chem_skin() && figures::random_key(fig) == Some(pd.key) => {
            // the chemistry page's reading: a text box with its unit and an outlined Random button
            param_label(ui, label, pd.hint);
            ui.add_space(if fig == "micropipette-reading" { -2.0 } else { 1.0 });
            let unit = if fig == "spring-scale" { "N".to_string() } else if fig == "volume-reading" && !p.s("unit").is_empty() { p.s("unit").to_string() } else if fig == "micropipette-reading" { "µL".to_string() } else { "mL".to_string() };
            let id = ui.make_persistent_id(("chem_reading", fig, pd.key));
            let ctx = ui.ctx().clone();
            let focused = ctx.memory(|m| m.has_focus(id));
            let shown = fmt_measure(p.f(pd.key));
            let mut text = if focused { ctx.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.clone()) } else { shown };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let r = ui.add(TextEdit::singleline(&mut text).id(id).desired_width(144.0).margin(Margin::symmetric(10, 10)).font(egui::FontId::new(15.0, theme::sans_family())));
                if r.changed() {
                    ctx.data_mut(|d| d.insert_temp(id, text.clone()));
                    if let Ok(v) = text.trim().parse::<f64>() {
                        p.0.insert(pd.key.to_string(), Value::F(v.clamp(*min, *max)));
                    }
                }
                // the reference's unit is a 600-weight 16 px span 8 px after the box, a pixel higher
                ui.add_space(-4.0);
                let g = ui.painter().layout_no_wrap(unit, egui::FontId::new(16.0, theme::semibold_family()), theme::muted_col());
                let (ur, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                ui.painter().galley(ur.min - Vec2::new(0.0, 1.0), g, theme::muted_col());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let btn = chem_shell::outline_chip_icon(ui, lucide::DICES, "Random");
                    if btn.on_hover_text("Set a random reading").clicked() {
                        figures::random_reading(fig, p, random_seed());
                        ctx.data_mut(|d| d.remove_temp::<String>(id));
                        ctx.memory_mut(|m| m.surrender_focus(id));
                    }
                });
            });
        }
        Kind::Float { min, max, .. } if theme::chem_skin() => {
            // the chemistry page types every number into a boxed field (no sliders)
            param_label(ui, label, pd.hint);
            let id = ui.make_persistent_id(("chem_float", fig, pd.key));
            let ctx = ui.ctx().clone();
            let focused = ctx.memory(|m| m.has_focus(id));
            let shown = fmt_measure(p.f(pd.key));
            let mut text = if focused { ctx.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.clone()) } else { shown };
            // the reference's unit sits after the box for the lengths and forces it measures
            let unit = match (fig, pd.key) {
                ("spring-scale", "offset") => Some("N".to_string()),
                ("length-reading", "length" | "offset") => Some(if p.s("units").starts_with("Imp") { "in" } else { "cm" }.to_string()),
                _ => None,
            };
            let r = ui.horizontal(|ui| {
                let w = if unit.is_some() { (ui.available_width() - 40.0 - ui.spacing().item_spacing.x).max(60.0) } else { f32::INFINITY };
                let r = ui.add(TextEdit::singleline(&mut text).id(id).desired_width(w).margin(Margin::symmetric(10, 10)).font(egui::FontId::new(15.0, theme::sans_family())));
                if let Some(u) = &unit {
                    ui.label(RichText::new(u.as_str()).size(16.0).family(theme::sans_family()).color(theme::muted_col()));
                }
                r
            }).inner;
            if r.changed() {
                ctx.data_mut(|d| d.insert_temp(id, text.clone()));
                if let Ok(v) = text.trim().parse::<f64>() {
                    p.0.insert(pd.key.to_string(), Value::F(v.clamp(*min, *max)));
                }
            }
        }
        Kind::Float { min, max, step } => {
            param_label(ui, label, pd.hint);
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
            let changed = skin_slider(ui, s);
            if changed {
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
                    param_label(ui, label, pd.hint);
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
                param_label(ui, label, pd.hint);
                if skin_slider(ui, egui::Slider::new(&mut v, *min..=*max).clamping(egui::SliderClamping::Always)) {
                    p.0.insert(pd.key.to_string(), Value::I(v));
                }
            }
        }
        Kind::Bool if theme::chem_skin() && fig == "lewis-structures" && pd.key == "answer" => {
            // a key icon before the label; greyed out until there is something to key
            let avail = figures::chemistry::lewis_answer_available(p);
            let mut v = p.b(pd.key) && avail;
            let note = figures::chemistry::notes::lewis_answer_note(p);
            let r = ui.scope(|ui| {
                ui.spacing_mut().icon_spacing = 9.0;
                ui.spacing_mut().icon_width = 18.0;
                ui.spacing_mut().interact_size.y = 18.0;
                ui.spacing_mut().item_spacing.y = 2.0;
                let rad = CornerRadius::same(4);
                let w = &mut ui.visuals_mut().widgets;
                w.inactive.corner_radius = rad;
                w.hovered.corner_radius = rad;
                w.active.corner_radius = rad;
                w.inactive.bg_stroke = Stroke::new(1.25, egui::Color32::from_rgb(107, 114, 128));
                let txt = RichText::new(format!("{}  Answer key", egui_phosphor::regular::KEY)).size(14.4).family(theme::bold_family());
                let r = ui.add_enabled(avail, egui::Checkbox::new(&mut v, if avail { theme::strong(format!("{}  Answer key", egui_phosphor::regular::KEY)).size(14.4) } else { txt.color(theme::muted_col()) }));
                ui.horizontal_top(|ui| {
                    ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
                    ui.add(egui::Label::new(theme::muted(note).size(13.0)).wrap());
                });
                r
            }).inner;
            if r.changed() {
                p.0.insert(pd.key.to_string(), Value::B(v));
            }
        }
        Kind::Bool => {
            let mut v = p.b(pd.key);
            let live_hint = figures::sections::note_after(fig, pd.key, p).unwrap_or_else(|| pd.hint.to_string());
            let skin_hint = theme::chem_skin() && !live_hint.is_empty();
            let mut r = if skin_hint {
                // the chemistry page names the option in bold with its explanation beneath
                ui.scope(|ui| {
                    ui.spacing_mut().icon_spacing = 9.0;
                    ui.spacing_mut().icon_width = 18.0;
                    ui.spacing_mut().interact_size.y = 18.0;
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let rad = CornerRadius::same(4);
                    let w = &mut ui.visuals_mut().widgets;
                    w.inactive.corner_radius = rad;
                    w.hovered.corner_radius = rad;
                    w.active.corner_radius = rad;
                    w.inactive.bg_stroke = Stroke::new(1.25, egui::Color32::from_rgb(107, 114, 128));
                    let r = ui.checkbox(&mut v, theme::strong(label).size(14.4));
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
                        ui.label(theme::muted(live_hint.as_str()).size(13.0));
                    });
                    r
                })
                .inner
            } else if ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false) && theme::chem_skin() {
                let r = chem_shell::phys_checkbox(ui, &mut v, label);
                ui.add_space(-3.0);
                r
            } else if ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false) && theme::chem_skin() {
                chem_shell::bio_checkbox(ui, &mut v, label)
            } else {
                ui.checkbox(&mut v, label)
            };
            if !pd.hint.is_empty() && !skin_hint {
                r = r.on_hover_text(pd.hint);
            }
            if r.changed() {
                p.0.insert(pd.key.to_string(), Value::B(v));
            }
        }
        Kind::Choice(opts) => {
            // the reference's segmented switch (the instrument picker) stands without a label
            if !(theme::chem_skin() && opts.len() <= 3 && (label == "Instrument" || (fig == "predator-prey" && pd.key == "source"))) {
                param_label(ui, label, pd.hint);
            }
            let mut cur = p.s(pd.key).to_string();
            // the titration's "no common titration picked" reads "Choose one…", as the reference's
            // placeholder does
            let shown = |o: &str| {
                // the Lewis page's "no list pick" is the select's placeholder
                if fig == "lewis-structures" && pd.key == "preset" && o.starts_with("None") {
                    return "Molecules with more than one central atom\u{2026}".to_string();
                }
                if fig == "titration-curve" && pd.key == "common" && o == "Custom" {
                    return "Choose one…".to_string();
                }
                // the chemistry page writes elements as "8 · O · Oxygen"
                if fig == "mass-spectrum" && pd.key == "element" && theme::chem_skin() {
                    if let Some(e) = figures::chemistry::data::ISOTOPES.iter().find(|e| e.0 == o) {
                        return format!("{} · {} · {}", figures::chemistry::data::z_of(e.1), e.1, e.0);
                    }
                }
                if theme::chem_skin() && matches!(pd.key, "element" | "compare") {
                    if let Some((n, rest)) = o.split_once(' ') {
                        if n.parse::<u32>().is_ok() {
                            return format!("{n} · {}", rest.replacen(" - ", " · ", 1));
                        }
                    }
                }
                o.to_string()
            };
            if theme::chem_skin() && fig == "lewis-structures" && pd.key == "title_style" {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("seg_icons"), true));
            }
            if theme::chem_skin() && fig == "lewis-structures" && pd.key == "question" {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("pills_small"), true));
                pills(ui, opts, &mut cur);
            } else if !(matches!(fig, "population-growth" | "predator-prey") && pd.key == "unit") && ((fig == "predator-prey" && pd.key == "source") || (opts.len() <= 7 && opts.iter().map(|o| o.chars().count()).sum::<usize>() <= 48 && opts.iter().all(|o| o.chars().count() <= if opts.len() <= 3 { 20 } else { 14 }))) {
                segmented(ui, opts, &mut cur);
            } else {
                let chem = theme::chem_skin();
                ui.scope(|ui| {
                    if chem {
                        // the reference's 40 px select with 15.2 px text
                        ui.spacing_mut().button_padding = Vec2::new(15.0, 9.5);
                        ui.spacing_mut().interact_size.y = 39.5;
                    }
                    let sel = if chem { RichText::new(shown(&cur)).size(15.2).family(theme::sans_family()) } else { RichText::new(shown(&cur)) };
                    let cb = ComboBox::from_id_salt(("fig_param", pd.key));
                    let cb = if chem { cb.icon(chem_shell::select_chevron) } else { cb };
                    cb
                        .selected_text(sel)
                        .width(if chem { ui.available_width() } else { 200.0 })
                        .show_ui(ui, |ui| {
                            for o in opts {
                                // the reference's list stops at perchloric acid
                                if fig == "lewis-structures" && pd.key == "preset" && o.starts_with("CH\u{2083}COCl") && cur != *o {
                                    continue;
                                }
                                ui.selectable_value(&mut cur, o.to_string(), shown(o));
                            }
                        });
                });
            }
            if cur != p.s(pd.key) {
                p.0.insert(pd.key.to_string(), Value::S(cur));
            }
            if theme::chem_skin() && fig == "lewis-structures" && pd.key == "preset" {
                ui.add_space(2.0);
            }
        }
        Kind::Text if ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false) && pd.key.contains("lab") && pd.key != figures::labels::KEY => {
            // the physics page's label boxes set the text as a formula (F with a subscript g)
            param_label(ui, label, pd.hint);
            ui.add_space(-7.0);
            if let Some(t) = cards_panel::phys_math_field(ui, ui.make_persistent_id(("phys_lab", fig, pd.key)), p.s(pd.key)) {
                p.0.insert(pd.key.to_string(), Value::S(t));
            }
        }
        Kind::Text => {
            if theme::chem_skin() && fig == "lewis-structures" && pd.key == "formula" {
                // the reference sets a "?" after the label, its tooltip the typing guide
                ui.add_space(0.0);
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(12.0, 0.0);
                    ui.spacing_mut().interact_size.y = 20.0;
                    ui.horizontal(|ui| {
                        ui.label(chem_shell::param_name(ui.ctx(), label));
                        chem_shell::help_dot(ui, pd.hint);
                    });
                });
                ui.add_space(4.0);
            } else {
                param_label(ui, label, pd.hint);
            }
            let mut cur = p.s(pd.key).to_string();
            // a label-less box (the reference's titles and point labels) shows its default as a placeholder
            // the reference pads the multiplication sign with wider spaces than the font's own
                let ink = ui.visuals().text_color();
                let fam = theme::italic_family(ui.ctx(), &theme::serif_family()).unwrap_or_else(theme::serif_family);
                let mut layouter = move |ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                    let mut job = egui::text::LayoutJob::default();
                    job.wrap.max_width = wrap;
                    for ch in text.as_str().chars() {
                        let lead = if ch == ' ' { PUNNETT_SPACE_EXTRA } else { 0.0 };
                        job.append(&ch.to_string(), lead, egui::TextFormat::simple(egui::FontId::new(18.4, fam.clone()), ink));
                    }
                    ui.fonts_mut(|f| f.layout_job(job))
                };
            let mut te = TextEdit::singleline(&mut cur).desired_width(220.0);
            if theme::chem_skin() {
                // the reference's full-width 46 px field
                te = te.desired_width(f32::INFINITY).margin(Margin::symmetric(if fig == "population-growth" && pd.key == "organism" { 11 } else { 14 }, if fig == "lewis-structures" || fig == "predator-prey" || (fig == "population-growth" && pd.key == "organism") { 10 } else { 12 })).font(egui::FontId::new(15.0, theme::sans_family()));
            }
            if theme::chem_skin() && fig == "punnett-square" && pd.key == "parents" {
                // the reference sets the genotypes in italic serif
                let fam = theme::italic_family(ui.ctx(), &theme::serif_family()).unwrap_or_else(theme::serif_family);
                te = te.margin(Margin { left: 13, right: 13, top: 7, bottom: 15 }).font(egui::FontId::new(18.4, fam.clone()));
                te = te.layouter(&mut layouter);
            }
            if label.is_empty() {
                te = te.hint_text(match &pd.default { Value::S(d) => d.as_str(), _ => "" });
            }
            if fig == "population-growth" && pd.key == "organism" {
                // the reference shows Chrome's placeholder grey (#757575) at the box's own 15 px
                te = te.hint_text(RichText::new("deer, yeast cells, bacteria").size(15.0).family(theme::sans_family()).color(egui::Color32::from_rgb(0x75, 0x75, 0x75)));
                ui.add_space(-1.0);
            }
            let changed_te = ui
                .scope(|ui| {
                    if fig == "population-growth" && pd.key == "organism" {
                        // egui paints a hint in the weak text colour; the reference uses #757575
                        ui.visuals_mut().weak_text_color = Some(egui::Color32::from_rgb(0x75, 0x75, 0x75));
                    }
                    ui.add(te).changed()
                })
                .inner;
            if fig == "population-growth" && pd.key == "organism" {
                ui.add_space(1.0);
            }
            if changed_te {
                p.0.insert(pd.key.to_string(), Value::S(cur));
            }
            if theme::chem_skin() && fig == "lewis-structures" && pd.key == "formula" {
                ui.add_space(1.0);
            }
        }
        Kind::Long => {
            param_label(ui, label, pd.hint);
            let mut cur = p.s(pd.key).to_string();
            if ui
                .add(TextEdit::multiline(&mut cur).desired_width(240.0).desired_rows(4).font(egui::TextStyle::Monospace))
                .changed()
            {
                p.0.insert(pd.key.to_string(), Value::S(cur));
            }
        }
        Kind::Data { columns } => data_entry::data_control(ui, fig, pd, *columns, label, p),
        Kind::List => {
            param_label(ui, label, pd.hint);
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
            param_label(ui, label, pd.hint);
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
    /// Whether a figure is open on the Figures screen.
    pub fn figure_open(&self) -> bool {
        self.figs.ws.sel.is_some()
    }

    /// Whether a chemistry figure is open on the Figures screen (it gets the chemistry page skin).
    pub fn chem_figure_open(&self) -> bool {
        self.figs.ws.chem_open() || self.figs.ws.chem_list()
    }

    /// The standalone Figures screen (sidebar already drawn onto `ui`).
    pub fn draw_figures(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut ws = std::mem::take(&mut self.figs.ws);
        let mut act = Act::None;
        let phone = ws.sel.is_some() && ui.max_rect().width() < 500.0;
        let go_home = false;
        if ws.chem_open() || ws.chem_list() {
            // the chemistry page skin: white top header over a cool hex-pattern page (no rail)
            let math_page = ws.skin_subject() == figures::Subject::Math;
            let title = ws.sel.map(|i| if math_page || ws.skin_subject() == figures::Subject::Physics { format!("{} Generator", match reg()[i].id { "waves" => "Wave", "motion-graphs" if !math_page => "Motion Graph", _ => reg()[i].title }) } else { reg()[i].title.to_string() }).unwrap_or_default();
            self.figs.ws = ws;
            self.chem_header(ui, &title);
            ws = std::mem::take(&mut self.figs.ws);
            let pad = if phone { 12 } else { chem_shell::PAGE_PAD as i8 + 1 };
            let chem_stack = ws.skin_subject() == figures::Subject::Chemistry && ui.ctx().content_rect().width() < 880.0;
            // the chemistry and biology pages run their cards 20 px from the header, the right edge and the bottom
            let tight_page = matches!(ws.skin_subject(), figures::Subject::Chemistry | figures::Subject::Biology);
            egui::CentralPanel::default()
                .frame(Frame::new().fill(theme::cream()).inner_margin(if ws.sel.is_some() && !phone && !chem_stack { Margin { left: pad - 1, right: if tight_page { pad - 3 } else { pad - 2 }, top: if tight_page { pad - 3 } else { pad - 1 }, bottom: if tight_page { pad - 3 } else { pad - 2 } } } else if ws.sel.is_some() && tight_page { Margin { left: 16, right: 16, top: 16, bottom: 11 } } else { Margin { left: pad, right: pad - 1, top: pad - 1, bottom: if ws.sel.is_none() { 0 } else { pad - 1 } } }))
                .show(ui, |ui| {
                    // the hexagon lattice is the chemistry page's; the biology page has none
                    if ws.skin_subject() == figures::Subject::Math {
                        chem_shell::paint_grid(ui.painter(), ui.max_rect().expand2(Vec2::new(pad as f32, pad as f32)));
                    } else if ws.skin_subject() == figures::Subject::Physics {
                        chem_shell::paint_physics_doodles(ui.painter(), egui::Rect::from_min_max(egui::pos2(0.0, chem_shell::HEADER_H), ui.ctx().content_rect().max));
                    } else if matches!(ws.skin_subject(), figures::Subject::Biology) {
                        chem_shell::paint_bio_doodles(ui.ctx(), ui.painter(), egui::Rect::from_min_max(egui::pos2(0.0, chem_shell::HEADER_H), ui.ctx().content_rect().max));
                    } else {
                        chem_shell::paint_hex(ui.painter(), egui::Rect::from_min_max(egui::pos2(0.0, chem_shell::HEADER_H), ui.ctx().content_rect().max));
                    }
                    act = ws.show(ui);
                });
        } else {
        crate::screens::desk_panel(ui, if phone { 8 } else if ws.sel.is_some() { 16 } else { 24 }, |ui| {
            if phone {
                // no breadcrumb row on a phone: the canvas wants the height, and "← Figures" below
                // leads back to the list (which has the rail with Home)
            } else if ws.sel.is_some() {
                // an open figure: just the breadcrumb, so the canvas gets the height (the
                // reference's canvas runs from under its slim top bar to the window bottom)
                ui.label(RichText::new("Home  /  Figures").size(11.5).color(theme::muted_col()));
                ui.add_space(4.0);
            } else {
                crate::kit::page_header(
                    ui,
                    &["Home", "Figures"],
                    "Figures",
                    Some("Physics, biology, chemistry, math and computing diagrams you can tune, export, or attach to a question."),
                );
                ui.add_space(12.0);
            }
            act = ws.show(ui);
        });
        }
        self.figs.ws = ws;
        if go_home {
            self.screen = crate::Screen::Home;
        }
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

    /// The Computing demo uses Kind::Data, style and show_answers: it opens in the generic (unskinned)
    /// workspace, its data box and Generate popover draw, and the pair export differs as expected.
    #[test]
    fn computing_demo_uses_the_shared_pieces() {
        let (i, def) = find_def("demo-data").unwrap();
        assert_eq!(def.subject, Subject::Computing);
        let mut ws = Workspace::new(false);
        ws.select(i);
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        assert!(!ws.chem_open(), "Computing has no reference skin");
        for _ in 0..3 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let _ = ws.show(ui);
            });
            out.textures_delta.clear();
        }
        assert!(ws.shown.is_some());
        // a spreadsheet column stored raw reads back; the tidy form is what the box writes on blur
        ws.params.set_s("data", "2\n4\n6");
        assert_eq!(figures::data::numbers(ws.params.s("data")), vec![2.0, 4.0, 6.0]);
        let (student, key) = figures::answer_pair(def, &ws.params).unwrap();
        assert!(!student.contains("ANSWER") && key.contains("ANSWER"));
        ws.params.set_s("style", "Print");
        let (print, _) = figures::answer_pair(def, &ws.params).unwrap();
        assert!(print.contains("vs-hatch0") && !print.contains("#4f7cd4"));
        assert!(figures::to_png(&print, 1.0).is_ok());
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
        // a math figure's controls wear the card skin: its collapsible rows are clickable, labelled by title
        if let Some(r) = tree.nodes.iter().filter(|(_, nd)| nd.role() != egui::accesskit::Role::Label && nd.label().is_some_and(|l| l == title)).filter_map(|(_, nd)| nd.bounds()).find(|b| b.x0 < 700.0) {
            let p = egui::pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32);
            tall_frame(ctx, ws, vec![egui::Event::PointerMoved(p)]);
            let mut ev = btn(p, true);
            ev.extend(btn(p, false));
            tall_frame(ctx, ws, ev);
            for _ in 0..12 {
                tall_frame(ctx, ws, vec![]);
            }
            return;
        }
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
        // the triangle opens with Heights, Labels and Position collapsed (as the reference does)
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
        // the shell's section row is a button labelled with its title
        assert_eq!(count_named(&ctx, &mut ws, "Symbols"), 1, "the header is there");
        assert_eq!(count_text(&ctx, &mut ws, "Carriers"), 0, "Symbols starts collapsed");
        assert_eq!(count_text(&ctx, &mut ws, "Key"), 0, "Labels starts collapsed");
        open_section(&ctx, &mut ws, "Symbols");
        assert!(count_text(&ctx, &mut ws, "Carriers") >= 1, "opens on a click");
        assert_eq!((open_sections("pedigree"), open_sections("pulley")), (0, 0));
    }

    /// A section header's summary wraps short of its Reset button instead of running under it
    /// (motion-graphs segment summaries are long). The plain headers with Reset are the insert-figure
    /// dialog's; the editor's shell rows cut their summary to one line short of the chevron.
    #[test]
    fn section_summaries_never_run_under_reset() {
        for id in ["motion-graphs", "pulley", "free-body-diagram"] {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            ctx.enable_accesskit();
            let mut ws = Workspace::new(true);
            ws.select(find_def(id).unwrap().0);
            for _ in 0..3 {
                tall_frame(&ctx, &mut ws, vec![]);
            }
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
        assert_eq!(count_named(&ctx, &mut ws, egui_phosphor::regular::ARROW_LEFT), 5, "one card per lane");
        let before = ws.params.clone();
        click_nth(&ctx, &mut ws, &format!("{}Ladder", egui_phosphor::regular::PLUS), 0);
        let lanes = gel::lane_cards(&ws.params);
        assert_eq!(lanes.len(), 6);
        assert_eq!(lanes[5], gel::LaneCard::ladder());
        assert_eq!(gel::current_setup(&ws.params), None, "editing a setup's lanes copies them into the text");
        click_nth(&ctx, &mut ws, egui_phosphor::regular::ARROW_RIGHT, 0); // Ladder moves right of Crime scene
        assert_eq!(gel::lane_cards(&ws.params)[1].label, "Ladder");
        click_nth(&ctx, &mut ws, egui_phosphor::regular::TRASH, 0); // remove the first lane (now Crime scene)
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
        open_section(&ctx, &mut ws, "Forces");
        assert_eq!(count_named(&ctx, &mut ws, "Up"), 2);
        click_nth(&ctx, &mut ws, "Left", 0);
        assert_eq!(ws.params.f("f1_ang"), 180.0);
        click_nth(&ctx, &mut ws, &format!("{}Friction", egui_phosphor::regular::PLUS), 0);
        assert_eq!((ws.params.i("n"), ws.params.s("f3_lab")), (3, "F_f"));
        click_nth(&ctx, &mut ws, "Remove", 0); // delete force 1: the others move up
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
        assert_eq!(figures::cards::summary("free-body-diagram", "motion", &ws.params).as_deref(), Some("velocity 0°, acceleration 0°"));
    }

    /// Vector diagram: cards with remove, reorder and "Add a vector".
    #[test]
    fn vector_cards_in_the_panel() {
        let (ctx, mut ws) = open_tall("vector-diagram");
        open_section(&ctx, &mut ws, "Vectors");
        click_nth(&ctx, &mut ws, &format!("{}Add a vector", egui_phosphor::regular::PLUS), 0);
        assert_eq!(ws.params.i("n"), 3);
        click_nth(&ctx, &mut ws, egui_phosphor::regular::ARROW_DOWN, 0); // A after B
        assert_eq!((ws.params.s("v1_lab"), ws.params.s("v2_lab")), ("B", "A"));
        click_nth(&ctx, &mut ws, "Remove", 1); // remove the second (A)
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
        click_nth(&ctx, &mut ws, egui_phosphor::regular::ARROW_UP, 1); // Helium above the edited hydrogen (strip 1's own ↑)
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
        open_section(&ctx, &mut ws, "Particles");
        assert_eq!(count_named(&ctx, &mut ws, "XL"), 2);
        click_nth(&ctx, &mut ws, "+ Add a kind", 0);
        assert!(atoms::kind_used(&ws.params, 2));
        assert_eq!(count_named(&ctx, &mut ws, "XL"), 3);
        click_nth(&ctx, &mut ws, "Pair", 2);
        assert_eq!(ws.params.s("c_shape"), "Pair");
        assert_eq!(atoms::kind_noun(&ws.params, 2), "Molecule");
        assert_eq!(count_named(&ctx, &mut ws, "XL"), 4, "a molecule has Center and Outer sizes");
        click_nth(&ctx, &mut ws, egui_phosphor::regular::X, 0);
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

/// The physics page's "Save preset" card: a dashed chip that asks for a name, the saved
/// presets as chips, and the note that they stay in this browser.
fn preset_card(ui: &mut Ui, def: &FigureDef, params: &Params) -> Option<Params> {
    let open_id = egui::Id::new(("preset_card_open", def.id));
    let mut open = ui.ctx().data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let name_id = open_id.with("name");
    let mut name = ui.ctx().data(|d| d.get_temp::<String>(name_id)).unwrap_or_default();
    let mut file = load_presets();
    let mut changed = false;
    let mut apply: Option<Params> = None;
    let m = egui::Margin { left: 18, right: 18, top: 16, bottom: 15 };
    chem_shell::panel_card_m(ui, m, |ui| {
        ui.spacing_mut().item_spacing.y = 11.0;
        let list = file.entry(def.id.to_string()).or_default();
        let mut remove: Option<usize> = None;
        for (i, pr) in list.iter().enumerate() {
            ui.horizontal(|ui| {
                if chem_shell::outline_chip(ui, &pr.name).clicked() {
                    let spec = FigureSpec { id: def.id.to_string(), params: pr.params.clone() };
                    apply = Some(params_from_spec(def, &spec));
                }
                if ui.add(Button::new(theme::muted("Delete").size(12.0)).frame(false)).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            list.remove(i);
            changed = true;
        }
        if open {
            ui.horizontal(|ui| {
                let r = ui.add(TextEdit::singleline(&mut name).hint_text("Name this preset").desired_width(160.0));
                let go = ui.button("Save").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                if go && !name.trim().is_empty() {
                    let spec = spec_for(def.id, params);
                    let nm = name.trim().to_string();
                    list.retain(|p| p.name != nm);
                    list.push(PresetEntry { name: nm, params: spec.params });
                    name.clear();
                    changed = true;
                    open = false;
                }
            });
        } else if chem_shell::dashed_chip(ui, &format!("{}  Save preset", egui_phosphor::regular::BOOKMARK_SIMPLE)).clicked() {
            open = true;
        }
        ui.label(theme::muted("Saved presets stay in this browser.").size(12.8));
    });
    ui.ctx().data_mut(|d| {
        d.insert_temp(open_id, open);
        d.insert_temp(name_id, name);
    });
    if changed {
        save_presets(&file);
    }
    apply
}


/// Adds a slider; on the page skin its rail is a grey bar filled blue up to a round white-ringed
/// handle (the card-white rail of the plain theme would vanish on the white card).
fn skin_slider(ui: &mut Ui, s: egui::Slider<'_>) -> bool {
    skin_slider_w(ui, s, None)
}

/// `skin_slider` with a fixed rail width (the physics cards' rail stops short of a value box).
fn skin_slider_w(ui: &mut Ui, s: egui::Slider<'_>, width: Option<f32>) -> bool {
    if !theme::chem_skin() {
        return ui.add(s).changed();
    }
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        v.widgets.inactive.bg_fill = theme::border();
        v.widgets.hovered.bg_fill = theme::border();
        v.widgets.active.bg_fill = theme::border();
        v.selection.bg_fill = theme::accent_col();
        v.slider_trailing_fill = true;
        // the rail runs the width of the panel up to the value box
        ui.spacing_mut().slider_width = width.unwrap_or_else(|| (ui.available_width() - 76.0).max(100.0));
        let v = ui.visuals_mut();
        v.handle_shape = egui::style::HandleShape::Circle;
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, theme::accent_col());
        ui.add(s).changed()
    })
    .inner
}

/// One text row of a list card: a plain box, or the math pages' 44 px rounded input.
fn row_edit<'a>(text: &'a mut String, hint: &'a str, w: f32, skin: bool) -> TextEdit<'a> {
    if skin {
        // the reference's equation field: 17 px text and 17 px placeholder
        return TextEdit::singleline(text)
            .hint_text(RichText::new(hint).size(17.0))
            .desired_width(w)
            .margin(Margin::symmetric(12, 13.5 as i8))
            .font(egui::FontId::new(17.0, theme::sans_family()));
    }
    TextEdit::singleline(text).hint_text(hint).desired_width(w)
}
/// Device-pixel y phase of the physics sheet in the reference (decides which hairlines land on two pixels).
const PHYS_PH: f32 = 0.5;
