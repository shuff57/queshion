//! Aggregate demand and supply: a downward aggregate demand curve, an upward short-run aggregate
//! supply curve and a vertical long-run aggregate supply at full employment, with optional shifts
//! of demand or short-run supply, the macroeconomic equilibrium and the output gap. The curves
//! are the question; the equilibrium, guide lines, gap and readout are the answers, so the
//! student copy (Show answers off) draws only the curves and their labels.
use super::{clip_line, dot, draw_axes, draw_line, guides, n2, nice_max, plane, tag};
use crate::figures::chart;
use crate::figures::svg::{self, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const SHIFTS: [&str; 3] = ["None", "Right (increase)", "Left (decrease)"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "ad-as",
        subject: Subject::Economics,
        title: "Aggregate demand and supply",
        blurb: "AD, short-run AS and long-run AS with optional shifts, the new equilibrium and the output gap (recessionary or inflationary), or only the curves for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "ad-as" => vec![
            Example { name: "Full-employment equilibrium", params: vec![] },
            Example { name: "AD falls: a recessionary gap", params: vec![("ad_shift", s(SHIFTS[2]))] },
            Example { name: "AD rises: an inflationary gap", params: vec![("ad_shift", s(SHIFTS[1]))] },
            Example { name: "A supply shock", params: vec![("sras_shift", s(SHIFTS[2]))] },
            Example { name: "Blank for students", params: vec![("ad_shift", s(SHIFTS[2])), ("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("ad_int", "AD: price level at output 0", 1.0, 1.0e6, 1.0, 200.0),
        ParamDef::float("ad_slope", "AD: slope", 0.01, 1.0e4, 0.1, 1.0),
        ParamDef::float("as_int", "SRAS: price level at output 0", 0.0, 1.0e6, 1.0, 40.0),
        ParamDef::float("as_slope", "SRAS: slope", 0.01, 1.0e4, 0.1, 1.0),
        ParamDef::float("full", "Full-employment output (LRAS)", 1.0, 1.0e6, 1.0, 80.0),
        ParamDef::choice("ad_shift", "Shift AD", &SHIFTS, SHIFTS[0]),
        ParamDef::float("ad_by", "AD shift (units of output)", 1.0, 1.0e6, 1.0, 20.0),
        ParamDef::choice("sras_shift", "Shift SRAS", &SHIFTS, SHIFTS[0]),
        ParamDef::float("sras_by", "SRAS shift (units of output)", 1.0, 1.0e6, 1.0, 20.0),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off draws the curves and their labels only: no equilibrium, guide lines, gap or readout"),
    ]
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("ad-as", "ad_by") => p.s("ad_shift") != SHIFTS[0],
        ("ad-as", "sras_by") => p.s("sras_shift") != SHIFTS[0],
        _ => true,
    }
}

fn render(p: &Params) -> String {
    let (a, b, c, e, full) = (p.f("ad_int"), p.f("ad_slope"), p.f("as_int"), p.f("as_slope"), p.f("full"));
    let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
    if !(ok(a, 1.0, 1.0e6) && ok(b, 0.01, 1.0e4) && ok(c, 0.0, 1.0e6) && ok(e, 0.01, 1.0e4) && ok(full, 1.0, 1.0e6) && ok(p.f("ad_by"), 0.0, 1.0e6) && ok(p.f("sras_by"), 0.0, 1.0e6)) {
        return chart::message("Enter the intercepts, slopes and shifts as numbers");
    }
    let sh = |kind: &str, by: f64| match kind {
        "Right (increase)" => by,
        "Left (decrease)" => -by,
        _ => 0.0,
    };
    let (sd, ss) = (sh(p.s("ad_shift"), p.f("ad_by")), sh(p.s("sras_shift"), p.f("sras_by")));
    let (a2, c2) = (a + b * sd, c - e * ss);
    if c2 < 0.0 {
        return chart::message("SRAS would start below a price level of 0: shift it by less");
    }
    let ye = (a2 - c2) / (b + e);
    let pe = a2 - b * ye;
    if !(ye > 0.0 && pe > 0.0) {
        return chart::message("The curves do not cross above zero: change the intercepts or the shifts");
    }
    let ye0 = (a - c) / (b + e);
    let pe0 = a - b * ye0;
    let shifted = sd != 0.0 || ss != 0.0;
    let ymax = nice_max(a.max(a2).max(c2) * 1.04);
    let xmax = nice_max((a.max(a2) / b).max(ye * 1.5).max(full * 1.5));
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let fills = Fills::new(p);
    let answers = p.b(SHOW_ANSWERS);
    let title = p.s("title").trim();
    let (mut d, f) = plane(xmax, ymax, 14.0 + 3.0 * fs * 1.75);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, f.w / 2.0, 16.0, fs * 1.2, "middle", title);
    }
    draw_axes(&mut d, &f, p.b("numbers"), fs, "Real GDP (output)", "Price level");
    // long-run aggregate supply, vertical at full employment
    let xl = f.px(full);
    d.raw(&format!(r##"<line x1="{xl:.2}" y1="{:.2}" x2="{xl:.2}" y2="{:.2}" stroke="#111827" stroke-width="3" stroke-dasharray="{}"/>"##, f.top(), f.bottom(), fills.dash(2).replace(' ', ",")));
    chart::text(&mut d, xl + 6.0, f.top() + fs, fs * 1.05, "start", "LRAS");
    if shifted {
        if sd != 0.0 {
            draw_line(&mut d, &f, -b, a, "#9ca3af", "6 4", 2.0);
        }
        if ss != 0.0 {
            draw_line(&mut d, &f, e, c, "#9ca3af", "6 4", 2.0);
        }
    }
    let dl = draw_line(&mut d, &f, -b, a2, fills.line(0), fills.dash(0), 3.0);
    let sl = draw_line(&mut d, &f, e, c2, fills.line(1), fills.dash(1), 3.0);
    let (dn, sn) = (if sd != 0.0 { "AD2" } else { "AD" }, if ss != 0.0 { "SRAS2" } else { "SRAS" });
    let _ = dl;
    tag(&mut d, &f, -b, a2, 0.88, dn, fs);
    if let Some((q, v)) = sl {
        chart::text(&mut d, f.px(q) - 4.0, f.py(v) - 8.0, fs * 1.05, "end", sn);
    }
    if shifted {
        if sd != 0.0 {
            if let Some((q, v)) = clip_line(&f, -b, a).map(|x| x.0) {
                chart::text(&mut d, f.px(q) + 8.0, f.py(v) + fs, fs, "start", "AD1");
            }
        }
        if ss != 0.0 {
            if let Some((q, v)) = clip_line(&f, e, c).map(|x| x.1) {
                chart::text(&mut d, f.px(q) - 4.0, f.py(v) - 8.0, fs, "end", "SRAS1");
            }
        }
    }
    if answers {
        guides(&mut d, &f, ye, pe);
        dot(&mut d, &f, ye, pe);
        let gap = ye - full;
        let tol = full * 0.005;
        if gap.abs() > tol {
            let y = f.bottom() - 16.0;
            let (x1, x2) = (f.px(ye.min(full)), f.px(ye.max(full)));
            d.line(x1, y, x2, y, "#111827", 2.2);
            d.line(x1, y - 6.0, x1, y + 6.0, "#111827", 2.2);
            d.line(x2, y - 6.0, x2, y + 6.0, "#111827", 2.2);
            chart::text(&mut d, (x1 + x2) / 2.0, y - 10.0, fs * 0.9, "middle", "gap");
        }
        let verdict = if gap.abs() <= tol {
            "Output is at full employment: no gap".to_string()
        } else if gap < 0.0 {
            format!("Recessionary gap: output {} is {} below full employment {}", n2(ye), n2(-gap), n2(full))
        } else {
            format!("Inflationary gap: output {} is {} above full employment {}", n2(ye), n2(gap), n2(full))
        };
        let l1 = if shifted { format!("New equilibrium: output {}, price level {}  (was {} and {})", n2(ye), n2(pe), n2(ye0), n2(pe0)) } else { format!("Equilibrium: output {}, price level {}", n2(ye), n2(pe)) };
        let l3 = if shifted {
            let dir = |x: f64, y: f64| if x > y + 1e-9 { "rises" } else if x < y - 1e-9 { "falls" } else { "is unchanged" };
            format!("Output {}, the price level {}", dir(ye, ye0), dir(pe, pe0))
        } else {
            String::new()
        };
        for (i, t) in [l1, verdict, l3].iter().enumerate() {
            chart::text(&mut d, f.left(), f.h + 20.0 + i as f64 * fs * 1.75, fs, "start", t);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn default_sits_at_full_employment() {
        // 200 - Y = 40 + Y: Y 80, P 120 = the LRAS at 80
        let svg = render(&defaults());
        assert!(svg.contains("Equilibrium: output 80, price level 120") && svg.contains("no gap"));
    }

    #[test]
    fn a_fall_in_demand_opens_a_recessionary_gap() {
        let mut p = defaults();
        p.set_s("ad_shift", SHIFTS[2]);
        // AD 180 - Y: Y 70, P 110
        let svg = render(&p);
        assert!(svg.contains("New equilibrium: output 70, price level 110  (was 80 and 120)"));
        assert!(svg.contains("Recessionary gap: output 70 is 10 below full employment 80") && svg.contains("Output falls, the price level falls"));
    }

    #[test]
    fn a_rise_in_demand_opens_an_inflationary_gap() {
        let mut p = defaults();
        p.set_s("ad_shift", SHIFTS[1]);
        assert!(render(&p).contains("Inflationary gap: output 90 is 10 above"));
    }

    #[test]
    fn a_supply_shock_raises_prices_and_lowers_output() {
        let mut p = defaults();
        p.set_s("sras_shift", SHIFTS[2]);
        // SRAS 60 + Y: Y 70, P 130
        let svg = render(&p);
        assert!(svg.contains("output 70, price level 130") && svg.contains("Output falls, the price level rises"));
    }

    #[test]
    fn student_copy_has_curves_only() {
        let mut p = defaults();
        p.set_s("ad_shift", SHIFTS[2]);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(student.contains(">AD2<") && student.contains(">AD1<") && student.contains(">SRAS<") && student.contains(">LRAS<"));
        assert!(!student.contains("equilibrium") && !student.contains("gap") && key.contains("gap"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn impossible_curves_are_explained() {
        let mut p = defaults();
        p.set_f("as_int", 500.0);
        assert!(render(&p).contains("do not cross"));
        let mut p = defaults();
        p.set_s("sras_shift", SHIFTS[1]);
        p.set_f("sras_by", 100.0);
        assert!(render(&p).contains("below a price level of 0"));
        let mut p = defaults();
        p.set_f("full", f64::NAN);
        assert!(render(&p).contains("as numbers"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("ad_shift", SHIFTS[2]);
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("ad-as").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in [0.0, -5.0, 1.0e12, f64::NAN, f64::INFINITY, 1.0e-9] {
            for key in ["ad_int", "ad_slope", "as_int", "as_slope", "full", "ad_by", "sras_by"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_f(key, v);
                    p.set_s("ad_shift", SHIFTS[1]);
                    p.set_s("sras_shift", SHIFTS[2]);
                    p.set_b(SHOW_ANSWERS, answers);
                    assert!(sane(&render(&p)), "{key} {v}");
                }
            }
        }
    }
}
