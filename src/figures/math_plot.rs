//! Direct-manipulation handles (see `interact`) for the plot-like math figures: coordinate grid,
//! number line, box plot, mapping diagram, composite shape and ruler reading. Geometry mirrors the
//! renderers in `math.rs` (layout constants are repeated here on purpose; keep them in sync).
#![allow(dead_code)]
use super::interact::{Handle, Pt};
use super::math::{ell, eval, nums, parse_ineq, preset_spec, quarter_pts, Interval, Map};
use super::Params;

fn fmt_n(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() { "0".into() } else { s }
}
fn snap(v: f64, r: f64) -> f64 { if r > 0.0 { (v / r).round() * r } else { v } }
fn near(a: Pt, b: Pt) -> bool { (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6 }

pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<Handle>> {
    Some(match id {
        "coordinate-grid" => grid(p),
        "number-line" => number_line(p),
        "box-plot" => box_plot(p),
        "mapping-diagram" => mapping(p, size),
        "composite-shape" => composite(p),
        "ruler-reading" => ruler(p),
        _ => return None,
    })
}

// ───────────────────────── coordinate grid ─────────────────────────

#[derive(Clone, Copy)]
struct Cg { x0: f64, x1: f64, y0: f64, y1: f64, ux: f64, uy: f64, ml: f64, mt: f64, pw: f64, ph: f64, rx: f64, ry: f64 }
impl Cg {
    fn new(p: &Params) -> Cg {
        let l = super::math::grid_layout(p);
        let minor = p.i("minor").max(1) as f64;
        Cg { x0: l.x0, x1: l.x1, y0: l.y0, y1: l.y1, ux: l.ux, uy: l.uy, ml: l.ml, mt: l.mt, pw: l.pw, ph: l.ph, rx: l.xs / minor, ry: l.ys / minor }
    }
    fn sx(&self, x: f64) -> f64 { self.ml + (x - self.x0) * self.ux }
    fn sy(&self, y: f64) -> f64 { self.mt + self.ph - (y - self.y0) * self.uy }
    fn ix(&self, px: f64) -> f64 { self.x0 + (px - self.ml) / self.ux }
    fn iy(&self, py: f64) -> f64 { self.y0 + (self.mt + self.ph - py) / self.uy }
    fn inside(&self, x: f64, y: f64) -> bool { x >= self.x0 - 1e-9 && x <= self.x1 + 1e-9 && y >= self.y0 - 1e-9 && y <= self.y1 + 1e-9 }
}

fn lin_str(m: f64, b: f64) -> String {
    let mut s = String::from("y = ");
    if m.abs() < 1e-9 { s += &fmt_n(b); return s; }
    s += &if (m - 1.0).abs() < 1e-9 { "x".to_string() } else if (m + 1.0).abs() < 1e-9 { "-x".to_string() } else { format!("{}x", fmt_n(m)) };
    if b > 1e-9 { s += &format!(" + {}", fmt_n(b)); } else if b < -1e-9 { s += &format!(" - {}", fmt_n(-b)); }
    s
}

/// (m, b) when `rhs` is linear in x.
fn linear(rhs: &str) -> Option<(f64, f64)> {
    let f = |x: f64| eval(rhs, x).filter(|v| v.is_finite());
    let (fm, f0, f1, f2) = (f(-1.0)?, f(0.0)?, f(1.0)?, f(2.0)?);
    let m = f1 - f0;
    let tol = 1e-9 * (1.0 + m.abs() + f0.abs());
    ((f2 - f1 - m).abs() < tol && (f0 - fm - m).abs() < tol).then_some((m, f0))
}

fn replace_piece(p: &mut Params, key: &str, idx: usize, f: impl Fn(&str) -> String) {
    let mut pieces: Vec<String> = p.s(key).split(';').map(|s| s.to_string()).collect();
    if idx < pieces.len() {
        let lead: String = pieces[idx].chars().take_while(|c| c.is_whitespace()).collect();
        let new = f(pieces[idx].trim());
        pieces[idx] = format!("{lead}{new}");
        p.set_s(key, &pieces.join(";"));
    }
}

fn grid(p: &Params) -> Vec<Handle> {
    let g = Cg::new(p);
    let mut hs = vec![];
    // Equations.
    for (ci, eq) in p.s("equations").split(';').enumerate() {
        let eq = eq.trim();
        // inequalities and pieces with a domain have no drag handles (rewriting would drop them)
        if eq.is_empty() || eq.contains(['<', '>', ',', '\u{2264}', '\u{2265}']) { continue; }
        let (lhs, rhs) = match eq.find('=') { Some(i) => (eq[..i].trim().to_lowercase(), eq[i + 1..].trim().to_string()), None => ("y".to_string(), eq.to_string()) };
        if lhs == "x" {
            if let Some(v) = eval(&rhs, 0.0).filter(|v| v.is_finite()) {
                let pos = (g.sx(v), g.sy((g.y0 + g.y1) / 2.0));
                if v >= g.x0 && v <= g.x1 {
                    hs.push(Handle::knob(pos, "Drag to move this vertical line", move |p, t| {
                        if near(t, pos) { return; }
                        let nv = snap(g.ix(t.0), g.rx).clamp(g.x0, g.x1);
                        replace_piece(p, "equations", ci, |_| format!("x = {}", fmt_n(nv)));
                    }).with_cursor(super::interact::Cursor::ResizeH));
                }
            }
            continue;
        }
        let Some((m, b)) = linear(&rhs) else { continue };
        let xa = if g.x0 <= 0.0 && g.x1 >= 0.0 { 0.0 } else { g.x0 };
        let ya = m * xa + b;
        let apos = (g.sx(xa), g.sy(ya));
        if g.inside(xa, ya) {
            hs.push(Handle::knob(apos, "Drag to shift the line up or down (y-intercept)", move |p, t| {
                if near(t, apos) { return; }
                let ny = snap(g.iy(t.1), g.ry).clamp(g.y0, g.y1);
                replace_piece(p, "equations", ci, |_| lin_str(m, ny - m * xa));
            }).with_cursor(super::interact::Cursor::ResizeV));
        }
        for f in [0.75, 0.5, 0.25, 0.9, 0.1] {
            let xb = g.x0 + (g.x1 - g.x0) * f;
            let yb = m * xb + b;
            if (xb - xa).abs() < (g.x1 - g.x0) * 0.1 || !g.inside(xb, yb) { continue; }
            let bpos = (g.sx(xb), g.sy(yb));
            hs.push(Handle::diamond(bpos, "Drag to change the slope", move |p, t| {
                if near(t, bpos) { return; }
                let ny = g.iy(t.1).clamp(g.y0 - (g.y1 - g.y0), g.y1 + (g.y1 - g.y0));
                let nm = snap((ny - ya) / (xb - xa), 0.1).clamp(-100.0, 100.0);
                replace_piece(p, "equations", ci, |_| lin_str(nm, snap(ya - nm * xa, 0.05)));
            }));
            break;
        }
    }
    // Points.
    for (pi, seg) in p.s("points").split(';').enumerate() {
        let seg = seg.trim();
        if let (Some(a), Some(b)) = (seg.find('('), seg.find(')')) {
            let n = nums(&seg[a + 1..b]);
            if n.len() == 2 && g.inside(n[0], n[1]) {
                let pos = (g.sx(n[0]), g.sy(n[1]));
                hs.push(Handle::knob(pos, "Drag to move this point", move |p, t| {
                    if near(t, pos) { return; }
                    let (nx, ny) = (snap(g.ix(t.0), g.rx).clamp(g.x0, g.x1), snap(g.iy(t.1), g.ry).clamp(g.y0, g.y1));
                    replace_piece(p, "points", pi, |s| match (s.find('('), s.find(')')) {
                        (Some(a), Some(b)) if b > a => format!("{}({}, {}){}", &s[..a], fmt_n(nx), fmt_n(ny), &s[b + 1..]),
                        _ => s.to_string(),
                    });
                }));
            }
        }
    }
    // Text labels.
    let (w, ext) = (g.pw + g.ml + 34.0, if p.b("arrows") { 16.0 } else { 0.0 });
    let ax_y = if g.y0 <= 0.0 && g.y1 >= 0.0 { 0.0 } else { g.y0 };
    let ax_x = if g.x0 <= 0.0 && g.x1 >= 0.0 { 0.0 } else { g.x0 };
    if !p.s("title").is_empty() && p.s("title_mode") != "None" && p.s("title_mode") != "Blank line" { hs.push(Handle::label((w / 2.0, 18.0), (180.0, 24.0), "title", "Double-click to edit the title")); }
    if !p.s("xlabel").is_empty() { hs.push(Handle::label((g.ml + g.pw + ext + 14.0, g.sy(ax_y)), (28.0, 22.0), "xlabel", "Double-click to edit the x-axis label")); }
    if !p.s("ylabel").is_empty() { hs.push(Handle::label((g.sx(ax_x), g.mt - ext - 9.0), (28.0, 22.0), "ylabel", "Double-click to edit the y-axis label")); }
    hs
}

// ───────────────────────── number line ─────────────────────────

struct Nl { lo: f64, hi: f64, x0: f64, x1: f64, y: f64, r: f64 }
fn nl(p: &Params) -> Nl {
    let (lo, mut hi) = (p.f("min"), p.f("max"));
    if hi <= lo { hi = lo + 1.0; }
    Nl { lo, hi, x0: 40.0, x1: 640.0, y: 64.0, r: super::math::nl_step(p).0 / (p.i("minor") + 1) as f64 }
}
fn ineq_str(ivs: &[Interval]) -> String {
    let op = |c: bool| if c { "<=" } else { "<" };
    ivs.iter().map(|iv| match (iv.lo.is_finite(), iv.hi.is_finite()) {
        (true, true) if iv.lo == iv.hi => format!("x = {}", fmt_n(iv.lo)),
        (true, true) => format!("{} {} x {} {}", fmt_n(iv.lo), op(iv.lo_c), op(iv.hi_c), fmt_n(iv.hi)),
        (true, false) => format!("x {} {}", if iv.lo_c { ">=" } else { ">" }, fmt_n(iv.lo)),
        (false, true) => format!("x {} {}", op(iv.hi_c), fmt_n(iv.hi)),
        _ => String::new(),
    }).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" or ")
}

fn number_line(p: &Params) -> Vec<Handle> {
    let n = nl(p);
    let (lo, hi, x0, x1, y, r) = (n.lo, n.hi, n.x0, n.x1, n.y, n.r);
    let sx = move |v: f64| x0 + (v - lo) / (hi - lo) * (x1 - x0);
    let ix = move |px: f64| lo + (px - x0) / (x1 - x0) * (hi - lo);
    let mut hs = vec![];
    for (i, iv) in parse_ineq(p.s("inequality")).iter().enumerate() {
        for end in 0..2usize {
            let v = if end == 0 { iv.lo } else { iv.hi };
            if !v.is_finite() || v < lo - 1e-9 || v > hi + 1e-9 { continue; }
            let pos = (sx(v), y);
            let tip = if end == 0 { "Drag the lower bound; click to toggle open/closed" } else { "Drag the upper bound; click to toggle open/closed" };
            hs.push(Handle::knob(pos, tip, move |p, t| {
                if near(t, pos) { return; }
                let mut ivs = parse_ineq(p.s("inequality"));
                let Some(iv) = ivs.get_mut(i) else { return };
                let nv = snap(ix(t.0), r).clamp(lo, hi);
                if end == 0 { iv.lo = if iv.hi.is_finite() { nv.min(iv.hi) } else { nv }; } else { iv.hi = if iv.lo.is_finite() { nv.max(iv.lo) } else { nv }; }
                p.set_s("inequality", &ineq_str(&ivs));
            }).with_click(move |p| {
                let mut ivs = parse_ineq(p.s("inequality"));
                let Some(iv) = ivs.get_mut(i) else { return };
                if end == 0 { iv.lo_c = !iv.lo_c; } else { iv.hi_c = !iv.hi_c; }
                p.set_s("inequality", &ineq_str(&ivs));
            }));
        }
    }
    for (i, tok) in p.s("points").split(',').enumerate() {
        let t = tok.trim();
        let (open, num) = if let Some(s) = t.strip_suffix('o') { (true, s) } else { (false, t) };
        if let Ok(v) = num.trim().replace('\u{2212}', "-").parse::<f64>() {
            if v < lo - 1e-9 || v > hi + 1e-9 { continue; }
            let pos = (sx(v), y + 0.01);
            let set = move |p: &mut Params, nv: f64, open: bool| {
                let mut pieces: Vec<String> = p.s("points").split(',').map(|s| s.to_string()).collect();
                if i < pieces.len() {
                    let lead: String = pieces[i].chars().take_while(|c| c.is_whitespace()).collect();
                    pieces[i] = format!("{lead}{}{}", fmt_n(nv), if open { "o" } else { "" });
                    p.set_s("points", &pieces.join(","));
                }
            };
            hs.push(Handle::knob(pos, "Drag to move this point; click to switch open/closed", move |p, t| {
                if near(t, pos) { return; }
                set(p, snap(ix(t.0), r).clamp(lo, hi), open);
            }).with_click(move |p| {
                set(p, v, !open);
            }));
        }
    }
    if !p.s("label").is_empty() { hs.push(Handle::label((x1 + 26.0 + 10.0, y), (40.0, 22.0), "label", "Double-click to edit the axis label")); }
    hs
}

// ───────────────────────── box plot ─────────────────────────

fn box_plot(p: &Params) -> Vec<Handle> {
    let Some(l) = super::math::box_geom(p) else { return vec![] };
    let (amin, amax, step, x0, x1) = (l.amin, l.amax, l.step, l.x0, l.x1);
    let sx = move |v: f64| x0 + (v - amin) / (amax - amin) * (x1 - x0);
    let mut hs = vec![];
    for (row, (which, f, _)) in l.sets.iter().enumerate() {
        let cy = l.top + l.rowh * (row as f64 + 0.5) + 6.0;
        let (which, f) = (*which, *f);
        let (_, sk, nk) = super::math::BOX_SETS[which];
        for j in 0..5usize {
            let pos = (sx(f[j]), cy);
            let tip = ["Drag the minimum", "Drag Q1", "Drag the median", "Drag Q3", "Drag the maximum"][j];
            let drag = move |p: &mut Params, t: Pt| {
                if near(t, pos) { return; }
                let Some((mut cur, _)) = super::math::box_set(p, which) else { return };
                let v = snap(amin + (t.0 - x0) / (x1 - x0) * (amax - amin), step / 2.0);
                let lo = if j > 0 { cur[j - 1] } else { amin };
                let hi = if j < 4 { cur[j + 1] } else { amax };
                cur[j] = v.clamp(lo.min(hi), hi.max(lo));
                if p.b("auto") {
                    p.set_b("auto", false);
                    p.set_f("axis_min", amin);
                    p.set_f("axis_max", amax);
                    p.set_f("axis_step", step);
                }
                let s: Vec<String> = cur.iter().map(|x| fmt_n(*x)).collect();
                p.set_s(sk, &s.join(", "));
            };
            hs.push(match j {
                1 | 3 => Handle::bar(pos, false, tip, drag),
                2 => Handle::diamond(pos, tip, drag),
                _ => Handle::knob(pos, tip, drag),
            });
        }
        if !p.s(nk).is_empty() { hs.push(Handle::label((x0 - 14.0 - 30.0, cy), (90.0, 22.0), nk, "Double-click to rename this set")); }
    }
    if !p.s("title").is_empty() && p.s("title_mode") != "None" && p.s("title_mode") != "Blank line" { hs.push(Handle::label((l.w / 2.0, 22.0), (200.0, 24.0), "title", "Double-click to edit the title")); }
    if !p.s("axis_title").is_empty() && p.s("axis_title_mode") != "None" && p.s("axis_title_mode") != "Blank line" { hs.push(Handle::label(((x0 + x1) / 2.0, l.ay + 46.0), (200.0, 22.0), "axis_title", "Double-click to edit the label")); }
    hs
}

// ───────────────────────── mapping diagram ─────────────────────────

fn mapping(p: &Params, _size: (f64, f64)) -> Vec<Handle> {
    let super::math::MapData { ins, outs, pairs, .. } = super::math::mapping_parse(p);
    if ins.is_empty() && outs.is_empty() { return vec![]; }
    let rows = ins.len().max(outs.len()).max(1) as f64;
    let spacing = 42.0;
    let top = if matches!(p.s("title_mode"), "Text" | "Blank line") { 40.0 } else { 0.0 };
    let cy = top + 66.0 + rows * spacing / 2.0;
    let (lx, rx) = (130.0, 350.0);
    let ypos = move |i: usize, n: usize| { let g = rows * spacing / n.max(1) as f64; cy + (i as f64 - (n as f64 - 1.0) / 2.0) * g };
    let nout = outs.len();
    let mut hs = vec![];
    for (k, pr) in pairs.iter().enumerate() {
        let (Some(_), Some(j)) = (ins.iter().position(|x| *x == pr.a), outs.iter().position(|x| *x == pr.b)) else { continue };
        let pos = (rx - 28.0, ypos(j, nout));
        hs.push(Handle::knob(pos, "Drag up or down to point this arrow at another output", move |p, t| {
            if near(t, pos) { return; }
            let m = super::math::mapping_parse(p);
            let (outs, pairs) = (&m.outs, &m.pairs);
            let Some(pr) = pairs.get(k) else { return };
            if outs.is_empty() { return; }
            let first = ypos(0, outs.len());
            let gap = if outs.len() > 1 { ypos(1, outs.len()) - first } else { spacing };
            let nj = (((t.1 - first) / gap).round().max(0.0) as usize).min(outs.len() - 1);
            if outs[nj] == pr.b { return; }
            let mut s = p.s(m.key).to_string();
            match pr.b_span {
                Some((st, en)) if en <= s.len() && s.is_char_boundary(st) && s.is_char_boundary(en) => {
                    s.replace_range(st..en, &outs[nj]);
                    p.set_s(m.key, &s);
                }
                // "Match in order" arrows have no text: write them all out with this one moved
                None => {
                    let all: Vec<String> = pairs.iter().enumerate().map(|(i, q)| format!("{} -> {}", q.a, if i == k { &outs[nj] } else { &q.b })).collect();
                    p.set_s(m.key, &all.join("; "));
                }
                _ => {}
            }
        }).with_cursor(super::interact::Cursor::ResizeV));
    }
    if p.s("in_mode") != "None" && p.s("in_mode") != "Blank line" { hs.push(Handle::label((lx, top + 28.0), (110.0, 24.0), "title_in", "Double-click to edit the input title")); }
    if p.s("out_mode") != "None" && p.s("out_mode") != "Blank line" { hs.push(Handle::label((rx, top + 28.0), (110.0, 24.0), "title_out", "Double-click to edit the output title")); }
    if p.s("title_mode") == "Text" { hs.push(Handle::label((240.0, 26.0), (200.0, 26.0), "title", "Double-click to edit the diagram title")); }
    hs
}

// ───────────────────────── ruler reading ─────────────────────────

fn ruler(p: &Params) -> Vec<Handle> {
    if p.s("object") == "None" { return vec![]; }
    let lay = super::math::ruler_layout(p);
    let (x0, x1, ppu) = (lay.x0, lay.x1, lay.ppu);
    let (s, l) = super::math::ruler_values(p);
    let (ox0, ox1) = (x0 + s * ppu, (x0 + (s + l) * ppu).min(x1 + 8.0));
    let end = s + l;
    let ym = (lay.top + lay.bot) / 2.0;
    let (lp, rp, mp) = ((ox0, ym), (ox1, ym), ((ox0 + ox1) / 2.0, lay.top));
    vec![
        Handle::knob(lp, "Drag the left end of the object", move |p, t| {
            if near(t, lp) { return; }
            let ns = ((t.0 - x0) / ppu).clamp(0.0, end - 0.1);
            p.set_f("start", ns);
            p.set_f("length", end - ns);
        }).with_cursor(super::interact::Cursor::ResizeH),
        Handle::knob(rp, "Drag the right end of the object", move |p, t| {
            if near(t, rp) { return; }
            p.set_f("length", ((t.0 - x0) / ppu - s).max(0.1));
        }).with_cursor(super::interact::Cursor::ResizeH),
        Handle::diamond(mp, "Drag to slide the whole object", move |p, t| {
            if near(t, mp) { return; }
            p.set_f("start", (s + (t.0 - mp.0) / ppu).max(0.0));
        }).with_cursor(super::interact::Cursor::Move),
    ]
}

// ───────────────────────── composite shape ─────────────────────────

/// Rows as tokens; a piece's ` | LABEL` stays one token ("| LABEL") so it is never read as a
/// number and is written back unchanged by `join(" ")`.
fn cmds(spec: &str) -> Vec<Vec<String>> {
    spec.split(|c| c == ';' || c == '\n')
        .map(|c| {
            let kw = c.split_whitespace().next().unwrap_or("").to_lowercase();
            let piece = ["rect", "tri", "poly", "circle", "semi", "quarter"].contains(&kw.as_str());
            let (h, l) = match c.split_once('|') { Some((h, l)) if piece => (h, Some(l.trim())), _ => (c, None) };
            let mut v: Vec<String> = h.split_whitespace().map(|s| s.to_string()).collect();
            if let Some(l) = l.filter(|l| !l.is_empty() && !v.is_empty()) { v.push(format!("| {l}")); }
            v
        })
        .filter(|t| !t.is_empty())
        .collect()
}
fn tn(tk: &[String], i: usize) -> Option<f64> { tk.get(i).and_then(|s| s.replace('\u{2212}', "-").parse::<f64>().ok()) }
fn active_spec(p: &Params) -> String { preset_spec(p.s("preset")).unwrap_or(p.s("spec")).to_string() }

#[derive(Clone, Copy, PartialEq)]
enum V { Abs(usize, usize), Corner(usize, usize), Radius(usize) }

/// Every draggable point of the spec with its model position.
fn points_of(c: &[Vec<String>]) -> Vec<(V, Pt)> {
    let mut out = vec![];
    for (ci, tk) in c.iter().enumerate() {
        let n = |i| tn(tk, i);
        match tk[0].to_lowercase().as_str() {
            "rect" => if let (Some(x), Some(y), Some(w), Some(h)) = (n(1), n(2), n(3), n(4)) {
                for (k, q) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h)].into_iter().enumerate() { out.push((V::Corner(ci, k), q)); }
            },
            "tri" => for i in [1, 3, 5] { if let (Some(x), Some(y)) = (n(i), n(i + 1)) { out.push((V::Abs(ci, i), (x, y))); } },
            "poly" => { let mut i = 1; while i + 1 < tk.len() { if let (Some(x), Some(y)) = (n(i), n(i + 1)) { out.push((V::Abs(ci, i), (x, y))); } i += 2; } }
            "circle" | "semi" | "quarter" => if let (Some(x), Some(y), Some(r)) = (n(1), n(2), n(3)) {
                out.push((V::Abs(ci, 1), (x, y)));
                let horiz = !matches!(tk.get(4).map(|s| s.to_lowercase()).as_deref(), Some("l") | Some("r"));
                out.push((V::Radius(ci), if horiz { (x + r, y) } else { (x, y + r) }));
            },
            "dash" | "dim" => for i in [1, 3] { if let (Some(x), Some(y)) = (n(i), n(i + 1)) { out.push((V::Abs(ci, i), (x, y))); } },
            "text" => if let (Some(x), Some(y)) = (n(1), n(2)) { out.push((V::Abs(ci, 1), (x, y))); },
            _ => {}
        }
    }
    out
}

fn all_points(c: &[Vec<String>]) -> Vec<Pt> {
    let mut all = vec![];
    for tk in c {
        let n = |i| tn(tk, i);
        match tk[0].to_lowercase().as_str() {
            "rect" => if let (Some(x), Some(y), Some(w), Some(h)) = (n(1), n(2), n(3), n(4)) { all.extend([(x, y), (x + w, y), (x + w, y + h), (x, y + h)]); },
            "tri" => if let (Some(a), Some(b), Some(c2), Some(d), Some(e), Some(f)) = (n(1), n(2), n(3), n(4), n(5), n(6)) { all.extend([(a, b), (c2, d), (e, f)]); },
            "poly" => { let v: Vec<f64> = tk[1..].iter().filter_map(|s| s.parse().ok()).collect(); if v.len() >= 6 { all.extend(v.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1]))); } }
            "circle" => if let (Some(x), Some(y), Some(r)) = (n(1), n(2), n(3)) { all.extend(ell(x, y, r, r, 0.0, 360.0)); },
            "semi" => if let (Some(x), Some(y), Some(r)) = (n(1), n(2), n(3)) {
                let (a0, a1) = match tk.get(4).map(|s| s.to_lowercase()).as_deref() { Some("d") => (180.0, 360.0), Some("l") => (90.0, 270.0), Some("r") => (-90.0, 90.0), _ => (0.0, 180.0) };
                all.extend(ell(x, y, r, r, a0, a1));
            },
            "quarter" => if let (Some(x), Some(y), Some(r)) = (n(1), n(2), n(3)) { all.extend(quarter_pts(x, y, r, tk.get(4).map(|s| s.as_str()).unwrap_or("ne"))); },
            "dash" | "dim" => if let (Some(a), Some(b), Some(c2), Some(e)) = (n(1), n(2), n(3), n(4)) { all.extend([(a, b), (c2, e)]); },
            _ => {}
        }
    }
    all
}

/// Straight sides of the polygon pieces (rect, tri, poly, quarter) as (a, b, piece centroid).
fn piece_sides(c: &[Vec<String>]) -> Vec<(Pt, Pt, Pt)> {
    let mut polys: Vec<Vec<Pt>> = vec![];
    for tk in c {
        let n = |i| tn(tk, i);
        match tk[0].to_lowercase().as_str() {
            "rect" => if let (Some(x), Some(y), Some(w), Some(h)) = (n(1), n(2), n(3), n(4)) { polys.push(vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)]); },
            "tri" => if let (Some(a), Some(b), Some(c2), Some(d), Some(e), Some(f)) = (n(1), n(2), n(3), n(4), n(5), n(6)) { polys.push(vec![(a, b), (c2, d), (e, f)]); },
            "poly" => { let v: Vec<f64> = tk[1..].iter().filter_map(|s| s.parse().ok()).collect(); if v.len() >= 6 { polys.push(v.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect()); } }
            _ => {}
        }
    }
    let mut out = vec![];
    for (pi, v) in polys.iter().enumerate() {
        let k = v.len() as f64;
        let cen = (v.iter().map(|q| q.0).sum::<f64>() / k, v.iter().map(|q| q.1).sum::<f64>() / k);
        for i in 0..v.len() {
            let (a, b) = (v[i], v[(i + 1) % v.len()]);
            if dist2(a, b) < 1e-6 { continue; }
            // a side shared with another piece is inside the shape: nothing can be added there
            let shared = polys.iter().enumerate().any(|(pj, w)| pj != pi && (0..w.len()).any(|j| {
                let (c2, d2) = (w[j], w[(j + 1) % w.len()]);
                overlap(a, b, c2, d2)
            }));
            if !shared { out.push((a, b, cen)); }
        }
    }
    out
}
fn dist2(a: Pt, b: Pt) -> f64 { (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) }
/// Two collinear segments that share more than a point.
fn overlap(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let l = dist2(a, b).sqrt();
    let u = ((b.0 - a.0) / l, (b.1 - a.1) / l);
    let off = |q: Pt| ((q.0 - a.0) * u.1 - (q.1 - a.1) * u.0).abs();
    if off(c) > 1e-6 || off(d) > 1e-6 { return false; }
    let t = |q: Pt| (q.0 - a.0) * u.0 + (q.1 - a.1) * u.1;
    let (t0, t1) = (t(c).min(t(d)), t(c).max(t(d)));
    t1.min(l) - t0.max(0.0) > 1e-6
}

/// The piece the reference adds on a side: a rectangle as wide as the side and 3/4 as deep
/// (a slanted side gets the same rectangle turned to it, as a poly).
pub(super) fn piece_on_side(a: Pt, b: Pt, cen: Pt) -> String {
    let l = dist2(a, b).sqrt();
    let u = ((b.0 - a.0) / l, (b.1 - a.1) / l);
    let mut nrm = (-u.1, u.0);
    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    if (mid.0 - cen.0) * nrm.0 + (mid.1 - cen.1) * nrm.1 < 0.0 { nrm = (-nrm.0, -nrm.1); }
    let dep = snap(l * 0.75, 0.5).max(0.5);
    let (c, d) = ((b.0 + nrm.0 * dep, b.1 + nrm.1 * dep), (a.0 + nrm.0 * dep, a.1 + nrm.1 * dep));
    let axis = u.0.abs() < 1e-9 || u.1.abs() < 1e-9;
    if axis {
        let xs = [a.0, b.0, c.0, d.0];
        let ys = [a.1, b.1, c.1, d.1];
        let (x0, y0) = (xs.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MAX, f64::min));
        let (x1, y1) = (xs.iter().cloned().fold(f64::MIN, f64::max), ys.iter().cloned().fold(f64::MIN, f64::max));
        format!("rect {} {} {} {}", fmt_n(x0), fmt_n(y0), fmt_n(x1 - x0), fmt_n(y1 - y0))
    } else {
        format!("poly {} {} {} {} {} {} {} {}", fmt_n(a.0), fmt_n(a.1), fmt_n(b.0), fmt_n(b.1), fmt_n(c.0), fmt_n(c.1), fmt_n(d.0), fmt_n(d.1))
    }
}

fn set_pt(tk: &mut [String], i: usize, q: Pt) { tk[i] = fmt_n(q.0); tk[i + 1] = fmt_n(q.1); }

fn composite(p: &Params) -> Vec<Handle> {
    let spec = active_spec(p);
    let c = cmds(&spec);
    let all = all_points(&c);
    if all.is_empty() { return vec![]; }
    let m = Map::fit(&all, 480.0, 360.0, 70.0, 56.0);
    let (ms, mox, moy) = (m.s, m.ox, m.oy);
    let bb = (
        all.iter().map(|q| q.0).fold(f64::MAX, f64::min), all.iter().map(|q| q.0).fold(f64::MIN, f64::max),
        all.iter().map(|q| q.1).fold(f64::MAX, f64::min), all.iter().map(|q| q.1).fold(f64::MIN, f64::max),
    );
    let mut hs: Vec<Handle> = vec![];
    let mut seen: Vec<Pt> = vec![];
    for (v, q) in points_of(&c) {
        if v != V::Corner(0, 0) && !matches!(v, V::Radius(_)) && seen.iter().any(|s| near(*s, q)) { continue; }
        if !matches!(v, V::Radius(_)) { seen.push(q); }
        let pos = m.p(q);
        let tip = match v { V::Radius(_) => "Drag to change the radius", V::Corner(..) => "Drag this corner (linked points follow)", _ => "Drag this point (linked points follow)" };
        let mk = move |t: Pt| -> Pt {
            let q = ((t.0 - mox) / ms, (moy - t.1) / ms);
            (snap(q.0, 0.5).clamp(bb.0, bb.1), snap(q.1, 0.5).clamp(bb.2, bb.3))
        };
        let h = Handle::knob(pos, tip, move |p, t| {
            if near(t, pos) { return; }
            let nq = mk(t);
            let spec = active_spec(p);
            let mut c = cmds(&spec);
            match v {
                V::Radius(ci) => {
                    let (Some(cx), Some(cy)) = (tn(&c[ci], 1), tn(&c[ci], 2)) else { return };
                    let horiz = !matches!(c[ci].get(4).map(|s| s.to_lowercase()).as_deref(), Some("l") | Some("r"));
                    let r = if horiz { (nq.0 - cx).abs() } else { (nq.1 - cy).abs() };
                    c[ci][3] = fmt_n(r.max(0.5));
                }
                _ => {
                    let old = q;
                    for (w, wq) in points_of(&c.clone()) {
                        if matches!(w, V::Radius(_)) || !near(wq, old) { continue; }
                        match w {
                            V::Abs(ci, i) => set_pt(&mut c[ci], i, nq),
                            V::Corner(ci, k) => {
                                let (x, y, w2, h2) = (tn(&c[ci], 1).unwrap(), tn(&c[ci], 2).unwrap(), tn(&c[ci], 3).unwrap(), tn(&c[ci], 4).unwrap());
                                let cs = [(x, y), (x + w2, y), (x + w2, y + h2), (x, y + h2)];
                                let opp = cs[(k + 2) % 4];
                                let (nx, ny) = (nq.0.min(opp.0), nq.1.min(opp.1));
                                c[ci][1] = fmt_n(nx);
                                c[ci][2] = fmt_n(ny);
                                c[ci][3] = fmt_n((nq.0 - opp.0).abs().max(0.5));
                                c[ci][4] = fmt_n((nq.1 - opp.1).abs().max(0.5));
                            }
                            V::Radius(_) => {}
                        }
                    }
                }
            }
            let out: Vec<String> = c.iter().map(|t| t.join(" ")).collect();
            p.set_s("spec", &out.join("; "));
            p.set_s("preset", "Custom (use the text below)");
        });
        hs.push(h);
    }
    // the reference's "+" beside each outside straight side: click to add a rectangle there
    for (a, b, cen) in piece_sides(&c) {
        let (pa, pb, pc) = (m.p(a), m.p(b), m.p(cen));
        let l = dist2(pa, pb).sqrt();
        if l < 30.0 { continue; }
        let mid = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
        let mut nrm = (-(pb.1 - pa.1) / l, (pb.0 - pa.0) / l);
        if (mid.0 - pc.0) * nrm.0 + (mid.1 - pc.1) * nrm.1 < 0.0 { nrm = (-nrm.0, -nrm.1); }
        let pos = (mid.0 + nrm.0 * 18.0, mid.1 + nrm.1 * 18.0);
        let piece = piece_on_side(a, b, cen);
        let h = Handle::zone(pos, (14.0, 14.0), "Click to add a rectangle on this side", move |p| {
            let mut spec = active_spec(p).trim().trim_end_matches(';').to_string();
            if !spec.is_empty() { spec.push_str("; "); }
            spec.push_str(&piece);
            p.set_s("spec", &spec);
            p.set_s("preset", "Custom (use the text below)");
        });
        hs.push(Handle { shape: super::interact::Shape::Diamond, ..h });
    }
    hs
}
