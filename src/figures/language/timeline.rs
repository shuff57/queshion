//! A timeline of dated events drawn to scale along a line, with a readout of the time spanned and
//! the longest gap between events. The events are the question; the student copy (Show answers
//! off) hides the dates or the event names (your choice) and the readouts, so they can be filled in.
use crate::figures::chart;
use crate::figures::geography::trim;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use super::wrap;

const HIDE: [&str; 2] = ["Dates", "Event names"];
const INK: &str = "#111827";
const MAX_EVENTS: usize = 12;
const DEFAULT: &str = "1492 Columbus reaches the Americas\n1620 Mayflower lands at Plymouth\n1776 Declaration of Independence\n1803 Louisiana Purchase\n1861 Civil War begins\n1865 Civil War ends";

pub fn def() -> FigureDef {
    FigureDef {
        id: "timeline",
        subject: Subject::Language,
        title: "Timeline",
        blurb: "A timeline of dated events drawn to scale, with the span and the longest gap worked out, or with the dates or event names hidden for students to fill in.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("timeline", "hide") => !p.b(SHOW_ANSWERS),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "timeline" => vec![
            Example { name: "American history", params: vec![] },
            Example { name: "Ancient Rome (BC dates negative)", params: vec![("events", s("-753 Rome is founded\n-509 The Republic begins\n-44 Julius Caesar is killed\n-27 Augustus becomes emperor\n476 The western empire falls"))] },
            Example { name: "A school day", params: vec![("unit", s("minutes")), ("events", s("0 Bell rings\n5 Register taken\n55 Break\n70 Second lesson\n125 Lunch"))] },
            Example { name: "Hidden event names", params: vec![("show_answers", Value::B(false)), ("hide", s(HIDE[1]))] },
            Example { name: "Hidden dates", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("events", "Events: date then what happened", DEFAULT).hint("One event per line: a number (a year; negative for BC) then the event; 2 to 12 events, in any order"),
        ParamDef::text("unit", "Unit of the dates", "years"),
        ParamDef::choice("hide", "Hide for students", &HIDE, HIDE[0]).hint("Used when Show answers is off"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the dates (or the event names) and the readouts for students to fill in"),
    ]
}

#[derive(Debug)]
pub struct Event {
    pub date: f64,
    pub name: String,
}

pub fn parse(spec: &str) -> Result<Vec<Event>, String> {
    if spec.chars().count() > 1500 {
        return Err("The events are limited to 1500 characters".into());
    }
    let mut v = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (num, rest) = line.split_once(char::is_whitespace).ok_or_else(|| format!("\u{201c}{}\u{201d} needs a date and an event", line.chars().take(20).collect::<String>()))?;
        let date: f64 = num.parse().ok().filter(|x: &f64| x.is_finite() && x.abs() <= 1.0e9).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a date", num.chars().take(14).collect::<String>()))?;
        v.push(Event { date, name: rest.trim().chars().take(70).collect() });
        if v.len() > MAX_EVENTS {
            return Err(format!("A timeline is limited to {MAX_EVENTS} events"));
        }
    }
    if v.len() < 2 {
        return Err("Give at least two events, such as 1066 Battle of Hastings".into());
    }
    v.sort_by(|a, b| a.date.total_cmp(&b.date));
    Ok(v)
}

/// The biggest gap between neighbouring dates, as (index of the earlier event, gap).
pub fn longest_gap(ev: &[Event]) -> (usize, f64) {
    (1..ev.len()).map(|i| (i - 1, ev[i].date - ev[i - 1].date)).fold((0, f64::MIN), |b, c| if c.1 > b.1 { c } else { b })
}

fn render(p: &Params) -> String {
    let ev = match parse(p.s("events")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let answers = p.b(SHOW_ANSWERS);
    let hide_dates = !answers && p.s("hide") != HIDE[1];
    let hide_names = !answers && p.s("hide") == HIDE[1];
    let unit: String = p.s("unit").trim().chars().take(14).collect();
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (lo, hi) = (ev[0].date, ev[ev.len() - 1].date);
    let span = hi - lo;
    let pad = if span > 0.0 { span * 0.05 } else { 1.0 };
    let (alo, ahi, step) = chart::axis_range(lo - pad, hi + pad, 8.0);
    let (w, h) = (1000.0, top + 590.0);
    let (ax0, ax1, ay) = (60.0, 940.0, top + 250.0);
    let px = |v: f64| ax0 + (v - alo) / (ahi - alo).max(1e-9) * (ax1 - ax0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    d.arrow(ax0 - 20.0, ay, ax1 + 30.0, ay, INK, 3.0);
    let tick_list = chart::ticks(alo, ahi, step);
    for (v, _) in &tick_list {
        let x = px(*v);
        d.line(x, ay - 6.0, x, ay + 6.0, INK, 1.6);
    }
    // each event takes the first free level, above and below the line alternately
    let levels: [f64; 4] = [-1.0, 1.0, -2.0, 2.0];
    let mut edge = [f64::MIN; 4];
    let bw = 160.0;
    for e in &ev {
        let x = px(e.date);
        let c = x.clamp(ax0 - 10.0 + bw / 2.0 - 40.0, w - bw / 2.0 - 10.0);
        let li = (0..4).find(|&i| edge[i] <= c - bw / 2.0).unwrap_or_else(|| (0..4).min_by(|&a, &b| edge[a].total_cmp(&edge[b])).unwrap());
        edge[li] = c + bw / 2.0;
        let dir = levels[li].signum();
        let reach = 62.0 + (levels[li].abs() - 1.0) * 96.0;
        let ey = ay + dir * reach;
        d.line(x, ay, x, ey, "#6b7280", 1.4);
        d.circle(x, ay, 5.0, INK, INK, 1.0);
        let lines = wrap(&e.name, 21, 3);
        let date = trim(e.date, 3);
        // the box sits beyond the stem: text rows run away from the line
        let rows = 1 + lines.len();
        let bh = rows as f64 * fs * 1.3 + 10.0;
        let by = if dir < 0.0 { ey - bh } else { ey };
        d.rect(c - bw / 2.0, by, bw, bh, "#fff", INK, 1.2);
        let mut y = by + fs * 1.15;
        if hide_dates {
            d.line(c - 40.0, y + 2.0, c + 40.0, y + 2.0, INK, 1.0);
        } else {
            chart::text(&mut d, c, y, fs * 0.95, "middle", &date);
        }
        for l in &lines {
            y += fs * 1.3;
            if hide_names {
                d.line(c - bw / 2.0 + 12.0, y + 2.0, c + bw / 2.0 - 12.0, y + 2.0, INK, 1.0);
            } else {
                chart::text(&mut d, c, y, fs * 0.9, "middle", l);
            }
        }
    }
    if !hide_dates {
        for (v, label) in &tick_list {
            let x = px(*v);
            d.rect(x - 20.0, ay + 10.0, 40.0, 19.0, "#fff", "none", 0.0);
            chart::text(&mut d, x, ay + 24.0, fs * 0.85, "middle", label);
        }
        if !unit.is_empty() {
            chart::text(&mut d, ax1 + 30.0, ay - 12.0, fs * 0.85, "end", &unit);
        }
    }
    // the readouts
    let (gi, gap) = longest_gap(&ev);
    let mut y = ay + 270.0;
    let unit_s = if unit.is_empty() { String::new() } else { format!(" {unit}") };
    let items = [
        ("Time from the first event to the last:", format!("{}{unit_s}", trim(span, 3))),
        ("Longest gap between neighbouring events:", format!("{}{unit_s}, between {} and {}", trim(gap, 3), trim(ev[gi].date, 3), trim(ev[gi + 1].date, 3))),
    ];
    for (label, val) in items {
        chart::text(&mut d, 40.0, y, fs, "start", label);
        if answers {
            chart::text(&mut d, 40.0 + 330.0, y, fs, "start", &val);
        } else {
            d.line(40.0 + 330.0, y + 3.0, 40.0 + 600.0, y + 3.0, INK, 1.0);
        }
        y += fs * 1.9;
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn events_parse_and_sort() {
        let v = parse("1776 B\n-44 Caesar killed\n1.5 half").unwrap();
        assert_eq!(v.iter().map(|e| e.date).collect::<Vec<_>>(), vec![-44.0, 1.5, 1776.0]);
        assert_eq!(v[0].name, "Caesar killed");
        for (spec, needle) in [("", "at least two"), ("1 a", "at least two"), ("1066", "date and an event"), ("abc x\n1 y", "not a date"), ("1e999 x\n1 y", "not a date"), ("nan x\n1 y", "not a date")] {
            assert!(parse(spec).unwrap_err().contains(needle), "{spec:?}");
        }
        assert!(parse(&"1 a\n".repeat(13)).unwrap_err().contains("limited to 12"));
        assert!(parse(&"1 a\n".repeat(600)).unwrap_err().contains("1500"));
    }

    #[test]
    fn the_gap_is_found() {
        let v = parse("1 a\n3 b\n10 c\n12 d").unwrap();
        let (i, g) = longest_gap(&v);
        assert_eq!((i, g), (1, 7.0));
    }

    #[test]
    fn key_works_the_readouts() {
        let svg = render(&defaults());
        // 1492 to 1865 = 373; longest gap 1492..1620 = 128, then 1620..1776 = 156
        assert!(svg.contains("373 years") && svg.contains("156 years, between 1620 and 1776"));
        assert!(svg.contains(">1066<") == false && svg.contains(">1492<"));
    }

    #[test]
    fn student_copies_hide_what_was_chosen() {
        let mut p = defaults();
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(!student.contains(">1492<") && student.contains("Columbus") && key.contains(">1492<") && !student.contains("373 years"));
        assert_eq!(size_of(&student), size_of(&key));
        p.set_s("hide", HIDE[1]);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(student.contains(">1492<") && !student.contains("Columbus") && key.contains("Columbus"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn same_dates_and_one_date_are_safe() {
        let mut p = defaults();
        p.set_s("events", "5 a\n5 b\n5 c\n5 d\n5 e\n5 f");
        assert!(sane(&render(&p)));
        p.set_s("events", "-1e9 a\n1e9 b");
        assert!(sane(&render(&p)));
    }

    #[test]
    fn controls_follow_show_answers() {
        let mut p = defaults();
        assert!(!relevant("timeline", "hide", &p));
        p.set_b(SHOW_ANSWERS, false);
        assert!(relevant("timeline", "hide", &p));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("timeline").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("not a date") && !svg.contains("at least two"), "{}", ex.name);
        }
        for spec in ["\u{0} x\n1 y", "<b> x\n1 y", "1 <b>x</b>\n2 y", "1 w\n2 ".repeat(1).as_str(), &format!("1 {}\n2 y", "w".repeat(500))] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("events", spec);
                p.set_s("unit", "<u>");
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>") && !svg.contains("<u>"), "{spec:?}");
            }
        }
    }
}
