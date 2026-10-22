//! The chemistry figures page shell: the reference's white top header (flask logo, wordmark,
//! breadcrumb, About / Math Figures / Physics Figures pills), its cool hex-pattern page, the
//! card section headers of the controls panel and the icon toolbar of the preview card.
//! Everything here is drawn only while `theme::chem_skin()` is on (a chemistry figure open).
use egui::{Align, Color32, CornerRadius, Frame, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use egui_phosphor::regular as ph;

use super::lucide;
use super::{Act, Workspace};
use crate::{figures, theme};

/// Width of the reference's controls column at 1440 px (x 23 to 447).
pub const PANEL_W: f32 = 424.0;
/// Gap between the controls column and the preview card.
pub const COL_GAP: f32 = 26.0;
/// Page margin around the two columns.
pub const PAGE_PAD: f32 = 22.0;
pub const HEADER_H: f32 = 56.0;

/// The subjects that wear the reference page skin.
/// True while the chemistry page (not the physics, biology or math ones) is the one being drawn.
pub fn chem_page(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new("chem_skin_page"))).unwrap_or(false)
}

/// A control label's name in the chemistry reference's 600 weight (other pages keep bold).
pub fn label_name(ctx: &egui::Context, text: &str, size: f32) -> RichText {
    let t = RichText::new(text).size(size).color(theme::ink());
    t.family(if chem_page(ctx) || ctx.data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false) { theme::semibold_family() } else { theme::bold_family() })
}

/// A parameter label ("Size", "Liquid", "Reading"): the chemistry reference sets them 14.4 px, 700.
pub fn param_name(ctx: &egui::Context, text: &str) -> RichText {
    if chem_page(ctx) { RichText::new(text).size(14.4).family(theme::bold_family()).color(theme::ink()) } else { label_name(ctx, text, 14.08) }
}

/// The reference sites' page skins (top header instead of the rail, subject home pages) are off:
/// a figure picked on the unified Figures tab opens in the app's own editor. Flip to bring them back.
pub const SITE_SKINS: bool = false;

pub fn skinned(s: figures::Subject) -> bool {
    matches!(s, figures::Subject::Chemistry | figures::Subject::Biology | figures::Subject::Math | figures::Subject::Physics)
}

/// Width of the controls column at 1440 px: chemistry's runs x 23 to 447, biology's 22 to 432.
pub fn panel_w(s: figures::Subject) -> f32 {
    match s {
        figures::Subject::Biology => 410.0,
        // the math reference's column runs x 23 to 399 at 1440
        figures::Subject::Math | figures::Subject::Physics => 378.0,
        _ => PANEL_W,
    }
}

/// Reference blue text (#1d4ed8) and pill border (#bfdbfe).
pub fn blue_text() -> Color32 {
    theme::accent_hover()
}
pub fn pill_border() -> Color32 {
    theme::rule()
}

/// The math page's faint square graph-paper grid (24 px cells) under the whole page.
pub fn paint_grid(painter: &egui::Painter, rect: Rect) {
    let line = if theme::is_dark() { Color32::from_rgb(0x2a, 0x33, 0x47) } else { Color32::from_rgb(0xe9, 0xed, 0xf5) };
    let st = Stroke::new(1.0, line);
    let mut x = rect.left() + 12.0;
    while x < rect.right() {
        painter.vline(x.round() + 0.5, rect.y_range(), st);
        x += 24.0;
    }
    let mut y = rect.top() + 12.0;
    while y < rect.bottom() {
        painter.hline(rect.x_range(), y.round() + 0.5, st);
        y += 24.0;
    }
}

/// The reference's wallpaper: a fixed 48.497 x 84 px hex tile anchored at the window origin
/// (strokes 1.25 px at 9 % blue, vertex dots r 2.25 at 16 %), masked so it is opaque left of
/// `W/2 - 608`, gone by `W/2 - 480`, and mirrored on the right.
pub fn paint_hex(painter: &egui::Painter, rect: Rect) {
    let painter = painter.with_clip_rect(rect);
    let base = if theme::is_dark() { Color32::from_rgb(0x60, 0x8c, 0xff) } else { Color32::from_rgb(0x25, 0x63, 0xeb) };
    let mid = rect.right() / 2.0;
    let mask = |x: f32| -> f32 {
        let d = (x - mid).abs();
        ((d - 480.0) / 128.0).clamp(0.0, 1.0)
    };
    let (tw, th) = (48.497_f32, 84.0_f32);
    let seg = |painter: &egui::Painter, ox: f32, oy: f32, p: (f32, f32), q: (f32, f32)| {
        const N: usize = 4;
        for i in 0..N {
            let (t0, t1) = (i as f32 / N as f32, (i + 1) as f32 / N as f32);
            let a = Pos2::new(ox + p.0 + (q.0 - p.0) * t0, oy + p.1 + (q.1 - p.1) * t0);
            let b = Pos2::new(ox + p.0 + (q.0 - p.0) * t1, oy + p.1 + (q.1 - p.1) * t1);
            let m = mask((a.x + b.x) / 2.0);
            if m > 0.01 {
                painter.line_segment([a, b], Stroke::new(1.25, base.gamma_multiply(0.09 * m)));
            }
        }
    };
    let paths: [&[(f32, f32)]; 3] = [
        &[(24.249, 0.0), (24.249, 14.0), (48.497, 28.0)],
        &[(48.497, 56.0), (24.249, 70.0), (24.249, 84.0)],
        &[(24.249, 70.0), (0.0, 56.0), (0.0, 28.0), (24.249, 14.0)],
    ];
    let dots = [(24.249, 14.0), (24.249, 70.0), (0.0, 56.0), (0.0, 28.0)];
    let c0 = (rect.left() / tw).floor() as i32 - 1;
    let c1 = (rect.right() / tw).ceil() as i32 + 1;
    let r0 = (rect.top() / th).floor() as i32 - 1;
    let r1 = (rect.bottom() / th).ceil() as i32 + 1;
    for ry in r0..=r1 {
        for cx in c0..=c1 {
            let (ox, oy) = (cx as f32 * tw, ry as f32 * th);
            if mask(ox).max(mask(ox + tw)) < 0.01 {
                continue;
            }
            for path in paths {
                for w in path.windows(2) {
                    seg(&painter, ox, oy, w[0], w[1]);
                }
            }
            for d in dots {
                let m = mask(ox + d.0);
                if m > 0.01 {
                    painter.circle_filled(Pos2::new(ox + d.0, oy + d.1), 2.25, base.gamma_multiply(0.16 * m));
                }
            }
        }
    }
}

/// The biology reference's wallpaper: a fixed 240 px tile of faint blue line art (helix, cell,
/// leaf, mitochondrion, grid) anchored at the window origin, masked like the chemistry hex
/// lattice (opaque left of `W/2 - 608`, gone by `W/2 - 480`, mirrored on the right).
pub fn paint_bio_doodles(ctx: &egui::Context, painter: &egui::Painter, rect: Rect) {
    let ppp = ctx.pixels_per_point();
    let id = egui::Id::new(("bio_bg_tile", (ppp * 100.0) as i32));
    let tex: egui::TextureHandle = match ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        Some(t) => t,
        None => {
            let Ok(png) = figures::to_png(include_str!("../../assets/bio_bg.svg"), ppp) else { return };
            let Ok(img) = image::load_from_memory(&png) else { return };
            let img = img.to_rgba8();
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
            let t = ctx.load_texture("bio_bg_tile", ci, egui::TextureOptions::LINEAR);
            ctx.data_mut(|d| d.insert_temp(id, t.clone()));
            t
        }
    };
    let painter = painter.with_clip_rect(rect);
    let mid = rect.right() / 2.0;
    let mask = |x: f32| (((x - mid).abs() - 480.0) / 128.0).clamp(0.0, 1.0);
    let tile = 240.0_f32;
    let strip = 6.0_f32;
    let mut mesh = egui::Mesh::with_texture(tex.id());
    let r0 = (rect.top() / tile).floor() as i32;
    let r1 = (rect.bottom() / tile).ceil() as i32;
    let c1 = (rect.right() / tile).ceil() as i32;
    for ry in r0..r1 {
        for cx in 0..c1 {
            for k in 0..(tile / strip) as i32 {
                let x0 = cx as f32 * tile + k as f32 * strip;
                let x1 = x0 + strip;
                let a = mask((x0 + x1) / 2.0);
                if a < 0.01 {
                    continue;
                }
                let col = Color32::from_white_alpha((a * 255.0) as u8);
                let y0 = ry as f32 * tile;
                let quad = Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y0 + tile));
                let uv = Rect::from_min_max(Pos2::new(k as f32 * strip / tile, 0.0), Pos2::new((k as f32 + 1.0) * strip / tile, 1.0));
                mesh.add_rect_with_uv(quad, uv, col);
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// A Phosphor glyph picked from a section's title.
pub fn section_icon(title: &str, math: bool) -> &'static str {
    let t = title.to_lowercase();
    let has = |ks: &[&str]| ks.iter().any(|k| t.contains(k));
    if math {
        // the math reference's own glyphs: arrow for Heights, tag for Labels, a turn for Position
        match t.as_str() {
            "titles" => return ph::TEXT_H,
            "x-axis" => return ph::ARROW_RIGHT,
            "y-axis" => return ph::ARROW_UP,
            "grid" => return ph::GRID_NINE,
            _ => {}
        }
        if has(&["height"]) {
            return ph::ARROW_DOWN;
        } else if has(&["label"]) {
            return ph::TAG;
        } else if has(&["position", "orientation", "rotat"]) {
            return ph::ARROW_CLOCKWISE;
        }
    }
    if has(&["phenotype", "trait", "allele"]) {
        ph::TAG
    } else if has(&["square", "cross", "dominance", "genotype", "parent"]) {
        ph::DNA
    } else if has(&["pedigree", "family", "individual", "generation"]) {
        ph::TREE_STRUCTURE
    } else if has(&["gel", "lane", "band", "ladder"]) {
        ph::ROWS
    } else if has(&["population", "predator", "prey", "growth"]) {
        ph::CHART_LINE_UP
    } else if has(&["pipette", "volume", "well"]) {
        ph::DROP
    } else if has(&["instrument", "apparatus", "setup", "family", "type", "figure"]) {
        ph::FLASK
    } else if title.eq_ignore_ascii_case("scale") {
        ph::TABLE
    } else if has(&["scale", "axes", "axis", "grid", "marks", "ticks"]) {
        ph::LIST_BULLETS
    } else if has(&["reading", "measure", "value", "data", "sample"]) {
        ph::RULER
    } else if has(&["label", "text", "caption", "title"]) {
        ph::TEXT_AA
    } else if has(&["colour", "color", "style", "look", "appearance", "line"]) {
        ph::PALETTE
    } else if has(&["magnif", "zoom", "view", "show"]) {
        ph::EYE
    } else if has(&["curve", "graph", "plot", "spectr"]) {
        ph::CHART_LINE
    } else if has(&["atom", "electron", "element", "nucle"]) {
        ph::ATOM
    } else if has(&["liquid", "solution", "drop"]) {
        ph::DROP
    } else if has(&["temperature"]) {
        ph::THERMOMETER
    } else {
        ph::SLIDERS_HORIZONTAL
    }
}

/// The Lucide drawing for the sections whose reference glyph is known.
fn chem_lucide_icon(title: &str) -> Option<&'static [lucide::Op]> {
    let t = title.to_lowercase();
    if t.starts_with("marked") {
        Some(lucide::MAP_PIN)
    } else if t == "curve" {
        Some(lucide::SPLINE)
    } else {
        match t.as_str() {
            "instrument" => Some(lucide::FLASK_CONICAL),
            "scale" => Some(lucide::ROWS_4),
            "reading" => Some(lucide::RULER),
            _ => None,
        }
    }
}

/// The chemistry reference's own row glyphs: a map pin for the marked points, a spline for curves.
fn chem_section_icon(title: &str) -> Option<&'static str> {
    let t = title.to_lowercase();
    if t.starts_with("marked") {
        Some(ph::MAP_PIN)
    } else if t == "curve" {
        Some(ph::BEZIER_CURVE)
    } else {
        Some(match t.as_str() {
            "peaks" => ph::CHART_BAR,
            "structure" => ph::ATOM,
            "look" => ph::EYE,
            "question" => ph::SELECTION,
            "changes" => ph::PENCIL_SIMPLE_LINE,
            "title and answer key" => ph::TEXT_T,
            "for the student" => ph::PENCIL_SIMPLE,
            "titles" => ph::TEXT_H,
            "x-axis" => ph::ARROW_RIGHT,
            "y-axis" => ph::ARROW_UP,
            _ => return None,
        })
    }
}

/// The physics reference's own Lucide row glyphs, by figure and section title.
fn phys_lucide_icon(fig: &str, title: &str) -> Option<&'static [lucide::Op]> {
    use lucide::*;
    let t = title.to_lowercase();
    Some(match (fig, t.as_str()) {
        (_, "figure") => PH_SLIDERS_HORIZONTAL,
        (_, "forces") | (_, "forces and motion") | (_, "velocity and gravity") | (_, "vectors") => PH_MOVE_UP_RIGHT,
        ("motion-graphs", "marks") => PH_MAP_PIN,
        (_, "marks") | (_, "reading") | (_, "number line") => PH_RULER,
        ("spring-scale" | "ray-diagram", "title and answer key") => PH_TYPE,
        (_, "title and answer key") | (_, "titles") | (_, "chart title") => PH_HEADING,
        (_, "grid and axes") | (_, "axes and grid") | (_, "grid") => PH_GRID_3X3,
        ("free-body-diagram", "body") => PH_BOX,
        ("free-body-diagram", "motion") => PH_GAUGE,
        (_, "motion") => PH_ROUTE,
        ("inclined-plane", "object") | ("pulley", "objects") => PH_BOX,
        ("projectile-motion", "object") => PH_CIRCLE,
        ("ray-diagram", "object") => PH_ARROW_UP_FROM_LINE,
        (_, "ramp") => PH_TRIANGLE,
        (_, "resultant") => PH_SIGMA,
        (_, "setup") => PH_COG,
        (_, "launch") => PH_MOUNTAIN,
        (_, "path") => PH_SPLINE,
        (_, "graphs") | (_, "graph") => PH_CHART_LINE,
        (_, "spring scale") => PH_WEIGHT,
        (_, "on the hook") => PH_PACKAGE,
        (_, "magnifier") => PH_ZOOM_IN,
        (_, "circuit") => PH_CIRCUIT_BOARD,
        (_, "cell or battery") => PH_BATTERY_FULL,
        (_, "switch and meters") => PH_GAUGE,
        (_, "symbols") => PH_PALETTE,
        (_, "wave") => PH_WAVES_HORIZONTAL,
        (_, "x-axis") => PH_MOVE_RIGHT,
        (_, "y-axis") => PH_MOVE_UP,
        (_, "dots") => PH_CIRCLE_DOT,
        (_, "arrows") => PH_MOVE_RIGHT,
        (_, "lens or mirror") => PH_GLASSES,
        (_, "rays and image") => PH_FLASHLIGHT,
        (_, "labels") => PH_TAG,
        _ => return None,
    })
}

/// The physics reference's own row glyphs (wave, tag, chart, H, arrows, grid).
fn phys_section_icon(title: &str) -> Option<&'static str> {
    let t = title.to_lowercase();
    Some(match t.as_str() {
        "wave" => ph::WAVE_SINE,
        "marks" => ph::TAG,
        "graph" => ph::CHART_LINE,
        "titles" => ph::TEXT_H,
        "x-axis" => ph::ARROW_RIGHT,
        "y-axis" => ph::ARROW_UP,
        "grid" => ph::GRID_FOUR,
        _ => return None,
    })
}

/// A faint hand-drawn physics doodle (wave, spring, resistor, lens, pulley, incline) scattered
/// through the margins of the physics page, as the reference's page background has.
pub fn paint_physics_doodles(painter: &egui::Painter, rect: Rect) {
    let painter = painter.with_clip_rect(rect);
    let base = if theme::is_dark() { Color32::from_rgb(0x60, 0x8c, 0xff) } else { Color32::from_rgb(0x3b, 0x82, 0xf6) };
    let mid = rect.right() / 2.0;
    let mask = |x: f32| (((x - mid).abs() - 480.0) / 128.0).clamp(0.0, 1.0);
    let tile = 240.0_f32;
    // a polyline drawn in short pieces so the left/right fade follows it (`k` scales the alpha)
    let line = |pts: &[Pos2], k: f32, dash: bool| {
        for (i, w) in pts.windows(2).enumerate() {
            if dash && i % 2 == 1 {
                continue;
            }
            let m = mask((w[0].x + w[1].x) / 2.0) * k;
            if m > 0.01 {
                painter.line_segment([w[0], w[1]], Stroke::new(1.3, base.gamma_multiply(0.17 * m)));
            }
        }
    };
    let ell = |c: Pos2, rx: f32, ry: f32, rot: f32, k: f32, dash: bool| {
        let n = 36;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32 * std::f32::consts::TAU;
                let (x, y) = (rx * t.cos(), ry * t.sin());
                Pos2::new(c.x + x * rot.cos() - y * rot.sin(), c.y + x * rot.sin() + y * rot.cos())
            })
            .collect();
        line(&pts, k, dash);
    };
    let dot = |c: Pos2, r: f32, a: f32| {
        let m = mask(c.x);
        if m > 0.01 {
            painter.circle_filled(c, r, base.gamma_multiply(a * m * 0.55));
        }
    };
    // the tile is anchored at the window origin: its atom sits at (46, 289) on the 1440 page
    let r0 = ((rect.top() - 300.0) / tile).floor() as i32;
    let r1 = ((rect.bottom() + 100.0) / tile).ceil() as i32;
    let cols: Vec<i32> = (0..=(rect.right() / tile).ceil() as i32).filter(|c| mask(*c as f32 * tile).max(mask(*c as f32 * tile + tile)) > 0.01).collect();
    for cx in cols {
        for ry in r0..=r1 {
            let o = Pos2::new(cx as f32 * tile, 49.0 + ry as f32 * tile);
            let p = |x: f32, y: f32| Pos2::new(o.x + x, o.y + y);
            // atom: three orbits and a nucleus
            let c = p(46.0, 240.0);
            for rot in [0.0, 1.0472, 2.0944] {
                ell(c, 24.0, 9.0, rot, 1.0, false);
            }
            dot(c, 2.2, 0.55);
            // sine wave, fading to the right
            let wave: Vec<Pos2> = (0..=40).map(|i| p(103.0 + i as f32 * 2.6, 237.0 + (i as f32 / 40.0 * 1.5 * std::f32::consts::TAU).sin() * -9.0)).collect();
            let n = wave.len();
            for i in 0..n - 1 {
                line(&wave[i..=i + 1], 1.0 - 0.7 * i as f32 / n as f32, false);
            }
            // pendulum: support bar, rod, bob and its dashed swing
            line(&[p(93.0, 43.0), p(130.0, 43.0)], 1.0, false);
            line(&[p(105.0, 43.0), p(128.0, 89.0)], 1.0, false);
            ell(p(128.0, 89.0), 5.0, 5.0, 0.0, 1.0, false);
            let arc: Vec<Pos2> = (0..=10).map(|i| p(95.0 + i as f32 * 3.1, 88.0 + (i as f32 / 10.0 * 3.14).sin() * 3.0)).collect();
            line(&arc, 1.0, true);
            // spring and block, fading to the right
            line(&[p(183.0, 35.0), p(220.0, 35.0)], 0.55, false);
            let mut sp = vec![p(205.0, 35.0)];
            for i in 0..6 {
                sp.push(p(if i % 2 == 0 { 210.0 } else { 200.0 }, 41.0 + i as f32 * 5.0));
            }
            sp.push(p(205.0, 78.0));
            line(&sp, 0.55, false);
            line(&[p(192.0, 79.0), p(212.0, 79.0), p(212.0, 95.0), p(192.0, 95.0), p(192.0, 79.0)], 0.5, false);
            // converging rays and a lens
            for dy in [-12.0, 0.0, 12.0] {
                line(&[p(13.0, 101.0 + dy), p(42.0, 101.0 + dy), p(68.0, 101.0)], 1.0, false);
            }
            ell(p(42.0, 101.0), 4.0, 17.0, 0.0, 1.0, false);
            dot(p(68.0, 101.0), 1.5, 0.6);
            // an electron's dashed orbit
            ell(p(62.0, 165.0), 27.0, 10.0, 0.0, 1.0, true);
            dot(p(62.0, 165.0), 4.0, 0.7);
            dot(p(88.0, 165.0), 2.0, 0.8);
            // a block with four force arrows
            let c = p(170.0, 147.0);
            line(&[c + Vec2::new(-7.0, -7.0), c + Vec2::new(7.0, -7.0), c + Vec2::new(7.0, 7.0), c + Vec2::new(-7.0, 7.0), c + Vec2::new(-7.0, -7.0)], 0.7, false);
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let (u, v) = (Vec2::new(dx, dy), Vec2::new(-dy, dx));
                let (a0, a1) = (c + u * 7.0, c + u * 25.0);
                line(&[a0, a1], 0.7, false);
                line(&[a1 - u * 5.0 + v * 4.0, a1, a1 - u * 5.0 - v * 4.0], 0.7, false);
            }
        }
    }
}

/// The reference's tooltip: a dark navy pill with white semibold text, 7 px under the button.
pub fn dark_tip(ui: &Ui, resp: egui::Response, tip: &str) -> egui::Response {
    if resp.hovered() {
        egui::Area::new(resp.id.with("dark_tip"))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .fixed_pos(Pos2::new(resp.rect.center().x, resp.rect.bottom() + 6.0))
            .pivot(egui::Align2::CENTER_TOP)
            .show(ui.ctx(), |ui| {
                Frame::new().fill(Color32::from_rgb(0x1f, 0x29, 0x37)).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(9, 4)).show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(tip).family(theme::semibold_family()).size(12.48).color(Color32::WHITE)).wrap_mode(egui::TextWrapMode::Extend));
                });
            });
    }
    resp
}

/// One square icon button (the preview toolbar): hover wash, disabled grey.
pub fn icon_btn(ui: &mut Ui, glyph: &str, tip: &str, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(38.0), if enabled { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        if enabled && resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(10), theme::hover_bg());
        }
        let col = if enabled { theme::ink() } else { theme::rule().gamma_multiply(0.8) };
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::new(20.0, theme::icon_font(20.0).family), col);
    }
    if enabled { dark_tip(ui, resp, tip) } else { resp }
}

fn tool_btn(ui: &mut Ui, chem: bool, ops: &[lucide::Op], glyph: &str, tip: &str, enabled: bool) -> egui::Response {
    if chem { lucide_btn(ui, ops, tip, enabled) } else { icon_btn(ui, glyph, tip, enabled) }
}

/// The chemistry toolbar's square Lucide icon button (38.4 px, 19 px glyph).
pub fn lucide_btn(ui: &mut Ui, ops: &[lucide::Op], tip: &str, enabled: bool) -> egui::Response {
    // a phone's buttons are 34.4 px
    let side = if ui.ctx().content_rect().width() < 500.0 { 34.4 } else { 38.4 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), if enabled { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        if enabled && resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(10), theme::hover_bg());
        }
        let col = if enabled { theme::ink() } else { Color32::from_rgb(0xb4, 0xba, 0xc4) };
        lucide::paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(19.0)), col, ops);
    }
    if enabled { dark_tip(ui, resp, tip) } else { resp }
}

/// A white pill of the header (Math Figures / Physics Figures); true when clicked.
fn header_pill(ui: &mut Ui, text: &str, wash: bool) -> bool {
    let t = RichText::new(text).family(theme::semibold_family()).size(13.6).color(blue_text());
    let b = egui::Button::new(t)
        .fill(if wash { theme::accent_faded() } else { theme::card() })
        .stroke(if wash { Stroke::NONE } else { Stroke::new(1.0, pill_border()) })
        .corner_radius(CornerRadius::same(8))
        .min_size(Vec2::new(0.0, 31.0));
    ui.scope(|ui| {
        ui.spacing_mut().button_padding.x = if wash { 11.5 } else { 10.5 };
        ui.add(b).clicked()
    })
    .inner
}

/// The chemistry header pill with its Lucide glyphs: `lead` (16 px) before the label, `trail`
/// (14 px) after it. The wash pill ("About") is 28.6 px tall, the outlined ones 31.3.
fn header_pill_lucide(ui: &mut Ui, text: &str, lead: Option<&[lucide::Op]>, trail: Option<&[lucide::Op]>, wash: bool) -> bool {
    let p = ui.painter().clone();
    let bio_hdr = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_hdr"))).unwrap_or(false);
    let g = p.layout_no_wrap(text.to_string(), egui::FontId::new(13.6, if bio_hdr && wash { theme::bold_family() } else { theme::semibold_family() }), blue_text());
    let pad = if wash { 10.4 } else { 10.5 };
    let gap = if bio_hdr && wash { 5.0 } else { 7.0 };
    let padl = if bio_hdr && wash { pad + 2.0 } else { pad };
    let mut w = pad + padl + g.size().x;
    if lead.is_some() {
        w += 16.0 + gap;
    }
    if trail.is_some() {
        w += 14.0 + gap - 3.5;
    }
    let h = if wash { 28.6 } else { 31.3 };
    // the physics reference's About chip is a pixel narrower and its Math Figures pill a pixel wider
    let phys_hdr = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_hdr"))).unwrap_or(false);
    if phys_hdr && wash {
        w -= 1.0;
    } else if phys_hdr && text.starts_with("Math") {
        w += 0.4;
    }
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let fill = if wash { theme::accent_faded() } else { theme::card() };
    p.rect(rect, CornerRadius::same(8), if resp.hovered() { theme::hover_bg() } else { fill }, if wash { Stroke::NONE } else { Stroke::new(1.0, pill_border()) }, egui::StrokeKind::Inside);
    let mut x = rect.left() + padl;
    if let Some(ops) = lead {
        lucide::paint(&p, Rect::from_min_size(Pos2::new(x, rect.center().y - 8.0), Vec2::splat(16.0)), blue_text(), ops);
        x += 16.0 + gap;
    }
    p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0 - if bio_hdr { -0.3 } else { 0.7 }), g.clone(), blue_text());
    x += g.size().x;
    if let Some(ops) = trail {
        x += gap - 3.5;
        lucide::paint(&p, Rect::from_min_size(Pos2::new(x, rect.center().y - 7.0), Vec2::splat(14.0)), blue_text(), ops);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// A single-line label painted 1 px above the centre line of a centred row.
fn lifted_label(ui: &mut Ui, text: &str, fam: egui::FontFamily, size: f32, col: Color32, sense: Sense) -> egui::Response {
    let g = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::new(size, fam), col);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(g.size().x, g.size().y + 2.0), sense);
    ui.painter().galley(rect.min, g, col);
    resp
}

impl crate::App {
    /// The reference's 56 px white top bar. Returns true when the logo / wordmark was clicked
    /// (back to the figure list).
    pub(super) fn chem_header(&mut self, root: &mut Ui, title: &str) {
        let subj = self.figs.ws.skin_subject();
        let bio = subj == figures::Subject::Biology;
        let math = subj == figures::Subject::Math;
        let phys = subj == figures::Subject::Physics;
        let name = if phys { "Physics Figures" } else if bio { "Biology Figures" } else if math { "Math Figures" } else { "Chemistry Figures" };
        let narrow = root.max_rect().width() < 640.0;
        let chem_hdr = !bio && !math;
        root.ctx().data_mut(|d| d.insert_temp(egui::Id::new("bio_hdr"), bio));
        root.ctx().data_mut(|d| d.insert_temp(egui::Id::new("phys_hdr"), phys));
        let mut go_list = false;
        let mut jump: Option<figures::Subject> = None;
        let mut about = false;
        egui::Panel::top("chem_header")
            .exact_size(HEADER_H)
            .show_separator_line(false)
            .frame(Frame::new().fill(theme::card()).inner_margin(Margin { left: 20, right: 20, top: 0, bottom: if bio { 1 } else { 0 } }))
            .show(root, |ui| {
                let r = ui.max_rect();
                ui.painter().hline(r.x_range().expand(20.0), r.bottom() - 0.5 + if bio { 1.0 } else { 0.0 }, Stroke::new(1.0, theme::border()));
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let (tile, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                    ui.painter().rect_filled(tile, CornerRadius::same(7), theme::accent_col());
                    if math {
                        // the reference's chart-increasing emoji: a white grid card with a red rising line
                        let pr = ui.painter();
                        let card = Rect::from_center_size(tile.center(), Vec2::splat(19.0));
                        pr.rect_filled(card, CornerRadius::same(2), Color32::WHITE);
                        let g = Stroke::new(0.6, Color32::from_rgb(190, 200, 220));
                        for i in 1..4 {
                            let f = i as f32 / 4.0;
                            pr.vline(card.left() + card.width() * f, card.y_range(), g);
                            pr.hline(card.x_range(), card.top() + card.height() * f, g);
                        }
                        let q = |x: f32, y: f32| Pos2::new(card.left() + card.width() * x, card.top() + card.height() * y);
                        pr.add(egui::Shape::line(vec![q(0.1, 0.8), q(0.4, 0.45), q(0.6, 0.62), q(0.9, 0.2)], Stroke::new(1.6, Color32::from_rgb(225, 40, 50))));
                    } else if chem_hdr && !phys {
                        // the reference's flask emoji: a glass flask with a blue liquid line
                        let pr = ui.painter();
                        let o = tile.min;
                        let q = |x: f32, y: f32| Pos2::new(o.x + x, o.y + y);
                        let ink = Color32::from_rgb(0x1e, 0x29, 0x3b);
                        let glass = Color32::from_rgb(0xe9, 0xee, 0xf7);
                        let flask = vec![q(11.0, 5.0), q(17.0, 5.0), q(17.0, 12.0), q(22.2, 21.8), q(21.2, 23.4), q(6.8, 23.4), q(5.8, 21.8), q(11.0, 12.0)];
                        pr.add(egui::Shape::convex_polygon(flask, glass, Stroke::new(1.0, ink)));
                        pr.add(egui::Shape::convex_polygon(vec![q(8.6, 17.4), q(19.4, 17.4), q(21.3, 21.6), q(20.6, 22.6), q(7.4, 22.6), q(6.7, 21.6)], Color32::from_rgb(0x93, 0xc5, 0xfd), Stroke::NONE));
                        pr.rect_filled(Rect::from_min_max(q(9.6, 3.8), q(18.4, 5.4)), CornerRadius::same(1), glass);
                        pr.rect_stroke(Rect::from_min_max(q(9.6, 3.8), q(18.4, 5.4)), CornerRadius::same(1), Stroke::new(1.0, ink), egui::StrokeKind::Middle);
                    } else if phys {
                        lucide::paint(ui.painter(), Rect::from_center_size(tile.center(), Vec2::splat(20.0)), theme::on_accent_col(), lucide::ATOM);
                    } else {
                        ui.painter().text(tile.center(), egui::Align2::CENTER_CENTER, if bio { ph::DNA } else { ph::FLASK }, theme::icon_font(18.0), theme::on_accent_col());
                    }
                    go_list |= resp.clicked();
                    ui.add_space(-1.0);
                    // the biology page sets its wordmark and title 1 px above where we centre them
                    let w = if bio || phys {
                        lifted_label(ui, name, theme::xbold_family(), 17.6, theme::ink(), Sense::click())
                    } else {
                        ui.add(egui::Label::new(RichText::new(name).family(theme::xbold_family()).size(17.6).color(theme::ink())).sense(Sense::click()))
                    };
                    go_list |= w.clicked();
                    if !narrow && !title.is_empty() {
                        // the chemistry reference's chevron and title sit 3 px nearer the wordmark
                        ui.add_space(if bio { 4.5 } else if !math && !phys { 6.0 } else { 9.0 });
                        ui.label(RichText::new(ph::CARET_RIGHT).size(13.0).color(theme::muted_col()));
                        if bio {
                            ui.add_space(2.0);
                        }
                        if phys {
                            ui.add_space(-3.0);
                        }
                        if bio || phys || chem_hdr {
                            lifted_label(ui, title, theme::semibold_family(), 16.0, theme::muted_col(), Sense::hover());
                        } else {
                            ui.add(egui::Label::new(RichText::new(title).family(theme::semibold_family()).size(16.0).color(theme::muted_col())).truncate());
                        }
                    }
                    if narrow && phys && title.is_empty() {
                        // the physics list has no About on a phone
                    } else if narrow {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            about |= if chem_hdr || bio { header_pill_lucide(ui, "About", Some(lucide::INFO), None, true) } else { header_pill(ui, &format!("{}  About", ph::INFO), true) };
                        });
                    } else {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = if bio || phys { 12.0 } else { 12.5 };
                            let others: &[(&str, figures::Subject)] = if phys {
                                &[("Chemistry Figures", figures::Subject::Chemistry), ("Math Figures", figures::Subject::Math)]
                            } else if bio {
                                &[("Chemistry Figures", figures::Subject::Chemistry), ("Physics Figures", figures::Subject::Physics), ("Math Figures", figures::Subject::Math)]
                            } else if math {
                                &[("Chemistry Figures", figures::Subject::Chemistry), ("Physics Figures", figures::Subject::Physics)]
                            } else {
                                &[("Physics Figures", figures::Subject::Physics), ("Math Figures", figures::Subject::Math)]
                            };
                            for (name, sub) in others {
                                let clicked = if chem_hdr || bio { header_pill_lucide(ui, name, None, Some(lucide::ARROW_UP_RIGHT), false) } else { header_pill(ui, &format!("{name} {}", ph::ARROW_UP_RIGHT), false) };
                                if clicked {
                                    jump = Some(*sub);
                                }
                            }
                            if !title.is_empty() {
                                about |= if chem_hdr || bio { header_pill_lucide(ui, "About", Some(lucide::INFO), None, true) } else { header_pill(ui, &format!("{}  About", ph::INFO), true) };
                            }
                        });
                    }
                });
            });
        let ctx = root.ctx().clone();
        let about_id = egui::Id::new("chem_about_open");
        if about {
            ctx.memory_mut(|m| m.data.insert_temp(about_id, true));
        }
        let mut open = ctx.memory(|m| m.data.get_temp::<bool>(about_id)).unwrap_or(false);
        let fig_id = self.figs.ws.sel.map(|i| super::reg()[i].id);
        if open && let Some(doc) = fig_id.and_then(about_doc) {
            if about_drawer(&ctx, doc) {
                open = false;
            }
            ctx.memory_mut(|m| m.data.insert_temp(about_id, open));
        } else if open {
            egui::Window::new(format!("About {name}")).open(&mut open).collapsible(false).resizable(false).show(&ctx, |ui| {
                ui.set_max_width(360.0);
                ui.label(if phys {
                    "Printable physics figures for tests and worksheets: waves, motion graphs, forces, circuits and more. Tune a figure on the left, then copy it, save it as PNG or SVG, or attach it to a question."
                } else if math {
                    "Printable math figures for tests and worksheets: triangles and polygons, solids, graphs, number lines and more. Tune a figure on the left, then copy it, save it as PNG or SVG, or attach it to a question."
                } else if bio {
                    "Printable biology figures for tests and worksheets: Punnett squares, pedigrees, gels, graphs and more. Tune a figure on the left, then copy it, save it as PNG or SVG, or attach it to a question."
                } else {
                    "Printable chemistry figures for tests and worksheets: instruments and readings, curves, spectra and atoms. Tune a figure on the left, then copy it, save it as PNG or SVG, or attach it to a question."
                });
            });
            ctx.memory_mut(|m| m.data.insert_temp(about_id, open));
        }
        if go_list {
            self.figs.ws.back();
        }
        if let Some(s) = jump {
            self.figs.ws.back();
            self.figs.ws.subject = Some(s);
        }
    }
}

impl Workspace {
    /// The chemistry skin applies to a generated chemistry figure in the standalone (non-modal) view.
    pub(super) fn chem_open(&self) -> bool {
        SITE_SKINS && !self.modal && self.imp.is_none() && self.sel.is_some_and(|i| skinned(super::reg()[i].subject) && super::reg()[i].id != "composite-shape")
    }

    /// The subject whose page skin is showing (chemistry or biology), if any.
    pub(super) fn skin_subject(&self) -> figures::Subject {
        // a figure the other site lists too (the length reading) wears the page it was opened from
        let own = self.sel.map(|i| &super::reg()[i]);
        own.map(|d| match self.subject {
            Some(s) if figures::also_listed(d.id).contains(&s) => s,
            _ => d.subject,
        })
        .or(self.subject)
        .filter(|s| skinned(*s))
        .unwrap_or(figures::Subject::Chemistry)
    }

    /// The icon toolbar across the top of the preview card: copy, image, download, share |
    /// undo, redo; on the right the extras this app adds (Presets, Handles, label size, Save).
    pub(super) fn chem_toolbar(&mut self, ui: &mut Ui) -> Act {
        let ctx = ui.ctx().clone();
        let mut act = Act::None;
        let have_svg = true;
        let chem_subj = self.skin_subject() == figures::Subject::Chemistry;
        let chem = chem_page(&ctx) || chem_subj || ctx.data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false) || ctx.data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false);
        let narrow = ui.ctx().content_rect().width() < 500.0;
        let bar = ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), if narrow { if self.skin_subject() == figures::Subject::Math { 48.0 } else { 46.0 } } else { 53.0 }), Layout::left_to_right(Align::Center), |ui| {
            // a phone spreads the six icons evenly across the card, as the reference does
            let phys_page = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false);
            let has_track = self.sel.is_some_and(|si| (super::reg()[si].params)().iter().any(|d| d.key == "label_size" && matches!(&d.kind, crate::figures::Kind::Choice(o) if o.len() == 3)));
            // the physics phone toolbar with no label-size track spreads its six icons edge to edge
            let phys_spread = narrow && ((phys_page && !has_track) || chem_page(&ctx) || chem_subj);
            ui.spacing_mut().item_spacing.x = if phys_spread { (ui.available_width() - 11.6 - 6.0 * 34.4) / 5.0 } else if narrow && !chem { -0.8 } else if narrow && self.skin_subject() == figures::Subject::Biology { 2.6 } else if narrow { ((ui.available_width() - 20.0 - 6.0 * 38.0) / 5.0).max(2.0) } else if chem { 4.8 } else { 5.0 };
            ui.add_space(if phys_spread { 5.8 } else if narrow && !chem { 2.0 } else if narrow { 10.0 } else if chem { 7.2 } else { 10.0 });
            if tool_btn(ui, chem, lucide::COPY, ph::COPY, "Copy image", true).clicked() {
                act = match self.output().ok().and_then(|o| super::color_image(&o.png)) {
                    Some(ci) => {
                        ctx.copy_image(ci);
                        Act::Toast("Copied PNG to the clipboard".into(), false)
                    }
                    None => Act::Toast("Couldn't render the figure".into(), true),
                };
            }
            if tool_btn(ui, chem, lucide::IMAGE_DOWN, ph::IMAGE, "Download PNG", true).clicked() {
                act = self.export(false);
            }
            if tool_btn(ui, chem, lucide::FILE_DOWN, ph::FILE_ARROW_DOWN, "Download SVG", have_svg).clicked() {
                act = self.export(true);
            }
            let share = tool_btn(ui, chem, lucide::SHARE, ph::EXPORT, "Share link", true);
            // the reference copies on a plain click; the extras this app adds live on a right-click
            if share.clicked()
                && let Some(l) = self.share_link()
            {
                ui.ctx().copy_text(l);
                self.toast = Some(("Link copied. Share it, and anyone who opens it sees this exact figure.".into(), false));
            }
            share.context_menu(|ui| {
                ui.set_min_width(300.0);
                ui.label(theme::muted("Share link works offline: the link itself holds the settings.").size(11.0));
                ui.separator();
                if self.link_box(ui, 190.0) {
                    ui.close();
                }
                // this app's extras live here, so the toolbar itself matches the reference's
                if !self.math_panel() {
                    ui.separator();
                    self.chem_extras(ui, &mut act);
                }
            });
            if !narrow {
                let (sep, _) = ui.allocate_exact_size(Vec2::new(if chem { 10.5 } else { 8.0 }, 24.0), Sense::hover());
                ui.painter().vline(sep.center().x, sep.y_range(), Stroke::new(1.0, theme::border()));
            }
            let can_undo = !self.undo.is_empty() || self.hist_base.is_some();
            let can_redo = !self.redo.is_empty();
            if tool_btn(ui, chem, lucide::UNDO, ph::ARROW_U_UP_LEFT, "Undo", can_undo).clicked() {
                self.step_history(true);
            }
            if tool_btn(ui, chem, lucide::REDO, ph::ARROW_U_UP_RIGHT, "Redo", can_redo).clicked() {
                self.step_history(false);
            }
        });
        let r = bar.response.rect;
        if let Some(a) = self.label_size_track(ui, r) {
            let _ = a;
        }
        ui.painter().hline(ui.max_rect().x_range(), r.bottom(), Stroke::new(1.0, theme::border()));
        act
    }

    /// The reference's three serif "A" buttons (large, medium, small labels) in a grey track at
    /// the right end of the toolbar, for figures with a three-way `label_size`.
    fn label_size_track(&mut self, ui: &mut Ui, bar: Rect) -> Option<()> {
        let si = self.sel?;
        let ls = (super::reg()[si].params)().into_iter().find(|d| d.key == "label_size")?;
        if !matches!(&ls.kind, crate::figures::Kind::Choice(o) if o.len() == 3) {
            return None;
        }
        let cur = self.params.s("label_size").to_string();
        let track = Rect::from_min_size(Pos2::new(ui.max_rect().right() - 7.0 - 110.0, bar.center().y - 17.0), Vec2::new(110.0, 34.0));
        let wash = if theme::is_dark() { Color32::from_rgb(0x11, 0x16, 0x21) } else if chem_page(ui.ctx()) { Color32::from_rgb(0xf6, 0xf7, 0xfb) } else { Color32::from_rgb(0xf0, 0xf1, 0xf6) };
        ui.painter().rect_filled(track, CornerRadius::same(10), wash);
        for (i, (name, px)) in [("Large", 20.0), ("Medium", 13.5), ("Small", 11.0)].into_iter().enumerate() {
            let cell = Rect::from_min_size(Pos2::new(track.left() + 3.0 + i as f32 * 34.7, track.top() + 3.0), Vec2::new(34.0, 28.0));
            let resp = ui.interact(cell, ui.id().with(("lsz", i)), Sense::click());
            let sel = cur == name;
            if sel {
                ui.painter().rect(cell, CornerRadius::same(7), theme::card(), Stroke::new(1.0, theme::border()), egui::StrokeKind::Inside);
            }
            let col = if sel { blue_text() } else { theme::muted_col() };
            ui.painter().text(cell.center(), egui::Align2::CENTER_CENTER, "A", egui::FontId::new(px, egui::FontFamily::Name("vs-serif".into())), col);
            if resp.on_hover_text(format!("{name} labels")).clicked() {
                self.params.0.insert("label_size".to_string(), crate::figures::Value::S(name.to_string()));
            }
        }
        Some(())
    }

    /// Save / Handles / Presets: this app's extras next to the reference's icon toolbar.
    fn chem_extras(&mut self, ui: &mut Ui, act: &mut Act) {
        ui.spacing_mut().item_spacing.x = 8.0;
        if super::accent_btn(ui, "Save to question…").clicked() {
            *act = match self.output() {
                Ok(o) => Act::Save(o),
                Err(e) => Act::Toast(e, true),
            };
        }
        if self.has_answers() && ui.button("Student + key").on_hover_text("Save the student copy (answers hidden) and the answer key side by side").clicked() {
            *act = self.export_pair(false);
        }
        if super::chip_handles(ui, !self.hide_handles) {
            self.hide_handles = !self.hide_handles;
        }
        self.presets_menu(ui);
    }
}

/// One white rounded card of the controls column (soft shadow, hairline border).
pub fn panel_card<R>(ui: &mut Ui, pad: i8, f: impl FnOnce(&mut Ui) -> R) -> R {
    panel_card_m(ui, Margin::same(pad), f)
}

fn card_shapes(rect: Rect) -> egui::Shape {
    let r = CornerRadius::same(16);
    let sh = egui::Shadow { offset: [0, 3], blur: 16, spread: 0, color: Color32::from_black_alpha(if theme::is_dark() { 70 } else { 20 }) };
    egui::Shape::Vec(vec![
        sh.as_shape(rect, r).into(),
        egui::epaint::RectShape::new(rect, r, theme::card(), Stroke::new(1.0, theme::border()), egui::StrokeKind::Inside).into(),
    ])
}

/// One controls card that the page splits in two where `mark_split` is called (the titration's flat
/// first section, then a second card of collapsible sections 16 px below).
pub fn split_cards(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    let a = ui.painter().add(egui::Shape::Noop);
    let b = ui.painter().add(egui::Shape::Noop);
    let id = egui::Id::new("chem_card_split");
    ui.ctx().data_mut(|d| d.remove_temp::<f32>(id));
    let resp = Frame::new()
        .inner_margin(Margin::same(1))
        .show(ui, |ui| {
            ui.set_width(w - 2.0);
            f(ui)
        })
        .response;
    let rect = resp.rect;
    match ui.ctx().data(|d| d.get_temp::<f32>(id)) {
        Some(bottom) => {
            ui.painter().set(a, card_shapes(Rect::from_min_max(rect.min, Pos2::new(rect.right(), bottom))));
            ui.painter().set(b, card_shapes(Rect::from_min_max(Pos2::new(rect.left(), bottom + 16.0), rect.max)));
        }
        None => ui.painter().set(a, card_shapes(rect)),
    }
}

/// Ends the first card of `split_cards` here and starts the next one 16 px lower.
pub fn mark_split(ui: &mut Ui) {
    let bottom = ui.cursor().top();
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("chem_card_split"), bottom));
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("chem_first_row"), true));
    ui.add_space(17.0);
}

/// `panel_card` with separate side margins.
pub fn panel_card_m<R>(ui: &mut Ui, m: Margin, f: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(16))
        .shadow(egui::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(if theme::is_dark() { 60 } else { 10 }) })
        .inner_margin(m)
        .show(ui, |ui| {
            ui.set_width(w - 2.0 - (m.left + m.right) as f32);
            f(ui)
        })
        .inner
}

/// Full-width outlined blue button ("Start from an example", "Random"). A leading phosphor icon
/// in the text is drawn as one centred icon + label group, as the reference does.
pub fn outline_button(ui: &mut Ui, text: &str, enabled: bool) -> egui::Response {
    let (icon, label) = match text.split_once("  ") {
        Some((i, l)) if i.chars().count() == 1 => (Some(i.to_string()), l.to_string()),
        _ => (None, text.to_string()),
    };
    let bio_h = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), if bio_h { 39.2 } else { 40.0 }), if enabled { Sense::click() } else { Sense::hover() });
    let hov = enabled && resp.hovered();
    let col = if enabled { blue_text() } else { blue_text().gamma_multiply(0.4) };
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(12), if hov { theme::hover_bg() } else { theme::card() }, Stroke::new(1.0, pill_border()), egui::StrokeKind::Inside);
    let g = p.layout_no_wrap(label, egui::FontId::new(15.2, theme::bold_family()), col);
    // the chemistry page draws the reference's "images" glyph (18 px); the others keep the font icon
    let phys_pg = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false);
    let chem = chem_page(ui.ctx()) || phys_pg || ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false);
    let ig = icon.as_ref().filter(|_| !chem).map(|i| p.layout_no_wrap(i.clone(), theme::icon_font(17.0), col));
    let gap = if phys_pg { 8.0 } else if chem { 7.5 } else { 7.0 };
    let icon_w = if chem && icon.is_some() { 18.0 } else { ig.as_ref().map_or(0.0, |i| i.size().x) };
    let total = g.size().x + if icon_w > 0.0 { icon_w + gap } else { 0.0 };
    let mut x = rect.center().x - total / 2.0;
    // the reference sets the label's glyphs 1.5 px above the box's middle
    let cy = rect.center().y - if phys_pg { 2.0 } else if chem { 1.5 } else { 0.0 };
    if chem && icon.is_some() {
        lucide::paint_grid(&p, Rect::from_min_size(Pos2::new(x, cy - 9.0), Vec2::splat(18.0)), col, lucide::IMAGES, 18.0, 1.5);
        x += icon_w + gap;
    } else if let Some(i) = ig {
        p.galley(Pos2::new(x, cy - i.size().y / 2.0), i.clone(), col);
        x += i.size().x + gap;
    }
    p.galley(Pos2::new(x, cy - g.size().y / 2.0), g.clone(), col);
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// A small outlined blue button that hugs its text ("Random").
pub fn outline_chip(ui: &mut Ui, text: &str) -> egui::Response {
    let b = egui::Button::new(RichText::new(text).family(theme::bold_family()).size(14.4).color(blue_text()))
        .fill(theme::card())
        .stroke(Stroke::new(1.0, pill_border()))
        .corner_radius(CornerRadius::same(8))
        .min_size(Vec2::new(0.0, 35.0));
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = Vec2::new(14.0, 3.0);
        ui.add(b)
    })
    .inner
}

/// `outline_chip` with a leading Lucide icon (16 px, 14 px in from the edge, 7 px before the text).
pub fn outline_chip_icon(ui: &mut Ui, ops: &[lucide::Op], text: &str) -> egui::Response {
    let r = outline_chip(ui, &format!("      {text}"));
    let c = Pos2::new(r.rect.left() + 14.0 + 8.0, r.rect.center().y);
    lucide::paint(ui.painter(), Rect::from_center_size(c, Vec2::splat(16.0)), blue_text(), ops);
    r
}

/// The reference's native-select chevron: a small 2 px "v" whose centre sits 10 px in from the box's
/// right edge (egui hands over the icon rect, whose right edge is the box's padding edge).
pub fn select_chevron(ui: &Ui, rect: Rect, _visuals: &egui::style::WidgetVisuals, _open: bool) {
    let c = Pos2::new(rect.right() + ui.spacing().button_padding.x - 9.0, rect.center().y + 0.9);
    let col = theme::ink();
    ui.painter().add(egui::Shape::line(
        vec![Pos2::new(c.x - 3.7, c.y - 2.0), Pos2::new(c.x, c.y + 1.7), Pos2::new(c.x + 3.7, c.y - 2.0)],
        Stroke::new(2.3, col),
    ));
}

/// A pill-shaped chip with a dashed blue outline and a leading icon ("Save preset").
pub fn dashed_chip(ui: &mut Ui, text: &str) -> egui::Response {
    let (icon, label) = match text.split_once("  ") {
        Some((i, l)) => (i.to_string(), l.to_string()),
        None => (String::new(), text.to_string()),
    };
    let p = ui.painter().clone();
    let ph = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false);
    let (fs, is, gap, pad, hh) = if ph { (15.0, 14.0, 6.2, 31.8, 38.0) } else { (15.2, 15.0, 7.0, 36.0, 37.0) };
    let g = p.layout_no_wrap(label, egui::FontId::new(fs, theme::bold_family()), blue_text());
    let ig = p.layout_no_wrap(icon, theme::icon_font(is), blue_text());
    let w = g.size().x + if ph { 14.0 } else { ig.size().x } + gap + pad;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, hh), Sense::click());
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(19), theme::hover_bg());
    }
    let st = Stroke::new(1.0, if ph { Color32::from_rgb(218, 226, 240) } else { pill_border() });
    // a dashed rounded outline: straight dashes along the sides, short arcs at the ends
    let r = rect.height() / 2.0;
    let mut pts: Vec<Pos2> = vec![];
    let steps = 14;
    for i in 0..=steps {
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / steps as f32;
        pts.push(Pos2::new(rect.right() - r + a.cos() * r, rect.center().y + a.sin() * r));
    }
    for i in 0..=steps {
        let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / steps as f32;
        pts.push(Pos2::new(rect.left() + r + a.cos() * r, rect.center().y + a.sin() * r));
    }
    pts.push(pts[0]);
    p.extend(egui::Shape::dashed_line(&pts, st, 4.0, 3.0));
    if ph {
        // the reference's lucide bookmark-plus, 14 px, 17 px in from the edge
        let x = rect.left() + 16.3;
        lucide::paint_grid(&p, Rect::from_min_size(Pos2::new(x, rect.center().y - 7.0), Vec2::splat(14.0)), blue_text(), lucide::BOOKMARK_PLUS, 24.0, 2.0);
        p.galley(Pos2::new(x + 14.0 + gap, rect.center().y - g.size().y / 2.0), g, blue_text());
        return resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    let mut x = rect.center().x - (g.size().x + ig.size().x + gap) / 2.0;
    p.galley(Pos2::new(x, rect.center().y - ig.size().y / 2.0), ig.clone(), blue_text());
    x += ig.size().x + gap;
    p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0), g, blue_text());
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The flat first-section header: small letter-spaced caps title and a help "?" on the right.
pub fn flat_header(ui: &mut Ui, title: &str, help: &str) {
    let w = ui.available_width();
    let chem = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("chem_skin_page"))).unwrap_or(false);
    let bio = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, if bio { 48.0 } else if chem { 49.0 } else { 50.0 }), Sense::hover());
    let p = ui.painter();
    let caps: Vec<String> = title.to_uppercase().chars().map(|c| c.to_string()).collect();
    let font = egui::FontId::new(12.5, theme::bold_family());
    let ty_off = if bio { 29.0 } else if chem { 28.0 } else { 31.0 };
    let mut x = rect.left() + 18.0;
    for c in caps {
        let g = p.layout_no_wrap(c, font.clone(), theme::muted_col());
        let adv = g.size().x + 1.1;
        p.galley(Pos2::new(x, rect.top() + ty_off - g.size().y / 2.0), g, theme::muted_col());
        x += adv;
    }
    let c = Pos2::new(rect.right() - 32.0, rect.top() + if bio { 30.0 } else if chem { 29.0 } else { 31.0 });
    let hit = Rect::from_center_size(c, Vec2::splat(22.0));
    let hov = ui.rect_contains_pointer(hit);
    let col = if hov { blue_text() } else { theme::muted_col() };
    let p = ui.painter();
    if hov {
        p.circle_filled(c + Vec2::new(0.4, 0.3), 13.6, Color32::from_rgb(239, 246, 255));
    }
    p.circle_stroke(c + Vec2::new(0.4, 0.3), if bio { 6.0 } else { 6.5 }, Stroke::new(1.5, col));
    p.text(c + Vec2::new(0.4, -0.1), egui::Align2::CENTER_CENTER, "?", egui::FontId::new(11.0, theme::bold_family()), col);
    let tip = if help.is_empty() { tip_for(title) } else { help };
    if hov && !tip.is_empty() {
        // the reference's dark 272 px tooltip, left-aligned a little left of the icon, just below it
        egui::Area::new(egui::Id::new("flat_header_tip")).order(egui::Order::Tooltip).fixed_pos(Pos2::new(c.x - 13.0, c.y + 20.0)).pivot(egui::Align2::LEFT_TOP).interactable(false).show(ui.ctx(), |ui| {
            let font = egui::FontId::new(12.8, theme::medium_family());
            let mut job = egui::text::LayoutJob::single_section(tip.to_string(), egui::TextFormat { font_id: font.clone(), color: Color32::WHITE, line_height: Some(17.8), ..Default::default() });
            job.wrap.max_width = 272.0 - 22.4;
            let g = ui.painter().layout_job(job);
            // the browser's 1.4 line box is 17.92 px, which egui would round to 18 a line; each line is
            // placed on its own so the box keeps the reference's height
            let pitch = 17.92;
            let n = g.rows.len() as f32;
            let (r, _) = ui.allocate_exact_size(Vec2::new(272.0, n * pitch + 17.6), Sense::hover());
            ui.painter().rect_filled(r, CornerRadius::same(8), Color32::from_rgb(31, 41, 55));
            for (i, row) in g.rows.iter().enumerate() {
                let text: String = row.row.glyphs.iter().map(|gl| gl.chr).collect();
                let lg = ui.painter().layout_no_wrap(text, font.clone(), Color32::WHITE);
                ui.painter().galley(r.min + Vec2::new(11.2, 8.1 + i as f32 * pitch + (pitch - lg.size().y) / 2.0), lg, Color32::WHITE);
            }
        });
    }
    let _ = resp;
}

/// The little "?" that follows a label: an outlined circle that turns blue on hover and opens the
/// reference's dark tooltip (272 px wide) under it.
pub fn help_dot(ui: &mut Ui, tip: &str) {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
    let hov = resp.hovered();
    let col = if hov { blue_text() } else { theme::muted_col() };
    let c = r.center();
    let p = ui.painter();
    if hov {
        p.circle_filled(c, 11.0, Color32::from_rgb(239, 246, 255));
    }
    p.circle_stroke(c, 7.5, Stroke::new(1.4, col));
    p.text(c + Vec2::new(0.0, -0.1), egui::Align2::CENTER_CENTER, "?", egui::FontId::new(11.0, theme::bold_family()), col);
    if hov && !tip.is_empty() {
        egui::Area::new(egui::Id::new("help_dot_tip")).order(egui::Order::Tooltip).fixed_pos(Pos2::new(c.x - 13.0, c.y + 20.0)).pivot(egui::Align2::LEFT_TOP).interactable(false).show(ui.ctx(), |ui| {
            let font = egui::FontId::new(12.8, theme::medium_family());
            let mut job = egui::text::LayoutJob::single_section(tip.to_string(), egui::TextFormat { font_id: font.clone(), color: Color32::WHITE, line_height: Some(17.8), ..Default::default() });
            job.wrap.max_width = 272.0 - 22.4;
            let g = ui.painter().layout_job(job);
            let pitch = 17.92;
            let n = g.rows.len() as f32;
            let (r, _) = ui.allocate_exact_size(Vec2::new(272.0, n * pitch + 17.6), Sense::hover());
            ui.painter().rect_filled(r, CornerRadius::same(8), Color32::from_rgb(31, 41, 55));
            for (i, row) in g.rows.iter().enumerate() {
                let text: String = row.row.glyphs.iter().map(|gl| gl.chr).collect();
                let lg = ui.painter().layout_no_wrap(text, font.clone(), Color32::WHITE);
                ui.painter().galley(r.min + Vec2::new(11.2, 8.1 + i as f32 * pitch + (pitch - lg.size().y) / 2.0), lg, Color32::WHITE);
            }
        });
    }
}

#[derive(serde::Deserialize)]
struct AboutDoc {
    title: String,
    paras: Vec<String>,
    bullets: Vec<String>,
    copy: String,
    faqs: Vec<(String, String)>,
}

/// The reference's per-figure About text (captured from the live site), keyed by figure id.
fn about_doc(id: &str) -> Option<&'static AboutDoc> {
    static DOCS: std::sync::OnceLock<std::collections::HashMap<String, AboutDoc>> = std::sync::OnceLock::new();
    DOCS.get_or_init(|| serde_json::from_str(include_str!("../../assets/chem_about.json")).unwrap_or_default()).get(id)
}

/// The full-height right-hand "About" drawer: a 543 px white panel over a dimmed page, with the
/// figure's two paragraphs, "What you can set", "Copying, printing and sharing" and its FAQs.
/// Returns true when it should close.
fn about_drawer(ctx: &egui::Context, d: &AboutDoc) -> bool {
    let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let screen = ctx.content_rect();
    let w = 543.0f32.min(screen.width());
    let panel = Rect::from_min_max(Pos2::new(screen.right() - w, screen.top()), screen.max);
    egui::Area::new(egui::Id::new("chem_about_drawer")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        let bg = ui.interact(screen, egui::Id::new("chem_about_bg"), Sense::click());
        ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(77));
        close |= bg.clicked();
        ui.interact(panel, egui::Id::new("chem_about_panel"), Sense::click());
        ui.painter().rect_filled(panel, 0.0, theme::card());
        ui.painter().vline(panel.left() - 0.5, panel.y_range(), Stroke::new(1.0, Color32::from_black_alpha(25)));
        let inner = Rect::from_min_max(panel.min + Vec2::new(24.0, 0.0), panel.max - Vec2::new(17.0, 0.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            let ink = theme::ink();
            fn body(ui: &mut Ui, t: &str) {
                ui.label(RichText::new(t).size(15.9).color(theme::ink()).line_height(Some(23.0)));
            }
            let (hr, _) = ui.allocate_exact_size(Vec2::new(inner.width(), 58.0), Sense::hover());
            ui.painter().text(Pos2::new(hr.left(), hr.top() + 29.0), egui::Align2::LEFT_CENTER, &d.title, egui::FontId::new(19.2, theme::bold_family()), ink);
            let xr = Rect::from_center_size(Pos2::new(hr.right() - 10.0, hr.top() + 34.0), Vec2::splat(28.0));
            let xo = ui.interact(xr, egui::Id::new("chem_about_x"), Sense::click());
            let xc = if xo.hovered() { ink } else { theme::muted_col() };
            let xs = Stroke::new(1.8, xc);
            ui.painter().line_segment([xr.center() + Vec2::splat(-6.0), xr.center() + Vec2::splat(6.0)], xs);
            ui.painter().line_segment([xr.center() + Vec2::new(-6.0, 6.0), xr.center() + Vec2::new(6.0, -6.0)], xs);
            close |= xo.on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
            egui::ScrollArea::vertical().id_salt("chem_about_scroll").auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(inner.width());
                ui.spacing_mut().item_spacing.y = 0.0;
                for p in &d.paras {
                    ui.add_space(4.0);
                    body(ui, p);
                    ui.add_space(10.0);
                }
                ui.add_space(4.0);
                ui.label(RichText::new("What you can set").family(theme::bold_family()).size(16.0).color(ink));
                ui.add_space(6.0);
                for b in &d.bullets {
                    ui.horizontal_top(|ui| {
                        ui.add_space(4.0);
                        ui.label(RichText::new("\u{2022}").size(15.9).color(ink));
                        ui.add_space(-2.0);
                        ui.add(egui::Label::new(RichText::new(b).size(15.9).color(ink).line_height(Some(23.0))).wrap());
                    });
                    ui.add_space(3.0);
                }
                ui.add_space(14.0);
                ui.label(RichText::new("Copying, printing and sharing").family(theme::bold_family()).size(16.0).color(ink));
                ui.add_space(6.0);
                body(ui, &d.copy);
                ui.add_space(22.0);
                ui.label(RichText::new("Frequently asked questions").family(theme::bold_family()).size(19.2).color(ink));
                ui.add_space(8.0);
                for (q, a) in &d.faqs {
                    egui::CollapsingHeader::new(RichText::new(q).family(theme::semibold_family()).size(15.0).color(ink)).id_salt(q).show(ui, |ui| {
                        body(ui, a);
                        ui.add_space(6.0);
                    });
                }
                ui.add_space(20.0);
            });
        });
    });
    close
}

/// The "?" tooltip on a flat section header, keyed by the section title.
fn tip_for(title: &str) -> &'static str {
    match title {
        "Titration" => "From concentrations, the pH at each volume is solved exactly from the chemistry, so a weak acid is at its pKa halfway to the equivalence point. From key points, the curve is a real titration curve bent to pass through the pH values you type.",
        "Curve" => "Schematic, you set how long each segment is, for a simple worksheet curve. To scale, each sloped segment takes q = m\u{b7}c\u{b7}\u{394}T and each plateau q = n\u{b7}\u{394}H, from the substance's specific heats and enthalpies: water's boiling plateau is almost 7 times as long as its melting plateau.",
        _ => "",
    }
}

/// A collapsible section's header row: blue icon tile, bold title, one-line summary, chevron.
pub fn section_header(ui: &mut Ui, title: &str, summary: &str, open: bool) -> egui::Response {
    let w = ui.available_width();
    let chem = chem_page(ui.ctx());
    let phys_pg = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false);
    // the chemistry reference's rows are 70 px with a summary
    // the first row of a card sits flush on its border; the rest carry a 1 px rule of their own
    let first = ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::new("chem_first_row"))).unwrap_or(false);
    let h = if summary.is_empty() { 54.0 } else if chem { if first { 70.0 } else { 71.0 } } else if phys_pg { if first { 69.0 } else { 71.0 } } else if first { 70.0 } else { 62.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    let p = ui.painter();
    // hairline above every header but it also closes the card above the next one
    if !first {
        p.hline(rect.x_range(), rect.top(), Stroke::new(1.0, theme::border()));
    }
    if resp.hovered() {
        p.rect_filled(rect.shrink2(Vec2::new(1.0, 1.0)), CornerRadius::same(0), if phys_pg { theme::accent_faded() } else { theme::hover_bg().gamma_multiply(0.5) });
    }
    let math = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("math_skin_page"))).unwrap_or(false);
    // the math reference sits its row content 4 px lower and its chevron 4 px further in
    let bio = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false);
    let (dy, dx) = if math { (4.0, 4.0) } else if chem || bio { (0.0, 4.0) } else if phys_pg { (0.0, 4.0) } else { (0.0, 0.0) };
    let cy = rect.center().y + dy;
    let tile = Rect::from_min_size(Pos2::new(rect.left() + 18.0, cy - 16.0 - if first && chem { 1.0 } else { 0.0 }), Vec2::splat(32.0));
    p.rect_filled(tile, CornerRadius::same(8), theme::accent_faded());
    let phys = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false);
    let icon = phys_section_icon(title).filter(|_| phys).or_else(|| chem_section_icon(title).filter(|_| chem)).unwrap_or_else(|| section_icon(title, math));
    // the biology reference draws Symbols as shapes and Labels as a tag
    let tl = title.to_lowercase();
    let icon = if bio && tl.contains("symbol") { ph::SHAPES } else if bio && tl.contains("label") { ph::TAG } else if bio && tl.contains("title") { ph::TEXT_T } else { icon };
    let icon = if bio {
        match tl.as_str() {
            "graphs" | "graph" => ph::CHART_LINE,
            "curve" => ph::BEZIER_CURVE,
            "census data" => ph::CHART_SCATTER,
            "marked on the graph" | "marked" => ph::MAP_PIN,
            "titles" => ph::TEXT_H,
            "x-axis" => ph::ARROW_RIGHT,
            "y-axis" => ph::ARROW_UP,
            "grid" => ph::GRID_NINE,
            "gel" => ph::SLIDERS_HORIZONTAL,
            _ => icon,
        }
    } else { icon };
    let fig_id = ui.ctx().data(|d| d.get_temp::<&'static str>(egui::Id::new("phys_fig_id"))).unwrap_or("");
    let lops = if phys { phys_lucide_icon(fig_id, title) } else if chem { chem_lucide_icon(title) } else if bio && tl == "micropipette" { Some(lucide::PIPETTE) } else { None };
    match lops {
        Some(ops) => lucide::paint(p, Rect::from_center_size(tile.center(), Vec2::splat(18.0)), theme::accent_col(), ops),
        None => {
            p.text(tile.center(), egui::Align2::CENTER_CENTER, icon, theme::icon_font(17.0), theme::accent_col());
        }
    }
    let x = tile.right() + 12.0;
    let galley_w = (rect.right() - 44.0 - x).max(60.0);
    let t = p.layout(title.to_string(), egui::FontId::new(if chem { 15.68 } else { 16.0 }, theme::bold_family()), theme::ink(), galley_w);
    let s = (!summary.is_empty()).then(|| {
        let g = p.layout(summary.to_string(), egui::FontId::new(13.8, egui::FontFamily::Name("vs-noto".into())), theme::muted_col(), galley_w);
        // keep the summary to one line, as the reference does
        if g.rows.len() > 1 {
            // cut to the row's width with an ellipsis, as the reference does
            let chars: Vec<char> = summary.chars().collect();
            let mut n = chars.len();
            loop {
                n = n.saturating_sub(1);
                let t: String = chars[..n].iter().collect::<String>().to_string() + "\u{2026}";
                let g = p.layout_no_wrap(t, egui::FontId::new(13.8, egui::FontFamily::Name("vs-noto".into())), theme::muted_col());
                if g.size().x <= galley_w || n <= 4 {
                    break g;
                }
            }
        } else {
            g
        }
    });
    let total = t.size().y + s.as_ref().map_or(0.0, |g| g.size().y + 1.0);
    let y0 = cy - total / 2.0 - if phys_pg && first { 1.0 } else { 0.0 };
    let ty = t.size().y;
    p.galley(Pos2::new(x, y0), t, theme::ink());
    if let Some(g) = s {
        p.galley(Pos2::new(x, y0 + ty + 1.0), g, theme::muted_col());
    }
    if (chem || bio || phys_pg) && !math {
        // the reference's chevron is a 2 px stroked "v", a pixel above the row's middle
        let c = Pos2::new(rect.right() - 24.0 - dx + 1.0, cy - if phys_pg { 0.5 } else { 0.4 });
        let k = if open { -1.0 } else { 1.0 };
        p.add(egui::Shape::line(
            { let (hw, hh) = if phys_pg { (4.2, 2.1) } else { (4.8, 2.4) }; vec![Pos2::new(c.x - hw, c.y - hh * k), Pos2::new(c.x, c.y + hh * k), Pos2::new(c.x + hw, c.y - hh * k)] },
            Stroke::new(if phys_pg { 1.7 } else { 1.8 }, if phys_pg { Color32::from_rgb(88, 98, 114) } else { theme::muted_col() }),
        ));
        return resp;
    }
    p.text(
        Pos2::new(rect.right() - 24.0 - dx, cy - if phys_pg { 1.0 } else { 0.0 }),
        egui::Align2::CENTER_CENTER,
        if open { ph::CARET_UP } else { ph::CARET_DOWN },
        theme::icon_font(15.0),
        theme::muted_col(),
    );
    resp
}

impl Workspace {
    /// The wide chemistry layout: a 424 px controls column beside the preview card (with its
    /// toolbar), both running to the bottom of the page.
    pub(super) fn chem_wide(&mut self, ui: &mut Ui, avail_h: f32, preview: Option<&egui::TextureHandle>) {
        let math = self.skin_subject() == figures::Subject::Math;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("math_skin_page"), math));
        let phys = self.skin_subject() == figures::Subject::Physics;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("phys_skin_page"), phys));
        let fig_id = self.sel.map(|i| super::reg()[i].id).unwrap_or("");
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("phys_fig_id"), fig_id));
        let chem = self.skin_subject() == figures::Subject::Chemistry;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("chem_skin_page"), chem));
        let bio_pg = self.skin_subject() == figures::Subject::Biology;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("bio_skin_page"), bio_pg));
        let sub = self.skin_subject();
        // the reference sizes the controls column to its figure: the charts and spectra open a wider
        // preview card (996 px at 1440) and the photoelectron spectrum a slightly wider one (964 px)
        let fig_panel = match (sub, self.sel.map(|i| super::reg()[i].id)) {
            // the biology reference opens the growth graph's two-column controls in a wider column, and the gel's slightly wider
            (figures::Subject::Biology, Some("population-growth")) => 474.0,
            (figures::Subject::Biology, Some("gel-electrophoresis")) => 426.0,
            // the physics reference opens the motion graphs' segment blocks in a wider column (416 at 1440)
            (figures::Subject::Physics, Some("motion-graphs" | "motion-map" | "ray-diagram")) => 410.0,
            // and the spring scale and length reading, whose first blocks open, in a wider one still (432)
            (figures::Subject::Physics, Some("spring-scale" | "length-reading")) => 426.0,
            (figures::Subject::Chemistry, Some("titration-curve" | "heating-cooling-curve" | "mass-spectrum")) => 378.0,
            (figures::Subject::Chemistry, Some("photoelectron-spectrum")) => 410.0,
            _ => panel_w(sub),
        };
        // the reference's default column runs 2 px wider than the nominal 424 and its gap 2 px tighter
        let wide_col = sub == figures::Subject::Chemistry && fig_panel >= PANEL_W && ui.available_width() * 0.55 >= fig_panel + 2.0;
        let panel_w = if wide_col { fig_panel + 2.0 } else { fig_panel.min(ui.available_width() * 0.55) };
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = if (sub == figures::Subject::Chemistry && (fig_panel < PANEL_W || wide_col)) || sub == figures::Subject::Biology { COL_GAP - 2.0 } else { COL_GAP };
            // the column keeps its width whatever its rows measure, so the preview card's left edge stays put
            let col = Rect::from_min_size(ui.cursor().min, Vec2::new(panel_w, avail_h));
            ui.advance_cursor_after_rect(col);
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(col));
            inner.set_width(panel_w);
            self.controls(&mut inner, avail_h);
            let w = ui.available_width();
            if (math || phys) && ui.ctx().content_rect().width() >= 880.0 {
                // the math (and physics) reference's preview card reaches 2 px past the controls column's top and left
                // and 1 px past its bottom and right edge
                let at = ui.cursor().min;
                ui.advance_cursor_after_rect(Rect::from_min_size(at, Vec2::new(w, avail_h)));
                let mut card = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(at + Vec2::new(-2.0, -2.0), Vec2::new(w + 3.0, avail_h + 3.0))));
                self.canvas_card(&mut card, w + 3.0, avail_h + 3.0, true, preview);
            } else {
                self.canvas_card(ui, w, avail_h, true, preview);
            }
        });
    }
}

/// The biology page's checkbox: an 18 px rounded square, filled blue with a white tick when on,
/// then the option in the page's 600 weight.
pub fn bio_checkbox(ui: &mut Ui, on: &mut bool, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::new(14.4, theme::semibold_family()), theme::ink());
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(16.8 + 8.0 + galley.size().x, 17.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let b = Rect::from_min_size(Pos2::new(rect.left(), rect.center().y - 8.4), Vec2::splat(16.8));
    if *on {
        ui.painter().rect_filled(b, CornerRadius::same(4), theme::accent_col());
        let st = Stroke::new(2.0, Color32::WHITE);
        let c = b.center();
        ui.painter().line_segment([c + Vec2::new(-4.5, 0.5), c + Vec2::new(-1.5, 3.5)], st);
        ui.painter().line_segment([c + Vec2::new(-1.5, 3.5), c + Vec2::new(4.5, -3.5)], st);
    } else {
        ui.painter().rect(b, CornerRadius::same(4), theme::card(), Stroke::new(1.25, Color32::from_rgb(107, 114, 128)), egui::StrokeKind::Inside);
    }
    ui.painter().galley(Pos2::new(b.right() + 8.0, rect.center().y - galley.size().y / 2.0), galley, theme::ink());
    resp
}

/// The physics page's checkbox: a 16.8 px box 4 px in from the edge, its label 11 px on in 14.4 px semibold, a 22.8 px row.
pub fn phys_checkbox(ui: &mut Ui, on: &mut bool, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::new(14.4, theme::semibold_family()), theme::ink());
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.8), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let b = Rect::from_min_size(Pos2::new(rect.left() + 4.0, rect.center().y - 8.4), Vec2::splat(16.8));
    if *on {
        ui.painter().rect_filled(b, CornerRadius::same(4), theme::accent_col());
        let st = Stroke::new(2.0, Color32::WHITE);
        let c = b.center();
        ui.painter().line_segment([c + Vec2::new(-4.5, 0.5), c + Vec2::new(-1.5, 3.5)], st);
        ui.painter().line_segment([c + Vec2::new(-1.5, 3.5), c + Vec2::new(4.5, -3.5)], st);
    } else {
        ui.painter().rect(b, CornerRadius::same(4), theme::card(), Stroke::new(1.25, Color32::from_rgb(107, 114, 128)), egui::StrokeKind::Inside);
    }
    ui.painter().galley(Pos2::new(b.right() + 11.0, rect.center().y - galley.size().y / 2.0), galley, theme::ink());
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The population page's "Fit the axes" box: a 17 px square 4 px in from the card edge, its label 12 px on.
pub fn fit_checkbox(ui: &mut Ui, on: &mut bool, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::new(14.4, theme::semibold_family()), theme::ink());
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(4.0 + 17.0 + 11.0 + galley.size().x, 28.5), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let b = Rect::from_min_size(Pos2::new(rect.left() + 4.0, rect.center().y - 8.5), Vec2::splat(17.0));
    if *on {
        ui.painter().rect_filled(b, CornerRadius::same(4), theme::accent_col());
        let st = Stroke::new(2.0, Color32::WHITE);
        let c = b.center();
        ui.painter().line_segment([c + Vec2::new(-4.5, 0.5), c + Vec2::new(-1.5, 3.5)], st);
        ui.painter().line_segment([c + Vec2::new(-1.5, 3.5), c + Vec2::new(4.5, -3.5)], st);
    } else {
        ui.painter().rect(b, CornerRadius::same(4), theme::card(), Stroke::new(1.25, Color32::from_rgb(107, 114, 128)), egui::StrokeKind::Inside);
    }
    ui.painter().galley(Pos2::new(b.right() + 11.0, rect.center().y - galley.size().y / 2.0), galley, theme::ink());
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The biology page's "Fit now" button: a 76 x 28.5 px white pill with a pale blue border and bold blue text.
pub fn fit_now_button(ui: &mut Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(76.0, 28.5), Sense::click());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(8), if resp.hovered() { theme::hover_bg() } else { theme::card() }, Stroke::new(1.0, pill_border()), egui::StrokeKind::Inside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, "Fit now", egui::FontId::new(13.76, theme::bold_family()), blue_text());
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The reference's segmented control: a grey track of equal cells, the chosen one a white chip
/// with blue text and a hairline shadow.
pub fn segmented_track(ui: &mut Ui, opts: &[&'static str], cur: &mut String) {
    let w = ui.available_width();
    let n = opts.len().max(1) as f32;
    // the chemistry page's switch is a 39 px track with 3 px padding; the biology and math ones are 58 px
    // the biology reference's switches match the chemistry ones: 39.4 px track, 3 px padding, 600 weight
    let chem = chem_page(ui.ctx()) || ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false);
    let bio = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bio_skin_page"))).unwrap_or(false);
    // the physics motion-graph blocks' Forward / Back switch is the slim 36 px track
    let slim = ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::new("seg_slim"))).unwrap_or(false);
    let compact = ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::new("seg_compact"))).unwrap_or(false);
    // the physics page's switches are the same 39 px track as the chemistry ones
    let chem = chem || compact || ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false);
    let (pad, cell_h, trk_r, cell_r) = if slim { (3.0, 30.0, 11, 8) } else if chem { (3.0, if bio { 33.4 } else { 33.0 }, 11, 8) } else { (4.0, 50.0, 12, 8) };
    let cell_w0 = (w - 2.0 * pad) / n;
    // a label that wraps makes the reference's cells (and so the track) grow with it: 13 px of padding around the text lines
    let cell_h = if chem && !slim {
        let fam = theme::semibold_family();
        let lines_h = opts
            .iter()
            .map(|o| ui.painter().layout(o.to_string(), egui::FontId::new(13.76, fam.clone()), Color32::BLACK, cell_w0 - 16.0).size().y)
            .fold(0.0_f32, f32::max);
        f32::max(cell_h, lines_h + if lines_h > 25.0 { 15.0 } else { 13.0 })
    } else {
        cell_h
    };
    let icons = ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::new("seg_icons"))).unwrap_or(false);
    let lucide_icons = ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::new("seg_lucide"))).unwrap_or(false)
        || (opts.len() == 3 && opts[0] == "Text" && opts[1] == "Blank line" && opts[2] == "None" && ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("phys_skin_page"))).unwrap_or(false));
    let wrapped = chem && !slim && cell_h > 40.0;
    let (track, track_resp) = ui.allocate_exact_size(Vec2::new(w, cell_h + 2.0 * pad), Sense::hover());
    // a wrapped label's track in the reference is followed by 2 px more than a one-line one
    if wrapped {
        ui.add_space(2.0);
    }
    let wash = if theme::is_dark() { Color32::from_rgb(0x11, 0x16, 0x21) } else if chem { Color32::from_rgb(0xf6, 0xf7, 0xfb) } else { Color32::from_rgb(0xf0, 0xf1, 0xf6) };
    ui.painter().rect_filled(track, CornerRadius::same(trk_r), wash);
    let cell_w = (track.width() - 2.0 * pad) / n;
    for (i, o) in opts.iter().enumerate() {
        let cell = Rect::from_min_size(Pos2::new(track.left() + pad + i as f32 * cell_w, track.top() + pad), Vec2::new(cell_w, cell_h));
        let resp = ui.interact(cell, track_resp.id.with(i), Sense::click());
        let sel = cur == o;
        if sel {
            let edge = if chem && !theme::is_dark() { Color32::from_rgb(0xee, 0xef, 0xf4) } else { theme::border() };
            ui.painter().rect(cell, CornerRadius::same(cell_r), theme::card(), Stroke::new(1.0, edge), egui::StrokeKind::Inside);
        }
        let col = if sel { blue_text() } else { theme::muted_col() };
        let g = ui.painter().layout(o.to_string(), egui::FontId::new(13.76, if chem { theme::semibold_family() } else { theme::bold_family() }), col, cell_w - 16.0);
        let mut pos = cell.center() - g.size() / 2.0 - if chem && !wrapped { Vec2::new(0.0, 0.6) } else { Vec2::ZERO };
        // wrapped lines are centred on each other, as the reference's text-align centre sets them, and sit a little lower
        let g = if wrapped && g.rows.len() > 1 {
            let mut job = egui::text::LayoutJob::single_section(o.to_string(), egui::TextFormat::simple(egui::FontId::new(13.76, theme::semibold_family()), col));
            job.wrap.max_width = cell_w - 16.0;
            job.halign = Align::Center;
            let g2 = ui.painter().layout_job(job);
            pos = Pos2::new(cell.center().x - g2.rect.center().x, pos.y + 0.4);
            g2
        } else {
            g
        };
        // the chart-title switch draws a glyph before its words (a T, a crossed circle)
        if icons {
            let glyph = if *o == "Text" { ph::TEXT_T } else { ph::PROHIBIT };
            let ig = ui.painter().layout_no_wrap(glyph.to_string(), theme::icon_font(15.0), col);
            let tot = ig.size().x + 6.0 + g.size().x;
            let x0 = cell.center().x - tot / 2.0;
            ui.painter().galley(Pos2::new(x0, cell.center().y - ig.size().y / 2.0), ig.clone(), col);
            pos.x = x0 + ig.size().x + 6.0;
        }
        if lucide_icons {
            // the physics label switch: Lucide T, pencil and slashed circle, 15 px, 6 px before the words
            let ops = match *o { "Text" => lucide::PH_TYPE, "Blank line" => lucide::PENCIL_LINE, _ => lucide::BAN };
            let tot = 15.0 + 6.0 + g.size().x;
            let x0 = cell.center().x - tot / 2.0;
            lucide::paint(ui.painter(), Rect::from_min_size(Pos2::new(x0, cell.center().y - 7.5), Vec2::splat(15.0)), col, ops);
            pos.x = x0 + 21.0;
        }
        ui.painter().galley(pos, g, col);
        if resp.clicked() {
            *cur = o.to_string();
        }
    }
}

impl Workspace {
    /// The Chemistry tab of the figure list wears the chemistry site's home page.
    pub(super) fn chem_list(&self) -> bool {
        SITE_SKINS && !self.modal && self.sel.is_none() && self.imp.is_none() && self.subject.is_some_and(skinned)
    }

    /// The reference's home page: a centred 888 px column with the hero, a search box and a
    /// three-column grid of figure cards (thumbnail over a hairline, title, one-line blurb).
    pub(super) fn chem_browser(&mut self, ui: &mut Ui) -> Act {
        let ctx = ui.ctx().clone();
        let mut picked = None;
        let subj = self.skin_subject();
        let bio = subj == figures::Subject::Biology;
        let math = subj == figures::Subject::Math;
        let phys = subj == figures::Subject::Physics;
        let q = self.search.trim().to_lowercase();
        let mut list: Vec<(usize, &figures::FigureDef)> = super::reg()
            .iter()
            .enumerate()
            .filter(|(_, d)| figures::listed_under(d, subj))
            .filter(|(_, d)| q.is_empty() || d.title.to_lowercase().contains(&q) || d.blurb.to_lowercase().contains(&q) || d.id.contains(&q))
            .collect();
        list.sort_by_key(|(_, d)| (d.subject != subj, if phys { PHYS_ORDER.iter().position(|x| *x == d.id).unwrap_or(99) } else { 0 }));
        ui.spacing_mut().scroll.fade.strength = 0.0;
        egui::ScrollArea::vertical().id_salt("chem_browser").auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            let avail = ui.available_width();
            let col_w = 888.0_f32.min(avail - if phys && avail < 560.0 { -6.0 } else { 4.0 }).max(200.0);
            let left = ((avail - col_w) / 2.0).max(0.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(if phys && avail < 560.0 { -3.0 } else { left });
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    ui.spacing_mut().item_spacing.y = 8.0;
                    let phone = col_w < 560.0;
                    ui.add_space(if phone && !phys { 6.0 } else if phone { 15.5 } else { 14.0 });
                    ui.label(RichText::new(if phys { "Physics figures for your tests and worksheets" } else if math { "Math figures for your tests and worksheets" } else if bio { "Biology figures for your tests and worksheets" } else { "Chemistry figures for your tests and worksheets" }).family(theme::bold_family()).size(if phone && phys { 30.0 } else if phone { 24.0 } else { 32.0 }).extra_letter_spacing(if phone { -0.48 } else { -0.64 }).line_height(Some(if phone && phys { 36.0 } else if phone { 30.0 } else { 39.0 })).color(theme::ink()));
                    ui.add_space(2.0);
                    ui.scope(|ui| {
                        ui.set_max_width(if phone { col_w } else { 650.0 });
                        ui.add(egui::Label::new(RichText::new("Make one to fit your lesson, then copy it straight into your document. Free, with no sign-up.").size(if phone { 15.0 } else { 16.0 }).line_height(Some(24.0)).color(theme::muted_col())));
                    });
                    ui.add_space(16.0);
                    Frame::new()
                        .fill(theme::card())
                        .stroke(Stroke::new(1.0, theme::border()))
                        .corner_radius(CornerRadius::same(12))
                        .inner_margin(Margin::symmetric(14, 0))
                        .show(ui, |ui| {
                            ui.set_width((if phone { col_w } else { 415.0 }) - 30.0);
                            ui.set_height(45.0);
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.label(RichText::new(ph::MAGNIFYING_GLASS).size(18.0).color(theme::muted_col()));
                                ui.add_space(7.0);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.search)
                                        .hint_text(RichText::new(if phys { "Search figures, like “free body diagram”" } else if math { "Search figures, like “triangle”" } else if bio { "Search figures, like “punnett square”" } else { "Search figures, like “graduated cylinder”" }).size(16.0))
                                        .font(egui::FontId::new(16.0, egui::FontFamily::Name("vs-noto".into())))
                                        .frame(Frame::NONE)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                        });
                    ui.add_space(11.0);
                    if self.import_open {
                        self.import_panel(ui);
                    }
                    if self.link_open {
                        panel_card(ui, 12, |ui| {
                            ui.label(theme::strong("Open a shared figure").size(14.0));
                            ui.label(theme::muted("Paste a link made with Share link, Copy link. It opens offline; the link holds the settings.").size(12.5));
                            let w = (ui.available_width() - 140.0).clamp(200.0, 560.0);
                            self.link_box(ui, w);
                        });
                        if self.sel.is_some() {
                            return;
                        }
                    }
                    if list.is_empty() {
                        ui.add_space(20.0);
                        ui.label(theme::muted("No figures match.").size(14.0));
                        return;
                    }
                    let gap = 16.0;
                    let cols = if col_w >= 640.0 { 3 } else if col_w >= 420.0 { 2 } else { 1 };
                    let cw = (col_w - gap * (cols as f32 - 1.0)) / cols as f32;
                    let mut budget = 10u8;
                    for chunk in list.chunks(cols) {
                        // a row's cards share the height of its tallest blurb
                        let h = chunk.iter().map(|(_, d)| card_height(ui, d.blurb, cw)).fold(0.0_f32, f32::max);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for (i, def) in chunk {
                                if chem_card(ui, &ctx, def, *i, &mut budget, cw, h, phys) {
                                    picked = Some(*i);
                                }
                            }
                        });
                        ui.add_space(gap - 8.0);
                    }
                    ui.add_space(14.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 16.0;
                        if ui.add(egui::Button::new(theme::muted("Open from a link").size(13.0)).frame(false)).clicked() {
                            self.link_open = !self.link_open;
                        }
                        if ui.add(egui::Button::new(theme::muted("Import SVG/PNG").size(13.0)).frame(false)).clicked() {
                            self.import_open = !self.import_open;
                        }
                    });
                    ui.add_space(30.0);
                });
            });
        });
        if let Some(i) = picked {
            self.select(i);
        }
        Act::None
    }
}

const THUMB_H: f32 = 152.0;

/// The physics site's card order.
const PHYS_ORDER: [&str; 13] = ["free-body-diagram", "vector-diagram", "inclined-plane", "pulley", "projectile-motion", "motion-graphs", "motion-map", "spring-scale", "length-reading", "waves", "circuit-diagram", "ray-diagram", "coordinate-grid"];

/// The physics site's card title: "X Generator" (its names for three differ from ours).
fn phys_card_title(def: &figures::FigureDef) -> String {
    match def.id {
        "motion-graphs" => "Motion Graph Generator".into(),
        "waves" => "Wave Generator".into(),
        "temperature-reading" => def.title.into(),
        _ if PHYS_ORDER.contains(&def.id) => format!("{} Generator", def.title),
        _ => def.title.into(),
    }
}

fn card_height(ui: &Ui, blurb: &str, cw: f32) -> f32 {
    let g = ui.painter().layout_job(blurb_job(blurb, cw - 22.0));
    THUMB_H + 1.0 + 10.0 + 22.0 + 2.0 + g.size().y + 14.0
}

/// The card blurb: 14 px Noto at the reference's 18 px line height.
fn blurb_job(blurb: &str, wrap: f32) -> egui::text::LayoutJob {
    let mut fmt = egui::TextFormat::simple(egui::FontId::new(14.0, egui::FontFamily::Name("vs-noto".into())), theme::muted_col());
    fmt.line_height = Some(18.0);
    let mut job = egui::text::LayoutJob::single_section(blurb.to_string(), fmt);
    job.wrap.max_width = wrap;
    job
}

#[derive(Clone, Copy, PartialEq)]
enum Tiles {
    Row,
    Diagonal,
}

type Over = Vec<(&'static str, figures::Value)>;

/// The reference's split-panel thumbnails: several variants of one figure side by side, or split
/// along a diagonal. `None` = one plain figure.
fn card_tiles(id: &str) -> Option<(Tiles, Vec<Over>)> {
    use figures::Value::S;
    let s = |k: &'static str, v: &str| (k, S(v.to_string()));
    Some(match id {
        "volume-reading" => (Tiles::Row, vec![vec![s("instrument", "Graduated cylinder")], vec![s("instrument", "Buret")], vec![s("instrument", "Beaker")]]),
        "mass-reading" => (Tiles::Diagonal, vec![vec![s("balance", "Triple beam balance")], vec![s("balance", "Digital balance")]]),
        "temperature-reading" => (Tiles::Row, vec![vec![s("thermometer", "Liquid-in-glass")], vec![s("thermometer", "Digital probe")]]),
        "ph-reading" => (Tiles::Row, vec![vec![s("instrument", "Digital meter")], vec![s("instrument", "Analog meter")], vec![s("instrument", "pH paper")]]),
        "particle-diagram" => (Tiles::Diagonal, vec![vec![s("layout", "In a box")], vec![s("layout", "Lattice")]]),
        "lewis-structures" => (Tiles::Row, vec![vec![s("formula", "H2O"), s("shape", "Shaped")], vec![s("preset", "HNO\u{2083}, nitric acid")]]),
        _ => return None,
    })
}

/// The thumbnail's params: the figure's plain defaults (no magnifier inset) plus the overrides
/// that make the reference's first screen.
fn thumb_params(def: &figures::FigureDef, over: &Over) -> figures::Params {
    let mut p = figures::start_params(def);
    for k in ["mag_show", "magnify"] {
        if p.0.contains_key(k) {
            let defs = (def.params)();
            if let Some(d) = defs.iter().find(|d| d.key == k) {
                p.0.insert(k.to_string(), d.default.clone());
            }
        }
    }
    for (k, v) in over {
        p.0.insert(k.to_string(), v.clone());
    }
    p
}

fn paint_fit(p: &egui::Painter, ctx: &egui::Context, def: &figures::FigureDef, over: &Over, budget: &mut u8, area: Rect) {
    if let Some((t, ppp)) = super::thumb_fit(ctx, def, &thumb_params(def, over), area.width() - 16.0, area.height() - 20.0, budget) {
        let sz = t.size_vec2() / ppp;
        p.image(t.id(), Rect::from_center_size(area.center(), sz), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
}

/// One figure card of the chemistry home: thumbnail, hairline, bold title, muted blurb.
fn chem_card(ui: &mut Ui, ctx: &egui::Context, def: &figures::FigureDef, idx: usize, budget: &mut u8, cw: f32, h: f32, phys: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(cw, h), Sense::click());
    let p = ui.painter();
    let radius = CornerRadius::same(12);
    p.rect(rect, radius, theme::card(), Stroke::new(1.0, if resp.hovered() { theme::accent_col() } else { theme::border() }), egui::StrokeKind::Inside);
    let thumb = Rect::from_min_size(rect.min + Vec2::new(1.0, 1.0), Vec2::new(cw - 2.0, THUMB_H));
    let line = Stroke::new(1.0, theme::border());
    match card_tiles(def.id) {
        Some((Tiles::Row, tiles)) => {
            let n = tiles.len() as f32;
            let w = thumb.width() / n;
            for (i, over) in tiles.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(thumb.left() + w * i as f32, thumb.top()), Vec2::new(w, THUMB_H));
                paint_fit(p, ctx, def, over, budget, r);
                if i > 0 {
                    p.vline(r.left(), thumb.y_range(), line);
                }
            }
        }
        Some((Tiles::Diagonal, tiles)) => {
            let (w, h) = (thumb.width(), THUMB_H);
            paint_fit(p, ctx, def, &tiles[0], budget, Rect::from_min_size(thumb.min, Vec2::new(w * 0.78, h * 0.62)));
            paint_fit(p, ctx, def, &tiles[1], budget, Rect::from_min_size(thumb.min + Vec2::new(w * 0.22, h * 0.38), Vec2::new(w * 0.78, h * 0.62)));
            p.line_segment([Pos2::new(thumb.left(), thumb.bottom()), Pos2::new(thumb.right(), thumb.top())], line);
        }
        None => {
            // the reference's titration card marks the half-equivalence point too
            let over: Over = match def.id {
                "titration-curve" => vec![("half", figures::Value::B(true))],
                // the physics site's first two cards: a block with four forces, three vectors and their resultant
                "free-body-diagram" if phys => {
                    use figures::Value::{F, I, S};
                    vec![("body", S("Block".into())), ("n", I(4)), ("f3_ang", F(180.0)), ("f4_ang", F(0.0)), ("f3_lab", S("F_f".into())), ("f4_lab", S("F_A".into()))]
                }
                "vector-diagram" if phys => {
                    use figures::Value::{F, I};
                    vec![("n", I(3)), ("v1_mag", F(4.0)), ("v1_ang", F(15.0)), ("v2_mag", F(3.0)), ("v2_ang", F(80.0)), ("v3_mag", F(2.5)), ("v3_ang", F(145.0))]
                }
                _ => Vec::new(),
            };
            paint_fit(p, ctx, def, &over, budget, thumb)
        }
    }
    p.hline(rect.x_range().shrink(1.0), thumb.bottom(), line);
    let x = rect.left() + 15.0;
    let y = thumb.bottom() + 9.0;
    let title = if phys { phys_card_title(def) } else { def.title.to_string() };
    let t = p.layout_no_wrap(title, egui::FontId::new(16.0, theme::bold_family()), theme::ink());
    let ty = t.size().y;
    p.galley(Pos2::new(x, y), t, theme::ink());
    let g = p.layout_job(blurb_job(def.blurb, cw - 22.0));
    p.galley(Pos2::new(x, y + ty + 2.0), g, theme::muted_col());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let _ = idx;
    resp.clicked()
}
