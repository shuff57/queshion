//! Figure engine: parametric SVG generators (physics/biology/chemistry/math) with a shared
//! parameter schema, so one auto-built UI edits any figure. Output is SVG; `to_png` rasterizes it
//! for egui preview, question attachment (store::put_image) and PDF/DOCX export.
//!
//! Contract for subject modules (`physics`, `biology`, `chemistry`, `math`): expose
//! `pub fn defs() -> Vec<FigureDef>`. Generators are original work; use `svg::Doc` helpers.

use std::collections::BTreeMap;

pub mod biology;
pub mod chemistry;
pub mod math;
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

/// Rasterize SVG to PNG at `scale`x its natural size. System fonts are loaded for text.
pub fn to_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
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
