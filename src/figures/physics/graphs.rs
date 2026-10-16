//! Graph-style figures: motion graphs and waves.
use super::common::*;
use crate::figures::{FigureDef, ParamDef, Params, Subject};
use std::f64::consts::PI;

// ---------------------------------------------------------------- motion graphs

const KINDS: [&str; 5] = ["At rest", "Constant velocity forward", "Constant velocity back", "Speeding up", "Slowing down"];
const SK: [[&str; 8]; 5] = [
    ["s1_kind", "Segment 1 motion", "s1_v", "Segment 1 speed (m/s)", "s1_t", "Segment 1 lasts (s)", "s1_dir", "Segment 1 direction"],
    ["s2_kind", "Segment 2 motion", "s2_v", "Segment 2 speed (m/s)", "s2_t", "Segment 2 lasts (s)", "s2_dir", "Segment 2 direction"],
    ["s3_kind", "Segment 3 motion", "s3_v", "Segment 3 speed (m/s)", "s3_t", "Segment 3 lasts (s)", "s3_dir", "Segment 3 direction"],
    ["s4_kind", "Segment 4 motion", "s4_v", "Segment 4 speed (m/s)", "s4_t", "Segment 4 lasts (s)", "s4_dir", "Segment 4 direction"],
    ["s5_kind", "Segment 5 motion", "s5_v", "Segment 5 speed (m/s)", "s5_t", "Segment 5 lasts (s)", "s5_dir", "Segment 5 direction"],
];
const SDEF: [(&str, f64, f64); 5] = [
    ("Speeding up", 4.0, 4.0),
    ("Constant velocity forward", 4.0, 4.0),
    ("Slowing down", 0.0, 4.0),
    ("At rest", 0.0, 3.0),
    ("At rest", 0.0, 2.0),
];

fn motion_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::int("n", "Number of segments", 1, 5, 4),
        ParamDef::float("x0", "Starting position (m)", -20.0, 20.0, 1.0, 0.0),
    ];
    for (i, k) in SK.iter().enumerate() {
        v.push(ParamDef::choice(k[0], k[1], &KINDS, SDEF[i].0));
        v.push(ParamDef::float(k[2], k[3], 0.0, 30.0, 0.5, SDEF[i].1));
        v.push(ParamDef::float(k[4], k[5], 0.5, 20.0, 0.5, SDEF[i].2));
        v.push(ParamDef::choice(k[6], k[7], &["Forward", "Back"], "Forward"));
    }
    v.push(ParamDef::choice("show", "Graph", &["Position-time", "Velocity-time", "Acceleration-time", "All three, stacked"], "Position-time"));
    v.push(ParamDef::bool("letters", "Letter the segment ends A, B, C...", false));
    v.push(ParamDef::bool("styles", "Each segment in its own line style", false));
    v.push(ParamDef::text("title", "Chart title", ""));
    v.push(ParamDef::text("xt", "Time axis title", "Time (s)"));
    v.push(ParamDef::text("yt_pos", "Position axis title", "Position (m)"));
    v.push(ParamDef::text("yt_vel", "Velocity axis title", "Velocity (m/s)"));
    v.push(ParamDef::text("yt_acc", "Acceleration axis title", "Acceleration (m/s^2)"));
    v.push(ParamDef::bool("numbers", "Numbers on the axes", true));
    v.push(ParamDef::bool("grid", "Gridlines", true));
    v.push(ParamDef::bool("color", "Color (for slides)", false));
    v
}

#[derive(Clone, Copy)]
struct Seg { t0: f64, t1: f64, v0: f64, v1: f64, x0: f64 }
impl Seg {
    fn a(&self) -> f64 { (self.v1 - self.v0) / (self.t1 - self.t0) }
    fn x(&self, t: f64) -> f64 { let d = t - self.t0; self.x0 + self.v0 * d + 0.5 * self.a() * d * d }
}

fn build_segs(p: &Params) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    let (mut t, mut v, mut x) = (0.0, 0.0, p.f("x0"));
    for k in SK.iter().take(p.i("n").clamp(1, 5) as usize) {
        let dur = p.f(k[4]).max(0.1);
        let sp = p.f(k[2]);
        let sgn = if p.s(k[6]) == "Back" { -1.0 } else { 1.0 };
        let v1 = match p.s(k[0]) {
            "At rest" => 0.0,
            "Constant velocity forward" => sp,
            "Constant velocity back" => -sp,
            "Speeding up" => sgn * sp,
            _ => {
                // slowing down: target speed keeps the sign of the current motion
                let s = if v < 0.0 { -1.0 } else if v > 0.0 { 1.0 } else { sgn };
                s * sp
            }
        };
        let v0 = match p.s(k[0]) { "At rest" => 0.0, "Constant velocity forward" | "Constant velocity back" => v1, _ => v };
        let seg = Seg { t0: t, t1: t + dur, v0, v1, x0: x };
        x = seg.x(seg.t1);
        t += dur;
        v = v1;
        out.push(seg);
    }
    out
}

const DASHES: [&str; 4] = ["none", "9 5", "2 5", "10 4 2 4"];

fn motion_panel(g: &mut G, p: &Params, segs: &[Seg], which: u8, (ox, oy, w, h): (f64, f64, f64, f64), xlabel: &str) {
    let tmax = segs.last().map(|s| s.t1).unwrap_or(1.0);
    let ser = |s: &Seg, t: f64| -> f64 {
        match which { 0 => s.x(t), 1 => s.v0 + s.a() * (t - s.t0), _ => s.a() }
    };
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for s in segs {
        for t in [s.t0, (s.t0 + s.t1) / 2.0, s.t1] { let y = ser(s, t); lo = lo.min(y); hi = hi.max(y); }
    }
    if hi - lo < 1e-9 { hi = 1.0; }
    let ystep = nice_step(hi - lo, 6.0);
    let (mut ymin, mut ymax) = ((lo / ystep).floor() * ystep, (hi / ystep).ceil() * ystep);
    if which == 2 { ymin -= ystep; ymax += ystep; }
    let xstep = nice_step(tmax, 10.0);
    let xmax = (tmax / xstep).ceil() * xstep;
    let ax = Axes { x0: ox, y0: oy, w, h, xmin: 0.0, xmax, ymin, ymax };
    let yl = match which { 0 => p.s("yt_pos"), 1 => p.s("yt_vel"), _ => p.s("yt_acc") };
    ax.draw(g, xstep, ystep, p.b("grid"), p.b("numbers"), &supers(xlabel), &supers(yl), 13.0);
    let col = p.b("color");
    let stroke = if col { PAL[1] } else { INK };
    for (i, s) in segs.iter().enumerate() {
        let pts: Vec<Pt> = if which == 2 {
            vec![(ax.px(s.t0), ax.py(s.a())), (ax.px(s.t1), ax.py(s.a()))]
        } else {
            (0..=40).map(|k| { let t = s.t0 + (s.t1 - s.t0) * k as f64 / 40.0; (ax.px(t), ax.py(ser(s, t))) }).collect()
        };
        let dash = if p.b("styles") { DASHES[i % 4] } else { "none" };
        let c = if col && p.b("styles") { PAL[i % 6] } else { stroke };
        if dash == "none" { g.polyline(&pts, c, 3.0); } else { g.pattern_polyline(&pts, c, 3.0, dash); }
    }
    if p.b("letters") && which != 2 {
        let letters = ["A", "B", "C", "D", "E", "F"];
        for i in 0..=segs.len() {
            let (t, y) = if i == 0 { (segs[0].t0, ser(&segs[0], segs[0].t0)) } else { (segs[i - 1].t1, ser(&segs[i - 1], segs[i - 1].t1)) };
            let (px, py) = (ax.px(t), ax.py(y));
            g.circle(px, py, 4.0, INK, INK, 1.0);
            let lx = if i == 0 { px + 14.0 } else { px - 6.0 };
            g.text(lx, py - 10.0, 15.0, if i == 0 { "start" } else { "end" }, letters[i], true);
        }
    }
}

fn motion_render(p: &Params) -> String {
    let segs = build_segs(p);
    let stacked = p.s("show") == "All three, stacked";
    let (w, h) = (580.0, if stacked { 780.0 } else { 420.0 });
    let mut g = G::new(w, h, false);
    let mut top = 28.0;
    if !p.s("title").trim().is_empty() {
        g.text(w / 2.0 + 20.0, 28.0, 19.0, "middle", &supers(p.s("title")), true);
        top = 52.0;
    }
    if stacked {
        let ph = (h - top - 30.0 - 3.0 * 62.0) / 3.0;
        for i in 0..3 {
            let oy = top + i as f64 * (ph + 62.0);
            motion_panel(&mut g, p, &segs, i as u8, (100.0, oy, 440.0, ph), if i == 2 { p.s("xt") } else { "" });
        }
    } else {
        let which = match p.s("show") { "Velocity-time" => 1, "Acceleration-time" => 2, _ => 0 };
        motion_panel(&mut g, p, &segs, which, (100.0, top, 440.0, h - top - 90.0), p.s("xt"));
    }
    g.finish()
}

// ---------------------------------------------------------------- waves

fn wave_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Wave", &["Transverse", "Longitudinal", "Both"], "Transverse"),
        ParamDef::choice("against", "Displacement against", &["Distance", "Time"], "Distance"),
        ParamDef::float("amp", "Amplitude", 0.5, 8.0, 0.5, 3.0),
        ParamDef::float("wl", "Wavelength", 1.0, 8.0, 0.5, 4.0),
        ParamDef::int("cycles", "Cycles", 1, 5, 2),
        ParamDef::bool("mark_wl", "Mark the wavelength", true),
        ParamDef::text("wl_lab", "Wavelength label", "lambda"),
        ParamDef::bool("mark_amp", "Mark the amplitude", true),
        ParamDef::text("amp_lab", "Amplitude label", "A"),
        ParamDef::bool("mark_ct", "Label crest and trough", false),
        ParamDef::bool("numbers", "Numbered axes, to measure on", true),
        ParamDef::bool("grid", "Gridlines (tick marks without)", true),
        ParamDef::text("title", "Chart title", ""),
        ParamDef::text("xt", "x-axis title (blank = automatic)", ""),
        ParamDef::text("yt", "y-axis title (blank = automatic)", ""),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

fn wave_render(p: &Params) -> String {
    let kind = p.s("kind").to_string();
    let time = p.s("against") == "Time";
    let (a, wl) = (p.f("amp"), p.f("wl").max(0.5));
    let cyc = p.i("cycles").clamp(1, 5) as f64;
    let xmax = wl * cyc;
    let col = p.b("color");
    let curve = if col { PAL[1] } else { INK };
    let trans = kind != "Longitudinal";
    let long = kind != "Transverse";
    let w = 600.0;
    let h = match kind.as_str() { "Transverse" => 400.0, "Longitudinal" => 250.0, _ => 530.0 };
    let mut g = G::new(w, h, false);
    let mut top = 24.0;
    if !p.s("title").trim().is_empty() {
        g.text(w / 2.0 + 20.0, 28.0, 19.0, "middle", &supers(p.s("title")), true);
        top = 52.0;
    }
    let xl = if p.s("xt").trim().is_empty() { if time { "Time (s)" } else { "Distance (m)" } } else { p.s("xt") };
    let yl = if p.s("yt").trim().is_empty() { "Displacement (cm)" } else { p.s("yt") };
    let xstep = nice_step(xmax, 10.0);
    let (ox, pw) = (100.0, 460.0);
    let wl_default = if time { "T" } else { "lambda" };
    let wl_lab = if p.s("wl_lab") == "lambda" && time { wl_default } else { p.s("wl_lab") };
    let mut y_cursor = top;
    if trans {
        let ph = if kind == "Both" { 260.0 } else { h - top - 85.0 };
        let ystep = nice_step(a * 2.6, 8.0);
        let ymax = (a * 1.35 / ystep).ceil() * ystep;
        let ax = Axes { x0: ox, y0: y_cursor, w: pw, h: ph, xmin: 0.0, xmax, ymin: -ymax, ymax };
        ax.draw(&mut g, xstep, ystep, p.b("grid"), p.b("numbers"), if kind == "Both" { "" } else { xl }, yl, 13.0);
        let pts: Vec<Pt> = (0..=400).map(|i| { let x = xmax * i as f64 / 400.0; (ax.px(x), ax.py(a * (2.0 * PI * x / wl).sin())) }).collect();
        g.polyline(&pts, curve, 3.2);
        let mc = if col { PAL[0] } else { INK };
        if p.b("mark_amp") {
            let xa = ax.px(wl / 4.0);
            g.dim_arrow(xa, ax.py(0.0) - 1.0, xa, ax.py(a), mc, 1.5);
            g.label_at(xa + 16.0, (ax.py(0.0) + ax.py(a)) / 2.0, 20.0, p.s("amp_lab"), ITALIC);
        }
        if p.b("mark_wl") {
            let (s, e) = if xmax >= wl * 1.25 - 1e-9 { (wl / 4.0, wl * 1.25) } else { (0.0, wl) };
            let y = ax.py(a + (ymax - a) * 0.5);
            g.dash(ax.px(s), ax.py(if s == 0.0 { 0.0 } else { a }), ax.px(s), y - 6.0, GREY, 1.0);
            g.dash(ax.px(e), ax.py(if s == 0.0 { 0.0 } else { a }), ax.px(e), y - 6.0, GREY, 1.0);
            g.dim_arrow(ax.px(s), y, ax.px(e), y, mc, 1.5);
            g.label_at((ax.px(s) + ax.px(e)) / 2.0, y - 14.0, 20.0, wl_lab, ITALIC);
        }
        if p.b("mark_ct") {
            let x = ax.px(wl / 4.0);
            g.text(x, ax.py(a) - 10.0, 15.0, "middle", "crest", false);
            if xmax >= wl * 0.75 { g.text(ax.px(wl * 0.75), ax.py(-a) + 22.0, 15.0, "middle", "trough", false); }
        }
        y_cursor += ph + if kind == "Both" { 46.0 } else { 0.0 };
    }
    if long {
        let bh = 70.0;
        let by = y_cursor + 20.0;
        let ax = Axes { x0: ox, y0: by, w: pw, h: bh, xmin: 0.0, xmax, ymin: 0.0, ymax: 1.0 };
        let n = (cyc * 26.0) as usize;
        let k = 2.0 * PI / wl;
        let dmax = 0.12 * wl;
        for i in 0..=n {
            let x0 = xmax * i as f64 / n as f64;
            let x = (x0 - dmax * (k * (x0 - wl / 4.0)).sin()).clamp(0.0, xmax);
            let px = ax.px(x);
            g.line(px, by + 4.0, px, by + bh - 4.0, if col { PAL[1] } else { INK }, 1.6);
        }
        g.polyline(&[(ox, by), (ox + pw, by), (ox + pw, by + bh), (ox, by + bh), (ox, by)], INK, 1.2);
        // x axis beneath the strip
        let ay = by + bh + 14.0;
        g.line(ox, ay, ox + pw, ay, INK, 2.4);
        let mut xv = 0.0;
        while xv <= xmax + 1e-6 {
            g.line(ax.px(xv), ay, ax.px(xv), ay + 6.0, INK, 1.4);
            if p.b("numbers") { g.text(ax.px(xv), ay + 22.0, 13.0, "middle", &fmt_num(xv), true); }
            xv += xstep;
        }
        g.text(ox + pw / 2.0, ay + 46.0, 16.0, "middle", xl, true);
        if p.b("mark_ct") {
            let wide = wl / xmax * pw * cyc > 0.0 && (wl * 0.5 / xmax * pw) > 100.0;
            for c in 0..(cyc as usize) {
                let xc = wl / 4.0 + wl * c as f64;
                g.text(ax.px(xc), by - 6.0, 14.0, "middle", if wide { "compression" } else { "C" }, false);
                let xr = wl * 0.75 + wl * c as f64;
                if xr <= xmax { g.text(ax.px(xr), by - 6.0, 14.0, "middle", if wide { "rarefaction" } else { "R" }, false); }
            }
        }
        if p.b("mark_wl") && !trans {
            let (s, e) = if xmax >= wl * 1.25 - 1e-9 { (wl / 4.0, wl * 1.25) } else { (0.0, wl) };
            let y = by - 26.0;
            g.dim_arrow(ax.px(s), y, ax.px(e), y, INK, 1.5);
            g.label_at((ax.px(s) + ax.px(e)) / 2.0, y - 14.0, 20.0, wl_lab, ITALIC);
        }
    }
    g.finish()
}

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "motion-graphs", subject: Subject::Physics, title: "Motion Graphs",
            blurb: "Position-, velocity- and acceleration-time graphs built from up to five motion segments (rest, constant velocity, speeding up, slowing down).",
            params: motion_params, render: motion_render },
        FigureDef { id: "waves", subject: Subject::Physics, title: "Waves",
            blurb: "Transverse and longitudinal waves with amplitude and wavelength marks on measurable, gridded axes.",
            params: wave_params, render: wave_render },
    ]
}
