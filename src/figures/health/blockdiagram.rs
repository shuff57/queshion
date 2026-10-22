//! A systems block diagram: blocks in a chain with a labelled signal on each arrow, and an
//! optional feedback path back to a summing junction. The description of the system is the
//! question; the names of the blocks and the signals are the answers, so the student copy (Show
//! answers off) leaves the boxes and the labels blank.
use crate::figures::chart;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use crate::figures::language::{lines_of, wrap};

const INK: &str = "#111827";
const MAX_BLOCKS: usize = 6;

pub fn def() -> FigureDef {
    FigureDef {
        id: "block-diagram",
        subject: Subject::Health,
        title: "Block diagram",
        blurb: "A systems block diagram (input, controller, actuator, process, output) with labelled signals and an optional feedback loop, or empty boxes and labels for students to fill in.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "block-diagram" => vec![
            Example { name: "Thermostat", params: vec![] },
            Example { name: "Open loop: a toaster", params: vec![("description", s("A toaster heats bread for a time set by a timer.")), ("blocks", s("Timer\nSwitch\nHeating element")), ("signals", s("Time setting\nOn/off signal\nHeat\nToast")), ("feedback", s(""))] },
            Example { name: "Cruise control", params: vec![("description", s("A car holds a steady speed set by the driver.")), ("blocks", s("Controller\nThrottle\nEngine and car")), ("signals", s("Target speed\nControl signal\nFuel flow\nActual speed")), ("feedback", s("Speed sensor reading"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("description", "What the system does", "A room is kept at a set temperature."),
        ParamDef::long("blocks", "Blocks, in order", "Thermostat\nBoiler\nRadiator").hint("One block per line, left to right; 1 to 6"),
        ParamDef::long("signals", "Signals: one for each arrow, input first and output last", "Set temperature\nOn/off signal\nHot water\nRoom heat").hint("One per line; there is one more signal than there are blocks"),
        ParamDef::text("feedback", "Feedback signal (empty for none)", "Room temperature").hint("A sensor reading fed back to the start, through a summing junction"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the description and the arrows and leaves the block names and signal labels blank for students"),
    ]
}

fn render(p: &Params) -> String {
    let blocks = lines_of(p.s("blocks"), 40);
    if blocks.is_empty() {
        return chart::message("Give at least one block");
    }
    if blocks.len() > MAX_BLOCKS {
        return chart::message(&format!("A diagram is limited to {MAX_BLOCKS} blocks"));
    }
    let signals = lines_of(p.s("signals"), 40);
    if signals.len() > blocks.len() + 1 {
        return chart::message("There can be at most one more signal than there are blocks");
    }
    let n = blocks.len();
    let feedback: String = p.s("feedback").trim().chars().take(40).collect();
    let has_fb = !feedback.is_empty();
    let description: String = p.s("description").trim().chars().take(140).collect();
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let (bw, bh, gap) = (140.0, 76.0, 100.0);
    let lead = if has_fb { 190.0 } else { 90.0 };
    let w = lead + n as f64 * (bw + gap) + gap + 30.0;
    let w = w.max(640.0);
    let h = top + 130.0 + bh / 2.0 + if has_fb { 150.0 } else { 70.0 };
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    chart::text(&mut d, 30.0, top + 34.0, fs * 1.05, "start", &description);
    let cy = top + 130.0;
    let fill = if print { "#ececec" } else { "#f0e9db" };
    // x of the left edge of block i
    let bx = |i: usize| lead + gap + i as f64 * (bw + gap) - if has_fb { 0.0 } else { 0.0 };
    let signal_label = |d: &mut Doc, x: f64, y: f64, i: usize| {
        let lines = signals.get(i).map(|s| wrap(s, 14, 2)).unwrap_or_default();
        if answers {
            for (k, l) in lines.iter().enumerate() {
                chart::text(d, x, y - 10.0 - (lines.len() - 1 - k) as f64 * fs * 1.2, fs * 0.9, "middle", l);
            }
        } else if signals.get(i).is_some() || true {
            d.line(x - 38.0, y - 6.0, x + 38.0, y - 6.0, INK, 1.0);
        }
    };
    // the arrows: input into the first block (or the junction), between blocks, and out of the last
    let first_in = if has_fb { lead - 10.0 } else { lead - 20.0 };
    if has_fb {
        d.arrow(24.0, cy, first_in - 18.0, cy, INK, 2.2);
        d.circle(first_in, cy, 18.0, "#fff", INK, 2.0);
        d.line(first_in - 9.0, cy - 9.0, first_in + 9.0, cy + 9.0, INK, 1.6);
        d.line(first_in - 9.0, cy + 9.0, first_in + 9.0, cy - 9.0, INK, 1.6);
        d.arrow(first_in + 18.0, cy, bx(0) - 2.0, cy, INK, 2.2);
        signal_label(&mut d, (24.0 + first_in - 18.0) / 2.0 + 4.0, cy, 0);
    } else {
        d.arrow(24.0, cy, bx(0) - 2.0, cy, INK, 2.2);
        signal_label(&mut d, (24.0 + bx(0)) / 2.0, cy, 0);
    }
    for i in 0..n {
        let x = bx(i);
        d.rect(x, cy - bh / 2.0, bw, bh, fill, INK, 2.0);
        if answers {
            let lines = wrap(&blocks[i], 16, 3);
            for (k, l) in lines.iter().enumerate() {
                chart::text(&mut d, x + bw / 2.0, cy + fs * 0.35 - (lines.len() as f64 - 1.0) * fs * 0.62 + k as f64 * fs * 1.25, fs, "middle", l);
            }
        }
        let end = if i + 1 < n { bx(i + 1) - 2.0 } else { x + bw + gap - 10.0 };
        d.arrow(x + bw, cy, end, cy, INK, 2.2);
        signal_label(&mut d, x + bw + gap / 2.0 - if i + 1 == n { 5.0 } else { 0.0 }, cy, i + 1);
    }
    if has_fb {
        let xo = bx(n - 1) + bw + gap * 0.5;
        let yb = cy + bh / 2.0 + 70.0;
        d.line(xo, cy, xo, yb, INK, 2.2);
        d.line(xo, yb, first_in, yb, INK, 2.2);
        d.arrow(first_in, yb, first_in, cy + 20.0, INK, 2.2);
        d.circle(xo, cy, 4.0, INK, INK, 1.0);
        let mx = (xo + first_in) / 2.0;
        if answers {
            chart::text(&mut d, mx, yb + fs * 1.5, fs * 0.95, "middle", &feedback);
        } else {
            d.line(mx - 70.0, yb + fs * 1.5 + 3.0, mx + 70.0, yb + fs * 1.5 + 3.0, INK, 1.0);
        }
        chart::text(&mut d, first_in + 8.0, cy + 40.0, fs * 1.3, "start", "\u{2212}");
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn key_names_blocks_and_signals() {
        let svg = render(&defaults());
        for t in [">Thermostat<", ">Boiler<", ">Radiator<", ">temperature<", ">Room temperature<", "A room is kept"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn student_copy_blanks_names_but_not_the_description() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains("A room is kept") && !student.contains(">Boiler<") && !student.contains("Room temperature") && key.contains(">Boiler<"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn open_loop_has_no_feedback_path() {
        let mut p = defaults();
        p.set_s("feedback", "");
        let svg = render(&p);
        assert!(sane(&svg) && !svg.contains("Room temperature") && !svg.contains("\u{2212}"));
    }

    #[test]
    fn bad_input_is_explained() {
        let mut p = defaults();
        p.set_s("blocks", "");
        assert!(render(&p).contains("at least one block"));
        p.set_s("blocks", &"b\n".repeat(7));
        assert!(render(&p).contains("limited to 6"));
        p.set_s("blocks", "a\nb");
        p.set_s("signals", &"s\n".repeat(4));
        assert!(render(&p).contains("at most one more"));
        p.set_s("signals", "just one");
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
        for ex in examples("block-diagram").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("limited to") && !svg.contains("at least one"), "{}", ex.name);
        }
        for spec in ["\u{0}", "<b>x</b>", "nan", &"w".repeat(500), "a\n\nb"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("blocks", spec);
                p.set_s("signals", spec);
                p.set_s("feedback", spec);
                p.set_s("description", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
