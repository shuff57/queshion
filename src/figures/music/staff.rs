//! A staff written from a text spec such as `C4 q, E4 q, G4 h` (a pitch and a length, `r` for a
//! rest), with a clef, a key signature, a time signature and bar lines drawn from the lengths.
//! The notes are the question; the note names written under them and the beats in each bar are
//! the answers, so the student copy (Show answers off) leaves them as blank lines.
use super::glyphs::{self, INK};
use super::{parse_dur, parse_pitch, Dur, Pitch};
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use std::collections::HashMap;

const CLEFS: [&str; 3] = ["Treble", "Bass", "Alto"];
const KEYS: [&str; 15] = [
    "C major (no sharps or flats)", "G major (1 sharp)", "D major (2 sharps)", "A major (3 sharps)", "E major (4 sharps)", "B major (5 sharps)", "F# major (6 sharps)", "C# major (7 sharps)",
    "F major (1 flat)", "Bb major (2 flats)", "Eb major (3 flats)", "Ab major (4 flats)", "Db major (5 flats)", "Gb major (6 flats)", "Cb major (7 flats)",
];
const MAX_ITEMS: usize = 48;
const MAX_SPEC: usize = 1200;
const DEFAULT: &str = "C4 q, D4 q, E4 q, F4 q | G4 h, E4 h | F4 q, D4 q, B3 e, C4 e, r q";
const SP: f64 = 11.0;
/// Letters of the sharps and flats of a key signature, in the order they are added.
const SHARP_LETTERS: [u8; 7] = [3, 0, 4, 1, 5, 2, 6];
const FLAT_LETTERS: [u8; 7] = [6, 2, 5, 1, 4, 0, 3];
/// Treble-clef steps of the key signature signs (F5 C5 G5 D5 A4 E5 B4, and B4 E5 A4 D5 G4 C5 F4).
const SHARP_STEPS: [i32; 7] = [38, 35, 39, 36, 33, 37, 34];
const FLAT_STEPS: [i32; 7] = [34, 37, 33, 36, 32, 35, 31];

pub fn def() -> FigureDef {
    FigureDef {
        id: "music-staff",
        subject: Subject::Music,
        title: "Staff and notes",
        blurb: "Music on a staff written from a text spec like C4 q, E4 q, G4 h, with a clef, key signature, time signature and bar lines, and the note names and beats per bar worked out or left blank for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "music-staff" => vec![
            Example { name: "C major scale up", params: vec![("notes", s("C4 q, D4 q, E4 q, F4 q | G4 q, A4 q, B4 q, C5 q"))] },
            Example { name: "G major, three-four", params: vec![("key", s(KEYS[1])), ("time", s("3/4")), ("notes", s("G4 q, B4 q, D5 q | C5 q., B4 e, A4 q | F#4 q, A4 q, D5 q | G4 h."))] },
            Example { name: "Bass clef with a flat", params: vec![("clef", s("Bass")), ("key", s(KEYS[8])), ("notes", s("F2 q, A2 q, C3 q, A2 q | Bb2 h, F2 h | G2 e, A2 e, Bb2 e, C3 e, D3 h"))] },
            Example { name: "Alto clef", params: vec![("clef", s("Alto")), ("notes", s("C4 q, E4 q, G4 q, E4 q | C4 w"))] },
            Example { name: "Rests and accidentals", params: vec![("notes", s("C5 q, r q, A4 e, G#4 e, A4 q | r h, E4 q., F4 e"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("notes", "Notes", DEFAULT).hint("A pitch and a length, separated by commas or | : C4 q, F#4 e, Bb3 h. Lengths w h q e s (whole to sixteenth), a dot after makes it longer (q.), r is a rest; up to 48"),
        ParamDef::choice("clef", "Clef", &CLEFS, CLEFS[0]),
        ParamDef::choice("key", "Key signature", &KEYS, KEYS[0]),
        ParamDef::text("time", "Time signature", "4/4").hint("Beats per bar over the beat note, such as 4/4, 3/4, 6/8"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the staff and notes and leaves the note names and the beats in each bar as blank lines"),
    ]
}

#[derive(Clone, Copy)]
struct Item {
    pitch: Option<Pitch>,
    dur: Dur,
    /// A `|` was written after this note: the bar ends here even if it is short or long.
    bar_after: bool,
}

fn parse(spec: &str) -> Result<Vec<Item>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The notes are limited to {MAX_SPEC} characters"));
    }
    let mut v: Vec<Item> = Vec::new();
    for part in spec.split_inclusive(|c: char| c == ',' || c == '|' || c == '\n' || c == ';') {
        let ends_bar = part.ends_with('|');
        let part = part.trim_end_matches(|c: char| c == ',' || c == '|' || c == '\n' || c == ';').trim();
        if part.is_empty() {
            if ends_bar {
                if let Some(last) = v.last_mut() {
                    last.bar_after = true;
                }
            }
            continue;
        }
        let toks: Vec<&str> = part.split_whitespace().collect();
        let shown = || part.chars().take(20).collect::<String>();
        if toks.len() != 2 {
            return Err(format!("\u{201c}{}\u{201d} should be a pitch and a length, such as C4 q", shown()));
        }
        let dur = parse_dur(toks[1]).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a length: use w h q e s, with a dot for longer", toks[1].chars().take(12).collect::<String>()))?;
        let pitch = if toks[0].eq_ignore_ascii_case("r") {
            None
        } else {
            Some(parse_pitch(toks[0]).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a pitch: use a letter A to G, an optional # or b, then an octave, such as F#4", toks[0].chars().take(12).collect::<String>()))?)
        };
        v.push(Item { pitch, dur, bar_after: ends_bar });
        if v.len() > MAX_ITEMS {
            return Err(format!("A figure is limited to {MAX_ITEMS} notes and rests"));
        }
    }
    if v.is_empty() {
        return Err("Give some notes, such as C4 q, E4 q, G4 h".into());
    }
    Ok(v)
}

/// `N/D` with N from 1 to 16 and D one of 2, 4, 8, 16.
fn parse_time(s: &str) -> Option<(u32, u32)> {
    let (n, d) = s.trim().split_once('/')?;
    let (n, d): (u32, u32) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
    ((1..=16).contains(&n) && [2, 4, 8, 16].contains(&d)).then_some((n, d))
}

/// How many sharps (positive) or flats (negative) a key choice has.
pub fn key_count(key: &str) -> i32 {
    match KEYS.iter().position(|k| *k == key) {
        Some(i) if i < 8 => i as i32,
        Some(i) => 7 - i as i32,
        None => 0,
    }
}

/// The accidental the key signature gives a letter: 1, -1 or 0.
pub fn key_acc(count: i32, letter: u8) -> i8 {
    if count > 0 && SHARP_LETTERS[..count as usize].contains(&letter) {
        1
    } else if count < 0 && FLAT_LETTERS[..(-count) as usize].contains(&letter) {
        -1
    } else {
        0
    }
}

fn bottom_step(clef: &str) -> i32 {
    match clef { "Bass" => 18, "Alto" => 24, _ => 30 }
}

/// Splits the items into bars of `bar` quarter notes; the last bar may be short.
fn bars(items: &[Item], bar: f64) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = vec![Vec::new()];
    let mut acc = 0.0;
    for (i, it) in items.iter().enumerate() {
        out.last_mut().unwrap().push(i);
        acc += it.dur.quarters();
        if acc >= bar - 1e-9 || it.bar_after {
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
    let items = match parse(p.s("notes")) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let Some((tn, td)) = parse_time(p.s("time")) else {
        return chart::message("The time signature should look like 4/4 or 6/8");
    };
    let clef = CLEFS.iter().find(|c| **c == p.s("clef")).copied().unwrap_or("Treble");
    let kc = key_count(p.s("key"));
    let bs = bottom_step(clef);
    for it in &items {
        if let Some(pt) = it.pitch {
            let off = pt.step() - bs;
            if !(-8..=16).contains(&off) {
                return chart::message(&format!("{}{} is too far from the staff for this clef", pt.name(), pt.octave));
            }
        }
    }
    let answers = p.b(SHOW_ANSWERS);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let bar_q = tn as f64 * 4.0 / td as f64;
    let measures = bars(&items, bar_q);
    let ksig_w = kc.unsigned_abs() as f64 * 13.0;
    let acc_w = |it: &Item, shown: bool| if it.pitch.is_some() && shown { 16.0 } else { 0.0 };
    // which notes need an accidental sign, bar by bar
    let mut shown = vec![false; items.len()];
    for m in &measures {
        let mut state: HashMap<(u8, i8), i8> = HashMap::new();
        for &i in m {
            if let Some(pt) = items[i].pitch {
                let cur = state.get(&(pt.letter, pt.octave)).copied().unwrap_or_else(|| key_acc(kc, pt.letter));
                if cur != pt.acc {
                    shown[i] = true;
                    state.insert((pt.letter, pt.octave), pt.acc);
                }
            }
        }
    }
    let block_w = |i: usize| 26.0 + 12.0 * items[i].dur.quarters().min(4.0) + acc_w(&items[i], shown[i]);
    let measure_w = |m: &Vec<usize>| m.iter().map(|&i| block_w(i)).sum::<f64>() + 16.0;
    let (x0, max_w) = (24.0, 850.0);
    let head_w = |first: bool| 48.0 + ksig_w + if first { 34.0 } else { 0.0 };
    // pack bars into systems
    let mut systems: Vec<Vec<usize>> = vec![Vec::new()];
    let mut used = head_w(true);
    for (mi, m) in measures.iter().enumerate() {
        let w = measure_w(m);
        if used + w > max_w && !systems.last().unwrap().is_empty() {
            systems.push(Vec::new());
            used = head_w(false);
        }
        systems.last_mut().unwrap().push(mi);
        used += w;
    }
    let sys_h = 14.0 * SP + 44.0;
    let h = top + systems.len() as f64 * sys_h + 70.0;
    let w = x0 + max_w + 26.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let mid_step = bs + 4;
    let clef_shift = match clef { "Bass" => -14, "Alto" => -7, _ => 0 };
    let mut counts: Vec<String> = Vec::new();
    for (si, sys) in systems.iter().enumerate() {
        let yb = top + si as f64 * sys_h + 9.0 * SP;
        let first = si == 0;
        let right = x0 + head_w(first) + sys.iter().map(|&mi| measure_w(&measures[mi])).sum::<f64>();
        let right = if si + 1 == systems.len() { right } else { right.min(x0 + max_w) };
        glyphs::staff_lines(&mut d, x0, right, yb, SP);
        d.line(x0, yb, x0, yb - 4.0 * SP, INK, 1.6);
        glyphs::clef(&mut d, clef, x0 + 6.0, yb, SP);
        // key signature
        for k in 0..kc.unsigned_abs() as usize {
            let step = if kc > 0 { SHARP_STEPS[k] } else { FLAT_STEPS[k] } + clef_shift;
            glyphs::accidental(&mut d, x0 + 46.0 + k as f64 * 13.0, glyphs::step_y(step, bs, yb, SP), kc.signum() as i8, SP);
        }
        if first {
            let tx = x0 + 46.0 + ksig_w + 12.0;
            chart::text(&mut d, tx, yb - 3.0 * SP + SP * 0.85, SP * 2.4, "middle", &tn.to_string());
            chart::text(&mut d, tx, yb - SP + SP * 0.85, SP * 2.4, "middle", &td.to_string());
        }
        let mut x = x0 + head_w(first);
        for (k, &mi) in sys.iter().enumerate() {
            let m = &measures[mi];
            let mut state_beats = 0.0;
            for &i in m {
                let it = &items[i];
                let bw = block_w(i);
                let cx = x + 8.0 + acc_w(it, shown[i]) + 6.0;
                let rx = cx;
                match it.pitch {
                    Some(pt) => {
                        let y = glyphs::step_y(pt.step(), bs, yb, SP);
                        glyphs::ledger_lines(&mut d, cx, pt.step(), bs, yb, SP);
                        if shown[i] {
                            glyphs::accidental(&mut d, cx - 15.0, y, pt.acc, SP);
                        }
                        glyphs::notehead(&mut d, cx, y, it.dur.kind, SP);
                        glyphs::stem(&mut d, cx, y, it.dur.kind, pt.step() < mid_step, SP);
                        if it.dur.dotted {
                            glyphs::dot(&mut d, cx, y, (pt.step() - bs) % 2 == 0, SP);
                        }
                        // the name
                        let ny = yb + 5.0 * SP + 22.0;
                        if answers {
                            chart::text(&mut d, cx, ny, fs, "middle", &pt.name());
                        } else {
                            d.line(cx - 12.0, ny + 3.0, cx + 12.0, ny + 3.0, INK, 1.0);
                        }
                    }
                    None => {
                        glyphs::rest(&mut d, rx, it.dur.kind, yb, SP);
                        if it.dur.dotted {
                            glyphs::dot(&mut d, rx + 4.0, yb - 2.0 * SP - SP * 0.5, false, SP);
                        }
                        let ny = yb + 5.0 * SP + 22.0;
                        if answers {
                            chart::text(&mut d, rx, ny, fs * 0.85, "middle", "rest");
                        } else {
                            d.line(rx - 12.0, ny + 3.0, rx + 12.0, ny + 3.0, INK, 1.0);
                        }
                    }
                }
                state_beats += it.dur.quarters();
                x += bw;
            }
            x += 16.0;
            let last = si + 1 == systems.len() && k + 1 == sys.len();
            if last {
                d.line(x - 6.0, yb, x - 6.0, yb - 4.0 * SP, INK, 1.4);
                d.rect(x - 2.0, yb - 4.0 * SP, 3.2, 4.0 * SP, INK, INK, 0.5);
            } else {
                d.line(x - 4.0, yb, x - 4.0, yb - 4.0 * SP, INK, 1.4);
            }
            counts.push(format!("{}", crate::figures::geography::trim(state_beats * td as f64 / 4.0, 2)));
        }
    }
    // the beats in each bar
    let by = top + systems.len() as f64 * sys_h + 24.0;
    chart::text(&mut d, x0, by, fs, "start", "Beats in each bar:");
    let bx = x0 + 150.0;
    if answers {
        let wrong: Vec<String> = counts.iter().enumerate().filter(|(_, c)| c.parse::<f64>().map_or(true, |v| (v - tn as f64).abs() > 1e-9)).map(|(i, c)| format!("bar {} has {c}", i + 1)).collect();
        let tail = if wrong.is_empty() { format!("each bar fills {tn}/{td}") } else { format!("{}, not {tn}", wrong.join("; ")) };
        chart::text(&mut d, bx, by, fs, "start", &format!("{} ({})", counts.join(", "), tail));
    } else {
        d.line(bx, by + 3.0, bx + 380.0, by + 3.0, INK, 1.0);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn key_signatures_name_their_accidentals() {
        assert_eq!(key_count(KEYS[0]), 0);
        assert_eq!(key_count(KEYS[2]), 2);
        assert_eq!(key_count(KEYS[7]), 7);
        assert_eq!(key_count(KEYS[8]), -1);
        assert_eq!(key_count(KEYS[14]), -7);
        // D major: F and C are sharp
        assert_eq!((key_acc(2, 3), key_acc(2, 0), key_acc(2, 4)), (1, 1, 0));
        // B flat major: B and E are flat
        assert_eq!((key_acc(-2, 6), key_acc(-2, 2), key_acc(-2, 5)), (-1, -1, 0));
        assert_eq!(key_acc(0, 3), 0);
    }

    #[test]
    fn bars_are_cut_where_the_beats_fill() {
        let it = |d: &str| Item { pitch: None, dur: parse_dur(d).unwrap(), bar_after: false };
        let v = [it("q"), it("q"), it("h"), it("w"), it("q"), it("q")];
        assert_eq!(bars(&v, 4.0), vec![vec![0, 1, 2], vec![3], vec![4, 5]]);
        assert_eq!(bars(&v[..4], 4.0), vec![vec![0, 1, 2], vec![3]]);
        assert_eq!(bars(&[it("h.")], 3.0), vec![vec![0]]);
    }

    #[test]
    fn a_written_bar_line_ends_the_bar() {
        let v = parse("C4 q, D4 q | E4 h, F4 h").unwrap();
        assert_eq!(bars(&v, 4.0), vec![vec![0, 1], vec![2, 3]]);
        // a bar line right after a full bar does not make an empty bar
        let v = parse("C4 w | D4 w |").unwrap();
        assert_eq!(bars(&v, 4.0), vec![vec![0], vec![1]]);
        let v = parse("C4 q | | D4 q").unwrap();
        assert_eq!(bars(&v, 4.0), vec![vec![0], vec![1]]);
    }

    #[test]
    fn times_parse() {
        assert_eq!(parse_time("4/4"), Some((4, 4)));
        assert_eq!(parse_time(" 6 / 8 "), Some((6, 8)));
        for bad in ["", "4", "0/4", "17/4", "4/3", "a/b", "4/0"] {
            assert!(parse_time(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn key_writes_the_names_and_the_beats() {
        let svg = render(&defaults());
        for t in [">C<", ">D<", ">E<", ">B<", ">rest<", ">Beats in each bar:<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert!(svg.contains("4, 4, 4 (each bar fills 4/4)"));
    }

    #[test]
    fn short_bars_are_reported() {
        let mut p = defaults();
        p.set_s("notes", "C4 q, D4 q, E4 q | F4 h");
        let svg = render(&p);
        assert!(svg.contains("bar 1 has 3; bar 2 has 2, not 4"));
        p.set_s("time", "3/4");
        p.set_s("notes", "C4 q, D4 q, E4 q | F4 h.");
        assert!(render(&p).contains("3, 3 (each bar fills 3/4)"));
        p.set_s("time", "6/8");
        p.set_s("notes", "C4 e, D4 e, E4 e, F4 e, G4 e, A4 e");
        assert!(render(&p).contains("6 (each bar fills 6/8)"));
    }

    #[test]
    fn accidentals_follow_the_key_and_carry_through_the_bar() {
        let mut p = defaults();
        // C major: a sharp is shown, then not repeated for the same pitch in the bar, then shown again after the bar
        p.set_s("notes", "F#4 q, F#4 q, F#4 h | F#4 w");
        let (_, key) = (0, render(&p));
        let sharps = key.matches("stroke-width=\"2.4\"").count();
        assert_eq!(sharps, 4, "two slanted strokes in each of two sharp signs");
        // G major: F# is in the signature, so it needs no sign
        p.set_s("key", KEYS[1]);
        p.set_s("notes", "F#4 q, F#4 q, F#4 h");
        let one = render(&p);
        assert_eq!(one.matches("stroke-width=\"2.4\"").count(), 2, "only the key signature's sharp");
        // a natural cancels it
        p.set_s("notes", "F#4 q, F4 q, F4 h");
        assert!(render(&p).matches("stroke-width=\"2.2\"").count() == 2);
    }

    #[test]
    fn student_copy_leaves_blank_lines() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(!student.contains(">rest<") && !student.contains("fills 4/4") && student.contains(">Beats in each bar:<"));
        assert!(!student.contains(">E<") || key.matches(">E<").count() > student.matches(">E<").count());
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_input_is_explained() {
        for (field, spec, needle) in [
            ("notes", "", "Give some notes"),
            ("notes", "C4", "pitch and a length"),
            ("notes", "C4 x", "not a length"),
            ("notes", "H4 q", "not a pitch"),
            ("notes", "C4 q extra", "pitch and a length"),
            ("notes", "C9 q", "too far from the staff"),
            ("notes", "C4 q,".repeat(49).as_str(), "limited to 48"),
            ("time", "4/3", "time signature"),
            ("time", "x", "time signature"),
        ] {
            let mut p = defaults();
            p.set_s(field, spec);
            assert!(render(&p).contains(needle), "{field}: {spec:?}");
        }
        let mut p = defaults();
        p.set_s("notes", &"C4 q, ".repeat(300));
        assert!(render(&p).contains("limited to 1200"));
    }

    #[test]
    fn every_clef_and_key_renders() {
        for clef in CLEFS {
            for key in KEYS {
                let mut p = defaults();
                p.set_s("clef", clef);
                p.set_s("key", key);
                p.set_s("notes", match clef { "Bass" => "C3 q, G2 q, E4 h", "Alto" => "C4 q, G3 q, G4 h", _ => "C5 q, E4 q, G5 h" });
                assert!(sane(&render(&p)), "{clef} {key}");
            }
        }
    }

    #[test]
    fn long_pieces_wrap_into_systems() {
        let mut p = defaults();
        p.set_s("notes", &"C4 q, D4 q, E4 q, F4 q | ".repeat(10));
        let svg = render(&p);
        assert!(sane(&svg));
        let h: f64 = svg.split("height=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
        assert!(h > 400.0, "{h}");
    }

    #[test]
    fn print_style_is_grey() {
        for key in [KEYS[0], KEYS[3], KEYS[9]] {
            let mut p = defaults();
            p.set_s("style", "Print");
            p.set_s("key", key);
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("music-staff").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("is not a") && !svg.contains("too far") && !svg.contains("should be"), "{}", ex.name);
        }
        for spec in ["\u{0} q", "<b> q", "nan q", "C4 q q", "r r", "C4 \u{1f600}", "C4 q,,,,|||;;", &"C4 w, ".repeat(48)] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("notes", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
