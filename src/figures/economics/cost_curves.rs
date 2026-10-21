//! Cost curves of a firm: marginal cost, average total cost and average variable cost from a
//! cubic variable-cost function and a fixed cost, with the price line, the profit-maximising
//! quantity (where marginal cost meets the price) and the profit or loss rectangle. The curves
//! and the price are the question; the quantity, the rectangle, the minimum points and the readout
//! are the answers, so the student copy (Show answers off) leaves them out.
use super::{draw_axes, n2, nice_max, plane};
use crate::figures::chart;
use crate::figures::svg::{self, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn def() -> FigureDef {
    FigureDef {
        id: "cost-curves",
        subject: Subject::Economics,
        title: "Cost curves and profit",
        blurb: "MC, ATC and AVC curves from a fixed cost and a cubic variable cost, with the price line, the profit-maximising quantity and the profit or loss rectangle, or only the curves for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    Some(match id {
        "cost-curves" => vec![
            Example { name: "A profit", params: vec![] },
            Example { name: "A loss: price below ATC", params: vec![("price", Value::F(11.0))] },
            Example { name: "Shut down: price below AVC", params: vec![("price", Value::F(3.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("fc", "Fixed cost", 0.0, 1.0e6, 10.0, 100.0),
        ParamDef::float("a", "Variable cost: q term", 0.0, 1.0e3, 0.1, 10.0).hint("Variable cost = a q \u{2212} b q\u{b2} + c q\u{b3}"),
        ParamDef::float("b", "Variable cost: q\u{b2} term (subtracted)", 0.0, 1.0e3, 0.05, 0.9),
        ParamDef::float("c", "Variable cost: q\u{b3} term", 0.0001, 1.0e3, 0.01, 0.05),
        ParamDef::float("price", "Price", 0.01, 1.0e6, 1.0, 18.0),
        ParamDef::float("qmax", "Quantity axis maximum", 2.0, 1000.0, 1.0, 20.0),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off draws the three curves and the price line only: no quantity, rectangle, markers or readout"),
    ]
}

/// The cost functions of a firm with VC = a q - b q^2 + c q^3.
#[derive(Clone, Copy)]
pub struct Costs {
    pub fc: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

impl Costs {
    pub fn vc(&self, q: f64) -> f64 { self.a * q - self.b * q * q + self.c * q * q * q }
    pub fn mc(&self, q: f64) -> f64 { self.a - 2.0 * self.b * q + 3.0 * self.c * q * q }
    pub fn avc(&self, q: f64) -> f64 { self.a - self.b * q + self.c * q * q }
    pub fn atc(&self, q: f64) -> f64 { self.avc(q) + self.fc / q }
    /// The profit-maximising quantity at `price`: where marginal cost, on its rising part, meets
    /// the price; 0 when the price is below the lowest marginal cost or below average variable cost.
    pub fn best_q(&self, price: f64) -> f64 {
        let disc = 4.0 * self.b * self.b - 12.0 * self.c * (self.a - price);
        if disc < 0.0 {
            return 0.0;
        }
        let q = (2.0 * self.b + disc.sqrt()) / (6.0 * self.c);
        if q > 0.0 && price >= self.avc(q) - 1e-9 { q } else { 0.0 }
    }
    /// The quantity and value at the lowest point of a curve (found on a fine grid).
    pub fn lowest(&self, f: impl Fn(f64) -> f64, qmax: f64) -> (f64, f64) {
        let mut best = (qmax, f(qmax));
        for i in 1..=2000 {
            let q = qmax * i as f64 / 2000.0;
            let v = f(q);
            if v < best.1 {
                best = (q, v);
            }
        }
        best
    }
}

/// Runs of consecutive curve points that lie inside the plot (a curve that leaves the top comes back as separate runs).
fn runs(f: &chart::Frame, curve: impl Fn(f64) -> f64, q0: f64, q1: f64) -> Vec<Vec<(f64, f64)>> {
    let mut out: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    for i in 0..=240 {
        let q = q0 + (q1 - q0) * i as f64 / 240.0;
        let v = curve(q);
        if v.is_finite() && (0.0..=f.y1).contains(&v) {
            cur.push((f.px(q), f.py(v)));
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn render(p: &Params) -> String {
    let c = Costs { fc: p.f("fc"), a: p.f("a"), b: p.f("b"), c: p.f("c") };
    let (price, qmax) = (p.f("price"), p.f("qmax"));
    let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
    if !(ok(c.fc, 0.0, 1.0e6) && ok(c.a, 0.0, 1.0e3) && ok(c.b, 0.0, 1.0e3) && ok(c.c, 0.0001, 1.0e3) && ok(price, 0.01, 1.0e6) && ok(qmax, 2.0, 1000.0)) {
        return chart::message("Enter the costs, the price and the axis maximum as numbers");
    }
    if 3.0 * c.a * c.c <= c.b * c.b {
        return chart::message("Marginal cost would go below zero: make the q term larger or the q\u{b2} term smaller");
    }
    let q = c.best_q(price);
    if q > qmax {
        return chart::message("The profit-maximising quantity is beyond the graph: raise the quantity maximum");
    }
    let (qa, atc_min) = c.lowest(|x| c.atc(x), qmax);
    let (qv, avc_min) = c.lowest(|x| c.avc(x), qmax);
    let ymax = nice_max((price * 1.5).max(atc_min * 2.0).max(c.mc(qmax) * 0.5));
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let fills = Fills::new(p);
    let answers = p.b(SHOW_ANSWERS);
    let title = p.s("title").trim();
    let (mut d, f) = plane(qmax, ymax, 14.0 + 2.0 * fs * 1.75);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, f.w / 2.0, 16.0, fs * 1.2, "middle", title);
    }
    let profit = if q > 0.0 { price * q - c.fc - c.vc(q) } else { -c.fc };
    if answers && q > 0.0 {
        let (top, bot) = (price.max(c.atc(q)), price.min(c.atc(q)));
        let fill = if profit >= 0.0 { fills.series(2) } else { fills.series(3) };
        d.rect(f.px(0.0), f.py(top), f.px(q) - f.px(0.0), f.py(bot) - f.py(top), &fill, "#111827", 1.2);
    }
    draw_axes(&mut d, &f, p.b("numbers"), fs, "Quantity", "Cost and price");
    let eps = qmax / 240.0;
    let tag = |d: &mut crate::figures::svg::Doc, run: &[Vec<(f64, f64)>], name: &str, i: usize, below: bool| {
        for r in run {
            d.polyline(r, fills.line(i), 3.0);
        }
        if let Some(&(x, y)) = run.last().and_then(|r| r.last()) {
            chart::text(d, x + 6.0, if below { y + fs } else { y + 4.0 }, fs * 1.05, "start", name);
        }
    };
    tag(&mut d, &runs(&f, |x| c.mc(x), 0.0, qmax), "MC", 0, false);
    tag(&mut d, &runs(&f, |x| c.atc(x), eps, qmax), "ATC", 1, false);
    tag(&mut d, &runs(&f, |x| c.avc(x), 0.0, qmax), "AVC", 2, true);
    if price <= ymax {
        let y = f.py(price);
        d.raw(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="#111827" stroke-width="2.2" stroke-dasharray="9 5"/>"##, f.left(), f.right()));
        chart::text(&mut d, f.left() + 8.0, y - 7.0, fs, "start", "P = MR");
    }
    if answers {
        if q > 0.0 {
            d.raw(&format!(r##"<path d="M{:.2},{:.2} L{:.2},{:.2}" stroke="#6b7280" stroke-width="1.3" stroke-dasharray="5 4"/>"##, f.px(q), f.py(price), f.px(q), f.bottom()));
            d.circle(f.px(q), f.py(price), 4.8, "#111827", "#fff", 1.4);
            chart::text(&mut d, f.px(q), f.bottom() + 8.0 + fs * 2.1, fs, "middle", &format!("Q* = {}", n2(q)));
        }
        for (qq, v) in [(qa, atc_min), (qv, avc_min)] {
            if v <= ymax {
                d.circle(f.px(qq), f.py(v), 3.6, "#fff", "#111827", 1.6);
            }
        }
        let l1 = if q > 0.0 {
            format!("Marginal cost equals the price at Q* = {}; ATC there is {}", n2(q), n2(c.atc(q)))
        } else {
            format!("Price {} is below the lowest AVC ({}): shut down, produce 0", n2(price), n2(avc_min))
        };
        let l2 = if q > 0.0 {
            format!("{} = ({} \u{2212} {}) \u{d7} {} = {}", if profit >= 0.0 { "Profit" } else { "Loss" }, n2(price), n2(c.atc(q)), n2(q), n2(profit.abs()))
        } else {
            format!("Loss = fixed cost = {}", n2(c.fc))
        };
        chart::text(&mut d, f.left(), f.h + 20.0, fs, "start", &l1);
        chart::text(&mut d, f.left(), f.h + 20.0 + fs * 1.75, fs, "start", &l2);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn marginal_cost_is_the_slope_of_variable_cost() {
        let c = Costs { fc: 100.0, a: 10.0, b: 0.9, c: 0.05 };
        for q in [1.0, 5.0, 12.0] {
            let h = 1e-5;
            let slope = (c.vc(q + h) - c.vc(q - h)) / (2.0 * h);
            assert!((slope - c.mc(q)).abs() < 1e-5, "{q}");
        }
        assert!((c.atc(10.0) - (c.avc(10.0) + 10.0)).abs() < 1e-12);
    }

    #[test]
    fn the_best_quantity_sits_where_mc_meets_the_price() {
        let c = Costs { fc: 100.0, a: 10.0, b: 0.9, c: 0.05 };
        let q = c.best_q(18.0);
        assert!((c.mc(q) - 18.0).abs() < 1e-9 && q > 6.0, "the rising part of MC");
        assert!((q - 15.45).abs() < 0.01);
        assert_eq!(c.best_q(3.0), 0.0, "below the lowest AVC");
        assert_eq!(c.best_q(1.0), 0.0);
        // at the shut-down price (the minimum of AVC) the firm is just willing to produce
        let (_, v) = c.lowest(|x| c.avc(x), 20.0);
        assert!(c.best_q(v + 0.01) > 0.0 && c.best_q(v - 0.2) == 0.0);
    }

    #[test]
    fn key_works_out_the_profit() {
        let svg = render(&defaults());
        assert!(svg.contains(">Q* = 15.45<") && svg.contains("Profit = (18 \u{2212} 14.5") && svg.contains("P = MR"));
    }

    #[test]
    fn losses_and_shutdown_are_named() {
        let mut p = defaults();
        p.set_f("price", 11.0);
        let svg = render(&p);
        assert!(svg.contains("Loss = (11 \u{2212}"), "price below ATC at the best quantity");
        p.set_f("price", 3.0);
        assert!(render(&p).contains("shut down, produce 0") && render(&p).contains("Loss = fixed cost = 100"));
    }

    #[test]
    fn student_copy_has_curves_only() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">MC<") && student.contains(">ATC<") && student.contains(">AVC<") && student.contains("P = MR"));
        assert!(!student.contains("Q*") && !student.contains("Profit") && key.contains("Q*"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_costs_are_explained() {
        let mut p = defaults();
        p.set_f("b", 5.0);
        assert!(render(&p).contains("below zero"));
        let mut p = defaults();
        p.set_f("price", 200.0);
        assert!(render(&p).contains("beyond the graph"));
        let mut p = defaults();
        p.set_f("c", f64::NAN);
        assert!(render(&p).contains("as numbers"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
        p.set_f("price", 11.0);
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("cost-curves").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for v in [0.0, -5.0, 1.0e12, f64::NAN, f64::INFINITY, 1.0e-9] {
            for key in ["fc", "a", "b", "c", "price", "qmax"] {
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
