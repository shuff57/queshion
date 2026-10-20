//! Scatterplot of (x, y) pairs with an optional least-squares line, its equation and r. The
//! points are what a student is given; the line, equation and r are the answers, so the student
//! copy (Show answers off) keeps the points and hides them.
use crate::figures::chart::{self, AxesOpts, Frame, INK};
use crate::figures::data;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn def() -> FigureDef {
    FigureDef {
        id: "scatterplot",
        subject: Subject::Math,
        title: "Scatterplot",
        blurb: "A scatterplot of x, y pairs with an optional line of best fit, its equation and the correlation r.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("scatterplot", "eq") => p.b("fit"),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "scatterplot" => vec![
            Example { name: "Hours studied and score", params: vec![] },
            Example {
                name: "Falling trend",
                params: vec![
                    ("pts", s(&data::pairs_text(&data::generate_pairs(30, 8, 40.0, 12.0, -1.5, 160.0, 9.0, 0)))),
                    ("x_title", s("Age of car (months)")),
                    ("y_title", s("Value (hundreds of dollars)")),
                ],
            },
            Example {
                name: "No relationship",
                params: vec![("pts", s(&data::pairs_text(&data::generate_pairs(30, 3, 50.0, 10.0, 0.0, 40.0, 8.0, 0))))],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("pts", "Pairs (x, y)", 2, "1, 52\n2, 55\n2.5, 61\n3, 60\n4, 68\n4.5, 71\n5, 70\n6, 79\n6.5, 77\n7, 85\n8, 88\n9, 93"),
        ParamDef::bool("fit", "Line of best fit", true),
        ParamDef::bool("eq", "Write its equation", false),
        ParamDef::bool("rval", "Write the correlation r", false),
        ParamDef::choice("point_size", "Point size", &["Small", "Medium", "Large"], "Medium"),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", "Hours studied"),
        ParamDef::text("y_title", "Vertical axis title", "Score"),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the line, equation and r, leaving just the points for students to work from"),
    ]
}

/// Least-squares line through a set of points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub slope: f64,
    pub intercept: f64,
    /// Correlation; None when y does not vary at all.
    pub r: Option<f64>,
}

/// The line of best fit, or None with fewer than two points, all the same x, or overflow.
pub fn fit(pts: &[(f64, f64)]) -> Option<Fit> {
    if pts.len() < 2 {
        return None;
    }
    let n = pts.len() as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in pts {
        sxx += (x - mx) * (x - mx);
        sxy += (x - mx) * (y - my);
        syy += (y - my) * (y - my);
    }
    if !(sxx > 0.0 && sxx.is_finite() && sxy.is_finite() && syy.is_finite()) {
        return None;
    }
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    if !(slope.is_finite() && intercept.is_finite()) {
        return None;
    }
    let r = (syy > 0.0).then(|| (sxy / (sxx * syy).sqrt()).clamp(-1.0, 1.0));
    Some(Fit { slope, intercept, r })
}

/// "y = 2.5x + 4" (the sign and spacing written out, coefficients to three decimals).
pub fn equation(f: &Fit) -> String {
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    let m = round(f.slope);
    let b = round(f.intercept);
    let mut s = String::from("y = ");
    if m == 0.0 {
        return s + &chart::fmt(b);
    }
    s += &match m {
        1.0 => "x".to_string(),
        -1.0 => "\u{2212}x".to_string(),
        _ => format!("{}x", chart::fmt(m)),
    };
    if b > 0.0 {
        s += &format!(" + {}", chart::fmt(b));
    } else if b < 0.0 {
        s += &format!(" \u{2212} {}", chart::fmt(-b));
    }
    s
}

/// The scatterplot's axes: about six intervals, padded (see `chart::axis_range`).
pub fn axis_range(lo: f64, hi: f64) -> (f64, f64, f64) {
    chart::axis_range(lo, hi, 6.0)
}

/// The part of the line y = m x + b inside the box, if any (Liang-Barsky on y).
pub fn clip(m: f64, b: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> Option<((f64, f64), (f64, f64))> {
    let (ya, yb) = (m * x0 + b, m * x1 + b);
    let dy = yb - ya;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    if dy == 0.0 {
        if ya < y0 || ya > y1 {
            return None;
        }
    } else {
        let (ta, tb) = ((y0 - ya) / dy, (y1 - ya) / dy);
        t0 = t0.max(ta.min(tb));
        t1 = t1.min(ta.max(tb));
        if t0 > t1 {
            return None;
        }
    }
    Some(((x0 + t0 * (x1 - x0), ya + t0 * dy), (x0 + t1 * (x1 - x0), ya + t1 * dy)))
}

fn render(p: &Params) -> String {
    let pts = data::pairs(p.s("pts"));
    if pts.is_empty() {
        return chart::message("Enter some (x, y) pairs");
    }
    let (xlo, xhi) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.0), b.max(q.0)));
    let (ylo, yhi) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.1), b.max(q.1)));
    let (ax0, ax1, xs) = axis_range(xlo, xhi);
    let (ay0, ay1, ys) = axis_range(ylo, yhi);
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (xt, yt, title) = (p.s("x_title").trim(), p.s("y_title").trim(), p.s("title").trim());
    let xticks = chart::ticks(ax0, ax1, xs);
    let yticks = chart::ticks(ay0, ay1, ys);
    let label_chars = yticks.iter().map(|t| t.1.chars().count()).max().unwrap_or(1) as f64;
    let (w, h) = (640.0, 420.0 + (k - 1.0) * 60.0);
    let frame = Frame {
        w,
        h,
        l: 34.0 + fs * 1.4 + label_chars * fs * 0.62,
        r: 28.0,
        t: if title.is_empty() { 24.0 } else { 34.0 + fs * 1.4 },
        b: 22.0 + fs * 1.9 + if xt.is_empty() { 0.0 } else { fs * 1.7 },
        x0: ax0,
        x1: ax1,
        y0: ay0,
        y1: ay1,
    };
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    chart::axes(&mut d, &frame, &AxesOpts { xticks: &xticks, yticks: &yticks, grid: p.b("grid"), x_title: xt, y_title: yt, fs });

    let answers = p.b(SHOW_ANSWERS);
    let f = fit(&pts);
    if answers && p.b("fit") {
        if let Some(f) = &f {
            if let Some(((xa, ya), (xb, yb))) = clip(f.slope, f.intercept, ax0, ax1, ay0, ay1) {
                let dash = fills.dash(1);
                d.raw(&format!(
                    r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2.6" stroke-linecap="round"{}/>"#,
                    frame.px(xa),
                    frame.py(ya),
                    frame.px(xb),
                    frame.py(yb),
                    fills.line(1),
                    if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) }
                ));
            }
        }
    }
    let radius = match p.s("point_size") {
        "Small" => 3.2,
        "Large" => 6.4,
        _ => 4.6,
    };
    for (x, y) in &pts {
        d.circle(frame.px(*x), frame.py(*y), radius, &fills.series(0), INK, 1.5);
    }
    if answers {
        let mut notes: Vec<String> = Vec::new();
        if p.b("fit") && p.b("eq") {
            if let Some(f) = &f {
                notes.push(equation(f));
            }
        }
        if p.b("rval") {
            if let Some(r) = f.as_ref().and_then(|f| f.r) {
                notes.push(format!("r = {}", chart::fmt((r * 1000.0).round() / 1000.0)));
            }
        }
        // the notes sit in the corner the line stays away from: top-left for a rising line, top-right for a falling one
        let right = f.is_some_and(|f| f.slope < 0.0) && p.b("fit");
        for (i, n) in notes.iter().enumerate() {
            let w = n.chars().count() as f64 * fs * 0.56 + 14.0;
            let y = frame.top() + 10.0 + i as f64 * (fs * 1.5);
            let (plate_x, text_x, anchor) = if right { (frame.right() - 14.0 - w, frame.right() - 14.0 - 6.0, "end") } else { (frame.left() + 14.0 - 6.0, frame.left() + 14.0, "start") };
            // a white plate keeps the note readable over gridlines and points
            d.rect(plate_x, y, w, fs * 1.4, "#fff", chart::GRID, 1.0);
            chart::text(&mut d, text_x, y + fs * 1.05, fs, anchor, n);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn fit_recovers_an_exact_line_and_its_correlation() {
        let f = fit(&[(0.0, 1.0), (1.0, 3.0), (2.0, 5.0), (3.0, 7.0)]).unwrap();
        assert!((f.slope - 2.0).abs() < 1e-12 && (f.intercept - 1.0).abs() < 1e-12);
        assert_eq!(f.r, Some(1.0));
        let neg = fit(&[(0.0, 9.0), (1.0, 6.0), (2.0, 3.0)]).unwrap();
        assert_eq!(neg.r, Some(-1.0));
    }

    #[test]
    fn fit_refuses_what_has_no_line() {
        assert!(fit(&[]).is_none() && fit(&[(1.0, 1.0)]).is_none());
        assert!(fit(&[(2.0, 1.0), (2.0, 5.0)]).is_none(), "a vertical set has no slope");
        let flat = fit(&[(0.0, 4.0), (1.0, 4.0), (2.0, 4.0)]).unwrap();
        assert_eq!((flat.slope, flat.r), (0.0, None), "a flat set has a line but no r");
        assert!(fit(&[(0.0, 0.0), (1e300, 1e300), (2e300, -1e300)]).is_none(), "overflow is not a line");
    }

    #[test]
    fn fit_matches_the_generator() {
        let pts = data::generate_pairs(500, 5, 10.0, 3.0, 2.5, 4.0, 1.0, 3);
        let f = fit(&pts).unwrap();
        assert!((f.slope - 2.5).abs() < 0.1 && f.r.unwrap() > 0.95, "{f:?}");
    }

    #[test]
    fn equations_read_naturally() {
        let eq = |m, b| equation(&Fit { slope: m, intercept: b, r: None });
        assert_eq!(eq(2.0, 1.0), "y = 2x + 1");
        assert_eq!(eq(1.0, -3.5), "y = x \u{2212} 3.5");
        assert_eq!(eq(-1.0, 0.0), "y = \u{2212}x");
        assert_eq!(eq(0.0, 4.25), "y = 4.25");
        assert_eq!(eq(0.12345, 10.0), "y = 0.123x + 10");
    }

    #[test]
    fn axis_ranges_have_air_around_the_data() {
        let (a, b, step) = axis_range(1.0, 9.0);
        assert!(a < 1.0 && b > 9.0 && step > 0.0);
        let (a, b, _) = axis_range(5.0, 5.0);
        assert!(a < 5.0 && b > 5.0, "no spread still has a range");
        let (a, b, _) = axis_range(0.0, 10.0);
        assert!(a < 0.0 && b > 10.0, "points on a tick are not on the frame");
    }

    #[test]
    fn clipping_keeps_the_line_inside_the_box() {
        let ((xa, ya), (xb, yb)) = clip(1.0, 0.0, 0.0, 10.0, 2.0, 8.0).unwrap();
        assert_eq!((xa, ya, xb, yb), (2.0, 2.0, 8.0, 8.0));
        assert!(clip(0.0, 20.0, 0.0, 10.0, 0.0, 10.0).is_none(), "a flat line above the box");
        assert!(clip(1.0, 50.0, 0.0, 10.0, 0.0, 10.0).is_none(), "a line that misses the box");
        assert_eq!(clip(0.0, 5.0, 0.0, 10.0, 0.0, 10.0).unwrap(), ((0.0, 5.0), (10.0, 5.0)));
    }

    #[test]
    fn default_draws_every_point() {
        let p = defaults();
        let svg = render(&p);
        assert_eq!(svg.matches("<circle").count(), data::pairs(p.s("pts")).len());
        assert!(svg.contains("stroke-width=\"2.6\""), "the line of best fit");
    }

    #[test]
    fn student_copy_keeps_the_points_and_hides_the_answers() {
        let def = def();
        let mut p = defaults();
        p.set_b("eq", true);
        p.set_b("rval", true);
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        let points = data::pairs(p.s("pts")).len();
        assert_eq!(student.matches("<circle").count(), points);
        assert!(!student.contains("stroke-width=\"2.6\"") && !student.contains("r = ") && !student.contains("y = "));
        assert!(key.contains("stroke-width=\"2.6\"") && key.contains("r = 0.") && key.contains("y = "));
    }

    #[test]
    fn print_style_is_grey_only_with_a_dashed_line() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#4f7cd4") && !svg.contains("#e0803c"), "no series colours");
        assert!(svg.contains("stroke-dasharray"), "the line is told apart by its dashes");
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        // the ink is a near-black with a slight tint; a leaked series colour is far further from grey
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24), "no colour, grey and ink only");
    }

    #[test]
    fn odd_input_never_panics() {
        let mut p = defaults();
        for text in ["", "abc", "1", "1, 2", "5, 5\n5, 5", "1, 2\n1, 9", "-1e308, 1e308\n1e308, -1e308", "1e300, 1\n2e300, 2", "0, 0\n0.0001, 1000000"] {
            p.set_s("pts", text);
            for answers in [true, false] {
                p.set_b(SHOW_ANSWERS, answers);
                p.set_b("eq", true);
                p.set_b("rval", true);
                let svg = render(&p);
                assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{text:?}");
                assert!(!svg.contains("NaN") && !svg.contains("inf"), "{text:?}");
            }
        }
        p.set_s("pts", "");
        assert!(render(&p).contains("Enter some"));
    }

    #[test]
    fn notes_move_away_from_a_falling_line() {
        let mut p = defaults();
        p.set_b("eq", true);
        let plate_x = |svg: &str| -> f64 {
            // the plate is the white rect with the grid-coloured stroke
            let i = svg.find(r##"fill="#fff" stroke="#dfe3ea""##).expect("a plate");
            let head = &svg[..i];
            let x = &head[head.rfind("<rect x=\"").unwrap() + 9..];
            x[..x.find('"').unwrap()].parse().unwrap()
        };
        let rising = plate_x(&render(&p));
        p.set_s("pts", "1, 9\n2, 7\n3, 6\n4, 3\n5, 2");
        let falling = plate_x(&render(&p));
        assert!(rising < 200.0 && falling > 320.0, "rising {rising}, falling {falling}");
    }

    #[test]
    fn eq_only_matters_with_the_line() {
        let mut p = defaults();
        assert!(relevant("scatterplot", "eq", &p));
        p.set_b("fit", false);
        assert!(!relevant("scatterplot", "eq", &p) && relevant("scatterplot", "rval", &p));
    }

    #[test]
    fn examples_build_and_render() {
        for e in examples("scatterplot").unwrap() {
            let mut p = defaults();
            for (k, v) in e.params {
                p.0.insert(k.to_string(), v);
            }
            assert!(render(&p).contains("<circle"), "{}", e.name);
        }
    }
}
