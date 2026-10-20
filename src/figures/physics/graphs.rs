//! Graph-style figures: motion graphs and waves.
use super::common::*;
use crate::figures::{FigureDef, ParamDef, Params, Subject};
use std::f64::consts::PI;

// ---------------------------------------------------------------- motion graphs

const KINDS: [&str; 5] = ["At rest", "Constant velocity forward", "Constant velocity back", "Speeding up", "Slowing down"];
pub(super) const SK: [[&str; 8]; 6] = [
    ["s1_kind", "Segment 1 motion", "s1_v", "Segment 1 speed (m/s)", "s1_t", "Segment 1 lasts (s)", "s1_dir", "Segment 1 direction"],
    ["s2_kind", "Segment 2 motion", "s2_v", "Segment 2 speed (m/s)", "s2_t", "Segment 2 lasts (s)", "s2_dir", "Segment 2 direction"],
    ["s3_kind", "Segment 3 motion", "s3_v", "Segment 3 speed (m/s)", "s3_t", "Segment 3 lasts (s)", "s3_dir", "Segment 3 direction"],
    ["s4_kind", "Segment 4 motion", "s4_v", "Segment 4 speed (m/s)", "s4_t", "Segment 4 lasts (s)", "s4_dir", "Segment 4 direction"],
    ["s5_kind", "Segment 5 motion", "s5_v", "Segment 5 speed (m/s)", "s5_t", "Segment 5 lasts (s)", "s5_dir", "Segment 5 direction"],
    ["s6_kind", "Segment 6 motion", "s6_v", "Segment 6 speed (m/s)", "s6_t", "Segment 6 lasts (s)", "s6_dir", "Segment 6 direction"],
];
const SDEF: [(&str, f64, f64); 6] = [
    ("Speeding up", 4.0, 4.0),
    ("Constant velocity forward", 4.0, 4.0),
    ("Slowing down", 0.0, 4.0),
    ("At rest", 0.0, 3.0),
    ("At rest", 0.0, 2.0),
    ("At rest", 0.0, 3.0),
];

const TMODES: [&str; 3] = ["Text", "Blank line", "None"];

fn motion_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::int("n", "Number of segments", 1, 6, 4),
        ParamDef::float("x0", "Starting position (m)", -50.0, 50.0, 1.0, 0.0),
    ];
    for (i, k) in SK.iter().enumerate() {
        v.push(ParamDef::choice(k[0], k[1], &KINDS, SDEF[i].0));
        v.push(ParamDef::float(k[2], k[3], 0.0, 100.0, 0.5, SDEF[i].1));
        v.push(ParamDef::float(k[4], k[5], 0.1, 60.0, 0.5, SDEF[i].2));
        v.push(ParamDef::choice(k[6], k[7], &["Forward", "Back"], "Forward"));
    }
    v.push(ParamDef::choice("show", "Graph", &["Position-time", "Velocity-time", "Acceleration-time", "All three, stacked"], "Position-time"));
    v.push(ParamDef::bool("letters", "Letter the segment ends A, B, C...", false));
    v.push(ParamDef::bool("styles", "Each segment in its own line style", false));
    v.push(ParamDef::bool("tangent", "Tangent to the position-time graph", false));
    v.push(ParamDef::float("tangent_t", "Tangent at (s)", 0.0, 300.0, 0.5, 2.0));
    v.push(ParamDef::choice("title_mode", "Chart title shows", &TMODES, "Text"));
    v.push(ParamDef::text("title", "Chart title", ""));
    v.push(ParamDef::choice("xt_mode", "Time axis title shows", &TMODES, "Text"));
    v.push(ParamDef::text("xt", "Time axis title", "Time (s)"));
    v.push(ParamDef::choice("yt_pos_mode", "Position axis title shows", &TMODES, "Text"));
    v.push(ParamDef::text("yt_pos", "Position axis title", "Position (m)"));
    v.push(ParamDef::choice("yt_vel_mode", "Velocity axis title shows", &TMODES, "Text"));
    v.push(ParamDef::text("yt_vel", "Velocity axis title", "Velocity (m/s)"));
    v.push(ParamDef::choice("yt_acc_mode", "Acceleration axis title shows", &TMODES, "Text"));
    v.push(ParamDef::text("yt_acc", "Acceleration axis title", "Acceleration (m/s^2)"));
    v.push(ParamDef::bool("numbers", "Numbers on the axes", true));
    v.push(ParamDef::bool("grid", "Gridlines", true));
    v.push(ParamDef::bool("ranges", "Set the ranges myself", false));
    v.push(ParamDef::float("t_min", "Time from (s)", -1000.0, 1000.0, 0.5, 0.0));
    v.push(ParamDef::float("t_max", "Time to (s)", -1000.0, 1000.0, 0.5, 15.0));
    v.push(ParamDef::float("t_step", "Time count by (s)", 0.01, 1000.0, 0.5, 1.0));
    v.push(ParamDef::float("p_min", "Position from (m)", -10000.0, 10000.0, 1.0, 0.0));
    v.push(ParamDef::float("p_max", "Position to (m)", -10000.0, 10000.0, 1.0, 35.0));
    v.push(ParamDef::float("p_step", "Position count by (m)", 0.01, 10000.0, 1.0, 5.0));
    v.push(ParamDef::float("v_min", "Velocity from (m/s)", -1000.0, 1000.0, 0.5, -5.0));
    v.push(ParamDef::float("v_max", "Velocity to (m/s)", -1000.0, 1000.0, 0.5, 5.0));
    v.push(ParamDef::float("v_step", "Velocity count by (m/s)", 0.01, 1000.0, 0.5, 1.0));
    v.push(ParamDef::float("a_min", "Acceleration from (m/s²)", -1000.0, 1000.0, 0.5, -1.5));
    v.push(ParamDef::float("a_max", "Acceleration to (m/s²)", -1000.0, 1000.0, 0.5, 1.5));
    v.push(ParamDef::float("a_step", "Acceleration count by (m/s²)", 0.01, 1000.0, 0.5, 0.5));
    v.push(ParamDef::bool("color", "Color (for slides)", false));
    v
}

#[derive(Clone, Copy)]
pub(super) struct Seg { pub t0: f64, pub t1: f64, pub v0: f64, pub v1: f64, pub x0: f64 }
impl Seg {
    pub(super) fn a(&self) -> f64 { (self.v1 - self.v0) / (self.t1 - self.t0) }
    pub(super) fn x(&self, t: f64) -> f64 { let d = t - self.t0; self.x0 + self.v0 * d + 0.5 * self.a() * d * d }
}

pub(super) fn build_segs(p: &Params) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    let (mut t, mut v, mut x) = (0.0, 0.0, p.f("x0"));
    for k in SK.iter().take(p.i("n").clamp(1, 6) as usize) {
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

/// Value of series `which` (0 position, 1 velocity, 2 acceleration) of segment `s` at time `t`.
pub(super) fn series(s: &Seg, which: u8, t: f64) -> f64 {
    match which { 0 => s.x(t), 1 => s.v0 + s.a() * (t - s.t0), _ => s.a() }
}

/// Axes (and tick steps) of one motion panel placed at (ox, oy, w, h); manual ranges when
/// "Set the ranges myself" is on (invalid ones fall back to the fitted range).
pub(super) fn motion_axes(p: &Params, segs: &[Seg], which: u8, (ox, oy, w, h): (f64, f64, f64, f64)) -> (Axes, f64, f64) {
    let tmax = segs.last().map(|s| s.t1).unwrap_or(1.0);
    let ser = |s: &Seg, t: f64| series(s, which, t);
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for s in segs {
        for t in [s.t0, (s.t0 + s.t1) / 2.0, s.t1] { let y = ser(s, t); lo = lo.min(y); hi = hi.max(y); }
    }
    if hi - lo < 1e-9 { hi = 1.0; }
    let mut ystep = nice_step(hi - lo, 7.0);
    let (mut ymin, mut ymax) = ((lo / ystep).floor() * ystep, (hi / ystep).ceil() * ystep);
    if which == 2 { ymin -= ystep; ymax += ystep; }
    // whole seconds for everyday durations, like the reference
    let mut xstep = if (4.0..=20.0).contains(&tmax) { 1.0 } else { nice_step(tmax, 12.0) };
    let mut xmin = 0.0;
    let mut xmax = (tmax / xstep).ceil() * xstep;
    if p.b("ranges") {
        let ok = |lo: f64, hi: f64, st: f64| hi > lo && st > 0.0 && (hi - lo) / st <= 300.0;
        let (a, b, c) = (p.f("t_min"), p.f("t_max"), p.f("t_step"));
        if ok(a, b, c) { xmin = a; xmax = b; xstep = c; }
        let pre = match which { 0 => "p", 1 => "v", _ => "a" };
        let (a, b, c) = (p.f(&format!("{pre}_min")), p.f(&format!("{pre}_max")), p.f(&format!("{pre}_step")));
        if ok(a, b, c) { ymin = a; ymax = b; ystep = c; }
    }
    (Axes { x0: ox, y0: oy, w, h, xmin, xmax, ymin, ymax }, xstep, ystep)
}

/// Canvas size and the panels (which series, rect, is-last) drawn for the current params.
pub(super) fn motion_layout(p: &Params) -> ((f64, f64), Vec<(u8, (f64, f64, f64, f64))>, f64) {
    let stacked = p.s("show") == "All three, stacked";
    let (w, h) = (580.0, if stacked { 780.0 } else { 420.0 });
    let top = if has_title(p) { 52.0 } else { 28.0 };
    if stacked {
        let ph = (h - top - 30.0 - 3.0 * 62.0) / 3.0;
        ((w, h), (0..3).map(|i| (i as u8, (100.0, top + i as f64 * (ph + 62.0), 440.0, ph))).collect(), top)
    } else {
        let which = match p.s("show") { "Velocity-time" => 1, "Acceleration-time" => 2, _ => 0 };
        ((w, h), vec![(which, (100.0, top, 440.0, h - top - 90.0))], top)
    }
}

fn motion_panel(g: &mut G, p: &Params, segs: &[Seg], which: u8, rect: (f64, f64, f64, f64), show_xt: bool) {
    let (ax, xstep, ystep) = motion_axes(p, segs, which, rect);
    let ser = |s: &Seg, t: f64| series(s, which, t);
    let (ykey, ymode) = match which { 0 => ("yt_pos", "yt_pos_mode"), 1 => ("yt_vel", "yt_vel_mode"), _ => ("yt_acc", "yt_acc_mode") };
    let (l, r, t, b) = (ax.x0, ax.x0 + ax.w, ax.y0, ax.y0 + ax.h);
    let (numbers, grid) = (p.b("numbers"), p.b("grid"));
    let fs = 13.0;
    let ticks = |lo: f64, hi: f64, st: f64| -> Vec<f64> {
        let mut v = Vec::new();
        let mut k = (lo / st - 1e-9).ceil();
        while k * st <= hi + st * 1e-6 { v.push(k * st); k += 1.0; }
        v
    };
    let zero_y = if ax.ymin <= 0.0 && ax.ymax >= 0.0 { ax.py(0.0) } else { b };
    let zero_x = if ax.xmin <= 0.0 && ax.xmax >= 0.0 { ax.px(0.0) } else { l };
    if grid {
        for xv in ticks(ax.xmin, ax.xmax, xstep) { let x = ax.px(xv); g.line(x, t, x, b, "#555", 0.8); }
        for yv in ticks(ax.ymin, ax.ymax, ystep) { let y = ax.py(yv); g.line(l, y, r, y, "#555", 0.8); }
        g.polyline(&[(l, t), (r, t), (r, b), (l, b), (l, t)], INK, 1.2);
        g.line(zero_x, t, zero_x, b, INK, 2.4);
        g.line(l, zero_y, r, zero_y, INK, 2.4);
    } else if numbers {
        g.line(zero_x, t, zero_x, b, INK, 2.4);
        g.line(l, zero_y, r, zero_y, INK, 2.4);
        for xv in ticks(ax.xmin, ax.xmax, xstep) { let x = ax.px(xv); g.line(x, zero_y - 4.0, x, zero_y + 4.0, INK, 1.3); }
        for yv in ticks(ax.ymin, ax.ymax, ystep) { let y = ax.py(yv); g.line(zero_x - 4.0, y, zero_x + 4.0, y, INK, 1.3); }
    } else {
        // shapes only: plain arrowed axes
        g.arrow(zero_x, b + 2.0, zero_x, t - 14.0, INK, 2.2);
        g.arrow(l - 2.0, zero_y, r + 14.0, zero_y, INK, 2.2);
    }
    if numbers {
        let along = if zero_y < b - 1.0 { zero_y } else { b };
        for xv in ticks(ax.xmin, ax.xmax, xstep) {
            if xv.abs() < 1e-9 && zero_y < b - 1.0 { continue; }
            g.text(ax.px(xv), along + fs + 5.0, fs, "middle", &fmt_num(xv).replace('-', "−"), true);
        }
        for yv in ticks(ax.ymin, ax.ymax, ystep) {
            if yv.abs() < 1e-9 && zero_y >= b - 1.0 && ax.xmin.abs() < 1e-9 { continue; }
            g.text(l - 7.0, ax.py(yv) + fs * 0.35, fs, "end", &fmt_num(yv).replace('-', "−"), true);
        }
    }
    let ytxt = lab_mode(p, ykey, ymode);
    let ylx = l - fs * 3.2 - 8.0;
    if ytxt.is_empty() { g.line(ylx, (t + b) / 2.0 - 50.0, ylx, (t + b) / 2.0 + 50.0, INK, 1.5); }
    else if ytxt != "none" { g.vtext(ylx, (t + b) / 2.0, fs + 3.0, &supers(&ytxt), true); }
    if show_xt {
        let xtxt = lab_mode(p, "xt", "xt_mode");
        let y = b + fs * 2.0 + 14.0;
        if xtxt.is_empty() { g.line((l + r) / 2.0 - 60.0, y + 2.0, (l + r) / 2.0 + 60.0, y + 2.0, INK, 1.5); }
        else if xtxt != "none" { g.text((l + r) / 2.0, y, fs + 3.0, "middle", &supers(&xtxt), true); }
    }
    let col = p.b("color");
    let stroke = if col { PAL[1] } else { INK };
    g.raw(&format!(r#"<clipPath id="mclip{which}"><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}"/></clipPath><g clip-path="url(#mclip{which})">"#, l - 3.0, t - 3.0, ax.w + 6.0, ax.h + 6.0));
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
    // tangent to the position-time graph
    if which == 0 && p.b("tangent") && !segs.is_empty() {
        let tt = p.f("tangent_t").clamp(segs[0].t0, segs.last().unwrap().t1);
        let s = segs.iter().find(|s| tt <= s.t1 + 1e-9).unwrap_or(segs.last().unwrap());
        let (x, v) = (s.x(tt), s.v0 + s.a() * (tt - s.t0));
        let (ta, tb) = (ax.xmin, ax.xmax);
        g.pattern_polyline(&[(ax.px(ta), ax.py(x + v * (ta - tt))), (ax.px(tb), ax.py(x + v * (tb - tt)))], INK, 2.2, "9 6");
        g.circle(ax.px(tt), ax.py(x), 5.0, INK, INK, 1.0);
    }
    g.raw("</g>");
    if p.b("letters") {
        let letters = ["A", "B", "C", "D", "E", "F", "G"];
        for i in 0..=segs.len() {
            let (tm, y) = if which == 2 {
                // acceleration: letter each step at its start (A..), the last at its end
                let s = &segs[i.min(segs.len() - 1)];
                (if i < segs.len() { s.t0 } else { s.t1 }, s.a())
            } else if i == 0 { (segs[0].t0, ser(&segs[0], segs[0].t0)) } else { (segs[i - 1].t1, ser(&segs[i - 1], segs[i - 1].t1)) };
            let (px, py) = (ax.px(tm), ax.py(y));
            if py < t - 1.0 || py > b + 1.0 { continue; }
            if which != 2 { g.circle(px, py, 4.0, INK, INK, 1.0); }
            let lx = if i == 0 { px + 12.0 } else { px - 6.0 };
            g.text(lx, py - 10.0, 15.0, if i == 0 { "start" } else { "end" }, letters[i], true);
        }
    }
}

fn motion_render(p: &Params) -> String {
    let segs = build_segs(p);
    let ((w, h), panels, _) = motion_layout(p);
    let mut g = G::new(w, h, false);
    draw_title(&mut g, p, w / 2.0 + 20.0, 28.0);
    let np = panels.len();
    for (k, (which, rect)) in panels.into_iter().enumerate() {
        motion_panel(&mut g, p, &segs, which, rect, k + 1 == np);
    }
    g.finish()
}

// ---------------------------------------------------------------- waves

const WMODES: [&str; 3] = ["Text", "Blank line", "None"];
const NUMS: [&str; 5] = ["Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"];

fn wave_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Wave", &["Transverse", "Longitudinal", "Both"], "Transverse"),
        ParamDef::choice("against", "Displacement against", &["Distance", "Time"], "Distance"),
        ParamDef::float("amp", "Amplitude (axis units)", 0.1, 100.0, 0.1, 3.0),
        ParamDef::float("wl", "Wavelength / period (axis units)", 0.1, 100.0, 0.1, 4.0),
        ParamDef::float("cycles", "Cycles", 0.5, 8.0, 0.5, 2.0),
        ParamDef::bool("mark_wl", "Mark the wavelength", true),
        ParamDef::choice("wl_lab_mode", "Wavelength label shows", &WMODES, "Text"),
        ParamDef::text("wl_lab", "Wavelength label", "lambda"),
        ParamDef::bool("mark_amp", "Mark the amplitude", true),
        ParamDef::choice("amp_lab_mode", "Amplitude label shows", &WMODES, "Text"),
        ParamDef::text("amp_lab", "Amplitude label", "A"),
        ParamDef::bool("mark_ct", "Label crest and trough (compression and rarefaction)", false),
        ParamDef::choice("crest_mode", "Crest / compression label", &WMODES, "Text"),
        ParamDef::choice("trough_mode", "Trough / rarefaction label", &WMODES, "Text"),
        ParamDef::bool("numbers", "Numbered axes, to measure on", true),
        ParamDef::bool("grid", "Gridlines (tick marks without)", true),
        ParamDef::choice("minor", "Minor gridlines", &["None", "2 per block", "4 per block", "5 per block"], "None"),
        ParamDef::choice("title_mode", "Chart title shows", &WMODES, "Text"),
        ParamDef::text("title", "Chart title", ""),
        ParamDef::choice("xt_mode", "x-axis title shows", &WMODES, "Text"),
        ParamDef::text("xt", "x-axis title (blank = automatic)", ""),
        ParamDef::choice("yt_mode", "y-axis title shows", &WMODES, "Text"),
        ParamDef::text("yt", "y-axis title (blank = automatic)", ""),
        ParamDef::bool("xaxis_fit", "x-axis: fit to the cycles drawn", true),
        ParamDef::float("xmin", "x-axis from", -1000.0, 1000.0, 0.1, 0.0),
        ParamDef::float("xmax", "x-axis to", -1000.0, 1000.0, 0.1, 8.0),
        ParamDef::float("xstep", "x-axis count by", 0.001, 1000.0, 0.1, 0.5),
        ParamDef::choice("xaxis_num", "x-axis numbers", &NUMS, "Every 2nd line"),
        ParamDef::choice("xaxis_letter", "x-axis letter (no gridlines)", &["Text", "None"], "Text"),
        ParamDef::bool("yaxis_fit", "y-axis: fit to the amplitude and marks", true),
        ParamDef::float("ymin", "y-axis from", -1000.0, 1000.0, 0.1, -5.0),
        ParamDef::float("ymax", "y-axis to", -1000.0, 1000.0, 0.1, 5.0),
        ParamDef::float("ystep", "y-axis count by", 0.001, 1000.0, 0.1, 1.0),
        ParamDef::choice("yaxis_num", "y-axis numbers", &NUMS, "Every line"),
        ParamDef::choice("yaxis_letter", "y-axis letter (no gridlines)", &["Text", "None"], "Text"),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

/// Geometry of the wave figure, shared with the drag handles.
pub(super) struct WLay {
    pub w: f64,
    pub h: f64,
    /// Transverse graph (data -> pixel mapping), if drawn.
    pub panel: Option<Axes>,
    pub xstep: f64,
    pub ystep: f64,
    /// Longitudinal strip (top y, height) and its own x mapping.
    pub strip: Option<(f64, f64)>,
    pub sx: Axes,
}

/// Snap a manual axis to something drawable.
fn axis_range(lo: f64, hi: f64, st: f64, fallback: (f64, f64, f64)) -> (f64, f64, f64) {
    if !(hi > lo) || !(st > 0.0) || (hi - lo) / st > 400.0 { fallback } else { (lo, hi, st) }
}

pub(super) fn wave_layout(p: &Params) -> WLay {
    let kind = p.s("kind");
    let (a, wl) = (p.f("amp").max(0.001), p.f("wl").max(0.001));
    let cyc = p.f("cycles").clamp(0.5, 8.0);
    let span = wl * cyc;
    let fx = (0.0, span, nice_step(span, 20.0));
    let (xmin, xmax, xstep) = if p.b("xaxis_fit") { fx } else { axis_range(p.f("xmin"), p.f("xmax"), p.f("xstep"), fx) };
    let ys = nice_step(2.0 * a, 8.0);
    let ym = ((a / ys) + 1.5).ceil() * ys;
    let fy = (-ym, ym, ys);
    let (ymin, ymax, ystep) = if p.b("yaxis_fit") { fy } else { axis_range(p.f("ymin"), p.f("ymax"), p.f("ystep"), fy) };
    let w = 600.0;
    // square gridlines, as the reference draws them (clamped so odd axis settings stay usable)
    let cells = |lo: f64, hi: f64, st: f64| ((hi - lo) / st).max(1.0);
    let ph = (470.0 * cells(ymin, ymax, ystep) / cells(xmin, xmax, xstep)).clamp(200.0, 380.0);
    let h = match kind { "Transverse" => 0.0, "Longitudinal" => 260.0, _ => 0.0 };
    let top = if has_title(p) { 54.0 } else { 22.0 };
    let (x0, pw) = (90.0, 470.0);
    let mut y = top;
    let strip = if kind != "Transverse" {
        let s = (y + if kind == "Both" { 26.0 } else { 34.0 }, 64.0);
        y = s.0 + s.1 + 36.0;
        Some(s)
    } else { None };
    let h = if h == 0.0 { y + 6.0 + ph + 62.0 } else { h };
    let panel = if kind != "Longitudinal" {
        Some(Axes { x0, y0: y + 6.0, w: pw, h: ph, xmin, xmax, ymin, ymax })
    } else { None };
    let sx = Axes { x0, y0: 0.0, w: pw, h: 1.0, xmin, xmax, ymin: 0.0, ymax: 1.0 };
    WLay { w, h, panel, xstep, ystep, strip, sx }
}

fn every(s: &str) -> usize {
    match s { "Every 2nd line" => 2, "Every 5th line" => 5, "Every 10th line" => 10, "No numbers" => 0, _ => 1 }
}

/// Axis title honouring its Text / Blank line / None mode (`auto` used for blank text).
fn axis_title(p: &Params, key: &str, mode: &str, auto: &str) -> Option<String> {
    match p.s(mode) {
        "None" => None,
        "Blank line" => Some(String::new()),
        _ => Some(if p.s(key).trim().is_empty() { auto.to_string() } else { p.s(key).to_string() }),
    }
}

/// The wave page's collapsed-row summaries ("Transverse · A = 3 · λ = 4 · 2 cycles").
pub(super) fn wave_summary(title: &str, p: &Params) -> Option<String> {
    let n = |v: f64| {
        let t = format!("{:.2}", v);
        t.trim_end_matches('0').trim_end_matches('.').replace('-', "\u{2212}")
    };
    let q = |mode: &str, text: &str| match p.s(mode) {
        "None" => "no label".to_string(),
        "Blank line" => "blank line".to_string(),
        _ => format!("\u{201c}{}\u{201d}", text),
    };
    let l = wave_layout(p);
    let num = |key: &str| match p.s(key) {
        "No numbers" => "no numbers".to_string(),
        "Every line" => "numbered".to_string(),
        v => format!("numbered {}", v.to_lowercase()),
    };
    let axis = |lo: f64, hi: f64, st: f64, fit: bool, numkey: &str, lettered: bool| {
        let mut v = vec![format!("{} to {}", n(lo), n(hi)), format!("by {}", n(st)), (if fit { "fitted" } else { "set by hand" }).to_string(), num(numkey)];
        if lettered && p.b("grid") {
            v.push("no label".into());
        }
        v.join(" \u{b7} ")
    };
    let parts: Vec<String> = match title {
        "Wave" => vec![
            p.s("kind").to_string(),
            format!("A = {}", n(p.f("amp"))),
            format!("\u{3bb} = {}", n(p.f("wl"))),
            { let c = p.f("cycles"); format!("{} {}", n(c), if (c - 1.0).abs() < 1e-9 { "cycle" } else { "cycles" }) },
        ],
        "Marks" => {
            let mut v = vec![];
            if p.b("mark_wl") { v.push(format!("wavelength {}", q("wl_lab_mode", p.s("wl_lab")))); }
            if p.b("mark_amp") { v.push(format!("amplitude {}", q("amp_lab_mode", p.s("amp_lab")))); }
            if p.b("mark_ct") { v.push("crest and trough".to_string()); }
            if v.is_empty() { v.push("no marks".into()); }
            v
        }
        "Graph" => vec![
            (if p.b("numbers") { "numbered axes" } else { "no axis numbers" }).to_string(),
            (if p.b("grid") { "gridlines" } else { "tick marks" }).to_string(),
            (if p.b("color") { "color" } else { "black and white" }).to_string(),
        ],
        "Titles" => {
            let time = p.s("against") == "Time";
            let mut v = vec![];
            if has_title(p) { v.push(q("title_mode", p.s("title"))); }
            v.push(q("xt_mode", if p.s("xt").trim().is_empty() { if time { "Time (s)" } else { "Distance (m)" } } else { p.s("xt") }));
            v.push(q("yt_mode", if p.s("yt").trim().is_empty() { "Displacement (cm)" } else { p.s("yt") }));
            v
        }
        "x-axis" => return Some(axis(l.sx.xmin, l.sx.xmax, l.xstep, p.b("xaxis_fit"), "xaxis_num", true)),
        "y-axis" => {
            let a = l.panel.as_ref()?;
            return Some(axis(a.ymin, a.ymax, l.ystep, p.b("yaxis_fit"), "yaxis_num", true));
        }
        "Grid" => vec![match p.s("minor") {
            "None" => "No minor gridlines".to_string(),
            m => format!("Minor gridlines, {}", m),
        }],
        _ => return None,
    };
    Some(parts.join(" \u{b7} "))
}

fn wave_render(p: &Params) -> String {
    let l = wave_layout(p);
    let kind = p.s("kind").to_string();
    let time = p.s("against") == "Time";
    let (a, wl) = (p.f("amp").max(0.001), p.f("wl").max(0.001));
    let cyc = p.f("cycles").clamp(0.5, 8.0);
    let span = wl * cyc;
    let col = p.b("color");
    let curve = if col { PAL[1] } else { INK };
    let mc = if col { PAL[0] } else { INK };
    let mut g = G::new(l.w, l.h, false);
    draw_title(&mut g, p, l.w / 2.0 + 20.0, 32.0);
    let xl = axis_title(p, "xt", "xt_mode", if time { "Time (s)" } else { "Distance (m)" });
    let yl = axis_title(p, "yt", "yt_mode", "Displacement (cm)");
    let wl_lab = { let t = lab_mode(p, "wl_lab", "wl_lab_mode"); if t == "lambda" && time { "T".to_string() } else { t } };
    let (numbers, grid) = (p.b("numbers"), p.b("grid"));
    let ticks = |lo: f64, hi: f64, st: f64| -> Vec<(i64, f64)> {
        let mut v = vec![];
        let mut k = (lo / st - 1e-9).ceil() as i64;
        while (k as f64) * st <= hi + st * 1e-6 { v.push((k, k as f64 * st)); k += 1; }
        v
    };
    let xe = every(p.s("xaxis_num"));
    let ye = every(p.s("yaxis_num"));
    let title_x = |g: &mut G, cx: f64, y: f64| {
        if let Some(t) = &xl {
            if t.is_empty() { g.line(cx - 60.0, y + 2.0, cx + 60.0, y + 2.0, INK, 1.5); } else { g.text(cx, y, 15.0, "middle", &supers(t), true); }
        }
    };
    let crest_txt = |mode: &str, word: &str| -> Option<String> {
        match p.s(mode) { "None" => None, "Blank line" => Some(String::new()), _ => Some(word.to_string()) }
    };
    // ---------------- transverse graph
    if let Some(ax) = &l.panel {
        let (lft, rgt, tp, bt) = (ax.x0, ax.x0 + ax.w, ax.y0, ax.y0 + ax.h);
        let zero_in = ax.ymin <= 0.0 && ax.ymax >= 0.0;
        let y0 = if zero_in { ax.py(0.0) } else { bt };
        let axes_drawn = numbers || grid;
        if grid {
            let m = match p.s("minor") { "2 per block" => 2, "4 per block" => 4, "5 per block" => 5, _ => 1 };
            if m > 1 {
                for (_, xv) in ticks(ax.xmin, ax.xmax, l.xstep / m as f64) { let x = ax.px(xv); g.line(x, tp, x, bt, "#c4c8ce", 0.6); }
                for (_, yv) in ticks(ax.ymin, ax.ymax, l.ystep / m as f64) { let y = ax.py(yv); g.line(lft, y, rgt, y, "#c4c8ce", 0.6); }
            }
            for (_, xv) in ticks(ax.xmin, ax.xmax, l.xstep) { let x = ax.px(xv); g.line(x, tp, x, bt, "#444", 0.9); }
            for (_, yv) in ticks(ax.ymin, ax.ymax, l.ystep) { let y = ax.py(yv); g.line(lft, y, rgt, y, "#444", 0.9); }
            g.polyline(&[(lft, tp), (rgt, tp), (rgt, bt), (lft, bt), (lft, tp)], INK, 1.2);
        }
        if axes_drawn {
            g.line(lft, tp, lft, bt, INK, 2.2);
            g.line(lft, y0, rgt, y0, INK, 2.4);
            if !grid {
                for (_, xv) in ticks(ax.xmin, ax.xmax, l.xstep) { let x = ax.px(xv); g.line(x, y0 - 4.0, x, y0 + 4.0, INK, 1.3); }
                for (_, yv) in ticks(ax.ymin, ax.ymax, l.ystep) { let y = ax.py(yv); g.line(lft - 4.0, y, lft + 4.0, y, INK, 1.3); }
                if p.s("xaxis_letter") == "Text" { g.tex(rgt + 8.0, y0 + 6.0, 20.0, "start", "x", ITALIC); }
                if p.s("yaxis_letter") == "Text" { g.tex(lft, tp - 8.0, 20.0, "middle", "y", ITALIC); }
            }
        } else {
            // no axes: just the rest line
            g.dash(lft, y0, rgt, y0, INK, 1.2);
        }
        if numbers {
            if xe > 0 {
                for (k, xv) in ticks(ax.xmin, ax.xmax, l.xstep) {
                    if k.rem_euclid(xe as i64) != 0 || (xv.abs() < 1e-9 && zero_in) { continue; }
                    g.text(ax.px(xv), y0 + 18.0, 11.5, "middle", &fmt_num(xv).replace('-', "−"), true);
                }
            }
            if ye > 0 {
                for (k, yv) in ticks(ax.ymin, ax.ymax, l.ystep) {
                    if k.rem_euclid(ye as i64) != 0 { continue; }
                    let zero = yv.abs() < 1e-9 && zero_in && xe > 0;
                    g.text(lft - 6.0, ax.py(yv) + if zero { 18.0 } else { 4.0 }, 11.5, "end", &fmt_num(yv).replace('-', "−"), true);
                }
            }
        }
        if let Some(t) = &yl {
            let (cx, cy) = (lft - 40.0, (tp + bt) / 2.0);
            if t.is_empty() { g.line(cx, cy - 50.0, cx, cy + 50.0, INK, 1.5); } else { g.vtext(cx, cy, 15.0, &supers(t), true); }
        }
        title_x(&mut g, (lft + rgt) / 2.0, bt + 24.0);
        // the curve, clipped to the graph
        g.raw(&format!(r#"<clipPath id="wclip"><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}"/></clipPath><g clip-path="url(#wclip)">"#, lft - 2.0, tp - 2.0, ax.w + 4.0, ax.h + 4.0));
        let (s0, s1) = (0f64.max(ax.xmin), span.min(ax.xmax));
        if s1 > s0 {
            let pts: Vec<Pt> = (0..=500).map(|i| { let x = s0 + (s1 - s0) * i as f64 / 500.0; (ax.px(x), ax.py(a * (2.0 * PI * x / wl).sin())) }).collect();
            g.polyline(&pts, curve, 3.0);
        }
        g.raw("</g>");
        let crest = |k: f64| wl / 4.0 + wl * k;
        let in_x = |x: f64| x >= ax.xmin - 1e-9 && x <= ax.xmax.min(span) + 1e-9;
        if p.b("mark_amp") && in_x(crest(0.0)) {
            let xa = ax.px(crest(0.0));
            g.dim_arrow_fine(xa, y0, xa, ax.py(a), mc, 1.5);
            g.label_at(xa + 14.0, (y0 + ax.py(a)) / 2.0 - 8.0, 20.0, &lab_mode(p, "amp_lab", "amp_lab_mode"), ITALIC);
        }
        // index of the last whole crest drawn (it carries the "crest" label)
        let mut kc = 0.0;
        while in_x(crest(kc + 1.0)) { kc += 1.0; }
        let crest_lab = p.b("mark_ct") && p.s("crest_mode") != "None" && in_x(crest(kc));
        if p.b("mark_wl") {
            let (s, e) = if in_x(crest(1.0)) { (crest(0.0), crest(1.0)) } else { (0.0, wl) };
            let busy = crest_lab && (kc - 1.0).abs() < 1e-9 && s > 0.0;
            let ya = ax.py(a) - if busy { 44.0 } else { 22.0 };
            let yb = if s == 0.0 { y0 } else { ax.py(a) };
            g.line(ax.px(s), yb, ax.px(s), ya - 7.0, mc, 1.1);
            g.line(ax.px(e), if busy { ya + 8.0 } else { yb }, ax.px(e), ya - 7.0, mc, 1.1);
            g.dim_arrow_fine(ax.px(s), ya, ax.px(e), ya, mc, 1.4);
            g.label_at((ax.px(s) + ax.px(e)) / 2.0, ya - 12.0, 20.0, &wl_lab, ITALIC);
        }
        if p.b("mark_ct") {
            // the last crest and trough drawn
            if in_x(crest(kc)) {
                if let Some(t) = crest_txt("crest_mode", "crest") {
                    let lift = if p.b("mark_wl") && kc <= 1.0 { 34.0 } else { 10.0 };
                    let _ = lift;
                    g.label_at(ax.px(crest(kc)), ax.py(a) - 16.0, 17.0, &t, UPRIGHT);
                }
            }
            let trough = |k: f64| wl * 0.75 + wl * k;
            let mut kt = 0.0;
            while in_x(trough(kt + 1.0)) { kt += 1.0; }
            if in_x(trough(kt)) {
                if let Some(t) = crest_txt("trough_mode", "trough") {
                    g.label_at(ax.px(trough(kt)), ax.py(-a) + 18.0, 17.0, &t, UPRIGHT);
                }
            }
        }
    }
    // ---------------- longitudinal strip
    if let Some((by, bh)) = l.strip {
        let ax = &l.sx;
        let n = (cyc * 26.0) as usize;
        let k = 2.0 * PI / wl;
        let dmax = 0.12 * wl;
        for i in 0..=n {
            let x0 = span * i as f64 / n as f64;
            let x = x0 - dmax * (k * (x0 - wl / 4.0)).sin();
            if x < ax.xmin - 1e-9 || x > ax.xmax + 1e-9 { continue; }
            let px = ax.px(x.clamp(0.0, span));
            g.line(px, by, px, by + bh, curve, 1.8);
        }
        let only = l.panel.is_none();
        if only {
            // distance axis beneath the strip
            let ay = by + bh + 44.0;
            let (lft, rgt) = (ax.x0, ax.x0 + ax.w);
            g.line(lft, ay, rgt, ay, INK, 2.2);
            for (kk, xv) in ticks(ax.xmin, ax.xmax, l.xstep) {
                let px = ax.px(xv);
                g.line(px, ay - 4.0, px, ay + 4.0, INK, 1.3);
                if numbers && xe > 0 && kk.rem_euclid(xe as i64) == 0 { g.text(px, ay + 20.0, 13.0, "middle", &fmt_num(xv).replace('-', "−"), true); }
            }
            title_x(&mut g, (lft + rgt) / 2.0, ay + 44.0);
            if p.b("mark_wl") {
                let (s, e) = if wl * 1.25 <= span.min(ax.xmax) + 1e-9 { (wl / 4.0, wl * 1.25) } else { (0.0, wl) };
                let y = by - 14.0;
                g.line(ax.px(s), by - 2.0, ax.px(s), y - 7.0, mc, 1.1);
                g.line(ax.px(e), by - 2.0, ax.px(e), y - 7.0, mc, 1.1);
                g.dim_arrow_fine(ax.px(s), y, ax.px(e), y, mc, 1.4);
                g.label_at((ax.px(s) + ax.px(e)) / 2.0, y - 12.0, 20.0, &wl_lab, ITALIC);
            }
        }
        if p.b("mark_ct") {
            // compression over the first dense region, rarefaction over the last sparse one
            let ly = if only { by + bh + 18.0 } else { by - 10.0 };
            if let Some(t) = crest_txt("crest_mode", "compression") {
                g.label_at(ax.px(wl / 4.0), ly, 16.0, &t, UPRIGHT);
            }
            let mut kt = 0.0;
            while wl * 0.75 + wl * (kt + 1.0) <= span.min(ax.xmax) + 1e-9 { kt += 1.0; }
            let xr = wl * 0.75 + wl * kt;
            if xr <= span.min(ax.xmax) + 1e-9 && kt >= 0.0 && cyc >= 0.75 {
                if let Some(t) = crest_txt("trough_mode", "rarefaction") {
                    g.label_at(ax.px(xr), ly, 16.0, &t, UPRIGHT);
                }
            }
        }
    }
    let _ = kind;
    g.finish()
}

/// Adds the L/M/S label-size param (the reference toolbar toggle) to a param list.
fn with_ls(mut v: Vec<ParamDef>) -> Vec<ParamDef> {
    v.push(crate::figures::svg::label_size_param());
    v
}

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "motion-graphs", subject: Subject::Physics, title: "Motion Graphs",
            blurb: "Position, velocity and acceleration against time, built segment by segment.",
            params: || with_ls(motion_params()), render: |p| crate::figures::svg::with_label_size(motion_render(p), p) },
        FigureDef { id: "waves", subject: Subject::Physics, title: "Waves",
            blurb: "Transverse and longitudinal waves on numbered axes, to measure.",
            params: || with_ls(wave_params()), render: |p| crate::figures::svg::with_label_size(wave_render(p), p) },
    ]
}
