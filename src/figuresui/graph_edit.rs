//! Canvas editor for the graph figure: drag nodes around, add nodes, draw an edge or an arrow
//! from one node to another, set a weight, rename, delete. Like the logic-circuit canvas it owns
//! no graph: each frame it reads the figure's `spec` param through `Graph::parse` and writes an
//! edit back with `to_spec`, so the typed spec, presets, undo and the exports all stay on the one
//! text the figure always had. A node's place is saved in the spec as `node A @ x,y`; adding or
//! removing a node first pins every node where it is, so the circle layout does not reshuffle.
use egui::{Align2, Color32, FontId, Frame, Id, Key, Pos2, Rect, Sense, Stroke, TextEdit, Ui, Vec2, vec2};

use super::composite_canvas::Change;
use super::graph_canvas::{self as gc, View, World};
use crate::figures::computing::graph_draw::node_radius;
use crate::figures::graph::{Graph, Layout};
use crate::figures::Params;
use crate::theme;

const INK: Color32 = Color32::from_rgb(0x11, 0x18, 0x27);
const FILL: Color32 = Color32::from_rgb(0xf5, 0xf0, 0xe6);
const SELECT: Color32 = Color32::from_rgb(0x2f, 0x61, 0x44);
/// World pixels per grid unit across and down.
const CELL: (f64, f64) = (100.0, 90.0);
/// Positions snap to a quarter of a grid cell.
const SNAP: f64 = 0.25;
const EDGE_REACH: f32 = 6.0;
const FS: f64 = 15.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Select,
    Node,
    Edge,
    Arrow,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Node => "Node",
            Tool::Edge => "Edge",
            Tool::Arrow => "Arrow",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Sel {
    #[default]
    None,
    Node(String),
    Edge(usize),
}

enum Drag {
    Pan,
    Move { name: String, from: World, orig: (f64, f64), moved: bool },
    Link { from: String, directed: bool, at: Pos2 },
}

#[derive(Clone, Debug, PartialEq)]
enum RenameTarget {
    Node(String),
    Weight(usize),
}

struct Rename {
    target: RenameTarget,
    text: String,
    err: Option<String>,
    first: bool,
}

pub struct Canvas {
    view: Option<View>,
    pub sel: Sel,
    pub tool: Tool,
    drag: Option<Drag>,
    pub note: Option<String>,
    rename: Option<Rename>,
    auto_fit: Option<Vec2>,
    last_spec: String,
    pub rect: Rect,
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas { view: None, sel: Sel::None, tool: Tool::Select, drag: None, note: None, rename: None, auto_fit: None, last_spec: String::new(), rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)) }
    }
}

struct Geo {
    pts: Vec<World>,
    radii: Vec<f64>,
}

impl Geo {
    fn new(g: &Graph, layout: Layout) -> Geo {
        let pts = g.layout(layout).into_iter().map(|p| (p.0 * CELL.0, p.1 * CELL.1)).collect();
        let radii = g.nodes.iter().map(|n| node_radius(&n.name, FS)).collect();
        Geo { pts, radii }
    }

    fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut it = self.pts.iter().zip(&self.radii).map(|(p, r)| (p.0 - r, p.1 - r, p.0 + r, p.1 + r));
        let first = it.next()?;
        Some(it.fold(first, |a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))))
    }
}

fn layout_of(p: &Params) -> Layout {
    if p.s("layout").starts_with("Layers") { Layout::Layers } else { Layout::Circle }
}

fn snap(v: f64) -> f64 {
    (v / SNAP).round() * SNAP
}

impl Canvas {
    fn view(&self) -> View {
        self.view.unwrap_or(View { s: 1.0, c: (0.0, 0.0) })
    }

    fn s2w(&self, p: Pos2) -> World {
        self.view().to_world(self.rect, p)
    }

    fn w2s(&self, w: World) -> Pos2 {
        self.view().to_screen(self.rect, w)
    }

    fn scale(&self) -> f32 {
        self.view().s as f32
    }

    /// Screen position of a node's centre (tests aim the pointer with it).
    #[cfg(test)]
    pub fn node_screen(&self, p: &Params, name: &str) -> Option<Pos2> {
        let g = Graph::parse(p.s("spec")).ok()?;
        let i = g.index(name)?;
        Some(self.w2s(Geo::new(&g, layout_of(p)).pts[i]))
    }

    /// Screen position of the middle of edge `k`.
    #[cfg(test)]
    pub fn edge_screen(&self, p: &Params, k: usize) -> Option<Pos2> {
        let g = Graph::parse(p.s("spec")).ok()?;
        let geo = Geo::new(&g, layout_of(p));
        let e = g.edges.get(k)?;
        let (a, b) = (geo.pts[e.a], geo.pts[e.b]);
        Some(self.w2s(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)))
    }

    fn write(&mut self, p: &mut Params, g: &Graph) {
        let spec = g.to_spec();
        p.set_s("spec", &spec);
        self.last_spec = spec;
        self.note = None;
    }

    /// Saves every node's current place in the graph (so adding or removing one does not move the rest).
    fn pin_all(&self, g: &mut Graph, geo: &Geo) {
        for (i, n) in g.nodes.iter_mut().enumerate() {
            if n.pos.is_none() {
                n.pos = Some((snap(geo.pts[i].0 / CELL.0), snap(geo.pts[i].1 / CELL.1)));
            }
        }
    }

    fn fit(&mut self, geo: &Geo) {
        self.view = Some(View::fit(self.rect, geo.bounds()));
        self.auto_fit = Some(self.rect.size());
    }

    pub fn show(&mut self, ui: &mut Ui, p: &mut Params) -> Change {
        let mut change = Change::None;
        let spec = p.s("spec").to_string();
        let mut g = match Graph::parse(&spec) {
            Ok(g) => g,
            Err(e) => {
                ui.add_space(8.0);
                ui.label(theme::muted(format!("{e}\nFix the graph text in the panel and the canvas comes back.")).size(12.5));
                return Change::None;
            }
        };
        self.toolbar(ui, p, &mut g, &mut change);
        ui.add_space(4.0);
        let (rect, resp) = ui.allocate_exact_size(ui.available_size().max(vec2(160.0, 160.0)), Sense::click_and_drag());
        self.rect = rect;
        let mut geo = Geo::new(&g, layout_of(p));
        if self.view.is_none() || self.auto_fit.is_some_and(|sz| (sz - rect.size()).abs().max_elem() > 1.0 && self.drag.is_none()) {
            self.fit(&geo);
        } else if spec != self.last_spec && self.drag.is_none() {
            if let (Some(v), Some(b)) = (self.view, geo.bounds())
                && !v.shows(rect, b)
            {
                self.fit(&geo);
            }
            self.sel = match &self.sel {
                Sel::Node(n) if g.index(n).is_some() => self.sel.clone(),
                Sel::Edge(k) if *k < g.edges.len() => self.sel.clone(),
                _ => Sel::None,
            };
        }
        self.last_spec = spec;
        let ctx = ui.ctx().clone();
        let hover = resp.hover_pos();

        if let Some(hp) = hover {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let f = zoom as f64 * (scroll as f64 * 0.0018).exp();
            if (f - 1.0).abs() > 1e-4
                && let Some(v) = self.view.as_mut()
            {
                v.zoom_about(rect, hp, f);
                self.auto_fit = None;
            }
        }

        let (pressed, released, down, ptr) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_released(), i.pointer.primary_down(), i.pointer.interact_pos()));
        if ui.input(|i| i.pointer.secondary_pressed() || i.pointer.button_pressed(egui::PointerButton::Middle)) && resp.hovered() {
            self.drag = Some(Drag::Pan);
        }
        if pressed && resp.hovered() && self.rename.is_none()
            && let Some(pp) = ptr
        {
            self.press(p, &mut g, &geo, pp, &mut change);
            geo = Geo::new(&g, layout_of(p));
        }
        if matches!(self.drag, Some(Drag::Pan)) {
            let d = ui.input(|i| i.pointer.delta());
            if let Some(v) = self.view.as_mut() {
                v.pan_by(d);
                self.auto_fit = None;
            }
            if !down {
                self.drag = None;
            }
        } else if let Some(pp) = ptr.filter(|_| self.drag.is_some()) {
            self.dragging(p, &mut g, &geo, pp, released || !down, &mut change);
            geo = Geo::new(&g, layout_of(p));
        }
        if resp.double_clicked()
            && let Some(pp) = ptr
        {
            self.start_rename(&g, &geo, pp);
        }
        if !ctx.egui_wants_keyboard_input() && (resp.hovered() || self.sel != Sel::None) {
            let (del, esc) = ui.input(|i| (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace), i.key_pressed(Key::Escape)));
            if del {
                self.delete_sel(p, &mut g, &geo, &mut change);
                geo = Geo::new(&g, layout_of(p));
            }
            if esc {
                self.tool = Tool::Select;
                self.sel = Sel::None;
                self.drag = None;
                self.note = None;
            }
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        gc::draw_grid(&painter, rect, &self.view(), CELL.0 / 4.0);
        self.draw(&painter, &g, &geo, p);
        self.rename_box(ui, &mut g, &geo, p, &mut change);
        let hint = match (&self.note, self.tool) {
            (Some(n), _) => n.clone(),
            (None, Tool::Select) if g.nodes.is_empty() => "Choose Node, then click the canvas to start".to_string(),
            (None, Tool::Select) => "Drag to move. Double-click a node to rename, an edge to set its weight.".to_string(),
            (None, Tool::Node) => "Click the canvas to add a node (Esc to cancel)".to_string(),
            (None, Tool::Edge) => "Drag from one node to another to join them (Esc to cancel)".to_string(),
            (None, Tool::Arrow) => "Drag from a node to the node the arrow points to (Esc to cancel)".to_string(),
        };
        painter.text(rect.left_bottom() + vec2(8.0, -8.0), Align2::LEFT_BOTTOM, hint, FontId::proportional(11.5), if self.note.is_some() { theme::red() } else { theme::muted_col() });
        if self.drag.is_some() || change != Change::None {
            ctx.request_repaint();
        }
        change
    }

    fn toolbar(&mut self, ui: &mut Ui, p: &mut Params, g: &mut Graph, change: &mut Change) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 4.0);
            for t in [Tool::Select, Tool::Node, Tool::Edge, Tool::Arrow] {
                let tip = match t {
                    Tool::Select => "Select and move; drag empty space to pan (Esc)",
                    Tool::Node => "Click the canvas to add a node",
                    Tool::Edge => "Drag from a node to another to join them with an edge",
                    Tool::Arrow => "Drag from a node to another to draw a directed edge",
                };
                if ui.add(gc::seg_btn(t.name(), self.tool == t)).on_hover_text(tip).clicked() {
                    self.tool = t;
                    self.note = None;
                }
            }
            ui.add_space(6.0);
            if ui.add_enabled(self.sel != Sel::None, gc::seg_btn("Delete", false)).on_hover_text("Delete the selected node (and its edges) or edge (Delete)").clicked() {
                let geo = Geo::new(g, layout_of(p));
                self.delete_sel(p, g, &geo, change);
            }
            if ui.add(gc::seg_btn("Fit", false)).on_hover_text("Fit the whole graph in view").clicked() {
                let geo = Geo::new(g, layout_of(p));
                self.fit(&geo);
            }
        });
    }

    fn delete_sel(&mut self, p: &mut Params, g: &mut Graph, geo: &Geo, change: &mut Change) {
        match self.sel.clone() {
            Sel::None => return,
            Sel::Node(n) => {
                let Some(i) = g.index(&n) else { return };
                self.pin_all(g, geo);
                g.remove_node(i);
            }
            Sel::Edge(k) => g.remove_edge(k),
        }
        self.sel = Sel::None;
        self.write(p, g);
        *change = Change::Step;
    }

    fn node_at(&self, g: &Graph, geo: &Geo, pp: Pos2) -> Option<usize> {
        (0..g.nodes.len()).rev().find(|i| self.w2s(geo.pts[*i]).distance(pp) <= (geo.radii[*i] as f32 * self.scale()).max(8.0))
    }

    fn edge_at(&self, g: &Graph, geo: &Geo, pp: Pos2) -> Option<usize> {
        g.edges.iter().position(|e| gc::seg_dist(pp, self.w2s(geo.pts[e.a]), self.w2s(geo.pts[e.b])) <= EDGE_REACH)
    }

    fn press(&mut self, p: &mut Params, g: &mut Graph, geo: &Geo, pp: Pos2, change: &mut Change) {
        let w = self.s2w(pp);
        match self.tool {
            Tool::Node => {
                let before = g.clone();
                self.pin_all(g, geo);
                match g.push_node(Some((snap(w.0 / CELL.0), snap(w.1 / CELL.1)))) {
                    Ok(i) => {
                        self.sel = Sel::Node(g.nodes[i].name.clone());
                        self.tool = Tool::Select;
                        self.write(p, g);
                        *change = Change::Step;
                    }
                    Err(e) => {
                        *g = before;
                        self.note = Some(e);
                    }
                }
            }
            Tool::Edge | Tool::Arrow => {
                if let Some(i) = self.node_at(g, geo, pp) {
                    self.drag = Some(Drag::Link { from: g.nodes[i].name.clone(), directed: self.tool == Tool::Arrow, at: pp });
                    self.sel = Sel::Node(g.nodes[i].name.clone());
                }
            }
            Tool::Select => {
                if let Some(i) = self.node_at(g, geo, pp) {
                    let name = g.nodes[i].name.clone();
                    self.sel = Sel::Node(name.clone());
                    self.drag = Some(Drag::Move { name, from: w, orig: (geo.pts[i].0 / CELL.0, geo.pts[i].1 / CELL.1), moved: false });
                } else if let Some(k) = self.edge_at(g, geo, pp) {
                    self.sel = Sel::Edge(k);
                } else {
                    self.sel = Sel::None;
                    self.drag = Some(Drag::Pan);
                }
            }
        }
    }

    fn dragging(&mut self, p: &mut Params, g: &mut Graph, geo: &Geo, pp: Pos2, done: bool, change: &mut Change) {
        let w = self.s2w(pp);
        match self.drag.take() {
            Some(Drag::Move { name, from, orig, mut moved }) => {
                if moved || self.w2s(from).distance(pp) > 3.0 {
                    moved = true;
                    let target = (snap(orig.0 + (w.0 - from.0) / CELL.0), snap(orig.1 + (w.1 - from.1) / CELL.1));
                    if let Some(i) = g.index(&name)
                        && g.nodes[i].pos != Some(target)
                    {
                        g.nodes[i].pos = Some(target);
                        self.write(p, g);
                        *change = Change::Live;
                    }
                }
                if !done {
                    self.drag = Some(Drag::Move { name, from, orig, moved });
                }
            }
            Some(Drag::Link { from, directed, at: _ }) => {
                if done {
                    let (Some(a), Some(b)) = (g.index(&from), self.node_at(g, geo, pp)) else { return };
                    match g.add_edge(a, b, directed) {
                        Ok(k) => {
                            self.sel = Sel::Edge(k);
                            self.write(p, g);
                            *change = Change::Step;
                        }
                        Err(e) => self.note = Some(e),
                    }
                } else {
                    self.drag = Some(Drag::Link { from, directed, at: pp });
                }
            }
            other => self.drag = other,
        }
    }

    // ----- rename / weight -----

    fn start_rename(&mut self, g: &Graph, geo: &Geo, pp: Pos2) {
        if let Some(i) = self.node_at(g, geo, pp) {
            self.rename = Some(Rename { target: RenameTarget::Node(g.nodes[i].name.clone()), text: g.nodes[i].name.clone(), err: None, first: true });
        } else if let Some(k) = self.edge_at(g, geo, pp) {
            self.rename = Some(Rename { target: RenameTarget::Weight(k), text: g.edges[k].weight.map(crate::figures::chart::fmt).unwrap_or_default(), err: None, first: true });
        }
    }

    fn rename_box(&mut self, ui: &mut Ui, g: &mut Graph, geo: &Geo, p: &mut Params, change: &mut Change) {
        let (view, rect) = (self.view(), self.rect);
        let Some(r) = self.rename.as_mut() else { return };
        let at = match &r.target {
            RenameTarget::Node(n) => g.index(n).map(|i| (geo.pts[i].0, geo.pts[i].1 - geo.radii[i] - 30.0)),
            RenameTarget::Weight(k) => g.edges.get(*k).map(|e| ((geo.pts[e.a].0 + geo.pts[e.b].0) / 2.0, (geo.pts[e.a].1 + geo.pts[e.b].1) / 2.0 - 30.0)),
        };
        let Some(at) = at else {
            self.rename = None;
            return;
        };
        let pos = view.to_screen(rect, at);
        let (mut commit, mut cancel) = (false, false);
        let is_weight = matches!(r.target, RenameTarget::Weight(_));
        egui::Area::new(Id::new("graph_rename")).order(egui::Order::Foreground).fixed_pos(pos - vec2(60.0, 0.0)).show(ui.ctx(), |ui| {
            Frame::popup(ui.style()).inner_margin(6.0).show(ui, |ui| {
                let te = ui.add(TextEdit::singleline(&mut r.text).desired_width(120.0).hint_text(if is_weight { "Weight (empty for none)" } else { "Name" }));
                if r.first {
                    te.request_focus();
                    r.first = false;
                }
                if te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    commit = true;
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel = true;
                }
                if let Some(e) = &r.err {
                    ui.label(theme::muted(e.as_str()).size(11.0).color(theme::red()));
                }
            });
        });
        if cancel {
            self.rename = None;
        } else if commit {
            let (target, text) = (r.target.clone(), r.text.clone());
            let res = match &target {
                RenameTarget::Node(n) => g.index(n).map(|i| g.rename_node(i, &text)),
                RenameTarget::Weight(k) => {
                    let t = text.trim();
                    Some(if t.is_empty() { g.set_weight(*k, None) } else { t.parse::<f64>().map_err(|_| format!("\u{201c}{t}\u{201d} is not a number")).and_then(|w| g.set_weight(*k, Some(w))) })
                }
            };
            match res {
                Some(Ok(())) => {
                    if let RenameTarget::Node(_) = target {
                        self.sel = Sel::Node(text.trim().to_string());
                    }
                    self.rename = None;
                    self.write(p, g);
                    *change = Change::Step;
                }
                Some(Err(e)) => {
                    if let Some(r) = self.rename.as_mut() {
                        r.err = Some(e);
                        r.first = true;
                    }
                }
                None => self.rename = None,
            }
        }
    }

    // ----- drawing -----

    fn draw(&self, painter: &egui::Painter, g: &Graph, geo: &Geo, p: &Params) {
        let k = self.scale().clamp(0.6, 1.6);
        let show_w = p.b("weights");
        for (ei, e) in g.edges.iter().enumerate() {
            let selected = self.sel == Sel::Edge(ei);
            let col = if selected { SELECT } else { INK };
            let st = Stroke::new(2.0 * k, col);
            let (a, b) = (self.w2s(geo.pts[e.a]), self.w2s(geo.pts[e.b]));
            let (ra, rb) = (geo.radii[e.a] as f32 * self.scale(), geo.radii[e.b] as f32 * self.scale());
            let d = b - a;
            let len = d.length().max(1e-3);
            let u = d / len;
            if len <= ra + rb + 2.0 {
                continue;
            }
            let (s0, s1) = (a + u * ra, b - u * rb);
            if e.directed {
                let base = s1 - u * 11.0 * k;
                painter.line_segment([s0, base], st);
                let n = vec2(-u.y, u.x) * 4.5 * k;
                painter.add(egui::Shape::convex_polygon(vec![s1, base + n, base - n], col, Stroke::NONE));
            } else {
                painter.line_segment([s0, s1], st);
            }
            if let (Some(w), true) = (e.weight, show_w) {
                let mid = s0 + (s1 - s0) * 0.5;
                let text = crate::figures::chart::fmt(w);
                let r = Rect::from_center_size(mid, vec2(text.chars().count() as f32 * 7.5 + 8.0, 16.0) * k);
                painter.rect_filled(r, 2.0, Color32::WHITE);
                painter.text(mid, Align2::CENTER_CENTER, text, FontId::proportional(12.5 * k), col);
            }
        }
        if let Some(Drag::Link { from, directed, at }) = &self.drag
            && let Some(i) = g.index(from)
        {
            let a = self.w2s(geo.pts[i]);
            painter.extend(egui::Shape::dashed_line(&[a, *at], Stroke::new(2.0, SELECT), 6.0, 4.0));
            if *directed {
                painter.circle_filled(*at, 4.0, SELECT);
            }
            if let Some(j) = self.node_at(g, geo, *at) {
                painter.circle_stroke(self.w2s(geo.pts[j]), geo.radii[j] as f32 * self.scale() + 4.0, Stroke::new(2.0, SELECT));
            }
        }
        for (i, n) in g.nodes.iter().enumerate() {
            let selected = self.sel == Sel::Node(n.name.clone());
            let c = self.w2s(geo.pts[i]);
            let r = geo.radii[i] as f32 * self.scale();
            painter.circle(c, r, FILL, Stroke::new(2.0 * k, if selected { SELECT } else { INK }));
            painter.text(c, Align2::CENTER_CENTER, &n.name, FontId::proportional(FS as f32 * k), INK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::registry;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        canvas: Canvas,
        p: Params,
        last: Change,
    }

    fn harness(spec: &str) -> Harness<'static, State> {
        let def = registry().into_iter().find(|d| d.id == "graph-diagram").unwrap();
        let mut p = Params::defaults(&(def.params)());
        p.set_s("spec", spec);
        let mut h = crate::figuresui::ui_test::harness(
            |ui, s: &mut State| {
                let ch = s.canvas.show(ui, &mut s.p);
                if ch != Change::None {
                    s.last = ch;
                }
            },
            State { canvas: Canvas::default(), p, last: Change::None },
        );
        h.run();
        h
    }

    fn mv(h: &mut Harness<'static, State>, to: Pos2) {
        h.event(egui::Event::PointerMoved(to));
        h.step();
        h.step();
    }

    fn press(h: &mut Harness<'static, State>, at: Pos2, down: bool) {
        h.event(egui::Event::PointerMoved(at));
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: down, modifiers: Default::default() });
        h.step();
        h.step();
    }

    fn drag(h: &mut Harness<'static, State>, from: Pos2, to: Pos2) {
        mv(h, from);
        press(h, from, true);
        mv(h, from + (to - from) * 0.5);
        mv(h, to);
        press(h, to, false);
        h.step();
    }

    fn click(h: &mut Harness<'static, State>, at: Pos2) {
        mv(h, at);
        press(h, at, true);
        press(h, at, false);
    }

    fn wait(h: &mut Harness<'static, State>) {
        for _ in 0..40 {
            h.step();
        }
    }

    fn graph(h: &Harness<'static, State>) -> Graph {
        Graph::parse(h.state().p.s("spec")).unwrap()
    }

    const SPEC: &str = "nodes A, B, C\nA - B";

    #[test]
    fn dragging_a_node_moves_it_and_pins_its_place_in_the_spec() {
        let mut h = harness(SPEC);
        let a = h.state().canvas.node_screen(&h.state().p, "A").unwrap();
        drag(&mut h, a, a + vec2(0.0, 90.0));
        let g = graph(&h);
        assert!(g.nodes[0].pos.is_some() && g.nodes[1].pos.is_none(), "only the moved node is pinned");
        assert!(h.state().p.s("spec").contains("node A @"));
        assert_eq!(h.state().last, Change::Live);
        assert_eq!(g.edges.len(), 1, "its edge is untouched");
    }

    #[test]
    fn the_node_tool_adds_a_node_and_pins_the_others_where_they_were() {
        let mut h = harness(SPEC);
        let before: Vec<Pos2> = ["A", "B", "C"].iter().map(|n| h.state().canvas.node_screen(&h.state().p, n).unwrap()).collect();
        h.state_mut().canvas.tool = Tool::Node;
        let at = h.state().canvas.rect.center() + vec2(200.0, 100.0);
        click(&mut h, at);
        let g = graph(&h);
        assert_eq!(g.nodes.len(), 4);
        assert_eq!(g.nodes[3].name, "D");
        assert!(g.nodes.iter().all(|n| n.pos.is_some()));
        let after: Vec<Pos2> = ["A", "B", "C"].iter().map(|n| h.state().canvas.node_screen(&h.state().p, n).unwrap()).collect();
        for (b, a) in before.iter().zip(&after) {
            assert!(b.distance(*a) < 30.0, "pinning only snaps to the grid, it does not reshuffle: {b:?} -> {a:?}");
        }
        assert_eq!(h.state().canvas.sel, Sel::Node("D".into()));
        assert_eq!(h.state().canvas.tool, Tool::Select);
    }

    #[test]
    fn the_edge_and_arrow_tools_join_two_nodes() {
        let mut h = harness(SPEC);
        h.state_mut().canvas.tool = Tool::Edge;
        let (b, c) = (h.state().canvas.node_screen(&h.state().p, "B").unwrap(), h.state().canvas.node_screen(&h.state().p, "C").unwrap());
        drag(&mut h, b, c);
        let g = graph(&h);
        assert_eq!(g.edges.len(), 2);
        assert!(!g.edges[1].directed && (g.edges[1].a, g.edges[1].b) == (1, 2));
        assert_eq!(h.state().last, Change::Step);
        // the same edge again is refused with a note
        drag(&mut h, c, b);
        assert_eq!(graph(&h).edges.len(), 2);
        assert!(h.state().canvas.note.as_deref().is_some_and(|n| n.contains("already")), "{:?}", h.state().canvas.note);
        // an arrow from A to C
        h.state_mut().canvas.tool = Tool::Arrow;
        let a = h.state().canvas.node_screen(&h.state().p, "A").unwrap();
        drag(&mut h, a, c);
        let g = graph(&h);
        assert!(g.edges[2].directed && (g.edges[2].a, g.edges[2].b) == (0, 2));
        // dropping on empty canvas joins nothing
        let empty = h.state().canvas.rect.center() + vec2(300.0, -200.0);
        drag(&mut h, a, empty);
        assert_eq!(graph(&h).edges.len(), 3);
    }

    #[test]
    fn delete_removes_a_node_with_its_edges_or_just_an_edge() {
        let mut h = harness("nodes A, B, C\nA - B\nB - C");
        let a = h.state().canvas.node_screen(&h.state().p, "B").unwrap();
        click(&mut h, a);
        assert_eq!(h.state().canvas.sel, Sel::Node("B".into()));
        h.key_press(Key::Delete);
        h.run();
        let g = graph(&h);
        assert_eq!(g.nodes.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["A", "C"]);
        assert!(g.edges.is_empty(), "both its edges went");
        // an edge alone
        let mut h = harness("nodes A, B, C\nA - B\nB - C");
        let e = h.state().canvas.edge_screen(&h.state().p, 0).unwrap();
        click(&mut h, e);
        assert_eq!(h.state().canvas.sel, Sel::Edge(0));
        h.key_press(Key::Delete);
        h.run();
        let g = graph(&h);
        assert_eq!((g.nodes.len(), g.edges.len()), (3, 1));
    }

    #[test]
    fn double_click_renames_a_node_and_sets_an_edge_weight() {
        let mut h = harness(SPEC);
        let a = h.state().canvas.node_screen(&h.state().p, "A").unwrap();
        click(&mut h, a);
        click(&mut h, a);
        assert!(h.state().canvas.rename.is_some(), "double-click opens the name box");
        h.get_by_role(egui::accesskit::Role::TextInput).focus();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
        h.get_by_role(egui::accesskit::Role::TextInput).type_text("X");
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(graph(&h).nodes[0].name, "X");
        wait(&mut h);
        let e = h.state().canvas.edge_screen(&h.state().p, 0).unwrap();
        click(&mut h, e);
        click(&mut h, e);
        assert!(matches!(h.state().canvas.rename.as_ref().map(|r| &r.target), Some(RenameTarget::Weight(0))));
        h.get_by_role(egui::accesskit::Role::TextInput).focus();
        h.run();
        h.get_by_role(egui::accesskit::Role::TextInput).type_text("7.5");
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(graph(&h).edges[0].weight, Some(7.5));
    }

    #[test]
    fn a_text_edit_outside_the_canvas_is_picked_up_and_a_bad_spec_shows_why() {
        let mut h = harness(SPEC);
        h.state_mut().p.set_s("spec", "nodes A, B, C, D\nA - D");
        h.run();
        assert!(h.state().canvas.node_screen(&h.state().p, "D").is_some());
        h.state_mut().p.set_s("spec", "A - A");
        h.run();
        assert!(h.query_by_label_contains("itself").is_some() || h.query_by_label_contains("Fix the graph").is_some());
    }

    #[test]
    fn hostile_graphs_never_panic_the_canvas() {
        for spec in ["", "node A @ 1000,-1000", "A - B\nnode A @ 0,0\nnode B @ 0,0", "nodes A"] {
            let mut h = harness(spec);
            for t in [Tool::Select, Tool::Node, Tool::Edge, Tool::Arrow] {
                h.state_mut().canvas.tool = t;
                let at = h.state().canvas.rect.center();
                click(&mut h, at);
                h.key_press(Key::Delete);
                h.run();
            }
            assert!(Graph::parse(h.state().p.s("spec")).is_ok(), "{spec:?} -> {:?}", h.state().p.s("spec"));
        }
    }
}
