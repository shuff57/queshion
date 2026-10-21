//! The circle of fifths: twelve sectors with the major keys on the outside, the relative minors
//! inside them and the key signatures on the inside ring. Choosing a key shades it with its
//! neighbours (IV and V) and its relative minor (vi) and writes them out in the centre. The
//! circle is the question; what Hide names is the answer, so the student copy (Show answers off)
//! leaves the chosen ring and the centre workings empty.
use super::glyphs::INK;
use crate::figures::chart;
use crate::figures::earth::polar;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAJORS: [&str; 12] = ["C", "G", "D", "A", "E", "B", "F#/Gb", "Db", "Ab", "Eb", "Bb", "F"];
const MINORS: [&str; 12] = ["Am", "Em", "Bm", "F#m", "C#m", "G#m", "D#m/Ebm", "Bbm", "Fm", "Cm", "Gm", "Dm"];
const SIGS: [&str; 12] = ["0", "1#", "2#", "3#", "4#", "5#", "6#/6b", "5b", "4b", "3b", "2b", "1b"];
const HIDE: [&str; 3] = ["Key names", "Key signatures", "Both"];
const NONE: &str = "None";

pub fn def() -> FigureDef {
    FigureDef {
        id: "circle-of-fifths",
        subject: Subject::Music,
        title: "Circle of fifths",
        blurb: "The circle of fifths with major keys, relative minors and key signatures, a chosen key shaded with its neighbours, or with the names and signatures left blank for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("circle-of-fifths", "hide") => !p.b(SHOW_ANSWERS),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "circle-of-fifths" => vec![
            Example { name: "The whole circle", params: vec![] },
            Example { name: "Key of D major", params: vec![("key", s("D"))] },
            Example { name: "Key of Eb major", params: vec![("key", s("Eb"))] },
            Example { name: "Blank names", params: vec![("show_answers", Value::B(false)), ("hide", s(HIDE[0]))] },
            Example { name: "Blank signatures for key of G", params: vec![("show_answers", Value::B(false)), ("hide", s(HIDE[1])), ("key", s("G"))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    let mut keys = vec![NONE];
    keys.extend(MAJORS.iter().copied().filter(|k| !k.contains('/')));
    keys.push("F#");
    vec![
        ParamDef::choice("key", "Shade the neighbours of", &keys, NONE).hint("Shades a major key, its IV and V and its relative minor, and writes them out in the middle"),
        ParamDef::choice("hide", "Leave blank for students", &HIDE, HIDE[2]).hint("Used when Show answers is off"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves the chosen names or signatures blank, and the I IV V vi workings when a key is shaded"),
    ]
}

/// The sector a major key name sits in (F# and Gb share one).
pub fn index_of(key: &str) -> Option<usize> {
    MAJORS.iter().position(|m| *m == key || m.split('/').any(|n| n == key))
}

/// One annular sector between radii `r0` and `r1` and angles `a0`..`a1` (degrees, anticlockwise from east).
fn sector(cx: f64, cy: f64, r0: f64, r1: f64, a0: f64, a1: f64) -> String {
    let (p0, p1) = (polar(cx, cy, r1, a0), polar(cx, cy, r1, a1));
    let (q1, q0) = (polar(cx, cy, r0, a1), polar(cx, cy, r0, a0));
    format!("M{:.2},{:.2} A{r1},{r1} 0 0 0 {:.2},{:.2} L{:.2},{:.2} A{r0},{r0} 0 0 1 {:.2},{:.2}Z", p0.0, p0.1, p1.0, p1.1, q1.0, q1.1, q0.0, q0.1)
}

fn render(p: &Params) -> String {
    let answers = p.b(SHOW_ANSWERS);
    let hide = p.s("hide");
    let hide_names = !answers && (hide == HIDE[0] || hide == HIDE[2]);
    let hide_sigs = !answers && (hide == HIDE[1] || hide == HIDE[2]);
    let chosen = index_of(p.s("key"));
    let fills = Fills::new(p);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (w, h) = (700.0, 700.0 + top);
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (cx, cy) = (w / 2.0, top + 350.0);
    let rings = [(255.0, 330.0), (180.0, 255.0), (120.0, 180.0)];
    let shade = |i: usize, ring: usize| -> Option<String> {
        let c = chosen?;
        let (maj, min) = (i == c || i == (c + 1) % 12 || i == (c + 11) % 12, i == c);
        let on = if ring == 0 { maj } else if ring == 1 { min } else { false };
        on.then(|| match (fills.is_print(), i == c) {
            (true, true) => "#bdbdbd".to_string(),
            (true, false) => "#e4e4e4".to_string(),
            (false, true) => "#9ec5f4".to_string(),
            (false, false) => "#cfe3fa".to_string(),
        })
    };
    for i in 0..12 {
        let (a0, a1) = (90.0 + 15.0 - 30.0 * i as f64 - 30.0, 90.0 + 15.0 - 30.0 * i as f64);
        for (ring, (r0, r1)) in rings.iter().enumerate() {
            let fill = shade(i, ring).unwrap_or_else(|| "#fff".to_string());
            d.path(&sector(cx, cy, *r0, *r1, a0, a1), &fill, INK, 1.4);
        }
        let mid = 90.0 - 30.0 * i as f64;
        if !hide_names {
            let (x, y) = polar(cx, cy, 292.0, mid);
            chart::text(&mut d, x, y + fs * 0.4, fs * 1.35, "middle", MAJORS[i]);
            let (x, y) = polar(cx, cy, 217.0, mid);
            chart::text(&mut d, x, y + fs * 0.4, fs * 1.05, "middle", MINORS[i]);
        }
        if !hide_sigs {
            let (x, y) = polar(cx, cy, 150.0, mid);
            chart::text(&mut d, x, y + fs * 0.4, fs, "middle", SIGS[i]);
        }
    }
    // the centre
    d.circle(cx, cy, 120.0, "#fff", INK, 1.4);
    match chosen {
        None => {
            chart::text(&mut d, cx, cy - 6.0, fs * 1.2, "middle", "Circle of fifths");
            chart::text(&mut d, cx, cy + 16.0, fs * 0.85, "middle", "up a fifth each step clockwise");
        }
        Some(c) => {
            let key = MAJORS[c].split('/').next().unwrap();
            chart::text(&mut d, cx, cy - 66.0, fs * 1.15, "middle", &format!("Key of {key} major"));
            let rows = [("I", key.to_string()), ("IV", MAJORS[(c + 11) % 12].split('/').next().unwrap().to_string()), ("V", MAJORS[(c + 1) % 12].split('/').next().unwrap().to_string()), ("vi", MINORS[c].split('/').next().unwrap().to_string())];
            for (k, (num, name)) in rows.iter().enumerate() {
                let y = cy - 30.0 + k as f64 * 28.0;
                chart::text(&mut d, cx - 46.0, y, fs * 1.05, "end", &format!("{num} ="));
                if answers {
                    chart::text(&mut d, cx - 36.0, y, fs * 1.05, "start", name);
                } else {
                    d.line(cx - 36.0, y + 3.0, cx + 44.0, y + 3.0, INK, 1.0);
                }
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
    fn keys_sit_in_their_sectors() {
        assert_eq!(index_of("C"), Some(0));
        assert_eq!(index_of("D"), Some(2));
        assert_eq!(index_of("F#"), Some(6));
        assert_eq!(index_of("Gb"), Some(6));
        assert_eq!(index_of("Eb"), Some(9));
        assert_eq!(index_of("H"), None);
        // a fifth up is the next sector: G is a fifth above C, and the sectors line up with the signatures
        for i in 0..12 {
            assert_eq!(MAJORS.len(), MINORS.len());
            assert_eq!(SIGS.len(), 12);
            assert!(MAJORS[i].len() <= 5);
        }
    }

    #[test]
    fn the_whole_circle_names_every_key() {
        let svg = render(&defaults());
        for k in MAJORS.iter().chain(MINORS.iter()).chain(SIGS.iter()) {
            assert!(svg.contains(&format!(">{k}<")), "{k}");
        }
    }

    #[test]
    fn a_chosen_key_gets_its_neighbours() {
        let mut p = defaults();
        p.set_s("key", "D");
        let svg = render(&p);
        assert!(svg.contains("Key of D major") && svg.contains(">I =<") && svg.contains(">vi =<"));
        for (name, _) in [("D", 0), ("G", 1), ("A", 2), ("Bm", 3)] {
            assert!(svg.contains(&format!(">{name}<")));
        }
        let mut p = defaults();
        p.set_s("key", "C");
        assert!(render(&p).contains(">F<"), "the IV of C is F, across the top of the circle");
    }

    #[test]
    fn student_copy_blanks_what_was_chosen() {
        for (hide, names, sigs) in [(HIDE[0], false, true), (HIDE[1], true, false), (HIDE[2], false, false)] {
            let mut p = defaults();
            p.set_s("key", "G");
            p.set_s("hide", hide);
            let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            assert_eq!(student.contains(">Em<"), names, "{hide}");
            assert_eq!(student.contains(">2#<"), sigs, "{hide}");
            assert!(key.contains(">Em<") && key.contains(">2#<"));
            assert_eq!(size_of(&student), size_of(&key));
        }
    }

    #[test]
    fn print_style_is_grey() {
        for key in [NONE, "A"] {
            let mut p = defaults();
            p.set_s("style", "Print");
            p.set_s("key", key);
            assert_grey(&render(&p));
        }
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("circle-of-fifths").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for key in ["", "nonsense", "<b>", "F#", "Gb", "\u{0}"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("key", key);
                p.set_s("title", "<script>");
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<script>"), "{key:?}");
            }
        }
    }
}
