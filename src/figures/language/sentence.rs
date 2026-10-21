//! A sentence diagram (Reed-Kellogg style): the subject, the verb and an optional object on the
//! base line, with each modifier on a slanted line under the word it describes. The sentence and
//! its parts are the question; the student copy (Show answers off) shows the sentence and an empty
//! frame of the same shape (base line, dividers and slanted lines) with no words in it.
use crate::figures::chart;
use crate::figures::svg::{self, esc, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use super::lines_of;

const INK: &str = "#111827";
const MAX_MODS: usize = 3;

pub fn def() -> FigureDef {
    FigureDef {
        id: "sentence-diagram",
        subject: Subject::Language,
        title: "Sentence diagram",
        blurb: "A sentence diagram with subject, verb, optional object and slanted modifiers, or the sentence and an empty frame of the same shape for students to fill in.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "sentence-diagram" => vec![
            Example { name: "Subject, verb, object", params: vec![] },
            Example { name: "No object", params: vec![("sentence", s("Birds sing loudly.")), ("subject", s("Birds")), ("subject_mods", s("")), ("verb", s("sing")), ("verb_mods", s("loudly")), ("object", s("")), ("object_mods", s(""))] },
            Example { name: "Busy sentence", params: vec![("sentence", s("The two old sailors quickly mended the torn blue sail.")), ("subject", s("sailors")), ("subject_mods", s("The\ntwo\nold")), ("verb", s("mended")), ("verb_mods", s("quickly")), ("object", s("sail")), ("object_mods", s("the\ntorn\nblue"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("sentence", "The sentence", "The tall boy kicked the red ball."),
        ParamDef::text("subject", "Subject", "boy"),
        ParamDef::long("subject_mods", "Words describing the subject", "The\ntall").hint("One word per line, up to 3"),
        ParamDef::text("verb", "Verb", "kicked"),
        ParamDef::long("verb_mods", "Words describing the verb", "").hint("One word per line, up to 3"),
        ParamDef::text("object", "Object (optional)", "ball"),
        ParamDef::long("object_mods", "Words describing the object", "the\nred").hint("One word per line, up to 3"),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off shows the sentence and an empty frame of the same shape, with no words in it"),
    ]
}

/// Space a word needs on the base line, in pixels.
fn word_w(s: &str, fs: f64) -> f64 {
    s.chars().count() as f64 * fs * 0.62
}

const ANGLE: f64 = 62.0;

fn render(p: &Params) -> String {
    let word = |k: &str| -> String { p.s(k).split_whitespace().next().unwrap_or("").chars().take(18).collect() };
    let (subject, verb, object) = (word("subject"), word("verb"), word("object"));
    if subject.is_empty() || verb.is_empty() {
        return chart::message("Give a subject and a verb");
    }
    let sentence: String = p.s("sentence").trim().chars().take(140).collect();
    let mods = |k: &str| -> Vec<String> { lines_of(p.s(k), 18).into_iter().map(|l| l.split_whitespace().next().unwrap_or("").to_string()).collect() };
    let parts = [(subject, mods("subject_mods")), (verb, mods("verb_mods")), (object, mods("object_mods"))];
    if parts.iter().any(|(_, m)| m.len() > MAX_MODS) {
        return chart::message(&format!("At most {MAX_MODS} words can describe each part"));
    }
    if parts[2].0.is_empty() && !parts[2].1.is_empty() {
        return chart::message("The object needs a word before it can have describing words");
    }
    let answers = p.b(SHOW_ANSWERS);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let fsw = fs * 1.25;
    // each base-line segment is wide enough for its word and for its slanted lines side by side
    let (cos, sin) = (ANGLE.to_radians().cos(), ANGLE.to_radians().sin());
    let mlen = |md: &str| (md.chars().count() as f64 * fs * 0.6 + 16.0).max(56.0);
    // where each modifier meets the base line, measured from the start of its segment
    let attach: Vec<Vec<f64>> = parts.iter().map(|(_, m)| {
        let mut cur = 18.0;
        m.iter().map(|md| { let ax = cur + mlen(md) * cos; cur = ax + 16.0; ax }).collect()
    }).collect();
    let seg: Vec<f64> = parts.iter().zip(&attach).map(|((w, _), a)| (word_w(w, fsw) + 40.0).max(70.0).max(a.last().map_or(0.0, |x| x + 24.0))).collect();
    let has_obj = !parts[2].0.is_empty();
    let total = seg[0] + seg[1] + if has_obj { seg[2] } else { 0.0 };
    let (w, h) = ((total + 120.0).max(620.0), 310.0);
    let mut d = Doc::new(w, h);
    chart::text(&mut d, w / 2.0, 34.0, fs * 1.1, "middle", &sentence);
    let base = 150.0;
    let x0 = (w - total) / 2.0;
    d.line(x0, base, x0 + total, base, INK, 2.4);
    let x1 = x0 + seg[0];
    d.line(x1, base - 28.0, x1, base + 28.0, INK, 2.4);
    let x2 = x1 + seg[1];
    if has_obj {
        d.line(x2, base - 28.0, x2, base, INK, 2.4);
    }
    let starts = [x0, x1, x2];
    for (i, (wd, m)) in parts.iter().enumerate() {
        if i == 2 && !has_obj {
            break;
        }
        if answers {
            chart::text(&mut d, starts[i] + seg[i] / 2.0, base - 9.0, fsw, "middle", wd);
        }
        for (k, md) in m.iter().enumerate() {
            let ax = starts[i] + attach[i][k];
            let len = mlen(md);
            let (bx, by) = (ax - len * cos, base + len * sin);
            d.line(ax, base, bx, by, INK, 2.0);
            if answers {
                let (mx, my) = ((ax + bx) / 2.0 - 7.0, (base + by) / 2.0 - 5.0);
                d.raw(&format!(
                    r##"<text x="{mx:.2}" y="{my:.2}" font-size="{}" text-anchor="middle" transform="rotate(-{ANGLE} {mx:.2} {my:.2})" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="#000">{}</text>"##,
                    fs * 0.95,
                    esc(md)
                ));
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
    fn key_places_every_word() {
        let svg = render(&defaults());
        for t in [">boy<", ">kicked<", ">ball<", ">The<", ">tall<", ">red<"] {
            assert!(svg.contains(t), "{t}");
        }
        assert!(svg.contains("The tall boy kicked the red ball."));
        assert!(svg.contains("rotate(-62"));
    }

    #[test]
    fn student_frame_has_no_words_but_the_same_shape() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains("The tall boy kicked the red ball.") && !student.contains(">boy<") && !student.contains(">tall<"));
        assert_eq!(student.matches("<line").count(), key.matches("<line").count());
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn no_object_and_bad_input() {
        let mut p = defaults();
        p.set_s("object", "");
        p.set_s("object_mods", "");
        let svg = render(&p);
        assert!(sane(&svg) && !svg.contains(">ball<"));
        p.set_s("object_mods", "the");
        assert!(render(&p).contains("object needs a word"));
        p.set_s("object_mods", "");
        p.set_s("subject_mods", "a\nb\nc\nd");
        assert!(render(&p).contains("At most 3"));
        p.set_s("subject_mods", "");
        p.set_s("subject", "  ");
        assert!(render(&p).contains("Give a subject"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("sentence-diagram").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("At most") && !svg.contains("Give a"), "{}", ex.name);
        }
        for spec in ["\u{0}", "<b>x</b>", "nan", &"w".repeat(500), "a b c"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("sentence", spec);
                p.set_s("subject", spec);
                p.set_s("verb_mods", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
