//! Supply and demand: two straight curves from their intercepts and slopes, optional shifts of
//! either, a price ceiling or floor, and the equilibrium with consumer and producer surplus. The
//! curves and the controls are the question; the equilibrium, the shading, the guide lines, the
//! shortage or surplus and the readout are the answers, so the student copy (Show answers off)
//! draws only the curves, the labels and any price control.
use super::{clip_line, dot, draw_axes, draw_line, guides, n2, nice_max, plane, tag};
use crate::figures::chart;
use crate::figures::svg::{self, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const SHIFTS: [&str; 3] = ["None", "Right (increase)", "Left (decrease)"];
const CONTROLS: [&str; 3] = ["None", "Price ceiling", "Price floor"];
const SHADING: [&str; 4] = ["None", "Consumer surplus", "Producer surplus", "Both"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "supply-demand",
        subject: Subject::Economics,
        title: "Supply and demand",
        blurb: "Supply and demand curves from their intercepts and slopes, with shifts, a price ceiling or floor, the equilibrium and the consumer and producer surplus, or only the curves for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("supply-demand", "d_shift_by") => p.s("d_shift") != SHIFTS[0],
        ("supply-demand", "s_shift_by") => p.s("s_shift") != SHIFTS[0],
        ("supply-demand", "control_at") => p.s("control") != CONTROLS[0],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "supply-demand" => vec![
            Example { name: "Equilibrium with surplus", params: vec![] },
            Example { name: "Demand increases", params: vec![("d_shift", s(SHIFTS[1])), ("shading", s(SHADING[0]))] },
            Example { name: "Supply decreases", params: vec![("s_shift", s(SHIFTS[2])), ("s_shift_by", Value::F(20.0)), ("shading", s(SHADING[0]))] },
            Example { name: "Price ceiling: a shortage", params: vec![("control", s(CONTROLS[1])), ("control_at", Value::F(40.0)), ("shading", s(SHADING[0]))] },
            Example { name: "Price floor: a surplus", params: vec![("control", s(CONTROLS[2])), ("control_at", Value::F(75.0)), ("shading", s(SHADING[0]))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("d_int", "Demand: price at quantity 0", 1.0, 1.0e6, 1.0, 100.0),
        ParamDef::float("d_slope", "Demand: slope (price falls per unit)", 0.01, 1.0e4, 0.1, 1.0),
        ParamDef::float("s_int", "Supply: price at quantity 0", 0.0, 1.0e6, 1.0, 20.0),
        ParamDef::float("s_slope", "Supply: slope (price rises per unit)", 0.01, 1.0e4, 0.1, 1.0),
        ParamDef::choice("d_shift", "Shift demand", &SHIFTS, SHIFTS[0]),
        ParamDef::float("d_shift_by", "Demand shift (units of quantity)", 1.0, 1.0e6, 1.0, 20.0),
        ParamDef::choice("s_shift", "Shift supply", &SHIFTS, SHIFTS[0]),
        ParamDef::float("s_shift_by", "Supply shift (units of quantity)", 1.0, 1.0e6, 1.0, 20.0),
        ParamDef::choice("control", "Price control", &CONTROLS, CONTROLS[0]),
        ParamDef::float("control_at", "Control price", 0.0, 1.0e6, 1.0, 40.0),
        ParamDef::choice("shading", "Shade surplus", &SHADING, SHADING[3]),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off draws only the curves, their labels and any price control: no equilibrium, shading, guide lines or readout"),
    ]
}

fn finite_in(v: f64, lo: f64, hi: f64) -> bool {
    v.is_finite() && v >= lo && v <= hi
}

fn render(p: &Params) -> String {
    let (a, b, c, e) = (p.f("d_int"), p.f("d_slope"), p.f("s_int"), p.f("s_slope"));
    if !(finite_in(a, 1.0, 1.0e6) && finite_in(b, 0.01, 1.0e4) && finite_in(c, 0.0, 1.0e6) && finite_in(e, 0.01, 1.0e4)) {
        return chart::message("Enter the intercepts and slopes as positive numbers");
    }
    let shift = |kind: &str, by: f64| -> Result<f64, String> {
        let by = if finite_in(by, 0.0, 1.0e6) { by } else { return Err("Enter the shifts as numbers".into()) };
        Ok(match kind {
            "Right (increase)" => by,
            "Left (decrease)" => -by,
            _ => 0.0,
        })
    };
    let (sd, ss) = match (shift(p.s("d_shift"), p.f("d_shift_by")), shift(p.s("s_shift"), p.f("s_shift_by"))) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(m), _) | (_, Err(m)) => return chart::message(&m),
    };
    // P = a2 - b Q for demand; P = c2 + e Q for supply (a right shift of s lowers its intercept)
    let (a2, c2) = (a + b * sd, c - e * ss);
    let shifted = sd != 0.0 || ss != 0.0;
    if c2 < 0.0 {
        return chart::message("Supply would start below a price of 0: shift it by less");
    }
    let qe = (a2 - c2) / (b + e);
    let pe = a2 - b * qe;
    if !(qe > 0.0 && pe > 0.0) {
        return chart::message("The curves do not cross above zero: change the intercepts or the shift");
    }
    let (qe0, pe0) = ((a - c) / (b + e), a - b * (a - c) / (b + e));
    let ymax = nice_max(a.max(a2).max(c2) * 1.04);
    let xmax = nice_max((a.max(a2) / b).max(qe * 1.6).max(qe0 * 1.3));
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let fills = Fills::new(p);
    let answers = p.b(SHOW_ANSWERS);
    let title = p.s("title").trim();
    let (mut d, f) = plane(xmax, ymax, 64.0);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, f.w / 2.0, 16.0, fs * 1.2, "middle", title);
    }
    // price control
    let control = p.s("control");
    let pc = p.f("control_at");
    let pc_ok = finite_in(pc, 0.0, ymax);
    let (mut qd_c, mut qs_c) = (0.0, 0.0);
    let binding = control != CONTROLS[0] && pc_ok && ((control == CONTROLS[1] && pc < pe) || (control == CONTROLS[2] && pc > pe));
    if binding {
        qd_c = ((a2 - pc) / b).max(0.0);
        qs_c = ((pc - c2) / e).max(0.0);
    }
    if answers {
        let cs = matches!(p.s("shading"), "Consumer surplus" | "Both");
        let ps = matches!(p.s("shading"), "Producer surplus" | "Both");
        if cs {
            let pts = [(0.0, pe), (0.0, a2), (qe, pe)];
            d.poly(&pts.iter().map(|(q, pr)| (f.px(*q), f.py(*pr))).collect::<Vec<_>>(), &fills.series(0), "none", 0.0);
        }
        if ps {
            let pts = [(0.0, pe), (qe, pe), (0.0, c2)];
            d.poly(&pts.iter().map(|(q, pr)| (f.px(*q), f.py(*pr))).collect::<Vec<_>>(), &fills.series(1), "none", 0.0);
        }
    }
    draw_axes(&mut d, &f, p.b("numbers"), fs, "Quantity", "Price");
    // the original curves stay as dashed grey when something shifts
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
    let (dn, sn) = (if sd != 0.0 { "D2" } else { "D" }, if ss != 0.0 { "S2" } else { "S" });
    let _ = dl;
    tag(&mut d, &f, -b, a2, 0.88, dn, fs);
    if let Some((q, pr)) = sl {
        chart::text(&mut d, f.px(q) - 4.0, f.py(pr) - 8.0, fs * 1.1, "end", sn);
    }
    if shifted {
        if sd != 0.0 {
            if let Some((qa, pa)) = clip_line(&f, -b, a).map(|x| x.0) {
                chart::text(&mut d, f.px(qa) + 8.0, f.py(pa) + fs, fs, "start", "D1");
            }
        }
        if ss != 0.0 {
            if let Some((q, pr)) = clip_line(&f, e, c).map(|x| x.1) {
                chart::text(&mut d, f.px(q) - 4.0, f.py(pr) - 8.0, fs, "end", "S1");
            }
        }
    }
    if control != CONTROLS[0] && pc_ok {
        let y = f.py(pc);
        d.line(f.left(), y, f.right(), y, "#111827", 2.2);
        chart::text(&mut d, f.right() - 4.0, y - 6.0, fs, "end", control);
        if answers && binding {
            let (x1, x2) = (f.px(qs_c.min(qd_c)), f.px(qs_c.max(qd_c)));
            d.line(x1, y + 14.0, x2, y + 14.0, "#111827", 2.0);
            d.line(x1, y + 9.0, x1, y + 19.0, "#111827", 2.0);
            d.line(x2, y + 9.0, x2, y + 19.0, "#111827", 2.0);
            chart::text(&mut d, (x1 + x2) / 2.0, y + 14.0 + fs * 1.3, fs, "middle", if control == CONTROLS[1] { "Shortage" } else { "Surplus" });
        }
    }
    if answers {
        guides(&mut d, &f, qe, pe);
        dot(&mut d, &f, qe, pe);
        let mut lines = vec![format!("Equilibrium: quantity {}, price {}", n2(qe), n2(pe))];
        if shifted {
            lines[0] = format!("New equilibrium: quantity {}, price {}  (was {} and {})", n2(qe), n2(pe), n2(qe0), n2(pe0));
        }
        if binding {
            let gap = (qd_c - qs_c).abs();
            lines.push(format!("{} at {}: demanded {}, supplied {}, {} of {}", control, n2(pc), n2(qd_c), n2(qs_c), if control == CONTROLS[1] { "shortage" } else { "surplus" }, n2(gap)));
        } else if control != CONTROLS[0] && pc_ok {
            lines.push(format!("The {} is not binding: it does not change the market price", control.to_lowercase()));
        }
        let (cs, ps) = (0.5 * (a2 - pe) * qe, 0.5 * (pe - c2) * qe);
        if p.s("shading") != SHADING[0] {
            lines.push(format!("Consumer surplus {}, producer surplus {}", n2(cs), n2(ps)));
        }
        for (i, t) in lines.iter().enumerate() {
            chart::text(&mut d, f.left(), f.h + 18.0 + i as f64 * fs * 1.5, fs, "start", t);
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
    fn key_works_out_the_equilibrium_and_surplus() {
        let svg = render(&defaults());
        // 100 - Q = 20 + Q: Q 40, P 60; surplus 0.5 * 40 * 40 each
        assert!(svg.contains("Equilibrium: quantity 40, price 60") && svg.contains("Consumer surplus 800, producer surplus 800"));
    }

    #[test]
    fn a_demand_shift_moves_the_equilibrium_and_keeps_the_old_curve() {
        let mut p = defaults();
        p.set_s("d_shift", SHIFTS[1]);
        let svg = render(&p);
        // demand 120 - Q against 20 + Q: Q 50, P 70
        assert!(svg.contains("New equilibrium: quantity 50, price 70  (was 40 and 60)") && svg.contains(">D2<") && svg.contains(">D1<"));
        p.set_s("d_shift", SHIFTS[2]);
        assert!(render(&p).contains("quantity 30, price 50"));
    }

    #[test]
    fn price_controls_bind_only_on_the_right_side() {
        let mut p = defaults();
        p.set_s("control", CONTROLS[1]);
        p.set_f("control_at", 40.0);
        // demanded 60, supplied 20
        assert!(render(&p).contains("Price ceiling at 40: demanded 60, supplied 20, shortage of 40"));
        p.set_f("control_at", 80.0);
        assert!(render(&p).contains("not binding"));
        p.set_s("control", CONTROLS[2]);
        assert!(render(&p).contains("Price floor at 80: demanded 20, supplied 60, surplus of 40"));
    }

    #[test]
    fn student_copy_has_curves_but_no_answers() {
        let def = def();
        let mut p = defaults();
        p.set_s("control", CONTROLS[1]);
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert!(student.contains(">D<") && student.contains(">S<") && student.contains(">Price ceiling<"));
        assert!(!student.contains("Equilibrium") && !student.contains("Shortage") && !student.contains("stroke-dasharray=\"5 4\""));
        assert!(key.contains("Equilibrium") && key.contains("Shortage"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn impossible_markets_are_explained() {
        let mut p = defaults();
        p.set_f("s_int", 200.0);
        assert!(render(&p).contains("do not cross"));
        let mut p = defaults();
        p.set_s("s_shift", SHIFTS[1]);
        p.set_f("s_shift_by", 50.0);
        assert!(render(&p).contains("below a price of 0"));
        let mut p = defaults();
        p.set_f("d_slope", f64::NAN);
        assert!(render(&p).contains("positive numbers"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_s("control", CONTROLS[2]);
        p.set_f("control_at", 75.0);
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("supply-demand").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in [0.0, -5.0, 1.0e12, f64::NAN, f64::INFINITY, 1.0e-9] {
            for key in ["d_int", "d_slope", "s_int", "s_slope", "d_shift_by", "s_shift_by", "control_at"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_f(key, v);
                    p.set_s("d_shift", SHIFTS[1]);
                    p.set_s("control", CONTROLS[1]);
                    p.set_b(SHOW_ANSWERS, answers);
                    assert!(sane(&render(&p)), "{key} {v}");
                }
            }
        }
    }
}
