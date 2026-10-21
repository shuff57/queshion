//! The time value of money on a timeline: cash flows at whole periods drawn as arrows (up in,
//! down out), a rate per period, and a table that discounts each one to the present (or compounds
//! it to the last period). The flows and the rate are the question; the factors, the values and
//! the total are the answers, so the student copy (Show answers off) leaves them as empty cells.
use crate::figures::accounting::{cents_param, currency_param, money, symbol};
use crate::figures::chart;
use crate::figures::geography::{rows, trim};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MODES: [&str; 2] = ["Present value", "Future value"];
const MAX_PERIOD: f64 = 12.0;
const INK: &str = "#111827";
const DEFAULT: &str = "0, -1000\n1, 300\n2, 400\n3, 500\n4, 300";

pub fn def() -> FigureDef {
    FigureDef {
        id: "time-value",
        subject: Subject::Business,
        title: "Time value of money",
        blurb: "A timeline of cash flows with a rate per period, discounted to the present (or compounded to the last period) in a table, with the factors, values and net present value worked out or left blank for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "time-value" => vec![
            Example { name: "Investment appraisal", params: vec![] },
            Example { name: "Future value of savings", params: vec![("mode", s(MODES[1])), ("flows", s("0, 500\n1, 500\n2, 500\n3, 500")), ("rate", Value::F(4.0))] },
            Example { name: "One payment in three years", params: vec![("flows", s("3, 10000")), ("rate", Value::F(5.0))] },
            Example { name: "Zero rate", params: vec![("rate", Value::F(0.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("flows", "Cash flows: period, amount", 2, DEFAULT).hint("One row per cash flow: the period (0 to 12) then the amount, negative for money paid out; up to 10"),
        ParamDef::float("rate", "Rate per period (%)", 0.0, 100.0, 0.5, 8.0),
        ParamDef::choice("mode", "Work out", &MODES, MODES[0]),
        ParamDef::text("unit", "What a period is called", "Year"),
        currency_param(),
        cents_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the timeline and the given cash flows and leaves the factors, the values and the total empty for students"),
    ]
}

pub struct Flow {
    pub t: u32,
    pub amount: f64,
}

/// The factor that moves a flow at period `t` to the present (`1/(1+r)^t`) or on to `last` (`(1+r)^(last-t)`).
pub fn factor(rate: f64, t: u32, last: u32, present: bool) -> f64 {
    if present { (1.0 + rate).powi(-(t as i32)) } else { (1.0 + rate).powi((last - t) as i32) }
}

fn render(p: &Params) -> String {
    let rate = p.f("rate");
    if !rate.is_finite() {
        return chart::message("Give a rate as a number");
    }
    let rate = rate.clamp(0.0, 100.0) / 100.0;
    let data = match rows(p.s("flows"), 2, 1, 10, -1.0e9, 1.0e9, "a period or an amount") {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let mut flows: Vec<Flow> = Vec::new();
    for r in &data {
        if r[0].fract() != 0.0 || !(0.0..=MAX_PERIOD).contains(&r[0]) {
            return chart::message(&format!("Periods are whole numbers from 0 to {MAX_PERIOD}"));
        }
        flows.push(Flow { t: r[0] as u32, amount: r[1] });
    }
    flows.sort_by_key(|f| f.t);
    let last = flows.iter().map(|f| f.t).max().unwrap().max(1);
    let present = p.s("mode") != MODES[1];
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let sym = symbol(p).to_string();
    let cents = p.b("cents");
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let unit: String = { let u = p.s("unit").trim(); if u.is_empty() { "Period".to_string() } else { u.chars().take(12).collect() } };
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let rowh = 30.0;
    let (w, h) = (900.0, top + 270.0 + (flows.len() as f64 + 2.0) * rowh + 40.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    // the timeline
    let (ax, ay) = (60.0, top + 130.0);
    let span = w - 120.0;
    let px = |t: f64| ax + t / last as f64 * span;
    d.arrow(ax - 20.0, ay, ax + span + 30.0, ay, INK, 3.0);
    let maxabs = flows.iter().map(|f| f.amount.abs()).fold(0.0, f64::max).max(1e-9);
    let (up, down) = if print { ("#333333", "#333333") } else { ("#2f8f4e", "#c0392b") };
    for f in &flows {
        let len = 28.0 + 52.0 * f.amount.abs() / maxabs;
        let x = px(f.t as f64);
        if f.amount >= 0.0 {
            d.arrow(x, ay - 2.0, x, ay - len, up, 3.2);
            chart::text(&mut d, x, ay - len - 8.0, fs, "middle", &money(f.amount, &sym, cents));
        } else {
            d.arrow(x, ay + 2.0, x, ay + len, down, 3.2);
            chart::text(&mut d, x, ay + len + fs + 4.0, fs, "middle", &money(f.amount, &sym, cents));
        }
    }
    for t in 0..=last {
        let x = px(t as f64);
        d.rect(x - 22.0, ay - 10.0, 44.0, 20.0, "#fff", "none", 0.0);
        chart::text(&mut d, x, ay + fs * 0.35, fs * 0.85, "middle", &t.to_string());
    }
    chart::text(&mut d, ax + span + 24.0, ay + 24.0, fs * 0.9, "end", &format!("{unit}s"));
    chart::text(&mut d, ax - 20.0, ay - 90.0 - 36.0 + 18.0, fs, "start", &format!("Rate: {}% per {}", trim(rate * 100.0, 2), unit.to_lowercase()));
    // the table
    let ty = top + 270.0;
    let cw = [120.0, 170.0, 170.0, 200.0];
    let tx = (w - cw.iter().sum::<f64>()) / 2.0;
    let sum = |c: usize| cw[..c].iter().sum::<f64>();
    let heads = [unit.clone(), "Cash flow".to_string(), if present { "Discount factor".to_string() } else { "Compound factor".to_string() }, if present { "Present value".to_string() } else { format!("Value at {unit} {last}") }];
    d.rect(tx, ty - 20.0, cw.iter().sum(), rowh, "none", INK, 1.2);
    for (c, hd) in heads.iter().enumerate() {
        chart::text(&mut d, tx + sum(c) + cw[c] / 2.0, ty, fs, "middle", hd);
    }
    let mut total = 0.0;
    for (i, f) in flows.iter().enumerate() {
        let y = ty + (i as f64 + 1.0) * rowh;
        d.rect(tx, y - 20.0, cw.iter().sum(), rowh, "none", INK, 1.0);
        chart::text(&mut d, tx + cw[0] / 2.0, y, fs, "middle", &f.t.to_string());
        chart::text(&mut d, tx + sum(1) + cw[1] / 2.0, y, fs, "middle", &money(f.amount, &sym, cents));
        let fac = factor(rate, f.t, last, present);
        let val = f.amount * fac;
        total += val;
        if answers {
            chart::text(&mut d, tx + sum(2) + cw[2] / 2.0, y, fs, "middle", &trim(fac, 4));
            chart::text(&mut d, tx + sum(3) + cw[3] / 2.0, y, fs, "middle", &money(val, &sym, true));
        }
    }
    let y = ty + (flows.len() as f64 + 1.0) * rowh;
    d.rect(tx, y - 20.0, cw.iter().sum(), rowh, "none", INK, 1.6);
    chart::text(&mut d, tx + 8.0, y, fs * 1.05, "start", if present { "Net present value (NPV)" } else { "Total future value" });
    if answers {
        chart::text(&mut d, tx + sum(3) + cw[3] / 2.0, y, fs * 1.05, "middle", &money(total, &sym, true));
    }
    for c in 1..4 {
        d.line(tx + sum(c), ty - 20.0, tx + sum(c), y - 20.0, INK, 1.0);
    }
    d.line(tx + sum(3), y - 20.0, tx + sum(3), y + 10.0, INK, 1.0);
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn factors_discount_and_compound() {
        assert!((factor(0.1, 2, 4, true) - 1.0 / 1.21).abs() < 1e-12);
        assert_eq!(factor(0.1, 0, 4, true), 1.0);
        assert!((factor(0.1, 1, 3, false) - 1.21).abs() < 1e-12);
        assert_eq!(factor(0.1, 3, 3, false), 1.0);
        assert_eq!(factor(0.0, 7, 9, true), 1.0);
    }

    #[test]
    fn key_works_the_npv() {
        let svg = render(&defaults());
        // 300/1.08 + 400/1.08^2 + 500/1.08^3 + 300/1.08^4 - 1000
        let npv = 300.0 / 1.08 + 400.0 / 1.08f64.powi(2) + 500.0 / 1.08f64.powi(3) + 300.0 / 1.08f64.powi(4) - 1000.0;
        assert!(svg.contains(&format!(">{}<", money(npv, "$", true))), "{npv}");
        assert!(svg.contains(">0.9259<") && svg.contains(">Net present value (NPV)<") && svg.contains(">($1,000)<"));
    }

    #[test]
    fn future_value_compounds_to_the_last_period() {
        let mut p = defaults();
        p.set_s("mode", MODES[1]);
        p.set_s("flows", "0, 100\n2, 100");
        p.set_f("rate", 10.0);
        let svg = render(&p);
        assert!(svg.contains(">1.21<") && svg.contains(">$221.00<") && svg.contains("Total future value"));
    }

    #[test]
    fn student_copy_leaves_the_working_empty() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">Discount factor<"));
        assert!(student.contains(">$300<") && !student.contains(">0.9259<") && key.contains(">0.9259<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_flows_are_explained() {
        for (spec, needle) in [("", "Give 1 row"), ("1.5, 100", "whole numbers"), ("13, 100", "whole numbers"), ("-1, 100", "whole numbers"), ("a, b", "not a period"), ("1, 2, 3", "2 numbers")] {
            let mut p = defaults();
            p.set_s("flows", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let mut p = defaults();
        p.set_s("flows", &"1, 1\n".repeat(11));
        assert!(render(&p).contains("limited to 10"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("time-value").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("not a period") && !svg.contains("whole numbers"), "{}", ex.name);
        }
        for rate in [f64::NAN, f64::INFINITY, -5.0, 0.0, 100.0, 1e9] {
            for spec in ["12, 1e9\n0, -1e9", "0, 0", "12, 1", "nan, 1"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_f("rate", rate);
                    p.set_s("flows", spec);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(sane(&svg), "{rate} {spec:?}");
                }
            }
        }
    }
}
