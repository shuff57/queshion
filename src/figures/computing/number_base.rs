//! Number bases: a conversion table with one column given (decimal, binary, hex or octal) and the
//! others worked out, optionally over a strip of binary place values. The student copy (Show
//! answers off) keeps the given column and leaves the rest as empty boxes to fill in.
use super::graph_draw as gd;
use crate::figures::chart::{self, INK};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const MAX_ROWS: usize = 24;
const GIVEN: [&str; 4] = ["Decimal", "Binary", "Hexadecimal", "Octal"];
const WIDTHS: [&str; 5] = ["Shortest", "4 bits", "8 bits", "12 bits", "16 bits"];

pub fn def() -> FigureDef {
    FigureDef {
        id: "number-base",
        subject: Subject::Computing,
        title: "Number bases",
        blurb: "A conversion table between decimal, binary, hexadecimal and octal: give one column and the others are worked out, or left as empty boxes for students, with an optional strip of binary place values.",
        params,
        render,
    }
}

pub fn relevant(_fig: &str, _key: &str, _p: &Params) -> bool {
    true
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "number-base" => vec![
            Example { name: "Decimal to the rest", params: vec![] },
            Example { name: "Binary to decimal", params: vec![("values", s("1010, 1111, 100000, 10110011")), ("given", s("Binary")), ("width", s("8 bits"))] },
            Example { name: "Hex colours", params: vec![("values", s("FF, 80, 1A, C0")), ("given", s("Hexadecimal")), ("width", s("8 bits"))] },
            Example { name: "Powers of two", params: vec![("values", s("1, 2, 4, 8, 16, 32, 64, 128, 255")), ("place_values", Value::B(true)), ("width", s("8 bits"))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("values", "Values", "13, 45, 200, 255").hint("Separated by commas or spaces, written in the given base"),
        ParamDef::choice("given", "Given in", &GIVEN, "Decimal").hint("The column that is filled in; the other three are the answers"),
        ParamDef::choice("width", "Binary width", &WIDTHS, "8 bits").hint("Pads binary with zeros to whole bytes (or 4, 12, 16 bits) and spaces every four digits, so 13 reads 0000 1101"),
        ParamDef::bool("place_values", "Show binary place values", false).hint("A strip of 128 64 32 16 8 4 2 1 above the table (as many as the width, or 8)"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the given column and leaves the others as empty boxes for students"),
    ]
}

fn radix(name: &str) -> u32 {
    match name {
        "Binary" => 2,
        "Hexadecimal" => 16,
        "Octal" => 8,
        _ => 10,
    }
}

fn parse(token: &str, base: u32) -> Option<u64> {
    let t = token.trim().trim_start_matches("0x").trim_start_matches("0b");
    let t: String = t.chars().filter(|c| *c != '_').collect();
    let v = u64::from_str_radix(&t, base).ok()?;
    (v <= u32::MAX as u64).then_some(v)
}

/// Binary padded to a multiple of `n` digits and spaced every four.
fn group(s: &str, n: usize) -> String {
    if n == 0 {
        return s.to_string();
    }
    let pad = (n - s.len() % n) % n;
    let padded = format!("{}{}", "0".repeat(pad), s);
    padded.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap_or("")).collect::<Vec<_>>().join(" ")
}

fn show(v: u64, base: &str, bits: usize) -> String {
    match base {
        "Binary" => group(&format!("{v:b}"), bits),
        "Hexadecimal" => format!("{v:X}"),
        "Octal" => format!("{v:o}"),
        _ => v.to_string(),
    }
}

fn render(p: &Params) -> String {
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 16.0 * k;
    let given = GIVEN.iter().copied().find(|g| *g == p.s("given")).unwrap_or("Decimal");
    let base = radix(given);
    let bits = match p.s("width") {
        "4 bits" => 4,
        "8 bits" => 8,
        "12 bits" => 12,
        "16 bits" => 16,
        _ => 0,
    };
    let tokens: Vec<&str> = p.s("values").split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()).take(MAX_ROWS).collect();
    if tokens.is_empty() {
        return chart::message("Add some values to convert");
    }
    let rows: Vec<(String, Option<u64>)> = tokens.iter().map(|t| (t.to_string(), parse(t, base))).collect();

    let cols: Vec<&str> = GIVEN.iter().copied().collect();
    let cw: Vec<f64> = cols
        .iter()
        .map(|c| {
            let longest = rows
                .iter()
                .map(|(tok, v)| match v {
                    Some(v) => show(*v, c, bits).chars().count().max(tok.chars().count()),
                    None => tok.chars().count(),
                })
                .max()
                .unwrap_or(1)
                .max(c.chars().count());
            (longest as f64 * fs * 0.62 + 24.0).max(90.0)
        })
        .collect();
    let row_h = fs * 2.1;
    let title = p.s("title").trim();
    let strip_n = if bits == 0 { 8 } else { bits.min(16) };
    let strip = p.b("place_values");
    let strip_cell = (fs * 2.4).max(38.0);
    let strip_w = strip_n as f64 * strip_cell;
    let tw: f64 = cw.iter().sum();
    let w = (tw.max(if strip { strip_w } else { 0.0 }) + 40.0).max(300.0);
    let top = if title.is_empty() { 18.0 } else { 40.0 + fs * 0.4 };
    let strip_h = if strip { row_h * 2.0 + 14.0 } else { 0.0 };
    let h = top + strip_h + row_h * (rows.len() + 1) as f64 + 18.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.3, fs * 1.3, "middle", title);
    }
    let head = if print { "#e5e5e5" } else { "#f0e9db" };
    let given_fill = if print { "#f3f3f3" } else { "#f6f2e9" };
    let mut y = top;
    if strip {
        let x0 = (w - strip_w) / 2.0;
        for i in 0..strip_n {
            let exp = strip_n - 1 - i;
            let x = x0 + i as f64 * strip_cell;
            d.rect(x, y, strip_cell, row_h, head, INK, 1.2);
            chart::text(&mut d, x + strip_cell / 2.0, y + row_h / 2.0 + fs * 0.35, fs * 0.85, "middle", &(1u64 << exp).to_string());
            d.rect(x, y + row_h, strip_cell, row_h, "#fff", INK, 1.2);
        }
        y += strip_h;
    }
    let x0 = (w - tw) / 2.0;
    let mut x = x0;
    for (ci, c) in cols.iter().enumerate() {
        d.rect(x, y, cw[ci], row_h, head, INK, 1.2);
        chart::text(&mut d, x + cw[ci] / 2.0, y + row_h / 2.0 + fs * 0.35, fs, "middle", c);
        x += cw[ci];
    }
    for (ri, (tok, v)) in rows.iter().enumerate() {
        let ry = y + row_h * (ri + 1) as f64;
        let mut x = x0;
        for (ci, c) in cols.iter().enumerate() {
            let is_given = *c == given;
            d.rect(x, ry, cw[ci], row_h, if is_given { given_fill } else { "#fff" }, INK, 1.2);
            let text = match (v, is_given) {
                (None, true) => Some(format!("{tok}?")),
                (None, false) => Some("\u{2014}".to_string()),
                (Some(v), true) => Some(show(*v, c, bits)),
                (Some(v), false) if answers => Some(show(*v, c, bits)),
                _ => None,
            };
            if let Some(t) = text {
                gd::text_fill(&mut d, x + cw[ci] / 2.0, ry + row_h / 2.0 + fs * 0.35, fs, INK, &t);
            }
            x += cw[ci];
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    fn size(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

    #[test]
    fn default_converts_every_column() {
        let svg = render(&defaults());
        for t in [">13<", ">0000 1101<", ">D<", ">15<", ">1111 1111<", ">FF<", ">377<", ">C8<"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn other_bases_in_and_grouping() {
        let mut p = defaults();
        p.set_s("given", "Binary");
        p.set_s("values", "1010 11111111 0b101");
        p.set_s("width", "Shortest");
        let svg = render(&p);
        assert!(svg.contains(">10<") && svg.contains(">255<") && svg.contains(">5<") && svg.contains(">A<"));
        p.set_s("given", "Hexadecimal");
        p.set_s("values", "ff, 0x1A");
        p.set_s("width", "8 bits");
        let svg = render(&p);
        assert!(svg.contains(">255<") && svg.contains(">26<") && svg.contains(">0001 1010<"));
    }

    #[test]
    fn bad_tokens_are_marked_not_dropped() {
        let mut p = defaults();
        p.set_s("given", "Binary");
        p.set_s("values", "102, 11");
        let svg = render(&p);
        assert!(svg.contains(">102?<") && svg.contains(">3<"));
    }

    #[test]
    fn student_copy_keeps_the_given_column_only() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        assert!(student.contains(">45<") && !student.contains(">0010 1101<") && key.contains(">0010 1101<"));
        assert_eq!(student.matches("<rect").count(), key.matches("<rect").count());
        assert_eq!(size(&student), size(&key));
    }

    #[test]
    fn place_values_strip() {
        let mut p = defaults();
        p.set_b("place_values", true);
        p.set_s("width", "8 bits");
        let svg = render(&p);
        for t in [">128<", ">64<", ">1<"] {
            assert!(svg.contains(t), "{t}");
        }
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#f6f2e9") && !svg.contains("#f0e9db"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("number-base").unwrap() {
            let mut p = defaults();
            for (k, v) in &ex.params {
                match v {
                    Value::S(s) => p.set_s(k, s),
                    Value::B(b) => p.set_b(k, *b),
                    _ => {}
                }
            }
            let svg = render(&p);
            assert!(svg.contains("<rect") && !svg.contains("NaN"), "{}", ex.name);
        }
        let many: String = (0..100).map(|i| format!("{i} ")).collect();
        for given in GIVEN {
            for text in ["", "abc", "99999999999999999999", "-5", "1e9", "\u{0}", &many, "4294967295", "4294967296", "0x", ",,,"] {
                for width in WIDTHS {
                    for answers in [true, false] {
                        let mut p = defaults();
                        p.set_s("given", given);
                        p.set_s("values", text);
                        p.set_s("width", width);
                        p.set_b("place_values", true);
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{given} {text:?}");
                    }
                }
            }
        }
    }
}
