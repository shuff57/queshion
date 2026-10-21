//! A structure chart of boxes and arrows: a government's branches, a trade route's stages, any set
//! of things linked by labelled arrows. The boxes and the links are the question; what each arrow
//! says is the answer, so the student copy (Show answers off) draws the same arrows with blank
//! labels to write on.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use super::wrap;

const INK: &str = "#111827";
const MAX_BOXES: usize = 10;
const MAX_LINKS: usize = 14;
const DEFAULT: &str = "Congress -> President : sends bills to be signed\nPresident -> Congress : can veto bills\nPresident -> Supreme Court : appoints judges\nSupreme Court -> Congress : can strike down laws\nCongress -> Supreme Court : confirms judges";

pub fn def() -> FigureDef {
    FigureDef {
        id: "structure-chart",
        subject: Subject::Language,
        title: "Structure chart",
        blurb: "Boxes joined by labelled arrows, for a government, a trade route or any system, with the arrow labels shown or left blank for students to write.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "structure-chart" => vec![
            Example { name: "Checks and balances", params: vec![] },
            Example { name: "A trade route", params: vec![("links", s("Farmers -> Merchants : sell grain\nMerchants -> Port : ship it\nPort -> Foreign traders : sell abroad\nForeign traders -> Port : bring spices\nPort -> Merchants : unload spices\nMerchants -> Town : sell spices"))] },
            Example { name: "A chain", params: vec![("links", s("Sun -> Plants : light\nPlants -> Rabbits : eaten by\nRabbits -> Foxes : eaten by"))] },
            Example { name: "Blank labels", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("links", "Links: from -> to : what the arrow means", DEFAULT).hint("One link per line, as Box A -> Box B : label. The label is optional. Up to 10 boxes and 14 links"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the boxes and arrows and leaves the labels blank for students to write"),
    ]
}

#[derive(Debug)]
pub struct Link {
    pub from: usize,
    pub to: usize,
    pub label: String,
}

pub fn parse(spec: &str) -> Result<(Vec<String>, Vec<Link>), String> {
    if spec.chars().count() > 1500 {
        return Err("The links are limited to 1500 characters".into());
    }
    let mut names: Vec<String> = Vec::new();
    let mut links = Vec::new();
    let id = |n: &str, names: &mut Vec<String>| -> usize {
        let n: String = n.trim().chars().take(24).collect();
        names.iter().position(|x| *x == n).unwrap_or_else(|| { names.push(n); names.len() - 1 })
    };
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (pair, label) = line.split_once(':').map_or((line, ""), |(a, b)| (a, b.trim()));
        let (a, b) = pair.split_once("->").ok_or_else(|| format!("\u{201c}{}\u{201d} needs an arrow: Box A -> Box B", line.chars().take(24).collect::<String>()))?;
        if a.trim().is_empty() || b.trim().is_empty() {
            return Err("Each link needs a box on both sides of the arrow".into());
        }
        let (f, t) = (id(a, &mut names), id(b, &mut names));
        if f == t {
            return Err("A link needs two different boxes".into());
        }
        links.push(Link { from: f, to: t, label: label.chars().take(60).collect() });
        if names.len() > MAX_BOXES {
            return Err(format!("A chart is limited to {MAX_BOXES} boxes"));
        }
        if links.len() > MAX_LINKS {
            return Err(format!("A chart is limited to {MAX_LINKS} links"));
        }
    }
    if links.is_empty() {
        return Err("Give some links, such as Congress -> President : sends bills".into());
    }
    Ok((names, links))
}

/// A layer for each box: the longest chain of arrows leading to it, ignoring arrows that would close a loop.
pub fn layers(n: usize, links: &[Link]) -> Vec<usize> {
    // an arrow that points back at a box on the current walk closes a loop
    fn visit(u: usize, links: &[Link], state: &mut [u8], back: &mut [bool]) {
        state[u] = 1;
        for (i, l) in links.iter().enumerate().filter(|(_, l)| l.from == u) {
            match state[l.to] {
                1 => back[i] = true,
                0 => visit(l.to, links, state, back),
                _ => {}
            }
        }
        state[u] = 2;
    }
    let mut state = vec![0u8; n];
    let mut back = vec![false; links.len()];
    let mut has_in = vec![false; n];
    for l in links {
        has_in[l.to] = true;
    }
    for u in (0..n).filter(|u| !has_in[*u]).chain(0..n) {
        if state[u] == 0 {
            visit(u, links, &mut state, &mut back);
        }
    }
    let mut depth = vec![0usize; n];
    for _ in 0..n {
        for (i, l) in links.iter().enumerate() {
            if !back[i] && depth[l.to] < depth[l.from] + 1 {
                depth[l.to] = depth[l.from] + 1;
            }
        }
    }
    depth
}

/// Where the line from a box's centre towards (tx, ty) leaves the box.
fn edge(cx: f64, cy: f64, hw: f64, hh: f64, tx: f64, ty: f64) -> (f64, f64) {
    let (dx, dy) = (tx - cx, ty - cy);
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return (cx, cy);
    }
    let s = (hw / dx.abs().max(1e-9)).min(hh / dy.abs().max(1e-9));
    (cx + dx * s, cy + dy * s)
}

fn render(p: &Params) -> String {
    let (names, links) = match parse(p.s("links")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let depth = layers(names.len(), &links);
    let nl = depth.iter().max().unwrap() + 1;
    let widest = (0..nl).map(|l| depth.iter().filter(|d| **d == l).count()).max().unwrap();
    let (bw, bh) = (170.0, 54.0);
    let (w, rowgap) = ((widest as f64 * (bw + 70.0) + 60.0).max(820.0), 175.0);
    let h = top + 60.0 + nl as f64 * rowgap;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let mut pos = vec![(0.0, 0.0); names.len()];
    for l in 0..nl {
        let members: Vec<usize> = (0..names.len()).filter(|i| depth[*i] == l).collect();
        for (k, &i) in members.iter().enumerate() {
            // a lone box in a layer zigzags left and right so arrows that skip a layer miss the box between
            let x = if members.len() == 1 { w * (0.22 + 0.56 * (l % 2) as f64) } else { (k as f64 + 0.5) * w / members.len() as f64 };
            pos[i] = (x, top + 60.0 + l as f64 * rowgap);
        }
    }
    let fills = if print { ["#eeeeee", "#dddddd"] } else { ["#dbe7fb", "#e1f2e3"] };
    // arrows first, so boxes sit on top of their ends; two-way pairs are bowed apart
    for l in &links {
        let (a, b) = (pos[l.from], pos[l.to]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let (ux, uy) = (-dy / len, dx / len);
        // a two-way pair curves apart, each arrow to its own side; a single arrow is straight
        let twoway = links.iter().any(|m| m.from == l.to && m.to == l.from);
        let bow = if twoway { 80.0 } else { 0.0 };
        let c = ((a.0 + b.0) / 2.0 + ux * bow, (a.1 + b.1) / 2.0 + uy * bow);
        let s0 = edge(a.0, a.1, bw / 2.0, bh / 2.0, c.0, c.1);
        let s1 = edge(b.0, b.1, bw / 2.0 + 5.0, bh / 2.0 + 5.0, c.0, c.1);
        d.path(&format!("M {:.2} {:.2} Q {:.2} {:.2} {:.2} {:.2}", s0.0, s0.1, c.0, c.1, s1.0, s1.1), "none", INK, 2.0);
        // the head points along the end of the curve
        let (hx, hy) = (s1.0 - c.0, s1.1 - c.1);
        let hl = (hx * hx + hy * hy).sqrt().max(1e-6);
        let (hx, hy) = (hx / hl, hy / hl);
        d.poly(&[s1, (s1.0 - hx * 12.0 - hy * 5.0, s1.1 - hy * 12.0 + hx * 5.0), (s1.0 - hx * 12.0 + hy * 5.0, s1.1 - hy * 12.0 - hx * 5.0)], INK, INK, 0.5);
        // the label sits on its arrow, a little off the middle for a two-way pair so the two do not meet
        let lines = wrap(&l.label, 17, 3);
        let lw = 124.0;
        let lh = lines.len().max(1) as f64 * fs * 1.2 + 8.0;
        let t = if twoway { 0.36 } else { 0.5 };
        let (mx, my) = (
            (1.0 - t) * (1.0 - t) * s0.0 + 2.0 * t * (1.0 - t) * c.0 + t * t * s1.0,
            (1.0 - t) * (1.0 - t) * s0.1 + 2.0 * t * (1.0 - t) * c.1 + t * t * s1.1,
        );
        d.rect(mx - lw / 2.0, my - lh / 2.0, lw, lh, "#fff", "#9ca3af", 1.0);
        if answers {
            for (k, t) in lines.iter().enumerate() {
                chart::text(&mut d, mx, my - (lines.len() as f64 - 1.0) * fs * 0.6 + fs * 0.33 + k as f64 * fs * 1.2, fs * 0.85, "middle", t);
            }
        }
    }
    for (i, n) in names.iter().enumerate() {
        let (cx, cy) = pos[i];
        d.raw(&format!(r##"<rect x="{:.2}" y="{:.2}" width="{bw}" height="{bh}" rx="8" fill="{}" stroke="{INK}" stroke-width="1.8"/>"##, cx - bw / 2.0, cy - bh / 2.0, fills[depth[i] % 2]));
        let lines = wrap(n, 19, 2);
        for (k, t) in lines.iter().enumerate() {
            chart::text(&mut d, cx, cy + fs * 0.35 - (lines.len() as f64 - 1.0) * fs * 0.6 + k as f64 * fs * 1.2, fs, "middle", t);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn links_parse_and_share_boxes() {
        let (names, links) = parse("A -> B : go\nB -> C\nA -> C : skip").unwrap();
        assert_eq!(names, vec!["A", "B", "C"]);
        assert_eq!(links.len(), 3);
        assert_eq!((links[0].from, links[0].to, links[0].label.as_str()), (0, 1, "go"));
        assert_eq!(links[1].label, "");
        for (spec, needle) in [("", "Give some links"), ("A B", "needs an arrow"), ("-> B", "both sides"), ("A -> A", "two different"), ("A ->", "both sides")] {
            assert!(parse(spec).unwrap_err().contains(needle), "{spec:?}");
        }
        assert!(parse(&(0..11).map(|i| format!("a{i} -> z\n")).collect::<String>()).unwrap_err().contains("limited to 10 boxes"));
        assert!(parse(&"A -> B\n".repeat(15)).unwrap_err().contains("limited to 14 links"));
    }

    #[test]
    fn layers_follow_the_arrows_and_survive_loops() {
        let (n, l) = parse("A -> B\nB -> C\nA -> C").unwrap();
        assert_eq!(layers(n.len(), &l), vec![0, 1, 2]);
        let (n, l) = parse("A -> B\nB -> C\nC -> A").unwrap();
        assert_eq!(layers(n.len(), &l), vec![0, 1, 2], "the arrow that closes the loop is ignored");
        let (n, l) = parse(DEFAULT).unwrap();
        let d = layers(n.len(), &l);
        assert_eq!(d.len(), 3);
        assert!(d.iter().all(|x| *x < 3));
    }

    #[test]
    fn key_labels_and_student_blanks() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(key.contains("can veto bills") && !student.contains("can veto bills") && student.contains(">Congress<"));
        assert_eq!(student.matches("<polygon").count(), key.matches("<polygon").count());
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
        for ex in examples("structure-chart").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("needs an arrow") && !svg.contains("limited"), "{}", ex.name);
        }
        for spec in ["\u{0} -> x", "<b> -> <i> : <u>", "nan -> nan2 : nan", "a -> b -> c", "a->b:c:d", &format!("{} -> b", "w".repeat(500))] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("links", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>") && !svg.contains("<i>"), "{spec:?}");
            }
        }
    }
}
