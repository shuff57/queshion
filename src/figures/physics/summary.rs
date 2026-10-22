//! The reference's collapsed-row summaries for the physics figures that have no panel of their
//! own (free-body, vector, pulley, projectile, motion graphs, spring scale, circuit), worded as
//! the live page words them: lower case, curly-quoted labels, "none" for an empty section.
use super::instr::comp_name;
use crate::figures::Params;

const DOT: &str = " \u{b7} ";

fn quoted(mode: &str, text: &str) -> Option<String> {
    match mode {
        "None" => None,
        "Blank line" => Some("blank line".into()),
        _ => Some(format!("\u{201c}{}\u{201d}", text.trim())),
    }
}

fn or_none(v: Vec<String>, sep: &str) -> String {
    if v.is_empty() { "none".into() } else { v.join(sep) }
}

/// 3.470 -> "3.47", 5.0 -> "5".
fn trim(v: f64, places: usize) -> String {
    let t = format!("{:.*}", places, v);
    let t = if t.contains('.') { t.trim_end_matches('0').trim_end_matches('.').to_string() } else { t };
    t.replace('-', "\u{2212}")
}

fn subscript(s: &str) -> String {
    match s.split_once('_') {
        Some((b, d)) => {
            let d = d.trim_matches(|c| c == '{' || c == '}');
            let sub: String = d.chars().map(|c| match c {
                '0'..='9' => char::from_u32(0x2080 + c as u32 - '0' as u32).unwrap_or(c),
                _ => c,
            }).collect();
            format!("{b}{sub}")
        }
        None => s.to_string(),
    }
}

pub(super) fn summary(id: &str, title: &str, p: &Params) -> Option<String> {
    Some(match (id, title) {
        ("free-body-diagram", "Body") => p.s("body").to_lowercase(),

        ("vector-diagram", "Resultant") => {
            if !p.b("resultant") || p.s("res_style") == "Left off" {
                return Some("none".into());
            }
            let (mut x, mut y) = (0.0f64, 0.0f64);
            for i in 1..=p.i("n").clamp(1, 4) {
                let (m, a) = (p.f(&format!("v{i}_mag")), p.f(&format!("v{i}_ang")).to_radians());
                x += m * a.cos();
                y += m * a.sin();
            }
            let m = (x * x + y * y).sqrt();
            let a = y.atan2(x).to_degrees().rem_euclid(360.0);
            let a = if (a - 360.0).abs() < 0.05 { 0.0 } else { a };
            format!("{} {} at {}\u{b0}", trim(m, 1), if (m - 1.0).abs() < 0.05 { "square" } else { "squares" }, trim(a, 1))
        }
        ("vector-diagram", "Grid and axes") => {
            let mut s = p.s("grid").to_lowercase();
            if p.b("axes") && p.s("grid") != "Axes only" {
                s.push_str(&format!("{DOT}x and y axes"));
            }
            s
        }

        ("pulley", "Setup") => match p.s("setup") {
            "Atwood" => "Atwood machine".into(),
            "Block & tackle" => "block and tackle".into(),
            s => s.to_lowercase(),
        },
        ("pulley", "Objects") => {
            let mut v: Vec<String> = Vec::new();
            v.extend(quoted(p.s("l_lab_mode"), p.s("l_lab")));
            v.extend(quoted(p.s("r_lab_mode"), p.s("r_lab")));
            if p.b("third") {
                v.extend(quoted(p.s("t_lab_mode"), p.s("t_lab")));
            }
            match v.len() {
                0 => "none".into(),
                1 => v.remove(0),
                _ => {
                    let last = v.pop().unwrap_or_default();
                    format!("{} and {}", v.join(", "), last)
                }
            }
        }
        ("pulley", "Forces and motion") => {
            let mut v: Vec<String> = Vec::new();
            if p.b("tension") { v.push("tension".into()); }
            if p.b("gravity") { v.push("gravity".into()); }
            if p.b("normal") { v.push("normal force".into()); }
            if p.s("friction") != "None" { v.push("friction".into()); }
            if p.s("accel") != "None" { v.push("acceleration".into()); }
            or_none(v, ", ")
        }

        ("projectile-motion", "Launch") => {
            let mut v = vec![
                format!("{}\u{b0}", trim(p.f("angle"), 1)),
                if p.s("from") == "A cliff" { "off a cliff".to_string() } else { "from the ground".to_string() },
            ];
            if p.b("show_angle") {
                v.extend(quoted(p.s("ang_lab_mode"), p.s("ang_lab")));
            }
            v.join(DOT)
        }
        ("projectile-motion", "Object") => format!("{}{DOT}{}% size", p.s("object").to_lowercase(), p.i("size")),
        ("projectile-motion", "Path") => {
            let mut s = p.s("path").to_lowercase();
            if p.i("steps") > 0 {
                s.push_str(&format!("{DOT}{} time steps", p.i("steps")));
            }
            s
        }
        ("projectile-motion", "Velocity and gravity") => {
            let mut v: Vec<String> = Vec::new();
            if p.b("show_v") {
                v.push(match quoted(p.s("v_lab_mode"), p.s("v_lab")) { Some(q) => format!("velocity {q}"), None => "velocity".into() });
            }
            if p.b("comps") { v.push("components".into()); }
            if p.b("show_g") {
                v.push(match quoted(p.s("g_lab_mode"), p.s("g_lab")) { Some(q) => format!("gravity {q}"), None => "gravity".into() });
            }
            or_none(v, DOT)
        }
        ("projectile-motion", "Marks") => {
            let mut v: Vec<String> = Vec::new();
            for (on, name, key) in [("mark_h", "maximum height", "h_lab"), ("mark_r", "range", "r_lab"), ("mark_cliff", "cliff height", "cliff_lab")] {
                if p.b(on) {
                    v.push(format!("{name} \u{201c}{}\u{201d}", p.s(key).trim()));
                }
            }
            or_none(v, DOT)
        }

        ("motion-graphs", "Motion") => {
            let n = p.i("n").clamp(1, 6) as usize;
            (1..=n)
                .map(|i| match p.s(&format!("s{i}_kind")) {
                    "At rest" => "rest".to_string(),
                    "Constant velocity forward" => "forward".to_string(),
                    "Constant velocity back" => "back".to_string(),
                    k => k.to_lowercase(),
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
        ("motion-graphs", "Graphs") => p.s("show").replace('-', "\u{2013}"),
        ("motion-graphs", "Marks") => {
            let mut v: Vec<String> = Vec::new();
            if p.b("letters") { v.push("lettered".into()); }
            if p.b("styles") { v.push("line styles".into()); }
            if p.b("tangent") { v.push("tangent".into()); }
            or_none(v, DOT)
        }
        ("motion-graphs", "Titles") => {
            let show = p.s("show");
            let mut v: Vec<String> = Vec::new();
            if p.s("title_mode") != "None" && !p.s("title").trim().is_empty() {
                v.extend(quoted(p.s("title_mode"), p.s("title")));
            }
            v.extend(quoted(p.s("xt_mode"), if p.s("xt").trim().is_empty() { "Time (s)" } else { p.s("xt") }));
            let all = show.starts_with("All");
            if all || show.starts_with("Position") { v.extend(quoted(p.s("yt_pos_mode"), p.s("yt_pos"))); }
            if all || show.starts_with("Velocity") { v.extend(quoted(p.s("yt_vel_mode"), p.s("yt_vel"))); }
            if all || show.starts_with("Acceleration") { v.extend(quoted(p.s("yt_acc_mode"), p.s("yt_acc"))); }
            v.join(DOT)
        }
        ("motion-graphs", "Axes and grid") => [
            if p.b("ranges") { "set by hand" } else { "fitted to the motion" },
            if p.b("numbers") { "numbered" } else { "no numbers" },
            if p.b("grid") { "gridlines" } else { "no gridlines" },
        ]
        .join(DOT),

        ("spring-scale", "Spring scale") => {
            let cap = p.s("cap").trim_end_matches(" N").trim().parse::<f64>().unwrap_or(10.0);
            let mut s = format!("{} N ({} g)", trim(cap, 1), trim(cap * 100.0, 0));
            if p.b("coded") {
                let colour = match (cap * 10.0).round() as i64 {
                    10 => "pink",
                    25 => "blue",
                    50 => "green",
                    100 => "tan",
                    200 => "red",
                    300 => "white",
                    _ => "yellow",
                };
                s.push_str(&format!(", {colour}"));
            }
            s.push_str(match p.s("units") { "Grams" => ", g", "Both" => ", N and g", _ => ", N" });
            s
        }
        ("spring-scale", "Reading") => format!("{} N", trim(p.f("reading"), 2)),
        ("spring-scale", "On the hook") => p.s("hook").to_string(),
        ("spring-scale", "Magnifier") => p.s("view").to_string(),
        ("spring-scale", "Title and answer key") => {
            let t = p.s("title").trim();
            format!("{}, {}", if t.is_empty() { "No title".to_string() } else { format!("Title \u{201c}{t}\u{201d}") }, if p.b("answer") { "answer key" } else { "no answer key" })
        }

        ("motion-map", "Motion") => {
            let mut v: Vec<&str> = Vec::new();
            if p.f("up") > 0.0 { v.push("speeding up"); }
            if p.f("cruise") > 0.0 { v.push(if p.s("dir") == "Back" { "back" } else { "forward" }); }
            if p.f("down") > 0.0 { v.push("slowing down"); }
            if p.f("rest") > 0.0 { v.push("rest"); }
            or_none(v.into_iter().map(String::from).collect(), ", ")
        }
        ("motion-map", "Dots") => format!("every {} s", trim(p.f("dt"), 2)),
        ("motion-map", "Arrows") => if p.s("arrows") == "Velocity" { "velocity".into() } else { "none".into() },
        ("motion-map", "Number line") => if p.b("numbers") { "numbered".into() } else { "not numbered".into() },
        ("motion-map", "Title and answer key") => {
            let t = p.s("title").trim();
            format!("{}{DOT}no answer key", if t.is_empty() { "no title".to_string() } else { format!("title \u{201c}{t}\u{201d}") })
        }
        ("ray-diagram", "Lens or mirror") => format!("{}{DOT}f = {} cm", p.s("kind"), trim(p.f("f"), 1)),
        ("ray-diagram", "Object") => format!("{} cm away{DOT}{} cm tall", trim(p.f("dist"), 1), trim(p.f("height"), 1)),
        ("ray-diagram", "Rays and image") => format!("{}{DOT}{} of 3 rays", if p.b("image") { "Complete" } else { "Rays only" }, p.i("rays").clamp(1, 3)),
        ("ray-diagram", "Labels") => {
            if !p.b("show_labels") { return Some("none".into()); }
            let v: Vec<String> = ["lab_f", "lab_f2", "lab_2f", "lab_2f2", "lab_o", "lab_i"].iter().filter_map(|k| { let t = p.s(k).trim(); (!t.is_empty()).then(|| format!("\u{201c}{t}\u{201d}")) }).collect();
            or_none(v, DOT)
        }
        ("ray-diagram", "Title and answer key") => {
            let t = p.s("title").trim();
            format!("{}, no answer key", if t.is_empty() { "No title".to_string() } else { format!("Title \u{201c}{t}\u{201d}") })
        }

        ("circuit-diagram", "Circuit") => {
            let n = p.i("n").clamp(1, 4) as usize;
            let names: Vec<String> = (0..n).map(|i| comp_name(p, i, n)).filter(|s| s != "none" && !s.is_empty()).map(|s| subscript(&s)).collect();
            let mut v = vec![format!("{} {}", n, if p.s("layout") == "In parallel" { "in parallel" } else { "in series" })];
            if !names.is_empty() {
                v.push(names.join(", "));
            }
            v.join(DOT)
        }
        ("circuit-diagram", "Cell or battery") => {
            let mut v = vec![if p.s("cell") == "One cell" { "Cell".to_string() } else { "Battery".to_string() }];
            if p.s("cell_val_mode") == "Text" && !p.s("cell_val").trim().is_empty() {
                v.push(p.s("cell_val").trim().to_string());
            }
            v.join(DOT)
        }
        ("circuit-diagram", "Switch and meters") => {
            let mut v: Vec<String> = Vec::new();
            match p.s("switch") {
                "Closed" => v.push("switch closed".into()),
                "Open" => v.push("switch open".into()),
                _ => {}
            }
            if p.b("ammeter") { v.push("ammeter".into()); }
            if p.s("voltmeter") != "None" { v.push("voltmeter".into()); }
            or_none(v, DOT)
        }
        ("circuit-diagram", "Symbols") => if p.s("style").starts_with("IEC") { "IEC symbols".into() } else { "US symbols".into() },
        ("circuit-diagram", "Chart title") => {
            if p.s("title_mode") == "Text" && !p.s("title").trim().is_empty() { format!("\u{201c}{}\u{201d}", p.s("title").trim()) } else { "none".into() }
        }
        _ => return None,
    })
}
