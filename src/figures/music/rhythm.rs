//! Rhythm counting: a time signature and a rhythm written as lengths (`q q h | q. e q q`), drawn
//! as notes and rests on a staff line with the beats of each bar laid out on a grid underneath.
//! The rhythm is the question; the count under each note (`1 & 2 e & a`) and the number of beats
//! in each bar are the answers, so the student copy (Show answers off) leaves them as blank lines.
use super::glyphs::{self, INK};
use super::{parse_dur, Dur};
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_ITEMS: usize = 40;
const MAX_SPEC: usize = 600;
const DEFAULT: &str = "q q h | q. e q q | e e e e q q | h r q q";
const SP: f64 = 11.0;

pub fn def() -> FigureDef {
    FigureDef {
        id: "rhythm-counting",
        subject: Subject::Music,
        title: "Rhythm counting",
        blurb: "A rhythm written as note and rest lengths in a time signature, drawn with bar lines and a beat grid, with the count under each note and the beats in each bar worked out or left blank for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "rhythm-counting" => vec![
            Example { name: "Four-four", params: vec![] },
            Example { name: "Three-four", params: vec![("time", s("3/4")), ("rhythm", s("q q q | h q | q. e q | h."))] },
            Example { name: "Six-eight", params: vec![("time", s("6/8")), ("rhythm", s("q e q e | e e e e e e | q. q."))] },
            Example { name: "Sixteenths", params: vec![("rhythm", s("s s s s e e h | q. s s h"))] },
            Example { name: "A bar that does not add up", params: vec![("rhythm", s("q q q | h h"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("rhythm", "Rhythm", DEFAULT).hint("Lengths separated by spaces or commas: w h q e s (whole to sixteenth), a dot after makes it longer (q.), r before a length is a rest (r q); | ends a bar; up to 40"),
        ParamDef::text("time", "Time signature", "4/4").hint("Beats per bar over the beat note: 4/4, 3/4, 6/8, 2/2"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the rhythm and the grid and leaves the counts and the beats in each bar as blank lines"),
    ]
}

#[derive(Clone, Copy, Debug)]
struct Item {
    rest: bool,
    dur: Dur,
    bar_after: bool,
}

fn parse(spec: &str) -> Result<Vec<Item>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The rhythm is limited to {MAX_SPEC} characters"));
    }
    let mut v: Vec<Item> = Vec::new();
    let mut rest_next = false;
    let spaced = spec.replace('|', " | ").replace(',', " ");
    for tok in spaced.split_whitespace() {
        if tok == "|" {
            if let Some(last) = v.last_mut() {
                last.bar_after = true;
            }
            continue;
        }
        if tok.eq_ignore_ascii_case("r") {
            rest_next = true;
            continue;
        }
        let dur = parse_dur(tok).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a length: use w h q e s, with a dot for longer, or r before one for a rest", tok.chars().take(12).collect::<String>()))?;
        v.push(Item { rest: rest_next, dur, bar_after: false });
        rest_next = false;
        if v.len() > MAX_ITEMS {
            return Err(format!("A figure is limited to {MAX_ITEMS} notes and rests"));
        }
    }
    if rest_next {
        return Err("A rest needs a length after it, such as r q".into());
    }
    if v.is_empty() {
        return Err("Give a rhythm, such as q q h".into());
    }
    Ok(v)
}

fn parse_time(s: &str) -> Option<(u32, u32)> {
    let (n, d) = s.trim().split_once('/')?;
    let (n, d): (u32, u32) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
    ((1..=16).contains(&n) && [2, 4, 8, 16].contains(&d)).then_some((n, d))
}

/// The count said for a note that starts `beat` beats into the bar (counted from 0): `1`, `1 e`
/// is written `e`, `&` and `a` for the quarters of a beat; None when it starts between the quarters.
pub fn count_label(beat: f64) -> Option<String> {
    let whole = beat.floor();
    let q = ((beat - whole) * 4.0).round();
    if ((beat - whole) * 4.0 - q).abs() > 1e-6 {
        return None;
    }
    Some(match q as i32 {
        0 => format!("{}", whole as i64 + 1),
        1 => "e".into(),
        2 => "&".into(),
        3 => "a".into(),
        _ => return None,
    })
}

/// Bars as lists of item indices: a bar ends when it holds `bar` beats or at a `|`.
fn bars(items: &[Item], bar_beats: f64, unit: f64) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = vec![Vec::new()];
    let mut acc = 0.0;
    for (i, it) in items.iter().enumerate() {
        out.last_mut().unwrap().push(i);
        acc += it.dur.quarters() * unit;
        if acc >= bar_beats - 1e-9 || it.bar_after {
            out.push(Vec::new());
            acc = 0.0;
        }
    }
    if out.last().map_or(false, |b| b.is_empty()) {
        out.pop();
    }
    out
}

fn render(p: &Params) -> String {
    let items = match parse(p.s("rhythm")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let Some((tn, td)) = parse_time(p.s("time")) else {
        return chart::message("The time signature should look like 4/4 or 6/8");
    };
    // one beat is the note the bottom number names, so a beat is 4/td quarter notes
    let unit = td as f64 / 4.0;
    let beats = |i: usize| items[i].dur.quarters() * unit;
    let measures = bars(&items, tn as f64, unit);
    // every start must land on a quarter of a beat to be counted
    for m in &measures {
        let mut t = 0.0;
        for &i in m {
            if count_label(t).is_none() {
                return chart::message("A note starts between the quarters of a beat, which this count cannot name");
            }
            t += beats(i);
        }
    }
    let answers = p.b(SHOW_ANSWERS);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    // wide beats for sixteenths, narrow ones for long bars such as 6/8 and 12/8
    let shortest = (0..items.len()).map(beats).fold(f64::INFINITY, f64::min);
    let beat_w = if shortest < 0.5 - 1e-9 { 88.0 } else if tn > 4 { 46.0 } else { 64.0 };
    let total_of = |m: &Vec<usize>| m.iter().map(|&i| beats(i)).sum::<f64>();
    let width_of = |m: &Vec<usize>| total_of(m).max(tn as f64) * beat_w + 28.0;
    let (x0, max_w) = (24.0, 860.0);
    let head = 56.0;
    let mut systems: Vec<Vec<usize>> = vec![Vec::new()];
    let mut used = head;
    for (mi, m) in measures.iter().enumerate() {
        let w = width_of(m);
        if used + w > max_w && !systems.last().unwrap().is_empty() {
            systems.push(Vec::new());
            used = head;
        }
        systems.last_mut().unwrap().push(mi);
        used += w;
    }
    let sys_h = 12.0 * SP + 96.0;
    let h = top + systems.len() as f64 * sys_h + 30.0;
    let w = x0 + max_w + 26.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let note_step_y = |yb: f64| yb - 2.0 * SP; // the middle line
    for (si, sys) in systems.iter().enumerate() {
        let yb = top + si as f64 * sys_h + 6.0 * SP;
        let right = x0 + head + sys.iter().map(|&mi| width_of(&measures[mi])).sum::<f64>();
        glyphs::staff_lines(&mut d, x0, right, yb, SP);
        d.line(x0, yb, x0, yb - 4.0 * SP, INK, 1.6);
        if si == 0 {
            chart::text(&mut d, x0 + 28.0, yb - 3.0 * SP + SP * 0.85, SP * 2.4, "middle", &tn.to_string());
            chart::text(&mut d, x0 + 28.0, yb - SP + SP * 0.85, SP * 2.4, "middle", &td.to_string());
        }
        let mut x = x0 + head;
        for (k, &mi) in sys.iter().enumerate() {
            let m = &measures[mi];
            let bw = width_of(m);
            let mut t = 0.0;
            // the beat grid under the bar
            let gy = yb + 4.5 * SP;
            let nb = total_of(m).max(tn as f64);
            for b in 0..=(nb.ceil() as i64) {
                let gx = x + 6.0 + b as f64 * beat_w;
                if (b as f64) <= nb + 1e-9 {
                    d.line(gx, gy, gx, gy + 36.0, "#9ca3af", 1.0);
                }
            }
            for &i in m {
                let it = &items[i];
                let cx = x + 6.0 + t * beat_w + 14.0;
                if it.rest {
                    glyphs::rest(&mut d, cx, it.dur.kind, yb, SP);
                    if it.dur.dotted {
                        glyphs::dot(&mut d, cx + 4.0, yb - 2.0 * SP - SP * 0.5, false, SP);
                    }
                } else {
                    let y = note_step_y(yb);
                    glyphs::notehead(&mut d, cx, y, it.dur.kind, SP);
                    glyphs::stem(&mut d, cx, y, it.dur.kind, false, SP);
                    if it.dur.dotted {
                        glyphs::dot(&mut d, cx, y, true, SP);
                    }
                }
                // the length of the note as a bar on the grid
                let len = beats(i) * beat_w;
                d.rect(x + 6.0 + t * beat_w + 2.0, gy + 6.0, (len - 4.0).max(2.0), 8.0, if it.rest { "#fff" } else { "#cbd5e1" }, INK, 1.0);
                let label = count_label(t).unwrap_or_default();
                if answers {
                    chart::text(&mut d, cx, gy + 34.0, fs * 1.05, "middle", &label);
                } else {
                    let hw = (beats(i) * beat_w / 2.0 - 3.0).clamp(3.0, 10.0);
                    d.line(cx - hw, gy + 36.0, cx + hw, gy + 36.0, INK, 1.0);
                }
                t += beats(i);
            }
            x += bw;
            let last = si + 1 == systems.len() && k + 1 == sys.len();
            if last {
                d.line(x - 8.0, yb, x - 8.0, yb - 4.0 * SP, INK, 1.4);
                d.rect(x - 4.0, yb - 4.0 * SP, 3.2, 4.0 * SP, INK, INK, 0.5);
            } else {
                d.line(x - 6.0, yb, x - 6.0, yb - 4.0 * SP, INK, 1.4);
            }
            // the beats in the bar
            let by = yb + 4.5 * SP + 62.0;
            let total = total_of(m);
            let ok = (total - tn as f64).abs() < 1e-9;
            if answers {
                let txt = format!("{} beat{}{}", crate::figures::geography::trim(total, 3), if (total - 1.0).abs() < 1e-9 { "" } else { "s" }, if ok { " \u{2713}".to_string() } else { format!(" (needs {tn})") });
                chart::text(&mut d, x - bw / 2.0, by, fs * 0.95, "middle", &txt);
            } else {
                d.line(x - bw / 2.0 - 28.0, by + 3.0, x - bw / 2.0 + 28.0, by + 3.0, INK, 1.0);
            }
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn counts_name_the_quarters_of_a_beat() {
        assert_eq!(count_label(0.0).unwrap(), "1");
        assert_eq!(count_label(0.25).unwrap(), "e");
        assert_eq!(count_label(0.5).unwrap(), "&");
        assert_eq!(count_label(0.75).unwrap(), "a");
        assert_eq!(count_label(3.0).unwrap(), "4");
        assert_eq!(count_label(2.5).unwrap(), "&");
        assert_eq!(count_label(0.125), None);
        assert_eq!(count_label(1.0 / 3.0), None);
    }

    #[test]
    fn rhythms_parse() {
        let v = parse("q q h | q. e r q").unwrap();
        assert_eq!(v.len(), 6);
        assert!(v[2].bar_after && !v[0].bar_after);
        assert!(v[5].rest && !v[4].rest);
        assert!(parse("").unwrap_err().contains("Give a rhythm"));
        assert!(parse("q x").unwrap_err().contains("not a length"));
        assert!(parse("q r").unwrap_err().contains("needs a length"));
        assert!(parse(&"q ".repeat(41)).unwrap_err().contains("limited to 40"));
        assert!(parse(&"q ".repeat(400)).unwrap_err().contains("limited to 600"));
    }

    #[test]
    fn bars_follow_the_time_signature() {
        let v = parse("q q h q q h").unwrap();
        // 4/4: a beat is a quarter
        assert_eq!(bars(&v, 4.0, 1.0), vec![vec![0, 1, 2], vec![3, 4, 5]]);
        // 6/8: a beat is an eighth, a quarter is two beats, so 6 beats is three quarters
        let v = parse("q q q q q q").unwrap();
        assert_eq!(bars(&v, 6.0, 2.0), vec![vec![0, 1, 2], vec![3, 4, 5]]);
        // 2/2: a beat is a half, so a whole note is two beats
        assert_eq!(bars(&parse("w w").unwrap(), 2.0, 0.5), vec![vec![0], vec![1]]);
    }

    #[test]
    fn key_counts_each_note_and_totals_each_bar() {
        let svg = render(&defaults());
        for t in [">1<", ">2<", ">3<", ">4<", ">&amp;<", ">4 beats \u{2713}<"] {
            assert!(svg.contains(t), "{t}");
        }
        let mut p = defaults();
        p.set_s("rhythm", "q q q | h h");
        let svg = render(&p);
        assert!(svg.contains("3 beats (needs 4)") && svg.contains("4 beats \u{2713}"));
        p.set_s("rhythm", "s s s s e e q h");
        let svg = render(&p);
        assert!(svg.contains(">e<") && svg.contains(">a<"));
    }

    #[test]
    fn compound_time_counts_in_eighths() {
        let mut p = defaults();
        p.set_s("time", "6/8");
        p.set_s("rhythm", "q e q e");
        let svg = render(&p);
        assert!(svg.contains("6 beats \u{2713}") && svg.contains(">3<") && svg.contains(">4<"), "{}", svg.len());
    }

    #[test]
    fn a_note_between_the_quarters_is_explained() {
        let mut p = defaults();
        p.set_s("rhythm", "s. s q h");
        assert!(render(&p).contains("between the quarters"));
    }

    #[test]
    fn student_copy_leaves_blank_lines() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(!student.contains("beats") && !student.contains(">&amp;<") && student.matches("<line").count() > key.matches("<line").count());
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn long_rhythms_wrap() {
        let mut p = defaults();
        p.set_s("rhythm", &"q q q q | ".repeat(9));
        let svg = render(&p);
        assert!(sane(&svg));
        let h: f64 = svg.split("height=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
        assert!(h > 400.0);
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("rhythm-counting").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("not a length") && !svg.contains("between the quarters"), "{}", ex.name);
        }
        for spec in ["\u{0}", "<b>", "nan", "q q ||| q", "r r r", "w w w w w w w w w w", "s s s s s s s s s s s s s s s s s s s s"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("rhythm", spec);
                p.set_s("time", if spec == "nan" { "99/99" } else { "4/4" });
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
