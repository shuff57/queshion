//! Network topology: a star, ring, bus, mesh or tree of devices, with the topology named and its
//! cable count and main strength and weakness worked out underneath. The student copy (Show
//! answers off) keeps the diagram and leaves the name, cable count and notes empty.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const KINDS: [&str; 5] = ["Star", "Ring", "Bus", "Mesh", "Tree"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "network-topology",
        subject: Subject::Computing,
        title: "Network topology",
        blurb: "A star, ring, bus, mesh or tree network of devices, with the topology named and its cable count, strength and weakness worked out underneath, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("network-topology", "centre") => matches!(p.s("kind"), "Star" | "Tree"),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "network-topology" => vec![
            Example { name: "Star", params: vec![] },
            Example { name: "Ring", params: vec![("kind", s("Ring")), ("devices", Value::I(6))] },
            Example { name: "Bus", params: vec![("kind", s("Bus")), ("devices", Value::I(6))] },
            Example { name: "Full mesh", params: vec![("kind", s("Mesh")), ("devices", Value::I(5))] },
            Example { name: "Tree", params: vec![("kind", s("Tree")), ("devices", Value::I(7))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Topology", &KINDS, "Star"),
        ParamDef::int("devices", "Devices", 2, 12, 6).hint("Computers on the network (a mesh is limited to 8)"),
        ParamDef::text("names", "Device names", "").hint("Separated by commas; empty gives PC 1, PC 2 ..."),
        ParamDef::text("centre", "Central device", "Switch"),
        ParamDef::bool("facts", "Show cables, strength and weakness", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the diagram and leaves the name, the cable count and the notes empty for students"),
    ]
}

fn facts(kind: &str, n: usize) -> (String, &'static str, &'static str) {
    match kind {
        "Ring" => (n.to_string(), "Data travels one way round, so there are no collisions.", "One broken cable or device can stop the whole ring."),
        "Bus" => (format!("{n} drops and one backbone"), "Cheap and easy to extend: just one backbone cable.", "A break in the backbone splits the network, and heavy traffic slows everyone."),
        "Mesh" => ((n * (n - 1) / 2).to_string(), "Very reliable: data can take another route if a link fails.", "The number of cables grows quickly, so it is costly to build."),
        "Tree" => ((n + n.div_ceil(4).clamp(1, 3)).to_string(), "Easy to grow in groups, and a fault is limited to one branch.", "If a switch higher up fails, everything below it is cut off."),
        _ => (n.to_string(), "A broken cable only cuts off one device.", "If the central switch fails the whole network fails."),
    }
}

/// A label with a white halo, so a cable passing behind it does not cut it.
fn halo(d: &mut Doc, x: f64, y: f64, fs: f64, anchor: &str, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}" stroke="#fff" stroke-width="4" stroke-linejoin="round" paint-order="stroke">{}</text>"##,
        svg::esc(s)
    ));
}

fn computer(d: &mut Doc, c: (f64, f64), label: &str, fs: f64, print: bool, above: bool) {
    let (w, h) = (44.0, 30.0);
    d.raw(&format!(
        r##"<rect x="{:.1}" y="{:.1}" width="{w}" height="{h}" rx="4" fill="{}" stroke="{INK}" stroke-width="2"/>"##,
        c.0 - w / 2.0,
        c.1 - h / 2.0,
        if print { "#fff" } else { "#f5f0e6" }
    ));
    d.line(c.0 - 10.0, c.1 + h / 2.0 + 5.0, c.0 + 10.0, c.1 + h / 2.0 + 5.0, INK, 2.0);
    d.line(c.0, c.1 + h / 2.0, c.0, c.1 + h / 2.0 + 5.0, INK, 2.0);
    if above {
        halo(d, c.0, c.1 - h / 2.0 - 6.0, fs * 0.9, "middle", label);
    } else {
        halo(d, c.0, c.1 + h / 2.0 + 5.0 + fs * 1.15, fs * 0.9, "middle", label);
    }
}

fn switch(d: &mut Doc, c: (f64, f64), label: &str, fs: f64, print: bool) {
    let (w, h) = (60.0, 24.0);
    d.raw(&format!(
        r##"<rect x="{:.1}" y="{:.1}" width="{w}" height="{h}" rx="3" fill="{}" stroke="{INK}" stroke-width="2"/>"##,
        c.0 - w / 2.0,
        c.1 - h / 2.0,
        if print { "#d9d9d9" } else { "#ffe9a8" }
    ));
    for i in 0..4 {
        d.circle(c.0 - 18.0 + i as f64 * 12.0, c.1, 2.6, INK, INK, 0.5);
    }
    halo(d, c.0 + w / 2.0 + 8.0, c.1 + fs * 0.3, fs * 0.9, "start", label);
}

fn render(p: &Params) -> String {
    let kind = KINDS.iter().copied().find(|k| *k == p.s("kind")).unwrap_or("Star");
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let mut n = p.i("devices").clamp(2, 12) as usize;
    if kind == "Mesh" && n > 8 {
        return chart::message("A mesh is limited to 8 devices");
    }
    if kind == "Mesh" || kind == "Ring" {
        n = n.max(3);
    }
    let given: Vec<String> = p.s("names").split(',').map(|s| s.trim().chars().take(10).collect::<String>()).filter(|s| !s.is_empty()).collect();
    let name = |i: usize| given.get(i).cloned().unwrap_or_else(|| format!("PC {}", i + 1));
    let centre: String = { let c = p.s("centre").trim(); if c.is_empty() { "Switch".to_string() } else { c.chars().take(12).collect() } };
    let title = p.s("title").trim();
    let top = if title.is_empty() { 10.0 } else { 36.0 + fs * 1.3 };
    let want_facts = p.b("facts");
    let (cables, good, bad) = facts(kind, n);
    let facts_h = if want_facts { fs * 1.8 * 4.0 + 14.0 } else { 0.0 };
    let (dw, dh) = match kind {
        "Bus" => ((n.div_ceil(2) as f64 * 100.0 + 90.0).max(300.0), 220.0),
        "Tree" => ((n.div_ceil(2) as f64 * 96.0 + 140.0).max(320.0), 280.0),
        _ => (380.0, 380.0),
    };
    let w = dw.max(if want_facts { gd::text_w(bad, fs) + 60.0 } else { 0.0 }).max(300.0);
    let h = top + dh + facts_h + 6.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let (cx, cy) = (w / 2.0, top + dh / 2.0);
    let ring_pts = |m: usize, r: f64| -> Vec<(f64, f64)> { (0..m).map(|i| { let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / m as f64; (cx + r * a.cos(), cy + r * a.sin()) }).collect() };
    match kind {
        "Ring" => {
            let pts = ring_pts(n, 140.0);
            for i in 0..n {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                d.line(a.0, a.1, b.0, b.1, INK, 2.0);
            }
            for (i, c) in pts.iter().enumerate() {
                computer(&mut d, *c, &name(i), fs, print, c.1 < cy - 5.0);
            }
        }
        "Mesh" => {
            let pts = ring_pts(n, 140.0);
            for i in 0..n {
                for j in i + 1..n {
                    d.line(pts[i].0, pts[i].1, pts[j].0, pts[j].1, INK, 1.6);
                }
            }
            for (i, c) in pts.iter().enumerate() {
                computer(&mut d, *c, &name(i), fs, print, c.1 < cy - 5.0);
            }
        }
        "Bus" => {
            let y = cy;
            let (x0, x1) = (40.0, w - 40.0);
            d.line(x0, y, x1, y, INK, 3.5);
            d.line(x0, y - 9.0, x0, y + 9.0, INK, 3.5);
            d.line(x1, y - 9.0, x1, y + 9.0, INK, 3.5);
            let cols = n.div_ceil(2);
            for i in 0..n {
                let x = x0 + (x1 - x0) * ((i / 2) as f64 + 0.5) / cols as f64;
                let up = i % 2 == 0;
                let py = if up { y - 70.0 } else { y + 62.0 };
                d.line(x, y, x, if up { py + 15.0 } else { py - 15.0 }, INK, 2.0);
                computer(&mut d, (x, py), &name(i), fs, print, up);
            }
        }
        "Tree" => {
            // a switch at the top, switches under it, computers under those
            let sw = n.div_ceil(4).clamp(1, 3);
            let root = (cx, top + 36.0);
            let per = n.div_ceil(sw);
            let sw_pts: Vec<(f64, f64)> = (0..sw).map(|i| (w * (i as f64 + 0.5) / sw as f64, top + 128.0)).collect();
            for s in &sw_pts {
                d.line(root.0, root.1, s.0, s.1, INK, 2.0);
            }
            let mut idx = 0;
            for (si, s) in sw_pts.iter().enumerate() {
                let cnt = per.min(n - idx.min(n));
                for j in 0..cnt {
                    let spread: f64 = 96.0;
                    let x = s.0 + (j as f64 - (cnt as f64 - 1.0) / 2.0) * spread.min((w / sw as f64 - 20.0) / cnt as f64);
                    let c = (x, top + 232.0);
                    d.line(s.0, s.1, c.0, c.1 - 15.0, INK, 2.0);
                    computer(&mut d, c, &name(idx), fs, print, false);
                    idx += 1;
                }
                let _ = si;
            }
            switch(&mut d, root, &centre, fs, print);
            for s in &sw_pts {
                switch(&mut d, *s, "Switch", fs, print);
            }
        }
        _ => {
            let pts = ring_pts(n, 140.0);
            for c in &pts {
                d.line(cx, cy, c.0, c.1, INK, 2.0);
            }
            for (i, c) in pts.iter().enumerate() {
                computer(&mut d, *c, &name(i), fs, print, c.1 < cy - 5.0);
            }
            switch(&mut d, (cx, cy), &centre, fs, print);
        }
    }
    if want_facts {
        let y0 = top + dh + 8.0;
        let x0 = 28.0;
        let rows: [(String, String); 4] = [
            ("Topology:".into(), format!("{kind}")),
            ("Cables:".into(), cables),
            ("Strength:".into(), good.to_string()),
            ("Weakness:".into(), bad.to_string()),
        ];
        let lab_w = gd::text_w("Weakness:", fs) + 12.0;
        for (i, (l, v)) in rows.iter().enumerate() {
            let y = y0 + fs * 1.8 * (i as f64 + 1.0) - fs * 0.5;
            gd::lines(&mut d, x0, y - fs * 1.65 + fs * 0.35, fs, &[l.clone()]);
            if answers {
                gd::lines(&mut d, x0 + lab_w, y - fs * 1.65 + fs * 0.35, fs, &[v.clone()]);
            } else {
                d.line(x0 + lab_w, y + 2.0, w - 28.0, y + 2.0, INK, 1.0);
            }
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    fn with(kind: &str, n: i64) -> Params {
        let mut p = defaults();
        p.set_s("kind", kind);
        p.set_i("devices", n);
        p
    }

    #[test]
    fn every_topology_draws_every_device() {
        for kind in KINDS {
            let svg = render(&with(kind, 6));
            for i in 1..=6 {
                assert!(svg.contains(&format!(">PC {i}<")), "{kind} PC {i}");
            }
            assert!(svg.contains(&format!(">{kind}<")), "{kind} named");
        }
        assert!(render(&defaults()).contains(">Switch<"));
        let mut p = defaults();
        p.set_s("names", "Anna, Ben");
        let svg = render(&p);
        assert!(svg.contains(">Anna<") && svg.contains(">Ben<") && svg.contains(">PC 3<"));
    }

    #[test]
    fn cable_counts() {
        assert_eq!(facts("Mesh", 5).0, "10");
        assert_eq!(facts("Mesh", 8).0, "28");
        assert_eq!(facts("Star", 6).0, "6");
        assert_eq!(facts("Tree", 7).0, "9", "seven computers and two switches under the top one");
        assert_eq!(facts("Ring", 6).0, "6");
        assert!(render(&with("Mesh", 9)).contains("limited to 8"));
    }

    #[test]
    fn student_copy_leaves_the_notes_empty() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &with("Mesh", 5)).unwrap();
        assert!(student.contains(">Topology:<") && !student.contains(">Mesh<") && key.contains(">Mesh<") && key.contains(">10<"));
        assert!(student.contains(">PC 1<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = with("Tree", 7);
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#f5f0e6") && !svg.contains("#ffe9a8"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("network-topology").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::I(i) => p.set_i(k, *i),
                    _ => {}
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<rect") && !svg.contains("NaN"), "{}", ex.name);
        }
        for kind in KINDS {
            for n in [-3, 0, 1, 2, 3, 7, 12, 99] {
                for names in ["", ",,,", "a very very long device name indeed, b", "\u{0}"] {
                    for answers in [true, false] {
                        let mut p = with(kind, n);
                        p.set_s("names", names);
                        p.set_s("centre", names);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{kind} {n}");
                    }
                }
            }
        }
    }
}
