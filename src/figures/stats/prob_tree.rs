//! Probability tree: up to three stages of branches with their probabilities, and the probability
//! of each whole path (the product along it) worked out on the right. The student copy (Show
//! answers off) keeps the tree and leaves the path probabilities as blanks, and can leave the
//! branch probabilities blank too.
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_BRANCHES: usize = 5;
const MAX_LEAVES: usize = 30;

pub fn def() -> FigureDef {
    FigureDef {
        id: "probability-tree",
        subject: Subject::Math,
        title: "Probability tree",
        blurb: "A probability tree of up to three stages with the probability on each branch and the probability of every whole path worked out, or left blank for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("probability-tree", "second") => true,
        ("probability-tree", "third") => !p.s("second").trim().is_empty(),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "probability-tree" => vec![
            Example { name: "Rain and being late", params: vec![] },
            Example {
                name: "Different second stage",
                params: vec![("first", s("Red 3/5, Blue 2/5")), ("second", s("Red 2/4, Blue 2/4\nRed 3/4, Blue 1/4")), ("title", s("Two draws without replacement"))],
            },
            Example { name: "Three coin flips", params: vec![("first", s("H 1/2, T 1/2")), ("second", s("H 1/2, T 1/2")), ("third", s("H 1/2, T 1/2"))] },
            Example { name: "Blank branches", params: vec![("blank_probs", Value::B(true))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("first", "First stage", "Rain 0.3, Dry 0.7").hint("Outcome and probability, separated by commas: Rain 0.3, Dry 0.7. Probabilities may be 0.3, 30% or 3/10"),
        ParamDef::long("second", "Second stage", "Late 0.4, On time 0.6\nLate 0.1, On time 0.9").hint("One line is used after every first-stage outcome; give one line per outcome, in order, when they differ"),
        ParamDef::text("third", "Third stage (after every second-stage outcome)", "").hint("Optional"),
        ParamDef::bool("products", "Work out each path", true),
        ParamDef::bool("blank_probs", "Leave the branch probabilities blank in the student copy", false),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the tree and leaves the path probabilities (and, if ticked, the branch probabilities) blank for students"),
    ]
}

struct Branch {
    name: String,
    text: String,
    p: f64,
}

fn prob(t: &str) -> Option<f64> {
    let t = t.trim();
    let v = if let Some(x) = t.strip_suffix('%') {
        x.trim().parse::<f64>().ok()? / 100.0
    } else if let Some((a, b)) = t.split_once('/') {
        let (a, b) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
        if b == 0.0 { return None; }
        a / b
    } else {
        t.parse::<f64>().ok()?
    };
    (v.is_finite() && (0.0..=1.0).contains(&v)).then_some(v)
}

fn branches(line: &str, what: &str) -> Result<Vec<Branch>, String> {
    let mut out = Vec::new();
    for item in line.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, p) = item.rsplit_once(char::is_whitespace).ok_or_else(|| format!("{what}: \u{201c}{item}\u{201d} needs an outcome and a probability, like Rain 0.3"))?;
        let value = prob(p).ok_or_else(|| format!("{what}: \u{201c}{p}\u{201d} is not a probability between 0 and 1"))?;
        let name = name.trim();
        if name.chars().count() > 14 {
            return Err(format!("{what}: \u{201c}{name}\u{201d} is longer than 14 characters"));
        }
        out.push(Branch { name: name.to_string(), text: p.trim().to_string(), p: value });
    }
    if out.is_empty() {
        return Err(format!("{what} needs at least one outcome"));
    }
    if out.len() > MAX_BRANCHES {
        return Err(format!("{what} has more than {MAX_BRANCHES} outcomes"));
    }
    Ok(out)
}

struct Node {
    label: String,
    p: Option<(String, f64)>,
    kids: Vec<usize>,
    depth: usize,
}

struct Tree {
    nodes: Vec<Node>,
    warnings: Vec<String>,
}

fn check_sum(bs: &[Branch], where_: &str, w: &mut Vec<String>) {
    let sum: f64 = bs.iter().map(|b| b.p).sum();
    if (sum - 1.0).abs() > 1e-6 {
        let msg = format!("The probabilities {where_} add to {}, not 1", chart::fmt((sum * 10000.0).round() / 10000.0));
        if !w.contains(&msg) {
            w.push(msg);
        }
    }
}

fn build(p: &Params) -> Result<Tree, String> {
    let first = branches(p.s("first"), "First stage")?;
    let lines: Vec<&str> = p.s("second").lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let third_line = p.s("third").trim();
    let mut t = Tree { nodes: vec![Node { label: String::new(), p: None, kids: vec![], depth: 0 }], warnings: vec![] };
    check_sum(&first, "at the first stage", &mut t.warnings);
    let second: Vec<Vec<Branch>> = if lines.is_empty() {
        vec![]
    } else if lines.len() == 1 {
        let b = branches(lines[0], "Second stage")?;
        check_sum(&b, "at the second stage", &mut t.warnings);
        (0..first.len()).map(|_| b.iter().map(|x| Branch { name: x.name.clone(), text: x.text.clone(), p: x.p }).collect()).collect()
    } else if lines.len() == first.len() {
        lines.iter().enumerate().map(|(i, l)| {
            let b = branches(l, "Second stage")?;
            check_sum(&b, &format!("after {}", first[i].name), &mut t.warnings);
            Ok(b)
        }).collect::<Result<_, String>>()?
    } else {
        return Err(format!("Give one second-stage line, or one for each of the {} first-stage outcomes (there are {})", first.len(), lines.len()));
    };
    let third = if third_line.is_empty() || second.is_empty() { None } else { Some(branches(third_line, "Third stage")?) };
    if let Some(b) = &third {
        check_sum(b, "at the third stage", &mut t.warnings);
    }
    let leaves = first.len() * second.first().map_or(1, |_| second.iter().map(Vec::len).sum::<usize>() / first.len().max(1)).max(1) * third.as_ref().map_or(1, Vec::len);
    let leaf_count = if second.is_empty() { first.len() } else { second.iter().map(|s| s.len() * third.as_ref().map_or(1, Vec::len)).sum() };
    let _ = leaves;
    if leaf_count > MAX_LEAVES {
        return Err(format!("That makes {leaf_count} paths; the figure is limited to {MAX_LEAVES}"));
    }
    for (i, b) in first.iter().enumerate() {
        t.nodes.push(Node { label: b.name.clone(), p: Some((b.text.clone(), b.p)), kids: vec![], depth: 1 });
        let a = t.nodes.len() - 1;
        t.nodes[0].kids.push(a);
        for b2 in second.get(i).into_iter().flatten() {
            t.nodes.push(Node { label: b2.name.clone(), p: Some((b2.text.clone(), b2.p)), kids: vec![], depth: 2 });
            let c = t.nodes.len() - 1;
            t.nodes[a].kids.push(c);
            for b3 in third.iter().flatten() {
                t.nodes.push(Node { label: b3.name.clone(), p: Some((b3.text.clone(), b3.p)), kids: vec![], depth: 3 });
                let e = t.nodes.len() - 1;
                t.nodes[c].kids.push(e);
            }
        }
    }
    Ok(t)
}

fn fmt4(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "0" && v > 0.0 { if v > 0.0 { "< 0.0001".into() } else { "0".into() } } else { s.to_string() }
}

fn render(p: &Params) -> String {
    let t = match build(p) {
        Ok(t) => t,
        Err(e) => return chart::message(&e),
    };
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 15.0 * k;
    let title = p.s("title").trim();
    let n = t.nodes.len();
    let max_depth = t.nodes.iter().map(|x| x.depth).max().unwrap_or(0);
    let tw = |s: &str| s.chars().count() as f64 * fs * 0.56;

    // leaves top to bottom, parents centred over their children
    let mut y = vec![0.0f64; n];
    let mut next = 0.0;
    fn place(t: &Tree, u: usize, y: &mut [f64], next: &mut f64) {
        if t.nodes[u].kids.is_empty() {
            y[u] = *next;
            *next += 1.0;
            return;
        }
        for k in &t.nodes[u].kids {
            place(t, *k, y, next);
        }
        let (a, b) = (t.nodes[u].kids[0], *t.nodes[u].kids.last().unwrap_or(&0));
        y[u] = (y[a] + y[b]) / 2.0;
    }
    place(&t, 0, &mut y, &mut next);
    let leaves: Vec<usize> = (0..n).filter(|i| t.nodes[*i].kids.is_empty() && *i != 0).collect();
    let row = (fs * 3.1).max(46.0);
    let label_w: Vec<f64> = (0..=max_depth).map(|dp| t.nodes.iter().filter(|x| x.depth == dp).map(|x| tw(&x.label)).fold(0.0, f64::max)).collect();
    let gap = 130.0;
    let mut xs = vec![0.0f64; max_depth + 1];
    let mut x = 24.0;
    for dp in 0..=max_depth {
        xs[dp] = x;
        x += label_w[dp] + gap + if dp == 0 { 0.0 } else { 14.0 };
    }
    let prod_on = p.b("products");
    let path_text = |leaf: usize| -> (String, f64) {
        let mut names = Vec::new();
        let mut prod = 1.0;
        let mut cur = leaf;
        loop {
            if let Some((_, pp)) = &t.nodes[cur].p {
                prod *= pp;
                names.push(t.nodes[cur].label.clone());
            }
            match (0..n).find(|i| t.nodes[*i].kids.contains(&cur)) {
                Some(par) => cur = par,
                None => break,
            }
        }
        names.reverse();
        (names.join(", "), prod)
    };
    let prod_w = if prod_on { leaves.iter().map(|l| tw(&path_text(*l).0) + tw(&format!(" = {}", fmt4(path_text(*l).1)))).fold(80.0, f64::max) + 20.0 } else { 0.0 };
    let tree_right = xs[max_depth] + label_w[max_depth] + 16.0;
    let w = (tree_right + prod_w + 24.0).max(320.0);
    let top = if title.is_empty() { 16.0 } else { 38.0 + fs * 1.3 };
    let warn_h = t.warnings.len() as f64 * fs * 1.5;
    let h = top + row * next.max(1.0) + warn_h + 10.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let py = |i: usize| top + row * (y[i] + 0.5);
    let right_edge = |i: usize| if i == 0 { xs[0] + 4.0 } else { xs[t.nodes[i].depth] + label_w[t.nodes[i].depth] + 8.0 };
    let left_edge = |i: usize| xs[t.nodes[i].depth] - 6.0;
    let hide_p = !answers && p.b("blank_probs");
    for (i, nd) in t.nodes.iter().enumerate() {
        for c in &nd.kids {
            let (a, b) = ((right_edge(i), py(i)), (left_edge(*c), py(*c)));
            d.line(a.0, a.1, b.0, b.1, INK, 1.8);
            if let Some((text, _)) = &t.nodes[*c].p {
                let (mx, my) = (a.0 + (b.0 - a.0) * 0.52, a.1 + (b.1 - a.1) * 0.52);
                let tw2 = tw(text).max(26.0) + 10.0;
                d.rect(mx - tw2 / 2.0, my - fs * 0.7, tw2, fs * 1.4, "#fff", if hide_p { INK } else { "none" }, 1.2);
                if !hide_p {
                    chart::text(&mut d, mx, my + fs * 0.35, fs * 0.95, "middle", text);
                }
            }
        }
    }
    d.circle(xs[0] + 2.0, py(0), 4.5, INK, INK, 1.0);
    for (i, nd) in t.nodes.iter().enumerate().skip(1) {
        chart::text(&mut d, xs[nd.depth], py(i) + fs * 0.35, fs, "start", &nd.label);
    }
    if prod_on {
        let px = tree_right + 8.0;
        for l in &leaves {
            let (names, prod) = path_text(*l);
            let yy = py(*l) + fs * 0.35;
            chart::text(&mut d, px, yy, fs * 0.95, "start", &names);
            let after = px + tw(&names) + fs * 0.4;
            if answers {
                chart::text(&mut d, after, yy, fs * 0.95, "start", &format!("= {}", fmt4(prod)));
            } else {
                chart::text(&mut d, after, yy, fs * 0.95, "start", "=");
                d.line(after + fs, yy + 2.0, after + fs * 4.6, yy + 2.0, INK, 1.0);
            }
        }
        d.line(tree_right + 2.0, top, tree_right + 2.0, top + row * next, if print { "#bdbdbd" } else { "#d1d5db" }, 1.0);
    }
    for (i, wmsg) in t.warnings.iter().enumerate() {
        chart::text(&mut d, 24.0, top + row * next + fs * 1.4 * (i as f64 + 1.0), fs * 0.9, "start", &format!("Check: {wmsg}"));
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn probabilities_read_as_decimals_percents_and_fractions() {
        for (t, want) in [("0.3", 0.3), (".3", 0.3), ("30%", 0.3), ("3/10", 0.3), ("1", 1.0), ("0", 0.0), (" 1/3 ", 1.0 / 3.0)] {
            assert!((prob(t).unwrap() - want).abs() < 1e-12, "{t}");
        }
        for t in ["", "x", "1.5", "-0.1", "5/0", "150%", "nan", "inf"] {
            assert!(prob(t).is_none(), "{t:?}");
        }
    }

    #[test]
    fn default_has_four_paths_with_the_right_products() {
        let svg = render(&defaults());
        for t in [">= 0.12<", ">= 0.18<", ">= 0.07<", ">= 0.63<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert!(svg.contains(">Rain, Late<") && svg.contains(">Dry, On time<"));
        assert!(!svg.contains("Check:"), "the defaults add to one");
    }

    #[test]
    fn a_second_line_for_each_first_outcome_and_a_third_stage() {
        let mut p = defaults();
        p.set_s("first", "Red 3/5, Blue 2/5");
        p.set_s("second", "Red 2/4, Blue 2/4\nRed 3/4, Blue 1/4");
        let svg = render(&p);
        assert!(svg.contains(">= 0.3<") && svg.contains(">= 0.1<"), "3/5 x 2/4 and 2/5 x 1/4");
        p.set_s("third", "H 1/2, T 1/2");
        assert_eq!(render(&p).matches(">= 0.").count() + render(&p).matches(">= 1<").count() >= 8, true);
        let b = build(&p).unwrap();
        assert_eq!(b.nodes.iter().filter(|n| n.kids.is_empty() && n.depth == 3).count(), 8);
    }

    #[test]
    fn mistakes_are_explained_and_sums_are_checked() {
        for (first, second, needle) in [
            ("", "", "at least one"),
            ("Rain", "", "needs an outcome and a probability"),
            ("Rain 2", "", "not a probability"),
            ("A 1/2, B 1/2", "x 1/2\ny 1/2\nz 1/2", "one second-stage line"),
            ("A 0.5, B 0.5, C 0, D 0, E 0, F 0", "", "more than 5"),
            ("averyveryverylongname 1", "", "longer than 14"),
        ] {
            let mut p = defaults();
            p.set_s("first", first);
            p.set_s("second", second);
            assert!(render(&p).contains(needle), "{first:?} {second:?}");
        }
        let mut p = defaults();
        p.set_s("first", "A 0.3, B 0.5");
        assert!(render(&p).contains("add to 0.8, not 1"));
        p.set_s("first", "A 1/5, B 1/5, C 1/5, D 1/5, E 1/5");
        p.set_s("second", "x 1/5, y 1/5, z 1/5, u 1/5, v 1/5");
        p.set_s("third", "p 1/2, q 1/2");
        assert!(render(&p).contains("limited to 30"));
    }

    #[test]
    fn student_copy_blanks_the_products_and_optionally_the_branches() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(key.contains(">= 0.12<") && !student.contains(">= 0.12<") && student.contains(">0.3<"));
        assert_eq!(size_of(&student), size_of(&key));
        let mut p = defaults();
        p.set_b("blank_probs", true);
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(!student.contains(">0.3<") && key.contains(">0.3<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("probability-tree").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    _ => {}
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<line") && !svg.contains("NaN"), "{}", ex.name);
        }
        for text in ["", "\u{0}", "a 1", "a 1/0", "a nan, b inf", ",,,", "x 0.5, y 0.5\nz 1", "a 1e-300, b 1"] {
            for third in ["", "t 1"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("first", text);
                    p.set_s("second", text);
                    p.set_s("third", third);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{text:?}");
                }
            }
        }
    }
}
