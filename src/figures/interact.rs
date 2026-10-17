//! Direct-manipulation layer. A figure may expose `Handle`s (positions in SVG canvas units) that
//! the workspace draws over the preview; dragging/clicking/scrolling/double-clicking maps the
//! pointer back into `Params`. Dispatch is by figure id so `FigureDef` literals stay untouched.
//!
//! Contract for handle authors (see `handles_for`):
//! * positions use the SAME geometry as the renderer (reuse its layout helpers, or capture the
//!   scale/origin in the closure) and are recomputed from `Params` every frame;
//! * drag closures receive the pointer position in SVG units and write ONLY the params the
//!   handle drives; the workspace then snaps + clamps via `Params::normalize`, so closures
//!   need not clamp (but should avoid NaN);
//! * a handle must round-trip: dragging it to its own `pos` must not change the params
//!   (checked for every figure by `figures::interact::tests`).
use std::sync::Arc;

use super::Params;

pub type Pt = (f64, f64);
pub type DragFn = Arc<dyn Fn(&mut Params, Pt) + Send + Sync>;
pub type ClickFn = Arc<dyn Fn(&mut Params) + Send + Sync>;
pub type ScrollFn = Arc<dyn Fn(&mut Params, f64) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Circle,
    Diamond,
    Square,
    /// Invisible hit-box that only outlines on hover (cells, labels, bonds).
    Zone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Grab,
    Move,
    ResizeH,
    ResizeV,
    Pointer,
    Crosshair,
}

#[derive(Clone)]
pub struct Handle {
    /// Centre in SVG units.
    pub pos: Pt,
    /// Half extent of the hit area in SVG units (min visual size is enforced on screen).
    pub half: Pt,
    pub shape: Shape,
    pub cursor: Cursor,
    pub tip: String,
    pub drag: Option<DragFn>,
    pub click: Option<ClickFn>,
    pub scroll: Option<ScrollFn>,
    /// Text param edited inline on double-click.
    pub edit: Option<&'static str>,
    /// A movable figure label (`labels::handles`): hit only over its own text box and drawn on
    /// top of the other handles, so a drag that starts on the label moves the label.
    pub label_drag: bool,
}

impl Handle {
    fn base(pos: Pt, shape: Shape, cursor: Cursor, tip: &str) -> Self {
        Handle { pos, half: (7.0, 7.0), shape, cursor, tip: tip.to_string(), drag: None, click: None, scroll: None, edit: None, label_drag: false }
    }
    /// Round knob that drives params when dragged.
    pub fn knob(pos: Pt, tip: &str, f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Self {
        Handle { drag: Some(Arc::new(f)), ..Handle::base(pos, Shape::Circle, Cursor::Grab, tip) }
    }
    pub fn diamond(pos: Pt, tip: &str, f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Self {
        Handle { drag: Some(Arc::new(f)), ..Handle::base(pos, Shape::Diamond, Cursor::Grab, tip) }
    }
    pub fn square(pos: Pt, tip: &str, f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Self {
        Handle { drag: Some(Arc::new(f)), ..Handle::base(pos, Shape::Square, Cursor::Grab, tip) }
    }
    /// Bar-like handle constrained to one axis (resize bars, levels).
    pub fn bar(pos: Pt, vertical: bool, tip: &str, f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Self {
        let c = if vertical { Cursor::ResizeV } else { Cursor::ResizeH };
        Handle { drag: Some(Arc::new(f)), ..Handle::base(pos, Shape::Square, c, tip) }
    }
    /// Clickable rectangle (width/height in SVG units) such as a cell, bond or toggle.
    pub fn zone(centre: Pt, size: Pt, tip: &str, f: impl Fn(&mut Params) + Send + Sync + 'static) -> Self {
        Handle { half: (size.0 / 2.0, size.1 / 2.0), click: Some(Arc::new(f)), ..Handle::base(centre, Shape::Zone, Cursor::Pointer, tip) }
    }
    /// Double-click a label to edit text param `key` in place.
    pub fn label(centre: Pt, size: Pt, key: &'static str, tip: &str) -> Self {
        Handle { half: (size.0 / 2.0, size.1 / 2.0), edit: Some(key), ..Handle::base(centre, Shape::Zone, Cursor::Pointer, tip) }
    }
    pub fn with_scroll(mut self, f: impl Fn(&mut Params, f64) + Send + Sync + 'static) -> Self {
        self.scroll = Some(Arc::new(f));
        self
    }
    pub fn with_click(mut self, f: impl Fn(&mut Params) + Send + Sync + 'static) -> Self {
        self.click = Some(Arc::new(f));
        self
    }
    pub fn with_cursor(mut self, c: Cursor) -> Self {
        self.cursor = c;
        self
    }
    pub fn with_half(mut self, hx: f64, hy: f64) -> Self {
        self.half = (hx, hy);
        self
    }
}

// ---------- geometry helpers for handle authors ----------

/// Angle in degrees (0 = +x, counter-clockwise, y-down screen aware) of `p` seen from `c`, 0..360.
pub fn angle_deg(c: Pt, p: Pt) -> f64 {
    let a = (-(p.1 - c.1)).atan2(p.0 - c.0).to_degrees();
    if a < 0.0 { a + 360.0 } else { a }
}

pub fn dist(a: Pt, b: Pt) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Point at `deg` (math convention) and `r` from `c`, in SVG coordinates.
pub fn polar(c: Pt, deg: f64, r: f64) -> Pt {
    (c.0 + r * deg.to_radians().cos(), c.1 - r * deg.to_radians().sin())
}

/// Cycle the choice string `key` to the next option of `opts`.
pub fn cycle(p: &mut Params, key: &str, opts: &[&str]) {
    let cur = p.s(key).to_string();
    let i = opts.iter().position(|o| *o == cur).map(|i| i + 1).unwrap_or(0) % opts.len().max(1);
    p.set_s(key, opts[i]);
}

/// Linear map of `v` from [a0,a1] to [b0,b1] (unclamped).
pub fn lerp_map(v: f64, a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    if (a1 - a0).abs() < 1e-12 { b0 } else { b0 + (v - a0) / (a1 - a0) * (b1 - b0) }
}

// ---------- dispatch ----------

/// Handles for figure `id` given its current params and canvas size (w, h) in SVG units.
/// Each subject module contributes via its own `handles(id, p, size) -> Option<Vec<Handle>>`.
pub fn handles_for(id: &str, p: &Params, size: Pt) -> Vec<Handle> {
    let mut hs = super::physics::handles(id, p, size)
        .or_else(|| super::biology::handles(id, p, size))
        .or_else(|| super::chemistry::handles(id, p, size))
        .or_else(|| super::math::handles(id, p, size))
        .unwrap_or_default();
    if super::labels::draggable(id)
        && let Some(def) = super::registry().into_iter().find(|d| d.id == id)
    {
        hs.extend(super::labels::handles(def.render, p));
    }
    hs
}

#[cfg(test)]
mod tests {
    use super::super::*;

    /// Parameter sets to probe: defaults, then each bool flipped and each choice option tried alone.
    fn variants(defs: &[ParamDef]) -> Vec<Params> {
        let base = Params::defaults(defs);
        let mut v = vec![base.clone()];
        for d in defs {
            match &d.kind {
                Kind::Bool => {
                    let mut p = base.clone();
                    p.set_b(d.key, !base.b(d.key));
                    v.push(p);
                }
                Kind::Choice(opts) => {
                    for o in opts {
                        let mut p = base.clone();
                        p.set_s(d.key, o);
                        v.push(p);
                    }
                }
                Kind::Int { min, max } if d.key == "n" => {
                    for n in *min..=*max {
                        let mut p = base.clone();
                        p.set_i(d.key, n);
                        v.push(p);
                    }
                }
                _ => {}
            }
        }
        v
    }

    fn in_range(defs: &[ParamDef], p: &Params) -> Result<(), String> {
        for d in defs {
            match (&d.kind, p.0.get(d.key)) {
                (Kind::Float { min, max, .. }, Some(Value::F(v))) if !(v.is_finite() && v >= min && v <= max) => {
                    return Err(format!("{} = {v} outside [{min},{max}]", d.key));
                }
                (Kind::Int { min, max }, Some(Value::I(v))) if v < min || v > max => {
                    return Err(format!("{} = {v} outside [{min},{max}]", d.key));
                }
                (Kind::Choice(o), Some(Value::S(v))) if !o.contains(&v.as_str()) => {
                    return Err(format!("{} = {v:?} not an option", d.key));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn close(defs: &[ParamDef], a: &Params, b: &Params) -> Result<(), String> {
        for d in defs {
            let (x, y) = (a.0.get(d.key), b.0.get(d.key));
            let ok = match (&d.kind, x, y) {
                (Kind::Float { min, max, step }, Some(Value::F(x)), Some(Value::F(y))) => {
                    let tol = if *step > 0.0 { *step * 1.01 } else { (max - min) * 0.01 }.max(1e-6);
                    (x - y).abs() <= tol
                }
                (_, x, y) => x == y,
            };
            if !ok {
                return Err(format!("{}: {:?} -> {:?}", d.key, x, y));
            }
        }
        Ok(())
    }

    #[test]
    fn figure_ids_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for d in registry() {
            assert!(seen.insert(d.id), "duplicate figure id {}", d.id);
        }
    }

    #[test]
    fn every_figure_defaults_render_nonempty_svg() {
        for d in registry() {
            let p = Params::defaults(&(d.params)());
            let svg = (d.render)(&p);
            assert!(svg.len() > 300 && svg.contains("<svg"), "{}: svg too small", d.id);
            let (w, h) = svg_size(&svg);
            assert!(w > 50.0 && h > 50.0, "{}: bad canvas {w}x{h}", d.id);
            let png = to_png(&svg, 0.5).unwrap_or_else(|e| panic!("{}: {e}", d.id));
            assert!(png.len() > 500, "{}: empty raster", d.id);
        }
    }

    #[test]
    fn params_clamp_and_snap() {
        for d in registry() {
            let defs = (d.params)();
            let mut p = Params::defaults(&defs);
            for pd in &defs {
                match &pd.kind {
                    Kind::Float { .. } => p.set_f(pd.key, f64::NAN),
                    Kind::Int { .. } => p.set_i(pd.key, i64::MAX),
                    Kind::Choice(_) => p.set_s(pd.key, "<<bogus>>"),
                    _ => {}
                }
            }
            p.normalize(&defs, true);
            in_range(&defs, &p).unwrap_or_else(|e| panic!("{}: {e}", d.id));
            for pd in &defs {
                if let Kind::Float { max, .. } = &pd.kind {
                    p.set_f(pd.key, f64::INFINITY);
                    p.normalize(&defs, false);
                    assert_eq!(p.f(pd.key), *max, "{}.{}", d.id, pd.key);
                }
            }
            // After clamping, the figure must still render.
            to_png(&(d.render)(&p), 0.3).unwrap_or_else(|e| panic!("{}: clamped render: {e}", d.id));
        }
    }

    #[test]
    fn every_handle_round_trips_and_stays_in_range() {
        let mut total = 0;
        for d in registry() {
            let defs = (d.params)();
            for p in variants(&defs) {
                let size = svg_size(&(d.render)(&p));
                let hs = d.handles(&p, size);
                for (i, h) in hs.iter().enumerate() {
                    assert!(h.pos.0.is_finite() && h.pos.1.is_finite(), "{} handle {i} ({}) non-finite", d.id, h.tip);
                    if let Some(f) = &h.drag {
                        total += 1;
                        // 1. dragging a handle onto its own position changes nothing.
                        let mut q = p.clone();
                        f(&mut q, h.pos);
                        q.normalize(&defs, true);
                        let mut want = p.clone();
                        want.normalize(&defs, true);
                        close(&defs, &want, &q).unwrap_or_else(|e| panic!("{} handle {i} ({}) round trip: {e}", d.id, h.tip));
                        // 2. wild targets stay in range and the figure still renders.
                        for t in [(0.0, 0.0), (size.0, size.1), (-5000.0, 9000.0), (size.0 / 2.0, -300.0), (1e9, -1e9)] {
                            let mut q = p.clone();
                            f(&mut q, t);
                            q.normalize(&defs, true);
                            in_range(&defs, &q).unwrap_or_else(|e| panic!("{} handle {i} ({}) -> {t:?}: {e}", d.id, h.tip));
                            let svg = (d.render)(&q);
                            assert!(svg.contains("<svg"), "{} handle {i}", d.id);
                        }
                    }
                    if let Some(f) = &h.click {
                        let mut q = p.clone();
                        f(&mut q);
                        q.normalize(&defs, true);
                        in_range(&defs, &q).unwrap_or_else(|e| panic!("{} click {i} ({}): {e}", d.id, h.tip));
                        assert!(!(d.render)(&q).is_empty());
                    }
                    if let Some(f) = &h.scroll {
                        for dy in [-3.0, 3.0] {
                            let mut q = p.clone();
                            f(&mut q, dy);
                            q.normalize(&defs, true);
                            in_range(&defs, &q).unwrap_or_else(|e| panic!("{} scroll {i} ({}): {e}", d.id, h.tip));
                        }
                    }
                    if let Some(k) = h.edit {
                        assert!(defs.iter().any(|x| x.key == k), "{}: edit key {k} unknown", d.id);
                    }
                }
            }
        }
        println!("drag handles probed: {total}");
    }

    /// Figures with nothing sensible to drag. Anything else must expose at least one handle.
    const NO_HANDLES: &[&str] = &[];

    #[test]
    fn handle_coverage_report() {
        let mut missing = Vec::new();
        for d in registry() {
            let p = Params::defaults(&(d.params)());
            let size = svg_size(&(d.render)(&p));
            let n = d.handles(&p, size).len();
            println!("HANDLES {} {}", d.id, n);
            if n == 0 && !NO_HANDLES.contains(&d.id) {
                missing.push(d.id);
            }
        }
        println!("without handles: {missing:?}");
    }
}
