//! Canvas editor for the tree figure: select a node, hang a new child under it, delete a node
//! with everything below it, rename. Like the graph canvas it owns no tree: each frame it reads
//! the figure's `spec` (parent lines) through `Tree::parse` and writes an edit back with
//! `to_spec`. A tree grown from values is turned into parent lines the first time it is edited.
use egui::{Align2, Color32, FontId, Frame, Id, Key, Pos2, Rect, Sense, Stroke, TextEdit, Ui, vec2};

use super::composite_canvas::Change;
use super::graph_canvas::{self as gc, View, World};
use crate::figures::computing::graph_draw::node_radius;
use crate::figures::graph::Tree;
use crate::figures::Params;
use crate::theme;

const INK: Color32 = Color32::from_rgb(0x11, 0x18, 0x27);
const FILL: Color32 = Color32::from_rgb(0xee, 0xf3, 0xfc);
const SELECT: Color32 = Color32::from_rgb(0x2f, 0x6f, 0xe0);
const CELL: (f64, f64) = (80.0, 90.0);
const FS: f64 = 15.0;
const BST: &str = "Insert values (binary search tree)";

pub struct Canvas {
    view: Option<View>,
    pub sel: Option<String>,
    pub note: Option<String>,
    rename: Option<(String, String, Option<String>, bool)>,
    auto_fit: Option<egui::Vec2>,
    last_spec: String,
    pub rect: Rect,
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas { view: None, sel: None, note: None, rename: None, auto_fit: None, last_spec: String::new(), rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)) }
    }
}

struct Geo {
    pts: Vec<World>,
    radii: Vec<f64>,
}

impl Geo {
    fn new(t: &Tree) -> Geo {
        Geo { pts: t.layout().into_iter().map(|p| (p.0 * CELL.0, p.1 * CELL.1)).collect(), radii: t.labels.iter().map(|l| node_radius(l, FS)).collect() }
    }

    fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut it = self.pts.iter().zip(&self.radii).map(|(p, r)| (p.0 - r, p.1 - r, p.0 + r, p.1 + r));
        let first = it.next()?;
        Some(it.fold(first, |a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))))
    }
}

impl Canvas {
    fn view(&self) -> View {
        self.view.unwrap_or(View { s: 1.0, c: (0.0, 0.0) })
    }

    fn w2s(&self, w: World) -> Pos2 {
        self.view().to_screen(self.rect, w)
    }

    fn scale(&self) -> f32 {
        self.view().s as f32
    }

    #[cfg(test)]
    pub fn node_screen(&self, p: &Params, name: &str) -> Option<Pos2> {
        let t = Tree::parse(p.s("spec")).ok()?;
        Some(self.w2s(Geo::new(&t).pts[t.index(name)?]))
    }

    fn write(&mut self, p: &mut Params, t: &Tree) {
        let spec = t.to_spec();
        p.set_s("source", "Parent lines");
        p.set_s("spec", &spec);
        self.last_spec = spec;
        self.note = None;
    }

    fn fit(&mut self, geo: &Geo) {
        self.view = Some(View::fit(self.rect, geo.bounds()));
        self.auto_fit = Some(self.rect.size());
    }

    fn node_at(&self, geo: &Geo, pp: Pos2) -> Option<usize> {
        (0..geo.pts.len()).rev().find(|i| self.w2s(geo.pts[*i]).distance(pp) <= (geo.radii[*i] as f32 * self.scale()).max(8.0))
    }

    /// The tree to edit: the typed parent lines, or the values' search tree as parent lines.
    fn current(&mut self, p: &mut Params) -> Result<Tree, String> {
        if p.s("source").starts_with("Insert") {
            let t = Tree::bst(&crate::figures::data::numbers(p.s("values")));
            Tree::parse(&t.to_spec()).ok().filter(|r| r.labels == t.labels || t.labels.len() < 2).ok_or("Values with a minus sign or a decimal point cannot be edited as parent lines")?;
            return Ok(t);
        }
        Tree::parse(p.s("spec"))
    }

    pub fn show(&mut self, ui: &mut Ui, p: &mut Params) -> Change {
        let mut change = Change::None;
        let bst = p.s("source") == BST;
        let t = match self.current(p) {
            Ok(t) => t,
            Err(e) => {
                ui.add_space(8.0);
                ui.label(theme::muted(format!("{e}\nFix the tree in the panel and the canvas comes back.")).size(12.5));
                return Change::None;
            }
        };
        let mut t = t;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 4.0);
            let sel = self.sel.as_ref().and_then(|n| t.index(n));
            if ui.add_enabled(sel.is_some() || t.labels.is_empty(), gc::seg_btn("Add child", false)).on_hover_text("Hang a new node under the selected one").clicked() {
                self.add_child(p, &mut t, sel, &mut change);
            }
            if ui.add_enabled(sel.is_some_and(|i| i != 0), gc::seg_btn("Delete", false)).on_hover_text("Delete the selected node and everything below it (Delete)").clicked() {
                self.delete(p, &mut t, sel, &mut change);
            }
            if ui.add(gc::seg_btn("Fit", false)).clicked() {
                self.fit(&Geo::new(&t));
            }
        });
        ui.add_space(4.0);
        let (rect, resp) = ui.allocate_exact_size(ui.available_size().max(vec2(160.0, 160.0)), Sense::click_and_drag());
        self.rect = rect;
        let spec = t.to_spec();
        let geo = Geo::new(&t);
        if self.view.is_none() || self.auto_fit.is_some_and(|sz| (sz - rect.size()).abs().max_elem() > 1.0) || spec != self.last_spec {
            if self.view.is_none() || self.auto_fit.is_some() && (self.auto_fit.unwrap() - rect.size()).abs().max_elem() > 1.0 {
                self.fit(&geo);
            } else if let (Some(v), Some(b)) = (self.view, geo.bounds())
                && !v.shows(rect, b)
            {
                self.fit(&geo);
            }
            if self.sel.as_ref().is_some_and(|n| t.index(n).is_none()) {
                self.sel = None;
            }
        }
        self.last_spec = spec;
        let ctx = ui.ctx().clone();

        if let Some(hp) = resp.hover_pos() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let f = zoom as f64 * (scroll as f64 * 0.0018).exp();
            if (f - 1.0).abs() > 1e-4
                && let Some(v) = self.view.as_mut()
            {
                v.zoom_about(rect, hp, f);
                self.auto_fit = None;
            }
        }
        if resp.dragged()
            && let Some(v) = self.view.as_mut()
        {
            v.pan_by(ui.input(|i| i.pointer.delta()));
            self.auto_fit = None;
        }
        if resp.clicked()
            && self.rename.is_none()
            && let Some(pp) = resp.interact_pointer_pos()
        {
            self.sel = self.node_at(&geo, pp).map(|i| t.labels[i].clone());
            self.note = None;
        }
        if resp.double_clicked()
            && let Some(pp) = resp.interact_pointer_pos()
            && let Some(i) = self.node_at(&geo, pp)
        {
            self.rename = Some((t.labels[i].clone(), t.labels[i].clone(), None, true));
        }
        if !ctx.egui_wants_keyboard_input() && self.rename.is_none() && (resp.hovered() || self.sel.is_some()) {
            let (del, esc) = ui.input(|i| (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace), i.key_pressed(Key::Escape)));
            if del {
                let sel = self.sel.as_ref().and_then(|n| t.index(n));
                self.delete(p, &mut t, sel, &mut change);
            }
            if esc {
                self.sel = None;
                self.note = None;
            }
        }
        let geo = Geo::new(&t);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        gc::draw_grid(&painter, rect, &self.view(), CELL.0 / 4.0);
        self.draw(&painter, &t, &geo);
        self.rename_box(ui, &mut t, &geo, p, &mut change);
        let hint = match (&self.note, bst) {
            (Some(n), _) => n.clone(),
            (None, true) => "Editing turns this search tree into parent lines (the values are kept in the panel)".to_string(),
            (None, false) if t.labels.is_empty() => "Press Add child to start a tree".to_string(),
            (None, false) => "Click a node, then Add child or Delete. Double-click to rename.".to_string(),
        };
        painter.text(rect.left_bottom() + vec2(8.0, -8.0), Align2::LEFT_BOTTOM, hint, FontId::proportional(11.5), if self.note.is_some() { theme::red() } else { theme::muted_col() });
        if change != Change::None {
            ctx.request_repaint();
        }
        change
    }

    fn add_child(&mut self, p: &mut Params, t: &mut Tree, sel: Option<usize>, change: &mut Change) {
        if t.labels.is_empty() {
            t.labels.push("A".into());
            t.kids.push(vec![]);
        }
        match t.add_child(sel.unwrap_or(0)) {
            Ok(i) => {
                self.sel = Some(t.labels[i].clone());
                self.write(p, t);
                *change = Change::Step;
            }
            Err(e) => self.note = Some(e),
        }
    }

    fn delete(&mut self, p: &mut Params, t: &mut Tree, sel: Option<usize>, change: &mut Change) {
        let Some(i) = sel else { return };
        let mut next = t.clone();
        match next.remove_subtree(i) {
            Ok(()) if next.to_spec().is_empty() => self.note = Some("A tree needs at least one parent line, so the last child stays".into()),
            Ok(()) => {
                *t = next;
                self.sel = None;
                self.write(p, t);
                *change = Change::Step;
            }
            Err(e) => self.note = Some(e),
        }
    }

    fn rename_box(&mut self, ui: &mut Ui, t: &mut Tree, geo: &Geo, p: &mut Params, change: &mut Change) {
        let (view, rect) = (self.view(), self.rect);
        let Some((name, text, err, first)) = self.rename.as_mut() else { return };
        let Some(i) = t.index(name) else {
            self.rename = None;
            return;
        };
        let pos = view.to_screen(rect, (geo.pts[i].0, geo.pts[i].1 - geo.radii[i] - 30.0));
        let (mut commit, mut cancel) = (false, false);
        egui::Area::new(Id::new("tree_rename")).order(egui::Order::Foreground).fixed_pos(pos - vec2(60.0, 0.0)).show(ui.ctx(), |ui| {
            Frame::popup(ui.style()).inner_margin(6.0).show(ui, |ui| {
                let te = ui.add(TextEdit::singleline(text).desired_width(120.0).hint_text("Name"));
                if *first {
                    te.request_focus();
                    *first = false;
                }
                if te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    commit = true;
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel = true;
                }
                if let Some(e) = err.as_ref() {
                    ui.label(theme::muted(e.as_str()).size(11.0).color(theme::red()));
                }
            });
        });
        if cancel {
            self.rename = None;
        } else if commit {
            let new = text.trim().to_string();
            match t.rename(i, &new) {
                Ok(()) => {
                    self.rename = None;
                    self.sel = Some(new);
                    self.write(p, t);
                    *change = Change::Step;
                }
                Err(e) => {
                    *err = Some(e);
                    *first = true;
                }
            }
        }
    }

    fn draw(&self, painter: &egui::Painter, t: &Tree, geo: &Geo) {
        let k = self.scale().clamp(0.6, 1.6);
        for (i, kids) in t.kids.iter().enumerate() {
            for c in kids.iter().flatten() {
                let (a, b) = (self.w2s(geo.pts[i]), self.w2s(geo.pts[*c]));
                let u = (b - a).normalized();
                if (b - a).length() > (geo.radii[i] + geo.radii[*c]) as f32 * self.scale() {
                    painter.line_segment([a + u * geo.radii[i] as f32 * self.scale(), b - u * geo.radii[*c] as f32 * self.scale()], Stroke::new(2.0 * k, INK));
                }
            }
        }
        for (i, l) in t.labels.iter().enumerate() {
            let selected = self.sel.as_deref() == Some(l.as_str());
            let c = self.w2s(geo.pts[i]);
            painter.circle(c, geo.radii[i] as f32 * self.scale(), FILL, Stroke::new(2.0 * k, if selected { SELECT } else { INK }));
            painter.text(c, Align2::CENTER_CENTER, l, FontId::proportional(FS as f32 * k), INK);
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

    fn harness(source: &str, spec: &str) -> Harness<'static, State> {
        let def = registry().into_iter().find(|d| d.id == "tree-diagram").unwrap();
        let mut p = Params::defaults(&(def.params)());
        p.set_s("source", source);
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

    fn click(h: &mut Harness<'static, State>, at: Pos2) {
        h.event(egui::Event::PointerMoved(at));
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
            h.step();
            h.step();
        }
    }

    fn spec(h: &Harness<'static, State>) -> String {
        h.state().p.s("spec").to_string()
    }

    const PARENT: &str = "Parent lines";

    #[test]
    fn select_add_child_and_delete() {
        let mut h = harness(PARENT, "A: B, C");
        let c = h.state().canvas.node_screen(&h.state().p, "C").unwrap();
        click(&mut h, c);
        assert_eq!(h.state().canvas.sel.as_deref(), Some("C"));
        h.get_by_label("Add child").click();
        h.run();
        assert_eq!(spec(&h), "A: B, C\nC: D\n");
        assert_eq!(h.state().last, Change::Step);
        assert_eq!(h.state().canvas.sel.as_deref(), Some("D"));
        h.key_press(Key::Delete);
        h.run();
        assert_eq!(spec(&h), "A: B, C\n");
    }

    #[test]
    fn the_root_and_the_last_child_stay() {
        let mut h = harness(PARENT, "A: B");
        let a = h.state().canvas.node_screen(&h.state().p, "A").unwrap();
        click(&mut h, a);
        h.key_press(Key::Delete);
        h.run();
        assert_eq!(spec(&h), "A: B\n".trim_end().to_string() + "");
        let b = h.state().canvas.node_screen(&h.state().p, "B").unwrap();
        click(&mut h, b);
        h.key_press(Key::Delete);
        h.run();
        assert!(spec(&h).contains("A: B"), "{:?}", spec(&h));
        assert!(h.state().canvas.note.as_deref().is_some_and(|n| n.contains("at least one")), "{:?}", h.state().canvas.note);
    }

    #[test]
    fn editing_a_search_tree_turns_it_into_parent_lines() {
        let mut h = harness("Insert values (binary search tree)", "");
        h.state_mut().p.set_s("values", "5, 3, 8");
        h.run();
        let n = h.state().canvas.node_screen(&h.state().p, "5");
        assert!(n.is_none(), "labels come from the spec until the first edit, so look the node up on the tree");
        let t = Tree::bst(&[5.0, 3.0, 8.0]);
        let g = Geo::new(&t);
        let at = h.state().canvas.w2s(g.pts[1]);
        click(&mut h, at);
        assert_eq!(h.state().canvas.sel.as_deref(), Some("3"));
        h.get_by_label("Add child").click();
        h.run();
        assert_eq!(h.state().p.s("source"), "Parent lines");
        assert!(spec(&h).starts_with("5: 3, 8\n3: A"), "{:?}", spec(&h));
    }

    #[test]
    fn a_bad_tree_shows_why_and_hostile_specs_never_panic() {
        let h = harness(PARENT, "A: B\nC: B");
        assert!(h.query_by_label_contains("two parents").is_some());
        for spec in ["", "\u{0}", "A: _, _", "A: B, C, D, E, F, G", "nonsense"] {
            let mut h = harness(PARENT, spec);
            if let Some(fit) = h.query_by_label("Fit") {
                fit.click();
            }
            h.run();
            h.key_press(Key::Delete);
            h.run();
        }
    }
}
