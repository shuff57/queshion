//! Canvas editor for the logic-circuit figure: drag gates around, wire an output pin to an
//! input pin, click an input to flip its test value, add and delete parts, rename inputs and
//! outputs. The canvas owns no circuit: each frame it reads the figure's `spec` param through
//! `Circuit::parse` and writes an edit back with `to_spec`, so the typed spec, the expression
//! source, presets, undo and the exports all stay on the one text the figure always had. Every
//! edit that would leave an invalid circuit (a loop, too many inputs) is refused with a note.
use egui::{Align2, Color32, CornerRadius, FontId, Frame, Id, Key, Pos2, Rect, Sense, Stroke, TextEdit, Ui, Vec2, vec2};

use super::composite_canvas::Change;
use super::graph_canvas::{self as gc, View, World};
use crate::figures::computing::logic_circuit::{dim, pins, Dim, BUB, COL_W, ROW_H};
use crate::figures::logic::{Circuit, Gate, Kind};
use crate::figures::Params;
use crate::theme;

const INK: Color32 = Color32::from_rgb(0x11, 0x18, 0x27);
const LIT_ON: Color32 = Color32::from_rgb(0x1a, 0x8a, 0x3a);
const LIT_OFF: Color32 = Color32::from_rgb(0x6b, 0x72, 0x80);
const SELECT: Color32 = Color32::from_rgb(0x2f, 0x6f, 0xe0);
/// Positions snap to a quarter of a grid cell.
const SNAP: f64 = 0.25;
/// How near (screen points) the pointer must be to grab a pin or a wire.
const PIN_REACH: f32 = 9.0;
const WIRE_REACH: f32 = 6.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Tool {
    #[default]
    Select,
    Input,
    Gate(Gate),
    Output,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Input => "Input",
            Tool::Gate(g) => g.name(),
            Tool::Output => "Output",
        }
    }
}

/// A wire: the gate (or output) it arrives at and which pin.
#[derive(Clone, Debug, PartialEq)]
pub struct Wire {
    pub to: String,
    pub is_output: bool,
    pub pin: usize,
}

/// What is selected, by name (a name survives the spec being read back in a new order).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Sel {
    #[default]
    None,
    Node(String),
    Out(String),
    Wire(Wire),
}

enum Drag {
    Pan,
    /// Moving a part: where the press began (world), the part's grid position then. An input's
    /// pin is also its body, so a press there is a click (flip), a move or, once it travels, a wire.
    Move { part: Sel, from: World, orig: (f64, f64), moved: bool, wire_ok: bool },
    /// Pulling a wire from the output pin of a node.
    Wire { src: String, at: Pos2 },
}

struct Rename {
    target: Sel,
    text: String,
    err: Option<String>,
    first: bool,
}

pub struct Canvas {
    view: Option<View>,
    pub sel: Sel,
    pub tool: Tool,
    drag: Option<Drag>,
    /// The last refusal ("That wire would make a loop"), shown until the next edit.
    pub note: Option<String>,
    rename: Option<Rename>,
    /// Whether the view is still the automatic fit (the user has not panned or zoomed), and the
    /// canvas size it was fitted to: a window that settles into its real size refits.
    auto_fit: Option<Vec2>,
    /// The spec the canvas last saw or wrote (an outside change may refit the view).
    last_spec: String,
    /// Canvas rectangle of the last frame (tests map world points to the screen with it).
    pub rect: Rect,
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas { view: None, sel: Sel::None, tool: Tool::Select, drag: None, note: None, rename: None, auto_fit: None, last_spec: String::new(), rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)) }
    }
}

/// Where everything sits in world pixels for one frame.
struct Geo {
    nodes: Vec<NodeGeo>,
    outs: Vec<OutGeo>,
}

struct NodeGeo {
    c: World,
    dm: Option<Dim>,
    /// Input pins (gates) and the output pin, world pixels.
    ins: Vec<World>,
    out: World,
    /// x0, y0, x1, y1 of the part for hit testing.
    bounds: (f64, f64, f64, f64),
}

struct OutGeo {
    c: World,
    bounds: (f64, f64, f64, f64),
}

fn name_w(s: &str) -> f64 {
    s.chars().count() as f64 * 8.5
}

impl Geo {
    fn new(c: &Circuit, iec: bool) -> Geo {
        let (npos, opos, _) = c.layout();
        let w = |g: (f64, f64)| (g.0 * COL_W, g.1 * ROW_H);
        let nodes = c
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let (x, y) = w(npos[i]);
                match &n.kind {
                    Kind::Gate(g, a) => {
                        let dm = dim(*g, a.len(), iec);
                        let (ins, out) = pins(&dm, a.len(), x, y);
                        let half = dm.total() / 2.0 + dm.back;
                        NodeGeo { c: (x, y), ins, out, bounds: (x - half, y - dm.h / 2.0, x + half, y + dm.h / 2.0), dm: Some(dm) }
                    }
                    Kind::Input => NodeGeo { c: (x, y), dm: None, ins: Vec::new(), out: (x, y), bounds: (x - 14.0 - name_w(&n.name), y - 14.0, x + 8.0, y + 14.0) },
                }
            })
            .collect();
        let outs = c
            .outputs
            .iter()
            .enumerate()
            .map(|(k, o)| {
                let (x, y) = w(opos[k]);
                OutGeo { c: (x, y), bounds: (x - 8.0, y - 14.0, x + 14.0 + name_w(&o.name), y + 14.0) }
            })
            .collect();
        Geo { nodes, outs }
    }

    fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut it = self.nodes.iter().map(|n| n.bounds).chain(self.outs.iter().map(|o| o.bounds));
        let first = it.next()?;
        Some(it.fold(first, |a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))))
    }
}

/// One drawn wire: where it starts and ends in world pixels, the corners between, who feeds it.
struct WireGeo {
    pts: Vec<World>,
    src: usize,
    to: Wire,
}

fn wires(c: &Circuit, g: &Geo) -> Vec<WireGeo> {
    let route = |a: World, b: World, src: usize| -> Vec<World> {
        if (a.1 - b.1).abs() < 0.01 {
            return vec![a, b];
        }
        let lane = ((src % 5) as f64 - 2.0) * 6.0;
        let mx = ((a.0 + b.0) / 2.0 + lane).clamp(a.0.min(b.0), a.0.max(b.0));
        vec![a, (mx, a.1), (mx, b.1), b]
    };
    let mut out = Vec::new();
    for (i, n) in c.nodes.iter().enumerate() {
        if let Kind::Gate(_, a) = &n.kind {
            for (k, s) in a.iter().enumerate() {
                out.push(WireGeo { pts: route(g.nodes[*s].out, g.nodes[i].ins[k], *s), src: *s, to: Wire { to: n.name.clone(), is_output: false, pin: k } });
            }
        }
    }
    for (k, o) in c.outputs.iter().enumerate() {
        out.push(WireGeo { pts: route(g.nodes[o.src].out, g.outs[k].c, o.src), src: o.src, to: Wire { to: o.name.clone(), is_output: true, pin: 0 } });
    }
    out
}

fn snap(v: f64) -> f64 {
    (v / SNAP).round() * SNAP
}

impl Canvas {
    fn s2w(&self, p: Pos2) -> World {
        self.view.unwrap_or(View { s: 1.0, c: (0.0, 0.0) }).to_world(self.rect, p)
    }

    fn w2s(&self, w: World) -> Pos2 {
        self.view.unwrap_or(View { s: 1.0, c: (0.0, 0.0) }).to_screen(self.rect, w)
    }

    fn scale(&self) -> f32 {
        self.view.map_or(1.0, |v| v.s as f32)
    }

    // Screen positions of parts: tests aim the pointer with these.
    /// Screen position of a node's centre.
    #[cfg(test)]
    pub fn node_screen(&self, p: &Params, name: &str) -> Option<Pos2> {
        let c = Circuit::parse(p.s("spec")).ok()?;
        let g = Geo::new(&c, p.s("symbols").starts_with("Boxes"));
        let i = c.nodes.iter().position(|n| n.name == name)?;
        let n = &g.nodes[i];
        Some(self.w2s(if n.dm.is_some() { n.c } else { (n.c.0 - 6.0, n.c.1) }))
    }

    /// Screen position of a node's output pin.
    #[cfg(test)]
    pub fn out_pin_screen(&self, p: &Params, name: &str) -> Option<Pos2> {
        let c = Circuit::parse(p.s("spec")).ok()?;
        let g = Geo::new(&c, p.s("symbols").starts_with("Boxes"));
        let i = c.nodes.iter().position(|n| n.name == name)?;
        Some(self.w2s(g.nodes[i].out))
    }

    /// Screen position of input pin `pin` of gate `name`.
    #[cfg(test)]
    pub fn in_pin_screen(&self, p: &Params, name: &str, pin: usize) -> Option<Pos2> {
        let c = Circuit::parse(p.s("spec")).ok()?;
        let g = Geo::new(&c, p.s("symbols").starts_with("Boxes"));
        let i = c.nodes.iter().position(|n| n.name == name)?;
        g.nodes[i].ins.get(pin).map(|w| self.w2s(*w))
    }

    fn write(&mut self, p: &mut Params, c: &Circuit) {
        let spec = c.to_spec();
        p.set_s("spec", &spec);
        self.last_spec = spec;
        self.note = None;
    }

    fn fit(&mut self, g: &Geo) {
        self.view = Some(View::fit(self.rect, g.bounds()));
        self.auto_fit = Some(self.rect.size());
    }

    /// The canvas: toolbar, then the drawing area. Edits go straight into `p`.
    pub fn show(&mut self, ui: &mut Ui, p: &mut Params) -> Change {
        let mut change = Change::None;
        let spec = p.s("spec").to_string();
        let mut circ = match Circuit::parse(&spec) {
            Ok(c) => c,
            Err(e) => {
                ui.add_space(8.0);
                ui.label(theme::muted(format!("{e}\nFix the circuit text in the panel and the canvas comes back.")).size(12.5));
                return Change::None;
            }
        };
        let iec = p.s("symbols").starts_with("Boxes");
        self.toolbar(ui, p, &mut circ, &mut change);
        ui.add_space(4.0);
        let (rect, resp) = ui.allocate_exact_size(ui.available_size().max(vec2(160.0, 160.0)), Sense::click_and_drag());
        self.rect = rect;
        let mut geo = Geo::new(&circ, iec);
        if self.view.is_none() || self.auto_fit.is_some_and(|sz| (sz - rect.size()).abs().max_elem() > 1.0 && self.drag.is_none()) {
            self.fit(&geo);
        } else if spec != self.last_spec && self.drag.is_none() {
            // the text changed outside the canvas: keep the view unless the circuit left it
            if let (Some(v), Some(b)) = (self.view, geo.bounds())
                && !v.shows(rect, b)
            {
                self.fit(&geo);
            }
            self.sel = self.valid_sel(&circ);
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
        if (ui.input(|i| i.pointer.secondary_pressed() || i.pointer.button_pressed(egui::PointerButton::Middle))) && resp.hovered() {
            self.drag = Some(Drag::Pan);
        }
        if pressed && resp.hovered() && self.rename.is_none()
            && let Some(pp) = ptr
        {
            self.press(p, &mut circ, &geo, pp, &mut change);
            geo = Geo::new(&circ, iec);
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
            self.dragging(p, &mut circ, &geo, pp, released || !down, &mut change);
            geo = Geo::new(&circ, iec);
        }
        if resp.double_clicked()
            && let Some(pp) = ptr
        {
            self.start_rename(&circ, &geo, pp);
        }
        if !ctx.egui_wants_keyboard_input() && (resp.hovered() || self.sel != Sel::None) {
            self.keys(ui, p, &mut circ, &mut change);
            geo = Geo::new(&circ, iec);
        }

        // ---- draw ----
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        if let Some(v) = self.view {
            gc::draw_grid(&painter, rect, &v, COL_W / 4.0);
        }
        self.draw(&painter, &circ, &geo, p, hover);
        self.rename_box(ui, &mut circ, &geo, p, &mut change);
        self.status(ui, &painter, &circ);
        if self.drag.is_some() || change != Change::None {
            ctx.request_repaint();
        }
        change
    }

    fn valid_sel(&self, c: &Circuit) -> Sel {
        match &self.sel {
            Sel::Node(n) if c.nodes.iter().any(|x| &x.name == n) => self.sel.clone(),
            Sel::Out(n) if c.outputs.iter().any(|x| &x.name == n) => self.sel.clone(),
            Sel::Wire(w) if c.nodes.iter().any(|x| x.name == w.to) || c.outputs.iter().any(|x| x.name == w.to) => self.sel.clone(),
            _ => Sel::None,
        }
    }

    fn toolbar(&mut self, ui: &mut Ui, p: &mut Params, c: &mut Circuit, change: &mut Change) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 4.0);
            let mut tools = vec![Tool::Select, Tool::Input];
            tools.extend(Gate::ALL.into_iter().map(Tool::Gate));
            tools.push(Tool::Output);
            for t in tools {
                let tip = match t {
                    Tool::Select => "Select and move parts; drag from an output pin to wire; click an input to flip it; drag empty space to pan (Esc)".to_string(),
                    Tool::Input => "Click the canvas to add an input".to_string(),
                    Tool::Gate(g) => format!("Click the canvas to add a {} gate. It starts wired to the selected part, or to the first inputs", g.name()),
                    Tool::Output => "Click the canvas to add an output reading the selected part".to_string(),
                };
                if ui.add(gc::seg_btn(t.name(), self.tool == t)).on_hover_text(tip).clicked() {
                    self.tool = t;
                    self.note = None;
                }
            }
            ui.add_space(6.0);
            let has = self.sel != Sel::None;
            if ui.add_enabled(has, gc::seg_btn("Delete", false)).on_hover_text("Delete the selected part or wire (Delete)").clicked() {
                self.delete_sel(p, c, change);
            }
            if ui.add(gc::seg_btn("Fit", false)).on_hover_text("Fit the whole circuit in view").clicked() {
                let g = Geo::new(c, p.s("symbols").starts_with("Boxes"));
                self.fit(&g);
            }
        });
    }

    fn keys(&mut self, ui: &mut Ui, p: &mut Params, c: &mut Circuit, change: &mut Change) {
        let (del, esc) = ui.input(|i| (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace), i.key_pressed(Key::Escape)));
        if del {
            self.delete_sel(p, c, change);
        }
        if esc {
            self.tool = Tool::Select;
            self.sel = Sel::None;
            self.drag = None;
            self.note = None;
        }
    }

    fn delete_sel(&mut self, p: &mut Params, c: &mut Circuit, change: &mut Change) {
        let idx = |n: &str| c.nodes.iter().position(|x| x.name == n);
        let gone = match self.sel.clone() {
            Sel::None => return,
            Sel::Node(n) => idx(&n).map(|i| c.delete_nodes(&[i])),
            Sel::Out(n) => c.outputs.iter().position(|o| o.name == n).map(|k| {
                c.delete_output(k);
                0
            }),
            Sel::Wire(w) if w.is_output => c.outputs.iter().position(|o| o.name == w.to).map(|k| {
                c.delete_output(k);
                0
            }),
            Sel::Wire(w) => idx(&w.to).map(|i| c.disconnect(i, w.pin)),
        };
        if gone.is_some() {
            self.sel = Sel::None;
            self.write(p, c);
            *change = Change::Step;
        }
    }

    // ----- pointer -----

    fn press(&mut self, p: &mut Params, c: &mut Circuit, geo: &Geo, pp: Pos2, change: &mut Change) {
        let w = self.s2w(pp);
        if self.tool != Tool::Select {
            self.place(p, c, w, change);
            return;
        }
        // a gate's output pin starts a wire (an input's pin is its terminal: see below)
        if let Some(i) = (0..c.nodes.len()).find(|i| geo.nodes[*i].dm.is_some() && self.w2s(geo.nodes[*i].out).distance(pp) <= PIN_REACH) {
            self.drag = Some(Drag::Wire { src: c.nodes[i].name.clone(), at: pp });
            self.sel = Sel::Node(c.nodes[i].name.clone());
            return;
        }
        // a part: gates by their body, inputs and outputs by terminal and name
        let inside = |b: &(f64, f64, f64, f64)| w.0 >= b.0 && w.0 <= b.2 && w.1 >= b.1 && w.1 <= b.3;
        if let Some(k) = (0..c.outputs.len()).find(|k| inside(&geo.outs[*k].bounds)) {
            let o = &c.outputs[k];
            let part = Sel::Out(o.name.clone());
            self.sel = part.clone();
            self.drag = Some(Drag::Move { part, from: w, orig: (geo.outs[k].c.0 / COL_W, geo.outs[k].c.1 / ROW_H), moved: false, wire_ok: false });
            return;
        }
        if let Some(i) = (0..c.nodes.len()).rev().find(|i| inside(&geo.nodes[*i].bounds)) {
            let part = Sel::Node(c.nodes[i].name.clone());
            self.sel = part.clone();
            let wire_ok = geo.nodes[i].dm.is_none() && self.w2s(geo.nodes[i].out).distance(pp) <= PIN_REACH;
            self.drag = Some(Drag::Move { part, from: w, orig: (geo.nodes[i].c.0 / COL_W, geo.nodes[i].c.1 / ROW_H), moved: false, wire_ok });
            return;
        }
        // a wire
        let hit = wires(c, geo).into_iter().find(|wg| wg.pts.windows(2).any(|s| gc::seg_dist(pp, self.w2s(s[0]), self.w2s(s[1])) <= WIRE_REACH));
        if let Some(wg) = hit {
            self.sel = Sel::Wire(wg.to);
            return;
        }
        self.sel = Sel::None;
        self.drag = Some(Drag::Pan);
    }

    fn dragging(&mut self, p: &mut Params, c: &mut Circuit, geo: &Geo, pp: Pos2, done: bool, change: &mut Change) {
        let w = self.s2w(pp);
        match self.drag.take() {
            Some(Drag::Move { part, from, orig, mut moved, wire_ok }) => {
                if wire_ok && !moved && !done && self.w2s(from).distance(pp) > 6.0 {
                    if let Sel::Node(n) = &part {
                        self.drag = Some(Drag::Wire { src: n.clone(), at: pp });
                    }
                    return;
                }
                let d = ((w.0 - from.0) / COL_W, (w.1 - from.1) / ROW_H);
                let target = (snap(orig.0 + d.0), snap(orig.1 + d.1));
                if moved || self.w2s(from).distance(pp) > 3.0 {
                    moved = true;
                    let changed = match &part {
                        Sel::Node(n) => c.nodes.iter_mut().find(|x| &x.name == n).map(|x| std::mem::replace(&mut x.pos, Some(target)) != Some(target)),
                        Sel::Out(n) => c.outputs.iter_mut().find(|x| &x.name == n).map(|x| std::mem::replace(&mut x.pos, Some(target)) != Some(target)),
                        _ => None,
                    };
                    if changed == Some(true) {
                        self.write(p, c);
                        *change = Change::Live;
                    }
                }
                if done {
                    if !moved
                        && let Sel::Node(n) = &part
                        && let Some(i) = c.nodes.iter().position(|x| &x.name == n)
                        && matches!(c.nodes[i].kind, Kind::Input)
                    {
                        self.flip_input(p, c, i);
                        *change = Change::Step;
                    }
                } else {
                    self.drag = Some(Drag::Move { part, from, orig, moved, wire_ok });
                }
            }
            Some(Drag::Wire { src, at: _ }) => {
                if done {
                    self.drop_wire(p, c, geo, &src, pp, change);
                } else {
                    self.drag = Some(Drag::Wire { src, at: pp });
                }
            }
            other => self.drag = other,
        }
    }

    /// Flips input `i` in the test row (`try` param): the wires light with it.
    fn flip_input(&mut self, p: &mut Params, c: &Circuit, i: usize) {
        let ins = c.inputs();
        let Some(k) = ins.iter().position(|x| *x == i) else { return };
        let mut bits: Vec<bool> = p.s("try").trim().chars().filter(|ch| matches!(ch, '0' | '1')).map(|ch| ch == '1').collect();
        bits.resize(ins.len(), false);
        bits[k] = !bits[k];
        let text: String = bits.iter().map(|b| if *b { '1' } else { '0' }).collect();
        p.set_s("try", &text);
    }

    fn drop_wire(&mut self, p: &mut Params, c: &mut Circuit, geo: &Geo, src: &str, pp: Pos2, change: &mut Change) {
        let Some(s) = c.nodes.iter().position(|n| n.name == src) else { return };
        let before = c.clone();
        let mut result: Option<Result<(), String>> = None;
        // an input pin of a gate
        for i in 0..c.nodes.len() {
            if let Some(k) = geo.nodes[i].ins.iter().position(|w| self.w2s(*w).distance(pp) <= PIN_REACH + 3.0) {
                result = Some(c.set_pin(i, k, s));
                break;
            }
        }
        // the body of a gate: the nearest pin, or a new one if it has room
        if result.is_none() {
            let w = self.s2w(pp);
            if let Some(i) = (0..c.nodes.len()).find(|i| geo.nodes[*i].dm.is_some() && w.0 >= geo.nodes[*i].bounds.0 && w.0 <= geo.nodes[*i].bounds.2 && w.1 >= geo.nodes[*i].bounds.1 && w.1 <= geo.nodes[*i].bounds.3) {
                let n_in = geo.nodes[i].ins.len();
                let full = matches!(&c.nodes[i].kind, Kind::Gate(g, a) if a.len() >= g.arity().1);
                let pin = if full {
                    (0..n_in).min_by(|a, b| self.w2s(geo.nodes[i].ins[*a]).distance(pp).total_cmp(&self.w2s(geo.nodes[i].ins[*b]).distance(pp))).unwrap_or(0)
                } else {
                    n_in
                };
                result = Some(c.set_pin(i, pin, s));
            }
        }
        // an output terminal
        if result.is_none()
            && let Some(k) = (0..c.outputs.len()).find(|k| self.w2s(geo.outs[*k].c).distance(pp) <= PIN_REACH + 6.0)
        {
            result = Some(c.set_output_src(k, s));
        }
        // empty canvas: a new output there
        if result.is_none() && self.rect.contains(pp) && self.w2s(geo.nodes[s].out).distance(pp) > 20.0 {
            let w = self.s2w(pp);
            result = Some(c.add_output(s, Some((snap(w.0 / COL_W), snap(w.1 / ROW_H)))).map(|k| {
                self.sel = Sel::Out(c.outputs[k].name.clone());
            }));
        }
        match result {
            Some(Ok(())) if *c != before => {
                self.write(p, c);
                *change = Change::Step;
            }
            Some(Err(e)) => {
                *c = before;
                self.note = Some(e);
            }
            _ => *c = before,
        }
    }

    fn place(&mut self, p: &mut Params, c: &mut Circuit, w: World, change: &mut Change) {
        let pos = Some((snap(w.0 / COL_W), snap(w.1 / ROW_H)));
        let before = c.clone();
        let sel_node = match &self.sel {
            Sel::Node(n) => c.nodes.iter().position(|x| &x.name == n),
            _ => None,
        };
        let res: Result<Sel, String> = match self.tool {
            Tool::Select => return,
            Tool::Input => c.add_input(pos).map(|i| Sel::Node(c.nodes[i].name.clone())),
            Tool::Gate(g) => {
                let (lo, _) = g.arity();
                let mut srcs: Vec<usize> = sel_node.into_iter().collect();
                srcs.extend(c.inputs().into_iter().filter(|i| Some(*i) != sel_node));
                if srcs.is_empty() {
                    Err("Add an input first: a gate has to read something".into())
                } else {
                    let first = srcs.clone();
                    let srcs: Vec<usize> = (0..lo).map(|k| first[k % first.len()]).collect();
                    c.add_gate(g, srcs, pos).map(|i| Sel::Node(c.nodes[i].name.clone()))
                }
            }
            Tool::Output => {
                let src = sel_node.or_else(|| (0..c.nodes.len()).rev().find(|i| matches!(c.nodes[*i].kind, Kind::Gate(..)))).or_else(|| c.inputs().last().copied());
                match src {
                    None => Err("Add an input or a gate for the output to read".into()),
                    Some(s) => c.add_output(s, pos).map(|k| Sel::Out(c.outputs[k].name.clone())),
                }
            }
        };
        match res {
            Ok(s) => {
                self.sel = s;
                self.tool = Tool::Select;
                self.write(p, c);
                *change = Change::Step;
            }
            Err(e) => {
                *c = before;
                self.note = Some(e);
            }
        }
    }

    // ----- rename -----

    fn start_rename(&mut self, c: &Circuit, geo: &Geo, pp: Pos2) {
        let w = self.s2w(pp);
        let inside = |b: &(f64, f64, f64, f64)| w.0 >= b.0 && w.0 <= b.2 && w.1 >= b.1 && w.1 <= b.3;
        if let Some(k) = (0..c.outputs.len()).find(|k| inside(&geo.outs[*k].bounds)) {
            self.rename = Some(Rename { target: Sel::Out(c.outputs[k].name.clone()), text: c.outputs[k].name.clone(), err: None, first: true });
        } else if let Some(i) = (0..c.nodes.len()).find(|i| inside(&geo.nodes[*i].bounds)) {
            self.rename = Some(Rename { target: Sel::Node(c.nodes[i].name.clone()), text: c.nodes[i].name.clone(), err: None, first: true });
        }
    }

    fn rename_box(&mut self, ui: &mut Ui, c: &mut Circuit, geo: &Geo, p: &mut Params, change: &mut Change) {
        let Some(r) = self.rename.as_mut() else { return };
        let at = match &r.target {
            Sel::Node(n) => c.nodes.iter().position(|x| &x.name == n).map(|i| geo.nodes[i].c),
            Sel::Out(n) => c.outputs.iter().position(|x| &x.name == n).map(|k| geo.outs[k].c),
            _ => None,
        };
        let Some(at) = at else {
            self.rename = None;
            return;
        };
        let pos = {
            let v = self.view.unwrap_or(View { s: 1.0, c: (0.0, 0.0) });
            v.to_screen(self.rect, (at.0, at.1 - 34.0))
        };
        let (mut commit, mut cancel) = (false, false);
        egui::Area::new(Id::new("logic_rename")).order(egui::Order::Foreground).fixed_pos(pos - vec2(60.0, 0.0)).show(ui.ctx(), |ui| {
            Frame::popup(ui.style()).inner_margin(6.0).show(ui, |ui| {
                let te = ui.add(TextEdit::singleline(&mut r.text).desired_width(120.0).hint_text("Name"));
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
                Sel::Node(n) => c.nodes.iter().position(|x| x.name == *n).map(|i| c.rename_node(i, &text)),
                Sel::Out(n) => c.outputs.iter().position(|x| x.name == *n).map(|k| c.rename_output(k, &text)),
                _ => None,
            };
            match res {
                Some(Ok(())) => {
                    self.sel = match target {
                        Sel::Node(_) => Sel::Node(text.trim().to_string()),
                        _ => Sel::Out(text.trim().to_string()),
                    };
                    self.rename = None;
                    self.write(p, c);
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

    fn draw(&self, painter: &egui::Painter, c: &Circuit, geo: &Geo, p: &Params, hover: Option<Pos2>) {
        let iec = p.s("symbols").starts_with("Boxes");
        let k = self.scale();
        let bits: Vec<bool> = p.s("try").trim().chars().filter(|ch| matches!(ch, '0' | '1')).map(|ch| ch == '1').collect();
        let lit = !bits.is_empty() && !c.inputs().is_empty();
        let ins: Vec<bool> = (0..c.inputs().len()).map(|i| bits.get(i).copied().unwrap_or(false)).collect();
        let (vals, out_vals) = c.eval(&ins);
        let sw = |w: f32| Stroke::new(w * k.clamp(0.6, 1.6), INK);
        let sel_wire = |w: &Wire| matches!(&self.sel, Sel::Wire(s) if s == w);
        let ws = wires(c, geo);
        for pass in [false, true] {
            for wg in &ws {
                let v = lit.then_some(vals[wg.src]);
                if v.unwrap_or(false) != pass {
                    continue;
                }
                let st = match (sel_wire(&wg.to), v) {
                    (true, _) => Stroke::new(3.4 * k.clamp(0.6, 1.6), SELECT),
                    (false, Some(true)) => Stroke::new(2.8 * k.clamp(0.6, 1.6), LIT_ON),
                    (false, Some(false)) => Stroke::new(2.0 * k.clamp(0.6, 1.6), LIT_OFF),
                    (false, None) => sw(2.0),
                };
                painter.add(egui::Shape::line(wg.pts.iter().map(|q| self.w2s(*q)).collect(), st));
            }
        }
        // gates
        for (i, n) in c.nodes.iter().enumerate() {
            let selected = matches!(&self.sel, Sel::Node(s) if *s == n.name);
            let col = if selected { SELECT } else { INK };
            let st = Stroke::new(2.0 * k.clamp(0.6, 1.6), col);
            let g = &geo.nodes[i];
            match (&n.kind, &g.dm) {
                (Kind::Gate(gate, a), Some(dm)) => {
                    self.draw_gate(painter, *gate, dm, a.len(), iec, g.c, st);
                    if lit {
                        let v = vals[i];
                        painter.text(self.w2s((g.c.0, g.bounds.1 - 2.0)), Align2::CENTER_BOTTOM, if v { "1" } else { "0" }, FontId::proportional(11.0 * k.clamp(0.8, 1.4)), if v { LIT_ON } else { LIT_OFF });
                    }
                    for pin in &g.ins {
                        painter.circle_filled(self.w2s(*pin), 2.2 * k.clamp(0.8, 1.4), Color32::WHITE);
                        painter.circle_stroke(self.w2s(*pin), 2.2 * k.clamp(0.8, 1.4), Stroke::new(1.2, col));
                    }
                    painter.circle_filled(self.w2s(g.out), 2.6 * k.clamp(0.8, 1.4), col);
                    painter.text(self.w2s((g.c.0, g.bounds.3 + 2.0)), Align2::CENTER_TOP, &n.name, FontId::proportional(10.5 * k.clamp(0.8, 1.4)), theme::muted_col());
                }
                _ => {
                    let q = self.w2s(g.out);
                    painter.circle(q, 4.0 * k.clamp(0.8, 1.4), Color32::WHITE, Stroke::new(1.8, col), );
                    painter.text(q + vec2(-9.0 * k, 0.0), Align2::RIGHT_CENTER, &n.name, FontId::proportional(14.0 * k.clamp(0.8, 1.4)), col);
                    if lit {
                        let ik = c.inputs().iter().position(|x| *x == i).unwrap_or(0);
                        painter.text(q + vec2(7.0 * k, -9.0 * k), Align2::LEFT_BOTTOM, if ins[ik] { "1" } else { "0" }, FontId::proportional(12.0 * k.clamp(0.8, 1.4)), if ins[ik] { LIT_ON } else { LIT_OFF });
                    }
                }
            }
        }
        for (k2, o) in c.outputs.iter().enumerate() {
            let selected = matches!(&self.sel, Sel::Out(s) if *s == o.name);
            let col = if selected { SELECT } else { INK };
            let q = self.w2s(geo.outs[k2].c);
            painter.circle(q, 4.0 * k.clamp(0.8, 1.4), Color32::WHITE, Stroke::new(1.8, col));
            painter.text(q + vec2(9.0 * k, 0.0), Align2::LEFT_CENTER, &o.name, FontId::proportional(14.0 * k.clamp(0.8, 1.4)), col);
            if lit {
                let v = out_vals[k2];
                painter.text(q + vec2(-6.0 * k, -9.0 * k), Align2::RIGHT_BOTTOM, if v { "1" } else { "0" }, FontId::proportional(12.0 * k.clamp(0.8, 1.4)), if v { LIT_ON } else { LIT_OFF });
            }
        }
        // a wire being pulled, with the pins it could land on
        if let Some(Drag::Wire { src, at }) = &self.drag
            && let Some(s) = c.nodes.iter().position(|n| &n.name == src)
        {
            let a = self.w2s(geo.nodes[s].out);
            painter.extend(egui::Shape::dashed_line(&[a, *at], Stroke::new(2.0, SELECT), 6.0, 4.0));
            for (i, g) in geo.nodes.iter().enumerate() {
                for pin in &g.ins {
                    let q = self.w2s(*pin);
                    let near = q.distance(*at) <= PIN_REACH + 3.0;
                    painter.circle_stroke(q, if near { 7.0 } else { 4.5 }, Stroke::new(1.6, SELECT));
                }
                let _ = i;
            }
        }
        if let Some(hp) = hover
            && matches!(self.tool, Tool::Select)
            && self.drag.is_none()
            && let Some(i) = (0..c.nodes.len()).find(|i| self.w2s(geo.nodes[*i].out).distance(hp) <= PIN_REACH)
        {
            painter.circle_stroke(self.w2s(geo.nodes[i].out), 7.0, Stroke::new(1.6, SELECT));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_gate(&self, painter: &egui::Painter, g: Gate, dm: &Dim, n: usize, iec: bool, c: World, st: Stroke) {
        let k = self.scale() as f64;
        let _ = k;
        let bx = c.0 + (dm.back - if dm.bubble { 2.0 * BUB } else { 0.0 }) / 2.0;
        let y = c.1;
        let (l, r, t, b) = (bx - dm.w / 2.0, bx + dm.w / 2.0, y - dm.h / 2.0, y + dm.h / 2.0);
        let s = |x: f64, y: f64| self.w2s((x, y));
        let white = Color32::WHITE;
        if iec {
            painter.rect(Rect::from_two_pos(s(l, t), s(r, b)), CornerRadius::ZERO, white, st, egui::StrokeKind::Middle);
            let label = match g {
                Gate::And | Gate::Nand => "&",
                Gate::Or | Gate::Nor => "\u{2265}1",
                Gate::Xor | Gate::Xnor => "=1",
                Gate::Not | Gate::Buf => "1",
            };
            painter.text(s(bx, y), Align2::CENTER_CENTER, label, FontId::proportional(14.0 * self.scale().clamp(0.8, 1.4)), st.color);
        } else {
            match g {
                Gate::And | Gate::Nand => {
                    let mut pts = vec![s(l, t), s(bx, t)];
                    let (rx, ry) = (dm.w / 2.0, dm.h / 2.0);
                    for i in 1..16 {
                        let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / 16.0;
                        pts.push(s(bx + rx * a.cos(), y + ry * a.sin()));
                    }
                    pts.extend([s(bx, b), s(l, b)]);
                    painter.add(egui::Shape::convex_polygon(pts, white, st));
                }
                Gate::Or | Gate::Nor | Gate::Xor | Gate::Xnor => {
                    let mut hull = gc::quad(s(l, t), s(bx, t), s(r, y), 14);
                    hull.extend(gc::quad(s(r, y), s(bx, b), s(l, b), 14));
                    painter.add(egui::Shape::convex_polygon(hull.clone(), white, Stroke::NONE));
                    painter.add(egui::Shape::line(hull, st));
                    painter.add(egui::Shape::line(gc::quad(s(l, b), s(l + 14.0, y), s(l, t), 12), st));
                    if dm.back > 0.0 {
                        painter.add(egui::Shape::line(gc::quad(s(l - dm.back, t), s(l - dm.back + 14.0, y), s(l - dm.back, b), 12), st));
                    }
                    let (ins, _) = pins(dm, n, c.0, c.1);
                    for (px, py) in ins {
                        painter.line_segment([s(px, py), s(l + 6.0, py)], st);
                    }
                }
                Gate::Not | Gate::Buf => {
                    painter.add(egui::Shape::convex_polygon(vec![s(l, t), s(r, y), s(l, b)], white, st));
                }
            }
        }
        if dm.bubble {
            painter.circle(s(r + BUB, y), (BUB * self.scale() as f64) as f32, white, st);
        }
    }

    fn status(&self, ui: &mut Ui, painter: &egui::Painter, c: &Circuit) {
        let hint = match (&self.note, self.tool) {
            (Some(n), _) => n.clone(),
            (None, Tool::Select) if c.nodes.is_empty() => "Choose Input, then click the canvas to start".to_string(),
            (None, Tool::Select) => "Drag to move. Pin to pin to wire. Click an input to flip it. Double-click to rename.".to_string(),
            (None, t) => format!("Click the canvas to add {} (Esc to cancel)", t.name()),
        };
        let col = if self.note.is_some() { theme::red() } else { theme::muted_col() };
        painter.text(self.rect.left_bottom() + vec2(8.0, -8.0), Align2::LEFT_BOTTOM, hint, FontId::proportional(11.5), col);
        let _ = ui;
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
        steps: usize,
    }

    fn harness(spec: &str) -> Harness<'static, State> {
        let def = registry().into_iter().find(|d| d.id == "logic-circuit").unwrap();
        let mut p = Params::defaults(&(def.params)());
        p.set_s("spec", spec);
        let mut h = crate::figuresui::ui_test::harness(
            |ui, s: &mut State| {
                let ch = s.canvas.show(ui, &mut s.p);
                if ch != Change::None {
                    s.last = ch;
                }
                if ch == Change::Step {
                    s.steps += 1;
                }
            },
            State { canvas: Canvas::default(), p, last: Change::None, steps: 0 },
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
        let mid = from + (to - from) * 0.5;
        mv(h, mid);
        mv(h, to);
        press(h, to, false);
        h.step();
        h.step();
    }

    fn click(h: &mut Harness<'static, State>, at: Pos2) {
        mv(h, at);
        press(h, at, true);
        press(h, at, false);
    }

    /// Lets enough time pass that the next click is not part of a double-click.
    fn wait(h: &mut Harness<'static, State>) {
        for _ in 0..40 {
            h.step();
        }
    }

    fn circuit(h: &Harness<'static, State>) -> Circuit {
        Circuit::parse(h.state().p.s("spec")).unwrap()
    }

    const SPEC: &str = "in A, B\ng1 = AND(A, B)\nout Q = g1";

    #[test]
    fn dragging_a_gate_moves_it_and_writes_its_place_into_the_spec() {
        let mut h = harness(SPEC);
        let from = h.state().canvas.node_screen(&h.state().p, "g1").unwrap();
        drag(&mut h, from, from + vec2(0.0, 120.0));
        let c = circuit(&h);
        let g = c.nodes.iter().find(|n| n.name == "g1").unwrap();
        assert!(g.pos.is_some_and(|p| p.1 > 1.0), "{:?}", g.pos);
        assert!(h.state().p.s("spec").contains("@"));
        assert_eq!(h.state().last, Change::Live);
        // wires, names and the table are untouched
        assert_eq!(c.expr(0), "A AND B");
    }

    #[test]
    fn wiring_an_output_pin_to_an_input_pin_replaces_that_wire() {
        let mut h = harness("in A, B, C\ng1 = AND(A, B)\nout Q = g1");
        let from = h.state().canvas.out_pin_screen(&h.state().p, "C").unwrap();
        let to = h.state().canvas.in_pin_screen(&h.state().p, "g1", 1).unwrap();
        drag(&mut h, from, to);
        assert_eq!(circuit(&h).expr(0), "A AND C");
        assert_eq!(h.state().last, Change::Step);
    }

    #[test]
    fn a_wire_that_would_make_a_loop_is_refused_with_a_note() {
        let mut h = harness("in A\ng1 = NOT(A)\ng2 = NOT(g1)\nout Q = g2");
        let before = h.state().p.s("spec").to_string();
        let from = h.state().canvas.out_pin_screen(&h.state().p, "g2").unwrap();
        let to = h.state().canvas.in_pin_screen(&h.state().p, "g1", 0).unwrap();
        drag(&mut h, from, to);
        assert_eq!(h.state().p.s("spec"), before, "the circuit is untouched");
        assert!(h.state().canvas.note.as_deref().is_some_and(|n| n.contains("loop")), "{:?}", h.state().canvas.note);
    }

    #[test]
    fn dropping_a_wire_on_the_gate_body_adds_an_input_while_there_is_room() {
        let mut h = harness("in A, B, C\ng1 = AND(A, B)\nout Q = g1");
        let from = h.state().canvas.out_pin_screen(&h.state().p, "C").unwrap();
        let to = h.state().canvas.node_screen(&h.state().p, "g1").unwrap();
        drag(&mut h, from, to);
        assert_eq!(circuit(&h).expr(0), "A AND B AND C");
    }

    #[test]
    fn dropping_a_wire_on_empty_canvas_makes_an_output() {
        let mut h = harness(SPEC);
        let from = h.state().canvas.out_pin_screen(&h.state().p, "A").unwrap();
        drag(&mut h, from, from + vec2(60.0, -150.0));
        let c = circuit(&h);
        assert_eq!(c.outputs.len(), 2);
        assert_eq!(c.expr(1), "A");
    }

    #[test]
    fn clicking_an_input_flips_it_and_lights_the_wires() {
        let mut h = harness(SPEC);
        let a = h.state().canvas.node_screen(&h.state().p, "A").unwrap();
        click(&mut h, a);
        assert_eq!(h.state().p.s("try"), "10");
        wait(&mut h);
        click(&mut h, a);
        assert_eq!(h.state().p.s("try"), "00");
        wait(&mut h);
        let b = h.state().canvas.node_screen(&h.state().p, "B").unwrap();
        click(&mut h, b);
        assert_eq!(h.state().p.s("try"), "01");
        assert_eq!(h.state().last, Change::Step);
    }

    #[test]
    fn adding_parts_with_the_palette_tools() {
        let mut h = harness(SPEC);
        h.state_mut().canvas.tool = Tool::Gate(Gate::Or);
        let at = h.state().canvas.rect.center() + vec2(150.0, 100.0);
        click(&mut h, at);
        let c = circuit(&h);
        let g = c.nodes.iter().find(|n| n.name == "g2").expect("a new gate");
        assert!(matches!(&g.kind, Kind::Gate(Gate::Or, a) if a.len() == 2));
        assert_eq!(h.state().canvas.tool, Tool::Select);
        assert_eq!(h.state().canvas.sel, Sel::Node("g2".into()));
        // an output reads the selected gate
        h.state_mut().canvas.tool = Tool::Output;
        { let at = h.state().canvas.rect.center() + vec2(250.0, 100.0); click(&mut h, at); }
        let c = circuit(&h);
        assert_eq!(c.outputs.len(), 2);
        assert_eq!(c.expr(1), "A OR B");
        // and an input
        h.state_mut().canvas.tool = Tool::Input;
        { let at = h.state().canvas.rect.center() + vec2(-250.0, 100.0); click(&mut h, at); }
        assert!(circuit(&h).nodes.iter().any(|n| n.name == "C"));
    }

    #[test]
    fn a_gate_cannot_be_added_to_an_empty_circuit() {
        let mut h = harness("");
        h.state_mut().canvas.tool = Tool::Gate(Gate::And);
        { let at = h.state().canvas.rect.center(); click(&mut h, at); }
        assert_eq!(h.state().p.s("spec"), "");
        assert!(h.state().canvas.note.as_deref().is_some_and(|n| n.contains("input first")));
        h.state_mut().canvas.tool = Tool::Input;
        { let at = h.state().canvas.rect.center(); click(&mut h, at); }
        assert!(h.state().p.s("spec").starts_with("in A"));
    }

    #[test]
    fn delete_removes_the_selected_part_and_what_cannot_stand_without_it() {
        let mut h = harness(SPEC);
        let a = h.state().canvas.node_screen(&h.state().p, "A").unwrap();
        // select it by dragging its name (a drag from its pin would pull a wire)
        drag(&mut h, a - vec2(14.0, 0.0), a + vec2(-14.0, 40.0));
        assert_eq!(h.state().canvas.sel, Sel::Node("A".into()));
        h.key_press(Key::Delete);
        h.run();
        let c = circuit(&h);
        assert!(c.nodes.iter().all(|n| n.name == "B"), "{:?}", c.nodes);
        assert!(c.outputs.is_empty(), "the AND needed A, so it went and its output with it");
        assert_eq!(h.state().canvas.sel, Sel::None);
    }

    #[test]
    fn selecting_a_wire_and_deleting_cuts_it() {
        let mut h = harness("in A, B, C\ng1 = AND(A, B, C)\nout Q = g1");
        // the wire from C to the third pin of g1: click on it just before the pin
        let pin = h.state().canvas.in_pin_screen(&h.state().p, "g1", 2).unwrap();
        click(&mut h, pin - vec2(12.0, 0.0));
        assert!(matches!(&h.state().canvas.sel, Sel::Wire(w) if w.to == "g1" && w.pin == 2), "{:?}", h.state().canvas.sel);
        h.key_press(Key::Delete);
        h.run();
        assert_eq!(circuit(&h).expr(0), "A AND B");
    }

    #[test]
    fn a_text_edit_outside_the_canvas_is_picked_up_and_a_bad_spec_shows_why() {
        let mut h = harness(SPEC);
        h.state_mut().p.set_s("spec", "in A, B, C\ng1 = OR(A, B, C)\nout Q = g1");
        h.run();
        assert!(h.state().canvas.node_screen(&h.state().p, "C").is_some());
        h.state_mut().p.set_s("spec", "in A\ng = AND(A");
        h.run();
        assert!(h.query_by_label_contains("closing").is_some() || h.query_by_label_contains("Fix the circuit").is_some());
    }

    #[test]
    fn renaming_an_input_validates_and_updates_the_spec() {
        let mut h = harness(SPEC);
        let a = h.state().canvas.node_screen(&h.state().p, "A").unwrap();
        mv(&mut h, a);
        h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
        h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
        h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
        h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
        h.run();
        assert!(h.state().canvas.rename.is_some(), "double-click opens the name box");
        h.get_by_role(egui::accesskit::Role::TextInput).focus();
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
        h.get_by_role(egui::accesskit::Role::TextInput).type_text("X");
        h.key_press(Key::Enter);
        h.run();
        assert!(circuit(&h).nodes.iter().any(|n| n.name == "X"), "{}", h.state().p.s("spec"));
        assert_eq!(circuit(&h).expr(0), "X AND B");
    }

    #[test]
    fn hostile_circuits_never_panic_the_canvas() {
        for spec in ["", "in A\nout Q = A", "in A\ng = NOT(A) @ 999,-999\nout Q = g @ -1000,1000", "in A, B\ng = AND(A, B) @ 0,0\nh = OR(A, B) @ 0,0\nout Q = g\nout R = h"] {
            let mut h = harness(spec);
            for t in [Tool::Select, Tool::Input, Tool::Gate(Gate::Xor), Tool::Output] {
                h.state_mut().canvas.tool = t;
                { let at = h.state().canvas.rect.center(); click(&mut h, at); }
                h.key_press(Key::Delete);
                h.run();
            }
            assert!(Circuit::parse(h.state().p.s("spec")).is_ok(), "{spec:?} -> {:?}", h.state().p.s("spec"));
        }
    }
}
