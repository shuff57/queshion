//! Sorting passes: the array after each pass of bubble, selection or insertion sort, one row per
//! pass, with the cells that changed marked, the sorted part shaded and the comparisons and swaps
//! counted. The student copy (Show answers off) keeps the start row and the headings and leaves
//! the other cells empty.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::data;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_ITEMS: usize = 12;
const ALGOS: [&str; 3] = ["Bubble sort", "Selection sort", "Insertion sort"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "sorting-passes",
        subject: Subject::Computing,
        title: "Sorting passes",
        blurb: "The array after each pass of bubble, selection or insertion sort, with the changed cells marked, the sorted part shaded and the comparisons and swaps counted, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(_fig: &str, _key: &str, _p: &Params) -> bool {
    true
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "sorting-passes" => vec![
            Example { name: "Bubble sort", params: vec![] },
            Example { name: "Selection sort", params: vec![("algorithm", s("Selection sort"))] },
            Example { name: "Insertion sort", params: vec![("algorithm", s("Insertion sort")), ("values", s("9, 4, 7, 1, 8, 2"))] },
            Example { name: "Largest first", params: vec![("values", s("3, 1, 4, 1, 5, 9, 2")), ("descending", Value::B(true))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("algorithm", "Algorithm", &ALGOS, "Bubble sort"),
        ParamDef::data("values", "Values", 1, "5, 2, 9, 1, 7"),
        ParamDef::bool("descending", "Largest first", false),
        ParamDef::bool("counts", "Show comparisons and swaps", true),
        ParamDef::int("given", "Rows filled in for students", 1, 12, 1).hint("When Show answers is off, this many rows from the top (the start counts as one) keep their values"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the start row and leaves the later passes empty for students"),
    ]
}

struct Pass {
    cells: Vec<f64>,
    changed: Vec<bool>,
    sorted: usize,
    compares: usize,
    moves: usize,
}

/// Rows of the sort: the start, then one per pass. `before(a, b)` is true when a must come first.
fn passes(algo: &str, start: &[f64], desc: bool) -> Vec<Pass> {
    let before = |a: f64, b: f64| if desc { a > b } else { a < b };
    let n = start.len();
    let mut a = start.to_vec();
    let mut out = vec![Pass { cells: a.clone(), changed: vec![false; n], sorted: if algo == "Insertion sort" { n.min(1) } else { 0 }, compares: 0, moves: 0 }];
    let push = |a: &[f64], prev: &Vec<f64>, sorted: usize, compares: usize, moves: usize, out: &mut Vec<Pass>| {
        out.push(Pass { cells: a.to_vec(), changed: a.iter().zip(prev).map(|(x, y)| x != y).collect(), sorted: sorted.min(a.len()), compares, moves });
    };
    match algo {
        "Selection sort" => {
            for k in 0..n.saturating_sub(1) {
                let prev = a.clone();
                let mut best = k;
                let mut compares = 0;
                for j in k + 1..n {
                    compares += 1;
                    if before(a[j], a[best]) {
                        best = j;
                    }
                }
                let moved = best != k;
                a.swap(k, best);
                push(&a, &prev, k + 1, compares, moved as usize, &mut out);
            }
        }
        "Insertion sort" => {
            for i in 1..n {
                let prev = a.clone();
                let key = a[i];
                let (mut j, mut compares, mut moves) = (i, 0, 0);
                while j > 0 {
                    compares += 1;
                    if before(key, a[j - 1]) {
                        a[j] = a[j - 1];
                        moves += 1;
                        j -= 1;
                    } else {
                        break;
                    }
                }
                a[j] = key;
                push(&a, &prev, i + 1, compares, moves, &mut out);
            }
        }
        _ => {
            for k in 0..n.saturating_sub(1) {
                let prev = a.clone();
                let (mut compares, mut swaps) = (0, 0);
                for j in 0..n - 1 - k {
                    compares += 1;
                    if before(a[j + 1], a[j]) {
                        a.swap(j, j + 1);
                        swaps += 1;
                    }
                }
                push(&a, &prev, k + 1, compares, swaps, &mut out);
                if swaps == 0 {
                    // a clean pass: everything is in place
                    if let Some(l) = out.last_mut() {
                        l.sorted = n;
                    }
                    break;
                }
            }
        }
    }
    if let Some(l) = out.last_mut() {
        l.sorted = n;
    }
    out
}

fn render(p: &Params) -> String {
    let vals: Vec<f64> = data::numbers(p.s("values")).into_iter().filter(|v| v.is_finite()).take(MAX_ITEMS).collect();
    if vals.is_empty() {
        return chart::message("Add some values to sort");
    }
    let algo = ALGOS.iter().copied().find(|a| *a == p.s("algorithm")).unwrap_or("Bubble sort");
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 16.0 * k;
    let rows = passes(algo, &vals, p.b("descending"));
    let given = p.i("given").max(0) as usize;
    let title = p.s("title").trim();
    let counts = p.b("counts");
    let move_word = match algo {
        "Insertion sort" => "shifts",
        _ => "swaps",
    };

    let cell_w = (vals.iter().map(|v| gd::text_w(&chart::fmt(*v), fs)).fold(0.0, f64::max) + 22.0).max(fs * 3.0);
    let row_h = fs * 2.2;
    let lab_w = (fs * 4.8).max(gd::text_w(algo.trim_end_matches(" sort"), fs) + 24.0);
    let stat_text = |r: &Pass| {
        let (c, m) = (r.compares, r.moves);
        format!("{c} {}, {m} {}", if c == 1 { "compare" } else { "compares" }, if m == 1 { move_word.trim_end_matches('s') } else { move_word })
    };
    let stat_w = if counts { rows.iter().skip(1).map(|r| gd::text_w(&stat_text(r), fs * 0.9)).fold(fs * 6.0, f64::max) + 20.0 } else { 0.0 };
    let table_w = lab_w + cell_w * vals.len() as f64 + stat_w;
    let top = if title.is_empty() { 14.0 } else { 38.0 + fs * 0.4 };
    let w = (table_w + 40.0).max(300.0);
    let h = top + row_h * (rows.len() + 1) as f64 + 18.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let x0 = (w - table_w) / 2.0;
    let head = if print { "#e5e5e5" } else { "#f0e9db" };
    let sorted_fill = if print { "#ececec" } else { "#dff3e4" };
    let changed_fill = if print { "#bdbdbd" } else { "#ffe9a8" };

    d.rect(x0, top, lab_w, row_h, head, INK, 1.2);
    gd::text_fill(&mut d, x0 + lab_w / 2.0, top + row_h / 2.0 + fs * 0.35, fs, INK, algo.trim_end_matches(" sort"));
    for i in 0..vals.len() {
        let x = x0 + lab_w + i as f64 * cell_w;
        d.rect(x, top, cell_w, row_h, head, INK, 1.2);
        gd::text_fill(&mut d, x + cell_w / 2.0, top + row_h / 2.0 + fs * 0.35, fs * 0.85, INK, &i.to_string());
    }
    if counts {
        d.rect(x0 + lab_w + cell_w * vals.len() as f64, top, stat_w, row_h, head, INK, 1.2);
        gd::text_fill(&mut d, x0 + lab_w + cell_w * vals.len() as f64 + stat_w / 2.0, top + row_h / 2.0 + fs * 0.35, fs * 0.9, INK, "Work done");
    }
    for (ri, r) in rows.iter().enumerate() {
        let y = top + row_h * (ri + 1) as f64;
        let shown = answers || ri < given;
        d.rect(x0, y, lab_w, row_h, head, INK, 1.2);
        gd::text_fill(&mut d, x0 + lab_w / 2.0, y + row_h / 2.0 + fs * 0.35, fs, INK, &if ri == 0 { "Start".to_string() } else { format!("Pass {ri}") });
        for (i, v) in r.cells.iter().enumerate() {
            let x = x0 + lab_w + i as f64 * cell_w;
            let in_sorted = if algo == "Bubble sort" || algo == "Selection sort" { if algo == "Bubble sort" { i >= vals.len() - r.sorted.min(vals.len()) } else { i < r.sorted } } else { i < r.sorted };
            let fill = if !shown {
                "#fff"
            } else if r.changed[i] {
                changed_fill
            } else if in_sorted && ri > 0 {
                sorted_fill
            } else {
                "#fff"
            };
            d.rect(x, y, cell_w, row_h, fill, INK, 1.2);
            if shown {
                gd::text_fill(&mut d, x + cell_w / 2.0, y + row_h / 2.0 + fs * 0.35, fs, INK, &chart::fmt(*v));
            }
        }
        if counts {
            let x = x0 + lab_w + cell_w * vals.len() as f64;
            d.rect(x, y, stat_w, row_h, "#fff", INK, 1.2);
            if shown && ri > 0 {
                gd::text_fill(&mut d, x + stat_w / 2.0, y + row_h / 2.0 + fs * 0.32, fs * 0.9, INK, &stat_text(r));
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

    fn last(rows: &[Pass]) -> Vec<f64> { rows.last().unwrap().cells.clone() }

    #[test]
    fn every_algorithm_sorts_and_stops() {
        for algo in ALGOS {
            for desc in [false, true] {
                for v in [vec![5.0, 2.0, 9.0, 1.0, 7.0], vec![1.0], vec![2.0, 2.0, 1.0], vec![1.0, 2.0, 3.0, 4.0], vec![9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0, 0.0, -1.0, -2.0]] {
                    let rows = passes(algo, &v, desc);
                    let mut want = v.clone();
                    want.sort_by(|a, b| a.total_cmp(b));
                    if desc {
                        want.reverse();
                    }
                    assert_eq!(last(&rows), want, "{algo} {desc} {v:?}");
                    assert!(rows.len() <= v.len() + 1);
                    assert_eq!(rows.last().unwrap().sorted, v.len());
                }
            }
        }
    }

    #[test]
    fn bubble_passes_and_counts_match_the_textbook() {
        let r = passes("Bubble sort", &[5.0, 2.0, 9.0, 1.0, 7.0], false);
        assert_eq!(r[1].cells, [2.0, 5.0, 1.0, 7.0, 9.0]);
        assert_eq!((r[1].compares, r[1].moves), (4, 3));
        assert_eq!(r[2].cells, [2.0, 1.0, 5.0, 7.0, 9.0]);
        assert_eq!(r[3].cells, [1.0, 2.0, 5.0, 7.0, 9.0]);
        assert_eq!(r.last().unwrap().moves, 0, "ends on a pass with no swaps: {}", r.len());
        let s = passes("Selection sort", &[5.0, 2.0, 9.0, 1.0, 7.0], false);
        assert_eq!(s[1].cells, [1.0, 2.0, 9.0, 5.0, 7.0]);
        assert_eq!((s[1].compares, s[1].moves), (4, 1));
        assert_eq!(s[2].moves, 0, "2 is already in place");
        let i = passes("Insertion sort", &[5.0, 2.0, 9.0, 1.0, 7.0], false);
        assert_eq!(i[0].sorted, 1);
        assert_eq!(i[1].cells, [2.0, 5.0, 9.0, 1.0, 7.0]);
        assert_eq!(i[4].cells, [1.0, 2.0, 5.0, 7.0, 9.0]);
        assert_eq!(i[4].moves, 1, "only the 9 shifts to let the 7 in");
    }

    #[test]
    fn default_draws_one_row_per_pass() {
        let svg = render(&defaults());
        assert!(svg.contains(">Start<") && svg.contains(">Pass 1<") && svg.contains(">Bubble<"));
        assert!(svg.contains(">4 compares, 3 swaps<") && svg.contains(">1 compare, 0 swaps<"));
        let mut p = defaults();
        p.set_s("algorithm", "Insertion sort");
        assert!(render(&p).contains("shifts"));
        p.set_b("counts", false);
        assert!(!render(&p).contains("compares"));
    }

    #[test]
    fn student_copy_keeps_the_start_row_only() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(student.contains(">Start<") && student.contains(">5<") && key.contains(">4 compares, 3 swaps<") && !student.contains("compares"));
        assert_eq!(student.matches("<rect").count(), key.matches("<rect").count());
        assert_eq!(size_of(&student), size_of(&key));
        assert!(student.matches(">9<").count() < key.matches(">9<").count());
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#ffe9a8") && !svg.contains("#dff3e4") && !svg.contains("#f0e9db"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("sorting-passes").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    _ => {}
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<rect") && !svg.contains("NaN"), "{}", ex.name);
        }
        let many: String = (0..60).map(|i| format!("{} ", 60 - i)).collect();
        for algo in ALGOS {
            for text in ["", "abc", "1", "1e300, -1e300", "nan, inf", &many, "1, 1, 1, 1", "0.1, 0.25, 1e-9"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("algorithm", algo);
                    p.set_s("values", text);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{algo} {text:?}");
                }
            }
        }
    }
}
