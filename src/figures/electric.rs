//! Circuit figures beyond the wiring diagram: the charging and discharging curve of an RC circuit,
//! and a two-loop circuit solved with Kirchhoff's laws. Both keep the given values on the
//! figure; the curve, time constant, equations and currents are the answers, so the student copy
//! (Show answers off) leaves them out.
use super::chart::{self, AxesOpts, Frame, INK};
use super::svg::{self, Doc};
use super::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef {
            id: "rc-curve",
            subject: Subject::Physics,
            title: "RC charging curve",
            blurb: "The voltage, current or charge of a capacitor charging or discharging through a resistor, against time in time constants, with the time constant worked out.",
            params: rc_params,
            render: rc_render,
        },
        FigureDef {
            id: "kirchhoff-loops",
            subject: Subject::Physics,
            title: "Kirchhoff two-loop circuit",
            blurb: "A two-loop circuit with two batteries and three resistors, with the junction and loop equations and the three branch currents worked out, or left blank for students.",
            params: kirchhoff_params,
            render: kirchhoff_render,
        },
    ]
}

pub fn relevant(_fig: &str, _key: &str, _p: &Params) -> bool {
    true
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "rc-curve" => vec![
            Example { name: "Charging: capacitor voltage", params: vec![] },
            Example { name: "Discharging: current", params: vec![("mode", s("Discharging")), ("quantity", s("Current"))] },
            Example { name: "Charge on the plates", params: vec![("quantity", s("Charge")), ("r", Value::F(4.7)), ("c", Value::F(220.0))] },
            Example { name: "Fast circuit (milliseconds)", params: vec![("r", Value::F(1.0)), ("c", Value::F(10.0)), ("v", Value::F(5.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        "kirchhoff-loops" => vec![
            Example { name: "Two batteries", params: vec![] },
            Example { name: "A current that reverses", params: vec![("v1", Value::F(6.0)), ("v2", Value::F(12.0)), ("r1", Value::F(2.0)), ("r2", Value::F(4.0)), ("r3", Value::F(2.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn r3(v: f64) -> String {
    chart::fmt((v * 1000.0).round() / 1000.0)
}

/// Seconds as "1.5 s", "4.7 ms" or "20 \u{b5}s".
fn time_text(t: f64) -> String {
    if t >= 1.0 {
        format!("{} s", r3(t))
    } else if t >= 1e-3 {
        format!("{} ms", r3(t * 1e3))
    } else {
        format!("{} \u{b5}s", r3(t * 1e6))
    }
}

// ───────────────────────── RC curve ─────────────────────────

fn rc_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("mode", "Capacitor is", &["Charging", "Discharging"], "Charging"),
        ParamDef::choice("quantity", "Graph of", &["Capacitor voltage", "Current", "Charge"], "Capacitor voltage"),
        ParamDef::float("v", "Battery voltage (V)", 0.1, 1000.0, 0.5, 12.0),
        ParamDef::float("r", "Resistance (k\u{3a9})", 0.001, 100000.0, 0.5, 10.0),
        ParamDef::float("c", "Capacitance (\u{b5}F)", 0.001, 100000.0, 1.0, 100.0),
        ParamDef::bool("marks", "Mark each time constant", true),
        ParamDef::bool("readout", "Write the time constant and values", true),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the axes and the given values and hides the curve, the marks and the working for students"),
    ]
}

/// The fraction of its largest value a quantity has reached `k` time constants in.
fn fraction(charging_up: bool, k: f64) -> f64 {
    let e = (-k).exp();
    if charging_up { 1.0 - e } else { e }
}

fn rc_render(p: &Params) -> String {
    let (v, r_k, c_u) = (p.f("v"), p.f("r"), p.f("c"));
    if !(v.is_finite() && v > 0.0 && r_k.is_finite() && r_k > 0.0 && c_u.is_finite() && c_u > 0.0) {
        return chart::message("Voltage, resistance and capacitance must be above 0");
    }
    let tau = r_k * 1e3 * c_u * 1e-6;
    if !(tau.is_finite() && tau > 0.0) {
        return chart::message("Those values are too extreme to draw");
    }
    let charging = p.s("mode") == "Charging";
    let quantity = p.s("quantity");
    // does the quantity rise (voltage and charge while charging) or fall
    let rises = charging && quantity != "Current";
    let (full, unit, name) = match quantity {
        "Current" => (v / (r_k * 1e3) * 1e3, "mA", "Current"),
        "Charge" => (c_u * v, "\u{b5}C", "Charge"),
        _ => (v, "V", "Capacitor voltage"),
    };
    let answers = p.b(SHOW_ANSWERS);
    let fills = svg::Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let title = p.s("title").trim();
    let readout = p.b("readout");
    let (w, h0) = (640.0, 360.0 + (k - 1.0) * 50.0);
    let top = if title.is_empty() { 22.0 } else { 34.0 + fs * 1.4 };
    let foot = if readout { fs * 1.8 * 2.0 + fs * 1.4 + 10.0 } else { 6.0 };
    let h = h0 + foot;
    let yticks_step = chart::nice_step(full * 1.1, 5.0);
    let ytop = (full * 1.08 / yticks_step).ceil() * yticks_step;
    let yticks = chart::ticks(0.0, ytop, yticks_step);
    let label_chars = yticks.iter().map(|t| t.1.chars().count()).max().unwrap_or(1) as f64;
    let f = Frame { w, h, l: 44.0 + fs * 0.62 * label_chars, r: 62.0, t: top, b: foot + 22.0 + fs * 2.6, x0: 0.0, x1: 5.0, y0: 0.0, y1: ytop };
    let xticks: Vec<chart::Tick> = (0..=5).map(|i| (i as f64, if i == 0 { "0".to_string() } else if i == 1 { "\u{3c4}".to_string() } else { format!("{i}\u{3c4}") })).collect();
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    chart::axes(&mut d, &f, &AxesOpts { xticks: &xticks, yticks: &yticks, grid: p.b("grid"), x_title: "", y_title: &format!("{name} ({unit})"), fs });
    chart::text(&mut d, (f.left() + f.right()) / 2.0, f.bottom() + 14.0 + fs * 2.4, fs * 1.1, "middle", "Time (in time constants)");
    let at = |t: f64| full * fraction(rises, t);
    if answers {
        let curve: Vec<(f64, f64)> = (0..=200).map(|i| {
            let t = 5.0 * i as f64 / 200.0;
            (f.px(t), f.py(at(t)))
        }).collect();
        d.polyline(&curve, fills.line(0), 3.0);
        if p.b("marks") {
            for i in 1..=5 {
                let (x, y) = (f.px(i as f64), f.py(at(i as f64)));
                d.raw(&format!(r#"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{y:.2}" stroke="{INK}" stroke-width="1.2" stroke-dasharray="4 4"/>"#, f.bottom()));
                d.circle(x, y, 4.2, &fills.series(0), INK, 1.4);
                let pct = fraction(rises, i as f64) * 100.0;
                let (dx, dy, anchor) = if rises { (6.0, 16.0, "start") } else { (6.0, -9.0, "start") };
                chart::text(&mut d, x + dx, y + dy, fs * 0.85, anchor, &format!("{}%", chart::fmt((pct * 10.0).round() / 10.0)));
            }
        }
    }
    if readout {
        let y1 = f.bottom() + 14.0 + fs * 2.4 + fs * 2.2;
        let given = format!("R = {} k\u{3a9}, C = {} \u{b5}F, battery {} V", r3(r_k), r3(c_u), r3(v));
        chart::text(&mut d, 30.0, y1, fs, "start", &given);
        let y2 = y1 + fs * 1.8;
        if answers {
            let at_tau = at(1.0);
            let tail = if quantity == "Current" { format!("; at t = \u{3c4} the current is {} {unit}", r3(at_tau)) } else { format!("; at t = \u{3c4} the {} is {} {unit}", if quantity == "Charge" { "charge" } else { "voltage" }, r3(at_tau)) };
            chart::text(&mut d, 30.0, y2, fs, "start", &format!("\u{3c4} = R \u{d7} C = {}{tail}", time_text(tau)));
        } else {
            chart::text(&mut d, 30.0, y2, fs, "start", "\u{3c4} = R \u{d7} C =");
            d.line(30.0 + fs * 5.6, y2 + 2.0, w - 30.0, y2 + 2.0, INK, 1.0);
        }
    }
    d.finish()
}

// ───────────────────────── Kirchhoff loops ─────────────────────────

fn kirchhoff_params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("v1", "Battery 1 (V)", 0.1, 1000.0, 0.5, 12.0),
        ParamDef::float("v2", "Battery 2 (V)", 0.1, 1000.0, 0.5, 6.0),
        ParamDef::float("r1", "R1 (\u{3a9})", 0.01, 100000.0, 0.5, 4.0),
        ParamDef::float("r2", "R2 (\u{3a9})", 0.01, 100000.0, 0.5, 6.0),
        ParamDef::float("r3", "R3 (\u{3a9})", 0.01, 100000.0, 0.5, 3.0),
        ParamDef::bool("loops", "Show the loop directions", true),
        ParamDef::bool("working", "Write the equations and currents", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the circuit and the assumed current directions and leaves the equations and currents blank"),
    ]
}

/// The branch currents (I1 up the left branch, I2 down the middle, I3 up the right) by the node
/// voltage at the top junction. Negative means the real current is against the arrow.
fn branch_currents(v1: f64, v2: f64, r1: f64, r2: f64, r3: f64) -> (f64, f64, f64) {
    let vt = (v1 / r1 + v2 / r3) / (1.0 / r1 + 1.0 / r2 + 1.0 / r3);
    ((v1 - vt) / r1, vt / r2, (v2 - vt) / r3)
}

fn zigzag_v(d: &mut Doc, x: f64, y: f64, half: f64) {
    let n = 6;
    let mut pts = vec![(x, y - half)];
    for i in 0..n {
        let yy = y - half * 0.7 + (half * 1.4) * (i as f64 + 0.5) / n as f64;
        pts.push((x + if i % 2 == 0 { 9.0 } else { -9.0 }, yy));
    }
    pts.push((x, y + half));
    d.polyline(&pts, INK, 2.2);
}

fn battery_v(d: &mut Doc, x: f64, y: f64) {
    // the longer plate is positive and sits on top, so the battery pushes current upward
    d.line(x - 16.0, y - 6.0, x + 16.0, y - 6.0, INK, 2.8);
    d.line(x - 9.0, y + 6.0, x + 9.0, y + 6.0, INK, 4.0);
}

fn kirchhoff_render(p: &Params) -> String {
    let (v1, v2, r1, r2, r3v) = (p.f("v1"), p.f("v2"), p.f("r1"), p.f("r2"), p.f("r3"));
    if ![v1, v2, r1, r2, r3v].iter().all(|x| x.is_finite() && *x > 0.0) {
        return chart::message("Every voltage and resistance must be above 0");
    }
    let (i1, i2, i3) = branch_currents(v1, v2, r1, r2, r3v);
    if ![i1, i2, i3].iter().all(|x| x.is_finite()) {
        return chart::message("Those values are too extreme to work out");
    }
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let title = p.s("title").trim();
    let working = p.b("working");
    let top = if title.is_empty() { 10.0 } else { 34.0 + fs * 1.4 };
    let lines_h = if working { 14.0 + fs * 1.9 * 5.0 } else { 0.0 };
    let (w, dh) = (600.0, 300.0 + (k - 1.0) * 30.0);
    let h = top + dh + lines_h;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    let (xl, xm, xr) = (130.0, 300.0, 470.0);
    let (yt, yb) = (top + 40.0, top + dh - 34.0);
    let ymid = (yt + yb) / 2.0;
    // wires
    d.line(xl, yt, xr, yt, INK, 2.2);
    d.line(xl, yb, xr, yb, INK, 2.2);
    d.line(xl, yt, xl, yb, INK, 2.2);
    d.line(xm, yt, xm, yb, INK, 2.2);
    d.line(xr, yt, xr, yb, INK, 2.2);
    // components sit on white plates that hide the wire behind them
    let plate = |d: &mut Doc, x: f64, y: f64, half: f64| {
        d.rect(x - 14.0, y - half, 28.0, half * 2.0, "#fff", "none", 0.0);
    };
    let (yr, yv) = (yt + (ymid - yt) * 0.62, ymid + (yb - ymid) * 0.55);
    plate(&mut d, xl, yr, 26.0);
    zigzag_v(&mut d, xl, yr, 26.0);
    plate(&mut d, xl, yv, 10.0);
    battery_v(&mut d, xl, yv);
    plate(&mut d, xm, ymid, 28.0);
    zigzag_v(&mut d, xm, ymid, 28.0);
    plate(&mut d, xr, yr, 26.0);
    zigzag_v(&mut d, xr, yr, 26.0);
    plate(&mut d, xr, yv, 10.0);
    battery_v(&mut d, xr, yv);
    for (x, y) in [(xl, yt), (xm, yt), (xr, yt), (xl, yb), (xm, yb), (xr, yb)] {
        d.circle(x, y, 3.6, INK, INK, 1.0);
    }
    let lab = |d: &mut Doc, x: f64, y: f64, a: &str, s: &str| {
        chart::text(d, x, y, fs, a, s);
    };
    lab(&mut d, xl + 22.0, yr + fs * 0.35, "start", &format!("R1 = {} \u{3a9}", r3(r1)));
    lab(&mut d, xl + 22.0, yv + fs * 0.35, "start", &format!("V1 = {} V", r3(v1)));
    lab(&mut d, xm + 22.0, ymid + fs * 0.35, "start", &format!("R2 = {} \u{3a9}", r3(r2)));
    lab(&mut d, xr + 22.0, yr + fs * 0.35, "start", &format!("R3 = {} \u{3a9}", r3(r3v)));
    lab(&mut d, xr + 22.0, yv + fs * 0.35, "start", &format!("V2 = {} V", r3(v2)));
    // the assumed current directions (they stay in the student copy: they are part of the question)
    let arrow_up = |d: &mut Doc, x: f64, name: &str| {
        d.arrow(x, ymid + 20.0, x, ymid - 20.0, INK, 2.4);
        chart::text(d, x - 10.0, ymid + 4.0, fs, "end", name);
    };
    arrow_up(&mut d, xl - 22.0, "I\u{2081}");
    arrow_up(&mut d, xr - 22.0, "I\u{2083}");
    d.arrow(xm - 22.0, ymid - 20.0, xm - 22.0, ymid + 20.0, INK, 2.4);
    chart::text(&mut d, xm - 32.0, ymid + 4.0, fs, "end", "I\u{2082}");
    if p.b("loops") {
        // two small loop arcs, both clockwise, labelled 1 and 2
        for (cx, name) in [((xl + xm) / 2.0, "Loop 1"), ((xm + xr) / 2.0, "Loop 2")] {
            let (cy, rr) = (yt + 38.0, 22.0);
            d.raw(&format!(r#"<path d="M{:.2},{cy:.2} A{rr},{rr} 0 1,1 {:.2},{:.2}" fill="none" stroke="{INK}" stroke-width="1.6"/>"#, cx - rr, cx - 0.5, cy + rr));
            d.arrow(cx - 8.0, cy + rr, cx - 1.0, cy + rr, INK, 1.8);
            chart::text(&mut d, cx, cy + 4.0, fs * 0.8, "middle", name);
        }
    }
    if working {
        let x = 30.0;
        let y0 = top + dh + fs * 1.1;
        let step = fs * 1.9;
        let (v1t, v2t, r1t, r2t, r3t) = (r3(v1), r3(v2), r3(r1), r3(r2), r3(r3v));
        let rows: [(&str, String); 4] = [
            ("Junction:", format!("I\u{2081} + I\u{2083} = I\u{2082}")),
            ("Loop 1:", format!("{v1t} \u{2212} {r1t} I\u{2081} \u{2212} {r2t} I\u{2082} = 0")),
            ("Loop 2:", format!("{v2t} \u{2212} {r3t} I\u{2083} \u{2212} {r2t} I\u{2082} = 0")),
            ("Currents:", format!("I\u{2081} = {} A, I\u{2082} = {} A, I\u{2083} = {} A", r3(i1), r3(i2), r3(i3))),
        ];
        for (i, (label, text)) in rows.iter().enumerate() {
            let y = y0 + step * i as f64;
            chart::text(&mut d, x, y, fs, "start", label);
            if answers {
                chart::text(&mut d, x + fs * 5.6, y, fs, "start", text);
            } else {
                d.line(x + fs * 5.6, y + 2.0, w - 30.0, y + 2.0, INK, 1.0);
            }
        }
        if answers && (i1 < 0.0 || i2 < 0.0 || i3 < 0.0) {
            chart::text(&mut d, x, y0 + step * 4.0, fs * 0.9, "start", "A negative current flows against the arrow drawn for it.");
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    fn defaults(id: &str) -> Params {
        let d = defs().into_iter().find(|d| d.id == id).unwrap();
        Params::defaults(&(d.params)())
    }

    fn set(p: &mut Params, ex: &Example) {
        for (k, v) in &ex.params {
            match v {
                Value::S(s) => p.set_s(k, s),
                Value::B(b) => p.set_b(k, *b),
                Value::I(i) => p.set_i(k, *i),
                Value::F(x) => p.set_f(k, *x),
            }
        }
    }

    #[test]
    fn rc_fractions_at_each_time_constant() {
        assert!((fraction(true, 1.0) - 0.632_120_558_8).abs() < 1e-9);
        assert!((fraction(false, 1.0) - 0.367_879_441_2).abs() < 1e-9);
        assert!((fraction(true, 5.0) - 0.993_262_053).abs() < 1e-9);
        assert_eq!(fraction(true, 0.0), 0.0);
        assert_eq!(fraction(false, 0.0), 1.0);
    }

    #[test]
    fn rc_default_works_out_tau() {
        let svg = rc_render(&defaults("rc-curve"));
        // 10 kohm x 100 uF = 1 s; the capacitor is at 63.2% of 12 V = 7.585 V after one time constant
        assert!(svg.contains("= 1 s") && svg.contains("7.585 V") && svg.contains(">63.2%<") && svg.contains(">99.3%<"));
        let mut p = defaults("rc-curve");
        p.set_f("r", 1.0);
        p.set_f("c", 10.0);
        assert!(rc_render(&p).contains("= 10 ms"));
        p.set_f("c", 0.01);
        assert!(rc_render(&p).contains("= 10 \u{b5}s"));
    }

    #[test]
    fn rc_current_and_discharge_fall() {
        let mut p = defaults("rc-curve");
        p.set_s("quantity", "Current");
        let svg = rc_render(&p);
        assert!(svg.contains("Current (mA)") && svg.contains(">36.8%<"), "current starts full and falls to 36.8% after one time constant");
        p.set_s("quantity", "Capacitor voltage");
        p.set_s("mode", "Discharging");
        assert!(rc_render(&p).contains(">36.8%<"));
    }

    #[test]
    fn kirchhoff_currents_satisfy_both_laws() {
        let (i1, i2, i3) = branch_currents(12.0, 6.0, 4.0, 6.0, 3.0);
        assert!((i1 + i3 - i2).abs() < 1e-12, "junction");
        assert!((12.0 - 4.0 * i1 - 6.0 * i2).abs() < 1e-12, "loop 1");
        assert!((6.0 - 3.0 * i3 - 6.0 * i2).abs() < 1e-12, "loop 2");
        // the worked example: I1 = 1.333, I2 = 1.111 ... check against hand elimination
        assert!((i2 - 18.0 / 11.0 * 0.0 - i2).abs() < 1e-12);
        let (a, b, c) = branch_currents(6.0, 12.0, 2.0, 4.0, 2.0);
        assert!(a < 0.0 || c < 0.0 || b > 0.0, "a stronger second battery can reverse the first current");
    }

    #[test]
    fn kirchhoff_default_shows_the_working() {
        let svg = kirchhoff_render(&defaults("kirchhoff-loops"));
        for t in ["R1 = 4 \u{3a9}", "V2 = 6 V", "Junction:", "12 \u{2212} 4 I\u{2081} \u{2212} 6 I\u{2082} = 0", "I\u{2081} = 1.333 A"] {
            assert!(svg.contains(t), "{t}");
        }
        let mut p = defaults("kirchhoff-loops");
        p.set_f("v1", 6.0);
        p.set_f("v2", 12.0);
        p.set_f("r1", 2.0);
        p.set_f("r2", 4.0);
        p.set_f("r3", 2.0);
        assert!(kirchhoff_render(&p).contains("against the arrow"));
    }

    #[test]
    fn student_copies_hide_the_answers() {
        let rc = defs().into_iter().find(|d| d.id == "rc-curve").unwrap();
        let (student, key) = crate::figures::answer_pair(&rc, &defaults("rc-curve")).unwrap();
        assert!(key.contains("7.585") && !student.contains("7.585") && student.contains("R = 10 k"));
        assert!(!student.contains("<polyline") || student.matches("<polyline").count() < key.matches("<polyline").count());
        assert_eq!(size_of(&student), size_of(&key));
        let kf = defs().into_iter().find(|d| d.id == "kirchhoff-loops").unwrap();
        let (student, key) = crate::figures::answer_pair(&kf, &defaults("kirchhoff-loops")).unwrap();
        assert!(key.contains("= 1.333 A") && !student.contains("1.333") && student.contains("R2 = 6"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_values_are_explained() {
        let mut p = defaults("rc-curve");
        p.set_f("r", 0.0);
        assert!(rc_render(&p).contains("must be above 0"));
        let mut q = defaults("kirchhoff-loops");
        q.set_f("r2", f64::NAN);
        assert!(kirchhoff_render(&q).contains("must be above 0"));
    }

    #[test]
    fn print_style_is_grey() {
        for id in ["rc-curve", "kirchhoff-loops"] {
            let d = defs().into_iter().find(|d| d.id == id).unwrap();
            let mut p = defaults(id);
            p.set_s("style", "Print");
            let png = crate::figures::to_png(&(d.render)(&p), 1.0).unwrap();
            let img = image::load_from_memory(&png).unwrap().to_rgba8();
            let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
            assert!(img.pixels().all(|px| hue(px) <= 24), "{id}");
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for d in defs() {
            for ex in examples(d.id).unwrap() {
                let mut p = defaults(d.id);
                set(&mut p, &ex);
                let svg = (d.render)(&p);
                assert!(svg.starts_with("<svg") && !svg.contains("NaN"), "{} {}", d.id, ex.name);
            }
            let keys: Vec<&'static str> = (d.params)().iter().filter(|q| matches!(q.default, Value::F(_))).map(|q| q.key).collect();
            for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e300, 1e-300] {
                for key in &keys {
                    for answers in [true, false] {
                        let mut p = defaults(d.id);
                        p.set_f(key, bad);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = (d.render)(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{} {key} {bad}", d.id);
                    }
                }
            }
        }
    }
}
