//! Figure engine: parametric SVG generators (physics/biology/chemistry/math) with a shared
//! parameter schema, so one auto-built UI edits any figure. Output is SVG; `to_png` rasterizes it
//! for egui preview, question attachment (store::put_image) and PDF/DOCX export.
//!
//! Contract for subject modules (`physics`, `biology`, `chemistry`, `math`): expose
//! `pub fn defs() -> Vec<FigureDef>`. Generators are original work; use `svg::Doc` helpers.

use std::collections::BTreeMap;

pub mod biology;
pub mod cards;
#[cfg(test)]
mod census;
pub mod chemistry;
pub mod composite;
pub mod faq;
pub mod interact;
pub mod items;
pub mod labels;
pub mod layout;
pub mod math;
pub mod math_geo;
pub mod math_plot;
pub mod physics;
pub mod sections;
pub mod share;
pub mod start;
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

/// Other subject tabs a figure is listed under besides its own, as the reference sites
/// cross-link them (physics links the coordinate grid and thermometer, biology the pH reading,
/// chemistry the spring scale, math the vector diagram).
pub fn also_listed(id: &str) -> &'static [Subject] {
    match id {
        "spring-scale" => &[Subject::Chemistry],
        "vector-diagram" => &[Subject::Math],
        "coordinate-grid" => &[Subject::Physics],
        "temperature-reading" => &[Subject::Physics],
        "ph-reading" => &[Subject::Biology],
        _ => &[],
    }
}

/// Whether `def` shows under the `s` subject tab (its own subject or a cross-listing).
pub fn listed_under(def: &FigureDef, s: Subject) -> bool {
    def.subject == s || also_listed(def.id).contains(&s)
}

pub fn registry() -> Vec<FigureDef> {
    let mut v = Vec::new();
    v.extend(physics::defs());
    v.extend(biology::defs());
    v.extend(chemistry::defs());
    v.extend(math::defs());
    v
}

/// One row of a figure's "Measures" card (the reference's way of entering a shape): a name, the
/// value (typed = given, empty = computed from the others and shown grey), and a label popover
/// (Measure / Text / None, plus optional congruence ticks or arcs). All keys name params the
/// figure declares; the card replaces their generic controls.
#[derive(Clone, Debug)]
pub struct MeasureRow {
    /// Row name shown on the left ("AB", "Radius", "∠").
    pub name: &'static str,
    /// Editable name param (the vertex letter of an angle row), if any.
    pub name_key: Option<&'static str>,
    /// Param holding the value: a Text param ("" = computed) or a Float/Int param (always given).
    pub value_key: &'static str,
    pub unit: &'static str,
    /// Choice param Measure | Text | None, and the Text param used when it is "Text".
    pub label_key: &'static str,
    pub text_key: &'static str,
    /// Choice param for congruence ticks / angle arcs ("None", "1", "2", "3"), if any.
    pub tick_key: Option<&'static str>,
    /// A linked row ("CD = AB"): no value box, the figure derives it.
    pub equals: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct MeasureGroup {
    pub title: &'static str,
    pub rows: Vec<MeasureRow>,
}

#[derive(Clone, Debug)]
pub struct MeasureCard {
    pub title: &'static str,
    pub help: &'static str,
    pub groups: Vec<MeasureGroup>,
}

impl MeasureCard {
    /// Every param key the card takes over.
    pub fn keys(&self) -> Vec<&'static str> {
        let mut k = Vec::new();
        for r in self.groups.iter().flat_map(|g| g.rows.iter()) {
            k.extend([r.value_key, r.label_key, r.text_key]);
            k.extend(r.name_key);
            k.extend(r.tick_key);
        }
        k
    }
}

/// One slot of a fixed-slot rows card (box-plot data sets, number-line inequalities).
#[derive(Clone, Debug)]
pub struct RowSlot {
    pub value_key: &'static str,
    pub name_key: Option<&'static str>,
    pub color_key: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub enum RowsSource {
    /// A fixed list of slots; empty ones beyond the first appear on "Add".
    Slots(Vec<RowSlot>),
    /// One text param holding the rows joined by `sep` (equations), with per-row colour params.
    Joined { key: &'static str, sep: &'static str, color_keys: Vec<&'static str> },
}

/// The reference's list cards (EQUATIONS, DATA): one editable row per equation / data set with
/// add and clear, and an optional colour per row.
#[derive(Clone, Debug)]
pub struct RowsCard {
    pub title: &'static str,
    pub help: &'static str,
    pub hint: &'static str,
    pub add_label: &'static str,
    pub source: RowsSource,
}

impl RowsCard {
    pub fn keys(&self) -> Vec<&'static str> {
        match &self.source {
            RowsSource::Slots(v) => v.iter().flat_map(|r| [Some(r.value_key), r.name_key, r.color_key]).flatten().collect(),
            RowsSource::Joined { key, color_keys, .. } => std::iter::once(*key).chain(color_keys.iter().copied()).collect(),
        }
    }
}

pub fn rows_for(id: &str) -> Option<RowsCard> {
    math::rows(id)
}

/// "Add a force" style chips: each appends one item (slot `{n}` = count + 1) with preset values.
/// Keys may contain `{n}`; text values may contain `{L}` (the letter A, B, C ... for the slot).
#[derive(Clone, Debug)]
pub struct Chip {
    pub label: &'static str,
    pub set: Vec<(&'static str, Value)>,
}

#[derive(Clone, Debug)]
pub struct ChipsCard {
    pub title: &'static str,
    pub count_key: &'static str,
    pub max: i64,
    pub chips: Vec<Chip>,
}

/// `count_key == ""` makes the chips plain actions (no slot is added); `Value::I(i64::MIN)` in a
/// chip's `set` means "a fresh random number" (a new seed, e.g. "New pedigree").
pub fn chips_for(id: &str) -> Option<ChipsCard> {
    physics::chips(id).or_else(|| math::chips(id)).or_else(|| chemistry::chips(id)).or_else(|| biology::chips(id))
}

/// Params a figure's own custom card takes over (drawn there, not in the generic sections): the
/// pedigree's Family card owns the family setup, the "New pedigree" seed, the stored family and
/// the old Overrides box (under its Advanced fold).
pub fn card_keys(id: &str) -> &'static [&'static str] {
    match id {
        "pedigree" => &["inherit", "gens", "size", "seed", "changes", "family"],
        _ => &[],
    }
}

/// The Measures card of a figure, if it has one (math shapes and solids contribute).
pub fn measures_for(id: &str) -> Option<MeasureCard> {
    math::measures(id)
}

/// Whether a Measures-card group (by title) or a row (by value key) applies to the current
/// params. The reference hides, e.g., the "Oblique" group of a right solid. Default: shown.
pub fn card_visible(id: &str, group: &str, row: Option<&str>, p: &Params) -> bool {
    math::card_visible(id, group, row, p)
}

/// Whether parameter `key` of figure `id` applies to the current params: the reference shows
/// only the controls of the chosen setup (a pulley's Atwood setup has no ramp angle). Hidden
/// params keep their values, so switching back restores them. Default: shown.
pub fn param_relevant(id: &str, key: &str, p: &Params) -> bool {
    match id {
        "pulley" => physics::pulley_relevant(key, p),
        _ => cards::relevant(id, key, p),
    }
}

/// Follow-up after the panel changed param `key` of figure `id` from `old` (a pulley's load
/// label follows a switch to or from the block and tackle).
pub fn after_panel_change(id: &str, key: &str, old: &Value, p: &mut Params) {
    if let ("pulley", "setup", Value::S(old)) = (id, key, old) {
        physics::pulley_setup_changed(old, p);
    }
}

/// Values worked out from the typed ones, as (value_key, text) pairs shown grey in empty boxes
/// and on "Measure" label buttons.
pub fn computed_for(id: &str, p: &Params) -> Vec<(&'static str, String)> {
    math::computed(id, p)
}

/// Upgrades a saved spec made before a figure's inputs were reworked: reads the old keys from
/// `spec` and writes the new ones into `p` (called after the defaults + spec are applied).
pub fn migrate(id: &str, spec: &crate::model::FigureSpec, p: &mut Params) {
    // Old inclined-plane specs with a second block become n = 2 objects.
    if id == "inclined-plane" && spec.params.get("two") == Some(&serde_json::Value::Bool(true)) && !spec.params.contains_key("n") {
        p.set_i("n", 2);
        p.set_b("two", false);
    }
    math::migrate(id, spec, p)
}

/// The param a "Random" button sits under (the reference's Random reading on instrument figures).
pub fn random_key(id: &str) -> Option<&'static str> {
    match id {
        "spring-scale" => Some("reading"),
        "micropipette-reading" => Some("volume"),
        _ => None,
    }
}

/// A uniform number in [0, 1) from `seed` (splitmix64), for the Random reading buttons.
pub fn unit_random(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Sets a random reading the instrument can show (the reference's "Random" button); false when
/// the figure has none.
pub fn random_reading(id: &str, p: &mut Params, seed: u64) -> bool {
    physics::random_reading(id, p, seed) || biology::random_reading(id, p, seed)
}

/// The reference's "Classic setups" dropdown (population growth, Punnett square, pedigree):
/// named starting points like `Example`, applied over the plain defaults.
pub fn classic_setups(id: &str) -> Vec<Example> {
    biology::classic_setups(id).unwrap_or_default()
}

/// A named starting point for a figure ("Start from an example"): parameter overrides on top of
/// the defaults. Each subject module contributes via `examples(id) -> Option<Vec<Example>>`.
#[derive(Clone, Debug)]
pub struct Example {
    pub name: &'static str,
    pub params: Vec<(&'static str, Value)>,
}

pub fn examples_for(id: &str) -> Vec<Example> {
    physics::examples(id)
        .or_else(|| biology::examples(id))
        .or_else(|| chemistry::examples(id))
        .or_else(|| math::examples(id))
        .unwrap_or_default()
}

/// The params a figure opens with on the Figures tab: its defaults plus `start::overrides`
/// (the reference generators' first screen).
pub fn start_params(def: &FigureDef) -> Params {
    let defs = (def.params)();
    let mut p = Params::defaults(&defs);
    for (k, v) in start::overrides(def.id) {
        p.0.insert(k.to_string(), v);
    }
    p.normalize(&defs, false);
    p
}

/// Params for `def`: its opening state (`start_params`) overridden by a JSON object of strings, bools and numbers
/// (used by the headless figure hooks).
pub fn params_from_json(def: &FigureDef, over: &serde_json::Value) -> Params {
    let mut p = start_params(def);
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
        // The print serif (page thumbnails draw the PDF's text in it).
        for face in crate::fonts::Style::ALL {
            db.load_font_data(face.bytes().to_vec());
        }
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

/// Rasterize SVG to premultiplied RGBA at `scale`x (no PNG round trip; the canvas editor
/// re-renders while panning and dragging). Returns (width, height, pixels).
pub fn to_rgba(svg: &str, scale: f32) -> Result<(u32, u32, Vec<u8>), String> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fontdb();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let size = tree.size();
    let (w, h) = ((size.width() * scale).ceil().max(1.0) as u32, (size.height() * scale).ceil().max(1.0) as u32);
    let mut pm = resvg::tiny_skia::Pixmap::new(w, h).ok_or("bad size")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    Ok((w, h, pm.take()))
}

/// Import path for the hybrid mode: accept pasted/loaded SVG text from anywhere.
pub fn validate_svg(svg: &str) -> Result<(), String> {
    to_png(svg, 0.25).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Every registered figure must render with default params into a rasterizable SVG.
    /// The reference cross-lists five generators on a second subject site.
    #[test]
    fn cross_listed_figures() {
        let reg = registry();
        let under = |s: Subject| reg.iter().filter(|d| listed_under(d, s)).map(|d| d.id).collect::<Vec<_>>();
        assert!(under(Subject::Chemistry).contains(&"spring-scale"));
        assert!(under(Subject::Math).contains(&"vector-diagram"));
        assert!(under(Subject::Physics).contains(&"coordinate-grid"));
        assert!(under(Subject::Physics).contains(&"temperature-reading"));
        assert!(under(Subject::Biology).contains(&"ph-reading"));
        // still listed under their own subject, and every cross-listed id exists
        assert!(under(Subject::Physics).contains(&"spring-scale"));
        for id in ["spring-scale", "vector-diagram", "coordinate-grid", "temperature-reading", "ph-reading"] {
            assert!(reg.iter().any(|d| d.id == id), "{id}");
        }
    }

    #[test]
    fn random_readings_stay_on_the_instrument() {
        let reg = registry();
        for (id, key) in [("spring-scale", "reading"), ("micropipette-reading", "volume")] {
            assert_eq!(random_key(id), Some(key));
            let def = reg.iter().find(|d| d.id == id).unwrap();
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..200u64 {
                let mut p = Params::defaults(&(def.params)());
                assert!(random_reading(id, &mut p, seed));
                let v = p.f(key);
                seen.insert((v * 1000.0) as i64);
                match id {
                    "spring-scale" => {
                        assert!((0.0..=10.0).contains(&v), "{v}");
                        assert!(((v * 100.0).round() - v * 100.0).abs() < 1e-6, "2 decimals: {v}");
                    }
                    _ => {
                        assert!((2.0..=20.0).contains(&v), "{v}");
                        assert!(((v * 10.0).round() - v * 10.0).abs() < 1e-6, "0.1 µL steps: {v}");
                    }
                }
            }
            assert!(seen.len() > 50, "{id}: readings should vary");
        }
        // larger instruments follow their own capacity / range
        let ss = reg.iter().find(|d| d.id == "spring-scale").unwrap();
        let mut p = Params::defaults(&(ss.params)());
        p.set_s("cap", "50 N");
        let hi = (0..100).map(|s| { random_reading("spring-scale", &mut p, s); p.f("reading") }).fold(0.0, f64::max);
        assert!(hi > 10.0 && hi <= 50.0, "{hi}");
        let pip = reg.iter().find(|d| d.id == "micropipette-reading").unwrap();
        let mut p = Params::defaults(&(pip.params)());
        p.set_s("size", "1000 µL");
        for s in 0..50 {
            random_reading("micropipette-reading", &mut p, s);
            let v = p.f("volume");
            assert!((100.0..=1000.0).contains(&v) && (v / 10.0).fract().abs() < 1e-9, "{v}");
        }
        let mut q = Params::default();
        assert!(!random_reading("waves", &mut q, 1));
    }

    #[test]
    fn classic_setups_match_the_reference_lists() {
        let names = |id: &str| classic_setups(id).iter().map(|e| e.name).collect::<Vec<_>>();
        assert_eq!(names("population-growth").len(), 10);
        assert_eq!(names("punnett-square").len(), 6);
        assert_eq!(names("pedigree").len(), 6);
        assert_eq!(names("population-growth")[0], "Yeast in a flask");
        assert!(names("pedigree").contains(&"Albinism"));
        assert!(names("pedigree").contains(&"Vitamin D–resistant rickets"));
        assert!(classic_setups("waves").is_empty());
        // every setup names real params with in-range values and renders
        for d in registry() {
            let defs = (d.params)();
            for ex in classic_setups(d.id) {
                let mut p = Params::defaults(&defs);
                for (k, v) in &ex.params {
                    assert!(defs.iter().any(|x| x.key == *k), "{} / {}: unknown key {k}", d.id, ex.name);
                    p.0.insert(k.to_string(), v.clone());
                }
                let before = p.clone();
                p.normalize(&defs, false);
                assert_eq!(format!("{:?}", before.0), format!("{:?}", p.0), "{} / {}: out of range", d.id, ex.name);
                to_png(&(d.render)(&p), 0.4).unwrap_or_else(|e| panic!("{} / {}: {e}", d.id, ex.name));
            }
        }
    }

    #[test]
    fn all_figures_render_with_defaults() {
        for d in registry() {
            let p = Params::defaults(&(d.params)());
            let svg = (d.render)(&p);
            to_png(&svg, 0.5).unwrap_or_else(|e| panic!("{} failed: {e}", d.id));
        }
    }
}

