//! Figure engine: parametric SVG generators (physics/biology/chemistry/math) with a shared
//! parameter schema, so one auto-built UI edits any figure. Output is SVG; `to_png` rasterizes it
//! for egui preview, question attachment (store::put_image) and PDF/DOCX export.
//!
//! Contract for subject modules (`physics`, `biology`, `chemistry`, `math`): expose
//! `pub fn defs() -> Vec<FigureDef>`. Generators are original work; use `svg::Doc` helpers.

use std::collections::BTreeMap;

pub mod biology;
pub mod chemistry;
pub mod interact;
pub mod layout;
pub mod math;
pub mod math_geo;
pub mod math_plot;
pub mod physics;
pub mod svg;

#[derive(Clone, Debug, PartialEq)]
pub enum Value { F(f64), I(i64), B(bool), S(String) }

#[derive(Clone, Debug)]
pub enum Kind {
    Float { min: f64, max: f64, step: f64 },
    Int { min: i64, max: i64 },
    Bool,
    /// Dropdown; value is the chosen string.
    Choice(Vec<&'static str>),
    Text,
    /// Multi-line text (DSL specs, lane tables). Stored as a string like `Text`.
    Long,
    /// Comma/semicolon separated list edited as rows (data tables, point lists). Stored as the
    /// same string the generator already parses, so saved specs stay compatible.
    List,
    /// Colour from a small palette of `#rrggbb` swatches (stored as the hex string).
    Color(Vec<&'static str>),
}

#[derive(Clone, Debug)]
pub struct ParamDef {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub default: Value,
}

impl ParamDef {
    pub fn float(key: &'static str, label: &'static str, min: f64, max: f64, step: f64, d: f64) -> Self {
        Self { key, label, kind: Kind::Float { min, max, step }, default: Value::F(d) }
    }
    pub fn int(key: &'static str, label: &'static str, min: i64, max: i64, d: i64) -> Self {
        Self { key, label, kind: Kind::Int { min, max }, default: Value::I(d) }
    }
    pub fn bool(key: &'static str, label: &'static str, d: bool) -> Self {
        Self { key, label, kind: Kind::Bool, default: Value::B(d) }
    }
    pub fn choice(key: &'static str, label: &'static str, opts: &[&'static str], d: &str) -> Self {
        Self { key, label, kind: Kind::Choice(opts.to_vec()), default: Value::S(d.into()) }
    }
    pub fn text(key: &'static str, label: &'static str, d: &str) -> Self {
        Self { key, label, kind: Kind::Text, default: Value::S(d.into()) }
    }
    pub fn long(key: &'static str, label: &'static str, d: &str) -> Self {
        Self { key, label, kind: Kind::Long, default: Value::S(d.into()) }
    }
    pub fn list(key: &'static str, label: &'static str, d: &str) -> Self {
        Self { key, label, kind: Kind::List, default: Value::S(d.into()) }
    }
    pub fn color(key: &'static str, label: &'static str, palette: &[&'static str], d: &str) -> Self {
        Self { key, label, kind: Kind::Color(palette.to_vec()), default: Value::S(d.into()) }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Params(pub BTreeMap<String, Value>);

impl Params {
    pub fn defaults(defs: &[ParamDef]) -> Self {
        Params(defs.iter().map(|d| (d.key.to_string(), d.default.clone())).collect())
    }
    pub fn f(&self, k: &str) -> f64 {
        match self.0.get(k) { Some(Value::F(v)) => *v, Some(Value::I(v)) => *v as f64, _ => 0.0 }
    }
    pub fn i(&self, k: &str) -> i64 {
        match self.0.get(k) { Some(Value::I(v)) => *v, Some(Value::F(v)) => *v as i64, _ => 0 }
    }
    pub fn b(&self, k: &str) -> bool { matches!(self.0.get(k), Some(Value::B(true))) }
    pub fn s(&self, k: &str) -> &str {
        match self.0.get(k) { Some(Value::S(v)) => v, _ => "" }
    }
    pub fn set_f(&mut self, k: &str, v: f64) { self.0.insert(k.to_string(), Value::F(v)); }
    pub fn set_i(&mut self, k: &str, v: i64) { self.0.insert(k.to_string(), Value::I(v)); }
    pub fn set_b(&mut self, k: &str, v: bool) { self.0.insert(k.to_string(), Value::B(v)); }
    pub fn set_s(&mut self, k: &str, v: &str) { self.0.insert(k.to_string(), Value::S(v.to_string())); }

    /// Clamp every value into its declared range / option set (never panics, ignores unknown
    /// keys). With `snap`, floats are also rounded to their step grid (used after drags).
    pub fn normalize(&mut self, defs: &[ParamDef], snap: bool) {
        for d in defs {
            let Some(cur) = self.0.get(d.key).cloned() else { continue };
            let new = match (&d.kind, cur) {
                (Kind::Float { min, max, step }, Value::F(v)) => {
                    let mut v = if v.is_nan() { *min } else { v };
                    if snap && *step > 0.0 {
                        v = min + ((v - min) / step).round() * step;
                        v = (v * 1e6).round() / 1e6;
                    }
                    Value::F(v.clamp(*min, *max))
                }
                (Kind::Int { min, max }, Value::I(v)) => Value::I(v.clamp(*min, *max)),
                (Kind::Choice(opts), Value::S(v)) if !opts.contains(&v.as_str()) => d.default.clone(),
                (_, v) => v,
            };
            self.0.insert(d.key.to_string(), new);
        }
    }
}

/// Optional interaction layer for a figure: draggable handles over the rendered preview.
/// Dispatch lives in `interact::handles_for` (keeps `FigureDef` literals untouched).
impl FigureDef {
    pub fn handles(&self, p: &Params, size: (f64, f64)) -> Vec<interact::Handle> {
        interact::handles_for(self.id, p, size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject { Physics, Biology, Chemistry, Math }

impl Subject {
    pub const ALL: [Subject; 4] = [Subject::Physics, Subject::Biology, Subject::Chemistry, Subject::Math];
    pub fn name(self) -> &'static str {
        match self { Subject::Physics => "Physics", Subject::Biology => "Biology", Subject::Chemistry => "Chemistry", Subject::Math => "Math" }
    }
}

#[derive(Clone)]
pub struct FigureDef {
    /// Stable slug, e.g. "free-body-diagram".
    pub id: &'static str,
    pub subject: Subject,
    pub title: &'static str,
    pub blurb: &'static str,
    pub params: fn() -> Vec<ParamDef>,
    /// Pure: params -> standalone SVG document string (white background, fixed viewBox).
    pub render: fn(&Params) -> String,
}

pub fn registry() -> Vec<FigureDef> {
    let mut v = Vec::new();
    v.extend(physics::defs());
    v.extend(biology::defs());
    v.extend(chemistry::defs());
    v.extend(math::defs());
    v
}

/// Params for `def`: its defaults overridden by a JSON object of strings, bools and numbers
/// (used by the headless figure hooks).
pub fn params_from_json(def: &FigureDef, over: &serde_json::Value) -> Params {
    let mut p = Params::defaults(&(def.params)());
    for (k, v) in over.as_object().into_iter().flatten() {
        match v {
            serde_json::Value::Bool(b) => p.set_b(k, *b),
            serde_json::Value::Number(n) if n.is_i64() => p.set_i(k, n.as_i64().unwrap_or(0)),
            serde_json::Value::Number(n) => p.set_f(k, n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(t) => p.set_s(k, t),
            _ => {}
        }
    }
    p
}

/// System fonts are loaded once and shared (loading per render made live drags stutter).
fn fontdb() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static DB: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        std::sync::Arc::new(db)
    })
    .clone()
}

/// Size (w, h) of the SVG canvas in its own units, read from the root `width`/`height`.
pub fn svg_size(svg: &str) -> (f64, f64) {
    let head = &svg[..svg.len().min(400)];
    let get = |name: &str| {
        head.find(&format!(" {name}=\""))
            .and_then(|i| head[i + name.len() + 3..].split('"').next())
            .and_then(|v| v.trim_end_matches("px").parse::<f64>().ok())
    };
    (get("width").unwrap_or(400.0), get("height").unwrap_or(300.0))
}

/// Rasterize SVG to PNG at `scale`x its natural size. System fonts are loaded for text.
pub fn to_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fontdb();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let size = tree.size();
    let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
    let mut pm = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("bad size")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    pm.encode_png().map_err(|e| e.to_string())
}

/// Import path for the hybrid mode: accept pasted/loaded SVG text from anywhere.
pub fn validate_svg(svg: &str) -> Result<(), String> {
    to_png(svg, 0.25).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Every registered figure must render with default params into a rasterizable SVG.
    #[test]
    fn all_figures_render_with_defaults() {
        for d in registry() {
            let p = Params::defaults(&(d.params)());
            let svg = (d.render)(&p);
            to_png(&svg, 0.5).unwrap_or_else(|e| panic!("{} failed: {e}", d.id));
        }
    }
}

