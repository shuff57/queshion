//! Instrument-style figures: spring scale and circuit diagram.
use super::common::*;
use crate::figures::{FigureDef, ParamDef, Params, Subject};

// ---------------------------------------------------------------- spring scale

const CAPS: [&str; 7] = ["1 N", "2.5 N", "5 N", "10 N", "20 N", "30 N", "50 N"];

fn cap_value(s: &str) -> f64 { s.trim_end_matches(" N").parse().unwrap_or(10.0) }

fn cap_color(c: f64) -> &'static str {
    match c as i64 * 10 {
        10 => "#f4a6c8",
        25 => "#86b4ec",
        50 => "#6cc18a",
        100 => "#c99a6a",
        200 => "#e46a6a",
        300 => "#ffffff",
        _ => "#f2c94c",
    }
}

fn spring_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("cap", "Capacity", &CAPS, "10 N"),
        ParamDef::choice("units", "Printed in", &["Newtons", "Grams", "Both"], "Newtons"),
        ParamDef::bool("coded", "Color-coded by capacity", true),
        ParamDef::float("reading", "Force on the hook (N)", 0.0, 50.0, 0.01, 3.47),
        ParamDef::float("offset", "Zero offset (fraction of capacity, -0.1 to 0.1)", -0.1, 0.1, 0.005, 0.0),
        ParamDef::choice("hook", "On the hook", &["Block", "Nothing", "Slotted masses"], "Block"),
        ParamDef::text("block_lab", "Label on the block", ""),
        ParamDef::choice("view", "Show", &["Instrument and magnifier", "Magnifier only", "Instrument only"], "Instrument and magnifier"),
        ParamDef::text("title", "Chart title", ""),
        ParamDef::bool("answer", "Print the answer key under the figure", false),
    ]
}

struct Inst { ix: f64, y0: f64, y1: f64, cap: f64, units: String, value: f64, coded: bool }

fn lab_step(cap: f64) -> f64 { if cap <= 1.0 { 0.1 } else if cap <= 2.5 { 0.25 } else if cap <= 5.0 { 0.5 } else { cap / 10.0 } }

/// Draw the scale; `k` is the stroke multiplier and `fs` the number size, so the same routine can
/// be drawn magnified. `full` adds the body, header and hook.
fn draw_inst(g: &mut G, c: &Inst, sw: f64, fs: f64, full: bool) {
    let ix = c.ix;
    let body = "#f6efe6";
    let ypos = |v: f64| c.y0 + (c.y1 - c.y0) * v / c.cap;
    if full {
        g.circle(ix, 46.0, 7.0, "#fff", INK, 1.6);
        g.raw(&format!(r##"<rect x="{:.1}" y="76" width="124" height="378" rx="10" fill="{body}" stroke="{INK}" stroke-width="1.6"/>"##, ix - 62.0));
        let head = if c.coded { cap_color(c.cap) } else { "#d9d9d9" };
        g.raw(&format!(r##"<rect x="{:.1}" y="53" width="124" height="24" rx="5" fill="{head}" stroke="{INK}" stroke-width="1.6"/>"##, ix - 62.0));
        g.text(ix, 70.0, 12.0, "middle", &format!("{} N", fmt_num(c.cap)), true);
    }
    // tube with spring
    let top = c.y0 - 24.0;
    let bot = c.y1 + 16.0;
    g.rect(ix - 10.0, top, 20.0, bot - top, "#ffffff", INK, sw * 1.2);
    let py = ypos(c.value.clamp(-0.08 * c.cap, 1.06 * c.cap));
    let mut zz = vec![(ix, top)];
    let n = (((py - top) / 11.0).floor() as usize).max(1);
    for i in 0..n {
        let y = top + (py - 4.0 - top) * (i as f64 + 0.5) / n as f64;
        zz.push((ix + if i % 2 == 0 { 6.0 } else { -6.0 }, y));
    }
    zz.push((ix, py - 4.0));
    g.polyline(&zz, "#555", sw * 1.1);
    g.rect(ix - 7.0, py - 4.0, 14.0, 8.0, "#c99a6a", INK, sw);
    // ticks and numbers
    let both = c.units == "Both";
    let sides: Vec<(f64, bool)> = match c.units.as_str() {
        "Grams" => vec![(-1.0, false)],
        "Both" => vec![(-1.0, true), (1.0, false)],
        _ => vec![(-1.0, true)],
    };
    let _ = both;
    for (side, newtons) in sides {
        let (cap_u, mult) = if newtons { (c.cap, 1.0) } else { (c.cap * 100.0, 100.0) };
        let ls = lab_step(c.cap) * mult;
        let minor = ls / 5.0;
        let steps = (cap_u / minor).round() as i64;
        for i in 0..=steps {
            let u = i as f64 * minor;
            let v = u / mult;
            let y = ypos(v);
            let major = i % 5 == 0;
            let mid = false;
            let len = if major { 14.0 } else if mid { 10.0 } else { 6.0 };
            let x0 = ix + side * 10.0;
            g.line(x0, y, x0 + side * len, y, INK, if major { sw * 1.4 } else { sw });
            if major {
                g.text(x0 + side * (len + 3.0), y + fs * 0.35, fs, if side < 0.0 { "end" } else { "start" }, &fmt_num(u), false);
            }
        }
        let lab = if newtons { "N" } else { "g" };
        g.text(ix + side * 36.0, c.y0 - 28.0, fs + 2.0, "middle", lab, true);
    }
    // pointer line across the scale
    g.line(ix - 34.0, py, ix + 34.0, py, "#c0392b", sw * 1.1);
    if full {
        // hook below the body
        g.polyline(&[(ix, 454.0), (ix, 470.0)], INK, 2.0);
        g.polyline(&arc_pts(ix, 480.0, 10.0, 90.0, -160.0), INK, 2.0);
    }
}

fn spring_render(p: &Params) -> String {
    let (w, h) = (580.0, 600.0);
    let mut g = G::new(w, h, false);
    let cap = cap_value(p.s("cap"));
    let reading = p.f("reading").min(cap);
    let value = reading + p.f("offset") * cap;
    let view = p.s("view").to_string();
    let show_inst = view != "Magnifier only";
    let show_mag = view != "Instrument only";
    let (ix, mcx) = if show_inst && show_mag { (130.0, 410.0) } else { (w / 2.0, w / 2.0) };
    let cfg = Inst { ix, y0: 130.0, y1: 424.0, cap, units: p.s("units").to_string(), value, coded: p.b("coded") };
    if !p.s("title").trim().is_empty() { g.text(w / 2.0, 26.0, 19.0, "middle", p.s("title"), true); }
    let py = cfg.y0 + (cfg.y1 - cfg.y0) * value.clamp(-0.08 * cap, 1.06 * cap) / cap;
    if show_inst {
        draw_inst(&mut g, &cfg, 1.0, 8.5, true);
        // thing on the hook
        match p.s("hook") {
            "Block" => {
                g.polyline(&[(ix, 490.0), (ix, 500.0)], INK, 2.0);
                g.rect(ix - 24.0, 500.0, 48.0, 42.0, "#e6e8ec", INK, 1.8);
                if !p.s("block_lab").trim().is_empty() { g.tex(ix, 527.0, 17.0, "middle", p.s("block_lab"), ITALIC); }
            }
            "Slotted masses" => {
                g.polyline(&[(ix, 490.0), (ix, 498.0)], INK, 2.0);
                g.line(ix - 22.0, 498.0, ix + 22.0, 498.0, INK, 2.0);
                for i in 0..3 {
                    let y = 502.0 + i as f64 * 15.0;
                    g.rect(ix - 30.0 + i as f64 * 0.0, y, 60.0, 12.0, "#cfd3da", INK, 1.5);
                    g.line(ix, y, ix, y + 6.0, INK, 1.2);
                }
            }
            _ => {}
        }
    }
    if show_mag {
        let (mcy, r, z) = (270.0, 128.0, 3.2);
        g.raw(r##"<defs><clipPath id="mag">"##);
        g.raw(&format!(r#"<circle cx="{mcx:.1}" cy="{mcy:.1}" r="{r:.1}"/></clipPath></defs>"#));
        g.raw(&format!(r##"<circle cx="{mcx:.1}" cy="{mcy:.1}" r="{r:.1}" fill="#f6efe6"/>"##));
        g.raw(&format!(r#"<g clip-path="url(#mag)"><g transform="translate({mcx:.2},{mcy:.2}) scale({z}) translate({:.2},{:.2})">"#, -cfg.ix, -py));
        draw_inst(&mut g, &cfg, 0.4, 3.8, false);
        g.raw("</g></g>");
        g.circle(mcx, mcy, r, "none", INK, 3.0);
        if show_inst {
            // connectors from the small circle to the magnifier
            let sr = 22.0;
            g.circle(cfg.ix, py, sr, "none", INK, 1.2);
            let a1 = pol(mcx, mcy, r, 115.0);
            let a2 = pol(mcx, mcy, r, 245.0);
            g.dash(cfg.ix + sr * 0.35, py - sr * 0.93, a1.0, a1.1, "#444", 1.0);
            g.dash(cfg.ix + sr * 0.35, py + sr * 0.93, a2.0, a2.1, "#444", 1.0);
        }
    }
    if p.b("answer") {
        g.text(w / 2.0, h - 14.0, 18.0, "middle", &format!("Force: {:.2} N", reading), true);
    }
    g.finish()
}

// ---------------------------------------------------------------- circuit diagram

const RK: [[&str; 6]; 4] = [
    ["r1_kind", "Component 1", "r1_name", "Component 1 name (blank = automatic)", "r1_val", "Component 1 value"],
    ["r2_kind", "Component 2", "r2_name", "Component 2 name (blank = automatic)", "r2_val", "Component 2 value"],
    ["r3_kind", "Component 3", "r3_name", "Component 3 name (blank = automatic)", "r3_val", "Component 3 value"],
    ["r4_kind", "Component 4", "r4_name", "Component 4 name (blank = automatic)", "r4_val", "Component 4 value"],
];
const RVAL: [&str; 4] = ["2 Ω", "4 Ω", "6 Ω", "8 Ω"];

fn circuit_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("layout", "Resistors and bulbs", &["In series", "In parallel"], "In series"),
        ParamDef::int("n", "Number of components", 1, 4, 3),
    ];
    for (i, k) in RK.iter().enumerate() {
        v.push(ParamDef::choice(k[0], k[1], &["Resistor", "Bulb"], "Resistor"));
        v.push(ParamDef::text(k[2], k[3], ""));
        v.push(ParamDef::text(k[4], k[5], RVAL[i]));
    }
    v.push(ParamDef::choice("cell", "Cell or battery", &["One cell", "Battery of two cells", "Battery of three cells"], "Battery of two cells"));
    v.push(ParamDef::text("cell_name", "Battery name", ""));
    v.push(ParamDef::text("cell_val", "Battery value", "12 V"));
    v.push(ParamDef::bool("polarity", "+ and - beside it", false));
    v.push(ParamDef::choice("switch", "Switch", &["None", "Closed", "Open"], "None"));
    v.push(ParamDef::bool("ammeter", "Ammeter in the main line", false));
    v.push(ParamDef::choice("voltmeter", "Voltmeter", &["None", "Across the battery", "Across component 1", "Across component 2", "Across component 3", "Across component 4"], "None"));
    v.push(ParamDef::choice("style", "Symbol style", &["US (zigzag resistor)", "IEC (box resistor)"], "US (zigzag resistor)"));
    v.push(ParamDef::text("title", "Chart title", ""));
    v.push(ParamDef::bool("mirror", "Mirror (flip left to right)", false));
    v.push(ParamDef::bool("color", "Color (for slides)", false));
    v
}

/// Resistor or bulb centred at (cx,cy), `len` long along its axis, horizontal or vertical.
fn component(g: &mut G, bulb: bool, iec: bool, cx: f64, cy: f64, len: f64, vert: bool) {
    let m = |lx: f64, ly: f64| if vert { (cx + ly, cy + lx) } else { (cx + lx, cy + ly) };
    let h = len / 2.0;
    if bulb {
        let r = 16.0;
        let (a, b) = (m(-h, 0.0), m(-r, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        let (a, b) = (m(r, 0.0), m(h, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        g.circle(cx, cy, r, "#fff", INK, SW);
        let d = r * 0.7071;
        for (p1, p2) in [((-d, -d), (d, d)), ((-d, d), (d, -d))] {
            let (a, b) = (m(p1.0, p1.1), m(p2.0, p2.1));
            g.line(a.0, a.1, b.0, b.1, INK, 1.8);
        }
    } else if iec {
        let (a, b) = (m(-h, 0.0), m(-h + 10.0, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        let (a, b) = (m(h - 10.0, 0.0), m(h, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        let pts = [m(-h + 10.0, -9.0), m(h - 10.0, -9.0), m(h - 10.0, 9.0), m(-h + 10.0, 9.0)];
        g.poly(&pts, "#fff", INK, SW);
    } else {
        let lead = 8.0;
        let zig = 6usize;
        let seg = (len - 2.0 * lead) / zig as f64;
        let mut pts = vec![m(-h, 0.0), m(-h + lead, 0.0)];
        for i in 0..zig {
            let x = -h + lead + seg * (i as f64 + 0.5);
            let amp = if i % 2 == 0 { -9.0 } else { 9.0 };
            pts.push(m(x, amp));
        }
        pts.push(m(h - lead, 0.0));
        pts.push(m(h, 0.0));
        g.polyline(&pts, INK, SW);
    }
}

fn meter(g: &mut G, cx: f64, cy: f64, letter: &str) {
    g.circle(cx, cy, 17.0, "#fff", INK, SW);
    g.text(cx, cy + 6.0, 17.0, "middle", letter, true);
}

fn circuit_render(p: &Params) -> String {
    let (w, h) = (580.0, 360.0);
    let mut g = G::new(w, h, p.b("mirror"));
    let col = p.b("color");
    let wire = if col { "#1f3a8a" } else { INK };
    let parallel = p.s("layout") == "In parallel";
    let n = p.i("n").clamp(1, 4) as usize;
    let iec = p.s("style").starts_with("IEC");
    let top_y = if parallel { 90.0 } else { 110.0 };
    let bot_y = 280.0;
    let lx = 110.0;
    let rx = if parallel { 215.0 + 85.0 * (n as f64 - 1.0) + 50.0 } else { 480.0 };
    let (mut top_gaps, mut bot_gaps): (Vec<(f64, f64)>, Vec<(f64, f64)>) = (vec![], vec![]);
    if !p.s("title").trim().is_empty() { g.text(w / 2.0, 28.0, 19.0, "middle", p.s("title"), true); }

    let name_of = |i: usize| -> String {
        let nm = p.s(RK[i][2]).trim().to_string();
        if nm.is_empty() { format!("R_{}", i + 1) } else { nm }
    };
    // components
    let mut term: Vec<(f64, f64, f64, f64)> = vec![]; // terminal positions per component
    for i in 0..n {
        let bulb = p.s(RK[i][0]) == "Bulb";
        let len: f64 = if bulb { 40.0 } else if n >= 4 && !parallel { 56.0 } else { 64.0 };
        let c = if col { PAL[i % 6] } else { INK };
        let _ = c;
        if parallel {
            let cx = 215.0 + 85.0 * i as f64;
            let cy = (top_y + bot_y) / 2.0;
            let len = len.max(60.0);
            component(&mut g, bulb, iec, cx, cy, len, true);
            g.line(cx, top_y, cx, cy - len / 2.0, wire, SW);
            g.line(cx, cy + len / 2.0, cx, bot_y, wire, SW);
            g.tex(cx + 24.0, cy - 2.0, 20.0, "start", &name_of(i), ITALIC);
            g.tex(cx + 24.0, cy + 22.0, 19.0, "start", p.s(RK[i][4]), UPRIGHT);
            if i > 0 { g.circle(cx, top_y, 3.2, INK, INK, 1.0); g.circle(cx, bot_y, 3.2, INK, INK, 1.0); }
            term.push((cx, cy - len / 2.0, cx, cy + len / 2.0));
        } else {
            let span = rx - 30.0 - (lx + 80.0);
            let cx = lx + 80.0 + span * (i as f64 + 0.5) / n as f64;
            component(&mut g, bulb, iec, cx, top_y, len, false);
            top_gaps.push((cx - len / 2.0, cx + len / 2.0));
            g.tex(cx, top_y - 30.0, 21.0, "middle", &name_of(i), ITALIC);
            g.tex(cx, top_y + 40.0, 19.0, "middle", p.s(RK[i][4]), UPRIGHT);
            term.push((cx - len / 2.0, top_y, cx + len / 2.0, top_y));
        }
    }
    // switch and ammeter on the bottom wire
    let sw_mid = if parallel { 150.0 } else { 270.0 };
    if p.s("switch") != "None" {
        let (a, b) = (sw_mid - 30.0, sw_mid + 30.0);
        bot_gaps.push((a, b));
        g.circle(a, bot_y, 3.5, "#fff", INK, 2.0);
        g.circle(b, bot_y, 3.5, "#fff", INK, 2.0);
        if p.s("switch") == "Closed" { g.line(a, bot_y, b, bot_y, INK, SW); } else { g.line(a, bot_y, b - 6.0, bot_y - 26.0, INK, SW); }
    }
    if p.b("ammeter") {
        let ax = if parallel { 150.0 } else { 390.0 };
        let ay = if parallel { top_y } else { bot_y };
        if parallel { top_gaps.push((ax - 17.0, ax + 17.0)); } else { bot_gaps.push((ax - 17.0, ax + 17.0)); }
        meter(&mut g, ax, ay, "A");
    }
    // battery on the left wire
    let cells = match p.s("cell") { "One cell" => 1, "Battery of three cells" => 3, _ => 2 };
    let plates = cells * 2;
    let by = (top_y + bot_y) / 2.0;
    let pitch = 12.0;
    let first = by - pitch * (plates as f64 - 1.0) / 2.0;
    g.line(lx, top_y, lx, first, wire, SW);
    g.line(lx, first + pitch * (plates as f64 - 1.0), lx, bot_y, wire, SW);
    for k in 0..plates {
        let y = first + pitch * k as f64;
        let long = k % 2 == 0;
        let half = if long { 24.0 } else { 12.0 };
        g.line(lx - half, y, lx + half, y, INK, if long { 2.6 } else { 5.0 });
    }
    let (cname, cval) = (p.s("cell_name").trim(), p.s("cell_val"));
    let bx = lx + 36.0;
    if !cname.is_empty() { g.tex(bx, by - 4.0, 20.0, "start", cname, ITALIC); }
    g.tex(bx, by + if cname.is_empty() { 7.0 } else { 20.0 }, 21.0, "start", cval, UPRIGHT);
    if p.b("polarity") {
        g.text(lx + 30.0, first - 4.0, 18.0, "start", "+", true);
        g.text(lx + 30.0, first + pitch * (plates as f64 - 1.0) + 20.0, 18.0, "start", "−", true);
    }
    // wires
    let seg_line = |g: &mut G, y: f64, x0: f64, x1: f64, gaps: &mut Vec<(f64, f64)>| {
        gaps.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut x = x0;
        for &(a, b) in gaps.iter() { if a > x { g.line(x, y, a, y, wire, SW); } x = x.max(b); }
        if x < x1 { g.line(x, y, x1, y, wire, SW); }
    };
    seg_line(&mut g, top_y, lx, rx, &mut top_gaps);
    seg_line(&mut g, bot_y, lx, rx, &mut bot_gaps);
    if !parallel { g.line(rx, top_y, rx, bot_y, wire, SW); }
    // voltmeter
    let vm = p.s("voltmeter").to_string();
    if vm != "None" {
        if vm == "Across the battery" {
            let vx = 44.0;
            let (ya, yb) = (first - 6.0, first + pitch * (plates as f64 - 1.0) + 6.0);
            let (ya, yb) = (ya.min(by - 30.0), yb.max(by + 30.0));
            g.line(lx, top_y + 22.0, vx, top_y + 22.0, wire, SW);
            g.line(vx, top_y + 22.0, vx, by - 17.0, wire, SW);
            meter(&mut g, vx, by, "V");
            g.line(vx, by + 17.0, vx, bot_y - 22.0, wire, SW);
            g.line(vx, bot_y - 22.0, lx, bot_y - 22.0, wire, SW);
            g.circle(lx, top_y + 22.0, 3.2, INK, INK, 1.0);
            g.circle(lx, bot_y - 22.0, 3.2, INK, INK, 1.0);
            let _ = (ya, yb);
        } else if parallel {
            let vx = rx + 40.0;
            let cy = (top_y + bot_y) / 2.0;
            g.line(rx - 30.0, top_y, vx, top_y, wire, SW);
            g.line(vx, top_y, vx, cy - 17.0, wire, SW);
            meter(&mut g, vx, cy, "V");
            g.line(vx, cy + 17.0, vx, bot_y, wire, SW);
            g.line(vx, bot_y, rx - 30.0, bot_y, wire, SW);
            g.circle(rx - 30.0, top_y, 3.2, INK, INK, 1.0);
            g.circle(rx - 30.0, bot_y, 3.2, INK, INK, 1.0);
        } else {
            let k = vm.chars().last().and_then(|c| c.to_digit(10)).unwrap_or(1) as usize;
            let k = k.clamp(1, n) - 1;
            let (a, b, _, _) = term[k];
            let t = term[k];
            let (xa, xb) = (t.0, t.2);
            let _ = (a, b);
            let vy = top_y + 85.0;
            let mid = (xa + xb) / 2.0;
            g.line(xa, top_y, xa, vy, wire, SW);
            g.line(xb, top_y, xb, vy, wire, SW);
            g.line(xa, vy, mid - 17.0, vy, wire, SW);
            g.line(mid + 17.0, vy, xb, vy, wire, SW);
            meter(&mut g, mid, vy, "V");
            g.circle(xa, top_y, 3.2, INK, INK, 1.0);
            g.circle(xb, top_y, 3.2, INK, INK, 1.0);
        }
    }
    g.finish()
}

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "spring-scale", subject: Subject::Physics, title: "Spring Scale",
            blurb: "A colour-coded spring scale reading any force, with an optional magnifier, zero offset and hanging object.",
            params: spring_params, render: spring_render },
        FigureDef { id: "circuit-diagram", subject: Subject::Physics, title: "Circuit Diagram",
            blurb: "Series or parallel circuits of resistors and bulbs with a battery, switch, ammeter and voltmeter in US or IEC symbols.",
            params: circuit_params, render: circuit_render },
    ]
}
