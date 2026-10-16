//! Direct-manipulation handles (see `interact`) for the plot-like math figures: coordinate grid,
//! number line, box plot, mapping diagram, composite shape and ruler reading. Geometry mirrors the
//! renderers in `math.rs` (layout constants are repeated here on purpose; keep them in sync).
#![allow(dead_code)]
use super::interact::{Handle, Pt};
use super::math::{ell, eval, five_number, nice_step, nums, parse_ineq, preset_spec, summary_for, Interval, Map};
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
        let (x0, mut x1, y0, mut y1) = (p.f("xmin"), p.f("xmax"), p.f("ymin"), p.f("ymax"));
        if x1 <= x0 { x1 = x0 + 1.0; }
        if y1 <= y0 { y1 = y0 + 1.0; }
        let (maxw, maxh) = (440.0, 400.0);
        let (ux, uy) = if p.b("square") { let u = (maxw / (x1 - x0)).min(maxh / (y1 - y0)); (u, u) } else { (maxw / (x1 - x0), maxh / (y1 - y0)) };
        let (pw, ph) = ((x1 - x0) * ux, (y1 - y0) * uy);
        let mt = if p.s("title").is_empty() { 30.0 } else { 62.0 };
        let minor = p.i("minor").max(1) as f64;
        Cg { x0, x1, y0, y1, ux, uy, ml: 52.0, mt, pw, ph, rx: p.f("xstep").max(1e-6) / minor, ry: p.f("ystep").max(1e-6) / minor }
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
        if eq.is_empty() { continue; }
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
    if !p.s("title").is_empty() { hs.push(Handle::label((w / 2.0, 18.0), (180.0, 24.0), "title", "Double-click to edit the title")); }
    hs.push(Handle::label((g.ml + g.pw + ext + 14.0, g.sy(ax_y)), (28.0, 22.0), "xlabel", "Double-click to edit the x-axis label"));
    hs.push(Handle::label((g.sx(ax_x), g.mt - ext - 9.0), (28.0, 22.0), "ylabel", "Double-click to edit the y-axis label"));
    hs
}

// ───────────────────────── number line ─────────────────────────

struct Nl { lo: f64, hi: f64, x0: f64, x1: f64, y: f64, r: f64 }
fn nl(p: &Params) -> Nl {
    let (lo, mut hi) = (p.f("min"), p.f("max"));
    if hi <= lo { hi = lo + 1.0; }
    Nl { lo, hi, x0: 40.0, x1: 640.0, y: 64.0, r: p.f("step").max(1e-6) / (p.i("minor") + 1) as f64 }
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

struct BoxL { sets: Vec<(u8, [f64; 5])>, amin: f64, amax: f64, step: f64, x0: f64, x1: f64, top: f64, rowh: f64, w: f64, named: bool, ay: f64 }
fn box_layout(p: &Params) -> Option<BoxL> {
    let mut sets = vec![];
    if let Some((f, _)) = summary_for(p.s("data"), p.s("summary"), p.b("outliers")) { sets.push((1u8, f)); }
    if let Some((f, _)) = summary_for(p.s("data2"), p.s("summary2"), p.b("outliers")) { sets.push((2u8, f)); }
    if sets.is_empty() { return None; }
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let mut all: Vec<f64> = vec![];
    for (n, k) in [(1u8, ("data", "summary")), (2, ("data2", "summary2"))] {
        if let Some((f, o)) = summary_for(p.s(k.0), p.s(k.1), p.b("outliers")) {
            let _ = n;
            all.extend(f.iter().chain(o.iter()));
        }
    }
    for v in &all { lo = lo.min(*v); hi = hi.max(*v); }
    let (amin, amax, step) = if p.b("auto") {
        let st = nice_step((hi - lo).max(1e-6));
        ((lo / st).floor() * st - if lo % st == 0.0 { st } else { 0.0 }, (hi / st).ceil() * st + if hi % st == 0.0 { st } else { 0.0 }, st)
    } else { (p.f("axis_min"), p.f("axis_max").max(p.f("axis_min") + 1e-3), p.f("axis_step").max(1e-3)) };
    let named = !p.s("name1").is_empty() && sets.iter().any(|s| s.0 == 1) || !p.s("name2").is_empty() && sets.iter().any(|s| s.0 == 2);
    let top = if p.s("title").is_empty() { 20.0 } else { 46.0 };
    let rowh = 84.0;
    let ay = top + rowh * sets.len() as f64 + 6.0;
    Some(BoxL { sets, amin, amax, step, x0: if named { 110.0 } else { 40.0 }, x1: 680.0 - 36.0, top, rowh, w: 680.0, named, ay })
}

fn box_plot(p: &Params) -> Vec<Handle> {
    let Some(l) = box_layout(p) else { return vec![] };
    let (amin, amax, step, x0, x1) = (l.amin, l.amax, l.step, l.x0, l.x1);
    let sx = move |v: f64| x0 + (v - amin) / (amax - amin) * (x1 - x0);
    let mut hs = vec![];
    for (row, (which, f)) in l.sets.iter().enumerate() {
        let cy = l.top + l.rowh * (row as f64 + 0.5) + 6.0;
        let (which, f) = (*which, *f);
        for j in 0..5usize {
            let pos = (sx(f[j]), cy);
            let tip = ["Drag the minimum", "Drag Q1", "Drag the median", "Drag Q3", "Drag the maximum"][j];
            let drag = move |p: &mut Params, t: Pt| {
                if near(t, pos) { return; }
                let (dk, sk) = if which == 1 { ("data", "summary") } else { ("data2", "summary2") };
                let Some((mut cur, _)) = summary_for(p.s(dk), p.s(sk), p.b("outliers")) else { return };
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
        let nk = if which == 1 { "name1" } else { "name2" };
        if !p.s(nk).is_empty() { hs.push(Handle::label((x0 - 14.0 - 30.0, cy), (90.0, 22.0), if which == 1 { "name1" } else { "name2" }, "Double-click to rename this set")); }
    }
    if !p.s("title").is_empty() { hs.push(Handle::label((l.w / 2.0, 22.0), (200.0, 24.0), "title", "Double-click to edit the title")); }
    if !p.s("axis_title").is_empty() { hs.push(Handle::label(((x0 + x1) / 2.0, l.ay + 46.0), (200.0, 22.0), "axis_title", "Double-click to edit the label")); }
    let _ = (l.named, five_number as fn(&[f64]) -> [f64; 5]);
    hs
}

// ───────────────────────── mapping diagram ─────────────────────────

struct Pair { a: String, b: String, b_span: (usize, usize) }
fn mp_parse(p: &Params) -> (Vec<String>, Vec<String>, Vec<Pair>) {
    let mut ins = super::math::list_items(p.s("inputs"));
    let mut outs = super::math::list_items(p.s("outputs"));
    let ps = p.s("pairs");
    // (inner text, byte offset of inner within ps)
    let mut segs: Vec<(String, usize)> = vec![];
    if ps.contains('(') {
        let mut at = 0;
        while let Some(i) = ps[at..].find('(') {
            let st = at + i + 1;
            let en = ps[st..].find(')').map(|j| st + j).unwrap_or(ps.len());
            segs.push((ps[st..en].to_string(), st));
            at = en.min(ps.len());
            if at >= ps.len() { break; }
        }
    } else {
        let mut off = 0;
        for s in ps.split(';') { segs.push((s.to_string(), off)); off += s.len() + 1; }
    }
    let mut pairs = vec![];
    for (inner, off) in segs {
        let sep = if let Some(i) = inner.find("->") { Some((i, 2)) } else if let Some(i) = inner.find('\u{2192}') { Some((i, 3)) } else { inner.find(',').map(|i| (i, 1)) };
        if let Some((i, l)) = sep {
            let (a, braw) = (inner[..i].trim().to_string(), &inner[i + l..]);
            let b = braw.trim().to_string();
            if a.is_empty() || b.is_empty() { continue; }
            let lead = braw.len() - braw.trim_start().len();
            let st = off + i + l + lead;
            if !ins.contains(&a) { ins.push(a.clone()); }
            if !outs.contains(&b) { outs.push(b.clone()); }
            pairs.push(Pair { a, b: b.clone(), b_span: (st, st + b.len()) });
        }
    }
    (ins, outs, pairs)
}

fn mapping(p: &Params, _size: (f64, f64)) -> Vec<Handle> {
    let (ins, outs, pairs) = mp_parse(p);
    if ins.is_empty() && outs.is_empty() { return vec![]; }
    let rows = ins.len().max(outs.len()).max(1) as f64;
    let spacing = 42.0;
    let cy = 66.0 + rows * spacing / 2.0;
    let (lx, rx) = (130.0, 350.0);
    let ypos = move |i: usize, n: usize| cy - (n as f64 - 1.0) * spacing / 2.0 + i as f64 * spacing;
    let nout = outs.len();
    let mut hs = vec![];
    for (k, pr) in pairs.iter().enumerate() {
        let (Some(_), Some(j)) = (ins.iter().position(|x| *x == pr.a), outs.iter().position(|x| *x == pr.b)) else { continue };
        let pos = (rx - 28.0, ypos(j, nout));
        hs.push(Handle::knob(pos, "Drag up or down to point this arrow at another output", move |p, t| {
            if near(t, pos) { return; }
            let (_, outs, pairs) = mp_parse(p);
            let Some(pr) = pairs.get(k) else { return };
            if outs.is_empty() { return; }
            let first = ypos(0, outs.len());
            let nj = (((t.1 - first) / spacing).round().max(0.0) as usize).min(outs.len() - 1);
            if outs[nj] == pr.b { return; }
            let mut s = p.s("pairs").to_string();
            if pr.b_span.1 <= s.len() && s.is_char_boundary(pr.b_span.0) && s.is_char_boundary(pr.b_span.1) {
                s.replace_range(pr.b_span.0..pr.b_span.1, &outs[nj]);
                p.set_s("pairs", &s);
            }
        }).with_cursor(super::interact::Cursor::ResizeV));
    }
    hs.push(Handle::label((lx, 28.0), (110.0, 24.0), "title_in", "Double-click to edit the input title"));
    hs.push(Handle::label((rx, 28.0), (110.0, 24.0), "title_out", "Double-click to edit the output title"));
    hs
}

// ───────────────────────── ruler reading ─────────────────────────

fn ruler(p: &Params) -> Vec<Handle> {
    if p.s("object") == "None" { return vec![]; }
    let len = p.i("ruler").max(1) as f64;
    let (x0, x1) = (30.0, 650.0);
    let ppu = (x1 - x0) / len;
    let (s, l) = (p.f("start"), p.f("length"));
    let (ox0, ox1) = (x0 + s * ppu, (x0 + (s + l) * ppu).min(x1 + 8.0));
    let end = s + l;
    let (lp, rp, mp) = ((ox0, 55.0), (ox1, 55.0), ((ox0 + ox1) / 2.0, 42.0));
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

fn cmds(spec: &str) -> Vec<Vec<String>> {
    spec.split(|c| c == ';' || c == '\n').map(|c| c.split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>()).filter(|t| !t.is_empty()).collect()
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
            "circle" | "semi" => if let (Some(x), Some(y), Some(r)) = (n(1), n(2), n(3)) {
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
            "dash" | "dim" => if let (Some(a), Some(b), Some(c2), Some(e)) = (n(1), n(2), n(3), n(4)) { all.extend([(a, b), (c2, e)]); },
            _ => {}
        }
    }
    all
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
    hs
}
