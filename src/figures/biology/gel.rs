//! Gel electrophoresis generator.
//!
//! Lanes syntax (one line, lanes separated by `|` or new lines):
//!   `Label: ladder 1kb`          ladder lane (`1kb`, `100bp`, `lambda`)
//!   `Label: 5200, 3100 x2, 1.5 kb, 450 x0.5`   sample lane; sizes in bp (or kb), `xN` = relative amount
use super::*;

const SETUPS: [&str; 5] = ["Use the lanes text", "Crime scene", "Paternity test", "Restriction digest", "PCR products"];

/// Geometry of the last render.
#[derive(Clone, Debug, Default)]
pub struct Layout { pub w: f64, pub gx: f64, pub gy: f64, pub well_y: f64, pub run: f64, pub lane_w: f64, pub agar: String, pub lanes: Vec<LaneInfo> }
#[derive(Clone, Debug, Default)]
pub struct LaneInfo { pub ladder: bool, pub bands: Vec<(f64, f64)> }

thread_local! {
    static LAST: std::cell::RefCell<Layout> = std::cell::RefCell::new(Layout::default());
}

pub fn layout_of(p: &Params) -> Layout {
    LAST.with(|l| *l.borrow_mut() = Layout::default());
    let _ = render(p);
    LAST.with(|l| l.borrow().clone())
}

pub fn lane_x(l: &Layout, i: usize) -> f64 { l.gx + 12.0 + l.lane_w * (i as f64 + 0.5) }
pub fn band_y(l: &Layout, bp: f64) -> f64 { l.well_y + 8.0 + run_frac(bp, &l.agar) * l.run }

/// Fragment size whose band sits at screen y (inverse of `band_y`).
pub fn bp_at(l: &Layout, y: f64) -> f64 {
    let (lo, hi): (f64, f64) = match l.agar.as_str() { "0.8%" => (800.0, 15000.0), "1.5%" => (300.0, 5000.0), "2%" => (100.0, 2500.0), _ => (500.0, 10000.0) };
    let frac = ((y - l.well_y - 8.0) / l.run).clamp(0.05, 0.95);
    let t = (frac - 0.12) / (0.76 - 0.12);
    10f64.powf(hi.log10() - t * (hi.log10() - lo.log10()))
}

/// The lanes text actually drawn (a classic setup overrides the typed text).
fn active_text(p: &Params) -> String {
    setup_text(p.s("setup")).unwrap_or(p.s("lanes")).to_string()
}

/// Edit band `band` (index among parsed bands) of sample lane `lane` (index among drawn lanes).
/// `f` gets (bp, amount) and returns the new pair; the text is only rewritten when it really
/// changed (>0.3% in size or a different amount), so a no-op drag leaves the params untouched.
pub fn edit_band(p: &mut Params, lane: usize, band: usize, f: impl Fn(f64, f64) -> (f64, f64)) {
    let text = active_text(p);
    let mut raw: Vec<String> = Vec::new();
    let mut drawn = Vec::new(); // raw index per drawn lane
    for (i, l) in text.split(['|', '\n']).enumerate() {
        raw.push(l.trim().to_string());
        if !l.trim().is_empty() { drawn.push(i); }
    }
    let Some(&ri) = drawn.get(lane) else { return };
    let line = raw[ri].clone();
    let Some((label, rest)) = line.split_once(':') else { return };
    let mut toks: Vec<String> = rest.split(',').map(|t| t.trim().to_string()).collect();
    let parsed: Vec<usize> = (0..toks.len()).filter(|i| parse_band(&toks[*i]).is_some()).collect();
    let Some(&ti) = parsed.get(band) else { return };
    let (bp, amt) = parse_band(&toks[ti]).unwrap();
    let (nbp, namt) = f(bp, amt);
    let namt = namt.clamp(0.1, 4.0);
    if ((nbp - bp).abs() / bp.max(1.0) < 0.003) && (namt - amt).abs() < 1e-6 { return; }
    let nbp = if nbp >= 1000.0 { (nbp / 10.0).round() * 10.0 } else { (nbp / 5.0).round() * 5.0 }.max(10.0);
    toks[ti] = if (namt - 1.0).abs() < 1e-6 { format!("{}", nbp as i64) } else { format!("{} x{}", nbp as i64, fnum(namt)) };
    raw[ri] = format!("{}: {}", label.trim(), toks.join(", "));
    let joined = raw.iter().filter(|r| !r.is_empty()).cloned().collect::<Vec<_>>().join(" | ");
    p.set_s("lanes", &joined);
    p.set_s("setup", SETUPS[0]);
}

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("setup", "Classic setup (overrides lanes text)", &SETUPS, SETUPS[0]),
        ParamDef::long("lanes", "Lanes: Label: bands | Label: ladder 1kb | ...", CRIME),
        ParamDef::choice("agarose", "Agarose", &["0.8%", "1%", "1.5%", "2%"], "1%"),
        ParamDef::choice("look", "Look", &["Printable", "Blue stain", "Glowing"], "Printable"),
        ParamDef::bool("ruler", "Ruler for measuring how far each band ran", false),
        ParamDef::bool("lane_names", "Lane names", true),
        ParamDef::bool("sizes", "Ladder sizes", true),
        ParamDef::bool("electrodes", "Electrode signs", true),
        ParamDef::bool("answer", "Answer key (matching lanes)", false),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::float("band_height", "Band thickness", 0.5, 2.5, 0.1, 1.0),
        ParamDef::float("run_length", "Gel length (px of run)", 250.0, 520.0, 10.0, 400.0),
    ]
}

const CRIME: &str = "Ladder: ladder 1kb | Crime scene: 5200, 3100, 1800, 900 | Suspect 1: 6000, 3100, 1500, 700 | Suspect 2: 5200, 3100, 1800, 900 | Suspect 3: 4200, 2400, 1800, 1100";

fn setup_text(s: &str) -> Option<&'static str> {
    Some(match s {
        "Crime scene" => CRIME,
        "Paternity test" => "Ladder: ladder 100bp | Mother: 800, 500, 300 | Child: 800, 600, 300 | Father A: 700, 600, 200 | Father B: 900, 400, 250",
        "Restriction digest" => "Ladder: ladder lambda | Uncut: 23000 | EcoRI: 21200, 5100, 4300 | HindIII: 23130, 9416, 6557, 4361, 2322, 2027 | BamHI: 16800, 7200, 5500",
        "PCR products" => "Ladder: ladder 100bp | Negative: 100 x0.3 | Sample 1: 500 | Sample 2: 300 | Sample 3: 500, 300",
        _ => return None,
    })
}

#[derive(Clone)]
struct Lane {
    label: String,
    ladder: Option<&'static str>,
    bands: Vec<(f64, f64)>,
}

fn ladder_bands(kind: &str) -> (&'static str, Vec<(f64, f64)>) {
    let k = kind.to_lowercase();
    if k.contains("100") {
        ("100 bp ladder", vec![(1500.0, 0.7), (1000.0, 1.0), (900.0, 1.0), (800.0, 1.0), (700.0, 1.0), (600.0, 1.0), (500.0, 2.0), (400.0, 1.0), (300.0, 1.0), (200.0, 1.0), (100.0, 1.0)])
    } else if k.contains("lam") || k.contains('λ') || k.contains("hind") {
        ("λ DNA / HindIII", vec![(23130.0, 1.0), (9416.0, 1.0), (6557.0, 1.0), (4361.0, 1.0), (2322.0, 0.8), (2027.0, 0.8), (564.0, 0.4)])
    } else {
        ("1 kb ladder", vec![(10000.0, 1.0), (8000.0, 1.0), (6000.0, 1.4), (5000.0, 1.0), (4000.0, 0.6), (3000.0, 2.0), (2000.0, 1.3), (1500.0, 0.6), (1000.0, 1.3), (500.0, 0.5)])
    }
}

fn parse_band(tok: &str) -> Option<(f64, f64)> {
    let t = tok.trim().to_lowercase();
    if t.is_empty() { return None; }
    let (mut num, mut amt) = (t.clone(), 1.0);
    if let Some(ix) = t.rfind('x') {
        if let Ok(a) = t[ix + 1..].trim().parse::<f64>() {
            if ix > 0 { amt = a; num = t[..ix].trim().to_string(); }
        }
    }
    let mult = if num.ends_with("kb") { 1000.0 } else { 1.0 };
    let n = num.trim_end_matches("kb").trim_end_matches("bp").trim().replace(',', "");
    let v: f64 = n.parse().ok()?;
    (v > 0.0).then_some((v * mult, amt.clamp(0.1, 4.0)))
}

fn parse_lanes(s: &str) -> Vec<Lane> {
    let mut out = Vec::new();
    for l in s.split(['|', '\n']) {
        let l = l.trim();
        if l.is_empty() { continue; }
        let (label, rest) = l.split_once(':').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((l, ""));
        if rest.to_lowercase().starts_with("ladder") {
            let (name, bands) = ladder_bands(rest);
            out.push(Lane { label: label.into(), ladder: Some(name), bands });
        } else {
            let bands = rest.split(',').filter_map(parse_band).collect();
            out.push(Lane { label: label.into(), ladder: None, bands });
        }
    }
    out.truncate(16);
    out
}

/// Fraction of the run length travelled (0 = well, 1 = bottom).
fn run_frac(bp: f64, agarose: &str) -> f64 {
    let (lo, hi): (f64, f64) = match agarose { "0.8%" => (800.0, 15000.0), "1.5%" => (300.0, 5000.0), "2%" => (100.0, 2500.0), _ => (500.0, 10000.0) };
    let t = (hi.log10() - bp.log10()) / (hi.log10() - lo.log10());
    (0.12 + t * (0.76 - 0.12)).clamp(0.05, 0.95)
}

pub fn render(p: &Params) -> String {
    let text = setup_text(p.s("setup")).unwrap_or(p.s("lanes")).to_string();
    let mut lanes = parse_lanes(&text);
    if lanes.is_empty() { lanes.push(Lane { label: "Sample".into(), ladder: None, bands: vec![] }); }
    let agar = p.s("agarose").to_string();
    let look = p.s("look").to_string();
    let (gel_fill, band, glow, bg_text) = match look.as_str() {
        "Blue stain" => ("#eaf1ff", "#1e3a8a", false, INK),
        "Glowing" => ("#0a0f1e", "#8ff5a8", true, INK),
        _ => ("#f3f3f3", "#111111", false, INK),
    };
    let title = p.s("title").trim().to_string();
    let n = lanes.len();
    let lane_w = 56.0;
    let name_len = if p.b("lane_names") { lanes.iter().map(|l| l.label.chars().count()).max().unwrap_or(0) } else { 0 };
    let name_h = if name_len == 0 { 10.0 } else { (name_len as f64 * 8.4 * 0.72 + 26.0).min(150.0) };
    let left = if p.b("sizes") || p.b("electrodes") { 112.0 } else { 40.0 };
    let right = if p.b("ruler") { 70.0 } else { 36.0 };
    let gel_w = n as f64 * lane_w + 24.0;
    let run = p.f("run_length").clamp(250.0, 520.0);
    let gel_h = run + 40.0;
    let top = 20.0 + if title.is_empty() { 0.0 } else { 36.0 } + name_h;
    let w = (left + gel_w + right).max(420.0);
    let gx = left + (w - left - gel_w - right) / 2.0;
    let gy = top;
    let well_y = gy + 18.0;
    let answer_lines: Vec<String> = if p.b("answer") {
        let samples: Vec<&Lane> = lanes.iter().filter(|l| l.ladder.is_none() && !l.bands.is_empty()).collect();
        let mut v = Vec::new();
        for i in 0..samples.len() {
            for j in i + 1..samples.len() {
                let (a, b) = (samples[i], samples[j]);
                let same = a.bands.len() == b.bands.len() && a.bands.iter().zip(&b.bands).all(|(x, y)| (x.0 - y.0).abs() / x.0.max(1.0) < 0.01);
                if same { v.push(format!("{} matches {}", a.label, b.label)); }
            }
        }
        if v.is_empty() { v.push("No lanes match.".into()); }
        v
    } else { vec![] };
    let h = gy + gel_h + 30.0 + if answer_lines.is_empty() { 0.0 } else { 14.0 + answer_lines.len() as f64 * 22.0 };
    LAST.with(|l| *l.borrow_mut() = Layout {
        w, gx, gy, well_y, run, lane_w, agar: agar.clone(),
        lanes: lanes.iter().map(|l| LaneInfo { ladder: l.ladder.is_some(), bands: l.bands.clone() }).collect(),
    });
    let mut d = Doc::new(w, h);
    if glow {
        d.raw(r#"<defs><filter id="glow" x="-50%" y="-200%" width="200%" height="500%"><feGaussianBlur stdDeviation="3"/></filter></defs>"#);
    }
    if !title.is_empty() { bold(&mut d, w / 2.0, 32.0, 20.0, "middle", &title); }
    d.raw(&format!(r##"<rect x="{gx:.1}" y="{gy:.1}" width="{gel_w:.1}" height="{gel_h:.1}" rx="7" fill="{gel_fill}" stroke="{INK}" stroke-width="2"/>"##));
    let lane_x = |i: usize| gx + 12.0 + lane_w * (i as f64 + 0.5);
    for (i, l) in lanes.iter().enumerate() {
        let cx = lane_x(i);
        let well_fill = if glow { "#222a44" } else { "#fff" };
        d.rect(cx - 18.0, well_y - 5.0, 36.0, 10.0, well_fill, if glow { "#667" } else { INK }, 1.3);
        if p.b("lane_names") {
            let (tx, ty) = (cx + 4.0, gy - 10.0);
            txt(&mut d, tx, ty, 14.0, "start", SANS, &format!(r#"font-weight="bold" transform="rotate(-45 {tx:.1} {ty:.1})""#), &l.label);
        }
        for (bp, amt) in &l.bands {
            let y = well_y + 8.0 + run_frac(*bp, &agar) * run;
            let th = (3.2 + 1.6 * amt.min(3.0)) * p.f("band_height").clamp(0.5, 2.5);
            let op = (0.4 + 0.55 * amt).min(1.0);
            if glow {
                d.raw(&format!(r#"<rect x="{:.1}" y="{:.1}" width="38" height="{:.1}" fill="{band}" opacity="0.8" filter="url(#glow)"/>"#, cx - 19.0, y - th / 2.0, th + 1.0));
            }
            d.raw(&format!(r#"<rect x="{:.1}" y="{:.1}" width="34" height="{th:.1}" rx="1.5" fill="{band}" opacity="{op:.2}"/>"#, cx - 17.0, y - th / 2.0));
        }
    }
    // ladder size labels from the first ladder lane
    if p.b("sizes") {
        if let Some(l) = lanes.iter().find(|l| l.ladder.is_some()) {
            let mut last = f64::MIN;
            let mut bands = l.bands.clone();
            bands.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            for (bp, _) in bands {
                let y = well_y + 8.0 + run_frac(bp, &agar) * run;
                if y - last < 11.0 { continue; }
                last = y;
                txt(&mut d, gx - 24.0, y + 4.0, 12.0, "end", SANS, "", &format!("{} bp", commas(bp as i64)));
                d.line(gx - 18.0, y, gx - 6.0, y, bg_text, 1.2);
            }
        }
    }
    if p.b("electrodes") {
        let ex = if p.b("sizes") { gx - 90.0 } else { gx - 22.0 };
        d.circle(ex, gy + 22.0, 10.0, INK, INK, 1.0);
        d.line(ex - 5.0, gy + 22.0, ex + 5.0, gy + 22.0, "#fff", 2.4);
        d.circle(ex, gy + gel_h - 22.0, 10.0, "#dc2626", "#dc2626", 1.0);
        d.line(ex - 5.0, gy + gel_h - 22.0, ex + 5.0, gy + gel_h - 22.0, "#fff", 2.4);
        d.line(ex, gy + gel_h - 27.0, ex, gy + gel_h - 17.0, "#fff", 2.4);
    }
    if p.b("ruler") {
        let rx = gx + gel_w + 14.0;
        let mm = 5.0; // px per mm: 80 mm over the run
        d.line(rx, well_y, rx, well_y + run, INK, 1.4);
        for i in 0..=((run / mm) as i64) {
            let y = well_y + i as f64 * mm;
            let len = if i % 10 == 0 { 12.0 } else if i % 5 == 0 { 8.0 } else { 4.0 };
            d.line(rx, y, rx + len, y, INK, 1.0);
            if i % 10 == 0 { txt(&mut d, rx + 16.0, y + 4.0, 11.0, "start", SANS, "", &i.to_string()); }
        }
        txt(&mut d, rx + 6.0, well_y - 10.0, 11.0, "middle", SANS, "", "mm");
    }
    for (i, l) in answer_lines.iter().enumerate() {
        let y = gy + gel_h + 36.0 + i as f64 * 22.0;
        if i == 0 { bold(&mut d, w / 2.0, y - 4.0 - 4.0, 14.0, "middle", "Answer key"); }
        txt(&mut d, w / 2.0, y + 14.0, 14.0, "middle", SANS, "", l);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smaller_runs_farther() {
        assert!(run_frac(500.0, "1%") > run_frac(5000.0, "1%"));
        assert!(run_frac(400.0, "2%") - run_frac(200.0, "2%") != 0.0);
        let spread = |a: &str| run_frac(200.0, a) - run_frac(400.0, a);
        assert!(spread("2%") > spread("0.8%"));
    }

    #[test]
    fn parse() {
        let l = parse_lanes("A: ladder 100bp | B: 450 x2, 1.5 kb, 300 bp x0.5");
        assert_eq!(l.len(), 2);
        assert!(l[0].ladder.is_some());
        assert_eq!(l[1].bands, vec![(450.0, 2.0), (1500.0, 1.0), (300.0, 0.5)]);
    }

    #[test]
    fn variants() {
        render_case("gel-electrophoresis", "default", &[]);
        render_case("gel-electrophoresis", "answer_glow", &[("look", s("Glowing")), ("answer", Value::B(true)), ("ruler", Value::B(true)), ("title", s("Who was at the scene?"))]);
        render_case("gel-electrophoresis", "paternity", &[("setup", s("Paternity test")), ("agarose", s("2%")), ("look", s("Blue stain"))]);
        render_case("gel-electrophoresis", "digest", &[("setup", s("Restriction digest")), ("agarose", s("0.8%"))]);
        render_case("gel-electrophoresis", "bare", &[("lane_names", Value::B(false)), ("sizes", Value::B(false)), ("electrodes", Value::B(false))]);
    }
}
