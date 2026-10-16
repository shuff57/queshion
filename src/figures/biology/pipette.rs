//! Micropipette reading generator: a single-channel pipette whose three-digit display
//! reads the chosen volume. Digit weights depend on the size (P2: 1, 0.1, 0.01 uL;
//! P10/P20: 10, 1, 0.1; P100/P200: 100, 10, 1; P1000: 1000, 100, 10).
use super::*;

const SIZES: [&str; 6] = ["2 µL", "10 µL", "20 µL", "100 µL", "200 µL", "1000 µL"];

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("size", "Micropipette size", &SIZES, "20 µL"),
        ParamDef::float("volume", "Volume it is set to (µL)", 0.0, 1000.0, 0.1, 12.5),
        ParamDef::bool("tip", "Tip on the end", true),
        ParamDef::bool("size_label", "Size printed above the display", true),
        ParamDef::bool("color", "Color (red marks the decimal digit)", true),
        ParamDef::bool("decimal_line", "Decimal line", true),
        ParamDef::bool("answer", "Print the reading under the figure", false),
    ]
}

struct Spec { max: f64, min: f64, unit: f64, red: usize, accent: &'static str, tip: &'static str }

fn spec(size: &str) -> Spec {
    match size {
        "2 µL" => Spec { max: 2.0, min: 0.2, unit: 0.01, red: 2, accent: "#a78bfa", tip: "#e5e7eb" },
        "10 µL" => Spec { max: 10.0, min: 1.0, unit: 0.1, red: 1, accent: "#d1d5db", tip: "#e5e7eb" },
        "100 µL" => Spec { max: 100.0, min: 10.0, unit: 1.0, red: 0, accent: "#facc15", tip: "#fde047" },
        "200 µL" => Spec { max: 200.0, min: 20.0, unit: 1.0, red: 0, accent: "#facc15", tip: "#fde047" },
        "1000 µL" => Spec { max: 1000.0, min: 100.0, unit: 10.0, red: 0, accent: "#60a5fa", tip: "#bfdbfe" },
        _ => Spec { max: 20.0, min: 2.0, unit: 0.1, red: 1, accent: "#facc15", tip: "#fde047" },
    }
}

/// Three digits shown for `volume` on the given pipette.
pub fn digits(size: &str, volume: f64) -> [u8; 3] {
    let s = spec(size);
    let v = volume.clamp(s.min, s.max);
    let units = (v / s.unit).round() as i64;
    [((units / 100) % 10) as u8, ((units / 10) % 10) as u8, (units % 10) as u8]
}

pub fn render(p: &Params) -> String {
    let size = p.s("size").to_string();
    let sp = spec(&size);
    let vol = p.f("volume").clamp(sp.min, sp.max);
    let dg = digits(&size, vol);
    let (w, h) = (320.0, 640.0);
    let cx = 150.0;
    let mut d = Doc::new(w, h);
    let body = "#e5e7eb";
    let color = p.b("color");
    let accent = if color { sp.accent } else { "#d4d4d4" };

    // plunger button + shaft
    d.rect(cx - 12.0, 36.0, 24.0, 14.0, accent, INK, 2.0);
    d.rect(cx - 9.0, 50.0, 18.0, 30.0, "#f3f4f6", INK, 2.0);
    // body with thumb hook
    let b = format!(
        "M{l:.1},470 L{l:.1},92 Q{l:.1},80 {l2:.1},80 L{r1:.1},80 C{r2:.1},68 {r3:.1},74 {r3:.1},106 C{r3:.1},134 {r4:.1},148 {r5:.1},148 C{r6:.1},136 {r7:.1},118 {r:.1},112 L{r:.1},470 Z",
        l = cx - 28.0, l2 = cx - 16.0, r1 = cx + 12.0, r2 = cx + 40.0, r3 = cx + 58.0, r4 = cx + 54.0, r5 = cx + 44.0,
        r6 = cx + 36.0, r7 = cx + 31.0, r = cx + 28.0
    );
    d.path(&b, body, INK, 2.2);
    // collar, neck, shaft
    d.rect(cx - 19.0, 470.0, 38.0, 16.0, "#f3f4f6", INK, 2.0);
    d.poly(&[(cx - 17.0, 486.0), (cx + 17.0, 486.0), (cx + 8.0, 530.0), (cx - 8.0, 530.0)], "#f3f4f6", INK, 2.0);
    d.rect(cx - 7.0, 530.0, 14.0, 42.0, "#f3f4f6", INK, 2.0);
    if p.b("tip") {
        d.rect(cx - 9.0, 568.0, 18.0, 7.0, accent, INK, 1.6);
        d.poly(&[(cx - 8.0, 574.0), (cx + 8.0, 574.0), (cx + 1.5, 600.0), (cx - 1.5, 600.0)], if color { sp.tip } else { "#f5f5f5" }, INK, 1.8);
    }
    // display window: three rows
    let (wx, wy, ww, rh) = (cx - 16.0, 150.0, 32.0, 28.0);
    d.rect(wx, wy, ww, rh * 3.0, "#fff", INK, 2.2);
    for i in 1..3 { d.line(wx, wy + rh * i as f64, wx + ww, wy + rh * i as f64, INK, 1.0); }
    for (i, v) in dg.iter().enumerate() {
        let red = color && sp.red > 0 && i >= 3 - sp.red;
        let col = if red { "#c1121f" } else { INK };
        d.raw(&format!(r##"<text x="{cx:.1}" y="{:.1}" font-size="24" text-anchor="middle" font-family="{SANS}" font-weight="bold" fill="{col}">{v}</text>"##, wy + rh * i as f64 + 22.0));
    }
    if p.b("decimal_line") && sp.red > 0 {
        let ly = wy + rh * (3 - sp.red) as f64;
        d.line(wx - 2.0, ly, wx + ww + 2.0, ly, INK, 3.4);
    }
    if p.b("size_label") {
        bold(&mut d, cx, wy - 10.0, 11.0, "middle", &size);
    }
    if p.b("answer") {
        bold(&mut d, w / 2.0, h - 8.0, 16.0, "middle", &format!("Reading: {} µL", fnum(vol)));
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
