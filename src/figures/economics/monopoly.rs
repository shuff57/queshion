//! A monopolist: a straight demand curve, its marginal revenue curve (twice as steep), a rising
//! marginal cost and the average total cost, with the profit-maximising output where MR = MC, the
//! price read off demand and the profit rectangle; optionally the competitive output and the
//! deadweight loss. The curves are the question; the output, price, rectangle, triangle and
//! readout are the answers, so the student copy (Show answers off) leaves them out.
use super::{dot, draw_axes, draw_line, guides, n2, nice_max, plane, tag};
use crate::figures::chart;
use crate::figures::svg::{self, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn def() -> FigureDef {
    FigureDef {
        id: "monopoly",
        subject: Subject::Economics,
        title: "Monopoly profit",
        blurb: "A monopolist's demand, marginal revenue, marginal cost and average total cost, with the profit-maximising output, the price, the profit rectangle and the deadweight loss, or only the curves for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    Some(match id {
        "monopoly" => vec![
            Example { name: "Profit and deadweight loss", params: vec![] },
            Example { name: "Constant marginal cost", params: vec![("mc_slope", Value::F(0.0))] },
            Example { name: "Making a loss", params: vec![("fixed", Value::F(900.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("d_int", "Demand: price at quantity 0", 1.0, 1.0e6, 1.0, 100.0),
        ParamDef::float("d_slope", "Demand: slope", 0.01, 1.0e4, 0.1, 1.0),
        ParamDef::float("mc_int", "Marginal cost at quantity 0", 0.0, 1.0e6, 1.0, 20.0),
        ParamDef::float("mc_slope", "Marginal cost: slope", 0.0, 1.0e4, 0.1, 0.5),
        ParamDef::float("fixed", "Fixed cost", 0.0, 1.0e9, 10.0, 200.0),
        ParamDef::bool("dwl", "Show the deadweight loss", true),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off draws the curves only: no output, price, profit rectangle, deadweight loss or readout"),
    ]
}

fn render(p: &Params) -> String {
    let (a, b, c, e, fx) = (p.f("d_int"), p.f("d_slope"), p.f("mc_int"), p.f("mc_slope"), p.f("fixed"));
    let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
    if !(ok(a, 1.0, 1.0e6) && ok(b, 0.01, 1.0e4) && ok(c, 0.0, 1.0e6) && ok(e, 0.0, 1.0e4) && ok(fx, 0.0, 1.0e9)) {
        return chart::message("Enter the intercepts, slopes and fixed cost as numbers");
    }
    if c >= a {
        return chart::message("Marginal cost starts at or above the demand curve: no output is worth making");
    }
    // MR = a - 2bQ meets MC = c + eQ
    let qm = (a - c) / (2.0 * b + e);
    let pm = a - b * qm;
    let qc = (a - c) / (b + e);
    let pc = a - b * qc;
    // TC = fixed + c Q + e Q^2 / 2, so ATC = fixed / Q + c + e Q / 2 and its marginal cost is MC
    let atc = fx / qm + c + e * qm / 2.0;
    let profit = (pm - atc) * qm;
    let ymax = nice_max(a * 1.04);
    let xmax = nice_max((a / b).max(qc * 1.5));
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
    let mc_at = |q: f64| c + e * q;
    if answers {
        if p.b("dwl") {
            let pts = [(qm, pm), (qc, pc), (qm, mc_at(qm))];
            d.poly(&pts.iter().map(|(q, v)| (f.px(*q), f.py(*v))).collect::<Vec<_>>(), &fills.series(3), "#111827", 1.0);
        }
        let (top, bot) = (pm.max(atc).min(ymax), pm.min(atc).min(ymax));
        let fill = if profit >= 0.0 { fills.series(2) } else { fills.series(1) };
        d.rect(f.px(0.0), f.py(top), f.px(qm) - f.px(0.0), f.py(bot) - f.py(top), &fill, "#111827", 1.2);
    }
    draw_axes(&mut d, &f, p.b("numbers"), fs, "Quantity", "Price and cost");
    draw_line(&mut d, &f, -b, a, fills.line(0), fills.dash(0), 3.0);
    draw_line(&mut d, &f, -2.0 * b, a, fills.line(1), fills.dash(1), 3.0);
    let ml = draw_line(&mut d, &f, e, c, fills.line(2), fills.dash(2), 3.0);
    tag(&mut d, &f, -b, a, 0.86, "D", fs);
    tag(&mut d, &f, -2.0 * b, a, 0.8, "MR", fs);
    if let Some((q, v)) = ml {
        chart::text(&mut d, f.px(q) - 4.0, f.py(v) - 8.0, fs * 1.05, "end", "MC");
    }
    // the average total cost curve (a hyperbola plus a line), drawn where it fits the plot
    let pts: Vec<(f64, f64)> = (1..=200)
        .map(|i| {
            let q = xmax * i as f64 / 200.0;
            (q, fx / q + c + e * q / 2.0)
        })
        .filter(|(_, v)| *v <= ymax)
        .map(|(q, v)| (f.px(q), f.py(v)))
        .collect();
    if pts.len() > 1 {
        d.polyline(&pts, fills.line(3), 3.0);
        if let Some(&(x, y)) = pts.last() {
            chart::text(&mut d, x + 6.0, y + 4.0, fs * 1.05, "start", "ATC");
        }
    }
    if answers {
        guides(&mut d, &f, qm, pm);
        dot(&mut d, &f, qm, pm);
        d.circle(f.px(qm), f.py(mc_at(qm)), 3.8, "#fff", "#111827", 1.6);
        chart::text(&mut d, f.px(qm), f.bottom() + 8.0 + fs * 2.1, fs, "middle", &format!("Q = {}", n2(qm)));
        let lines = [
            format!("MR = MC at output {}; price {} from the demand curve", n2(qm), n2(pm)),
            format!("{} = ({} \u{2212} {}) \u{d7} {} = {}", if profit >= 0.0 { "Profit" } else { "Loss" }, n2(pm), n2(atc), n2(qm), n2(profit.abs())),
            if p.b("dwl") { format!("Deadweight loss = {} (competitive: output {}, price {})", n2(0.5 * (pm - mc_at(qm)) * (qc - qm)), n2(qc), n2(pc)) } else { String::new() },
        ];
        for (i, t) in lines.iter().enumerate() {
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
    fn key_works_out_output_price_profit_and_loss() {
        let svg = render(&defaults());
        // MR 100 - 2Q = MC 20 + 0.5Q: Q 32, P 68; ATC = 200/32 + 20 + 8 = 34.25; profit 33.75 * 32 = 1080
        assert!(svg.contains("MR = MC at output 32; price 68"), "{svg}");
        assert!(svg.contains("Profit = (68 \u{2212} 34.25) \u{d7} 32 = 1,080") || svg.contains("Profit = (68 \u{2212} 34.25) \u{d7} 32 = 1080"));
        // competitive output (100 - 20) / 1.5 = 53.33; triangle 0.5 * (68 - 36) * 21.33 = 341.33
        assert!(svg.contains("Deadweight loss = 341.33"));
    }

    #[test]
    fn a_big_fixed_cost_makes_a_loss() {
        let mut p = defaults();
        p.set_f("fixed", 2000.0);
        assert!(render(&p).contains("Loss = (68 \u{2212} 90.5"));
    }

    #[test]
    fn constant_marginal_cost_works() {
        let mut p = defaults();
        p.set_f("mc_slope", 0.0);
        // MR 100 - 2Q = 20: Q 40, P 60
        assert!(render(&p).contains("MR = MC at output 40; price 60"));
    }

    #[test]
    fn student_copy_has_curves_only() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">MR<") && student.contains(">MC<") && student.contains(">D<") && student.contains(">ATC<"));
        assert!(!student.contains("Profit") && !student.contains("Deadweight") && key.contains("Deadweight"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_input_is_explained() {
        let mut p = defaults();
        p.set_f("mc_int", 100.0);
        assert!(render(&p).contains("at or above the demand"));
        let mut p = defaults();
        p.set_f("d_slope", f64::NAN);
        assert!(render(&p).contains("as numbers"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("monopoly").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in [0.0, -5.0, 1.0e12, f64::NAN, f64::INFINITY, 1.0e-9] {
            for key in ["d_int", "d_slope", "mc_int", "mc_slope", "fixed"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_f(key, v);
                    p.set_b(SHOW_ANSWERS, answers);
                    assert!(sane(&render(&p)), "{key} {v}");
                }
            }
        }
    }
}
