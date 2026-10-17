//! Math figure generators (original line-art SVG, modelled on the controls of mathfigures.com).
//! Entry point: `pub fn defs() -> Vec<FigureDef>`.
use super::svg::Doc;
use super::*;
use std::f64::consts::PI;

const INK: &str = "#111";
const GREY: &str = "#6b6b6b";
const TINT: &str = "#e9f1fb";
const BLUE: &str = "#1f5fbf";

type Pt = (f64, f64);

// ───────────────────────── shared helpers ─────────────────────────

pub(super) fn fmt(v: f64, dec: i64) -> String {
    let s = format!("{:.*}", dec.clamp(0, 4) as usize, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s.replace('-', "\u{2212}") }
}
fn fmtu(v: f64, dec: i64, unit: &str) -> String {
    let n = fmt(v, dec);
    if unit.trim().is_empty() { n } else { format!("{n} {}", unit.trim()) }
}
fn tw(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.6 }
/// Text with a broad font fallback list (svg::Doc::text only names DejaVu/Arial).
fn txt(doc: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str) {
    doc.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="DejaVu Sans, Arial, Liberation Sans, Noto Sans, Helvetica, sans-serif" fill="#000">{}</text>"##, super::svg::esc(s)));
}
fn put(doc: &mut Doc, x: f64, y: f64, text: &str, size: f64) { txt(doc, x, y + size * 0.34, size, "middle", text); }
fn dline(doc: &mut Doc, a: Pt, b: Pt, color: &str, sw: f64, dash: &str) {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = write!(s, r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{color}" stroke-width="{sw}" stroke-dasharray="{dash}" stroke-linecap="round"/>"#, a.0, a.1, b.0, b.1);
    doc.raw(&s);
}
fn dpoly(doc: &mut Doc, pts: &[Pt], color: &str, sw: f64, dash: &str) {
    use std::fmt::Write;
    let p: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
    let mut s = String::new();
    let _ = write!(s, r#"<polyline points="{}" fill="none" stroke="{color}" stroke-width="{sw}" stroke-dasharray="{dash}" stroke-linejoin="round"/>"#, p.join(" "));
    doc.raw(&s);
}
/// Colour param with fallback (older specs have no value for newly added colour keys).
fn colp(p: &Params, key: &str, default: &str) -> String {
    let v = p.s(key);
    if v.starts_with('#') && v.len() >= 4 { v.to_string() } else { default.to_string() }
}
const SWATCHES: [&str; 8] = ["#111111", "#1f5fbf", "#c0392b", "#2a8a3e", "#8e44ad", "#d68910", "#e9f1fb", "#c9def4"];

pub(super) fn nums(s: &str) -> Vec<f64> {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter_map(|t| t.trim().replace('\u{2212}', "-").parse().ok()).collect()
}
fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).map(|t| t.trim().to_uppercase()).filter(|t| !t.is_empty()).collect()
}
fn listed(list: &[String], name: &str) -> bool {
    let rev: String = name.chars().rev().collect();
    list.iter().any(|t| t == name || *t == rev)
}
fn unit_v(a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = (dx * dx + dy * dy).sqrt().max(1e-12);
    (dx / l, dy / l)
}
fn dist(a: Pt, b: Pt) -> f64 { ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() }
fn mix(c1: (f64, f64, f64), c2: (f64, f64, f64), t: f64) -> String {
    let t = t.clamp(0.0, 1.0);
    format!("#{:02x}{:02x}{:02x}", (c1.0 + (c2.0 - c1.0) * t) as u8, (c1.1 + (c2.1 - c1.1) * t) as u8, (c1.2 + (c2.2 - c1.2) * t) as u8)
}

/// Uniform fit of model points (y up) into a w x h canvas.
pub(super) struct Map { pub s: f64, pub ox: f64, pub oy: f64 }
thread_local! {
    /// Last layout produced by `Map::fit` on this thread (scale, origin x, origin y). Lets the
    /// interaction layer (`math_geo`) place handles with exactly the renderer's geometry.
    static LAST_MAP: std::cell::Cell<Option<(f64, f64, f64)>> = const { std::cell::Cell::new(None) };
}
/// Renders figure `p` with `render` and returns (scale, ox, oy) of the fit it used, if any.
pub(super) fn probe_map(render: fn(&Params) -> String, p: &Params) -> Option<(f64, f64, f64)> {
    LAST_MAP.with(|c| c.set(None));
    let _ = render(p);
    LAST_MAP.with(|c| c.get())
}
impl Map {
    pub(super) fn fit(pts: &[Pt], w: f64, h: f64, mx: f64, my: f64) -> Map {
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in pts { x0 = x0.min(p.0); x1 = x1.max(p.0); y0 = y0.min(p.1); y1 = y1.max(p.1); }
        if x0 > x1 { x0 = 0.0; x1 = 1.0; y0 = 0.0; y1 = 1.0; }
        let s = ((w - 2.0 * mx) / (x1 - x0).max(1e-9)).min((h - 2.0 * my) / (y1 - y0).max(1e-9));
        let m = Map { s, ox: w / 2.0 - s * (x0 + x1) / 2.0, oy: h / 2.0 + s * (y0 + y1) / 2.0 };
        LAST_MAP.with(|c| c.set(Some((m.s, m.ox, m.oy))));
        m
    }
    pub(super) fn p(&self, q: Pt) -> Pt { (self.ox + q.0 * self.s, self.oy - q.1 * self.s) }
}

/// Right-angle square at vertex v (screen), arms toward da and db.
fn square_mark(doc: &mut Doc, v: Pt, da: Pt, db: Pt, size: f64) {
    let (u, w) = (unit_v(v, da), unit_v(v, db));
    let p1 = (v.0 + u.0 * size, v.1 + u.1 * size);
    let p3 = (v.0 + w.0 * size, v.1 + w.1 * size);
    let p2 = (p1.0 + w.0 * size, p1.1 + w.1 * size);
    doc.polyline(&[p1, p2, p3], INK, 1.2);
}

/// Arc of radius r at v between unit directions u and w (screen), interior (short) way.
fn arc(doc: &mut Doc, v: Pt, u: Pt, w: Pt, r: f64) {
    let p1 = (v.0 + u.0 * r, v.1 + u.1 * r);
    let p2 = (v.0 + w.0 * r, v.1 + w.1 * r);
    let cross = u.0 * w.1 - u.1 * w.0;
    let sweep = if cross > 0.0 { 1 } else { 0 };
    doc.path(&format!("M{:.2},{:.2} A{r:.2},{r:.2} 0 0 {sweep} {:.2},{:.2}", p1.0, p1.1, p2.0, p2.1), "none", INK, 1.3);
}

fn new_doc(w: f64, h: f64) -> Doc { Doc::new(w, h) }

// ───────────────────────── polygon engine ─────────────────────────

const MSERIF: &str = "Liberation Serif, Noto Serif, DejaVu Serif, Times New Roman, serif";

/// Math-typeset label centred on (x, y): serif face; letters (variables, vertex names) italic;
/// numbers, operators, multi-letter lowercase words (sin, cm) and the unit upright.
fn mlabel(doc: &mut Doc, x: f64, y: f64, size: f64, s: &str, unit: &str) {
    doc.raw(&mlabel_svg(x, y, size, s, unit));
}
/// `mlabel` as a movable label with id `id` (see `labels::wrap`).
fn mlabel_id(doc: &mut Doc, id: &str, x: f64, y: f64, size: f64, s: &str, unit: &str) {
    doc.raw(&super::labels::wrap(id, (x, y), super::labels::text_half(s, size), &mlabel_svg(x, y, size, s, unit)));
}
fn mlabel_svg(x: f64, y: f64, size: f64, s: &str, unit: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let unit = unit.trim();
    let mut body = String::new();
    let mut i = 0;
    while i < chars.len() {
        let alpha = chars[i].is_alphabetic();
        let mut j = i;
        while j < chars.len() && chars[j].is_alphabetic() == alpha { j += 1; }
        let run: String = chars[i..j].iter().collect();
        let upright = !alpha || (!unit.is_empty() && run == unit) || (run.chars().count() > 1 && run.chars().all(|c| c.is_lowercase()));
        let e = super::svg::esc(&run);
        if upright { body.push_str(&e); } else { body.push_str(&format!(r#"<tspan font-style="italic">{e}</tspan>"#)); }
        i = j;
    }
    format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{size:.1}" text-anchor="middle" font-family="{MSERIF}" fill="#000">{body}</text>"##, y + size * 0.34)
}

/// Label-size toolbar setting (Large / Medium / Small) as a font scale.
fn fscale(p: &Params) -> f64 { match p.s("label_size") { "Large" => 1.25, "Small" => 0.8, _ => 1.0 } }
/// Construction-line style (diagonals, heights, radius, apothem) as an SVG dash array.
fn dash_of(p: &Params) -> &'static str { match p.s("line_style") { "Solid" => "none", "Dotted" => "0.1 5", _ => "6 4" } }

/// `BC = 2x + 3; A = y` -> [(BC, 2x + 3), (A, y)] (keys upper-cased).
fn text_map(s: &str) -> Vec<(String, String)> {
    s.split(|c| c == ';' || c == '\n')
        .filter_map(|it| {
            let (k, v) = it.split_once('=')?;
            let k = k.trim().to_uppercase();
            (!k.is_empty()).then(|| (k, v.trim().to_string()))
        })
        .collect()
}
fn text_for(map: &[(String, String)], name: &str) -> Option<String> {
    let name = name.to_uppercase();
    let rev: String = name.chars().rev().collect();
    map.iter().find(|(k, _)| *k == name || *k == rev).map(|(_, v)| v.clone())
}

/// `AB 1, CD 2` / `all 2` / `none` -> per-item counts (sides when `sides`, else corners).
/// None when the text is empty (caller falls back to its default marks).
fn parse_counts(s: &str, names: &[String], sides: bool) -> Option<Vec<u8>> {
    if s.trim().is_empty() { return None; }
    let n = names.len();
    let mut out = vec![0u8; n];
    for item in s.split(|c| c == ',' || c == ';' || c == '\n') {
        let t: Vec<String> = item.split(|c: char| c.is_whitespace() || c == ':' || c == '=').filter(|x| !x.is_empty()).map(|x| x.to_uppercase()).collect();
        let Some(head) = t.first() else { continue };
        let k = t.get(1).and_then(|v| v.parse::<u8>().ok()).unwrap_or(1).min(3);
        for i in 0..n {
            let nm = if sides { format!("{}{}", names[i], names[(i + 1) % n]) } else { names[i].clone() };
            if head == "ALL" || listed(std::slice::from_ref(head), &nm) { out[i] = k; }
        }
    }
    Some(out)
}

/// Label for a construction line from its choice (`<name> = measure` | Measure | Text | None).
/// Returns (label, raw): "=" means the measure; raw labels are drawn as given.
fn named_label(choice: &str, text: &str, name: &str) -> (Option<String>, bool) {
    let t = if text.trim().is_empty() { name.to_string() } else { text.trim().to_string() };
    match choice {
        "Measure" => (Some("=".into()), true),
        "Text" => (Some(t), true),
        "None" => (None, true),
        _ => (Some(t), false),
    }
}

fn vnames(p: &Params, n: usize) -> Vec<String> {
    let custom: Vec<String> = p.s("vnames").split(|c: char| c == ',' || c.is_whitespace()).map(|t| t.trim().to_uppercase()).filter(|t| !t.is_empty()).collect();
    (0..n).map(|i| custom.get(i).cloned().unwrap_or_else(|| ((b'A' + i as u8) as char).to_string())).collect()
}

#[derive(Clone, Default)]
struct Extra { a: Pt, b: Pt, label: Option<String>, t: f64, dash: &'static str, raw: bool, /** movable-label id of its label (numbered when repeated) */ id: &'static str }
type Mark = (Pt, Pt, Pt);

struct PolySpec {
    pts: Vec<Pt>,
    names: Vec<String>,
    side_txt: Vec<Option<String>>,
    ang_txt: Vec<Option<String>>,
    ticks: Vec<u8>,
    arrows: Vec<u8>,
    arcs: Vec<u8>,
    right_marks: bool,
    vlabels: bool,
    extras: Vec<Extra>,
    marks: Vec<Mark>,
    dots: Vec<Pt>,
    /// (model anchor, model direction, text): free labels such as the centre's name.
    notes: Vec<(Pt, Pt, String)>,
    rot: f64,
    flip: bool,
    bottom: Option<usize>,
    fill: String,
    fs: f64,
}

thread_local! {
    /// (flip, total turn in degrees) used by the last `draw_poly` on this thread, so the handle
    /// layer can map model points exactly as the renderer did.
    static LAST_XF: std::cell::Cell<(bool, f64)> = const { std::cell::Cell::new((false, 0.0)) };
}
pub(super) fn last_xf() -> (bool, f64) { LAST_XF.with(|c| c.get()) }

fn rot_deg(p: Pt, d: f64) -> Pt {
    let (c, s) = (d.to_radians().cos(), d.to_radians().sin());
    (p.0 * c - p.1 * s, p.0 * s + p.1 * c)
}

fn auto_ticks(lens: &[f64]) -> Vec<u8> {
    let mut groups: Vec<f64> = Vec::new();
    let mut out = vec![0u8; lens.len()];
    for (i, l) in lens.iter().enumerate() {
        let cnt = lens.iter().filter(|m| (**m - l).abs() < 1e-6 * l.abs().max(1.0)).count();
        if cnt < 2 { continue; }
        let g = match groups.iter().position(|g| (g - l).abs() < 1e-6 * l.abs().max(1.0)) {
            Some(g) => g,
            None => { groups.push(*l); groups.len() - 1 }
        };
        out[i] = (g + 1).min(3) as u8;
    }
    out
}

/// Height from vertex `v` onto line p-q: the dashed height, extensions past the side when the
/// foot falls outside it, and the right-angle mark at the foot.
fn height_from(v: Pt, p: Pt, q: Pt, label: (Option<String>, bool), dash: &'static str) -> (Vec<Extra>, Vec<Mark>, f64) {
    let u = unit_v(p, q);
    let l = dist(p, q);
    let t = (v.0 - p.0) * u.0 + (v.1 - p.1) * u.1;
    let foot = (p.0 + u.0 * t, p.1 + u.1 * t);
    // label nearer the foot, where the shape is usually wider
    let mut ex = vec![Extra { a: v, b: foot, label: label.0, t: 0.62, dash, raw: label.1, id: "h" }];
    if t < 0.0 { ex.push(Extra { a: p, b: foot, t: 0.5, dash, ..Default::default() }); }
    if t > l { ex.push(Extra { a: q, b: foot, t: 0.5, dash, ..Default::default() }); }
    let dir = if t < l / 2.0 { (foot.0 + u.0, foot.1 + u.1) } else { (foot.0 - u.0, foot.1 - u.1) };
    (ex, vec![(foot, v, dir)], dist(v, foot))
}

fn height_label(p: &Params) -> (Option<String>, bool) { named_label(p.s("height_label"), p.s("height_text"), "h") }

/// Renders a polygon spec: flip, side-at-bottom and turn, then marks and math-typeset labels.
fn draw_poly(s: PolySpec, dec: i64, unit: &str) -> String {
    // dragged labels keep their place relative to the shape when it is turned
    let turn = s.rot;
    super::labels::turned(turn, move || draw_poly_inner(s, dec, unit))
}
fn draw_poly_inner(s: PolySpec, dec: i64, unit: &str) -> String {
    let n = s.pts.len();
    let flip = s.flip;
    let fl = move |p: Pt| if flip { (-p.0, p.1) } else { p };
    let mut base = 0.0;
    if let Some(i) = s.bottom.filter(|i| *i < n) {
        let (a, b) = (fl(s.pts[i]), fl(s.pts[(i + 1) % n]));
        base = -(b.1 - a.1).atan2(b.0 - a.0).to_degrees();
        let c = (s.pts.iter().map(|q| fl(*q).0).sum::<f64>() / n as f64, s.pts.iter().map(|q| fl(*q).1).sum::<f64>() / n as f64);
        if rot_deg(c, base).1 < rot_deg(a, base).1 { base += 180.0; }
    }
    let total = base + s.rot;
    LAST_XF.with(|c| c.set((flip, total)));
    let rot = move |p: Pt| rot_deg(fl(p), total);
    let fs = s.fs;
    let pts: Vec<Pt> = s.pts.iter().map(|p| rot(*p)).collect();
    let mut all = pts.clone();
    for e in &s.extras { all.push(rot(e.a)); all.push(rot(e.b)); }
    for d in &s.dots { all.push(rot(*d)); }
    let (w, h) = (480.0, 360.0);
    let k = 0.6 + 0.4 * fs;
    let m = Map::fit(&all, w, h, 64.0 * k, 46.0 * k);
    let sp: Vec<Pt> = pts.iter().map(|p| m.p(*p)).collect();
    let cen = (sp.iter().map(|p| p.0).sum::<f64>() / n as f64, sp.iter().map(|p| p.1).sum::<f64>() / n as f64);
    let mut doc = new_doc(w, h);
    doc.poly(&sp, &s.fill, INK, 2.2);

    // construction lines
    for e in &s.extras {
        let (a, b) = (m.p(rot(e.a)), m.p(rot(e.b)));
        dline(&mut doc, a, b, "#333", 1.6, if e.dash.is_empty() { "6 4" } else { e.dash });
    }
    if s.right_marks {
        for mk in &s.marks {
            let (v, a, b) = (m.p(rot(mk.0)), m.p(rot(mk.1)), m.p(rot(mk.2)));
            square_mark(&mut doc, v, a, b, 10.0);
        }
    }
    for d in &s.dots { let q = m.p(rot(*d)); doc.circle(q.0, q.1, 3.4, INK, INK, 1.0); }

    // congruence ticks / parallel arrows (middle of the side; split when a side has both)
    for i in 0..n {
        let (a, b) = (sp[i], sp[(i + 1) % n]);
        let u = unit_v(a, b);
        let nn = (-u.1, u.0);
        let kt = s.ticks.get(i).copied().unwrap_or(0) as i32;
        let ka = s.arrows.get(i).copied().unwrap_or(0) as i32;
        let both = kt > 0 && ka > 0;
        let at = |t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        if kt > 0 {
            let mid = at(if both { 0.34 } else { 0.5 });
            for j in 0..kt {
                let o = (j as f64 - (kt - 1) as f64 / 2.0) * 5.0;
                let c = (mid.0 + u.0 * o, mid.1 + u.1 * o);
                doc.line(c.0 - nn.0 * 6.0, c.1 - nn.1 * 6.0, c.0 + nn.0 * 6.0, c.1 + nn.1 * 6.0, INK, 1.7);
            }
        }
        if ka > 0 {
            // parallel sides point their arrows the same way (rightward, or up when vertical)
            let u = if u.0 < -1e-6 || (u.0.abs() <= 1e-6 && u.1 > 0.0) { (-u.0, -u.1) } else { u };
            let mid = at(if both { 0.66 } else { 0.5 });
            for j in 0..ka {
                let o = (j as f64 - (ka - 1) as f64 / 2.0) * 7.0;
                let c = (mid.0 + u.0 * o, mid.1 + u.1 * o);
                let tip = (c.0 + u.0 * 4.0, c.1 + u.1 * 4.0);
                let back = (c.0 - u.0 * 4.0, c.1 - u.1 * 4.0);
                doc.polyline(&[(back.0 + nn.0 * 6.0, back.1 + nn.1 * 6.0), tip, (back.0 - nn.0 * 6.0, back.1 - nn.1 * 6.0)], INK, 1.8);
            }
        }
    }

    // angles: right-angle squares, arcs (one per congruence mark, or one for a labelled angle), labels
    let asz = 15.0 * fs;
    for i in 0..n {
        let v = sp[i];
        let (pa, pb) = (sp[(i + n - 1) % n], sp[(i + 1) % n]);
        let (u, wv) = (unit_v(v, pa), unit_v(v, pb));
        let ang = (u.0 * wv.0 + u.1 * wv.1).clamp(-1.0, 1.0).acos().to_degrees();
        let is_right = (ang - 90.0).abs() < 0.3;
        let txt = s.ang_txt.get(i).cloned().flatten();
        let narc = s.arcs.get(i).copied().unwrap_or(0) as usize;
        let minlen = dist(v, pa).min(dist(v, pb));
        let mut ar = 20.0f64.min(minlen * 0.3).max(8.0);
        if s.right_marks && is_right {
            square_mark(&mut doc, v, pa, pb, 12.0);
            ar = 17.0;
        } else if txt.is_some() || narc > 0 {
            for j in 0..narc.max(1) { arc(&mut doc, v, u, wv, ar + 4.0 * j as f64); }
            ar += 4.0 * (narc.max(1) - 1) as f64;
        }
        if let Some(t) = txt {
            let mut b = (u.0 + wv.0, u.1 + wv.1);
            let bl = (b.0 * b.0 + b.1 * b.1).sqrt();
            if bl < 1e-6 { b = (-u.1, u.0); } else { b = (b.0 / bl, b.1 / bl); }
            let r = ar + 6.0 + 0.5 * (tw(&t, asz) * 0.85 * b.0.abs() + asz * b.1.abs()) + (40.0 - ang).max(0.0) * 0.5;
            mlabel_id(&mut doc, &format!("a{i}"), v.0 + b.0 * r, v.1 + b.1 * r, asz, &t, unit);
        }
    }
    // side labels
    let ssz = 17.0 * fs;
    for i in 0..n {
        if let Some(t) = s.side_txt.get(i).and_then(|t| t.clone()) {
            let (a, b) = (sp[i], sp[(i + 1) % n]);
            let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            let u = unit_v(a, b);
            let mut nn = (-u.1, u.0);
            if nn.0 * (mid.0 - cen.0) + nn.1 * (mid.1 - cen.1) < 0.0 { nn = (-nn.0, -nn.1); }
            let marked = s.ticks.get(i).copied().unwrap_or(0) > 0 || s.arrows.get(i).copied().unwrap_or(0) > 0;
            let off = if marked { 12.0 } else { 9.0 } + 0.5 * (tw(&t, ssz) * 0.85 * nn.0.abs() + ssz * 0.9 * nn.1.abs());
            mlabel_id(&mut doc, &format!("s{i}"), mid.0 + nn.0 * off, mid.1 + nn.1 * off, ssz, &t, unit);
        }
    }
    let ex_scr: Vec<(Pt, Pt)> = s.extras.iter().map(|e| (m.p(rot(e.a)), m.p(rot(e.b)))).collect();
    for (ei, e) in s.extras.iter().enumerate() {
        if let Some(t) = &e.label {
            let hv = dist(e.a, e.b);
            let txt = if t.starts_with('=') { fmtu(hv, dec, unit) } else if e.raw || s.names.is_empty() { t.clone() } else { format!("{t} = {}", fmtu(hv, dec, unit)) };
            let (a, b) = ex_scr[ei];
            let tt = if e.t == 0.0 { 0.5 } else { e.t };
            let mid = (a.0 + (b.0 - a.0) * tt, a.1 + (b.1 - a.1) * tt);
            let u = unit_v(a, b);
            let nn = (-u.1, u.0);
            let off = 6.0 + 0.5 * (tw(&txt, ssz) * 0.85 * nn.0.abs() + ssz * 0.9 * nn.1.abs());
            let mid_in = point_in(mid, &sp);
            let mut best = (f64::MIN, mid);
            for sg in [1.0, -1.0] {
                let c = (mid.0 + nn.0 * off * sg, mid.1 + nn.1 * off * sg);
                let mut dmin = f64::MAX;
                for i in 0..n { dmin = dmin.min(seg_dist(c, sp[i], sp[(i + 1) % n])); }
                for (j, (p1, p2)) in ex_scr.iter().enumerate() { if j != ei { dmin = dmin.min(seg_dist(c, *p1, *p2)); } }
                let ok = point_in(c, &sp) == mid_in;
                let sc = if ok { dmin } else { -1.0 };
                if sc > best.0 { best = (sc, c); }
            }
            let base = if e.id.is_empty() { "x" } else { e.id };
            let nth = s.extras[..ei].iter().filter(|o| o.label.is_some() && (if o.id.is_empty() { "x" } else { o.id }) == base).count();
            let id = if nth == 0 && !e.id.is_empty() { base.to_string() } else { format!("{base}{nth}") };
            mlabel_id(&mut doc, &id, best.1 .0, best.1 .1, ssz, &txt, unit);
        }
    }
    // free notes (centre name, ...)
    for (ni, (at, dir, t)) in s.notes.iter().enumerate() {
        let q = m.p(rot(*at));
        let d = rot_deg(fl(*dir), total);
        let l = (d.0 * d.0 + d.1 * d.1).sqrt().max(1e-9);
        let (dx, dy) = (d.0 / l, -d.1 / l);
        let r = 12.0 + 0.5 * (tw(t, ssz) * 0.85 * dx.abs() + ssz * dy.abs());
        mlabel_id(&mut doc, &format!("n{ni}"), q.0 + dx * r, q.1 + dy * r, ssz, t, unit);
    }
    // vertex letters
    if s.vlabels {
        let vsz = 18.0 * fs;
        for i in 0..n {
            let v = sp[i];
            let (u, wv) = (unit_v(v, sp[(i + n - 1) % n]), unit_v(v, sp[(i + 1) % n]));
            let mut b = (-(u.0 + wv.0), -(u.1 + wv.1));
            let bl = (b.0 * b.0 + b.1 * b.1).sqrt();
            if bl < 1e-6 { b = (u.1, -u.0); } else { b = (b.0 / bl, b.1 / bl); }
            let off = 10.0 + 0.5 * vsz * 0.75;
            mlabel_id(&mut doc, &format!("v{i}"), v.0 + b.0 * off, v.1 + b.1 * off, vsz, &s.names[i], "");
        }
    }
    doc.finish()
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = (dx * dx + dy * dy).max(1e-12);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    dist(p, (a.0 + dx * t, a.1 + dy * t))
}
fn point_in(p: Pt, poly: &[Pt]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 { inside = !inside; }
    }
    inside
}

fn poly_angles(pts: &[Pt]) -> Vec<f64> {
    let n = pts.len();
    (0..n).map(|i| {
        let (u, w) = (unit_v(pts[i], pts[(i + n - 1) % n]), unit_v(pts[i], pts[(i + 1) % n]));
        (u.0 * w.0 + u.1 * w.1).clamp(-1.0, 1.0).acos().to_degrees()
    }).collect()
}

/// Side index chosen by the `bottom` param (None = as it stands / upright).
fn bottom_side(p: &Params, n: usize) -> Option<usize> {
    let b = p.s("bottom").trim().to_uppercase();
    if b.is_empty() || b.contains("AS IT STANDS") || b.starts_with("NONE") { return None; }
    (0..n).position(|i| {
        let nm = format!("{}{}", (b'A' + i as u8) as char, (b'A' + ((i + 1) % n) as u8) as char);
        listed(std::slice::from_ref(&b), &nm)
    })
}

/// Height label params for figures that draw heights.
fn height_common() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("height_label", "Height label", &["h = measure", "Measure", "Text", "None"], "h = measure"),
        ParamDef::text("height_text", "Height name / text", "h"),
    ]
}

fn msg(text: &str) -> String {
    let mut d = new_doc(480.0, 120.0);
    put(&mut d, 240.0, 60.0, text, 16.0);
    d.finish()
}

// ───────────────────────── flat shapes: the Measures model (meas-a) ─────────────────────────
//
// Triangle, rectangle, parallelogram, trapezoid, kite and regular polygon are entered the way the
// reference does it: one row per side / angle. A typed value is "given"; an empty box is worked out
// from the others (`computed`). Each row has a label mode ("" = automatic: the measure when given,
// nothing when worked out; or Measure / Text / None), a label text and congruence ticks or arcs.
// Angles and parallel lines use a rays / lines card instead. The solvers report what is wrong in
// the reference's words when the givens are too few, too many or inconsistent.

mod flat {
use super::*;

pub(crate) const TICKS: [&str; 4] = ["None", "1", "2", "3"];
pub(crate) const STYLES: [&str; 3] = ["Solid", "Dashed", "Dotted"];
pub(crate) const NAME_K: [&str; 4] = ["name_a", "name_b", "name_c", "name_d"];
const BAD_NUM: &str = "Type a number, like 12, 2.5, 5/2 or 3\u{221a}2.";

// ----- typed values -----

/// A typed measure: its value, how it prints (as typed, with √ and π) and how far a worked-out
/// value may differ from it before the two disagree (half a unit in its last typed place).
#[derive(Clone, Debug)]
pub(crate) struct Typed { pub v: f64, pub show: String, pub tol: f64 }

/// `5sqrt2` -> 5√2, `sqrt(x+1)` -> √(x+1), `pi` -> π, `*` -> ·, `-` -> −.
pub(crate) fn pretty(t: &str) -> String {
    let s = t.trim().replace('\u{221a}', "sqrt");
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i..].starts_with(&['s', 'q', 'r', 't']) {
            i += 4;
            if i < cs.len() && cs[i] == '(' {
                let st = i;
                let mut depth = 0;
                while i < cs.len() {
                    if cs[i] == '(' { depth += 1; } else if cs[i] == ')' { depth -= 1; if depth == 0 { break; } }
                    i += 1;
                }
                let inner: String = cs[st + 1..i.min(cs.len())].iter().collect();
                i += 1;
                let inner = pretty(&inner);
                if inner.chars().all(|c| c.is_ascii_digit() || c == '.') { out.push('\u{221a}'); out.push_str(&inner); } else { out.push_str(&format!("\u{221a}({inner})")); }
            } else {
                out.push('\u{221a}');
            }
            continue;
        }
        if cs[i..].starts_with(&['p', 'i']) { out.push('\u{3c0}'); i += 2; continue; }
        match cs[i] { '*' => out.push('\u{b7}'), '-' => out.push('\u{2212}'), c => out.push(c) }
        i += 1;
    }
    out
}

/// Value of a typed measure: numbers, fractions, sqrt / √, pi / π and + − × ÷. Letters are refused.
pub(crate) fn eval_typed(t: &str) -> Option<f64> {
    let s = t.trim().replace('\u{221a}', "sqrt").replace('\u{3c0}', "pi").replace('\u{2212}', "-").replace('\u{b7}', "*").replace('\u{d7}', "*").replace('\u{f7}', "/").to_lowercase();
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i..].starts_with(&['s', 'q', 'r', 't']) {
            out.push_str("sqrt");
            i += 4;
            if i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                out.push('(');
                while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') { out.push(cs[i]); i += 1; }
                out.push(')');
            }
            continue;
        }
        out.push(cs[i]);
        i += 1;
    }
    if out.replace("sqrt", "").replace("pi", "").chars().any(|c| c.is_alphabetic()) { return None; }
    eval(&out, 0.0).filter(|v| v.is_finite())
}

/// "" -> Ok(None); a number or expression -> Ok(Some); anything else -> Err.
pub(crate) fn parse_typed(s: &str) -> Result<Option<Typed>, ()> {
    let t = s.trim();
    if t.is_empty() { return Ok(None); }
    let v = eval_typed(t).ok_or(())?;
    let plain = t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '\u{2212}');
    let tol = if plain {
        let d = t.split_once('.').map(|(_, f)| f.len()).unwrap_or(0) as i32;
        0.5 * 10f64.powi(-d) + 1e-9
    } else {
        0.005 + 1e-9 * v.abs()
    };
    Ok(Some(Typed { v, show: pretty(t), tol }))
}

fn typed_of(p: &Params, key: &str) -> Result<Option<Typed>, String> {
    if key.is_empty() { return Ok(None); }
    parse_typed(p.s(key)).map_err(|_| BAD_NUM.to_string())
}

/// Plain ASCII number for writing back into a value box (drag handles, migration).
pub(crate) fn num_str(v: f64, dec: usize) -> String {
    let s = format!("{:.*}", dec, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// Corner names from the per-corner boxes (blank = A, B, C, ...).
pub(crate) fn corner_names(p: &Params, n: usize) -> Vec<String> {
    (0..n).map(|i| {
        let v = NAME_K.get(i).map(|k| p.s(k).trim().to_string()).unwrap_or_default();
        if v.is_empty() { ((b'A' + i as u8) as char).to_string() } else { v }
    }).collect()
}

/// What prints for one measure: label mode ("" = automatic), text, typed value, worked-out value.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lab_text(p: &Params, lab: &str, txt: &str, typed: Option<&Typed>, val: Option<f64>, linked: bool, angle: bool, dec: i64, unit: &str) -> Option<String> {
    let mode = p.s(lab);
    let mode = if mode.is_empty() { if typed.is_some() && !linked { "Measure" } else { "None" } } else { mode };
    match mode {
        "None" => None,
        "Text" => {
            let t = p.s(txt).trim();
            Some(if t.is_empty() { "x".to_string() } else { t.to_string() })
        }
        _ => {
            let n = match (typed, val) {
                (Some(t), _) if !linked => t.show.clone(),
                (_, Some(v)) => fmt(v, dec),
                _ => "?".to_string(),
            };
            Some(if angle { format!("{n}\u{b0}") } else if unit.trim().is_empty() { n } else { format!("{n} {}", unit.trim()) })
        }
    }
}

fn tick_n(p: &Params, key: &str) -> u8 { p.s(key).parse::<u8>().unwrap_or(0).min(3) }

/// A message in place of the figure (what the reference shows in the card when the measures fail).
pub(crate) fn note_fig(text: &str) -> String {
    let mut d = new_doc(480.0, 360.0);
    let mut lines: Vec<String> = vec![String::new()];
    for w in text.split(' ') {
        let cur = lines.last_mut().unwrap();
        if !cur.is_empty() && cur.chars().count() + w.chars().count() > 44 { lines.push(w.to_string()); } else { if !cur.is_empty() { cur.push(' '); } cur.push_str(w); }
    }
    let y0 = 180.0 - (lines.len() as f64 - 1.0) * 12.0;
    for (i, l) in lines.iter().enumerate() { put(&mut d, 240.0, y0 + 24.0 * i as f64, l, 16.0); }
    d.finish()
}

fn few(n: usize, what: &str) -> String {
    if n == 1 { format!("Give one more measure to draw the {what}.") } else { format!("Give {n} more measures to draw the {what}.") }
}

fn mismatch(name: &str, would: f64, t: &Typed, angle: bool) -> String {
    let d = if angle { "\u{b0}" } else { "" };
    format!("With the other measures, {name} would be {}{d}, not {}{d}.", fmt(would, 2), t.show)
}

fn range_check(name: &str, t: &Option<Typed>, angle: bool) -> Result<(), String> {
    if let Some(t) = t {
        if angle && (t.v <= 0.0 || t.v >= 180.0) { return Err(format!("{name} has to be between 0\u{b0} and 180\u{b0}.")); }
        if !angle && t.v <= 0.0 { return Err(format!("{name} has to be longer than 0.")); }
    }
    Ok(())
}

// ----- triangle solver -----

/// Why three-of-six measures fail (indices: 0-2 angles A B C, 3 AB, 4 BC, 5 CA).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TriErr { Few(usize), Sum2(usize, usize, f64), Sum3(f64), Ineq(usize), Obtuse(usize, bool), Short(usize, f64), Mismatch(usize, f64) }

#[derive(Debug, Clone)]
pub(crate) struct TriOut {
    pub ang: [f64; 3],
    pub side: Option<[f64; 3]>,
    /// The measures make two triangles (SSA); `other` picks the second.
    #[allow(dead_code)]
    pub two: bool,
}

/// Side opposite each angle (A -> BC, B -> CA, C -> AB) and angle opposite each side.
const OPP_SIDE: [usize; 3] = [4, 5, 3];
const OPP_ANG: [usize; 3] = [2, 0, 1];

/// Solves a triangle from any measures (value, tolerance) that fix it. Two or more angles: the
/// third follows, and the first given side (AB, BC, CA) scales it. Else three sides (SSS), else two
/// sides and an angle: between them (SAS) or not (SSA, acute solution unless `other`). Every given
/// measure the solution did not use is checked against it.
pub(crate) fn tri_core(g: &[Option<(f64, f64)>; 6], other: bool) -> Result<TriOut, TriErr> {
    let v = |i: usize| g[i].map(|x| x.0).unwrap_or(0.0);
    let angs: Vec<usize> = (0..3).filter(|i| g[*i].is_some()).collect();
    let sides: Vec<usize> = (3..6).filter(|i| g[*i].is_some()).collect();
    if angs.len() >= 2 {
        let s2 = v(angs[0]) + v(angs[1]);
        if s2 >= 180.0 - 1e-9 { return Err(TriErr::Sum2(angs[0], angs[1], s2)); }
    }
    if angs.len() == 3 {
        let s: f64 = (0..3).map(v).sum();
        let tol: f64 = (0..3).map(|i| g[i].map(|x| x.1).unwrap_or(0.0)).sum();
        if (s - 180.0).abs() > tol { return Err(TriErr::Sum3(s)); }
    }
    let n = angs.len() + sides.len();
    if n < 3 { return Err(TriErr::Few(3 - n)); }
    let (sd, cd) = (|d: f64| d.to_radians().sin(), |d: f64| d.to_radians().cos());
    let acosd = |x: f64| x.clamp(-1.0, 1.0).acos().to_degrees();
    let mut used = [false; 6];
    let mut two = false;
    let (ang, side): ([f64; 3], Option<[f64; 3]>) = if angs.len() >= 2 {
        let (i, j) = (angs[0], angs[1]);
        let k = 3 - i - j;
        let mut a = [0.0; 3];
        a[i] = v(i);
        a[j] = v(j);
        a[k] = 180.0 - a[i] - a[j];
        for &x in &angs { used[x] = true; }
        let side = sides.first().map(|&s| {
            used[s] = true;
            let kk = v(s) / sd(a[OPP_ANG[s - 3]]);
            [kk * sd(a[2]), kk * sd(a[0]), kk * sd(a[1])]
        });
        (a, side)
    } else if sides.len() == 3 {
        let (c, a, b) = (v(3), v(4), v(5));
        let arr = [(3usize, c), (4, a), (5, b)];
        let (li, lv) = arr.iter().copied().fold((3usize, f64::MIN), |m, x| if x.1 > m.1 { x } else { m });
        let rest: f64 = arr.iter().filter(|x| x.0 != li).map(|x| x.1).sum();
        if lv >= rest - 1e-12 { return Err(TriErr::Ineq(li)); }
        for i in 3..6 { used[i] = true; }
        let aa = acosd((b * b + c * c - a * a) / (2.0 * b * c));
        let bb = acosd((a * a + c * c - b * b) / (2.0 * a * c));
        ([aa, bb, 180.0 - aa - bb], Some([c, a, b]))
    } else {
        // one angle, two sides
        let x = angs[0];
        let xs = OPP_SIDE[x];
        used[x] = true;
        for &s in &sides { used[s] = true; }
        let mut s3 = [0.0f64; 3]; // AB, BC, CA
        for &s in &sides { s3[s - 3] = v(s); }
        if g[xs].is_none() {
            // SAS: the angle lies between the two given sides
            let (p, q): (f64, f64) = { let o: Vec<f64> = sides.iter().map(|&s| v(s)).collect(); (o[0], o[1]) };
            let opp = (p * p + q * q - 2.0 * p * q * cd(v(x))).max(0.0).sqrt();
            s3[xs - 3] = opp;
            let (c, a, b) = (s3[0], s3[1], s3[2]);
            let aa = acosd((b * b + c * c - a * a) / (2.0 * b * c));
            let bb = acosd((a * a + c * c - b * b) / (2.0 * a * c));
            ([aa, bb, 180.0 - aa - bb], Some(s3))
        } else {
            let y = *sides.iter().find(|&&s| s != xs).unwrap();
            let (xa, xv, yv) = (v(x), v(xs), v(y));
            let ya = OPP_ANG[y - 3];
            let yang = if xa >= 90.0 - 1e-9 {
                if xv <= yv + 1e-9 { return Err(TriErr::Obtuse(x, (xa - 90.0).abs() < 1e-9)); }
                (yv * sd(xa) / xv).clamp(-1.0, 1.0).asin().to_degrees()
            } else {
                let h = yv * sd(xa);
                if xv < h - 1e-9 { return Err(TriErr::Short(x, h)); }
                let y1 = (h / xv).clamp(-1.0, 1.0).asin().to_degrees();
                if xv >= yv - 1e-9 || (xv - h).abs() < 1e-9 { y1 } else { two = true; if other { 180.0 - y1 } else { y1 } }
            };
            let mut a = [0.0; 3];
            a[x] = xa;
            a[ya] = yang;
            let za = 3 - x - ya;
            a[za] = 180.0 - xa - yang;
            let kk = xv / sd(xa);
            (a, Some([kk * sd(a[2]), kk * sd(a[0]), kk * sd(a[1])]))
        }
    };
    for i in 0..6 {
        let Some((gv, tol)) = g[i] else { continue };
        if used[i] { continue; }
        let c = if i < 3 { Some(ang[i]) } else { side.map(|s| s[i - 3]) };
        if let Some(c) = c { if (c - gv).abs() > tol { return Err(TriErr::Mismatch(i, c)); } }
    }
    Ok(TriOut { ang, side, two })
}

pub(crate) const TRI_V: [&str; 6] = ["m_a", "m_b", "m_c", "m_ab", "m_bc", "m_ca"];
pub(crate) const TRI_LAB: [&str; 6] = ["a_lab", "b_lab", "c_lab", "ab_lab", "bc_lab", "ca_lab"];
pub(crate) const TRI_TXT: [&str; 6] = ["a_label", "b_label", "c_label", "ab_label", "bc_label", "ca_label"];
pub(crate) const TRI_MARK: [&str; 6] = ["a_arcs", "b_arcs", "c_arcs", "ab_ticks", "bc_ticks", "ca_ticks"];

#[derive(Debug)]
pub(crate) struct TriSolved { pub typed: Vec<Option<Typed>>, pub out: TriOut, pub pts: [Pt; 3] }

fn tri_names(p: &Params) -> [String; 6] {
    let n = corner_names(p, 3);
    [format!("\u{2220}{}", n[0]), format!("\u{2220}{}", n[1]), format!("\u{2220}{}", n[2]), format!("{}{}", n[0], n[1]), format!("{}{}", n[1], n[2]), format!("{}{}", n[2], n[0])]
}

pub(crate) fn tri_solve(p: &Params) -> Result<TriSolved, String> {
    let names = tri_names(p);
    let mut typed = Vec::new();
    for k in TRI_V { typed.push(typed_of(p, k)?); }
    for i in 0..6 { range_check(&names[i], &typed[i], i < 3)?; }
    let g: [Option<(f64, f64)>; 6] = std::array::from_fn(|i| typed[i].as_ref().map(|t| (t.v, t.tol)));
    let out = tri_core(&g, p.b("other")).map_err(|e| match e {
        TriErr::Few(n) => few(n, "triangle"),
        TriErr::Sum2(i, j, s) => format!("{} + {} is already {}\u{b0}, and a triangle's angles add up to 180\u{b0}.", names[i], names[j], fmt(s, 2)),
        TriErr::Sum3(s) => format!("{} + {} + {} is {}\u{b0}, not 180\u{b0}.", names[0], names[1], names[2], fmt(s, 2)),
        TriErr::Ineq(l) => {
            let o: Vec<&String> = (3..6).filter(|i| *i != l).map(|i| &names[i]).collect();
            format!("These sides can't make a triangle: {} + {} has to be longer than {}.", o[0], o[1], names[l])
        }
        TriErr::Obtuse(x, right) => format!("{} has to be the longest side, since {} is {}.", names[OPP_SIDE[x]], names[x], if right { "a right angle" } else { "obtuse" }),
        TriErr::Short(x, m) => format!("{} is too short to close the triangle: with these measures it has to be at least {}.", names[OPP_SIDE[x]], fmt(m, 2)),
        TriErr::Mismatch(i, c) => mismatch(&names[i], c, typed[i].as_ref().unwrap(), i < 3),
    })?;
    let c = out.side.map(|s| s[0]).unwrap_or(10.0);
    let (a, b, cc) = (out.ang[0].to_radians(), out.ang[1].to_radians(), out.ang[2].to_radians());
    let ca = c * b.sin() / cc.sin();
    let pts = [(0.0, 0.0), (c, 0.0), (ca * a.cos(), ca * a.sin())];
    Ok(TriSolved { typed, out, pts })
}

/// Worked-out value of each triangle row (angles, then AB, BC, CA; sides unknown for three angles).
pub(crate) fn tri_values(s: &TriSolved) -> [Option<f64>; 6] {
    let sd = s.out.side;
    [Some(s.out.ang[0]), Some(s.out.ang[1]), Some(s.out.ang[2]), sd.map(|x| x[0]), sd.map(|x| x[1]), sd.map(|x| x[2])]
}

fn shared_tail(v: &mut Vec<ParamDef>, right_default: bool, vertices: bool) {
    v.extend([
        ParamDef::bool("right", "Right-angle squares at 90\u{b0} angles and where heights meet a side", right_default),
        ParamDef::bool("vertices", "Corner letters", vertices),
        ParamDef::text("unit", "Unit (e.g. cm)", ""),
        ParamDef::int("dec", "Round to (decimal places)", 0, 3, 1),
        ParamDef::float("rotate", "Turn (degrees)", -180.0, 180.0, 1.0, 0.0),
        ParamDef::color("fill", "Fill colour", &["#e9f1fb", "#fdf0d5", "#e3f4e1", "#fbe3e3", "#efe5f7", "#ffffff"], "#e9f1fb"),
        ParamDef::bool("flip", "Flip (mirror left to right)", false),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        ParamDef::choice("line_style", "Construction lines (diagonals, heights)", &["Solid", "Dashed", "Dotted"], "Dashed"),
    ]);
}

fn label_params(v: &mut Vec<ParamDef>, labs: &[&'static str], txts: &[&'static str], marks: &[&'static str], n_angles_first: bool, angle_count: usize) {
    for i in 0..labs.len() {
        let angle = if n_angles_first { i < angle_count } else { i >= labs.len() - angle_count };
        v.push(ParamDef::choice(labs[i], "Label", &LAB_OPTS, "None"));
        v.push(ParamDef::text(txts[i], "Label text", "x"));
        v.push(ParamDef::choice(marks[i], if angle { "Congruence arcs" } else { "Congruence ticks" }, &TICKS, "None"));
    }
}

pub(crate) fn triangle_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::text("name_a", "Name of corner A", "A"),
        ParamDef::text("name_b", "Name of corner B", "B"),
        ParamDef::text("name_c", "Name of corner C", "C"),
        ParamDef::text("m_a", "Angle A (blank = worked out)", "24"),
        ParamDef::text("m_b", "Angle B (blank = worked out)", "90"),
        ParamDef::text("m_c", "Angle C (blank = worked out)", ""),
        ParamDef::text("m_ab", "Side AB (blank = worked out)", "12"),
        ParamDef::text("m_bc", "Side BC (blank = worked out)", ""),
        ParamDef::text("m_ca", "Side CA (blank = worked out)", ""),
    ];
    label_params(&mut v, &TRI_LAB, &TRI_TXT, &TRI_MARK, true, 3);
    for d in v.iter_mut() {
        match d.key { "a_lab" | "b_lab" | "ab_lab" => d.default = Value::S("Measure".into()), "bc_lab" => d.default = Value::S("Text".into()), _ => {} }
    }
    v.extend([
        ParamDef::bool("other", "Show the other triangle (when the measures make two)", false),
        ParamDef::choice("height", "Height", &["None", "From A", "From B", "From C", "From A and B", "From A and C", "From B and C", "From all three"], "None"),
        ParamDef::choice("bottom", "Side at the bottom", &["AB", "BC", "CA"], "AB"),
    ]);
    shared_tail(&mut v, true, true);
    v.extend(height_common());
    v
}

fn fill_of(p: &Params) -> String { if p.s("fill").starts_with('#') { p.s("fill").to_string() } else { TINT.to_string() } }

pub(crate) fn triangle_render(p: &Params) -> String {
    let s = match tri_solve(p) { Ok(s) => s, Err(e) => return note_fig(&e) };
    let [a, b, c] = s.pts;
    let (dec, unit) = (p.i("dec"), p.s("unit").to_string());
    let vals = tri_values(&s);
    let lab = |i: usize| lab_text(p, TRI_LAB[i], TRI_TXT[i], s.typed[i].as_ref(), vals[i], false, i < 3, dec, &unit);
    let (mut ex, mut mk) = (vec![], vec![]);
    let hs = p.s("height");
    let has = |v: &str| hs == "From all three" || hs.split(|ch: char| !ch.is_alphanumeric()).any(|t| t == v);
    for (on, v, q1, q2) in [(has("A"), a, b, c), (has("B"), b, c, a), (has("C"), c, a, b)] {
        if !on { continue; }
        let (e, m, _) = height_from(v, q1, q2, height_label(p), dash_of(p));
        ex.extend(e);
        mk.extend(m);
    }
    draw_poly(PolySpec {
        pts: vec![a, b, c],
        names: corner_names(p, 3),
        side_txt: (3..6).map(lab).collect(),
        ang_txt: (0..3).map(lab).collect(),
        ticks: (3..6).map(|i| tick_n(p, TRI_MARK[i])).collect(),
        arrows: vec![0; 3],
        arcs: (0..3).map(|i| tick_n(p, TRI_MARK[i])).collect(),
        right_marks: p.b("right"),
        vlabels: p.b("vertices"),
        extras: ex, marks: mk, dots: vec![], notes: vec![],
        rot: p.f("rotate"), flip: p.b("flip"), bottom: bottom_side(p, 3), fill: fill_of(p), fs: fscale(p),
    }, dec, &unit)
}

// ----- quadrilaterals -----

pub(crate) const Q_V: [&str; 8] = ["m_ab", "m_bc", "m_cd", "m_da", "m_a", "m_b", "m_c", "m_d"];
pub(crate) const Q_LAB: [&str; 8] = ["ab_lab", "bc_lab", "cd_lab", "da_lab", "a_lab", "b_lab", "c_lab", "d_lab"];
pub(crate) const Q_TXT: [&str; 8] = ["ab_label", "bc_label", "cd_label", "da_label", "a_label", "b_label", "c_label", "d_label"];
pub(crate) const Q_MARK: [&str; 8] = ["ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "a_arcs", "b_arcs", "c_arcs", "d_arcs"];
/// Parallel arrows per side (the label popover's second mark): "Auto" keeps the kind's own arrows
/// (or the older `arrow_marks` list), anything else sets that side.
pub(crate) const Q_ARROW: [&str; 4] = ["ab_arrows", "bc_arrows", "cd_arrows", "da_arrows"];
/// Value keys of the card's "Parallel arrows" rows (linked rows: not params, text from `computed`).
pub(crate) const Q_PAR: [&str; 4] = ["ab_par", "bc_par", "cd_par", "da_par"];
const ARROW_OPTS: [&str; 5] = ["Auto", "None", "1", "2", "3"];

/// Which of the eight rows (AB BC CD DA ∠A ∠B ∠C ∠D) have a value box in each quadrilateral;
/// the others are linked ("CD = AB") or fixed (a rectangle's 90°).
pub(crate) fn q_inputs(id: &str) -> [bool; 8] {
    match id {
        "rectangle" => [true, true, false, false, false, false, false, false],
        "parallelogram" => [true, false, false, true, true, true, false, false],
        "kite" => [true, false, false, true, true, true, false, true],
        _ => [true; 8],
    }
}

#[derive(Debug)]
pub(crate) struct QSol { pub pts: [Pt; 4], pub vals: [Option<f64>; 8], pub h: Option<f64>, pub typed: Vec<Option<Typed>> }

fn q_names(p: &Params) -> [String; 8] {
    let n = corner_names(p, 4);
    std::array::from_fn(|i| if i < 4 { format!("{}{}", n[i], n[(i + 1) % 4]) } else { format!("\u{2220}{}", n[i - 4]) })
}

fn q_typed(p: &Params, id: &str) -> Result<Vec<Option<Typed>>, String> {
    let inp = q_inputs(id);
    let names = q_names(p);
    let mut t = Vec::new();
    for i in 0..8 {
        let v = if inp[i] { typed_of(p, Q_V[i])? } else { None };
        range_check(&names[i], &v, i >= 4)?;
        t.push(v);
    }
    Ok(t)
}

/// Checks every given row against the solution.
fn q_check(names: &[String; 8], typed: &[Option<Typed>], vals: &[Option<f64>; 8]) -> Result<(), String> {
    for i in 0..8 {
        if let (Some(t), Some(v)) = (&typed[i], vals[i]) {
            if (t.v - v).abs() > t.tol { return Err(mismatch(&names[i], v, t, i >= 4)); }
        }
    }
    Ok(())
}

fn quad_vals(pts: &[Pt; 4]) -> [Option<f64>; 8] {
    let ang = poly_angles(pts);
    std::array::from_fn(|i| Some(if i < 4 { dist(pts[i], pts[(i + 1) % 4]) } else { ang[i - 4] }))
}

pub(crate) fn rect_solve(p: &Params) -> Result<QSol, String> {
    let names = q_names(p);
    let typed = q_typed(p, "rectangle")?;
    let sq = p.s("kind") == "Square";
    let w = match (&typed[0], &typed[1]) {
        (Some(a), _) => a.v,
        (None, Some(b)) if sq => b.v,
        _ => return Err(format!("Give {} a length.", names[0])),
    };
    let h = if sq { w } else { typed[1].as_ref().map(|t| t.v).ok_or(format!("Give {} a length.", names[1]))? };
    let pts = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
    let vals = quad_vals(&pts);
    q_check(&names, &typed, &vals)?;
    Ok(QSol { pts, vals, h: None, typed })
}

pub(crate) fn para_solve(p: &Params) -> Result<QSol, String> {
    let names = q_names(p);
    let typed = q_typed(p, "parallelogram")?;
    let rh = p.s("kind") == "Rhombus";
    let ab = match (&typed[0], &typed[3]) {
        (Some(a), _) => a.v,
        (None, Some(d)) if rh => d.v,
        _ => return Err(format!("Give {} a length.", names[0])),
    };
    let da = if rh { ab } else { typed[3].as_ref().map(|t| t.v).ok_or(format!("Give {} a length.", names[3]))? };
    let a = match (&typed[4], &typed[5]) {
        (Some(a), _) => a.v,
        (None, Some(b)) => 180.0 - b.v,
        _ => return Err(format!("Give {} in degrees.", names[4])),
    };
    let t = a.to_radians();
    let d = (da * t.cos(), da * t.sin());
    let pts = [(0.0, 0.0), (ab, 0.0), (ab + d.0, d.1), d];
    let vals = quad_vals(&pts);
    q_check(&names, &typed, &vals)?;
    Ok(QSol { pts, vals, h: None, typed })
}

/// Kite ABCD standing on its axis BD: triangle ABD has ∠A, ½∠B, ½∠D and sides AB, DA, so any
/// three of AB, DA, ∠A, ∠B, ∠D that fix that triangle fix the kite.
pub(crate) fn kite_solve(p: &Params) -> Result<QSol, String> {
    let names = q_names(p);
    let typed = q_typed(p, "kite")?;
    let gv = |i: usize, half: bool| typed[i].as_ref().map(|t| if half { (t.v / 2.0, t.tol / 2.0) } else { (t.v, t.tol) });
    let g = [gv(4, false), gv(5, true), gv(7, true), gv(0, false), None, gv(3, false)];
    let out = tri_core(&g, false).map_err(|e| match e {
        TriErr::Few(n) => few(n, "kite"),
        TriErr::Sum2(..) | TriErr::Sum3(_) => format!("These angles can't make a kite: {} + \u{bd}{} + \u{bd}{} has to be 180\u{b0}.", names[4], names[5], names[7]),
        TriErr::Short(x, m) => format!("{} is too short to reach: with these measures it has to be longer than {}.", if x == 1 { &names[3] } else { &names[0] }, fmt(m, 2)),
        TriErr::Obtuse(..) | TriErr::Ineq(_) => "These measures don't make a kite.".to_string(),
        TriErr::Mismatch(i, c) => {
            let (row, v) = match i { 0 => (4, c), 1 => (5, 2.0 * c), 2 => (7, 2.0 * c), 3 => (0, c), _ => (3, c) };
            mismatch(&names[row], v, typed[row].as_ref().unwrap(), row >= 4)
        }
    })?;
    let (ab, bd) = match out.side { Some(s) => (s[0], s[1]), None => {
        // three angles only: shape without size
        let ab = 5.0;
        let half_d = out.ang[2].to_radians();
        (ab, ab * out.ang[0].to_radians().sin() / half_d.sin())
    } };
    let hb = out.ang[1].to_radians();
    let (b, d) = ((0.0, 0.0), (0.0, -bd));
    let a = (-ab * hb.sin(), -ab * hb.cos());
    let c = (-a.0, a.1);
    let pts = [a, b, c, d];
    let mut vals = quad_vals(&pts);
    if out.side.is_none() { for v in vals.iter_mut().take(4) { *v = None; } }
    Ok(QSol { pts, vals, h: None, typed })
}

/// Base-angle right triangle (height h, run along the base, slanted leg, angle at the base): any
/// two fix the rest. Err when the leg is shorter than the height or the run.
fn rt(h: &mut Option<f64>, run: &mut Option<f64>, leg: &mut Option<f64>, ang: &mut Option<f64>) -> Result<(), ()> {
    let n = [h.is_some(), run.is_some(), leg.is_some(), ang.is_some()].iter().filter(|x| **x).count();
    if !(2..4).contains(&n) { return Ok(()); }
    let (sd, cd) = (|d: f64| d.to_radians().sin(), |d: f64| d.to_radians().cos());
    if let (Some(a), Some(l)) = (*ang, *leg) {
        h.get_or_insert(l * sd(a));
        run.get_or_insert(l * cd(a));
    } else if let (Some(a), Some(hh)) = (*ang, *h) {
        leg.get_or_insert(hh / sd(a));
        run.get_or_insert(hh * cd(a) / sd(a));
    } else if let (Some(a), Some(r)) = (*ang, *run) {
        if cd(a).abs() < 1e-9 { return Ok(()); }
        leg.get_or_insert(r / cd(a));
        h.get_or_insert(r * sd(a) / cd(a));
    } else if let (Some(hh), Some(l)) = (*h, *leg) {
        if l < hh - 1e-9 { return Err(()); }
        let r = (l * l - hh * hh).max(0.0).sqrt();
        run.get_or_insert(r);
        ang.get_or_insert(hh.atan2(r).to_degrees());
    } else if let (Some(hh), Some(r)) = (*h, *run) {
        leg.get_or_insert(hh.hypot(r));
        ang.get_or_insert(hh.atan2(r).to_degrees());
    } else if let (Some(r), Some(l)) = (*run, *leg) {
        if l < r.abs() - 1e-9 { return Err(()); }
        let hh = (l * l - r * r).max(0.0).sqrt();
        h.get_or_insert(hh);
        ang.get_or_insert(hh.atan2(r).to_degrees());
    }
    Ok(())
}

/// Trapezoid with AB ∥ CD (AB at the bottom). Kinds: plain (needs four measures, e.g. AB, CD, ∠A
/// and h), isosceles (three: legs and base angles equal) and right (three: DA at right angles to
/// both bases). The left leg makes the triangle (h, offset of D, DA, ∠A), the right leg (h, run
/// of C, BC, ∠B), and AB = CD + offset + run.
pub(crate) fn trap_solve(p: &Params) -> Result<QSol, String> {
    let names = q_names(p);
    let typed = q_typed(p, "trapezoid")?;
    let ht = typed_of(p, "m_h")?;
    range_check("h", &ht, false)?;
    let kind = match p.s("kind") { "Isosceles trapezoid" => 1, "Right trapezoid" => 2, _ => 0 };
    let tv = |i: usize| typed[i].as_ref().map(|t| t.v);
    // ∠D = 180° − ∠A and ∠C = 180° − ∠B (the bases are parallel)
    let mut aa = tv(4).or(tv(7).map(|d| 180.0 - d));
    let mut bb = tv(5).or(tv(6).map(|c| 180.0 - c));
    let (mut ab, mut cd, mut h, mut da, mut bc) = (tv(0), tv(2), ht.as_ref().map(|t| t.v), tv(3), tv(1));
    let (mut off, mut run): (Option<f64>, Option<f64>) = (None, None);
    let count = [ab.is_some(), cd.is_some(), h.is_some(), da.is_some(), bc.is_some(), aa.is_some(), bb.is_some()].iter().filter(|x| **x).count();
    let need = if kind == 0 { 4 } else { 3 };
    if kind == 2 {
        if let Some(t) = typed[4].as_ref().or(typed[7].as_ref()) { if (t.v - 90.0).abs() > t.tol { return Err(mismatch(if typed[4].is_some() { &names[4] } else { &names[7] }, 90.0, t, true)); } }
        off = Some(0.0);
        aa = Some(90.0);
    }
    let effective = if kind == 2 { count - usize::from(tv(4).is_some() || tv(7).is_some()) } else { count };
    if effective < need { return Err(few(need - effective, "trapezoid")); }
    let short = |leg: &str| format!("{leg} is too short to reach between the bases.");
    for _ in 0..12 {
        rt(&mut h, &mut off, &mut da, &mut aa).map_err(|_| short(&names[3]))?;
        rt(&mut h, &mut run, &mut bc, &mut bb).map_err(|_| short(&names[1]))?;
        if kind == 1 {
            if off.is_none() { off = run; } else if run.is_none() { run = off; }
            if da.is_none() { da = bc; } else if bc.is_none() { bc = da; }
            if aa.is_none() { aa = bb; } else if bb.is_none() { bb = aa; }
            if let (Some(a), Some(c), None) = (ab, cd, off) { off = Some((a - c) / 2.0); run = off; }
            if let (Some(o), None, Some(c)) = (off, ab, cd) { ab = Some(c + 2.0 * o); }
            if let (Some(o), Some(a), None) = (off, ab, cd) { cd = Some(a - 2.0 * o); }
        }
        match (ab, cd, off, run) {
            (None, Some(c), Some(o), Some(r)) => ab = Some(c + o + r),
            (Some(a), None, Some(o), Some(r)) => cd = Some(a - o - r),
            (Some(a), Some(c), None, Some(r)) => off = Some(a - c - r),
            (Some(a), Some(c), Some(o), None) => run = Some(a - c - o),
            _ => {}
        }
    }
    // plain trapezoid with AB, CD and one measure on each leg: find h numerically
    if h.is_none() && kind == 0 {
        if let (Some(a), Some(c)) = (ab, cd) {
            let side = |hh: f64, aa: Option<f64>, leg: Option<f64>| -> Option<f64> {
                if let Some(t) = aa { return Some(hh / t.to_radians().tan()); }
                leg.filter(|l| *l >= hh).map(|l| (l * l - hh * hh).sqrt())
            };
            let f = |hh: f64| Some(side(hh, aa, da)? + side(hh, bb, bc)? - (a - c));
            let hmax = [da, bc].iter().flatten().fold(10.0 * (a + c).abs().max(1.0), |m, x| m.min(*x));
            let mut lo = None;
            let steps = 400;
            let mut prev: Option<(f64, f64)> = None;
            for k in 1..=steps {
                let x = hmax * k as f64 / steps as f64;
                if let Some(fx) = f(x) {
                    if let Some((px, pf)) = prev { if pf.signum() != fx.signum() || fx == 0.0 { lo = Some((px, x)); break; } }
                    prev = Some((x, fx));
                }
            }
            if let Some((mut l, mut r)) = lo {
                for _ in 0..80 {
                    let m = (l + r) / 2.0;
                    match (f(l), f(m)) { (Some(fl), Some(fm)) if fl.signum() == fm.signum() => l = m, _ => r = m }
                }
                h = Some((l + r) / 2.0);
                for _ in 0..3 {
                    rt(&mut h, &mut off, &mut da, &mut aa).map_err(|_| short(&names[3]))?;
                    rt(&mut h, &mut run, &mut bc, &mut bb).map_err(|_| short(&names[1]))?;
                }
            }
        }
    }
    let (Some(ab), Some(cd), Some(h), Some(off)) = (ab, cd, h, off) else { return Err("These measures don't fix the trapezoid.".to_string()) };
    if cd <= 1e-9 { return Err(format!("{} has to be longer than 0: these measures leave no room for it.", names[2])); }
    if ab <= 1e-9 { return Err(format!("{} has to be longer than 0: these measures leave no room for it.", names[0])); }
    let pts = [(0.0, 0.0), (ab, 0.0), (off + cd, h), (off, h)];
    let vals = quad_vals(&pts);
    q_check(&names, &typed, &vals)?;
    if let Some(t) = &ht { if (t.v - h).abs() > t.tol { return Err(mismatch("h", h, t, false)); } }
    Ok(QSol { pts, vals, h: Some(h), typed })
}

pub(crate) fn quad_solve(id: &str, p: &Params) -> Result<QSol, String> {
    match id {
        "rectangle" => rect_solve(p),
        "parallelogram" => para_solve(p),
        "kite" => kite_solve(p),
        _ => trap_solve(p),
    }
}

/// Rows of each quadrilateral that are linked to another row (no value box, never "given").
fn q_linked(id: &str) -> [bool; 8] {
    let inp = q_inputs(id);
    std::array::from_fn(|i| !inp[i])
}

fn quad_params(id: &str, vals: &[(&'static str, &'static str)]) -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::text("name_a", "Name of corner A", "A"),
        ParamDef::text("name_b", "Name of corner B", "B"),
        ParamDef::text("name_c", "Name of corner C", "C"),
        ParamDef::text("name_d", "Name of corner D", "D"),
    ];
    let inp = q_inputs(id);
    for (i, &k) in Q_V.iter().enumerate() {
        if !inp[i] { continue; }
        let d = vals.iter().find(|x| x.0 == k).map(|x| x.1).unwrap_or("");
        let label = ["Side AB", "Side BC", "Side CD", "Side DA", "Angle A", "Angle B", "Angle C", "Angle D"][i];
        v.push(ParamDef { key: k, label, kind: Kind::Text, default: Value::S(d.into()) });
    }
    label_params(&mut v, &Q_LAB, &Q_TXT, &Q_MARK, false, 4);
    for (k, d) in vals {
        if let Some(pd) = v.iter_mut().find(|pd| pd.key == *k) { pd.default = Value::S(d.to_string()); }
    }
    v
}

fn diag_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("diagonals", "Diagonals", &["None", "Drawn", "Drawn and labelled", "AC only", "BD only"], "None"),
        ParamDef::choice("ac_lab", "Diagonal AC label", &LAB_OPTS, "None"),
        ParamDef::text("ac_label", "Diagonal AC label text", "d"),
        ParamDef::choice("bd_lab", "Diagonal BD label", &LAB_OPTS, "None"),
        ParamDef::text("bd_label", "Diagonal BD label text", "e"),
        ParamDef::list("arrow_marks", "Parallel arrows per side, e.g. AB 1, CD 1 (or none; blank = the figure's default)", ""),
        ParamDef::choice("ab_arrows", "Parallel arrows on AB", &ARROW_OPTS, "Auto"),
        ParamDef::choice("bc_arrows", "Parallel arrows on BC", &ARROW_OPTS, "Auto"),
        ParamDef::choice("cd_arrows", "Parallel arrows on CD", &ARROW_OPTS, "Auto"),
        ParamDef::choice("da_arrows", "Parallel arrows on DA", &ARROW_OPTS, "Auto"),
    ]
}

/// Arrows drawn on each side: the per-side choice, else the `arrow_marks` list, else the kind's own.
pub(crate) fn quad_arrows(p: &Params, default_arrows: &[u8]) -> Vec<u8> {
    let names = corner_names(p, 4);
    let base = parse_counts(p.s("arrow_marks"), &names, true).unwrap_or_else(|| default_arrows.to_vec());
    (0..4).map(|i| match p.s(Q_ARROW[i]) { "None" => 0, "1" => 1, "2" => 2, "3" => 3, _ => base.get(i).copied().unwrap_or(0) }).collect()
}
/// Each kind's own parallel arrows (drawn while a side is on "Auto").
pub(crate) fn quad_default_arrows(id: &str) -> Vec<u8> {
    match id { "parallelogram" => vec![1, 2, 1, 2], "trapezoid" => vec![1, 0, 1, 0], _ => vec![0; 4] }
}

/// Diagonals AC / BD with their own label settings ("Drawn and labelled" forces the measure) and a
/// right-angle mark where they cross at 90°.
fn diagonals(p: &Params, pts: &[Pt], ex: &mut Vec<Extra>, mk: &mut Vec<Mark>) {
    let sel = p.s("diagonals");
    let (ac, bd) = match sel { "" | "None" => (false, false), "AC only" => (true, false), "BD only" => (false, true), _ => (true, true) };
    let forced = sel.contains("labelled");
    let dash = dash_of(p);
    for (on, i, j, t, lab, txt) in [(ac, 0usize, 2usize, 0.3, "ac_lab", "ac_label"), (bd, 1, 3, 0.72, "bd_lab", "bd_label")] {
        if !on { continue; }
        let label = if forced { Some("=".to_string()) } else {
            match p.s(lab) {
                "Measure" => Some("=".to_string()),
                "Text" => Some({ let t = p.s(txt).trim(); if t.is_empty() { "x".to_string() } else { t.to_string() } }),
                _ => None,
            }
        };
        ex.push(Extra { a: pts[i], b: pts[j], label, t, dash, raw: true, id: if i == 0 { "ac" } else { "bd" } });
    }
    if ac && bd {
        let (u, w) = (unit_v(pts[0], pts[2]), unit_v(pts[1], pts[3]));
        if (u.0 * w.0 + u.1 * w.1).abs() < 0.005 {
            let den = u.0 * w.1 - u.1 * w.0;
            let s = ((pts[1].0 - pts[0].0) * w.1 - (pts[1].1 - pts[0].1) * w.0) / den;
            let x = (pts[0].0 + u.0 * s, pts[0].1 + u.1 * s);
            mk.push((x, pts[2], pts[1]));
        }
    }
}

fn quad_draw(id: &str, p: &Params, s: &QSol, default_arrows: Vec<u8>, mut ex: Vec<Extra>, mut mk: Vec<Mark>) -> String {
    let (dec, unit) = (p.i("dec"), p.s("unit").to_string());
    let linked = q_linked(id);
    let lab = |i: usize| lab_text(p, Q_LAB[i], Q_TXT[i], s.typed[i].as_ref(), s.vals[i], linked[i], i >= 4, dec, &unit);
    let pts = s.pts.to_vec();
    diagonals(p, &pts, &mut ex, &mut mk);
    let names = corner_names(p, 4);
    let arrows = quad_arrows(p, &default_arrows);
    draw_poly(PolySpec {
        pts,
        names,
        side_txt: (0..4).map(lab).collect(),
        ang_txt: (4..8).map(lab).collect(),
        ticks: (0..4).map(|i| tick_n(p, Q_MARK[i])).collect(),
        arrows,
        arcs: (4..8).map(|i| tick_n(p, Q_MARK[i])).collect(),
        right_marks: p.b("right"),
        vlabels: p.b("vertices"),
        extras: ex, marks: mk, dots: vec![], notes: vec![],
        rot: p.f("rotate"), flip: p.b("flip"), bottom: bottom_side(p, 4), fill: fill_of(p), fs: fscale(p),
    }, dec, &unit)
}

const QUAD_BOTTOM2: [&str; 5] = ["AB, as it stands", "AB", "BC", "CD", "DA"];

pub(crate) fn rectangle_params() -> Vec<ParamDef> {
    let mut v = vec![ParamDef::choice("kind", "Kind", &["Rectangle", "Square"], "Rectangle")];
    v.extend(quad_params("rectangle", &[("m_ab", "10"), ("m_bc", "6"), ("ab_lab", "Measure"), ("bc_lab", "Measure")]));
    v.extend(diag_params());
    v.push(ParamDef::choice("bottom", "Side at the bottom", &QUAD_BOTTOM2, "AB, as it stands"));
    shared_tail(&mut v, true, true);
    v
}

pub(crate) fn rectangle_render(p: &Params) -> String {
    match rect_solve(p) { Ok(s) => quad_draw("rectangle", p, &s, vec![0; 4], vec![], vec![]), Err(e) => note_fig(&e) }
}

fn heights_of(p: &Params, list: Vec<(Pt, Pt, Pt)>) -> (Vec<Extra>, Vec<Mark>) {
    let (mut ex, mut mk) = (vec![], vec![]);
    for (v, q1, q2) in list {
        let (e, m, _) = height_from(v, q1, q2, height_label(p), dash_of(p));
        ex.extend(e);
        mk.extend(m);
    }
    (ex, mk)
}

pub(crate) fn parallelogram_params() -> Vec<ParamDef> {
    let mut v = vec![ParamDef::choice("kind", "Kind", &["Parallelogram", "Rhombus"], "Parallelogram")];
    v.extend(quad_params("parallelogram", &[("m_ab", "10"), ("m_da", "6"), ("m_a", "60"), ("ab_lab", "Measure"), ("da_lab", "Measure"), ("a_lab", "Measure")]));
    v.push(ParamDef::choice("height", "Height", &["None", "Onto AB (from D)", "Onto AD (from B)", "Onto AB (from C)", "Onto AB (from D and C)"], "None"));
    v.extend(diag_params());
    v.push(ParamDef::choice("bottom", "Side at the bottom", &QUAD_BOTTOM2, "AB, as it stands"));
    shared_tail(&mut v, true, true);
    v.extend(height_common());
    v
}

pub(crate) fn parallelogram_render(p: &Params) -> String {
    let s = match para_solve(p) { Ok(s) => s, Err(e) => return note_fig(&e) };
    let [a, b, c, d] = s.pts;
    let hs: Vec<(Pt, Pt, Pt)> = match p.s("height") {
        "Onto AB (from D)" => vec![(d, a, b)],
        "Onto AD (from B)" => vec![(b, a, d)],
        "Onto AB (from C)" => vec![(c, a, b)],
        "Onto AB (from D and C)" => vec![(d, a, b), (c, a, b)],
        _ => vec![],
    };
    let (ex, mk) = heights_of(p, hs);
    quad_draw("parallelogram", p, &s, vec![1, 2, 1, 2], ex, mk)
}

pub(crate) fn trapezoid_params() -> Vec<ParamDef> {
    let mut v = vec![ParamDef::choice("kind", "Kind", &["Trapezoid", "Isosceles trapezoid", "Right trapezoid"], "Right trapezoid")];
    v.extend(quad_params("trapezoid", &[("m_ab", "12"), ("m_cd", "7"), ("m_da", "5"), ("ab_lab", "Measure"), ("cd_lab", "Measure"), ("da_lab", "Measure"), ("bc_lab", "Text")]));
    v.push(ParamDef::text("m_h", "Height between the bases (blank = worked out)", ""));
    v.extend([
        ParamDef::bool("show_height", "Draw the height", false),
        ParamDef::choice("hfrom", "Height drawn from", &["D", "C", "D and C"], "D"),
    ]);
    v.extend(diag_params());
    v.push(ParamDef::choice("bottom", "Side at the bottom", &QUAD_BOTTOM2, "AB, as it stands"));
    shared_tail(&mut v, true, true);
    v.extend(height_common());
    v
}

pub(crate) fn trapezoid_render(p: &Params) -> String {
    let s = match trap_solve(p) { Ok(s) => s, Err(e) => return note_fig(&e) };
    let [a, b, c, d] = s.pts;
    let h = s.h.unwrap_or(d.1);
    let ab = b.0;
    let off = d.0;
    let (mut ex, mut mk) = (vec![], vec![]);
    if p.b("show_height") {
        let hf = p.s("hfrom");
        let (from_d, from_c) = (hf != "C", hf == "C" || hf == "D and C");
        let (lab, dash) = (height_label(p), dash_of(p));
        if from_d && off.abs() > 1e-9 {
            let (e, m, _) = height_from(d, a, b, lab.clone(), dash);
            ex.extend(e);
            mk.extend(m);
        } else if from_d {
            ex.push(Extra { a: (ab / 2.0, 0.0), b: (ab / 2.0, h), label: lab.0.clone(), t: 0.5, dash, raw: lab.1, id: "h" });
            mk.push(((ab / 2.0, 0.0), (ab / 2.0, h), (ab, 0.0)));
        }
        if from_c && (c.0 - ab).abs() > 1e-9 {
            let (e, m, _) = height_from(c, a, b, lab, dash);
            ex.extend(e);
            mk.extend(m);
        }
    }
    quad_draw("trapezoid", p, &s, vec![1, 0, 1, 0], ex, mk)
}

pub(crate) fn kite_params() -> Vec<ParamDef> {
    let mut v = quad_params("kite", &[("m_ab", "5"), ("m_da", "9"), ("m_b", "100"), ("ab_lab", "Measure"), ("da_lab", "Measure"), ("b_lab", "Measure"), ("ab_ticks", "1"), ("bc_ticks", "1"), ("cd_ticks", "2"), ("da_ticks", "2")]);
    v.push(ParamDef::choice("height", "Height", &["None", "From D to AB", "From C to AB", "From D and C to AB"], "None"));
    v.extend(diag_params());
    v.push(ParamDef::choice("bottom", "Side at the bottom", &["None (standing upright)", "AB", "BC", "CD", "DA"], "None (standing upright)"));
    shared_tail(&mut v, true, true);
    v.extend(height_common());
    v
}

pub(crate) fn kite_render(p: &Params) -> String {
    let s = match kite_solve(p) { Ok(s) => s, Err(e) => return note_fig(&e) };
    let [a, b, c, d] = s.pts;
    let hs: Vec<(Pt, Pt, Pt)> = match p.s("height") { "From D to AB" => vec![(d, a, b)], "From C to AB" => vec![(c, a, b)], "From D and C to AB" => vec![(d, a, b), (c, a, b)], _ => vec![] };
    let (ex, mk) = heights_of(p, hs);
    quad_draw("kite", p, &s, vec![0; 4], ex, mk)
}

/// (value_key, text) for every row of a quadrilateral that is worked out or linked.
pub(crate) fn quad_computed(id: &str, p: &Params) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    // the "Parallel arrows" rows' buttons: the arrows each side gets
    let arr = quad_arrows(p, &quad_default_arrows(id));
    for i in 0..4 {
        let chev = "\u{203a}".repeat(arr[i] as usize);
        let auto = matches!(p.s(Q_ARROW[i]), "" | "Auto");
        out.push((Q_PAR[i], match (auto, arr[i]) { (true, 0) => "auto".to_string(), (true, _) => format!("{chev} auto"), _ => chev }));
    }
    // the solver's reason (too few / too many / inconsistent givens) shows under the card
    let s = match quad_solve(id, p) { Ok(s) => s, Err(e) => { out.push(("__msg", e)); return out; } };
    let dec = p.i("dec");
    let inp = q_inputs(id);
    for i in 0..8 {
        let Some(v) = s.vals[i] else { continue };
        if inp[i] && s.typed[i].is_some() { continue; }
        out.push((Q_V[i], fmt(v, dec)));
    }
    if id == "trapezoid" { if let Some(h) = s.h { out.push(("m_h", fmt(h, dec))); } }
    out
}

/// Writes worked-out measures from dragged geometry back into the rows that are given.
pub(crate) fn write_givens(p: &mut Params, keys: &[&str], vals: &[Option<f64>]) {
    for (k, v) in keys.iter().zip(vals) {
        if k.is_empty() || p.s(k).trim().is_empty() { continue; }
        if let Some(v) = v { if v.is_finite() && *v > 0.0 { p.set_s(k, &num_str(*v, 2)); } }
    }
}

pub(crate) fn quad_vals_of(pts: &[Pt; 4]) -> [Option<f64>; 8] { quad_vals(pts) }

// ----- regular polygon -----

pub(crate) fn regular_polygon_params() -> Vec<ParamDef> {
    vec![
        ParamDef::int("n", "Sides", 3, 20, 6),
        ParamDef::choice("from", "Size from", &["Side", "Radius", "Apothem"], "Side"),
        ParamDef::float("value", "Size", 0.5, 100.0, 0.5, 8.0),
        ParamDef::choice("side_lab", "Side label", &LAB_OPTS, "Measure"),
        ParamDef::text("side_label", "Side label text", "s"),
        ParamDef::choice("side_ticks", "Congruence ticks (every side)", &TICKS, "None"),
        ParamDef::choice("ang_lab", "Angle label", &LAB_OPTS, "None"),
        ParamDef::text("ang_label", "Angle label text", "x"),
        ParamDef::choice("ang_arcs", "Congruence arcs (every angle)", &TICKS, "None"),
        ParamDef::bool("center_dot", "Center dot", true),
        ParamDef::text("center_name", "Name the center (e.g. O)", ""),
        ParamDef::bool("apothem", "Apothem (center to the middle of the bottom side)", true),
        ParamDef::choice("apothem_label", "Apothem label", &["a = measure", "Measure", "Text", "None"], "Text"),
        ParamDef::text("apothem_text", "Apothem name / text", "a"),
        ParamDef::bool("radius", "Radius (center to the bottom right corner)", false),
        ParamDef::choice("radius_label", "Radius label", &["r = measure", "Measure", "Text", "None"], "Text"),
        ParamDef::text("radius_text", "Radius name / text", "r"),
        ParamDef::bool("all_radii", "All the radii (center to every corner)", false),
        ParamDef::bool("vertices", "Letter the corners A, B, C... from the bottom left", false),
        ParamDef::text("vnames", "Corner letters in order (e.g. P, Q, R; blank = A, B, C)", ""),
        ParamDef::text("unit", "Unit (e.g. cm)", ""),
        ParamDef::int("dec", "Round to (decimal places)", 0, 3, 1),
        ParamDef::bool("right", "Right-angle squares where the apothem meets its side", true),
        ParamDef::float("rotate", "Turn (degrees)", -180.0, 180.0, 1.0, 0.0),
        ParamDef::color("fill", "Fill colour", &["#e9f1fb", "#fdf0d5", "#e3f4e1", "#fbe3e3", "#efe5f7", "#ffffff"], "#e9f1fb"),
        ParamDef::bool("flip", "Flip (mirror left to right)", false),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        ParamDef::choice("line_style", "Construction lines (radius, apothem)", &["Solid", "Dashed", "Dotted"], "Dashed"),
    ]
}

/// (side, radius, apothem, interior angle, central angle) of the regular polygon.
pub(crate) fn rp_measures(p: &Params) -> (f64, f64, f64, f64, f64) {
    let n = p.i("n").clamp(3, 20) as f64;
    let v = p.f("value");
    let half = PI / n;
    let r = match p.s("from") { "Radius" => v, "Apothem" => v / half.cos(), _ => v / (2.0 * half.sin()) };
    (2.0 * r * half.sin(), r, r * half.cos(), (n - 2.0) * 180.0 / n, 360.0 / n)
}

pub(crate) fn regular_polygon_pts(p: &Params) -> Vec<Pt> {
    let n = p.i("n").clamp(3, 20) as usize;
    let (_, r, _, _, _) = rp_measures(p);
    let half = PI / n as f64;
    let phi0 = -PI / 2.0 - half;
    (0..n).map(|k| { let t = phi0 + 2.0 * PI * k as f64 / n as f64; (r * t.cos(), r * t.sin()) }).collect()
}

pub(crate) fn regular_polygon_render(p: &Params) -> String {
    let pts = regular_polygon_pts(p);
    let n = pts.len();
    let o = (0.0, 0.0);
    let dash = dash_of(p);
    let (dec, unit) = (p.i("dec"), p.s("unit").to_string());
    let (side, _, _, ang, _) = rp_measures(p);
    let (mut ex, mut mk, mut dots, mut notes) = (vec![], vec![], vec![], vec![]);
    if p.b("center_dot") { dots.push(o); }
    let rad_at = 1usize.min(n - 1); // bottom right corner
    if p.b("all_radii") {
        for (k, q) in pts.iter().enumerate() {
            if k == rad_at && p.b("radius") { continue; }
            ex.push(Extra { a: o, b: *q, t: 0.5, dash, ..Default::default() });
        }
    }
    if p.b("radius") {
        let (label, raw) = named_label(p.s("radius_label"), p.s("radius_text"), "r");
        ex.push(Extra { a: o, b: pts[rad_at], label, t: 0.5, dash, raw, id: "r" });
    }
    if p.b("apothem") {
        let mid = ((pts[0].0 + pts[1].0) / 2.0, (pts[0].1 + pts[1].1) / 2.0);
        let (label, raw) = named_label(p.s("apothem_label"), p.s("apothem_text"), "a");
        ex.push(Extra { a: o, b: mid, label, t: 0.5, dash, raw, id: "ap" });
        mk.push((mid, o, pts[1]));
    }
    let cname = p.s("center_name").trim();
    if !cname.is_empty() {
        let k = (n / 2) as f64;
        let t = -PI / 2.0 + 2.0 * PI * k / n as f64;
        notes.push((o, (t.cos(), t.sin()), cname.to_string()));
    }
    let given_side = p.s("from") == "Side";
    let side_mode = if p.s("side_lab").is_empty() { if given_side { "Measure" } else { "None" } } else { p.s("side_lab") };
    let side_txt = match side_mode {
        "None" => None,
        "Text" => Some({ let t = p.s("side_label").trim(); if t.is_empty() { "s".to_string() } else { t.to_string() } }),
        _ => Some(fmtu(side, dec, &unit)),
    };
    let ang_txt = match if p.s("ang_lab").is_empty() { "None" } else { p.s("ang_lab") } {
        "None" => None,
        "Text" => Some({ let t = p.s("ang_label").trim(); if t.is_empty() { "x".to_string() } else { t.to_string() } }),
        _ => Some(format!("{}\u{b0}", fmt(ang, dec))),
    };
    let names = vnames(p, n);
    let st = tick_n(p, "side_ticks");
    let sa = tick_n(p, "ang_arcs");
    draw_poly(PolySpec {
        pts, names,
        side_txt: (0..n).map(|i| if i == 0 { side_txt.clone() } else { None }).collect(),
        ang_txt: (0..n).map(|i| if i == 0 { ang_txt.clone() } else { None }).collect(),
        ticks: vec![st; n], arrows: vec![0; n], arcs: vec![sa; n],
        right_marks: p.b("right"), vlabels: p.b("vertices"),
        extras: ex, marks: mk, dots, notes,
        rot: p.f("rotate"), flip: p.b("flip"), bottom: None, fill: fill_of(p), fs: fscale(p),
    }, dec, &unit)
}

pub(crate) fn rp_computed(p: &Params) -> Vec<(&'static str, String)> {
    let dec = p.i("dec");
    let (side, r, a, ang, _) = rp_measures(p);
    vec![("m_side", fmt(side, dec)), ("m_angle", format!("{}\u{b0}", fmt(ang, dec))), ("m_apothem", fmt(a, dec)), ("m_radius", fmt(r, dec))]
}

// ----- angles (rays card) -----

pub(crate) const RAY_V: [&str; 6] = ["ray1", "ray2", "ray3", "ray4", "ray5", "ray6"];
pub(crate) const RAY_STYLE: [&str; 6] = ["ray1_style", "ray2_style", "ray3_style", "ray4_style", "ray5_style", "ray6_style"];
pub(crate) const RAY_LINE: [&str; 6] = ["ray1_line", "ray2_line", "ray3_line", "ray4_line", "ray5_line", "ray6_line"];

pub(crate) fn angles_params() -> Vec<ParamDef> {
    let mut v = vec![];
    for (i, &k) in RAY_V.iter().enumerate().skip(1) {
        v.push(ParamDef { key: k, label: ["", "Ray 2 direction", "Ray 3 direction", "Ray 4 direction", "Ray 5 direction", "Ray 6 direction"][i], kind: Kind::Text, default: Value::S(if i == 1 { "60".into() } else { String::new() }) });
    }
    for i in 0..6 {
        v.push(ParamDef::choice(RAY_STYLE[i], "Line", &STYLES, "Solid"));
        v.push(ParamDef::choice(RAY_LINE[i], "Drawn as", &["Ray", "Line"], "Ray"));
    }
    v.extend([
        ParamDef::bool("around", "Angle back around to the baseline (angles around a point)", false),
        ParamDef::list("unknown", "Sector numbers shown as x (counterclockwise from baseline, e.g. 2)", ""),
        ParamDef::list("hide", "Sector numbers left unlabelled", ""),
        ParamDef::long("sector_text", "Text labels for sectors, e.g. 1 = 2x + 10; 3 = y", ""),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::bool("letters", "Point letters on the rays", true),
        ParamDef::int("dec", "Round measures to (decimal places)", 0, 3, 1),
        ParamDef::float("turn", "Turn the figure (degrees)", -180.0, 180.0, 1.0, 0.0),
        ParamDef::choice("ray_ends", "Ray ends", &["Arrows", "Plain", "Points"], "Arrows"),
        ParamDef::bool("shade", "Shade the angles", true),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        // "Add ray" (chips card) counts the rays in use; an empty row within the count is drawn
        // in the middle of the widest gap, as the reference places a new ray.
        ParamDef::int("ray_count", "Rays in use, the baseline included", 2, 6, 2),
    ]);
    v
}

/// The middle of the widest gap between `dirs` (degrees), searched over 0-180° while every ray is
/// in the upper half, else round the whole circle. Whole degrees, like the reference's new rays.
pub(crate) fn widest_gap_mid(dirs: &[f64]) -> f64 {
    let upper = if dirs.iter().all(|d| *d <= 180.0 + 1e-9) { 180.0 } else { 360.0 };
    let mut pts: Vec<f64> = dirs.iter().copied().filter(|d| *d <= upper).collect();
    pts.extend([0.0, upper]);
    pts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut best = (0.0, upper);
    for w in pts.windows(2) {
        if w[1] - w[0] > best.1 - best.0 + 1e-9 || best == (0.0, upper) { best = (w[0], w[1]); }
    }
    ((best.0 + best.1) / 2.0).round()
}

/// Ray directions with their source row: typed rows first, then each empty row within
/// `ray_count` placed in the widest gap left by the rays before it (bool: placed automatically).
pub(crate) fn ray_rows(p: &Params) -> Vec<(usize, f64, bool)> {
    let cnt = p.i("ray_count").clamp(2, 6) as usize;
    let mut out: Vec<(usize, f64, bool)> = vec![];
    for i in 1..6 {
        if let Ok(Some(t)) = parse_typed(p.s(RAY_V[i])) { out.push((i, t.v.rem_euclid(360.0), false)); }
    }
    for i in 1..cnt {
        if !p.s(RAY_V[i]).trim().is_empty() { continue; }
        let mut have: Vec<f64> = out.iter().map(|r| r.1).collect();
        have.push(0.0);
        out.push((i, widest_gap_mid(&have), true));
    }
    out
}

/// Ray directions in degrees ccw from the baseline, sorted: (direction, source row 0-5, is the
/// back half of a ray drawn as a line), and whether the last sector wraps back to the baseline.
pub(crate) fn angle_dirs(p: &Params) -> (Vec<(f64, usize, bool)>, bool) {
    let mut d: Vec<(f64, usize, bool)> = vec![(0.0, 0, false)];
    for (i, a, _) in ray_rows(p) { d.push((a, i, false)); }
    let mut lines = false;
    for k in 0..d.len() {
        let (a, i, _) = d[k];
        if p.s(RAY_LINE[i]) == "Line" { d.push(((a + 180.0).rem_euclid(360.0), i, true)); lines = true; }
    }
    d.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    d.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-6);
    (d, lines || p.b("around"))
}

fn style_dash(s: &str) -> &'static str { match s { "Dashed" => "8 5", "Dotted" => "0.1 5", _ => "none" } }

pub(crate) fn angles_render(p: &Params) -> String {
    let (dirs, wrap) = angle_dirs(p);
    if dirs.len() < 2 { return note_fig("Give ray 2 a direction in degrees."); }
    let rays: Vec<f64> = dirs.iter().map(|d| d.0).collect();
    let turn = p.f("turn");
    let fs = fscale(p);
    let (w, h) = (480.0, 360.0);
    let c = (w / 2.0, h / 2.0);
    let len = 140.0;
    let unk: Vec<usize> = nums(p.s("unknown")).iter().map(|v| *v as usize).collect();
    let hide: Vec<usize> = nums(p.s("hide")).iter().map(|v| *v as usize).collect();
    let texts = num_text_map(p.s("sector_text"));
    let mut d = new_doc(w, h);
    let mut secs: Vec<(f64, f64)> = rays.windows(2).map(|w| (w[0], w[1])).collect();
    if wrap { secs.push((*rays.last().unwrap(), rays[0] + 360.0)); }
    if p.b("shade") {
        for (a0, a1) in &secs {
            let n = 24;
            let mut pts = vec![c];
            for i in 0..=n { let t = (turn + a0 + (a1 - a0) * i as f64 / n as f64).to_radians(); pts.push((c.0 + 42.0 * t.cos(), c.1 - 42.0 * t.sin())); }
            d.poly(&pts, "#eef4fb", "none", 0.0);
        }
    }
    for (i, (r, src, _)) in dirs.iter().enumerate() {
        let t = (r + turn).to_radians();
        let e = (c.0 + len * t.cos(), c.1 - len * t.sin());
        let dash = style_dash(p.s(RAY_STYLE[*src]));
        dline(&mut d, c, e, INK, 2.0, dash);
        match p.s("ray_ends") {
            "Plain" => {}
            "Points" => { d.circle(e.0, e.1, 3.4, INK, INK, 1.0); }
            _ => {
                let u = unit_v(c, e);
                let nn = (-u.1, u.0);
                let back = (e.0 - u.0 * 12.0, e.1 - u.1 * 12.0);
                d.poly(&[e, (back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, INK, 1.0);
            }
        }
        if p.b("letters") {
            let name = ((b'B' + i as u8) as char).to_string();
            mlabel(&mut d, c.0 + (len + 16.0) * t.cos(), c.1 - (len + 16.0) * t.sin(), 17.0 * fs, &name, "");
        }
    }
    d.circle(c.0, c.1, 3.0, INK, INK, 1.0);
    if p.b("letters") && !wrap {
        let back = ((rays[0] + rays[rays.len() - 1]) / 2.0 + 180.0 + turn).to_radians();
        mlabel(&mut d, c.0 + 18.0 * back.cos(), c.1 - 18.0 * back.sin(), 17.0 * fs, "A", "");
    }
    for (i, (a0, a1)) in secs.iter().enumerate() {
        let idx = i + 1;
        let span = a1 - a0;
        let txt = if hide.contains(&idx) { String::new() }
            else if let Some((_, t)) = texts.iter().find(|(k, _)| *k == idx) { t.clone() }
            else if unk.contains(&idx) { "x".into() } else { format!("{}\u{b0}", fmt(span, p.i("dec"))) };
        let rad = if span < 28.0 { 48.0 + (i % 2) as f64 * 10.0 } else { 36.0 + (i % 2) as f64 * 6.0 };
        sector(&mut d, c, turn + a0, turn + a1, rad, &txt, p.b("right"), 15.0 * fs);
    }
    d.finish()
}

pub(crate) fn angles_computed(p: &Params) -> Vec<(&'static str, String)> {
    let mut out = vec![("ray1", "0".to_string())];
    // a row added with "Add ray" and left empty shows where its ray is drawn
    for (i, a, auto) in ray_rows(p) { if auto { out.push((RAY_V[i], fmt(a, 1))); } }
    if angle_dirs(p).0.len() < 2 { out.push(("__msg", "Give ray 2 a direction in degrees.".to_string())); }
    out
}

// ----- parallel lines (lines card) -----

pub(crate) const PL_NAMES: [&str; 5] = ["name_m", "name_n", "name_p", "name_t", "name_u"];
pub(crate) const PL_STYLE: [&str; 5] = ["m_style", "n_style", "p_style", "t_style", "u_style"];
pub(crate) const PL_ARROWS: [&str; 5] = ["m_arrows", "n_arrows", "p_arrows", "t_arrows", "u_arrows"];

pub(crate) fn parallel_lines_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::int("lines", "Parallel lines", 2, 3, 2),
        ParamDef::int("transversals", "Transversals", 1, 2, 1),
        ParamDef::text("name_m", "Name of the first line", "m"),
        ParamDef::text("name_n", "Name of the second line", "n"),
        ParamDef::text("name_p", "Name of the third line", "p"),
        ParamDef::text("name_t", "Name of the transversal", "t"),
        ParamDef::text("name_u", "Name of the second transversal", "u"),
        ParamDef::float("angle", "Angle above m, right of the transversal (degrees)", 10.0, 170.0, 1.0, 65.0),
        ParamDef::float("angle_b", "Second transversal angle (degrees; its angles number on after the first's)", 10.0, 170.0, 1.0, 115.0),
    ];
    for i in 0..5 {
        v.push(ParamDef::choice(PL_STYLE[i], "Line", &STYLES, "Solid"));
        v.push(ParamDef::choice(PL_ARROWS[i], "Parallel arrows", &TICKS, if i < 3 { "1" } else { "None" }));
    }
    v.extend([
        ParamDef::float("gap", "Gap between lines", 0.5, 3.0, 0.1, 1.0),
        ParamDef::choice("labels", "Label style", &["Degrees", "Numbers"], "Degrees"),
        ParamDef::list("marked", "Angles to label (1-8 at the first two crossings, 5-8 at m and n, 9-12 at the third line), e.g. 2, 6", "2, 6"),
        ParamDef::list("unknown", "Marked angles shown as x", "6"),
        ParamDef::long("angle_text", "Text labels for marked angles, e.g. 2 = 3x + 5; 6 = y", ""),
        ParamDef::bool("names", "Line names (m, n, t)", true),
        ParamDef::int("dec", "Round measures to (decimal places)", 0, 3, 1),
        ParamDef::float("turn", "Turn the figure (degrees)", -180.0, 180.0, 1.0, 0.0),
        ParamDef::choice("line_ends", "Line ends", &["Arrowheads", "Plain"], "Arrowheads"),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
    ]);
    v
}

/// `k` parallel-arrow chevrons centred at c pointing along unit u.
fn chevrons(d: &mut Doc, c: Pt, u: Pt, k: u8) {
    let nn = (-u.1, u.0);
    for j in 0..k {
        let o = (j as f64 - (k as f64 - 1.0) / 2.0) * 8.0;
        let m = (c.0 + u.0 * o, c.1 + u.1 * o);
        let tip = (m.0 + u.0 * 3.0, m.1 + u.1 * 3.0);
        let back = (m.0 - u.0 * 3.0, m.1 - u.1 * 3.0);
        d.polyline(&[(back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), tip, (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, 1.6);
    }
}

pub(crate) fn parallel_lines_render(p: &Params) -> String {
    let (w, h, ys) = pl_layout(p);
    let nl = ys.len();
    let turn = p.f("turn");
    let fs = fscale(p);
    let cc = (w / 2.0, h / 2.0);
    let t = |q: Pt| turn_scr(q, cc, turn);
    let mut d = new_doc(w, h);
    let unk = nums(p.s("unknown")).iter().map(|v| *v as usize).collect::<Vec<_>>();
    let marked = nums(p.s("marked")).iter().map(|v| *v as usize).collect::<Vec<_>>();
    let texts = num_text_map(p.s("angle_text"));
    let nm = |k: &str, dflt: &str| { let v = p.s(k).trim(); if v.is_empty() { dflt.to_string() } else { v.to_string() } };
    let names = [nm("name_m", "m"), nm("name_n", "n"), nm("name_p", "p")];
    let ends = p.s("line_ends") != "Plain";
    let (x0, x1) = (30.0, w - 30.0);
    for (i, y) in ys.iter().enumerate() {
        let (a, b) = (t((x0, *y)), t((x1, *y)));
        dline(&mut d, a, b, INK, 2.0, style_dash(p.s(PL_STYLE[i])));
        if ends { line_heads(&mut d, a, b); }
        let k = tick_n(p, PL_ARROWS[i]);
        if k > 0 { chevrons(&mut d, t((w - 80.0, *y)), unit_v(a, b), k); }
        if p.b("names") { let q = t((50.0, *y - 15.0)); mlabel(&mut d, q.0, q.1, 18.0 * fs, &names[i], ""); }
    }
    let nt = p.i("transversals").clamp(1, 2) as usize;
    let rad = 26.0;
    let lsz = 15.0 * fs;
    for ti in 0..nt {
        let th = if ti == 0 { p.f("angle").clamp(10.0, 170.0) } else { p.f("angle_b").clamp(10.0, 170.0) };
        let cx0 = if nt == 1 { w / 2.0 } else if ti == 0 { w / 2.0 - 70.0 } else { w / 2.0 + 70.0 };
        let tr = th.to_radians();
        let slope = (tr.cos(), -tr.sin());
        let cross: Vec<Pt> = ys.iter().map(|y| { let s = (*y - ys[0]) / slope.1; (cx0 + slope.0 * s, *y) }).collect();
        let tl = 70.0 / slope.1.abs().max(0.2);
        let a = (cross[0].0 + slope.0 * tl, cross[0].1 + slope.1 * tl);
        let last = cross[nl - 1];
        let b = (last.0 - slope.0 * tl, last.1 - slope.1 * tl);
        let (ta, tb) = (t(a), t(b));
        dline(&mut d, ta, tb, INK, 2.0, style_dash(p.s(PL_STYLE[3 + ti])));
        if ends { line_heads(&mut d, ta, tb); }
        let k = tick_n(p, PL_ARROWS[3 + ti]);
        if k > 0 {
            let m = t((a.0 * 0.45 + cross[0].0 * 0.55, a.1 * 0.45 + cross[0].1 * 0.55));
            chevrons(&mut d, m, unit_v(tb, ta), k);
        }
        if p.b("names") {
            let tn = if ti == 0 { nm("name_t", "t") } else { nm("name_u", "u") };
            let q = t((a.0 + slope.0 * 18.0, a.1 + slope.1 * 18.0));
            mlabel(&mut d, q.0, q.1, 18.0 * fs, &tn, "");
        }
        for (i, c) in cross.iter().enumerate() {
            let secs = [(th, 180.0, 1usize), (0.0, th, 2), (180.0 + th, 360.0, 3), (180.0, 180.0 + th, 4)];
            let c = t(*c);
            for (a0, a1, k) in secs {
                let num = ti * 4 * nl + i * 4 + k;
                let (a0t, a1t) = (a0 + turn, a1 + turn);
                if !marked.contains(&num) {
                    if p.s("labels") == "Numbers" { sector(&mut d, c, a0t, a1t, rad, &num.to_string(), false, lsz); }
                    continue;
                }
                let span = a1 - a0;
                let txt = if p.s("labels") == "Numbers" { num.to_string() }
                    else if let Some((_, s)) = texts.iter().find(|(k, _)| *k == num) { s.clone() }
                    else if unk.contains(&num) { "x".to_string() } else { format!("{}\u{b0}", fmt(span, p.i("dec"))) };
                let r = if span < 50.0 { rad + 8.0 } else { rad };
                sector(&mut d, c, a0t, a1t, r, &txt, true, lsz);
            }
        }
    }
    d.finish()
}

pub(crate) fn pl_computed(p: &Params) -> Vec<(&'static str, String)> {
    let keys = ["m_line", "n_line", "p_line"];
    (0..3).map(|i| {
        let k = tick_n(p, PL_ARROWS[i]);
        let arrows = "\u{203a}".repeat(k as usize);
        (keys[i], format!("{} {arrows}", p.s(PL_STYLE[i])).trim().to_string())
    }).collect()
}

// ----- Measures cards -----

pub(crate) fn card(id: &str) -> Option<MeasureCard> {
    let row = |name: &'static str, value_key: &'static str, unit: &'static str, label_key: &'static str, text_key: &'static str, tick_key: Option<&'static str>| MeasureRow { name, name_key: None, value_key, unit, label_key, text_key, tick_key, equals: None };
    let side_names = ["AB", "BC", "CD", "DA"];
    let quad_rows = |id: &str| -> Vec<MeasureGroup> {
        let inp = q_inputs(id);
        let links: [Option<&'static str>; 8] = match id {
            "rectangle" => [None, None, Some("AB"), Some("BC"), Some("90\u{b0}"), Some("90\u{b0}"), Some("90\u{b0}"), Some("90\u{b0}")],
            "parallelogram" => [None, Some("DA"), Some("AB"), None, None, None, Some("\u{2220}A"), Some("\u{2220}B")],
            "kite" => [None, Some("AB"), Some("DA"), None, None, None, Some("\u{2220}A"), None],
            _ => [None; 8],
        };
        let mut sides = Vec::new();
        let mut angs = Vec::new();
        for i in 0..8 {
            let mut r = row(if i < 4 { side_names[i] } else { "\u{2220}" }, Q_V[i], if i < 4 { "$unit" } else { "\u{b0}" }, Q_LAB[i], Q_TXT[i], Some(Q_MARK[i]));
            if !inp[i] { r.equals = links[i]; }
            if i >= 4 { r.name_key = Some(NAME_K[i - 4]); }
            if i < 4 { sides.push(r); } else { angs.push(r); }
        }
        if id == "trapezoid" { sides.push(row("h", "m_h", "$unit", "height_label", "height_text", None)); }
        // the side popover's second mark (parallel arrows) as its own rows: the card's rows hold
        // one mark each
        let arrows = (0..4).map(|i| MeasureRow { equals: Some("\u{203a}"), ..row(side_names[i], Q_PAR[i], "", Q_ARROW[i], "", None) }).collect();
        vec![MeasureGroup { title: "Sides", rows: sides }, MeasureGroup { title: "Angles", rows: angs }, MeasureGroup { title: "Parallel arrows", rows: arrows }]
    };
    let help_quad = "Fill in the kind's measures; the rest are worked out and shown faintly. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}. Rename a corner in the box before its angle. The button after each measure sets its label and congruence marks.";
    Some(match id {
        "triangle" => {
            let mut angs = Vec::new();
            for i in 0..3 {
                let mut r = row("\u{2220}", TRI_V[i], "\u{b0}", TRI_LAB[i], TRI_TXT[i], Some(TRI_MARK[i]));
                r.name_key = Some(NAME_K[i]);
                angs.push(r);
            }
            let sides = (3..6).map(|i| row(["AB", "BC", "CA"][i - 3], TRI_V[i], "$unit", TRI_LAB[i], TRI_TXT[i], Some(TRI_MARK[i]))).collect();
            MeasureCard {
                title: "Measures",
                help: "Fill in any three: two angles and a side, two sides and an angle, or three sides. The rest are worked out and shown faintly. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}. Rename a corner in the box before its angle.",
                groups: vec![MeasureGroup { title: "Angles", rows: angs }, MeasureGroup { title: "Sides", rows: sides }],
            }
        }
        "rectangle" | "parallelogram" | "trapezoid" | "kite" => MeasureCard { title: "Measures", help: help_quad, groups: quad_rows(id) },
        "regular-polygon" => {
            let rows = vec![
                row("Sides", "n", "", "", "", None),
                row("Size", "value", "$unit", "", "", None),
            ];
            let mut side = row("Side", "m_side", "$unit", "side_lab", "side_label", Some("side_ticks"));
            side.equals = Some("worked out");
            let mut ang = row("Angle", "m_angle", "", "ang_lab", "ang_label", Some("ang_arcs"));
            ang.equals = Some("(n \u{2212} 2)\u{b7}180\u{b0}/n");
            let mut apo = row("Apothem", "m_apothem", "$unit", "apothem_label", "apothem_text", None);
            apo.equals = Some("worked out");
            let mut rad = row("Radius", "m_radius", "$unit", "radius_label", "radius_text", None);
            rad.equals = Some("worked out");
            MeasureCard {
                title: "Polygon",
                help: "Pick the number of sides, then give one measure: the side, the radius (center to a corner) or the apothem (center to the middle of a side) under Size from. The rest are worked out. The side's label goes on the bottom side and the angle's in the bottom left corner; their marks go on every side or angle.",
                groups: vec![MeasureGroup { title: "", rows }, MeasureGroup { title: "Measures", rows: vec![side, ang, apo, rad] }],
            }
        }
        "angles" => {
            let mut base = row("Baseline", "ray1", "\u{b0}", RAY_STYLE[0], "", Some(RAY_LINE[0]));
            base.equals = Some("0\u{b0}");
            let mut rows = vec![base];
            for i in 1..6 { rows.push(row(["", "Ray 2", "Ray 3", "Ray 4", "Ray 5", "Ray 6"][i], RAY_V[i], "\u{b0}", RAY_STYLE[i], "", Some(RAY_LINE[i]))); }
            MeasureCard {
                title: "Rays",
                help: "Every ray starts at the vertex. The baseline points right, and each other ray's direction is its angle from the baseline, counterclockwise, as a protractor reads it: 90\u{b0} points straight up. Leave a direction blank for no ray. The button after a ray sets its line style and whether it runs through the vertex as a line. Click an angle on the figure to label it.",
                groups: vec![MeasureGroup { title: "", rows }],
            }
        }
        "parallel-lines" => {
            let line_keys = ["m_line", "n_line", "p_line"];
            let lines = (0..3).map(|i| {
                let mut r = row(["Line 1", "Line 2", "Line 3"][i], line_keys[i], "", PL_STYLE[i], "", Some(PL_ARROWS[i]));
                r.name_key = Some(PL_NAMES[i]);
                r.equals = Some("\u{2225}");
                r
            }).collect();
            let trans = (0..2).map(|i| {
                let mut r = row(["Transversal 1", "Transversal 2"][i], ["angle", "angle_b"][i], "\u{b0}", PL_STYLE[3 + i], "", Some(PL_ARROWS[3 + i]));
                r.name_key = Some(PL_NAMES[3 + i]);
                r
            }).collect();
            MeasureCard {
                title: "Parallel lines",
                help: "The parallel lines run across the page, evenly spaced. Each transversal crosses them at the angle you give: the angle above the top line, to the right of the transversal. The button after a line sets its style and parallel arrows. Click an angle on the figure to label it.",
                groups: vec![MeasureGroup { title: "Parallel lines", rows: lines }, MeasureGroup { title: "Transversals", rows: trans }],
            }
        }
        _ => return None,
    })
}

pub(crate) fn computed(id: &str, p: &Params) -> Option<Vec<(&'static str, String)>> {
    Some(match id {
        "triangle" => {
            let s = match tri_solve(p) {
                Ok(s) => s,
                // The card shows this line under the rows ("__msg" is not a row).
                Err(e) => return Some(vec![("__msg", e.to_string())]),
            };
            let vals = tri_values(&s);
            let dec = p.i("dec");
            (0..6).filter(|i| s.typed[*i].is_none()).filter_map(|i| vals[i].map(|v| (TRI_V[i], fmt(v, dec)))).collect()
        }
        "rectangle" | "parallelogram" | "trapezoid" | "kite" => quad_computed(id, p),
        "regular-polygon" => rp_computed(p),
        "angles" => angles_computed(p),
        "parallel-lines" => pl_computed(p),
        _ => return None,
    })
}

// ----- legacy specs -----

type Spec = crate::model::FigureSpec;
fn js(spec: &Spec, k: &str) -> Option<String> { spec.params.get(k).and_then(|v| v.as_str()).map(|s| s.to_string()) }
fn jf(spec: &Spec, k: &str) -> Option<f64> { spec.params.get(k).and_then(|v| v.as_f64()) }
fn jb(spec: &Spec, k: &str) -> Option<bool> { spec.params.get(k).and_then(|v| v.as_bool()) }
fn has(spec: &Spec, k: &str) -> bool { spec.params.contains_key(k) }

/// Old label lists (`sides`, `angles`, `unknown`, `side_text`) -> one row's mode and text.
fn old_label(name: &str, angle: bool, show: &[String], unk: &[String], tmap: &[(String, String)]) -> (&'static str, String) {
    if let Some(t) = text_for(tmap, name) { return ("Text", t); }
    let up = name.to_uppercase();
    let hit = |l: &[String]| if angle { l.contains(&up) } else { listed(l, &up) };
    if hit(unk) { ("Text", "x".into()) } else if hit(show) { ("Measure", String::new()) } else { ("None", String::new()) }
}

fn set_label(p: &mut Params, lab: &str, txt: &str, v: (&'static str, String)) {
    p.set_s(lab, v.0);
    if v.0 == "Text" { p.set_s(txt, &v.1); }
}

/// Shared part of the polygon migrations: corner names, label modes from the old lists, ticks
/// (explicit `marks`, or the old automatic equal-side ticks) and arcs.
fn migrate_labels(spec: &Spec, p: &mut Params, n: usize, rows: &[(usize, bool, &str, &str, &str)], d_sides: &str, d_angles: &str, d_unknown: &str, lens: &[f64]) {
    let custom: Vec<String> = js(spec, "vnames").unwrap_or_default().split(|c: char| c == ',' || c.is_whitespace()).map(|t| t.trim().to_uppercase()).filter(|t| !t.is_empty()).collect();
    let names: Vec<String> = (0..n).map(|i| custom.get(i).cloned().unwrap_or_else(|| ((b'A' + i as u8) as char).to_string())).collect();
    for i in 0..n { p.set_s(NAME_K[i], &names[i]); }
    let show = tokens(&js(spec, "sides").unwrap_or_else(|| d_sides.into()));
    let angs = tokens(&js(spec, "angles").unwrap_or_else(|| d_angles.into()));
    let unk = tokens(&js(spec, "unknown").unwrap_or_else(|| d_unknown.into()));
    let tmap = text_map(&js(spec, "side_text").unwrap_or_default());
    let ticks = parse_counts(&js(spec, "marks").unwrap_or_default(), &names, true)
        .unwrap_or_else(|| if jb(spec, "ticks").unwrap_or(true) { auto_ticks(lens) } else { vec![0; n] });
    let arcs = parse_counts(&js(spec, "arcs").unwrap_or_default(), &names, false).unwrap_or_else(|| vec![0; n]);
    for &(i, angle, lab, txt, mark) in rows {
        let name = if angle { names[i].clone() } else { format!("{}{}", names[i], names[(i + 1) % n]) };
        set_label(p, lab, txt, old_label(&name, angle, if angle { &angs } else { &show }, &unk, &tmap));
        let k = if angle { arcs[i] } else { ticks[i] };
        p.set_s(mark, if k == 0 { "None" } else { TICKS[k as usize] });
    }
    // diagonals: the old lists also labelled AC / BD
    for (dk, lab, txt) in [("AC", "ac_lab", "ac_label"), ("BD", "bd_lab", "bd_label")] {
        if n == 4 && p.0.contains_key(lab) {
            let dn = if dk == "AC" { format!("{}{}", names[0], names[2]) } else { format!("{}{}", names[1], names[3]) };
            set_label(p, lab, txt, old_label(&dn, false, &show, &unk, &tmap));
        }
    }
}

fn ns(v: f64) -> String { num_str(v, 4) }

pub(crate) fn migrate(id: &str, spec: &Spec, p: &mut Params) {
    match id {
        "triangle" => {
            if has(spec, "m_ab") || !(has(spec, "mode") || has(spec, "ab")) { return; }
            let mode = js(spec, "mode").unwrap_or_else(|| "Right triangle (legs AB, BC)".into());
            let (ab, bc, ca) = (jf(spec, "ab").unwrap_or(12.0), jf(spec, "bc").unwrap_or(5.0), jf(spec, "ca").unwrap_or(13.0));
            let (aa, bb) = (jf(spec, "ang_a").unwrap_or(60.0), jf(spec, "ang_b").unwrap_or(60.0));
            for k in TRI_V { p.set_s(k, ""); }
            let set: Vec<(&str, f64)> = if mode.starts_with("Three") { vec![("m_ab", ab), ("m_bc", bc), ("m_ca", ca)] }
                else if mode.starts_with("Two sides and an angle not") { vec![("m_ab", ab), ("m_bc", bc), ("m_a", aa)] }
                else if mode.starts_with("Two sides") { vec![("m_ab", ab), ("m_ca", ca), ("m_a", aa)] }
                else if mode.starts_with("Two angles and a side not") { vec![("m_a", aa), ("m_b", bb), ("m_bc", bc)] }
                else if mode.starts_with("Two angles") { vec![("m_a", aa), ("m_b", bb), ("m_ab", ab)] }
                else if mode.starts_with("Isosceles") { vec![("m_ab", ab), ("m_bc", bc), ("m_ca", bc)] }
                else if mode.starts_with("Equilateral") { vec![("m_ab", ab), ("m_bc", ab), ("m_ca", ab)] }
                else { vec![("m_ab", ab), ("m_bc", bc), ("m_b", 90.0)] };
            for (k, v) in set { p.set_s(k, &ns(v)); }
            let lens: Vec<f64> = tri_solve(p).ok().and_then(|s| s.out.side.map(|x| x.to_vec())).unwrap_or_else(|| vec![1.0, 2.0, 3.0]);
            let rows: Vec<(usize, bool, &str, &str, &str)> = (0..3).map(|i| (i, true, TRI_LAB[i], TRI_TXT[i], TRI_MARK[i])).chain((0..3).map(|i| (i, false, TRI_LAB[i + 3], TRI_TXT[i + 3], TRI_MARK[i + 3]))).collect();
            migrate_labels(spec, p, 3, &rows, "AB, BC, CA", "A", "BC", &lens);
        }
        "rectangle" | "parallelogram" | "trapezoid" | "kite" => {
            if has(spec, "m_ab") || !has(spec, "ab") { return; }
            for k in Q_V { if p.0.contains_key(k) { p.set_s(k, ""); } }
            let f = |k: &str, d: f64| jf(spec, k).unwrap_or(d);
            let kind = js(spec, "kind").unwrap_or_default();
            let (d_sides, d_angles, d_unknown) = match id {
                "rectangle" => {
                    p.set_s("m_ab", &ns(f("ab", 10.0)));
                    if kind != "Square" { p.set_s("m_bc", &ns(f("bc", 6.0))); }
                    ("AB, BC", "", "")
                }
                "parallelogram" => {
                    p.set_s("m_ab", &ns(f("ab", 10.0)));
                    if kind != "Rhombus" { p.set_s("m_da", &ns(f("da", 6.0))); }
                    p.set_s("m_a", &ns(f("ang_a", 60.0)));
                    ("AB, DA", "A", "")
                }
                "kite" => {
                    let (a_len, d_len, b) = (f("ab", 5.0), f("da", 9.0), f("ang_b", 100.0));
                    // the old kite stretched DA to at least just past the half-width
                    let w = a_len * (b.to_radians() / 2.0).sin();
                    p.set_s("m_ab", &ns(a_len));
                    p.set_s("m_da", &ns(d_len.max(w * 1.02)));
                    p.set_s("m_b", &ns(b));
                    ("AB, DA", "B", "")
                }
                _ => {
                    let (ab, cd, h) = (f("ab", 12.0), f("cd", 7.0), f("height", 5.0));
                    p.set_s("m_ab", &ns(ab));
                    p.set_s("m_cd", &ns(cd));
                    match kind.as_str() {
                        "Isosceles trapezoid" => p.set_s("m_h", &ns(h)),
                        "Trapezoid" => {
                            let off = f("offset", 2.0);
                            p.set_s("m_h", &ns(h));
                            p.set_s("m_a", &ns(h.atan2(off).to_degrees()));
                        }
                        _ => p.set_s("m_da", &ns(h)),
                    }
                    // the old height choice was a number; the drawn-height label stays
                    ("AB, CD, DA", "", "BC")
                }
            };
            let lens: Vec<f64> = quad_solve(id, p).ok().map(|s| (0..4).map(|i| s.vals[i].unwrap_or(0.0)).collect()).unwrap_or_else(|| vec![1.0, 2.0, 3.0, 4.0]);
            let rows: Vec<(usize, bool, &str, &str, &str)> = (0..4).map(|i| (i, false, Q_LAB[i], Q_TXT[i], Q_MARK[i])).chain((0..4).map(|i| (i, true, Q_LAB[i + 4], Q_TXT[i + 4], Q_MARK[i + 4]))).collect();
            migrate_labels(spec, p, 4, &rows, d_sides, d_angles, d_unknown, &lens);
        }
        "regular-polygon" => {
            if has(spec, "side_lab") || !has(spec, "n") { return; }
            let n = jf(spec, "n").unwrap_or(6.0).clamp(3.0, 20.0) as usize;
            let custom: Vec<String> = js(spec, "vnames").unwrap_or_default().split(|c: char| c == ',' || c.is_whitespace()).map(|t| t.trim().to_uppercase()).filter(|t| !t.is_empty()).collect();
            let names: Vec<String> = (0..n).map(|i| custom.get(i).cloned().unwrap_or_else(|| ((b'A' + i as u8) as char).to_string())).collect();
            let show = tokens(&js(spec, "sides").unwrap_or_else(|| "AB".into()));
            let angs = tokens(&js(spec, "angles").unwrap_or_default());
            let unk = tokens(&js(spec, "unknown").unwrap_or_default());
            let tmap = text_map(&js(spec, "side_text").unwrap_or_default());
            let s0 = format!("{}{}", names[0], names[1 % n]);
            set_label(p, "side_lab", "side_label", old_label(&s0, false, &show, &unk, &tmap));
            set_label(p, "ang_lab", "ang_label", old_label(&names[0], true, &angs, &unk, &tmap));
            let ticks = parse_counts(&js(spec, "marks").unwrap_or_default(), &names, true)
                .unwrap_or_else(|| if jb(spec, "ticks").unwrap_or(true) { vec![1; n] } else { vec![0; n] });
            let arcs = parse_counts(&js(spec, "arcs").unwrap_or_default(), &names, false).unwrap_or_else(|| vec![0; n]);
            let t0 = ticks.iter().copied().max().unwrap_or(0);
            let a0 = arcs.iter().copied().max().unwrap_or(0);
            p.set_s("side_ticks", TICKS[t0.min(3) as usize]);
            p.set_s("ang_arcs", TICKS[a0.min(3) as usize]);
            // the old figure drew the radius to A (bottom left) by default and lettered the corners
            p.set_b("radius", jb(spec, "radius").unwrap_or(true));
            p.set_b("apothem", jb(spec, "apothem").unwrap_or(false));
            p.set_b("vertices", jb(spec, "vertices").unwrap_or(true));
            if !has(spec, "apothem_label") { p.set_s("apothem_label", "a = measure"); }
            if !has(spec, "radius_label") { p.set_s("radius_label", "r = measure"); }
        }
        "angles" => {
            if has(spec, "ray2") || !(has(spec, "mode") || has(spec, "angle")) { return; }
            let mode = js(spec, "mode").unwrap_or_else(|| "Angles on a straight line".into());
            let (a, a2) = (jf(spec, "angle").unwrap_or(60.0), jf(spec, "angle2").unwrap_or(110.0));
            for k in &RAY_V[1..] { p.set_s(k, ""); }
            let mut turn = jf(spec, "turn").unwrap_or(0.0);
            match mode.as_str() {
                "Single angle" => p.set_s("ray2", &ns(a)),
                "Angles around a point" => { p.set_s("ray2", &ns(a)); p.set_s("ray3", &ns((a + a2).min(358.0))); p.set_b("around", true); }
                "Vertical angles" => { p.set_s("ray2", &ns(a)); p.set_s("ray1_line", "Line"); p.set_s("ray2_line", "Line"); }
                "Custom rays" => {
                    let mut r: Vec<f64> = nums(&js(spec, "rays").unwrap_or_else(|| "0, 50, 130, 250".into())).iter().map(|x| x.rem_euclid(360.0)).collect();
                    r.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                    r.dedup();
                    if r.len() >= 2 {
                        let wrap = r.len() > 2 && r[r.len() - 1] - r[0] < 340.0;
                        let base = r[0];
                        turn = (turn + base + 180.0).rem_euclid(360.0) - 180.0;
                        for (i, x) in r.iter().skip(1).take(5).enumerate() { p.set_s(RAY_V[i + 1], &ns(x - base)); }
                        p.set_b("around", wrap);
                    }
                }
                _ => { p.set_s("ray2", &ns(a)); p.set_s("ray3", "180"); }
            }
            p.set_f("turn", turn);
        }
        "parallel-lines" => {
            if has(spec, "m_arrows") || !has(spec, "angle") { return; }
            // the old figure drew double chevrons on every parallel line when `arrows` was on
            let k = if jb(spec, "arrows").unwrap_or(true) { "2" } else { "None" };
            for key in &PL_ARROWS[..3] { p.set_s(key, k); }
        }
        _ => {}
    }
}

// ----- examples (mirror the reference's "Start from an example") -----

pub(crate) fn examples(id: &str) -> Option<Vec<super::super::Example>> {
    use Value::{B, F, I, S};
    let s = |v: &str| S(v.to_string());
    let ex = |name: &'static str, params: Vec<(&'static str, Value)>| super::super::Example { name, params };
    let tri = |m: [&str; 6]| -> Vec<(&'static str, Value)> { (0..6).map(|i| (TRI_V[i], s(m[i]))).collect() };
    let with = |mut a: Vec<(&'static str, Value)>, b: Vec<(&'static str, Value)>| { a.extend(b); a };
    let v = match id {
        "triangle" => vec![
            ex("Right triangle with legs 3 and 4: find the hypotenuse", with(tri(["", "90", "", "4", "3", ""]), vec![("a_lab", s("None")), ("b_lab", s("None")), ("bc_lab", s("Measure")), ("ca_lab", s("Text")), ("ca_label", s("x"))])),
            ex("30-60-90 triangle with hypotenuse 10", with(tri(["30", "90", "", "", "5", ""]), vec![("b_lab", s("None")), ("c_lab", s("Measure")), ("ab_lab", s("Text")), ("ab_label", s("y")), ("bc_lab", s("Text")), ("bc_label", s("x")), ("ca_lab", s("Measure"))])),
            ex("SOHCAHTOA: find the side opposite a 35\u{b0} angle", with(tri(["35", "90", "", "10", "", ""]), vec![("b_lab", s("None")), ("bc_lab", s("Text")), ("bc_label", s("x"))])),
            ex("Law of cosines: two sides and the angle between them", with(tri(["50", "", "", "9", "", "7"]), vec![("b_lab", s("None")), ("ca_lab", s("Measure")), ("bc_lab", s("Text")), ("bc_label", s("x"))])),
            ex("Isosceles triangle with its height drawn, for area", with(tri(["", "", "", "10", "13", "13"]), vec![("height", s("From C")), ("a_lab", s("None")), ("b_lab", s("None")), ("bc_lab", s("None")), ("ca_lab", s("None")), ("bc_ticks", s("1")), ("ca_ticks", s("1"))])),
            ex("Ambiguous case: two sides and an angle not between them", with(tri(["40", "", "", "10", "7", ""]), vec![("b_lab", s("None")), ("bc_lab", s("Measure")), ("other", B(true))])),
        ],
        "rectangle" => vec![
            ex("Rectangle 12 cm by 5 cm for area and perimeter", vec![("m_ab", s("12")), ("m_bc", s("5")), ("unit", s("cm"))]),
            ex("Rectangle with a diagonal labeled x", vec![("m_ab", s("8")), ("m_bc", s("6")), ("diagonals", s("AC only")), ("ac_lab", s("Text")), ("ac_label", s("x"))]),
            ex("Rectangle with sides labeled 2x + 3 and x", vec![("m_ab", s("10")), ("m_bc", s("4")), ("ab_lab", s("Text")), ("ab_label", s("2x + 3")), ("bc_lab", s("Text")), ("bc_label", s("x"))]),
            ex("Square with its diagonals drawn", vec![("kind", s("Square")), ("m_ab", s("6")), ("m_bc", s("")), ("bc_lab", s("None")), ("diagonals", s("Drawn")), ("ab_ticks", s("1")), ("bc_ticks", s("1")), ("cd_ticks", s("1")), ("da_ticks", s("1"))]),
        ],
        "parallelogram" => vec![
            ex("Parallelogram with base and height for area", vec![("m_ab", s("10")), ("m_da", s("6")), ("m_a", s("60")), ("height", s("Onto AB (from D)")), ("da_lab", s("None")), ("a_lab", s("None"))]),
            ex("Parallelogram: find the missing angles", vec![("m_a", s("65")), ("ab_lab", s("None")), ("da_lab", s("None")), ("b_lab", s("Text")), ("b_label", s("x")), ("c_lab", s("Text")), ("c_label", s("y")), ("d_lab", s("Text")), ("d_label", s("z"))]),
            ex("Rhombus with its diagonals drawn", vec![("kind", s("Rhombus")), ("m_ab", s("5")), ("m_da", s("")), ("m_a", s("70")), ("diagonals", s("Drawn")), ("a_lab", s("None")), ("da_lab", s("None")), ("ab_ticks", s("1")), ("bc_ticks", s("1")), ("cd_ticks", s("1")), ("da_ticks", s("1")), ("arrow_marks", s("none"))]),
        ],
        "trapezoid" => vec![
            ex("Isosceles trapezoid with bases and height for area", vec![("kind", s("Isosceles trapezoid")), ("m_ab", s("12")), ("m_cd", s("6")), ("m_da", s("")), ("m_h", s("4")), ("show_height", B(true)), ("height_label", s("Measure")), ("da_lab", s("None")), ("bc_lab", s("None"))]),
            ex("Right trapezoid: find the slanted side", vec![("kind", s("Right trapezoid")), ("m_ab", s("12")), ("m_cd", s("7")), ("m_da", s("5")), ("bc_lab", s("Text")), ("bc_label", s("x"))]),
            ex("Trapezoid with an angle and its height", vec![("kind", s("Trapezoid")), ("m_ab", s("11")), ("m_cd", s("6")), ("m_da", s("")), ("m_a", s("53")), ("m_h", s("4")), ("show_height", B(true)), ("da_lab", s("None")), ("bc_lab", s("None"))]),
            ex("Isosceles trapezoid with its diagonals drawn", vec![("kind", s("Isosceles trapezoid")), ("m_ab", s("12")), ("m_cd", s("6")), ("m_da", s("")), ("m_h", s("5")), ("diagonals", s("Drawn")), ("da_lab", s("None")), ("bc_lab", s("None")), ("bc_ticks", s("1")), ("da_ticks", s("1"))]),
        ],
        "kite" => vec![
            ex("Kite with diagonals 6 and 12 for area", vec![("m_ab", s("3sqrt2")), ("m_da", s("3sqrt10")), ("m_b", s("90")), ("diagonals", s("Drawn and labelled")), ("ab_lab", s("None")), ("da_lab", s("None")), ("b_lab", s("None")), ("dec", I(0))]),
            ex("Kite: find the missing angle", vec![("m_b", s("110")), ("d_lab", s("Measure")), ("a_lab", s("Text")), ("a_label", s("x")), ("ab_lab", s("None")), ("da_lab", s("None"))]),
            ex("Kite with its line of symmetry", vec![("diagonals", s("BD only")), ("b_lab", s("None"))]),
        ],
        "regular-polygon" => vec![
            ex("Regular hexagon with its apothem, for area", vec![("n", I(6)), ("value", F(8.0)), ("apothem", B(true)), ("apothem_label", s("Measure")), ("radius", B(false))]),
            ex("Regular pentagon: find the interior angle", vec![("n", I(5)), ("apothem", B(false)), ("center_dot", B(false)), ("side_lab", s("None")), ("ang_lab", s("Text")), ("ang_label", s("x")), ("ang_arcs", s("1")), ("side_ticks", s("1"))]),
            ex("Regular octagon split into triangles by its radii", vec![("n", I(8)), ("all_radii", B(true)), ("apothem", B(false)), ("side_lab", s("None"))]),
            ex("Regular decagon with congruence marks", vec![("n", I(10)), ("apothem", B(false)), ("center_dot", B(false)), ("side_ticks", s("1")), ("ang_arcs", s("1")), ("side_lab", s("None"))]),
        ],
        "angles" => vec![
            ex("Single angle of 60\u{b0}", vec![("ray2", s("60"))]),
            ex("Angles on a straight line: find x", vec![("ray2", s("125")), ("ray3", s("180")), ("unknown", s("2"))]),
            ex("Vertical angles", vec![("ray2", s("50")), ("ray1_line", s("Line")), ("ray2_line", s("Line")), ("unknown", s("3")), ("hide", s("2, 4"))]),
            ex("Angles around a point with expressions", vec![("ray2", s("100")), ("ray3", s("220")), ("around", B(true)), ("sector_text", s("3 = 2x + 20"))]),
            ex("Complementary angles", vec![("ray2", s("35")), ("ray3", s("90")), ("unknown", s("2"))]),
        ],
        _ => return None,
    };
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(v: &[(usize, f64)]) -> [Option<(f64, f64)>; 6] {
        let mut a = [None; 6];
        for (i, x) in v { a[*i] = Some((*x, 0.005)); }
        a
    }
    fn near(a: f64, b: f64) -> bool { (a - b).abs() < 0.05 }

    #[test]
    fn triangle_solver_matches_reference() {
        // reference default: A 24, B 90, AB 12 -> BC 5.3, CA 13.1, C 66
        let o = tri_core(&g(&[(0, 24.0), (1, 90.0), (3, 12.0)]), false).unwrap();
        let s = o.side.unwrap();
        assert!(near(s[1], 5.34) && near(s[2], 13.14) && near(o.ang[2], 66.0));
        // SSS 3-4-5
        let o = tri_core(&g(&[(3, 3.0), (4, 4.0), (5, 5.0)]), false).unwrap();
        assert!(near(o.ang[0], 53.13) && near(o.ang[1], 90.0));
        // SSA two triangles: A 40, AB 10, BC 7 -> B 73.3 C 66.7 CA 10.4; other: C 113.3
        let o = tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 7.0)]), false).unwrap();
        assert!(o.two && near(o.ang[1], 73.3) && near(o.ang[2], 66.7) && near(o.side.unwrap()[2], 10.43));
        let o2 = tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 7.0)]), true).unwrap();
        assert!(near(o2.ang[2], 113.3));
        // SSA one triangle, and too short
        assert!(!tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 12.0)]), false).unwrap().two);
        assert!(matches!(tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 5.0)]), false), Err(TriErr::Short(0, m)) if near(m, 6.43)));
        assert!(matches!(tri_core(&g(&[(0, 100.0), (3, 10.0), (4, 8.0)]), false), Err(TriErr::Obtuse(0, false))));
        // AAS: A 40, B 60, BC 10 -> AB 15.3, CA 13.5
        let s = tri_core(&g(&[(0, 40.0), (1, 60.0), (4, 10.0)]), false).unwrap().side.unwrap();
        assert!(near(s[0], 15.32) && near(s[2], 13.47));
        // too few, sums, inequality
        assert_eq!(tri_core(&g(&[(3, 10.0)]), false).unwrap_err(), TriErr::Few(2));
        assert!(matches!(tri_core(&g(&[(0, 40.0), (1, 150.0), (4, 10.0)]), false), Err(TriErr::Sum2(0, 1, _))));
        assert!(matches!(tri_core(&g(&[(0, 30.0), (1, 60.0), (2, 91.0)]), false), Err(TriErr::Sum3(_))));
        assert_eq!(tri_core(&g(&[(3, 3.0), (4, 4.0), (5, 8.0)]), false).unwrap_err(), TriErr::Ineq(5));
        // three angles: shape only
        assert!(tri_core(&g(&[(0, 40.0), (1, 60.0), (2, 80.0)]), false).unwrap().side.is_none());
        // over-determined: angles + AB win, BC is checked (reference: "BC would be 6.53, not 7")
        assert!(matches!(tri_core(&g(&[(0, 40.0), (1, 60.0), (3, 10.0), (4, 7.0)]), false), Err(TriErr::Mismatch(4, c)) if near(c, 6.53)));
        // SSS + one angle: the sides win, the angle is checked
        assert!(matches!(tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 7.0), (5, 12.0)]), false), Err(TriErr::Mismatch(0, c)) if near(c, 35.66)));
    }

    #[test]
    fn typed_values() {
        assert!((eval_typed("5sqrt2").unwrap() - 7.0711).abs() < 1e-3);
        assert!((eval_typed("7/2").unwrap() - 3.5).abs() < 1e-9);
        assert!((eval_typed("\u{221a}3").unwrap() - 1.732).abs() < 1e-3);
        assert!((eval_typed("2pi").unwrap() - 6.2832).abs() < 1e-3);
        assert!(eval_typed("x").is_none() && eval_typed("abc").is_none());
        assert_eq!(pretty("5sqrt2"), "5\u{221a}2");
        assert_eq!(pretty("sqrt(10)"), "\u{221a}10");
        assert_eq!(parse_typed("37").unwrap().unwrap().tol, 0.5 + 1e-9);
        // 37 accepted against 36.87 (reference), 40 refused against 40.54
        assert!((parse_typed("37").unwrap().unwrap().v - 36.87).abs() <= 0.5);
    }

    /// Old spec (keys as saved before the Measures rework) -> params the way figuresui loads it.
    fn load_old(id: &str, old: serde_json::Value) -> Params {
        let d = super::super::defs().into_iter().find(|d| d.id == id).unwrap();
        let spec = crate::model::FigureSpec { id: id.into(), params: old.as_object().unwrap().clone().into_iter().collect() };
        let defs = (d.params)();
        let mut p = Params::defaults(&defs);
        for pd in &defs {
            if let Some(j) = spec.params.get(pd.key) {
                let v = match &pd.default {
                    Value::F(_) => j.as_f64().map(Value::F),
                    Value::I(_) => j.as_i64().map(Value::I),
                    Value::B(_) => j.as_bool().map(Value::B),
                    Value::S(_) => j.as_str().map(|x| Value::S(x.into())),
                };
                if let Some(v) = v { p.0.insert(pd.key.into(), v); }
            }
        }
        super::super::migrate(id, &spec, &mut p);
        p.normalize(&defs, false);
        p
    }

    #[test]
    fn migrate_triangle() {
        let p = load_old("triangle", serde_json::json!({"mode": "Two angles and a side not between (AAS: A, B and BC)", "ab": 12.0, "bc": 5.0, "ca": 13.0, "ang_a": 30.0, "ang_b": 90.0,
            "sides": "RP", "angles": "P, R", "unknown": "QR", "side_text": "PQ = y", "ticks": true, "marks": "", "arcs": "Q 2", "vnames": "P, Q, R", "dec": 1}));
        assert_eq!((p.s("m_a"), p.s("m_b"), p.s("m_bc"), p.s("m_ab"), p.s("m_ca")), ("30", "90", "5", "", ""));
        assert_eq!((p.s("ca_lab"), p.s("bc_lab"), p.s("bc_label"), p.s("ab_lab"), p.s("ab_label")), ("Measure", "Text", "x", "Text", "y"));
        assert_eq!((p.s("a_lab"), p.s("b_lab"), p.s("c_lab"), p.s("b_arcs")), ("Measure", "None", "Measure", "2"));
        assert_eq!((p.s("name_a"), p.s("name_b"), p.s("name_c")), ("P", "Q", "R"));
        let s = tri_solve(&p).unwrap();
        assert!((s.out.side.unwrap()[2] - 10.0).abs() < 1e-6);
        // right triangle mode (the old default) -> AB, BC and B = 90
        let p = load_old("triangle", serde_json::json!({"mode": "Right triangle (legs AB, BC)", "ab": 4.0, "bc": 3.0}));
        assert_eq!((p.s("m_ab"), p.s("m_bc"), p.s("m_b")), ("4", "3", "90"));
        assert!((tri_solve(&p).unwrap().out.side.unwrap()[2] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn migrate_quads() {
        let p = load_old("rectangle", serde_json::json!({"kind": "Rectangle", "ab": 8.0, "bc": 6.0, "sides": "AB, BC", "unknown": "AC", "diagonals": "AC only", "ticks": true}));
        assert_eq!((p.s("m_ab"), p.s("m_bc"), p.s("ab_lab"), p.s("cd_lab"), p.s("ac_lab"), p.s("ac_label")), ("8", "6", "Measure", "None", "Text", "x"));
        assert_eq!((p.s("ab_ticks"), p.s("bc_ticks"), p.s("cd_ticks"), p.s("da_ticks")), ("1", "2", "1", "2"));
        let p = load_old("parallelogram", serde_json::json!({"kind": "Rhombus", "ab": 5.0, "da": 9.0, "ang_a": 70.0, "sides": "AB", "angles": "A"}));
        assert_eq!((p.s("m_ab"), p.s("m_da"), p.s("m_a"), p.s("a_lab")), ("5", "", "70", "Measure"));
        let s = para_solve(&p).unwrap();
        assert!((s.vals[3].unwrap() - 5.0).abs() < 1e-9);
        let p = load_old("trapezoid", serde_json::json!({"kind": "Trapezoid", "ab": 11.0, "cd": 6.0, "height": 4.0, "offset": 3.0, "sides": "AB, CD", "unknown": ""}));
        let s = trap_solve(&p).unwrap();
        assert!((s.pts[3].0 - 3.0).abs() < 1e-3 && (s.pts[3].1 - 4.0).abs() < 1e-3 && (s.pts[2].0 - 9.0).abs() < 1e-3);
        let p = load_old("trapezoid", serde_json::json!({"kind": "Isosceles trapezoid", "ab": 12.0, "cd": 6.0, "height": 4.0}));
        let s = trap_solve(&p).unwrap();
        assert!((s.pts[3].0 - 3.0).abs() < 1e-9 && (s.pts[3].1 - 4.0).abs() < 1e-9);
        let p = load_old("trapezoid", serde_json::json!({"kind": "Right trapezoid", "ab": 12.0, "cd": 7.0, "height": 5.0, "unknown": "BC"}));
        let s = trap_solve(&p).unwrap();
        assert!((s.vals[1].unwrap() - 50f64.sqrt()).abs() < 1e-9 && p.s("bc_lab") == "Text");
        let p = load_old("kite", serde_json::json!({"ab": 5.0, "da": 9.0, "ang_b": 100.0, "sides": "AB, DA", "angles": "B"}));
        let s = kite_solve(&p).unwrap();
        assert!((s.vals[4].unwrap() - 104.8).abs() < 0.05 && (s.vals[7].unwrap() - 50.4).abs() < 0.05);
        assert_eq!((p.s("ab_ticks"), p.s("bc_ticks"), p.s("cd_ticks"), p.s("da_ticks")), ("1", "1", "2", "2"));
    }

    #[test]
    fn migrate_poly_angles_lines() {
        let p = load_old("regular-polygon", serde_json::json!({"n": 5, "from": "Side", "value": 8.0, "sides": "", "side_text": "A = x", "arcs": "all 1", "ticks": true, "radius": true}));
        assert_eq!((p.s("side_lab"), p.s("ang_lab"), p.s("ang_label"), p.s("ang_arcs"), p.s("side_ticks")), ("None", "Text", "x", "1", "1"));
        assert!(p.b("radius") && !p.b("apothem") && p.b("vertices"));
        let p = load_old("angles", serde_json::json!({"mode": "Vertical angles", "angle": 50.0, "unknown": "3", "hide": "2, 4"}));
        let (d, wrap) = angle_dirs(&p);
        assert!(wrap && d.iter().map(|x| x.0).collect::<Vec<_>>() == vec![0.0, 50.0, 180.0, 230.0]);
        let p = load_old("angles", serde_json::json!({"mode": "Custom rays", "rays": "30, 100, 200", "turn": 0.0}));
        let (d, wrap) = angle_dirs(&p);
        assert!(wrap && (p.f("turn") - 30.0).abs() < 1e-9 && d.iter().map(|x| x.0).collect::<Vec<_>>() == vec![0.0, 70.0, 170.0]);
        let p = load_old("angles", serde_json::json!({"mode": "Angles on a straight line", "angle": 125.0}));
        assert_eq!((p.s("ray2"), p.s("ray3")), ("125", "180"));
        assert!(!angle_dirs(&p).1);
        let p = load_old("parallel-lines", serde_json::json!({"angle": 65.0, "arrows": false}));
        assert_eq!(p.s("m_arrows"), "None");
        let p = load_old("parallel-lines", serde_json::json!({"angle": 65.0, "arrows": true}));
        assert_eq!(p.s("n_arrows"), "2");
    }

    #[test]
    fn quad_solvers() {
        let mut p = Params::defaults(&trapezoid_params());
        // reference: right trapezoid 12, 7, DA 5 -> BC 7.1, B 45, C 135
        let s = trap_solve(&p).unwrap();
        assert!((s.vals[1].unwrap() - 7.07).abs() < 0.01 && (s.vals[5].unwrap() - 45.0).abs() < 1e-6);
        // plain: AB 12, CD 7, A 65, h 5 -> DA 5.5, BC 5.7, B 61.9 (reference)
        for (k, v) in [("kind", "Trapezoid"), ("m_da", ""), ("m_a", "65"), ("m_h", "5")] { p.set_s(k, v); }
        let s = trap_solve(&p).unwrap();
        assert!((s.vals[3].unwrap() - 5.52).abs() < 0.01 && (s.vals[1].unwrap() - 5.67).abs() < 0.01 && (s.vals[5].unwrap() - 61.9).abs() < 0.05);
        // plain with one measure on each leg, h found numerically
        for (k, v) in [("m_a", "60"), ("m_b", "45"), ("m_h", "")] { p.set_s(k, v); }
        let s = trap_solve(&p).unwrap();
        let h = s.h.unwrap();
        assert!((h / 60f64.to_radians().tan() + h - 5.0).abs() < 1e-6);
        // isosceles: AB 12, CD 6, h 5 -> A 59 (reference)
        for (k, v) in [("kind", "Isosceles trapezoid"), ("m_a", ""), ("m_b", ""), ("m_cd", "6"), ("m_h", "5")] { p.set_s(k, v); }
        let s = trap_solve(&p).unwrap();
        assert!((s.vals[4].unwrap() - 59.04).abs() < 0.01);
        // too few / mismatch
        p.set_s("m_h", "");
        assert!(trap_solve(&p).unwrap_err().contains("one more"));
        for (k, v) in [("m_h", "5"), ("m_bc", "9")] { p.set_s(k, v); }
        assert!(trap_solve(&p).unwrap_err().contains("would be"));
        // kite: reference DA 3 too short, DA 4.5 -> A 71.7, D 116.7
        let mut k = Params::defaults(&kite_params());
        k.set_s("m_da", "3");
        assert!(kite_solve(&k).unwrap_err().contains("3.83"));
        k.set_s("m_da", "4.5");
        let s = kite_solve(&k).unwrap();
        assert!((s.vals[4].unwrap() - 71.7).abs() < 0.05 && (s.vals[7].unwrap() - 116.7).abs() < 0.05);
        // parallelogram from angle B
        let mut q = Params::defaults(&parallelogram_params());
        q.set_s("m_a", "");
        q.set_s("m_b", "120");
        assert!((para_solve(&q).unwrap().vals[4].unwrap() - 60.0).abs() < 1e-9);
        q.set_s("m_a", "70");
        assert!(para_solve(&q).unwrap_err().contains("would be"));
    }
}
}

pub(super) use flat::{angle_dirs, quad_solve, quad_vals_of, regular_polygon_pts, tri_solve, write_givens, RAY_V, TRI_V, Q_V};
use flat::{angles_params, angles_render, kite_params, kite_render, parallel_lines_params, parallel_lines_render, parallelogram_params, parallelogram_render, rectangle_params, rectangle_render, regular_polygon_params, regular_polygon_render, trapezoid_params, trapezoid_render, triangle_params, triangle_render};

// ───────────────────────── coordinate grid ─────────────────────────

/// Expression evaluator: + - * / ^, implicit multiplication, x (and y for implicit linear
/// equations), pi, e, sqrt abs sin cos tan sec csc cot arcsin arccos arctan ln log log_b exp.
/// With `deg`, trig functions read (and inverse trig return) degrees.
struct Ev<'a> { s: &'a [char], i: usize, x: f64, y: Option<f64>, deg: bool }
impl<'a> Ev<'a> {
    fn ws(&mut self) { while self.i < self.s.len() && self.s[self.i] == ' ' { self.i += 1; } }
    fn peek(&mut self) -> Option<char> { self.ws(); self.s.get(self.i).copied() }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some('+') => { self.i += 1; v += self.term()?; }
                Some('-') => { self.i += 1; v -= self.term()?; }
                _ => return Some(v),
            }
        }
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            match self.peek() {
                Some('*') => { self.i += 1; v *= self.unary()?; }
                Some('/') => { self.i += 1; v /= self.unary()?; }
                Some(c) if c.is_ascii_digit() || c == '.' || c == '(' || c.is_alphabetic() => { v *= self.power()?; }
                _ => return Some(v),
            }
        }
    }
    fn unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some('-') => { self.i += 1; Some(-self.unary()?) }
            Some('+') => { self.i += 1; self.unary() }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Option<f64> {
        let b = self.atom()?;
        if self.peek() == Some('^') { self.i += 1; let e = self.unary()?; return Some(b.powf(e)); }
        Some(b)
    }
    fn arg(&mut self) -> Option<f64> {
        if self.peek()? != '(' { return None; }
        self.i += 1; let a = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1;
        Some(a)
    }
    fn atom(&mut self) -> Option<f64> {
        match self.peek()? {
            '(' => { self.i += 1; let v = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1; Some(v) }
            c if c.is_ascii_digit() || c == '.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '.') { self.i += 1; }
                self.s[st..self.i].iter().collect::<String>().parse().ok()
            }
            '\u{3c0}' => { self.i += 1; Some(PI) }
            c if c.is_alphabetic() => {
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() { self.i += 1; }
                let name: String = self.s[st..self.i].iter().collect();
                // a run of single-letter factors such as "xy" or "pix" is split from the left
                match name.as_str() {
                    "x" => return Some(self.x),
                    "y" => return self.y,
                    "pi" => return Some(PI),
                    "e" => return Some(std::f64::consts::E),
                    _ => {}
                }
                let (to_r, from_r) = if self.deg { (PI / 180.0, 180.0 / PI) } else { (1.0, 1.0) };
                if name == "log" && self.peek() == Some('_') {
                    self.i += 1;
                    let base = self.atom()?;
                    let a = self.arg()?;
                    return Some(a.ln() / base.ln());
                }
                let known = ["sqrt", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "asin", "acos", "atan", "ln", "log", "exp"];
                if !known.contains(&name.as_str()) {
                    // implicit product of a known prefix, e.g. "xsin(x)" is rare; give up cleanly
                    if name.starts_with('x') { self.i = st + 1; return Some(self.x); }
                    return None;
                }
                let a = self.arg()?;
                Some(match name.as_str() {
                    "sqrt" => a.sqrt(), "abs" => a.abs(), "ln" => a.ln(), "log" => a.log10(), "exp" => a.exp(),
                    "sin" => (a * to_r).sin(), "cos" => (a * to_r).cos(), "tan" => (a * to_r).tan(),
                    "sec" => 1.0 / (a * to_r).cos(), "csc" => 1.0 / (a * to_r).sin(), "cot" => 1.0 / (a * to_r).tan(),
                    "arcsin" | "asin" => a.asin() * from_r, "arccos" | "acos" => a.acos() * from_r, "arctan" | "atan" => a.atan() * from_r,
                    _ => return None,
                })
            }
            _ => None,
        }
    }
}
/// Normalises typed math: unicode minus, middle dot, |a| -> abs(a), "sqrt x" stays as typed.
fn norm_expr(expr: &str) -> Vec<char> {
    let s = expr.replace('\u{2212}', "-").replace('\u{b7}', "*").replace('\u{d7}', "*").to_lowercase();
    let mut out = String::new();
    let mut open = false;
    for c in s.chars() {
        if c == '|' { out.push_str(if open { ")" } else { "abs(" }); open = !open; } else { out.push(c); }
    }
    out.chars().collect()
}
fn eval_full(expr: &str, x: f64, y: Option<f64>, deg: bool) -> Option<f64> {
    let cs = norm_expr(expr);
    let mut e = Ev { s: &cs, i: 0, x, y, deg };
    let v = e.expr()?;
    e.ws();
    if e.i == cs.len() { Some(v) } else { None }
}
pub(super) fn eval(expr: &str, x: f64) -> Option<f64> { eval_full(expr, x, None, false) }

const NUM_EVERY_LINE: [&str; 5] = ["Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"];

fn coordinate_grid_params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("xmin", "x-axis minimum (from)", -1000.0, 1000.0, 1.0, 0.0),
        ParamDef::float("xmax", "x-axis maximum (to)", -1000.0, 1000.0, 1.0, 10.0),
        ParamDef::float("xstep", "x-axis step (count by)", 0.05, 180.0, 0.5, 1.0),
        ParamDef::float("ymin", "y-axis minimum (from)", -1000.0, 1000.0, 1.0, 0.0),
        ParamDef::float("ymax", "y-axis maximum (to)", -1000.0, 1000.0, 1.0, 10.0),
        ParamDef::float("ystep", "y-axis step (count by)", 0.05, 180.0, 0.5, 1.0),
        ParamDef::int("minor", "Minor gridlines per step (0 = none)", 0, 10, 0),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::bool("arrows", "Arrowheads on axes", true),
        ParamDef::bool("square", "Square cells", true),
        ParamDef::text("xlabel", "x-axis label (at the arrow; empty = none)", "x"),
        ParamDef::text("ylabel", "y-axis label (at the arrow; empty = none)", "y"),
        ParamDef::text("title", "Title", ""),
        ParamDef::long("equations", "Equations, one per ; - y = 2x + 1; 2x + 3y = 6; x = 4; y > x - 2; -2 < x <= 3; y = x + 2, x < 1 (piece with a domain)", "y = 2x + 1"),
        ParamDef::bool("eq_labels", "Write each equation on its line", false),
        ParamDef::long("points", "Points - A(1, 2), B(-3, 4)  or  (2,3) A; (-1,4) B", ""),
        ParamDef::color("eq1_color", "Equation 1 colour", &SWATCHES, "#1f5fbf"),
        ParamDef::color("eq2_color", "Equation 2 colour", &SWATCHES, "#c0392b"),
        ParamDef::color("eq3_color", "Equation 3 colour", &SWATCHES, "#2a8a3e"),
        ParamDef::color("eq4_color", "Equation 4 colour", &SWATCHES, "#8e44ad"),
        ParamDef::color("pt_color", "Point colour", &SWATCHES, "#111111"),
        ParamDef::choice("title_mode", "Chart title", &TITLE_MODES, "Text"),
        ParamDef::choice("xtitle_mode", "x-axis title", &TITLE_MODES, "None"),
        ParamDef::text("xtitle", "x-axis title text", "Time (s)"),
        ParamDef::choice("ytitle_mode", "y-axis title", &TITLE_MODES, "None"),
        ParamDef::text("ytitle", "y-axis title text", "Distance (m)"),
        ParamDef::choice("xnum", "x-axis numbers", &NUM_EVERY_LINE, "Every line"),
        ParamDef::choice("ynum", "y-axis numbers", &NUM_EVERY_LINE, "Every line"),
        ParamDef::choice("xunits", "x-axis units (range and step typed in these)", &["Numbers", "Multiples of \u{3c0}", "Degrees"], "Numbers"),
        ParamDef::choice("grid", "Gridlines (bold for photocopies)", &["Light", "Standard", "Bold", "None"], "Standard"),
        ParamDef::choice("line_width", "Graph line thickness", &["Thin", "Medium", "Thick"], "Medium"),
        ParamDef::choice("shading", "Inequality shading", &["Tint", "Hatching"], "Tint"),
        ParamDef::bool("asymptotes", "Show vertical asymptotes (dotted)", false),
        ParamDef::bool("endpoints", "Endpoint circles on pieces with a domain", true),
        ParamDef::choice("pt_style", "Point style", &["Dot", "Cross"], "Dot"),
        ParamDef::choice("pt_label", "Point labels", &["Letter", "Letter and coordinates", "None"], "Letter"),
    ]
}

const PALETTE: [&str; 5] = ["#1f5fbf", "#c0392b", "#2a8a3e", "#8e44ad", "#d68910"];

/// Grid layout shared with the drag handles (math_plot::Cg): model ranges (x already in real
/// units, i.e. multiplied by pi in pi mode), scales, margins and plot size.
pub(super) struct GridL { pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64, pub xs: f64, pub ys: f64, pub ux: f64, pub uy: f64, pub ml: f64, pub mt: f64, pub pw: f64, pub ph: f64 }
pub(super) fn grid_layout(p: &Params) -> GridL {
    let k = if p.s("xunits") == "Multiples of \u{3c0}" { PI } else { 1.0 };
    let (x0, mut x1, y0, mut y1) = (p.f("xmin") * k, p.f("xmax") * k, p.f("ymin"), p.f("ymax"));
    if x1 <= x0 { x1 = x0 + k; }
    if y1 <= y0 { y1 = y0 + 1.0; }
    let (xs, ys) = (p.f("xstep").max(1e-6) * k, p.f("ystep").max(1e-6));
    let (maxw, maxh) = (440.0, 400.0);
    let (ux, uy) = if p.b("square") && k == 1.0 && p.s("xunits") != "Degrees" {
        let u = (maxw / (x1 - x0)).min(maxh / (y1 - y0)); (u, u)
    } else { (maxw / (x1 - x0), maxh / (y1 - y0)) };
    let (pw, ph) = ((x1 - x0) * ux, (y1 - y0) * uy);
    let title = match p.s("title_mode") { "None" => false, "Blank line" => true, _ => !p.s("title").is_empty() };
    let ml = 52.0 + if p.s("ytitle_mode") == "Text" || p.s("ytitle_mode") == "Blank line" { 26.0 } else { 0.0 };
    GridL { x0, x1, y0, y1, xs, ys, ux, uy, ml, mt: if title { 62.0 } else { 30.0 }, pw, ph }
}

/// "3π/4"-style label for v (in real units) when the axis counts in multiples of π.
fn pi_label(v: f64) -> String {
    let r = v / PI;
    if r.abs() < 1e-9 { return "0".into(); }
    for den in 1..=12i64 {
        let num = r * den as f64;
        if (num - num.round()).abs() < 1e-6 {
            let n = num.round() as i64;
            let sign = if n < 0 { "\u{2212}" } else { "" };
            let a = n.abs();
            let top = if a == 1 { "\u{3c0}".to_string() } else { format!("{a}\u{3c0}") };
            return if den == 1 { format!("{sign}{top}") } else { format!("{sign}{top}/{den}") };
        }
    }
    fmt(v, 2)
}
fn every_n(s: &str) -> i64 { match s { "Every 2nd line" => 2, "Every 5th line" => 5, "Every 10th line" => 10, "No numbers" => 0, _ => 1 } }

/// One typed row of the Equations box.
enum Row { Func(String), Vert(f64), Implicit(String), Ineq { rhs: String, above: bool, strict: bool }, Band(Vec<Interval>) }
/// Splits "y = x + 2, x < 1" into (main, domain) at the first comma outside brackets.
fn split_domain(s: &str) -> (&str, Option<&str>) {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c { '(' => depth += 1, ')' => depth -= 1, ',' if depth == 0 => return (&s[..i], Some(&s[i + 1..])), _ => {} }
    }
    (s, None)
}
fn parse_row(main: &str) -> Option<Row> {
    let m = main.replace('\u{2264}', "<=").replace('\u{2265}', ">=").replace('\u{2212}', "-");
    let lower = m.to_lowercase();
    if lower.contains('<') || lower.contains('>') {
        if !lower.contains('y') { return Some(Row::Band(parse_ineq(&lower))); }
        let (pos, len) = ["<=", ">=", "<", ">"].iter().find_map(|op| lower.find(op).map(|i| (i, op.len())))?;
        let op = &lower[pos..pos + len];
        let (l, r) = (lower[..pos].trim(), lower[pos + len..].trim());
        let strict = !op.contains('=');
        let gt = op.starts_with('>');
        return if l == "y" { Some(Row::Ineq { rhs: r.to_string(), above: gt, strict }) }
            else if r == "y" { Some(Row::Ineq { rhs: l.to_string(), above: !gt, strict }) }
            else { None };
    }
    match lower.find('=') {
        Some(i) => {
            let (l, r) = (lower[..i].trim(), lower[i + 1..].trim());
            if l == "x" { eval(r, 0.0).map(Row::Vert) }
            else if l == "y" || l == "f(x)" { Some(Row::Func(r.to_string())) }
            else if r == "y" { Some(Row::Func(l.to_string())) }
            else { Some(Row::Implicit(format!("({l})-({r})"))) }
        }
        None => Some(Row::Func(lower.trim().to_string())),
    }
}

fn coordinate_grid_render(p: &Params) -> String {
    let g = grid_layout(p);
    let (x0, x1, y0, y1, xs, ys, ux, uy, ml, mt, pw, ph) = (g.x0, g.x1, g.y0, g.y1, g.xs, g.ys, g.ux, g.uy, g.ml, g.mt, g.pw, g.ph);
    let pi_mode = p.s("xunits") == "Multiples of \u{3c0}";
    let deg = p.s("xunits") == "Degrees";
    let xtitle = p.s("xtitle_mode") == "Text" || p.s("xtitle_mode") == "Blank line";
    let (mr, mb) = (34.0, 46.0 + if xtitle { 26.0 } else { 0.0 });
    let (w, h) = (pw + ml + mr, ph + mt + mb);
    let mut d = new_doc(w, h);
    let sx = |x: f64| ml + (x - x0) * ux;
    let sy = |y: f64| mt + ph - (y - y0) * uy;
    let f = |expr: &str, x: f64| eval_full(expr, x, None, deg);
    let minor = p.i("minor");
    let gstyle = p.s("grid");
    let (gcol, gw, mcol, axw) = match gstyle { "Light" => ("#dde3ea", 0.9, "#eef1f5", 1.6), "Bold" => ("#6f7d8c", 1.4, "#b9c2cc", 2.6), "None" => ("none", 0.0, "none", 1.8), _ => ("#c4cdd8", 1.0, "#e6ebf1", 1.8) };
    // gridlines
    if gstyle != "None" {
        if minor > 1 {
            let ms = xs / minor as f64;
            let mut k = (x0 / ms).ceil();
            while k * ms <= x1 + 1e-9 { d.line(sx(k * ms), mt, sx(k * ms), mt + ph, mcol, 0.8); k += 1.0; }
            let ms = ys / minor as f64;
            let mut k = (y0 / ms).ceil();
            while k * ms <= y1 + 1e-9 { d.line(ml, sy(k * ms), ml + pw, sy(k * ms), mcol, 0.8); k += 1.0; }
        }
        let mut k = (x0 / xs - 1e-9).ceil();
        while k * xs <= x1 + 1e-9 { d.line(sx(k * xs), mt, sx(k * xs), mt + ph, gcol, gw); k += 1.0; }
        let mut k = (y0 / ys - 1e-9).ceil();
        while k * ys <= y1 + 1e-9 { d.line(ml, sy(k * ys), ml + pw, sy(k * ys), gcol, gw); k += 1.0; }
    }
    // axes
    let ax_y = if y0 <= 0.0 && y1 >= 0.0 { 0.0 } else { y0 };
    let ax_x = if x0 <= 0.0 && x1 >= 0.0 { 0.0 } else { x0 };
    let ext = if p.b("arrows") { 16.0 } else { 0.0 };
    let (axy, axx) = (sy(ax_y), sx(ax_x));
    if p.b("arrows") {
        d.arrow(ml, axy, ml + pw + ext, axy, INK, axw);
        d.arrow(axx, mt + ph, axx, mt - ext, INK, axw);
    } else {
        d.line(ml, axy, ml + pw, axy, INK, axw);
        d.line(axx, mt + ph, axx, mt, INK, axw);
    }
    // numbers + ticks
    if p.b("numbers") {
        let (xe, ye) = (every_n(p.s("xnum")), every_n(p.s("ynum")));
        let origin = ax_y == 0.0 && ax_x == 0.0;
        let mut k = (x0 / xs - 1e-9).ceil();
        let mut n = 0i64;
        while k * xs <= x1 + 1e-9 {
            let v = k * xs;
            d.line(sx(v), axy - 3.5, sx(v), axy + 3.5, INK, 1.2);
            let lab = if pi_mode { pi_label(v) } else if deg { format!("{}\u{b0}", fmt(v, 3)) } else { fmt(v, 3) };
            let numbered = xe > 0 && (k as i64).rem_euclid(xe) == 0;
            if numbered && !(v.abs() < 1e-9 && origin) { put(&mut d, sx(v), if axy + 18.0 > mt + ph + 30.0 { axy - 14.0 } else { axy + 15.0 }, &lab, 13.0); }
            k += 1.0; n += 1;
        }
        let _ = n;
        let mut k = (y0 / ys - 1e-9).ceil();
        while k * ys <= y1 + 1e-9 {
            let v = k * ys;
            d.line(axx - 3.5, sy(v), axx + 3.5, sy(v), INK, 1.2);
            let numbered = ye > 0 && (k as i64).rem_euclid(ye) == 0;
            if numbered && !(v.abs() < 1e-9 && origin) { txt(&mut d, axx - 8.0, sy(v) + 4.5, 13.0, "end", &fmt(v, 3)); }
            k += 1.0;
        }
        if origin && xe > 0 { txt(&mut d, axx - 7.0, axy + 15.0, 13.0, "end", "0"); }
    }
    if !p.s("xlabel").is_empty() { math_text(&mut d, ml + pw + ext + 6.0, axy, 17.0, "start", p.s("xlabel"), false); }
    if !p.s("ylabel").is_empty() { math_text(&mut d, axx, mt - ext - 10.0, 17.0, "middle", p.s("ylabel"), false); }
    match p.s("title_mode") {
        "None" => {}
        "Blank line" => { d.line(w / 2.0 - 90.0, 28.0, w / 2.0 + 90.0, 28.0, INK, 1.4); }
        _ => if !p.s("title").is_empty() { math_text(&mut d, w / 2.0, 18.0, 18.0, "middle", p.s("title"), true); },
    }
    match p.s("xtitle_mode") {
        "Text" => math_text(&mut d, ml + pw / 2.0, h - 16.0, 15.0, "middle", p.s("xtitle"), true),
        "Blank line" => { d.line(ml + pw / 2.0 - 70.0, h - 12.0, ml + pw / 2.0 + 70.0, h - 12.0, INK, 1.4); }
        _ => {}
    }
    match p.s("ytitle_mode") {
        "Text" => {
            let (cx, cy) = (16.0, mt + ph / 2.0);
            d.raw(&format!(r##"<g transform="rotate(-90 {cx:.2} {cy:.2})">"##));
            math_text(&mut d, cx, cy, 15.0, "middle", p.s("ytitle"), true);
            d.raw("</g>");
        }
        "Blank line" => { d.line(12.0, mt + ph / 2.0 - 70.0, 12.0, mt + ph / 2.0 + 70.0, INK, 1.4); }
        _ => {}
    }
    // graphs
    d.raw(&format!(r#"<clipPath id="plot"><rect x="{ml}" y="{mt}" width="{pw}" height="{ph}"/></clipPath>"#));
    let lw = match p.s("line_width") { "Thin" => 1.6, "Thick" => 3.6, _ => 2.6 };
    let hatch = p.s("shading") == "Hatching";
    let span_y = y1 - y0;
    for (ci, raw_row) in p.s("equations").split(';').enumerate() {
        let raw_row = raw_row.trim();
        if raw_row.is_empty() { continue; }
        let col_s = if ci < 4 { colp(p, &format!("eq{}_color", ci + 1), PALETTE[ci]) } else { PALETTE[ci % PALETTE.len()].to_string() };
        let col = col_s.as_str();
        let (main, dom) = split_domain(raw_row);
        let Some(row) = parse_row(main) else { continue };
        let dom_iv: Vec<Interval> = dom.map(|s| parse_ineq(&s.to_lowercase().replace('y', "x"))).unwrap_or_default();
        let in_dom = |v: f64| dom_iv.is_empty() || dom_iv.iter().any(|iv| (v > iv.lo || (iv.lo_c && v >= iv.lo)) && (v < iv.hi || (iv.hi_c && v <= iv.hi)));
        let fill_id = format!("sh{ci}");
        let fill = if hatch {
            d.raw(&format!(r##"<defs><pattern id="{fill_id}" patternUnits="userSpaceOnUse" width="9" height="9" patternTransform="rotate({})"><line x1="0" y1="0" x2="0" y2="9" stroke="{col}" stroke-width="1.6"/></pattern></defs>"##, if ci % 2 == 0 { 45 } else { -45 }));
            format!("url(#{fill_id})")
        } else { col.to_string() };
        let fill_op = if hatch { 1.0 } else { 0.18 };
        d.raw(r#"<g clip-path="url(#plot)">"#);
        // sampled curve y = g(x) on the domain, split at gaps and jumps (asymptotes)
        let sample = |gf: &dyn Fn(f64) -> Option<f64>| -> (Vec<Vec<(f64, f64)>>, Vec<f64>) {
            let steps = 900;
            let mut runs: Vec<Vec<(f64, f64)>> = vec![vec![]];
            let mut asym = vec![];
            let mut prev: Option<(f64, f64)> = None;
            for i in 0..=steps {
                let x = x0 + (x1 - x0) * i as f64 / steps as f64;
                let yv = if in_dom(x) { gf(x).filter(|y| y.is_finite() && y.abs() < 1e6) } else { None };
                match yv {
                    Some(y) => {
                        if let Some((px, py)) = prev {
                            let jump = (y - py).abs() > 2.0 * span_y && (y.max(py) > y1 || y.min(py) < y0) && (y - py).signum() != 0.0;
                            let steep_flip = jump && ((py > y1 && y < y0) || (py < y0 && y > y1));
                            if steep_flip { asym.push((px + x) / 2.0); runs.push(vec![]); }
                        }
                        runs.last_mut().unwrap().push((x, y));
                        prev = Some((x, y));
                    }
                    None => { if !runs.last().unwrap().is_empty() { runs.push(vec![]); } prev = None; }
                }
            }
            runs.retain(|r| r.len() > 1);
            (runs, asym)
        };
        let mut label_at: Option<(f64, f64)> = None;
        match &row {
            Row::Vert(v) => {
                let (ya, yb) = if dom_iv.is_empty() { (y0 - span_y, y1 + span_y) } else { (dom_iv[0].lo.max(y0 - span_y), dom_iv[0].hi.min(y1 + span_y)) };
                d.line(sx(*v), sy(ya), sx(*v), sy(yb), col, lw);
                label_at = Some((*v, y0 + span_y * 0.9));
                if p.b("endpoints") {
                    for iv in &dom_iv { for (b, c) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] { if b.is_finite() { d.circle(sx(*v), sy(b), 5.0, if c { col } else { "#fff" }, col, 2.0); } } }
                }
            }
            Row::Band(ivs) => {
                for iv in ivs {
                    let (a, b) = (iv.lo.max(x0 - 1.0), iv.hi.min(x1 + 1.0));
                    if b > a { d.raw(&format!(r##"<rect x="{:.2}" y="{mt}" width="{:.2}" height="{ph}" fill="{fill}" fill-opacity="{fill_op}"/>"##, sx(a), sx(b) - sx(a))); }
                    for (e, c) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] {
                        if e.is_finite() { dline(&mut d, (sx(e), mt), (sx(e), mt + ph), col, lw, if c { "none" } else { "9 6" }); }
                    }
                }
            }
            _ => {
                let expr: Box<dyn Fn(f64) -> Option<f64>> = match &row {
                    Row::Func(r) | Row::Ineq { rhs: r, .. } => { let r = r.clone(); Box::new(move |x| eval_full(&r, x, None, deg)) }
                    Row::Implicit(e) => {
                        let e = e.clone();
                        Box::new(move |x| {
                            let (a, b) = (eval_full(&e, x, Some(0.0), deg)?, eval_full(&e, x, Some(1.0), deg)?);
                            let slope = b - a;
                            (slope.abs() > 1e-12).then(|| -a / slope)
                        })
                    }
                    _ => unreachable!(),
                };
                let (runs, mut asym) = sample(&*expr);
                // holes and asymptotes sitting on grid points (x = k * step/4)
                let mut holes = vec![];
                if let Row::Func(r) = &row {
                    let q = xs / 4.0;
                    let mut k = (x0 / q).ceil();
                    let eps = (x1 - x0) * 1e-7;
                    while k * q <= x1 + 1e-9 {
                        let c = k * q;
                        if in_dom(c) && f(r, c).map_or(true, |v| !v.is_finite()) {
                            if let (Some(l), Some(rr)) = (f(r, c - eps).filter(|v| v.is_finite()), f(r, c + eps).filter(|v| v.is_finite())) {
                                if (l - rr).abs() < 1e-3 * (1.0 + l.abs()) { holes.push((c, (l + rr) / 2.0)); }
                                else if !asym.iter().any(|a| (a - c).abs() < (x1 - x0) / 200.0) { asym.push(c); }
                            }
                        }
                        k += 1.0;
                    }
                }
                let strict = matches!(row, Row::Ineq { strict: true, .. });
                if let Row::Ineq { above, .. } = row {
                    let edge = if above { y1 + span_y } else { y0 - span_y };
                    for r in &runs {
                        let mut poly: Vec<Pt> = r.iter().map(|(x, y)| (sx(*x), sy(y.clamp(y0 - span_y, y1 + span_y)))).collect();
                        poly.push((sx(r.last().unwrap().0), sy(edge)));
                        poly.push((sx(r[0].0), sy(edge)));
                        let pts: Vec<String> = poly.iter().map(|(a, b)| format!("{a:.2},{b:.2}")).collect();
                        d.raw(&format!(r##"<polygon points="{}" fill="{fill}" fill-opacity="{fill_op}" stroke="none"/>"##, pts.join(" ")));
                    }
                }
                for r in &runs {
                    let pts: Vec<Pt> = r.iter().map(|(x, y)| (sx(*x), sy(y.clamp(y0 - span_y, y1 + span_y)))).collect();
                    if strict { dpoly(&mut d, &pts, col, lw, "9 6"); } else { d.polyline(&pts, col, lw); }
                }
                if p.b("asymptotes") {
                    for a in &asym { dline(&mut d, (sx(*a), mt), (sx(*a), mt + ph), col, 1.4, "2 4"); }
                }
                for (hx, hy) in &holes { d.circle(sx(*hx), sy(*hy), 4.5, "#fff", col, 2.0); }
                // piece endpoints: closed for <=, open for <
                if p.b("endpoints") {
                    for iv in &dom_iv {
                        for (b, c) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] {
                            if b.is_finite() && b >= x0 - 1e-9 && b <= x1 + 1e-9 {
                                if let Some(yb) = expr(b).filter(|v| v.is_finite() && *v >= y0 && *v <= y1) { d.circle(sx(b), sy(yb), 5.0, if c { col } else { "#fff" }, col, 2.0); }
                            }
                        }
                    }
                }
                let xl = x0 + (x1 - x0) * (0.55 + 0.08 * (ci % 3) as f64);
                if let Some(yv) = expr(xl).filter(|v| *v > y0 && *v < y1) { label_at = Some((xl, yv)); }
            }
        }
        d.raw("</g>");
        if p.b("eq_labels") {
            if let Some((lx, ly)) = label_at { math_text(&mut d, sx(lx) + 8.0, sy(ly) - 12.0, 15.0, "start", raw_row, false); }
        }
    }
    // points: "A(1, 2), B(-3, 4)" or the older "(2,3) A; (-1,4) B"
    let pcol = colp(p, "pt_color", INK);
    for seg in p.s("points").split(';') {
        let seg = seg.trim();
        let groups: Vec<(usize, usize)> = {
            let mut v = vec![];
            let mut at = 0;
            while let Some(a) = seg[at..].find('(') {
                let a = at + a;
                let Some(b) = seg[a..].find(')').map(|b| a + b) else { break };
                v.push((a, b));
                at = b + 1;
            }
            v
        };
        for (gi, (a, b)) in groups.iter().enumerate() {
            let n = nums(&seg[a + 1..*b]);
            if n.len() != 2 { continue; }
            let prev_end = if gi == 0 { 0 } else { groups[gi - 1].1 + 1 };
            let prefix: String = seg[prev_end..*a].trim().trim_start_matches(',').trim().to_string();
            let name = if !prefix.is_empty() { prefix } else if groups.len() == 1 { seg[b + 1..].trim().to_string() } else { String::new() };
            let (px, py) = (sx(n[0]), sy(n[1]));
            if p.s("pt_style") == "Cross" {
                d.line(px - 5.0, py - 5.0, px + 5.0, py + 5.0, &pcol, 2.2);
                d.line(px - 5.0, py + 5.0, px + 5.0, py - 5.0, &pcol, 2.2);
            } else { d.circle(px, py, 4.5, &pcol, "#fff", 1.2); }
            let coord = format!("({}, {})", fmt(n[0], 3), fmt(n[1], 3));
            let lab = match p.s("pt_label") {
                "None" => String::new(),
                "Letter and coordinates" => if name.is_empty() { coord } else { format!("{name}{coord}") },
                _ => name,
            };
            if !lab.is_empty() { math_text(&mut d, px + 7.0, py - 11.0, 15.0, "start", &lab, false); }
        }
    }
    d.finish()
}

// ───────────────────────── number line ─────────────────────────

#[derive(Clone, Copy)]
pub(super) struct Interval { pub lo: f64, pub lo_c: bool, pub hi: f64, pub hi_c: bool }

pub(super) fn parse_ineq(s: &str) -> Vec<Interval> {
    let s = s.replace('\u{2264}', "<=").replace('\u{2265}', ">=").replace('\u{2212}', "-").replace(" ", "").to_lowercase();
    let mut out = vec![];
    let s = s.replace("or", "|").replace('\u{222a}', "|").replace("and", "&");
    for part in s.split('|') {
        let mut iv = Interval { lo: f64::NEG_INFINITY, lo_c: false, hi: f64::INFINITY, hi_c: false };
        for sub in part.split('&') {
            // tokenise into operands and ops
            let cs: Vec<char> = sub.chars().collect();
            let (mut ops, mut opnds, mut cur) = (vec![], vec![], String::new());
            let mut i = 0;
            while i < cs.len() {
                let c = cs[i];
                if c == '<' || c == '>' || c == '=' {
                    let mut op = c.to_string();
                    if i + 1 < cs.len() && cs[i + 1] == '=' && c != '=' { op.push('='); i += 1; }
                    opnds.push(std::mem::take(&mut cur)); ops.push(op);
                } else { cur.push(c); }
                i += 1;
            }
            opnds.push(cur);
            if ops.is_empty() { continue; }
            // build constraints between consecutive operands
            for (k, op) in ops.iter().enumerate() {
                let (l, r) = (&opnds[k], &opnds[k + 1]);
                let (xl, val) = if l == "x" { (true, r) } else if r == "x" { (false, l) } else { continue };
                let v: f64 = match val.parse() { Ok(v) => v, Err(_) => match eval(val, 0.0) { Some(v) => v, None => continue } };
                let closed = op.contains('=');
                let gt = if xl { op.starts_with('>') } else { op.starts_with('<') };
                if op == "=" { iv.lo = v; iv.hi = v; iv.lo_c = true; iv.hi_c = true; }
                else if gt { if v > iv.lo || (v == iv.lo && !closed) { iv.lo = v; iv.lo_c = closed; } }
                else if v < iv.hi || (v == iv.hi && !closed) { iv.hi = v; iv.hi_c = closed; }
            }
        }
        if iv.lo.is_finite() || iv.hi.is_finite() { out.push(iv); }
    }
    out
}

fn number_line_params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("min", "Minimum (from)", -1000.0, 1000.0, 1.0, -10.0),
        ParamDef::float("max", "Maximum (to)", -1000.0, 1000.0, 1.0, 10.0),
        ParamDef::float("step", "Tick step (count by)", 0.05, 100.0, 0.5, 1.0),
        ParamDef::int("label_every", "Number every n-th tick", 1, 20, 1),
        ParamDef::int("minor", "Minor ticks between ticks (0 = none)", 0, 9, 0),
        ParamDef::bool("numbers", "Show numbers", true),
        ParamDef::bool("arrows", "Arrowheads", true),
        ParamDef::text("inequality", "Inequality to graph (e.g. -2 < x <= 5, x > 3, x < -4 or x >= 1, x != 2, all real numbers, no solution)", "-2 < x <= 5"),
        ParamDef::list("points", "Points to mark: 3, -4, 5o (open), P(3/4), Q(3/2)", ""),
        ParamDef::text("label", "Axis label", ""),
        ParamDef::color("ineq_color", "Inequality colour", &SWATCHES, "#1f5fbf"),
        ParamDef::color("pt_color", "Point colour", &SWATCHES, "#c0392b"),
        ParamDef::text("step_text", "Count by as a fraction, e.g. 1/4 (overrides the tick step; numbers become fractions)", ""),
        ParamDef::choice("frac_style", "Fractions written as", &["Improper (5/4)", "Mixed (1 1/4)"], "Improper (5/4)"),
        ParamDef::text("inequality2", "Second graph, drawn separately above the line (its own colour)", ""),
        ParamDef::color("ineq2_color", "Second graph colour", &SWATCHES, "#c0392b"),
        ParamDef::bool("pt_values", "Write the value above unlettered points that are off the numbered ticks", false),
    ]
}

/// A typed number: 3, -2.5, 3/4, 1 1/4, pi/2.
pub(super) fn nl_num(s: &str) -> Option<f64> {
    let t = s.trim().replace('\u{2212}', "-");
    if t.is_empty() { return None; }
    let parts: Vec<&str> = t.split_whitespace().collect();
    if parts.len() == 2 && parts[1].contains('/') {
        let (w, f) = (parts[0].parse::<f64>().ok()?, eval(parts[1], 0.0)?);
        return Some(if w < 0.0 { w - f } else { w + f });
    }
    t.parse::<f64>().ok().or_else(|| eval(&t, 0.0))
}
/// Tick step and, when counted by a fraction a/b, its denominator b.
pub(super) fn nl_step(p: &Params) -> (f64, Option<i64>) {
    let st = p.s("step_text").trim();
    if let Some(v) = nl_num(st).filter(|v| *v > 1e-9) {
        let den = st.split('/').nth(1).and_then(|d| d.trim().parse::<i64>().ok()).filter(|d| *d > 1);
        return (v, den);
    }
    (p.f("step").max(1e-6), None)
}
/// Stacked fraction label n/d (already reduced) centred at x; `mixed` writes whole parts.
fn frac_label(d: &mut Doc, x: f64, y: f64, v: f64, den: i64, mixed: bool, size: f64) {
    let mut n = (v * den as f64).round() as i64;
    let mut dd = den;
    let g = { let (mut a, mut b) = (n.abs(), dd); while b != 0 { let t = a % b; a = b; b = t; } a.max(1) };
    n /= g; dd /= g;
    if dd == 1 { put(d, x, y, &fmt(n as f64, 0), size); return; }
    let neg = n < 0;
    let (whole, rem) = if mixed { (n.abs() / dd, n.abs() % dd) } else { (0, n.abs()) };
    let sign = if neg { "\u{2212}" } else { "" };
    let ws = if whole > 0 { format!("{sign}{whole}") } else { sign.to_string() };
    let fw = stw(&rem.max(dd).to_string(), size * 0.8).max(size * 0.6);
    let total = stw(&ws, size) + fw;
    let left = x - total / 2.0;
    if !ws.is_empty() { txt(d, left, y + size * 0.34, size, "start", &ws); }
    let cx = left + stw(&ws, size) + fw / 2.0 + 1.0;
    txt(d, cx, y - 2.0, size * 0.8, "middle", &rem.to_string());
    d.line(cx - fw / 2.0, y + 1.0, cx + fw / 2.0, y + 1.0, INK, 1.0);
    txt(d, cx, y + size * 0.8 + 1.0, size * 0.8, "middle", &dd.to_string());
}

/// Intervals for one graph row, with the extra words the reference accepts.
fn nl_intervals(s: &str) -> Vec<Interval> {
    let l = s.trim().to_lowercase();
    if l.is_empty() || l.contains("no solution") { return vec![]; }
    if l.contains("all real") { return vec![Interval { lo: f64::NEG_INFINITY, lo_c: false, hi: f64::INFINITY, hi_c: false }]; }
    let ne = l.replace('\u{2260}', "!=");
    if let Some(i) = ne.find("!=") {
        if let Some(v) = nl_num(&ne[i + 2..]) {
            return vec![Interval { lo: f64::NEG_INFINITY, lo_c: false, hi: v, hi_c: false }, Interval { lo: v, lo_c: false, hi: f64::INFINITY, hi_c: false }];
        }
    }
    parse_ineq(&l)
}

fn number_line_render(p: &Params) -> String {
    let (lo, mut hi) = (p.f("min"), p.f("max"));
    if hi <= lo { hi = lo + 1.0; }
    let (step, den) = nl_step(p);
    let mixed = p.s("frac_style").starts_with("Mixed");
    let (w, h) = (680.0, 120.0 + if den.is_some() { 8.0 } else { 0.0 });
    let (x0, x1) = (40.0, w - 40.0);
    let y = 64.0;
    let sx = |v: f64| x0 + (v - lo) / (hi - lo) * (x1 - x0);
    let mut d = new_doc(w, h);
    if p.b("arrows") { d.arrow(x1 - 10.0, y, x1 + 22.0, y, INK, 1.8); d.arrow(x0 + 10.0, y, x0 - 22.0, y, INK, 1.8); }
    d.line(x0, y, x1, y, INK, 1.8);
    let k0 = (lo / step - 1e-9).ceil() as i64;
    let k1 = ((hi + 1e-9) / step).floor() as i64;
    let every = p.i("label_every").max(1);
    let minor = p.i("minor");
    let mut numbered: Vec<f64> = vec![];
    for k in k0..=k1 {
        let v = k as f64 * step;
        d.line(sx(v), y - 8.0, sx(v), y + 8.0, INK, 1.5);
        if p.b("numbers") && (k - k0) % every == 0 {
            numbered.push(v);
            match den { Some(dd) => frac_label(&mut d, sx(v), y + 24.0, v, dd, mixed, 14.0), None => put(&mut d, sx(v), y + 24.0, &fmt(v, 3), 14.0) }
        }
        if minor > 0 && k < k1 {
            for m in 1..=minor { let mv = v + step * m as f64 / (minor + 1) as f64; d.line(sx(mv), y - 4.0, sx(mv), y + 4.0, INK, 1.0); }
        }
    }
    if !p.s("label").is_empty() { math_text(&mut d, x1 + 26.0, y, 16.0, "start", p.s("label"), false); }
    // graphs: row 1 on the line, row 2 (its own colour) drawn separately just above it
    let rows = [(p.s("inequality").to_string(), colp(p, "ineq_color", BLUE), 0.0), (p.s("inequality2").to_string(), colp(p, "ineq2_color", "#c0392b"), -18.0)];
    for (text, col, dy) in rows.iter() {
        let col = col.as_str();
        let yy = y + dy;
        for iv in nl_intervals(text) {
            let a = if iv.lo.is_finite() { iv.lo.max(lo) } else { lo };
            let b = if iv.hi.is_finite() { iv.hi.min(hi) } else { hi };
            if b >= a && iv.hi != iv.lo { d.line(sx(a), yy, sx(b), yy, col, 5.0); }
            if !iv.lo.is_finite() && p.b("arrows") { d.arrow(sx(lo) + 4.0, yy, x0 - 24.0, yy, col, 3.5); }
            if !iv.hi.is_finite() && p.b("arrows") { d.arrow(x1 - 4.0, yy, x1 + 24.0, yy, col, 3.5); }
            for (v, closed) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] {
                if v.is_finite() && v >= lo - 1e-9 && v <= hi + 1e-9 {
                    d.circle(sx(v), yy, 6.5, if closed { col } else { "#fff" }, col, 2.4);
                }
            }
        }
    }
    let red_s = colp(p, "pt_color", "#c0392b");
    let red = red_s.as_str();
    for t in p.s("points").split(',') {
        let t = t.trim();
        if t.is_empty() { continue; }
        let (t, open) = match t.strip_suffix('o') { Some(s) if !s.ends_with(|c: char| c.is_alphabetic()) => (s, true), _ => (t, false) };
        // "P(3/4)": a letter names the point
        let (name, num) = match (t.find('('), t.rfind(')')) {
            (Some(a), Some(b)) if b > a && a > 0 => (t[..a].trim(), &t[a + 1..b]),
            _ => ("", t),
        };
        let Some(v) = nl_num(num) else { continue };
        if v < lo - 1e-9 || v > hi + 1e-9 { continue; }
        d.circle(sx(v), y, 6.0, if open { "#fff" } else { red }, red, 2.2);
        if !name.is_empty() {
            d.raw(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="17" text-anchor="middle" font-family="{SERIF}" font-style="italic" font-weight="bold" fill="#000">{}</text>"##, sx(v), y - 16.0, super::svg::esc(name)));
        } else if p.b("pt_values") && !numbered.iter().any(|n| (n - v).abs() < 1e-9) {
            match den { Some(dd) if ((v * dd as f64) - (v * dd as f64).round()).abs() < 1e-6 => frac_label(&mut d, sx(v), y - 26.0, v, dd, mixed, 13.0), _ => put(&mut d, sx(v), y - 18.0, &fmt(v, 3), 13.0) }
        }
    }
    d.finish()
}

// ───────────────────────── angles & parallel lines ─────────────────────────

/// Screen point `q` turned `deg` counterclockwise (as seen) about screen point `c`.
pub(super) fn turn_scr(q: Pt, c: Pt, deg: f64) -> Pt {
    let (s, co) = (deg.to_radians().sin(), deg.to_radians().cos());
    let (dx, dy) = (q.0 - c.0, q.1 - c.1);
    (c.0 + dx * co + dy * s, c.1 - dx * s + dy * co)
}

/// `2 = 2x + 10; 3 = y` -> [(2, "2x + 10"), (3, "y")].
fn num_text_map(s: &str) -> Vec<(usize, String)> {
    s.split(|c| c == ';' || c == '\n')
        .filter_map(|it| { let (k, v) = it.split_once('=')?; Some((k.trim().parse().ok()?, v.trim().to_string())) })
        .collect()
}

/// Angular sector mark (math degrees ccw, y-up) at screen center c, with a math-typeset label.
fn sector(d: &mut Doc, c: Pt, a0: f64, a1: f64, r: f64, text: &str, right: bool, size: f64) {
    let dir = |a: f64| (a.to_radians().cos(), -a.to_radians().sin());
    let span = a1 - a0;
    if right && (span - 90.0).abs() < 0.3 {
        let (u, w) = (dir(a0), dir(a1));
        let s = r * 0.55;
        let p1 = (c.0 + u.0 * s, c.1 + u.1 * s);
        let p3 = (c.0 + w.0 * s, c.1 + w.1 * s);
        d.polyline(&[p1, (p1.0 + w.0 * s, p1.1 + w.1 * s), p3], INK, 1.3);
    } else {
        let n = 24;
        let pts: Vec<Pt> = (0..=n).map(|i| { let a = a0 + span * i as f64 / n as f64; let q = dir(a); (c.0 + q.0 * r, c.1 + q.1 * r) }).collect();
        d.polyline(&pts, INK, 1.4);
    }
    if !text.is_empty() {
        let mid = dir((a0 + a1) / 2.0);
        let tr = r + 8.0 + 0.5 * (tw(text, size) * 0.85 * mid.0.abs() + size * mid.1.abs());
        mlabel(d, c.0 + mid.0 * tr, c.1 + mid.1 * tr, size, text, "");
    }
}

/// Canvas size and parallel-line rows (screen y) for the parallel-lines figure.
pub(super) fn pl_layout(p: &Params) -> (f64, f64, Vec<f64>) {
    let nl = p.i("lines").clamp(2, 3) as usize;
    let gap = 120.0 * p.f("gap").clamp(0.5, 3.0);
    let (w, h) = (520.0, 240.0 + gap * (nl as f64 - 1.0));
    let ys = (0..nl).map(|i| h / 2.0 - gap * (nl as f64 - 1.0) / 2.0 + gap * i as f64).collect();
    (w, h, ys)
}

/// Filled arrowheads at both ends of line a-b (a line, not a segment).
fn line_heads(d: &mut Doc, a: Pt, b: Pt) {
    for (from, to) in [(a, b), (b, a)] {
        let u = unit_v(from, to);
        let nn = (-u.1, u.0);
        let back = (to.0 - u.0 * 12.0, to.1 - u.1 * 12.0);
        d.poly(&[to, (back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, INK, 1.0);
    }
}

// ───────────────────────── 3D solids ─────────────────────────

type P3 = (f64, f64, f64);
const OBL_K: f64 = 0.5;
const OBL_A: f64 = 40.0;
/// Cabinet-style oblique projection; `sx` = +1 sends depth back to the right, -1 to the left.
pub(super) fn oblp(p: P3, sx: f64) -> Pt {
    let a = OBL_A.to_radians();
    (p.0 + sx * p.1 * OBL_K * a.cos(), p.2 + p.1 * OBL_K * a.sin())
}
/// Direction toward the viewer for `oblp` (faces whose normal points this way are visible).
fn oblv(sx: f64) -> P3 { let a = OBL_A.to_radians(); (sx * OBL_K * a.cos(), -1.0, OBL_K * a.sin()) }
pub(super) fn depth_sx(p: &Params) -> f64 { if p.s("depth") == "Left" { -1.0 } else { 1.0 } }
fn sub3(a: P3, b: P3) -> P3 { (a.0 - b.0, a.1 - b.1, a.2 - b.2) }
fn dot3(a: P3, b: P3) -> f64 { a.0 * b.0 + a.1 * b.1 + a.2 * b.2 }
fn dist3(a: P3, b: P3) -> f64 { dot3(sub3(a, b), sub3(a, b)).sqrt() }
fn mid3(a: P3, b: P3) -> P3 { ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0) }

// ----- solid labels: serif, sized, per-measure Measure / Text / None -----
const SERIF: &str = "Liberation Serif, DejaVu Serif, Times New Roman, Times, serif";
fn lsz(p: &Params) -> f64 { match p.s("label_size") { "Large" => 22.0, "Small" => 14.0, _ => 18.0 } }
fn label_size_param() -> ParamDef { ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium") }
/// Short letter labels (r, h, x, A, B1) are set in italic like math symbols.
fn is_var(s: &str) -> bool {
    let t = s.trim();
    let mut ch = t.chars();
    matches!(ch.next(), Some(c) if c.is_alphabetic()) && t.chars().count() <= 3 && t.chars().all(|c| c.is_alphanumeric() || c == '\'')
}
fn stw(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.5 }
/// Solid label centred on (x, y). A non-empty `id` makes it a movable label (`labels::wrap`).
fn sput(d: &mut Doc, id: &str, x: f64, y: f64, text: &str, size: f64) {
    let style = if is_var(text) { r#" font-style="italic""# } else { "" };
    let t = format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{size}" text-anchor="middle" font-family="{SERIF}"{style} fill="#000" stroke="#fff" stroke-width="3" stroke-linejoin="round" paint-order="stroke">{}</text>"##, y + size * 0.34, super::svg::esc(text));
    if id.is_empty() { d.raw(&t); } else { d.raw(&super::labels::wrap(id, (x, y), super::labels::text_half(text, size), &t)); }
}
/// Label beside screen segment a-b at fraction t, on the side away from `center`.
fn sbeside(d: &mut Doc, id: &str, a: Pt, b: Pt, t: f64, center: Pt, text: &str, size: f64) {
    let m = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    let u = unit_v(a, b);
    let mut n = (-u.1, u.0);
    if n.0 * (m.0 - center.0) + n.1 * (m.1 - center.1) < 0.0 { n = (-n.0, -n.1); }
    let off = 7.0 + 0.5 * (stw(text, size) * n.0.abs() + size * 0.9 * n.1.abs());
    sput(d, id, m.0 + n.0 * off, m.1 + n.1 * off, text, size);
}
/// Label for measure `key` of value `v`: `<key>_lab` picks Measure / Text / None (`def` when the
/// key is absent, e.g. older saved figures) and `<key>_label` holds the text. `hidden` comes from
/// the older "Dimensions to hide" list and always wins.
fn mtext(p: &Params, key: &str, def: &str, v: f64, hidden: bool) -> Option<String> {
    if hidden { return None; }
    let mode = match p.s(&format!("{key}_lab")) { "" => def, m => m };
    match mode {
        "Text" => { let t = p.s(&format!("{key}_label")).trim(); (!t.is_empty()).then(|| t.to_string()) }
        "None" => None,
        _ => Some(fmtu(v, p.i("dec"), p.s("unit"))),
    }
}
const LAB_OPTS: [&str; 3] = ["Measure", "Text", "None"];
fn hide_list(p: &Params) -> Vec<String> { tokens(p.s("hide")).iter().map(|s| s.to_lowercase()).collect() }
fn corner_name(i: usize) -> String {
    let c = (b'A' + (i % 26) as u8) as char;
    if i < 26 { c.to_string() } else { format!("{c}{}", i / 26) }
}
/// Corner letter just outside vertex `q` (screen), pushed away from `center`.
fn name_at(d: &mut Doc, id: &str, q: Pt, center: Pt, name: &str, size: f64) {
    let u = unit_v(center, q);
    let u = if u.0.is_finite() { u } else { (0.0, -1.0) };
    sput(d, id, q.0 + u.0 * (size * 0.9), q.1 + u.1 * (size * 0.9), name, size);
}

struct Solid { v: Vec<P3>, faces: Vec<Vec<usize>> }

/// Draw a convex solid: shaded visible faces, solid visible edges, dashed hidden edges (when
/// `hidden`).
fn draw_solid(d: &mut Doc, m: &Map, s: &Solid, proj: &dyn Fn(P3) -> Pt, view: P3, hidden: bool) {
    let n = s.v.len() as f64;
    let cen = (s.v.iter().map(|p| p.0).sum::<f64>() / n, s.v.iter().map(|p| p.1).sum::<f64>() / n, s.v.iter().map(|p| p.2).sum::<f64>() / n);
    let mut vis = vec![];
    let mut normals = vec![];
    for f in &s.faces {
        let mut nrm = (0.0, 0.0, 0.0);
        for i in 0..f.len() {
            let (a, b) = (s.v[f[i]], s.v[f[(i + 1) % f.len()]]);
            nrm = (nrm.0 + (a.1 - b.1) * (a.2 + b.2), nrm.1 + (a.2 - b.2) * (a.0 + b.0), nrm.2 + (a.0 - b.0) * (a.1 + b.1));
        }
        let fc = f.iter().fold((0.0, 0.0, 0.0), |acc, i| (acc.0 + s.v[*i].0, acc.1 + s.v[*i].1, acc.2 + s.v[*i].2));
        let fc = (fc.0 / f.len() as f64, fc.1 / f.len() as f64, fc.2 / f.len() as f64);
        if dot3(nrm, sub3(fc, cen)) < 0.0 { nrm = (-nrm.0, -nrm.1, -nrm.2); }
        let l = dot3(nrm, nrm).sqrt().max(1e-12);
        nrm = (nrm.0 / l, nrm.1 / l, nrm.2 / l);
        vis.push(dot3(nrm, view) > 1e-6);
        normals.push(nrm);
    }
    for (fi, f) in s.faces.iter().enumerate() {
        if !vis[fi] { continue; }
        let l = dot3(normals[fi], (-0.35, -0.45, 0.82));
        let col = mix((196.0, 216.0, 240.0), (246.0, 250.0, 255.0), (l + 1.0) / 2.0);
        let pts: Vec<Pt> = f.iter().map(|i| m.p(proj(s.v[*i]))).collect();
        d.poly(&pts, &col, &col, 0.8);
    }
    let mut edges: std::collections::BTreeMap<(usize, usize), bool> = Default::default();
    for (fi, f) in s.faces.iter().enumerate() {
        for i in 0..f.len() {
            let (a, b) = (f[i], f[(i + 1) % f.len()]);
            let e = edges.entry((a.min(b), a.max(b))).or_insert(false);
            if vis[fi] { *e = true; }
        }
    }
    if hidden {
        for ((a, b), visible) in &edges {
            if !*visible { dline(d, m.p(proj(s.v[*a])), m.p(proj(s.v[*b])), "#555", 1.4, "6 4"); }
        }
    }
    for ((a, b), visible) in &edges {
        if *visible { let (p, q) = (m.p(proj(s.v[*a])), m.p(proj(s.v[*b]))); d.line(p.0, p.1, q.0, q.1, INK, 2.2); }
    }
}

pub(super) fn poly_base(kind: &str, l: f64, w: f64, n: usize, side: f64) -> Vec<P3> {
    match kind {
        "Right triangle" => vec![(0.0, 0.0, 0.0), (l, 0.0, 0.0), (l, w, 0.0)],
        "Isosceles triangle" => vec![(0.0, 0.0, 0.0), (l, 0.0, 0.0), (l / 2.0, w, 0.0)],
        "Regular polygon" => {
            let half = PI / n as f64;
            let r = side / (2.0 * half.sin());
            (0..n).map(|k| { let t = -PI / 2.0 - half + 2.0 * PI * k as f64 / n as f64; (r * t.cos(), r * t.sin(), 0.0) }).collect()
        }
        _ => vec![(0.0, 0.0, 0.0), (l, 0.0, 0.0), (l, w, 0.0), (0.0, w, 0.0)],
    }
}

/// Right-angle square at 3D point o with arms toward o+e1 and o+e2 (screen size fixed).
fn rmark3(d: &mut Doc, m: &Map, proj: &dyn Fn(P3) -> Pt, o: P3, e1: P3, e2: P3) {
    let (po, p1, p2) = (m.p(proj(o)), m.p(proj((o.0 + e1.0, o.1 + e1.1, o.2 + e1.2))), m.p(proj((o.0 + e2.0, o.1 + e2.1, o.2 + e2.2))));
    let (u, w) = (unit_v(po, p1), unit_v(po, p2));
    let s = 10.0;
    let a = (po.0 + u.0 * s, po.1 + u.1 * s);
    let c = (po.0 + w.0 * s, po.1 + w.1 * s);
    d.polyline(&[a, (a.0 + w.0 * s, a.1 + w.1 * s), c], INK, 1.2);
}

fn solid_common(unit: &str) -> Vec<ParamDef> {
    vec![
        ParamDef::bool("hidden", "Show hidden edges (dashed)", true),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::text("unit", "Unit", unit),
        ParamDef::int("dec", "Decimal places", 0, 3, 1),
    ]
}

// ----- solids' Measures card: typed (given) vs worked-out values -----

/// A solid's optional measure kept as text ("" = worked out from the others): the typed value
/// when it is a positive number or expression (sqrt, pi, fractions), else None.
pub(super) fn typed(p: &Params, k: &str) -> Option<f64> {
    let t = p.s(k).trim();
    if t.is_empty() { return None; }
    eval(&t.replace('\u{221a}', "sqrt").replace('\u{3c0}', "pi"), 0.0).filter(|v| v.is_finite() && *v > 0.0)
}
/// Label for a measure whose value box is text (`vk`): Measure prints what was typed ("as you
/// typed it") or the worked-out value rounded; Text and None as usual.
fn tlabel(p: &Params, key: &str, vk: &str, def: &str, v: f64) -> Option<String> {
    let mode = match p.s(&format!("{key}_lab")) { "" => def, m => m };
    if mode == "Measure" && typed(p, vk).is_some() {
        let t = p.s(vk).trim().replace("sqrt", "\u{221a}").replace("pi", "\u{3c0}");
        let u = p.s("unit").trim();
        return Some(if u.is_empty() { t } else { format!("{t} {u}") });
    }
    mtext(p, key, def, v, false)
}
/// Height of an oblique prism/cylinder: typed, or worked out from a typed slanted edge and the
/// lean (any two of height, lean and slanted edge, as the reference allows).
pub(super) fn edge_height(p: &Params, oblique: bool) -> f64 {
    if oblique && let Some(e) = typed(p, "edge") {
        let l = p.f("lean");
        if e > l.abs() + 1e-6 { return (e * e - l * l).sqrt(); }
    }
    p.f("height")
}
/// Worked-out text of a measure (number rounded to the figure's decimals, no unit).
fn ctext(p: &Params, v: f64) -> String { fmt(v, p.i("dec")) }
/// Diameter row of the round solids: a linked row (= 2 x radius) with its own label.
fn diameter_params() -> [ParamDef; 2] {
    [
        ParamDef::choice("d_lab", "Diameter label (drawn right across the circle)", &LAB_OPTS, "None"),
        ParamDef::text("d_label", "Diameter label text", "d"),
    ]
}
/// The reference's SHAPE/Measures card of each solid. The card is static, so rows that only apply
/// to one base or to oblique solids are grouped and say so.
fn measures_solid(id: &str) -> Option<super::MeasureCard> {
    use super::{MeasureCard, MeasureGroup, MeasureRow};
    let row = |name, value_key, label_key, text_key| MeasureRow { name, name_key: None, value_key, unit: "$unit", label_key, text_key, tick_key: None, equals: None };
    let link = |name, value_key, label_key, text_key, eq| MeasureRow { equals: Some(eq), ..row(name, value_key, label_key, text_key) };
    let g = |title, rows| MeasureGroup { title, rows };
    // the Diameter row (value "diam") replaces Radius with "Give the diameter instead" (card_visible)
    let radius = || vec![row("Radius", "radius", "r_lab", "r_label"), row("Diameter", "diam", "r_lab", "d_label"), link("Diameter", "diameter", "d_lab", "d_label", "2 \u{d7} radius")];
    let (help, groups) = match id {
        "sphere" => ("Give the radius. The diameter is worked out; its label button draws it right across the circle.", vec![g("", radius())]),
        "cylinder" => (
            "Give every measure. Worked-out measures are shown faintly. Oblique: give the slanted edge to have the height worked out from it and the lean. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}.",
            vec![g("", { let mut r = radius(); r.push(row("Height", "height", "h_lab", "h_label")); r }), g("Oblique", vec![row("Lean", "lean", "lean_lab", "lean_label"), row("Slanted edge", "edge", "edge_lab", "edge_label")])],
        ),
        "cone" => (
            "Give the radius and the height, or type the slant height to have the height worked out. Worked-out measures are shown faintly. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}.",
            vec![
                g("", { let mut r = radius(); r.extend([row("Height", "height", "h_lab", "h_label"), row("Slant height", "slant", "l_lab", "l_label")]); r }),
                g("Oblique", vec![row("Lean", "lean", "lean_lab", "lean_label")]),
            ],
        ),
        "pyramid" => (
            "Give every measure. Type the slant height to have the height worked out. Worked-out measures are shown faintly. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}.",
            vec![
                g("Rectangle base", vec![row("Length", "length", "length_lab", "length_label"), row("Width", "width", "width_lab", "width_label")]),
                g("Regular polygon base", vec![row("Side", "side", "side_lab", "side_label"), link("Apothem", "apothem", "apothem_lab", "apothem_label", "worked out")]),
                g("", vec![row("Height", "height", "height_lab", "height_label"), row("Slant height", "slant", "slant_lab", "slant_label")]),
                g("Oblique", vec![row("Lean", "lean", "lean_lab", "lean_label")]),
            ],
        ),
        "prism" => (
            "Give every measure. For a triangle base, Length is the triangle's base and Width the triangle's height; when the prism lies on its side, Height is its length. Oblique: give the slanted edge to have the height worked out. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}.",
            vec![
                g("Rectangle or triangle base", vec![
                    row("Length", "length", "length_lab", "length_label"),
                    row("Width", "width", "width_lab", "width_label"),
                    link("Slanted side", "hyp", "hyp_lab", "hyp_label", "worked out"),
                ]),
                g("Regular polygon base", vec![row("Side", "side", "side_lab", "side_label"), link("Apothem", "apothem", "apothem_lab", "apothem_label", "worked out")]),
                g("", vec![row("Height", "height", "height_lab", "height_label")]),
                g("Oblique", vec![row("Lean", "lean", "lean_lab", "lean_label"), row("Slanted edge", "edge", "edge_lab", "edge_label")]),
            ],
        ),
        _ => return None,
    };
    Some(MeasureCard { title: "Shape", help, groups })
}

/// Worked-out values of the solids (the grey text in empty boxes and on linked rows).
fn computed_solid(id: &str, p: &Params) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let c = |v: f64| ctext(p, v);
    if matches!(id, "sphere" | "cylinder" | "cone") { out.push(("diameter", c(2.0 * solid_r(p)))); }
    let oblique = p.s("stands") == "Oblique";
    match id {
        "cylinder" => {
            let h = edge_height(p, oblique);
            if typed(p, "edge").is_none() { out.push(("edge", c((h * h + p.f("lean").powi(2)).sqrt()))); } else { out.push(("height", c(h))); }
        }
        "cone" => {
            let (r, h) = (solid_r(p), cone_height(p));
            let lean = if oblique { p.f("lean") } else { 0.0 };
            if typed(p, "slant").is_none() { out.push(("slant", c(((r - lean).powi(2) + h * h).sqrt()))); } else { out.push(("height", c(h))); }
        }
        "pyramid" => {
            let a = pyramid_apothem(p);
            let h = pyramid_height(p);
            let poly = poly_base(p.s("base"), p.f("length"), p.f("width"), p.i("n").clamp(3, 12) as usize, p.f("side"));
            out.push(("apothem", c(dist3(mid3(poly[0], poly[1]), centroid3(&poly)))));
            let lean = if oblique { p.f("lean") } else { 0.0 };
            if typed(p, "slant").is_none() { out.push(("slant", c((a * a + h * h + lean * lean).sqrt()))); } else { out.push(("height", c(h))); }
        }
        "prism" => {
            let (l, w) = (p.f("length"), p.f("width"));
            let hyp = if p.s("base") == "Isosceles triangle" { ((l / 2.0).powi(2) + w * w).sqrt() } else { (l * l + w * w).sqrt() };
            out.push(("hyp", c(hyp)));
            let poly = poly_base("Regular polygon", l, w, p.i("n").clamp(3, 12) as usize, p.f("side"));
            out.push(("apothem", c(dist3(mid3(poly[0], poly[1]), centroid3(&poly)))));
            let h = edge_height(p, oblique);
            if typed(p, "edge").is_none() { out.push(("edge", c((h * h + p.f("lean").powi(2)).sqrt()))); } else { out.push(("height", c(h))); }
        }
        _ => {}
    }
    if let Some(m) = solid_msg(id, p) { out.push(("__msg", m)); }
    out
}

/// The reference's red line under a solid's measures when the typed ones can't all hold.
pub(super) fn solid_msg(id: &str, p: &Params) -> Option<String> {
    let oblique = p.s("stands") == "Oblique";
    let bad = |k: &str| !p.s(k).trim().is_empty() && typed(p, k).is_none();
    match id {
        "cylinder" | "prism" => {
            let lying = id == "prism" && p.s("base") != "Rectangle" && p.s("sits") == "Lying on its side";
            if !oblique || lying { return None; }
            if bad("edge") { return Some("Type a number for the slanted edge, like 12, 2.5, 5/2 or 3\u{221a}2.".into()); }
            let e = typed(p, "edge")?;
            (e <= p.f("lean").abs() + 1e-6).then(|| "The height, lean and slanted edge don\u{2019}t fit together: the slanted edge has to be longer than the lean. Clear one to have it worked out.".into())
        }
        "cone" | "pyramid" => {
            if bad("slant") { return Some("Type a number for the slant height, like 12, 2.5, 5/2 or 3\u{221a}2.".into()); }
            let s = typed(p, "slant")?;
            if oblique {
                let what = if id == "cone" { "cone" } else { "pyramid" };
                return Some(format!("An oblique {what}\u{2019}s slant height is worked out from its height and lean. Clear the slant height to have it worked out."));
            }
            let (min, what) = if id == "cone" { (solid_r(p), "radius") } else if p.s("base") == "Regular polygon" { (pyramid_apothem(p), "apothem") } else { (pyramid_apothem(p), "half the width") };
            (s <= min + 1e-6).then(|| format!("The height and slant height don\u{2019}t fit together: the slant height has to be longer than the {what}. Clear one to have it worked out."))
        }
        _ => None,
    }
}

/// Old specs: the `hide` list and `measure` choice become per-measure label modes.
fn migrate_solid(id: &str, spec: &crate::model::FigureSpec, p: &mut Params) {
    let old = |k: &str| spec.params.get(k);
    let olds = |k: &str| old(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let oldb = |k: &str| old(k).and_then(|v| v.as_bool());
    let oldf = |k: &str| old(k).and_then(|v| v.as_f64());
    // hide list -> label None
    for t in tokens(&olds("hide")) {
        let key = match t.to_lowercase().as_str() {
            "length" => "length_lab", "width" => "width_lab", "height" => if matches!(id, "cylinder" | "cone") { "h_lab" } else { "height_lab" },
            "side" => "side_lab", "slant" => if id == "cone" { "l_lab" } else { "slant_lab" }, "radius" => "r_lab",
            "hypotenuse" | "slanted" => "hyp_lab", "apothem" => "apothem_lab", "lean" => "lean_lab", "edge" => "edge_lab",
            _ => continue,
        };
        p.set_s(key, "None");
    }
    // radius / diameter
    if olds("measure") == "Diameter" {
        let (m, t) = (p.s("r_lab").to_string(), p.s("r_label").to_string());
        p.set_s("d_lab", &m);
        p.set_s("d_label", &t);
        p.set_s("r_lab", "None");
    }
    // the slant height was given instead of the height
    if matches!(id, "cone" | "pyramid") {
        let given = olds("given") == "Slant height";
        let sl = oldf("slant");
        if old("given").is_some() || sl.is_some() {
            p.set_s("slant", &if given { sl.map(|v| fmt(v, 3)).unwrap_or_default() } else { String::new() });
        }
        let lab = if id == "cone" { "l_lab" } else { "slant_lab" };
        if oldb("show_slant") == Some(false) { p.set_s(lab, "None"); }
        if id == "cone" && oldb("show_slant") == Some(true) && old("l_lab").is_none() { p.set_s(lab, "Measure"); }
    }
    // lean labels did not exist / defaulted to None before the rework
    if old("stands").is_some() && old("lean_lab").is_none() && id != "sphere" { p.set_s("lean_lab", "None"); }
}

/// Prism vertices: (base ring then top ring, ring size, lying on its side, lean actually used).
/// Lying: the base polygon is the front face (x-z plane) and `height` is the depth.
pub(super) fn prism_geom(p: &Params) -> (Vec<P3>, usize, bool, f64) {
    let kind = p.s("base");
    let base = poly_base(kind, p.f("length"), p.f("width"), p.i("n").clamp(3, 12) as usize, p.f("side"));
    let k = base.len();
    let lying = kind != "Rectangle" && p.s("sits") == "Lying on its side";
    let oblique = !lying && p.s("stands") == "Oblique";
    let hh = edge_height(p, oblique);
    let lean = if oblique { p.f("lean") } else { 0.0 };
    let mut v: Vec<P3>;
    if lying {
        let vmin = base.iter().map(|q| q.1).fold(f64::MAX, f64::min);
        // right triangle: put the right angle on the side away from the depth so the hypotenuse
        // face does not fold onto the back edges (b1 stays the right-angle corner)
        let flip = kind == "Right triangle" && depth_sx(p) > 0.0;
        let fx = |x: f64| if flip { p.f("length") - x } else { x };
        v = base.iter().map(|q| (fx(q.0), 0.0, q.1 - vmin)).collect();
        v.extend(base.iter().map(|q| (fx(q.0), hh, q.1 - vmin)));
    } else {
        v = base.clone();
        v.extend(base.iter().map(|q| (q.0 + lean, q.1, hh)));
    }
    (v, k, lying, lean)
}

fn prism_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("base", "Base", &["Rectangle", "Right triangle", "Isosceles triangle", "Regular polygon"], "Rectangle"),
        ParamDef::choice("stands", "Stands", &["Right", "Oblique"], "Right"),
        ParamDef::float("lean", "Lean of the top when oblique", -20.0, 20.0, 0.5, 2.0),
        ParamDef::float("length", "Length", 0.5, 50.0, 0.5, 5.0),
        ParamDef::float("width", "Width (depth, or triangle height)", 0.5, 50.0, 0.5, 3.0),
        ParamDef::int("n", "Sides of regular-polygon base", 3, 12, 5),
        ParamDef::float("side", "Side of regular-polygon base", 0.5, 50.0, 0.5, 3.0),
        ParamDef::float("height", "Height", 0.5, 50.0, 0.5, 4.0),
    ];
    v.extend(solid_common("cm"));
    v.extend([
        ParamDef::choice("sits", "Sits (triangle or polygon base)", &["Standing on its base", "Lying on its side"], "Standing on its base"),
        ParamDef::choice("depth", "Position: depth goes back to the", &["Right", "Left"], "Right"),
        ParamDef::bool("names", "Name the corners around the base, then around the top", false),
        label_size_param(),
        ParamDef::choice("length_lab", "Length (triangle base) label", &LAB_OPTS, "Measure"),
        ParamDef::text("length_label", "Length label text", "l"),
        ParamDef::choice("width_lab", "Width (triangle height) label", &LAB_OPTS, "Measure"),
        ParamDef::text("width_label", "Width label text", "w"),
        ParamDef::choice("height_lab", "Height label", &LAB_OPTS, "Measure"),
        ParamDef::text("height_label", "Height label text", "h"),
        ParamDef::choice("side_lab", "Polygon side label", &LAB_OPTS, "Measure"),
        ParamDef::text("side_label", "Polygon side label text", "s"),
        ParamDef::choice("hyp_lab", "Hypotenuse / slanted side label (worked out)", &LAB_OPTS, "None"),
        ParamDef::text("hyp_label", "Hypotenuse label text", "x"),
        ParamDef::choice("apothem_lab", "Apothem label (worked out)", &LAB_OPTS, "None"),
        ParamDef::text("apothem_label", "Apothem label text", "a"),
        ParamDef::choice("lean_lab", "Lean label (oblique)", &LAB_OPTS, "Measure"),
        ParamDef::text("lean_label", "Lean label text", "d"),
        ParamDef::text("edge", "Slanted edge (oblique; empty = worked out, typed = the height is worked out)", ""),
        ParamDef::choice("edge_lab", "Slanted edge label (oblique)", &LAB_OPTS, "None"),
        ParamDef::text("edge_label", "Slanted edge label text", "e"),
    ]);
    v
}

fn prism_render(p: &Params) -> String {
    let (l, wd) = (p.f("length"), p.f("width"));
    let kind = p.s("base");
    let sx = depth_sx(p);
    let (v, k, lying, lean) = prism_geom(p);
    let hh = edge_height(p, !lying && p.s("stands") == "Oblique");
    let mut faces = vec![(0..k).collect::<Vec<_>>(), (k..2 * k).collect()];
    for i in 0..k { faces.push(vec![i, (i + 1) % k, k + (i + 1) % k, k + i]); }
    let solid = Solid { v, faces };
    let proj = |q: P3| oblp(q, sx);
    let b: Vec<P3> = solid.v[..k].to_vec();
    let t: Vec<P3> = solid.v[k..].to_vec();
    let mut all: Vec<Pt> = solid.v.iter().map(|q| proj(*q)).collect();
    let cenp = (all.iter().map(|q| q.0).sum::<f64>() / all.len() as f64, all.iter().map(|q| q.1).sum::<f64>() / all.len() as f64);
    // oblique: height dropped from the top of the front edge's leading corner to the extended base line
    let e = if lean >= 0.0 { b[1] } else { b[0] };
    let foot = (e.0 + lean, e.1, 0.0);
    if lean.abs() > 1e-9 { all.push(proj(foot)); }
    let m = Map::fit(&all, 480.0, 360.0, 70.0, 50.0);
    let mut d = new_doc(480.0, 360.0);
    draw_solid(&mut d, &m, &solid, &proj, oblv(sx), p.b("hidden"));
    let fs = lsz(p);
    let hide = hide_list(p);
    let hid = |n: &str| hide.iter().any(|h| h == n);
    let mc = m.p(cenp);
    let lab = |d: &mut Doc, id: &str, a: P3, bb: P3, s: Option<String>| if let Some(s) = s { sbeside(d, id, m.p(proj(a)), m.p(proj(bb)), 0.5, mc, &s, fs); };
    // labels inside the base face sit away from the base's own centre
    let mb = m.p(proj(centroid3(&b)));
    let labb = |d: &mut Doc, id: &str, a: P3, bb: P3, t: f64, s: Option<String>| if let Some(s) = s { sbeside(d, id, m.p(proj(a)), m.p(proj(bb)), t, mb, &s, fs); };
    let right = p.b("right");
    let poly = kind == "Regular polygon";
    // base edge (front bottom) b0-b1
    if poly { lab(&mut d, "side", b[0], b[1], mtext(p, "side", "Measure", p.f("side"), hid("side"))); }
    else { lab(&mut d, "length", b[0], b[1], mtext(p, "length", "Measure", l, hid("length"))); }
    // the base face's own measures: on the bottom face when standing, the front face when lying
    let wtxt = mtext(p, "width", "Measure", wd, hid("width"));
    match kind {
        "Rectangle" => lab(&mut d, "width", b[1], b[2], wtxt),
        "Right triangle" => {
            labb(&mut d, "width", b[1], b[2], 0.5, wtxt);
            if right { rmark3(&mut d, &m, &proj, b[1], sub3(b[0], b[1]), sub3(b[2], b[1])); }
            labb(&mut d, "hyp", b[0], b[2], 0.5, mtext(p, "hyp", "None", dist3(b[0], b[2]), hid("hypotenuse")));
        }
        "Isosceles triangle" => {
            if let Some(s) = wtxt {
                let f0 = mid3(b[0], b[1]);
                dline(&mut d, m.p(proj(f0)), m.p(proj(b[2])), "#444", 1.4, "6 4");
                labb(&mut d, "width", f0, b[2], 0.6, Some(s));
                if right { rmark3(&mut d, &m, &proj, f0, sub3(b[1], f0), sub3(b[2], f0)); }
            }
            labb(&mut d, "hyp", b[1], b[2], 0.5, mtext(p, "hyp", "None", dist3(b[1], b[2]), hid("slanted")));
        }
        _ => {
            if let Some(s) = mtext(p, "apothem", "None", dist3(mid3(b[0], b[1]), centroid3(&b)), hid("apothem")) {
                let (c0, f0) = (centroid3(&b), mid3(b[0], b[1]));
                dline(&mut d, m.p(proj(c0)), m.p(proj(f0)), "#444", 1.4, "6 4");
                d.circle(m.p(proj(c0)).0, m.p(proj(c0)).1, 2.4, INK, INK, 1.0);
                labb(&mut d, "apothem", c0, f0, 0.4, Some(s));
                if right { rmark3(&mut d, &m, &proj, f0, sub3(b[1], f0), sub3(c0, f0)); }
            }
        }
    }
    let htxt = mtext(p, "height", "Measure", hh, hid("height"));
    if lying {
        // prism length runs back along the depth from the front corner nearest the depth side
        let vi = (0..k).filter(|i| b[*i].2 < 1e-9).max_by(|x, y| (sx * b[*x].0).partial_cmp(&(sx * b[*y].0)).unwrap()).unwrap_or(1);
        lab(&mut d, "height", b[vi], t[vi], htxt);
    } else if lean.abs() < 1e-9 {
        // vertical edge at the front corner away from the depth side
        let ymin = b.iter().map(|q| q.1).fold(f64::MAX, f64::min);
        let vi = (0..k).filter(|i| b[*i].1 < ymin + 1e-9).min_by(|x, y| (sx * b[*x].0).partial_cmp(&(sx * b[*y].0)).unwrap()).unwrap_or(0);
        lab(&mut d, "height", b[vi], t[vi], htxt);
    } else {
        let top = (foot.0, foot.1, hh);
        dline(&mut d, m.p(proj(top)), m.p(proj(foot)), "#444", 1.4, "6 4");
        let outside = (lean > 0.0 && foot.0 > e.0) || (lean < 0.0 && foot.0 < e.0);
        if outside { dline(&mut d, m.p(proj(e)), m.p(proj(foot)), "#444", 1.4, "6 4"); }
        if htxt.is_some() || right {
            if right { rmark3(&mut d, &m, &proj, foot, sub3(e, foot), (0.0, 0.0, 1.0)); }
        }
        if let Some(s) = htxt { sbeside(&mut d, "height", m.p(proj(foot)), m.p(proj(top)), 0.7, mc, &s, fs); }
        lab(&mut d, "lean", e, foot, mtext(p, "lean", "None", lean.abs(), hid("lean")));
        let et = (e.0 + lean, e.1, hh);
        if let Some(s) = tlabel(p, "edge", "edge", "None", dist3(e, et)) { sbeside(&mut d, "edge", m.p(proj(e)), m.p(proj(et)), 0.3, mc, &s, fs); }
    }
    if p.b("names") {
        let fsn = fs * 0.95;
        for (i, q) in solid.v.iter().enumerate() { name_at(&mut d, &format!("v{i}"), m.p(proj(*q)), mc, &corner_name(i), fsn); }
    }
    d.finish()
}
fn centroid3(v: &[P3]) -> P3 {
    let n = v.len().max(1) as f64;
    (v.iter().map(|q| q.0).sum::<f64>() / n, v.iter().map(|q| q.1).sum::<f64>() / n, v.iter().map(|q| q.2).sum::<f64>() / n)
}

/// Pyramid height actually drawn: the typed height, or worked out from the slant height.
pub(super) fn pyramid_height(p: &Params) -> f64 {
    if p.s("stands") != "Oblique" && let Some(s) = typed(p, "slant") {
        let a = pyramid_apothem(p);
        if s > a + 1e-6 { return (s * s - a * a).sqrt(); }
    }
    p.f("height")
}
/// Distance from the base's centre to the middle of the front base edge (where the slant height
/// meets the base): half the width for a rectangle, the apothem for a regular polygon.
pub(super) fn pyramid_apothem(p: &Params) -> f64 {
    let b = poly_base(p.s("base"), p.f("length"), p.f("width"), p.i("n").clamp(3, 12) as usize, p.f("side"));
    dist3(mid3(b[0], b[1]), centroid3(&b))
}

fn pyramid_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("base", "Base", &["Rectangle", "Regular polygon"], "Rectangle"),
        ParamDef::choice("stands", "Stands", &["Right", "Oblique"], "Right"),
        ParamDef::float("lean", "Lean of the apex when oblique", -20.0, 20.0, 0.5, 2.0),
        ParamDef::float("length", "Length", 0.5, 50.0, 0.5, 6.0),
        ParamDef::float("width", "Width", 0.5, 50.0, 0.5, 6.0),
        ParamDef::int("n", "Sides of regular-polygon base", 3, 12, 5),
        ParamDef::float("side", "Side of regular-polygon base", 0.5, 50.0, 0.5, 4.0),
        ParamDef::float("height", "Height", 0.5, 50.0, 0.5, 4.0),
        ParamDef::bool("show_height", "Draw the height", true),
    ];
    v.extend(solid_common("cm"));
    v.extend([
        ParamDef::text("slant", "Slant height (empty = worked out; typed = the height is worked out)", ""),
        ParamDef::choice("depth", "Position: depth goes back to the", &["Right", "Left"], "Right"),
        ParamDef::bool("names", "Name the corners around the base, then the tip", false),
        label_size_param(),
        ParamDef::choice("length_lab", "Length label", &LAB_OPTS, "Measure"),
        ParamDef::text("length_label", "Length label text", "l"),
        ParamDef::choice("width_lab", "Width label", &LAB_OPTS, "Measure"),
        ParamDef::text("width_label", "Width label text", "w"),
        ParamDef::choice("side_lab", "Polygon side label", &LAB_OPTS, "Measure"),
        ParamDef::text("side_label", "Polygon side label text", "s"),
        ParamDef::choice("height_lab", "Height label", &LAB_OPTS, "Measure"),
        ParamDef::text("height_label", "Height label text", "h"),
        ParamDef::choice("slant_lab", "Slant height label (drawn apex to the middle of the front edge)", &LAB_OPTS, "None"),
        ParamDef::text("slant_label", "Slant height label text", "l"),
        ParamDef::choice("apothem_lab", "Apothem label (worked out)", &LAB_OPTS, "None"),
        ParamDef::text("apothem_label", "Apothem label text", "a"),
        ParamDef::choice("lean_lab", "Lean label (oblique)", &LAB_OPTS, "Measure"),
        ParamDef::text("lean_label", "Lean label text", "d"),
    ]);
    v
}

fn pyramid_render(p: &Params) -> String {
    let (l, wd) = (p.f("length"), p.f("width"));
    let hh = pyramid_height(p);
    let kind = p.s("base");
    let sx = depth_sx(p);
    let base = poly_base(kind, l, wd, p.i("n").clamp(3, 12) as usize, p.f("side"));
    let k = base.len();
    let c3 = centroid3(&base);
    let lean = if p.s("stands") == "Oblique" { p.f("lean") } else { 0.0 };
    let apex = (c3.0 + lean, c3.1, hh);
    let mut v = base.clone();
    v.push(apex);
    let mut faces = vec![(0..k).collect::<Vec<_>>()];
    for i in 0..k { faces.push(vec![i, (i + 1) % k, k]); }
    let solid = Solid { v, faces };
    let proj = |q: P3| oblp(q, sx);
    let all: Vec<Pt> = solid.v.iter().map(|q| proj(*q)).collect();
    let cenp = (all.iter().map(|q| q.0).sum::<f64>() / all.len() as f64, all.iter().map(|q| q.1).sum::<f64>() / all.len() as f64);
    let m = Map::fit(&all, 480.0, 360.0, 70.0, 50.0);
    let mut d = new_doc(480.0, 360.0);
    draw_solid(&mut d, &m, &solid, &proj, oblv(sx), p.b("hidden"));
    let fs = lsz(p);
    let hide = hide_list(p);
    let hid = |n: &str| hide.iter().any(|h| h == n);
    let mc = m.p(cenp);
    let lab = |d: &mut Doc, id: &str, a: P3, bb: P3, s: Option<String>| if let Some(s) = s { sbeside(d, id, m.p(proj(a)), m.p(proj(bb)), 0.5, mc, &s, fs); };
    let right = p.b("right");
    let foot = (c3.0, c3.1, 0.0);
    let hfoot = (apex.0, apex.1, 0.0);
    if p.b("show_height") {
        dline(&mut d, m.p(proj(apex)), m.p(proj(hfoot)), "#444", 1.5, "6 4");
        if lean.abs() > 1e-9 { dline(&mut d, m.p(proj(foot)), m.p(proj(hfoot)), "#444", 1.5, "6 4"); }
        lab(&mut d, "height", hfoot, apex, mtext(p, "height", "Measure", hh, hid("height")));
        if right { rmark3(&mut d, &m, &proj, hfoot, (-sx, 0.0, 0.0), (0.0, 0.0, 1.0)); }
    }
    let mid = mid3(base[0], base[1]);
    if let Some(s) = tlabel(p, "slant", "slant", "None", dist3(mid, apex)).filter(|_| !hid("slant")) {
        dline(&mut d, m.p(proj(mid)), m.p(proj(apex)), "#444", 1.5, "6 4");
        let (pa, pm) = (m.p(proj(apex)), m.p(proj(mid)));
        sbeside(&mut d, "slant", pm, pa, 0.3, mc, &s, fs);
        if right && lean.abs() < 1e-9 { rmark3(&mut d, &m, &proj, mid, sub3(base[1], mid), sub3(apex, mid)); }
    }
    if lean.abs() > 1e-9 && let Some(s) = mtext(p, "lean", "Measure", lean.abs(), false) {
        // lean: from the base centre to the foot of the height, along the base
        lab(&mut d, "lean", foot, hfoot, Some(s));
    }
    if let Some(s) = mtext(p, "apothem", "None", dist3(mid, foot), hid("apothem")) {
        dline(&mut d, m.p(proj(foot)), m.p(proj(mid)), "#444", 1.4, "6 4");
        lab(&mut d, "apothem", foot, mid, Some(s));
        if right { rmark3(&mut d, &m, &proj, mid, sub3(base[1], mid), sub3(foot, mid)); }
    }
    if kind == "Regular polygon" {
        lab(&mut d, "side", base[0], base[1], mtext(p, "side", "Measure", p.f("side"), hid("side")));
    } else {
        lab(&mut d, "length", base[0], base[1], mtext(p, "length", "Measure", l, hid("length")));
        let (w0, w1) = if sx > 0.0 { (base[1], base[2]) } else { (base[0], base[3]) };
        lab(&mut d, "width", w0, w1, mtext(p, "width", "Measure", wd, hid("width")));
    }
    if p.b("names") {
        for (i, q) in solid.v.iter().enumerate() { name_at(&mut d, &format!("v{i}"), m.p(proj(*q)), mc, &corner_name(i), fs * 0.95); }
    }
    d.finish()
}

// ----- round solids (elevated orthographic view) -----

pub(super) fn ell(cx: f64, cy: f64, rx: f64, ry: f64, t0: f64, t1: f64) -> Vec<Pt> {
    let n = 64;
    (0..=n).map(|i| { let t = (t0 + (t1 - t0) * i as f64 / n as f64).to_radians(); (cx + rx * t.cos(), cy + ry * t.sin()) }).collect()
}
fn mpts(m: &Map, pts: &[Pt]) -> Vec<Pt> { pts.iter().map(|q| m.p(*q)).collect() }

fn round_common() -> Vec<ParamDef> {
    vec![
        ParamDef::float("view", "View elevation (degrees above the horizon)", 8.0, 60.0, 1.0, 22.0),
        ParamDef::bool("hidden", "Show hidden edges (dashed)", true),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::text("unit", "Unit", "cm"),
        ParamDef::int("dec", "Decimal places", 0, 3, 1),
        // the reference's "Give the diameter instead": the Radius row becomes a Diameter row and the
        // diameter is drawn right across the circle with the radius row's label
        ParamDef::bool("give_d", "Give the diameter instead (drawn right across the circle)", false),
        ParamDef::float("diam", "Diameter", 1.0, 100.0, 0.5, 6.0),
    ]
}
/// Radius of a round solid: the Radius row, or half the Diameter row with "Give the diameter instead".
pub(super) fn solid_r(p: &Params) -> f64 { if p.b("give_d") { p.f("diam") / 2.0 } else { p.f("radius") } }
/// Writes a dragged radius into whichever row is given.
pub(super) fn set_solid_r(p: &mut Params, r: f64) { if p.b("give_d") { p.set_f("diam", 2.0 * r) } else { p.set_f("radius", r) } }
/// The diameter's label: with "Give the diameter instead" it takes the radius row's label mode
/// (Measure by default) and the diameter text; otherwise its own linked row's label.
fn diam_text(p: &Params, v: f64) -> Option<String> {
    if !p.b("give_d") { return mtext(p, "d", "None", v, false); }
    match match p.s("r_lab") { "" => "Measure", m => m } {
        "None" => None,
        "Text" => { let t = p.s("d_label").trim(); Some(if t.is_empty() { "d".to_string() } else { t.to_string() }) }
        _ => Some(fmtu(v, p.i("dec"), p.s("unit"))),
    }
}
fn round_label_params() -> [ParamDef; 5] {
    let [d0, d1] = diameter_params();
    [
        label_size_param(),
        ParamDef::choice("r_lab", "Radius label", &LAB_OPTS, "Measure"),
        ParamDef::text("r_label", "Radius label text", "r"),
        d0,
        d1,
    ]
}
/// The diameter line is drawn (its label is shown) instead of / as well as the radius.
fn diam_shown(p: &Params) -> bool { diam_text(p, 1.0).is_some() }

/// Radius (or diameter) across the circle centred at model point o. `solid`: drawn as a solid
/// line (sphere); otherwise dashed. `above`: label above the line.
fn base_measure(d: &mut Doc, m: &Map, o: Pt, r: f64, p: &Params, solid: bool, above: bool) {
    let fs = lsz(p);
    let c = m.p(o);
    let dash = if solid { "none" } else { "6 4" };
    let rt = if p.b("give_d") { None } else { mtext(p, "r", "Measure", r, false) };
    let dt = diam_text(p, 2.0 * r);
    // nothing is drawn across the circle when neither label prints (as the reference)
    if rt.is_none() && dt.is_none() { return; }
    let diam = dt.is_some();
    let rb = m.p((o.0 + r, o.1));
    let (a, b) = if diam { (m.p((o.0 - r, o.1)), rb) } else { (c, rb) };
    if solid { d.line(a.0, a.1, b.0, b.1, INK, 2.0); } else { dline(d, a, b, "#444", 1.5, dash); }
    let off = if above { -fs * 0.85 } else { fs * 0.85 };
    if let Some(s) = dt { sput(d, "d", (a.0 + b.0) / 2.0, c.1 + off, &s, fs); }
    if let Some(s) = rt {
        // with the diameter also labelled, the radius sits on the other side of the line
        let y = if diam { c.1 - off } else { c.1 + off };
        sput(d, "r", (c.0 + rb.0) / 2.0, y, &s, fs);
        d.circle(c.0, c.1, 2.8, INK, INK, 1.0);
    }
}

fn cylinder_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("stands", "Stands", &["Right", "Oblique"], "Right"),
        ParamDef::float("lean", "Lean of the top when oblique", -20.0, 20.0, 0.5, 2.0),
        ParamDef::float("radius", "Radius", 0.5, 50.0, 0.5, 3.0),
        ParamDef::float("height", "Height", 0.5, 50.0, 0.5, 4.0),
        ParamDef::bool("show_height", "Draw the height", true),
    ];
    v.extend(round_common());
    v.extend(round_label_params());
    v.extend([
        ParamDef::choice("h_lab", "Height label", &LAB_OPTS, "Measure"),
        ParamDef::text("h_label", "Height label text", "h"),
        ParamDef::choice("lean_lab", "Lean label (oblique)", &LAB_OPTS, "Measure"),
        ParamDef::text("lean_label", "Lean label text", "d"),
        ParamDef::text("edge", "Slanted edge (oblique; empty = worked out, typed = the height is worked out)", ""),
        ParamDef::choice("edge_lab", "Slanted edge label (oblique)", &LAB_OPTS, "None"),
        ParamDef::text("edge_label", "Slanted edge label text", "e"),
    ]);
    v
}

fn cylinder_render(p: &Params) -> String {
    let oblique = p.s("stands") == "Oblique";
    let (r, h) = (solid_r(p), edge_height(p, oblique));
    let (s, c) = (p.f("view").to_radians().sin(), p.f("view").to_radians().cos());
    let lean = if oblique { p.f("lean") } else { 0.0 };
    let ht = h * c;
    let mut all = ell(0.0, 0.0, r, r * s, 0.0, 360.0);
    all.extend(ell(lean, ht, r, r * s, 0.0, 360.0));
    let m = Map::fit(&all, 480.0, 360.0, 70.0, 48.0);
    let mut d = new_doc(480.0, 360.0);
    let fs = lsz(p);
    let mut body = ell(0.0, 0.0, r, r * s, 180.0, 360.0);
    body.extend(ell(lean, ht, r, r * s, 360.0, 180.0));
    d.poly(&mpts(&m, &body), "#d9e7f6", "none", 0.0);
    d.poly(&mpts(&m, &ell(lean, ht, r, r * s, 0.0, 360.0)), "#f4f9ff", INK, 2.2);
    let (l0, l1, r0, r1) = (m.p((-r, 0.0)), m.p((lean - r, ht)), m.p((r, 0.0)), m.p((lean + r, ht)));
    d.line(l0.0, l0.1, l1.0, l1.1, INK, 2.2);
    d.line(r0.0, r0.1, r1.0, r1.1, INK, 2.2);
    d.polyline(&mpts(&m, &ell(0.0, 0.0, r, r * s, 180.0, 360.0)), INK, 2.2);
    if p.b("hidden") { dpoly(&mut d, &mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 180.0)), "#555", 1.4, "6 4"); }
    base_measure(&mut d, &m, (0.0, 0.0), r, p, false, false);
    let htxt = mtext(p, "h", "Measure", h, false);
    let center = m.p((lean / 2.0, ht / 2.0));
    if p.b("show_height") {
        let (t, b) = (m.p((lean, ht)), m.p((lean, 0.0)));
        dline(&mut d, t, b, "#444", 1.5, "6 4");
        d.circle(t.0, t.1, 2.8, INK, INK, 1.0);
        if let Some(txt) = htxt { sput(&mut d, "h", t.0 - 10.0 - stw(&txt, fs) / 2.0, (t.1 + b.1) / 2.0, &txt, fs); }
        if lean.abs() > 1e-9 { dline(&mut d, m.p((0.0, 0.0)), b, "#444", 1.5, "6 4"); }
        if p.b("right") && !diam_shown(p) { square_mark(&mut d, b, (b.0, b.1 - 10.0), (b.0 + 10.0, b.1), 10.0); }
    } else if let Some(txt) = htxt {
        sbeside(&mut d, "h", r0, r1, 0.5, center, &txt, fs);
    }
    if lean.abs() > 1e-9 {
        if let Some(txt) = mtext(p, "lean", "None", lean.abs(), false) {
            let (a, b) = (m.p((0.0, 0.0)), m.p((lean, 0.0)));
            sput(&mut d, "lean", (a.0 + b.0) / 2.0, a.1 - fs * 0.85, &txt, fs);
        }
        let side = if lean > 0.0 { (r0, r1) } else { (l0, l1) };
        if let Some(txt) = tlabel(p, "edge", "edge", "None", (h * h + lean * lean).sqrt()) { sbeside(&mut d, "edge", side.0, side.1, 0.5, center, &txt, fs); }
    }
    d.finish()
}

/// Cone height actually drawn: typed, or worked out from a given slant height (right cones).
pub(super) fn cone_height(p: &Params) -> f64 {
    if p.s("stands") != "Oblique" && let Some(s) = typed(p, "slant") {
        let r = solid_r(p);
        if s > r + 1e-6 { return (s * s - r * r).sqrt(); }
    }
    p.f("height")
}

fn cone_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("stands", "Stands", &["Right", "Oblique"], "Right"),
        ParamDef::float("lean", "Lean of the apex when oblique", -20.0, 20.0, 0.5, 2.0),
        ParamDef::float("radius", "Radius", 0.5, 50.0, 0.5, 3.0),
        ParamDef::float("height", "Height", 0.5, 50.0, 0.5, 4.0),
        ParamDef::bool("show_height", "Draw the height", true),
    ];
    v.extend(round_common());
    v.extend(round_label_params());
    v.extend([
        ParamDef::text("slant", "Slant height (empty = worked out; typed = the height is worked out)", ""),
        ParamDef::choice("h_lab", "Height label", &LAB_OPTS, "Measure"),
        ParamDef::text("h_label", "Height label text", "h"),
        ParamDef::choice("l_lab", "Slant height label", &LAB_OPTS, "None"),
        ParamDef::text("l_label", "Slant height label text", "l"),
        ParamDef::choice("lean_lab", "Lean label (oblique)", &LAB_OPTS, "Measure"),
        ParamDef::text("lean_label", "Lean label text", "d"),
    ]);
    v
}

fn cone_render(p: &Params) -> String {
    let r = solid_r(p);
    let h = cone_height(p);
    let (s, c) = (p.f("view").to_radians().sin(), p.f("view").to_radians().cos());
    let lean = if p.s("stands") == "Oblique" { p.f("lean") } else { 0.0 };
    let apex = (lean, h * c);
    // tangents from apex to base ellipse
    let pp = (apex.0, apex.1 / s);
    let dd = (pp.0 * pp.0 + pp.1 * pp.1).sqrt().max(r * 1.001);
    let (phi, del) = (pp.1.atan2(pp.0), (r / dd).acos());
    let tp = |a: f64| (r * a.cos(), r * s * a.sin());
    let (t1, t2) = (phi + del, phi - del);
    let (mut tl, mut tr) = (t1, t2);
    if tp(tl).0 > tp(tr).0 { std::mem::swap(&mut tl, &mut tr); }
    let (pl, pr) = (tp(tl), tp(tr));
    let mut all = ell(0.0, 0.0, r, r * s, 0.0, 360.0);
    all.push(apex);
    let m = Map::fit(&all, 480.0, 360.0, 70.0, 48.0);
    let mut d = new_doc(480.0, 360.0);
    let fs = lsz(p);
    let (a0, a1) = (tl.to_degrees().rem_euclid(360.0), tr.to_degrees().rem_euclid(360.0));
    let (front0, front1) = (a0, if a1 < a0 { a1 + 360.0 } else { a1 });
    let front = ell(0.0, 0.0, r, r * s, front0, front1);
    let mut body = vec![apex, pl];
    body.extend(front.iter().copied());
    body.push(pr);
    d.poly(&mpts(&m, &body), "#d9e7f6", "none", 0.0);
    d.line(m.p(apex).0, m.p(apex).1, m.p(pl).0, m.p(pl).1, INK, 2.2);
    d.line(m.p(apex).0, m.p(apex).1, m.p(pr).0, m.p(pr).1, INK, 2.2);
    d.polyline(&mpts(&m, &front), INK, 2.2);
    if p.b("hidden") {
        let back = ell(0.0, 0.0, r, r * s, front1, front0 + 360.0);
        dpoly(&mut d, &mpts(&m, &back), "#555", 1.4, "6 4");
    }
    base_measure(&mut d, &m, (0.0, 0.0), r, p, false, false);
    if p.b("show_height") {
        let (t, b) = (m.p(apex), m.p((lean, 0.0)));
        dline(&mut d, t, b, "#444", 1.5, "6 4");
        if let Some(txt) = mtext(p, "h", "Measure", h, false) { sput(&mut d, "h", t.0 - 10.0 - stw(&txt, fs) / 2.0, (t.1 + b.1) / 2.0 - 8.0, &txt, fs); }
        if lean.abs() > 1e-9 { dline(&mut d, m.p((0.0, 0.0)), b, "#444", 1.5, "6 4"); }
        if p.b("right") && !diam_shown(p) && lean.abs() < 1e-9 { square_mark(&mut d, b, (b.0, b.1 - 10.0), (b.0 + 10.0, b.1), 10.0); }
        if lean.abs() > 1e-9 {
            if p.b("right") { square_mark(&mut d, b, (b.0, b.1 - 10.0), (b.0 - lean.signum() * 10.0, b.1), 10.0); }
            if let Some(txt) = mtext(p, "lean", "None", lean.abs(), false) {
                let a = m.p((0.0, 0.0));
                sput(&mut d, "lean", (a.0 + b.0) / 2.0, a.1 - fs * 0.8, &txt, fs);
            }
        }
    }
    let sl = ((r - lean).powi(2) + h * h).sqrt();
    // an oblique cone has no slant height to give (its slant edge is always worked out)
    let ltxt = if lean.abs() > 1e-9 { mtext(p, "l", "None", sl, false) } else { tlabel(p, "l", "slant", "None", sl) };
    if let Some(txt) = ltxt {
        let (a, b) = (m.p(apex), m.p(pr));
        sbeside(&mut d, "l", a, b, 0.5, m.p((lean * 0.5, h * c * 0.4)), &txt, fs);
    }
    d.finish()
}

fn sphere_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("draw", "Draw", &["A whole sphere", "A hemisphere"], "A whole sphere"),
        ParamDef::float("radius", "Radius", 0.5, 50.0, 0.5, 3.0),
    ];
    v.extend(round_common());
    v.extend(round_label_params());
    v.push(ParamDef::choice("face", "Position (hemisphere): flat face", &["Down, like a dome", "Up, like a bowl"], "Down, like a dome"));
    v
}

fn sphere_render(p: &Params) -> String {
    let r = solid_r(p);
    let s = p.f("view").to_radians().sin();
    let half = p.s("draw") == "A hemisphere";
    let bowl = half && p.s("face") == "Up, like a bowl";
    let (w, h) = (480.0, 360.0);
    let all = if bowl {
        let mut a = ell(0.0, 0.0, r, r * s, 0.0, 360.0);
        a.extend(ell(0.0, 0.0, r, r, 180.0, 360.0));
        a
    } else if half {
        let mut a = ell(0.0, 0.0, r, r, 0.0, 180.0);
        a.extend(ell(0.0, 0.0, r, r * s, 180.0, 360.0));
        a
    } else { ell(0.0, 0.0, r, r, 0.0, 360.0) };
    let m = Map::fit(&all, w, h, 90.0, 50.0);
    let mut d = new_doc(w, h);
    if bowl {
        let mut sil = ell(0.0, 0.0, r, r, 180.0, 360.0);
        sil.extend(ell(0.0, 0.0, r, r * s, 0.0, 180.0));
        d.poly(&mpts(&m, &sil), "#e3eef9", INK, 2.2);
        d.poly(&mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 360.0)), "#f4f9ff", INK, 2.2);
    } else if half {
        let mut sil = ell(0.0, 0.0, r, r, 0.0, 180.0);
        sil.extend(ell(0.0, 0.0, r, r * s, 180.0, 360.0));
        d.poly(&mpts(&m, &sil), "#e3eef9", INK, 2.2);
        if p.b("hidden") { dpoly(&mut d, &mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 180.0)), "#555", 1.4, "6 4"); }
    } else {
        d.poly(&mpts(&m, &ell(0.0, 0.0, r, r, 0.0, 360.0)), "#e3eef9", INK, 2.2);
        d.polyline(&mpts(&m, &ell(0.0, 0.0, r, r * s, 180.0, 360.0)), "#333", 1.4);
        if p.b("hidden") { dpoly(&mut d, &mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 180.0)), "#555", 1.4, "6 4"); }
    }
    base_measure(&mut d, &m, (0.0, 0.0), r, p, true, true);
    d.finish()
}

/// "Start from an example" entries for math-b's figures (solids and plots).
fn examples_b(id: &str) -> Option<Vec<super::Example>> {
    use Value::{B, F, I, S};
    let s = |v: &str| S(v.to_string());
    let ex = |name: &'static str, params: Vec<(&'static str, Value)>| super::Example { name, params };
    let v = match id {
        "sphere" => vec![
            ex("Sphere with radius 6 cm", vec![("radius", F(6.0))]),
            ex("Sphere with diameter 14 inches", vec![("radius", F(7.0)), ("r_lab", s("None")), ("d_lab", s("Measure")), ("unit", s("in"))]),
            ex("Hemisphere with radius 5 m", vec![("draw", s("A hemisphere")), ("radius", F(5.0)), ("unit", s("m"))]),
            ex("Hemisphere bowl with radius 8 cm", vec![("draw", s("A hemisphere")), ("face", s("Up, like a bowl")), ("radius", F(8.0))]),
            ex("Sphere with radius r", vec![("unit", s("")), ("r_lab", s("Text")), ("r_label", s("r"))]),
        ],
        "cylinder" => vec![
            ex("Cylinder with radius 4 cm and height 10 cm", vec![("radius", F(4.0)), ("height", F(10.0))]),
            ex("Cylinder with diameter 12 inches and height 9 inches", vec![("radius", F(6.0)), ("height", F(9.0)), ("r_lab", s("None")), ("d_lab", s("Measure")), ("unit", s("in"))]),
            ex("Oblique cylinder with its height and lean", vec![("stands", s("Oblique")), ("lean", F(4.0)), ("radius", F(3.0)), ("height", F(8.0))]),
            ex("Cylinder with radius r and height h", vec![("unit", s("")), ("r_lab", s("Text")), ("r_label", s("r")), ("h_lab", s("Text")), ("h_label", s("h"))]),
        ],
        "cone" => vec![
            ex("Cone with radius 4 cm and height 9 cm", vec![("radius", F(4.0)), ("height", F(9.0))]),
            ex("Cone with the slant height labeled x", vec![("radius", F(6.0)), ("height", F(8.0)), ("l_lab", s("Text")), ("l_label", s("x"))]),
            ex("Cone with diameter 10 cm and slant height 13 cm", vec![("radius", F(5.0)), ("r_lab", s("None")), ("d_lab", s("Measure")), ("slant", s("13")), ("l_lab", s("Measure")), ("h_lab", s("None"))]),
            ex("Cone with radius r and height h", vec![("unit", s("")), ("r_lab", s("Text")), ("r_label", s("r")), ("h_lab", s("Text")), ("h_label", s("h"))]),
            ex("Oblique cone with its height and lean", vec![("stands", s("Oblique")), ("lean", F(2.5))]),
        ],
        "prism" => vec![
            ex("Rectangular prism 8 cm by 3 cm by 5 cm", vec![("length", F(8.0)), ("width", F(3.0)), ("height", F(5.0))]),
            ex("Cube with 6-inch edges", vec![("length", F(6.0)), ("width", F(6.0)), ("height", F(6.0)), ("unit", s("in"))]),
            ex("Triangular prism with a right triangle base", vec![("base", s("Right triangle")), ("sits", s("Lying on its side")), ("length", F(6.0)), ("width", F(8.0)), ("height", F(12.0)), ("hyp_lab", s("Measure"))]),
            ex("Hexagonal prism with its apothem", vec![("base", s("Regular polygon")), ("n", I(6)), ("side", F(4.0)), ("height", F(10.0)), ("apothem_lab", s("Measure"))]),
            ex("Rectangular prism with a missing height x", vec![("length", F(12.0)), ("width", F(4.0)), ("height", F(5.0)), ("height_lab", s("Text")), ("height_label", s("x")), ("unit", s("ft"))]),
            ex("Oblique prism with its lean", vec![("stands", s("Oblique")), ("lean", F(2.0))]),
        ],
        "pyramid" => vec![
            ex("Square pyramid with slant height 10 cm", vec![("length", F(12.0)), ("width", F(12.0)), ("slant", s("10")), ("slant_lab", s("Measure")), ("height_lab", s("None")), ("show_height", B(false))]),
            ex("Rectangular pyramid 8 cm by 6 cm with height 9 cm", vec![("length", F(8.0)), ("width", F(6.0)), ("height", F(9.0))]),
            ex("Hexagonal pyramid with its apothem and height", vec![("base", s("Regular polygon")), ("n", I(6)), ("side", F(4.0)), ("height", F(9.0)), ("apothem_lab", s("Measure")), ("depth", s("Left")), ("hidden", B(false))]),
            ex("Pyramid with named corners", vec![("names", B(true))]),
        ],
        _ => return examples_b_plots(id),
    };
    Some(v)
}

#[cfg(test)]
mod examples_b_tests {
    use super::*;
    /// Every math-b example names only real params, keeps choice values valid and renders.
    #[test]
    fn math_b_examples_render() {
        for id in ["sphere", "cylinder", "cone", "prism", "pyramid", "coordinate-grid", "number-line", "box-plot", "mapping-diagram", "ruler-reading"] {
            let def = defs().into_iter().find(|d| d.id == id).unwrap();
            let pdefs = (def.params)();
            let exs = examples_b(id).unwrap_or_else(|| panic!("{id}: no examples"));
            assert!(exs.len() >= 3, "{id}: only {} examples", exs.len());
            for e in exs {
                let mut p = Params::defaults(&pdefs);
                for (k, v) in &e.params {
                    let pd = pdefs.iter().find(|d| d.key == *k).unwrap_or_else(|| panic!("{id}/{}: unknown key {k}", e.name));
                    if let (Kind::Choice(opts), Value::S(s)) = (&pd.kind, v) { assert!(opts.contains(&s.as_str()), "{id}/{}: bad choice {k}={s}", e.name); }
                    p.0.insert(k.to_string(), v.clone());
                }
                let svg = (def.render)(&p);
                super::super::to_png(&svg, 0.3).unwrap_or_else(|er| panic!("{id}/{}: {er}", e.name));
            }
        }
    }
}

#[cfg(test)]
mod solids_measures_tests {
    use super::*;
    use serde_json::json;

    fn def(id: &str) -> FigureDef { defs().into_iter().find(|d| d.id == id).unwrap() }
    fn with(id: &str, set: &[(&str, Value)]) -> Params {
        let mut p = Params::defaults(&(def(id).params)());
        for (k, v) in set { p.0.insert(k.to_string(), v.clone()); }
        p
    }
    fn comp(id: &str, p: &Params, key: &str) -> String {
        computed(id, p).into_iter().find(|(k, _)| *k == key).map(|(_, v)| v).unwrap_or_else(|| panic!("{id}: no computed {key}"))
    }
    /// An old saved spec loaded through the real load path (defaults + spec + migrate).
    fn load(id: &str, old: serde_json::Value) -> Params {
        let spec = crate::model::FigureSpec { id: id.into(), params: old.as_object().unwrap().clone().into_iter().collect() };
        crate::figuresui::params_from_spec(&def(id), &spec)
    }
    fn svg_has(id: &str, p: &Params, text: &str) -> bool { (def(id).render)(p).contains(text) }
    fn s(v: &str) -> Value { Value::S(v.into()) }

    #[test]
    fn solvers_match_the_reference() {
        assert_eq!(comp("sphere", &with("sphere", &[]), "diameter"), "6");
        // cylinder: slanted edge worked out (4.5), or given -> height worked out (sqrt 21 = 4.6)
        let p = with("cylinder", &[("stands", s("Oblique")), ("lean", Value::F(2.0))]);
        assert_eq!(comp("cylinder", &p, "edge"), "4.5");
        let p = with("cylinder", &[("stands", s("Oblique")), ("lean", Value::F(2.0)), ("edge", s("5"))]);
        assert_eq!(comp("cylinder", &p, "height"), "4.6");
        assert!((edge_height(&p, true) - 21f64.sqrt()).abs() < 1e-9);
        assert!(svg_has("cylinder", &with("cylinder", &[("stands", s("Oblique")), ("lean", Value::F(2.0)), ("edge", s("5")), ("edge_lab", s("Measure"))]), "5 cm"));
        // cone: slant 5 worked out; slant 6 given -> height 5.2; diameter 10 + slant 13 -> height 12
        assert_eq!(comp("cone", &with("cone", &[]), "slant"), "5");
        assert_eq!(comp("cone", &with("cone", &[("slant", s("6"))]), "height"), "5.2");
        let p = with("cone", &[("radius", Value::F(5.0)), ("slant", s("13"))]);
        assert!((cone_height(&p) - 12.0).abs() < 1e-9);
        // a slant height no longer than the radius cannot be used: the typed height stays
        assert_eq!(cone_height(&with("cone", &[("slant", s("2"))])), 4.0);
        // pyramid: 6x6 h4 -> slant 5; width 8 + slant 5 -> height 3; hexagon side 4 h4 -> 5.3, apothem 3.5
        assert_eq!(comp("pyramid", &with("pyramid", &[]), "slant"), "5");
        assert!((pyramid_height(&with("pyramid", &[("width", Value::F(8.0)), ("slant", s("5"))])) - 3.0).abs() < 1e-9);
        let hex = with("pyramid", &[("base", s("Regular polygon")), ("n", Value::I(6))]);
        assert_eq!(comp("pyramid", &hex, "slant"), "5.3");
        assert_eq!(comp("pyramid", &hex, "apothem"), "3.5");
        // expressions are accepted as typed values
        assert!((pyramid_height(&with("pyramid", &[("slant", s("sqrt(25)"))])) - 4.0).abs() < 1e-9);
        // prism: right triangle 6 x 4 -> hypotenuse 7.2, isosceles -> slanted side 5, hexagon apothem 3.5
        assert_eq!(comp("prism", &with("prism", &[("base", s("Right triangle")), ("length", Value::F(6.0)), ("width", Value::F(4.0))]), "hyp"), "7.2");
        assert_eq!(comp("prism", &with("prism", &[("base", s("Isosceles triangle")), ("length", Value::F(6.0)), ("width", Value::F(4.0))]), "hyp"), "5");
        assert_eq!(comp("prism", &with("prism", &[("base", s("Regular polygon")), ("n", Value::I(6)), ("side", Value::F(4.0))]), "apothem"), "3.5");
        assert_eq!(comp("prism", &with("prism", &[("stands", s("Oblique")), ("lean", Value::F(2.0))]), "edge"), "4.5");
    }

    #[test]
    fn cards_name_real_params() {
        for id in ["cylinder", "cone", "sphere", "pyramid", "prism"] {
            let card = measures(id).unwrap_or_else(|| panic!("{id}: no card"));
            let pdefs = (def(id).params)();
            let has = |k: &str| pdefs.iter().any(|d| d.key == k);
            let comp: Vec<&str> = computed(id, &Params::defaults(&pdefs)).into_iter().map(|(k, _)| k).collect();
            for r in card.groups.iter().flat_map(|g| g.rows.iter()) {
                assert!(has(r.label_key) && has(r.text_key), "{id}/{}: label keys", r.name);
                if r.equals.is_some() { assert!(comp.contains(&r.value_key), "{id}/{}: linked row not computed", r.name); } else { assert!(has(r.value_key), "{id}/{}: value key", r.name); }
            }
        }
    }

    #[test]
    fn new_specs_round_trip_unchanged() {
        for id in ["cylinder", "cone", "sphere", "pyramid", "prism"] {
            let p = with(id, &[("stands", s("Oblique")), ("slant", s("7")), ("edge", s("6")), ("d_lab", s("Text"))]);
            let p2 = crate::figuresui::params_from_spec(&def(id), &crate::figuresui::spec_for(id, &p));
            for (k, v) in &p.0 { if (def(id).params)().iter().any(|d| d.key == k) { assert_eq!(p2.0.get(k), Some(v), "{id}: {k}"); } }
        }
    }

    #[test]
    fn old_specs_migrate() {
        // cone: diameter + slant height given instead of the height
        let p = load("cone", json!({"radius": 5.0, "height": 4.0, "measure": "Diameter", "given": "Slant height", "slant": 13.0, "show_slant": true, "r_lab": "Measure", "l_lab": "Measure", "show_height": false, "stands": "Right", "lean_lab": "None"}));
        assert_eq!((p.s("slant"), p.s("d_lab"), p.s("r_lab"), p.s("l_lab")), ("13", "Measure", "None", "Measure"));
        assert!((cone_height(&p) - 12.0).abs() < 1e-9);
        assert!(svg_has("cone", &p, "10 cm") && svg_has("cone", &p, "13 cm"));
        // cone: height given, slant label switched off
        let p = load("cone", json!({"given": "Height", "slant": 5.0, "show_slant": false, "l_lab": "Measure", "stands": "Right"}));
        assert_eq!((p.s("slant"), p.s("l_lab")), ("", "None"));
        // cylinder: diameter 12 in
        let p = load("cylinder", json!({"radius": 6.0, "measure": "Diameter", "r_lab": "Measure", "unit": "in", "stands": "Right", "lean_lab": "None"}));
        assert_eq!((p.s("d_lab"), p.s("r_lab")), ("Measure", "None"));
        assert!(svg_has("cylinder", &p, "12 in") && !svg_has("cylinder", &p, ">6 in"));
        // sphere: diameter with a text label
        let p = load("sphere", json!({"measure": "Diameter", "r_lab": "Text", "r_label": "d"}));
        assert_eq!((p.s("d_lab"), p.s("d_label"), p.s("r_lab")), ("Text", "d", "None"));
        // pyramid: slant height given, hide list
        let p = load("pyramid", json!({"length": 12.0, "width": 12.0, "given": "Slant height", "slant": 10.0, "show_slant": true, "slant_lab": "Measure", "height_lab": "None", "hide": "width", "stands": "Right"}));
        assert_eq!((p.s("slant"), p.s("slant_lab"), p.s("width_lab"), p.s("lean_lab")), ("10", "Measure", "None", "None"));
        assert!((pyramid_height(&p) - 8.0).abs() < 1e-9);
        assert!(svg_has("pyramid", &p, "10 cm"));
        // pyramid made before the slant height could be given: no slant line
        let p = load("pyramid", json!({"show_slant": false, "slant_lab": "Measure", "stands": "Right", "lean_lab": "Measure"}));
        assert_eq!((p.s("slant"), p.s("slant_lab"), p.s("lean_lab")), ("", "None", "Measure"));
        // prism: hide list + oblique without a lean label (pre-label spec)
        let p = load("prism", json!({"hide": "height, hypotenuse", "stands": "Oblique", "lean": 2.0}));
        assert_eq!((p.s("height_lab"), p.s("hyp_lab"), p.s("lean_lab")), ("None", "None", "None"));
        assert!(!svg_has("prism", &p, ">4 cm<"));
    }
}

/// Plot figures' examples (mirroring the reference's "Start from an example" lists).
fn examples_b_plots(id: &str) -> Option<Vec<super::Example>> {
    use Value::{B, F, I, S};
    let s = |v: &str| S(v.to_string());
    let ex = |name: &'static str, params: Vec<(&'static str, Value)>| super::Example { name, params };
    let q4 = |eq: &str| vec![("xmin", F(-10.0)), ("xmax", F(10.0)), ("ymin", F(-10.0)), ("ymax", F(10.0)), ("equations", S(eq.to_string()))];
    let with = |mut v: Vec<(&'static str, Value)>, more: Vec<(&'static str, Value)>| { v.extend(more); v };
    Some(match id {
        "coordinate-grid" => vec![
            ex("Graph of y = 2x + 1 on a coordinate plane", q4("y = 2x + 1")),
            ex("Plotting points in all four quadrants", with(q4(""), vec![("points", s("A(3, 4), B(-5, 2), C(-4, -6), D(6, -3)"))])),
            ex("First-quadrant graph with axis titles: distance over time", vec![("xmin", F(0.0)), ("xmax", F(10.0)), ("ymin", F(0.0)), ("ymax", F(50.0)), ("ystep", F(5.0)), ("square", B(false)), ("equations", s("y = 5x")), ("xtitle_mode", s("Text")), ("xtitle", s("Time (s)")), ("ytitle_mode", s("Text")), ("ytitle", s("Distance (m)")), ("xlabel", s("")), ("ylabel", s(""))]),
            ex("Graph of the parabola y = x\u{b2} \u{2212} 2x \u{2212} 3", q4("y = x^2 - 2x - 3")),
            ex("Piecewise function graph with open and closed circles", q4("y = x + 2, x < 1; y = -2x + 4, x >= 1")),
            ex("Graph of y = sin x from \u{2212}2\u{3c0} to 2\u{3c0}", vec![("xunits", s("Multiples of \u{3c0}")), ("xmin", F(-2.0)), ("xmax", F(2.0)), ("xstep", F(0.5)), ("ymin", F(-2.0)), ("ymax", F(2.0)), ("square", B(false)), ("xnum", s("Every 2nd line")), ("equations", s("y = sin(x)"))]),
            ex("Graph of y = 3 sin 2x in degrees, from 0\u{b0} to 360\u{b0}", vec![("xunits", s("Degrees")), ("xmin", F(0.0)), ("xmax", F(360.0)), ("xstep", F(90.0)), ("ymin", F(-4.0)), ("ymax", F(4.0)), ("equations", s("y = 3sin(2x)"))]),
            ex("Graphs of y = 2\u{2e3} and y = log\u{2082} x, reflected in y = x", with(q4("y = 2^x; y = log_2(x); y = x"), vec![("xmin", F(-6.0)), ("xmax", F(6.0)), ("ymin", F(-6.0)), ("ymax", F(6.0))])),
            ex("Rational function with a hole and dotted asymptotes", with(q4("y = (x^2 - 1)/((x - 1)(x + 3))"), vec![("asymptotes", B(true))])),
            ex("System of linear inequalities: y > x \u{2212} 2 and y \u{2264} \u{2212}x + 3", with(q4("y > x - 2; y <= -x + 3"), vec![("shading", s("Hatching"))])),
        ],
        "number-line" => vec![
            ex("Graphing the inequality x \u{2265} \u{2212}2 on a number line", vec![("inequality", s("x >= -2"))]),
            ex("Compound inequality \u{2212}3 < x \u{2264} 4 on a number line", vec![("inequality", s("-3 < x <= 4"))]),
            ex("Graph of x < \u{2212}1 or x \u{2265} 3 on a number line", vec![("inequality", s("x < -1 or x >= 3"))]),
            ex("Fraction number line from 0 to 2 with lettered points", vec![("min", F(0.0)), ("max", F(2.0)), ("step_text", s("1/4")), ("label_every", I(4)), ("inequality", s("")), ("points", s("P(3/4), Q(3/2), R(7/4)"))]),
            ex("The sequence a\u{2099} = 1/n on a number line", vec![("min", F(0.0)), ("max", F(1.0)), ("step", F(0.1)), ("inequality", s("")), ("points", s("1, 1/2, 1/3, 1/4, 1/5, 1/6"))]),
        ],
        "box-plot" => vec![
            ex("Box plot of test scores", vec![("data", s("62, 68, 70, 71, 74, 75, 78, 80, 82, 85, 88, 91, 95")), ("axis_title", s("Test score")), ("values", B(false))]),
            ex("Comparing two box plots: Class A and Class B", vec![("data", s("62, 70, 71, 75, 78, 80, 82, 85, 88, 95")), ("name1", s("Class A")), ("data2", s("55, 60, 68, 72, 74, 79, 84, 90")), ("name2", s("Class B")), ("values", B(false))]),
            ex("Box plot from a five-number summary, with Q1, the median and Q3 labeled", vec![("data", s("")), ("summary", s("12, 18, 25, 31, 40")), ("min_lab", s("None")), ("max_lab", s("None"))]),
            ex("Box plot with outliers", vec![("data", s("3, 21, 22, 24, 25, 26, 27, 28, 30, 31, 48")), ("outliers", B(true)), ("values", B(false))]),
            ex("Parts of a box plot: label the five-number summary", vec![("min_lab", s("Text")), ("min_label", s("minimum")), ("q1_lab", s("Text")), ("q1_label", s("Q_1")), ("med_lab", s("Text")), ("med_label", s("median")), ("q3_lab", s("Text")), ("q3_label", s("Q_3")), ("max_lab", s("Text")), ("max_label", s("maximum"))]),
        ],
        "mapping-diagram" => vec![
            ex("Mapping diagram of a function", vec![("inputs", s("1, 2, 3, 4")), ("outputs", s("3, 5, 7, 9")), ("pairs", s("(1,3); (2,5); (3,7); (4,9)"))]),
            ex("Mapping diagram of a relation that is not a function", vec![("inputs", s("1, 2, 3")), ("outputs", s("4, 5, 6")), ("pairs", s("(1,4); (1,5); (2,6); (3,6)"))]),
            ex("Mapping diagram of y = x\u{b2}, a function that isn\u{2019}t one-to-one", vec![("inputs", s("-2, -1, 1, 2")), ("outputs", s("1, 4")), ("pairs", s("(-2,4); (-1,1); (1,1); (2,4)")), ("title_in", s("x")), ("title_out", s("y")), ("title_mode", s("Text")), ("title", s("y = x^2"))]),
            ex("Mapping diagram with words: students and their sports", vec![("inputs", s("Ana, Ben, Carla, Dev")), ("outputs", s("soccer, tennis, chess")), ("pairs", s("(Ana,soccer); (Ben,chess); (Carla,soccer); (Dev,tennis)")), ("title_in", s("Student")), ("title_out", s("Sport"))]),
            ex("Mapping diagram without arrows, for students to draw", vec![("inputs", s("0, 1, 2, 3")), ("outputs", s("1, 3, 5, 7")), ("pairs", s(""))]),
        ],
        "ruler-reading" => vec![
            ex("Measuring to the nearest quarter inch", vec![("marks", s("1/4 in")), ("ruler", I(6)), ("length", F(3.25)), ("read_to", s("Nearest mark")), ("object", s("Metal cylinder")), ("answer", B(true))]),
            ex("Measuring to the nearest eighth inch", vec![("marks", s("1/8 in")), ("ruler", I(6)), ("length", F(1.75)), ("read_to", s("Nearest mark")), ("object", s("Marbles")), ("marbles", I(3)), ("magnify", s("Ruler and magnifiers")), ("mag_marks", I(1))]),
            ex("Measuring to the nearest centimeter", vec![("marks", s("1 cm")), ("ruler", I(15)), ("length", F(7.0)), ("read_to", s("Nearest mark")), ("object", s("Rock")), ("guides", B(true))]),
            ex("Measuring in centimeters and millimeters: 8.4 cm", vec![("marks", s("1 mm")), ("ruler", I(15)), ("length", F(8.4)), ("read_to", s("Nearest mark")), ("object", s("Cube"))]),
            ex("Measuring with a ruler not starting at zero, in inches", vec![("marks", s("1/8 in")), ("ruler", I(6)), ("start", F(1.0)), ("length", F(2.375)), ("read_to", s("Nearest mark")), ("magnify", s("Ruler and magnifiers")), ("answer", B(true))]),
            ex("Reading a 4.37 cm metal cylinder with magnifiers", vec![("object", s("Metal cylinder")), ("magnify", s("Ruler and magnifiers")), ("ruler_color", s("#f2f2f2"))]),
        ],
        _ => return None,
    })
}

// ───────────────────────── box plot ─────────────────────────

fn median(v: &[f64]) -> f64 {
    let n = v.len();
    if n == 0 { 0.0 } else if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}
pub(super) fn five_number(data: &[f64]) -> [f64; 5] {
    let mut v = data.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    let (lo, hi) = (&v[..n / 2], &v[(n + 1) / 2..]);
    [v[0], median(lo), median(&v), median(hi), v[n - 1]]
}
pub(super) fn nice_step(range: f64) -> f64 {
    let raw = range / 8.0;
    let mag = 10f64.powf(raw.log10().floor());
    for f in [1.0, 2.0, 5.0, 10.0] { if raw <= f * mag { return f * mag; } }
    10.0 * mag
}

/// (data key, summary key, name key) of the up-to-four data sets.
pub(super) const BOX_SETS: [(&str, &str, &str); 4] = [("data", "summary", "name1"), ("data2", "summary2", "name2"), ("data3", "summary3", "name3"), ("data4", "summary4", "name4")];
const BOX_STATS: [(&str, &str); 5] = [("min_lab", "min_label"), ("q1_lab", "q1_label"), ("med_lab", "med_label"), ("q3_lab", "q3_label"), ("max_lab", "max_label")];

fn box_plot_params() -> Vec<ParamDef> {
    vec![
        ParamDef::list("data", "Data set 1", "2, 4, 5, 5, 6, 7, 8, 9, 12"),
        ParamDef::list("summary", "Or five-number summary instead: min, Q1, median, Q3, max", ""),
        ParamDef::text("name1", "Name of set 1", ""),
        ParamDef::list("data2", "Data set 2 (optional, to compare)", ""),
        ParamDef::list("summary2", "Or five-number summary for set 2", ""),
        ParamDef::text("name2", "Name of set 2", ""),
        ParamDef::bool("values", "Write the five values", true),
        ParamDef::bool("outliers", "Mark outliers (beyond 1.5 x IQR)", false),
        ParamDef::bool("auto", "Choose the number line automatically", true),
        ParamDef::float("axis_min", "Number line minimum (if not automatic)", -1000.0, 1000.0, 1.0, 0.0),
        ParamDef::float("axis_max", "Number line maximum (if not automatic)", -1000.0, 1000.0, 1.0, 14.0),
        ParamDef::float("axis_step", "Number line step (if not automatic)", 0.05, 100.0, 0.5, 1.0),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("axis_title", "Number line label", ""),
        ParamDef::color("fill", "Box colour", &SWATCHES, "#e9f1fb"),
        ParamDef::list("data3", "Data set 3 (optional)", ""),
        ParamDef::list("summary3", "Or five-number summary for set 3", ""),
        ParamDef::text("name3", "Name of set 3", ""),
        ParamDef::list("data4", "Data set 4 (optional)", ""),
        ParamDef::list("summary4", "Or five-number summary for set 4", ""),
        ParamDef::text("name4", "Name of set 4", ""),
        ParamDef::bool("read5", "Read exactly five numbers as a five-number summary", false),
        ParamDef::choice("min_lab", "Minimum label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("min_label", "Minimum label text", "min"),
        ParamDef::choice("q1_lab", "Q1 label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("q1_label", "Q1 label text", "Q1"),
        ParamDef::choice("med_lab", "Median label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("med_label", "Median label text", "median"),
        ParamDef::choice("q3_lab", "Q3 label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("q3_label", "Q3 label text", "Q3"),
        ParamDef::choice("max_lab", "Maximum label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("max_label", "Maximum label text", "max"),
        ParamDef::bool("caps", "Lines at the whisker ends", true),
        ParamDef::choice("title_mode", "Chart title", &TITLE_MODES, "Text"),
        ParamDef::choice("axis_title_mode", "Axis title", &TITLE_MODES, "Text"),
        ParamDef::choice("numbers", "Numbers", &NUMBER_EVERY, "Every tick"),
        label_size_param(),
    ]
}
const NUMBER_EVERY: [&str; 6] = ["Every tick", "Every 2nd tick", "Every 4th tick", "Every 5th tick", "Every 10th tick", "No numbers"];
/// Number every n-th tick (0 = no numbers).
fn number_every(p: &Params) -> i64 {
    match p.s("numbers") { "Every 2nd tick" | "Every 2nd line" => 2, "Every 4th tick" => 4, "Every 5th tick" | "Every 5th line" => 5, "Every 10th tick" | "Every 10th line" => 10, "No numbers" => 0, _ => 1 }
}

pub(super) fn summary_for(data: &str, summ: &str, outliers: bool) -> Option<([f64; 5], Vec<f64>)> {
    let s = nums(summ);
    if s.len() == 5 { return Some(([s[0], s[1], s[2], s[3], s[4]], vec![])); }
    let d = nums(data);
    if d.is_empty() { return None; }
    let mut f = five_number(&d);
    let mut out = vec![];
    if outliers {
        let iqr = f[3] - f[1];
        let (lo, hi) = (f[1] - 1.5 * iqr, f[3] + 1.5 * iqr);
        out = d.iter().copied().filter(|x| *x < lo || *x > hi).collect();
        let inl: Vec<f64> = d.iter().copied().filter(|x| *x >= lo && *x <= hi).collect();
        if !inl.is_empty() {
            f[0] = inl.iter().cloned().fold(f64::MAX, f64::min);
            f[4] = inl.iter().cloned().fold(f64::MIN, f64::max);
        }
    }
    Some((f, out))
}

/// One set as drawn: (index into BOX_SETS, five numbers, outliers).
pub(super) fn box_set(p: &Params, i: usize) -> Option<([f64; 5], Vec<f64>)> {
    let (dk, sk, _) = BOX_SETS[i];
    if p.b("read5") && nums(p.s(sk)).len() != 5 {
        let d = nums(p.s(dk));
        if d.len() == 5 { let mut s = d.clone(); s.sort_by(|a, b| a.partial_cmp(b).unwrap()); return Some(([s[0], s[1], s[2], s[3], s[4]], vec![])); }
    }
    summary_for(p.s(dk), p.s(sk), p.b("outliers"))
}

pub(super) struct BoxG { pub sets: Vec<(usize, [f64; 5], Vec<f64>)>, pub amin: f64, pub amax: f64, pub step: f64, pub x0: f64, pub x1: f64, pub top: f64, pub rowh: f64, pub w: f64, pub h: f64, pub ay: f64 }
fn shows_title(p: &Params, mode_key: &str, text_key: &str) -> bool {
    match p.s(mode_key) { "None" => false, "Blank line" => true, _ => !p.s(text_key).trim().is_empty() }
}
/// Layout shared with the drag handles (math_plot::box_plot).
pub(super) fn box_geom(p: &Params) -> Option<BoxG> {
    let sets: Vec<(usize, [f64; 5], Vec<f64>)> = (0..4).filter_map(|i| box_set(p, i).map(|(f, o)| (i, f, o))).collect();
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for (_, f, o) in &sets { for v in f.iter().chain(o.iter()) { lo = lo.min(*v); hi = hi.max(*v); } }
    // no data: a bare number line for students to draw on (the reference's blank box plot,
    // 0 to 10 by 1 when the line is automatic), with one empty row above it
    let blank = sets.is_empty();
    let (amin, amax, step) = if blank && p.b("auto") {
        (0.0, 10.0, 1.0)
    } else if p.b("auto") {
        let st = nice_step((hi - lo).max(1e-6));
        ((lo / st).floor() * st - if lo % st == 0.0 { st } else { 0.0 }, (hi / st).ceil() * st + if hi % st == 0.0 { st } else { 0.0 }, st)
    } else { (p.f("axis_min"), p.f("axis_max").max(p.f("axis_min") + 1e-3), p.f("axis_step").max(1e-3)) };
    let named = sets.iter().any(|(i, _, _)| !p.s(BOX_SETS[*i].2).is_empty());
    let w = 680.0;
    let top = if shows_title(p, "title_mode", "title") { 46.0 } else { 20.0 };
    let rowh = 84.0;
    let rows = sets.len().max(1) as f64;
    let ay = top + rowh * rows + 6.0;
    let h = top + rowh * rows + 62.0 + if shows_title(p, "axis_title_mode", "axis_title") { 22.0 } else { 0.0 };
    Some(BoxG { sets, amin, amax, step, x0: if named { 110.0 } else { 40.0 }, x1: w - 36.0, top, rowh, w, h, ay })
}

fn box_plot_render(p: &Params) -> String {
    let Some(g) = box_geom(p) else { return msg("Enter some data or a five-number summary") };
    let (amin, amax, step, x0, x1, w) = (g.amin, g.amax, g.step, g.x0, g.x1, g.w);
    let fs = lsz(p);
    let sx = |v: f64| x0 + (v - amin) / (amax - amin) * (x1 - x0);
    let mut d = new_doc(w, g.h);
    match p.s("title_mode") {
        "None" => {}
        "Blank line" => { d.line(w / 2.0 - 90.0, 30.0, w / 2.0 + 90.0, 30.0, INK, 1.4); }
        _ => if !p.s("title").is_empty() { math_text(&mut d, w / 2.0, 22.0, 18.0, "middle", p.s("title"), true); },
    }
    let ay = g.ay;
    d.line(x0, ay, x1, ay, INK, 1.8);
    let every = number_every(p);
    let k0 = (amin / step).ceil() as i64;
    let k1 = ((amax + 1e-9) / step).floor() as i64;
    for k in k0..=k1 {
        let v = k as f64 * step;
        d.line(sx(v), ay, sx(v), ay + 7.0, INK, 1.4);
        if every > 0 && (k - k0) % every == 0 { put(&mut d, sx(v), ay + 21.0, &fmt(v, 3), 13.0); }
    }
    match p.s("axis_title_mode") {
        "None" => {}
        "Blank line" => { d.line((x0 + x1) / 2.0 - 80.0, ay + 52.0, (x0 + x1) / 2.0 + 80.0, ay + 52.0, INK, 1.4); }
        _ => if !p.s("axis_title").is_empty() { math_text(&mut d, (x0 + x1) / 2.0, ay + 46.0, 15.0, "middle", p.s("axis_title"), true); },
    }
    let caps = !matches!(p.0.get("caps"), Some(Value::B(false)));
    for (row, (si, f, outl)) in g.sets.iter().enumerate() {
        let cy = g.top + g.rowh * (row as f64 + 0.5) + 6.0;
        let bh = 36.0;
        d.line(sx(f[0]), cy, sx(f[1]), cy, INK, 2.0);
        d.line(sx(f[3]), cy, sx(f[4]), cy, INK, 2.0);
        if caps {
            d.line(sx(f[0]), cy - 11.0, sx(f[0]), cy + 11.0, INK, 2.0);
            d.line(sx(f[4]), cy - 11.0, sx(f[4]), cy + 11.0, INK, 2.0);
        }
        d.rect(sx(f[1]), cy - bh / 2.0, (sx(f[3]) - sx(f[1])).max(1.0), bh, &colp(p, "fill", TINT), INK, 2.0);
        d.line(sx(f[2]), cy - bh / 2.0, sx(f[2]), cy + bh / 2.0, INK, 3.4);
        for o in outl { d.circle(sx(*o), cy, 3.6, "#fff", INK, 1.8); }
        let name = p.s(BOX_SETS[*si].2);
        if !name.is_empty() { txt(&mut d, x0 - 14.0, cy + 5.0, 15.0, "end", name); }
        let mut last_x = -1e9;
        let mut lrow = 0;
        for (j, v) in f.iter().enumerate() {
            let (mk, tk) = BOX_STATS[j];
            let mode = match p.s(mk) { "" => "Value", m => m };
            let text = match mode {
                "Text" => p.s(tk).trim().to_string(),
                "Value" if p.b("values") => fmt(*v, 2),
                _ => String::new(),
            };
            if text.is_empty() { continue; }
            let x = sx(*v);
            if x - last_x < stw(&text, fs * 0.8) + 8.0 { lrow = 1 - lrow; } else { lrow = 0; }
            last_x = x;
            let y = cy - bh / 2.0 - 10.0 - lrow as f64 * (fs * 0.85);
            if mode == "Text" { math_text(&mut d, x, y, fs * 0.8, "middle", &text, false); } else { sput(&mut d, "", x, y, &text, fs * 0.8); }
        }
    }
    d.finish()
}

/// Inline math typesetting for plot labels: single letters (x, y, a in "2x + a") are italic,
/// `^` raises the next token (digits, a letter or a (group)), `_` lowers it, and '-' becomes a
/// minus sign. Words of two or more letters (names, sin, log) stay upright. Returns the escaped
/// content of a <text> element.
pub(super) fn math_tspans(s: &str) -> String {
    let ch: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let push_plain = |out: &mut String, t: &str| out.push_str(&super::svg::esc(t));
    let mut prose = false; // inside a word: keep '-' as a hyphen
    while i < ch.len() {
        let c = ch[i];
        if c == '^' || c == '_' {
            i += 1;
            let st = i;
            if i < ch.len() && ch[i] == '(' {
                let mut depth = 0;
                while i < ch.len() { if ch[i] == '(' { depth += 1; } if ch[i] == ')' { depth -= 1; if depth == 0 { i += 1; break; } } i += 1; }
            } else if i < ch.len() && (ch[i] == '-' || ch[i] == '\u{2212}') { i += 1; while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } }
            else if i < ch.len() && ch[i].is_alphabetic() { i += 1; }
            else { while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } }
            let mut tok: String = ch[st..i].iter().collect();
            if tok.starts_with('(') && tok.ends_with(')') && tok.len() >= 2 { tok = tok[1..tok.len() - 1].to_string(); }
            let shift = if c == '^' { "super" } else { "sub" };
            out.push_str(&format!(r#"<tspan baseline-shift="{shift}" font-size="70%">{}</tspan>"#, math_tspans(&tok)));
            continue;
        }
        if c.is_alphabetic() {
            let st = i;
            while i < ch.len() && ch[i].is_alphabetic() { i += 1; }
            let w: String = ch[st..i].iter().collect();
            if w.chars().count() == 1 && !prose {
                out.push_str(&format!(r#"<tspan font-style="italic">{}</tspan>"#, super::svg::esc(&w)));
            } else {
                prose = w.chars().count() > 3 || prose;
                push_plain(&mut out, &w);
            }
            continue;
        }
        if c == '-' && !prose { out.push('\u{2212}'); } else { push_plain(&mut out, &c.to_string()); }
        i += 1;
    }
    out
}

/// Text with math typesetting (see `math_tspans`) in the serif face used for math figures.
pub(super) fn math_text(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str, bold: bool) {
    let (fam, wt) = if bold { ("DejaVu Sans, Arial, Liberation Sans, Helvetica, sans-serif", r#" font-weight="bold""#) } else { (SERIF, "") };
    d.raw(&format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{fam}"{wt} fill="#000">{}</text>"##, y + size * 0.34, math_tspans(s)));
}

pub(super) fn list_items(s: &str) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    // commas, semicolons, or one item a line / a column pasted from a spreadsheet
    for t in s.split(|c| c == ',' || c == ';' || c == '\n' || c == '\t').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        if !v.contains(&t) { v.push(t); }
    }
    v
}

const TITLE_MODES: [&str; 3] = ["Text", "Blank line", "None"];
/// `pairs` value that joins the i-th input to the i-th output ("Match in order").
pub(super) const MATCH_IN_ORDER: &str = "in order";

fn mapping_diagram_params() -> Vec<ParamDef> {
    vec![
        ParamDef::list("inputs", "Inputs (order is top to bottom)", "1, 2, 3, 4"),
        ParamDef::list("outputs", "Outputs", "3, 5, 7, 9"),
        ParamDef::long("pairs", "Arrows as ordered pairs, e.g. (1,3); (2,5)  or  1->3; 2->5", "(1,3); (2,5); (3,7); (4,9)"),
        ParamDef::text("title_in", "Input title", "Input"),
        ParamDef::text("title_out", "Output title", "Output"),
        ParamDef::choice("shape", "Draw each side in", &["Ovals", "Boxes", "None"], "Ovals"),
        ParamDef::color("arrow_color", "Arrow colour", &SWATCHES, "#333333"),
        ParamDef::choice("in_mode", "Input title", &TITLE_MODES, "Text"),
        ParamDef::choice("out_mode", "Output title", &TITLE_MODES, "Text"),
        ParamDef::choice("title_mode", "Diagram title", &TITLE_MODES, "None"),
        ParamDef::text("title", "Diagram title text (type ^ for a power, e.g. y = x^2)", "y = 2x + 1"),
        ParamDef::choice("fill", "Side fill", &["Tinted", "White"], "Tinted"),
        label_size_param(),
        ParamDef::long("paste", "Ordered pairs, e.g. (1, 3), (2, 5), (3, 7): they replace both lists and the arrows", ""),
    ]
}

/// One arrow of a mapping diagram: input, output, and where the output sits in the source text
/// (None for "Match in order" arrows, which have no text of their own).
pub(super) struct MapPair { pub a: String, pub b: String, pub b_span: Option<(usize, usize)> }
pub(super) struct MapData { pub ins: Vec<String>, pub outs: Vec<String>, pub pairs: Vec<MapPair>, pub key: &'static str }

/// Pieces of a pairs text with their byte offsets: `(1,3), (2,5)`, `1->3; 2->5`, or one pair a line.
fn pair_segments(ps: &str) -> Vec<(String, usize)> {
    let mut segs = vec![];
    let mut off = 0;
    for piece in ps.split(|c| c == ';' || c == '\n') {
        if piece.contains('(') {
            let mut at = 0;
            while let Some(i) = piece[at..].find('(') {
                let st = at + i + 1;
                let en = piece[st..].find(')').map(|j| st + j).unwrap_or(piece.len());
                segs.push((piece[st..en].to_string(), off + st));
                at = en.min(piece.len());
                if at >= piece.len() { break; }
            }
        } else {
            segs.push((piece.to_string(), off));
        }
        off += piece.len() + 1;
    }
    segs
}

/// Inputs, outputs and arrows actually drawn: the pasted pairs when there are any (they replace
/// both lists and the arrows), else the lists plus the arrows (a pair may add a new item).
pub(super) fn mapping_parse(p: &Params) -> MapData {
    let pasted = !p.s("paste").trim().is_empty();
    let key = if pasted { "paste" } else { "pairs" };
    let (mut ins, mut outs) = if pasted { (vec![], vec![]) } else { (list_items(p.s("inputs")), list_items(p.s("outputs"))) };
    let ps = p.s(key);
    let mut pairs = vec![];
    if !pasted && ps.trim().eq_ignore_ascii_case(MATCH_IN_ORDER) {
        for (a, b) in ins.iter().zip(outs.iter()) { pairs.push(MapPair { a: a.clone(), b: b.clone(), b_span: None }); }
        return MapData { ins, outs, pairs, key };
    }
    for (inner, off) in pair_segments(ps) {
        let sep = if let Some(i) = inner.find("->") { Some((i, 2)) } else if let Some(i) = inner.find('\u{2192}') { Some((i, 3)) } else { inner.find(',').map(|i| (i, 1)) };
        let Some((i, l)) = sep else { continue };
        let (a, braw) = (inner[..i].trim().to_string(), &inner[i + l..]);
        let b = braw.trim().to_string();
        if a.is_empty() || b.is_empty() { continue; }
        let st = off + i + l + (braw.len() - braw.trim_start().len());
        if !ins.contains(&a) { ins.push(a.clone()); }
        if !outs.contains(&b) { outs.push(b.clone()); }
        if !pairs.iter().any(|q: &MapPair| q.a == a && q.b == b) { pairs.push(MapPair { a, b: b.clone(), b_span: Some((st, st + b.len())) }); }
    }
    MapData { ins, outs, pairs, key }
}

/// The reference's check under the arrows: every input needs exactly one arrow to be a function
/// (only once some arrows are drawn; a diagram with none is for students to finish).
pub(super) fn mapping_msg(p: &Params) -> Option<String> {
    let m = mapping_parse(p);
    if m.pairs.is_empty() { return None; }
    let n = |a: &str| m.pairs.iter().filter(|q| q.a == a).count();
    if let Some(a) = m.ins.iter().find(|a| n(a) > 1) { return Some(format!("Not a function: {a} has {} outputs. Every input needs exactly one.", n(a))); }
    if let Some(a) = m.ins.iter().find(|a| n(a) == 0) { return Some(format!("Not a function yet: {a} has no output. Every input needs exactly one.")); }
    None
}

/// A title in one of the Text / Blank line / None modes.
fn mode_title(d: &mut Doc, mode: &str, x: f64, y: f64, text: &str, size: f64) {
    match mode {
        "None" => {}
        "Blank line" => { d.line(x - 50.0, y + size * 0.45, x + 50.0, y + size * 0.45, INK, 1.4); }
        _ => math_text(d, x, y, size, "middle", text, true),
    }
}

fn mapping_diagram_render(p: &Params) -> String {
    let MapData { ins, outs, pairs, .. } = mapping_parse(p);
    let pairs: Vec<(String, String)> = pairs.into_iter().map(|q| (q.a, q.b)).collect();
    let fs = lsz(p);
    // no pairs: two empty ovals for students to fill in (the reference's blank diagram)
    let rows = if ins.is_empty() && outs.is_empty() { 4.0 } else { ins.len().max(outs.len()).max(1) as f64 };
    let spacing = 42.0;
    let has_title = p.s("title_mode") == "Text" || p.s("title_mode") == "Blank line";
    let top = if has_title { 40.0 } else { 0.0 };
    let h = 100.0 + rows * spacing + top;
    let w = 480.0;
    let (lx, rx) = (130.0, 350.0);
    let cy = top + 66.0 + rows * spacing / 2.0;
    let mut d = new_doc(w, h);
    let shape = p.s("shape");
    let fill = if p.s("fill") == "White" { "#ffffff" } else { TINT };
    // item widths decide how wide the ovals are
    let iw = |v: &[String]| v.iter().map(|t| stw(t, fs)).fold(0.0, f64::max);
    let draw_side = |d: &mut Doc, x: f64, n: usize, half_w: f64| {
        let _ = n;
        let ry = rows * spacing / 2.0 + 14.0;
        let rxx = half_w.max(62.0);
        match shape {
            "Ovals" => { d.raw(&format!(r##"<ellipse cx="{x}" cy="{cy}" rx="{rxx}" ry="{ry}" fill="{fill}" stroke="{INK}" stroke-width="2"/>"##)); }
            "Boxes" | "Rectangles" => { d.raw(&format!(r##"<rect x="{}" y="{}" width="{}" height="{}" rx="4" fill="{fill}" stroke="{INK}" stroke-width="2"/>"##, x - rxx, cy - ry, 2.0 * rxx, 2.0 * ry)); }
            _ => {}
        }
    };
    let (wi, wo) = (iw(&ins) / 2.0 + 24.0, iw(&outs) / 2.0 + 24.0);
    draw_side(&mut d, lx, ins.len(), wi);
    draw_side(&mut d, rx, outs.len(), wo);
    if has_title { mode_title(&mut d, p.s("title_mode"), w / 2.0, 26.0, p.s("title"), 19.0); }
    let im = match p.s("in_mode") { "" => "Text", m => m };
    let om = match p.s("out_mode") { "" => "Text", m => m };
    mode_title(&mut d, im, lx, top + 28.0, p.s("title_in"), 17.0);
    mode_title(&mut d, om, rx, top + 28.0, p.s("title_out"), 17.0);
    // both sides are the same height; a shorter list spreads out over it
    let pos = |i: usize, n: usize| { let g = rows * spacing / n.max(1) as f64; cy + (i as f64 - (n as f64 - 1.0) / 2.0) * g };
    let tw_in: Vec<f64> = ins.iter().map(|t| stw(t, fs)).collect();
    let tw_out: Vec<f64> = outs.iter().map(|t| stw(t, fs)).collect();
    for (a, b) in &pairs {
        let (i, j) = (ins.iter().position(|x| x == a).unwrap(), outs.iter().position(|x| x == b).unwrap());
        let (ya, yb) = (pos(i, ins.len()), pos(j, outs.len()));
        let (xa, xb) = (lx + (tw_in[i] / 2.0 + 8.0).max(26.0), rx - (tw_out[j] / 2.0 + 10.0).max(28.0));
        d.arrow(xa, ya, xb, yb, &colp(p, "arrow_color", "#333"), 1.8);
    }
    for (i, t) in ins.iter().enumerate() { math_text(&mut d, lx, pos(i, ins.len()), fs, "middle", t, false); }
    for (j, t) in outs.iter().enumerate() { math_text(&mut d, rx, pos(j, outs.len()), fs, "middle", t, false); }
    d.finish()
}

// ───────────────────────── length reading (ruler) ─────────────────────────

fn length_reading_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("marks", "Smallest marks", &["1 cm", "5 mm", "1 mm", "1 in", "1/2 in", "1/4 in", "1/8 in", "1/16 in", "10 cm", "5 cm"], "1 mm"),
        ParamDef::int("ruler", "Ruler length (cm or in)", 3, 100, 15),
        ParamDef::choice("object", "Object", &["Bar", "Pencil", "None", "Marbles", "Rock", "Cube", "Metal cylinder"], "Bar"),
        ParamDef::float("start", "Left end of the object (on the ruler)", 0.0, 100.0, 0.05, 0.0),
        ParamDef::float("length", "Length of the object", 0.1, 100.0, 0.01, 4.37),
        ParamDef::bool("guides", "Dotted guides from the object's ends to the ruler", true),
        ParamDef::bool("show_value", "Write the length above the object", false),
        ParamDef::int("dec", "Decimals in the written length", 0, 3, 2),
        ParamDef::color("ruler_color", "Ruler colour", &["#fbf1c7", "#e9f1fb", "#c9def4", "#f3c969", "#ffffff", "#d9d9d9", "#f2f2f2"], "#fbf1c7"),
        ParamDef::color("obj_color", "Bar colour", &["#c9def4", "#f3c969", "#e48a9a", "#b9e0b5", "#d9d9d9", "#ffffff"], "#c9def4"),
        ParamDef::choice("read_to", "Read to", &["One estimated digit", "Nearest mark"], "One estimated digit"),
        ParamDef::int("marbles", "Marbles: how many, in a row", 1, 5, 3),
        ParamDef::choice("magnify", "Magnifiers", &["Ruler only", "Ruler and magnifiers"], "Ruler only"),
        ParamDef::int("mag_marks", "Numbered marks in the magnifier", 1, 6, 2),
        ParamDef::choice("title_mode", "Chart title", &["Text", "None"], "None"),
        ParamDef::text("title", "Chart title text", "How long is the object?"),
        ParamDef::bool("answer", "Answer key: print the length under the figure", false),
    ]
}

/// Units between the smallest marks, and whether the ruler is metric (cm).
fn ruler_step(marks: &str) -> (f64, bool) {
    match marks {
        "10 cm" => (10.0, true), "5 cm" => (5.0, true), "1 cm" => (1.0, true), "5 mm" => (0.5, true), "1 mm" => (0.1, true),
        "1 in" => (1.0, false), "1/2 in" => (0.5, false), "1/4 in" => (0.25, false), "1/8 in" => (0.125, false), _ => (0.0625, false),
    }
}

/// Decimal places a reading carries: the smallest mark's, plus one estimated digit.
fn ruler_read_dec(step: f64, estimate: bool) -> i64 {
    let base = if step >= 1.0 { 0 } else if step >= 0.1 - 1e-9 { 1 } else { 2 };
    base + if estimate { 1 } else { 0 }
}

/// (start, length) as drawn: snapped to the "Read to" precision when that is set.
pub(super) fn ruler_values(p: &Params) -> (f64, f64) {
    let (s, l) = (p.f("start"), p.f("length"));
    let (step, metric) = ruler_step(p.s("marks"));
    match p.s("read_to") {
        "Nearest mark" => { let q = |v: f64| (v / step).round() * step; (q(s), q(l).max(step)) }
        "One estimated digit" => {
            let r = if metric { 10f64.powi(ruler_read_dec(step, true) as i32) } else { 100.0 };
            let q = |v: f64| (v * r).round() / r;
            (q(s), q(l).max(1.0 / r))
        }
        _ => (s, l),
    }
}

/// A reading written for the answer key: decimals for cm, a mixed fraction for inches read to a mark.
fn ruler_fmt(v: f64, p: &Params) -> String {
    let (step, metric) = ruler_step(p.s("marks"));
    let near_mark = p.s("read_to") == "Nearest mark";
    if metric {
        let dec = ruler_read_dec(step, !near_mark) as usize;
        return format!("{:.*} cm", dec, v);
    }
    if near_mark {
        let den = (1.0 / step).round() as i64;
        let n = (v * den as f64).round() as i64;
        let (whole, mut num, mut den) = (n / den, n % den, den);
        while num > 0 && num % 2 == 0 && den % 2 == 0 { num /= 2; den /= 2; }
        return match (whole, num) { (w, 0) => format!("{w} in"), (0, n) => format!("{n}/{den} in"), (w, n) => format!("{w} {n}/{den} in") };
    }
    format!("{:.2} in", v)
}

pub(super) struct RulerL { pub w: f64, pub h: f64, pub x0: f64, pub x1: f64, pub ppu: f64, pub ry: f64, pub top: f64, pub bot: f64, pub title_y: f64, pub mag_cy: f64 }
const MAG_R: f64 = 92.0;

/// Object height on screen for the new objects (sitting on the ruler).
fn ruler_obj_h(p: &Params, len_px: f64) -> f64 {
    match p.s("object") {
        "Marbles" => len_px / p.i("marbles").clamp(1, 5) as f64,
        "Rock" => (len_px * 0.55).clamp(14.0, 90.0),
        "Cube" => len_px.clamp(10.0, 110.0),
        "Metal cylinder" => (len_px * 0.45).clamp(16.0, 50.0),
        _ => 26.0,
    }
}

/// Layout shared with the drag handles (math_plot::ruler).
pub(super) fn ruler_layout(p: &Params) -> RulerL {
    let len = p.i("ruler").max(1) as f64;
    let (w, x0, x1) = (680.0, 30.0, 650.0);
    let ppu = (x1 - x0) / len;
    let (_, l) = ruler_values(p);
    let new_obj = matches!(p.s("object"), "Marbles" | "Rock" | "Cube" | "Metal cylinder");
    let oh = if new_obj { ruler_obj_h(p, l * ppu) } else { 0.0 };
    let title = if p.s("title_mode") == "Text" && !p.s("title").trim().is_empty() { 34.0 } else { 0.0 };
    let mag = if p.s("magnify") == "Ruler and magnifiers" && p.s("object") != "None" { 2.0 * MAG_R + 44.0 } else { 0.0 };
    let extra = (oh - 70.0).max(0.0);
    let ry = 96.0 + title + mag + extra;
    let (top, bot) = if new_obj { (ry - oh, ry) } else { (ry - 54.0, ry - 28.0) };
    let h = 190.0 + title + mag + extra + if p.b("answer") { 30.0 } else { 0.0 };
    RulerL { w, h, x0, x1, ppu, ry, top, bot, title_y: 24.0, mag_cy: title + 18.0 + MAG_R }
}

/// Draws ticks, numbers and the object. `xv` maps a ruler value to x, `k` scales tick heights
/// and text (magnifier), values outside [vmin, vmax] are skipped.
fn ruler_scene(d: &mut Doc, p: &Params, xv: &dyn Fn(f64) -> f64, ry: f64, k: f64, vmin: f64, vmax: f64, ppu: f64, top: f64, bot: f64) {
    let (step0, metric) = ruler_step(p.s("marks"));
    let len = p.i("ruler").max(1) as f64;
    // drop to coarser marks when they would crowd (main ruler only; k > 1 is a magnifier)
    let mut step = step0;
    while ppu * step <= 3.2 && k <= 1.0 {
        step = if metric { match step { s if s < 0.5 => 0.5, s if s < 1.0 => 1.0, s if s < 5.0 => 5.0, _ => 10.0 } } else { step * 2.0 };
        if step >= 10.0 { break; }
    }
    let label_every = if step >= 10.0 { 10.0 } else if step >= 5.0 { 5.0 } else if ppu * k >= 22.0 { 1.0 } else if ppu * k >= 9.0 { 5.0 } else { 10.0 };
    let total = (len / step).round() as i64;
    let isn = |v: f64, m: f64| ((v / m) - (v / m).round()).abs() < 1e-6;
    for i in 0..=total {
        let v = i as f64 * step;
        if v < vmin - 1e-9 || v > vmax + 1e-9 { continue; }
        let x = xv(v);
        let th = if metric {
            if step >= 1.0 { if step == 5.0 && !isn(v, 10.0) { 20.0 } else { 26.0 } }
            else if isn(v, 1.0) { 26.0 } else if isn(v, 0.5) { 18.0 } else { 11.0 }
        } else {
            let lg = (1.0 / step).log2();
            let mut tz = 0.0;
            while tz < lg && isn(v, step * 2f64.powf(tz + 1.0)) { tz += 1.0; }
            (26.0 - 4.5 * (lg - tz)).max(7.0)
        };
        d.line(x, ry, x, ry + th * k, INK, if th >= 26.0 { 1.6 } else { 1.1 } * k.sqrt());
        if isn(v, 1.0) && isn(v, label_every) { put(d, x, ry + 42.0 * k, &fmt(v, 0), 14.0 * k); }
    }
    // object
    let (s, l) = ruler_values(p);
    let (ox0, ox1) = (xv(s), xv(s + l));
    let oh = bot - top;
    let col = colp(p, "obj_color", "#c9def4");
    match p.s("object") {
        "Bar" => { d.rect(ox0, top, ox1 - ox0, oh, &col, INK, 1.8); }
        "Pencil" => {
            let tip = (16.0 * k).min((ox1 - ox0) * 0.4);
            let mid = (top + bot) / 2.0;
            d.poly(&[(ox0, top), (ox1 - tip, top), (ox1, mid), (ox1 - tip, bot), (ox0, bot)], "#f3c969", INK, 1.8);
            d.poly(&[(ox1 - tip, top), (ox1, mid), (ox1 - tip, bot)], "#f1dcb4", INK, 1.8);
            d.rect(ox0, top, 22.0 * k, oh, "#e48a9a", INK, 1.6);
        }
        "Metal cylinder" => {
            let r = (oh * 0.18).min(8.0 * k);
            d.raw(&format!(r##"<rect x="{ox0:.2}" y="{top:.2}" width="{:.2}" height="{oh:.2}" rx="{r:.2}" fill="#a6a6a6" stroke="none"/>"##, ox1 - ox0));
            d.rect(ox0 + r * 0.3, top + oh * 0.18, (ox1 - ox0 - r * 0.6).max(0.0), oh * 0.12, "#d6d6d6", "none", 0.0);
            d.rect(ox0 + r * 0.3, top + oh * 0.66, (ox1 - ox0 - r * 0.6).max(0.0), oh * 0.16, "#7d7d7d", "none", 0.0);
            d.raw(&format!(r##"<rect x="{ox0:.2}" y="{top:.2}" width="{:.2}" height="{oh:.2}" rx="{r:.2}" fill="none" stroke="#111" stroke-width="1.8"/>"##, ox1 - ox0));
        }
        "Cube" => {
            d.rect(ox0, top, ox1 - ox0, oh, "#b3b3b3", INK, 1.8);
            d.rect(ox0 + 1.0, top + 1.0, (ox1 - ox0 - 2.0).max(0.0), oh * 0.12, "#d9d9d9", "none", 0.0);
            d.line(ox0, top + oh * 0.12, ox1, top + oh * 0.12, "#555", 1.0);
        }
        "Rock" => {
            let (w, mid) = (ox1 - ox0, top + oh * 0.55);
            let path = format!(
                "M{:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} Z",
                ox0, mid,
                ox0, top + oh * 0.2, ox0 + w * 0.25, top, ox0 + w * 0.5, top,
                ox0 + w * 0.8, top, ox1, top + oh * 0.25, ox1, mid,
                ox1, top + oh * 0.85, ox0 + w * 0.75, bot, ox0 + w * 0.5, bot,
                ox0 + w * 0.2, bot, ox0, top + oh * 0.9, ox0, mid);
            d.path(&path, "#a3a3a3", INK, 1.8);
            d.path(&format!("M{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2}", ox0 + w * 0.15, top + oh * 0.35, ox0 + w * 0.5, top + oh * 0.25, ox0 + w * 0.85, top + oh * 0.45), "none", INK, 1.0);
            d.path(&format!("M{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2}", ox0 + w * 0.62, top + oh * 0.32, ox0 + w * 0.55, top + oh * 0.7, ox0 + w * 0.66, bot - oh * 0.05), "none", INK, 1.0);
        }
        "Marbles" => {
            let n = p.i("marbles").clamp(1, 5) as usize;
            let dm = (ox1 - ox0) / n as f64;
            for i in 0..n {
                let cx = ox0 + dm * (i as f64 + 0.5);
                let cy = bot - dm / 2.0;
                d.circle(cx, cy, dm / 2.0, "#8f8f8f", INK, 1.4);
                d.raw(&format!(r##"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}" transform="rotate(-30 {:.2} {:.2})" fill="#fff" stroke="#111" stroke-width="0.8"/>"##, cx - dm * 0.2, cy - dm * 0.18, dm * 0.11, dm * 0.065, cx - dm * 0.2, cy - dm * 0.18));
            }
        }
        _ => {}
    }
}

fn length_reading_render(p: &Params) -> String {
    let lay = ruler_layout(p);
    let (x0, x1, ppu, ry) = (lay.x0, lay.x1, lay.ppu, lay.ry);
    let len = p.i("ruler").max(1) as f64;
    let (_, metric) = ruler_step(p.s("marks"));
    let mut d = new_doc(lay.w, lay.h);
    if p.s("title_mode") == "Text" && !p.s("title").trim().is_empty() {
        txt(&mut d, lay.w / 2.0, lay.title_y, 17.0, "middle", p.s("title").trim());
    }
    let rc = colp(p, "ruler_color", "#fbf1c7");
    d.rect(x0 - 8.0, ry, x1 - x0 + 16.0, 62.0, &rc, INK, 1.8);
    let xv = |v: f64| x0 + v * ppu;
    let xclip = |v: f64| xv(v).min(x1 + 8.0);
    ruler_scene(&mut d, p, &xclip, ry, 1.0, 0.0, len, ppu, lay.top, lay.bot);
    txt(&mut d, x0 + 2.0, ry + 56.0, 12.0, "start", if metric { "cm" } else { "in" });
    let (s, l) = ruler_values(p);
    let (ox0, ox1) = (xv(s), xclip(s + l));
    let obj = p.s("object");
    if obj != "None" {
        if p.b("guides") {
            match obj {
                "Bar" => { dline(&mut d, (ox0, lay.bot), (ox0, ry), "#555", 1.2, "2 3"); dline(&mut d, (ox1, lay.bot), (ox1, ry), "#555", 1.2, "2 3"); }
                "Pencil" => { dline(&mut d, (ox0, lay.bot), (ox0, ry), "#555", 1.2, "2 3"); dline(&mut d, (ox1, (lay.top + lay.bot) / 2.0), (ox1, ry), "#555", 1.2, "2 3"); }
                "Marbles" | "Rock" => {
                    let ym = if obj == "Rock" { lay.top + (lay.bot - lay.top) * 0.55 } else { (lay.top + lay.bot) / 2.0 };
                    dline(&mut d, (ox0, ym), (ox0, ry), "#555", 1.2, "5 4");
                    dline(&mut d, (ox1, ym), (ox1, ry), "#555", 1.2, "5 4");
                }
                _ => {}
            }
        }
        if p.b("show_value") { put(&mut d, (ox0 + ox1) / 2.0, lay.top - 16.0, &fmtu(l, p.i("dec"), if metric { "cm" } else { "in" }), 16.0); }
        if p.s("magnify") == "Ruler and magnifiers" {
            // one on the right end, one on the left end unless it is lined up with 0
            let mut ends = vec![s + l];
            if s > 1e-9 { ends.insert(0, s); }
            // span about `mag_marks` numbered marks, but always enlarge at least 2.5 times
            let (step, _) = ruler_step(p.s("marks"));
            let num_gap = if step >= 1.0 { step } else { 1.0 };
            let ppm = (2.0 * MAG_R / ((p.i("mag_marks").clamp(1, 6) as f64 + 0.6) * num_gap)).max(1.6 * ppu);
            let units = MAG_R / ppm + step;
            let k = 2.0;
            let mut cxs: Vec<f64> = ends.iter().map(|v| xv(*v).clamp(MAG_R + 6.0, lay.w - MAG_R - 6.0)).collect();
            if cxs.len() == 2 && cxs[1] - cxs[0] < 2.0 * MAG_R + 12.0 {
                let mid = (cxs[0] + cxs[1]) / 2.0;
                let left = (mid - MAG_R - 6.0).clamp(MAG_R + 6.0, lay.w - 3.0 * MAG_R - 18.0);
                cxs = vec![left, left + 2.0 * MAG_R + 12.0];
            }
            for (j, (v, cx)) in ends.iter().zip(cxs).enumerate() {
                let cy = lay.mag_cy;
                let rs = MAG_R * ppu / ppm; // radius of the circled spot on the ruler
                let (sx, sy) = (xv(*v), ry + 6.0);
                d.circle(sx, sy, rs, "none", INK, 1.2);
                dline(&mut d, (sx - rs, sy), (cx - MAG_R * 0.95, cy + MAG_R * 0.3), "#333", 1.0, "3 3");
                dline(&mut d, (sx + rs, sy), (cx + MAG_R * 0.95, cy + MAG_R * 0.3), "#333", 1.0, "3 3");
                let id = format!("rmag{j}");
                d.raw(&format!(r##"<defs><clipPath id="{id}"><circle cx="{cx:.2}" cy="{cy:.2}" r="{MAG_R}"/></clipPath></defs><circle cx="{cx:.2}" cy="{cy:.2}" r="{MAG_R}" fill="#fff"/><g clip-path="url(#{id})">"##));
                let mry = cy - MAG_R * 0.3;
                let mx = |u: f64| cx + (u - v) * ppm;
                d.rect(cx - MAG_R - 4.0, mry, 2.0 * MAG_R + 8.0, 2.0 * MAG_R, &rc, INK, 2.0);
                let (top, bot) = (mry - (lay.bot - lay.top) * ppm / ppu, mry);
                let (top, bot) = if matches!(obj, "Bar" | "Pencil") { (mry - 54.0 * 1.3, mry - 28.0 * 1.3) } else { (top, bot) };
                ruler_scene(&mut d, p, &mx, mry, k, v - units, v + units, ppm, top, bot);
                d.raw("</g>");
                d.circle(cx, cy, MAG_R, "none", INK, 2.2);
            }
        }
    }
    if p.b("answer") && obj != "None" {
        let t = if s > 1e-9 {
            format!("Left end: {}, right end: {}, length: {}", ruler_fmt(s, p), ruler_fmt(s + l, p), ruler_fmt(l, p))
        } else { format!("Length: {}", ruler_fmt(l, p)) };
        txt(&mut d, lay.w / 2.0, lay.h - 12.0, 15.0, "middle", &t);
    }
    d.finish()
}

// ───────────────────────── composite shape ─────────────────────────

/// A filled piece of a composite shape, with its own shading when one is named.
enum Piece { Poly(Vec<Pt>, Option<&'static str>) }

/// The "Start with" chips (the reference's first screen): each loads the preset's pieces into the
/// Pieces card, so they can be edited, dragged and added to.
pub(super) const COMPOSITE_STARTS: [&str; 7] = ["Rectangle", "Triangle", "Semicircle", "Circle", "L-shape", "House", "Arch"];
pub const COMPOSITE_CUSTOM: &str = "Custom (use the text below)";
const COMPOSITE_HELP: &str = "One piece or mark a row. Pieces: rect X Y W H | tri X1 Y1 X2 Y2 X3 Y3 | poly X Y X Y ... | circle CX CY R | semi CX CY R u/d/l/r (the way it bulges) | quarter CX CY R ne/nw/se/sw. End a piece with blue, red, green or yellow to shade it, and with | TEXT to write a label inside it (rect 0 0 6 4 blue | A). Marks: dim X1 Y1 X2 Y2 TEXT (a dimension; no text = its length; drawn right of the direction of travel) | dash X1 Y1 X2 Y2 (a height or other line) | ticks X1 Y1 X2 Y2 N (congruence ticks) | arrows X1 Y1 X2 Y2 N (parallel arrows) | text X Y WORDS | ra X Y NE/NW/SE/SW (right angle). On the canvas: pick a tool to draw a piece, click a piece to select it, drag it to move, drag its handles to resize, click + beside a side to add a piece there; the Label tool labels sides and marks right angles.";

/// Piece shading named at the end of a row (the reference's None / Blue / Red / Green / Yellow).
#[cfg(test)]
pub(super) fn shade_word(tk: &[&str]) -> Option<&'static str> {
    tk.iter().skip(1).find_map(|t| super::composite::shade_hex(t))
}

/// Arc points of a quarter circle sector piece (`quarter CX CY R ne`), centre first.
pub(super) fn quarter_pts(x: f64, y: f64, r: f64, dir: &str) -> Vec<Pt> {
    let a0 = match dir.to_lowercase().as_str() { "nw" => 90.0, "sw" => 180.0, "se" => 270.0, _ => 0.0 };
    let mut v = vec![(x, y)];
    v.extend(ell(x, y, r, r, a0, a0 + 90.0));
    v
}

fn composite_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("preset", "Start from", &[COMPOSITE_CUSTOM, "L-shape", "House", "Arch", "T-shape", "Rectangle", "Triangle", "Semicircle", "Circle"], COMPOSITE_CUSTOM),
        ParamDef::long("spec",
            "Pieces separated by ;  rect X Y W H | tri X1 Y1 X2 Y2 X3 Y3 | poly X Y X Y ... | circle CX CY R | semi CX CY R DIR(u/d/l/r bulge) | quarter CX CY R ne/nw/se/sw | dash X1 Y1 X2 Y2 | dim X1 Y1 X2 Y2 TEXT (label drawn to the right of travel, go counterclockwise) | ticks X1 Y1 X2 Y2 N | arrows X1 Y1 X2 Y2 N | text X Y WORDS | ra X Y NE/NW/SE/SW (right angle); end a piece with blue/red/green/yellow to shade it and | TEXT to label it",
            "rect 0 0 6 4; semi 3 4 3 u; dash 3 4 6 4; text 4.5 4.6 r = 3; dim 0 0 6 0 6 cm; dim 6 0 6 4 4 cm; ra 0 0 NE; ra 6 0 NW"),
        ParamDef::bool("shade", "Shade the shape", true),
        ParamDef::color("fill_color", "Shade colour", &SWATCHES, "#e9f1fb"),
        ParamDef::text("unit", "Unit for dims given without text (e.g. cm)", ""),
        ParamDef::int("dec", "Round measured dims to (decimal places)", 0, 3, 1),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
    ]
}

pub(super) fn preset_spec(name: &str) -> Option<&'static str> {
    Some(match name {
        "L-shape" => "rect 0 0 8 3; rect 0 3 3 5; dim 0 0 8 0 8 cm; dim 8 0 8 3 3 cm; dim 8 3 3 3 5 cm; dim 3 3 3 8 5 cm; dim 3 8 0 8 3 cm; dim 0 8 0 0 8 cm; ra 0 0 NE; ra 8 0 NW; ra 0 8 SE",
        "House" => "rect 0 0 6 4; tri 0 4 6 4 3 7; dash 3 4 3 7; text 3.5 5.4 3 cm; dim 0 0 6 0 6 cm; dim 0 4 0 0 4 cm; ra 0 0 NE; ra 6 0 NW",
        "Arch" => "rect 0 0 6 4; semi 3 4 3 u; dash 3 4 6 4; text 4.5 4.6 r = 3; dim 0 0 6 0 6 cm; dim 6 0 6 4 4 cm; ra 0 0 NE; ra 6 0 NW",
        "T-shape" => "rect 3 0 3 6; rect 0 6 9 3; dim 3 0 6 0 3 cm; dim 6 0 6 6 6 cm; dim 9 9 0 9 9 cm; dim 9 6 9 9 3 cm; ra 3 0 NE; ra 6 0 NW",
        "Rectangle" => "rect 0 0 8 5; dim 0 0 8 0; dim 8 0 8 5; ra 0 0 NE; ra 8 0 NW; ra 8 5 SW; ra 0 5 SE",
        "Triangle" => "tri 0 0 8 0 3 5; dash 3 5 3 0; dim 0 0 8 0; text 3.6 2.2 h = 5; ra 3 0 NE",
        "Semicircle" => "semi 0 0 4 u; dash 0 0 4 0; dim -4 0 4 0; text 2 0.6 r = 4",
        "Circle" => "circle 0 0 4; dash 0 0 4 0; text 2 0.6 r = 4",
        _ => return None,
    })
}

fn composite_render(p: &Params) -> String {
    composite_svg(p, None)
}

/// An explicit model -> canvas mapping for the canvas editor: canvas size (w, h) and the map
/// (scale, origin). The editor's render has no background and draws nothing when empty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompositeView { pub w: f64, pub h: f64, pub s: f64, pub ox: f64, pub oy: f64 }

/// The composite shape drawn by the canvas editor at its own pan / zoom.
pub fn composite_view_svg(p: &Params, v: CompositeView) -> String {
    composite_svg(p, Some(v))
}

/// The spec the figure draws: a "Start from" preset's, or the typed `spec`.
pub fn composite_spec(p: &Params) -> String {
    preset_spec(p.s("preset")).unwrap_or(p.s("spec")).to_string()
}

fn composite_svg(p: &Params, view: Option<CompositeView>) -> String {
    use super::composite::{self as cs, Shape};
    let spec = composite_spec(p);
    let mut pieces: Vec<Piece> = vec![];
    let mut labels: Vec<(Pt, String)> = vec![];
    let mut dashes: Vec<(Pt, Pt)> = vec![];
    let mut dims: Vec<(Pt, Pt, String)> = vec![];
    let mut texts: Vec<(Pt, String)> = vec![];
    let mut rights: Vec<(Pt, String)> = vec![];
    let mut marks: Vec<(Pt, Pt, u8, bool)> = vec![];
    for it in cs::parse(&spec) {
        let Some(sh) = &it.shape else { continue };
        match sh {
            Shape::Dash(a, b) => dashes.push((*a, *b)),
            Shape::Dim(a, b, t) => dims.push((*a, *b, t.clone())),
            Shape::Ticks(a, b, k) => marks.push((*a, *b, *k, false)),
            Shape::Arrows(a, b, k) => marks.push((*a, *b, *k, true)),
            Shape::Text(a, t) => texts.push((*a, t.clone())),
            Shape::Ra(a, d) => rights.push((*a, d.clone())),
            _ => {
                let v = cs::outline(sh);
                if !it.label.trim().is_empty() { labels.push((cs::centroid(&v), it.label.clone())); }
                pieces.push(Piece::Poly(v, it.shade.as_deref().and_then(cs::shade_hex)));
            }
        }
    }
    if pieces.is_empty() && view.is_none() { return msg("No pieces: start the text with rect, tri, circle, semi or poly"); }
    let mut all: Vec<Pt> = vec![];
    for Piece::Poly(v, _) in &pieces { all.extend(v.iter().copied()); }
    for (a, b) in &dashes { all.push(*a); all.push(*b); }
    for (a, b, _) in &dims { all.push(*a); all.push(*b); }
    let (w, h, m) = match view {
        Some(v) => (v.w, v.h, Map { s: v.s, ox: v.ox, oy: v.oy }),
        None => (480.0, 360.0, Map::fit(&all, 480.0, 360.0, 70.0, 56.0)),
    };
    let mut d = new_doc(w, h);
    let fill_s = colp(p, "fill_color", TINT);
    let fill = if p.b("shade") { fill_s.as_str() } else { "#fff" };
    for Piece::Poly(v, _) in &pieces { d.poly(&mpts(&m, v), INK, INK, 5.0); }
    for Piece::Poly(v, sh) in &pieces { let f = sh.unwrap_or(fill); d.poly(&mpts(&m, v), f, f, 1.2); }
    for (a, b) in &dashes { dline(&mut d, m.p(*a), m.p(*b), "#444", 1.6, "6 4"); }
    for (a, dirs) in &rights {
        let (sx, sy) = (if dirs.contains('W') { -1.0 } else { 1.0 }, if dirs.contains('S') { 1.0 } else { -1.0 });
        let v = m.p(*a);
        d.polyline(&[(v.0 + sx * 12.0, v.1), (v.0 + sx * 12.0, v.1 + sy * 12.0), (v.0, v.1 + sy * 12.0)], INK, 1.3);
    }
    // congruence ticks / parallel arrows across the middle of a side
    for (a, b, k, arrows) in &marks {
        let (pa, pb) = (m.p(*a), m.p(*b));
        let u = unit_v(pa, pb);
        let nn = (-u.1, u.0);
        let c = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
        for j in 0..*k {
            let o = (j as f64 - (*k as f64 - 1.0) / 2.0) * if *arrows { 8.0 } else { 6.0 };
            let q = (c.0 + u.0 * o, c.1 + u.1 * o);
            if *arrows {
                let (tip, back) = ((q.0 + u.0 * 3.0, q.1 + u.1 * 3.0), (q.0 - u.0 * 3.0, q.1 - u.1 * 3.0));
                d.polyline(&[(back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), tip, (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, 1.6);
            } else {
                d.line(q.0 + nn.0 * 6.0, q.1 + nn.1 * 6.0, q.0 - nn.0 * 6.0, q.1 - nn.1 * 6.0, INK, 1.6);
            }
        }
    }
    let lsz = 15.0 * fscale(p);
    // units typed in a label stay upright: the trailing word after a number
    let unit_of = |t: &str| -> String {
        let w: Vec<&str> = t.split_whitespace().collect();
        if w.len() >= 2 && w[w.len() - 2].chars().any(|c| c.is_ascii_digit()) { w[w.len() - 1].to_string() } else { p.s("unit").to_string() }
    };
    for (a, b, t) in &dims {
        let (pa, pb) = (m.p(*a), m.p(*b));
        let u = unit_v(pa, pb);
        let right = (-u.1, u.0);
        let off = 16.0;
        let (qa, qb) = ((pa.0 + right.0 * off, pa.1 + right.1 * off), (pb.0 + right.0 * off, pb.1 + right.1 * off));
        d.line(qa.0, qa.1, qb.0, qb.1, GREY, 1.3);
        for q in [qa, qb] { d.line(q.0 - right.0 * 5.0, q.1 - right.1 * 5.0, q.0 + right.0 * 5.0, q.1 + right.1 * 5.0, GREY, 1.3); }
        let mid = ((qa.0 + qb.0) / 2.0, (qa.1 + qb.1) / 2.0);
        // a dim with no text shows its measured length
        let t = if t.trim().is_empty() { fmtu(dist(*a, *b), p.i("dec"), p.s("unit")) } else { t.clone() };
        let o = 10.0 + 0.5 * (tw(&t, lsz) * 0.85 * right.0.abs() + lsz * right.1.abs());
        mlabel(&mut d, mid.0 + right.0 * o, mid.1 + right.1 * o, lsz, &t, &unit_of(&t));
    }
    for (a, t) in texts.iter().chain(&labels) { let q = m.p(*a); mlabel(&mut d, q.0, q.1, lsz, t, &unit_of(t)); }
    let svg = d.finish();
    if view.is_some() { svg.replacen(r##"<rect width="100%" height="100%" fill="#fff"/>"##, "", 1) } else { svg }
}

/// "Start from an example" entries for math-a's flat-shape figures (mirrors mathfigures.com).
fn examples_a(id: &str) -> Option<Vec<super::Example>> {
    use Value::{I, S};
    let s = |v: &str| S(v.to_string());
    let ex = |name: &'static str, params: Vec<(&'static str, Value)>| super::Example { name, params };
    let v = match id {
        "parallel-lines" => vec![
            ex("Corresponding angles", vec![("marked", s("2, 6")), ("unknown", s("6"))]),
            ex("Alternate interior angles", vec![("marked", s("4, 6")), ("unknown", s("6"))]),
            ex("Same-side interior angles with expressions", vec![("marked", s("3, 6")), ("unknown", s("")), ("angle_text", s("3 = 2x + 10; 6 = 3x - 5"))]),
            ex("All eight angles numbered", vec![("labels", s("Numbers")), ("marked", s("1, 2, 3, 4, 5, 6, 7, 8"))]),
            ex("Two transversals", vec![("transversals", I(2)), ("marked", s("2, 10")), ("unknown", s("10"))]),
        ],
        "composite-shape" => vec![
            // examples load the pieces into the Pieces card (preset stays Custom) so they can be edited
            ex("L-shape for area", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("L-shape").unwrap_or("")))]),
            ex("House: rectangle and triangle", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("House").unwrap_or("")))]),
            ex("Arch: rectangle and semicircle", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("Arch").unwrap_or("")))]),
            ex("T-shape", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("T-shape").unwrap_or("")))]),
            ex("Semicircle", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("Semicircle").unwrap_or(""))), ("unit", s("cm"))]),
            ex("Shaded pieces with marks", vec![("preset", s(COMPOSITE_CUSTOM)), ("shade", Value::B(false)), ("spec", s("rect 0 0 6 4 blue; tri 6 0 9 0 6 4 yellow; quarter 0 4 2 ne green; dim 0 0 6 0; dim 6 0 9 0; dim 0 6 0 0; arrows 0 0 6 0 1; arrows 2 4 6 4 1; ra 0 0 NE"))]),
        ],
        _ => return None,
    };
    Some(v)
}

// ───────────────────────── registry ─────────────────────────

/// Direct-manipulation handles for this subject's figures (None = not one of ours).
/// Renderer lookup for the interaction layer (`math_geo`, `math_plot`).
pub(super) fn render_for(id: &str) -> Option<fn(&Params) -> String> {
    Some(match id {
        "triangle" => triangle_render,
        "rectangle" => rectangle_render,
        "parallelogram" => parallelogram_render,
        "trapezoid" => trapezoid_render,
        "kite" => kite_render,
        "regular-polygon" => regular_polygon_render,
        "prism" => prism_render,
        "pyramid" => pyramid_render,
        "cylinder" => cylinder_render,
        "cone" => cone_render,
        "sphere" => sphere_render,
        _ => return None,
    })
}

/// "Start from an example" entries for this subject's figures. Solids and plots (math-b) live in
/// `examples_b`; flat shapes (math-a) chain their own `examples_a` here with `.or_else`.
pub fn examples(id: &str) -> Option<Vec<super::Example>> {
    flat::examples(id).or_else(|| examples_a(id)).or_else(|| examples_b(id))
}

/// The Measures card of the figure (none yet): rows for the shape's sides/angles/dimensions.
pub fn measures(id: &str) -> Option<super::MeasureCard> {
    use super::{MeasureCard, MeasureGroup, MeasureRow};
    let row = |name, value_key, label_key, text_key| MeasureRow { name, name_key: None, value_key, unit: "$unit", label_key, text_key, tick_key: None, equals: None };
    match id {
        // solids (meas-b): Shape card rows
        "cylinder" | "cone" | "sphere" | "pyramid" | "prism" => measures_solid(id),
        // flat shapes, angles and parallel lines (meas-a)
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon" | "angles" | "parallel-lines" => flat::card(id),
        // the reference's INPUTS AND OUTPUTS card (its function check shows under it)
        "mapping-diagram" => Some(MeasureCard {
            title: "Inputs and outputs",
            help: "Type or paste each list with commas or one item a line, like 1, 2, 3 or a column copied from a spreadsheet. Items can be numbers, math like 2x or 1/2, or words. Each side shows an item once, in the order typed.",
            groups: vec![MeasureGroup {
                title: "",
                rows: vec![
                    MeasureRow { unit: "", ..row("Inputs", "inputs", "", "") },
                    MeasureRow { unit: "", ..row("Outputs", "outputs", "", "") },
                ],
            }],
        }),
        _ => {
            let _ = (row, MeasureCard { title: "", help: "", groups: Vec::<MeasureGroup>::new() });
            None
        }
    }
}

/// The rows card (equations / data sets) of a figure, if it has one.
pub fn rows(id: &str) -> Option<super::RowsCard> {
    use super::{RowSlot, RowsCard, RowsSource};
    let slot = |value_key, name_key, color_key| RowSlot { value_key, name_key, color_key };
    Some(match id {
        "coordinate-grid" => RowsCard {
            title: "Equations",
            help: "One per row: y = 2x + 1; 2x + 3y = 6; x = 4; y > x - 2; -2 < x <= 3; y = x + 2, x < 1 (a piece with a domain).",
            hint: "y = 2x + 1",
            add_label: "Add an equation",
            source: RowsSource::Joined { key: "equations", sep: ";", color_keys: vec!["eq1_color", "eq2_color", "eq3_color", "eq4_color"] },
        },
        "number-line" => RowsCard {
            title: "Equations",
            help: "An inequality such as -2 < x <= 5, or x < -1 or x >= 3. Leave empty to draw no graph.",
            hint: "-2 < x <= 5",
            add_label: "Add an inequality",
            source: RowsSource::Slots(vec![slot("inequality", None, Some("ineq_color")), slot("inequality2", None, Some("ineq2_color"))]),
        },
        // one row per arrow; "Match in order" / "Clear arrows" are the chips under it
        "mapping-diagram" => RowsCard {
            title: "Arrows",
            help: "One arrow a row, from an input to an output: 1 -> 3 or (1, 3). An input can go to more than one output, and an output can come from more than one input. Leave the arrows off for students to draw.",
            hint: "1 -> 3",
            add_label: "Add an arrow",
            // no colours; the empty keys only lift the card's 8-row limit for longer lists
            source: RowsSource::Joined { key: "pairs", sep: ";", color_keys: vec![""; 24] },
        },
        "composite-shape" => RowsCard {
            title: "Pieces",
            help: COMPOSITE_HELP,
            hint: "rect 0 0 6 4",
            add_label: "Add a piece or mark",
            source: RowsSource::Joined { key: "spec", sep: ";", color_keys: vec![""; 40] },
        },
        "box-plot" => RowsCard {
            title: "Data",
            help: "Type the numbers of each data set, separated by commas. Name a set to label its box.",
            hint: "62, 68, 70, 75, 80",
            add_label: "Add a data set",
            source: RowsSource::Slots(vec![slot("data", Some("name1"), None), slot("data2", Some("name2"), None), slot("data3", Some("name3"), None), slot("data4", Some("name4"), None)]),
        },
        _ => return None,
    })
}

/// Visibility of a card group / row for the current params (default: everything is shown).
pub fn card_visible(id: &str, group: &str, row: Option<&str>, p: &Params) -> bool {
    match (id, group, row) {
        // solids: the Oblique rows only for an oblique solid, the base rows only for that base
        (_, "Oblique", _) => p.s("stands") == "Oblique",
        ("prism", "Rectangle or triangle base", Some("hyp")) => p.s("base").contains("triangle"),
        ("prism", "Rectangle or triangle base", _) | ("pyramid", "Rectangle base", _) => p.s("base") != "Regular polygon",
        ("prism" | "pyramid", "Regular polygon base", _) => p.s("base") == "Regular polygon",
        // "Give the diameter instead" swaps the Radius row for a Diameter row
        ("cylinder" | "cone" | "sphere", _, Some("radius" | "diameter")) => !p.b("give_d"),
        ("cylinder" | "cone" | "sphere", _, Some("diam")) => p.b("give_d"),
        // angles: Baseline and Ray 2, then one more row per "Add ray" (a typed row always shows)
        ("angles", _, Some(k)) if k != "ray1" => {
            let i = flat::RAY_V.iter().position(|r| *r == k).unwrap_or(0);
            i < p.i("ray_count").clamp(2, 6) as usize || !p.s(k).trim().is_empty()
        }
        // parallel lines: the third line / second transversal rows once added
        ("parallel-lines", _, Some("p_line")) => p.i("lines") >= 3,
        ("parallel-lines", _, Some("angle_b")) => p.i("transversals") >= 2,
        _ => true,
    }
}

/// (value_key, text) for every measure the figure worked out from the typed ones.
pub fn computed(id: &str, p: &Params) -> Vec<(&'static str, String)> {
    match id {
        "cylinder" | "cone" | "sphere" | "pyramid" | "prism" => computed_solid(id, p),
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon" | "angles" | "parallel-lines" => flat::computed(id, p).unwrap_or_default(),
        "mapping-diagram" => mapping_msg(p).map(|m| vec![("__msg", m)]).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Maps a saved spec's legacy keys onto the reworked inputs.
pub fn migrate(id: &str, spec: &crate::model::FigureSpec, p: &mut Params) {
    match id {
        "cylinder" | "cone" | "sphere" | "pyramid" | "prism" => migrate_solid(id, spec, p),
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon" | "angles" | "parallel-lines" => flat::migrate(id, spec, p),
        // the reference calls the rectangle sides "Boxes"
        "mapping-diagram" => if spec.params.get("shape").and_then(|v| v.as_str()) == Some("Rectangles") { p.set_s("shape", "Boxes"); },
        _ => {}
    }
}

/// "Add ..." chips / action buttons for this subject's figures.
pub fn chips(id: &str) -> Option<super::ChipsCard> {
    use super::{Chip, ChipsCard};
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        // a new ray row (cleared, so it is drawn in the widest gap until a direction is typed)
        "angles" => ChipsCard { title: "Add a ray", count_key: "ray_count", max: 6, chips: vec![Chip { label: "Add ray", set: vec![("ray{n}", s(""))] }] },
        "parallel-lines" => ChipsCard {
            title: "Add a line",
            count_key: "",
            max: 0,
            chips: vec![
                Chip { label: "Add parallel line", set: vec![("lines", Value::I(3))] },
                Chip { label: "Add transversal", set: vec![("transversals", Value::I(2))] },
            ],
        },
        "mapping-diagram" => ChipsCard {
            title: "Quick arrows",
            count_key: "",
            max: 0,
            chips: vec![
                Chip { label: "Match in order", set: vec![("pairs", s(MATCH_IN_ORDER))] },
                Chip { label: "Clear arrows", set: vec![("pairs", s(""))] },
            ],
        },
        "composite-shape" => ChipsCard {
            title: "Start with",
            count_key: "",
            max: 0,
            chips: COMPOSITE_STARTS
                .iter()
                .map(|&name| Chip { label: name, set: vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec(name).unwrap_or("")))] })
                .collect(),
        },
        _ => return None,
    })
}

pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<interact::Handle>> {
    super::math_plot::handles(id, p, size).or_else(|| super::math_geo::handles(id, p, size))
}

/// Solids' "Shading" choice: Tinted (the shaded faces drawn so far) or White, the reference's
/// black-and-white look. Saved solids without the key keep their tint.
fn with_solid_fill(mut v: Vec<ParamDef>) -> Vec<ParamDef> {
    v.push(ParamDef::choice("fill", "Shading", &["Tinted", "White"], "Tinted"));
    v
}

/// White shading: every light tint (the face and cap fills, all channels >= 0xc0) becomes white.
/// Ink, dashed hidden edges, black text and white label halos are left as drawn.
fn solid_fill(svg: String, p: &Params) -> String {
    if p.s("fill") != "White" {
        return svg;
    }
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg.as_str();
    while let Some(i) = rest.find("=\"#") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let hex = tail.get(..6).filter(|h| h.chars().all(|c| c.is_ascii_hexdigit()) && tail[6..].starts_with('"'));
        let light = hex.is_some_and(|h| (0..3).all(|k| u8::from_str_radix(&h[k * 2..k * 2 + 2], 16).is_ok_and(|c| c >= 0xc0)));
        match hex {
            Some(_) if light => out.push_str("ffffff"),
            Some(h) => out.push_str(h),
            None => {}
        }
        rest = if hex.is_some() { &tail[6..] } else { tail };
    }
    out.push_str(rest);
    out
}

pub fn defs() -> Vec<FigureDef> {
    let f = |id, title, blurb, params: fn() -> Vec<ParamDef>, render: fn(&Params) -> String| FigureDef { id, subject: Subject::Math, title, blurb, params, render };
    vec![
        f("coordinate-grid", "Coordinate Grid", "Graph paper with numbered axes, minor gridlines, plotted equations and labelled points.", coordinate_grid_params, coordinate_grid_render),
        f("composite-shape", "Composite Shape", "Build a compound figure from rectangles, triangles, circles and semicircles, with dimension marks.", composite_params, composite_render),
        f("number-line", "Number Line", "A number line with adjustable range and ticks, graphed inequalities and marked points.", number_line_params, number_line_render),
        f("triangle", "Triangle", "A triangle drawn to scale from sides, angles or right-triangle legs, with optional height.", || super::labels::with_param(triangle_params()), |p| super::labels::rendering(p, || triangle_render(p))),
        f("rectangle", "Rectangle", "A rectangle or square drawn to scale with labelled sides and optional diagonals.", || super::labels::with_param(rectangle_params()), |p| super::labels::rendering(p, || rectangle_render(p))),
        f("parallelogram", "Parallelogram", "A parallelogram or rhombus from its sides and an angle, with height and diagonals.", || super::labels::with_param(parallelogram_params()), |p| super::labels::rendering(p, || parallelogram_render(p))),
        f("trapezoid", "Trapezoid", "A plain, isosceles or right trapezoid from its bases and height.", || super::labels::with_param(trapezoid_params()), |p| super::labels::rendering(p, || trapezoid_render(p))),
        f("kite", "Kite", "A kite from its two side lengths and the angle between the short sides.", || super::labels::with_param(kite_params()), |p| super::labels::rendering(p, || kite_render(p))),
        f("regular-polygon", "Regular Polygon", "Any regular polygon from 3 to 20 sides, sized by side, radius or apothem.", || super::labels::with_param(regular_polygon_params()), |p| super::labels::rendering(p, || regular_polygon_render(p))),
        f("angles", "Angles", "Single, adjacent, straight-line, around-a-point or vertical angles with measures.", angles_params, angles_render),
        f("parallel-lines", "Parallel Lines and Transversal", "Parallel lines cut by a transversal with chosen angles marked in degrees or by number.", parallel_lines_params, parallel_lines_render),
        f("prism", "Prism", "A prism on a rectangle, triangle or regular-polygon base, right or oblique, with dashed hidden edges.", || super::labels::with_param(with_solid_fill(prism_params())), |p| super::labels::rendering(p, || solid_fill(prism_render(p), p))),
        f("cylinder", "Cylinder", "A cylinder, right or oblique, with radius or diameter and height.", || super::labels::with_param(with_solid_fill(cylinder_params())), |p| super::labels::rendering(p, || solid_fill(cylinder_render(p), p))),
        f("pyramid", "Pyramid", "A pyramid on a rectangle or regular-polygon base with optional height and slant height.", || super::labels::with_param(with_solid_fill(pyramid_params())), |p| super::labels::rendering(p, || solid_fill(pyramid_render(p), p))),
        f("cone", "Cone", "A cone, right or oblique, with radius or diameter, height and slant height.", || super::labels::with_param(with_solid_fill(cone_params())), |p| super::labels::rendering(p, || solid_fill(cone_render(p), p))),
        f("sphere", "Sphere", "A sphere or hemisphere with its radius or diameter.", || super::labels::with_param(with_solid_fill(sphere_params())), |p| super::labels::rendering(p, || solid_fill(sphere_render(p), p))),
        f("box-plot", "Box Plot", "A box plot from a data list or five-number summary; compare two sets.", box_plot_params, box_plot_render),
        f("mapping-diagram", "Mapping Diagram", "Input and output sets joined by arrows, from ordered pairs.", mapping_diagram_params, mapping_diagram_render),
        f("ruler-reading", "Ruler Reading", "A metric or inch ruler with an object to measure at any position and length.", length_reading_params, length_reading_render),
    ]
}

// ───────────────────────── tests ─────────────────────────

#[cfg(test)]
mod solid_fill_tests {
    use super::*;

    /// Solids are tinted by default (saved figures unchanged) and white on request.
    #[test]
    fn solids_white_shading() {
        for id in ["prism", "cylinder", "pyramid", "cone", "sphere"] {
            let d = defs().into_iter().find(|d| d.id == id).unwrap();
            let mut p = Params::defaults(&(d.params)());
            assert_eq!(p.s("fill"), "Tinted", "{id}");
            let tinted = (d.render)(&p);
            p.set_s("fill", "White");
            let white = (d.render)(&p);
            assert_ne!(tinted, white, "{id}: white shading changed nothing");
            for h in ["#d9e7f6", "#e3eef9", "#f4f9ff"] {
                assert!(!white.contains(h), "{id}: {h} left in white shading");
            }
            assert!(white.contains(INK), "{id}: edges kept");
            to_png(&white, 0.5).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: &str, tag: &str, set: &[(&str, Value)]) {
        let d = defs().into_iter().find(|d| d.id == id).unwrap();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in set {
            assert!(p.0.contains_key(*k), "{id}: unknown key {k}");
            p.0.insert(k.to_string(), v.clone());
        }
        let svg = (d.render)(&p);
        let png = to_png(&svg, 1.5).unwrap_or_else(|e| panic!("{id}/{tag}: {e}"));
        let dir = "/tmp/claude-1000/-home-shuff57/4fa1e9b4-a716-475e-b753-2c60208b3c78/scratchpad/math";
        let _ = std::fs::create_dir_all(dir);
        std::fs::write(format!("{dir}/{id}-{tag}.png"), png).unwrap();
    }
    fn s(x: &str) -> Value { Value::S(x.into()) }
    fn f(x: f64) -> Value { Value::F(x) }
    fn i(x: i64) -> Value { Value::I(x) }
    fn b(x: bool) -> Value { Value::B(x) }

    #[test]
    fn evaluator() {
        assert!((eval("2x + 1", 3.0).unwrap() - 7.0).abs() < 1e-9);
        assert!((eval("x^2 - 3", 2.0).unwrap() - 1.0).abs() < 1e-9);
        assert!((eval("-(x+1)/2", 3.0).unwrap() + 2.0).abs() < 1e-9);
        assert!(eval("2x +", 1.0).is_none());
        assert_eq!(parse_ineq("-2 < x <= 5").len(), 1);
        assert_eq!(parse_ineq("x < -4 or x >= 1").len(), 2);
    }

    #[test]
    fn all_math_defaults() {
        for d in defs() { run(d.id, "default", &[]); }
    }

    #[test]
    fn grid() {
        run("coordinate-grid", "neg", &[("xmin", f(-6.0)), ("xmax", f(6.0)), ("ymin", f(-5.0)), ("ymax", f(7.0)), ("minor", i(4)), ("equations", s("y = x^2 - 3; y = -x + 2; x = 4")), ("points", s("(2,3) A; (-4,-2) B")), ("eq_labels", b(true)), ("title", s("Graphs"))]);
    }
    #[test]
    fn composite() {
        for pr in ["L-shape", "House", "Arch", "T-shape"] { run("composite-shape", &pr.to_lowercase(), &[("preset", s(pr))]); }
    }
    #[test]
    fn numline() {
        run("number-line", "ineq", &[("inequality", s("x < -4 or x >= 1")), ("points", s("-8, 6o")), ("min", f(-10.0)), ("max", f(10.0)), ("minor", i(1))]);
        run("number-line", "dec", &[("min", f(0.0)), ("max", f(2.0)), ("step", f(0.25)), ("inequality", s("x > 0.5")), ("label_every", i(2))]);
    }
    #[test]
    fn polys() {
        run("triangle", "sss", &[("m_a", s("")), ("m_b", s("")), ("m_ab", s("7")), ("m_bc", s("5")), ("m_ca", s("6")), ("height", s("From C")), ("a_lab", s("Measure")), ("b_lab", s("Measure")), ("c_lab", s("Measure")), ("unit", s("cm")), ("bc_lab", s("Measure"))]);
        run("triangle", "obtuse-h", &[("m_a", s("120")), ("m_b", s("")), ("m_ab", s("8")), ("m_ca", s("5")), ("height", s("From C")), ("rotate", f(20.0))]);
        run("triangle", "asa", &[("m_a", s("40")), ("m_b", s("70")), ("m_ab", s("9")), ("c_lab", s("Measure"))]);
        run("triangle", "iso", &[("m_a", s("")), ("m_b", s("")), ("m_ab", s("6")), ("m_bc", s("5")), ("m_ca", s("5")), ("bc_ticks", s("1")), ("ca_ticks", s("1"))]);
        run("triangle", "sqrt", &[("m_a", s("")), ("m_b", s("90")), ("m_ab", s("5")), ("m_bc", s("5")), ("ca_lab", s("Measure"))]);
        run("rectangle", "sq", &[("kind", s("Square")), ("m_ab", s("4")), ("diagonals", s("Drawn and labelled")), ("unit", s("m"))]);
        run("parallelogram", "rh", &[("kind", s("Rhombus")), ("height", s("Onto AB (from D)")), ("m_a", s("110")), ("diagonals", s("Drawn"))]);
        run("parallelogram", "h", &[("height", s("Onto AB (from D)")), ("m_a", s("")), ("m_b", s("55"))]);
        run("trapezoid", "iso", &[("kind", s("Isosceles trapezoid")), ("m_da", s("")), ("m_h", s("5")), ("show_height", b(true)), ("a_lab", s("Measure")), ("b_lab", s("Measure"))]);
        run("trapezoid", "plain", &[("kind", s("Trapezoid")), ("m_da", s("")), ("m_a", s("60")), ("m_h", s("5")), ("show_height", b(true))]);
        run("kite", "diag", &[("diagonals", s("Drawn")), ("a_lab", s("Measure")), ("c_lab", s("Measure")), ("d_lab", s("Measure"))]);
        run("regular-polygon", "n8", &[("n", i(8)), ("apothem", b(true)), ("from", s("Radius")), ("value", f(5.0))]);
        run("regular-polygon", "n3", &[("n", i(3)), ("side_lab", s("Measure")), ("ang_lab", s("Measure"))]);
        run("regular-polygon", "n17", &[("n", i(17)), ("radius", b(false))]);
    }
    #[test]
    fn angle_figs() {
        run("angles", "single", &[("ray2", s("35"))]);
        run("angles", "straight", &[("ray2", s("35")), ("ray3", s("180")), ("unknown", s("2"))]);
        run("angles", "around", &[("ray2", s("35")), ("ray3", s("145")), ("around", b(true)), ("unknown", s("2"))]);
        run("angles", "vertical", &[("ray2", s("35")), ("ray1_line", s("Line")), ("ray2_line", s("Line")), ("ray2_style", s("Dashed"))]);
        run("angles", "custom", &[("ray2", s("50")), ("ray3", s("130")), ("ray4", s("250"))]);
        run("angles", "right", &[("ray2", s("90")), ("ray3", s("180"))]);
        run("parallel-lines", "num", &[("labels", s("Numbers")), ("marked", s("1,2,3,4,5,6,7,8")), ("angle", f(55.0))]);
        run("parallel-lines", "three", &[("lines", i(3)), ("angle", f(110.0)), ("marked", s("2, 6, 10")), ("unknown", s("10")), ("t_arrows", s("2")), ("n_style", s("Dashed"))]);
    }
    #[test]
    fn flat_shapes_parity() {
        run("triangle", "ssa", &[("m_a", s("40")), ("m_b", s("")), ("m_ab", s("10")), ("m_bc", s("7")), ("other", b(true)), ("height", s("From all three")), ("bottom", s("BC")), ("flip", b(true))]);
        run("triangle", "aas", &[("m_a", s("35")), ("m_b", s("95")), ("m_ab", s("")), ("m_bc", s("5")), ("height", s("From A")), ("height_label", s("Text"))]);
        run("triangle", "bad-ssa", &[("m_a", s("40")), ("m_b", s("")), ("m_ab", s("10")), ("m_bc", s("2"))]);
        run("rectangle", "diag", &[("diagonals", s("AC only")), ("ac_lab", s("Text")), ("ac_label", s("x")), ("ab_lab", s("Text")), ("ab_label", s("2x + 3")), ("ab_ticks", s("1")), ("cd_ticks", s("1")), ("label_size", s("Large")), ("bottom", s("BC"))]);
        run("parallelogram", "h2", &[("height", s("Onto AB (from D and C)")), ("diagonals", s("BD only")), ("line_style", s("Dotted")), ("arrow_marks", s("none"))]);
        run("trapezoid", "hc", &[("kind", s("Trapezoid")), ("m_da", s("")), ("m_a", s("65")), ("m_h", s("5")), ("show_height", b(true)), ("hfrom", s("D and C")), ("a_arcs", s("2")), ("b_arcs", s("1"))]);
        run("kite", "h", &[("height", s("From D and C to AB")), ("diagonals", s("Drawn")), ("bottom", s("CD"))]);
        run("regular-polygon", "all", &[("n", i(9)), ("all_radii", b(true)), ("apothem", b(true)), ("center_name", s("O")), ("apothem_label", s("Text")), ("radius_label", s("None")), ("flip", b(true))]);
        run("angles", "turn", &[("ray2", s("50")), ("ray1_line", s("Line")), ("ray2_line", s("Line")), ("turn", f(40.0)), ("ray_ends", s("Points")), ("sector_text", s("1 = 2x")), ("shade", b(false))]);
        run("parallel-lines", "two", &[("lines", i(3)), ("transversals", i(2)), ("turn", f(-20.0)), ("line_ends", s("Plain")), ("angle_text", s("14 = y")), ("marked", s("2, 14"))]);
        for pr in ["Rectangle", "Triangle", "Semicircle", "Circle"] { run("composite-shape", &pr.to_lowercase(), &[("preset", s(pr)), ("unit", s("m"))]); }
        for id in ["triangle", "rectangle", "parallelogram", "trapezoid", "kite", "regular-polygon", "angles", "parallel-lines", "composite-shape"] {
            let exs = examples(id).expect(id);
            assert!((3..=6).contains(&exs.len()), "{id}");
            for e in exs {
                run(id, &format!("ex-{}", e.name.len()), &e.params);
                // every example solves (no message in place of the figure)
                let d = defs().into_iter().find(|d| d.id == id).unwrap();
                let mut p = Params::defaults(&(d.params)());
                for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
                assert!(!(d.render)(&p).contains("Give ") && !(d.render)(&p).contains("would be"), "{id}: {}", e.name);
            }
        }
        // handles still build for the new layouts
        for (id, set) in [("triangle", vec![("flip", b(true))]), ("kite", vec![("bottom", s("AB"))]), ("parallel-lines", vec![("turn", f(30.0)), ("transversals", i(2))]), ("angles", vec![("turn", f(10.0))]), ("trapezoid", vec![]), ("rectangle", vec![]), ("parallelogram", vec![])] {
            let d = defs().into_iter().find(|d| d.id == id).unwrap();
            let mut p = Params::defaults(&(d.params)());
            for (k, v) in set { p.0.insert(k.to_string(), v); }
            assert!(!handles(id, &p, (480.0, 360.0)).unwrap_or_default().is_empty(), "{id}");
        }
    }
    #[test]
    fn solids() {
        run("prism", "tri", &[("base", s("Right triangle"))]);
        run("prism", "iso", &[("base", s("Isosceles triangle"))]);
        run("prism", "hex", &[("base", s("Regular polygon")), ("n", i(6))]);
        run("prism", "oblique", &[("stands", s("Oblique"))]);
        run("pyramid", "slant", &[("slant_lab", s("Measure"))]);
        run("pyramid", "hex", &[("base", s("Regular polygon")), ("n", i(6)), ("stands", s("Oblique")), ("slant_lab", s("Measure"))]);
        run("cylinder", "oblique", &[("stands", s("Oblique")), ("d_lab", s("Measure"))]);
        run("cone", "oblique", &[("stands", s("Oblique")), ("view", f(35.0))]);
        run("sphere", "hemi", &[("draw", s("A hemisphere")), ("d_lab", s("Measure")), ("r_lab", s("None"))]);
    }
    #[test]
    fn data_figs() {
        run("box-plot", "two", &[("data2", s("10, 12, 15, 15, 18, 20, 21, 40")), ("name1", s("Class A")), ("name2", s("Class B")), ("outliers", b(true)), ("title", s("Scores")), ("axis_title", s("Points"))]);
        run("box-plot", "summary", &[("summary", s("20, 35, 40, 55, 90"))]);
        run("mapping-diagram", "many", &[("inputs", s("a, b, c")), ("outputs", s("1, 2")), ("pairs", s("a->1; b->1; c->2")), ("shape", s("Rectangles"))]);
        run("ruler-reading", "pencil", &[("object", s("Pencil")), ("start", f(2.0)), ("length", f(6.45)), ("show_value", b(true))]);
        run("ruler-reading", "inch", &[("marks", s("1/8 in")), ("ruler", i(6)), ("length", f(3.625)), ("start", f(0.0))]);
        run("ruler-reading", "long", &[("ruler", i(30)), ("marks", s("1 mm")), ("length", f(17.3))]);
    }
}

#[cfg(test)]
mod cfg_math_tests {
    use super::*;
    use super::flat::*;
    fn params(id: &str, set: &[(&str, Value)]) -> Params {
        let d = defs().into_iter().find(|d| d.id == id).unwrap();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in set { p.0.insert(k.to_string(), v.clone()); }
        p
    }
    fn s(x: &str) -> Value { Value::S(x.into()) }

    #[test]
    fn new_rays_go_in_the_widest_gap() {
        assert_eq!(widest_gap_mid(&[0.0, 60.0]), 120.0);
        assert_eq!(widest_gap_mid(&[0.0]), 90.0);
        assert_eq!(widest_gap_mid(&[0.0, 200.0]), 100.0);
        let p = params("angles", &[("ray_count", Value::I(3))]);
        let rows = ray_rows(&p);
        assert!(rows.iter().any(|r| r.0 == 2 && r.1 == 120.0 && r.2), "{rows:?}");
        assert!(angles_computed(&p).iter().any(|(k, v)| *k == "ray3" && v == "120"));
        // typed rows always show; empty rows beyond the count are hidden
        assert!(card_visible("angles", "", Some("ray3"), &p) && !card_visible("angles", "", Some("ray4"), &p));
        let p = params("angles", &[("ray5", s("200"))]);
        assert!(card_visible("angles", "", Some("ray5"), &p));
    }

    #[test]
    fn chips_name_real_params() {
        for id in ["angles", "parallel-lines", "mapping-diagram", "composite-shape"] {
            let d = defs().into_iter().find(|d| d.id == id).unwrap();
            let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
            let card = chips(id).unwrap();
            assert!(card.count_key.is_empty() || keys.contains(&card.count_key), "{id}");
            for c in &card.chips {
                for (k, _) in &c.set { assert!(keys.contains(&k.replace("{n}", "3").as_str()), "{id}: {k}"); }
            }
        }
        // every "Start with" chip loads a spec that draws
        for name in COMPOSITE_STARTS { assert!(preset_spec(name).is_some(), "{name}"); }
    }

    #[test]
    fn mapping_pairs_paste_and_order() {
        let p = params("mapping-diagram", &[("pairs", s("1->3; (2,5)"))]);
        let m = mapping_parse(&p);
        assert_eq!(m.pairs.len(), 2);
        assert_eq!(mapping_msg(&p).unwrap(), "Not a function yet: 3 has no output. Every input needs exactly one.");
        let p = params("mapping-diagram", &[("pairs", s(MATCH_IN_ORDER))]);
        let m = mapping_parse(&p);
        assert_eq!(m.pairs.len(), 4);
        assert!(m.pairs.iter().all(|q| q.b_span.is_none()) && mapping_msg(&p).is_none());
        let p = params("mapping-diagram", &[("paste", s("(a, 1), (a, 2), (b, 2)"))]);
        let m = mapping_parse(&p);
        assert_eq!((m.ins.clone(), m.outs.clone(), m.key), (vec!["a".to_string(), "b".into()], vec!["1".to_string(), "2".into()], "paste"));
        assert!(mapping_msg(&p).unwrap().starts_with("Not a function: a has 2 outputs"));
        assert!(mapping_msg(&params("mapping-diagram", &[("pairs", s(""))])).is_none());
        assert_eq!(list_items("1\n2\t3, 3"), vec!["1", "2", "3"]);
    }

    #[test]
    fn quad_side_arrows_and_messages() {
        let p = params("parallelogram", &[("ab_arrows", s("3")), ("bc_arrows", s("None"))]);
        assert_eq!(quad_arrows(&p, &quad_default_arrows("parallelogram")), vec![3, 0, 1, 2]);
        let c = quad_computed("parallelogram", &p);
        assert!(c.iter().any(|(k, v)| *k == "cd_par" && v.ends_with("auto")));
        // the older list still applies to sides left on Auto
        let p = params("trapezoid", &[("arrow_marks", s("AB 2, CD 2"))]);
        assert_eq!(quad_arrows(&p, &quad_default_arrows("trapezoid")), vec![2, 0, 2, 0]);
        let p = params("rectangle", &[("m_ab", s("")), ("m_bc", s(""))]);
        assert!(quad_computed("rectangle", &p).iter().any(|(k, _)| *k == "__msg"));
    }

    #[test]
    fn solids_diameter_and_conflicts() {
        let p = params("cylinder", &[("give_d", Value::B(true)), ("diam", Value::F(3.0))]);
        assert_eq!(solid_r(&p), 1.5);
        assert!(card_visible("cylinder", "", Some("diam"), &p) && !card_visible("cylinder", "", Some("radius"), &p));
        assert!(!card_visible("cylinder", "Oblique", None, &p));
        assert!(diam_text(&p, 3.0).is_some());
        let p = params("cylinder", &[("stands", s("Oblique")), ("edge", s("1"))]);
        assert!(solid_msg("cylinder", &p).unwrap().contains("don\u{2019}t fit together"));
        assert!(card_visible("cylinder", "Oblique", None, &p));
        let p = params("cone", &[("slant", s("2"))]);
        assert!(solid_msg("cone", &p).is_some());
        assert!(solid_msg("cone", &params("cone", &[("slant", s("5"))])).is_none());
        let p = params("prism", &[("base", s("Regular polygon"))]);
        assert!(card_visible("prism", "Regular polygon base", None, &p) && !card_visible("prism", "Rectangle or triangle base", None, &p));
        assert!(!card_visible("prism", "Rectangle or triangle base", Some("hyp"), &params("prism", &[])));
    }

    #[test]
    fn composite_add_piece_and_shading() {
        assert_eq!(super::super::math_plot::piece_on_side((0.0, 4.0), (6.0, 4.0), (3.0, 2.0)), "rect 0 4 6 4.5");
        assert_eq!(super::super::math_plot::piece_on_side((6.0, 0.0), (6.0, 4.0), (3.0, 2.0)), "rect 6 0 3 4");
        assert_eq!(shade_word(&["rect", "0", "0", "1", "1", "Blue"]), Some("#cfe1fa"));
        let p = params("composite-shape", &[("spec", s("rect 0 0 6 4 red; quarter 0 4 2 ne; ticks 0 0 6 0 2; arrows 0 4 6 4 1"))]);
        assert!(composite_render(&p).contains("#fbd0d0"));
        // the side handles add a piece when clicked
        let hs = handles("composite-shape", &p, (480.0, 360.0)).unwrap();
        let add = hs.iter().find(|h| h.click.is_some()).expect("an add-piece handle");
        let mut q = p.clone();
        (add.click.as_ref().unwrap())(&mut q);
        assert_eq!(q.s("spec").matches(';').count(), p.s("spec").matches(';').count() + 1);
    }
}
