//! Residuals from the line of best fit: the scatterplot with a segment from each point to the line,
//! the residual plot (residual against x with a zero line), or both, with an optional table of
//! predicted values and residuals. The points are what a student is given; the line, residuals
//! and sum of squares are the answers, so the student copy (Show answers off) keeps the points on
//! the scatterplot and the empty residual axes, and leaves the table cells blank.
use super::scatter::{axis_range as scatter_range, clip, fit};
use crate::figures::chart::{self, AxesOpts, Frame, INK};
use crate::figures::data;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const VIEWS: [&str; 3] = ["Both", "Scatter with residuals", "Residual plot"];
const MAX_TABLE: usize = 15;

pub fn def() -> FigureDef {
    FigureDef {
        id: "residual-plot",
        subject: Subject::Math,
        title: "Residuals",
        blurb: "The residuals of a line of best fit: a scatterplot with a segment from each point to the line, the residual plot, and a table of predicted values and residuals.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("residual-plot", "y_title") => p.s("view") != "Residual plot",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "residual-plot" => vec![
            Example { name: "Scatterplot and residual plot", params: vec![] },
            Example { name: "Residual plot only", params: vec![("view", s("Residual plot"))] },
            Example { name: "With the table", params: vec![("view", s("Scatter with residuals")), ("table", Value::B(true)), ("sse", Value::B(true))] },
            Example {
                name: "A curved pattern",
                params: vec![("pts", s("1, 2\n2, 5\n3, 9\n4, 17\n5, 26\n6, 38\n7, 51\n8, 66")), ("x_title", s("Week")), ("y_title", s("Bacteria (thousands)"))],
            },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false)), ("table", Value::B(true)), ("sse", Value::B(true))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("pts", "Pairs (x, y)", 2, "1, 52\n2, 55\n2.5, 61\n3, 60\n4, 68\n4.5, 71\n5, 70\n6, 79\n6.5, 77\n7, 85\n8, 88\n9, 93"),
        ParamDef::choice("view", "Show", &VIEWS, "Both"),
        ParamDef::bool("table", "Table of predicted values and residuals", false).hint("Up to 15 points"),
        ParamDef::bool("sse", "Write the sum of squared residuals", false),
        ParamDef::choice("point_size", "Point size", &["Small", "Medium", "Large"], "Medium"),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", "Hours studied"),
        ParamDef::text("y_title", "Vertical axis title", "Score"),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the points and leaves the line, the residuals and the sum of squares for students to find"),
    ]
}

fn r3(v: f64) -> String {
    chart::fmt((v * 1000.0).round() / 1000.0)
}

fn render(p: &Params) -> String {
    let pts = data::pairs(p.s("pts"));
    if pts.len() < 2 {
        return chart::message("Enter at least two (x, y) pairs");
    }
    let Some(f) = fit(&pts) else {
        return chart::message("Residuals need points with different x values");
    };
    let pred: Vec<f64> = pts.iter().map(|q| f.slope * q.0 + f.intercept).collect();
    let res: Vec<f64> = pts.iter().zip(&pred).map(|(q, yh)| q.1 - yh).collect();
    if !res.iter().all(|r| r.is_finite()) || !pred.iter().all(|r| r.is_finite()) {
        return chart::message("Those values are too large to work with");
    }
    let sse: f64 = res.iter().map(|r| r * r).sum();
    let view = p.s("view");
    let (want_scatter, want_resid) = (view != "Residual plot", view != "Scatter with residuals");
    let answers = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (xt, yt, title) = (p.s("x_title").trim(), p.s("y_title").trim(), p.s("title").trim());

    let (xlo, xhi) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.0), b.max(q.0)));
    let (ylo, yhi) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.1), b.max(q.1)));
    let (ax0, ax1, xs) = scatter_range(xlo, xhi);
    let (ay0, ay1, ys) = scatter_range(ylo, yhi);
    let m = res.iter().fold(0.0f64, |a, r| a.max(r.abs())).max(1e-9);
    let (rb0, rb1, rs) = chart::axis_range(-m, m, 5.0);
    let (rb0, rb1) = (rb0.min(-rb1), rb1.max(-rb0));
    let xticks = chart::ticks(ax0, ax1, xs);
    let yticks = chart::ticks(ay0, ay1, ys);
    let rticks = chart::ticks(rb0, rb1, rs);
    let label_chars = yticks.iter().chain(rticks.iter()).map(|t| t.1.chars().count()).max().unwrap_or(1) as f64;
    let left = 34.0 + fs * 1.4 + label_chars * fs * 0.62;

    let w = 640.0;
    let panel_h = 300.0 + (k - 1.0) * 40.0;
    let axis_foot = 22.0 + fs * 1.9;
    let panels = want_scatter as usize + want_resid as usize;
    let top = if title.is_empty() { 22.0 } else { 34.0 + fs * 1.4 };
    let table_rows = if p.b("table") && pts.len() <= MAX_TABLE { pts.len() } else { 0 };
    let row_h = fs * 1.9;
    let table_h = if table_rows > 0 { (table_rows as f64 + 1.0) * row_h + 20.0 } else { 0.0 };
    let sse_h = if p.b("sse") { fs * 2.4 } else { 0.0 };
    let xt_h = if xt.is_empty() { 0.0 } else { fs * 1.7 };
    let panels_h = panels as f64 * (panel_h + axis_foot) + xt_h;
    let h = top + panels_h + table_h + sse_h + 10.0;
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    let radius = match p.s("point_size") {
        "Small" => 3.2,
        "Large" => 6.4,
        _ => 4.6,
    };
    let mut y0 = top;
    let mut drawn = 0;
    let panel_frame = |y0: f64, x0: f64, x1: f64, ya: f64, yb: f64| Frame { w, h, l: left, r: 28.0, t: y0, b: h - (y0 + panel_h), x0, x1, y0: ya, y1: yb };

    if want_scatter {
        drawn += 1;
        let fr = panel_frame(y0, ax0, ax1, ay0, ay1);
        let last = drawn == panels;
        chart::axes(&mut d, &fr, &AxesOpts { xticks: &xticks, yticks: &yticks, grid: p.b("grid"), x_title: "", y_title: yt, fs });
        if answers {
            if let Some(((xa, ya), (xb, yb))) = clip(f.slope, f.intercept, ax0, ax1, ay0, ay1) {
                let dash = fills.dash(1);
                d.raw(&format!(
                    r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2.6" stroke-linecap="round"{}/>"#,
                    fr.px(xa),
                    fr.py(ya),
                    fr.px(xb),
                    fr.py(yb),
                    fills.line(1),
                    if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) }
                ));
            }
            for (q, yh) in pts.iter().zip(&pred) {
                d.raw(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.4" stroke-dasharray="3 3"/>"#, fr.px(q.0), fr.py(q.1), fr.px(q.0), fr.py(yh.clamp(ay0, ay1))));
            }
        }
        for q in &pts {
            d.circle(fr.px(q.0), fr.py(q.1), radius, &fills.series(0), INK, 1.5);
        }
        if last && !xt.is_empty() {
            chart::text(&mut d, w / 2.0, fr.bottom() + 14.0 + fs * 2.4, fs * 1.1, "middle", xt);
        }
        y0 += panel_h + axis_foot;
    }
    if want_resid {
        drawn += 1;
        let fr = panel_frame(y0, ax0, ax1, rb0, rb1);
        let last = drawn == panels;
        chart::axes(&mut d, &fr, &AxesOpts { xticks: &xticks, yticks: &rticks, grid: p.b("grid"), x_title: "", y_title: "Residual", fs });
        let zy = fr.py(0.0);
        d.raw(&format!(r#"<line x1="{:.2}" y1="{zy:.2}" x2="{:.2}" y2="{zy:.2}" stroke="{INK}" stroke-width="2" stroke-dasharray="7 4"/>"#, fr.left(), fr.right()));
        if answers {
            for (q, r) in pts.iter().zip(&res) {
                d.raw(&format!(r#"<line x1="{:.2}" y1="{zy:.2}" x2="{:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.2"/>"#, fr.px(q.0), fr.px(q.0), fr.py(*r)));
                d.circle(fr.px(q.0), fr.py(*r), radius, &fills.series(0), INK, 1.5);
            }
        }
        if last && !xt.is_empty() {
            chart::text(&mut d, w / 2.0, fr.bottom() + 14.0 + fs * 2.4, fs * 1.1, "middle", xt);
        }
        y0 += panel_h + axis_foot;
    }
    y0 += xt_h;
    if table_rows > 0 {
        let heads = ["x", "y", "predicted y", "residual"];
        let cw = 130.0;
        let x0 = (w - cw * 4.0) / 2.0;
        for (c, hd) in heads.iter().enumerate() {
            d.rect(x0 + c as f64 * cw, y0, cw, row_h, if fills.is_print() { "#e5e5e5" } else { "#e6edf8" }, INK, 1.2);
            chart::text(&mut d, x0 + c as f64 * cw + cw / 2.0, y0 + row_h * 0.68, fs, "middle", hd);
        }
        for (i, q) in pts.iter().enumerate() {
            let y = y0 + row_h * (i as f64 + 1.0);
            let cells = [r3(q.0), r3(q.1), r3(pred[i]), r3(res[i])];
            for (c, text) in cells.iter().enumerate() {
                d.rect(x0 + c as f64 * cw, y, cw, row_h, "#fff", INK, 1.2);
                if c < 2 || answers {
                    chart::text(&mut d, x0 + c as f64 * cw + cw / 2.0, y + row_h * 0.68, fs, "middle", text);
                }
            }
        }
        y0 += table_h;
    }
    if p.b("sse") {
        let ty = y0 + fs * 1.2;
        if answers {
            chart::text(&mut d, w / 2.0, ty, fs * 1.05, "middle", &format!("Sum of squared residuals = {}", r3(sse)));
        } else {
            chart::text(&mut d, w / 2.0 - 60.0, ty, fs * 1.05, "end", "Sum of squared residuals =");
            d.line(w / 2.0 - 52.0, ty + 2.0, w / 2.0 + 90.0, ty + 2.0, INK, 1.0);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn residuals_of_a_known_set_sum_to_zero() {
        let pts = [(1.0, 2.0), (2.0, 3.0), (3.0, 5.0), (4.0, 4.0)];
        let f = fit(&pts).unwrap();
        let res: Vec<f64> = pts.iter().map(|q| q.1 - (f.slope * q.0 + f.intercept)).collect();
        assert!(res.iter().sum::<f64>().abs() < 1e-12);
        // slope 0.8, intercept 1.5: residuals -0.3, -0.1, 1.1, -0.7
        assert!((res[2] - 1.1).abs() < 1e-12 && (res[0] + 0.3).abs() < 1e-12);
    }

    #[test]
    fn views_draw_the_panels_asked_for() {
        let both = render(&defaults());
        assert!(both.contains(">Residual<") && both.contains(">Score<"));
        let mut p = defaults();
        p.set_s("view", "Residual plot");
        let only = render(&p);
        assert!(only.contains(">Residual<") && !only.contains(">Score<"));
        p.set_s("view", "Scatter with residuals");
        let sc = render(&p);
        assert!(!sc.contains(">Residual<") && sc.contains(">Score<"));
    }

    #[test]
    fn table_and_sum_of_squares() {
        let mut p = defaults();
        p.set_s("pts", "1, 2\n2, 3\n3, 5\n4, 4");
        p.set_b("table", true);
        p.set_b("sse", true);
        let svg = render(&p);
        for t in [">predicted y<", ">residual<", ">1.1<", ">\u{2212}0.3<", "Sum of squared residuals = 1.8"] {
            assert!(svg.contains(t), "{t}");
        }
        p.set_s("pts", &(1..=16).map(|i| format!("{i}, {}", i * i)).collect::<Vec<_>>().join("\n"));
        assert!(!render(&p).contains(">predicted y<"), "more than 15 points gets no table");
    }

    #[test]
    fn student_copy_keeps_the_points_only() {
        let def = def();
        let mut p = defaults();
        p.set_s("pts", "1, 2\n2, 3\n3, 5\n4, 4");
        p.set_b("table", true);
        p.set_b("sse", true);
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(key.contains(">1.1<") && !student.contains(">1.1<") && !student.contains("= 1.8"));
        assert!(student.contains(">predicted y<") && student.contains(">5<"));
        assert_eq!(size_of(&student), size_of(&key));
        assert!(student.matches("<circle").count() < key.matches("<circle").count(), "the residual points are the answer");
    }

    #[test]
    fn bad_input_is_explained() {
        let mut p = defaults();
        p.set_s("pts", "1, 2");
        assert!(render(&p).contains("at least two"));
        p.set_s("pts", "3, 1\n3, 5");
        assert!(render(&p).contains("different x"));
        p.set_s("pts", "0, 0\n1e300, 1e300\n2e300, -1e300");
        assert!(render(&p).starts_with("<svg"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_b("table", true);
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("residual-plot").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    Value::I(i) => p.set_i(k, *i),
                    Value::F(x) => p.set_f(k, *x),
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<circle") && !svg.contains("NaN"), "{}", ex.name);
        }
        for pts in ["", "words", "1, 2, 3", "1, 1\n1, 1", "0, 0\n1, 1e308\n2, -1e308", "1, 2\n2, 4\n3, 6", "nan, 1\n2, inf"] {
            for view in VIEWS {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("pts", pts);
                    p.set_s("view", view);
                    p.set_b("table", true);
                    p.set_b("sse", true);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{pts:?} {view}");
                }
            }
        }
    }
}
