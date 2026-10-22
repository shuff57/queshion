//! Depreciation schedule: the expense, the accumulated depreciation and the book value for each
//! year of an asset's life by straight-line, double-declining-balance or sum-of-the-years'-digits.
//! The cost, salvage value, life and method are what a student is given; every figure in the
//! table is the answer, so the student copy (Show answers off) leaves the cells empty.
use super::{cents_param, currency_param, digits, money, symbol};
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const METHODS: [&str; 3] = ["Straight-line", "Double-declining balance", "Sum of the years' digits"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "depreciation-schedule",
        subject: Subject::Accounting,
        title: "Depreciation schedule",
        blurb: "A depreciation schedule by straight-line, double-declining-balance or sum-of-the-years'-digits: the expense, accumulated depreciation and book value each year, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(_fig: &str, _key: &str, _p: &Params) -> bool {
    true
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "depreciation-schedule" => vec![
            Example { name: "Straight-line, five years", params: vec![] },
            Example { name: "Double-declining balance", params: vec![("method", s(METHODS[1]))] },
            Example { name: "Sum of the years' digits", params: vec![("method", s(METHODS[2])), ("life", Value::I(4)), ("cost", Value::F(24000.0)), ("salvage", Value::F(2000.0))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("asset", "Asset", "Delivery van"),
        ParamDef::float("cost", "Cost", 0.01, 1.0e12, 100.0, 18000.0),
        ParamDef::float("salvage", "Salvage value", 0.0, 1.0e12, 100.0, 3000.0),
        ParamDef::int("life", "Useful life (years)", 1, 15, 5),
        ParamDef::choice("method", "Method", &METHODS, METHODS[0]),
        currency_param(),
        cents_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the facts about the asset and leaves every figure in the table empty for students"),
    ]
}

fn cents(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// The yearly expenses for an asset: the rounded figure each year, with the last year bringing
/// the book value exactly to salvage.
pub fn schedule(method: &str, cost: f64, salvage: f64, life: usize) -> Vec<f64> {
    let base = cost - salvage;
    let mut out = Vec::with_capacity(life);
    let mut book = cost;
    let syd = (life * (life + 1) / 2) as f64;
    for y in 1..=life {
        let raw = match method {
            "Double-declining balance" => (book * 2.0 / life as f64).min(book - salvage),
            "Sum of the years' digits" => base * (life - y + 1) as f64 / syd,
            _ => base / life as f64,
        };
        let e = if y == life { cents(book - salvage) } else { cents(raw.max(0.0)).min(cents(book - salvage)) };
        out.push(e);
        book = cents(book - e);
    }
    out
}

fn render(p: &Params) -> String {
    let (cost, salvage) = (p.f("cost"), p.f("salvage"));
    let life = p.i("life").clamp(1, 15) as usize;
    if !(cost.is_finite() && cost > 0.0 && cost <= 1.0e12) {
        return chart::message("The cost must be above 0");
    }
    if !(salvage.is_finite() && salvage >= 0.0 && salvage < cost) {
        return chart::message("The salvage value must be at least 0 and less than the cost");
    }
    let method = METHODS.iter().copied().find(|m| *m == p.s("method")).unwrap_or(METHODS[0]);
    let (sym, show_cents) = (symbol(p), p.b("cents"));
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let title = p.s("title").trim();
    let asset: String = p.s("asset").trim().chars().take(30).collect();
    let exp = schedule(method, cost, salvage, life);
    let cols = [70.0, 150.0, 170.0, 130.0];
    let (x0, tw) = (20.0, cols.iter().sum::<f64>());
    let w = tw + 40.0;
    let row = fs * 1.9;
    let top = if title.is_empty() { 14.0 } else { 38.0 + fs * 1.3 };
    let facts_h = fs * 1.7 * 2.0 + 8.0;
    let h = top + facts_h + row * (life as f64 + 3.0) + 14.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let name = if asset.is_empty() { "Asset".to_string() } else { asset };
    chart::text(&mut d, x0, top + fs * 1.1, fs, "start", &format!("{name}: cost {}, salvage value {}, {life} year{} useful life", money(cost, sym, show_cents), money(salvage, sym, show_cents), if life == 1 { "" } else { "s" }));
    chart::text(&mut d, x0, top + fs * 2.8, fs, "start", &format!("Method: {method}"));
    let y0 = top + facts_h;
    let xs: Vec<f64> = cols.iter().scan(x0, |a, c| { let v = *a; *a += c; Some(v) }).collect();
    d.rect(x0, y0, tw, row, if print { "#e5e5e5" } else { "#f4eee2" }, INK, 1.6);
    for (i, t) in ["Year", "Expense", "Accumulated depreciation", "Book value"].iter().enumerate() {
        let (x, a) = if i == 0 { (xs[0] + 10.0, "start") } else { (xs[i] + cols[i] - 10.0, "end") };
        chart::text(&mut d, x, y0 + row * 0.68, fs * if i == 2 { 0.88 } else { 1.0 }, a, t);
    }
    // the year-0 line shows the starting book value, the same in both copies
    let r0 = y0 + row;
    chart::text(&mut d, xs[0] + 10.0, r0 + row * 0.68, fs, "start", "0");
    chart::text(&mut d, xs[3] + cols[3] - 10.0, r0 + row * 0.68, fs, "end", &digits(cost, show_cents));
    let (mut acc, mut book) = (0.0, cost);
    for (i, e) in exp.iter().enumerate() {
        let y = y0 + row * (i as f64 + 2.0);
        acc = cents(acc + e);
        book = cents(book - e);
        chart::text(&mut d, xs[0] + 10.0, y + row * 0.68, fs, "start", &(i + 1).to_string());
        if answers {
            for (c, v) in [(1, *e), (2, acc), (3, book)] {
                chart::text(&mut d, xs[c] + cols[c] - 10.0, y + row * 0.68, fs, "end", &digits(v, show_cents));
            }
        }
        d.line(x0, y, x0 + tw, y, "#c9ced6", 0.8);
    }
    let bottom = y0 + row * (life as f64 + 2.0);
    // a totals row: the expense column adds up to cost less salvage
    chart::text(&mut d, xs[0] + 10.0, bottom + row * 0.7, fs, "start", "Total");
    if answers {
        chart::text(&mut d, xs[1] + cols[1] - 10.0, bottom + row * 0.7, fs, "end", &money(exp.iter().sum::<f64>(), sym, show_cents));
    } else {
        d.rect(xs[1] + 14.0, bottom + 4.0, cols[1] - 24.0, row - 8.0, "#fff", INK, 1.0);
    }
    for x in xs.iter().skip(1) {
        d.line(*x, y0, *x, bottom + row, INK, 1.2);
    }
    d.line(x0, bottom, x0 + tw, bottom, INK, 1.6);
    d.rect(x0, y0, tw, bottom + row - y0, "none", INK, 1.8);
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn straight_line_is_even_and_ends_at_salvage() {
        let e = schedule(METHODS[0], 18000.0, 3000.0, 5);
        assert_eq!(e, vec![3000.0; 5]);
        // an amount that does not divide evenly: the last year takes the rounding
        let e = schedule(METHODS[0], 10000.0, 0.0, 3);
        assert_eq!(e, vec![3333.33, 3333.33, 3333.34]);
        assert!((e.iter().sum::<f64>() - 10000.0).abs() < 1e-9);
    }

    #[test]
    fn declining_balance_stops_at_salvage() {
        let e = schedule(METHODS[1], 18000.0, 3000.0, 5);
        // 40% of 18000, 10800, 6480 ... then limited by salvage
        assert_eq!(e[0], 7200.0);
        assert_eq!(e[1], 4320.0);
        assert_eq!(e[2], 2592.0);
        assert!((e.iter().sum::<f64>() - 15000.0).abs() < 1e-6, "total is cost less salvage");
        assert!(e.iter().all(|v| *v >= 0.0));
    }

    #[test]
    fn sum_of_years_digits_falls_each_year() {
        let e = schedule(METHODS[2], 24000.0, 2000.0, 4);
        // base 22000, digits 10: 4/10, 3/10, 2/10, 1/10
        assert_eq!(e, vec![8800.0, 6600.0, 4400.0, 2200.0]);
    }

    #[test]
    fn every_method_reaches_salvage_exactly() {
        for m in METHODS {
            for life in 1..=15 {
                for (cost, salv) in [(18000.0, 3000.0), (9999.99, 0.0), (100.0, 99.99), (123456.78, 1000.0)] {
                    let e = schedule(m, cost, salv, life);
                    assert_eq!(e.len(), life);
                    assert!((e.iter().sum::<f64>() - (cost - salv)).abs() < 0.0051 * life as f64, "{m} {life} {cost} {salv}");
                    assert!(e.iter().all(|v| v.is_finite() && *v >= -1e-9));
                }
            }
        }
    }

    #[test]
    fn key_writes_the_table() {
        let svg = render(&defaults());
        for t in [">3,000<", ">15,000<", ">12,000<", ">$15,000<", "Method: Straight-line", "Delivery van: cost $18,000, salvage value $3,000, 5 years useful life"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn student_copy_leaves_the_cells_empty() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(student.contains(">18,000<") && student.contains("Method: Straight-line"));
        assert!(!student.contains(">15,000<") && !student.contains(">12,000<") && key.contains(">12,000<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_values_are_explained() {
        let mut p = defaults();
        p.set_f("salvage", 20000.0);
        assert!(render(&p).contains("salvage value must be"));
        p.set_f("salvage", 0.0);
        p.set_f("cost", 0.0);
        assert!(render(&p).contains("cost must be above 0"));
        p.set_f("cost", f64::NAN);
        assert!(render(&p).contains("cost must be above 0"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("depreciation-schedule").unwrap() {
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
            assert!(svg.contains("<rect") && !svg.contains("NaN"), "{}", ex.name);
        }
        for cost in [0.0, -1.0, 0.01, 1.0e12, f64::NAN, f64::INFINITY] {
            for salvage in [0.0, 0.005, 1.0e11, f64::NAN] {
                for life in [-3, 0, 1, 15, 99] {
                    for answers in [true, false] {
                        let mut p = defaults();
                        p.set_f("cost", cost);
                        p.set_f("salvage", salvage);
                        p.set_i("life", life);
                        p.set_s("method", METHODS[(life.unsigned_abs() as usize) % 3]);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN") && !svg.contains("inf"), "{cost} {salvage} {life}");
                    }
                }
            }
        }
    }
}
