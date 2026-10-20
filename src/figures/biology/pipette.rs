//! Micropipette reading generator: a single-channel pipette whose three-digit display
//! reads the chosen volume. Digit weights depend on the size (P2: 1, 0.1, 0.01 uL;
//! P10/P20: 10, 1, 0.1; P100/P200: 100, 10, 1; P1000: 1000, 100, 10).
use super::*;

const SIZES: [&str; 6] = ["2 µL", "10 µL", "20 µL", "100 µL", "200 µL", "1000 µL"];
pub const SHAPES: [&str; 2] = ["Long body, short shaft", "Short body, long shaft and tip"];

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("size", "Size", &SIZES, "20 µL"),
        ParamDef::float("volume", "Volume it's set to", 0.0, 1000.0, 0.01, 12.5),
        ParamDef::bool("tip", "Tip on the end", true),
        ParamDef::bool("size_label", "Size printed above the display", true),
        // saved figures keep the long body; the opening state has the reference's shorter body
        // with a long shaft and tip
        ParamDef::choice("shape", "Shape", &SHAPES, SHAPES[0]),
        ParamDef::bool("color", "Decimals in red", true),
        ParamDef::bool("decimal_line", "Line at the decimal point", true),
        ParamDef::bool("answer", "Print the reading under the figure", false),
        ParamDef::text("title", "Title above the pipette (blank = none)", ""),
        ParamDef::text("answer_text", "Answer text (blank = automatic)", ""),
    ]
}

pub struct Spec { pub max: f64, pub min: f64, pub unit: f64, pub red: usize, accent: &'static str, tip: &'static str }

pub fn spec(size: &str) -> Spec {
    match size {
        "2 µL" => Spec { max: 2.0, min: 0.2, unit: 0.01, red: 2, accent: "#cbd5e1", tip: "#f8fafc" },
        "10 µL" => Spec { max: 10.0, min: 1.0, unit: 0.1, red: 1, accent: "#cbd5e1", tip: "#f8fafc" },
        "100 µL" => Spec { max: 100.0, min: 10.0, unit: 1.0, red: 0, accent: "#facc15", tip: "#fde047" },
        "200 µL" => Spec { max: 200.0, min: 20.0, unit: 1.0, red: 0, accent: "#facc15", tip: "#fde047" },
        "1000 µL" => Spec { max: 1000.0, min: 100.0, unit: 10.0, red: 0, accent: "#60a5fa", tip: "#bfdbfe" },
        _ => Spec { max: 20.0, min: 2.0, unit: 0.1, red: 1, accent: "#facc15", tip: "#fde047" },
    }
}

fn trim(v: f64) -> String {
    let t = format!("{v:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The page's two notes: the one under the size ("Set from 2 to 20 µL. Its digits count ...")
/// and the one under the reading.
pub fn notes(p: &Params) -> (String, String) {
    let size = p.s("size");
    let sp = spec(size);
    let size_note = format!(
        "Set from {} to {} µL. Its digits count {} µL, {} µL and {} µL, top to bottom.",
        trim(sp.min), trim(sp.max), trim(sp.unit * 100.0), trim(sp.unit * 10.0), trim(sp.unit)
    );
    let reading = format!("A {size} pipette is set from {} to {} µL, in steps of {} µL.", trim(sp.min), trim(sp.max), trim(sp.unit));
    (size_note, reading)
}

/// The reference's one-line summaries of the three cards.
pub fn skin_summary(id: &str, title: &str, p: &Params) -> Option<String> {
    if id != "micropipette-reading" {
        return None;
    }
    let size = p.s("size");
    Some(match title {
        "Micropipette" => format!("{size}, {}", if p.b("tip") { "with a tip" } else { "no tip" }),
        "Reading" => {
            let dg = digits(size, p.f("volume"));
            format!("{} µL, reads {}-{}-{}", trim((p.f("volume") * 100.0).round() / 100.0), dg[0], dg[1], dg[2])
        }
        "Color" => format!("{}, {}", if p.b("color") { "Color" } else { "Black and white" }, if p.b("decimal_line") { "decimal line" } else { "no decimal line" }),
        _ => return None,
    })
}

/// Three digits shown for `volume` on the given pipette.
pub fn digits(size: &str, volume: f64) -> [u8; 3] {
    let s = spec(size);
    let v = volume.clamp(s.min, s.max);
    let units = (v / s.unit).round() as i64;
    [((units / 100) % 10) as u8, ((units / 10) % 10) as u8, (units % 10) as u8]
}

/// An absolute SVG path (M H V L C Q Z) moved by (`dx`, `dy`).
fn shift_path(d: &str, dx: f64, dy: f64) -> String {
    let mut out = String::new();
    let mut cmd = ' ';
    let mut k = 0usize;
    for tok in d.split_whitespace() {
        if tok.len() == 1 && tok.chars().all(|c| c.is_ascii_alphabetic()) {
            cmd = tok.chars().next().unwrap_or(' ');
            k = 0;
            out.push_str(tok);
            out.push(' ');
            continue;
        }
        let v: f64 = tok.parse().unwrap_or(0.0);
        let sh = match cmd { 'H' => dx, 'V' => dy, _ => if k % 2 == 0 { dx } else { dy } };
        out.push_str(&format!("{:.2} ", v + sh));
        k += 1;
    }
    out.trim_end().to_string()
}

pub fn render(p: &Params) -> String {
    let size = p.s("size").to_string();
    let sp = spec(&size);
    let vol = p.f("volume").clamp(sp.min, sp.max);
    let dg = digits(&size, vol);
    let short = p.s("shape") == SHAPES[1];
    let titled = !p.s("title").trim().is_empty();
    // the reference's short pipette is drawn on a 138 x 608 canvas with 16 px of padding
    let (w, h) = if short { (138.0, 608.0 + if titled { 30.0 } else { 0.0 } + if p.b("answer") { 30.0 } else { 0.0 }) } else { (320.0, 640.0) };
    let (ox, oy) = (16.0, 16.0 + if short && titled { 30.0 } else { 0.0 });
    let cx = if short { ox + 40.0 } else { 150.0 };
    let mut d = Doc::new(w, h);
    let color = p.b("color");
    // black and white: black outlines on white, like the reference
    let body = if color { "#e5e7eb" } else { "#fff" };
    let light = if color { "#f3f4f6" } else { "#fff" };
    let accent = if color { sp.accent } else { "#fff" };

    // plunger button + shaft
    if !short {
        d.raw(&format!(r#"<rect x="{:.1}" y="36" width="24" height="15" rx="4" fill="{accent}" stroke="{INK}" stroke-width="2"/>"#, cx - 12.0));
        d.rect(cx - 9.0, 50.0, 18.0, 30.0, light, INK, 2.0);
    }
    // body with thumb hook: the finger rest curls down to a rounded tip, and a rounded nub
    // (the notch between the rest and the body) rather than a sharp V, as the reference draws it
    if short {
        // the reference's pipette, drawn in its own coordinates: a short body, a tapering neck, a
        // long thin shaft and a long tip
        let path = |d: &mut Doc, v: &str, fill: &str, sw: f64| { d.raw(&format!(r#"<path d="{}" fill="{fill}" stroke="{INK}" stroke-width="{sw}"/>"#, shift_path(v, ox, oy))); };
        path(&mut d, "M 27 12 V 5 Q 27 0 32 0 H 48 Q 53 0 53 5 V 12 Z", accent, 2.0);
        d.rect(ox + 29.0, oy + 10.0, 22.0, 40.0, body, INK, 2.0);
        path(&mut d, "M 10 272 V 58 Q 10 46 22 46 H 50 C 62 44 86 32 100 42 C 108 52 107 86 100 112 C 98 120 89 121 88 113 C 87 94 86 76 80 74 C 76 73 70 78 70 92 V 272 C 70 298 62 304 62 318 H 18 C 18 304 10 298 10 272 Z", body, 2.0);
        path(&mut d, "M 31 334 H 49 L 44 501 Q 44 504 41 504 H 39 Q 36 504 36 501 Z", light, 2.0);
        path(&mut d, "M 22 332 H 58 C 58 358 50.53 358 50.53 381 Q 50.53 384 47.53 384 H 32.47 Q 29.47 384 29.47 381 C 29.47 358 22 358 22 332 Z", light, 2.0);
        d.raw(&format!(r#"<rect x="{:.1}" y="{:.1}" width="42" height="18" rx="3" fill="{light}" stroke="{INK}" stroke-width="2"/>"#, ox + 19.0, oy + 316.0));
        if p.b("tip") {
            d.raw(&format!(r#"<path d="{}" fill="{}" fill-opacity="0.75" stroke="{INK}" stroke-width="1.6"/>"#, shift_path("M 32 489 H 48 L 41.5 574 H 38.5 Z", ox, oy), if color { sp.tip } else { "#fff" }));
            d.raw(&format!(r#"<rect x="{:.1}" y="{:.1}" width="19" height="7" rx="1.5" fill="{accent}" fill-opacity="0.9" stroke="{INK}" stroke-width="1.6"/>"#, ox + 30.5, oy + 482.0));
        }
    } else {
        let b = format!(
            "M{lb:.1},470 Q{l:.1},452 {l:.1},430 L{l:.1},92 Q{l:.1},80 {l2:.1},80 L{r1:.1},80 C{r2:.1},68 {r3:.1},74 {r3:.1},106 C{r3:.1},138 {r4:.1},158 {r5:.1},158 C{r6:.1},158 {r7:.1},150 {r7:.1},134 C{r7:.1},112 {r:.1},112 {r:.1},128 L{r:.1},430 Q{r:.1},452 {rb:.1},470 Z",
            lb = cx - 21.0, rb = cx + 21.0,
            l = cx - 28.0, l2 = cx - 16.0, r1 = cx + 12.0, r2 = cx + 40.0, r3 = cx + 58.0, r4 = cx + 56.0, r5 = cx + 48.0,
            r6 = cx + 42.0, r7 = cx + 40.0, r = cx + 28.0
        );
        d.path(&b, body, INK, 2.2);
        // collar, neck, shaft
        d.raw(&format!(r#"<rect x="{:.1}" y="470" width="38" height="16" rx="3" fill="{light}" stroke="{INK}" stroke-width="2"/>"#, cx - 19.0));
        d.poly(&[(cx - 17.0, 486.0), (cx + 17.0, 486.0), (cx + 8.0, 530.0), (cx - 8.0, 530.0)], light, INK, 2.0);
        d.poly(&[(cx - 7.0, 530.0), (cx + 7.0, 530.0), (cx + 5.0, 572.0), (cx - 5.0, 572.0)], light, INK, 2.0);
        if p.b("tip") {
            d.rect(cx - 9.0, 568.0, 18.0, 7.0, accent, INK, 1.6);
            d.poly(&[(cx - 8.0, 574.0), (cx + 8.0, 574.0), (cx + 1.5, 612.0), (cx - 1.5, 612.0)], if color { sp.tip } else { "#fff" }, INK, 1.8);
        }
    }
    // display window: three rows
    let (wx, wy, ww, rh, dfs) = if short { (cx - 17.0, oy + 94.0, 34.0, 24.0, 20.0) } else { (cx - 16.0, 150.0, 32.0, 28.0, 24.0) };
    d.raw(&format!(r##"<rect x="{wx:.1}" y="{wy:.1}" width="{ww:.1}" height="{:.1}" rx="5" fill="#fff" stroke="{INK}" stroke-width="2.2"/>"##, if short { 82.0 } else { rh * 3.0 }));
    // the short body's rows start 5 px under the window's top edge
    let top = if short { 5.0 } else { 0.0 };
    for i in 1..3 { d.line(wx + 4.0, wy + top + rh * i as f64, wx + ww - 4.0, wy + top + rh * i as f64, "#9ca3af", 0.8); }
    for (i, v) in dg.iter().enumerate() {
        let red = color && sp.red > 0 && i >= 3 - sp.red;
        let col = if red { "#c8102e" } else { INK };
        let ty = if short { wy + rh * i as f64 + 17.0 + 0.36 * dfs } else { wy + rh * i as f64 + 22.0 };
        d.raw(&format!(r##"<text x="{cx:.1}" y="{ty:.1}" font-size="{dfs}" text-anchor="middle" font-family="{SANS}" font-weight="bold" fill="{col}">{v}</text>"##));
    }
    if p.b("decimal_line") && sp.red > 0 {
        let ly = wy + top + rh * (3 - sp.red) as f64;
        d.line(wx, ly, wx + ww, ly, INK, 2.4);
    }
    if p.b("size_label") {
        if short { bold(&mut d, cx, wy - 14.0 + 0.35 * 12.0, 12.0, "middle", &size); } else { bold(&mut d, cx, wy - 10.0, 11.0, "middle", &size); }
    }
    if p.b("answer") {
        let custom = p.s("answer_text").trim();
        let t = if custom.is_empty() { format!("Reading: {} µL", fnum(vol)) } else { custom.to_string() };
        bold(&mut d, w / 2.0, h - 8.0, 16.0, "middle", &t);
    }
    if !p.s("title").trim().is_empty() {
        bold(&mut d, w / 2.0, 22.0, 17.0, "middle", p.s("title").trim());
    }
    d.finish()
}

/// "Start from an example" entries (the reference gallery).
pub fn examples() -> Vec<Example> {
    let st = |v: &str| Value::S(v.into());
    vec![
        Example { name: "P20 micropipette set to 15.7 µL", params: vec![("size", st("20 µL")), ("volume", Value::F(15.7))] },
        Example { name: "P200 micropipette set to 154 µL", params: vec![("size", st("200 µL")), ("volume", Value::F(154.0))] },
        Example { name: "P1000 micropipette set to 820 µL", params: vec![("size", st("1000 µL")), ("volume", Value::F(820.0))] },
        Example { name: "P2 micropipette set to 0.85 µL", params: vec![("size", st("2 µL")), ("volume", Value::F(0.85))] },
        Example { name: "Black and white P10 micropipette set to 4.6 µL", params: vec![("size", st("10 µL")), ("volume", Value::F(4.6)), ("tip", Value::B(false)), ("color", Value::B(false))] },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The body outline has the reference's rounded finger-rest nub (a curve, no sharp V) and a
    /// rounded plunger button.
    #[test]
    fn rounded_nub_and_button() {
        let d = crate::figures::registry().into_iter().find(|d| d.id == "micropipette-reading").unwrap();
        let svg = (d.render)(&Params::defaults(&(d.params)()));
        assert!(svg.contains(r#"y="36" width="24" height="15" rx="4""#), "rounded button");
        assert!(svg.contains("C190.0,112 178.0,112 178.0,128"), "rounded nub between rest and body");
    }

    #[test]
    fn digit_values() {
        assert_eq!(digits("20 µL", 12.5), [1, 2, 5]);
        assert_eq!(digits("2 µL", 0.75), [0, 7, 5]);
        assert_eq!(digits("1000 µL", 350.0), [0, 3, 5]);
        assert_eq!(digits("200 µL", 150.0), [1, 5, 0]);
    }

    #[test]
    fn variants() {
        render_case("micropipette-reading", "default", &[]);
        render_case("micropipette-reading", "p2", &[("size", s("2 µL")), ("volume", Value::F(0.75)), ("tip", Value::B(false))]);
        render_case("micropipette-reading", "p1000", &[("size", s("1000 µL")), ("volume", Value::F(350.0)), ("color", Value::B(false)), ("size_label", Value::B(false))]);
        render_case("micropipette-reading", "p200", &[("size", s("200 µL")), ("volume", Value::F(150.0))]);
        for ex in examples() { render_case("micropipette-reading", "example", &ex.params); }
    }
}
