//! A cause-and-effect web: causes arrow into one central event, and effects arrow out of it. The
//! causes and effects are listed mixed up under "Events to sort"; which side each belongs on is the
//! answer, so the student copy (Show answers off) leaves the boxes empty.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use super::{lines_of, shuffled, wrap};

const INK: &str = "#111827";
const MAX_EACH: usize = 5;
const DEFAULT: &str = "Heavy rain for a week\nSnow melting in the hills\nThe river banks were low";

pub fn def() -> FigureDef {
    FigureDef {
        id: "cause-effect",
        subject: Subject::Language,
        title: "Cause and effect web",
        blurb: "A web of causes arrowing into a central event and effects arrowing out of it, with the causes and effects sorted into place, or empty boxes and a mixed-up list for students to sort.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "cause-effect" => vec![
            Example { name: "A flood", params: vec![] },
            Example { name: "The fall of Rome", params: vec![("event", s("The western Roman Empire falls")), ("causes", s("Invasions on the borders\nA weak economy\nLeaders who kept changing")), ("effects", s("Europe splits into kingdoms\nTrade routes break up")) ]},
            Example { name: "One of each", params: vec![("causes", s("A dry summer")), ("effects", s("Crops fail"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("event", "Central event", "The river floods the town"),
        ParamDef::long("causes", "Causes", DEFAULT).hint("One per line, up to 5"),
        ParamDef::long("effects", "Effects", "Families leave their homes\nThe market is closed\nA new flood wall is built").hint("One per line, up to 5"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the central event and empty boxes with the mixed-up list, for students to sort"),
    ]
}

fn render(p: &Params) -> String {
    let (causes, effects) = (lines_of(p.s("causes"), 60), lines_of(p.s("effects"), 60));
    let event: String = p.s("event").trim().chars().take(80).collect();
    if causes.len() > MAX_EACH || effects.len() > MAX_EACH {
        return chart::message(&format!("There can be at most {MAX_EACH} causes and {MAX_EACH} effects"));
    }
    if causes.is_empty() && effects.is_empty() {
        return chart::message("Give some causes or effects");
    }
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let rows = causes.len().max(effects.len()).max(1);
    let rowh = 84.0;
    let area = rows as f64 * rowh + 20.0;
    let all: Vec<&String> = causes.iter().chain(effects.iter()).collect();
    let n = all.len();
    let order = shuffled(n);
    let mut number = vec![0usize; n];
    for (pos, &orig) in order.iter().enumerate() {
        number[orig] = pos + 1;
    }
    let list_rows = (n + 1) / 2;
    let (w, h) = (1000.0, top + 60.0 + area + 60.0 + list_rows as f64 * fs * 1.9 + 30.0);
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let y0 = top + 50.0;
    let mid = y0 + area / 2.0;
    let (bw, bh) = (280.0, 64.0);
    let (lx, rx) = (30.0, w - 30.0 - bw);
    chart::text(&mut d, lx + bw / 2.0, y0 - 14.0, fs * 1.1, "middle", "Causes");
    chart::text(&mut d, rx + bw / 2.0, y0 - 14.0, fs * 1.1, "middle", "Effects");
    // the centre
    let (cw, ch) = (230.0, 110.0);
    let (cx, cy) = (w / 2.0 - cw / 2.0, mid - ch / 2.0);
    let (cfill, lfill, rfill) = if print { ("#d6d6d6", "#f0f0f0", "#e4e4e4") } else { ("#fdeccf", "#dbe8fa", "#dff0e1") };
    d.rect(cx, cy, cw, ch, cfill, INK, 2.2);
    for (k, l) in wrap(&event, 24, 4).iter().enumerate() {
        let n = wrap(&event, 24, 4).len() as f64;
        chart::text(&mut d, w / 2.0, mid + fs * 0.35 - (n - 1.0) * fs * 0.65 + k as f64 * fs * 1.3, fs * 1.05, "middle", l);
    }
    let ys = |count: usize, i: usize| mid + (i as f64 - (count as f64 - 1.0) / 2.0) * rowh;
    for (side, count, x, fill) in [(0, causes.len(), lx, lfill), (1, effects.len(), rx, rfill)] {
        for i in 0..count {
            let y = ys(count, i);
            d.rect(x, y - bh / 2.0, bw, bh, fill, INK, 1.6);
            if side == 0 {
                d.arrow(x + bw + 4.0, y, cx - 6.0, mid + (y - mid) * 0.35, INK, 2.2);
            } else {
                d.arrow(cx + cw + 6.0, mid + (y - mid) * 0.35, x - 4.0, y, INK, 2.2);
            }
        }
    }
    if answers {
        for (side, v) in [&causes, &effects].iter().enumerate() {
            let off = if side == 0 { 0 } else { causes.len() };
            let x = if side == 0 { lx } else { rx };
            for (i, t) in v.iter().enumerate() {
                let label = format!("{}. {}", number[off + i], t);
                let lines = wrap(&label, 33, 3);
                let n = lines.len() as f64;
                for (k, l) in lines.iter().enumerate() {
                    chart::text(&mut d, x + bw / 2.0, ys(v.len(), i) + fs * 0.3 - (n - 1.0) * fs * 0.62 + k as f64 * fs * 1.25, fs * 0.9, "middle", l);
                }
            }
        }
    }
    let ly = y0 + area + 50.0;
    chart::text(&mut d, 30.0, ly - 12.0, fs * 1.05, "start", "Events to sort");
    for (pos, &orig) in order.iter().enumerate() {
        let (col, row) = (pos / list_rows, pos % list_rows);
        chart::text(&mut d, 30.0 + col as f64 * 480.0, ly + 10.0 + row as f64 * fs * 1.9, fs * 0.95, "start", &format!("{}. {}", pos + 1, wrap(all[orig], 52, 1)[0]));
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn key_places_numbered_events() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert_eq!(key.matches("Heavy rain for a week").count(), 2);
        assert_eq!(student.matches("Heavy rain for a week").count(), 1);
        assert!(student.contains("The river floods the") && key.contains(">Causes<") && student.contains(">Effects<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_input_is_explained() {
        let mut p = defaults();
        p.set_s("causes", &"x\n".repeat(6));
        assert!(render(&p).contains("at most 5"));
        p.set_s("causes", "");
        p.set_s("effects", "");
        assert!(render(&p).contains("Give some"));
        p.set_s("effects", "only effects");
        assert!(sane(&render(&p)));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("cause-effect").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("at most") && !svg.contains("Give some"), "{}", ex.name);
        }
        for spec in ["", "\u{0}", "<b>x</b>", "nan", &"w".repeat(5000), "a\n\n b "] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("event", spec);
                p.set_s("causes", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
