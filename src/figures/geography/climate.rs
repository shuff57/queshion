//! A climate graph: monthly rainfall as bars on the left axis and monthly temperature as a line
//! on the right axis, with a table of the twelve months below. The table is the question; the
//! bars, the line and the summary (annual rainfall, temperature range, warmest and wettest
//! month) are the answers, so the student copy (Show answers off) draws only the empty axes and
//! leaves the summary as blank lines.
use super::{rows, trim};
use crate::figures::chart::{self, AxesOpts, Frame};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const INK: &str = "#111827";
const DEFAULT: &str = "2, 190\n3, 160\n6, 130\n10, 90\n14, 70\n17, 55\n19, 40\n19, 55\n16, 85\n11, 125\n6, 170\n3, 195";

pub fn def() -> FigureDef {
    FigureDef {
        id: "climate-graph",
        subject: Subject::Geography,
        title: "Climate graph",
        blurb: "A climate graph from twelve months of temperature and rainfall, with rainfall bars and a temperature line on twin axes and a summary, or a table with empty axes for students to plot.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "climate-graph" => vec![
            Example { name: "Wet temperate climate", params: vec![("place", s("Bergen, Norway"))] },
            Example { name: "Hot desert", params: vec![("place", s("Aswan, Egypt")), ("data", s("15, 0\n17, 0\n21, 0\n27, 0\n31, 1\n34, 0\n34, 1\n34, 1\n32, 0\n28, 1\n21, 1\n16, 0"))] },
            Example { name: "Tropical monsoon", params: vec![("place", s("Mumbai, India")), ("data", s("24, 1\n24, 1\n27, 0\n29, 3\n30, 11\n28, 505\n27, 840\n27, 540\n27, 315\n28, 64\n27, 13\n25, 2"))] },
            Example { name: "Cold, below zero", params: vec![("place", s("Yakutsk, Russia")), ("data", s("-39, 7\n-34, 5\n-22, 5\n-7, 8\n7, 19\n17, 38\n20, 46\n16, 42\n7, 28\n-8, 20\n-27, 12\n-37, 8"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("data", "Temperature (\u{b0}C) and rainfall (mm), January to December", 2, DEFAULT).hint("Twelve rows of temperature then rainfall, January first"),
        ParamDef::text("place", "Place", "Bergen, Norway"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the table and the empty axes and leaves the bars, the line and the summary for students"),
    ]
}

/// The smallest of 1, 2, 5 times a power of ten whose five steps reach `max` (at least a step of 1).
fn step_for(max: f64) -> f64 {
    let mut m = 1.0;
    loop {
        for f in [1.0, 2.0, 5.0] {
            if 5.0 * f * m >= max {
                return f * m;
            }
        }
        m *= 10.0;
        if m > 1e12 {
            return m;
        }
    }
}

struct Summary {
    rain_total: f64,
    t_min: (f64, usize),
    t_max: (f64, usize),
    wettest: usize,
    driest: usize,
}

fn summarise(t: &[f64], r: &[f64]) -> Summary {
    let idx = |v: &[f64], f: fn(f64, f64) -> bool| (0..v.len()).fold(0, |b, i| if f(v[i], v[b]) { i } else { b });
    let (lo, hi) = (idx(t, |a, b| a < b), idx(t, |a, b| a > b));
    Summary { rain_total: r.iter().sum(), t_min: (t[lo], lo), t_max: (t[hi], hi), wettest: idx(r, |a, b| a > b), driest: idx(r, |a, b| a < b) }
}

fn render(p: &Params) -> String {
    let data = match rows(p.s("data"), 2, 12, 12, -90.0, 1.0e5, "a temperature or rainfall") {
        Ok(v) => v,
        Err(e) => return chart::message(&if e.contains("limited") || e.contains("Give") { "Give twelve months, one row of temperature and rainfall each".to_string() } else { e }),
    };
    if data.iter().any(|r| r[1] < 0.0 || r[0] > 70.0 || r[1] > 20000.0) {
        return chart::message("Temperatures are \u{2212}90 to 70 \u{b0}C and rainfall 0 to 20000 mm");
    }
    let t: Vec<f64> = data.iter().map(|r| r[0]).collect();
    let r: Vec<f64> = data.iter().map(|r| r[1]).collect();
    let sm = summarise(&t, &r);
    let answers = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let place: String = p.s("place").trim().chars().take(40).collect();
    let heading = match (title.is_empty(), place.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("Climate of {place}"),
        (false, _) => title.to_string(),
    };
    let top = if heading.is_empty() { 0.0 } else { 30.0 };
    let (w, ph) = (900.0, 380.0);
    let h = top + ph + 40.0 + 130.0 + 110.0;
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !heading.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", &heading);
    }
    let rstep = step_for(r.iter().cloned().fold(0.0, f64::max));
    // temperature axis: five steps from at or below 0 up to the warmest month
    let tmin = t.iter().cloned().fold(f64::INFINITY, f64::min).min(0.0);
    let tmax = t.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut tstep = 5.0;
    let mut tlo = (tmin / tstep).floor() * tstep;
    while tlo + 5.0 * tstep < tmax || tlo > tmin {
        tstep = if tstep < 5.0 { 5.0 } else { tstep * 2.0 };
        tlo = (tmin / tstep).floor() * tstep;
        if tstep > 1e6 {
            break;
        }
    }
    let f = Frame { w, h, l: 84.0, r: 84.0, t: top + 24.0, b: h - top - ph - 24.0 + 0.0, x0: 0.0, x1: 12.0, y0: 0.0, y1: 5.0 * rstep };
    let yt: Vec<(f64, String)> = (0..=5).map(|i| (i as f64 * rstep, chart::fmt(i as f64 * rstep))).collect();
    chart::axes(&mut d, &f, &AxesOpts { xticks: &[], yticks: &yt, grid: true, x_title: "", y_title: "Rainfall (mm)", fs });
    // right axis
    d.line(f.right(), f.top(), f.right(), f.bottom(), INK, 1.5);
    for i in 0..=5 {
        let y = f.py(i as f64 * rstep);
        d.line(f.right(), y, f.right() + 5.0, y, INK, 1.2);
        chart::text(&mut d, f.right() + 9.0, y + fs * 0.35, fs, "start", &chart::fmt(tlo + i as f64 * tstep));
    }
    chart::vtext(&mut d, f.right() + 62.0, (f.top() + f.bottom()) / 2.0, fs * 1.05, "Temperature (\u{b0}C)");
    for (i, m) in MONTHS.iter().enumerate() {
        chart::text(&mut d, f.px(i as f64 + 0.5), f.bottom() + fs * 1.5, fs, "middle", &m[..1]);
    }
    let ty = |v: f64| f.py((v - tlo) / (5.0 * tstep) * 5.0 * rstep);
    if answers {
        for (i, v) in r.iter().enumerate() {
            let (x0, x1) = (f.px(i as f64 + 0.15), f.px(i as f64 + 0.85));
            d.rect(x0, f.py(*v), x1 - x0, f.py(0.0) - f.py(*v), &fills.series(0), INK, 1.0);
        }
        let pts: Vec<(f64, f64)> = t.iter().enumerate().map(|(i, v)| (f.px(i as f64 + 0.5), ty(*v))).collect();
        d.polyline(&pts, fills.line(1), 3.0);
        for (x, y) in &pts {
            d.circle(*x, *y, 4.0, "#fff", INK, 1.8);
        }
    }
    // the table
    let ty0 = f.bottom() + fs * 3.4;
    let (cx0, cw) = (f.left(), f.right() - f.left());
    let lw = 96.0;
    let colw = (cw - lw) / 12.0;
    for (ri, (label, vals)) in [("Temp (\u{b0}C)", &t), ("Rain (mm)", &r)].iter().enumerate() {
        let y = ty0 + (ri as f64 + 1.0) * 26.0;
        d.rect(cx0, y - 18.0, cw, 26.0, "none", INK, 1.0);
        chart::text(&mut d, cx0 + 6.0, y, fs, "start", label);
        for (i, v) in vals.iter().enumerate() {
            chart::text(&mut d, cx0 + lw + colw * (i as f64 + 0.5), y, fs, "middle", &trim(*v, 1));
        }
    }
    d.rect(cx0, ty0 - 18.0, cw, 26.0, "none", INK, 1.0);
    chart::text(&mut d, cx0 + 6.0, ty0, fs, "start", "Month");
    for (i, m) in MONTHS.iter().enumerate() {
        chart::text(&mut d, cx0 + lw + colw * (i as f64 + 0.5), ty0, fs, "middle", m);
    }
    // the summary
    let mut y = ty0 + 3.0 * 26.0 + 30.0;
    let line = |d: &mut Doc, y: f64, label: &str, val: &str| {
        chart::text(d, cx0, y, fs, "start", label);
        let x = cx0 + 250.0;
        if answers {
            chart::text(d, x, y, fs, "start", val);
        } else {
            d.line(x, y + 3.0, x + 260.0, y + 3.0, INK, 1.0);
        }
    };
    line(&mut d, y, "Total annual rainfall (mm):", &trim(sm.rain_total, 1));
    y += fs * 1.9;
    line(&mut d, y, "Annual temperature range (\u{b0}C):", &format!("{} ({} to {})", trim(sm.t_max.0 - sm.t_min.0, 1), trim(sm.t_min.0, 1), trim(sm.t_max.0, 1)));
    y += fs * 1.9;
    line(&mut d, y, "Warmest and coldest month:", &format!("{} and {}", MONTHS[sm.t_max.1], MONTHS[sm.t_min.1]));
    y += fs * 1.9;
    line(&mut d, y, "Wettest and driest month:", &format!("{} and {}", MONTHS[sm.wettest], MONTHS[sm.driest]));
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn summary_finds_the_extremes() {
        let t = [2.0, 3.0, 6.0, 10.0, 14.0, 17.0, 19.0, 19.0, 16.0, 11.0, 6.0, 3.0];
        let r = [190.0, 160.0, 130.0, 90.0, 70.0, 55.0, 40.0, 55.0, 85.0, 125.0, 170.0, 195.0];
        let s = summarise(&t, &r);
        assert_eq!((s.t_min.1, s.t_max.1, s.wettest, s.driest), (0, 6, 11, 6));
        assert_eq!(s.rain_total, 1365.0);
    }

    #[test]
    fn axis_steps_reach_the_data() {
        assert_eq!(step_for(0.0), 1.0);
        assert_eq!(step_for(195.0), 50.0);
        assert_eq!(step_for(840.0), 200.0);
        assert!(5.0 * step_for(1234.0) >= 1234.0);
    }

    #[test]
    fn key_writes_the_summary() {
        let svg = render(&defaults());
        assert!(svg.contains(">1365<") && svg.contains("Jul and Jan") && svg.contains("Dec and Jul") && svg.contains(">Rainfall (mm)<"));
        assert!(svg.contains("Climate of Bergen, Norway"));
    }

    #[test]
    fn student_copy_has_the_table_but_not_the_graph() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">190<") && student.contains(">Total annual rainfall (mm):<") && !student.contains(">1365<") && !student.contains("Jul and Jan"));
        assert!(student.matches("<rect").count() < key.matches("<rect").count());
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn below_zero_temperatures_get_an_axis() {
        let mut p = defaults();
        p.set_s("data", "-39, 7\n-34, 5\n-22, 5\n-7, 8\n7, 19\n17, 38\n20, 46\n16, 42\n7, 28\n-8, 20\n-27, 12\n-37, 8");
        let svg = render(&p);
        assert!(svg.contains("\u{2212}40") && sane(&svg));
    }

    #[test]
    fn bad_data_is_explained() {
        for (spec, needle) in [("", "twelve months"), ("1, 2", "twelve months"), ("a, b", "not a"), ("1, -5\n".repeat(12).as_str(), "rainfall 0 to"), ("99, 1\n".repeat(12).as_str(), "70")] {
            let mut p = defaults();
            p.set_s("data", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("climate-graph").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)) && !render(&p).contains("limited"), "{}", ex.name);
        }
        for spec in ["nan, 1\n".repeat(12), "1e999, 1\n".repeat(12), "<b>, 1\n".repeat(12), "0, 0\n".repeat(12), "70, 20000\n".repeat(12), "\u{0}".to_string()] {
            for place in ["", "<script>", &"x".repeat(5000)] {
                let mut p = defaults();
                p.set_s("data", &spec);
                p.set_s("place", place);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<script>"), "{spec:?}");
            }
        }
    }
}
