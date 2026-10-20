//! Two-way (contingency) table: counts for two categorical variables with row and column totals.
//! The totals (the marginals) and chosen cells are what a student is asked to work out, so the
//! student copy (Show answers off) leaves those boxes empty and the key fills them in, in bold.
use crate::figures::chart::{self, INK};
use crate::figures::data;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const MAX_LINES: usize = 8;

pub fn def() -> FigureDef {
    FigureDef {
        id: "two-way-table",
        subject: Subject::Math,
        title: "Two-Way Table",
        blurb: "A two-way table of counts with row and column totals; hide the totals or chosen cells for students to fill in.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("two-way-table", "blank_count") => p.s("blank") != "Totals",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "two-way-table" => vec![
            Example { name: "Favourite lunch", params: vec![] },
            Example {
                name: "Missing cells",
                params: vec![
                    ("row_labels", s("Freshmen, Sophomores, Juniors")),
                    ("col_labels", s("Bus, Walk, Car")),
                    ("cells", s("40, 22, 6\n35, 30, 14\n12, 18, 41")),
                    ("row_title", s("Grade")),
                    ("col_title", s("Gets to school by")),
                    ("blank", s("Some cells")),
                    ("blank_count", Value::I(3)),
                ],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("row_labels", "Row names", "Girls, Boys").hint("Separate with commas, up to 8"),
        ParamDef::text("col_labels", "Column names", "Pizza, Tacos, Salad").hint("Separate with commas, up to 8"),
        ParamDef::long("cells", "Counts", "24, 18, 8\n19, 27, 4").hint("One line per row, numbers separated by commas or spaces. Missing counts are 0"),
        ParamDef::text("row_title", "Row variable", ""),
        ParamDef::text("col_title", "Column variable", ""),
        ParamDef::text("title", "Title", ""),
        ParamDef::choice("blank", "Blank for students", &["Totals", "Some cells", "Totals and some cells"], "Totals").hint("What Show answers off leaves empty"),
        ParamDef::int("blank_count", "How many cells", 1, 20, 2),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves the chosen boxes empty for students; on fills them in bold"),
    ]
}

/// Names split at commas (or semicolons), trimmed, at most [`MAX_LINES`]; unnamed ones are
/// "Row 1" / "Column 2" style.
pub fn labels(text: &str, fallback: &str, want_at_least: usize) -> Vec<String> {
    let mut v: Vec<String> = text.split([',', ';']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    v.truncate(MAX_LINES);
    while v.len() < want_at_least.max(1) {
        v.push(format!("{fallback} {}", v.len() + 1));
    }
    v
}

/// The counts row-major, `nr` by `nc`; short input is padded with 0 and extra is ignored.
pub fn grid(text: &str, nr: usize, nc: usize) -> Vec<Vec<f64>> {
    let nums = data::numbers(text);
    (0..nr).map(|r| (0..nc).map(|c| nums.get(r * nc + c).copied().unwrap_or(0.0)).collect()).collect()
}

/// Which interior cells (row-major index) a student fills in: `count` of them taken along
/// diagonals, so the blanks fall in different rows and columns instead of emptying one line.
pub fn blanked(nr: usize, nc: usize, count: usize) -> Vec<usize> {
    let n = nr * nc;
    (0..count.min(n)).map(|j| (j % nr) * nc + (j / nr + j % nr) % nc).collect()
}

fn render(p: &Params) -> String {
    let text = p.s("cells");
    // the table has as many columns and rows as there are names; names fall back to counts of the
    // lines and columns actually typed
    let lines = data::parse_table(text);
    let typed_cols = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let named_r = labels(p.s("row_labels"), "Row", lines.len());
    let named_c = labels(p.s("col_labels"), "Column", typed_cols.min(MAX_LINES));
    let (nr, nc) = (named_r.len(), named_c.len());
    // read per line, so a blank count in the middle keeps its place
    let mut g = vec![vec![0.0; nc]; nr];
    for (r, line) in lines.iter().take(nr).enumerate() {
        for (c, cell) in line.iter().take(nc).enumerate() {
            g[r][c] = data::numbers(cell).first().copied().unwrap_or(0.0);
        }
    }
    if lines.is_empty() {
        g = grid(text, nr, nc);
    }
    let row_tot: Vec<f64> = g.iter().map(|r| r.iter().sum()).collect();
    let col_tot: Vec<f64> = (0..nc).map(|c| g.iter().map(|r| r[c]).sum()).collect();
    let grand: f64 = row_tot.iter().sum();

    let mode = p.s("blank");
    let answers = p.b(SHOW_ANSWERS);
    let hide_totals = !answers && mode != "Some cells";
    let hidden: Vec<usize> = if mode == "Totals" { Vec::new() } else { blanked(nr, nc, p.i("blank_count").max(0) as usize) };
    let is_blank_cell = |r: usize, c: usize| hidden.contains(&(r * nc + c));

    let fills = Fills::new(p);
    let tint = if fills.is_print() { "#ececec" } else { "#e3ebfa" };
    let k = svg::label_factor(p.s("label_size"));
    let fs = 17.0 * k;
    let (title, row_title, col_title) = (p.s("title").trim(), p.s("row_title").trim(), p.s("col_title").trim());
    let char_w = fs * 0.62;
    let num = |v: f64| chart::fmt(v);
    let first_w = (named_r.iter().map(|s| s.chars().count()).chain([row_title.chars().count(), 5]).max().unwrap() as f64 * char_w + 28.0).max(90.0);
    let col_w: Vec<f64> = (0..=nc)
        .map(|c| {
            let name = if c < nc { named_c[c].chars().count() } else { 5 };
            let widest_num = (0..nr).map(|r| num(if c < nc { g[r][c] } else { row_tot[r] }).chars().count()).max().unwrap_or(1).max(num(if c < nc { col_tot[c] } else { grand }).chars().count());
            (name.max(widest_num) as f64 * char_w + 28.0).max(80.0)
        })
        .collect();
    let rowh = fs * 2.5;
    let top_pad = if title.is_empty() { 14.0 } else { 36.0 + fs * 1.1 };
    let span_h = if col_title.is_empty() { 0.0 } else { rowh * 0.8 };
    let (w, h) = (20.0 + first_w + col_w.iter().sum::<f64>() + 20.0, top_pad + span_h + rowh * (nr as f64 + 2.0) + 16.0);

    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.1, fs * 1.15, "middle", title);
    }
    let x0 = 20.0;
    let y0 = top_pad + span_h;
    let xs: Vec<f64> = std::iter::once(x0).chain(col_w.iter().scan(x0 + first_w, |x, cw| { let v = *x; *x += cw; Some(v) })).collect();
    // xs[0] = first column, xs[1..=nc+1] = data columns and the Total column
    let cell_x = |c: usize| xs[c + 1];
    let cell_text = |d: &mut Doc, x: f64, y: f64, wid: f64, s: &str, bold: bool| {
        d.raw(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{fs}" text-anchor="middle" font-weight="{}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
            x + wid / 2.0,
            y + rowh * 0.64,
            if bold { 700 } else { 400 },
            esc(s)
        ));
    };
    // spanning column variable, then the header row
    if !col_title.is_empty() {
        let (a, b) = (cell_x(0), cell_x(nc));
        chart::text(&mut d, (a + b) / 2.0, top_pad + span_h * 0.7, fs * 1.05, "middle", col_title);
    }
    let table_w = first_w + col_w.iter().sum::<f64>();
    // header row and total row are tinted
    d.rect(x0, y0, table_w, rowh, tint, INK, 1.6);
    d.rect(x0, y0 + rowh * (nr as f64 + 1.0), table_w, rowh, tint, INK, 1.6);
    d.rect(x0, y0 + rowh, first_w, rowh * nr as f64, tint, INK, 1.6);
    d.rect(cell_x(nc), y0 + rowh, col_w[nc], rowh * nr as f64, tint, INK, 1.6);
    for r in 0..=nr + 1 {
        let y = y0 + rowh * r as f64;
        d.line(x0, y, x0 + table_w, y, INK, 1.6);
    }
    for c in 0..=nc + 1 {
        let x = xs[c];
        d.line(x, y0, x, y0 + rowh * (nr as f64 + 2.0), INK, 1.6);
    }
    d.line(x0 + table_w, y0, x0 + table_w, y0 + rowh * (nr as f64 + 2.0), INK, 1.6);
    // outline
    d.rect(x0, y0, table_w, rowh * (nr as f64 + 2.0), "none", INK, 2.4);
    cell_text(&mut d, x0, y0, first_w, row_title, false);
    for c in 0..nc {
        cell_text(&mut d, cell_x(c), y0, col_w[c], &named_c[c], true);
    }
    cell_text(&mut d, cell_x(nc), y0, col_w[nc], "Total", true);
    for r in 0..nr {
        let y = y0 + rowh * (r as f64 + 1.0);
        cell_text(&mut d, x0, y, first_w, &named_r[r], true);
        for c in 0..nc {
            if is_blank_cell(r, c) && !answers {
                continue;
            }
            cell_text(&mut d, cell_x(c), y, col_w[c], &num(g[r][c]), is_blank_cell(r, c));
        }
        if !hide_totals {
            cell_text(&mut d, cell_x(nc), y, col_w[nc], &num(row_tot[r]), !answers_given_totals(mode));
        }
    }
    let ty = y0 + rowh * (nr as f64 + 1.0);
    cell_text(&mut d, x0, ty, first_w, "Total", true);
    if !hide_totals {
        for c in 0..nc {
            cell_text(&mut d, cell_x(c), ty, col_w[c], &num(col_tot[c]), !answers_given_totals(mode));
        }
        cell_text(&mut d, cell_x(nc), ty, col_w[nc], &num(grand), !answers_given_totals(mode));
    }
    d.finish()
}

/// Totals are given (so plain) when the student copy only blanks cells; otherwise they are answers.
fn answers_given_totals(mode: &str) -> bool {
    mode == "Some cells"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn labels_are_trimmed_capped_and_padded() {
        assert_eq!(labels(" a , b;c ", "Row", 0), ["a", "b", "c"]);
        assert_eq!(labels("", "Row", 2), ["Row 1", "Row 2"]);
        assert_eq!(labels(&vec!["x"; 20].join(","), "Row", 0).len(), MAX_LINES);
    }

    #[test]
    fn grid_pads_missing_counts_with_zero() {
        assert_eq!(grid("1, 2, 3", 2, 2), vec![vec![1.0, 2.0], vec![3.0, 0.0]]);
        assert_eq!(grid("", 1, 2), vec![vec![0.0, 0.0]]);
    }

    #[test]
    fn blanked_cells_are_distinct_and_spread() {
        for nr in 1..6 {
            for nc in 1..6 {
                for c in 0..=nr * nc + 2 {
                    let b = blanked(nr, nc, c);
                    assert_eq!(b.len(), c.min(nr * nc));
                    let mut s = b.clone();
                    s.sort();
                    s.dedup();
                    assert_eq!(s.len(), b.len(), "{nr}x{nc} c={c}: {b:?}");
                    assert!(b.iter().all(|i| *i < nr * nc));
                }
            }
        }
        let three = blanked(3, 3, 3);
        assert_eq!(three.iter().map(|i| i % 3).collect::<std::collections::BTreeSet<_>>().len(), 3, "three blanks hit three columns");
        assert_eq!(three.iter().map(|i| i / 3).collect::<std::collections::BTreeSet<_>>().len(), 3, "and three rows");
    }

    #[test]
    fn default_shows_counts_and_totals() {
        let svg = render(&defaults());
        // 6 counts, 2 row totals, 3 column totals, grand total, 3 column names, 2 row names, "Total"x2, "Total" col header
        assert!(svg.contains(">24<") && svg.contains(">50<") && svg.contains(">43<") && svg.contains(">100<"), "{svg}");
        assert!(svg.contains(">Girls<") && svg.contains(">Pizza<"));
    }

    #[test]
    fn totals_are_the_sums() {
        let mut p = defaults();
        p.set_s("cells", "1, 2\n3, 4");
        p.set_s("row_labels", "A, B");
        p.set_s("col_labels", "X, Y");
        let svg = render(&p);
        for t in [">3<", ">7<", ">4<", ">6<", ">10<"] {
            assert!(svg.contains(t), "{t} missing");
        }
    }

    #[test]
    fn student_copy_blanks_the_totals_only() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        let texts = |s: &str| s.matches("<text").count();
        // 2 rows x 3 cols = 6 counts stay; 2 + 3 + 1 totals go
        assert_eq!(texts(&key) - texts(&student), 6);
        assert!(!student.contains(">100<") && !student.contains(">50<"));
        assert!(student.contains(">24<"), "the given counts stay");
        let size = |s: &str| s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string();
        assert_eq!(size(&student), size(&key));
    }

    #[test]
    fn student_copy_can_blank_cells_instead() {
        let mut p = defaults();
        p.set_s("blank", "Some cells");
        p.set_i("blank_count", 2);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert_eq!(key.matches("<text").count() - student.matches("<text").count(), 2);
        assert!(student.contains(">50<"), "totals are given when only cells are blank");
        let mut q = defaults();
        q.set_s("blank", "Totals and some cells");
        q.set_i("blank_count", 2);
        let (s2, k2) = crate::figures::answer_pair(&def(), &q).unwrap();
        assert_eq!(k2.matches("<text").count() - s2.matches("<text").count(), 8);
        assert!(relevant("two-way-table", "blank_count", &q) && !relevant("two-way-table", "blank_count", &defaults()));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("title", "Lunch survey");
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn odd_input_never_panics_or_prints_nan() {
        let mut p = defaults();
        let cells = ["", "abc", "1", "-5, 3", "1e12, 1e12\n1e12, 1e12", "0.5, 0.25", "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17", "NaN, inf", "9999999999999999999999"];
        for cell in cells {
            for (rl, cl) in [("", ""), ("A", "B"), ("a,b,c,d,e,f,g,h,i,j", "1,2,3,4,5,6,7,8,9,10,11")] {
                for blank in ["Totals", "Some cells", "Totals and some cells"] {
                    p.set_s("cells", cell);
                    p.set_s("row_labels", rl);
                    p.set_s("col_labels", cl);
                    p.set_s("blank", blank);
                    for answers in [true, false] {
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{cell:?}");
                        assert!(!svg.contains("NaN") && !svg.contains("inf"), "{cell:?}: {svg}");
                    }
                }
            }
        }
    }

    #[test]
    fn names_are_escaped() {
        let mut p = defaults();
        p.set_s("row_labels", "<b>&, Boys");
        let svg = render(&p);
        assert!(svg.contains("&lt;b&gt;&amp;") && !svg.contains("<b>"));
    }

    #[test]
    fn examples_render() {
        for e in examples("two-way-table").unwrap() {
            let mut p = defaults();
            for (k, v) in e.params {
                p.0.insert(k.to_string(), v);
            }
            assert!(render(&p).contains("Total"), "{}", e.name);
        }
    }
}
