//! Orthographic views of a stack of cubes: an isometric drawing with the top, front and right
//! views beside it. The stack (a grid of heights) is the question; the three views are the answers,
//! so the student copy (Show answers off) draws the isometric view and three empty grids of the
//! right size.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";
const MAX_SIDE: usize = 5;
const MAX_H: usize = 5;
const DEFAULT: &str = "0 1 0\n2 3 1\n1 1 1";

pub fn def() -> FigureDef {
    FigureDef {
        id: "orthographic",
        subject: Subject::Health,
        title: "Orthographic views",
        blurb: "A stack of cubes drawn from a grid of heights in isometric view, with its top, front and right views, or the isometric drawing and empty grids for students to draw the views.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "orthographic" => vec![
            Example { name: "A small stack", params: vec![] },
            Example { name: "An L shape", params: vec![("heights", s("1 0\n1 0\n1 1"))] },
            Example { name: "Steps", params: vec![("heights", s("1 2 3\n1 2 3\n1 2 3"))] },
            Example { name: "A tower", params: vec![("heights", s("4"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("heights", "Heights of the stacks: one row per line, back row first", DEFAULT).hint("Numbers of cubes, 0 to 5, in a grid up to 5 by 5. The top line is the back, the bottom line the front; left to right is as seen from the front"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the isometric drawing and leaves the top, front and right views empty for students"),
    ]
}

/// The grid of heights, back row first.
pub fn parse(spec: &str) -> Result<Vec<Vec<usize>>, String> {
    if spec.chars().count() > 200 {
        return Err("The heights are limited to 200 characters".into());
    }
    let mut g: Vec<Vec<usize>> = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let row: Result<Vec<usize>, String> = line.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()).map(|t| t.parse::<usize>().ok().filter(|h| *h <= MAX_H).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a height from 0 to {MAX_H}", t.chars().take(12).collect::<String>()))).collect();
        g.push(row?);
        if g.len() > MAX_SIDE {
            return Err(format!("The grid is limited to {MAX_SIDE} rows"));
        }
    }
    let cols = g.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return Err("Give a grid of heights, such as 1 2 on one line".into());
    }
    if cols > MAX_SIDE {
        return Err(format!("The grid is limited to {MAX_SIDE} columns"));
    }
    for r in &mut g {
        r.resize(cols, 0);
    }
    if g.iter().all(|r| r.iter().all(|h| *h == 0)) {
        return Err("Put at least one cube in the grid".into());
    }
    Ok(g)
}

/// The three views as grids of filled cells (rows from the top down): (top, front, right).
pub fn views(g: &[Vec<usize>]) -> (Vec<Vec<bool>>, Vec<Vec<bool>>, Vec<Vec<bool>>) {
    let (rows, cols) = (g.len(), g[0].len());
    let hmax = g.iter().flatten().copied().max().unwrap_or(0);
    let top = g.iter().map(|r| r.iter().map(|h| *h > 0).collect()).collect();
    // front: columns left to right, the tallest stack in each column
    let col_h: Vec<usize> = (0..cols).map(|c| g.iter().map(|r| r[c]).max().unwrap()).collect();
    let front = (0..hmax).map(|lvl| col_h.iter().map(|h| *h >= hmax - lvl).collect()).collect();
    // right: the front is on the left, the back on the right
    let row_h: Vec<usize> = (0..rows).rev().map(|r| *g[r].iter().max().unwrap()).collect();
    let right = (0..hmax).map(|lvl| row_h.iter().map(|h| *h >= hmax - lvl).collect()).collect();
    (top, front, right)
}

fn render(p: &Params) -> String {
    let g = match parse(p.s("heights")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let (rows, cols) = (g.len(), g[0].len());
    let hmax = g.iter().flatten().copied().max().unwrap();
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (1000.0, top + 600.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    // the isometric drawing: x to the right, y towards the viewer, so the front and right faces show
    let s = 38.0;
    let (c30, s30) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let ox = 40.0 + rows as f64 * s * c30;
    let oy = top + 80.0 + hmax as f64 * s;
    let proj = |x: f64, y: f64, z: f64| (ox + (x - y) * s * c30, oy + (x + y) * s * s30 - z * s);
    let (ctop, cleft, cright) = if print { ("#f4f4f4", "#d6d6d6", "#b4b4b4") } else { ("#f6d8a8", "#e3b974", "#c99a4e") };
    chart::text(&mut d, ox + (cols as f64 - rows as f64) * s * c30 / 2.0, top + 34.0, fs * 1.05, "middle", "Isometric view");
    let mut cells: Vec<(usize, usize, usize)> = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            for z in 0..g[r][c] {
                cells.push((r, c, z));
            }
        }
    }
    // far cubes first: a smaller x + y is further away, and lower cubes come before the ones on top
    cells.sort_by_key(|&(r, c, z)| (r + c, z));
    for (r, c, z) in cells {
        let (x, y, zz) = (c as f64, r as f64, z as f64);
        let face = |d: &mut Doc, pts: [(f64, f64); 4], fill: &str| { d.poly(&pts, fill, INK, 1.4); };
        face(&mut d, [proj(x, y, zz + 1.0), proj(x + 1.0, y, zz + 1.0), proj(x + 1.0, y + 1.0, zz + 1.0), proj(x, y + 1.0, zz + 1.0)], ctop);
        face(&mut d, [proj(x, y + 1.0, zz + 1.0), proj(x + 1.0, y + 1.0, zz + 1.0), proj(x + 1.0, y + 1.0, zz), proj(x, y + 1.0, zz)], cleft);
        face(&mut d, [proj(x + 1.0, y, zz + 1.0), proj(x + 1.0, y + 1.0, zz + 1.0), proj(x + 1.0, y + 1.0, zz), proj(x + 1.0, y, zz)], cright);
    }
    // the three views, in the usual arrangement: top above front, right beside front
    let u = 36.0;
    let vx = 560.0;
    let ty = top + 70.0;
    let fy = ty + rows as f64 * u + 56.0;
    let rx = vx + cols as f64 * u + 50.0;
    let (vt, vf, vr) = views(&g);
    let fill = if print { "#d0d0d0" } else { "#cfe0fa" };
    let grid = |d: &mut Doc, label: &str, x: f64, y: f64, cells: &[Vec<bool>], nr: usize, nc: usize, heights: Option<&Vec<Vec<usize>>>| {
        chart::text(d, x + nc as f64 * u / 2.0, y - 12.0, fs * 1.05, "middle", label);
        for r in 0..nr {
            for c in 0..nc {
                let filled = answers && cells[r][c];
                d.rect(x + c as f64 * u, y + r as f64 * u, u, u, if filled { fill } else { "#fff" }, INK, 1.4);
                if let (true, Some(hs)) = (answers && cells[r][c], heights) {
                    chart::text(d, x + c as f64 * u + u / 2.0, y + r as f64 * u + u / 2.0 + fs * 0.35, fs * 0.9, "middle", &hs[r][c].to_string());
                }
            }
        }
        d.rect(x, y, nc as f64 * u, nr as f64 * u, "none", INK, 2.4);
    };
    grid(&mut d, "Top view", vx, ty, &vt, rows, cols, Some(&g));
    grid(&mut d, "Front view", vx, fy, &vf, hmax, cols, None);
    grid(&mut d, "Right view", rx, fy, &vr, hmax, rows, None);
    chart::text(&mut d, vx, fy + hmax as f64 * u + 36.0, fs * 0.85, "start", "Top view: the number is how many cubes are stacked there.");
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn heights_parse_and_pad() {
        assert_eq!(parse("1 2\n3").unwrap(), vec![vec![1, 2], vec![3, 0]]);
        assert_eq!(parse("1,2,3").unwrap(), vec![vec![1, 2, 3]]);
        for (spec, needle) in [("", "Give a grid"), ("0", "at least one cube"), ("6", "not a height"), ("-1", "not a height"), ("a", "not a height"), ("1.5", "not a height")] {
            assert!(parse(spec).unwrap_err().contains(needle), "{spec:?}");
        }
        assert!(parse(&"1\n".repeat(6)).unwrap_err().contains("5 rows"));
        assert!(parse("1 1 1 1 1 1").unwrap_err().contains("5 columns"));
        assert!(parse(&"1 ".repeat(150)).unwrap_err().contains("200"));
    }

    #[test]
    fn views_are_the_silhouettes() {
        // back row 0 1 0, middle 2 3 1, front 1 1 1
        let g = parse(DEFAULT).unwrap();
        let (top, front, right) = views(&g);
        assert_eq!(top, vec![vec![false, true, false], vec![true, true, true], vec![true, true, true]]);
        // column heights 2, 3, 1: levels from the top: 3 only; 2 and 3; all three
        assert_eq!(front, vec![vec![false, true, false], vec![true, true, false], vec![true, true, true]]);
        // rows front to back have maxima 1, 3, 1
        assert_eq!(right, vec![vec![false, true, false], vec![false, true, false], vec![true, true, true]]);
        // a single tower
        let (t, f, r) = views(&parse("3").unwrap());
        assert_eq!((t, f.len(), r.len()), (vec![vec![true]], 3, 3));
    }

    #[test]
    fn key_fills_the_views() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(key.contains(">Top view<") && student.contains(">Front view<") && student.contains(">Right view<"));
        let fills = |s: &str| s.matches("#cfe0fa").count();
        assert_eq!(fills(&student), 0);
        assert_eq!(fills(&key), 7 + 6 + 5 - 0, "top 8 cells, front 6, right 5 cells minus none");
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn the_stack_draws_three_faces_a_cube() {
        let svg = render(&defaults());
        let cubes: usize = parse(DEFAULT).unwrap().iter().flatten().sum();
        // each cube has 3 faces; the views add no polygons
        assert_eq!(svg.matches("<polygon").count(), cubes * 3);
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("orthographic").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("not a height") && !svg.contains("Give a grid"), "{}", ex.name);
        }
        for spec in ["\u{0}", "<b>", "nan", "5 5 5 5 5\n5 5 5 5 5\n5 5 5 5 5\n5 5 5 5 5\n5 5 5 5 5", "0 0 1\n\n1 0 0", "1e9"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("heights", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
