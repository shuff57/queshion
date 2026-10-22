//! Canvas editor for the composite-shape figure (mathfigures.com/composite-shape): a dotted grid
//! with snap, pan / zoom, pieces drawn with the real renderer at the canvas' own scale, and tools
//! to select, move, resize, delete, duplicate and draw pieces (rectangle, triangle, semicircle,
//! quarter circle, circle, free polygon), height lines and side / corner labels.
//!
//! The canvas owns no shape state: every frame it reads the `spec` param through
//! `figures::composite::parse` and writes edits back with `to_spec`, so the typed Pieces card,
//! the "Start with" chips, presets, undo / redo, the render and the PDF export all stay on the
//! one text the figure always had. Geometry and spec editing live in `figures::composite`
//! (unit tested there); this file is input handling and drawing.
use std::hash::{Hash, Hasher};

use egui::{Align2, Button, Color32, CornerRadius, FontId, Frame, Id, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, TextEdit, TextureHandle, Ui, pos2, vec2};

use crate::figures::composite::{self as cs, Grip, Item, NewKind, Pt, Shape, Side};
use crate::figures::math::{CompositeView, COMPOSITE_CUSTOM, COMPOSITE_STARTS, composite_label_boxes, composite_spec, composite_view_svg, preset_spec};
use crate::figures::Params;
use crate::theme;

/// Screen points per model unit at "100%".
const UNIT_PX: f64 = 40.0;
/// Turn slider length (points for the full -180..180 range: 2.25° a point; it was 130).
const TURN_SLIDER_W: f32 = 160.0;
/// Bottom band kept free of popovers: the zoom bar, and in the Label tool the Turn bar above it.
const POPOVER_BAND: f32 = 54.0;
const POPOVER_BAND_LABEL: f32 = 106.0;
/// Degrees per point when scrubbing the Turn number box (the reference's knob: 0.5).
const TURN_SCRUB: f64 = 0.5;
const MIN_S: f64 = 3.0;
const MAX_S: f64 = 600.0;
/// Snap steps offered (model units); 0 = off.
const SNAPS: [(f64, &str); 4] = [(0.5, "\u{bd}"), (1.0, "1"), (0.25, "\u{bc}"), (0.0, "Off")];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Select,
    Rect,
    Tri,
    Semi,
    Quarter,
    Circle,
    Poly,
    Line,
    Label,
}

impl Tool {
    const ALL: [Tool; 9] = [Tool::Select, Tool::Rect, Tool::Tri, Tool::Semi, Tool::Quarter, Tool::Circle, Tool::Poly, Tool::Line, Tool::Label];
    fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Rect => "Rectangle",
            Tool::Tri => "Triangle",
            Tool::Semi => "Semicircle",
            Tool::Quarter => "Quarter",
            Tool::Circle => "Circle",
            Tool::Poly => "Polygon",
            Tool::Line => "Line",
            Tool::Label => "Label",
        }
    }
    fn tip(self) -> &'static str {
        match self {
            Tool::Select => "Select, move and resize pieces; drag empty space to pan (Esc)",
            Tool::Rect => "Drag out a rectangle, or click for a 4 by 3 one",
            Tool::Tri => "Drag out a triangle (base where you start, tip where you let go)",
            Tool::Semi => "Drag from the centre the way it should bulge",
            Tool::Quarter => "Drag from the corner into the quarter it should fill",
            Tool::Circle => "Drag from the centre out to the radius",
            Tool::Poly => "Click corners; click the first corner, double-click or press Enter to close",
            Tool::Line => "Drag a height or other dashed line between the markers: corners and meeting points (circles), midpoints (diamonds)",
            Tool::Label => "Click a side, an angle (inside or outside a corner) or a corner to label and mark it; Turn turns the figure",
        }
    }
    fn new_kind(self) -> Option<NewKind> {
        Some(match self {
            Tool::Rect => NewKind::Rect,
            Tool::Tri => NewKind::Tri,
            Tool::Semi => NewKind::Semi,
            Tool::Quarter => NewKind::Quarter,
            Tool::Circle => NewKind::Circle,
            _ => return None,
        })
    }
}

/// What a frame did to the params: nothing, a continuous edit (drag / typing, settled into one
/// undo step by the workspace's history), or a discrete step to record now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    None,
    Live,
    Step,
}

/// What a Label-tool click lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LabelHit {
    Corner(Pt),
    Side(Side),
    /// Corner, exterior angle?
    Angle(Pt, bool),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct View {
    /// Screen points per model unit.
    s: f64,
    /// Model point at the canvas centre.
    c: Pt,
}

enum Drag {
    Pan,
    Move { i: usize, from: Pt, anchor: Pt, orig: Vec<Item> },
    Grip { i: usize, g: Grip, orig: Vec<Item> },
    Create { kind: NewKind, a: Pt },
    Line { a: Pt },
    /// A label being dragged: its row, where the press began, its offset then, the spec then.
    Label { i: usize, at: Pos2, from: Pt, orig: Vec<Item>, moved: bool },
}

pub struct Canvas {
    view: Option<View>,
    pub sel: Option<usize>,
    pub tool: Tool,
    drag: Option<Drag>,
    /// Corners of the polygon being drawn.
    pub poly: Vec<Pt>,
    /// Label tool: the side whose popover is open, and its text buffer.
    pub side: Option<Side>,
    side_text: String,
    /// "+" menu: the side to add a piece on, and where the menu opens.
    add: Option<(Side, Pos2)>,
    /// Label tool: the angle whose popover is open (corner, exterior?) and its text buffer.
    pub angle: Option<(Pt, bool)>,
    angle_text: String,
    /// Label tool: the corner whose popover is open and its name buffer.
    pub corner: Option<Pt>,
    corner_text: String,
    /// The figure's Turn as the canvas shows it (degrees, centre): only the Label tool shows the
    /// figure turned, as the reference does; the drawing tools edit it straight.
    turn: (f64, Pt),
    snap_i: usize,
    tex: Option<(u64, TextureHandle)>,
    /// The spec the canvas last saw or wrote (an outside change may refit the view).
    last_spec: String,
    qa_done: bool,
    /// Canvas rectangle of the last frame (tests map model points to the screen with it).
    pub rect: Rect,
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas {
            view: None,
            sel: None,
            tool: Tool::Select,
            drag: None,
            poly: Vec::new(),
            side: None,
            side_text: String::new(),
            add: None,
            angle: None,
            angle_text: String::new(),
            corner: None,
            corner_text: String::new(),
            turn: (0.0, (0.0, 0.0)),
            snap_i: 0,
            tex: None,
            last_spec: String::new(),
            qa_done: false,
            rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
        }
    }
}

impl Canvas {
    fn snap_step(&self) -> f64 {
        SNAPS[self.snap_i % SNAPS.len()].0
    }

    /// Screen position of a point of the (turned) view space.
    fn vmap(&self, q: Pt) -> Pos2 {
        let v = self.view.unwrap_or(View { s: UNIT_PX, c: (0.0, 0.0) });
        let c = self.rect.center();
        pos2(c.x + ((q.0 - v.c.0) * v.s) as f32, c.y - ((q.1 - v.c.1) * v.s) as f32)
    }
    fn vunmap(&self, p: Pos2) -> Pt {
        let v = self.view.unwrap_or(View { s: UNIT_PX, c: (0.0, 0.0) });
        let c = self.rect.center();
        (v.c.0 + (p.x - c.x) as f64 / v.s, v.c.1 - (p.y - c.y) as f64 / v.s)
    }
    /// Screen position of model point `q` (last frame's view, turned with the figure in the
    /// Label tool).
    pub fn to_screen(&self, q: Pt) -> Pos2 {
        self.vmap(cs::rot(q, self.turn.1, self.turn.0))
    }
    fn to_model(&self, p: Pos2) -> Pt {
        cs::rot(self.vunmap(p), self.turn.1, -self.turn.0)
    }
    /// Screen direction of model direction `u` at model point `q`.
    fn sdir(&self, q: Pt, u: Pt) -> egui::Vec2 {
        (self.to_screen((q.0 + u.0, q.1 + u.1)) - self.to_screen(q)).normalized()
    }
    fn scale(&self) -> f64 {
        self.view.map(|v| v.s).unwrap_or(UNIT_PX)
    }

    /// The canvas view as the renderer takes it (and the params it draws: the Turn shows only
    /// in the Label tool).
    fn render_view(&self, p: &Params) -> (CompositeView, Params) {
        let r = self.rect;
        let v = self.view.unwrap_or(View { s: UNIT_PX, c: (0.0, 0.0) });
        let (w, h) = (r.width() as f64, r.height() as f64);
        let cv = CompositeView { w, h, s: v.s, ox: w / 2.0 - v.c.0 * v.s, oy: h / 2.0 + v.c.1 * v.s };
        let mut pr = p.clone();
        if self.tool != Tool::Label {
            pr.set_f("rotate", 0.0);
        }
        (cv, pr)
    }

    /// The draggable label (spec row) under screen point `pp`, if any: dimension, angle and
    /// point labels and text marks, as the renderer placed them.
    fn label_at(&self, p: &Params, pp: Pos2) -> Option<usize> {
        if !matches!(self.tool, Tool::Select | Tool::Label) {
            return None;
        }
        let (cv, pr) = self.render_view(p);
        let r = self.rect;
        composite_label_boxes(&pr, cv)
            .into_iter()
            .rev()
            .find(|b| ((pp.x - r.min.x) as f64 - b.c.0).abs() <= b.half.0 && ((pp.y - r.min.y) as f64 - b.c.1).abs() <= b.half.1)
            .map(|b| b.item)
    }

    /// Where each draggable label sits on screen, by spec row (tests aim pointer events at them).
    #[cfg(test)]
    pub fn label_centres(&self, p: &Params) -> Vec<(usize, Pos2)> {
        let (cv, pr) = self.render_view(p);
        composite_label_boxes(&pr, cv).into_iter().map(|b| (b.item, self.rect.min + vec2(b.c.0 as f32, b.c.1 as f32))).collect()
    }

    fn fit(&mut self, items: &[Item]) {
        let r = self.rect;
        let pts: Vec<Pt> = cs::all_points(items).into_iter().map(|q| cs::rot(q, self.turn.1, self.turn.0)).collect();
        let Some((x0, y0, x1, y1)) = cs::bbox(&pts) else {
            self.view = Some(View { s: UNIT_PX, c: (4.0, 3.0) });
            return;
        };
        let (mw, mh) = ((r.width() as f64 - 150.0).max(60.0), (r.height() as f64 - 130.0).max(60.0));
        let s = (mw / (x1 - x0).max(1.0)).min(mh / (y1 - y0).max(1.0)).clamp(MIN_S, MAX_S);
        self.view = Some(View { s, c: ((x0 + x1) / 2.0, (y0 + y1) / 2.0) });
    }

    fn zoom_about(&mut self, p: Pos2, f: f64) {
        let Some(v) = self.view else { return };
        let m0 = self.vunmap(p);
        let s = (v.s * f).clamp(MIN_S, MAX_S);
        let c = self.rect.center();
        self.view = Some(View { s, c: (m0.0 - (p.x - c.x) as f64 / s, m0.1 + (p.y - c.y) as f64 / s) });
    }

    /// Grid snap, or a corner / midpoint of a piece when one is within reach.
    fn snap_q(&self, items: &[Item], q: Pt, skip: Option<usize>) -> Pt {
        let reach = 9.0 / self.scale();
        let others: Vec<Item> = items.iter().enumerate().filter(|(j, _)| Some(*j) != skip).map(|(_, it)| it.clone()).collect();
        if let Some(t) = cs::snap_targets(&others).into_iter().filter(|t| cs::dist(*t, q) < reach).min_by(|a, b| cs::dist(*a, q).total_cmp(&cs::dist(*b, q))) {
            return t;
        }
        cs::snap_pt(q, self.snap_step())
    }

    fn write(&mut self, p: &mut Params, items: &[Item]) {
        let spec = cs::to_spec(items);
        p.set_s("spec", &spec);
        p.set_s("preset", COMPOSITE_CUSTOM);
        self.last_spec = spec;
    }

    /// QA hook: `VERSHION_COMP="sel:0,tool:label,side:2,add:1,poly:0 0 4 0 4 3"` sets up the canvas
    /// for a screenshot (selection, tool, an open side / add menu, a polygon in progress).
    fn qa(&mut self, items: &[Item]) {
        if self.qa_done {
            return;
        }
        self.qa_done = true;
        let Ok(cmds) = crate::env_var("VERSHION_COMP") else { return };
        for c in cmds.split(',') {
            let (k, v) = c.split_once(':').unwrap_or((c, ""));
            let v = v.trim();
            match k.trim() {
                "sel" => self.sel = v.parse().ok(),
                "tool" => self.tool = Tool::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(v)).unwrap_or_default(),
                "side" => {
                    let sides = cs::outer_sides(items);
                    self.side = v.parse::<usize>().ok().and_then(|i| sides.get(i).copied());
                    if let Some(s) = self.side {
                        self.side_text = cs::side_label(items, s.a, s.b).unwrap_or_default();
                    }
                }
                "add" => {
                    let sel = self.sel;
                    let sides: Vec<Side> = cs::outer_sides(items).into_iter().filter(|s| Some(s.piece) == sel).collect();
                    if let Some(s) = v.parse::<usize>().ok().and_then(|i| sides.get(i).copied()) {
                        self.add = Some((s, self.plus_pos(&s)));
                    }
                }
                "poly" => {
                    let n: Vec<f64> = v.split_whitespace().filter_map(|t| t.parse().ok()).collect();
                    self.poly = n.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect();
                }
                "angle" | "ext" => {
                    let corners = cs::outer_corners(items);
                    if let Some(v) = v.parse::<usize>().ok().and_then(|i| corners.get(i).copied()) {
                        self.open_angle(items, v, k.trim() == "ext");
                    }
                }
                "corner" => {
                    let corners = cs::outer_corners(items);
                    if let Some(v) = v.parse::<usize>().ok().and_then(|i| corners.get(i).copied()) {
                        self.open_corner(items, v);
                    }
                }
                "zoom" => {
                    if let (Some(f), Some(vw)) = (v.parse::<f64>().ok(), self.view.as_mut()) {
                        vw.s = (UNIT_PX * f / 100.0).clamp(MIN_S, MAX_S);
                    }
                }
                _ => {}
            }
        }
    }

    fn plus_pos(&self, s: &Side) -> Pos2 {
        self.to_screen(s.mid()) + self.sdir(s.mid(), s.out) * 20.0
    }

    /// `pivot_for(d)`, flipped to the other side when a popover of `size` would not fit there
    /// (so it does not get pushed back over the part it is for).
    fn pivot_in(&self, at: Pos2, d: egui::Vec2, size: egui::Vec2) -> Align2 {
        let r = self.popover_room();
        match pivot_for(d) {
            Align2::RIGHT_CENTER if at.x - size.x < r.left() => Align2::LEFT_CENTER,
            Align2::LEFT_CENTER if at.x + size.x > r.right() => Align2::RIGHT_CENTER,
            Align2::CENTER_TOP if at.y + size.y > r.bottom() => Align2::CENTER_BOTTOM,
            Align2::CENTER_BOTTOM if at.y - size.y < r.top() => Align2::CENTER_TOP,
            p => p,
        }
    }

    /// Where popovers may sit: the canvas minus the bottom-right bars (zoom, and Turn in the
    /// Label tool), so a popover near the bottom never covers them.
    fn popover_room(&self) -> Rect {
        let band = if self.tool == Tool::Label { POPOVER_BAND_LABEL } else { POPOVER_BAND };
        let r = self.rect;
        Rect::from_min_max(r.min, pos2(r.max.x, (r.max.y - band).max(r.min.y + 40.0)))
    }

    fn open_angle(&mut self, items: &[Item], v: Pt, ext: bool) {
        self.angle_text = cs::angle_mark(items, v, ext).text;
        self.angle = Some((v, ext));
    }

    fn open_corner(&mut self, items: &[Item], v: Pt) {
        self.corner_text = cs::corner_point(items, v).unwrap_or_default();
        self.corner = Some(v);
    }

    fn close_popovers(&mut self) {
        self.side = None;
        self.add = None;
        self.angle = None;
        self.corner = None;
    }

    /// The Label tool's target under screen point `pp`: a corner, a side, or an angle.
    fn label_target(&self, items: &[Item], pp: Pos2) -> Option<LabelHit> {
        let m = self.to_model(pp);
        let corners = cs::outer_corners(items);
        if let Some(v) = corners.iter().copied().filter(|v| self.to_screen(*v).distance(pp) < 9.0).min_by(|a, b| self.to_screen(*a).distance(pp).total_cmp(&self.to_screen(*b).distance(pp))) {
            return Some(LabelHit::Corner(v));
        }
        let side = cs::outer_sides(items).into_iter().map(|s| (cs::seg_dist(m, s.a, s.b), s)).filter(|(d, _)| *d < 7.0 / self.scale()).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, s)) = side {
            return Some(LabelHit::Side(s));
        }
        let mut near: Vec<Pt> = corners.into_iter().filter(|v| self.to_screen(*v).distance(pp) < 48.0).collect();
        near.sort_by(|a, b| self.to_screen(*a).distance(pp).total_cmp(&self.to_screen(*b).distance(pp)));
        for v in near {
            let l = cs::dist(v, m).max(1e-12);
            let u = ((m.0 - v.0) / l, (m.1 - v.1) / l);
            for ext in [false, true] {
                if let Some(g) = cs::angle_geo(items, v, ext)
                    && cs::in_sweep(&g, u)
                {
                    return Some(LabelHit::Angle(v, ext));
                }
            }
        }
        None
    }

    /// The canvas: toolbar, then the drawing area. Edits go straight into `p`.
    pub fn show(&mut self, ui: &mut Ui, p: &mut Params) -> Change {
        let mut change = Change::None;
        let spec = composite_spec(p);
        let mut items = cs::parse(&spec);
        let turn0 = self.turn;
        self.turn = if self.tool == Tool::Label { (p.f("rotate"), cs::turn_centre(&items)) } else { (0.0, (0.0, 0.0)) };

        self.toolbar(ui, p, &mut items, &mut change);
        ui.add_space(4.0);

        let (rect, resp) = ui.allocate_exact_size(ui.available_size().max(vec2(120.0, 120.0)), Sense::click_and_drag());
        self.rect = rect;
        // the spec changed outside the canvas (typed rows, chips, undo): keep the view unless
        // the shape left it
        if self.view.is_none() {
            self.fit(&items);
        } else if (spec != self.last_spec || self.turn != turn0) && self.drag.is_none() {
            let pts: Vec<Pt> = cs::all_points(&items).into_iter().map(|q| cs::rot(q, self.turn.1, self.turn.0)).collect();
            if let Some((x0, y0, x1, y1)) = cs::bbox(&pts) {
                let (a, b) = (self.vmap((x0, y1)), self.vmap((x1, y0)));
                let r = Rect::from_min_max(a, b);
                if !rect.shrink(10.0).contains_rect(r) || r.width().max(r.height()) < rect.width().min(rect.height()) * 0.15 {
                    self.fit(&items);
                }
            }
            if self.sel.is_some_and(|i| i >= items.len()) {
                self.sel = None;
            }
        }
        self.last_spec = spec.clone();
        self.qa(&items);

        let ctx = ui.ctx().clone();
        let hover = resp.hover_pos();

        // zoom: wheel / pinch about the pointer
        if let Some(hp) = hover {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let f = zoom as f64 * (scroll as f64 * 0.0018).exp();
            if (f - 1.0).abs() > 1e-4 {
                self.zoom_about(hp, f);
            }
        }

        // ---- pointer ----
        let (pressed, released, down, ptr, sec_or_mid) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.primary_down(),
                i.pointer.interact_pos(),
                i.pointer.secondary_pressed() || i.pointer.button_pressed(egui::PointerButton::Middle),
            )
        });
        if sec_or_mid && resp.hovered() {
            self.drag = Some(Drag::Pan);
        }
        if pressed && resp.hovered()
            && let Some(pp) = ptr
        {
            self.close_popovers();
            self.press(p, &mut items, pp, &mut change);
        }
        // pan follows the pointer delta (any button)
        if matches!(self.drag, Some(Drag::Pan)) {
            let d = ui.input(|i| i.pointer.delta());
            if let Some(v) = self.view.as_mut() {
                v.c.0 -= d.x as f64 / v.s;
                v.c.1 += d.y as f64 / v.s;
            }
            if !ui.input(|i| i.pointer.any_down()) {
                self.drag = None;
            }
        } else if let Some(pp) = ptr.filter(|_| self.drag.is_some()) {
            let m = self.to_model(pp);
            let drag = self.drag.take();
            match &drag {
                Some(Drag::Move { i, from, anchor, orig }) => {
                    let raw = (anchor.0 + m.0 - from.0, anchor.1 + m.1 - from.1);
                    let to = cs::snap_pt(raw, self.snap_step());
                    let d = (to.0 - anchor.0, to.1 - anchor.1);
                    let mut next = orig.clone();
                    cs::move_item(&mut next, *i, d);
                    if cs::to_spec(&next) != cs::to_spec(&items) {
                        items = next;
                        self.write(p, &items);
                        change = Change::Live;
                    }
                }
                Some(Drag::Label { i, at, from, orig, moved }) => {
                    let d = pp - *at;
                    let s = self.scale();
                    if *moved || d.length() > 3.0 {
                        let mut next = orig.clone();
                        // the pointer moves on the turned figure: store the move unturned
                        let u = cs::turn_off((d.x as f64 / s, d.y as f64 / s), -self.turn.0);
                        if let Some(it) = next.get_mut(*i) {
                            it.set_off(Some((from.0 + u.0, from.1 + u.1)));
                        }
                        if cs::to_spec(&next) != cs::to_spec(&items) {
                            items = next;
                            self.write(p, &items);
                            change = Change::Live;
                        }
                        self.drag = Some(Drag::Label { i: *i, at: *at, from: *from, orig: orig.clone(), moved: true });
                    }
                }
                Some(Drag::Grip { i, g, orig }) => {
                    let q = self.snap_q(orig, m, Some(*i));
                    let mut next = orig.clone();
                    cs::drag_grip_linked(&mut next, *i, *g, q);
                    if cs::to_spec(&next) != cs::to_spec(&items) {
                        items = next;
                        self.write(p, &items);
                        change = Change::Live;
                    }
                }
                _ => {}
            }
            if !matches!((&self.drag, &drag), (Some(Drag::Label { .. }), Some(Drag::Label { .. }))) {
                self.drag = drag;
            }
            if released || !down {
                self.release(p, &mut items, m, &mut change);
            }
        }
        if resp.double_clicked() && self.tool == Tool::Poly {
            self.finish_poly(p, &mut items, &mut change);
        }

        // ---- keys (not while a text box has focus) ----
        if !ctx.egui_wants_keyboard_input() && (resp.hovered() || self.sel.is_some() || !self.poly.is_empty()) {
            self.keys(ui, p, &mut items, &mut change);
        }

        // a label under the pointer can be dragged: say so (the reference's "Drag to move")
        if let Some(hp) = hover.filter(|_| self.drag.is_none() || matches!(self.drag, Some(Drag::Label { .. }))) {
            if matches!(self.drag, Some(Drag::Label { moved: true, .. })) {
                ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if self.label_at(p, hp).is_some() {
                ctx.set_cursor_icon(egui::CursorIcon::Move);
                resp.clone().on_hover_text_at_pointer("Drag to move this label");
            }
        }

        // ---- draw ----
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        // the Label tool shows the figure as it will look: no grid, turned
        if self.tool != Tool::Label {
            self.grid(&painter);
        }
        self.texture(&ctx, p, &painter);
        self.overlay(ui, &painter, &items, hover);

        // ---- popovers ----
        if items.is_empty() && self.poly.is_empty() && self.drag.is_none() {
            self.start_screen(ui, p, &mut items, &mut change);
        }
        self.props(ui, p, &mut items, &mut change);
        self.add_menu(ui, p, &mut items, &mut change);
        self.side_popover(ui, p, &mut items, &mut change);
        self.angle_popover(ui, p, &mut items, &mut change);
        self.corner_popover(ui, p, &mut items, &mut change);
        self.turn_bar(ui, p, &mut change);
        self.zoom_bar(ui, &items);

        if self.drag.is_some() || change != Change::None {
            ctx.request_repaint();
        }
        change
    }

    fn press(&mut self, p: &mut Params, items: &mut Vec<Item>, pp: Pos2, change: &mut Change) {
        let m = self.to_model(pp);
        let tol = 6.0 / self.scale();
        // a label under the pointer is dragged (Select and Label tools), unless a grip of the
        // selection is there; a press that never moves acts as a plain click on release
        let on_grip = self.sel.and_then(|i| items.get(i)).and_then(|it| it.shape.as_ref()).is_some_and(|sh| cs::grips(sh).into_iter().any(|(_, q)| self.to_screen(q).distance(pp) < 9.0));
        if !on_grip && let Some(i) = self.label_at(p, pp) {
            let from = items[i].off.unwrap_or((0.0, 0.0));
            self.drag = Some(Drag::Label { i, at: pp, from, orig: items.clone(), moved: false });
            return;
        }
        self.click(p, items, pp, m, tol, change);
    }

    fn click(&mut self, p: &mut Params, items: &mut Vec<Item>, pp: Pos2, m: Pt, tol: f64, change: &mut Change) {
        match self.tool {
            Tool::Select => {
                // grips of the selection, then its "+" buttons, then whatever is under the pointer
                if let Some(i) = self.sel
                    && let Some(sh) = items.get(i).and_then(|it| it.shape.as_ref())
                {
                    if let Some((g, _)) = cs::grips(sh).into_iter().find(|(_, q)| self.to_screen(*q).distance(pp) < 9.0) {
                        self.drag = Some(Drag::Grip { i, g, orig: items.clone() });
                        return;
                    }
                    if sh.is_piece()
                        && let Some(s) = cs::outer_sides(items).into_iter().filter(|s| s.piece == i && s.len() * self.scale() > 24.0).find(|s| self.plus_pos(s).distance(pp) < 11.0)
                    {
                        self.add = Some((s, self.plus_pos(&s)));
                        return;
                    }
                }
                match cs::hit(items, m, tol) {
                    Some(i) => {
                        self.sel = Some(i);
                        let anchor = items[i].shape.as_ref().map(|s| cs::outline(s).first().copied().or_else(|| cs::mark_points(s).first().copied()).unwrap_or(m)).unwrap_or(m);
                        self.drag = Some(Drag::Move { i, from: m, anchor, orig: items.clone() });
                    }
                    None => {
                        self.sel = None;
                        self.drag = Some(Drag::Pan);
                    }
                }
            }
            Tool::Poly => {
                let q = self.snap_q(items, m, None);
                if self.poly.len() >= 3 && self.to_screen(self.poly[0]).distance(pp) < 10.0 {
                    self.finish_poly(p, items, change);
                } else if self.poly.last() != Some(&q) {
                    self.poly.push(q);
                }
            }
            Tool::Line => self.drag = Some(Drag::Line { a: self.snap_q(items, m, None) }),
            Tool::Label => match self.label_target(items, pp) {
                Some(LabelHit::Corner(v)) => self.open_corner(items, v),
                Some(LabelHit::Side(s)) => {
                    self.side_text = cs::side_label(items, s.a, s.b).unwrap_or_default();
                    self.side = Some(s);
                }
                Some(LabelHit::Angle(v, ext)) => self.open_angle(items, v, ext),
                None => self.drag = Some(Drag::Pan),
            },
            t => {
                if let Some(kind) = t.new_kind() {
                    self.drag = Some(Drag::Create { kind, a: self.snap_q(items, m, None) });
                }
            }
        }
    }

    fn release(&mut self, p: &mut Params, items: &mut Vec<Item>, m: Pt, change: &mut Change) {
        match self.drag.take() {
            Some(Drag::Label { moved: true, .. }) => *change = Change::Step,
            Some(Drag::Label { at, i, .. }) => {
                // a click on a label: select its mark (Select) or open its popover (Label)
                if self.tool == Tool::Select {
                    self.sel = Some(i);
                } else {
                    // the popover of the part the label belongs to
                    let same = |u: Pt, v: Pt| cs::dist(u, v) < 1e-6;
                    match items.get(i).and_then(|it| it.shape.clone()) {
                        Some(Shape::Dim(a, b, _)) => {
                            if let Some(sd) = cs::outer_sides(items).into_iter().find(|sd| (same(sd.a, a) && same(sd.b, b)) || (same(sd.a, b) && same(sd.b, a))) {
                                self.side_text = cs::side_label(items, sd.a, sd.b).unwrap_or_default();
                                self.side = Some(sd);
                            }
                        }
                        Some(Shape::Angle { v, ext, .. }) => self.open_angle(items, v, ext),
                        Some(Shape::Point(v, _)) => self.open_corner(items, v),
                        _ => {
                            let tol = 6.0 / self.scale();
                            let mm = self.to_model(at);
                            self.click(p, items, at, mm, tol, change);
                            if matches!(self.drag, Some(Drag::Pan)) {
                                self.drag = None;
                            }
                        }
                    }
                }
            }
            Some(Drag::Create { kind, a }) => {
                let b = self.snap_q(items, m, None);
                let sh = cs::piece_from_drag(kind, a, b).unwrap_or_else(|| cs::default_piece(kind, a));
                items.push(Item::new(sh));
                self.sel = Some(items.len() - 1);
                self.tool = Tool::Select;
                self.write(p, items);
                *change = Change::Step;
            }
            Some(Drag::Line { a }) => {
                let b = self.snap_q(items, m, None);
                if cs::dist(a, b) > 1e-6 {
                    items.push(Item::new(Shape::Dash(a, b)));
                    self.write(p, items);
                    *change = Change::Step;
                }
            }
            _ => {}
        }
    }

    fn finish_poly(&mut self, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        if let Some(sh) = cs::poly_from(&self.poly) {
            items.push(Item::new(sh));
            self.sel = Some(items.len() - 1);
            self.tool = Tool::Select;
            self.write(p, items);
            *change = Change::Step;
        }
        self.poly.clear();
    }

    fn delete_sel(&mut self, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        if let Some(i) = self.sel.take() {
            cs::delete(items, i);
            self.write(p, items);
            *change = Change::Step;
        }
    }

    fn duplicate_sel(&mut self, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        if let Some(i) = self.sel {
            let st = self.snap_step();
            let off = if st > 0.0 { st.max(1.0) } else { 1.0 };
            self.sel = cs::duplicate(items, i, (off, -off));
            self.write(p, items);
            *change = Change::Step;
        }
    }

    fn keys(&mut self, ui: &mut Ui, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        let (del, back, esc, enter) = ui.input(|i| (i.key_pressed(Key::Delete), i.key_pressed(Key::Backspace), i.key_pressed(Key::Escape), i.key_pressed(Key::Enter)));
        let dup = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::D));
        if !self.poly.is_empty() {
            if back {
                self.poly.pop();
            }
            if enter {
                self.finish_poly(p, items, change);
            }
            if esc {
                self.poly.clear();
            }
            return;
        }
        if esc {
            if self.side.is_some() || self.add.is_some() || self.angle.is_some() || self.corner.is_some() {
                self.close_popovers();
            } else if self.sel.is_some() {
                self.sel = None;
            } else {
                self.tool = Tool::Select;
            }
        }
        if del || back {
            self.delete_sel(p, items, change);
        }
        if dup {
            self.duplicate_sel(p, items, change);
        }
        if let Some(i) = self.sel {
            let st = if self.snap_step() > 0.0 { self.snap_step() } else { 0.1 };
            let d = ui.input(|inp| {
                let mut d = (0.0, 0.0);
                if inp.key_pressed(Key::ArrowLeft) { d.0 -= st; }
                if inp.key_pressed(Key::ArrowRight) { d.0 += st; }
                if inp.key_pressed(Key::ArrowUp) { d.1 += st; }
                if inp.key_pressed(Key::ArrowDown) { d.1 -= st; }
                d
            });
            if d != (0.0, 0.0) && i < items.len() {
                cs::move_item(items, i, d);
                self.write(p, items);
                *change = Change::Step;
            }
        }
    }

    // ───────────────────────── toolbar ─────────────────────────

    fn toolbar(&mut self, ui: &mut Ui, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 4.0);
            for t in Tool::ALL {
                let on = self.tool == t;
                if ui.add(seg_btn(t.name(), on)).on_hover_text(t.tip()).clicked() {
                    self.tool = t;
                    self.poly.clear();
                    self.close_popovers();
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 4.0);
            if !self.poly.is_empty() && ui.add_enabled(self.poly.len() >= 3, seg_btn("Finish", false)).on_hover_text("Close the polygon (Enter)").clicked() {
                self.finish_poly(p, items, change);
            }
            let has = self.sel.is_some_and(|i| i < items.len());
            if ui.add_enabled(has, seg_btn("Duplicate", false)).on_hover_text("Copy the selected piece (Ctrl+D)").clicked() {
                self.duplicate_sel(p, items, change);
            }
            if ui.add_enabled(has, seg_btn("Delete", false)).on_hover_text("Delete the selected piece and the marks only it has (Delete)").clicked() {
                self.delete_sel(p, items, change);
            }
            if ui.add_enabled(!items.is_empty(), seg_btn("Clear", false)).on_hover_text("Remove every piece and mark").clicked() {
                items.clear();
                self.sel = None;
                self.write(p, items);
                *change = Change::Step;
            }
            ui.add_space(6.0);
            let cur = SNAPS[self.snap_i % SNAPS.len()].1;
            egui::ComboBox::from_id_salt("comp_snap").width(52.0).selected_text(RichText::new(format!("Snap {cur}")).size(12.0)).show_ui(ui, |ui| {
                for (i, (_, l)) in SNAPS.iter().enumerate() {
                    ui.selectable_value(&mut self.snap_i, i, *l);
                }
            });
            ui.menu_button(RichText::new("Settings").size(12.0), |ui| {
                ui.set_min_width(220.0);
                ui.label(theme::muted("Unit").size(11.0));
                let mut unit = p.s("unit").to_string();
                if ui.add(TextEdit::singleline(&mut unit).hint_text("cm").desired_width(120.0)).changed() {
                    p.set_s("unit", &unit);
                    *change = Change::Live;
                }
                ui.add_space(4.0);
                ui.label(theme::muted("Round lengths to").size(11.0));
                ui.horizontal(|ui| {
                    for (d, l) in [(0, "Whole"), (1, "Tenths"), (2, "Hundredths")] {
                        if ui.add(seg_btn(l, p.i("dec") == d)).clicked() {
                            p.set_i("dec", d);
                            *change = Change::Step;
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(theme::muted("Round angles to").size(11.0));
                ui.horizontal(|ui| {
                    for (d, l) in [(0, "Whole"), (1, "Tenths"), (2, "Hundredths")] {
                        if ui.add(seg_btn(l, p.i("angle_dec") == d)).clicked() {
                            p.set_i("angle_dec", d);
                            *change = Change::Step;
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(theme::muted("Label size").size(11.0));
                ui.horizontal(|ui| {
                    for (l, px) in [("Large", 16.0), ("Medium", 13.0), ("Small", 10.0)] {
                        let on = p.s("label_size") == l;
                        let b = Button::new(RichText::new("A").size(px).color(if on { theme::accent_col() } else { theme::muted_col() }))
                            .fill(if on { theme::accent_faded() } else { Color32::TRANSPARENT })
                            .stroke(Stroke::new(1.0, if on { theme::accent_col() } else { theme::border() }));
                        if ui.add(b).on_hover_text(format!("{l} labels")).clicked() {
                            p.set_s("label_size", l);
                            *change = Change::Step;
                        }
                    }
                });
                ui.add_space(4.0);
                let mut sh = p.b("shade");
                if ui.checkbox(&mut sh, "Shade the shape").changed() {
                    p.set_b("shade", sh);
                    *change = Change::Step;
                }
            });
        });
    }

    fn zoom_bar(&mut self, ui: &mut Ui, items: &[Item]) {
        let r = self.rect;
        let at = pos2(r.right() - 8.0, r.bottom() - 8.0);
        egui::Area::new(Id::new("comp_zoom"))
            .order(egui::Order::Middle)
            .fixed_pos(at)
            .pivot(Align2::RIGHT_BOTTOM)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let c = r.center();
                        if ui.add(Button::new("\u{2212}").frame(false)).on_hover_text("Zoom out").clicked() {
                            self.zoom_about(c, 1.0 / 1.25);
                        }
                        let pct = (self.scale() / UNIT_PX * 100.0).round();
                        if ui.add(Button::new(RichText::new(format!("{pct}%")).size(12.0)).frame(false)).on_hover_text("Zoom to 100%").clicked() {
                            self.zoom_about(c, UNIT_PX / self.scale());
                        }
                        if ui.add(Button::new("+").frame(false)).on_hover_text("Zoom in").clicked() {
                            self.zoom_about(c, 1.25);
                        }
                        if ui.add(Button::new(RichText::new("Fit").size(12.0)).frame(false)).on_hover_text("Fit the shape in view").clicked() {
                            self.fit(items);
                        }
                    });
                });
            });
    }

    // ───────────────────────── drawing ─────────────────────────

    fn grid(&self, painter: &egui::Painter) {
        let s = self.scale();
        let g = [0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0].into_iter().find(|g| g * s >= 18.0).unwrap_or(100.0);
        let r = self.rect;
        let (a, b) = (self.to_model(r.left_bottom()), self.to_model(r.right_top()));
        let (i0, i1) = ((a.0 / g).floor() as i64, (b.0 / g).ceil() as i64);
        let (j0, j1) = ((a.1 / g).floor() as i64, (b.1 / g).ceil() as i64);
        if (i1 - i0) * (j1 - j0) > 40_000 {
            return;
        }
        let dot = Color32::from_rgb(0xb8, 0xbd, 0xc6);
        for i in i0..=i1 {
            for j in j0..=j1 {
                let q = self.to_screen((i as f64 * g, j as f64 * g));
                let major = (i as f64 * g).rem_euclid(g * 5.0).abs() < 1e-9 && (j as f64 * g).rem_euclid(g * 5.0).abs() < 1e-9;
                painter.circle_filled(q, if major { 1.9 } else { 1.2 }, dot);
            }
        }
    }

    /// The figure itself, rendered by the real renderer at the canvas' scale (so labels,
    /// rounding, unit and label size look exactly as exported).
    fn texture(&mut self, ctx: &egui::Context, p: &Params, painter: &egui::Painter) {
        let r = self.rect;
        let (cv, pr) = self.render_view(p);
        // never past the GPU's texture side; the image is drawn at the rect's size regardless
        let ppp = ctx.pixels_per_point().min(ctx.input(|i| i.max_texture_side) as f32 / (cv.w.max(cv.h) as f32).max(1.0));
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}{:?}{}", pr.0, cv, ppp).hash(&mut hs);
        let key = hs.finish();
        if self.tex.as_ref().is_none_or(|(k, _)| *k != key) {
            let svg = composite_view_svg(&pr, cv);
            if let Ok((iw, ih, px)) = crate::figures::to_rgba(&svg, ppp) {
                let img = egui::ColorImage::from_rgba_premultiplied([iw as usize, ih as usize], &px);
                match &mut self.tex {
                    Some((k, t)) => {
                        t.set(img, egui::TextureOptions::LINEAR);
                        *k = key;
                    }
                    None => self.tex = Some((key, ctx.load_texture("composite-canvas", img, egui::TextureOptions::LINEAR))),
                }
            }
        }
        if let Some((_, t)) = &self.tex {
            let sz = vec2(r.width(), r.height());
            // Lines mode greys the figure so the snap markers stand out
            let tint = if self.tool == Tool::Line { Color32::from_white_alpha(110) } else { Color32::WHITE };
            painter.image(t.id(), Rect::from_min_size(r.min, sz), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
        }
    }

    fn overlay(&self, ui: &Ui, painter: &egui::Painter, items: &[Item], hover: Option<Pos2>) {
        let acc = theme::accent_col();
        let blue = Color32::from_rgb(0x25, 0x63, 0xeb);
        let sel_col = blue;
        let shape_pts = |sh: &Shape| -> Vec<Pos2> { cs::outline(sh).into_iter().map(|q| self.to_screen(q)).collect() };
        let hm = hover.map(|h| self.to_model(h));
        let tol = 6.0 / self.scale();
        // hover outline (select tool)
        if self.tool == Tool::Select
            && self.drag.is_none()
            && let Some(m) = hm
            && let Some(i) = cs::hit(items, m, tol).filter(|i| Some(*i) != self.sel)
            && let Some(sh) = &items[i].shape
        {
            self.draw_item_outline(painter, sh, Stroke::new(1.5, sel_col.gamma_multiply(0.55)));
            ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
        }
        // selection, grips and "+" buttons
        if let Some(i) = self.sel
            && let Some(sh) = items.get(i).and_then(|it| it.shape.as_ref())
        {
            self.draw_item_outline(painter, sh, Stroke::new(2.2, sel_col));
            if self.tool == Tool::Select {
                if sh.is_piece() {
                    for s in cs::outer_sides(items).into_iter().filter(|s| s.piece == i && s.len() * self.scale() > 24.0) {
                        let c = self.plus_pos(&s);
                        let hot = hover.is_some_and(|h| h.distance(c) < 11.0);
                        painter.circle_filled(c, if hot { 10.0 } else { 9.0 }, sel_col);
                        painter.line_segment([c - vec2(4.5, 0.0), c + vec2(4.5, 0.0)], Stroke::new(2.0, Color32::WHITE));
                        painter.line_segment([c - vec2(0.0, 4.5), c + vec2(0.0, 4.5)], Stroke::new(2.0, Color32::WHITE));
                    }
                }
                for (g, q) in cs::grips(sh) {
                    let c = self.to_screen(q);
                    let hot = hover.is_some_and(|h| h.distance(c) < 9.0);
                    if hot {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                    let (fill, st) = if hot { (sel_col, Stroke::new(2.0, Color32::WHITE)) } else { (Color32::WHITE, Stroke::new(1.8, sel_col)) };
                    match g {
                        Grip::Side(k) => {
                            let sz = if k % 2 == 0 { vec2(16.0, 8.0) } else { vec2(8.0, 16.0) };
                            painter.rect(Rect::from_center_size(c, sz), CornerRadius::same(4), fill, st, egui::StrokeKind::Middle);
                        }
                        Grip::Corner(_) => {
                            painter.rect(Rect::from_center_size(c, vec2(10.0, 10.0)), CornerRadius::same(2), fill, st, egui::StrokeKind::Middle);
                        }
                        _ => {
                            painter.circle(c, if hot { 7.0 } else { 5.5 }, fill, st);
                        }
                    }
                }
            }
        }
        match self.tool {
            Tool::Poly if !self.poly.is_empty() => {
                let mut pts: Vec<Pos2> = self.poly.iter().map(|q| self.to_screen(*q)).collect();
                if let Some(h) = hover {
                    pts.push(self.to_screen(self.snap_q(items, self.to_model(h), None)));
                }
                painter.add(egui::Shape::line(pts.clone(), Stroke::new(2.0, blue)));
                for (k, q) in self.poly.iter().enumerate() {
                    let c = self.to_screen(*q);
                    let first_hot = k == 0 && self.poly.len() >= 3 && hover.is_some_and(|h| h.distance(c) < 10.0);
                    painter.circle(c, if first_hot { 8.0 } else if k == 0 { 6.0 } else { 4.5 }, Color32::WHITE, Stroke::new(2.0, blue));
                }
            }
            Tool::Line | Tool::Poly | Tool::Rect | Tool::Tri | Tool::Semi | Tool::Quarter | Tool::Circle => {
                // snap targets: circles on corners and where pieces meet, diamonds on midpoints
                let snapped = hover.filter(|h| self.rect.contains(*h)).map(|h| self.snap_q(items, self.to_model(h), None));
                let (corners, mids) = cs::line_targets(items);
                let hot = |q: Pt| snapped.is_some_and(|s| cs::dist(s, q) < 1e-9);
                for q in corners {
                    let c = self.to_screen(q);
                    painter.circle(c, if hot(q) { 6.5 } else { 4.5 }, if hot(q) { blue } else { Color32::WHITE }, Stroke::new(1.5, blue));
                }
                for q in mids {
                    let c = self.to_screen(q);
                    diamond_filled(painter, c, if hot(q) { 7.0 } else { 5.0 }, if hot(q) { blue } else { Color32::WHITE }, Stroke::new(1.5, blue));
                }
                if let Some(q) = snapped {
                    let c = self.to_screen(q);
                    painter.circle_stroke(c, 3.0, Stroke::new(1.5, blue));
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                }
            }
            Tool::Label => {
                let target = hover.filter(|h| self.rect.contains(*h) && self.drag.is_none()).and_then(|h| self.label_target(items, h));
                let soft = blue.gamma_multiply(0.3);
                // the open popover's part stays lit
                let lit_side = self.side.or(match target { Some(LabelHit::Side(s)) => Some(s), _ => None });
                if let Some(s) = lit_side {
                    painter.line_segment([self.to_screen(s.a), self.to_screen(s.b)], Stroke::new(8.0, soft));
                }
                for (v, ext) in self.angle.into_iter().chain(match target { Some(LabelHit::Angle(v, e)) => Some((v, e)), _ => None }) {
                    if let Some(g) = cs::angle_geo(items, v, ext) {
                        let c = self.to_screen(v);
                        let a1 = self.sdir(v, g.r1);
                        let f1 = a1.y.atan2(a1.x) as f64;
                        let n = ((g.deg / 6.0).ceil() as usize).max(2);
                        let mut pts = vec![c];
                        pts.extend((0..=n).map(|i| {
                            let t = f1 - (g.deg * i as f64 / n as f64).to_radians();
                            c + vec2(t.cos() as f32, t.sin() as f32) * 36.0
                        }));
                        painter.add(egui::Shape::convex_polygon(pts, blue.gamma_multiply(0.18), Stroke::NONE));
                    }
                }
                for v in cs::outer_corners(items) {
                    let c = self.to_screen(v);
                    let hot = self.corner.is_some_and(|w| cs::dist(w, v) < 1e-6) || matches!(target, Some(LabelHit::Corner(w)) if cs::dist(w, v) < 1e-6);
                    if hot {
                        painter.circle(c, 7.0, blue.gamma_multiply(0.25), Stroke::new(1.5, blue));
                    } else {
                        painter.circle_stroke(c, 4.0, Stroke::new(1.2, blue.gamma_multiply(0.45)));
                    }
                }
                if target.is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
            _ => {}
        }
        // drag previews
        if let (Some(h), Some(d)) = (hover.or_else(|| ui.input(|i| i.pointer.interact_pos())), &self.drag) {
            let m = self.snap_q(items, self.to_model(h), None);
            match d {
                Drag::Create { kind, a } => {
                    let sh = cs::piece_from_drag(*kind, *a, m).unwrap_or_else(|| cs::default_piece(*kind, *a));
                    let mut pts = shape_pts(&sh);
                    if let Some(f) = pts.first().copied() {
                        pts.push(f);
                    }
                    painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(2.0, blue), 7.0, 4.0));
                }
                Drag::Line { a } => {
                    painter.extend(egui::Shape::dashed_line(&[self.to_screen(*a), self.to_screen(m)], Stroke::new(2.0, blue), 6.0, 4.0));
                }
                _ => {}
            }
        }
        // hint
        let hint = if items.is_empty() { "" } else { self.tool.tip() };
        painter.text(self.rect.left_bottom() + vec2(8.0, -8.0), Align2::LEFT_BOTTOM, hint, FontId::proportional(11.0), theme::muted_col());
        let _ = acc;
    }

    fn draw_item_outline(&self, painter: &egui::Painter, sh: &Shape, st: Stroke) {
        if sh.is_piece() {
            let pts: Vec<Pos2> = cs::outline(sh).into_iter().map(|q| self.to_screen(q)).collect();
            painter.add(egui::Shape::closed_line(pts, st));
        } else {
            let pts = cs::mark_points(sh);
            match pts.as_slice() {
                [a, b] => {
                    painter.line_segment([self.to_screen(*a), self.to_screen(*b)], st);
                }
                [a] => {
                    painter.circle_stroke(self.to_screen(*a), 12.0, st);
                }
                _ => {}
            }
        }
    }

    // ───────────────────────── popovers ─────────────────────────

    /// The selection's properties: sizes, shading, label, duplicate / delete.
    fn props(&mut self, ui: &mut Ui, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        if self.tool != Tool::Select {
            return;
        }
        let Some(i) = self.sel.filter(|i| *i < items.len()) else { return };
        let Some(mut sh) = items[i].shape.clone() else { return };
        let before = sh.clone();
        let mut shade = items[i].shade.clone();
        let mut label = items[i].label.clone();
        let mut close = false;
        let mut act: Option<&str> = None;
        // the panel sits on the side of the canvas away from the selection
        let pts: Vec<Pt> = cs::outline(&sh).into_iter().chain(cs::mark_points(&sh)).collect();
        let left_half = cs::bbox(&pts).is_some_and(|(x0, _, x1, _)| self.to_screen(((x0 + x1) / 2.0, 0.0)).x < self.rect.center().x);
        let (at, pivot) = if left_half { (self.rect.right_top() + vec2(-10.0, 10.0), Align2::RIGHT_TOP) } else { (self.rect.left_top() + vec2(10.0, 10.0), Align2::LEFT_TOP) };
        egui::Area::new(Id::new("comp_props"))
            .order(egui::Order::Middle)
            .fixed_pos(at)
            .pivot(pivot)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(210.0);
                    ui.horizontal(|ui| {
                        ui.label(theme::strong(sh.name()).size(13.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(Button::new("\u{d7}").frame(false)).on_hover_text("Close").clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.separator();
                    shape_fields(ui, &mut sh);
                    if sh.is_piece() {
                        ui.add_space(4.0);
                        ui.label(theme::muted("Shading").size(11.0));
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let none = shade.is_none();
                            let b = Button::new(RichText::new("\u{d7}").size(11.0).color(theme::muted_col()))
                                .fill(Color32::WHITE)
                                .stroke(Stroke::new(if none { 2.0 } else { 1.0 }, if none { theme::accent_col() } else { theme::border() }))
                                .min_size(vec2(26.0, 22.0));
                            if ui.add(b).on_hover_text("None").clicked() {
                                shade = None;
                            }
                            for (w, hex) in cs::SHADES {
                                let on = shade.as_deref() == Some(w);
                                let b = Button::new("")
                                    .fill(hex_col(hex))
                                    .stroke(Stroke::new(if on { 2.0 } else { 1.0 }, if on { theme::accent_col() } else { theme::border() }))
                                    .min_size(vec2(26.0, 22.0));
                                if ui.add(b).on_hover_text(cap(w)).clicked() {
                                    shade = Some(w.to_string());
                                }
                            }
                        });
                        ui.add_space(4.0);
                        ui.label(theme::muted("Label inside").size(11.0));
                        ui.add(TextEdit::singleline(&mut label).hint_text("e.g. A or 24 cm\u{b2}").desired_width(190.0));
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.add(seg_btn("Duplicate", false)).clicked() {
                            act = Some("dup");
                        }
                        if ui.add(Button::new(RichText::new("Delete").size(12.0).color(theme::red())).stroke(Stroke::new(1.0, theme::border()))).clicked() {
                            act = Some("del");
                        }
                    });
                });
            });
        if close {
            self.sel = None;
            return;
        }
        let it = &mut items[i];
        let mut edited = false;
        if sh != before {
            it.set(sh);
            edited = true;
        }
        if shade != it.shade {
            it.set_shade(shade.as_deref());
            edited = true;
            *change = Change::Step;
        }
        if label.trim() != it.label {
            it.set_label(&label);
            edited = true;
        }
        if edited {
            self.write(p, items);
            if *change == Change::None {
                *change = Change::Live;
            }
        }
        match act {
            Some("dup") => self.duplicate_sel(p, items, change),
            Some("del") => self.delete_sel(p, items, change),
            _ => {}
        }
    }

    /// The "+" menu: add a rectangle, triangle or semicircle on a side of the selection.
    fn add_menu(&mut self, ui: &mut Ui, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        let Some((side, at)) = self.add else { return };
        let mut pick: Option<Shape> = None;
        let mut close = false;
        egui::Area::new(Id::new("comp_add"))
            .order(egui::Order::Foreground)
            .fixed_pos(at + vec2(12.0, 12.0))
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).inner_margin(6.0).show(ui, |ui| {
                    ui.label(theme::muted("Add a piece on this side").size(11.0));
                    for (k, l) in [(NewKind::Rect, "Rectangle"), (NewKind::Tri, "Triangle"), (NewKind::Semi, "Semicircle")] {
                        let sh = cs::piece_on_side(k, &side);
                        if ui.add_enabled(sh.is_some(), Button::new(l).min_size(vec2(150.0, 0.0))).clicked() {
                            pick = sh;
                        }
                    }
                    if ui.add(Button::new(theme::muted("Cancel").size(11.0)).frame(false)).clicked() {
                        close = true;
                    }
                });
            });
        if let Some(sh) = pick {
            let mut it = Item::new(sh);
            // a new piece takes the shading of the one it grows from
            if let Some(w) = items.get(side.piece).and_then(|i| i.shade.clone()) {
                it.set_shade(Some(&w));
            }
            items.push(it);
            self.sel = Some(items.len() - 1);
            self.write(p, items);
            *change = Change::Step;
            self.add = None;
        } else if close {
            self.add = None;
        }
    }

    /// Label tool: a side's label (None / Measure / Text), congruence ticks and parallel arrows.
    fn side_popover(&mut self, ui: &mut Ui, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        if self.tool != Tool::Label {
            self.side = None;
            return;
        }
        let Some(side) = self.side else { return };
        let cur = cs::side_label(items, side.a, side.b);
        let mode = match &cur { None => "None", Some(t) if t.trim().is_empty() => "Measure", _ => "Text" };
        let (mut new_mode, mut text) = (mode, self.side_text.clone());
        let (ticks0, arrows0) = (cs::side_marks(items, side.a, side.b, false), cs::side_marks(items, side.a, side.b, true));
        let (mut ticks, mut arrows) = (ticks0, arrows0);
        let style0 = cs::side_style(items, side.a, side.b);
        let (mut style, mut thick) = style0.clone();
        let measured = {
            let dec = p.i("dec").clamp(0, 4) as usize;
            let n = format!("{:.*}", dec, side.len());
            let n = if n.contains('.') { n.trim_end_matches('0').trim_end_matches('.').to_string() } else { n };
            if p.s("unit").trim().is_empty() { n } else { format!("{n} {}", p.s("unit").trim()) }
        };
        let mut close = false;
        let out = self.sdir(side.mid(), side.out);
        let anchor = self.plus_pos(&side) + out * 10.0;
        let pivot = self.pivot_in(anchor, out, vec2(250.0, 330.0));
        egui::Area::new(Id::new("comp_side"))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .pivot(pivot)
            .constrain_to(self.popover_room())
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(230.0);
                    ui.horizontal(|ui| {
                        ui.label(theme::strong("Side").size(13.0));
                        ui.label(theme::muted(&measured).size(12.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(Button::new("\u{d7}").frame(false)).clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.label(theme::muted("Label").size(11.0));
                    ui.horizontal(|ui| {
                        for m in ["None", "Measure", "Text"] {
                            if ui.add(seg_btn(m, new_mode == m)).clicked() {
                                new_mode = m;
                            }
                        }
                    });
                    if new_mode == "Text" {
                        ui.add(TextEdit::singleline(&mut text).hint_text("e.g. x or 2x + 1").desired_width(200.0));
                    }
                    ui.add_space(4.0);
                    ui.label(theme::muted("Congruence ticks").size(11.0));
                    ui.horizontal(|ui| {
                        for (k, l) in [(0u8, "None"), (1, "|"), (2, "||"), (3, "|||")] {
                            if ui.add(seg_btn(l, ticks == k)).clicked() {
                                ticks = k;
                            }
                        }
                    });
                    ui.label(theme::muted("Parallel arrows").size(11.0));
                    ui.horizontal(|ui| {
                        for (k, l) in [(0u8, "None"), (1, "\u{203a}"), (2, "\u{203a}\u{203a}"), (3, "\u{203a}\u{203a}\u{203a}")] {
                            if ui.add(seg_btn(l, arrows == k)).clicked() {
                                arrows = k;
                            }
                        }
                    });
                    ui.label(theme::muted("Line").size(11.0));
                    ui.horizontal(|ui| {
                        for w in cs::LINE_STYLES {
                            if ui.add(seg_btn(&cap(w), style == w)).clicked() {
                                style = w.to_string();
                            }
                        }
                    });
                    ui.label(theme::muted("Thickness").size(11.0));
                    ui.horizontal(|ui| {
                        for w in ["thin", "regular", "thick"] {
                            if ui.add(seg_btn(&cap(w), thick == w)).clicked() {
                                thick = w.to_string();
                            }
                        }
                    });
                });
            });
        if close {
            self.side = None;
            return;
        }
        let mut edited = false;
        if new_mode != mode || (new_mode == "Text" && text != self.side_text) {
            if new_mode == "Text" && text.trim().is_empty() && mode != "Text" {
                text = measured.clone();
            }
            let lab = match new_mode {
                "None" => None,
                "Measure" => Some(String::new()),
                _ => Some(text.clone()),
            };
            // an emptied text box keeps the label until something is typed
            if !(new_mode == "Text" && text.trim().is_empty()) {
                cs::set_side_label(items, side.a, side.b, side.out, lab.as_deref());
                edited = true;
                *change = if new_mode == mode { Change::Live } else { Change::Step };
            }
        }
        self.side_text = text;
        if ticks != ticks0 {
            cs::set_side_marks(items, side.a, side.b, false, ticks);
            edited = true;
            *change = Change::Step;
        }
        if arrows != arrows0 {
            cs::set_side_marks(items, side.a, side.b, true, arrows);
            edited = true;
            *change = Change::Step;
        }
        if (style.clone(), thick.clone()) != style0 {
            cs::set_side_style(items, side.a, side.b, &style, &thick);
            edited = true;
            *change = Change::Step;
        }
        if edited {
            self.write(p, items);
        }
    }

    /// Label tool: an angle's label (None / Measure / Text), congruence arcs or the right-angle
    /// mark, and shading.
    fn angle_popover(&mut self, ui: &mut Ui, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        if self.tool != Tool::Label {
            self.angle = None;
            return;
        }
        let Some((v, ext)) = self.angle else { return };
        let Some(g) = cs::angle_geo(items, v, ext) else {
            self.angle = None;
            return;
        };
        let m0 = cs::angle_mark(items, v, ext);
        let mut m = m0.clone();
        let right = (g.deg - 90.0).abs() < 1e-6;
        let ra0 = if ext { m0.square } else { cs::has_right_mark(items, v) };
        let mut ra = ra0;
        let mode0 = if m0.measure { "Measure" } else if m0.text.trim().is_empty() { "None" } else { "Text" };
        let mut mode = mode0;
        let mut text = self.angle_text.clone();
        let measured = format!("{}\u{b0}", trim_num(g.deg, p.i("angle_dec")));
        let mut close = false;
        // an exterior angle's popover opens beyond the wedge, along its bisector; an interior
        // angle's opens just outside the corner (the other way), so it never covers the shape
        let a1 = self.sdir(v, g.r1);
        let t = a1.y.atan2(a1.x) as f64 - (g.deg / 2.0).to_radians();
        let bis = vec2(t.cos() as f32, t.sin() as f32);
        let (dir, reach) = angle_popover_dir(bis, ext);
        let anchor = self.to_screen(v) + dir * reach;
        egui::Area::new(Id::new("comp_angle"))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .pivot(self.pivot_in(anchor, dir, vec2(250.0, 250.0)))
            .constrain_to(self.popover_room())
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(230.0);
                    ui.horizontal(|ui| {
                        ui.label(theme::strong(if ext { "Exterior angle" } else { "Angle" }).size(13.0));
                        ui.label(theme::muted(&measured).size(12.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(Button::new("\u{d7}").frame(false)).clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.label(theme::muted("Label").size(11.0));
                    ui.horizontal(|ui| {
                        for l in ["None", "Measure", "Text"] {
                            if ui.add(seg_btn(l, mode == l)).clicked() {
                                mode = l;
                            }
                        }
                    });
                    if mode == "Text" {
                        ui.add(TextEdit::singleline(&mut text).hint_text("x").desired_width(200.0));
                    }
                    ui.add_space(4.0);
                    if right {
                        ui.label(theme::muted("Right-angle mark").size(11.0));
                        ui.horizontal(|ui| {
                            if ui.add(seg_btn("None", !ra)).clicked() {
                                ra = false;
                            }
                            if ui.add(seg_btn("\u{221f} Square", ra)).on_hover_text("Right-angle square").clicked() {
                                ra = true;
                            }
                        });
                    } else {
                        ui.label(theme::muted("Congruence arcs").size(11.0));
                        ui.horizontal(|ui| {
                            for (k, l) in [(0u8, "None"), (1, "One arc"), (2, "Two"), (3, "Three")] {
                                if ui.add(seg_btn(l, m.arcs == k)).clicked() {
                                    m.arcs = k;
                                }
                            }
                        });
                    }
                    ui.label(theme::muted("Shading").size(11.0));
                    shade_row(ui, &mut m.shade);
                });
            });
        if close {
            self.angle = None;
            return;
        }
        m.measure = mode == "Measure";
        match mode {
            "None" => m.text.clear(),
            "Text" => {
                // a fresh Text starts from x, as the reference's placeholder suggests
                if mode0 != "Text" && text.trim().is_empty() {
                    text = "x".into();
                }
                m.text = text.clone();
            }
            _ => {}
        }
        if ext {
            m.square = ra && right;
        }
        if !right {
            m.square = false;
        }
        let mut edited = false;
        if m != m0 {
            cs::set_angle_mark(items, v, ext, &m);
            edited = true;
            *change = if mode == mode0 && m.text != m0.text && m.arcs == m0.arcs && m.shade == m0.shade { Change::Live } else { Change::Step };
        }
        if !ext && right && ra != ra0 {
            cs::set_right_mark(items, v, ra);
            edited = true;
            *change = Change::Step;
        }
        self.angle_text = text;
        if edited {
            self.write(p, items);
        }
    }

    /// Label tool: a corner's point (No point / Show) and its name.
    fn corner_popover(&mut self, ui: &mut Ui, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        if self.tool != Tool::Label {
            self.corner = None;
            return;
        }
        let Some(v) = self.corner else { return };
        let cur = cs::corner_point(items, v);
        let mut show = cur.is_some();
        let mut name = self.corner_text.clone();
        let mut close = false;
        let dir = self.sdir(v, cs::outside_dir(items, v));
        let anchor = self.to_screen(v) + dir * 22.0;
        egui::Area::new(Id::new("comp_corner"))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .pivot(self.pivot_in(anchor, dir, vec2(220.0, 140.0)))
            .constrain_to(self.popover_room())
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(200.0);
                    ui.horizontal(|ui| {
                        ui.label(theme::strong("Corner").size(13.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(Button::new("\u{d7}").frame(false)).clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.label(theme::muted("Point").size(11.0));
                    ui.horizontal(|ui| {
                        if ui.add(seg_btn("No point", !show)).clicked() {
                            show = false;
                        }
                        if ui.add(seg_btn("\u{25cf} Show", show)).clicked() {
                            show = true;
                        }
                    });
                    if show {
                        ui.label(theme::muted("Label").size(11.0));
                        ui.add(TextEdit::singleline(&mut name).hint_text("A").desired_width(170.0));
                    }
                });
            });
        if close {
            self.corner = None;
            return;
        }
        let want = show.then(|| name.trim().to_string());
        if want != cur {
            cs::set_corner_point(items, v, want.as_deref());
            *change = if show == cur.is_some() { Change::Live } else { Change::Step };
            self.write(p, items);
        }
        self.corner_text = name;
    }

    /// The figure's Turn (Label tool), with Straighten: bottom right, over the zoom bar. The slider
    /// is TURN_SLIDER_W points for 360°; the number box scrubs at TURN_SCRUB.
    fn turn_bar(&mut self, ui: &mut Ui, p: &mut Params, change: &mut Change) {
        if self.tool != Tool::Label {
            return;
        }
        let r = self.rect;
        egui::Area::new(Id::new("comp_turn"))
            .order(egui::Order::Middle)
            .fixed_pos(pos2(r.right() - 8.0, r.bottom() - 48.0))
            .pivot(Align2::RIGHT_BOTTOM)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).inner_margin(6.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(RichText::new("\u{21bb}").size(14.0)).on_hover_text("Turn the figure");
                        // drag the slider to turn (-180 to 180), or type/scrub the angle in the box
                        let mut t = p.f("rotate");
                        ui.spacing_mut().slider_width = TURN_SLIDER_W;
                        // the popup is filled in the card colour the theme also gives slider
                        // rails, which made the rail invisible here: draw it in the border colour
                        ui.visuals_mut().widgets.inactive.bg_fill = theme::border();
                        let sl = ui
                            .add(egui::Slider::new(&mut t, -180.0..=180.0).show_value(false).step_by(1.0))
                            .on_hover_text("Drag to change the figure's turn");
                        // the box scrubs at the reference knob's half a degree per point
                        let resp = ui.add(egui::DragValue::new(&mut t).range(-180.0..=180.0).speed(TURN_SCRUB).max_decimals(1).suffix("\u{b0}"));
                        if sl.changed() || resp.changed() {
                            p.set_f("rotate", t);
                            *change = Change::Live;
                        }
                        if ui.add_enabled(p.f("rotate") != 0.0, seg_btn("Straighten", false)).on_hover_text("Turn the figure back to 0\u{b0}").clicked() {
                            p.set_f("rotate", 0.0);
                            *change = Change::Step;
                        }
                    });
                });
            });
    }

    /// The opening screen of an empty canvas (the reference's "Start with"): one piece, or a
    /// shape already built from pieces.
    fn start_screen(&mut self, ui: &mut Ui, p: &mut Params, items: &mut Vec<Item>, change: &mut Change) {
        let mut pick: Option<&str> = None;
        egui::Area::new(Id::new("comp_start"))
            .order(egui::Order::Middle)
            .fixed_pos(self.rect.center())
            .pivot(Align2::CENTER_CENTER)
            .show(ui.ctx(), |ui| {
                // the reference sets it straight on the page: no card, a wide centred block
                Frame::new().inner_margin(8.0).show(ui, |ui| {
                    ui.set_max_width(620.0);
                    ui.vertical_centered(|ui| {
                        ui.label(theme::strong("Start with").size(20.0));
                        ui.add_space(4.0);
                        ui.allocate_ui(vec2(480.0, 20.0), |ui| {
                            ui.set_max_width(480.0);
                            ui.add(egui::Label::new(theme::muted("One piece, or a shape already built from pieces. Then add more pieces onto any straight side.").size(14.0)).wrap());
                        });
                    });
                    ui.add_space(14.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                        for name in COMPOSITE_STARTS {
                            if start_chip(ui, name).clicked() {
                                pick = Some(name);
                            }
                        }
                    });
                });
            });
        if let Some(name) = pick
            && let Some(spec) = preset_spec(name)
        {
            *items = cs::parse(spec);
            self.write(p, items);
            self.sel = None;
            self.tool = Tool::Select;
            self.view = None;
            *change = Change::Step;
        }
    }
}

/// Where a popover hangs off its anchor so it opens along screen direction `d`.
/// Direction and distance (points) from a corner to its angle popover's anchor: along the
/// wedge's bisector `bis` past the wedge for an exterior angle, the opposite way (outside the
/// shape) for an interior one.
fn angle_popover_dir(bis: egui::Vec2, ext: bool) -> (egui::Vec2, f32) {
    if ext { (bis, 54.0) } else { (-bis, 26.0) }
}

fn pivot_for(d: egui::Vec2) -> Align2 {
    if d.x.abs() >= d.y.abs() {
        if d.x >= 0.0 { Align2::LEFT_CENTER } else { Align2::RIGHT_CENTER }
    } else if d.y >= 0.0 {
        Align2::CENTER_TOP
    } else {
        Align2::CENTER_BOTTOM
    }
}

/// `v` to `dec` decimals without trailing zeros.
fn trim_num(v: f64, dec: i64) -> String {
    let n = format!("{:.*}", dec.clamp(0, 4) as usize, v);
    if n.contains('.') { n.trim_end_matches('0').trim_end_matches('.').to_string() } else { n }
}

/// None / Blue / Red / Green / Yellow swatches.
fn shade_row(ui: &mut Ui, shade: &mut Option<String>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let none = shade.is_none();
        let b = Button::new(RichText::new("\u{d7}").size(11.0).color(theme::muted_col()))
            .fill(Color32::WHITE)
            .stroke(Stroke::new(if none { 2.0 } else { 1.0 }, if none { theme::accent_col() } else { theme::border() }))
            .min_size(vec2(26.0, 22.0));
        if ui.add(b).on_hover_text("None").clicked() {
            *shade = None;
        }
        for (w, hex) in cs::SHADES {
            let on = shade.as_deref() == Some(w);
            let b = Button::new("")
                .fill(hex_col(hex))
                .stroke(Stroke::new(if on { 2.0 } else { 1.0 }, if on { theme::accent_col() } else { theme::border() }))
                .min_size(vec2(26.0, 22.0));
            if ui.add(b).on_hover_text(cap(w)).clicked() {
                *shade = Some(w.to_string());
            }
        }
    });
}

/// A "Start with" chip: a small drawing of the shape over its name.
fn start_chip(ui: &mut Ui, name: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(144.0, 90.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    let hot = resp.hovered();
    let pt = ui.painter();
    pt.rect(r, CornerRadius::same(10), if hot { theme::accent_faded() } else { Color32::WHITE }, Stroke::new(1.0, if hot { theme::accent_col() } else { theme::border() }), egui::StrokeKind::Inside);
    // the reference draws each shape as a bare blue outline
    let ink = Stroke::new(2.0, Color32::from_rgb(0x1d, 0x4e, 0xd8));
    let fill = Color32::TRANSPARENT;
    let c = r.center() - vec2(0.0, 12.0);
    let poly = |pts: Vec<Pos2>| egui::Shape::convex_polygon(pts, fill, ink);
    let arc = |cx: Pos2, rad: f32, a0: f32, a1: f32| -> Vec<Pos2> { (0..=16).map(|i| { let t = (a0 + (a1 - a0) * i as f32 / 16.0).to_radians(); cx + vec2(t.cos(), -t.sin()) * rad }).collect() };
    match name {
        "Rectangle" => { pt.rect(Rect::from_center_size(c, vec2(36.0, 24.0)), CornerRadius::ZERO, fill, ink, egui::StrokeKind::Middle); }
        "Triangle" => { pt.add(poly(vec![c + vec2(-18.0, 12.0), c + vec2(18.0, 12.0), c + vec2(-4.0, -13.0)])); }
        "Semicircle" => { let mut v = arc(c + vec2(0.0, 9.0), 18.0, 0.0, 180.0); v.push(v[0]); pt.add(poly(v)); }
        "Circle" => { pt.circle(c, 14.0, fill, ink); }
        "L-shape" => {
            let v = [(-14.0, -14.0), (-4.0, -14.0), (-4.0, 4.0), (16.0, 4.0), (16.0, 14.0), (-14.0, 14.0)];
            let pts: Vec<Pos2> = v.iter().map(|(x, y)| c + vec2(*x, *y)).collect();
            pt.rect_filled(Rect::from_min_max(c + vec2(-14.0, -14.0), c + vec2(-4.0, 14.0)), CornerRadius::ZERO, fill);
            pt.rect_filled(Rect::from_min_max(c + vec2(-14.0, 4.0), c + vec2(16.0, 14.0)), CornerRadius::ZERO, fill);
            pt.add(egui::Shape::closed_line(pts, ink));
        }
        "House" => {
            pt.rect(Rect::from_min_max(c + vec2(-14.0, -2.0), c + vec2(14.0, 15.0)), CornerRadius::ZERO, fill, ink, egui::StrokeKind::Middle);
            pt.add(poly(vec![c + vec2(-14.0, -2.0), c + vec2(14.0, -2.0), c + vec2(0.0, -15.0)]));
        }
        _ => {
            // arch: a rectangle under a semicircle
            let mut v = arc(c + vec2(0.0, -1.0), 13.0, 0.0, 180.0);
            v.extend([c + vec2(-13.0, 15.0), c + vec2(13.0, 15.0)]);
            pt.rect_filled(Rect::from_min_max(c + vec2(-13.0, -1.0), c + vec2(13.0, 15.0)), CornerRadius::ZERO, fill);
            pt.add(poly(arc(c + vec2(0.0, -1.0), 13.0, 0.0, 180.0)));
            let mut o = arc(c + vec2(0.0, -1.0), 13.0, 0.0, 180.0);
            o.extend([c + vec2(-13.0, 15.0), c + vec2(13.0, 15.0)]);
            pt.add(egui::Shape::closed_line(o, ink));
            let _ = v;
        }
    }
    pt.text(pos2(r.center().x, r.bottom() - 14.0), Align2::CENTER_CENTER, name, FontId::new(13.5, theme::bold_family()), theme::text_col());
    resp.on_hover_text(format!("Start with {}", if name.contains('-') { name.to_string() } else { format!("a {}", name.to_lowercase()) }))
}

/// Size / position fields of the selected item.
fn shape_fields(ui: &mut Ui, sh: &mut Shape) {
    let num = |ui: &mut Ui, label: &str, v: &mut f64, min: f64| {
        ui.horizontal(|ui| {
            ui.add_sized([62.0, 18.0], egui::Label::new(theme::muted(label).size(11.0)));
            ui.add(egui::DragValue::new(v).speed(0.05).range(min..=10_000.0).max_decimals(3));
        });
    };
    let pt = |ui: &mut Ui, label: &str, q: &mut Pt| {
        ui.horizontal(|ui| {
            ui.add_sized([62.0, 18.0], egui::Label::new(theme::muted(label).size(11.0)));
            ui.add(egui::DragValue::new(&mut q.0).speed(0.05).range(-10_000.0..=10_000.0).max_decimals(3).prefix("x "));
            ui.add(egui::DragValue::new(&mut q.1).speed(0.05).range(-10_000.0..=10_000.0).max_decimals(3).prefix("y "));
        });
    };
    let dirs = |ui: &mut Ui, cur: &mut String, opts: &[(&str, &str)]| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (v, l) in opts {
                if ui.add(seg_btn(l, cur.as_str() == *v)).clicked() {
                    *cur = v.to_string();
                }
            }
        });
    };
    const SEMI: [(&str, &str); 4] = [("u", "Up"), ("d", "Down"), ("l", "Left"), ("r", "Right")];
    const QUAD: [(&str, &str); 4] = [("ne", "NE"), ("nw", "NW"), ("sw", "SW"), ("se", "SE")];
    match sh {
        Shape::Rect { x, y, w, h } => {
            num(ui, "Width", w, 0.1);
            num(ui, "Height", h, 0.1);
            let mut q = (*x, *y);
            pt(ui, "Corner", &mut q);
            (*x, *y) = q;
            ui.label(theme::muted("Or drag the handles on its sides.").size(11.0));
        }
        Shape::Tri(v) => {
            for (k, q) in v.iter_mut().enumerate() {
                pt(ui, ["A", "B", "C"][k], q);
            }
        }
        Shape::Poly(v) => {
            egui::ScrollArea::vertical().max_height(150.0).id_salt("comp_poly_pts").show(ui, |ui| {
                for (k, q) in v.iter_mut().enumerate() {
                    pt(ui, &format!("Corner {}", k + 1), q);
                }
            });
        }
        Shape::Circle { c, r } => {
            num(ui, "Radius", r, 0.1);
            pt(ui, "Centre", c);
        }
        Shape::Semi { c, r, dir } => {
            num(ui, "Radius", r, 0.1);
            pt(ui, "Centre", c);
            ui.label(theme::muted("Bulges").size(11.0));
            dirs(ui, dir, &SEMI);
        }
        Shape::Quarter { c, r, dir } => {
            num(ui, "Radius", r, 0.1);
            pt(ui, "Corner", c);
            ui.label(theme::muted("Fills").size(11.0));
            dirs(ui, dir, &QUAD);
        }
        Shape::Dash(a, b) => {
            pt(ui, "From", a);
            pt(ui, "To", b);
        }
        Shape::Dim(a, b, t) => {
            ui.label(theme::muted("Text (empty = its length)").size(11.0));
            ui.add(TextEdit::singleline(t).desired_width(190.0));
            pt(ui, "From", a);
            pt(ui, "To", b);
        }
        Shape::Ticks(a, b, k) | Shape::Arrows(a, b, k) => {
            ui.horizontal(|ui| {
                for n in 1..=3u8 {
                    if ui.add(seg_btn(&n.to_string(), *k == n)).clicked() {
                        *k = n;
                    }
                }
            });
            pt(ui, "From", a);
            pt(ui, "To", b);
        }
        Shape::Text(a, t) => {
            ui.add(TextEdit::singleline(t).desired_width(190.0));
            pt(ui, "At", a);
        }
        Shape::Ra(a, d) => {
            let mut lc = d.to_lowercase();
            dirs(ui, &mut lc, &QUAD);
            *d = lc.to_uppercase();
            pt(ui, "At", a);
        }
        // set from the Label tool's popovers
        Shape::SideStyle(..) | Shape::Angle { .. } | Shape::Point(..) => {
            ui.label(theme::muted("Change it with the Label tool.").size(11.0));
        }
    }
}

fn seg_btn(label: &str, on: bool) -> Button<'static> {
    Button::new(RichText::new(label.to_string()).size(12.0).color(if on { theme::accent_col() } else { theme::text_col() }))
        .fill(if on { theme::accent_faded() } else { Color32::TRANSPARENT })
        .stroke(Stroke::new(1.0, if on { theme::accent_col() } else { theme::border() }))
        .corner_radius(CornerRadius::same(8))
}

fn diamond_filled(painter: &egui::Painter, c: Pos2, r: f32, fill: Color32, st: Stroke) {
    let pts = vec![c + vec2(0.0, -r), c + vec2(r, 0.0), c + vec2(0.0, r), c + vec2(-r, 0.0)];
    painter.add(egui::Shape::convex_polygon(pts, fill, st));
}

fn hex_col(hex: &str) -> Color32 {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn cap(w: &str) -> String {
    let mut c = w.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(spec: &str) -> (Canvas, Vec<Item>) {
        let mut c = Canvas { rect: Rect::from_min_size(pos2(100.0, 50.0), vec2(800.0, 500.0)), ..Default::default() };
        let items = cs::parse(spec);
        c.fit(&items);
        (c, items)
    }

    #[test]
    fn view_maps_both_ways_and_fit_shows_everything() {
        let (c, items) = canvas("rect 0 0 8 3; rect 0 3 3 5; dim 0 0 8 0");
        for q in [(0.0, 0.0), (8.0, 8.0), (-2.5, 1.25)] {
            let back = c.to_model(c.to_screen(q));
            assert!(cs::dist(back, q) < 1e-4, "{q:?} -> {back:?}");
        }
        let (x0, y0, x1, y1) = cs::bbox(&cs::all_points(&items)).unwrap();
        let r = Rect::from_two_pos(c.to_screen((x0, y0)), c.to_screen((x1, y1)));
        assert!(c.rect.contains_rect(r) && r.height() > c.rect.height() * 0.5, "{r:?} in {:?}", c.rect);
        // y grows upward on screen
        assert!(c.to_screen((0.0, 1.0)).y < c.to_screen((0.0, 0.0)).y);
        // an empty spec still gets a sane view
        let (e, _) = canvas("");
        assert_eq!(e.scale(), UNIT_PX);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_pointer_and_clamps() {
        let (mut c, _) = canvas("rect 0 0 4 3");
        let p = pos2(300.0, 200.0);
        let m = c.to_model(p);
        c.zoom_about(p, 2.0);
        assert!(cs::dist(c.to_model(p), m) < 1e-6);
        for _ in 0..50 {
            c.zoom_about(p, 2.0);
        }
        assert_eq!(c.scale(), MAX_S);
        for _ in 0..80 {
            c.zoom_about(p, 0.5);
        }
        assert_eq!(c.scale(), MIN_S);
    }

    #[test]
    fn snapping_prefers_nearby_corners_then_the_grid() {
        let (c, items) = canvas("rect 0 0 4 3");
        let reach = 5.0 / c.scale(); // within the 9 px reach
        assert_eq!(c.snap_q(&items, (4.0 + reach, 3.0 - reach), None), (4.0, 3.0));
        assert_eq!(c.snap_q(&items, (2.0 + reach, 3.0), None), (2.0, 3.0)); // a side's midpoint
        assert_eq!(c.snap_q(&items, (6.2, 6.3), None), (6.0, 6.5));
        // the piece being reshaped does not snap to itself
        assert_eq!(c.snap_q(&items, (4.0 + reach, 3.0), Some(0)), (4.0, 3.0));
        let mut off = Canvas { snap_i: 3, ..Default::default() };
        off.rect = c.rect;
        off.view = c.view;
        assert_eq!(off.snap_q(&[], (6.2, 6.3), None), (6.2, 6.3));
    }
}
