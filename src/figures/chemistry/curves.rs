//! Titration curve and heating/cooling curve.
use super::*;
use crate::figures::{Params, ParamDef, Subject};

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "titration-curve", subject: Subject::Chemistry, title: "Titration Curve",
            blurb: "An acid–base titration curve from molarities and pKa, or from the pH values you type.",
            params: tit_params, render: |p| crate::figures::svg::with_label_size(tit_render(p), p) },
        FigureDef { id: "heating-cooling-curve", subject: Subject::Chemistry, title: "Heating and Cooling Curve",
            blurb: "A heating or cooling curve for water, ethanol and other substances, or your own melting and boiling points, schematic or to scale.",
            params: heat_params, render: |p| crate::figures::svg::with_label_size(heat_render(p), p) },
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
const COMMON: [&str; 7] = ["Custom", "HCl with NaOH", "CH₃COOH (acetic acid) with NaOH", "HCOOH (formic acid) with NaOH", "HF with NaOH", "NaOH with HCl", "NH₃ (ammonia) with HCl"];
const MARKS: [&str; 3] = ["Dot and lines to the axes", "Dot", "Not marked"];

/// Text size factor of the L/M/S label-size choice (the graph margins follow it).
pub(super) fn lf(p: &Params) -> f64 { crate::figures::svg::label_factor(p.s("label_size")) }

fn tit_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("kind", "What\u{2019}s titrated", &KINDS, "Weak acid with a strong base"),
        ParamDef::choice("common", "Common titration|25.0 mL of 0.100 M, with 0.100 M", &COMMON, "Custom"),
        ParamDef::choice("source", "", &["Concentrations", "Key points"], "Concentrations"),
        ParamDef::float("molarity", "Analyte molarity (M)", 0.001, 2.0, 0.005, 0.1),
        ParamDef::float("volume", "Analyte volume (mL)", 1.0, 200.0, 0.5, 25.0),
        ParamDef::float("titrant", "Titrant molarity (M)", 0.001, 2.0, 0.005, 0.1),
        ParamDef::float("pk", "pKa", 1.0, 12.0, 0.01, 4.76)
            .hint("pKb for a weak base."),
        ParamDef::float("kp_start", "Starting pH", 0.0, 14.0, 0.01, 2.88),
        ParamDef::float("kp_end", "Ending pH", 0.0, 14.0, 0.01, 12.52),
        ParamDef::float("kp_veq", "Equivalence volume (mL)", 0.5, 500.0, 0.5, 25.0),
        ParamDef::float("kp_eqph", "Equivalence pH", 0.0, 14.0, 0.01, 8.73),
        ParamDef::choice("equiv", "Equivalence point", &MARKS, "Dot and lines to the axes"),
        ParamDef::choice("eq_label_style", "Its label", &TITLE3, "Text"),
        ParamDef::text("eq_label", "", "Equivalence point"),
        ParamDef::choice("half_mark", "Half-equivalence point|where pH = pKa", &MARKS, "Not marked"),
        ParamDef::choice("half_label_style", "Its label", &TITLE3, "Text"),
        ParamDef::text("half_label", "", "Half-equivalence point"),
        ParamDef::bool("half", "Mark half-equivalence", false).hint("Weak only; same as Half-equivalence point = Dot and lines."),
        ParamDef::choice("title_style", "Chart title", &TITLE3, "None"),
        ParamDef::text("title", "", ""),
        ParamDef::choice("xtitle_style", "x-axis title", &TITLE3, "Text"),
        ParamDef::text("xlabel", "", "").hint("Blank = automatic."),
        ParamDef::choice("ytitle_style", "y-axis title", &TITLE3, "Text"),
        ParamDef::text("ytitle", "", "pH"),
        ParamDef::color("curve_color", "Curve", &CURVE_COLORS, "#2563eb"),
        ParamDef::float("xmax", "x-axis maximum when fitted (mL, 0 = twice the equivalence volume)", 0.0, 1000.0, 1.0, 0.0),
        answer_param("Answer key (equivalence and half-equivalence points)"),
        crate::figures::svg::label_size_param(),
        ParamDef::bool("xaxis_fit", "x-axis: fit to the titration", true),
    ];
    v.extend(xaxis_params(AxisDefaults { from: 0.0, to: 50.0, by: 2.0, num: "Every 5th line" }, 1000.0));
    v.extend(yaxis_params(AxisDefaults { from: 0.0, to: 14.0, by: 1.0, num: "Every 2nd line" }, 14.0));
    v.push(minor_param());
    v
}


/// Lowercased mark name for a section summary ("dot and lines to the axes").
fn mark_words(m: &str) -> String { m.to_lowercase() }

/// Section-header summary the reference shows under a titration section title (`None` = the
/// generic one). The Titration section itself shows none.
pub fn tit_section_summary(title: &str, p: &Params) -> Option<String> {
    let axis = |pre: &str| {
        let fr = tit_frame(p);
        let a = if pre == "xaxis" { fr.g.x } else { fr.g.y };
        let num = match p.s(&format!("{pre}_num")) {
            "Every 2nd line" => "numbered every 2".to_string(),
            "Every 5th line" => "numbered every 5".to_string(),
            "Every 10th line" => "numbered every 10".to_string(),
            "No numbers" => "no numbers".to_string(),
            _ => "numbered every line".to_string(),
        };
        let lab = if p.s(&format!("{pre}_lab")) == "Text" { format!("labelled “{}”", p.s(&format!("{pre}_lab_text"))) } else { "no label".to_string() };
        let (lo, hi) = (p.s(&format!("{pre}_lo")), p.s(&format!("{pre}_hi")));
        let ends = if lo == "None" && hi == "None" { "plain ends".to_string() } else { format!("{} / {} ends", lo.to_lowercase(), hi.to_lowercase()) };
        format!("{} to {} · by {} · {num} · {lab} · {ends}", tick(a.from), tick(a.to), tick(a.by))
    };
    Some(match title {
        "Titration" => String::new(),
        "Marked points" => format!("Equivalence: {} · half-equivalence: {}", mark_words(p.s("equiv")), mark_words(p.s("half_mark"))),
        "Curve" => {
            let i = CURVE_COLORS.iter().position(|c| c.eq_ignore_ascii_case(p.s("curve_color"))).unwrap_or(0);
            ["Blue", "Red", "Green", "Purple", "Orange", "Black"][i].to_string()
        }
        "Titles" => {
            let f = tit_frame(p);
            let auto = format!("Volume of {} added (mL)", if f.acid_analyte { "NaOH" } else { "HCl" });
            let xl = if p.s("xlabel").trim().is_empty() { auto } else { p.s("xlabel").to_string() };
            let mut v = Vec::new();
            for (st, t) in [("title_style", p.s("title").to_string()), ("xtitle_style", xl), ("ytitle_style", p.s("ytitle").to_string())] {
                if p.s(st) == "Text" && !t.trim().is_empty() { v.push(format!("“{t}”")); }
            }
            if v.is_empty() { "No titles".to_string() } else { v.join(" · ") }
        }
        "x-axis" => axis("xaxis"),
        "y-axis" => axis("yaxis"),
        "Grid" => match p.s("minor_grid") { "2 per block" => "2 minor gridlines per block", "4 per block" => "4 minor gridlines per block", "5 per block" => "5 minor gridlines per block", _ => "No minor gridlines" }.to_string(),
        _ => return None,
    })
}

/// The readout box under the Titration controls: where the equivalence point (and, for weak
/// analytes, the half-equivalence point) falls.
pub fn tit_readout(p: &Params) -> Vec<String> {
    let f = tit_frame(p);
    let mut v = vec![format!("Equivalence point: {} mL, pH {}", tick((f.veq * 100.0).round() / 100.0), num(f.ph(f.veq), 2))];
    if f.kind.starts_with("Weak") {
        v.push(format!("Half-equivalence point: {} mL, pH {}", tick((f.veq * 50.0).round() / 100.0), num(f.ph(f.veq / 2.0), 2)));
    }
    v
}

/// "The ending pH is at the end of the x-axis (50 mL)." under the Key points boxes.
pub fn tit_end_note(p: &Params) -> String {
    format!("The ending pH is at the end of the x-axis ({} mL).", tick(tit_frame(p).g.x.to))
}

/// Whether titration control `key` applies now (the reference shows only the chosen mode's boxes,
/// a label's text box only while its choice is "Text", and the half-equivalence tick-box only
/// through the Half-equivalence point dropdown).
pub fn tit_relevant(key: &str, p: &Params) -> bool {
    match key {
        "molarity" | "volume" | "titrant" | "pk" => p.s("source") != "Key points",
        "kp_start" | "kp_end" | "kp_veq" | "kp_eqph" => p.s("source") == "Key points",
        "half" => false,
        "eq_label_style" => p.s("equiv") != "Not marked",
        "eq_label" => p.s("equiv") != "Not marked" && p.s("eq_label_style") == "Text",
        "half_label_style" => p.s("half_mark") != "Not marked",
        "half_label" => p.s("half_mark") != "Not marked" && p.s("half_label_style") == "Text",
        "title" => p.s("title_style") == "Text",
        "xlabel" => p.s("xtitle_style") == "Text",
        "ytitle" => p.s("ytitle_style") == "Text",
        _ => true,
    }
}

/// Chemistry + axis frame shared by the titration renderer and its handles.
pub(super) struct TitFrame { pub kind: String, pub ca: f64, pub va: f64, pub ct: f64, pub pk: f64, pub acid_analyte: bool, pub veq: f64, pub g: Graph, pub remap: Option<[f64; 6]> }

impl TitFrame {
    pub fn px(&self, v: f64) -> f64 { self.g.px(v) }
    pub fn py(&self, ph: f64) -> f64 { self.g.py(ph) }
    pub fn ph(&self, v: f64) -> f64 {
        let raw = titration_ph(&self.kind, self.ca, self.va, self.ct, self.pk, v);
        match self.remap {
            // piecewise-linear map of the model curve through the key points
            Some([m0, me, m1, s0, se, s1]) => {
                if (raw - m0) * (me - m0) <= 0.0 { s0 } else if v <= self.veq { s0 + (raw - m0) / (me - m0) * (se - s0) }
                else if (m1 - me).abs() < 1e-9 { se } else { se + (raw - me) / (m1 - me) * (s1 - se) }
            }
            None => raw,
        }
    }
    pub fn mark(&self, p: &Params, half: bool) -> &'static str {
        if !half { return match p.s("equiv") { "Dot" => "Dot", "Not marked" => "Not marked", _ => "Dot and lines to the axes" }; }
        if !self.kind.starts_with("Weak") { return "Not marked"; }
        match p.s("half_mark") { "Dot" => "Dot", "Dot and lines to the axes" => "Dot and lines to the axes", _ => if p.b("half") { "Dot and lines to the axes" } else { "Not marked" } }
    }
}

pub(super) fn tit_frame(p: &Params) -> TitFrame {
    let mut kind = p.s("kind").to_string();
    let (mut ca, mut va, mut ct) = (p.f("molarity").max(1e-6), p.f("volume").max(0.1), p.f("titrant").max(1e-6));
    let mut pk = p.f("pk");
    let common = p.s("common");
    if common != "Custom" && !common.is_empty() {
        (ca, va, ct) = (0.1, 25.0, 0.1);
        let (k, kk) = match common {
            "HCl with NaOH" => (KINDS[0], 4.76),
            "CH₃COOH (acetic acid) with NaOH" => (KINDS[1], 4.76),
            "HCOOH (formic acid) with NaOH" => (KINDS[1], 3.75),
            "HF with NaOH" => (KINDS[1], 3.17),
            "NaOH with HCl" => (KINDS[2], 4.75),
            _ => (KINDS[3], 4.75),
        };
        kind = k.to_string();
        pk = kk;
    }
    let mut remap = None;
    let key_points = p.s("source") == "Key points";
    if key_points {
        let veq = p.f("kp_veq").max(0.5);
        (ca, va, ct) = (0.1, veq, 0.1);
        let acid = kind.starts_with("Strong acid") || kind.starts_with("Weak acid");
        let start = p.f("kp_start");
        if kind.starts_with("Weak") { pk = if acid { 2.0 * start - 1.0 } else { 2.0 * (14.0 - start) - 1.0 }.clamp(1.0, 12.0); }
    }
    let acid_analyte = kind.starts_with("Strong acid") || kind.starts_with("Weak acid");
    let veq = ca * va / ct;
    let auto_max = if p.f("xmax") >= 1.0 { p.f("xmax") } else { veq * 2.0 };
    let mut x = Axis::read(p, "xaxis");
    if p.b("xaxis_fit") {
        let by = nice(auto_max, 25.0);
        x.from = 0.0;
        x.to = (auto_max / (by * 5.0)).ceil() * by * 5.0;
        x.by = by;
        x.fix();
    }
    let y = Axis::read(p, "yaxis");
    let xt = axis_title(p, "xtitle_style", "x").is_some();
    let yt = axis_title(p, "ytitle_style", p.s("ytitle")).is_some();
    let ((x0, x1, y0, y1), (w, h)) = plot_rect(&x, &y, xt, yt, 0.0, lf(p));
    let minor = match p.s("minor_grid") { "2 per block" => 2, "4 per block" => 4, "5 per block" => 5, _ => 0 };
    let g = Graph { x, y, minor, x0, x1, y0, y1, w, h, lf: lf(p) };
    if key_points {
        let m0 = titration_ph(&kind, ca, va, ct, pk, 0.0);
        let me = titration_ph(&kind, ca, va, ct, pk, veq);
        let m1 = titration_ph(&kind, ca, va, ct, pk, g.x.to.max(veq * 1.01));
        remap = Some([m0, me, m1, p.f("kp_start"), p.f("kp_eqph"), p.f("kp_end")]);
    }
    TitFrame { kind, ca, va, ct, pk, acid_analyte, veq, g, remap }
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
    let g = &f.g;
    let mut d = Doc::new(g.w, g.h);
    let titr_name = if f.acid_analyte { "NaOH" } else { "HCl" };
    let xl = if p.s("xlabel").trim().is_empty() { format!("Volume of {titr_name} added (mL)") } else { p.s("xlabel").to_string() };
    draw_graph(&mut d, g, axis_title(p, "xtitle_style", &xl), axis_title(p, "ytitle_style", p.s("ytitle")));
    let n = 500;
    let (v0, v1) = (g.x.from.max(0.0), g.x.to);
    let pts: Vec<(f64, f64)> = (0..=n).map(|i| {
        let v = v0 + (v1 - v0) * i as f64 / n as f64;
        (g.px(v), g.py(f.ph(v).clamp(g.y.from, g.y.to)))
    }).collect();
    let cc = if p.s("curve_color").starts_with('#') { p.s("curve_color") } else { "#2563eb" };
    d.polyline(&pts, cc, 2.8);
    let label = |d: &mut Doc, x: f64, y: f64, style_key: &str, text: &str| {
        match p.s(style_key) {
            "None" => {}
            "Blank line" => { d.line(x + 10.0, y + 4.0, x + 110.0, y + 4.0, INK, 1.3); }
            _ => { d.raw(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="15.4" font-family="{FONT}" font-weight="bold" fill="{INK}" stroke="#fff" stroke-width="4" paint-order="stroke">{}</text>"##, x + 12.0, y + 5.6, crate::figures::svg::esc(text))); }
        }
    };
    let mut foot = Vec::new();
    for half in [false, true] {
        let mark = f.mark(p, half);
        if mark == "Not marked" { continue; }
        let v = if half { f.veq / 2.0 } else { f.veq };
        let ph = f.ph(v);
        let (ex, ey) = (g.px(v), g.py(ph));
        if ex < g.x0 || ex > g.x1 || ey < g.y0 || ey > g.y1 { continue; }
        if mark.starts_with("Dot and") {
            // the reference's guides: 1.6 wide, dashed 6 on / 5 off, butt caps
            d.raw(&format!(r##"<g stroke="{INK}" stroke-width="1.6" stroke-dasharray="6 5"><line x1="{ex:.2}" y1="{ey:.2}" x2="{ex:.2}" y2="{:.2}"/><line x1="{ex:.2}" y1="{ey:.2}" x2="{:.2}" y2="{ey:.2}"/></g>"##, g.y1, g.x0));
        }
        d.circle(ex, ey, 5.5, INK, "#fff", 1.5);
        let (sk, tk) = if half { ("half_label_style", "half_label") } else { ("eq_label_style", "eq_label") };
        label(&mut d, ex, ey, sk, p.s(tk));
    }
    if p.b("answer") {
        foot.push(format!("Equivalence point: {} mL, pH {}", tick((f.veq * 100.0).round() / 100.0), num(f.ph(f.veq), 2)));
        if f.kind.starts_with("Weak") {
            foot.push(format!("Half-equivalence point: {} mL, pH {}", tick((f.veq * 50.0).round() / 100.0), num(f.ph(f.veq / 2.0), 2)));
        }
    }
    wrap_at(p, d.finish(), &foot, Some((g.x0 + g.x1) / 2.0))
}

// ---------------------------------------------------------------------------------------------
// Heating / cooling
// ---------------------------------------------------------------------------------------------

const SUBST: [(&str, f64, f64); 6] = [
    ("Water", 0.0, 100.0), ("Ethanol", -114.0, 78.0), ("Acetone", -95.0, 56.0),
    ("Mercury", -39.0, 357.0), ("Sodium chloride", 801.0, 1413.0), ("Substance X", 20.0, 90.0),
];

/// "Melts at 0 °C and boils at 100 °C." under the Substance choice (freezes / condenses when cooling).
pub fn substance_note(p: &Params) -> Option<String> {
    let (mp, bp) = if p.s("substance") == "Custom" { (p.f("mp"), p.f("bp")) } else { let s = SUBST.iter().find(|s| s.0 == p.s("substance"))?; (s.1, s.2) };
    let t = |v: f64| num(v, if v.fract() == 0.0 { 0 } else { 1 }).replace('-', "\u{2212}");
    Some(if p.s("mode") == "Cooling" { format!("Freezes at {} \u{b0}C and condenses at {} \u{b0}C.", t(mp), t(bp)) } else { format!("Melts at {} \u{b0}C and boils at {} \u{b0}C.", t(mp), t(bp)) })
}

fn heat_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("mode", "Heating or cooling", &["Heating", "Cooling"], "Heating"),
        ParamDef::choice("substance", "Substance", &["Water", "Ethanol", "Acetone", "Mercury", "Sodium chloride", "Substance X", "Custom"], "Water"),
        ParamDef::float("mp", "Custom melting point (°C)", -270.0, 3000.0, 1.0, 0.0)
            .hint("The freezing point when cooling."),
        ParamDef::float("bp", "Custom boiling point (°C)", -260.0, 4000.0, 1.0, 100.0),
        ParamDef::float("start", "Starting temperature (°C)", -273.0, 4000.0, 1.0, -20.0),
        ParamDef::float("end", "Ending temperature (°C)", -273.0, 4000.0, 1.0, 120.0),
        ParamDef::choice("xaxis", "Along the x-axis", &["Time", "Heat added"], "Time"),
        ParamDef::choice("scale", "Segment lengths", &["Schematic", "To scale"], "Schematic"),
        ParamDef::float("t_solid", "Solid warms / cools (min)", 0.5, 30.0, 0.5, 2.0),
        ParamDef::float("t_melt", "Melting / freezing (min)", 0.5, 30.0, 0.5, 3.0),
        ParamDef::float("t_liquid", "Liquid warms / cools (min)", 0.5, 30.0, 0.5, 5.0),
        ParamDef::float("t_boil", "Boiling / condensing (min)", 0.5, 30.0, 0.5, 8.0),
        ParamDef::float("t_gas", "Gas warms / cools (min)", 0.5, 30.0, 0.5, 2.0),
        ParamDef::bool("labels", "Labels on the curve", true)
            .hint("Off draws none at all."),
        ParamDef::text("title", "", ""),
        ParamDef::text("ylabel", "y-axis title text", "Temperature (°C)"),
        ParamDef::list("phase_names", "Phase labels (5): solid, melting, liquid, boiling, gas", "solid, solid + liquid, liquid, liquid + gas, gas"),
        ParamDef::color("curve_color", "Curve", &CURVE_COLORS, "#2563eb"),
        ParamDef::bool("supercool", "Supercooled", false)
            .hint("The liquid cools past its freezing point before it starts to freeze, then warms back up to it."),
        ParamDef::choice("seg_labels", "Segment labels", &["States", "Blank lines", "None"], "States"),
        ParamDef::choice("plateau_labels", "Plateaus labeled with", &["Both states (Solid + liquid)", "The change (Melting)"], "Both states (Solid + liquid)"),
        ParamDef::bool("letters", "Letters at the corners", false)
            .hint("A, B, C… for questions like “What is happening between B and C?”"),
        ParamDef::bool("plateau_dashes", "Dashed lines at the plateaus", true)
            .hint("Across to the temperature axis, to read the melting and boiling points."),
        ParamDef::choice("temp_labels", "By the temperature axis", &["Temperatures", "m.p. and b.p.", "Blank lines", "None"], "Temperatures"),
        title_style(true),
        ParamDef::choice("xtitle_style", "x-axis title", &TITLE3, "Text"),
        ParamDef::text("xtitle", "x-axis title text (blank = automatic)", ""),
        ParamDef::choice("ytitle_style", "y-axis title", &TITLE3, "Text"),
        ParamDef::bool("xaxis_fit", "Fit the axes to the curve", true),
    ];
    v.extend(xaxis_params(AxisDefaults { from: 0.0, to: 20.0, by: 1.0, num: "Every 2nd line" }, 1000.0));
    v.extend(yaxis_params(AxisDefaults { from: -20.0, to: 120.0, by: 10.0, num: "Every 2nd line" }, 5000.0));
    v.push(crate::figures::svg::label_size_param());
    v.push(minor_param());
    v
}


/// Stage list + axis frame shared by the heating-curve renderer and its handles.
pub(super) struct HeatFrame { pub seq: Vec<(Ph, f64, f64, f64)>, pub mp: f64, pub bp: f64, pub lo: f64, pub hi: f64, pub total: f64, pub scaled: bool, pub cooling: bool, pub g: Graph, pub k: f64 }

impl HeatFrame {
    /// x of `u` along the curve (u in duration units).
    pub fn px(&self, u: f64) -> f64 { self.g.px(u * self.k) }
    pub fn py(&self, t: f64) -> f64 { self.g.py(t) }
    pub fn inv_t(&self, y: f64) -> f64 { self.g.vy(y) }
    pub fn inv_u(&self, x: f64) -> f64 { self.g.vx(x) / self.k }
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
    // schematic units are stretched to the real total so the time axis still reads in minutes
    let real: f64 = st.iter().map(|s| s.3).sum();
    let k = if scaled { 1.0 } else { real / total.max(1e-9) };
    let mut x = Axis::read(p, "xaxis");
    let mut y = Axis::read(p, "yaxis");
    if p.b("xaxis_fit") {
        let tx_ = total * k;
        x.by = nice(tx_, 20.0);
        x.from = 0.0;
        x.to = (tx_ / (x.by * 2.0)).ceil() * x.by * 2.0;
        x.fix();
        let by = nice(hi - lo, 14.0);
        y.by = by;
        y.from = (lo / (2.0 * by)).floor() * 2.0 * by;
        y.to = (hi / (2.0 * by)).ceil() * 2.0 * by;
        y.fix();
    }
    let xt = axis_title(p, "xtitle_style", "x").is_some();
    let yt = axis_title(p, "ytitle_style", p.s("ylabel")).is_some();
    let ((x0, x1, y0, y1), (w, h)) = plot_rect(&x, &y, xt, yt, 0.0, lf(p));
    let minor = match p.s("minor_grid") { "2 per block" => 2, "4 per block" => 4, "5 per block" => 5, _ => 0 };
    HeatFrame { seq, mp, bp, lo, hi, total, scaled, cooling, g: Graph { x, y, minor, x0, x1, y0, y1, w, h, lf: lf(p) }, k }
}

/// The five phase labels (solid, melting, liquid, boiling, gas), defaults filling blanks.
fn phase_names(p: &Params) -> Vec<String> {
    let dflt = ["solid", "solid + liquid", "liquid", "liquid + gas", "gas"];
    let given: Vec<&str> = p.s("phase_names").split(',').map(|t| t.trim()).collect();
    (0..5).map(|i| {
        let t = given.get(i).filter(|t| !t.is_empty()).map(|t| t.to_string()).unwrap_or_else(|| dflt[i].to_string());
        // the reference capitalizes the state names
        if t == dflt[i] { let mut c = t.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() } else { t }
    }).collect()
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Ph { Solid, Melt, Liquid, Boil, Gas }

fn halo(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, t: &str) {
    d.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{FONT}" font-weight="bold" fill="{INK}" stroke="#fff" stroke-width="4" paint-order="stroke">{}</text>"##, crate::figures::svg::esc(t)));
}

/// Label placement helper: boxes are `[x0, y0, x1, y1]`, lines `[x1, y1, x2, y2]`.
struct Placer { lines: Vec<[f64; 4]>, boxes: Vec<[f64; 4]>, inner: [f64; 4] }

impl Placer {
    fn grow(b: &[f64; 4], m: f64) -> [f64; 4] { [b[0] - m, b[1] - m, b[2] + m, b[3] + m] }
    fn hits_line(l: &[f64; 4], b: &[f64; 4]) -> bool {
        let (n, r) = (l[2] - l[0], l[3] - l[1]);
        let (mut i, mut a) = (0.0_f64, 1.0_f64);
        for (o, s) in [(-n, l[0] - b[0]), (n, b[2] - l[0]), (-r, l[1] - b[1]), (r, b[3] - l[1])] {
            if o == 0.0 { if s < 0.0 { return false; } } else if o < 0.0 { i = i.max(s / o); } else { a = a.min(s / o); }
        }
        i <= a
    }
    fn overlap(a: &[f64; 4], b: &[f64; 4]) -> bool { a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3] }
    fn clear(&self, b: &[f64; 4]) -> bool {
        let g3 = Self::grow(b, 3.0);
        let g2 = Self::grow(b, 2.0);
        !self.lines.iter().any(|l| Self::hits_line(l, &g3)) && !self.boxes.iter().any(|q| Self::overlap(q, &g2))
    }
    fn ok(&self, b: &[f64; 4]) -> bool {
        b[0] >= self.inner[0] && b[2] <= self.inner[2] && b[1] >= self.inner[1] && b[3] <= self.inner[3] && self.clear(b)
    }
    fn shift(&self, b: &[f64; 4]) -> [f64; 4] {
        let tx = (self.inner[0] - b[0]).max((self.inner[2] - b[2]).min(0.0));
        let ty = (self.inner[1] - b[1]).max((self.inner[3] - b[3]).min(0.0));
        [b[0] + tx, b[1] + ty, b[2] + tx, b[3] + ty]
    }
    fn score(&self, b: &[f64; 4]) -> usize {
        let g3 = Self::grow(b, 3.0);
        let g2 = Self::grow(b, 2.0);
        self.lines.iter().filter(|l| Self::hits_line(l, &g3)).count() + 2 * self.boxes.iter().filter(|q| Self::overlap(q, &g2)).count()
    }
    fn best(&self, c: &[[f64; 4]]) -> [f64; 4] {
        let mut best: Option<([f64; 4], usize)> = None;
        for b in c { let s = self.shift(b); let sc = self.score(&s); if best.map_or(true, |(_, bs)| sc < bs) { best = Some((s, sc)); } }
        best.map(|q| q.0).unwrap_or([0.0; 4])
    }
}

fn heat_render(p: &Params) -> String {
    let hf = heat_frame(p);
    let g = &hf.g;
    let (mp, bp, lo, hi, cooling) = (hf.mp, hf.bp, hf.lo, hf.hi, hf.cooling);
    let seq = hf.seq.clone();
    let mut d = Doc::new(g.w, g.h);
    let auto_x = if p.s("xaxis") == "Heat added" { if cooling { "Heat removed" } else { "Heat added" } } else { "Time (min)" };
    let xl = if p.s("xtitle").trim().is_empty() { auto_x.to_string() } else { p.s("xtitle").to_string() };
    draw_graph(&mut d, g, axis_title(p, "xtitle_style", &xl), axis_title(p, "ytitle_style", p.s("ylabel")));
    // curve (with the supercooling dip before freezing)
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut corners: Vec<(f64, f64)> = Vec::new();
    let mut u = 0.0;
    let dip = (hi - lo) * 0.04;
    for s in &seq {
        if pts.is_empty() { pts.push((hf.px(u), hf.py(s.1))); corners.push(*pts.last().unwrap()); }
        if cooling && p.b("supercool") && s.0 == Ph::Melt {
            let du = s.3 * 0.06;
            pts.push((hf.px(u + du), hf.py(mp - dip)));
            pts.push((hf.px(u + 2.0 * du), hf.py(mp)));
        }
        u += s.3;
        pts.push((hf.px(u), hf.py(s.2)));
        corners.push(*pts.last().unwrap());
    }
    let clip: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| (x.clamp(g.x0, g.x1), y.clamp(g.y0, g.y1))).collect();
    let cc = if p.s("curve_color").starts_with('#') { p.s("curve_color") } else { "#2563eb" };
    d.polyline(&clip, cc, 3.0);
    let names = phase_names(p);
    let change = p.s("plateau_labels").starts_with("The change");
    if p.b("labels") {
        // Placement follows the reference generator: candidate boxes are tried in order and the
        // first one clear of the curve, guides and earlier labels wins.
        let h = 15.4_f64;
        let cw = h * 0.58;
        let mut corners_px: Vec<(f64, f64)> = Vec::new();
        {
            let mut u = 0.0;
            if let Some(f0) = seq.first() { corners_px.push((hf.px(0.0), hf.py(f0.1))); }
            for s in &seq { u += s.3; corners_px.push((hf.px(u), hf.py(s.2))); }
        }
        let mut pl = Placer { lines: Vec::new(), boxes: Vec::new(), inner: [g.x0 + 3.0, g.y0 + 3.0, g.x1 - 3.0, g.y1 - 3.0] };
        let plateau_of = |ph: &Ph| matches!(ph, Ph::Melt | Ph::Boil);
        if p.b("plateau_dashes") {
            for s in &seq {
                if !plateau_of(&s.0) { continue; }
                let tv = if s.0 == Ph::Melt { mp } else { bp };
                let i = seq.iter().position(|q| std::ptr::eq(q, s)).unwrap_or(0);
                let xa = corners_px[i].0;
                if tv < lo || tv > hi || xa - g.x0 <= 1.0 { continue; }
                pl.lines.push([g.x0, hf.py(tv), xa, hf.py(tv)]);
            }
        }
        for w in clip.windows(2) { pl.lines.push([w[0].0, w[0].1, w[1].0, w[1].1]); }
        // temperature labels at the plateaus
        let mut point_out: Vec<(f64, f64, String)> = Vec::new();
        let mut point_blank: Vec<[f64; 4]> = Vec::new();
        let tmode = p.s("temp_labels");
        if tmode != "None" {
            for s in &seq {
                if !plateau_of(&s.0) { continue; }
                let tv = if s.0 == Ph::Melt { mp } else { bp };
                if tv < lo || tv > hi { continue; }
                let ay = hf.py(tv);
                let text = match tmode {
                    "m.p. and b.p." => if s.0 == Ph::Melt { if cooling { "f.p." } else { "m.p." } } else { "b.p." }.to_string(),
                    "Blank lines" => String::new(),
                    _ => format!("{} °C", tick(tv)),
                };
                let w = if text.is_empty() { 60.0 } else { text.chars().count() as f64 * cw };
                let mut c = Vec::new();
                for e in [6.0, 30.0, 60.0, 100.0, 150.0] { for t in [ay - 6.0 - h, ay + 6.0] { c.push([g.x0 + e, t, g.x0 + e + w, t + h]); } }
                let c4: Vec<[f64; 4]> = c.iter().take(4).cloned().collect();
                let l = c.iter().find(|b| pl.ok(b)).cloned().unwrap_or_else(|| pl.best(&c4));
                pl.boxes.push(l);
                if text.is_empty() { point_blank.push([l[0], l[3], l[2], l[3]]); } else { point_out.push((l[0], l[3] - h * 0.22, text)); }
            }
        }
        // segment labels: plateaus first, then the sloped runs
        let seg_mode = p.s("seg_labels");
        let mut seg_out: Vec<(usize, [f64; 4])> = Vec::new();
        if seg_mode != "None" {
            let mut order: Vec<usize> = (0..seq.len()).collect();
            order.sort_by_key(|&i| if plateau_of(&seq[i].0) { 0 } else { 1 });
            for i in order {
                let s = &seq[i];
                let (a, o) = (corners_px[i], corners_px[i + 1]);
                let mid = ((a.0 + o.0) / 2.0, (a.1 + o.1) / 2.0);
                if mid.0 < g.x0 || mid.0 > g.x1 || mid.1 < g.y0 || mid.1 > g.y1 { continue; }
                let name = match s.0 {
                    Ph::Solid => names[0].clone(), Ph::Liquid => names[2].clone(), Ph::Gas => names[4].clone(),
                    Ph::Melt => if change { if cooling { "Freezing".into() } else { "Melting".into() } } else { names[1].clone() },
                    Ph::Boil => if change { if cooling { "Condensing".into() } else { "Boiling".into() } } else { names[3].clone() },
                };
                let c = if seg_mode == "Blank lines" { 80.0 } else { name.chars().count() as f64 * cw };
                let ub = |x: f64, y: f64| [x, y - h / 2.0, x + c, y + h / 2.0];
                let lp = |t: f64| (a.0 + (o.0 - a.0) * t, a.1 + (o.1 - a.1) * t);
                let mut d2: Vec<[f64; 4]> = Vec::new();
                if plateau_of(&s.0) {
                    for k in 0..6 {
                        let n = 10.0 + k as f64 * h * 1.1;
                        for e in [a.1 - n - h / 2.0, a.1 + n + h / 2.0] {
                            d2.push(ub((a.0 + o.0 - c) / 2.0, e)); d2.push(ub(o.0 - 2.0 - c, e)); d2.push(ub(a.0 + 2.0, e));
                            d2.push(ub(a.0 + 8.0, e)); d2.push(ub(a.0 + 16.0, e)); d2.push(ub(a.0 - 4.0 - c, e));
                        }
                    }
                } else {
                    for e in [10.0, 10.0 + h, 10.0 + 2.0 * h] {
                        for t in [0.5, 0.35, 0.65, 0.2, 0.8] { let q = lp(t); d2.push(ub(q.0 - e - c, q.1)); d2.push(ub(q.0 + e, q.1)); }
                    }
                    for e in [10.0, 10.0 + h, 10.0 + 2.0 * h] {
                        for t in [0.5, 0.0, 1.0] { let q = lp(t); d2.push(ub(q.0 - c / 2.0, q.1 - e - h / 2.0)); d2.push(ub(q.0 - c / 2.0, q.1 + e + h / 2.0)); }
                    }
                }
                let f = d2.iter().find(|b| pl.ok(b)).cloned().unwrap_or_else(|| pl.best(&d2));
                pl.boxes.push(f);
                seg_out.push((i, f));
            }
            seg_out.sort_by_key(|q| q.0);
        }
        for (i, f) in &seg_out {
            if seg_mode == "Blank lines" { d.line(f[0], f[3], f[2], f[3], INK, 1.5); }
            else {
                let name = match seq[*i].0 {
                    Ph::Solid => names[0].clone(), Ph::Liquid => names[2].clone(), Ph::Gas => names[4].clone(),
                    Ph::Melt => if change { if cooling { "Freezing".into() } else { "Melting".into() } } else { names[1].clone() },
                    Ph::Boil => if change { if cooling { "Condensing".into() } else { "Boiling".into() } } else { names[3].clone() },
                };
                halo(&mut d, f[0], f[3] - h * 0.22, 15.4, "start", &name);
            }
        }
        for (x, y, t) in &point_out { halo(&mut d, *x, *y, 15.4, "start", t); }
        for b in &point_blank { d.line(b[0], b[1], b[2], b[3], INK, 1.5); }
        for s in &seq {
            if !plateau_of(&s.0) || !p.b("plateau_dashes") { continue; }
            let tv = if s.0 == Ph::Melt { mp } else { bp };
            let i = seq.iter().position(|q| std::ptr::eq(q, s)).unwrap_or(0);
            dashed(&mut d, g.x0, hf.py(tv), corners_px[i].0.min(corners_px[i + 1].0), hf.py(tv), INK, 1.3);
        }
    }
    if p.b("letters") {
        for (i, &(x, y)) in corners.iter().enumerate() {
            let prev = if i > 0 { corners[i - 1] } else { (x - 1.0, y) };
            let next = corners.get(i + 1).cloned().unwrap_or((x + 1.0, y));
            let cross = (x - prev.0) * (next.1 - y) - (y - prev.1) * (next.0 - x);
            let (ox, oy) = if cross < 0.0 { (-6.0, -12.0) } else { (8.0, 22.0) };
            halo(&mut d, x + ox, y + oy, 16.0, "middle", &((b'A' + i as u8) as char).to_string());
        }
    }
    wrap_at(p, d.finish(), &[], Some((g.x0 + g.x1) / 2.0))
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
    if p.s("source") == "Key points" || (p.s("common") != "Custom" && !p.s("common").is_empty()) { return v; }
    let ph_eq = f.ph(f.veq);
    v.push(
        Handle::diamond((f.px(f.veq), f.py(ph_eq)), "Equivalence point: drag sideways to change the analyte volume", |q, t| {
            let f = tit_frame(q);
            let vol = f.g.vx(t.0) * f.ct / f.ca;
            super::handles::set_eff(q, "volume", vol, 1.0, 200.0);
        })
        .with_cursor(Cursor::ResizeH),
    );
    if f.kind.starts_with("Weak") {
        let hv = f.veq / 2.0;
        v.push(
            Handle::knob((f.px(hv), f.py(f.ph(hv))), "Half-equivalence point: drag up or down to set pKa / pKb", |q, t| {
                let f0 = tit_frame(q);
                let target = f0.g.vy(t.1).clamp(0.0, 14.0);
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
        if i == 0 {
            let first_is_lo = !hf.cooling;
            let key = key_for(first_is_lo);
            out.push(Handle::knob((xa, hf.py(s.1)), "Starting temperature: drag up or down", move |q, t| {
                let fr = heat_frame(q);
                q.set_f(key, fr.inv_t(t.1).round());
            }).with_cursor(Cursor::ResizeV));
        }
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
                    let d = fr.inv_u(t.0) - ustart;
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

// ---------------------------------------------------------------------------------------------
// Examples
// ---------------------------------------------------------------------------------------------

#[allow(unused_imports)]
use crate::figures::{Example, Value};
#[allow(dead_code)]
fn ex(name: &'static str, params: Vec<(&'static str, Value)>) -> Example { Example { name, params } }

fn sv(v: &str) -> Value { Value::S(v.into()) }
fn bv(v: bool) -> Value { Value::B(v) }
fn fv(v: f64) -> Value { Value::F(v) }

pub(super) fn examples(id: &str) -> Option<Vec<Example>> {
    Some(match id {
        "heating-cooling-curve" => vec![
            ex("Heating curve of water (lettered)", vec![("letters", bv(true)), ("plateau_labels", sv("The change (Melting)")), ("scale", sv("To scale")), ("curve_color", sv("#c0392b"))]),
            ex("Cooling curve of water", vec![("mode", sv("Cooling")), ("start", fv(120.0)), ("end", fv(-20.0)), ("plateau_labels", sv("The change (Melting)"))]),
            ex("Heating curve of ethanol", vec![("substance", sv("Ethanol")), ("start", fv(-140.0)), ("end", fv(100.0)), ("plateau_labels", sv("The change (Melting)"))]),
            ex("Heating curve of water to scale (heat added)", vec![("scale", sv("To scale")), ("xaxis", sv("Heat added"))]),
            ex("Cooling curve with supercooling", vec![("mode", sv("Cooling")), ("start", fv(80.0)), ("end", fv(-20.0)), ("supercool", bv(true)), ("letters", bv(true)), ("plateau_labels", sv("The change (Melting)"))]),
            ex("Heating curve worksheet: substance X", vec![("substance", sv("Substance X")), ("start", fv(0.0)), ("end", fv(110.0)), ("seg_labels", sv("Blank lines")), ("temp_labels", sv("Blank lines")), ("letters", bv(true))]),
        ],
        "titration-curve" => vec![
            ex("Weak acid with a strong base", vec![("common", sv("CH₃COOH (acetic acid) with NaOH")), ("half_mark", sv("Dot and lines to the axes"))]),
            ex("Strong acid with a strong base", vec![("common", sv("HCl with NaOH"))]),
            ex("Weak base with a strong acid", vec![("common", sv("NH₃ (ammonia) with HCl")), ("half_mark", sv("Dot"))]),
            ex("Strong base with a strong acid", vec![("common", sv("NaOH with HCl"))]),
            ex("Worksheet: label the equivalence point", vec![("common", sv("HF with NaOH")), ("half_mark", sv("Dot and lines to the axes")), ("eq_label_style", sv("Blank line")), ("half_label_style", sv("Blank line")), ("xaxis_fit", bv(false)), ("xaxis_to", fv(40.0)), ("xaxis_by", fv(2.0)), ("answer", bv(true))]),
        ],
        _ => return None,
    })
}
