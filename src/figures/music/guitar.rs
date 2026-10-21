//! Guitar chord boxes: up to four chords side by side, each a grid of six strings and five frets
//! with dots where to press, `o` for an open string and `x` for one not played. Written as a name
//! and six fret numbers (`C x32010`: low string first). Two worksheets from one drawing: Name the
//! chord shows the dots and leaves the name as a blank line; Mark the chord shows the name and
//! leaves the grid empty. Show answers off leaves out whichever half is the answer.
use super::glyphs::INK;
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const TASKS: [&str; 2] = ["Name the chord", "Mark the chord"];
const MAX_CHORDS: usize = 4;
const STRINGS: [&str; 6] = ["E", "A", "D", "G", "B", "E"];
const DEFAULT: &str = "C x32010\nG 320003\nAm x02210\nEm 022000";
const SG: f64 = 24.0;
const FG: f64 = 28.0;
const CELL: f64 = 200.0;

pub fn def() -> FigureDef {
    FigureDef {
        id: "guitar-chords",
        subject: Subject::Music,
        title: "Guitar chord boxes",
        blurb: "Guitar chord boxes from a name and six fret numbers, with the chord to name, or with the name given and the box left empty for students to mark.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "guitar-chords" => vec![
            Example { name: "Four open chords", params: vec![] },
            Example { name: "A barre chord up the neck", params: vec![("chords", s("F 133211\nBm x24432")), ("fret", Value::I(0))] },
            Example { name: "Mark the chords", params: vec![("task", s(TASKS[1]))] },
            Example { name: "Blank to mark", params: vec![("task", s(TASKS[1])), ("show_answers", Value::B(false))] },
            Example { name: "Blank to name", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("chords", "Chords: name, then six frets", DEFAULT).hint("One per line, such as C x32010: the low E string first, x for a string not played, 0 for open, 1 to 9 for a fret; up to 4"),
        ParamDef::choice("task", "Worksheet", &TASKS, TASKS[0]),
        ParamDef::int("fret", "First fret shown", 0, 12, 0).hint("0 picks the first fret for you"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves out the half that is the answer: the chord name (Name the chord) or the dots (Mark the chord)"),
    ]
}

/// One string: not played, or the fret it is pressed at (0 is open).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Str {
    Muted,
    Fret(u8),
}

#[derive(Debug)]
pub struct Chord {
    pub name: String,
    pub strings: [Str; 6],
}

fn parse(spec: &str) -> Result<Vec<Chord>, String> {
    let mut v = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let shown = || line.chars().take(20).collect::<String>();
        if toks.len() != 2 {
            return Err(format!("\u{201c}{}\u{201d} should be a name and six frets, such as C x32010", shown()));
        }
        let frets: Vec<char> = toks[1].chars().collect();
        if frets.len() != 6 {
            return Err(format!("\u{201c}{}\u{201d} needs six frets, one for each string", toks[1].chars().take(12).collect::<String>()));
        }
        let mut strings = [Str::Muted; 6];
        for (i, c) in frets.iter().enumerate() {
            strings[i] = match c {
                'x' | 'X' => Str::Muted,
                '0'..='9' => Str::Fret(*c as u8 - b'0'),
                _ => return Err(format!("\u{201c}{}\u{201d} should be x or a fret from 0 to 9", c)),
            };
        }
        v.push(Chord { name: toks[0].chars().take(8).collect(), strings });
        if v.len() > MAX_CHORDS {
            return Err(format!("A figure is limited to {MAX_CHORDS} chords"));
        }
    }
    if v.is_empty() {
        return Err("Give a chord, such as C x32010".into());
    }
    Ok(v)
}

/// The first fret of the five shown: the one asked for, else fret 1 for chords inside the first
/// five frets and the lowest pressed fret otherwise.
pub fn base_fret(c: &Chord, asked: i64) -> Result<u8, String> {
    let pressed: Vec<u8> = c.strings.iter().filter_map(|s| match s { Str::Fret(f) if *f > 0 => Some(*f), _ => None }).collect();
    let (lo, hi) = (pressed.iter().min().copied(), pressed.iter().max().copied());
    let base = if asked > 0 { asked as u8 } else if hi.map_or(true, |h| h <= 5) { 1 } else { lo.unwrap_or(1) };
    if let (Some(l), Some(h)) = (lo, hi) {
        if l < base || h > base + 4 {
            return Err(format!("{} spans frets {l} to {h}, which do not fit in five frets from fret {base}", c.name));
        }
    }
    Ok(base)
}

fn render(p: &Params) -> String {
    let chords = match parse(p.s("chords")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let mut bases = Vec::new();
    for c in &chords {
        match base_fret(c, p.i("fret").clamp(0, 12)) {
            Ok(b) => bases.push(b),
            Err(e) => return chart::message(&e),
        }
    }
    let name_task = p.s("task") != TASKS[1];
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let w = (40.0 + chords.len() as f64 * CELL).max(640.0);
    let h = top + 40.0 + 36.0 + 5.0 * FG + 60.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let dot = if print { "#333333" } else { "#e0662a" };
    for (ci, c) in chords.iter().enumerate() {
        let gx = 70.0 + ci as f64 * CELL;
        let gy = top + 40.0 + 36.0;
        let base = bases[ci];
        // the name
        let nx = gx + 2.5 * SG;
        let show_name = !name_task || answers;
        if show_name {
            chart::text(&mut d, nx, top + 36.0, fs * 1.5, "middle", &c.name);
        } else {
            d.line(nx - 36.0, top + 40.0, nx + 36.0, top + 40.0, INK, 1.2);
        }
        // the grid
        for s in 0..6 {
            d.line(gx + s as f64 * SG, gy, gx + s as f64 * SG, gy + 5.0 * FG, INK, 1.3);
            chart::text(&mut d, gx + s as f64 * SG, gy + 5.0 * FG + 18.0, fs * 0.85, "middle", STRINGS[s]);
        }
        for f in 0..=5 {
            d.line(gx, gy + f as f64 * FG, gx + 5.0 * SG, gy + f as f64 * FG, INK, if f == 0 && base == 1 { 4.5 } else { 1.3 });
        }
        if base > 1 {
            chart::text(&mut d, gx - 10.0, gy + FG * 0.5 + fs * 0.35, fs, "end", &format!("{base}fr"));
        }
        // the dots, o and x
        let show_marks = answers || name_task;
        if show_marks {
            for (s, st) in c.strings.iter().enumerate() {
                let x = gx + s as f64 * SG;
                match st {
                    Str::Muted => chart::text(&mut d, x, gy - 8.0, fs * 1.1, "middle", "x"),
                    Str::Fret(0) => {
                        d.circle(x, gy - 12.0, 5.5, "#fff", INK, 1.6);
                    }
                    Str::Fret(f) => {
                        let row = (*f - base) as f64;
                        d.circle(x, gy + (row + 0.5) * FG, 9.0, dot, INK, 1.2);
                    }
                }
            }
        }
    }
    if !name_task && !answers {
        chart::text(&mut d, 40.0, h - 14.0, fs * 0.9, "start", "Mark the dots, then mark open strings (o) and strings not played (x).");
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn chords_parse() {
        let v = parse("C x32010\nG 320003").unwrap();
        assert_eq!(v[0].strings, [Str::Muted, Str::Fret(3), Str::Fret(2), Str::Fret(0), Str::Fret(1), Str::Fret(0)]);
        assert_eq!(v[1].name, "G");
        assert!(parse("").unwrap_err().contains("Give a chord"));
        assert!(parse("C").unwrap_err().contains("name and six frets"));
        assert!(parse("C x3201").unwrap_err().contains("six frets"));
        assert!(parse("C x32010 extra").is_err());
        assert!(parse("C x3201q").unwrap_err().contains("x or a fret"));
        assert!(parse(&"C 000000\n".repeat(5)).unwrap_err().contains("limited to 4"));
    }

    #[test]
    fn the_first_fret_is_picked_or_checked() {
        let c = |s: &str| parse(&format!("A {s}")).unwrap().remove(0);
        assert_eq!(base_fret(&c("x02210"), 0), Ok(1));
        assert_eq!(base_fret(&c("000000"), 0), Ok(1));
        assert_eq!(base_fret(&c("xxxxxx"), 0), Ok(1));
        assert_eq!(base_fret(&c("577655"), 0), Ok(5));
        assert_eq!(base_fret(&c("577655"), 4), Ok(4));
        assert!(base_fret(&c("577655"), 7).is_err());
        assert!(base_fret(&c("1x9xxx"), 0).is_err());
        assert_eq!(base_fret(&c("133211"), 0), Ok(1));
    }

    #[test]
    fn name_worksheet_shows_dots_and_not_names() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains("circle") && !student.contains(">Am<") && key.contains(">Am<") && key.contains(">Em<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn mark_worksheet_shows_names_and_not_dots() {
        let mut p = defaults();
        p.set_s("task", TASKS[1]);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(student.contains(">Am<") && !student.contains("fill=\"#e0662a\"") && key.contains("fill=\"#e0662a\""));
        assert!(!student.contains(">x<") && key.contains(">x<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn a_high_chord_labels_its_first_fret() {
        let mut p = defaults();
        p.set_s("chords", "B 799877");
        assert!(render(&p).contains(">7fr<"));
        p.set_s("chords", "G 355433");
        assert!(!render(&p).contains("fr<"), "inside the first five frets, so it starts at the nut");
        p.set_i("fret", 3);
        assert!(render(&p).contains(">3fr<"));
    }

    #[test]
    fn bad_chords_are_explained() {
        let mut p = defaults();
        p.set_s("chords", "X 1x9xxx");
        assert!(render(&p).contains("do not fit"));
        p.set_s("chords", "");
        assert!(render(&p).contains("Give a chord"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("guitar-chords").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("do not fit") && !svg.contains("should be"), "{}", ex.name);
        }
        for spec in ["\u{0} 000000", "<b> xxxxxx", "nan 123456", "C 99999999", "C ......", &"C x32010\n".repeat(50)] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("chords", spec);
                p.set_i("fret", 9999);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
