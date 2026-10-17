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
        ParamDef::bool("show_zero", "Show the unloaded scale beside it (zero error)", false),
        ParamDef::choice("hook", "On the hook", &["Block", "Nothing", "Slotted masses"], "Block"),
        ParamDef::int("masses", "Number of slotted masses", 1, 8, 3),
        ParamDef::text("block_lab", "Label on the block", ""),
        ParamDef::choice("view", "Show", &["Instrument and magnifier", "Magnifier only", "Instrument only"], "Instrument and magnifier"),
        ParamDef::choice("mag_marks", "Numbered marks in the magnifier", &["1", "2", "3", "4", "5", "6"], "3"),
        ParamDef::text("title", "Chart title", ""),
        ParamDef::bool("answer", "Print the answer key under the figure", false),
    ]
}

struct Inst { ix: f64, y0: f64, y1: f64, cap: f64, units: String, value: f64, coded: bool }

fn lab_step(cap: f64) -> f64 { if cap <= 1.0 { 0.1 } else if cap <= 2.5 { 0.25 } else if cap <= 5.0 { 0.5 } else { cap / 10.0 } }

/// Light tint of a capacity colour for the instrument body.
fn tint(hex: &str) -> String {
    let v = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(255) as f64;
    let m = |c: f64| (c + (255.0 - c) * 0.78).round() as u8;
    format!("#{:02x}{:02x}{:02x}", m(v(1)), m(v(3)), m(v(5)))
}

/// Draw the scale; `k` is the stroke multiplier and `fs` the number size, so the same routine can
/// be drawn magnified. `full` adds the body, header and hook.
fn draw_inst(g: &mut G, c: &Inst, sw: f64, fs: f64, full: bool) {
    let ix = c.ix;
    let body = if c.coded { tint(cap_color(c.cap)) } else { "#ffffff".to_string() };
    let ypos = |v: f64| c.y0 + (c.y1 - c.y0) * v / c.cap;
    if full {
        g.circle(ix, 46.0, 7.0, "#fff", INK, 1.6);
        g.raw(&format!(r##"<rect x="{:.1}" y="76" width="124" height="378" rx="10" fill="{body}" stroke="{INK}" stroke-width="1.6"/>"##, ix - 62.0));
        let head = if c.coded { cap_color(c.cap) } else { "#d9d9d9" };
        g.raw(&format!(r##"<rect x="{:.1}" y="53" width="124" height="24" rx="5" fill="{head}" stroke="{INK}" stroke-width="1.6"/>"##, ix - 62.0));
        let txt = match c.units.as_str() {
            "Grams" => format!("{} g", fmt_num(c.cap * 100.0)),
            "Both" => format!("{} N / {} g", fmt_num(c.cap), fmt_num(c.cap * 100.0)),
            _ => format!("{} N", fmt_num(c.cap)),
        };
        g.text(ix, 70.0, if txt.len() > 8 { 10.0 } else { 12.0 }, "middle", &txt, true);
    } else {
        // magnified view: the body colour behind the tube
        g.raw(&format!(r##"<rect x="{:.1}" y="{:.1}" width="124" height="{:.1}" fill="{body}"/>"##, ix - 62.0, c.y0 - 60.0, c.y1 - c.y0 + 120.0));
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

/// Canvas width, x of the loaded instrument, x of the unloaded one (zero error) and the
/// magnifier centre; shared with the handles.
pub(super) fn spring_layout(p: &Params) -> (f64, f64, Option<f64>, f64) {
    let view = p.s("view");
    let (inst, mag) = (view != "Magnifier only", view != "Instrument only");
    let zero = inst && p.b("show_zero");
    match (zero, mag) {
        (true, true) => (720.0, 260.0, Some(110.0), 550.0),
        (true, false) => (580.0, 370.0, Some(210.0), 290.0),
        (false, true) if inst => (580.0, 130.0, None, 410.0),
        _ => (580.0, 290.0, None, 290.0),
    }
}

fn spring_render(p: &Params) -> String {
    let (w, ix, ix0, mcx) = spring_layout(p);
    let h = 600.0;
    let mut g = G::new(w, h, false);
    let cap = cap_value(p.s("cap"));
    let reading = p.f("reading").min(cap);
    let zero = p.f("offset") * cap;
    let value = reading + zero;
    let view = p.s("view").to_string();
    let show_inst = view != "Magnifier only";
    let show_mag = view != "Instrument only";
    let cfg = Inst { ix, y0: 130.0, y1: 424.0, cap, units: p.s("units").to_string(), value, coded: p.b("coded") };
    if !p.s("title").trim().is_empty() { g.title_text(w / 2.0, 26.0, 19.0, p.s("title")); }
    let py = cfg.y0 + (cfg.y1 - cfg.y0) * value.clamp(-0.08 * cap, 1.06 * cap) / cap;
    if let Some(x0) = ix0 {
        let c0 = Inst { ix: x0, value: zero, units: cfg.units.clone(), ..cfg };
        draw_inst(&mut g, &c0, 1.0, 8.5, true);
    }
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
                let k = p.i("masses").clamp(1, 8) as usize;
                g.polyline(&[(ix, 490.0), (ix, 498.0)], INK, 2.0);
                g.line(ix, 498.0, ix, 502.0 + 9.0 * k as f64, INK, 2.0);
                for i in 0..k {
                    let y = 502.0 + i as f64 * 9.0;
                    g.rect(ix - 26.0, y, 52.0, 8.0, "#cfd3da", INK, 1.4);
                }
                g.rect(ix - 30.0, 502.0 + 9.0 * k as f64, 60.0, 4.0, "#333", INK, 1.0);
            }
            _ => {}
        }
    }
    if show_mag {
        let (mcy, r) = (270.0, 128.0);
        // zoom so that about `mag_marks` numbered marks fit across the lens
        let marks: f64 = p.s("mag_marks").parse().unwrap_or(3.0);
        let major_px = (cfg.y1 - cfg.y0) * lab_step(cap) / cap;
        let z = (2.0 * r * 0.92 / (marks * major_px)).clamp(1.2, 12.0);
        g.raw(r##"<defs><clipPath id="mag">"##);
        g.raw(&format!(r#"<circle cx="{mcx:.1}" cy="{mcy:.1}" r="{r:.1}"/></clipPath></defs>"#));
        g.raw(&format!(r##"<circle cx="{mcx:.1}" cy="{mcy:.1}" r="{r:.1}" fill="#ffffff"/>"##));
        g.raw(&format!(r#"<g clip-path="url(#mag)"><g transform="translate({mcx:.2},{mcy:.2}) scale({z:.3}) translate({:.2},{:.2})">"#, -cfg.ix, -py));
        draw_inst(&mut g, &cfg, 1.28 / z, 12.0 / z, false);
        g.raw("</g></g>");
        g.circle(mcx, mcy, r, "none", INK, 3.0);
        if show_inst {
            // connectors from the small circle to the magnifier
            let sr = (r / z).clamp(14.0, 40.0);
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

pub(super) const RK: [[&str; 6]; 4] = [
    ["r1_kind", "Component 1", "r1_name", "Component 1 name (blank = automatic)", "r1_val", "Component 1 value"],
    ["r2_kind", "Component 2", "r2_name", "Component 2 name (blank = automatic)", "r2_val", "Component 2 value"],
    ["r3_kind", "Component 3", "r3_name", "Component 3 name (blank = automatic)", "r3_val", "Component 3 value"],
    ["r4_kind", "Component 4", "r4_name", "Component 4 name (blank = automatic)", "r4_val", "Component 4 value"],
];
const RVAL: [&str; 4] = ["2 Ω", "4 Ω", "6 Ω", "8 Ω"];
/// (name mode key, label, value mode key, label)
const RMODE: [[&str; 4]; 4] = [
    ["r1_name_mode", "Component 1 name shows", "r1_val_mode", "Component 1 value shows"],
    ["r2_name_mode", "Component 2 name shows", "r2_val_mode", "Component 2 value shows"],
    ["r3_name_mode", "Component 3 name shows", "r3_val_mode", "Component 3 value shows"],
    ["r4_name_mode", "Component 4 name shows", "r4_val_mode", "Component 4 value shows"],
];
pub(super) const LABEL_MODES: [&str; 3] = ["Text", "Blank line", "None"];

fn circuit_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("layout", "Resistors and bulbs", &["In series", "In parallel"], "In series"),
        ParamDef::int("n", "Number of components", 1, 4, 3),
    ];
    for (i, k) in RK.iter().enumerate() {
        v.push(ParamDef::choice(k[0], k[1], &["Resistor", "Bulb"], "Resistor"));
        v.push(ParamDef::choice(RMODE[i][0], RMODE[i][1], &LABEL_MODES, "Text"));
        v.push(ParamDef::text(k[2], k[3], ""));
        v.push(ParamDef::choice(RMODE[i][2], RMODE[i][3], &LABEL_MODES, "Text"));
        v.push(ParamDef::text(k[4], k[5], RVAL[i]));
    }
    v.push(ParamDef::choice("cell", "Cell or battery", &["One cell", "Battery of two cells", "Battery of three cells"], "Battery of two cells"));
    v.push(ParamDef::choice("cell_name_mode", "Battery name shows", &LABEL_MODES, "Text"));
    v.push(ParamDef::text("cell_name", "Battery name (e.g. epsilon; blank = none)", ""));
    v.push(ParamDef::choice("cell_val_mode", "Battery value shows", &LABEL_MODES, "Text"));
    v.push(ParamDef::text("cell_val", "Battery value", "12 V"));
    v.push(ParamDef::bool("polarity", "+ and - beside it", false));
    v.push(ParamDef::choice("switch", "Switch", &["None", "Closed", "Open"], "None"));
    v.push(ParamDef::bool("ammeter", "Ammeter in the main line", false));
    v.push(ParamDef::choice("amm_read_mode", "Ammeter reading shows", &LABEL_MODES, "None"));
    v.push(ParamDef::text("amm_read", "Ammeter reading", "2 A"));
    v.push(ParamDef::choice("voltmeter", "Voltmeter", &["None", "Across the battery", "Across component 1", "Across component 2", "Across component 3", "Across component 4"], "None"));
    v.push(ParamDef::choice("vm_read_mode", "Voltmeter reading shows", &LABEL_MODES, "None"));
    v.push(ParamDef::text("vm_read", "Voltmeter reading", "4 V"));
    v.push(ParamDef::choice("style", "Symbol style", &["US (zigzag resistor)", "IEC (box resistor)"], "US (zigzag resistor)"));
    v.push(ParamDef::choice("title_mode", "Chart title shows", &LABEL_MODES, "Text"));
    v.push(ParamDef::text("title", "Chart title", ""));
    v.push(ParamDef::bool("mirror", "Mirror (flip left to right)", false));
    v.push(ParamDef::bool("color", "Color (for slides)", false));
    v
}

/// Geometry of the circuit shared by the renderer and the drag handles.
pub(super) struct CLay {
    pub w: f64,
    pub h: f64,
    pub parallel: bool,
    pub n: usize,
    pub top_y: f64,
    pub bot_y: f64,
    pub lx: f64,
    pub rx: f64,
    /// (cx, cy, len) per component; vertical in parallel.
    pub comps: Vec<(f64, f64, f64)>,
    /// Battery centre y on the left wire and its plate count.
    pub by: f64,
    pub plates: usize,
    /// Ammeter centre and switch terminals (a, b) on the side wire.
    pub amm: Option<(f64, f64)>,
    pub sw: Option<((f64, f64), (f64, f64))>,
    /// Which component (0-based) the voltmeter is across, if any.
    pub vm_part: Option<usize>,
}

pub(super) const PITCH: f64 = 12.0;

pub(super) fn circuit_layout(p: &Params) -> CLay {
    let parallel = p.s("layout") == "In parallel";
    let n = p.i("n").clamp(1, 4) as usize;
    let vm = p.s("voltmeter");
    let vm_part = vm.strip_prefix("Across component ").and_then(|d| d.trim().parse::<usize>().ok()).filter(|k| *k >= 1 && *k <= n).map(|k| k - 1);
    let cells = match p.s("cell") { "One cell" => 1, "Battery of three cells" => 3, _ => 2 };
    let plates = cells * 2;
    let bat_h = PITCH * (plates as f64 - 1.0);
    let bulb = |i: usize| p.s(RK[i][0]) == "Bulb";
    let mut lx = 110.0;
    let (top_y, bot_y, h) = if parallel { (80.0, 330.0, 380.0) } else { (150.0, 320.0, 380.0) };
    let mut comps = vec![];
    let (rx, w);
    if parallel {
        let pitch = if vm_part.is_some() { 175.0 } else { 125.0 };
        for i in 0..n {
            let len: f64 = if bulb(i) { 44.0 } else { 64.0 };
            comps.push((240.0 + pitch * i as f64, (top_y + bot_y) / 2.0, len));
        }
        let need = comps.last().map(|c| c.0).unwrap_or(240.0) + if vm_part == Some(n - 1) { 140.0 } else { 100.0 };
        // centre narrow circuits on the standard 580-wide canvas
        let shift = ((580.0 - need) / 2.0).max(0.0);
        lx += shift;
        for c in comps.iter_mut() { c.0 += shift; }
        rx = comps.last().map(|c| c.0).unwrap_or(240.0);
        w = need.max(580.0);
    } else {
        rx = 490.0;
        w = 580.0;
    }
    let has_a = p.b("ammeter");
    let has_s = p.s("switch") != "None";
    let (mut amm, mut sw) = (None, None);
    let mut by = (top_y + bot_y) / 2.0;
    if !parallel {
        // everything but the battery sits along the top wire, evenly spaced
        let count = n + has_a as usize + has_s as usize;
        let (a, b) = (lx + 60.0, rx - 30.0);
        let slot = |i: usize| a + (b - a) * (i as f64 + 0.5) / count as f64;
        for i in 0..n {
            let len: f64 = if bulb(i) { 40.0 } else if count >= 5 { 48.0 } else if count >= 4 { 56.0 } else { 64.0 };
            comps.push((slot(i), top_y, len));
        }
        let mut k = n;
        if has_a { amm = Some((slot(k), top_y)); k += 1; }
        if has_s { let c = slot(k); sw = Some(((c - 20.0, top_y), (c + 20.0, top_y))); }
    } else {
        // ammeter, switch and battery stacked down the left wire
        let mut items: Vec<(char, f64)> = vec![];
        if has_a { items.push(('a', 34.0)); }
        if has_s { items.push(('s', 44.0)); }
        items.push(('b', bat_h + 16.0));
        let total: f64 = items.iter().map(|i| i.1).sum();
        let gap = (bot_y - top_y - total) / (items.len() as f64 + 1.0);
        let mut y = top_y + gap;
        for (k, s) in items {
            let c = y + s / 2.0;
            match k {
                'a' => amm = Some((lx, c)),
                's' => sw = Some(((lx, c - 22.0), (lx, c + 22.0))),
                _ => by = c,
            }
            y += s + gap;
        }
    }
    CLay { w, h, parallel, n, top_y, bot_y, lx, rx, comps, by, plates, amm, sw, vm_part }
}

/// Resistor or bulb centred at (cx,cy), `len` long along its axis, horizontal or vertical.
fn component(g: &mut G, bulb: bool, iec: bool, cx: f64, cy: f64, len: f64, vert: bool, c: &str) {
    let m = |lx: f64, ly: f64| if vert { (cx + ly, cy + lx) } else { (cx + lx, cy + ly) };
    let h = len / 2.0;
    if bulb {
        let r = 17.0;
        let (a, b) = (m(-h, 0.0), m(-r, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        let (a, b) = (m(r, 0.0), m(h, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        g.circle(cx, cy, r, "#fff", c, SW);
        if iec {
            let d = r * 0.7071;
            for (p1, p2) in [((-d, -d), (d, d)), ((-d, d), (d, -d))] {
                let (a, b) = (m(p1.0, p1.1), m(p2.0, p2.1));
                g.line(a.0, a.1, b.0, b.1, c, 2.0);
            }
        } else {
            // US: leads run in to a looped filament
            let bump = if vert { 6.0 } else { -6.0 };
            let mut pts = vec![m(-r, 0.0), m(-6.0, 0.0)];
            for k in 0..=12 {
                let t = std::f64::consts::PI * k as f64 / 12.0;
                pts.push(m(-6.0 * t.cos(), bump * t.sin() * 1.5));
            }
            pts.push(m(r, 0.0));
            g.polyline(&pts, c, 2.2);
        }
    } else if iec {
        let (a, b) = (m(-h, 0.0), m(-h + 10.0, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        let (a, b) = (m(h - 10.0, 0.0), m(h, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        let pts = [m(-h + 10.0, -9.0), m(h - 10.0, -9.0), m(h - 10.0, 9.0), m(-h + 10.0, 9.0)];
        g.poly(&pts, "#fff", c, SW);
    } else {
        let lead = 8.0;
        let zig = 6usize;
        let seg = (len - 2.0 * lead) / zig as f64;
        let (a, b) = (m(-h, 0.0), m(-h + lead, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        let (a, b) = (m(h - lead, 0.0), m(h, 0.0));
        g.line(a.0, a.1, b.0, b.1, INK, SW);
        let mut pts = vec![m(-h + lead, 0.0)];
        for i in 0..zig {
            let x = -h + lead + seg * (i as f64 + 0.5);
            let amp = if i % 2 == 0 { -9.0 } else { 9.0 };
            pts.push(m(x, amp));
        }
        pts.push(m(h - lead, 0.0));
        g.polyline(&pts, c, SW);
    }
}

fn meter(g: &mut G, cx: f64, cy: f64, letter: &str, c: &str) {
    g.circle(cx, cy, 17.0, "#fff", c, SW);
    g.tex(cx, cy + 7.0, 20.0, "middle", letter, UPRIGHT);
}

/// The voltmeter's reading, written to the right of the meter.
fn vm_reading(g: &mut G, p: &Params, x: f64, y: f64) {
    let rd = lab_mode(p, "vm_read", "vm_read_mode");
    clabel(g, x + 24.0, y + 8.0, "start", &rd, 21.0, UPRIGHT);
}

fn dot(g: &mut G, x: f64, y: f64) { g.circle(x, y, 3.6, INK, INK, 1.0); }

/// A circuit label at baseline `y`: "none" draws nothing, "" a blank answer line.
fn clabel(g: &mut G, x: f64, y: f64, anchor: &str, s: &str, size: f64, st: u8) {
    if s.eq_ignore_ascii_case("none") { return; }
    if s.trim().is_empty() {
        let (a, b) = match anchor { "start" => (x, x + 34.0), "end" => (x - 34.0, x), _ => (x - 18.0, x + 18.0) };
        g.line(a, y + 2.0, b, y + 2.0, INK, 1.5);
    } else {
        // a bare unknown such as "x" or "R" is a symbol: italic even where values are upright
        let st = if st == UPRIGHT && s.trim().chars().count() <= 2 && s.trim().chars().all(|c| c.is_alphabetic()) { ITALIC } else { st };
        g.tex(x, y, size, anchor, s, st);
    }
}

/// Name label text of component `i` (automatic R_k / L_k numbering when left blank).
pub(super) fn comp_name(p: &Params, i: usize, n: usize) -> String {
    match p.s(RMODE[i][0]) {
        "Blank line" => return String::new(),
        "None" => return "none".into(),
        _ => {}
    }
    let nm = p.s(RK[i][2]).trim();
    if !nm.is_empty() { return nm.to_string(); }
    let bulb = p.s(RK[i][0]) == "Bulb";
    let same: Vec<usize> = (0..n).filter(|&j| (p.s(RK[j][0]) == "Bulb") == bulb).collect();
    let letter = if bulb { "L" } else { "R" };
    if same.len() == 1 { letter.to_string() } else {
        let k = same.iter().position(|&j| j == i).unwrap_or(0) + 1;
        format!("{letter}_{k}")
    }
}

fn circuit_render(p: &Params) -> String {
    let l = circuit_layout(p);
    let mut g = G::new(l.w, l.h, p.b("mirror"));
    let cc = if p.b("color") { "#1d4ed8" } else { INK };
    let iec = p.s("style").starts_with("IEC");
    let (top_y, bot_y, lx, rx) = (l.top_y, l.bot_y, l.lx, l.rx);
    draw_title(&mut g, p, l.w / 2.0, 32.0);
    let seg_line = |g: &mut G, a: (f64, f64), b: (f64, f64), gaps: &mut Vec<(f64, f64)>| {
        // straight wire from a to b (horizontal or vertical) skipping the gaps (along-axis ranges)
        let horiz = (a.1 - b.1).abs() < 1e-6;
        let (s0, s1) = if horiz { (a.0, b.0) } else { (a.1, b.1) };
        gaps.sort_by(|u, v| u.0.partial_cmp(&v.0).unwrap());
        let mut s = s0;
        let put = |g: &mut G, u: f64, v: f64| {
            if v - u < 0.5 { return; }
            if horiz { g.line(u, a.1, v, a.1, INK, SW); } else { g.line(a.0, u, a.0, v, INK, SW); }
        };
        for &(u, v) in gaps.iter() { if u > s { put(g, s, u); } s = s.max(v); }
        if s < s1 { put(g, s, s1); }
    };
    // components and their labels
    let (mut top_gaps, mut left_gaps) = (vec![], vec![]);
    for (i, &(cx, cy, len)) in l.comps.iter().enumerate() {
        let bulb = p.s(RK[i][0]) == "Bulb";
        component(&mut g, bulb, iec, cx, cy, len, l.parallel, cc);
        let name = comp_name(p, i, l.n);
        let val = lab_mode(p, RK[i][4], RMODE[i][2]);
        if l.parallel {
            let mut br = vec![(cy - len / 2.0, cy + len / 2.0)];
            seg_line(&mut g, (cx, top_y), (cx, bot_y), &mut br);
            clabel(&mut g, cx + 24.0, cy + 8.0, "start", &name, 22.0, ITALIC);
            clabel(&mut g, cx - 24.0, cy + 8.0, "end", &val, 21.0, UPRIGHT);
            if i + 1 < l.n { dot(&mut g, cx, top_y); dot(&mut g, cx, bot_y); }
        } else {
            top_gaps.push((cx - len / 2.0, cx + len / 2.0));
            clabel(&mut g, cx, top_y - 24.0, "middle", &name, 22.0, ITALIC);
            clabel(&mut g, cx, top_y + if bulb { 46.0 } else { 40.0 }, "middle", &val, 21.0, UPRIGHT);
        }
    }
    // ammeter and switch (top wire in series, left wire in parallel)
    if let Some((x, y)) = l.amm {
        meter(&mut g, x, y, "A", cc);
        let rd = lab_mode(p, "amm_read", "amm_read_mode");
        if l.parallel { left_gaps.push((y - 17.0, y + 17.0)); clabel(&mut g, x + 24.0, y + 8.0, "start", &rd, 21.0, UPRIGHT); }
        else { top_gaps.push((x - 17.0, x + 17.0)); clabel(&mut g, x, y + 44.0, "middle", &rd, 21.0, UPRIGHT); }
    }
    if let Some((a, b)) = l.sw {
        let horiz = (a.1 - b.1).abs() < 1e-6;
        g.circle(a.0, a.1, 4.0, "#fff", cc, 2.2);
        g.circle(b.0, b.1, 4.0, "#fff", cc, 2.2);
        if p.s("switch") == "Closed" {
            if horiz { g.line(a.0 + 4.0, a.1, b.0 - 4.0, b.1, cc, SW); } else { g.line(a.0, a.1 + 4.0, b.0, b.1 - 4.0, cc, SW); }
        } else if horiz {
            // blade hinged at the left terminal, lifted clear of the right one
            g.line(a.0 + 3.0, a.1 - 1.5, b.0 + 2.0, b.1 - 22.0, cc, SW);
        } else {
            g.line(a.0 + 1.5, a.1 + 3.0, b.0 + 22.0, b.1 - 2.0, cc, SW);
        }
        if horiz { top_gaps.push((a.0 - 4.0, b.0 + 4.0)); } else { left_gaps.push((a.1 - 4.0, b.1 + 4.0)); }
    }
    // battery on the left wire
    let by = l.by;
    let first = by - PITCH * (l.plates as f64 - 1.0) / 2.0;
    let last = first + PITCH * (l.plates as f64 - 1.0);
    left_gaps.push((first, last));
    for k in 0..l.plates {
        let y = first + PITCH * k as f64;
        let long = k % 2 == 0;
        let half = if long { 24.0 } else { 12.0 };
        g.line(lx - half, y, lx + half, y, cc, if long { 2.6 } else { 5.5 });
        if !long && k + 1 < l.plates { g.line(lx, y, lx, y + PITCH, cc, 2.0); }
    }
    let cname = lab_mode(p, "cell_name", "cell_name_mode");
    let cname_show = !(p.s("cell_name_mode") == "Text" && cname.trim().is_empty());
    if cname_show { clabel(&mut g, lx - 32.0, by + 8.0, "end", &cname, 22.0, ITALIC); }
    let cval = lab_mode(p, "cell_val", "cell_val_mode");
    clabel(&mut g, lx + 34.0, by + 8.0, "start", &cval, 22.0, UPRIGHT);
    if p.b("polarity") {
        g.tex(lx - 20.0, first - 6.0, 17.0, "middle", "+", UPRIGHT);
        g.tex(lx - 20.0, last + 19.0, 17.0, "middle", "−", UPRIGHT);
    }
    // wires
    seg_line(&mut g, (lx, top_y), (rx, top_y), &mut top_gaps);
    seg_line(&mut g, (lx, bot_y), (rx, bot_y), &mut vec![]);
    seg_line(&mut g, (lx, top_y), (lx, bot_y), &mut left_gaps);
    if !l.parallel { seg_line(&mut g, (rx, top_y), (rx, bot_y), &mut vec![]); }
    // voltmeter
    if p.s("voltmeter") == "Across the battery" {
        let vx = lx - 66.0;
        let ya = (by - 46.0).max(top_y + 8.0);
        let yb = (by + 46.0).min(bot_y - 8.0);
        let ym = (ya + yb) / 2.0;
        g.polyline(&[(lx, ya), (vx, ya), (vx, ym - 17.0)], INK, SW);
        g.polyline(&[(vx, ym + 17.0), (vx, yb), (lx, yb)], INK, SW);
        meter(&mut g, vx, ym, "V", cc);
        vm_reading(&mut g, p, vx, ym);
        dot(&mut g, lx, ya);
        dot(&mut g, lx, yb);
    } else if let Some(k) = l.vm_part {
        let (cx, cy, len) = l.comps[k];
        if l.parallel {
            let (ya, yb, vx) = (cy - len / 2.0 - 18.0, cy + len / 2.0 + 18.0, cx + 76.0);
            g.polyline(&[(cx, ya), (vx, ya), (vx, cy - 17.0)], INK, SW);
            g.polyline(&[(vx, cy + 17.0), (vx, yb), (cx, yb)], INK, SW);
            meter(&mut g, vx, cy, "V", cc);
            vm_reading(&mut g, p, vx, cy);
            dot(&mut g, cx, ya);
            dot(&mut g, cx, yb);
        } else {
            let (xa, xb, vy) = (cx - len / 2.0 - 10.0, cx + len / 2.0 + 10.0, top_y - 82.0);
            g.polyline(&[(xa, top_y), (xa, vy), (cx - 17.0, vy)], INK, SW);
            g.polyline(&[(cx + 17.0, vy), (xb, vy), (xb, top_y)], INK, SW);
            meter(&mut g, cx, vy, "V", cc);
            vm_reading(&mut g, p, cx, vy);
            dot(&mut g, xa, top_y);
            dot(&mut g, xb, top_y);
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
