//! Titration curve and heating/cooling curve.
use super::*;
use crate::figures::{Params, ParamDef, Subject};

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "titration-curve", subject: Subject::Chemistry, title: "Titration Curve",
            blurb: "pH against volume of titrant for strong or weak acids and bases, computed from the concentrations.",
            params: tit_params, render: tit_render },
        FigureDef { id: "heating-cooling-curve", subject: Subject::Chemistry, title: "Heating and Cooling Curve",
            blurb: "Temperature against time as a substance warms or cools through melting and boiling.",
            params: heat_params, render: heat_render },
    ]
}

fn nice_step(range: f64, ticks: f64) -> f64 {
    let raw = range / ticks;
    let mag = 10f64.powf(raw.log10().floor());
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] { if m * mag >= raw { return m * mag; } }
    10.0 * mag
}

fn fmt_tick(v: f64, step: f64) -> String { if step >= 1.0 { format!("{}", v.round() as i64) } else { num(v, 1) } }

// ---------------------------------------------------------------------------------------------
// Titration
// ---------------------------------------------------------------------------------------------

const KINDS: [&str; 4] = ["Strong acid with a strong base", "Weak acid with a strong base", "Strong base with a strong acid", "Weak base with a strong acid"];

fn tit_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "What's titrated", &KINDS, "Weak acid with a strong base"),
        ParamDef::float("molarity", "Analyte molarity (M)", 0.001, 2.0, 0.005, 0.1),
        ParamDef::float("volume", "Analyte volume (mL)", 1.0, 200.0, 0.5, 25.0),
        ParamDef::float("titrant", "Titrant molarity (M)", 0.001, 2.0, 0.005, 0.1),
        ParamDef::float("pk", "pKa (weak acid) or pKb (weak base)", 1.0, 12.0, 0.01, 4.76),
        ParamDef::choice("equiv", "Equivalence point", &["Dot and lines to the axes", "Dot", "Not marked"], "Dot and lines to the axes"),
        ParamDef::bool("half", "Mark half-equivalence (weak only)", false),
        ParamDef::float("xmax", "x-axis maximum (mL, 0 = automatic)", 0.0, 1000.0, 1.0, 0.0),
        ParamDef::text("title", "Title (optional)", ""),
        ParamDef::text("xlabel", "x-axis label (blank = automatic)", ""),
        ParamDef::text("eq_label", "Equivalence point label", "equivalence point"),
        ParamDef::text("half_label", "Half-equivalence label", "half-equivalence point"),
        ParamDef::color("curve_color", "Curve colour", &CURVE_COLORS, "#1f5fa8"),
    ]
}

pub(super) const T_X0: f64 = 84.0;
pub(super) const T_X1: f64 = 600.0;
pub(super) const T_Y0: f64 = 40.0;
pub(super) const T_Y1: f64 = 400.0;

/// Axis frame shared by the titration renderer and its handles.
pub(super) struct TitFrame { pub kind: String, pub ca: f64, pub va: f64, pub ct: f64, pub pk: f64, pub acid_analyte: bool, pub veq: f64, pub xmax: f64, pub step_x: f64 }

impl TitFrame {
    pub fn px(&self, v: f64) -> f64 { T_X0 + v / self.xmax * (T_X1 - T_X0) }
    pub fn py(&self, ph: f64) -> f64 { T_Y1 - ph / 14.0 * (T_Y1 - T_Y0) }
    pub fn ph(&self, v: f64) -> f64 { titration_ph(&self.kind, self.ca, self.va, self.ct, self.pk, v) }
}

pub(super) fn tit_frame(p: &Params) -> TitFrame {
    let kind = p.s("kind").to_string();
    let (ca, va, ct) = (p.f("molarity").max(1e-6), p.f("volume").max(0.1), p.f("titrant").max(1e-6));
    let pk = p.f("pk");
    let acid_analyte = kind.starts_with("Strong acid") || kind.starts_with("Weak acid");
    let veq = ca * va / ct;
    let xmax_raw = if p.f("xmax") >= 1.0 { p.f("xmax") } else { veq * 2.0 };
    let step_x = nice_step(xmax_raw, 10.0);
    let xmax = (xmax_raw / step_x).ceil() * step_x;
    TitFrame { kind, ca, va, ct, pk, acid_analyte, veq, xmax, step_x }
}

/// pH after `v` mL of titrant.
fn titration_ph(kind: &str, ca: f64, va: f64, ct: f64, pk: f64, v: f64) -> f64 {
    let kw = 1e-14;
    let vt = va + v;
    let f = |h: f64| -> f64 {
        match kind {
            "Strong acid with a strong base" => ct * v / vt + h - ca * va / vt - kw / h,
            "Weak acid with a strong base" => {
                let ka = 10f64.powf(-pk);
                ct * v / vt + h - (ca * va / vt) * ka / (ka + h) - kw / h
            }
            "Strong base with a strong acid" => ca * va / vt + h - ct * v / vt - kw / h,
            _ => {
                let kac = kw / 10f64.powf(-pk);
                h + (ca * va / vt) * h / (h + kac) - ct * v / vt - kw / h
            }
        }
    };
    let (mut lo, mut hi) = (-16.0f64, 2.0f64); // log10 h
    for _ in 0..80 {
        let mid = (lo + hi) / 2.0;
        if f(10f64.powf(mid)) > 0.0 { hi = mid } else { lo = mid }
    }
    (-(lo + hi) / 2.0).clamp(0.0, 14.0)
}

fn tit_render(p: &Params) -> String {
    let f = tit_frame(p);
    let (kind, ca, va, ct, pk, acid_analyte, veq, xmax, step_x) = (f.kind.as_str(), f.ca, f.va, f.ct, f.pk, f.acid_analyte, f.veq, f.xmax, f.step_x);
    let (w, h) = (640.0, 480.0);
    let (x0, x1, y0, y1) = (84.0, 600.0, 40.0, 400.0); // y0 top (pH14), y1 bottom (pH 0)
    let px = |v: f64| x0 + v / xmax * (x1 - x0);
    let py = |ph: f64| y1 - ph / 14.0 * (y1 - y0);
    let mut d = Doc::new(w, h);
    title_at(&mut d, p, (x0 + x1) / 2.0, 24.0, 16.0);
    // grid + axes
    for k in 0..=14 {
        let y = py(k as f64);
        if k % 2 == 0 { d.line(x0, y, x1, y, "#e3e3e3", 0.8); }
        d.line(x0 - 5.0, y, x0, y, INK, 1.2);
        if k % 2 == 0 { tx(&mut d, x0 - 10.0, y + 4.5, 13.0, "end", INK, false, &format!("{k}")); }
    }
    let nx = (xmax / step_x).round() as i64;
    for k in 0..=nx {
        let v = k as f64 * step_x;
        d.line(px(v), y1, px(v), y1 + 5.0, INK, 1.2);
        tx(&mut d, px(v), y1 + 20.0, 13.0, "middle", INK, false, &fmt_tick(v, step_x));
    }
    d.line(x0, y0, x0, y1, INK, 1.8);
    d.line(x0, y1, x1, y1, INK, 1.8);
    let titr_name = if acid_analyte { "NaOH" } else { "HCl" };
    let xl = if p.s("xlabel").trim().is_empty() { format!("Volume of {titr_name} added (mL)") } else { p.s("xlabel").to_string() };
    tx(&mut d, (x0 + x1) / 2.0, y1 + 50.0, 15.0, "middle", INK, false, &xl);
    tr(&mut d, 28.0, (y0 + y1) / 2.0, 15.0, -90.0, "middle", "pH");
    // curve
    let n = 400;
    let pts: Vec<(f64, f64)> = (0..=n).map(|i| {
        let v = xmax * i as f64 / n as f64;
        (px(v), py(titration_ph(kind, ca, va, ct, pk, v)))
    }).collect();
    let cc = if p.s("curve_color").starts_with('#') { p.s("curve_color") } else { "#1f5fa8" };
    d.polyline(&pts, cc, 2.6);
    let weak = kind.starts_with("Weak");
    let eq = p.s("equiv");
    let ph_eq = titration_ph(kind, ca, va, ct, pk, veq);
    if eq != "Not marked" {
        let (ex, ey) = (px(veq), py(ph_eq));
        if eq.starts_with("Dot and") {
            dashed(&mut d, ex, ey, ex, y1, "#666", 1.1);
            dashed(&mut d, x0, ey, ex, ey, "#666", 1.1);
        }
        d.circle(ex, ey, 5.0, "#c0392b", INK, 1.2);
        let up = !acid_analyte;
        tx(&mut d, ex + 12.0, if up { ey - 12.0 } else { ey + 24.0 }, 13.0, "start", INK, false, p.s("eq_label"));
    }
    if weak && p.b("half") {
        let hv = veq / 2.0;
        let hp = titration_ph(kind, ca, va, ct, pk, hv);
        let (hx, hy) = (px(hv), py(hp));
        dashed(&mut d, hx, hy, hx, y1, "#666", 1.1);
        dashed(&mut d, x0, hy, hx, hy, "#666", 1.1);
        d.circle(hx, hy, 5.0, "#2e8b57", INK, 1.2);
        tx(&mut d, hx + 12.0, if acid_analyte { hy + 22.0 } else { hy - 12.0 }, 13.0, "start", INK, false, p.s("half_label"));
    }
    d.finish()
}

// ---------------------------------------------------------------------------------------------
// Heating / cooling
// ---------------------------------------------------------------------------------------------

const SUBST: [(&str, f64, f64); 6] = [
    ("Water", 0.0, 100.0), ("Ethanol", -114.0, 78.0), ("Acetone", -95.0, 56.0),
    ("Mercury", -39.0, 357.0), ("Sodium chloride", 801.0, 1413.0), ("Substance X", 20.0, 90.0),
];

fn heat_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("mode", "Heating or cooling", &["Heating", "Cooling"], "Heating"),
        ParamDef::choice("substance", "Substance", &["Water", "Ethanol", "Acetone", "Mercury", "Sodium chloride", "Substance X", "Custom"], "Water"),
        ParamDef::float("mp", "Custom melting point (°C)", -270.0, 3000.0, 1.0, 0.0),
        ParamDef::float("bp", "Custom boiling point (°C)", -260.0, 4000.0, 1.0, 100.0),
        ParamDef::float("start", "Starting temperature (°C)", -273.0, 4000.0, 1.0, -20.0),
        ParamDef::float("end", "Ending temperature (°C)", -273.0, 4000.0, 1.0, 120.0),
        ParamDef::choice("xaxis", "Along the x-axis", &["Time", "Heat added"], "Time"),
        ParamDef::choice("scale", "Segment lengths", &["Schematic", "To scale"], "Schematic"),
        ParamDef::float("t_solid", "Solid warms (min)", 0.5, 30.0, 0.5, 2.0),
        ParamDef::float("t_melt", "Melting (min)", 0.5, 30.0, 0.5, 3.0),
        ParamDef::float("t_liquid", "Liquid warms (min)", 0.5, 30.0, 0.5, 5.0),
        ParamDef::float("t_boil", "Boiling (min)", 0.5, 30.0, 0.5, 8.0),
        ParamDef::float("t_gas", "Gas warms (min)", 0.5, 30.0, 0.5, 2.0),
        ParamDef::bool("labels", "Label phases and transition temperatures", true),
        ParamDef::text("title", "Title (optional)", ""),
        ParamDef::text("ylabel", "y-axis label", "Temperature (°C)"),
        ParamDef::list("phase_names", "Phase labels (5)", "solid, solid + liquid, liquid, liquid + gas, gas"),
        ParamDef::color("curve_color", "Curve colour", &CURVE_COLORS, "#1f5fa8"),
    ]
}

pub(super) const H_X0: f64 = 110.0;
pub(super) const H_X1: f64 = 660.0;
pub(super) const H_Y0: f64 = 40.0;
pub(super) const H_Y1: f64 = 400.0;
const H_XPAD: f64 = 20.0;

/// Stage list + axis scaling shared by the heating-curve renderer and its handles.
pub(super) struct HeatFrame { pub seq: Vec<(Ph, f64, f64, f64)>, pub mp: f64, pub bp: f64, pub lo: f64, pub hi: f64, pub total: f64, pub ymin: f64, pub ymax: f64, pub ystep: f64, pub scaled: bool, pub cooling: bool }

impl HeatFrame {
    pub fn px(&self, u: f64) -> f64 { H_X0 + H_XPAD + u / self.total * (H_X1 - H_X0 - 2.0 * H_XPAD) }
    pub fn py(&self, t: f64) -> f64 { H_Y1 - (t - self.ymin) / (self.ymax - self.ymin) * (H_Y1 - H_Y0) }
    pub fn inv_t(&self, y: f64) -> f64 { self.ymin + (H_Y1 - y) / (H_Y1 - H_Y0) * (self.ymax - self.ymin) }
}

/// (mp, bp) the renderer will use (named substance table, or the custom params).
pub(super) fn heat_points(p: &Params) -> (f64, f64) {
    let (mp, mut bp) = match SUBST.iter().find(|s| s.0 == p.s("substance")) {
        Some(s) => (s.1, s.2), None => (p.f("mp"), p.f("bp")),
    };
    if bp <= mp { bp = mp + 50.0; }
    (mp, bp)
}

pub(super) fn heat_frame(p: &Params) -> HeatFrame {
    let (mp, bp) = heat_points(p);
    let a = p.f("start");
    let b = p.f("end");
    let (lo, hi) = (a.min(b), a.max(b));
    let hi = if hi - lo < 10.0 { lo + 10.0 } else { hi };
    let cooling = p.s("mode") == "Cooling";
    let mut st: Vec<(Ph, f64, f64, f64)> = Vec::new();
    if lo < mp { st.push((Ph::Solid, lo, mp.min(hi), p.f("t_solid"))); }
    if lo <= mp && hi > mp { st.push((Ph::Melt, mp, mp, p.f("t_melt"))); }
    let (l0, l1) = (lo.max(mp), hi.min(bp));
    if l1 > l0 && lo < bp && hi > mp { st.push((Ph::Liquid, l0, l1, p.f("t_liquid"))); }
    if lo <= bp && hi > bp { st.push((Ph::Boil, bp, bp, p.f("t_boil"))); }
    if hi > bp { st.push((Ph::Gas, lo.max(bp), hi, p.f("t_gas"))); }
    if st.is_empty() { st.push((Ph::Solid, lo, hi, 1.0)); }
    let scaled = p.s("scale") == "To scale";
    let dur = |s: &(Ph, f64, f64, f64)| if scaled { s.3 } else if matches!(s.0, Ph::Melt | Ph::Boil) { 1.4 } else { 1.0 };
    let mut seq: Vec<(Ph, f64, f64, f64)> = st.iter().map(|s| (s.0, s.1, s.2, dur(s))).collect();
    if cooling {
        seq.reverse();
        for s in seq.iter_mut() { std::mem::swap(&mut s.1, &mut s.2); }
    }
    let total: f64 = seq.iter().map(|s| s.3).sum();
    let ystep = nice_step(hi - lo, 7.0);
    let ymin = (lo / ystep).floor() * ystep;
    let ymax = (hi / ystep).ceil() * ystep + ystep * 0.5;
    HeatFrame { seq, mp, bp, lo, hi, total, ymin, ymax, ystep, scaled, cooling }
}

/// The five phase labels (solid, melting, liquid, boiling, gas), defaults filling blanks.
fn phase_names(p: &Params) -> Vec<String> {
    let dflt = ["solid", "solid + liquid", "liquid", "liquid + gas", "gas"];
    let given: Vec<&str> = p.s("phase_names").split(',').map(|t| t.trim()).collect();
    (0..5).map(|i| given.get(i).filter(|t| !t.is_empty()).map(|t| t.to_string()).unwrap_or_else(|| dflt[i].to_string())).collect()
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Ph { Solid, Melt, Liquid, Boil, Gas }

fn heat_render(p: &Params) -> String {
    let hf = heat_frame(p);
    let (mp, bp, lo, hi, total, ymin, ymax, ystep, scaled, cooling) = (hf.mp, hf.bp, hf.lo, hf.hi, hf.total, hf.ymin, hf.ymax, hf.ystep, hf.scaled, hf.cooling);
    let seq = hf.seq.clone();
    let (w, h) = (750.0, 480.0);
    let (x0, x1, y0, y1) = (H_X0, H_X1, H_Y0, H_Y1);
    let py = |t: f64| hf.py(t);
    let px = |u: f64| hf.px(u);
    let mut d = Doc::new(w, h);
    // y ticks
    let mut t = (ymin / ystep).ceil() * ystep;
    while t <= ymax + 1e-9 {
        d.line(x0 - 5.0, py(t), x0, py(t), INK, 1.2);
        tx(&mut d, x0 - 10.0, py(t) + 4.5, 13.0, "end", INK, false, &fmt_tick(t, ystep));
        t += ystep;
    }
    d.line(x0, y0 - 10.0, x0, y1, INK, 1.8);
    d.line(x0, y1, x1 + 10.0, y1, INK, 1.8);
    title_at(&mut d, p, (x0 + x1) / 2.0, 24.0, 16.0);
    tr(&mut d, 28.0, (y0 + y1) / 2.0, 15.0, -90.0, "middle", p.s("ylabel"));
    let xlabel = if p.s("xaxis") == "Heat added" { if cooling { "Heat removed" } else { "Heat added" } } else { "Time" };
    let xl = if scaled && xlabel == "Time" { "Time (min)".to_string() } else { xlabel.to_string() };
    tx(&mut d, (x0 + x1) / 2.0, y1 + 42.0, 15.0, "middle", INK, false, &xl);
    if scaled && xlabel == "Time" {
        let xs = nice_step(total, 8.0);
        let mut v = 0.0;
        while v <= total + 1e-9 {
            d.line(px(v), y1, px(v), y1 + 5.0, INK, 1.2);
            tx(&mut d, px(v), y1 + 20.0, 13.0, "middle", INK, false, &fmt_tick(v, xs));
            v += xs;
        }
    }
    // curve
    let mut pts = Vec::new();
    let mut u = 0.0;
    for s in &seq {
        if pts.is_empty() { pts.push((px(u), py(s.1))); }
        u += s.3;
        pts.push((px(u), py(s.2)));
    }
    let cc = if p.s("curve_color").starts_with('#') { p.s("curve_color") } else { "#1f5fa8" };
    d.polyline(&pts, cc, 2.8);
    if p.b("labels") {
        let mut u = 0.0;
        for s in &seq {
            let (xa, xb) = (px(u), px(u + s.3));
            u += s.3;
            let xm = (xa + xb) / 2.0;
            let (ya, yb) = (py(s.1), py(s.2));
            let ym = (ya + yb) / 2.0;
            let names = phase_names(p);
            let (name, plateau) = match s.0 {
                Ph::Solid => (names[0].as_str(), false), Ph::Melt => (names[1].as_str(), true), Ph::Liquid => (names[2].as_str(), false),
                Ph::Boil => (names[3].as_str(), true), Ph::Gas => (names[4].as_str(), false),
            };
            let temp_axis_val = match s.0 { Ph::Melt => Some(mp), Ph::Boil => Some(bp), _ => None };
            if plateau {
                tx(&mut d, xm, ym - 10.0, 12.5, "middle", INK, false, name);
                if let Some(tv) = temp_axis_val {
                    dashed(&mut d, x0, py(tv), xa, py(tv), "#888", 1.0);
                    let rising = (s.1 - s.2).abs() < 1e-9;
                    let _ = rising;
                }
            } else {
                // place beside the sloped segment, on the side away from the curve's neighbours
                let heating_dir = s.2 >= s.1;
                let (lx, ly) = if heating_dir { (xm + 16.0, ym + 20.0) } else { (xm + 30.0, ym + 4.0) };
                tx(&mut d, lx, ly, 12.5, "middle", INK, false, name);
            }
        }
        // transition temperature labels on the y axis side
        for (tv, nm) in [(mp, "mp"), (bp, "bp")] {
            if tv > lo + 1e-9 && tv < hi - 1e-9 || (tv >= lo && tv <= hi) {
                tx(&mut d, x1 + 4.0, py(tv) + 4.0, 12.0, "start", "#555", false, &format!("{nm} {}", fmt_tick(tv, 1.0)));
            }
        }
    }
    d.finish()
}

// ---------------------------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------------------------

use crate::figures::interact::{Cursor, Handle};

fn dur_key(ph: Ph) -> &'static str {
    match ph { Ph::Solid => "t_solid", Ph::Melt => "t_melt", Ph::Liquid => "t_liquid", Ph::Boil => "t_boil", Ph::Gas => "t_gas" }
}

pub(super) fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    match id {
        "titration-curve" => Some(tit_handles(p)),
        "heating-cooling-curve" => Some(heat_handles(p)),
        _ => None,
    }
}

fn tit_handles(p: &Params) -> Vec<Handle> {
    let f = tit_frame(p);
    let mut v = Vec::new();
    let ph_eq = f.ph(f.veq);
    v.push(
        Handle::diamond((f.px(f.veq), f.py(ph_eq)), "Equivalence point: drag sideways to change the analyte volume", |q, t| {
            let f = tit_frame(q);
            let vol = ((t.0 - T_X0) / (T_X1 - T_X0) * f.xmax) * f.ct / f.ca;
            super::handles::set_eff(q, "volume", vol, 1.0, 200.0);
        })
        .with_cursor(Cursor::ResizeH),
    );
    if f.kind.starts_with("Weak") {
        let hv = f.veq / 2.0;
        v.push(
            Handle::knob((f.px(hv), f.py(f.ph(hv))), "Half-equivalence point: drag up or down to set pKa / pKb", |q, t| {
                let f0 = tit_frame(q);
                let target = ((T_Y1 - t.1) / (T_Y1 - T_Y0) * 14.0).clamp(0.0, 14.0);
                let mut pk = f0.pk;
                let eval = |pk: f64| {
                    let mut f = tit_frame(q);
                    f.pk = pk;
                    f.ph(f.veq / 2.0)
                };
                for _ in 0..8 {
                    let a = eval(pk);
                    let slope = (eval((pk + 0.25).min(12.0)) - eval((pk - 0.25).max(1.0))) / ((pk + 0.25).min(12.0) - (pk - 0.25).max(1.0));
                    if slope.abs() < 0.05 { break; }
                    pk = (pk + (target - a) / slope).clamp(1.0, 12.0);
                }
                q.set_f("pk", pk);
            })
            .with_cursor(Cursor::ResizeV),
        );
    }
    v
}

fn heat_handles(p: &Params) -> Vec<Handle> {
    let hf = heat_frame(p);
    let mut out = Vec::new();
    let n = hf.seq.len();
    let a = p.f("start");
    let b = p.f("end");
    let key_for = |is_lo: bool| if (a <= b) == is_lo { "start" } else { "end" };
    let mut u = 0.0;
    for (i, s) in hf.seq.iter().enumerate() {
        let (xa, xb) = (hf.px(u), hf.px(u + s.3));
        // plateau level (melting / boiling point)
        if matches!(s.0, Ph::Melt | Ph::Boil) {
            let is_mp = s.0 == Ph::Melt;
            let cur = if is_mp { hf.mp } else { hf.bp };
            out.push(
                Handle::bar(((xa + xb) / 2.0, hf.py(cur)), true, if is_mp { "Melting point: drag up or down" } else { "Boiling point: drag up or down" }, move |q, t| {
                    let fr = heat_frame(q);
                    let v = fr.inv_t(t.1);
                    let now = if is_mp { fr.mp } else { fr.bp };
                    if (v - now).abs() < 0.5 { return; }
                    let (mp0, bp0) = heat_points(q);
                    q.set_s("substance", "Custom");
                    q.set_f("mp", if is_mp { v } else { mp0 });
                    q.set_f("bp", if is_mp { bp0 } else { v });
                }),
            );
        }
        // start point: temperature only
        if i == 0 {
            let first_is_lo = !hf.cooling;
            let key = key_for(first_is_lo);
            out.push(Handle::knob((xa, hf.py(s.1)), "Starting temperature: drag up or down", move |q, t| {
                let fr = heat_frame(q);
                q.set_f(key, fr.inv_t(t.1).round());
            }).with_cursor(Cursor::ResizeV));
        }
        // end of the segment
        let last = i + 1 == n;
        let dk = dur_key(s.0);
        let end_is_lo = hf.cooling;
        let end_key = key_for(end_is_lo);
        if last || hf.scaled {
            let tip = if last { "End point: drag to change the ending temperature and last segment time" } else { "Drag sideways to change this segment's time" };
            let idx = i;
            out.push(Handle::knob((xb, hf.py(s.2)), tip, move |q, t| {
                let fr = heat_frame(q);
                if fr.scaled && idx < fr.seq.len() {
                    let ustart: f64 = fr.seq[..idx].iter().map(|x| x.3).sum();
                    let w = H_X1 - H_X0 - 2.0 * H_XPAD;
                    let d = (t.0 - fr.px(ustart)) / (w / fr.total);
                    q.set_f(dk, (d * 2.0).round() / 2.0);
                }
                if last {
                    q.set_f(end_key, fr.inv_t(t.1).round());
                }
            }));
        }
        u += s.3;
    }
    out
}
