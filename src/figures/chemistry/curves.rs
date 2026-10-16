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
    ]
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
    let kind = p.s("kind");
    let (ca, va, ct) = (p.f("molarity").max(1e-6), p.f("volume").max(0.1), p.f("titrant").max(1e-6));
    let pk = p.f("pk");
    let acid = kind.contains("acid with");
    let acid_analyte = kind.starts_with("Strong acid") || kind.starts_with("Weak acid");
    let _ = acid;
    let veq = ca * va / ct;
    let xmax_raw = veq * 2.0;
    let step_x = nice_step(xmax_raw, 10.0);
    let xmax = (xmax_raw / step_x).ceil() * step_x;
    let (w, h) = (640.0, 480.0);
    let (x0, x1, y0, y1) = (84.0, 600.0, 40.0, 400.0); // y0 top (pH14), y1 bottom (pH 0)
    let px = |v: f64| x0 + v / xmax * (x1 - x0);
    let py = |ph: f64| y1 - ph / 14.0 * (y1 - y0);
    let mut d = Doc::new(w, h);
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
    tx(&mut d, (x0 + x1) / 2.0, y1 + 50.0, 15.0, "middle", INK, false, &format!("Volume of {titr_name} added (mL)"));
    tr(&mut d, 28.0, (y0 + y1) / 2.0, 15.0, -90.0, "middle", "pH");
    // curve
    let n = 400;
    let pts: Vec<(f64, f64)> = (0..=n).map(|i| {
        let v = xmax * i as f64 / n as f64;
        (px(v), py(titration_ph(kind, ca, va, ct, pk, v)))
    }).collect();
    d.polyline(&pts, "#1f5fa8", 2.6);
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
        tx(&mut d, ex + 12.0, if up { ey - 12.0 } else { ey + 24.0 }, 13.0, "start", INK, false, "equivalence point");
    }
    if weak && p.b("half") {
        let hv = veq / 2.0;
        let hp = titration_ph(kind, ca, va, ct, pk, hv);
        let (hx, hy) = (px(hv), py(hp));
        dashed(&mut d, hx, hy, hx, y1, "#666", 1.1);
        dashed(&mut d, x0, hy, hx, hy, "#666", 1.1);
        d.circle(hx, hy, 5.0, "#2e8b57", INK, 1.2);
        tx(&mut d, hx + 12.0, if acid_analyte { hy + 22.0 } else { hy - 12.0 }, 13.0, "start", INK, false, "half-equivalence point");
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
    ]
}

#[derive(Clone, Copy, PartialEq)]
enum Ph { Solid, Melt, Liquid, Boil, Gas }

fn heat_render(p: &Params) -> String {
    let (mut mp, mut bp) = match SUBST.iter().find(|s| s.0 == p.s("substance")) {
        Some(s) => (s.1, s.2), None => (p.f("mp"), p.f("bp")),
    };
    if bp <= mp { bp = mp + 50.0; }
    let _ = &mut mp;
    let a = p.f("start");
    let b = p.f("end");
    let (lo, hi) = (a.min(b), a.max(b));
    let hi = if hi - lo < 10.0 { lo + 10.0 } else { hi };
    let cooling = p.s("mode") == "Cooling";
    // heating-order stages: (phase, T from, T to, minutes)
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
    let ymin = (lo / ystep).floor() * ystep - if (lo / ystep).fract() == 0.0 { ystep * 0.0 } else { 0.0 };
    let ymax = (hi / ystep).ceil() * ystep + ystep * 0.5;
    let (w, h) = (750.0, 480.0);
    let (x0, x1, y0, y1) = (110.0, 660.0, 40.0, 400.0);
    let py = |t: f64| y1 - (t - ymin) / (ymax - ymin) * (y1 - y0);
    let xpad = 20.0;
    let px = |u: f64| x0 + xpad + u / total * (x1 - x0 - 2.0 * xpad);
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
    tr(&mut d, 28.0, (y0 + y1) / 2.0, 15.0, -90.0, "middle", "Temperature (°C)");
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
    d.polyline(&pts, "#1f5fa8", 2.8);
    if p.b("labels") {
        let mut u = 0.0;
        for s in &seq {
            let (xa, xb) = (px(u), px(u + s.3));
            u += s.3;
            let xm = (xa + xb) / 2.0;
            let (ya, yb) = (py(s.1), py(s.2));
            let ym = (ya + yb) / 2.0;
            let (name, plateau) = match s.0 {
                Ph::Solid => ("solid", false), Ph::Liquid => ("liquid", false), Ph::Gas => ("gas", false),
                Ph::Melt => ("solid + liquid", true), Ph::Boil => ("liquid + gas", true),
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
