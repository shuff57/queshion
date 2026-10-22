//! Instrument-reading figures: volume, displacement, gas syringe, ruler, balance, thermometer, pH.
use super::*;
use super::magnify::{self, Focus};
use crate::figures::{Params, ParamDef, Subject};

const SIZES: [&str; 6] = ["10 mL", "25 mL", "50 mL", "100 mL", "250 mL", "1000 mL"];
const MAG3: [&str; 3] = ["Instrument and magnifier", "Magnifier only", "Instrument only"];
const VOL_MARKS: [&str; 12] = ["Automatic", "0.1 mL", "0.2 mL", "0.5 mL", "1 mL", "2 mL", "5 mL", "10 mL", "20 mL", "25 mL", "50 mL", "100 mL"];
const VOL_NUMS: [&str; 12] = ["Automatic", "1 mL", "2 mL", "5 mL", "10 mL", "20 mL", "25 mL", "50 mL", "100 mL", "200 mL", "0.5 mL", "No numbers"];
const COLORS: [&str; 4] = ["Gray", "Blue", "Red", "Green"];

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "volume-reading", subject: Subject::Chemistry, title: "Volume Reading",
            blurb: "A graduated cylinder, buret, or beaker showing the volume you type.",
            params: volume_params, render: volume_render },
        FigureDef { id: "volume-by-displacement", subject: Subject::Chemistry, title: "Volume by Displacement",
            blurb: "A graduated cylinder before and after an object is dropped in.",
            params: disp_params, render: disp_render },
        FigureDef { id: "gas-syringe", subject: Subject::Chemistry, title: "Gas Syringe",
            blurb: "A gas syringe showing the volume of gas you type, alone or collecting from a flask.",
            params: syringe_params, render: syringe_render },
        FigureDef { id: "length-reading", subject: Subject::Chemistry, title: "Length Reading",
            blurb: "A ruler, caliper or micrometer with an object to measure.",
            params: length_params, render: length_render },
        FigureDef { id: "mass-reading", subject: Subject::Chemistry, title: "Mass Reading",
            blurb: "A triple beam or digital balance showing the mass you type.",
            params: mass_params, render: mass_render },
        FigureDef { id: "temperature-reading", subject: Subject::Chemistry, title: "Temperature Reading",
            blurb: "A liquid-in-glass or digital thermometer showing the temperature you type.",
            params: temp_params, render: temp_render },
        FigureDef { id: "ph-reading", subject: Subject::Chemistry, title: "pH Reading",
            blurb: "A digital or analog pH meter, or pH paper, showing the pH you type.",
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
        ParamDef::choice("size", "Size", &SIZES, "100 mL")
            .hint("Cylinder or beaker size; the buret is 50 mL, or 25 mL for sizes up to 25."),
        ParamDef::choice("color", "Liquid", &COLORS, "Gray"),
        ParamDef::float("reading", "Reading", 0.0, 1000.0, 0.05, 43.6),
        ParamDef::bool("dotted", "Dotted line at the meniscus", false)
            .hint("From its bottom across to the marks, to show where to read."),
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

/// Summaries under the volume reading's section headers, worded as the reference's are.
pub fn skin_summary(id: &str, title: &str, p: &Params) -> Option<String> {
    if id != "volume-reading" { return None; }
    let unit = if p.s("unit").is_empty() { "mL" } else { p.s("unit") };
    let (max, num, minor, numbers) = vol_scale(p);
    Some(match title {
        "Instrument" => format!("{} {} {}", num_trim(max), unit, p.s("instrument").to_lowercase()),
        "Scale" => if numbers { format!("{} {unit} marks, numbered every {} {unit}", num_trim(minor), num_trim(num)) } else { format!("{} {unit} marks, no numbers", num_trim(minor)) },
        "Reading" => format!("{} {unit}", num_trim((p.f("reading") * 1000.0).round() / 1000.0)),
        _ => return None,
    })
}

fn num_trim(v: f64) -> String { let s = format!("{v:.3}"); s.trim_end_matches('0').trim_end_matches('.').to_string() }

/// Random reading on the volume instrument's scale, to its readable decimal places.
pub fn random_reading(id: &str, p: &mut Params, seed: u64) -> bool {
    if id != "volume-reading" { return false; }
    let (max, _, minor, _) = vol_scale(p);
    let dp = read_dp(minor) as i32;
    let q = 10f64.powi(dp);
    let v = (crate::figures::unit_random(seed) * max * q).round() / q;
    p.set_f("reading", v.clamp(0.0, max));
    true
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

/// A graduated cylinder drawn the way the reference lays it out: a thick-walled tube `tw` wide,
/// the 0 mark 456 px below the tube's top margin and the top mark 400 px above it (4 px per
/// division at the opening state's scale, whatever the size), ticks running in from the left
/// wall, numbers just right of the ticks.
struct RefCyl { xl: f64, xr: f64, cx: f64, oy: f64, tw: f64, v: Vessel }

fn ref_cyl(ox: f64, tw: f64, max: f64, num_step: f64, minor: f64, level: f64, fill: &'static str, edge: &'static str) -> RefCyl {
    let oy = 16.0;
    let (xl, xr) = (ox + 28.0, ox + 28.0 + tw);
    let cx = (xl + xr) / 2.0;
    let v = Vessel { cx, w: tw, ytop: oy + 56.0, ybot: oy + 456.0, max, major: num_step, minor, inverted: false, level, color: fill, edge, mdepth: 0.12 * tw };
    RefCyl { xl, xr, cx, oy, tw, v }
}

fn ref_cyl_back(d: &mut Doc, c: &RefCyl) {
    let (xl, xr, oy, tw) = (c.xl, c.xr, c.oy, c.tw);
    d.path(&format!("M{:.2},{:.2} H{:.2} L{:.2},{:.2} H{:.2} Z", xl - 20.0, oy + 488.0, xr + 20.0, xr + 26.0, oy + 502.0, xl - 26.0), "#fff", INK, 2.0);
    d.path(&format!("M{xl:.2},{:.2} V{:.2} M{xr:.2},{:.2} V{:.2}", oy + 465.0, oy + 488.0, oy + 465.0, oy + 488.0), "none", INK, 2.0);
    let yl = c.v.y(c.v.level);
    let m = c.v.mdepth;
    d.path(&format!(
        "M{xl:.2},{:.2} C{:.2},{:.2} {:.2},{yl:.2} {:.2},{yl:.2} C{:.2},{yl:.2} {:.2},{:.2} {xr:.2},{:.2} V{:.2} Q{xr:.2},{:.2} {:.2},{:.2} H{:.2} Q{xl:.2},{:.2} {xl:.2},{:.2} Z",
        yl - m, xl + 0.04 * tw, yl - 0.4 * m, xl + 0.22 * tw, c.cx, xr - 0.22 * tw, xr - 0.04 * tw, yl - 0.4 * m, yl - m,
        oy + 465.0, oy + 472.0, xr - 7.0, oy + 472.0, xl + 7.0, oy + 472.0, oy + 465.0
    ), c.v.color, "none", 0.0);
}

fn ref_cyl_front(d: &mut Doc, c: &RefCyl, unit: &str, num_step: f64, numbers: bool) {
    let (xl, xr, oy, tw, v) = (c.xl, c.xr, c.oy, c.tw, &c.v);
    let n = (v.max / v.minor).round() as i64;
    let on = |val: f64, step: f64| { let r = val / step; (r - r.round()).abs() < 1e-6 };
    let lab_gap = (v.y(num_step) - v.y(0.0)).abs();
    let lab_every = ((15.0 / lab_gap.max(0.1)).ceil() as i64).max(1);
    // the reference draws no mark at the bottom (0) of a cylinder
    for k in 1..=n {
        let val = k as f64 * v.minor;
        let y = v.y(val);
        let is_major = on(val, num_step);
        let is_mid = !is_major && on(val, num_step / 2.0) && (num_step / 2.0) / v.minor >= 1.0 - 1e-9;
        let len = tw * if is_major { 0.46 } else if is_mid { 0.34 } else { 0.22 };
        d.line(xl, y, xl + len, y, INK, if is_major { 1.5 } else { 1.0 });
        let idx = (val / num_step).round() as i64;
        if is_major && numbers && idx != 0 && idx % lab_every == 0 {
            // dy in em and the gap past the tick end (data-xe) shrink with the type inside a magnifier
            let xe = xl + tw * 0.46;
            d.raw(&format!(r##"<text x="{:.2}" y="{y:.2}" dy="0.35em" data-xe="{xe:.2}" font-size="11" text-anchor="start" font-family="{FONT}" fill="{INK}">{}</text>"##, xe + 3.0, crate::figures::svg::esc(&lab(val, num_step))));
        }
    }
    tx(d, c.cx, oy + 34.0, 11.0, "middle", INK, false, unit);
    let yl = v.y(v.level);
    let m = v.mdepth;
    d.path(&format!("M{xl:.2},{:.2} C{:.2},{:.2} {:.2},{yl:.2} {:.2},{yl:.2} C{:.2},{yl:.2} {:.2},{:.2} {xr:.2},{:.2}",
        yl - m, xl + 0.04 * tw, yl - 0.4 * m, xl + 0.22 * tw, c.cx, xr - 0.22 * tw, xr - 0.04 * tw, yl - 0.4 * m, yl - m), "none", "#444", 1.6);
    d.path(&format!("M{xl:.2},{:.2} V{:.2} Q{xl:.2},{:.2} {:.2},{:.2} H{:.2} Q{xr:.2},{:.2} {xr:.2},{:.2} V{:.2}",
        oy + 12.0, oy + 465.0, oy + 472.0, xl + 7.0, oy + 472.0, xr - 7.0, oy + 472.0, oy + 465.0, oy + 12.0), "none", INK, 2.0);
    d.path(&format!("M{:.2},{:.2} Q{:.2},{:.2} {xl:.2},{:.2} M{:.2},{:.2} H{:.2} V{:.2} H{xr:.2}",
        xl - 6.0, oy + 6.0, xl - 2.0, oy + 12.0, oy + 16.0, xl - 6.0, oy + 6.0, xr + 3.0, oy + 12.0), "none", INK, 2.0);
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
    let (w_img, h_img) = if burette { (300.0, 740.0) } else if beaker { (330.0, 540.0) } else { (150.0, 536.0) };
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
    } else if !beaker {
        let c = ref_cyl(19.0, 58.0, max, num_step, minor, reading, fill, edge);
        ref_cyl_back(&mut d, &c);
        ref_cyl_front(&mut d, &c, unit, num_step, numbers);
        if p.b("dotted") { dashed(&mut d, c.cx, c.v.y(reading), c.xr + 40.0, c.v.y(reading), "#555", 1.2); }
        focus = Focus { x: 76.0, y: c.v.y(reading), label_px: (c.v.y(num_step) - c.v.y(0.0)).abs().max(40.0 / p.i("mag_marks").clamp(1, 6) as f64) };
    } else {
        let (w, ytop, ybot) = (170.0, 90.0, 470.0);
        let md = 3.0;
        let v = Vessel { cx, w, ytop, ybot, max, major: num_step, minor, inverted: false, level: reading, color: fill, edge, mdepth: md };
        let (xl, xr) = (cx - w / 2.0, cx + w / 2.0);
        liquid_shape(&mut d, &v, ybot + 6.0);
        d.path(&format!("M{:.1},{:.1} L{xl:.1},{:.1} L{xl:.1},{:.1} L{xr:.1},{:.1} L{xr:.1},{:.1} L{:.1},{:.1}", xl - 8.0, ytop - 40.0, ytop - 30.0, ybot + 6.0, ybot + 6.0, ytop - 30.0, xr + 8.0, ytop - 40.0), "none", GLASS_EDGE, 1.8);
        scale_ticks_n(&mut d, &v, xr, -1.0, xr + 12.0, "start", unit, num_step, numbers);
        if p.b("dotted") { dashed(&mut d, cx, v.y(reading), xr + 40.0, v.y(reading), "#555", 1.2); }
        focus = Focus { x: xr + 4.0, y: v.y(reading), label_px: (v.y(num_step) - v.y(0.0)).abs().max(80.0 / p.i("mag_marks").clamp(1, 6) as f64) };
    }
    let svg = magnify::compose_side_w(p, d.finish(), &focus, if burette || beaker { None } else { Some(505.0) });
    let foot = if p.b("answer") { vec![format!("Reading: {} {unit}", num(reading, read_dp(minor)))] } else { vec![] };
    wrap(p, svg, &foot)
}

// ---------------------------------------------------------------------------------------------
// Displacement
// ---------------------------------------------------------------------------------------------

fn disp_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("size", "Graduated cylinder size", &["10 mL", "25 mL", "50 mL", "100 mL"], "10 mL"),
        ParamDef::choice("color", "Liquid", &COLORS, "Gray"),
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
    let mut d = Doc::new(280.0, 570.0);
    let w = 36.0;
    let mut foci = Vec::new();
    for (idx, (ox, lvl)) in [(16.0, before), (172.0, after)].into_iter().enumerate() {
        let c = ref_cyl(ox, w, max, num_step, minor, lvl, fill, edge);
        let (cx, v) = (c.cx, &c.v);
        ref_cyl_back(&mut d, &c);
        if idx == 1 {
            let dv_h = (after - before) / max * (v.ybot - v.ytop);
            let area = (dv_h * w * 0.85).max(120.0);
            let base = c.oy + 472.0 - 1.0;
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
                        let (x, y) = (x0 + col as f64 * 2.0 * r, base - r - row as f64 * 2.0 * r);
                        let (body, rim, shine) = if p.s("marble_color") == "Gray" { ("#8f8f8f", "#111", "#fff") } else { ("#8fb4d6", "#2e4f6e", "#d9e9f6") };
                        d.circle(x, y, r, body, rim, 1.6);
                        d.raw(&format!(r##"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}" fill="{shine}" transform="rotate(-35 {:.2} {:.2})"/>"##, x - r * 0.36, y - r * 0.38, r * 0.26, r * 0.17, x - r * 0.36, y - r * 0.38));
                    }
                }
                other => {
                    let size = (area.sqrt() * 1.1).clamp(10.0, w - 8.0);
                    object(&mut d, other, cx, base, size, 7);
                }
            }
        }
        ref_cyl_front(&mut d, &c, unit, num_step, numbers);
        if p.b("dotted") { dashed(&mut d, cx, v.y(lvl), c.xr + 40.0, v.y(lvl), "#555", 1.2); }
        tx(&mut d, cx, 544.0, 18.0, "middle", INK, true, if idx == 0 { p.s("lab_before") } else { p.s("lab_after") });
        foci.push(Focus { x: c.xr - 2.0, y: v.y(lvl), label_px: (v.y(num_step) - v.y(0.0)).abs().max(70.0 / p.i("mag_marks").clamp(1, 6) as f64) });
    }
    if p.b("arrow") {
        d.arrow(112.0, 300.0, 160.0, 300.0, INK, 2.0);
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

/// Where the gas syringe's axis starts on the canvas (the reference's frame).
const SYR_X0: f64 = 186.0;
const SYR_CY: f64 = 422.0;

/// Shifts an SVG path written in a local frame to (`x0`, `y0`): numbers are read as x, y pairs
/// (or a lone x after H, a lone y after V).
fn rel_path(tpl: &str, x0: f64, y0: f64) -> String {
    let mut out = String::new();
    let mut cmd = ' ';
    let mut idx = 0usize;
    for tok in tpl.split_whitespace() {
        if let Ok(v) = tok.parse::<f64>() {
            let is_x = match cmd { 'H' => true, 'V' => false, _ => idx % 2 == 0 };
            out.push_str(&format!("{:.2} ", v + if is_x { x0 } else { y0 }));
            idx += 1;
        } else {
            cmd = tok.chars().next().unwrap_or(' ');
            idx = 0;
            out.push_str(tok);
            out.push(' ');
        }
    }
    out
}

/// x of the plunger head for volume `vol` (the barrel's 0 mark is 38 px in from its start).
fn syr_head(max: f64, vol: f64) -> f64 { SYR_X0 + 38.0 + vol / max * 360.0 }

/// The flask-and-syringe set-up as the reference draws it: a 1039 x 639 canvas, the barrel
/// running right from the delivery tube, the lens above.
fn syringe_full(p: &Params, max: f64, vol: f64, unit: &str) -> String {
    let mut d = Doc::new(1039.0, 639.0);
    let (x0, cy) = (SYR_X0, SYR_CY);
    let head = syr_head(max, vol);
    let rp = |tpl: &str| rel_path(tpl, x0, cy);
    let flask = rp("M -124 54 H -121 V 80 L -164 194 Q -168 200 -160 200 H -48 Q -40 200 -44 194 L -87 80 V 54 H -84");
    let tube = rp("M -104 98 V 14 Q -104 0 -90 0 H -6");
    let (fx, g) = (|x: f64| x0 + x, |x: f64, y: f64| format!("{:.2} {:.2}", x0 + x, cy + y));
    let fy = |y: f64| cy + y;
    // plunger: rod, thumb plate, tube from the head
    d.raw(&format!(r##"<path d="M{} H{} L{} H{} V{} H{} L{} H{} Z" fill="#fff"/>"##, g(0.0, -5.0), fx(24.0), g(38.0, -28.0), fx(438.0), fy(28.0), fx(38.0), g(24.0, 5.0), fx(0.0)));
    d.raw(&format!(r##"<g stroke-linecap="round" stroke-linejoin="round" stroke="{INK}"><rect fill="#e6e6e6" x="{:.2}" y="{:.2}" width="16" height="15.68" stroke-width="1.5"/><rect fill="#e6e6e6" x="{:.2}" y="{:.2}" width="8" height="44.8" rx="3" stroke-width="2"/><rect fill="#e6e6e6" x="{:.2}" y="{:.2}" width="416" height="52.5" stroke-width="1.5"/><line x1="{head:.2}" x2="{head:.2}" y1="{:.2}" y2="{:.2}" stroke-width="2.4"/></g>"##,
        head + 416.0, cy - 7.84, head + 430.0, cy - 22.4, head, cy - 26.25, cy - 25.8, cy + 25.8));
    // scale along the barrel's top edge
    let n = max as i64;
    for k in 0..=n {
        let x = x0 + 38.0 + k as f64 / max * 360.0;
        let (l, sw) = if k % 10 == 0 { (23.52, 1.5) } else if k % 5 == 0 { (16.8, 1.0) } else { (10.64, 1.0) };
        d.line(x, cy - 28.0, x, cy - 28.0 + l, INK, sw);
        if k % 10 == 0 { txy(&mut d, x, cy + 6.87, cy - 28.0 + 23.52, 11.0, "middle", INK, &format!("{k}")); }
    }
    tx(&mut d, x0 + 398.0, cy - 35.0, 11.0, "middle", INK, false, unit);
    d.raw(&format!(r##"<g stroke-linecap="round" stroke-linejoin="round"><path d="M{} H{} L{} H{} V{} H{} L{} H{}" fill="none" stroke="{INK}" stroke-width="2"/><rect fill="#fff" x="{:.2}" y="{:.2}" width="6" height="68" rx="2" stroke="{INK}" stroke-width="2"/></g>"##,
        g(0.0, -5.0), fx(24.0), g(38.0, -28.0), fx(438.0), fy(28.0), fx(38.0), g(24.0, 5.0), fx(0.0), x0 + 438.0, cy - 34.0));
    let flask_svg = format!(r##"<g stroke-linecap="round" stroke-linejoin="round"><clipPath id="qsflask"><path d="{flask} Z"/></clipPath><rect clip-path="url(#qsflask)" fill="#dcdcdc" x="{:.2}" y="{:.2}" width="128" height="42"/><line clip-path="url(#qsflask)" stroke="#444" stroke-width="1.4" x1="{:.2}" x2="{:.2}" y1="{:.2}" y2="{:.2}"/><path d="{flask}" fill="none" stroke="{INK}" stroke-width="2"/><path d="{}Z" fill="#bdbdbd" stroke="{INK}" stroke-width="2"/><path d="{tube}" fill="none" stroke="{INK}" stroke-width="9"/><path d="{tube}" fill="none" stroke="#fff" stroke-width="6"/><rect fill="#bdbdbd" x="{:.2}" y="{:.2}" width="26" height="16" rx="3" stroke="{INK}" stroke-width="2"/><rect fill="#fff" x="{:.2}" y="{:.2}" width="8" height="153" stroke="{INK}" stroke-width="2"/><rect fill="#fff" x="{:.2}" y="{:.2}" width="14" height="70" rx="2" stroke="{INK}" stroke-width="2"/><rect fill="#fff" x="{:.2}" y="{:.2}" width="160" height="12" rx="2" stroke="{INK}" stroke-width="2"/></g>"##,
        x0 - 168.0, cy + 158.0, x0 - 168.0, x0 - 40.0, cy + 158.0, cy + 158.0,
        rp("M -127 45 H -81 L -89 70 H -119 "),
        x0 - 14.0, cy - 8.0, x0 + 414.0, cy + 35.0, x0 + 411.0, cy - 35.0, x0 + 338.0, cy + 188.0);
    d.raw(&flask_svg);
    let focus = Focus { x: head, y: cy, label_px: 36.0 };
    magnify::compose_fixed(p, d.finish(), &focus, (1039.0, 639.0), (head, 166.0), 36.0, 371.0)
}

fn syringe_render(p: &Params) -> String {
    let max = if p.s("size").starts_with("50") { 50.0 } else { 100.0 };
    let unit = p.s("unit");
    let vol = p.f("volume").clamp(0.0, max);
    let full = p.s("show") != "Syringe only";
    if full {
        let svg = syringe_full(p, max, vol, unit);
        let foot = if p.b("answer") { vec![format!("Volume of gas: {} {}", num(vol, 1), unit)] } else { vec![] };
        return wrap(p, svg, &foot);
    }
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
        ParamDef::bool("dashes", "Dashed lines at its ends", false)
            .hint("Down to the ruler, for marbles and rocks, whose ends are above where they touch it."),
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

/// The length reading's geometry, as the reference lays it out: (x of the 0 mark, pixels per
/// unit, y of the ruler's top edge). The scale is 480 px along the ruler whatever its length.
const LEN_W: f64 = 668.0;
const LEN_RH: f64 = 84.0;
const LEN_PAD: f64 = 22.0;
fn length_geom(total: f64) -> (f64, f64, f64) { ((LEN_W - 480.0) / 2.0 + 0.0, 480.0 / total, 70.0) }

pub(super) fn length_render(p: &Params) -> String {
    let imperial = p.s("units").starts_with("Imp");
    let cm: f64 = p.s("ruler").split_whitespace().next().and_then(|t| t.parse().ok()).unwrap_or(15.0);
    let (total, unit) = if imperial { (match cm as i64 { 15 => 6.0, 20 => 8.0, 30 => 12.0, 50 => 20.0, _ => 40.0 }, "in") } else { (cm, "cm") };
    let want = match (imperial, p.s("marks")) {
        (false, "10 cm") => 10.0, (false, "5 cm") => 5.0, (false, "1 cm") => 1.0, (false, "0.5 cm") => 0.5, (false, _) => 0.1,
        (true, "10 cm") => 1.0, (true, "5 cm") => 0.5, (true, "1 cm") => 0.25, (true, "0.5 cm") => 0.125, (true, _) => 0.0625,
    };
    let (rx0, ppu, ry) = length_geom(total);
    let rw = total * ppu;
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
    let mut d = Doc::new(LEN_W, ry + LEN_RH + 16.0);
    d.raw(&format!(r##"<rect x="{:.2}" y="{ry:.2}" width="{:.2}" height="{LEN_RH}" rx="4" fill="#f4f4f4" stroke="{INK}" stroke-width="2"/>"##, rx0 - LEN_PAD, rw + 2.0 * LEN_PAD));
    let n = (total / u).round() as i64;
    for k in 0..=n {
        let v = k as f64 * u;
        let x = rx0 + v * ppu;
        let rl = v / lstep;
        let rh = v / (lstep / 2.0);
        let (len, sw) = if (rl - rl.round()).abs() < 1e-6 { (0.36 * LEN_RH, 1.5) } else if (rh - rh.round()).abs() < 1e-6 { (0.26 * LEN_RH, 1.0) } else { (0.17 * LEN_RH, 1.0) };
        d.line(x, ry, x, ry + len, INK, sw);
        if (rl - rl.round()).abs() < 1e-6 {
            // data-ye marks the tick end: the gap below it shrinks with the type inside a magnifier
            d.raw(&format!(r##"<text x="{x:.2}" y="{:.2}" data-ye="{:.2}" font-size="15" text-anchor="middle" font-family="{FONT}" fill="{INK}">{}</text>"##, ry + 46.0, ry + len, v.round() as i64));
        }
    }
    tx(&mut d, rx0, ry + 74.0, 12.75, "middle", INK, true, unit);
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
            let h = 60.0;
            d.raw(&format!(r##"<rect x="{x1:.2}" y="{:.2}" width="{wpx:.2}" height="{h}" rx="6" fill="#a8a8a8"/><rect x="{x1:.2}" y="{:.2}" width="{wpx:.2}" height="9.6" fill="#e2e2e2"/><rect x="{x1:.2}" y="{:.2}" width="{wpx:.2}" height="10.8" fill="#7a7a7a"/><rect x="{x1:.2}" y="{:.2}" width="{wpx:.2}" height="{h}" rx="6" fill="none" stroke="{INK}" stroke-width="1.6"/>"##, ry - h, ry - 50.4, ry - 18.0, ry - h));
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
        magnify::compose_above(p, d.finish(), &Focus { x: x2, y: ry + 14.4, label_px: lstep * ppu })
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

/// Where the triple beam balance's own frame sits on the canvas.
const MASS_OX: f64 = 16.0;
const MASS_OY: f64 = 20.0;

fn mass_render(p: &Params) -> String {
    let triple = p.s("balance").starts_with("Triple");
    let mass = p.f("mass").clamp(0.0, if triple { 610.0 } else { 9999.0 });
    let pan = p.s("pan");
    let mut d = if triple { Doc::new(1080.0, 332.0) } else { Doc::new(700.0, 400.0) };
    let focus;
    if triple {
        // laid out like the reference: the balance in the left 700 px, the lens on the right
        let (ox, oy) = (MASS_OX, MASS_OY);
        let hundreds = ((mass / 100.0).floor() * 100.0).min(500.0);
        let rest = mass - hundreds;
        let tens = ((rest / 10.0).floor() * 10.0).min(90.0);
        let ones = (rest - tens).clamp(0.0, 10.0);
        let bx = |v: f64, range: f64| ox + 248.0 + v / range * 336.0;
        d.raw(&format!(r##"<g stroke-linecap="round" stroke-linejoin="round" fill="#fff" stroke="{INK}" stroke-width="2"><rect x="{:.2}" y="{:.2}" width="676" height="34" rx="10"/><rect x="{:.2}" y="{:.2}" width="20" height="96"/><path d="M{:.2} {:.2} H{:.2} L{:.2} {:.2} Q{:.2} {:.2} {:.2} {:.2} Z"/><rect x="{:.2}" y="{:.2}" width="110" height="9"/><path d="M{:.2} {:.2} L{:.2} {:.2} H{:.2} Z"/><rect x="{:.2}" y="{:.2}" width="18" height="128"/></g>"##,
            ox + 8.0, oy + 254.0, ox + 88.0, oy + 158.0, ox + 18.0, oy + 142.0, ox + 178.0, ox + 168.0, oy + 154.0, ox + 98.0, oy + 162.0, ox + 28.0, oy + 154.0,
            ox + 108.0, oy + 164.0, ox + 227.0, oy + 246.0, ox + 237.0, oy + 254.0, ox + 217.0, ox + 218.0, oy + 118.0));
        if pan != "Nothing" { object(&mut d, pan, ox + 98.0, oy + 140.0, 62.0, 3); }
        // the three beams: (top y, range, notch step, label step, label y)
        for (yb, range, notch, lab_step, ly) in [(122.0f64, 100.0f64, 10.0f64, 10.0f64, 141.0f64), (170.0, 500.0, 100.0, 100.0, 189.0)] {
            let mut path = format!("M{:.2} {:.2}", ox + 236.0, oy + yb);
            let n = (range / notch).round() as i64;
            for k in 0..=n {
                let x = bx(k as f64 * notch, range);
                path.push_str(&format!(" H{:.2} L{:.2} {:.2} L{:.2} {:.2}", x - 4.0, x, oy + yb + 5.0, x + 4.0, oy + yb));
            }
            path.push_str(&format!(" H{:.2} V{:.2} H{:.2} Z", ox + 596.0, oy + yb + 24.0, ox + 236.0));
            d.path(&path, "#fff", INK, 2.0);
            let m = (range / lab_step).round() as i64;
            for k in 0..=m { tx(&mut d, bx(k as f64 * lab_step, range), oy + ly, 10.0, "middle", INK, false, &format!("{}", (k as f64 * lab_step).round() as i64)); }
        }
        d.rect(ox + 236.0, oy + 218.0, 360.0, 24.0, "#fff", INK, 2.0);
        for k in 0..=100 {
            let x = bx(k as f64 / 10.0, 10.0);
            let (len, sw) = if k % 10 == 0 { (10.0, 1.4) } else if k % 5 == 0 { (7.0, 1.0) } else { (4.5, 1.0) };
            d.line(x, oy + 218.0, x, oy + 218.0 + len, INK, sw);
            if k % 10 == 0 { tx(&mut d, x, oy + 239.0, 9.0, "middle", INK, false, &format!("{}", k / 10)); }
        }
        d.rect(ox + 647.0, oy + 212.0, 10.0, 42.0, "#fff", INK, 2.0);
        d.path(&format!("M{:.2} {:.2} H{:.2} L{:.2} {:.2} L{:.2} {:.2} H{:.2}", ox + 596.0, oy + 122.0, ox + 618.0, ox + 640.0, oy + 182.0, ox + 618.0, oy + 242.0, ox + 596.0), "none", INK, 2.0);
        d.rect(ox + 644.0, oy + 152.0, 16.0, 60.0, "#fff", INK, 1.6);
        for (y, x2, sw) in [(162.0, 652.0, 1.0), (172.0, 652.0, 1.0), (182.0, 658.0, 1.6), (192.0, 652.0, 1.0), (202.0, 652.0, 1.0)] { d.line(ox + 644.0, oy + y, ox + x2, oy + y, INK, sw); }
        d.line(ox + 626.0, oy + 182.0, ox + 646.0, oy + 182.0, INK, 2.0);
        // riders
        let (x1, x2, x3) = (bx(tens, 100.0), bx(hundreds, 500.0), bx(ones, 10.0));
        let rider = |d: &mut Doc, x: f64, y: f64, half: f64, h: f64, tip: f64| {
            d.path(&format!("M{:.2} {:.2} H{:.2} V{:.2} H{:.2} L{:.2} {:.2} L{:.2} {:.2} H{:.2} Z", x - half, oy + y, x + half, oy + y + h, x + 3.0f64.min(half / 2.0), x, oy + y + tip, x - 3.0f64.min(half / 2.0), oy + y + h, x - half), "#d4d4d4", INK, 1.5);
        };
        rider(&mut d, x1, 117.0, 12.0, 11.0, 15.0);
        rider(&mut d, x2, 165.0, 12.0, 11.0, 15.0);
        rider(&mut d, x3, 213.0, 9.0, 6.0, 9.5);
        focus = Focus { x: x3, y: oy + 222.0, label_px: 33.6 };
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
        let mut c = Doc::new(460.0, 150.0);
        c.raw(&format!(r#"<g transform="translate(-232 -124)">{body}</g>"#));
        return wrap(p, c.finish(), &foot);
    }
    let svg = if triple { magnify::compose_fixed(p, d.finish(), &focus, (1080.0, 332.0), (914.0, 166.0), 50.4, 120.0) } else { magnify::compose(p, d.finish(), &[focus], false) };
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
    let probe = p.s("thermometer") == "Digital probe";
    let mut d = if probe { Doc::new(300.0, 640.0) } else { Doc::new(150.0, 704.0) };
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
        let (ytop, ybot) = (64.0, 544.0);
        let t = t.clamp(lo, hi);
        let y = |v: f64| ybot - (v - lo) / (hi - lo) * (ybot - ytop);
        let (ox, oy) = (38.0, 16.0);
        let col = match p.s("liquid") { "Blue" => "#2e6fd0", "Gray" => "#7e8790", _ => "#d42a2a" };
        d.raw(&format!(r##"<path d="M{:.2} {:.2} Q{:.2} {:.2} {:.2} {:.2} H{:.2} Q{:.2} {:.2} {:.2} {:.2} V{:.2} C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} L{:.2} {:.2} C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} V{:.2} A9 9 0 0 1 {:.2} {:.2} V{:.2} C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} L{:.2} {:.2} C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} Z" fill="#f5f8fa" stroke="{INK}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"##,
            ox + 4.0, oy + 18.0, ox + 4.0, oy + 4.0, ox + 18.0, oy + 4.0, ox + 54.0, ox + 68.0, oy + 4.0, ox + 68.0, oy + 18.0, oy + 556.0,
            ox + 68.0, oy + 580.0, ox + 42.0, oy + 566.0, ox + 42.0, oy + 590.0, ox + 42.0, oy + 594.0,
            ox + 42.0, oy + 603.0, ox + 45.0, oy + 603.0, ox + 45.0, oy + 612.0, oy + 661.0, ox + 27.0, oy + 661.0, oy + 612.0,
            ox + 27.0, oy + 603.0, ox + 30.0, oy + 603.0, ox + 30.0, oy + 594.0, ox + 30.0, oy + 590.0,
            ox + 30.0, oy + 566.0, ox + 4.0, oy + 580.0, ox + 4.0, oy + 556.0));
        d.raw(&format!(r##"<rect x="{:.2}" y="{:.2}" width="6" height="568" rx="3" fill="#fff" stroke="#8a8a8a" stroke-width="1"/>"##, ox + 33.0, oy + 22.0));
        d.raw(&format!(r##"<path d="M{:.2} {:.2} H{:.2} L{:.2} {:.2} C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} V{:.2} A6 6 0 0 1 {:.2} {:.2} V{:.2} C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} Z" fill="{col}"/>"##,
            ox + 33.0, y(t), ox + 39.0, ox + 39.0, oy + 594.0, ox + 39.0, oy + 603.0, ox + 42.0, oy + 603.0, ox + 42.0, oy + 612.0, oy + 661.0, ox + 30.0, oy + 661.0, oy + 612.0,
            ox + 30.0, oy + 603.0, ox + 33.0, oy + 603.0, ox + 33.0, oy + 594.0));
        let n = ((hi - lo) / minor).round() as i64;
        for k in 0..=n {
            let v = lo + k as f64 * minor;
            let r = (v / lstep) - (v / lstep).round();
            let mj = r.abs() < 1e-6;
            let r5 = (v / (lstep / 2.0)) - (v / (lstep / 2.0)).round();
            let md = r5.abs() < 1e-6;
            let x1 = ox + if mj { 13.0 } else if md { 19.0 } else { 24.0 };
            d.line(x1, y(v), ox + 31.0, y(v), INK, if mj { 1.5 } else { 1.0 });
            if mj { tx(&mut d, ox + 43.0, y(v) + 3.85, 11.0, "start", INK, false, &format!("{}", if v < 0.0 { format!("\u{2212}{}", (-v).round() as i64) } else { format!("{}", v.round() as i64) })); }
        }
        tx(&mut d, ox + 43.0, oy + 24.0 + 4.2, 12.1, "start", INK, true, sym);
        // the lens: 3 numbered marks wide, centred on the liquid's top
        focus = Focus { x: 76.0, y: y(t), label_px: lstep / (hi - lo) * (ybot - ytop) };
    }
    let dp = if p.s("thermometer") == "Digital probe" { if p.s("places") == "0" { 0 } else { 1 } } else { 1 };
    let foot = if p.b("answer") { vec![format!("Temperature: {} {sym}", num(t, dp))] } else { vec![] };
    // the digital probe keeps the old lens over its display; the glass thermometer gets the
    // reference-sized lens that keeps its numbers whole
    let svg = if p.s("thermometer") == "Digital probe" { magnify::compose(p, d.finish(), &[focus], false) } else { magnify::compose_side_w(p, d.finish(), &focus, Some(482.0)) };
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
/// The digital meter's seven-segment readout, right-aligned so its last digit ends at x = 146.31
/// (the reference's), as polygons in the meter's own frame.
fn seven_seg(text: &str) -> String {
    // segments: top, upper-left, upper-right, middle, lower-left, lower-right, bottom
    const SEGS: [&str; 10] = ["1110111", "0010010", "1011101", "1011011", "0111010", "1101011", "1101111", "1010010", "1111111", "1111011"];
    let hex = |pts: &[(f64, f64)]| format!(r##"<polygon fill="#111" points="{}"/>"##, pts.iter().map(|p| format!("{:.2},{:.2}", p.0, p.1)).collect::<Vec<_>>().join(" "));
    let chars: Vec<char> = text.chars().collect();
    let mut x_end = 146.31;
    let mut out = String::new();
    for &ch in chars.iter().rev() {
        if ch == '.' {
            let x0 = x_end - 5.544;
            out.push_str(&format!(r##"<polygon fill="#111" points="{:.4},132.456 {:.4},132.456 {:.4},138 {:.4},138"/>"##, x0, x0 + 5.544, x0 + 5.544, x0));
            x_end = x0 - 3.03;
        } else if let Some(dg) = ch.to_digit(10) {
            let x0 = x_end - 19.35;
            let on = SEGS[dg as usize].as_bytes();
            let h = |yc: f64| hex(&[(x0 + 3.53, yc), (x0 + 6.05, yc - 2.52), (x0 + 13.31, yc - 2.52), (x0 + 15.83, yc), (x0 + 13.31, yc + 2.52), (x0 + 6.05, yc + 2.52)]);
            let v = |cx: f64, yc: f64| hex(&[(cx, yc - 6.73), (cx + 2.52, yc - 4.21), (cx + 2.52, yc + 4.21), (cx, yc + 6.73), (cx - 2.52, yc + 4.21), (cx - 2.52, yc - 4.21)]);
            if on[0] == b'1' { out.push_str(&h(104.52)); }
            if on[1] == b'1' { out.push_str(&v(x0 + 2.52, 112.26)); }
            if on[2] == b'1' { out.push_str(&v(x0 + 16.83, 112.26)); }
            if on[3] == b'1' { out.push_str(&h(120.0)); }
            if on[4] == b'1' { out.push_str(&v(x0 + 2.52, 127.74)); }
            if on[5] == b'1' { out.push_str(&v(x0 + 16.83, 127.74)); }
            if on[6] == b'1' { out.push_str(&h(135.48)); }
            x_end = x0 - 3.69;
        }
    }
    out
}

/// The "probe in a beaker" look, laid out like the reference: a 432 x 332 canvas, the meter on
/// the left, the probe standing in a beaker on the right.
fn ph_meter_beaker(d: &mut Doc, ph: f64, dp: usize) {
    let shown = num(ph, dp);
    let mut ticks = String::new();
    for k in 0..4 { let y = 182.66 + k as f64 * 28.4; ticks.push_str(&format!(r##"<line stroke="{INK}" stroke-width="1.2" x1="330" x2="346" y1="{y:.2}" y2="{y:.2}"/>"##)); }
    d.raw(&format!(r##"<g transform="translate(16 16)"><title>A digital pH meter reading {shown}</title><g stroke-linecap="round" stroke-linejoin="round"><path d="M172 58 C172 -8 300 -8 300 26" fill="none" stroke="{INK}" stroke-width="5"/><path d="M172 58 C172 -8 300 -8 300 26" fill="none" stroke="#fff" stroke-width="2"/><rect fill="#fff" height="142" width="104" x="248" y="150"/><path d="M248 194 H352 V288 Q352 292 348 292 H252 Q248 292 248 288 Z" fill="#dcdcdc"/><line stroke="#444" stroke-width="1.5" x1="248" x2="352" y1="194" y2="194"/>{ticks}<path d="M296 22 H304 L308 34 H292 Z" fill="#fff" stroke="{INK}" stroke-width="1.5"/><rect fill="#555" height="40" rx="4" stroke="{INK}" stroke-width="2" width="22" x="289" y="34"/><path d="M293 74 V268 Q293 270 295 270 H305 Q307 270 307 268 V74" fill="#f4f6f7" stroke="{INK}" stroke-width="1.5"/><line stroke="#9a9a9a" stroke-width="1.5" x1="300" x2="300" y1="80" y2="264"/><path d="M240 146 Q246 148 248 154 V288 Q248 292 252 292 H348 Q352 292 352 288 V148" fill="none" stroke="{INK}" stroke-width="2"/></g><g stroke-linecap="round" stroke-linejoin="round"><rect fill="#fff" height="12" rx="3" stroke="{INK}" stroke-width="2" width="24" x="160" y="56"/><rect fill="#fff" height="228" rx="18" stroke="{INK}" stroke-width="2" width="200" x="10" y="64"/><rect fill="#eef0ec" height="64" rx="5" stroke="{INK}" stroke-width="2" width="168" x="26" y="88"/>{seg}<text fill="{INK}" font-size="18" font-weight="700" x="156" y="137">pH</text><rect fill="#fff" height="28" rx="6" stroke="{INK}" stroke-width="1.5" width="60" x="36" y="196"/><text fill="{INK}" font-size="10" font-weight="700" text-anchor="middle" x="66" y="214">ON/OFF</text><rect fill="#fff" height="28" rx="6" stroke="{INK}" stroke-width="1.5" width="60" x="124" y="196"/><text fill="{INK}" font-size="10" font-weight="700" text-anchor="middle" x="154" y="214">CAL</text><path d="M70 256 H150 M70 266 H150" stroke="#bbb" stroke-width="2"/></g></g>"##, seg = seven_seg(&shown)));
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
    let beaker_look = !matches!(p.s("instrument"), "Analog meter" | "pH paper") && p.s("meter") == METER_LOOKS[1];
    let mut d = if beaker_look { Doc::new(432.0, 332.0) } else { Doc::new(560.0, 440.0) };
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
        _ => (72.0, 472.0, false, 76.0),
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
    let (ytop, ybot) = (72.0, 472.0);
    let before = p.f("before").clamp(0.0, max);
    let after = p.f("after").clamp(0.0, max).max(before);
    let ly = move |v: f64| ybot - v / max * (ybot - ytop);
    vec![
        Handle::bar((62.0, ly(before)), true, "Drag the 'before' liquid level", move |q, t| {
            let max = size_ml(q.s("size"));
            set_eff(q, "before", (ybot - t.1) / (ybot - ytop) * max, 0.0, max);
        }),
        Handle::bar((218.0, ly(after)), true, "Drag the 'after' liquid level", move |q, t| {
            let max = size_ml(q.s("size"));
            let b = q.f("before").clamp(0.0, max);
            set_eff(q, "after", (ybot - t.1) / (ybot - ytop) * max, b, max);
        }),
        Handle::label((62.0, 544.0), (110.0, 26.0), "lab_before", "Double-click to rename"),
        Handle::label((218.0, 544.0), (110.0, 26.0), "lab_after", "Double-click to rename"),
    ]
}

fn syringe_geom(p: &Params) -> (f64, f64, f64, f64) {
    let max = if p.s("size").starts_with("50") { 50.0 } else { 100.0 };
    let full = p.s("show") != "Syringe only";
    if full { return (max, SYR_X0 + 38.0, 360.0, SYR_CY); }
    (max, 70.0, 350.0, 120.0)
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
    let (rx0, ppu, ry) = length_geom(total);
    let len = p.f("length").clamp(0.05, total);
    let off = p.f("offset").clamp(0.0, (total - len).max(0.0));
    let ymid = ry - 18.0 + if length_one_above(p) { magnify::above_shift() } else { 0.0 };
    let (x1, x2) = (rx0 + off * ppu, rx0 + (off + len) * ppu);
    vec![
        Handle::knob((x1, ymid), "Left end: drag to move it (the right end stays put)", move |q, t| {
            let total = ruler_total(q);
            let (rx0, ppu, _) = length_geom(total);
            let ln = q.f("length").clamp(0.05, total);
            let of = q.f("offset").clamp(0.0, (total - ln).max(0.0));
            let right = of + ln;
            let new_off = ((t.0 - rx0) / ppu).clamp(0.0, right - 0.05);
            set_eff(q, "offset", new_off, 0.0, total);
            set_eff(q, "length", right - new_off, 0.05, total);
        })
        .with_cursor(Cursor::ResizeH),
        Handle::knob((x2, ymid), "Right end: drag to change the length", move |q, t| {
            let total = ruler_total(q);
            let (rx0, ppu, _) = length_geom(total);
            let ln = q.f("length").clamp(0.05, total);
            let of = q.f("offset").clamp(0.0, (total - ln).max(0.0));
            set_eff(q, "length", ((t.0 - rx0) / ppu - of).clamp(0.05, total - of), 0.05, total);
        })
        .with_cursor(Cursor::ResizeH),
        Handle::square(((x1 + x2) / 2.0, ymid - 14.0), "Drag to slide the object along the ruler", move |q, t| {
            let total = ruler_total(q);
            let (rx0, ppu, _) = length_geom(total);
            let ln = q.f("length").clamp(0.05, total);
            let of = q.f("offset").clamp(0.0, (total - ln).max(0.0));
            let _ = of;
            set_eff(q, "offset", (t.0 - rx0) / ppu - ln / 2.0, 0.0, (total - ln).max(0.0));
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
    let x0 = MASS_OX + 248.0;
    let rx = move |val: f64, range: f64| x0 + val / range * 336.0;
    // (beam rider y, range, step, current)
    let beams: [(f64, f64, f64, f64, &str); 3] = [
        (MASS_OY + 124.0, 100.0, 10.0, tens, "10 g rider: drag along its beam"),
        (MASS_OY + 172.0, 500.0, 100.0, hundreds, "100 g rider: drag along its beam"),
        (MASS_OY + 218.0, 10.0, 0.0, ones, "1 g rider: drag along its beam"),
    ];
    beams
        .iter()
        .enumerate()
        .map(|(i, &(y, range, step, cur, tip))| {
            Handle::knob((rx(cur, range), y), tip, move |q, t| {
                let m = q.f("mass").clamp(0.0, 610.0);
                let h = ((m / 100.0).floor() * 100.0).min(500.0);
                let r = m - h;
                let te = ((r / 10.0).floor() * 10.0).min(90.0);
                let on = (r - te).clamp(0.0, 10.0);
                let mut v = (t.0 - x0) / 336.0 * range;
                if step > 0.0 { v = (v / step).round() * step; }
                v = v.clamp(0.0, range);
                // beam order on screen: tens, hundreds, ones
                let (nh, nt, no) = match i { 0 => (h, v, on), 1 => (v, te, on), _ => (h, te, v) };
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
    let (ytop, ybot) = (64.0, 544.0);
    let t = p.f("temperature").clamp(lo, hi);
    let y = ybot - (t - lo) / (hi - lo) * (ybot - ytop);
    vec![Handle::bar((74.0, y), true, "Drag the top of the liquid to set the temperature", move |q, tg| {
        let (lo, hi) = temp_scale(q);
        set_eff(q, "temperature", lo + (544.0 - tg.1) / 480.0 * (hi - lo), lo, hi);
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
        _ => vec![Handle::zone(if p.s("meter") == METER_LOOKS[1] { (126.0, 136.0) } else { (280.0, 95.0) }, if p.s("meter") == METER_LOOKS[1] { (168.0, 64.0) } else { (230.0, 90.0) }, "Scroll to change the pH", |_| {}).with_scroll(|q, dy| {
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
