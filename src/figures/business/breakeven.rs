//! A break-even chart: the fixed cost, the total cost and the revenue against the number of units
//! sold, with the break-even point where revenue meets total cost, the loss and profit regions, a
//! table of values and the figures worked out. The costs and the price are the question; the
//! lines, the point, the table values and the readouts are the answers, so the student copy
//! (Show answers off) draws only the empty axes and leaves the cells and readouts blank.
use crate::figures::accounting::{currency_param, cents_param, money, symbol};
use crate::figures::chart::{self, AxesOpts, Frame};
use crate::figures::geography::trim;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";

pub fn def() -> FigureDef {
    FigureDef {
        id: "break-even",
        subject: Subject::Business,
        title: "Break-even chart",
        blurb: "A break-even chart from fixed costs, variable cost per unit and price, with the break-even point, profit and loss regions and a table worked out, or empty axes and a blank table for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let f = Value::F;
    Some(match id {
        "break-even" => vec![
            Example { name: "A small bakery", params: vec![] },
            Example { name: "With expected sales", params: vec![("expected", f(600.0))] },
            Example { name: "High margin product", params: vec![("fixed", f(20000.0)), ("price", f(120.0)), ("variable", f(30.0))] },
            Example { name: "Thin margin", params: vec![("fixed", f(5000.0)), ("price", f(10.0)), ("variable", f(8.5)), ("expected", f(4000.0))] },
            Example { name: "Blank for students", params: vec![("expected", f(600.0)), ("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("fixed", "Fixed costs", 0.0, 1.0e9, 100.0, 6000.0),
        ParamDef::float("price", "Selling price per unit", 0.01, 1.0e6, 0.5, 20.0),
        ParamDef::float("variable", "Variable cost per unit", 0.0, 1.0e6, 0.5, 8.0),
        ParamDef::float("expected", "Expected sales (units, 0 for none)", 0.0, 1.0e9, 10.0, 0.0).hint("Adds the margin of safety and the profit at this level of sales"),
        currency_param(),
        cents_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the axes and the given figures and leaves the lines, the break-even point, the table values and the readouts for students"),
    ]
}

/// The figures behind the chart.
pub struct Model {
    pub fixed: f64,
    pub price: f64,
    pub variable: f64,
}

impl Model {
    pub fn contribution(&self) -> f64 {
        self.price - self.variable
    }
    /// Units at which revenue meets total cost (None when each unit loses money).
    pub fn break_even(&self) -> Option<f64> {
        (self.contribution() > 1e-12).then(|| self.fixed / self.contribution())
    }
    pub fn revenue(&self, q: f64) -> f64 {
        self.price * q
    }
    pub fn cost(&self, q: f64) -> f64 {
        self.fixed + self.variable * q
    }
    pub fn profit(&self, q: f64) -> f64 {
        self.revenue(q) - self.cost(q)
    }
}

fn render(p: &Params) -> String {
    let (fixed, price, variable, expected) = (p.f("fixed"), p.f("price"), p.f("variable"), p.f("expected"));
    if ![fixed, price, variable, expected].iter().all(|v| v.is_finite()) {
        return chart::message("Give numbers for the costs and the price");
    }
    let m = Model { fixed: fixed.clamp(0.0, 1.0e9), price: price.clamp(0.0, 1.0e6), variable: variable.clamp(0.0, 1.0e6) };
    let expected = expected.clamp(0.0, 1.0e9);
    let Some(beq) = m.break_even() else {
        return chart::message("The price must be above the variable cost per unit, or the business can never break even");
    };
    let answers = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let print = fills.is_print();
    let sym = symbol(p).to_string();
    let cents = p.b("cents");
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (1060.0, top + 520.0);
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    // the axes cover the break-even point with room to spare
    let xwant = (beq * 1.7).max(expected * 1.25).max(1.0);
    let (_, xmax, xstep) = chart::axis_range(0.0, xwant, 6.0);
    let ywant = m.revenue(xmax).max(m.cost(xmax));
    let (_, ymax, ystep) = chart::axis_range(0.0, ywant, 6.0);
    let f = Frame { w: 620.0, h, l: 96.0, r: 24.0, t: top + 24.0, b: 70.0, x0: 0.0, x1: xmax, y0: 0.0, y1: ymax };
    let xt = chart::ticks(0.0, xmax, xstep);
    let yt: Vec<(f64, String)> = chart::ticks(0.0, ymax, ystep).into_iter().map(|(v, _)| (v, money(v, &sym, false))).collect();
    chart::axes(&mut d, &f, &AxesOpts { xticks: &xt, yticks: &yt, grid: true, x_title: "Units sold", y_title: "Money", fs });
    let pt = |q: f64, v: f64| (f.px(q), f.py(v));
    if answers {
        let (loss_fill, profit_fill) = if print { ("#e6e6e6", "#cccccc") } else { ("#f9d5d0", "#d3ecd5") };
        d.poly(&[pt(0.0, m.cost(0.0)), pt(beq, m.cost(beq)), pt(0.0, 0.0)], loss_fill, "none", 0.0);
        d.poly(&[pt(beq, m.cost(beq)), pt(xmax, m.cost(xmax)), pt(xmax, m.revenue(xmax))], profit_fill, "none", 0.0);
        // the lines
        let dash = |d: &mut Doc, a: (f64, f64), b: (f64, f64), col: &str, da: &str| {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{col}" stroke-width="3" stroke-dasharray="{da}" stroke-linecap="round"/>"##, a.0, a.1, b.0, b.1));
        };
        dash(&mut d, pt(0.0, m.fixed), pt(xmax, m.fixed), fills.line(2), if print { "2 4" } else { "8 5" });
        dash(&mut d, pt(0.0, m.cost(0.0)), pt(xmax, m.cost(xmax)), fills.line(1), if print { "8 3 2 3" } else { "" });
        dash(&mut d, pt(0.0, 0.0), pt(xmax, m.revenue(xmax)), fills.line(0), "");
        // labels at the right-hand end
        let lx = f.right() - 8.0;
        chart::text(&mut d, lx, f.py(m.revenue(xmax)) + 18.0, fs, "end", "Revenue");
        chart::text(&mut d, lx, f.py(m.cost(xmax)) - 8.0, fs, "end", "Total cost");
        chart::text(&mut d, lx, f.py(m.fixed) - 8.0, fs, "end", "Fixed cost");
        // the break-even point
        let (bx, by) = pt(beq, m.revenue(beq));
        d.raw(&format!(r##"<line x1="{bx:.2}" y1="{by:.2}" x2="{bx:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.4" stroke-dasharray="4 4"/>"##, f.bottom()));
        d.circle(bx, by, 6.0, "#fff", INK, 2.4);
        chart::text(&mut d, bx - 14.0, by - 14.0, fs, "end", "Break-even point");
        let lq = beq * 0.3;
        chart::text(&mut d, f.px(lq), f.py((m.revenue(lq) + m.fixed.min(m.cost(lq))) / 2.0) + fs * 0.35, fs * 1.05, "middle", "Loss");
        chart::text(&mut d, f.px((beq + xmax) / 2.0 + (xmax - beq) * 0.1), f.py(m.cost((beq + xmax) / 2.0)) - 36.0 + 0.0, fs * 1.05, "middle", "Profit");
        if expected > 0.0 && expected <= xmax {
            let (ex, ey) = pt(expected, m.revenue(expected));
            d.raw(&format!(r##"<line x1="{ex:.2}" y1="{:.2}" x2="{ex:.2}" y2="{ey:.2}" stroke="{INK}" stroke-width="1.2" stroke-dasharray="2 4"/>"##, f.bottom()));
            chart::text(&mut d, ex, f.bottom() - 8.0, fs * 0.85, "middle", "expected");
        }
    }
    // the table of values
    let (tx, ty) = (660.0, top + 34.0);
    let cw = [72.0, 92.0, 92.0, 92.0];
    let rowh = 30.0;
    let step = chart::nice_step(xmax / 1.0, 5.0).min(xmax);
    let units: Vec<f64> = (0..=5).map(|i| i as f64 * step).collect();
    d.rect(tx, ty - 20.0, cw.iter().sum(), rowh, "none", INK, 1.2);
    let sum = |c: usize| cw[..c].iter().sum::<f64>();
    for (c, hd) in ["Units", "Revenue", "Total cost", "Profit"].iter().enumerate() {
        chart::text(&mut d, tx + sum(c) + cw[c] / 2.0, ty, fs * 0.95, "middle", hd);
    }
    for (i, q) in units.iter().enumerate() {
        let y = ty + (i as f64 + 1.0) * rowh;
        d.rect(tx, y - 20.0, cw.iter().sum(), rowh, "none", INK, 1.0);
        chart::text(&mut d, tx + cw[0] / 2.0, y, fs * 0.95, "middle", &trim(*q, 2));
        if answers {
            for (c, v) in [(1, m.revenue(*q)), (2, m.cost(*q)), (3, m.profit(*q))] {
                chart::text(&mut d, tx + sum(c) + cw[c] / 2.0, y, fs * 0.9, "middle", &money(v, &sym, cents));
            }
        }
    }
    for c in 1..4 {
        d.line(tx + sum(c), ty - 20.0, tx + sum(c), ty + 6.0 * rowh + 10.0, INK, 1.0);
    }
    // what was given, and what is asked
    let mut y = ty + 6.0 * rowh + 40.0;
    let given = [("Fixed costs", money(m.fixed, &sym, cents)), ("Price per unit", money(m.price, &sym, true)), ("Variable cost per unit", money(m.variable, &sym, true))];
    for (k, v) in &given {
        chart::text(&mut d, tx, y, fs, "start", &format!("{k}: {v}"));
        y += fs * 1.8;
    }
    if expected > 0.0 {
        chart::text(&mut d, tx, y, fs, "start", &format!("Expected sales: {} units", trim(expected, 2)));
        y += fs * 1.8;
    }
    y += 10.0;
    let mut line = |d: &mut Doc, label: &str, val: String| {
        chart::text(d, tx, y, fs, "start", label);
        let vx = tx + 220.0;
        if answers {
            chart::text(d, vx, y, fs, "start", &val);
        } else {
            d.line(vx, y + 3.0, vx + 130.0, y + 3.0, INK, 1.0);
        }
        y += fs * 1.8;
    };
    line(&mut d, "Contribution per unit:", money(m.contribution(), &sym, true));
    let whole = beq.ceil();
    line(&mut d, "Break-even units:", if (beq - whole).abs() < 1e-9 { trim(beq, 2) } else { format!("{} (so {})", trim(beq, 2), trim(whole, 0)) });
    line(&mut d, "Break-even revenue:", money(m.revenue(beq), &sym, cents));
    if expected > 0.0 {
        line(&mut d, "Margin of safety (units):", format!("{} ({}%)", trim(expected - beq, 2), trim((expected - beq) / expected * 100.0, 1)));
        line(&mut d, "Profit at expected sales:", money(m.profit(expected), &sym, cents));
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn the_model_works_the_point() {
        let m = Model { fixed: 6000.0, price: 20.0, variable: 8.0 };
        assert_eq!(m.contribution(), 12.0);
        assert_eq!(m.break_even(), Some(500.0));
        assert_eq!(m.revenue(500.0), m.cost(500.0));
        assert_eq!(m.profit(600.0), 1200.0);
        assert_eq!(m.profit(0.0), -6000.0);
        assert_eq!(Model { fixed: 1.0, price: 5.0, variable: 5.0 }.break_even(), None);
        assert_eq!(Model { fixed: 1.0, price: 4.0, variable: 5.0 }.break_even(), None);
        assert_eq!(Model { fixed: 0.0, price: 5.0, variable: 1.0 }.break_even(), Some(0.0));
    }

    #[test]
    fn key_writes_the_figures() {
        let mut p = defaults();
        p.set_f("expected", 600.0);
        let svg = render(&p);
        for t in [">Break-even point<", ">Loss<", ">Profit<", ">$12.00<", ">500<", ">$10,000<", ">100 (16.7%)<", ">$1,200<"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn a_fractional_break_even_rounds_up_for_whole_units() {
        let mut p = defaults();
        p.set_f("fixed", 1000.0);
        p.set_f("price", 7.0);
        p.set_f("variable", 3.0);
        p.set_f("expected", 0.0);
        let svg = render(&p);
        assert!(svg.contains(">250<"));
        p.set_f("fixed", 1001.0);
        assert!(render(&p).contains(">250.25 (so 251)<"));
    }

    #[test]
    fn student_copy_has_empty_axes_and_cells() {
        let mut p = defaults();
        p.set_f("expected", 600.0);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(student.contains(">Fixed costs: $6,000<") && !student.contains("Break-even point") && !student.contains(">$12.00<") && !student.contains(">Loss<"));
        assert!(key.matches("<polygon").count() >= student.matches("<polygon").count() + 2);
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn an_impossible_business_is_explained() {
        let mut p = defaults();
        p.set_f("variable", 20.0);
        assert!(render(&p).contains("price must be above"));
        p.set_f("variable", 25.0);
        assert!(render(&p).contains("price must be above"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        p.set_f("expected", 700.0);
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("break-even").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("must be above"), "{}", ex.name);
        }
        for v in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e300, 1e-300] {
            for key in ["fixed", "price", "variable", "expected"] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_f(key, v);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(sane(&svg), "{key} {v}");
                }
            }
        }
        let mut p = defaults();
        p.set_s("title", "<script>");
        assert!(!render(&p).contains("<script>"));
    }
}
