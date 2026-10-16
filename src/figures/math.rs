//! Math figure generators (original line-art SVG, modelled on the controls of mathfigures.com).
//! Entry point: `pub fn defs() -> Vec<FigureDef>`.
use super::svg::Doc;
use super::*;
use std::f64::consts::PI;

const INK: &str = "#111";
const GREY: &str = "#6b6b6b";
const TINT: &str = "#e9f1fb";
const BLUE: &str = "#1f5fbf";
const FONT: f64 = 15.0;

type Pt = (f64, f64);

// ───────────────────────── shared helpers ─────────────────────────

fn fmt(v: f64, dec: i64) -> String {
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
fn nums(s: &str) -> Vec<f64> {
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
struct Map { s: f64, ox: f64, oy: f64 }
impl Map {
    fn fit(pts: &[Pt], w: f64, h: f64, mx: f64, my: f64) -> Map {
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in pts { x0 = x0.min(p.0); x1 = x1.max(p.0); y0 = y0.min(p.1); y1 = y1.max(p.1); }
        if x0 > x1 { x0 = 0.0; x1 = 1.0; y0 = 0.0; y1 = 1.0; }
        let s = ((w - 2.0 * mx) / (x1 - x0).max(1e-9)).min((h - 2.0 * my) / (y1 - y0).max(1e-9));
        Map { s, ox: w / 2.0 - s * (x0 + x1) / 2.0, oy: h / 2.0 + s * (y0 + y1) / 2.0 }
    }
    fn p(&self, q: Pt) -> Pt { (self.ox + q.0 * self.s, self.oy - q.1 * self.s) }
}

/// Text next to segment a-b (screen coords), on the side facing away from `center`.
fn label_beside(doc: &mut Doc, a: Pt, b: Pt, t: f64, center: Pt, text: &str, gap: f64) {
    let m = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    let u = unit_v(a, b);
    let mut n = (-u.1, u.0);
    if n.0 * (m.0 - center.0) + n.1 * (m.1 - center.1) < 0.0 { n = (-n.0, -n.1); }
    let off = gap + 0.5 * (tw(text, FONT) * n.0.abs() + FONT * 0.9 * n.1.abs());
    put(doc, m.0 + n.0 * off, m.1 + n.1 * off, text, FONT);
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

#[derive(Clone, Default)]
struct Extra { a: Pt, b: Pt, label: Option<String>, t: f64 }
type Mark = (Pt, Pt, Pt);

struct PolySpec {
    pts: Vec<Pt>,
    names: Vec<String>,
    side_txt: Vec<Option<String>>,
    ang_txt: Vec<Option<String>>,
    ticks: Vec<u8>,
    arrows: Vec<u8>,
    right_marks: bool,
    vlabels: bool,
    extras: Vec<Extra>,
    marks: Vec<Mark>,
    dots: Vec<Pt>,
    rot: f64,
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

fn side_label(names: &[String], i: usize, show: &[String], unk: &[String], len: f64, dec: i64, unit: &str) -> Option<String> {
    let n = names.len();
    let nm = format!("{}{}", names[i], names[(i + 1) % n]);
    if listed(unk, &nm) { Some("x".into()) } else if listed(show, &nm) { Some(fmtu(len, dec, unit)) } else { None }
}

fn height_from(v: Pt, p: Pt, q: Pt, label: &str) -> (Vec<Extra>, Vec<Mark>, f64) {
    let u = unit_v(p, q);
    let l = dist(p, q);
    let t = (v.0 - p.0) * u.0 + (v.1 - p.1) * u.1;
    let foot = (p.0 + u.0 * t, p.1 + u.1 * t);
    let mut ex = vec![Extra { a: v, b: foot, label: Some(label.into()), t: 0.5 }];
    if t < 0.0 { ex.push(Extra { a: p, b: foot, label: None, t: 0.5 }); }
    if t > l { ex.push(Extra { a: q, b: foot, label: None, t: 0.5 }); }
    let dir = if t < l / 2.0 { (foot.0 + u.0, foot.1 + u.1) } else { (foot.0 - u.0, foot.1 - u.1) };
    (ex, vec![(foot, v, dir)], dist(v, foot))
}

fn draw_poly(s: PolySpec, dec: i64, unit: &str) -> String {
    let (cr, sr) = (s.rot.to_radians().cos(), s.rot.to_radians().sin());
    let rot = |p: Pt| (p.0 * cr - p.1 * sr, p.0 * sr + p.1 * cr);
    let n = s.pts.len();
    let pts: Vec<Pt> = s.pts.iter().map(|p| rot(*p)).collect();
    let mut all = pts.clone();
    for e in &s.extras { all.push(rot(e.a)); all.push(rot(e.b)); }
    for d in &s.dots { all.push(rot(*d)); }
    let (w, h) = (480.0, 360.0);
    let m = Map::fit(&all, w, h, 64.0, 46.0);
    let sp: Vec<Pt> = pts.iter().map(|p| m.p(*p)).collect();
    let cen = (sp.iter().map(|p| p.0).sum::<f64>() / n as f64, sp.iter().map(|p| p.1).sum::<f64>() / n as f64);
    let mut doc = new_doc(w, h);
    doc.poly(&sp, TINT, INK, 2.2);

    // extras (dashed construction lines)
    for e in &s.extras {
        let (a, b) = (m.p(rot(e.a)), m.p(rot(e.b)));
        dline(&mut doc, a, b, "#444", 1.5, "6 4");
    }
    for mk in &s.marks {
        let (v, a, b) = (m.p(rot(mk.0)), m.p(rot(mk.1)), m.p(rot(mk.2)));
        square_mark(&mut doc, v, a, b, 10.0);
    }
    for d in &s.dots { let q = m.p(rot(*d)); doc.circle(q.0, q.1, 3.2, INK, INK, 1.0); }

    // tick marks / parallel arrows
    for i in 0..n {
        let (a, b) = (sp[i], sp[(i + 1) % n]);
        let u = unit_v(a, b);
        let nn = (-u.1, u.0);
        let k = s.ticks.get(i).copied().unwrap_or(0) as i32;
        let at = |t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        if k > 0 {
            let mid = at(0.34);
            for j in 0..k {
                let o = (j as f64 - (k - 1) as f64 / 2.0) * 5.0;
                let c = (mid.0 + u.0 * o, mid.1 + u.1 * o);
                doc.line(c.0 - nn.0 * 5.0, c.1 - nn.1 * 5.0, c.0 + nn.0 * 5.0, c.1 + nn.1 * 5.0, INK, 1.6);
            }
        }
        let k = s.arrows.get(i).copied().unwrap_or(0) as i32;
        if k > 0 {
            let mid = at(0.66);
            for j in 0..k {
                let o = (j as f64 - (k - 1) as f64 / 2.0) * 7.0;
                let c = (mid.0 + u.0 * o, mid.1 + u.1 * o);
                let tip = (c.0 + u.0 * 3.5, c.1 + u.1 * 3.5);
                let back = (c.0 - u.0 * 3.5, c.1 - u.1 * 3.5);
                doc.polyline(&[(back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), tip, (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, 1.6);
            }
        }
    }

    // angles
    for i in 0..n {
        let v = sp[i];
        let (pa, pb) = (sp[(i + n - 1) % n], sp[(i + 1) % n]);
        let (u, wv) = (unit_v(v, pa), unit_v(v, pb));
        let ang = (u.0 * wv.0 + u.1 * wv.1).clamp(-1.0, 1.0).acos().to_degrees();
        let is_right = (ang - 90.0).abs() < 0.3;
        if s.right_marks && is_right {
            square_mark(&mut doc, v, pa, pb, 12.0);
            continue;
        }
        if let Some(txt) = s.ang_txt.get(i).and_then(|t| t.clone()) {
            let minlen = dist(v, pa).min(dist(v, pb));
            let ar = (20.0f64).min(minlen * 0.3).max(8.0);
            arc(&mut doc, v, u, wv, ar);
            let mut b = (u.0 + wv.0, u.1 + wv.1);
            let bl = (b.0 * b.0 + b.1 * b.1).sqrt();
            if bl < 1e-6 { b = (-u.1, u.0); } else { b = (b.0 / bl, b.1 / bl); }
            let ww = tw(&txt, 13.0);
            let r = ar + 7.0 + 0.5 * (ww * b.0.abs() + 13.0 * b.1.abs());
            put(&mut doc, v.0 + b.0 * r, v.1 + b.1 * r, &txt, 13.0);
        }
    }
    // side labels
    for i in 0..n {
        if let Some(t) = s.side_txt.get(i).and_then(|t| t.clone()) {
            label_beside(&mut doc, sp[i], sp[(i + 1) % n], 0.5, cen, &t, 9.0);
        }
    }
    let ex_scr: Vec<(Pt, Pt)> = s.extras.iter().map(|e| (m.p(rot(e.a)), m.p(rot(e.b)))).collect();
    for (ei, e) in s.extras.iter().enumerate() {
        if let Some(t) = &e.label {
            let hv = dist(e.a, e.b);
            let txt = if t.starts_with('=') { fmtu(hv, dec, unit) } else if s.names.is_empty() { t.clone() } else { format!("{t} = {}", fmtu(hv, dec, unit)) };
            let (a, b) = ex_scr[ei];
            let tt = if e.t == 0.0 { 0.5 } else { e.t };
            let mid = (a.0 + (b.0 - a.0) * tt, a.1 + (b.1 - a.1) * tt);
            let u = unit_v(a, b);
            let nn = (-u.1, u.0);
            let off = 6.0 + 0.5 * (tw(&txt, FONT) * nn.0.abs() + FONT * 0.9 * nn.1.abs());
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
            put(&mut doc, best.1 .0, best.1 .1, &txt, FONT);
        }
    }
    // vertex letters
    if s.vlabels {
        for i in 0..n {
            let v = sp[i];
            let (u, wv) = (unit_v(v, sp[(i + n - 1) % n]), unit_v(v, sp[(i + 1) % n]));
            let mut b = (-(u.0 + wv.0), -(u.1 + wv.1));
            let bl = (b.0 * b.0 + b.1 * b.1).sqrt();
            if bl < 1e-6 { b = (u.1, -u.0); } else { b = (b.0 / bl, b.1 / bl); }
            let nm = &s.names[i];
            put(&mut doc, v.0 + b.0 * 15.0, v.1 + b.1 * 15.0, nm, 17.0);
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

/// Shared builder for polygon-type figures from parameters + geometry.
fn poly_from_params(p: &Params, pts: Vec<Pt>, arrows: Vec<u8>, extras: Vec<Extra>, marks: Vec<Mark>, dots: Vec<Pt>) -> String {
    let n = pts.len();
    let names: Vec<String> = (0..n).map(|i| ((b'A' + i as u8) as char).to_string()).collect();
    let (dec, unit) = (p.i("dec"), p.s("unit").to_string());
    let show = tokens(p.s("sides"));
    let unk = tokens(p.s("unknown"));
    let angs = tokens(p.s("angles"));
    let lens: Vec<f64> = (0..n).map(|i| dist(pts[i], pts[(i + 1) % n])).collect();
    let angv = poly_angles(&pts);
    let side_txt = (0..n).map(|i| side_label(&names, i, &show, &unk, lens[i], dec, &unit)).collect();
    let ang_txt = (0..n).map(|i| {
        if unk.contains(&names[i]) { Some("x".to_string()) }
        else if angs.contains(&names[i]) { Some(format!("{}\u{b0}", fmt(angv[i], dec))) } else { None }
    }).collect();
    let ticks = if p.b("ticks") { auto_ticks(&lens) } else { vec![0; n] };
    draw_poly(PolySpec {
        pts, names, side_txt, ang_txt, ticks, arrows, right_marks: p.b("right"), vlabels: p.b("vertices"),
        extras, marks, dots, rot: p.f("rotate"),
    }, dec, &unit)
}

fn common(sides: &str, angles: &str, unknown: &str) -> Vec<ParamDef> {
    vec![
        ParamDef::text("sides", "Sides to label (e.g. AB, BC)", sides),
        ParamDef::text("angles", "Angles to label (vertex letters, e.g. A, C)", angles),
        ParamDef::text("unknown", "Show as x (sides like BC, angles like B)", unknown),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::bool("ticks", "Equal-side tick marks", true),
        ParamDef::bool("vertices", "Vertex letters", true),
        ParamDef::text("unit", "Unit (e.g. cm)", ""),
        ParamDef::int("dec", "Decimal places", 0, 3, 1),
        ParamDef::float("rotate", "Rotate (degrees)", -180.0, 180.0, 5.0, 0.0),
    ]
}

fn msg(text: &str) -> String {
    let mut d = new_doc(480.0, 120.0);
    put(&mut d, 240.0, 60.0, text, 16.0);
    d.finish()
}

// ───────────────────────── triangle ─────────────────────────

fn triangle_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("mode", "Triangle given by", &["Three sides (SSS)", "Two sides and the angle between (SAS)", "Two angles and the side between (ASA)", "Right triangle (legs AB, BC)", "Isosceles (base AB, legs BC = CA)", "Equilateral (side AB)"], "Right triangle (legs AB, BC)"),
        ParamDef::float("ab", "Side AB", 0.5, 100.0, 0.5, 12.0),
        ParamDef::float("bc", "Side BC", 0.5, 100.0, 0.5, 5.0),
        ParamDef::float("ca", "Side CA", 0.5, 100.0, 0.5, 13.0),
        ParamDef::float("ang_a", "Angle A (degrees)", 1.0, 178.0, 1.0, 60.0),
        ParamDef::float("ang_b", "Angle B (degrees)", 1.0, 178.0, 1.0, 60.0),
        ParamDef::choice("height", "Height", &["None", "From A", "From B", "From C"], "None"),
    ];
    v.extend(common("AB, BC, CA", "A", "BC"));
    v
}

fn triangle_render(p: &Params) -> String {
    let (ab, bc, ca) = (p.f("ab"), p.f("bc"), p.f("ca"));
    let (aa, ab_ang) = (p.f("ang_a").to_radians(), p.f("ang_b").to_radians());
    let a = (0.0, 0.0);
    let (b, c);
    match p.s("mode") {
        m if m.starts_with("Three") => {
            let cx = (ca * ca + ab * ab - bc * bc) / (2.0 * ab);
            let h2 = ca * ca - cx * cx;
            if h2 <= 1e-9 { return msg("These side lengths do not make a triangle"); }
            b = (ab, 0.0); c = (cx, h2.sqrt());
        }
        m if m.starts_with("Two sides") => { b = (ab, 0.0); c = (ca * aa.cos(), ca * aa.sin()); }
        m if m.starts_with("Two angles") => {
            let gc = PI - aa - ab_ang;
            if gc <= 1e-6 { return msg("The two angles add up to 180\u{b0} or more"); }
            let bl = ab * ab_ang.sin() / gc.sin();
            b = (ab, 0.0); c = (bl * aa.cos(), bl * aa.sin());
        }
        m if m.starts_with("Right") => { b = (ab, 0.0); c = (ab, bc); }
        m if m.starts_with("Isosceles") => {
            let h2 = bc * bc - ab * ab / 4.0;
            if h2 <= 1e-9 { return msg("The legs are too short to reach"); }
            b = (ab, 0.0); c = (ab / 2.0, h2.sqrt());
        }
        _ => { b = (ab, 0.0); c = (ab / 2.0, ab * 3f64.sqrt() / 2.0); }
    }
    let pts = vec![a, b, c];
    let (mut ex, mut mk) = (vec![], vec![]);
    let hv = match p.s("height") { "From A" => Some((a, b, c)), "From B" => Some((b, c, a)), "From C" => Some((c, a, b)), _ => None };
    if let Some((v, q1, q2)) = hv {
        let (e, m, _) = height_from(v, q1, q2, "h");
        ex = e; mk = m;
    }
    poly_from_params(p, pts, vec![0; 3], ex, mk, vec![])
}

// ───────────────────────── rectangle ─────────────────────────

fn rectangle_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("kind", "Kind", &["Rectangle", "Square"], "Rectangle"),
        ParamDef::float("ab", "Side AB (width)", 0.5, 100.0, 0.5, 10.0),
        ParamDef::float("bc", "Side BC (height; ignored for a square)", 0.5, 100.0, 0.5, 6.0),
        ParamDef::choice("diagonals", "Diagonals", &["None", "Drawn", "Drawn and labelled"], "None"),
    ];
    v.extend(common("AB, BC", "", ""));
    v
}

fn rectangle_render(p: &Params) -> String {
    let w = p.f("ab");
    let h = if p.s("kind") == "Square" { w } else { p.f("bc") };
    let pts = vec![(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
    let mut ex = vec![];
    if p.s("diagonals") != "None" {
        let lab = |s: &str| if p.s("diagonals").contains("labelled") { Some(s.to_string()) } else { None };
        ex.push(Extra { a: pts[0], b: pts[2], label: lab("=").map(|_| "=".into()), t: 0.3 });
        ex.push(Extra { a: pts[1], b: pts[3], label: lab("=").map(|_| "=".into()), t: 0.3 });
    }
    poly_from_params(p, pts, vec![0; 4], ex, vec![], vec![])
}

// ───────────────────────── parallelogram ─────────────────────────

fn parallelogram_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("kind", "Kind", &["Parallelogram", "Rhombus"], "Parallelogram"),
        ParamDef::float("ab", "Side AB", 0.5, 100.0, 0.5, 10.0),
        ParamDef::float("da", "Side DA (ignored for a rhombus)", 0.5, 100.0, 0.5, 6.0),
        ParamDef::float("ang_a", "Angle A (degrees)", 10.0, 170.0, 1.0, 60.0),
        ParamDef::choice("height", "Height", &["None", "Onto AB (from D)", "Onto AD (from B)"], "None"),
        ParamDef::choice("diagonals", "Diagonals", &["None", "Drawn", "Drawn and labelled"], "None"),
    ];
    v.extend(common("AB, DA", "A", ""));
    v
}

fn parallelogram_render(p: &Params) -> String {
    let ab = p.f("ab");
    let da = if p.s("kind") == "Rhombus" { ab } else { p.f("da") };
    let t = p.f("ang_a").to_radians();
    let d = (da * t.cos(), da * t.sin());
    let (a, b) = ((0.0, 0.0), (ab, 0.0));
    let c = (b.0 + d.0, d.1);
    let pts = vec![a, b, c, d];
    let (mut ex, mut mk) = (vec![], vec![]);
    match p.s("height") {
        "Onto AB (from D)" => { let (e, m, _) = height_from(d, a, b, "h"); ex = e; mk = m; }
        "Onto AD (from B)" => { let (e, m, _) = height_from(b, a, d, "h"); ex = e; mk = m; }
        _ => {}
    }
    if p.s("diagonals") != "None" {
        let lab = p.s("diagonals").contains("labelled");
        ex.push(Extra { a, b: c, label: if lab { Some("=".into()) } else { None }, t: 0.3 });
        ex.push(Extra { a: b, b: d, label: if lab { Some("=".into()) } else { None }, t: 0.3 });
    }
    poly_from_params(p, pts, vec![1, 2, 1, 2], ex, mk, vec![])
}

// ───────────────────────── trapezoid ─────────────────────────

fn trapezoid_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("kind", "Kind", &["Trapezoid", "Isosceles trapezoid", "Right trapezoid"], "Right trapezoid"),
        ParamDef::float("ab", "Base AB (bottom)", 0.5, 100.0, 0.5, 12.0),
        ParamDef::float("cd", "Base CD (top)", 0.5, 100.0, 0.5, 7.0),
        ParamDef::float("height", "Height", 0.5, 100.0, 0.5, 5.0),
        ParamDef::float("offset", "Slant of top (plain trapezoid only; DA leans by this)", -50.0, 50.0, 0.5, 2.0),
        ParamDef::bool("show_height", "Draw the height", false),
        ParamDef::choice("diagonals", "Diagonals", &["None", "Drawn"], "None"),
    ];
    v.extend(common("AB, CD, DA", "", "BC"));
    v
}

fn trapezoid_render(p: &Params) -> String {
    let (ab, cd, h) = (p.f("ab"), p.f("cd"), p.f("height"));
    let off = match p.s("kind") { "Isosceles trapezoid" => (ab - cd) / 2.0, "Right trapezoid" => 0.0, _ => p.f("offset") };
    let (a, b, c, d) = ((0.0, 0.0), (ab, 0.0), (off + cd, h), (off, h));
    let (mut ex, mut mk) = (vec![], vec![]);
    if p.b("show_height") && off.abs() > 1e-9 {
        let (e, m, _) = height_from(d, a, b, "h");
        ex = e; mk = m;
    } else if p.b("show_height") {
        ex.push(Extra { a: (ab / 2.0, 0.0), b: (ab / 2.0, h), label: Some("h".into()), t: 0.5 });
        mk.push(((ab / 2.0, 0.0), (ab / 2.0, h), (ab, 0.0)));
    }
    if p.s("diagonals") == "Drawn" {
        ex.push(Extra { a, b: c, label: None, t: 0.5 });
        ex.push(Extra { a: b, b: d, label: None, t: 0.5 });
    }
    poly_from_params(p, vec![a, b, c, d], vec![1, 0, 1, 0], ex, mk, vec![])
}

// ───────────────────────── kite ─────────────────────────

fn kite_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::float("ab", "Short sides AB = BC", 0.5, 100.0, 0.5, 5.0),
        ParamDef::float("da", "Long sides CD = DA", 0.5, 100.0, 0.5, 9.0),
        ParamDef::float("ang_b", "Angle B between the short sides (degrees)", 20.0, 170.0, 1.0, 100.0),
        ParamDef::choice("diagonals", "Diagonals", &["None", "Drawn"], "None"),
    ];
    v.extend(common("AB, DA", "B", ""));
    v
}

fn kite_render(p: &Params) -> String {
    let (a_len, d_len) = (p.f("ab"), p.f("da"));
    let half = p.f("ang_b").to_radians() / 2.0;
    let w = a_len * half.sin();
    let ay = -a_len * half.cos();
    let dlen = d_len.max(w * 1.02);
    let dy = ay - (dlen * dlen - w * w).sqrt();
    let (a, b, c, d) = ((-w, ay), (0.0, 0.0), (w, ay), (0.0, dy));
    let (mut ex, mut mk) = (vec![], vec![]);
    if p.s("diagonals") == "Drawn" {
        ex.push(Extra { a: b, b: d, label: None, t: 0.5 });
        ex.push(Extra { a, b: c, label: None, t: 0.5 });
        mk.push(((0.0, ay), (0.0, 0.0), (w, ay)));
    }
    poly_from_params(p, vec![a, b, c, d], vec![0; 4], ex, mk, vec![])
}

// ───────────────────────── regular polygon ─────────────────────────

fn regular_polygon_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::int("n", "Number of sides", 3, 20, 6),
        ParamDef::choice("from", "Size given as", &["Side", "Radius", "Apothem"], "Side"),
        ParamDef::float("value", "Size value", 0.5, 100.0, 0.5, 8.0),
        ParamDef::bool("radius", "Draw radius (center to A)", true),
        ParamDef::bool("apothem", "Draw apothem (center to side AB)", false),
    ];
    v.extend(common("AB", "", ""));
    v
}

fn regular_polygon_render(p: &Params) -> String {
    let n = p.i("n").clamp(3, 20) as usize;
    let v = p.f("value");
    let half = PI / n as f64;
    let r = match p.s("from") { "Radius" => v, "Apothem" => v / half.cos(), _ => v / (2.0 * half.sin()) };
    let phi0 = -PI / 2.0 - half;
    let pts: Vec<Pt> = (0..n).map(|k| { let t = phi0 + 2.0 * PI * k as f64 / n as f64; (r * t.cos(), r * t.sin()) }).collect();
    let mut ex = vec![];
    let mut mk = vec![];
    let mut dots = vec![];
    if p.b("radius") || p.b("apothem") { dots.push((0.0, 0.0)); }
    if p.b("radius") { ex.push(Extra { a: (0.0, 0.0), b: pts[0], label: Some("r".into()), t: 0.5 }); }
    if p.b("apothem") {
        let mid = ((pts[0].0 + pts[1].0) / 2.0, (pts[0].1 + pts[1].1) / 2.0);
        ex.push(Extra { a: (0.0, 0.0), b: mid, label: Some("a".into()), t: 0.5 });
        mk.push((mid, (0.0, 0.0), pts[1]));
    }
    poly_from_params(p, pts, vec![0; n], ex, mk, dots)
}

// ───────────────────────── coordinate grid ─────────────────────────

struct Ev<'a> { s: &'a [char], i: usize, x: f64 }
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
                Some(c) if c.is_ascii_digit() || c == '.' || c == '(' || c.is_alphabetic() => { v *= self.unary()?; }
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
    fn atom(&mut self) -> Option<f64> {
        match self.peek()? {
            '(' => { self.i += 1; let v = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1; Some(v) }
            c if c.is_ascii_digit() || c == '.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '.') { self.i += 1; }
                self.s[st..self.i].iter().collect::<String>().parse().ok()
            }
            c if c.is_alphabetic() => {
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_alphabetic() { self.i += 1; }
                let name: String = self.s[st..self.i].iter().collect();
                match name.as_str() {
                    "x" => Some(self.x),
                    "pi" => Some(PI),
                    "e" => Some(std::f64::consts::E),
                    f => {
                        if self.peek()? != '(' { return None; }
                        self.i += 1; let a = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1;
                        Some(match f { "sqrt" => a.sqrt(), "abs" => a.abs(), "sin" => a.sin(), "cos" => a.cos(), "tan" => a.tan(), "ln" => a.ln(), "log" => a.log10(), "exp" => a.exp(), _ => return None })
                    }
                }
            }
            _ => None,
        }
    }
}
fn eval(expr: &str, x: f64) -> Option<f64> {
    let cs: Vec<char> = expr.replace('\u{2212}', "-").replace('\u{b7}', "*").to_lowercase().chars().collect();
    let mut e = Ev { s: &cs, i: 0, x };
    let v = e.expr()?;
    e.ws();
    if e.i == cs.len() { Some(v) } else { None }
}

fn coordinate_grid_params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("xmin", "x-axis minimum", -100.0, 100.0, 1.0, 0.0),
        ParamDef::float("xmax", "x-axis maximum", -100.0, 100.0, 1.0, 10.0),
        ParamDef::float("xstep", "x-axis step", 0.1, 50.0, 0.5, 1.0),
        ParamDef::float("ymin", "y-axis minimum", -100.0, 100.0, 1.0, 0.0),
        ParamDef::float("ymax", "y-axis maximum", -100.0, 100.0, 1.0, 10.0),
        ParamDef::float("ystep", "y-axis step", 0.1, 50.0, 0.5, 1.0),
        ParamDef::int("minor", "Minor gridlines per step (0 = none)", 0, 10, 0),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::bool("arrows", "Arrowheads on axes", true),
        ParamDef::bool("square", "Square cells", true),
        ParamDef::text("xlabel", "x-axis label", "x"),
        ParamDef::text("ylabel", "y-axis label", "y"),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("equations", "Equations, separated by ; (e.g. y = 2x + 1; y = x^2 - 3; x = 4)", "y = 2x + 1"),
        ParamDef::bool("eq_labels", "Write each equation on its line", false),
        ParamDef::text("points", "Points, separated by ; (e.g. (2,3) A; (-1,4) B)", ""),
    ]
}

const PALETTE: [&str; 5] = ["#1f5fbf", "#c0392b", "#2a8a3e", "#8e44ad", "#d68910"];

fn coordinate_grid_render(p: &Params) -> String {
    let (x0, mut x1, y0, mut y1) = (p.f("xmin"), p.f("xmax"), p.f("ymin"), p.f("ymax"));
    if x1 <= x0 { x1 = x0 + 1.0; }
    if y1 <= y0 { y1 = y0 + 1.0; }
    let (xs, ys) = (p.f("xstep").max(1e-6), p.f("ystep").max(1e-6));
    let (maxw, maxh) = (440.0, 400.0);
    let (ux, uy) = if p.b("square") {
        let u = (maxw / (x1 - x0)).min(maxh / (y1 - y0)); (u, u)
    } else { (maxw / (x1 - x0), maxh / (y1 - y0)) };
    let (pw, ph) = ((x1 - x0) * ux, (y1 - y0) * uy);
    let title = p.s("title");
    let (ml, mr, mt, mb) = (52.0, 34.0, if title.is_empty() { 30.0 } else { 62.0 }, 46.0);
    let (w, h) = (pw + ml + mr, ph + mt + mb);
    let mut d = new_doc(w, h);
    let sx = |x: f64| ml + (x - x0) * ux;
    let sy = |y: f64| mt + ph - (y - y0) * uy;
    let minor = p.i("minor");
    // gridlines
    if minor > 1 {
        let ms = xs / minor as f64;
        let mut k = (x0 / ms).ceil();
        while k * ms <= x1 + 1e-9 { d.line(sx(k * ms), mt, sx(k * ms), mt + ph, "#e6ebf1", 0.8); k += 1.0; }
        let ms = ys / minor as f64;
        let mut k = (y0 / ms).ceil();
        while k * ms <= y1 + 1e-9 { d.line(ml, sy(k * ms), ml + pw, sy(k * ms), "#e6ebf1", 0.8); k += 1.0; }
    }
    let mut k = (x0 / xs).ceil();
    while k * xs <= x1 + 1e-9 { d.line(sx(k * xs), mt, sx(k * xs), mt + ph, "#c4cdd8", 1.0); k += 1.0; }
    let mut k = (y0 / ys).ceil();
    while k * ys <= y1 + 1e-9 { d.line(ml, sy(k * ys), ml + pw, sy(k * ys), "#c4cdd8", 1.0); k += 1.0; }
    // axes
    let ax_y = if y0 <= 0.0 && y1 >= 0.0 { 0.0 } else { y0 };
    let ax_x = if x0 <= 0.0 && x1 >= 0.0 { 0.0 } else { x0 };
    let ext = if p.b("arrows") { 16.0 } else { 0.0 };
    let (axy, axx) = (sy(ax_y), sx(ax_x));
    if p.b("arrows") {
        d.arrow(ml, axy, ml + pw + ext, axy, INK, 1.8);
        d.arrow(axx, mt + ph, axx, mt - ext, INK, 1.8);
    } else {
        d.line(ml, axy, ml + pw, axy, INK, 1.8);
        d.line(axx, mt + ph, axx, mt, INK, 1.8);
    }
    // numbers + ticks
    if p.b("numbers") {
        let mut k = (x0 / xs).ceil();
        while k * xs <= x1 + 1e-9 {
            let v = k * xs;
            d.line(sx(v), axy - 3.5, sx(v), axy + 3.5, INK, 1.2);
            if !(v.abs() < 1e-9 && ax_y == 0.0 && ax_x == 0.0) { put(&mut d, sx(v), if axy + 18.0 > mt + ph + 30.0 { axy - 14.0 } else { axy + 15.0 }, &fmt(v, 3), 13.0); }
            k += 1.0;
        }
        let mut k = (y0 / ys).ceil();
        while k * ys <= y1 + 1e-9 {
            let v = k * ys;
            d.line(axx - 3.5, sy(v), axx + 3.5, sy(v), INK, 1.2);
            if !(v.abs() < 1e-9 && ax_y == 0.0 && ax_x == 0.0) { txt(&mut d, axx - 8.0, sy(v) + 4.5, 13.0, "end", &fmt(v, 3)); }
            k += 1.0;
        }
        if ax_y == 0.0 && ax_x == 0.0 { txt(&mut d, axx - 7.0, axy + 15.0, 13.0, "end", "0"); }
    }
    txt(&mut d, ml + pw + ext + 6.0, axy + 5.0, 16.0, "start", p.s("xlabel"));
    put(&mut d, axx, mt - ext - 9.0, p.s("ylabel"), 16.0);
    if !title.is_empty() { put(&mut d, w / 2.0, 18.0, title, 18.0); }
    // curves
    d.raw(&format!(r#"<clipPath id="plot"><rect x="{ml}" y="{mt}" width="{pw}" height="{ph}"/></clipPath>"#));
    for (ci, eq) in p.s("equations").split(';').enumerate() {
        let eq = eq.trim();
        if eq.is_empty() { continue; }
        let col = PALETTE[ci % PALETTE.len()];
        let rhs = match eq.find('=') { Some(i) => (eq[..i].trim().to_lowercase(), eq[i + 1..].trim().to_string()), None => ("y".into(), eq.to_string()) };
        let mut runs: Vec<Vec<Pt>> = vec![vec![]];
        if rhs.0 == "x" {
            if let Some(v) = eval(&rhs.1, 0.0) { runs[0] = vec![(sx(v), mt + ph + 5.0), (sx(v), mt - 5.0)]; }
        } else {
            let steps = 600;
            for i in 0..=steps {
                let x = x0 + (x1 - x0) * i as f64 / steps as f64;
                match eval(&rhs.1, x) {
                    Some(y) if y.is_finite() && y.abs() < 1e5 => runs.last_mut().unwrap().push((sx(x), sy(y))),
                    _ => { if !runs.last().unwrap().is_empty() { runs.push(vec![]); } }
                }
            }
        }
        d.raw(r#"<g clip-path="url(#plot)">"#);
        for r in runs.iter().filter(|r| r.len() > 1) { d.polyline(r, col, 2.6); }
        d.raw("</g>");
        if p.b("eq_labels") {
            let xl = x0 + (x1 - x0) * (0.55 + 0.08 * (ci % 3) as f64);
            let yv = if rhs.0 == "x" { y0 + (y1 - y0) * 0.9 } else { eval(&rhs.1, xl).unwrap_or(y1) };
            if rhs.0 == "x" || (yv > y0 && yv < y1) {
                let xp = if rhs.0 == "x" { eval(&rhs.1, 0.0).unwrap_or(0.0) } else { xl };
                txt(&mut d, sx(xp) + 8.0, sy(yv) - 8.0, 14.0, "start", eq);
            }
        }
    }
    // points
    for seg in p.s("points").split(';') {
        let seg = seg.trim();
        if let (Some(a), Some(b)) = (seg.find('('), seg.find(')')) {
            let n = nums(&seg[a + 1..b]);
            if n.len() == 2 {
                let (px, py) = (sx(n[0]), sy(n[1]));
                d.circle(px, py, 4.5, INK, "#fff", 1.2);
                let lab = seg[b + 1..].trim();
                if !lab.is_empty() { txt(&mut d, px + 7.0, py - 7.0, 15.0, "start", lab); }
            }
        }
    }
    d.finish()
}

// ───────────────────────── number line ─────────────────────────

#[derive(Clone, Copy)]
struct Interval { lo: f64, lo_c: bool, hi: f64, hi_c: bool }

fn parse_ineq(s: &str) -> Vec<Interval> {
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
        ParamDef::float("min", "Minimum", -1000.0, 1000.0, 1.0, -10.0),
        ParamDef::float("max", "Maximum", -1000.0, 1000.0, 1.0, 10.0),
        ParamDef::float("step", "Tick step", 0.05, 100.0, 0.5, 1.0),
        ParamDef::int("label_every", "Number every n-th tick", 1, 20, 1),
        ParamDef::int("minor", "Minor ticks between ticks (0 = none)", 0, 9, 0),
        ParamDef::bool("numbers", "Show numbers", true),
        ParamDef::bool("arrows", "Arrowheads", true),
        ParamDef::text("inequality", "Inequality to graph (e.g. -2 < x <= 5, x > 3, x < -4 or x >= 1)", "-2 < x <= 5"),
        ParamDef::text("points", "Points to mark, comma separated (append o for open, e.g. 3, -4, 5o)", ""),
        ParamDef::text("label", "Axis label", ""),
    ]
}

fn number_line_render(p: &Params) -> String {
    let (lo, mut hi) = (p.f("min"), p.f("max"));
    if hi <= lo { hi = lo + 1.0; }
    let step = p.f("step").max(1e-6);
    let (w, h) = (680.0, 120.0);
    let (x0, x1) = (40.0, w - 40.0);
    let y = 64.0;
    let sx = |v: f64| x0 + (v - lo) / (hi - lo) * (x1 - x0);
    let mut d = new_doc(w, h);
    if p.b("arrows") { d.arrow(x1 - 10.0, y, x1 + 22.0, y, INK, 1.8); d.arrow(x0 + 10.0, y, x0 - 22.0, y, INK, 1.8); }
    d.line(x0, y, x1, y, INK, 1.8);
    let k0 = (lo / step).ceil() as i64;
    let k1 = ((hi + 1e-9) / step).floor() as i64;
    let every = p.i("label_every").max(1);
    let minor = p.i("minor");
    for k in k0..=k1 {
        let v = k as f64 * step;
        d.line(sx(v), y - 8.0, sx(v), y + 8.0, INK, 1.5);
        if p.b("numbers") && (k - k0) % every == 0 { put(&mut d, sx(v), y + 24.0, &fmt(v, 3), 14.0); }
        if minor > 0 && k < k1 {
            for m in 1..=minor { let mv = v + step * m as f64 / (minor + 1) as f64; d.line(sx(mv), y - 4.0, sx(mv), y + 4.0, INK, 1.0); }
        }
    }
    if !p.s("label").is_empty() { txt(&mut d, x1 + 26.0, y + 5.0, 16.0, "start", p.s("label")); }
    // inequality
    for iv in parse_ineq(p.s("inequality")) {
        let a = if iv.lo.is_finite() { iv.lo.max(lo) } else { lo };
        let b = if iv.hi.is_finite() { iv.hi.min(hi) } else { hi };
        if b >= a && iv.hi != iv.lo { d.line(sx(a), y, sx(b), y, BLUE, 5.0); }
        if !iv.lo.is_finite() && p.b("arrows") { d.arrow(sx(lo) + 4.0, y, x0 - 24.0, y, BLUE, 3.5); }
        if !iv.hi.is_finite() && p.b("arrows") { d.arrow(x1 - 4.0, y, x1 + 24.0, y, BLUE, 3.5); }
        for (v, closed) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] {
            if v.is_finite() && v >= lo - 1e-9 && v <= hi + 1e-9 {
                d.circle(sx(v), y, 6.5, if closed { BLUE } else { "#fff" }, BLUE, 2.4);
            }
        }
    }
    for t in p.s("points").split(',') {
        let t = t.trim();
        let (open, num) = if let Some(s) = t.strip_suffix('o') { (true, s) } else { (false, t) };
        if let Ok(v) = num.trim().replace('\u{2212}', "-").parse::<f64>() {
            if v >= lo - 1e-9 && v <= hi + 1e-9 { d.circle(sx(v), y, 6.0, if open { "#fff" } else { "#c0392b" }, "#c0392b", 2.2); }
        }
    }
    d.finish()
}

// ───────────────────────── angles & parallel lines ─────────────────────────

/// Angular sector label (math degrees ccw, y-up) at screen center c.
fn sector(d: &mut Doc, c: Pt, a0: f64, a1: f64, r: f64, text: &str, right: bool) {
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
        let tr = r + 8.0 + 0.5 * (tw(text, 14.0) * mid.0.abs() + 14.0 * mid.1.abs());
        put(d, c.0 + mid.0 * tr, c.1 + mid.1 * tr, text, 14.0);
    }
}

fn angles_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("mode", "Diagram", &["Single angle", "Angles on a straight line", "Angles around a point", "Vertical angles", "Custom rays"], "Angles on a straight line"),
        ParamDef::float("angle", "Angle (degrees)", 5.0, 175.0, 1.0, 60.0),
        ParamDef::float("angle2", "Second angle (around a point)", 5.0, 175.0, 1.0, 110.0),
        ParamDef::text("rays", "Custom rays: directions in degrees counterclockwise from the baseline, e.g. 0, 50, 130, 250", "0, 50, 130, 250"),
        ParamDef::text("unknown", "Sector numbers shown as x (counterclockwise from baseline, e.g. 2)", "2"),
        ParamDef::text("hide", "Sector numbers left unlabelled", ""),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::bool("letters", "Point letters on the rays", true),
        ParamDef::int("dec", "Decimal places", 0, 3, 1),
    ]
}

fn angles_render(p: &Params) -> String {
    let (a, a2) = (p.f("angle"), p.f("angle2"));
    // (ray directions, wraps around)
    let (rays, wrap): (Vec<f64>, bool) = match p.s("mode") {
        "Single angle" => (vec![0.0, a], false),
        "Angles on a straight line" => (vec![0.0, a, 180.0], false),
        "Angles around a point" => (vec![0.0, a, (a + a2).min(358.0)], true),
        "Vertical angles" => (vec![0.0, a, 180.0, 180.0 + a], true),
        _ => {
            let mut r = nums(p.s("rays")); r.iter_mut().for_each(|x| *x = x.rem_euclid(360.0));
            r.sort_by(|x, y| x.partial_cmp(y).unwrap()); r.dedup();
            let wr = r.len() > 2 && r.last().copied().unwrap_or(0.0) - r[0] < 340.0 && p.s("mode") == "Custom rays";
            (r, wr)
        }
    };
    if rays.len() < 2 { return msg("Give at least two ray directions"); }
    let (w, h) = (480.0, 360.0);
    let c = (w / 2.0, h / 2.0);
    let len = 140.0;
    let unk: Vec<usize> = nums(p.s("unknown")).iter().map(|v| *v as usize).collect();
    let hide: Vec<usize> = nums(p.s("hide")).iter().map(|v| *v as usize).collect();
    let mut d = new_doc(w, h);
    // sector fills first
    let mut secs: Vec<(f64, f64)> = rays.windows(2).map(|w| (w[0], w[1])).collect();
    if wrap { secs.push((*rays.last().unwrap(), rays[0] + 360.0)); }
    for (a0, a1) in &secs {
        let n = 24;
        let mut pts = vec![c];
        for i in 0..=n { let t = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians(); pts.push((c.0 + 42.0 * t.cos(), c.1 - 42.0 * t.sin())); }
        d.poly(&pts, "#eef4fb", "none", 0.0);
    }
    for (i, r) in rays.iter().enumerate() {
        let t = r.to_radians();
        let e = (c.0 + len * t.cos(), c.1 - len * t.sin());
        d.arrow(c.0, c.1, e.0, e.1, INK, 2.0);
        if p.b("letters") {
            let name = ((b'B' + i as u8) as char).to_string();
            put(&mut d, c.0 + (len + 16.0) * t.cos(), c.1 - (len + 16.0) * t.sin(), &name, 16.0);
        }
    }
    d.circle(c.0, c.1, 3.0, INK, INK, 1.0);
    if p.b("letters") && !wrap { put(&mut d, c.0 - 14.0, c.1 + 16.0, "A", 16.0); }
    for (i, (a0, a1)) in secs.iter().enumerate() {
        let idx = i + 1;
        let span = a1 - a0;
        let txt = if hide.contains(&idx) { String::new() } else if unk.contains(&idx) { "x".into() } else { format!("{}\u{b0}", fmt(span, p.i("dec"))) };
        let rad = if span < 28.0 { 48.0 + (i % 2) as f64 * 10.0 } else { 36.0 + (i % 2) as f64 * 6.0 };
        sector(&mut d, c, *a0, *a1, rad, &txt, p.b("right"));
    }
    d.finish()
}

fn parallel_lines_params() -> Vec<ParamDef> {
    vec![
        ParamDef::int("lines", "Parallel lines", 2, 3, 2),
        ParamDef::float("angle", "Angle 2: above m, right of the transversal (degrees)", 15.0, 165.0, 1.0, 65.0),
        ParamDef::float("gap", "Gap between lines", 0.5, 3.0, 0.1, 1.0),
        ParamDef::choice("labels", "Label style", &["Degrees", "Numbers"], "Degrees"),
        ParamDef::text("marked", "Angles to label (1-8 at the first two crossings, 5-8 at m and n, 9-12 at the third line), e.g. 2, 6", "2, 6"),
        ParamDef::text("unknown", "Marked angles shown as x", "6"),
        ParamDef::bool("arrows", "Parallel arrows on the lines", true),
        ParamDef::bool("names", "Line names (m, n, t)", true),
        ParamDef::int("dec", "Decimal places", 0, 3, 1),
    ]
}

fn parallel_lines_render(p: &Params) -> String {
    let nl = p.i("lines").clamp(2, 3) as usize;
    let th = p.f("angle");
    let (w, h) = (520.0, 360.0 + 40.0 * (nl as f64 - 2.0));
    let gap = 120.0 * p.f("gap").clamp(0.5, 3.0);
    let ys: Vec<f64> = (0..nl).map(|i| h / 2.0 - gap * (nl as f64 - 1.0) / 2.0 + gap * i as f64).collect();
    let cx0 = w / 2.0;
    let tr = th.to_radians();
    let slope = (tr.cos(), -tr.sin()); // screen direction up-right
    let mut d = new_doc(w, h);
    let unk = nums(p.s("unknown")).iter().map(|v| *v as usize).collect::<Vec<_>>();
    let marked = nums(p.s("marked")).iter().map(|v| *v as usize).collect::<Vec<_>>();
    let names = ["m", "n", "p"];
    for (i, y) in ys.iter().enumerate() {
        d.line(30.0, *y, w - 30.0, *y, INK, 2.0);
        if p.b("arrows") {
            let c = (w - 80.0, *y);
            for dx in [0.0, 8.0] { d.polyline(&[(c.0 + dx - 5.0, c.1 - 5.0), (c.0 + dx, c.1), (c.0 + dx - 5.0, c.1 + 5.0)], INK, 1.6); }
        }
        if p.b("names") { put(&mut d, 46.0, *y - 14.0, names[i], 17.0); }
    }
    // transversal through (cx0, ys[0]) with direction slope
    let cross: Vec<Pt> = ys.iter().map(|y| { let t = (*y - ys[0]) / slope.1; (cx0 + slope.0 * t, *y) }).collect();
    let tl = 70.0 / slope.1.abs().max(0.2);
    let a = (cross[0].0 + slope.0 * tl, cross[0].1 + slope.1 * tl);
    let last = cross[nl - 1];
    let b = (last.0 - slope.0 * tl, last.1 - slope.1 * tl);
    d.line(a.0, a.1, b.0, b.1, INK, 2.0);
    if p.b("names") { put(&mut d, a.0 + 14.0, a.1 - 6.0, "t", 17.0); }
    let rad = 26.0;
    for (i, c) in cross.iter().enumerate() {
        // sectors: 1 top-left, 2 top-right, 3 bottom-right, 4 bottom-left
        let angs = [(th, 180.0), (0.0, th), (-(180.0 - th), 0.0), (180.0, 360.0 - (180.0 - th) + 0.0)];
        let _ = angs;
        let secs = [(th, 180.0, 1usize), (0.0, th, 2), (-(180.0 - th), 0.0, 3), (180.0, 180.0 + th, 4)];
        for (a0, a1, k) in secs {
            let (a0, a1) = if k == 3 { (360.0 - (180.0 - th), 360.0) } else { (a0, a1) };
            let num = i * 4 + k;
            if !marked.contains(&num) {
                if p.s("labels") == "Numbers" { sector(&mut d, *c, a0, a1, rad, &num.to_string(), false); }
                continue;
            }
            let span = a1 - a0;
            let txt = if p.s("labels") == "Numbers" { num.to_string() } else if unk.contains(&num) { "x".to_string() } else { format!("{}\u{b0}", fmt(span, p.i("dec"))) };
            let r = if span < 50.0 { rad + 8.0 } else { rad };
            sector(&mut d, *c, a0, a1, r, &txt, true);
        }
    }
    d.finish()
}

// ───────────────────────── 3D solids ─────────────────────────

type P3 = (f64, f64, f64);
fn obl(p: P3) -> Pt {
    let (k, a) = (0.5, 40f64.to_radians());
    (p.0 + p.1 * k * a.cos(), p.2 + p.1 * k * a.sin())
}
fn obl_view() -> P3 { let (k, a) = (0.5f64, 40f64.to_radians()); (k * a.cos(), -1.0, k * a.sin()) }
fn sub3(a: P3, b: P3) -> P3 { (a.0 - b.0, a.1 - b.1, a.2 - b.2) }
fn dot3(a: P3, b: P3) -> f64 { a.0 * b.0 + a.1 * b.1 + a.2 * b.2 }

struct Solid { v: Vec<P3>, faces: Vec<Vec<usize>> }

/// Draw a convex solid: shaded visible faces, solid visible edges, dashed hidden edges.
fn draw_solid(d: &mut Doc, m: &Map, s: &Solid, proj: &dyn Fn(P3) -> Pt, view: P3) {
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
            let key = (a.min(b), a.max(b));
            let e = edges.entry(key).or_insert(false);
            if vis[fi] { *e = true; }
        }
    }
    for ((a, b), visible) in &edges {
        if !*visible { dline(d, m.p(proj(s.v[*a])), m.p(proj(s.v[*b])), "#555", 1.4, "6 4"); }
    }
    for ((a, b), visible) in &edges {
        if *visible { let (p, q) = (m.p(proj(s.v[*a])), m.p(proj(s.v[*b]))); d.line(p.0, p.1, q.0, q.1, INK, 2.2); }
    }
}

fn poly_base(kind: &str, l: f64, w: f64, n: usize, side: f64) -> Vec<P3> {
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

fn dim_label(d: &mut Doc, m: &Map, proj: &dyn Fn(P3) -> Pt, a: P3, b: P3, center: Pt, text: &str, t: f64) {
    let (pa, pb) = (m.p(proj(a)), m.p(proj(b)));
    label_beside(d, pa, pb, t, center, text, 7.0);
}

fn rmark3(d: &mut Doc, m: &Map, proj: &dyn Fn(P3) -> Pt, o: P3, e1: P3, e2: P3) {
    let (po, p1, p2) = (m.p(proj(o)), m.p(proj((o.0 + e1.0, o.1 + e1.1, o.2 + e1.2))), m.p(proj((o.0 + e2.0, o.1 + e2.1, o.2 + e2.2))));
    let (u, w) = (unit_v(po, p1), unit_v(po, p2));
    let s = 11.0;
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
        ParamDef::text("hide", "Dimensions to hide (length, width, height, side, slant, radius)", ""),
    ]
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
    v
}

fn prism_render(p: &Params) -> String {
    let (l, wd, hh) = (p.f("length"), p.f("width"), p.f("height"));
    let nsides = p.i("n").clamp(3, 12) as usize;
    let base = poly_base(p.s("base"), l, wd, nsides, p.f("side"));
    let k = base.len();
    let lean = if p.s("stands") == "Oblique" { p.f("lean") } else { 0.0 };
    let mut v = base.clone();
    v.extend(base.iter().map(|q| (q.0 + lean, q.1, hh)));
    let mut faces = vec![(0..k).collect::<Vec<_>>(), (k..2 * k).collect()];
    for i in 0..k { faces.push(vec![i, (i + 1) % k, k + (i + 1) % k, k + i]); }
    let solid = Solid { v, faces };
    let proj = |q: P3| obl(q);
    let mut all: Vec<Pt> = solid.v.iter().map(|q| proj(*q)).collect();
    let cenp = (all.iter().map(|q| q.0).sum::<f64>() / all.len() as f64, all.iter().map(|q| q.1).sum::<f64>() / all.len() as f64);
    let foot = (lean, 0.0, 0.0);
    all.push(proj(foot));
    let m = Map::fit(&all, 480.0, 360.0, 70.0, 50.0);
    let mut d = new_doc(480.0, 360.0);
    if p.b("hidden") { draw_solid(&mut d, &m, &solid, &proj, obl_view()); }
    else { // hide dashed: draw with only visible edges by drawing solid then covering nothing
        draw_solid(&mut d, &m, &solid, &proj, obl_view());
    }
    let (unit, dec) = (p.s("unit").to_string(), p.i("dec"));
    let hide = tokens(p.s("hide")).iter().map(|s| s.to_lowercase()).collect::<Vec<_>>();
    let mc = m.p(cenp);
    let b = &base;
    let show = |n: &str| !hide.iter().any(|h| h == n);
    let kind = p.s("base");
    if show("length") && kind != "Regular polygon" { dim_label(&mut d, &m, &proj, b[0], b[1], mc, &fmtu(l, dec, &unit), 0.5); }
    if show("side") && kind == "Regular polygon" { dim_label(&mut d, &m, &proj, b[0], b[1], mc, &fmtu(p.f("side"), dec, &unit), 0.5); }
    if show("width") {
        match kind {
            "Rectangle" | "Right triangle" => dim_label(&mut d, &m, &proj, b[1], b[2], mc, &fmtu(wd, dec, &unit), 0.5),
            "Isosceles triangle" => {
                let (f0, f1) = ((l / 2.0, 0.0, 0.0), (l / 2.0, wd, 0.0));
                dline(&mut d, m.p(proj(f0)), m.p(proj(f1)), "#444", 1.4, "6 4");
                dim_label(&mut d, &m, &proj, f0, f1, mc, &fmtu(wd, dec, &unit), 0.5);
            }
            _ => {}
        }
    }
    if show("height") {
        if lean.abs() < 1e-9 {
            let vi = (0..k).min_by(|x, y| proj(b[*x]).0.partial_cmp(&proj(b[*y]).0).unwrap()).unwrap_or(0);
            dim_label(&mut d, &m, &proj, b[vi], (b[vi].0, b[vi].1, hh), mc, &fmtu(hh, dec, &unit), 0.5);
            if p.b("right") && matches!(kind, "Rectangle" | "Right triangle") { rmark3(&mut d, &m, &proj, b[0], (1.0 * l.min(hh) * 0.0 + 0.001, 0.0, 0.0), (0.0, 0.0, 0.001)); }
        } else {
            let top = (lean, 0.0, hh);
            dline(&mut d, m.p(proj(top)), m.p(proj(foot)), "#444", 1.4, "6 4");
            if lean < 0.0 || lean > l { dline(&mut d, m.p(proj(b[0])), m.p(proj(foot)), "#444", 1.4, "6 4"); }
            dim_label(&mut d, &m, &proj, foot, top, mc, &fmtu(hh, dec, &unit), 0.5);
            if p.b("right") { rmark3(&mut d, &m, &proj, foot, (-lean.signum().max(-1.0).min(1.0) * 0.0 - 1.0, 0.0, 0.0), (0.0, 0.0, 1.0)); }
        }
    }
    d.finish()
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
        ParamDef::bool("show_slant", "Draw the slant height (apex to the middle of the front base edge)", false),
    ];
    v.extend(solid_common("cm"));
    v
}

fn pyramid_render(p: &Params) -> String {
    let (l, wd, hh) = (p.f("length"), p.f("width"), p.f("height"));
    let kind = p.s("base");
    let base = poly_base(kind, l, wd, p.i("n").clamp(3, 12) as usize, p.f("side"));
    let k = base.len();
    let c = (base.iter().map(|q| q.0).sum::<f64>() / k as f64, base.iter().map(|q| q.1).sum::<f64>() / k as f64);
    let lean = if p.s("stands") == "Oblique" { p.f("lean") } else { 0.0 };
    let apex = (c.0 + lean, c.1, hh);
    let mut v = base.clone();
    v.push(apex);
    let mut faces = vec![(0..k).collect::<Vec<_>>()];
    for i in 0..k { faces.push(vec![i, (i + 1) % k, k]); }
    let solid = Solid { v, faces };
    let proj = |q: P3| obl(q);
    let all: Vec<Pt> = solid.v.iter().map(|q| proj(*q)).collect();
    let cenp = (all.iter().map(|q| q.0).sum::<f64>() / all.len() as f64, all.iter().map(|q| q.1).sum::<f64>() / all.len() as f64);
    let m = Map::fit(&all, 480.0, 360.0, 70.0, 50.0);
    let mut d = new_doc(480.0, 360.0);
    draw_solid(&mut d, &m, &solid, &proj, obl_view());
    let (unit, dec) = (p.s("unit").to_string(), p.i("dec"));
    let hide = tokens(p.s("hide")).iter().map(|s| s.to_lowercase()).collect::<Vec<_>>();
    let show = |n: &str| !hide.iter().any(|h| h == n);
    let mc = m.p(cenp);
    let foot = (c.0, c.1, 0.0);
    if p.b("show_height") {
        dline(&mut d, m.p(proj(apex)), m.p(proj((apex.0, apex.1, 0.0))), "#444", 1.5, "6 4");
        if lean.abs() > 1e-9 { dline(&mut d, m.p(proj(foot)), m.p(proj((apex.0, apex.1, 0.0))), "#444", 1.5, "6 4"); }
        if show("height") { dim_label(&mut d, &m, &proj, (apex.0, apex.1, 0.0), apex, mc, &fmtu(hh, dec, &unit), 0.5); }
        if p.b("right") { rmark3(&mut d, &m, &proj, (apex.0, apex.1, 0.0), (-1.0, 0.0, 0.0), (0.0, 0.0, 1.0)); }
    }
    if p.b("show_slant") {
        let mid = ((base[0].0 + base[1].0) / 2.0, (base[0].1 + base[1].1) / 2.0, 0.0);
        dline(&mut d, m.p(proj(mid)), m.p(proj(apex)), "#444", 1.5, "6 4");
        let sl = dist3(mid, apex);
        if show("slant") { dim_label(&mut d, &m, &proj, mid, apex, mc, &fmtu(sl, dec, &unit), 0.5); }
    }
    if kind == "Regular polygon" {
        if show("side") { dim_label(&mut d, &m, &proj, base[0], base[1], mc, &fmtu(p.f("side"), dec, &unit), 0.5); }
    } else {
        if show("length") { dim_label(&mut d, &m, &proj, base[0], base[1], mc, &fmtu(l, dec, &unit), 0.5); }
        if show("width") { dim_label(&mut d, &m, &proj, base[1], base[2], mc, &fmtu(wd, dec, &unit), 0.5); }
    }
    d.finish()
}
fn dist3(a: P3, b: P3) -> f64 { dot3(sub3(a, b), sub3(a, b)).sqrt() }

// ----- round solids (elevated orthographic view) -----

fn ell(cx: f64, cy: f64, rx: f64, ry: f64, t0: f64, t1: f64) -> Vec<Pt> {
    let n = 64;
    (0..=n).map(|i| { let t = (t0 + (t1 - t0) * i as f64 / n as f64).to_radians(); (cx + rx * t.cos(), cy + ry * t.sin()) }).collect()
}
fn mpts(m: &Map, pts: &[Pt]) -> Vec<Pt> { pts.iter().map(|q| m.p(*q)).collect() }

fn round_common() -> Vec<ParamDef> {
    vec![
        ParamDef::float("view", "View elevation (degrees above the horizon)", 8.0, 60.0, 1.0, 22.0),
        ParamDef::choice("measure", "Measure across the base", &["Radius", "Diameter"], "Radius"),
        ParamDef::bool("hidden", "Show hidden edges (dashed)", true),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::text("unit", "Unit", "cm"),
        ParamDef::int("dec", "Decimal places", 0, 3, 1),
    ]
}

fn base_measure(d: &mut Doc, m: &Map, o: Pt, r: f64, p: &Params) {
    let (unit, dec) = (p.s("unit"), p.i("dec"));
    let c = m.p(o);
    if p.s("measure") == "Diameter" {
        let (a, b) = (m.p((o.0 - r, o.1)), m.p((o.0 + r, o.1)));
        dline(d, a, b, "#444", 1.5, "6 4");
        put(d, c.0, c.1 + 12.0 + 1.0, &format!("d = {}", fmtu(2.0 * r, dec, unit)), FONT);
    } else {
        let b = m.p((o.0 + r, o.1));
        dline(d, c, b, "#444", 1.5, "6 4");
        put(d, (c.0 + b.0) / 2.0, c.1 + 12.0, &format!("r = {}", fmtu(r, dec, unit)), FONT);
    }
    d.circle(c.0, c.1, 2.8, INK, INK, 1.0);
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
    v
}

fn cylinder_render(p: &Params) -> String {
    let (r, h) = (p.f("radius"), p.f("height"));
    let (s, c) = (p.f("view").to_radians().sin(), p.f("view").to_radians().cos());
    let lean = if p.s("stands") == "Oblique" { p.f("lean") } else { 0.0 };
    let ht = h * c;
    let mut all = ell(0.0, 0.0, r, r * s, 0.0, 360.0);
    all.extend(ell(lean, ht, r, r * s, 0.0, 360.0));
    let m = Map::fit(&all, 480.0, 360.0, 70.0, 48.0);
    let mut d = new_doc(480.0, 360.0);
    // body fill
    let mut body = ell(0.0, 0.0, r, r * s, 180.0, 360.0);
    body.extend(ell(lean, ht, r, r * s, 0.0, 180.0).into_iter().rev().collect::<Vec<_>>());
    let mut body2 = ell(0.0, 0.0, r, r * s, 180.0, 360.0);
    body2.extend(ell(lean, ht, r, r * s, 360.0, 180.0));
    let _ = body;
    d.poly(&mpts(&m, &body2), "#d9e7f6", "none", 0.0);
    d.poly(&mpts(&m, &ell(lean, ht, r, r * s, 0.0, 360.0)), "#f4f9ff", INK, 2.2);
    let (l0, l1, r0, r1) = (m.p((-r, 0.0)), m.p((lean - r, ht)), m.p((r, 0.0)), m.p((lean + r, ht)));
    d.line(l0.0, l0.1, l1.0, l1.1, INK, 2.2);
    d.line(r0.0, r0.1, r1.0, r1.1, INK, 2.2);
    d.polyline(&mpts(&m, &ell(0.0, 0.0, r, r * s, 180.0, 360.0)), INK, 2.2);
    if p.b("hidden") { dpoly(&mut d, &mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 180.0)), "#555", 1.4, "6 4"); }
    base_measure(&mut d, &m, (0.0, 0.0), r, p);
    if p.b("show_height") {
        let (t, b) = (m.p((lean, ht)), m.p((lean, 0.0)));
        dline(&mut d, t, b, "#444", 1.5, "6 4");
        d.circle(t.0, t.1, 2.8, INK, INK, 1.0);
        let txt = fmtu(h, p.i("dec"), p.s("unit"));
        put(&mut d, t.0 - 12.0 - tw(&txt, FONT) / 2.0, (t.1 + b.1) / 2.0, &txt, FONT);
        if lean.abs() > 1e-9 { dline(&mut d, m.p((0.0, 0.0)), b, "#444", 1.5, "6 4"); }
        if p.b("right") && p.s("measure") == "Radius" { square_mark(&mut d, b, (b.0, b.1 - 10.0), (b.0 + 10.0, b.1), 10.0); }
    }
    d.finish()
}

fn cone_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("stands", "Stands", &["Right", "Oblique"], "Right"),
        ParamDef::float("lean", "Lean of the apex when oblique", -20.0, 20.0, 0.5, 2.0),
        ParamDef::float("radius", "Radius", 0.5, 50.0, 0.5, 3.0),
        ParamDef::float("height", "Height", 0.5, 50.0, 0.5, 4.0),
        ParamDef::bool("show_height", "Draw the height", true),
        ParamDef::bool("show_slant", "Label the slant height", true),
    ];
    v.extend(round_common());
    v
}

fn cone_render(p: &Params) -> String {
    let (r, h) = (p.f("radius"), p.f("height"));
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
    // front arc from left tangent through the bottom (270deg) to right tangent
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
    base_measure(&mut d, &m, (0.0, 0.0), r, p);
    let (unit, dec) = (p.s("unit"), p.i("dec"));
    if p.b("show_height") {
        let (t, b) = (m.p(apex), m.p((lean, 0.0)));
        dline(&mut d, t, b, "#444", 1.5, "6 4");
        let txt = fmtu(h, dec, unit);
        put(&mut d, t.0 - 12.0 - tw(&txt, FONT) / 2.0, (t.1 + b.1) / 2.0 - 8.0, &txt, FONT);
        if lean.abs() > 1e-9 { dline(&mut d, m.p((0.0, 0.0)), b, "#444", 1.5, "6 4"); }
        if p.b("right") && p.s("measure") == "Radius" && lean.abs() < 1e-9 { square_mark(&mut d, b, (b.0, b.1 - 10.0), (b.0 + 10.0, b.1), 10.0); }
    }
    if p.b("show_slant") {
        let sl = ((r - lean).powi(2) + h * h).sqrt();
        let (a, b) = (m.p(apex), m.p(pr));
        label_beside(&mut d, a, b, 0.5, m.p((lean * 0.5, h * c * 0.4)), &format!("l = {}", fmtu(sl, dec, unit)), 6.0);
    }
    d.finish()
}

fn sphere_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("draw", "Draw", &["A whole sphere", "A hemisphere"], "A whole sphere"),
        ParamDef::float("radius", "Radius", 0.5, 50.0, 0.5, 3.0),
    ];
    v.extend(round_common());
    v
}

fn sphere_render(p: &Params) -> String {
    let r = p.f("radius");
    let s = p.f("view").to_radians().sin();
    let half = p.s("draw") == "A hemisphere";
    let (w, h) = (480.0, 360.0);
    let mut all = ell(0.0, 0.0, r, r, 0.0, 360.0);
    if half { all = ell(0.0, 0.0, r, r, 0.0, 180.0); all.extend(ell(0.0, 0.0, r, r * s, 180.0, 360.0)); }
    let m = Map::fit(&all, w, h, 90.0, 50.0);
    let mut d = new_doc(w, h);
    if half {
        let mut sil = ell(0.0, 0.0, r, r, 0.0, 180.0);
        sil.extend(ell(0.0, 0.0, r, r * s, 180.0, 360.0));
        d.poly(&mpts(&m, &sil), "#e3eef9", INK, 2.2);
        if p.b("hidden") { dpoly(&mut d, &mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 180.0)), "#555", 1.4, "6 4"); }
    } else {
        d.poly(&mpts(&m, &ell(0.0, 0.0, r, r, 0.0, 360.0)), "#e3eef9", INK, 2.2);
        d.polyline(&mpts(&m, &ell(0.0, 0.0, r, r * s, 180.0, 360.0)), "#333", 1.4);
        if p.b("hidden") { dpoly(&mut d, &mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 180.0)), "#555", 1.4, "6 4"); }
    }
    base_measure(&mut d, &m, (0.0, 0.0), r, p);
    d.finish()
}

// ───────────────────────── box plot ─────────────────────────

fn median(v: &[f64]) -> f64 {
    let n = v.len();
    if n == 0 { 0.0 } else if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}
fn five_number(data: &[f64]) -> [f64; 5] {
    let mut v = data.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    let (lo, hi) = (&v[..n / 2], &v[(n + 1) / 2..]);
    [v[0], median(lo), median(&v), median(hi), v[n - 1]]
}
fn nice_step(range: f64) -> f64 {
    let raw = range / 8.0;
    let mag = 10f64.powf(raw.log10().floor());
    for f in [1.0, 2.0, 5.0, 10.0] { if raw <= f * mag { return f * mag; } }
    10.0 * mag
}

fn box_plot_params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("data", "Data set 1 (numbers separated by commas)", "2, 4, 5, 5, 6, 7, 8, 9, 12"),
        ParamDef::text("summary", "Or five-number summary instead: min, Q1, median, Q3, max", ""),
        ParamDef::text("name1", "Name of set 1", ""),
        ParamDef::text("data2", "Data set 2 (optional, to compare)", ""),
        ParamDef::text("summary2", "Or five-number summary for set 2", ""),
        ParamDef::text("name2", "Name of set 2", ""),
        ParamDef::bool("values", "Write the five values", true),
        ParamDef::bool("outliers", "Mark outliers (beyond 1.5 x IQR)", false),
        ParamDef::bool("auto", "Choose the number line automatically", true),
        ParamDef::float("axis_min", "Number line minimum (if not automatic)", -1000.0, 1000.0, 1.0, 0.0),
        ParamDef::float("axis_max", "Number line maximum (if not automatic)", -1000.0, 1000.0, 1.0, 14.0),
        ParamDef::float("axis_step", "Number line step (if not automatic)", 0.05, 100.0, 0.5, 1.0),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("axis_title", "Number line label", ""),
    ]
}

fn summary_for(data: &str, summ: &str, outliers: bool) -> Option<([f64; 5], Vec<f64>)> {
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

fn box_plot_render(p: &Params) -> String {
    let mut sets = vec![];
    if let Some(s) = summary_for(p.s("data"), p.s("summary"), p.b("outliers")) { sets.push((s, p.s("name1"))); }
    if let Some(s) = summary_for(p.s("data2"), p.s("summary2"), p.b("outliers")) { sets.push((s, p.s("name2"))); }
    if sets.is_empty() { return msg("Enter some data or a five-number summary"); }
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for ((f, o), _) in &sets { for v in f.iter().chain(o.iter()) { lo = lo.min(*v); hi = hi.max(*v); } }
    let (amin, amax, step) = if p.b("auto") {
        let st = nice_step((hi - lo).max(1e-6));
        ((lo / st).floor() * st - if lo % st == 0.0 { st } else { 0.0 }, (hi / st).ceil() * st + if hi % st == 0.0 { st } else { 0.0 }, st)
    } else { (p.f("axis_min"), p.f("axis_max").max(p.f("axis_min") + 1e-3), p.f("axis_step").max(1e-3)) };
    let named = sets.iter().any(|s| !s.1.is_empty());
    let left = if named { 110.0 } else { 40.0 };
    let (w, rowh) = (680.0, 84.0);
    let title = p.s("title");
    let top = if title.is_empty() { 20.0 } else { 46.0 };
    let h = top + rowh * sets.len() as f64 + 62.0 + if p.s("axis_title").is_empty() { 0.0 } else { 22.0 };
    let (x0, x1) = (left, w - 36.0);
    let sx = |v: f64| x0 + (v - amin) / (amax - amin) * (x1 - x0);
    let mut d = new_doc(w, h);
    if !title.is_empty() { put(&mut d, w / 2.0, 22.0, title, 18.0); }
    let ay = top + rowh * sets.len() as f64 + 6.0;
    d.line(x0, ay, x1, ay, INK, 1.8);
    let k0 = (amin / step).ceil() as i64;
    let k1 = ((amax + 1e-9) / step).floor() as i64;
    for k in k0..=k1 { let v = k as f64 * step; d.line(sx(v), ay, sx(v), ay + 7.0, INK, 1.4); put(&mut d, sx(v), ay + 21.0, &fmt(v, 3), 13.0); }
    if !p.s("axis_title").is_empty() { put(&mut d, (x0 + x1) / 2.0, ay + 46.0, p.s("axis_title"), 15.0); }
    for (i, ((f, outl), name)) in sets.iter().enumerate() {
        let cy = top + rowh * (i as f64 + 0.5) + 6.0;
        let bh = 36.0;
        d.line(sx(f[0]), cy, sx(f[1]), cy, INK, 2.0);
        d.line(sx(f[3]), cy, sx(f[4]), cy, INK, 2.0);
        d.line(sx(f[0]), cy - 11.0, sx(f[0]), cy + 11.0, INK, 2.0);
        d.line(sx(f[4]), cy - 11.0, sx(f[4]), cy + 11.0, INK, 2.0);
        d.rect(sx(f[1]), cy - bh / 2.0, (sx(f[3]) - sx(f[1])).max(1.0), bh, TINT, INK, 2.0);
        d.line(sx(f[2]), cy - bh / 2.0, sx(f[2]), cy + bh / 2.0, INK, 3.4);
        for o in outl { d.circle(sx(*o), cy, 3.6, "#fff", INK, 1.8); }
        if !name.is_empty() { txt(&mut d, x0 - 14.0, cy + 5.0, 15.0, "end", name); }
        if p.b("values") {
            let mut last_x = -1e9;
            let mut row = 0;
            for v in f {
                let x = sx(*v);
                if x - last_x < 24.0 { row = 1 - row; } else { row = 0; }
                last_x = x;
                put(&mut d, x, cy - bh / 2.0 - 9.0 - row as f64 * 14.0 - 2.0, &fmt(*v, 2), 12.0);
            }
        }
    }
    d.finish()
}

// ───────────────────────── mapping diagram ─────────────────────────

fn list_items(s: &str) -> Vec<String> {
    s.split(|c| c == ',' || c == ';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

fn mapping_diagram_params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("inputs", "Inputs, comma separated (order is top to bottom)", "1, 2, 3, 4"),
        ParamDef::text("outputs", "Outputs, comma separated", "3, 5, 7, 9"),
        ParamDef::text("pairs", "Arrows as ordered pairs, e.g. (1,3); (2,5)  or  1->3; 2->5", "(1,3); (2,5); (3,7); (4,9)"),
        ParamDef::text("title_in", "Input title", "Input"),
        ParamDef::text("title_out", "Output title", "Output"),
        ParamDef::choice("shape", "Sides", &["Ovals", "Rectangles", "None"], "Ovals"),
    ]
}

fn mapping_diagram_render(p: &Params) -> String {
    let mut ins = list_items(p.s("inputs"));
    let mut outs = list_items(p.s("outputs"));
    let ps = p.s("pairs");
    let raw: Vec<String> = if ps.contains('(') {
        ps.split('(').skip(1).map(|s| s.split(')').next().unwrap_or("").to_string()).collect()
    } else { ps.split(';').map(|s| s.to_string()).collect() };
    let mut pairs = vec![];
    for r in raw {
        let seg = if let Some(i) = r.find("->") { Some((r[..i].to_string(), r[i + 2..].to_string())) }
            else if let Some(i) = r.find('\u{2192}') { Some((r[..i].to_string(), r[i + 3..].to_string())) }
            else { r.split_once(',').map(|(a, b)| (a.to_string(), b.to_string())) };
        if let Some((a, b)) = seg {
            let (a, b) = (a.trim().to_string(), b.trim().to_string());
            if a.is_empty() || b.is_empty() { continue; }
            if !ins.contains(&a) { ins.push(a.clone()); }
            if !outs.contains(&b) { outs.push(b.clone()); }
            pairs.push((a, b));
        }
    }
    if ins.is_empty() && outs.is_empty() { return msg("Enter inputs and outputs"); }
    let rows = ins.len().max(outs.len()).max(1) as f64;
    let spacing = 42.0;
    let h = 100.0 + rows * spacing;
    let w = 480.0;
    let (lx, rx) = (130.0, 350.0);
    let cy = 66.0 + rows * spacing / 2.0;
    let ry = rows * spacing / 2.0 + 14.0;
    let mut d = new_doc(w, h);
    let shape = p.s("shape");
    let draw_side = |d: &mut Doc, x: f64, n: usize| {
        let ry = (n.max(1) as f64) * spacing / 2.0 + 14.0;
        use std::fmt::Write;
        let mut s = String::new();
        match shape {
            "Ovals" => { let _ = write!(s, r##"<ellipse cx="{x}" cy="{cy}" rx="62" ry="{ry}" fill="{TINT}" stroke="{INK}" stroke-width="2"/>"##); }
            "Rectangles" => { let _ = write!(s, r##"<rect x="{}" y="{}" width="124" height="{}" rx="4" fill="{TINT}" stroke="{INK}" stroke-width="2"/>"##, x - 62.0, cy - ry, 2.0 * ry); }
            _ => {}
        }
        d.raw(&s);
    };
    let _ = ry;
    draw_side(&mut d, lx, ins.len());
    draw_side(&mut d, rx, outs.len());
    put(&mut d, lx, 28.0, p.s("title_in"), 17.0);
    put(&mut d, rx, 28.0, p.s("title_out"), 17.0);
    let pos = |i: usize, n: usize| cy - (n as f64 - 1.0) * spacing / 2.0 + i as f64 * spacing;
    for (a, b) in &pairs {
        let (i, j) = (ins.iter().position(|x| x == a).unwrap(), outs.iter().position(|x| x == b).unwrap());
        let (ya, yb) = (pos(i, ins.len()), pos(j, outs.len()));
        d.arrow(lx + 26.0, ya, rx - 28.0, yb, "#333", 1.8);
    }
    for (i, t) in ins.iter().enumerate() { put(&mut d, lx, pos(i, ins.len()), t, 17.0); }
    for (j, t) in outs.iter().enumerate() { put(&mut d, rx, pos(j, outs.len()), t, 17.0); }
    d.finish()
}

// ───────────────────────── length reading (ruler) ─────────────────────────

fn length_reading_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("marks", "Smallest marks", &["1 cm", "5 mm", "1 mm", "1 in", "1/2 in", "1/4 in", "1/8 in", "1/16 in"], "1 mm"),
        ParamDef::int("ruler", "Ruler length (cm or in)", 3, 30, 15),
        ParamDef::choice("object", "Object", &["Bar", "Pencil", "None"], "Bar"),
        ParamDef::float("start", "Left end of the object (on the ruler)", 0.0, 100.0, 0.05, 0.0),
        ParamDef::float("length", "Length of the object", 0.1, 100.0, 0.01, 4.37),
        ParamDef::bool("guides", "Dotted guides from the object's ends to the ruler", true),
        ParamDef::bool("show_value", "Write the length above the object", false),
        ParamDef::int("dec", "Decimals in the written length", 0, 3, 2),
    ]
}

fn length_reading_render(p: &Params) -> String {
    let marks = p.s("marks");
    let metric = marks.ends_with("cm") || marks.ends_with("mm");
    let sub: u32 = match marks { "1 cm" => 1, "5 mm" => 2, "1 mm" => 10, "1 in" => 1, "1/2 in" => 2, "1/4 in" => 4, "1/8 in" => 8, _ => 16 };
    let len = p.i("ruler").max(1) as f64;
    let (w, h) = (680.0, 190.0);
    let (x0, x1) = (30.0, w - 30.0);
    let ppu = (x1 - x0) / len;
    let ry = 96.0;
    let mut d = new_doc(w, h);
    d.rect(x0 - 8.0, ry, x1 - x0 + 16.0, 62.0, "#fbf1c7", INK, 1.8);
    // tick spacing guard
    let mut sub_eff = sub;
    while ppu / sub_eff as f64 <= 3.2 && sub_eff > 1 { sub_eff = if metric { if sub_eff == 10 { 5 } else if sub_eff == 5 { 1 } else { 1 } } else { sub_eff / 2 }; }
    let total = (len * sub_eff as f64).round() as i64;
    let label_every = if ppu >= 22.0 { 1 } else if ppu >= 9.0 { 5 } else { 10 };
    let lg = (sub_eff as f64).log2();
    for i in 0..=total {
        let x = x0 + i as f64 * ppu / sub_eff as f64;
        let th = if metric {
            if sub_eff == 10 { if i % 10 == 0 { 26.0 } else if i % 5 == 0 { 18.0 } else { 11.0 } }
            else if sub_eff == 2 { if i % 2 == 0 { 26.0 } else { 17.0 } } else { 26.0 }
        } else {
            let tz = if i == 0 { lg } else { (i as u64).trailing_zeros() as f64 };
            (26.0 - 4.5 * (lg - tz.min(lg))).max(7.0)
        };
        d.line(x, ry, x, ry + th, INK, if th >= 26.0 { 1.6 } else { 1.1 });
        if i % sub_eff as i64 == 0 {
            let u = i / sub_eff as i64;
            if u % label_every == 0 { put(&mut d, x, ry + 42.0, &u.to_string(), 14.0); }
        }
    }
    txt(&mut d, x0 + 2.0, ry + 56.0, 12.0, "start", if metric { "cm" } else { "in" });
    // object
    let (s, l) = (p.f("start"), p.f("length"));
    let (ox0, ox1) = (x0 + s * ppu, (x0 + (s + l) * ppu).min(x1 + 8.0));
    match p.s("object") {
        "Bar" => { d.rect(ox0, 42.0, ox1 - ox0, 26.0, "#c9def4", INK, 1.8); }
        "Pencil" => {
            let tip = 16.0f64.min((ox1 - ox0) * 0.4);
            d.poly(&[(ox0, 42.0), (ox1 - tip, 42.0), (ox1, 55.0), (ox1 - tip, 68.0), (ox0, 68.0)], "#f3c969", INK, 1.8);
            d.poly(&[(ox1 - tip, 42.0), (ox1, 55.0), (ox1 - tip, 68.0)], "#f1dcb4", INK, 1.8);
            d.line(ox0 + 22.0, 42.0, ox0 + 22.0, 68.0, INK, 1.3);
            d.rect(ox0, 42.0, 22.0, 26.0, "#e48a9a", INK, 1.6);
        }
        _ => {}
    }
    if p.s("object") != "None" {
        if p.b("guides") {
            dline(&mut d, (ox0, 68.0), (ox0, ry), "#555", 1.2, "2 3");
            if p.s("object") == "Bar" { dline(&mut d, (ox1, 68.0), (ox1, ry), "#555", 1.2, "2 3"); }
            else { dline(&mut d, (ox1, 55.0), (ox1, ry), "#555", 1.2, "2 3"); }
        }
        if p.b("show_value") { put(&mut d, (ox0 + ox1) / 2.0, 26.0, &fmtu(l, p.i("dec"), if metric { "cm" } else { "in" }), 16.0); }
    }
    d.finish()
}

// ───────────────────────── composite shape ─────────────────────────

enum Piece { Poly(Vec<Pt>) }

fn composite_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("preset", "Start from", &["Custom (use the text below)", "L-shape", "House", "Arch", "T-shape"], "Custom (use the text below)"),
        ParamDef::text("spec",
            "Pieces separated by ;  rect X Y W H | tri X1 Y1 X2 Y2 X3 Y3 | poly X Y X Y ... | circle CX CY R | semi CX CY R DIR(u/d/l/r bulge) | dash X1 Y1 X2 Y2 | dim X1 Y1 X2 Y2 TEXT (label drawn to the right of travel, go counterclockwise) | text X Y WORDS | ra X Y NE/NW/SE/SW (right angle)",
            "rect 0 0 6 4; semi 3 4 3 u; dash 3 4 6 4; text 4.5 4.6 r = 3; dim 0 0 6 0 6 cm; dim 6 0 6 4 4 cm; ra 0 0 NE; ra 6 0 NW"),
        ParamDef::bool("shade", "Shade the shape", true),
    ]
}

fn preset_spec(name: &str) -> Option<&'static str> {
    Some(match name {
        "L-shape" => "rect 0 0 8 3; rect 0 3 3 5; dim 0 0 8 0 8 cm; dim 8 0 8 3 3 cm; dim 8 3 3 3 5 cm; dim 3 3 3 8 5 cm; dim 3 8 0 8 3 cm; dim 0 8 0 0 8 cm; ra 0 0 NE; ra 8 0 NW; ra 0 8 SE",
        "House" => "rect 0 0 6 4; tri 0 4 6 4 3 7; dash 3 4 3 7; text 3.5 5.4 3 cm; dim 0 0 6 0 6 cm; dim 0 4 0 0 4 cm; ra 0 0 NE; ra 6 0 NW",
        "Arch" => "rect 0 0 6 4; semi 3 4 3 u; dash 3 4 6 4; text 4.5 4.6 r = 3; dim 0 0 6 0 6 cm; dim 6 0 6 4 4 cm; ra 0 0 NE; ra 6 0 NW",
        "T-shape" => "rect 3 0 3 6; rect 0 6 9 3; dim 3 0 6 0 3 cm; dim 6 0 6 6 6 cm; dim 9 9 0 9 9 cm; dim 9 6 9 9 3 cm; ra 3 0 NE; ra 6 0 NW",
        _ => return None,
    })
}

fn composite_render(p: &Params) -> String {
    let spec = preset_spec(p.s("preset")).unwrap_or(p.s("spec")).to_string();
    let mut pieces: Vec<Piece> = vec![];
    let mut dashes: Vec<(Pt, Pt)> = vec![];
    let mut dims: Vec<(Pt, Pt, String)> = vec![];
    let mut texts: Vec<(Pt, String)> = vec![];
    let mut rights: Vec<(Pt, String)> = vec![];
    for cmd in spec.split(|c| c == ';' || c == '\n') {
        let tk: Vec<&str> = cmd.split_whitespace().collect();
        if tk.is_empty() { continue; }
        let num = |i: usize| tk.get(i).and_then(|s| s.replace('\u{2212}', "-").parse::<f64>().ok());
        match tk[0].to_lowercase().as_str() {
            "rect" => if let (Some(x), Some(y), Some(w), Some(h)) = (num(1), num(2), num(3), num(4)) { pieces.push(Piece::Poly(vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)])); },
            "tri" => if let (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f)) = (num(1), num(2), num(3), num(4), num(5), num(6)) { pieces.push(Piece::Poly(vec![(a, b), (c, d), (e, f)])); },
            "poly" => { let v: Vec<f64> = tk[1..].iter().filter_map(|s| s.parse().ok()).collect(); if v.len() >= 6 { pieces.push(Piece::Poly(v.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect())); } }
            "circle" => if let (Some(x), Some(y), Some(r)) = (num(1), num(2), num(3)) { pieces.push(Piece::Poly(ell(x, y, r, r, 0.0, 360.0))); },
            "semi" => if let (Some(x), Some(y), Some(r)) = (num(1), num(2), num(3)) {
                let (a0, a1) = match tk.get(4).map(|s| s.to_lowercase()).as_deref() { Some("d") => (180.0, 360.0), Some("l") => (90.0, 270.0), Some("r") => (-90.0, 90.0), _ => (0.0, 180.0) };
                pieces.push(Piece::Poly(ell(x, y, r, r, a0, a1)));
            },
            "dash" => if let (Some(a), Some(b), Some(c), Some(e)) = (num(1), num(2), num(3), num(4)) { dashes.push(((a, b), (c, e))); },
            "dim" => if let (Some(a), Some(b), Some(c), Some(e)) = (num(1), num(2), num(3), num(4)) { dims.push(((a, b), (c, e), tk[5..].join(" "))); },
            "text" => if let (Some(a), Some(b)) = (num(1), num(2)) { texts.push(((a, b), tk[3..].join(" "))); },
            "ra" => if let (Some(a), Some(b)) = (num(1), num(2)) { rights.push(((a, b), tk.get(3).unwrap_or(&"NE").to_uppercase())); },
            _ => {}
        }
    }
    if pieces.is_empty() { return msg("No pieces: start the text with rect, tri, circle, semi or poly"); }
    let mut all: Vec<Pt> = vec![];
    for Piece::Poly(v) in &pieces { all.extend(v.iter().copied()); }
    for (a, b) in &dashes { all.push(*a); all.push(*b); }
    for (a, b, _) in &dims { all.push(*a); all.push(*b); }
    let (w, h) = (480.0, 360.0);
    let m = Map::fit(&all, w, h, 70.0, 56.0);
    let mut d = new_doc(w, h);
    let fill = if p.b("shade") { TINT } else { "#fff" };
    for Piece::Poly(v) in &pieces { d.poly(&mpts(&m, v), INK, INK, 5.0); }
    for Piece::Poly(v) in &pieces { d.poly(&mpts(&m, v), fill, fill, 1.2); }
    for (a, b) in &dashes { dline(&mut d, m.p(*a), m.p(*b), "#444", 1.6, "6 4"); }
    for (a, dirs) in &rights {
        let (sx, sy) = (if dirs.contains('W') { -1.0 } else { 1.0 }, if dirs.contains('S') { 1.0 } else { -1.0 });
        let v = m.p(*a);
        d.polyline(&[(v.0 + sx * 12.0, v.1), (v.0 + sx * 12.0, v.1 + sy * 12.0), (v.0, v.1 + sy * 12.0)], INK, 1.3);
    }
    let bc = m.p((all.iter().map(|q| q.0).sum::<f64>() / all.len() as f64, all.iter().map(|q| q.1).sum::<f64>() / all.len() as f64));
    let _ = bc;
    for (a, b, t) in &dims {
        let (pa, pb) = (m.p(*a), m.p(*b));
        let u = unit_v(pa, pb);
        let nr = (u.1 * -1.0 * -1.0, -u.0); // right of travel in screen coords
        let _ = nr;
        let right = (-u.1, u.0);
        let off = 16.0;
        let (qa, qb) = ((pa.0 + right.0 * off, pa.1 + right.1 * off), (pb.0 + right.0 * off, pb.1 + right.1 * off));
        d.line(qa.0, qa.1, qb.0, qb.1, GREY, 1.3);
        for q in [qa, qb] { d.line(q.0 - right.0 * 5.0, q.1 - right.1 * 5.0, q.0 + right.0 * 5.0, q.1 + right.1 * 5.0, GREY, 1.3); }
        let mid = ((qa.0 + qb.0) / 2.0, (qa.1 + qb.1) / 2.0);
        let o = 10.0 + 0.5 * (tw(t, 14.0) * right.0.abs() + 14.0 * right.1.abs());
        put(&mut d, mid.0 + right.0 * o, mid.1 + right.1 * o, t, 14.0);
    }
    for (a, t) in &texts { let q = m.p(*a); put(&mut d, q.0, q.1, t, 14.0); }
    d.finish()
}

// ───────────────────────── registry ─────────────────────────

pub fn defs() -> Vec<FigureDef> {
    let f = |id, title, blurb, params: fn() -> Vec<ParamDef>, render: fn(&Params) -> String| FigureDef { id, subject: Subject::Math, title, blurb, params, render };
    vec![
        f("coordinate-grid", "Coordinate Grid", "Graph paper with numbered axes, minor gridlines, plotted equations and labelled points.", coordinate_grid_params, coordinate_grid_render),
        f("composite-shape", "Composite Shape", "Build a compound figure from rectangles, triangles, circles and semicircles, with dimension marks.", composite_params, composite_render),
        f("number-line", "Number Line", "A number line with adjustable range and ticks, graphed inequalities and marked points.", number_line_params, number_line_render),
        f("triangle", "Triangle", "A triangle drawn to scale from sides, angles or right-triangle legs, with optional height.", triangle_params, triangle_render),
        f("rectangle", "Rectangle", "A rectangle or square drawn to scale with labelled sides and optional diagonals.", rectangle_params, rectangle_render),
        f("parallelogram", "Parallelogram", "A parallelogram or rhombus from its sides and an angle, with height and diagonals.", parallelogram_params, parallelogram_render),
        f("trapezoid", "Trapezoid", "A plain, isosceles or right trapezoid from its bases and height.", trapezoid_params, trapezoid_render),
        f("kite", "Kite", "A kite from its two side lengths and the angle between the short sides.", kite_params, kite_render),
        f("regular-polygon", "Regular Polygon", "Any regular polygon from 3 to 20 sides, sized by side, radius or apothem.", regular_polygon_params, regular_polygon_render),
        f("angles", "Angles", "Single, adjacent, straight-line, around-a-point or vertical angles with measures.", angles_params, angles_render),
        f("parallel-lines", "Parallel Lines and Transversal", "Parallel lines cut by a transversal with chosen angles marked in degrees or by number.", parallel_lines_params, parallel_lines_render),
        f("prism", "Prism", "A prism on a rectangle, triangle or regular-polygon base, right or oblique, with dashed hidden edges.", prism_params, prism_render),
        f("cylinder", "Cylinder", "A cylinder, right or oblique, with radius or diameter and height.", cylinder_params, cylinder_render),
        f("pyramid", "Pyramid", "A pyramid on a rectangle or regular-polygon base with optional height and slant height.", pyramid_params, pyramid_render),
        f("cone", "Cone", "A cone, right or oblique, with radius or diameter, height and slant height.", cone_params, cone_render),
        f("sphere", "Sphere", "A sphere or hemisphere with its radius or diameter.", sphere_params, sphere_render),
        f("box-plot", "Box Plot", "A box plot from a data list or five-number summary; compare two sets.", box_plot_params, box_plot_render),
        f("mapping-diagram", "Mapping Diagram", "Input and output sets joined by arrows, from ordered pairs.", mapping_diagram_params, mapping_diagram_render),
        f("length-reading", "Length Reading", "A metric or inch ruler with an object to measure at any position and length.", length_reading_params, length_reading_render),
    ]
}

// ───────────────────────── tests ─────────────────────────

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
        run("triangle", "sss", &[("mode", s("Three sides (SSS)")), ("ab", f(7.0)), ("bc", f(5.0)), ("ca", f(6.0)), ("height", s("From C")), ("angles", s("A, B, C")), ("unit", s("cm")), ("unknown", s(""))]);
        run("triangle", "obtuse-h", &[("mode", s("Two sides and the angle between (SAS)")), ("ab", f(8.0)), ("ca", f(5.0)), ("ang_a", f(120.0)), ("height", s("From C")), ("rotate", f(20.0))]);
        run("triangle", "asa", &[("mode", s("Two angles and the side between (ASA)")), ("ab", f(9.0)), ("ang_a", f(40.0)), ("ang_b", f(70.0)), ("angles", s("A,B,C"))]);
        run("triangle", "iso", &[("mode", s("Isosceles (base AB, legs BC = CA)")), ("ab", f(6.0)), ("bc", f(5.0))]);
        run("rectangle", "sq", &[("kind", s("Square")), ("ab", f(4.0)), ("diagonals", s("Drawn and labelled")), ("unit", s("m"))]);
        run("parallelogram", "rh", &[("kind", s("Rhombus")), ("height", s("Onto AB (from D)")), ("ang_a", f(110.0)), ("diagonals", s("Drawn"))]);
        run("parallelogram", "h", &[("height", s("Onto AB (from D)")), ("ang_a", f(125.0))]);
        run("trapezoid", "iso", &[("kind", s("Isosceles trapezoid")), ("show_height", b(true)), ("angles", s("A,B"))]);
        run("trapezoid", "plain", &[("kind", s("Trapezoid")), ("show_height", b(true)), ("offset", f(3.0))]);
        run("kite", "diag", &[("diagonals", s("Drawn")), ("angles", s("A,B,C,D"))]);
        run("regular-polygon", "n8", &[("n", i(8)), ("apothem", b(true)), ("from", s("Radius")), ("value", f(5.0))]);
        run("regular-polygon", "n3", &[("n", i(3)), ("sides", s("AB, BC, CA")), ("angles", s("A"))]);
        run("regular-polygon", "n17", &[("n", i(17)), ("radius", b(false))]);
    }
    #[test]
    fn angle_figs() {
        for m in ["Single angle", "Angles on a straight line", "Angles around a point", "Vertical angles", "Custom rays"] {
            run("angles", &m.split(' ').next().unwrap().to_lowercase(), &[("mode", s(m)), ("angle", f(35.0)), ("unknown", s("2"))]);
        }
        run("angles", "right", &[("mode", s("Angles on a straight line")), ("angle", f(90.0))]);
        run("parallel-lines", "num", &[("labels", s("Numbers")), ("marked", s("1,2,3,4,5,6,7,8")), ("angle", f(55.0))]);
        run("parallel-lines", "three", &[("lines", i(3)), ("angle", f(110.0)), ("marked", s("2, 6, 10")), ("unknown", s("10"))]);
    }
    #[test]
    fn solids() {
        run("prism", "tri", &[("base", s("Right triangle"))]);
        run("prism", "iso", &[("base", s("Isosceles triangle"))]);
        run("prism", "hex", &[("base", s("Regular polygon")), ("n", i(6))]);
        run("prism", "oblique", &[("stands", s("Oblique"))]);
        run("pyramid", "slant", &[("show_slant", b(true))]);
        run("pyramid", "hex", &[("base", s("Regular polygon")), ("n", i(6)), ("stands", s("Oblique")), ("show_slant", b(true))]);
        run("cylinder", "oblique", &[("stands", s("Oblique")), ("measure", s("Diameter"))]);
        run("cone", "oblique", &[("stands", s("Oblique")), ("view", f(35.0))]);
        run("sphere", "hemi", &[("draw", s("A hemisphere")), ("measure", s("Diameter"))]);
    }
    #[test]
    fn data_figs() {
        run("box-plot", "two", &[("data2", s("10, 12, 15, 15, 18, 20, 21, 40")), ("name1", s("Class A")), ("name2", s("Class B")), ("outliers", b(true)), ("title", s("Scores")), ("axis_title", s("Points"))]);
        run("box-plot", "summary", &[("summary", s("20, 35, 40, 55, 90"))]);
        run("mapping-diagram", "many", &[("inputs", s("a, b, c")), ("outputs", s("1, 2")), ("pairs", s("a->1; b->1; c->2")), ("shape", s("Rectangles"))]);
        run("length-reading", "pencil", &[("object", s("Pencil")), ("start", f(2.0)), ("length", f(6.45)), ("show_value", b(true))]);
        run("length-reading", "inch", &[("marks", s("1/8 in")), ("ruler", i(6)), ("length", f(3.625)), ("start", f(0.0))]);
        run("length-reading", "long", &[("ruler", i(30)), ("marks", s("1 mm")), ("length", f(17.3))]);
    }
}
