//! Instrument-reading figures: volume, displacement, gas syringe, ruler, balance, thermometer, pH.
use super::*;
use crate::figures::{Params, ParamDef, Subject};

const SIZES: [&str; 6] = ["10 mL", "25 mL", "50 mL", "100 mL", "250 mL", "1000 mL"];
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

fn scale_ticks(d: &mut Doc, v: &Vessel, side_x: f64, dir: f64, label_x: f64, label_anchor: &str, unit: &str) {
    let n = (v.max / v.minor).round() as i64;
    for k in 0..=n {
        let val = k as f64 * v.minor;
        let y = v.y(val);
        let rel = val / v.major;
        let is_major = (rel - rel.round()).abs() < 1e-6;
        let relh = val / (v.major / 2.0);
        let is_mid = (relh - relh.round()).abs() < 1e-6;
        let len = if is_major { 22.0 } else if is_mid { 15.0 } else { 9.0 };
        d.line(side_x, y, side_x + dir * len, y, INK, if is_major { 1.4 } else { 0.9 });
        if is_major { tx(d, label_x, y + 4.0, 12.0, label_anchor, INK, false, &lab(val, v.major)); }
    }
    tx(d, label_x, v.ytop.min(v.ybot) - 36.0, 12.0, label_anchor, INK, true, unit);
}

fn volume_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("instrument", "Instrument", &["Graduated cylinder", "Buret", "Beaker"], "Graduated cylinder"),
        ParamDef::choice("size", "Size", &SIZES, "100 mL"),
        ParamDef::choice("color", "Liquid color", &COLORS, "Gray"),
        ParamDef::float("reading", "Reading (mL)", 0.0, 1000.0, 0.05, 43.6),
        ParamDef::bool("dotted", "Dotted line at the meniscus", false),
        ParamDef::bool("answer", "Show the reading as a caption", false),
        ParamDef::text("title", "Title (optional)", ""),
    ]
}

fn size_ml(s: &str) -> f64 { s.split_whitespace().next().and_then(|t| t.parse().ok()).unwrap_or(100.0) }

/// (major, minor) graduation spacing for a cylinder of `max` mL.
fn cyl_marks(max: f64) -> (f64, f64) {
    match max as i64 { 10 => (1.0, 0.2), 25 => (5.0, 0.5), 50 => (10.0, 1.0), 100 => (10.0, 1.0), 250 => (50.0, 2.0), _ => (100.0, 10.0) }
}

fn volume_render(p: &Params) -> String {
    let kind = p.s("instrument");
    let size = size_ml(p.s("size"));
    let (fill, edge) = liquid(p.s("color"));
    let burette = kind == "Buret";
    let beaker = kind == "Beaker";
    let max = if burette { if size <= 25.0 { 25.0 } else { 50.0 } } else { size };
    let reading = p.f("reading").clamp(0.0, max);
    let (w_img, h_img) = if burette { (360.0, 760.0) } else { (360.0, 600.0) };
    let mut d = Doc::new(w_img, h_img);
    let cx = 140.0;
    if burette {
        let (major, minor) = if max <= 25.0 { (1.0, 0.1) } else { (1.0, 0.2) };
        let v = Vessel { cx, w: 30.0, ytop: 60.0, ybot: 620.0, max, major, minor, inverted: true, level: reading, color: fill, edge, mdepth: 5.0 };
        liquid_shape(&mut d, &v, 640.0);
        // glass: tube, stopcock, tip
        let (xl, xr) = (cx - 15.0, cx + 15.0);
        d.path(&format!("M{xl},{:.1} L{xl},640 L{:.1},640 L{:.1},700 L{:.1},700 L{:.1},640 L{xr},640 L{xr},{:.1}", 36.0, cx - 4.0, cx - 2.0, cx + 2.0, cx + 4.0, 36.0), "none", GLASS_EDGE, 1.6);
        d.rect(cx - 24.0, 650.0, 48.0, 12.0, "#e8edf0", GLASS_EDGE, 1.4);
        d.rect(cx + 24.0, 653.0, 30.0, 6.0, "#d44", "#833", 1.0);
        scale_ticks(&mut d, &v, xr, -1.0, xr + 12.0, "start", "mL");
        if p.b("dotted") { dashed(&mut d, cx, v.y(reading), xr + 40.0, v.y(reading), "#555", 1.2); }
    } else {
        let (w, ytop, ybot) = if beaker { (170.0, 90.0, 470.0) } else { (70.0, 90.0, 500.0) };
        let (major, minor) = if beaker { (max / 5.0, max / 10.0) } else { cyl_marks(max) };
        let md = if beaker { 3.0 } else { 7.0 };
        let v = Vessel { cx, w, ytop, ybot, max, major, minor, inverted: false, level: reading, color: fill, edge, mdepth: md };
        let (xl, xr) = (cx - w / 2.0, cx + w / 2.0);
        liquid_shape(&mut d, &v, ybot + 6.0);
        d.rect(xl, ytop - 40.0, 0.0, 0.0, "none", "none", 0.0);
        if beaker {
            d.path(&format!("M{:.1},{:.1} L{xl:.1},{:.1} L{xl:.1},{:.1} L{xr:.1},{:.1} L{xr:.1},{:.1} L{:.1},{:.1}", xl - 8.0, ytop - 40.0, ytop - 30.0, ybot + 6.0, ybot + 6.0, ytop - 30.0, xr + 8.0, ytop - 40.0), "none", GLASS_EDGE, 1.8);
        } else {
            d.path(&format!("M{:.1},{:.1} L{xl:.1},{:.1} L{xl:.1},{:.1} L{xr:.1},{:.1} L{xr:.1},{:.1} L{:.1},{:.1}", xl - 5.0, ytop - 28.0, ytop - 24.0, ybot + 6.0, ybot + 6.0, ytop - 24.0, xr + 5.0, ytop - 28.0), "none", GLASS_EDGE, 1.8);
            d.poly(&[(xl, ybot + 6.0), (xl - 24.0, ybot + 22.0), (xr + 24.0, ybot + 22.0), (xr, ybot + 6.0)], "#eef3f6", GLASS_EDGE, 1.6);
        }
        scale_ticks(&mut d, &v, xr, -1.0, xr + 12.0, "start", "mL");
        if p.b("dotted") { dashed(&mut d, cx, v.y(reading), xr + 40.0, v.y(reading), "#555", 1.2); }
    }
    if p.b("answer") { tx(&mut d, 180.0, h_img - 14.0, 14.0, "middle", INK, true, &format!("Reading: {} mL", num(reading, 2))); }
    title_at(&mut d, p, 180.0, h_img - 38.0, 15.0);
    d.finish()
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
        ParamDef::bool("dotted", "Dotted line at the meniscus", false),
        ParamDef::text("title", "Title (optional)", ""),
        ParamDef::text("lab_before", "Left label", "Before"),
        ParamDef::text("lab_after", "Right label", "After"),
    ]
}

fn disp_render(p: &Params) -> String {
    let max = size_ml(p.s("size"));
    let (fill, edge) = liquid(p.s("color"));
    let before = p.f("before").clamp(0.0, max);
    let after = p.f("after").clamp(0.0, max).max(before);
    let (major, minor) = cyl_marks(max);
    let mut d = Doc::new(600.0, 600.0);
    let (w, ytop, ybot) = (84.0, 90.0, 500.0);
    for (idx, (cx, lvl)) in [(120.0, before), (400.0, after)].into_iter().enumerate() {
        let v = Vessel { cx, w, ytop, ybot, max, major, minor, inverted: false, level: lvl, color: fill, edge, mdepth: 7.0 };
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
                        d.circle(x, y, r, "#8fb4d6", "#2e4f6e", 1.5);
                        d.circle(x - r * 0.3, y - r * 0.3, r * 0.25, "#d9e9f6", "none", 0.0);
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
        scale_ticks(&mut d, &v, xr, -1.0, xr + 12.0, "start", "mL");
        if p.b("dotted") { dashed(&mut d, cx, v.y(lvl), xr + 40.0, v.y(lvl), "#555", 1.2); }
        tx(&mut d, cx, 560.0, 15.0, "middle", INK, true, if idx == 0 { p.s("lab_before") } else { p.s("lab_after") });
    }
    d.arrow(222.0, 300.0, 292.0, 300.0, INK, 2.0);
    title_at(&mut d, p, 300.0, 592.0, 15.0);
    d.finish()
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
        ParamDef::bool("answer", "Show the reading as a caption", false),
        ParamDef::text("title", "Title (optional)", ""),
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
    if p.b("answer") { tx(&mut d, x0 + len / 2.0, h - 12.0, 14.0, "middle", INK, true, &format!("Reading: {} {}", num(vol, 1), unit)); }
    title_at(&mut d, p, x0 + len / 2.0, h - 34.0, 15.0);
    d.finish()
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
        ParamDef::bool("answer", "Show the length as a caption", false),
        ParamDef::text("title", "Title (optional)", ""),
    ]
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
    if p.b("answer") { tx(&mut d, 350.0, 238.0, 14.0, "middle", INK, true, &format!("Length: {} {}", num(len, 2), unit)); }
    title_at(&mut d, p, 350.0, 26.0, 15.0);
    d.finish()
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
        ParamDef::text("title", "Title (optional)", ""),
    ]
}

fn mass_render(p: &Params) -> String {
    let triple = p.s("balance").starts_with("Triple");
    let mass = p.f("mass").clamp(0.0, if triple { 610.0 } else { 9999.0 });
    let pan = p.s("pan");
    let mut d = Doc::new(700.0, 400.0);
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
    }
    title_at(&mut d, p, 350.0, 26.0, 15.0);
    d.finish()
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
        ParamDef::text("title", "Title (optional)", ""),
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
    let mut d = Doc::new(360.0, 640.0);
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
    } else {
        let (ytop, ybot) = (70.0, 500.0);
        let t = t.clamp(lo, hi);
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
            if mj { tx(&mut d, cx + 40.0, y(v) + 4.0, 12.0, "start", INK, false, &format!("{}", v.round() as i64)); }
        }
        tx(&mut d, cx + 12.0, ytop - 20.0, 14.0, "start", INK, true, sym);
    }
    title_at(&mut d, p, 180.0, 26.0, 15.0);
    d.finish()
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

fn ph_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("instrument", "Instrument", &["Digital meter", "Analog meter", "pH paper"], "Digital meter"),
        ParamDef::choice("places", "Decimal places", &["0.0", "0.00"], "0.00"),
        ParamDef::float("ph", "pH", 0.0, 14.0, 0.01, 4.62),
        ParamDef::text("title", "Title (optional)", ""),
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
    title_at(&mut d, p, 280.0, 14.0, 13.0);
    d.finish()
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
fn vol_max(p: &Params) -> f64 {
    let size = size_ml(p.s("size"));
    if p.s("instrument") == "Buret" { if size <= 25.0 { 25.0 } else { 50.0 } } else { size }
}

/// y of volume `v` and its inverse, for the vessel the renderer draws.
fn vol_axis(p: &Params) -> (f64, f64, bool, f64) {
    match p.s("instrument") {
        "Buret" => (60.0, 620.0, true, 140.0),
        "Beaker" => (90.0, 470.0, false, 140.0),
        _ => (90.0, 500.0, false, 140.0),
    }
}

fn vol_handles(p: &Params) -> Vec<Handle> {
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
    let total = ruler_total(p);
    let (rx0, rw, ry) = (30.0, 640.0, 150.0);
    let ppu = rw / total;
    let len = p.f("length").clamp(0.05, total);
    let off = p.f("offset").clamp(0.0, (total - len).max(0.0));
    let ymid = ry - 18.0;
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
        _ => vec![Handle::zone((280.0, 95.0), (230.0, 90.0), "Scroll to change the pH", |_| {}).with_scroll(|q, dy| {
            q.set_f("ph", q.f("ph") + dy * 0.05);
        })],
    }
}
