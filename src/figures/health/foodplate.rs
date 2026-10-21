//! A food plate: a plate divided into the four food groups by share, with an optional dairy
//! circle. Foods are listed mixed up with the group each belongs to as the answer, so the student
//! copy (Show answers off) leaves a table of the groups empty to sort the foods into.
use crate::figures::chart;
use crate::figures::earth::polar;
use crate::figures::geography::trim;
use crate::figures::language::{shuffled, wrap};
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

const GROUPS: [&str; 5] = ["Vegetables", "Grains", "Protein", "Fruits", "Dairy"];
const INK: &str = "#111827";
const MAX_FOODS: usize = 14;
const DEFAULT: &str = "Broccoli: Vegetables\nBrown rice: Grains\nChicken: Protein\nApple: Fruits\nYoghurt: Dairy\nCarrot: Vegetables\nLentils: Protein\nOatmeal: Grains";

pub fn def() -> FigureDef {
    FigureDef {
        id: "food-plate",
        subject: Subject::Health,
        title: "Food plate",
        blurb: "A plate divided into vegetables, grains, protein and fruits by share, with a dairy circle, and foods sorted into their groups, or an empty table and a mixed-up list of foods for students.",
        params,
        render,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "food-plate" => vec![
            Example { name: "Balanced plate", params: vec![] },
            Example { name: "More vegetables", params: vec![("veg", Value::F(50.0)), ("fruit", Value::F(10.0)), ("grains", Value::F(20.0)), ("protein", Value::F(20.0))] },
            Example { name: "No dairy", params: vec![("dairy", Value::B(false)), ("foods", s("Spinach: Vegetables\nBanana: Fruits\nBeans: Protein\nPasta: Grains"))] },
            Example { name: "Blank for students", params: vec![("show_answers", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("veg", "Vegetables (share of the plate)", 0.0, 100.0, 5.0, 30.0),
        ParamDef::float("grains", "Grains", 0.0, 100.0, 5.0, 30.0),
        ParamDef::float("protein", "Protein", 0.0, 100.0, 5.0, 20.0),
        ParamDef::float("fruit", "Fruits", 0.0, 100.0, 5.0, 20.0),
        ParamDef::bool("dairy", "Show a dairy circle", true),
        ParamDef::long("foods", "Foods: name, then its group", DEFAULT).hint("One per line, as Broccoli: Vegetables. Groups are Vegetables, Grains, Protein, Fruits and Dairy; up to 14 foods"),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the plate and the foods and leaves the table of groups empty for students to sort the foods into"),
    ]
}

pub fn parse_foods(spec: &str, dairy: bool) -> Result<Vec<(String, usize)>, String> {
    if spec.chars().count() > 1000 {
        return Err("The foods are limited to 1000 characters".into());
    }
    let mut v = Vec::new();
    for line in spec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (name, group) = line.rsplit_once(':').ok_or_else(|| format!("\u{201c}{}\u{201d} needs a group after a colon", line.chars().take(20).collect::<String>()))?;
        let g = group.trim().to_lowercase();
        let idx = GROUPS.iter().position(|x| x.to_lowercase() == g || (g.len() >= 3 && x.to_lowercase().starts_with(&g))).ok_or_else(|| format!("\u{201c}{}\u{201d} is not a food group", group.trim().chars().take(16).collect::<String>()))?;
        if idx == 4 && !dairy {
            return Err("Dairy is switched off, so no food can be in it".into());
        }
        v.push((name.trim().chars().take(24).collect(), idx));
        if v.len() > MAX_FOODS {
            return Err(format!("There can be at most {MAX_FOODS} foods"));
        }
    }
    Ok(v)
}

fn render(p: &Params) -> String {
    let shares = [p.f("veg"), p.f("grains"), p.f("protein"), p.f("fruit")];
    if !shares.iter().all(|v| v.is_finite()) {
        return chart::message("Give the shares as numbers");
    }
    let shares: Vec<f64> = shares.iter().map(|v| v.clamp(0.0, 100.0)).collect();
    let sum: f64 = shares.iter().sum();
    if sum <= 0.0 {
        return chart::message("At least one group needs a share above 0");
    }
    let dairy = p.b("dairy");
    let foods = match parse_foods(p.s("foods"), dairy) {
        Ok(v) => v,
        Err(e) => return chart::message(&e),
    };
    let answers = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fs = 13.0 * svg::label_factor(p.s("label_size"));
    let title = p.s("title").trim();
    let top = if title.is_empty() { 0.0 } else { 30.0 };
    let n = foods.len();
    let order = shuffled(n);
    let mut number = vec![0usize; n];
    for (pos, &orig) in order.iter().enumerate() {
        number[orig] = pos + 1;
    }
    let list_rows = (n + 1) / 2;
    let w = 960.0;
    let h = top + (440.0f64).max(120.0 + 5.0 * 64.0) + 40.0 + list_rows as f64 * fs * 1.9 + 30.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 22.0, fs * 1.3, "middle", title);
    }
    let (cx, cy, r) = (230.0, top + 230.0, 180.0);
    let colours = if print { ["#e4e4e4", "#d2d2d2", "#c0c0c0", "#eeeeee"] } else { ["#9fd49f", "#f2c98a", "#e9a1a1", "#f4b6c8"] };
    d.circle(cx, cy, r + 14.0, "#fff", INK, 2.0);
    let mut a0 = 90.0;
    for (i, name) in GROUPS.iter().take(4).enumerate() {
        let frac = shares[i] / sum;
        if frac <= 0.0 {
            continue;
        }
        let sweep = frac * 360.0;
        let a1 = a0 - sweep;
        if frac >= 0.9999 {
            d.circle(cx, cy, r, colours[i], INK, 2.0);
        } else {
            let (p0, p1) = (polar(cx, cy, r, a0), polar(cx, cy, r, a1));
            d.path(&format!("M {cx:.2} {cy:.2} L {:.2} {:.2} A {r} {r} 0 {} 1 {:.2} {:.2} Z", p0.0, p0.1, if sweep > 180.0 { 1 } else { 0 }, p1.0, p1.1), colours[i], INK, 2.0);
        }
        let mid = polar(cx, cy, r * if frac >= 0.9999 { 0.0 } else { 0.6 }, a0 - sweep / 2.0);
        chart::text(&mut d, mid.0, mid.1 - 2.0, fs * 1.1, "middle", name);
        chart::text(&mut d, mid.0, mid.1 + fs * 1.2, fs, "middle", &format!("{}%", trim(frac * 100.0, 0)));
        a0 = a1;
    }
    if dairy {
        let (dx, dy) = (cx + r + 70.0, top + 70.0);
        d.circle(dx, dy, 46.0, if print { "#f6f6f6" } else { "#cfe0fa" }, INK, 2.0);
        chart::text(&mut d, dx, dy + fs * 0.35, fs * 1.1, "middle", "Dairy");
    }
    // the table of groups
    let tx = 560.0;
    let (c0, c1) = (110.0, 270.0);
    let ty = top + 40.0;
    d.rect(tx, ty - 22.0, c0 + c1, 32.0, "none", INK, 1.4);
    chart::text(&mut d, tx + c0 / 2.0, ty, fs * 1.05, "middle", "Group");
    chart::text(&mut d, tx + c0 + c1 / 2.0, ty, fs * 1.05, "middle", "Foods (numbers)");
    let groups = if dairy { 5 } else { 4 };
    for g in 0..groups {
        let y = ty + 10.0 + g as f64 * 64.0;
        d.rect(tx, y, c0 + c1, 64.0, "none", INK, 1.2);
        chart::text(&mut d, tx + c0 / 2.0, y + 36.0, fs, "middle", GROUPS[g]);
        if answers {
            let nums: Vec<String> = foods.iter().enumerate().filter(|(_, f)| f.1 == g).map(|(i, _)| number[i].to_string()).collect::<Vec<_>>();
            let mut nums: Vec<usize> = nums.iter().map(|s| s.parse().unwrap()).collect();
            nums.sort();
            let text = nums.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ");
            for (k, l) in wrap(&text, 26, 2).iter().enumerate() {
                chart::text(&mut d, tx + c0 + c1 / 2.0, y + 30.0 + k as f64 * fs * 1.3, fs, "middle", l);
            }
        }
    }
    d.line(tx + c0, ty - 22.0, tx + c0, ty + 10.0 + groups as f64 * 64.0, INK, 1.2);
    let ly = top + 480.0;
    chart::text(&mut d, 40.0, ly, fs * 1.05, "start", "Foods to sort");
    for (pos, &orig) in order.iter().enumerate() {
        let (col, row) = (pos / list_rows.max(1), pos % list_rows.max(1));
        chart::text(&mut d, 40.0 + col as f64 * 460.0, ly + 24.0 + row as f64 * fs * 1.9, fs * 0.95, "start", &format!("{}. {}", pos + 1, foods[orig].0));
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::economics::testing::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn foods_parse_with_group_names() {
        let v = parse_foods("a: veg\nb: GRAINS\nc: Fruit\nd: dairy", true).unwrap();
        assert_eq!(v.iter().map(|x| x.1).collect::<Vec<_>>(), vec![0, 1, 3, 4]);
        assert!(parse_foods("a", true).unwrap_err().contains("colon"));
        assert!(parse_foods("a: sweets", true).unwrap_err().contains("not a food group"));
        assert!(parse_foods("a: dairy", false).unwrap_err().contains("switched off"));
        assert!(parse_foods("a: ", true).unwrap_err().contains("not a food group"));
        assert!(parse_foods(&"a: veg\n".repeat(15), true).unwrap_err().contains("at most 14"));
        assert!(parse_foods(&"a: veg\n".repeat(200), true).unwrap_err().contains("1000"));
    }

    #[test]
    fn key_sorts_foods_into_groups() {
        let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
        assert!(key.contains(">30%<") && key.contains(">Dairy<") && student.contains(">Vegetables<"));
        assert_eq!(size_of(&student), size_of(&key));
        // two vegetables in the default list: their numbers appear in the Vegetables row of the key only
        let order = crate::figures::language::shuffled(8);
        let mut nums: Vec<usize> = [0usize, 5].iter().map(|o| order.iter().position(|x| x == o).unwrap() + 1).collect();
        nums.sort();
        let text = format!(">{}, {}<", nums[0], nums[1]);
        assert!(key.contains(&text) && !student.contains(&text), "{text}");
    }

    #[test]
    fn shares_are_normalised() {
        let mut p = defaults();
        for k in ["veg", "grains", "protein", "fruit"] {
            p.set_f(k, 10.0);
        }
        assert!(render(&p).matches(">25%<").count() == 4);
        p.set_f("veg", 100.0);
        for k in ["grains", "protein", "fruit"] {
            p.set_f(k, 0.0);
        }
        let svg = render(&p);
        assert!(svg.contains(">100%<") && sane(&svg));
        p.set_f("veg", 0.0);
        assert!(render(&p).contains("above 0"));
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        assert_grey(&render(&p));
    }

    #[test]
    fn examples_render_and_hostile_input_is_safe() {
        for ex in examples("food-plate").unwrap() {
            let mut p = defaults();
            apply(&mut p, &ex.params);
            let svg = render(&p);
            assert!(sane(&svg) && !svg.contains("not a food group") && !svg.contains("above 0"), "{}", ex.name);
        }
        for v in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e300] {
            let mut p = defaults();
            p.set_f("veg", v);
            assert!(sane(&render(&p)), "{v}");
        }
        for spec in ["\u{0}: veg", "<b>: veg", "nan: nan", ":", "a: veg: dairy"] {
            for answers in [true, false] {
                let mut p = defaults();
                p.set_s("foods", spec);
                p.set_b(SHOW_ANSWERS, answers);
                let svg = render(&p);
                assert!(sane(&svg) && !svg.contains("<b>"), "{spec:?}");
            }
        }
    }
}
