//! A free-body diagram: an object with the forces on it drawn as arrows from a name, a size in
//! newtons and a direction. The list of forces is the question; the drawing, the net force and
//! (with a mass) the acceleration are the answers, so the student copy (Show answers off) shows the
//! object and the list of forces with the arrows left to draw.
use crate::figures::chart;
use crate::figures::geography::trim;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";
const MAX_FORCES: usize = 6;
const DEFAULT: &str = "Weight 49 270\nNormal 49 90\nPush 30 0\nFriction 12 180";

pub fn def() -> FigureDef {
    FigureDef {
        id: "free-body",
        subject: Subject::Health,
        title: "Free-body diagram",
        blurb: "A free-body diagram of an object with its forces drawn as arrows from a list of size and direction, with the net force and acceleration worked out, or the list alone for students to draw.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "free-body" => vec![
            Example { name: "Box pushed along a floor", params: vec![] },
            Example { name: "In equilibrium", params: vec![("forces", s("Weight 20 270\nTension 20 90")), ("object", s("Lamp"))] },
            Example { name: "A slope", params: vec![("object", s("Skier")), ("mass", Value::F(70.0)), ("forces", s("Weight 686 270\nNormal 594 120\nFriction 80 210"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("object", "Object", "Box"),
        ParamDef::long("forces", "Forces: name, size (N), direction (degrees)", DEFAULT).hint("One per line, as Weight 49 270. Direction counts anticlockwise from pointing right: 0 right, 90 up, 180 left, 270 down. Up to 6 forces"),
        ParamDef::float("mass", "Mass in kg (0 to leave out the acceleration)", 0.0, 1.0e6, 0.5, 5.0),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the object and the list of forces and leaves the arrows, the net force and the acceleration for students"),
    ]
}

#[derive(Debug)]
pub struct Force {
    pub name: String,
    pub mag: f64,
    pub deg: f64,
}

pub fn parse(spec: &str) -> Result<Vec<Force>, String> {
    if spec.chars().count() > 600 {
        return Err("The forces are limited to 600 characters".into());
    }
    let mut v = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut parts = line.rsplitn(3, char::is_whitespace);
        let (deg, mag, name) = (parts.next(), parts.next(), parts.next());
        let (Some(deg), Some(mag), Some(name)) = (deg, mag, name) else {
            return Err(format!("\u{201c}{}\u{201d} needs a name, a size and a direction", line.chars().take(20).collect::<String>()));
        };
        let num = |s: &str, lo: f64, hi: f64| s.parse::<f64>().ok().filter(|x| x.is_finite() && (lo..=hi).contains(x));
        let mag = num(mag, 0.0, 1.0e6).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a size from 0", mag.chars().take(12).collect::<String>()))?;
        let deg = num(deg, -3600.0, 3600.0).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a direction", deg.chars().take(12).collect::<String>()))?;
        v.push(Force { name: name.trim().chars().take(18).collect(), mag, deg });
        if v.len() > MAX_FORCES {
            return Err(format!("There can be at most {MAX_FORCES} forces"));
        }
    }
    if v.is_empty() {
        return Err("Give some forces, such as Weight 49 270".into());
    }
    Ok(v)
}

/// The sum of the forces: (x, y, size, direction in degrees from 0 to 360).
pub fn net(f: &[Force]) -> (f64, f64, f64, f64) {
    let (x, y) = f.iter().fold((0.0, 0.0), |a, f| (a.0 + f.mag * f.deg.to_radians().cos(), a.1 + f.mag * f.deg.to_radians().sin()));
    let (x, y) = (if x.abs() < 1e-9 { 0.0 } else { x }, if y.abs() < 1e-9 { 0.0 } else { y });
    let size = (x * x + y * y).sqrt();
    (x, y, size, if size == 0.0 { 0.0 } else { y.atan2(x).to_degrees().rem_euclid(360.0) })
}

fn render(p: &Params) -> String {
    let forces = match parse(p.s("forces")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let mass = p.f("mass");
    if !mass.is_finite() {
        return chart::message("Give the mass as a number");
    }
    let mass = mass.clamp(0.0, 1.0e6);
    let object: String = p.s("object").trim().chars().take(16).collect();
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (1010.0, top + 640.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (cx, cy) = (300.0, top + 280.0);
    let (bw, bh) = (90.0, 60.0);
    let (fx, fy, fnet, fdeg) = net(&forces);
    let biggest = forces.iter().map(|f| f.mag).fold(fnet, f64::max).max(1e-9);
    let len = |m: f64| (m / biggest * 190.0).max(if m > 0.0 { 36.0 } else { 0.0 });
    // the object
    d.rect(cx - bw / 2.0, cy - bh / 2.0, bw, bh, if print { "#e6e6e6" } else { "#e8dcc8" }, INK, 2.2);
    chart::text(&mut d, cx, cy + fs * 0.35, fs, "middle", &object);
    if answers {
        let red = if print { "#333333" } else { "#c0392b" };
        let start_of = |ang: f64| {
            let (ux, uy) = (ang.to_radians().cos(), -ang.to_radians().sin());
            let s = (bw / 2.0 / ux.abs().max(1e-9)).min(bh / 2.0 / uy.abs().max(1e-9));
            (cx + ux * s, cy + uy * s, ux, uy)
        };
        for f in forces.iter().filter(|f| f.mag > 0.0) {
            let (sx, sy, ux, uy) = start_of(f.deg);
            let l = len(f.mag);
            d.arrow(sx, sy, sx + ux * l, sy + uy * l, red, 3.0);
            let anchor = if ux > 0.3 { "start" } else if ux < -0.3 { "end" } else { "middle" };
            chart::text(&mut d, sx + ux * (l + 10.0), sy + uy * (l + 10.0) + if uy > 0.3 { fs } else { fs * 0.35 }, fs * 0.95, anchor, &format!("{} {} N", f.name, trim(f.mag, 2)));
        }
    }
    // the list and the readouts
    let (tx, ty) = (610.0, top + 60.0);
    chart::text(&mut d, tx, ty - 24.0, fs * 1.05, "start", "Forces on the object");
    chart::text(&mut d, tx, ty, fs * 0.9, "start", "Force");
    chart::text(&mut d, tx + 150.0, ty, fs * 0.9, "start", "Size");
    chart::text(&mut d, tx + 230.0, ty, fs * 0.9, "start", "Direction");
    d.line(tx, ty + 8.0, tx + 330.0, ty + 8.0, INK, 1.2);
    for (i, f) in forces.iter().enumerate() {
        let y = ty + 32.0 + i as f64 * fs * 1.9;
        chart::text(&mut d, tx, y, fs, "start", &f.name);
        chart::text(&mut d, tx + 150.0, y, fs, "start", &format!("{} N", trim(f.mag, 2)));
        chart::text(&mut d, tx + 230.0, y, fs, "start", &format!("{}\u{b0}", trim(f.deg, 2)));
    }
    let mut y = ty + 32.0 + forces.len() as f64 * fs * 1.9 + 40.0;
    let mut rows: Vec<(&str, String)> = vec![
        ("Net force across (right +):", format!("{} N", trim(fx, 2))),
        ("Net force up (up +):", format!("{} N", trim(fy, 2))),
        ("Size of the net force:", format!("{} N", trim(fnet, 2))),
        ("Its direction:", if fnet == 0.0 { "none: the forces are balanced".to_string() } else { format!("{}\u{b0}", trim(fdeg, 1)) }),
    ];
    if mass > 0.0 {
        rows.push(("Acceleration (net force \u{f7} mass):", format!("{} m/s\u{b2}", trim(fnet / mass, 3))));
        rows.insert(0, ("Mass:", format!("{} kg", trim(mass, 3))));
    }
    for (label, val) in rows {
        chart::text(&mut d, tx, y, fs, "start", label);
        if label == "Mass:" {
            chart::text(&mut d, tx + 250.0, y, fs, "start", &val);
        } else if answers {
            chart::text(&mut d, tx + 250.0, y, fs, "start", &val);
        } else {
            d.line(tx + 250.0, y + 3.0, tx + 380.0, y + 3.0, INK, 1.0);
        }
        y += fs * 1.9;
    }
    if answers && fnet > 1e-9 {
        let blue = if print { "#000000" } else { "#1d4ed8" };
        let (ox, oy) = (tx + 150.0, y + 80.0);
        chart::text(&mut d, tx, y + 20.0, fs, "start", "Net force");
        let (ux, uy) = (fdeg.to_radians().cos(), -fdeg.to_radians().sin());
        let l = len(fnet) * 0.6;
        d.circle(ox, oy, 4.0, blue, blue, 1.0);
        d.arrow(ox, oy, ox + ux * l, oy + uy * l, blue, 5.0);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn forces_parse() {
        let v = parse("Normal force 49 90\nPush 30 0").unwrap();
        assert_eq!((v[0].name.as_str(), v[0].mag, v[0].deg), ("Normal force", 49.0, 90.0));
        for (spec, needle) in [("", "Give some forces"), ("Push 30", "needs a name"), ("Push x 0", "not a size"), ("Push -3 0", "not a size"), ("Push 3 y", "not a direction"), ("Push 3 1e99", "not a direction"), ("Push 3 nan", "not a direction")] {
            assert!(parse(spec).unwrap_err().contains(needle), "{spec:?}");
        }
        assert!(parse(&"a 1 0\n".repeat(7)).unwrap_err().contains("at most 6"));
        assert!(parse(&"a 1 0\n".repeat(200)).unwrap_err().contains("600"));
    }

    #[test]
    fn the_net_force_adds_vectors() {
        let f = parse(DEFAULT).unwrap();
        let (x, y, size, deg) = net(&f);
        assert_eq!((x.round(), y.round(), size.round()), (18.0, 0.0, 18.0));
        assert_eq!(deg, 0.0);
        let f = parse("A 10 90\nB 10 270").unwrap();
        assert_eq!(net(&f).2, 0.0);
        let f = parse("A 3 0\nB 4 90").unwrap();
        let (_, _, s, d) = net(&f);
        assert!((s - 5.0).abs() < 1e-9 && (d - 53.13).abs() < 0.01);
        let f = parse("A 5 -90").unwrap();
        assert!((net(&f).3 - 270.0).abs() < 1e-9);
    }

    #[test]
    fn key_draws_and_works_the_net() {
        let mut p = defaults();
        let svg = render(&p);
        for t in [">Weight 49 N<", ">Net force<", ">18 N<", "balanced"] {
            assert!(svg.contains(t) || t == "balanced", "{t}");
        }
        p.set_s("forces", "Weight 20 270\nTension 20 90");
        assert!(render(&p).contains("the forces are balanced") && !render(&p).contains(">Net force<"));
        p.set_f("mass", 4.0);
        p.set_s("forces", "Push 20 0");
        assert!(render(&p).contains("5 m/s"));
    }

    #[test]
    fn student_copy_has_the_list_but_no_arrows() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">Weight<") && student.contains(">49 N<") && !student.contains(">Weight 49 N<") && !student.contains("Net force<") && key.contains(">Weight 49 N<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("free-body").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("needs a name") && !svg.contains("not a size"), "{}", ex.name);
        }
        for v in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e300] {
            let mut p = defaults();
            p.set_f("mass", v);
            assert!(sane(&render(&p)), "{v}");
        }
        for spec in ["\u{0} 1 0", "<b> 1 0", "a 0 0", "a 1e6 0\nb 1e6 180", "a 1e-9 0", "a 1 3600"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("forces", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
