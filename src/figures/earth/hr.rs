//! The Hertzsprung-Russell diagram: stars plotted by surface temperature (hot on the left) against
//! luminosity in solar units, both on log scales, with the spectral classes along the top. The
//! stars are the question; the regions (main sequence, giants, supergiants, white dwarfs) and each
//! star's class are the answers, so the student copy (Show answers off) plots the stars on empty
//! axes and leaves the class column blank.
use crate::figures::chart::{self, Frame, INK};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_STARS: usize = 10;
const MAX_SPEC: usize = 1500;
const DEFAULT_STARS: &str = "Sun 5800 1\nSirius A 9900 25\nVega 9600 40\nRigel 12100 120000\nBetelgeuse 3500 126000\nAldebaran 3900 440\nProxima Centauri 3000 0.0017\nSirius B 25000 0.026";
// the axes: log10 temperature from 4.7 (50 000 K, left) to 3.4 (2 500 K, right); log10 luminosity from -4 to 6
const LT0: f64 = 4.7;
const LT1: f64 = 3.4;
const LL0: f64 = -4.0;
const LL1: f64 = 6.0;

pub fn def() -> FigureDef {
    FigureDef {
        id: "hr-diagram",
        subject: Subject::EarthScience,
        title: "Hertzsprung-Russell diagram",
        blurb: "Stars plotted by temperature and luminosity on the Hertzsprung-Russell diagram, with the main sequence, giants, supergiants and white dwarfs shaded and each star classed, or only the plotted stars for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "hr-diagram" => vec![
            Example { name: "Eight well-known stars", params: vec![] },
            Example { name: "Just the Sun", params: vec![("stars", s("Sun 5800 1"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("stars", "Stars", DEFAULT_STARS).hint("One per line: a name, the surface temperature in kelvin, then the luminosity in solar units, like Sun 5800 1 (up to ten)"),
        ParamDef::bool("table", "List the stars with their class", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off plots the stars on bare axes and leaves each star's class blank"),
    ]
}

struct Star {
    name: String,
    t: f64,
    l: f64,
}

fn parse(spec: &str) -> Result<Vec<Star>, String> {
    if spec.chars().count() > MAX_SPEC {
        return Err(format!("The stars are limited to {MAX_SPEC} characters"));
    }
    let mut out = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(format!("\u{201c}{line}\u{201d} should be a name, a temperature and a luminosity"));
        }
        let (name, t, l) = (parts[..parts.len() - 2].join(" "), parts[parts.len() - 2], parts[parts.len() - 1]);
        let ok = |s: &str, lo: f64, hi: f64| s.parse::<f64>().ok().filter(|v| v.is_finite() && (lo..=hi).contains(v));
        let (Some(t), Some(l)) = (ok(t, 2500.0, 50000.0), ok(l, 1.0e-4, 1.0e6)) else {
            return Err(format!("\u{201c}{}\u{201d}: the temperature must be 2,500 to 50,000 K and the luminosity 0.0001 to 1,000,000", name.chars().take(20).collect::<String>()));
        };
        if out.len() >= MAX_STARS {
            return Err(format!("A figure is limited to {MAX_STARS} stars"));
        }
        out.push(Star { name: name.chars().take(18).collect(), t, l });
    }
    Ok(out)
}

/// The log luminosity of the main sequence at a log temperature.
pub fn main_sequence(lt: f64) -> f64 {
    7.0 * (lt - 3.763)
}

/// Which part of the diagram a star sits in.
pub fn class(t: f64, l: f64) -> &'static str {
    let (lt, ll) = (t.log10(), l.log10());
    let ms = main_sequence(lt);
    if ll < ms - 2.0 && ll < -1.0 && t >= 7000.0 {
        "white dwarf"
    } else if (ll - ms).abs() <= 1.0 {
        "main sequence"
    } else if ll > ms + 1.0 {
        if ll >= 4.0 { "supergiant" } else { "giant" }
    } else {
        "below the main sequence"
    }
}

/// Spectral class letters with the temperature they start at.
const SPECTRAL: [(&str, f64); 7] = [("O", 30000.0), ("B", 10000.0), ("A", 7500.0), ("F", 6000.0), ("G", 5200.0), ("K", 3700.0), ("M", 2500.0)];

fn render(p: &Params) -> String {
    let stars = match parse(p.s("stars")) {
        Ok(s) => s,
        Err(e) => return chart::message(&e),
    };
    let answers = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let table = p.b("table") && !stars.is_empty();
    let (pw, ph) = (600.0, 500.0);
    let tw = if table { 290.0 } else { 0.0 };
    let (w, h) = (pw + tw, ph + top);
    let f = Frame { w: pw, h: ph + top, l: 78.0, r: 24.0, t: 46.0 + top, b: 60.0, x0: LT0, x1: LT1, y0: LL0, y1: LL1 };
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, pw / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    // regions first (answers): the main sequence band, giants, supergiants and white dwarfs
    if answers {
        let clip = |lt: f64, ll: f64| (f.px(lt.clamp(LT1, LT0)), f.py(ll.clamp(LL0, LL1)));
        let band = |a: f64, b: f64| -> Vec<(f64, f64)> {
            let (l0, l1) = (LT0, LT1);
            vec![clip(l0, main_sequence(l0) + b), clip(l1, main_sequence(l1) + b), clip(l1, main_sequence(l1) + a), clip(l0, main_sequence(l0) + a)]
        };
        d.poly(&band(-1.0, 1.0), &fills.series(0), "none", 0.0);
        // giants: above the band and below log L 4; supergiants: above log L 4 and above the band
        let ms1 = |lt: f64| main_sequence(lt) + 1.0;
        let (g_lt, s_lt) = (3.763 + 3.0 / 7.0, 3.763 + 5.0 / 7.0);
        d.poly(&[clip(g_lt, 4.0), clip(LT1, 4.0), clip(LT1, ms1(LT1))], &fills.series(1), "none", 0.0);
        d.poly(&[clip(LT1, LL1), clip(s_lt, LL1), clip(g_lt, 4.0), clip(LT1, 4.0)], &fills.series(2), "none", 0.0);
        // white dwarfs: hotter than 7000 K, fainter than log L -1 and more than 2 below the main sequence
        let lw = 7000f64.log10();
        let wd_edge = 3.763 + 1.0 / 7.0;
        d.poly(&[clip(LT0, LL0), clip(LT0, -1.0), clip(wd_edge, -1.0), clip(lw, main_sequence(lw) - 2.0), clip(lw, LL0)], &fills.series(3), "none", 0.0);
    }
    // axes: ticks by temperature and luminosity
    let xt: Vec<(f64, String)> = [40000.0, 20000.0, 10000.0, 5000.0, 2500.0].iter().map(|t: &f64| (t.log10(), crate::figures::economics::n2(*t).replace(".0", ""))).map(|(v, s)| (v, format_k(&s))).collect();
    let sup = |e: i32| -> String {
        let digits = ["\u{2070}", "\u{b9}", "\u{b2}", "\u{b3}", "\u{2074}", "\u{2075}", "\u{2076}"];
        if e >= 0 { format!("10{}", digits[e as usize]) } else { format!("10\u{207b}{}", digits[(-e) as usize]) }
    };
    let yt: Vec<(f64, String)> = (-4..=6).step_by(2).map(|e| (e as f64, sup(e))).collect();
    chart::axes(&mut d, &f, &chart::AxesOpts { xticks: &xt, yticks: &yt, grid: true, x_title: "Surface temperature (K)", y_title: "Luminosity (Sun = 1)", fs });
    // spectral classes along the top
    for (i, (letter, t)) in SPECTRAL.iter().enumerate() {
        let hi = if i == 0 { 50000.0 } else { SPECTRAL[i - 1].1 };
        let (x0, x1) = (f.px(hi.log10()), f.px(t.log10()));
        d.line(x1, f.top() - 14.0, x1, f.top(), "#9ca3af", 1.0);
        chart::text(&mut d, (x0 + x1) / 2.0, f.top() - 20.0, fs * 1.05, "middle", letter);
    }
    chart::text(&mut d, (f.left() + f.right()) / 2.0, f.top() - 36.0, fs * 0.9, "middle", "Spectral class");
    if answers {
        let lab = |d: &mut Doc, lt: f64, ll: f64, t: &str| chart::text(d, f.px(lt), f.py(ll), fs, "middle", t);
        lab(&mut d, 4.15, 3.2, "Main sequence");
        lab(&mut d, 3.62, 3.2, "Giants");
        lab(&mut d, 4.42, 5.3, "Supergiants");
        lab(&mut d, 4.28, -3.1, "White dwarfs");
    }
    for (i, s) in stars.iter().enumerate() {
        let (x, y) = (f.px(s.t.log10()), f.py(s.l.log10()));
        d.circle(x, y, 5.5, "#fff", INK, 2.0);
        d.circle(x, y, 2.2, INK, INK, 0.5);
        // neighbours in the list put their names on opposite sides so close stars do not collide
        if i % 2 == 0 {
            chart::text(&mut d, x + 9.0, y - 7.0, fs * 0.92, "start", &s.name);
        } else {
            chart::text(&mut d, x - 9.0, y + fs * 1.1, fs * 0.92, "end", &s.name);
        }
    }
    if table {
        let tx = pw + 10.0;
        let row = fs * 1.9;
        let ty = f.top();
        d.rect(tx, ty, tw - 20.0, row, if fills.is_print() { "#e5e5e5" } else { "#e6edf8" }, INK, 1.4);
        chart::text(&mut d, tx + 8.0, ty + row * 0.68, fs * 0.9, "start", "Star");
        chart::text(&mut d, tx + 132.0, ty + row * 0.68, fs * 0.9, "start", "Class");
        for (i, s) in stars.iter().enumerate() {
            let y = ty + row * (i as f64 + 1.0);
            chart::text(&mut d, tx + 8.0, y + row * 0.68, fs * 0.92, "start", &s.name);
            if answers {
                chart::text(&mut d, tx + 132.0, y + row * 0.68, fs * 0.92, "start", class(s.t, s.l));
            } else {
                d.rect(tx + 128.0, y + 4.0, tw - 20.0 - 134.0, row - 8.0, "#fff", INK, 1.0);
            }
            d.line(tx, y, tx + tw - 20.0, y, "#c9ced6", 0.8);
        }
        d.rect(tx, ty, tw - 20.0, row * (stars.len() as f64 + 1.0), "none", INK, 1.6);
    }
    d.finish()
}

fn format_k(s: &str) -> String {
    // thousands with a comma: 40000 -> 40,000
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn well_known_stars_are_classed_correctly() {
        for (name, t, l, want) in [
            ("Sun", 5800.0, 1.0, "main sequence"),
            ("Sirius A", 9900.0, 25.0, "main sequence"),
            ("Vega", 9600.0, 40.0, "main sequence"),
            ("Rigel", 12100.0, 120000.0, "supergiant"),
            ("Betelgeuse", 3500.0, 126000.0, "supergiant"),
            ("Aldebaran", 3900.0, 440.0, "giant"),
            ("Proxima Centauri", 3000.0, 0.0017, "main sequence"),
            ("Sirius B", 25000.0, 0.026, "white dwarf"),
        ] {
            assert_eq!(class(t, l), want, "{name}");
        }
    }

    #[test]
    fn the_sun_sits_on_the_main_sequence_line() {
        assert!(main_sequence(5800f64.log10()).abs() < 0.2);
        assert_eq!(class(5800.0, 0.0001), "below the main sequence");
    }

    #[test]
    fn key_classes_every_star() {
        let svg = render(&defaults());
        assert_eq!(svg.matches(">main sequence<").count(), 4);
        for t in [">white dwarf<", ">giant<", ">supergiant<", ">Main sequence<", ">White dwarfs<", ">Spectral class<"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn student_copy_has_stars_but_no_regions_or_classes() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(student.contains(">Betelgeuse<") && student.contains(">Spectral class<") && student.contains(">K<"));
        assert!(!student.contains("Main sequence") && !student.contains(">giant<") && !student.contains("supergiant") && key.contains("supergiant"));
        assert_eq!(size_of(&student), size_of(&key));
    }

    #[test]
    fn bad_stars_are_explained() {
        for (spec, needle) in [("Sun 5800", "name, a temperature"), ("Sun x 1", "must be 2,500"), ("Sun 100 1", "must be 2,500"), ("Sun 5800 0", "must be 2,500"), ("Sun 5800 1e9", "must be 2,500"), ("Sun NaN 1", "must be 2,500")] {
            let mut p = defaults();
            p.set_s("stars", spec);
            assert!(render(&p).contains(needle), "{spec:?}");
        }
        let many: String = (0..11).map(|i| format!("S{i} 5000 1\n")).collect();
        let mut p = defaults();
        p.set_s("stars", &many);
        assert!(render(&p).contains("limited to 10"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("hr-diagram").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            assert!(sane(&render(&p)), "{}", ex.name);
        }
        for spec in ["", "\u{0}", "\u{1f600} 5000 1", "A 5000 1\nB 5000 1", "A B C 5000 1"] {
            for answers in [true, false] {
                for table in [true, false] {
                    let mut p = defaults();
                    p.set_s("stars", spec);
                    p.set_b("table", table);
                    p.set_b(SHOW_ANSWERS, answers);
                    assert!(sane(&render(&p)), "{spec:?}");
                }
            }
        }
    }
}
