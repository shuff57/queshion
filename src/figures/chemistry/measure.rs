//! Instrument-reading figures: volume, displacement, gas syringe, ruler, balance, thermometer, pH.
use super::*;
use super::magnify::{self, Focus};
use crate::figures::{Params, ParamDef, Subject};

const SIZES: [&str; 7] = ["10 mL", "25 mL", "50 mL", "100 mL", "250 mL", "1000 mL", "600 mL"];
const MAG3: [&str; 3] = ["Instrument and magnifier", "Magnifier only", "Instrument only"];
const VOL_MARKS: [&str; 12] = ["Automatic", "0.1 mL", "0.2 mL", "0.5 mL", "1 mL", "2 mL", "5 mL", "10 mL", "20 mL", "25 mL", "50 mL", "100 mL"];
const VOL_NUMS: [&str; 12] = ["Automatic", "1 mL", "2 mL", "5 mL", "10 mL", "20 mL", "25 mL", "50 mL", "100 mL", "200 mL", "0.5 mL", "No numbers"];
const COLORS: [&str; 4] = ["Gray", "Blue", "Red", "Green"];

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "volume-reading", subject: Subject::Chemistry, title: "Volume Reading",
            blurb: "Graduated cylinder, burette or beaker with the liquid at a chosen level, for practising meniscus readings.",
            params: volume_params, render: volume_render },
        FigureDef { id: "volume-by-displacement", subject: Subject::Chemistry, title: "Volume by Displacement",
            blurb: "Before and after cylinders showing how a submerged solid raises the liquid level.",
            params: disp_params, render: disp_render },
        FigureDef { id: "gas-syringe", subject: Subject::Chemistry, title: "Gas Syringe",
            blurb: "A gas syringe with the plunger at a set volume, alone or connected to a flask on a stand.",
            params: syringe_params, render: syringe_render },
        FigureDef { id: "length-reading", subject: Subject::Chemistry, title: "Length Reading",
            blurb: "A ruler with an object laid along it, for measuring to the right number of digits.",
            params: length_params, render: length_render },
        FigureDef { id: "mass-reading", subject: Subject::Chemistry, title: "Mass Reading",
            blurb: "A triple beam or digital balance showing the mass of an object on the pan.",
            params: mass_params, render: mass_render },
        FigureDef { id: "temperature-reading", subject: Subject::Chemistry, title: "Temperature Reading",
            blurb: "A liquid-in-glass thermometer or digital probe in Celsius, Kelvin or Fahrenheit.",
            params: temp_params, render: temp_render },
        FigureDef { id: "ph-reading", subject: Subject::Chemistry, title: "pH Reading",
            blurb: "A digital meter, analog meter or strip of pH paper showing a chosen pH.",
            params: ph_params, render: ph_render },
    ]
}

// ---------------------------------------------------------------------------------------------
// Vessels
// ---------------------------------------------------------------------------------------------

struct Vessel {
    cx: f64, w: f64, ytop: f64, ybot: f64, max: f64, major: f64, minor: f64,
    inverted: bool, level: f64, color: &'static str, edge: &'static str, mdepth: f64,
}

impl Vessel {
    fn y(&self, v: f64) -> f64 {
        let t = v / self.max;
        if self.inverted { self.ytop + t * (self.ybot - self.ytop) } else { self.ybot - t * (self.ybot - self.ytop) }
    }
}

fn lab(v: f64, major: f64) -> String { if major >= 1.0 || (v - v.round()).abs() < 1e-9 { format!("{}", v.round() as i64) } else { num(v, 1) } }

fn liquid_shape(d: &mut Doc, v: &Vessel, yb: f64) {
    let (xl, xr) = (v.cx - v.w / 2.0, v.cx + v.w / 2.0);
    let yl = v.y(v.level);
    let m = v.mdepth;
    let p = format!(
        "M{xl:.2},{yb:.2} L{xl:.2},{:.2} C{:.2},{:.2} {:.2},{yl:.2} {:.2},{yl:.2} C{:.2},{yl:.2} {:.2},{:.2} {xr:.2},{:.2} L{xr:.2},{yb:.2} Z",
        yl - m, xl + 3.0, yl - m * 0.15, xl + v.w * 0.2, v.cx, xr - v.w * 0.2, xr - 3.0, yl - m * 0.15, yl - m
    );
    d.path(&p, v.color, v.edge, 1.0);
}

/// Tick marks every `v.minor`, long ticks + numbers every `num` (0 = long ticks every
/// `v.major` without numbers), medium ticks halfway.
fn scale_ticks(d: &mut Doc, v: &Vessel, side_x: f64, dir: f64, label_x: f64, label_anchor: &str, unit: &str) {
    scale_ticks_n(d, v, side_x, dir, label_x, label_anchor, unit, v.major, true);
}

fn scale_ticks_n(d: &mut Doc, v: &Vessel, side_x: f64, dir: f64, label_x: f64, label_anchor: &str, unit: &str, num_step: f64, numbers: bool) {
    let n = (v.max / v.minor).round() as i64;
    let on = |val: f64, step: f64| { let r = val / step; (r - r.round()).abs() < 1e-6 };
    let gap = (v.y(v.minor) - v.y(0.0)).abs();
    let thin = (gap * 0.35).min(0.9).max(0.15);
    let lab_gap = (v.y(num_step) - v.y(0.0)).abs();
    let lab_every = ((15.0 / lab_gap.max(0.1)).ceil() as i64).max(1);
    for k in 0..=n {
        let val = k as f64 * v.minor;
        let y = v.y(val);
        let is_major = on(val, num_step);
        let is_mid = !is_major && on(val, num_step / 2.0) && (num_step / 2.0) / v.minor >= 1.0 - 1e-9;
        let len = if is_major { 22.0 } else if is_mid { 15.0 } else { 9.0 };
        d.line(side_x, y, side_x + dir * len, y, INK, if is_major { 1.4f64.min(thin * 2.5).max(0.6) } else { thin });
        let idx = (val / num_step).round() as i64;
        // The reference omits the number at the bottom of the scale (0 or -10).
        if is_major && numbers && idx != 0 && idx % lab_every == 0 { tx(d, label_x, y + 4.0, 12.0, label_anchor, INK, false, &lab(val, num_step)); }
    }
    tx(d, label_x, v.ytop.min(v.ybot) - 36.0, 12.0, label_anchor, INK, true, unit);
}

fn volume_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("instrument", "Instrument", &["Graduated cylinder", "Buret", "Beaker"], "Graduated cylinder"),
        ParamDef::choice("size", "Size (cylinder or beaker; the buret is 50 mL, or 25 mL for sizes up to 25)", &SIZES, "100 mL"),
        ParamDef::choice("color", "Liquid color", &COLORS, "Gray"),
        ParamDef::float("reading", "Reading (mL)", 0.0, 1000.0, 0.05, 43.6),
        ParamDef::bool("dotted", "Dotted line at the meniscus", false),
        ParamDef::bool("answer", "Answer key", false)
            .hint("The reading is printed under the figure."),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::choice("marks", "Smallest marks", &VOL_MARKS, "Automatic"),
        ParamDef::choice("numbered", "Numbered every", &VOL_NUMS, "Automatic"),
        ParamDef::choice("unit", "Unit", &["mL", "cm³"], "mL"),
        magnify::show_param(&MAG3, "Instrument only"),
        magnify::marks_param(),
        title_style(false),
    ]
}

/// "one decimal place" / "2 decimal places" / "whole numbers".
fn places_text(dp: usize) -> String {
    match dp { 0 => "whole numbers".into(), 1 => "one decimal place".into(), n => format!("{n} decimal places") }
}

/// The reading section's range line ("From 0 to 100 mL, to one decimal place.") for the current
/// size, unit and marks, so it never names a range the figure isn't drawing.
pub fn range_note(id: &str, p: &Params) -> Option<String> {
    let max_txt = |v: f64| num(v, 0);
    Some(match id {
        "volume-reading" => {
            let (max, _, minor, _) = vol_scale(p);
            let unit = if p.s("unit").is_empty() { "mL" } else { p.s("unit") };
            format!("From 0 to {} {unit}, to {}.", max_txt(max), places_text(read_dp(minor)))
        }
        "volume-by-displacement" => {
            let max = size_ml(p.s("size"));
            let (n0, m0) = cyl_marks(max);
            let (_, minor, _) = marks_from(p, max, n0, m0);
            let unit = if p.s("unit").is_empty() { "mL" } else { p.s("unit") };
            format!("From 0 to {} {unit}, to {}.", max_txt(max), places_text(read_dp(minor)))
        }
        "gas-syringe" => {
            let max = if p.s("size").starts_with("50") { 50.0 } else { 100.0 };
            format!("From 0 to {} {}, to one decimal place.", max_txt(max), p.s("unit"))
        }
        "length-reading" => {
            let imperial = p.s("units").starts_with("Imp");
            let cm: f64 = p.s("ruler").split_whitespace().next().and_then(|t| t.parse().ok()).unwrap_or(15.0);
            let (total, unit) = if imperial { (match cm as i64 { 15 => 6.0, 20 => 8.0, 30 => 12.0, 50 => 20.0, _ => 40.0 }, "in") } else { (cm, "cm") };
            format!("From 0.05 to {} {unit}, to 2 decimal places.", num(total, 2))
        }
        "mass-reading" => {
            let dp = if p.s("balance").starts_with("Triple") { 2 } else { match p.s("places") { "0.1" => 1, "0.001" => 3, _ => 2 } };
            format!("From 0 to 610 g, to {}.", places_text(dp))
        }
        "temperature-reading" => {
            let sym = match p.s("unit") { "Kelvin (K)" => "K", "Fahrenheit (°F)" => "°F", _ => "°C" };
            if p.s("thermometer") == "Digital probe" {
                let dp = if p.s("places") == "0" { 0 } else { 1 };
                format!("From -50 to 400 {sym}, to {}.", places_text(dp))
            } else {
                let (lo, hi) = match sym { "K" => (260, 380), "°F" => (10, 230), _ => (-10, 110) };
                format!("From {} to {hi} {sym}, to one decimal place.", lo)
            }
        }
        "ph-reading" => format!("From 0 to 14, to {}.", places_text(if p.s("places") == "0.0" { 1 } else { 2 })),
        _ => return None,
    })
}

fn size_ml(s: &str) -> f64 { s.split_whitespace().next().and_then(|t| t.parse().ok()).unwrap_or(100.0) }
fn ml_of(s: &str) -> Option<f64> { s.split_whitespace().next().and_then(|t| t.parse().ok()) }

/// (numbered step, minor) graduation spacing for a cylinder of `max` mL (reference defaults).
fn cyl_marks(max: f64) -> (f64, f64) {
    match max as i64 { 10 => (1.0, 0.1), 25 => (5.0, 0.5), 50 => (10.0, 1.0), 100 => (10.0, 1.0), 250 => (20.0, 2.0), 600 => (50.0, 5.0), _ => (100.0, 10.0) }
}

/// Decimal places for a reading with one estimated digit past the smallest mark.
fn read_dp(minor: f64) -> usize { ((-(minor.log10().floor())) + 1.0).max(0.0) as usize }

/// (max, numbered step, minor, numbers shown) of the volume instrument.
fn vol_scale(p: &Params) -> (f64, f64, f64, bool) {
    let kind = p.s("instrument");
    let size = size_ml(p.s("size"));
    let (max, (num0, minor0)) = match kind {
        "Buret" => { let m = if size <= 25.0 { 25.0 } else { 50.0 }; (m, (1.0, if m <= 25.0 { 0.1 } else { 0.1 })) }
        "Beaker" => (size, (size / 5.0, size / 10.0)),
        _ => (size, cyl_marks(size)),
    };
    let (num, minor, numbers) = marks_from(p, max, num0, minor0);
    (max, num, minor, numbers)
}

/// Applies the "Smallest marks" / "Numbered every" choices (Automatic = the given defaults).
fn marks_from(p: &Params, max: f64, num0: f64, minor0: f64) -> (f64, f64, bool) {
    let minor = ml_of(p.s("marks")).filter(|m| max / m <= 600.0).unwrap_or(minor0);
    let numbers = p.s("numbered") != "No numbers";
    let mut num = ml_of(p.s("numbered")).filter(|n| *n >= minor - 1e-9 && max / n <= 60.0).unwrap_or(num0);
    if num < minor { num = minor; }
    (num, minor, numbers)
}

fn volume_render(p: &Params) -> String {
    let kind = p.s("instrument");
    let (fill, edge) = liquid(p.s("color"));
    let burette = kind == "Buret";
    let beaker = kind == "Beaker";
    let (max, num_step, minor, numbers) = vol_scale(p);
    let unit = p.s("unit");
    let unit = if unit.is_empty() { "mL" } else { unit };
    let reading = p.f("reading").clamp(0.0, max);
    let (w_img, h_img) = if burette { (300.0, 740.0) } else if beaker { (330.0, 540.0) } else { (290.0, 560.0) };
    let mut d = Doc::new(w_img, h_img);
    let cx = 140.0;
    let focus;
    if burette {
        let v = Vessel { cx, w: 30.0, ytop: 60.0, ybot: 620.0, max, major: num_step, minor, inverted: true, level: reading, color: fill, edge, mdepth: 5.0 };
        liquid_shape(&mut d, &v, 640.0);
        // glass: tube, stopcock, tip
        let (xl, xr) = (cx - 15.0, cx + 15.0);
        d.path(&format!("M{xl},{:.1} L{xl},640 L{:.1},640 L{:.1},700 L{:.1},700 L{:.1},640 L{xr},640 L{xr},{:.1}", 36.0, cx - 4.0, cx - 2.0, cx + 2.0, cx + 4.0, 36.0), "none", GLASS_EDGE, 1.6);
        d.rect(cx - 24.0, 650.0, 48.0, 12.0, "#e8edf0", GLASS_EDGE, 1.4);
        d.rect(cx + 24.0, 653.0, 30.0, 6.0, "#d44", "#833", 1.0);
        scale_ticks_n(&mut d, &v, xr, -1.0, xr + 12.0, "start", unit, num_step, numbers);
        if p.b("dotted") { dashed(&mut d, cx, v.y(reading), xr + 40.0, v.y(reading), "#555", 1.2); }
        focus = Focus { x: xr + 4.0, y: v.y(reading), label_px: (v.y(num_step) - v.y(0.0)).abs().max(60.0 / p.i("mag_marks").clamp(1, 6) as f64) };
    } else {
        let (w, ytop, ybot) = if beaker { (170.0, 90.0, 470.0) } else { (70.0, 90.0, 500.0) };
        let md = if beaker { 3.0 } else { 7.0 };
        let v = Vessel { cx, w, ytop, ybot, max, major: num_step, minor, inverted: false, level: reading, color: fill, edge, mdepth: md };
        let (xl, xr) = (cx - w / 2.0, cx + w / 2.0);
        liquid_shape(&mut d, &v, ybot + 6.0);
        if beaker {
            d.path(&format!("M{:.1},{:.1} L{xl:.1},{:.1} L{xl:.1},{:.1} L{xr:.1},{:.1} L{xr:.1},{:.1} L{:.1},{:.1}", xl - 8.0, ytop - 40.0, ytop - 30.0, ybot + 6.0, ybot + 6.0, ytop - 30.0, xr + 8.0, ytop - 40.0), "none", GLASS_EDGE, 1.8);
        } else {
            d.path(&format!("M{:.1},{:.1} L{xl:.1},{:.1} L{xl:.1},{:.1} L{xr:.1},{:.1} L{xr:.1},{:.1} L{:.1},{:.1}", xl - 5.0, ytop - 28.0, ytop - 24.0, ybot + 6.0, ybot + 6.0, ytop - 24.0, xr + 5.0, ytop - 28.0), "none", GLASS_EDGE, 1.8);
            d.poly(&[(xl, ybot + 6.0), (xl - 24.0, ybot + 22.0), (xr + 24.0, ybot + 22.0), (xr, ybot + 6.0)], "#eef3f6", GLASS_EDGE, 1.6);
        }
        scale_ticks_n(&mut d, &v, xr, -1.0, xr + 12.0, "start", unit, num_step, numbers);
        if p.b("dotted") { dashed(&mut d, cx, v.y(reading), xr + 40.0, v.y(reading), "#555", 1.2); }
        focus = Focus { x: xr + 4.0, y: v.y(reading), label_px: (v.y(num_step) - v.y(0.0)).abs().max(80.0 / p.i("mag_marks").clamp(1, 6) as f64) };
    }
    let svg = magnify::compose_side(p, d.finish(), &focus);
    let foot = if p.b("answer") { vec![format!("Reading: {} {unit}", num(reading, read_dp(minor)))] } else { vec![] };
    wrap(p, svg, &foot)
}

// ---------------------------------------------------------------------------------------------
// Displacement
// ---------------------------------------------------------------------------------------------

fn disp_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("size", "Graduated cylinder size", &["10 mL", "25 mL", "50 mL", "100 mL"], "10 mL"),
        ParamDef::choice("color", "Liquid color", &COLORS, "Gray"),
        ParamDef::float("before", "Before reading (mL)", 0.0, 100.0, 0.05, 4.0),
        ParamDef::float("after", "After reading (mL)", 0.0, 100.0, 0.05, 6.0),
        ParamDef::choice("object", "Object", &["Marbles", "Rock", "Cube", "Metal cylinder"], "Marbles"),
        ParamDef::int("count", "Number of marbles", 1, 5, 2),
        ParamDef::choice("marble_color", "Marble colour", &["Blue", "Gray"], "Blue"),
        ParamDef::bool("arrow", "Arrow between the cylinders", true),
        ParamDef::bool("dotted", "Dotted line at the meniscus", false),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::text("lab_before", "Under the first cylinder", "Before"),
        ParamDef::text("lab_after", "Under the second cylinder", "After"),
        ParamDef::choice("marks", "Smallest marks", &VOL_MARKS, "Automatic"),
        ParamDef::choice("numbered", "Numbered every", &VOL_NUMS, "Automatic"),
        ParamDef::choice("unit", "Unit", &["mL", "cm³"], "mL"),
        magnify::show_param(&["Cylinders only", "Cylinders and magnifiers"], "Cylinders only"),
        magnify::marks_param(),
        answer_param("Answer key (before, after and the object's volume)"),
        title_style(false),
    ]
}

fn disp_render(p: &Params) -> String {
    let max = size_ml(p.s("size"));
    let (fill, edge) = liquid(p.s("color"));
    let before = p.f("before").clamp(0.0, max);
    let after = p.f("after").clamp(0.0, max).max(before);
    let (n0, m0) = cyl_marks(max);
    let (num_step, minor, numbers) = marks_from(p, max, n0, m0);
    let unit = if p.s("unit").is_empty() { "mL" } else { p.s("unit") };
    let mut d = Doc::new(600.0, 600.0);
    let (w, ytop, ybot) = (84.0, 90.0, 500.0);
    let mut foci = Vec::new();
    for (idx, (cx, lvl)) in [(120.0, before), (400.0, after)].into_iter().enumerate() {
        let v = Vessel { cx, w, ytop, ybot, max, major: num_step, minor, inverted: false, level: lvl, color: fill, edge, mdepth: 7.0 };
        let (xl, xr) = (cx - w / 2.0, cx + w / 2.0);
        liquid_shape(&mut d, &v, ybot + 6.0);
        if idx == 1 {
            let dv_h = (after - before) / max * (ybot - ytop);
            let area = (dv_h * w * 0.85).max(120.0);
            let base = ybot + 4.0;
            match p.s("object") {
                "Marbles" => {
                    let n = p.i("count").clamp(1, 5) as usize;
                    let r = (area / (n as f64 * std::f64::consts::PI)).sqrt().clamp(6.0, w / 2.0 - 3.0);
                    let per = (((w - 4.0) / (2.0 * r)).floor() as usize).max(1);
                    for k in 0..n {
                        let row = k / per;
                        let col = k % per;
                        let in_row = per.min(n - row * per);
                        let x0 = cx - (in_row as f64 - 1.0) * r;
                        let (x, y) = (x0 + col as f64 * 2.0 * r, base - r - row as f64 * r * 1.8);
                        let (body, rim, shine) = if p.s("marble_color") == "Gray" { ("#9a9a9a", "#2b2b2b", "#e6e6e6") } else { ("#8fb4d6", "#2e4f6e", "#d9e9f6") };
                        d.circle(x, y, r, body, rim, 1.5);
                        d.circle(x - r * 0.3, y - r * 0.3, r * 0.25, shine, "none", 0.0);
                    }
                }
                other => {
                    let size = (area.sqrt() * 1.1).clamp(10.0, w - 8.0);
                    object(&mut d, other, cx, base, size, 7);
                }
            }
        }
        d.path(&format!("M{:.1},{:.1} L{xl:.1},{:.1} L{xl:.1},{:.1} L{xr:.1},{:.1} L{xr:.1},{:.1} L{:.1},{:.1}", xl - 5.0, ytop - 28.0, ytop - 24.0, ybot + 6.0, ybot + 6.0, ytop - 24.0, xr + 5.0, ytop - 28.0), "none", GLASS_EDGE, 1.8);
        d.poly(&[(xl, ybot + 6.0), (xl - 24.0, ybot + 22.0), (xr + 24.0, ybot + 22.0), (xr, ybot + 6.0)], "#eef3f6", GLASS_EDGE, 1.6);
        scale_ticks_n(&mut d, &v, xr, -1.0, xr + 12.0, "start", unit, num_step, numbers);
        if p.b("dotted") { dashed(&mut d, cx, v.y(lvl), xr + 40.0, v.y(lvl), "#555", 1.2); }
        tx(&mut d, cx, 560.0, 15.0, "middle", INK, true, if idx == 0 { p.s("lab_before") } else { p.s("lab_after") });
        foci.push(Focus { x: xr + 2.0, y: v.y(lvl), label_px: (v.y(num_step) - v.y(0.0)).abs().max(70.0 / p.i("mag_marks").clamp(1, 6) as f64) });
    }
    if p.b("arrow") {
        d.arrow(222.0, 300.0, 292.0, 300.0, INK, 2.0);
    }
    let svg = magnify::compose(p, d.finish(), &foci, true);
    let dp = read_dp(minor);
    let foot = if p.b("answer") {
        vec![format!("{}: {} {unit} · {}: {} {unit} · Object: {} {unit}", p.s("lab_before"), num(before, dp), p.s("lab_after"), num(after, dp), num(after - before, dp))]
    } else { vec![] };
    wrap(p, svg, &foot)
}

// ---------------------------------------------------------------------------------------------
// Gas syringe
// ---------------------------------------------------------------------------------------------

fn syringe_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("show", "Show", &["Syringe only", "With flask and stand"], "With flask and stand"),
        ParamDef::choice("size", "Gas syringe size", &["50 cm³", "100 cm³"], "100 cm³"),
        ParamDef::choice("unit", "Unit", &["cm³", "mL"], "cm³"),
        ParamDef::float("volume", "Volume of gas", 0.0, 100.0, 0.1, 43.6),
        ParamDef::bool("answer", "Answer key", false)
            .hint("The volume is printed under the figure."),
        ParamDef::text("title", "Chart title text", ""),
        magnify::show_param(&MAG3, "Instrument only"),
        magnify::marks_param(),
        title_style(false),
    ]
}

fn syringe_render(p: &Params) -> String {
    let max = if p.s("size").starts_with("50") { 50.0 } else { 100.0 };
    let unit = p.s("unit");
    let vol = p.f("volume").clamp(0.0, max);
    let full = p.s("show") != "Syringe only";
    let h = if full { 400.0 } else { 220.0 };
    let (x0, len) = (if full { 250.0 } else { 70.0 }, 350.0);
    let mut d = Doc::new(x0 + len + 120.0, h);
    let cy = if full { 110.0 } else { 120.0 };
    let top = cy - 30.0;
    let xp = x0 + vol / max * len;
    if full {
        // stand
        d.rect(40.0, 372.0, 170.0, 12.0, "#cfd4d8", "#4a5258", 1.5);
        d.rect(150.0, 150.0, 8.0, 222.0, "#aab1b6", "#4a5258", 1.2);
        d.rect(100.0, 224.0, 56.0, 7.0, "#aab1b6", "#4a5258", 1.2);
        // flask
        d.path("M75,175 L75,225 L30,340 Q24,358 42,358 L138,358 Q156,358 150,340 L105,225 L105,175 Z", GLASS, GLASS_EDGE, 1.8);
        d.path("M38,326 L142,326 L150,340 Q156,358 138,358 L42,358 Q24,358 30,340 Z", "#dbe6ee", "none", 0.0);
        d.circle(70.0, 342.0, 3.0, "#fff", "#8a9aa6", 1.0);
        d.circle(100.0, 336.0, 2.2, "#fff", "#8a9aa6", 1.0);
        d.rect(70.0, 160.0, 40.0, 20.0, "#c98a4b", "#6e4720", 1.4);
        // tube
        d.polyline(&[(90.0, 160.0), (90.0, cy), (x0 - 28.0, cy)], "#6a7780", 3.0);
    }
    // barrel
    d.rect(x0, top, len, 60.0, GLASS, GLASS_EDGE, 1.8);
    d.rect(x0 - 10.0, top, 10.0, 60.0, "#e3eaef", GLASS_EDGE, 1.4);
    d.rect(x0 - 28.0, cy - 5.0, 18.0, 10.0, "#e3eaef", GLASS_EDGE, 1.4);
    d.rect(x0 + len, top - 4.0, 8.0, 68.0, "#e3eaef", GLASS_EDGE, 1.4);
    // gas fill (light tint)
    d.rect(x0 + 1.0, top + 1.0, (xp - x0 - 1.0).max(0.0), 58.0, "#eaf4e6", "none", 0.0);
    // plunger
    d.rect(xp, top + 3.0, 20.0, 54.0, "#4a4f55", INK, 1.4);
    d.rect(xp + 20.0, cy - 5.0, x0 + len + 90.0 - xp - 20.0, 10.0, "#d5dbe0", GLASS_EDGE, 1.4);
    d.rect(x0 + len + 90.0, cy - 22.0, 8.0, 44.0, "#d5dbe0", GLASS_EDGE, 1.4);
    // scale along the top edge
    let n = (max / 1.0) as i64;
    for k in 0..=n {
        let v = k as f64;
        let x = x0 + v / max * len;
        let maj = k % 10 == 0;
        let mid = k % 5 == 0;
        let l = if maj { 18.0 } else if mid { 12.0 } else { 7.0 };
        d.line(x, top, x, top + l, INK, if maj { 1.3 } else { 0.8 });
        if maj { tx(&mut d, x, top - 6.0, 12.0, "middle", INK, false, &format!("{}", k)); }
    }
    tx(&mut d, x0 - 6.0, top - 6.0, 12.0, "end", INK, true, unit);
    let focus = Focus { x: xp, y: top + 14.0, label_px: len / max * 10.0 };
    let svg = magnify::compose(p, d.finish(), &[focus], true);
    let foot = if p.b("answer") { vec![format!("Volume of gas: {} {}", num(vol, 1), unit)] } else { vec![] };
    wrap(p, svg, &foot)
}

// ---------------------------------------------------------------------------------------------
// Ruler
// ---------------------------------------------------------------------------------------------

fn length_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("units", "Units", &["Metric (cm)", "Imperial (in)"], "Metric (cm)"),
        ParamDef::choice("ruler", "Ruler length", &["15 cm", "20 cm", "30 cm", "50 cm", "100 cm"], "15 cm"),
        ParamDef::choice("marks", "Smallest marks", &["10 cm", "5 cm", "1 cm", "0.5 cm", "1 mm"], "1 mm"),
        ParamDef::float("length", "Object's length", 0.05, 100.0, 0.01, 4.37),
        ParamDef::float("offset", "Left end at", 0.0, 100.0, 0.01, 0.0),
        ParamDef::choice("object", "Object", &["Marbles", "Rock", "Cube", "Metal cylinder"], "Metal cylinder"),
        ParamDef::bool("dashes", "Dashed lines at its ends", false),
        ParamDef::bool("answer", "Answer key", false)
            .hint("The length is printed under the figure."),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::choice("read_to", "Read to", &["One estimated digit", "Nearest mark"], "One estimated digit")
            .hint("How precise the answer key should be."),
        magnify::show_param(&LENGTH_SHOW, "Ruler only"),
        magnify::marks_param(),
        title_style(false),
    ]
}

/// Length reading "Show": one magnifier above the object's right end (the reference's opening
/// look), the original pair of magnifiers under the ruler, or the ruler alone.
const LENGTH_SHOW: [&str; 3] = ["Ruler and magnifiers", "Ruler only", "Ruler and one magnifier above"];

fn length_one_above(p: &Params) -> bool {
    p.s("mag_show") == LENGTH_SHOW[2]
}

fn length_render(p: &Params) -> String {
    let imperial = p.s("units").starts_with("Imp");
    let cm: f64 = p.s("ruler").split_whitespace().next().and_then(|t| t.parse().ok()).unwrap_or(15.0);
    let (total, unit) = if imperial { (match cm as i64 { 15 => 6.0, 20 => 8.0, 30 => 12.0, 50 => 20.0, _ => 40.0 }, "in") } else { (cm, "cm") };
    let want = match (imperial, p.s("marks")) {
        (false, "10 cm") => 10.0, (false, "5 cm") => 5.0, (false, "1 cm") => 1.0, (false, "0.5 cm") => 0.5, (false, _) => 0.1,
        (true, "10 cm") => 1.0, (true, "5 cm") => 0.5, (true, "1 cm") => 0.25, (true, "0.5 cm") => 0.125, (true, _) => 0.0625,
    };
    let (rx0, rw) = (30.0, 640.0);
    let ppu = rw / total;
    // coarsen if the smallest marks would be crowded
    let ladder: [f64; 9] = if imperial { [0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0] } else { [0.1, 0.5, 1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0] };
    let mut u = want;
    for &c in ladder.iter() { if c >= want && c * ppu >= 3.0 { u = c; break; } u = c; }
    // label step: a multiple of u with >= 26px spacing, preferring 1, 2, 5, 10...
    let mut lstep = u;
    for &c in [1.0f64, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0].iter() {
        let c = if imperial { c.max(1.0) } else { c };
        if c >= u && c * ppu >= 26.0 && ((c / u) - (c / u).round()).abs() < 1e-6 { lstep = c; break; }
        lstep = c;
    }
    let mut d = Doc::new(700.0, 250.0);
    let ry = 150.0;
    d.rect(rx0, ry, rw, 64.0, "#f6f1dc", "#6f6640", 1.6);
    let n = (total / u).round() as i64;
    for k in 0..=n {
        let v = k as f64 * u;
        let x = rx0 + v * ppu;
        let rl = v / lstep;
        let rh = v / (lstep / 2.0);
        let (len, sw) = if (rl - rl.round()).abs() < 1e-6 { (30.0, 1.4) } else if (rh - rh.round()).abs() < 1e-6 { (21.0, 1.0) } else { (13.0, 0.8) };
        d.line(x, ry, x, ry + len, INK, sw);
        if (rl - rl.round()).abs() < 1e-6 { tx(&mut d, x, ry + 48.0, 13.0, "middle", INK, false, &format!("{}", v.round() as i64)); }
    }
    tx(&mut d, rx0 + rw - 6.0, ry + 62.0, 11.0, "end", "#555", true, unit);
    // object
    let len = p.f("length").clamp(0.05, total);
    let off = p.f("offset").clamp(0.0, (total - len).max(0.0));
    let (x1, x2) = (rx0 + off * ppu, rx0 + (off + len) * ppu);
    let wpx = x2 - x1;
    let top_of_obj;
    match p.s("object") {
        "Marbles" => {
            let n = ((wpx / 34.0).round() as i64).clamp(1, 12);
            let dia = wpx / n as f64;
            let r = dia / 2.0;
            for k in 0..n {
                let x = x1 + r + k as f64 * dia;
                d.circle(x, ry - r, r, "#8fb4d6", "#2e4f6e", 1.5);
                d.circle(x - r * 0.3, ry - r * 1.3, r * 0.22, "#d9e9f6", "none", 0.0);
            }
            top_of_obj = ry - dia;
        }
        "Rock" => {
            let h = (wpx * 0.55).clamp(20.0, 70.0);
            let c = [(0.0, 0.0), (0.04, -0.55), (0.2, -0.9), (0.5, -1.0), (0.78, -0.85), (0.96, -0.5), (1.0, 0.0)];
            let mut pts: Vec<(f64, f64)> = c.iter().map(|&(a, b)| (x1 + a * wpx, ry + b * h)).collect();
            pts[0].0 = x1;
            d.poly(&pts, "#b9b2a6", "#4d4840", 1.6);
            d.polyline(&[(x1 + wpx * 0.25, ry - h * 0.5), (x1 + wpx * 0.45, ry - h * 0.35), (x1 + wpx * 0.7, ry - h * 0.5)], "#7d766b", 1.0);
            top_of_obj = ry - h;
        }
        "Cube" => {
            let h = wpx.clamp(24.0, 50.0);
            d.rect(x1, ry - h, wpx, h, "#d9c08a", "#5f4b1f", 1.6);
            d.line(x1 + 3.0, ry - h + 6.0, x2 - 3.0, ry - h + 6.0, "#ecdcb4", 3.0);
            top_of_obj = ry - h;
        }
        _ => {
            let h = 36.0;
            d.rect(x1, ry - h, wpx, h, "#b9c1c9", "#3a444d", 1.6);
            d.line(x1 + 3.0, ry - h + 8.0, x2 - 3.0, ry - h + 8.0, "#eef1f3", 3.0);
            top_of_obj = ry - h;
        }
    }
    if p.b("dashes") {
        dashed(&mut d, x1, top_of_obj + 4.0, x1, ry + 40.0, "#c0392b", 1.1);
        dashed(&mut d, x2, top_of_obj + 4.0, x2, ry + 40.0, "#c0392b", 1.1);
    }
    let foci = [Focus { x: x1, y: ry + 22.0, label_px: lstep * ppu }, Focus { x: x2, y: ry + 22.0, label_px: lstep * ppu }];
    let svg = if length_one_above(p) {
        // the lens over the right end, centred on the ruler's edge where the object ends
        magnify::compose_above(p, d.finish(), &Focus { x: x2, y: ry + 6.0, label_px: lstep * ppu })
    } else {
        magnify::compose(p, d.finish(), &foci, true)
    };
    let foot = if p.b("answer") {
        let shown = if p.s("read_to") == "Nearest mark" {
            let r = (len / u).round() * u;
            if imperial { inch_fraction(r) } else { num(r, read_dp(u).saturating_sub(1)) }
        } else { num(len, read_dp(u)) };
        vec![format!("Length: {shown} {}", if imperial { "in" } else { "cm" })]
    } else { vec![] };
    wrap(p, svg, &foot)
}

/// 3.3125 -> "3 5/16".
fn inch_fraction(v: f64) -> String {
    let sixteenths = (v * 16.0).round() as i64;
    let (whole, mut num_, mut den) = (sixteenths / 16, sixteenths % 16, 16);
    while num_ > 0 && num_ % 2 == 0 { num_ /= 2; den /= 2; }
    match (whole, num_) { (w, 0) => format!("{w}"), (0, n) => format!("{n}/{den}"), (w, n) => format!("{w} {n}/{den}") }
}

// ---------------------------------------------------------------------------------------------
// Balance
// ---------------------------------------------------------------------------------------------

fn mass_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("balance", "Balance", &["Triple beam balance", "Digital balance"], "Triple beam balance"),
        ParamDef::choice("pan", "On the pan", &["Nothing", "Marbles", "Rock", "Cube", "Metal cylinder"], "Nothing"),
        ParamDef::float("mass", "Mass (g)", 0.0, 610.0, 0.01, 24.73),
        ParamDef::choice("places", "Digital decimal places", &["0.1", "0.01", "0.001"], "0.01"),
        ParamDef::text("title", "Chart title text", ""),
        magnify::show_param(&["Instrument and magnifier", "Instrument only", "Beams only"], "Instrument only"),
        magnify::marks_param(),
        answer_param("Answer key (the mass under the figure)"),
        title_style(false),
    ]
}

fn mass_render(p: &Params) -> String {
    let triple = p.s("balance").starts_with("Triple");
    let mass = p.f("mass").clamp(0.0, if triple { 610.0 } else { 9999.0 });
    let pan = p.s("pan");
    let mut d = Doc::new(700.0, 400.0);
    let focus;
    if triple {
        // base and pan
        d.path("M40,350 L330,350 L330,372 L40,372 Z", "#7a8790", INK, 1.6);
        d.rect(60.0, 320.0, 80.0, 30.0, "#9fb0bb", INK, 1.4);
        d.rect(96.0, 270.0, 8.0, 50.0, "#4a5258", INK, 1.0);
        d.raw(r##"<ellipse cx="100" cy="266" rx="62" ry="10" fill="#dfe5e9" stroke="#1b1b1b" stroke-width="1.6"/>"##);
        if pan != "Nothing" { object(&mut d, pan, 100.0, 262.0, 62.0, 3); }
        // column
        d.rect(300.0, 120.0, 30.0, 230.0, "#8d9aa3", INK, 1.6);
        let hundreds = ((mass / 100.0).floor() * 100.0).min(500.0);
        let rest = mass - hundreds;
        let tens = ((rest / 10.0).floor() * 10.0).min(90.0);
        let ones = (rest - tens).clamp(0.0, 10.0);
        let bx0 = 330.0;
        let bl = 330.0;
        let beams: [(f64, f64, f64, f64, &str, bool); 3] = [(70.0, 500.0, 100.0, hundreds, "100 g", false), (150.0, 100.0, 10.0, tens, "10 g", false), (230.0, 10.0, 1.0, ones, "1 g", true)];
        for (y, range, step, val, name, fine) in beams {
            d.rect(bx0, y, bl, 22.0, "#e8ecef", INK, 1.6);
            let nsteps = (range / step).round() as i64;
            for k in 0..=nsteps {
                let x = bx0 + 10.0 + k as f64 / nsteps as f64 * (bl - 20.0);
                d.line(x, y + 22.0, x, y + 32.0, INK, 1.3);
                tx(&mut d, x, y + 46.0, 12.0, "middle", INK, false, &format!("{}", (k as f64 * step).round() as i64));
            }
            if fine {
                for k in 0..(nsteps * 10) {
                    if k % 10 == 0 { continue; }
                    let x = bx0 + 10.0 + k as f64 / (nsteps * 10) as f64 * (bl - 20.0);
                    d.line(x, y + 22.0, x, y + if k % 5 == 0 { 28.0 } else { 25.0 }, INK, 0.8);
                }
            }
            let rx = bx0 + 10.0 + val / range * (bl - 20.0);
            d.poly(&[(rx - 8.0, y - 14.0), (rx + 8.0, y - 14.0), (rx + 8.0, y + 2.0), (rx, y + 22.0), (rx - 8.0, y + 2.0)], "#4a5258", INK, 1.2);
            tx(&mut d, bx0 + bl + 6.0, y + 14.0, 12.0, "start", INK, true, name);
        }
        d.rect(bx0 - 6.0, 60.0, 6.0, 210.0, "#6c7880", INK, 1.0);
        d.line(bx0 + bl + 2.0, 60.0, bx0 + bl + 2.0, 270.0, "#888", 0.8);
        focus = Focus { x: bx0 + 10.0 + ones / 10.0 * (bl - 20.0), y: 248.0, label_px: (bl - 20.0) / 10.0 };
    } else {
        let places = match p.s("places") { "0.1" => 1, "0.001" => 3, _ => 2 };
        d.rect(100.0, 250.0, 420.0, 100.0, "#e7ebee", INK, 1.8);
        d.rect(120.0, 266.0, 230.0, 62.0, "#f9fbfa", INK, 1.5);
        tx(&mut d, 336.0, 312.0, 36.0, "end", INK, true, &format!("{}", num(mass, places)));
        tx(&mut d, 130.0, 284.0, 12.0, "start", "#555", false, "g");
        for (i, name) in ["TARE", "ON/OFF"].iter().enumerate() {
            d.circle(400.0 + i as f64 * 54.0, 298.0, 16.0, "#cfd6db", INK, 1.2);
            tx(&mut d, 400.0 + i as f64 * 54.0, 326.0, 8.0, "middle", INK, false, name);
        }
        d.poly(&[(180.0, 250.0), (200.0, 232.0), (440.0, 232.0), (460.0, 250.0)], "#c9d1d6", INK, 1.5);
        d.rect(210.0, 218.0, 220.0, 14.0, "#dde3e7", INK, 1.5);
        if pan != "Nothing" { object(&mut d, pan, 320.0, 218.0, 90.0, 5); }
        focus = Focus { x: 250.0, y: 297.0, label_px: 220.0 / p.i("mag_marks").clamp(1, 6) as f64 };
    }
    let places = if triple { 2 } else { match p.s("places") { "0.1" => 1, "0.001" => 3, _ => 2 } };
    let foot = if p.b("answer") { vec![format!("Mass: {} g", num(mass, places))] } else { vec![] };
    if p.s("mag_show") == "Beams only" && triple {
        let (_, _, body) = magnify::inner(&d.finish());
        let mut c = Doc::new(440.0, 250.0);
        c.raw(&format!(r#"<g transform="translate(-300 -44)">{body}</g>"#));
        return wrap(p, c.finish(), &foot);
    }
    let svg = magnify::compose(p, d.finish(), &[focus], false);
    wrap(p, svg, &foot)
}

// ---------------------------------------------------------------------------------------------
// Thermometer
// ---------------------------------------------------------------------------------------------

fn temp_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("thermometer", "Thermometer", &["Liquid-in-glass", "Digital probe"], "Liquid-in-glass"),
        ParamDef::choice("unit", "Unit", &["Celsius (°C)", "Kelvin (K)", "Fahrenheit (°F)"], "Celsius (°C)"),
        ParamDef::choice("liquid", "Liquid color", &["Red", "Blue", "Gray"], "Red"),
        ParamDef::float("temperature", "Temperature", -50.0, 400.0, 0.1, 23.4),
        ParamDef::choice("places", "Digital decimal places", &["0", "0.1"], "0.1"),
        ParamDef::text("title", "Chart title text", ""),
        magnify::show_param(&MAG3, "Instrument only"),
        magnify::marks_param(),
        answer_param("Answer key (the temperature under the figure)"),
        title_style(false),
    ]
}

fn temp_render(p: &Params) -> String {
    // (lo, hi, minor, label step, symbol)
    let (lo, hi, minor, lstep, sym) = match p.s("unit") {
        "Kelvin (K)" => (260.0, 380.0, 1.0, 10.0, "K"),
        "Fahrenheit (°F)" => (10.0, 230.0, 2.0, 20.0, "°F"),
        _ => (-10.0, 110.0, 1.0, 10.0, "°C"),
    };
    let t = p.f("temperature");
    let mut d = Doc::new(300.0, 640.0);
    let focus;
    if p.s("thermometer") == "Digital probe" {
        let dp = if p.s("places") == "0" { 0 } else { 1 };
        d.rect(60.0, 60.0, 240.0, 120.0, "#e3e8ec", INK, 1.8);
        d.rect(80.0, 78.0, 200.0, 62.0, "#f9fbfa", INK, 1.5);
        tx(&mut d, 255.0, 124.0, 38.0, "end", INK, true, &num(t, dp));
        tx(&mut d, 270.0, 96.0, 15.0, "end", "#444", true, sym);
        d.circle(120.0, 160.0, 7.0, "#c9d1d6", INK, 1.0);
        d.circle(150.0, 160.0, 7.0, "#c9d1d6", INK, 1.0);
        d.path("M180,180 C180,260 120,260 120,330 C120,380 200,360 200,420", "none", "#3c4850", 4.0);
        d.rect(190.0, 420.0, 20.0, 150.0, "#b8c0c6", INK, 1.5);
        d.poly(&[(190.0, 570.0), (210.0, 570.0), (200.0, 600.0)], "#8e999f", INK, 1.5);
        focus = Focus { x: 190.0, y: 109.0, label_px: 200.0 / p.i("mag_marks").clamp(1, 6) as f64 };
    } else {
        let (ytop, ybot) = (70.0, 500.0);
        let t = t.clamp(lo, hi);
        let t_shown = t;
        let _ = t_shown;
        let y = |v: f64| ybot - (v - lo) / (hi - lo) * (ybot - ytop);
        let cx = 110.0;
        let col = match p.s("liquid") { "Blue" => "#2e6fd0", "Gray" => "#7e8790", _ => "#d6342c" };
        d.path(&format!("M{:.1},{:.1} L{:.1},{:.1} A24,24 0 1 0 {:.1},{:.1} L{:.1},{:.1} A7,7 0 0 0 {:.1},{:.1} Z", cx - 7.0, ytop, cx - 7.0, ybot + 14.0, cx + 7.0, ybot + 14.0, cx + 7.0, ytop, cx - 7.0, ytop), GLASS, GLASS_EDGE, 1.8);
        d.circle(cx, ybot + 36.0, 18.0, col, col, 1.0);
        d.rect(cx - 3.0, y(t), 6.0, ybot + 36.0 - y(t), col, col, 1.0);
        let n = ((hi - lo) / minor).round() as i64;
        for k in 0..=n {
            let v = lo + k as f64 * minor;
            let r = (v / lstep) - (v / lstep).round();
            let mj = r.abs() < 1e-6;
            let r5 = (v / (lstep / 2.0)) - (v / (lstep / 2.0)).round();
            let md = r5.abs() < 1e-6;
            let l = if mj { 22.0 } else if md { 15.0 } else { 9.0 };
            d.line(cx + 12.0, y(v), cx + 12.0 + l, y(v), INK, if mj { 1.3 } else { 0.8 });
            // The reference omits the number at the bottom of the scale (-10).
            if mj && k > 0 { tx(&mut d, cx + 40.0, y(v) + 4.0, 12.0, "start", INK, false, &format!("{}", v.round() as i64)); }
        }
        tx(&mut d, cx + 12.0, ytop - 20.0, 14.0, "start", INK, true, sym);
        // between the ticks and their numbers, so the lens shows both whole
        focus = Focus { x: cx + 26.0, y: y(t), label_px: lstep / (hi - lo) * (ybot - ytop) };
    }
    let dp = if p.s("thermometer") == "Digital probe" { if p.s("places") == "0" { 0 } else { 1 } } else { 1 };
    let foot = if p.b("answer") { vec![format!("Temperature: {} {sym}", num(t, dp))] } else { vec![] };
    // the digital probe keeps the old lens over its display; the glass thermometer gets the
    // reference-sized lens that keeps its numbers whole
    let svg = if p.s("thermometer") == "Digital probe" { magnify::compose(p, d.finish(), &[focus], false) } else { magnify::compose_side(p, d.finish(), &focus) };
    wrap(p, svg, &foot)
}

// ---------------------------------------------------------------------------------------------
// pH
// ---------------------------------------------------------------------------------------------

const PH_COLORS: [(f64, f64, f64); 15] = [
    (230.0, 33.0, 42.0), (234.0, 58.0, 40.0), (239.0, 90.0, 42.0), (244.0, 138.0, 43.0), (246.0, 184.0, 50.0),
    (243.0, 223.0, 59.0), (207.0, 224.0, 74.0), (123.0, 194.0, 103.0), (63.0, 174.0, 120.0), (47.0, 159.0, 154.0),
    (59.0, 130.0, 183.0), (74.0, 95.0, 176.0), (90.0, 71.0, 158.0), (106.0, 58.0, 146.0), (122.0, 45.0, 134.0),
];

fn ph_color(ph: f64) -> String {
    let x = ph.clamp(0.0, 14.0);
    let i = (x.floor() as usize).min(13);
    let f = x - i as f64;
    let (a, b) = (PH_COLORS[i], PH_COLORS[i + 1]);
    format!("#{:02x}{:02x}{:02x}", (a.0 + (b.0 - a.0) * f) as u8, (a.1 + (b.1 - a.1) * f) as u8, (a.2 + (b.2 - a.2) * f) as u8)
}

/// Digital meter looks: the original handheld meter with a hanging probe (saved figures), and
/// the reference's opening look, a meter with ON/OFF and CAL whose probe stands in a beaker.
const METER_LOOKS: [&str; 2] = ["Probe hanging", "Probe in a beaker"];

/// The digital meter with its probe standing in a beaker of the solution (reference look).
fn ph_meter_beaker(d: &mut Doc, ph: f64, dp: usize) {
    // meter body, screen and reading
    d.path("M64,128 Q40,128 40,152 L40,396 Q40,420 64,420 L306,420 Q330,420 330,396 L330,152 Q330,128 306,128 Z", "#ffffff", INK, 2.4);
    d.rect(170.0, 116.0, 30.0, 12.0, "#ffffff", INK, 2.0);
    d.path("M72,156 L298,156 Q306,156 306,164 L306,226 Q306,234 298,234 L72,234 Q64,234 64,226 L64,164 Q64,156 72,156 Z", "#eef0ec", INK, 2.2);
    tx(d, 252.0, 214.0, 54.0, "end", INK, true, &num(ph, dp));
    tx(d, 262.0, 214.0, 24.0, "start", INK, true, "pH");
    for (x, n) in [(76.0, "ON/OFF"), (194.0, "CAL")] {
        d.path(&format!("M{a},282 L{b},282 Q{c},282 {c},290 L{c},318 Q{c},326 {b},326 L{a},326 Q{x},326 {x},318 L{x},290 Q{x},282 {a},282 Z", a = x + 8.0, b = x + 92.0, c = x + 100.0), "#ffffff", INK, 2.0);
        tx(d, x + 50.0, 311.0, 15.0, "middle", INK, true, n);
    }
    d.line(126.0, 366.0, 244.0, 366.0, "#a8a8a8", 2.0);
    d.line(126.0, 380.0, 244.0, 380.0, "#a8a8a8", 2.0);
    // cable from the meter's top to the probe
    d.path("M185,116 C185,40 452,28 455,86", "none", INK, 5.0);
    d.path("M185,116 C185,40 452,28 455,86", "none", "#ffffff", 2.2);
    // beaker of solution, the probe standing in it
    d.rect(392.0, 300.0, 116.0, 116.0, "#dedede", "none", 0.0);
    d.line(392.0, 300.0, 508.0, 300.0, "#6a6a6a", 1.6);
    for k in 0..4 {
        let y = 312.0 + k as f64 * 30.0 - 30.0;
        d.line(478.0, y, 500.0, y, INK, 1.8);
    }
    d.path("M449,86 L461,86 L466,100 L444,100 Z", "#ffffff", INK, 1.8);
    d.path("M444,100 L466,100 Q470,100 470,104 L470,144 Q470,148 466,148 L444,148 Q440,148 440,144 L440,104 Q440,100 444,100 Z", "#555555", INK, 2.0);
    d.rect(446.0, 148.0, 18.0, 230.0, "#f4f4f4", INK, 1.8);
    d.line(455.0, 156.0, 455.0, 366.0, "#a8a8a8", 1.6);
    d.path("M380,250 Q392,254 392,266 L392,418 L508,418 L508,250", "none", INK, 2.6);
}

fn ph_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("instrument", "Instrument", &["Digital meter", "Analog meter", "pH paper"], "Digital meter"),
        ParamDef::choice("places", "Decimal places", &["0.0", "0.00"], "0.00"),
        ParamDef::choice("meter", "Digital meter look", &METER_LOOKS, METER_LOOKS[0]),
        ParamDef::float("ph", "pH", 0.0, 14.0, 0.01, 4.62),
        ParamDef::text("title", "Chart title text", ""),
        answer_param("Answer key (the pH under the figure)"),
        title_style(false),
    ]
}

fn ph_render(p: &Params) -> String {
    let ph = p.f("ph").clamp(0.0, 14.0);
    let mut d = Doc::new(560.0, 440.0);
    match p.s("instrument") {
        "Analog meter" => {
            d.rect(60.0, 30.0, 440.0, 330.0, "#e7ebee", INK, 1.8);
            d.rect(80.0, 50.0, 400.0, 250.0, "#fafbf8", INK, 1.4);
            let (cx, cy, r) = (280.0, 270.0, 170.0);
            let ang = |v: f64| std::f64::consts::PI * (1.0 - v / 14.0);
            let pt = |v: f64, rr: f64| (cx + rr * ang(v).cos(), cy - rr * ang(v).sin());
            let a0 = pt(0.0, r);
            let a1 = pt(14.0, r);
            d.path(&format!("M{:.1},{:.1} A{r},{r} 0 0 1 {:.1},{:.1}", a0.0, a0.1, a1.0, a1.1), "none", INK, 1.6);
            for k in 0..=70 {
                let v = k as f64 * 0.2;
                let major = k % 5 == 0;
                let (x1, y1) = pt(v, r);
                let (x2, y2) = pt(v, r - if major { 20.0 } else { 10.0 });
                d.line(x1, y1, x2, y2, INK, if major { 1.6 } else { 0.8 });
                if major { let (lx, ly) = pt(v, r - 36.0); tx(&mut d, lx, ly + 5.0, 14.0, "middle", INK, false, &format!("{}", k / 5)); }
            }
            tx(&mut d, cx, cy - 60.0, 22.0, "middle", INK, true, "pH");
            let (nx, ny) = pt(ph, r - 8.0);
            d.line(cx, cy, nx, ny, "#b02020", 2.4);
            d.circle(cx, cy, 9.0, "#444", INK, 1.2);
            d.circle(150.0, 330.0, 11.0, "#cfd6db", INK, 1.2);
            d.circle(410.0, 330.0, 11.0, "#cfd6db", INK, 1.2);
            d.path("M280,360 C280,400 330,390 330,430", "none", "#3c4850", 3.5);
        }
        "pH paper" => {
            let chart_y = 250.0;
            // strip: lower part wet / coloured
            d.rect(255.0, 20.0, 50.0, 190.0, "#f7f4ea", "#9a9482", 1.4);
            d.rect(255.0, 110.0, 50.0, 100.0, &ph_color(ph), "#9a9482", 1.4);
            tx(&mut d, 280.0, 245.0, 0.1, "middle", INK, false, "");
            d.rect(40.0, chart_y, 480.0, 90.0, "#fbfbf6", INK, 1.4);
            for i in 0..=14 {
                let x = 52.0 + i as f64 * 31.0;
                d.rect(x - 14.0, chart_y + 12.0, 28.0, 40.0, &ph_color(i as f64), INK, 0.9);
                tx(&mut d, x, chart_y + 72.0, 14.0, "middle", INK, false, &format!("{i}"));
            }
            tx(&mut d, 280.0, chart_y + 108.0 + 30.0, 13.0, "middle", "#444", false, "Compare the strip with the colour chart");
        }
        _ if p.s("meter") == METER_LOOKS[1] => {
            ph_meter_beaker(&mut d, ph, if p.s("places") == "0.0" { 1 } else { 2 });
        }
        _ => {
            let dp = if p.s("places") == "0.0" { 1 } else { 2 };
            d.rect(140.0, 20.0, 280.0, 300.0, "#e3e8ec", INK, 1.8);
            d.rect(165.0, 50.0, 230.0, 90.0, "#f9fbfa", INK, 1.5);
            tx(&mut d, 365.0, 114.0, 52.0, "end", INK, true, &num(ph, dp));
            tx(&mut d, 178.0, 78.0, 16.0, "start", "#444", true, "pH");
            for (i, n) in ["CAL", "MODE", "HOLD"].iter().enumerate() {
                d.circle(200.0 + i as f64 * 80.0, 200.0, 20.0, "#cfd6db", INK, 1.3);
                tx(&mut d, 200.0 + i as f64 * 80.0, 238.0, 10.0, "middle", INK, false, n);
            }
            d.path("M280,320 C280,350 330,340 330,372", "none", "#3c4850", 3.5);
            d.rect(321.0, 372.0, 18.0, 56.0, "#b8c0c6", INK, 1.5);
        }
    }
    let dp = match p.s("instrument") { "pH paper" => 0, "Analog meter" => 2, _ => if p.s("places") == "0.0" { 1 } else { 2 } };
    let foot = if p.b("answer") { vec![format!("pH: {}", num(ph, dp))] } else { vec![] };
    wrap(p, d.finish(), &foot)
}

// ---------------------------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------------------------

use crate::figures::interact::{self, Cursor, Handle};
use super::handles::set_eff;

pub(super) fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    Some(match id {
        "volume-reading" => vol_handles(p),
        "volume-by-displacement" => disp_handles(p),
        "gas-syringe" => syringe_handles(p),
        "length-reading" => length_handles(p),
        "mass-reading" => mass_handles(p),
        "temperature-reading" => temp_handles(p),
        "ph-reading" => ph_handles(p),
        _ => return None,
    })
}

/// Effective scale maximum of the volume-reading instrument (mirrors `volume_render`).
fn vol_max(p: &Params) -> f64 { vol_scale(p).0 }

/// y of volume `v` and its inverse, for the vessel the renderer draws.
fn vol_axis(p: &Params) -> (f64, f64, bool, f64) {
    match p.s("instrument") {
        "Buret" => (60.0, 620.0, true, 140.0),
        "Beaker" => (90.0, 470.0, false, 140.0),
        _ => (90.0, 500.0, false, 140.0),
    }
}

fn vol_handles(p: &Params) -> Vec<Handle> {
    if magnify::magnifier_only(p) { return vec![]; }
    let max = vol_max(p);
    let (ytop, ybot, inv, cx) = vol_axis(p);
    let r = p.f("reading").clamp(0.0, max);
    let y = if inv { ytop + r / max * (ybot - ytop) } else { ybot - r / max * (ybot - ytop) };
    vec![Handle::bar((cx, y), true, "Drag the liquid level to set the reading", move |q, t| {
        let max = vol_max(q);
        let (ytop, ybot, inv, _) = vol_axis(q);
        let v = if inv { (t.1 - ytop) / (ybot - ytop) * max } else { (ybot - t.1) / (ybot - ytop) * max };
        set_eff(q, "reading", v, 0.0, max);
    })
    .with_scroll(|q, dy| {
        let max = vol_max(q);
        set_eff(q, "reading", q.f("reading").clamp(0.0, max) + dy * 0.1, 0.0, max);
    })]
}

fn disp_handles(p: &Params) -> Vec<Handle> {
    let max = size_ml(p.s("size"));
    let (ytop, ybot) = (90.0, 500.0);
    let before = p.f("before").clamp(0.0, max);
    let after = p.f("after").clamp(0.0, max).max(before);
    let ly = move |v: f64| ybot - v / max * (ybot - ytop);
    vec![
        Handle::bar((120.0, ly(before)), true, "Drag the 'before' liquid level", move |q, t| {
            let max = size_ml(q.s("size"));
            set_eff(q, "before", (ybot - t.1) / (ybot - ytop) * max, 0.0, max);
        }),
        Handle::bar((400.0, ly(after)), true, "Drag the 'after' liquid level", move |q, t| {
            let max = size_ml(q.s("size"));
            let b = q.f("before").clamp(0.0, max);
            set_eff(q, "after", (ybot - t.1) / (ybot - ytop) * max, b, max);
        }),
        Handle::label((120.0, 560.0), (110.0, 26.0), "lab_before", "Double-click to rename"),
        Handle::label((400.0, 560.0), (110.0, 26.0), "lab_after", "Double-click to rename"),
    ]
}

fn syringe_geom(p: &Params) -> (f64, f64, f64, f64) {
    let max = if p.s("size").starts_with("50") { 50.0 } else { 100.0 };
    let full = p.s("show") != "Syringe only";
    let x0 = if full { 250.0 } else { 70.0 };
    let cy = if full { 110.0 } else { 120.0 };
    (max, x0, 350.0, cy)
}

fn syringe_handles(p: &Params) -> Vec<Handle> {
    if magnify::magnifier_only(p) { return vec![]; }
    let (max, x0, len, cy) = syringe_geom(p);
    let vol = p.f("volume").clamp(0.0, max);
    vec![Handle::bar((x0 + vol / max * len + 10.0, cy), false, "Drag the plunger to set the volume", move |q, t| {
        let (max, x0, len, _) = syringe_geom(q);
        set_eff(q, "volume", (t.0 - 10.0 - x0) / len * max, 0.0, max);
    })
    .with_scroll(|q, dy| {
        let (max, ..) = syringe_geom(q);
        set_eff(q, "volume", q.f("volume").clamp(0.0, max) + dy * 0.5, 0.0, max);
    })]
}

/// Total ruler length in the ruler's own unit (mirrors `length_render`).
fn ruler_total(p: &Params) -> f64 {
    let imperial = p.s("units").starts_with("Imp");
    let cm: f64 = p.s("ruler").split_whitespace().next().and_then(|t| t.parse().ok()).unwrap_or(15.0);
    if imperial { match cm as i64 { 15 => 6.0, 20 => 8.0, 30 => 12.0, 50 => 20.0, _ => 40.0 } } else { cm }
}

fn length_handles(p: &Params) -> Vec<Handle> {
    if magnify::magnifier_only(p) { return vec![]; }
    let total = ruler_total(p);
    let (rx0, rw, ry) = (30.0, 640.0, 150.0);
    let ppu = rw / total;
    let len = p.f("length").clamp(0.05, total);
    let off = p.f("offset").clamp(0.0, (total - len).max(0.0));
    let ymid = ry - 18.0 + if length_one_above(p) { magnify::above_shift() } else { 0.0 };
    let (x1, x2) = (rx0 + off * ppu, rx0 + (off + len) * ppu);
    vec![
        Handle::knob((x1, ymid), "Left end: drag to move it (the right end stays put)", move |q, t| {
            let total = ruler_total(q);
            let ppu = 640.0 / total;
            let ln = q.f("length").clamp(0.05, total);
            let of = q.f("offset").clamp(0.0, (total - ln).max(0.0));
            let right = of + ln;
            let new_off = ((t.0 - 30.0) / ppu).clamp(0.0, right - 0.05);
            set_eff(q, "offset", new_off, 0.0, total);
            set_eff(q, "length", right - new_off, 0.05, total);
        })
        .with_cursor(Cursor::ResizeH),
        Handle::knob((x2, ymid), "Right end: drag to change the length", move |q, t| {
            let total = ruler_total(q);
            let ppu = 640.0 / total;
            let ln = q.f("length").clamp(0.05, total);
            let of = q.f("offset").clamp(0.0, (total - ln).max(0.0));
            set_eff(q, "length", ((t.0 - 30.0) / ppu - of).clamp(0.05, total - of), 0.05, total);
        })
        .with_cursor(Cursor::ResizeH),
        Handle::square(((x1 + x2) / 2.0, ymid - 14.0), "Drag to slide the object along the ruler", move |q, t| {
            let total = ruler_total(q);
            let ppu = 640.0 / total;
            let ln = q.f("length").clamp(0.05, total);
            let of = q.f("offset").clamp(0.0, (total - ln).max(0.0));
            let _ = of;
            set_eff(q, "offset", (t.0 - 30.0) / ppu - ln / 2.0, 0.0, (total - ln).max(0.0));
        })
        .with_cursor(Cursor::Move),
    ]
}

fn mass_handles(p: &Params) -> Vec<Handle> {
    if p.s("mag_show") == "Beams only" && p.s("balance").starts_with("Triple") { return vec![]; }
    if !p.s("balance").starts_with("Triple") {
        return vec![Handle::zone((230.0, 297.0), (230.0, 62.0), "Scroll to change the mass", |_| {}).with_scroll(|q, dy| {
            q.set_f("mass", q.f("mass") + dy * 0.1);
        })];
    }
    let mass = p.f("mass").clamp(0.0, 610.0);
    let hundreds = ((mass / 100.0).floor() * 100.0).min(500.0);
    let rest = mass - hundreds;
    let tens = ((rest / 10.0).floor() * 10.0).min(90.0);
    let ones = (rest - tens).clamp(0.0, 10.0);
    let rx = |val: f64, range: f64| 340.0 + val / range * 310.0;
    // (beam y, range, step, current)
    let beams: [(f64, f64, f64, f64, &str); 3] = [
        (70.0, 500.0, 100.0, hundreds, "100 g rider: drag along its beam"),
        (150.0, 100.0, 10.0, tens, "10 g rider: drag along its beam"),
        (230.0, 10.0, 0.0, ones, "1 g rider: drag along its beam"),
    ];
    beams
        .iter()
        .enumerate()
        .map(|(i, &(y, range, step, cur, tip))| {
            Handle::knob((rx(cur, range), y - 4.0), tip, move |q, t| {
                let m = q.f("mass").clamp(0.0, 610.0);
                let h = ((m / 100.0).floor() * 100.0).min(500.0);
                let r = m - h;
                let te = ((r / 10.0).floor() * 10.0).min(90.0);
                let on = (r - te).clamp(0.0, 10.0);
                let mut v = (t.0 - 340.0) / 310.0 * range;
                if step > 0.0 { v = (v / step).round() * step; }
                v = v.clamp(0.0, range);
                let (nh, nt, no) = match i { 0 => (v, te, on), 1 => (h, v, on), _ => (h, te, v) };
                set_eff(q, "mass", nh + nt + no, 0.0, 610.0);
            })
            .with_cursor(Cursor::ResizeH)
        })
        .collect()
}

/// (lo, hi) of the liquid-in-glass scale for the chosen unit.
fn temp_scale(p: &Params) -> (f64, f64) {
    match p.s("unit") { "Kelvin (K)" => (260.0, 380.0), "Fahrenheit (°F)" => (10.0, 230.0), _ => (-10.0, 110.0) }
}

fn temp_handles(p: &Params) -> Vec<Handle> {
    if magnify::magnifier_only(p) { return vec![]; }
    if p.s("thermometer") == "Digital probe" {
        return vec![Handle::zone((180.0, 109.0), (200.0, 62.0), "Scroll to change the temperature", |_| {}).with_scroll(|q, dy| {
            q.set_f("temperature", q.f("temperature") + dy * 0.1);
        })];
    }
    let (lo, hi) = temp_scale(p);
    let (ytop, ybot) = (70.0, 500.0);
    let t = p.f("temperature").clamp(lo, hi);
    let y = ybot - (t - lo) / (hi - lo) * (ybot - ytop);
    vec![Handle::bar((110.0, y), true, "Drag the top of the liquid to set the temperature", move |q, tg| {
        let (lo, hi) = temp_scale(q);
        set_eff(q, "temperature", lo + (500.0 - tg.1) / 430.0 * (hi - lo), lo, hi);
    })
    .with_scroll(|q, dy| {
        let (lo, hi) = temp_scale(q);
        set_eff(q, "temperature", q.f("temperature").clamp(lo, hi) + dy * 0.1, lo, hi);
    })]
}

fn ph_handles(p: &Params) -> Vec<Handle> {
    let ph = p.f("ph").clamp(0.0, 14.0);
    match p.s("instrument") {
        "Analog meter" => {
            let (cx, cy, r) = (280.0, 270.0, 170.0);
            let a = std::f64::consts::PI * (1.0 - ph / 14.0);
            vec![Handle::knob((cx + (r - 8.0) * a.cos(), cy - (r - 8.0) * a.sin()), "Drag the needle to set the pH", move |q, t| {
                let mut th = interact::angle_deg((cx, cy), t);
                if th > 180.0 { th = if th < 270.0 { 180.0 } else { 0.0 }; }
                set_eff(q, "ph", 14.0 * (1.0 - th / 180.0), 0.0, 14.0);
            })]
        }
        "pH paper" => (0..=14)
            .map(|i| {
                Handle::zone((52.0 + i as f64 * 31.0, 282.0), (28.0, 40.0), &format!("Click to colour the strip like pH {i}"), move |q| {
                    q.set_f("ph", i as f64);
                })
            })
            .collect(),
        // the screen: the bench meter's sits lower left
        _ => vec![Handle::zone(if p.s("meter") == METER_LOOKS[1] { (185.0, 195.0) } else { (280.0, 95.0) }, if p.s("meter") == METER_LOOKS[1] { (242.0, 78.0) } else { (230.0, 90.0) }, "Scroll to change the pH", |_| {}).with_scroll(|q, dy| {
            q.set_f("ph", q.f("ph") + dy * 0.05);
        })],
    }
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
fn iv(v: i64) -> Value { Value::I(v) }

pub(super) fn examples(id: &str) -> Option<Vec<Example>> {
    let m = ("mag_show", sv("Instrument and magnifier"));
    Some(match id {
        "volume-reading" => vec![
            ex("Graduated cylinder reading 34.5 mL", vec![("reading", fv(34.5)), m.clone()]),
            ex("10 mL graduated cylinder reading 7.36 mL", vec![("size", sv("10 mL")), ("reading", fv(7.36)), m.clone()]),
            ex("Meniscus close-up: 50 mL cylinder at 26.0 mL", vec![("size", sv("50 mL")), ("reading", fv(26.0)), ("mag_show", sv("Magnifier only")), ("mag_marks", iv(3))]),
            ex("1000 mL graduated cylinder reading 640 mL", vec![("size", sv("1000 mL")), ("reading", fv(640.0)), m.clone()]),
            ex("Buret reading 12.35 mL", vec![("instrument", sv("Buret")), ("reading", fv(12.35)), m.clone(), ("mag_marks", iv(3))]),
            ex("Beaker reading 160 mL", vec![("instrument", sv("Beaker")), ("size", sv("250 mL")), ("reading", fv(160.0)), ("color", sv("Red"))]),
            ex("25 cm³ graduated cylinder reading 18.25 cm³", vec![("size", sv("25 mL")), ("reading", fv(18.25)), ("unit", sv("cm³")), m.clone()]),
        ],
        "volume-by-displacement" => vec![
            ex("Marbles in a 25 mL graduated cylinder", vec![("size", sv("25 mL")), ("before", fv(12.0)), ("after", fv(15.5)), ("count", iv(3)), ("answer", bv(true))]),
            ex("Rock in a 100 mL graduated cylinder", vec![("size", sv("100 mL")), ("object", sv("Rock")), ("before", fv(50.0)), ("after", fv(63.0))]),
            ex("Metal cube in a 50 mL graduated cylinder", vec![("size", sv("50 mL")), ("object", sv("Cube")), ("before", fv(20.0)), ("after", fv(28.0)), ("mag_show", sv("Cylinders and magnifiers"))]),
            ex("Metal cylinder in a 10 mL graduated cylinder", vec![("size", sv("10 mL")), ("object", sv("Metal cylinder")), ("before", fv(5.0)), ("after", fv(7.4)), ("color", sv("Blue"))]),
            ex("Cube in a 100 cm³ cylinder with 2 cm³ marks", vec![("size", sv("100 mL")), ("object", sv("Cube")), ("before", fv(40.0)), ("after", fv(52.0)), ("marks", sv("2 mL")), ("unit", sv("cm³"))]),
        ],
        "gas-syringe" => vec![
            ex("Gas syringe collecting gas: 64 cm³", vec![("volume", fv(64.0))]),
            ex("Gas syringe plunger close-up: 37.6 cm³", vec![("show", sv("Syringe only")), ("size", sv("50 cm³")), ("volume", fv(37.6)), ("mag_show", sv("Magnifier only"))]),
            ex("Gas syringe reading in mL: 31.8 mL", vec![("show", sv("Syringe only")), ("unit", sv("mL")), ("volume", fv(31.8)), m.clone()]),
            ex("50 cm³ gas syringe reading 23.5 cm³", vec![("size", sv("50 cm³")), ("volume", fv(23.5)), m.clone(), ("answer", bv(true))]),
        ],
        "length-reading" => vec![
            ex("Metric ruler reading 7.45 cm", vec![("length", fv(7.45)), ("mag_show", sv("Ruler and magnifiers"))]),
            ex("Centimeter ruler without millimeters: 5.3 cm", vec![("marks", sv("1 cm")), ("length", fv(5.3)), ("object", sv("Cube"))]),
            ex("Ruler measuring not from zero (rock)", vec![("object", sv("Rock")), ("offset", fv(2.0)), ("length", fv(4.6)), ("dashes", bv(true))]),
            ex("Meter stick reading 62.4 cm", vec![("ruler", sv("100 cm")), ("marks", sv("1 cm")), ("length", fv(62.4))]),
            ex("Inch ruler reading 3 5/16 inches", vec![("units", sv("Imperial (in)")), ("length", fv(3.3125)), ("read_to", sv("Nearest mark")), ("answer", bv(true))]),
        ],
        "mass-reading" => vec![
            ex("Triple beam balance reading 263.47 g", vec![("mass", fv(263.47)), m.clone()]),
            ex("Triple beam balance: rock, 72.35 g", vec![("mass", fv(72.35)), ("pan", sv("Rock"))]),
            ex("Digital balance reading 12.47 g", vec![("balance", sv("Digital balance")), ("mass", fv(12.47))]),
            ex("Digital balance: metal cube, 21.6 g", vec![("balance", sv("Digital balance")), ("pan", sv("Cube")), ("mass", fv(21.6)), ("places", sv("0.1"))]),
            ex("Analytical balance reading 0.2537 g", vec![("balance", sv("Digital balance")), ("mass", fv(0.254)), ("places", sv("0.001"))]),
        ],
        "temperature-reading" => vec![
            ex("Thermometer reading 22.6 °C", vec![("temperature", fv(22.6)), m.clone(), ("mag_marks", iv(3))]),
            ex("Thermometer below zero: -4.5 °C", vec![("temperature", fv(-4.5)), m.clone(), ("mag_marks", iv(3))]),
            ex("Fahrenheit thermometer reading 98.6 °F", vec![("unit", sv("Fahrenheit (°F)")), ("temperature", fv(98.6)), m.clone()]),
            ex("Kelvin thermometer reading 295.4 K", vec![("unit", sv("Kelvin (K)")), ("temperature", fv(295.4)), ("liquid", sv("Blue"))]),
            ex("Digital thermometer reading 78.4 °C", vec![("thermometer", sv("Digital probe")), ("temperature", fv(78.4))]),
        ],
        "ph-reading" => vec![
            ex("pH meter reading 3.27", vec![("ph", fv(3.27))]),
            ex("Analog pH meter reading 8.46", vec![("instrument", sv("Analog meter")), ("ph", fv(8.46))]),
            ex("pH paper: acid, pH 2", vec![("instrument", sv("pH paper")), ("ph", fv(2.0))]),
            ex("pH paper: base, pH 11", vec![("instrument", sv("pH paper")), ("ph", fv(11.0))]),
        ],
        _ => return None,
    })
}
