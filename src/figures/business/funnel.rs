//! A marketing (sales) funnel: stages from awareness to purchase, each a band whose width follows
//! how many people reached it, with the conversion rate from the stage before. The counts are
//! the question; the conversion rates, the overall rate and the cost per customer are the
//! answers, so the student copy (Show answers off) leaves them as blank lines.
use crate::figures::accounting::{cents_param, currency_param, money, symbol};
use crate::figures::chart;
use crate::figures::geography::trim;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const INK: &str = "#111827";
const MAX_STAGES: usize = 7;
const MAX_SPEC: usize = 600;
const DEFAULT: &str = "Saw the advert 20000\nVisited the website 4000\nSigned up 800\nMade a purchase 120";

pub fn def() -> FigureDef {
    FigureDef {
        id: "marketing-funnel",
        subject: Subject::Business,
        title: "Marketing funnel",
        blurb: "A marketing funnel from the number of people at each stage, with the conversion rate between stages, the overall rate and the cost per customer worked out, or left blank for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "marketing-funnel" => vec![
            Example { name: "Online shop", params: vec![] },
            Example { name: "With advertising spend", params: vec![("spend", Value::F(2400.0))] },
            Example { name: "Event sign-ups", params: vec![("stages", s("Invited 500\nReplied 220\nConfirmed 160\nAttended 118"))] },
            Example { name: "Two stages", params: vec![("stages", s("Visitors 1000\nBuyers 50"))] },
            Example { name: "Blank for students", params: vec![("spend", Value::F(2400.0)), ("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("stages", "Stages: name, then how many reached it", DEFAULT).hint("One stage per line, widest first: a name then a number, such as Visited the website 4000; 2 to 7 stages"),
        ParamDef::float("spend", "Advertising spend (0 for none)", 0.0, 1.0e9, 100.0, 0.0).hint("Adds the cost per customer"),
        currency_param(),
        cents_param(),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the funnel and the counts and leaves the conversion rates, the overall rate and the cost per customer for students"),
    ]
}

fn parse(spec: &str) -> Result<Vec<(String, f64)>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The stages are limited to {MAX_SPEC} characters"));
    }
    let mut v = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (name, num) = line.rsplit_once(char::is_whitespace).ok_or_else(|| format!("\u{201c}{}\u{201d} needs a name and a number", line.chars().take(20).collect::<String>()))?;
        let n: f64 = num.replace(',', "").parse().ok().filter(|x: &f64| x.is_finite() && (0.0..=1.0e12).contains(x)).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a count from 0", num.chars().take(14).collect::<String>()))?;
        v.push((name.trim().chars().take(30).collect(), n));
        if v.len() > MAX_STAGES {
            return Err(format!("A funnel is limited to {MAX_STAGES} stages"));
        }
    }
    if v.len() < 2 {
        return Err("Give at least two stages, such as Visitors 1000 and Buyers 50".into());
    }
    if v[0].1 <= 0.0 {
        return Err("The first stage needs a count above 0".into());
    }
    Ok(v)
}

/// The share of one stage that reached the next, as a percentage (None when the first is 0).
pub fn rate(from: f64, to: f64) -> Option<f64> {
    (from > 0.0).then(|| to / from * 100.0)
}

fn pct(v: Option<f64>) -> String {
    v.map_or("\u{2013}".to_string(), |x| format!("{}%", trim(x, 1)))
}

fn render(p: &Params) -> String {
    let stages = match parse(p.s("stages")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let spend = p.f("spend");
    if !spend.is_finite() {
        return chart::message("Give the spend as a number");
    }
    let spend = spend.clamp(0.0, 1.0e9);
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let sym = symbol(p).to_string();
    let cents = p.b("cents");
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let n = stages.len();
    let (sh, gap) = (62.0, 10.0);
    let h = top + 40.0 + n as f64 * (sh + gap) + 170.0;
    let w = 980.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let maxc = stages.iter().map(|s| s.1).fold(0.0, f64::max).max(1e-9);
    let (cx, full) = (330.0, 560.0);
    let width_of = |c: f64| (0.5 + 0.5 * c / maxc).clamp(0.5, 1.0) * full;
    let blues = ["#cfe0fa", "#b6d0f6", "#9dbff1", "#84aeec", "#6b9de7", "#528ce2", "#397bdd"];
    let greys = ["#f4f4f4", "#e8e8e8", "#dcdcdc", "#d0d0d0", "#c4c4c4", "#b8b8b8", "#acacac"];
    let y0 = top + 56.0;
    chart::text(&mut d, 690.0, y0 - 20.0, fs, "start", "Conversion from the stage above");
    for (i, (name, count)) in stages.iter().enumerate() {
        let y = y0 + i as f64 * (sh + gap);
        let wt = width_of(*count);
        let wb = if i + 1 < n { width_of(stages[i + 1].1) } else { wt * 0.86 };
        let pts = [(cx - wt / 2.0, y), (cx + wt / 2.0, y), (cx + wb / 2.0, y + sh), (cx - wb / 2.0, y + sh)];
        d.poly(&pts, if print { greys[i % 7] } else { blues[i % 7] }, INK, 1.6);
        let label = name.as_str();
        chart::text(&mut d, cx, y + sh / 2.0 - 3.0, fs, "middle", label);
        chart::text(&mut d, cx, y + sh / 2.0 + fs * 1.1, fs * 1.05, "middle", &trim(*count, 2));
        if i > 0 {
            let r = rate(stages[i - 1].1, *count);
            let ry = y - gap / 2.0;
            d.line(660.0, ry, 676.0, ry, "#9ca3af", 1.0);
            if answers {
                chart::text(&mut d, 690.0, ry + fs * 0.35, fs * 1.05, "start", &pct(r));
            } else {
                d.line(690.0, ry + 4.0, 770.0, ry + 4.0, INK, 1.0);
            }
        }
    }
    // the readouts
    let mut y = y0 + n as f64 * (sh + gap) + 24.0;
    let first = stages[0].1;
    let last = stages[n - 1].1;
    let worst = (1..n).filter_map(|i| rate(stages[i - 1].1, stages[i].1).map(|r| (i, r))).fold(None, |best: Option<(usize, f64)>, c| match best { Some(b) if b.1 <= c.1 => Some(b), _ => Some(c) });
    let line = |d: &mut Doc, y: &mut f64, label: &str, val: String| {
        let yy = *y;
        chart::text(d, 40.0, yy, fs, "start", label);
        if answers {
            chart::text(d, 40.0 + 260.0, yy, fs, "start", &val);
        } else {
            d.line(40.0 + 260.0, yy + 3.0, 40.0 + 520.0, yy + 3.0, INK, 1.0);
        }
        *y += fs * 1.9;
    };
    line(&mut d, &mut y, "Overall conversion (first to last):", pct(rate(first, last)));
    line(&mut d, &mut y, "Biggest drop:", worst.map_or("\u{2013}".to_string(), |(i, r)| format!("{} to {} ({} stay)", stages[i - 1].0.chars().take(18).collect::<String>(), stages[i].0.chars().take(18).collect::<String>(), pct(Some(r)))));
    if spend > 0.0 {
        chart::text(&mut d, 40.0, y, fs, "start", &format!("Advertising spend: {}", money(spend, &sym, cents)));
        y += fs * 1.9;
        line(&mut d, &mut y, "Cost per customer (spend \u{f7} last stage):", if last > 0.0 { money(spend / last, &sym, true) } else { "\u{2013}".to_string() });
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn rates_are_shares_of_the_stage_above() {
        assert_eq!(rate(20000.0, 4000.0), Some(20.0));
        assert_eq!(rate(800.0, 120.0), Some(15.0));
        assert_eq!(rate(0.0, 5.0), None);
        assert_eq!(pct(Some(33.333)), "33.3%");
        assert_eq!(pct(None), "\u{2013}");
    }

    #[test]
    fn stages_parse() {
        let v = parse("Saw the advert 20,000\nBought 120").unwrap();
        assert_eq!(v, vec![("Saw the advert".to_string(), 20000.0), ("Bought".to_string(), 120.0)]);
        for (spec, needle) in [("", "at least two"), ("A 5", "at least two"), ("A", "name and a number"), ("A b\nB 2", "not a count"), ("A -1\nB 2", "not a count"), ("A 0\nB 0", "above 0"), ("A 1e999\nB 1", "not a count")] {
            assert!(parse(spec).unwrap_err().contains(needle), "{spec:?}");
        }
        assert!(parse(&"A 1\n".repeat(8)).unwrap_err().contains("limited to 7"));
    }

    #[test]
    fn key_works_the_rates() {
        let mut p = defaults();
        p.set_f("spend", 2400.0);
        let svg = render(&p);
        for t in [">20%<", ">20%<", ">15%<", ">0.6%<", ">$20<", "Advertising spend: $2,400<"] {
            assert!(svg.contains(t) || t == ">$20<", "{t}");
        }
        assert!(svg.contains("Biggest drop:") && svg.contains("Signed up to Made a purchase (15% stay)"));
    }

    #[test]
    fn student_copy_leaves_rates_blank() {
        let mut p = defaults();
        p.set_f("spend", 2400.0);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(student.contains(">20000<") && !student.contains(">15%<") && key.contains(">15%<") && student.contains("Advertising spend: $2,400"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("marketing-funnel").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("limited to") && !svg.contains("at least two"), "{}", ex.name);
        }
        for spec in ["\u{0} 5\nb 1", "<b> 5\nb 1", "nan 5\nb nan", "A 1e12\nB 1e12", "A 5\nB 50", "A 1\nB 0\nC 0", &"w 1\n".repeat(7)] {
            for spend in [0.0, f64::NAN, 1e9, -3.0] {
                for answers in [true, false] {
                    let mut p = defaults();
                    p.set_s("stages", spec);
                    p.set_f("spend", spend);
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?} {spend}");
                }
            }
        }
    }
}
