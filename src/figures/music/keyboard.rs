//! A piano keyboard with keys marked: the notes of a scale or chord worked out from a root, or
//! keys typed in by hand. Two worksheets from one drawing: Mark the keys gives the name of a scale
//! or chord and leaves the keys for students to mark; Name the keys marks the keys and leaves
//! their names as blank lines. Show answers off leaves out whichever half is the answer.
use super::glyphs::INK;
use super::parse_pitch;
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MODES: [&str; 6] = ["Custom keys", "Major scale", "Natural minor scale", "Major triad", "Minor triad", "Dominant 7th chord"];
const ROOTS: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const TASKS: [&str; 2] = ["Mark the keys", "Name the keys"];
const SHARPS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const WHITE: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const MAX_CUSTOM: usize = 12;
const KW: f64 = 42.0;
const KH: f64 = 170.0;

pub fn def() -> FigureDef {
    FigureDef {
        id: "piano-keyboard",
        subject: Subject::Music,
        title: "Piano keyboard",
        blurb: "A piano keyboard with the keys of a scale or chord marked, for naming the keys, or with the scale or chord named and the keys left for students to mark.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("piano-keyboard", "root") => p.s("mode") != MODES[0],
        ("piano-keyboard", "keys") => p.s("mode") == MODES[0],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "piano-keyboard" => vec![
            Example { name: "C major scale", params: vec![("mode", s(MODES[1]))] },
            Example { name: "A natural minor scale", params: vec![("mode", s(MODES[2])), ("root", s("A"))] },
            Example { name: "G major chord", params: vec![("mode", s(MODES[3])), ("root", s("G")), ("octaves", Value::I(2))] },
            Example { name: "Eb minor chord", params: vec![("mode", s(MODES[4])), ("root", s("Eb"))] },
            Example { name: "Name the keys", params: vec![("task", s(TASKS[1]))] },
            Example { name: "Mark the keys (blank)", params: vec![("mode", s(MODES[5])), ("root", s("D")), ("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("task", "Worksheet", &TASKS, TASKS[1]),
        ParamDef::choice("mode", "Keys to mark", &MODES, MODES[0]),
        ParamDef::choice("root", "Root note", &ROOTS, ROOTS[0]),
        ParamDef::text("keys", "Custom keys", "C4 E4 G4 C5").hint("Pitches such as C4 F#4 Bb4, middle C is C4; up to 12"),
        ParamDef::int("octaves", "Octaves", 1, 4, 2),
        ParamDef::int("start", "Octave of the lowest C", 1, 6, 4),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves out the half that is the answer: the names (Name the keys) or the marked keys (Mark the keys)"),
    ]
}

fn intervals(mode: &str) -> &'static [i32] {
    match mode {
        "Major scale" => &[0, 2, 4, 5, 7, 9, 11, 12],
        "Natural minor scale" => &[0, 2, 3, 5, 7, 8, 10, 12],
        "Major triad" => &[0, 4, 7],
        "Minor triad" => &[0, 3, 7],
        "Dominant 7th chord" => &[0, 4, 7, 10],
        _ => &[],
    }
}

/// How many letter names above the root a note `semitones` above it is spelled (a third is
/// always two letters up, a fifth four, and so on).
fn letter_steps(semitones: i32) -> i32 {
    match semitones { 0 => 0, 1 | 2 => 1, 3 | 4 => 2, 5 | 6 => 3, 7 => 4, 8 | 9 => 5, 10 | 11 => 6, _ => 7 }
}

/// The name of the note `semitones` above `root` (a name from [`ROOTS`]), spelled by scale degree so
/// that a minor seventh above C is B-flat and a major third above D is F-sharp.
pub fn spell(root: &str, semitones: i32) -> String {
    let rl = root.chars().next().map_or(0, |c| "CDEFGAB".find(c).unwrap_or(0) as i32);
    let rpc = WHITE[rl as usize] + if root.contains('#') { 1 } else if root.contains('b') { -1 } else { 0 };
    let letter = (rl + letter_steps(semitones)).rem_euclid(7);
    let pc = (rpc + semitones).rem_euclid(12);
    let acc = (pc - WHITE[letter as usize] + 18).rem_euclid(12) - 6;
    match acc {
        0 => format!("{}", "CDEFGAB".chars().nth(letter as usize).unwrap()),
        1 => format!("{}#", "CDEFGAB".chars().nth(letter as usize).unwrap()),
        -1 => format!("{}b", "CDEFGAB".chars().nth(letter as usize).unwrap()),
        // a double sharp or flat: use the plain name of the same key instead
        _ => SHARPS[pc as usize].to_string(),
    }
}

/// The marked keys as (midi number, name), lowest first.
pub fn marked(p: &Params, lo: i32, hi: i32) -> Result<Vec<(i32, String)>, String> {
    let mode = p.s("mode");
    let mut v: Vec<(i32, String)> = Vec::new();
    if mode == MODES[0] {
        for tok in p.s("keys").split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|t| !t.is_empty()) {
            let pt = parse_pitch(tok).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a pitch, such as C4 or F#4", tok.chars().take(12).collect::<String>()))?;
            if !(lo..=hi).contains(&pt.midi()) {
                return Err(format!("{}{} is off the keyboard: change the octaves or the starting octave", pt.name(), pt.octave));
            }
            v.push((pt.midi(), pt.name()));
            if v.len() > MAX_CUSTOM {
                return Err(format!("A figure is limited to {MAX_CUSTOM} marked keys"));
            }
        }
        if v.is_empty() {
            return Err("Give some keys to mark, such as C4 E4 G4".into());
        }
    } else {
        let root = ROOTS.iter().position(|r| *r == p.s("root")).unwrap_or(0);
        let rname = ROOTS[root];
        let rootmidi = lo + [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][root];
        for i in intervals(mode) {
            let m = rootmidi + i;
            if m > hi {
                return Err(format!("{rname} {} does not fit on {} octave{}: add an octave or choose a lower root", mode.to_lowercase(), (hi - lo) / 12, if hi - lo == 12 { "" } else { "s" }));
            }
            v.push((m, spell(rname, *i)));
        }
    }
    v.sort_by_key(|(m, _)| *m);
    v.dedup_by_key(|(m, _)| *m);
    Ok(v)
}

/// The x of a key's centre, counted in white keys from the left edge of the keyboard.
pub fn key_x(midi: i32, lo: i32) -> f64 {
    let rel = midi - lo;
    let (oct, pc) = (rel.div_euclid(12), rel.rem_euclid(12));
    match WHITE.iter().position(|w| *w == pc) {
        Some(i) => (oct * 7 + i as i32) as f64 + 0.5,
        None => {
            // a black key sits on the boundary after the white key below it
            let below = WHITE.iter().rposition(|w| *w < pc).unwrap();
            (oct * 7 + below as i32) as f64 + 1.0
        }
    }
}

pub fn is_black(midi: i32) -> bool {
    !WHITE.contains(&midi.rem_euclid(12))
}

fn heading(p: &Params, keys: &[(i32, String)]) -> String {
    let mode = p.s("mode");
    if mode == MODES[0] {
        if p.s("task") == TASKS[0] {
            format!("Mark: {}", keys.iter().map(|(_, n)| n.clone()).collect::<Vec<_>>().join(" "))
        } else {
            "Name the marked keys".to_string()
        }
    } else {
        let root = p.s("root");
        let kind = mode.to_lowercase();
        if p.s("task") == TASKS[0] { format!("Mark the keys of the {root} {kind}") } else { format!("Name the keys of the {root} {kind}") }
    }
}

fn render(p: &Params) -> String {
    let octaves = p.i("octaves").clamp(1, 4) as i32;
    let start = p.i("start").clamp(1, 6) as i32;
    let lo = 12 * (start + 1);
    let hi = lo + 12 * octaves;
    let keys = match marked(p, lo, hi) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let answers = p.b(SHOW_ANSWERS);
    let mark_task = p.s("task") == TASKS[0];
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let whites = (octaves * 7 + 1) as f64;
    let (w, x0) = (whites * KW + 60.0, 30.0);
    let h = top + 40.0 + KH + 100.0;
    let mut d = Doc::new(w.max(420.0), h);
    if !title.is_empty() {
        chart::text(&mut d, w.max(420.0) / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    chart::text(&mut d, x0, top + 26.0, fs * 1.15, "start", &heading(p, &keys));
    let y0 = top + 40.0;
    // white keys, then black keys on top
    for k in 0..(octaves * 7 + 1) {
        d.rect(x0 + k as f64 * KW, y0, KW, KH, "#fff", INK, 1.4);
        if k % 7 == 0 {
            chart::text(&mut d, x0 + (k as f64 + 0.5) * KW, y0 + KH - 6.0, fs * 0.8, "middle", &format!("C{}", start + k / 7));
        }
    }
    for m in lo..=hi {
        if is_black(m) {
            let x = x0 + key_x(m, lo) * KW;
            d.rect(x - 13.0, y0, 26.0, KH * 0.62, if print { "#444" } else { "#1f2937" }, INK, 1.2);
        }
    }
    // the dots
    let show_dots = answers || !mark_task;
    let dot_fill = if print { "#444444" } else { "#e0662a" };
    if show_dots {
        for (m, _) in &keys {
            let x = x0 + key_x(*m, lo) * KW;
            if is_black(*m) {
                d.circle(x, y0 + KH * 0.62 - 16.0, 8.0, if print { "#e5e5e5" } else { dot_fill }, "#fff", 1.6);
            } else {
                d.circle(x, y0 + KH - 30.0, 11.0, dot_fill, INK, 1.4);
            }
        }
    }
    // the names, on two rows so that neighbouring marked keys do not collide
    let show_names = answers;
    for (m, name) in &keys {
        let x = x0 + key_x(*m, lo) * KW;
        let row = if is_black(*m) { 1.0 } else { 0.0 };
        let y = y0 + KH + 24.0 + row * 26.0;
        if mark_task && !answers {
            continue;
        }
        if show_names {
            chart::text(&mut d, x, y, fs, "middle", name);
        } else {
            d.line(x - 12.0, y + 3.0, x + 12.0, y + 3.0, INK, 1.0);
        }
    }
    if mark_task && !answers {
        chart::text(&mut d, x0, y0 + KH + 34.0, fs * 0.9, "start", "Mark the keys, then write each name under its key.");
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }
    fn with(mode: &str, root: &str) -> Params {
        let mut p = defaults();
        p.set_s("mode", mode);
        p.set_s("root", root);
        p
    }

    #[test]
    fn scales_and_chords_are_worked_out() {
        let names = |p: &Params| marked(p, 60, 84).unwrap().into_iter().map(|(_, n)| n).collect::<Vec<_>>().join(" ");
        assert_eq!(names(&with(MODES[1], "C")), "C D E F G A B C");
        assert_eq!(names(&with(MODES[1], "G")), "G A B C D E F# G");
        assert_eq!(names(&with(MODES[1], "F")), "F G A Bb C D E F");
        assert_eq!(names(&with(MODES[2], "A")), "A B C D E F G A");
        assert_eq!(names(&with(MODES[2], "D")), "D E F G A Bb C D");
        assert_eq!(names(&with(MODES[3], "G")), "G B D");
        assert_eq!(names(&with(MODES[4], "Eb")), "Eb Gb Bb");
        assert_eq!(names(&with(MODES[5], "C")), "C E G Bb");
        assert_eq!(names(&with(MODES[3], "F#")), "F# A# C#");
        assert_eq!(names(&with(MODES[5], "D")), "D F# A C");
        assert_eq!(names(&with(MODES[5], "Bb")), "Bb D F Ab");
        assert_eq!(names(&with(MODES[1], "Eb")), "Eb F G Ab Bb C D Eb");
        assert_eq!(names(&with(MODES[2], "Bb")), "Bb C Db Eb F Gb Ab Bb");
    }

    #[test]
    fn spelling_goes_by_scale_degree() {
        assert_eq!(spell("C", 10), "Bb");
        assert_eq!(spell("D", 4), "F#");
        assert_eq!(spell("F", 4), "A");
        assert_eq!(spell("Ab", 7), "Eb");
        assert_eq!(spell("B", 7), "F#");
        assert_eq!(spell("E", 11), "D#");
    }

    #[test]
    fn keys_sit_where_they_belong() {
        assert_eq!(key_x(60, 60), 0.5);
        assert_eq!(key_x(62, 60), 1.5);
        assert_eq!(key_x(61, 60), 1.0, "C# is between C and D");
        assert_eq!(key_x(64, 60), 2.5);
        assert_eq!(key_x(65, 60), 3.5);
        assert_eq!(key_x(66, 60), 4.0, "F# is between F and G");
        assert_eq!(key_x(71, 60), 6.5);
        assert_eq!(key_x(72, 60), 7.5);
        assert_eq!(key_x(70, 60), 6.0, "A# is between A and B");
        assert!(is_black(61) && is_black(70) && !is_black(60) && !is_black(71));
    }

    #[test]
    fn custom_keys_must_fit() {
        let mut p = defaults();
        p.set_s("keys", "C4 E4 G4 C5");
        assert_eq!(marked(&p, 60, 84).unwrap().len(), 4);
        p.set_s("keys", "C3");
        assert!(marked(&p, 60, 84).unwrap_err().contains("off the keyboard"));
        p.set_s("keys", "C4 H4");
        assert!(marked(&p, 60, 84).unwrap_err().contains("not a pitch"));
        p.set_s("keys", "");
        assert!(marked(&p, 60, 84).unwrap_err().contains("Give some keys"));
        p.set_s("keys", "C4 D4 E4 F4 G4 A4 B4 C5 D5 E5 F5 G5 A5");
        assert!(marked(&p, 60, 96).unwrap_err().contains("limited to 12"));
    }

    #[test]
    fn a_scale_that_does_not_fit_is_explained() {
        let mut p = with(MODES[1], "G");
        p.set_i("octaves", 1);
        assert!(render(&p).contains("does not fit"));
    }

    #[test]
    fn mark_worksheet_hides_the_dots_and_name_worksheet_hides_the_names() {
        let mut p = with(MODES[1], "C");
        p.set_s("task", TASKS[0]);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(student.contains("Mark the keys of the C major scale") && !student.contains("fill=\"#e0662a\"") && key.contains("fill=\"#e0662a\""));
        assert!(key.contains(">F<") && !student.contains(">F<"));
        assert_eq!(size_of(&student), size_of(&key));
        let mut p = with(MODES[3], "C");
        p.set_s("task", TASKS[1]);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        assert!(student.contains("fill=\"#e0662a\"") && !student.contains(">G<") && key.contains(">G<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn print_style_is_grey() {
        for mode in MODES {
            let mut p = with(mode, "A");
            p.set_s("style", "Print");
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("piano-keyboard").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("does not fit") && !svg.contains("off the keyboard"), "{}", ex.name);
        }
        for keys in ["\u{0}", "<b>", "nan", "C4,,,", "C4 C4 C4", "A0 C8 C9", &"C4 ".repeat(200)] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("keys", keys);
                p.set_i("octaves", 99);
                p.set_i("start", -3);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{keys:?}");
            }
        }
    }
}
