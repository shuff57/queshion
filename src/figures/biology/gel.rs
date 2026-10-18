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

// ---------------------------------------------------------------- lane cards
//
// The side panel edits the lanes as cards (label, bands or ladder choice, reorder, delete, add
// Sample / Ladder). The cards read and write the same `lanes` text the generator parses, so saved
// figures keep rendering identically; a classic setup is copied into `lanes` on the first card
// edit (and `setup` goes back to "Use the lanes text").

/// The ladders a ladder lane can show, as named in its dropdown.
pub const LADDERS: [&str; 3] = ["100 bp ladder", "1 kb ladder", "λ DNA / HindIII"];
/// Most lanes the gel draws.
pub const MAX_LANES: usize = 16;

/// One lane card: its label and either sample bands (the typed text) or a ladder (index into `LADDERS`).
#[derive(Clone, Debug, PartialEq)]
pub enum LaneKind { Sample(String), Ladder(usize) }

#[derive(Clone, Debug, PartialEq)]
pub struct LaneCard { pub label: String, pub kind: LaneKind }

impl LaneCard {
    pub fn sample() -> Self { LaneCard { label: String::new(), kind: LaneKind::Sample(String::new()) } }
    pub fn ladder() -> Self { LaneCard { label: "Ladder".into(), kind: LaneKind::Ladder(1) } }
}

fn ladder_index(rest: &str) -> usize {
    let (name, _) = ladder_bands(rest);
    LADDERS.iter().position(|l| *l == name).unwrap_or(1)
}

/// The lanes as cards (from the classic setup when one is chosen, else the typed lanes text).
pub fn lane_cards(p: &Params) -> Vec<LaneCard> {
    active_text(p)
        .split(['|', '\n'])
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (label, rest) = l.split_once(':').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((l, ""));
            let kind = if rest.to_lowercase().starts_with("ladder") { LaneKind::Ladder(ladder_index(rest)) } else { LaneKind::Sample(rest.to_string()) };
            LaneCard { label: label.to_string(), kind }
        })
        .collect()
}

/// Writes the cards back as the lanes text (and leaves any classic setup).
pub fn set_lane_cards(p: &mut Params, lanes: &[LaneCard]) {
    let clean = |s: &str, extra: char| s.chars().map(|c| if c == '|' || c == '\n' || c == extra { ' ' } else { c }).collect::<String>().trim().to_string();
    let text = lanes
        .iter()
        .take(MAX_LANES)
        .map(|l| {
            let label = clean(&l.label, ':');
            match &l.kind {
                LaneKind::Ladder(i) => format!("{label}: ladder {}", ["100bp", "1kb", "lambda"][(*i).min(2)]),
                LaneKind::Sample(b) => format!("{label}: {}", clean(b, '|')).trim_end().to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" | ");
    p.set_s("lanes", &text);
    p.set_s("setup", SETUPS[0]);
}

/// The classic setups offered on the Lanes card (without the "use the lanes text" entry).
pub fn setup_names() -> &'static [&'static str] { &SETUPS[1..] }

/// The chosen classic setup, if any (also when the typed lanes are exactly a setup's, as the
/// opening figure's are).
pub fn current_setup(p: &Params) -> Option<&'static str> {
    SETUPS[1..].iter().copied().find(|s| *s == p.s("setup") || (p.s("setup") == SETUPS[0] && setup_text(s) == Some(p.s("lanes").trim())))
}

/// One line under the Classic setups dropdown.
pub fn setup_note(name: Option<&str>) -> &'static str {
    match name {
        Some("Crime scene") => "DNA from a crime scene beside three suspects’; Suspect 2’s bands match.",
        Some("Paternity test") => "Every band in the child comes from the mother or the father; Father A’s bands fill the gaps.",
        Some("Restriction digest") => "Lambda DNA uncut and cut by three restriction enzymes, beside a λ HindIII ladder.",
        Some("PCR products") => "PCR products beside a 100 bp ladder, with a faint primer band in the negative control.",
        _ => "Replaces the lanes with a ready-made experiment. Undo brings yours back.",
    }
}

/// "Ladder, Crime scene, Suspect 1" for the Lanes section header.
pub fn lanes_summary(p: &Params) -> String {
    lane_cards(p).iter().map(|l| if l.label.is_empty() { "Sample".to_string() } else { l.label.clone() }).collect::<Vec<_>>().join(", ")
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
        ParamDef::choice("lane_labels", "Lane labels", &["Names", "Blank lines", "None"], "Names"),
        ParamDef::bool("lane_numbers", "Lane numbers", false),
        ParamDef::choice("ladder_sizes", "Ladder sizes", &["Sizes", "Blank lines", "None"], "Sizes"),
        ParamDef::choice("size_units", "Ladder size units", &["bp", "kb"], "bp"),
        ParamDef::choice("electrode_style", "Electrodes", &["Signs", "Named", "Blank", "None"], "Signs"),
        ParamDef::choice("answer_kind", "Answer key lists", &["Matching lanes", "Band sizes", "Both"], "Matching lanes"),
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
        ("100 bp ladder", vec![(1517.0, 1.0), (1200.0, 1.0), (1000.0, 1.6), (900.0, 0.8), (800.0, 0.8), (700.0, 0.7), (600.0, 0.6), (517.0, 1.0), (500.0, 1.0), (400.0, 1.0), (300.0, 0.8), (200.0, 0.7), (100.0, 1.0)])
    } else if k.contains("lam") || k.contains('λ') || k.contains("hind") {
        ("λ DNA / HindIII", vec![(23130.0, 1.6), (9416.0, 1.2), (6557.0, 1.0), (4361.0, 0.8), (2322.0, 0.6), (2027.0, 0.6), (564.0, 0.35), (125.0, 0.3)])
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

/// Ladder-size label text in the chosen units ("8000 bp", "10,000 bp", "1.5 kb").
fn size_label(bp: f64, units: &str) -> String {
    if units == "kb" {
        // three significant figures, like 23.1 kb, 9.42 kb, 0.564 kb
        let kb = bp / 1000.0;
        let dec = (2 - kb.abs().max(1e-9).log10().floor() as i32).clamp(0, 4) as u32;
        let f = 10f64.powi(dec as i32);
        format!("{} kb", fnum((kb * f).round() / f))
    } else if bp >= 10000.0 {
        format!("{} bp", commas(bp.round() as i64))
    } else {
        format!("{} bp", bp.round() as i64)
    }
}

fn plain_sizes(bands: &[(f64, f64)]) -> String {
    bands.iter().map(|(bp, _)| format!("{}", bp.round() as i64)).collect::<Vec<_>>().join(", ")
}

/// Rough text width in px for the sans face at `size`.
fn tw(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.56 }

pub fn render(p: &Params) -> String {
    let text = setup_text(p.s("setup")).unwrap_or(p.s("lanes")).to_string();
    let mut lanes = parse_lanes(&text);
    if lanes.is_empty() { lanes.push(Lane { label: "Sample".into(), ladder: None, bands: vec![] }); }
    let agar = p.s("agarose").to_string();
    let look = p.s("look").to_string();
    // effective modes (the old bools still switch a feature off entirely)
    let lmode = if !p.b("lane_names") { "None" } else { match p.s("lane_labels") { "Blank lines" => "Blank lines", "None" => "None", _ => "Names" } };
    let smode = if !p.b("sizes") { "None" } else { match p.s("ladder_sizes") { "Blank lines" => "Blank lines", "None" => "None", _ => "Sizes" } };
    let emode = if !p.b("electrodes") { "None" } else { match p.s("electrode_style") { "Named" => "Named", "Blank" => "Blank", "None" => "None", _ => "Signs" } };
    let units = if p.s("size_units") == "kb" { "kb" } else { "bp" };
    let numbers = p.b("lane_numbers");
    let (gel_fill, band, glow, well_fill, well_stroke) = match look.as_str() {
        "Blue stain" => ("#e3ecfb", "#1e3a8a", false, "#fff", INK),
        "Glowing" => ("#1b1d22", "#fffaf0", true, "#2c3039", "#5b6170"),
        _ => ("#f6f6f6", "#111111", false, "#fff", INK),
    };
    let title = p.s("title").trim().to_string();
    let n = lanes.len();
    let lane_w = 56.0;
    // ladder lanes whose sizes get written: first lane (left side) and last lane (right side)
    let left_ladder = if smode != "None" && lanes[0].ladder.is_some() { Some(0usize) } else { None };
    let right_ladder = if smode != "None" && n > 1 && lanes[n - 1].ladder.is_some() { Some(n - 1) } else { None };
    let label_w = |li: Option<usize>| -> f64 {
        let Some(i) = li else { return 0.0 };
        if smode == "Blank lines" { return 96.0; }
        lanes[i].bands.iter().map(|(bp, _)| tw(&size_label(*bp, units), 13.0)).fold(0.0, f64::max) + 56.0
    };
    let name_len = if lmode == "Names" { lanes.iter().map(|l| l.label.chars().count()).max().unwrap_or(0) } else { 0 };
    // short names sit level above their lanes; long ones are turned 45 degrees
    let flat = lmode == "Names" && (name_len as f64) * 8.6 <= lane_w + 6.0;
    let name_h = match lmode {
        "Names" if flat => 26.0,
        "Names" if name_len > 0 => (name_len as f64 * 8.4 * 0.72 + 26.0).min(150.0),
        "Blank lines" => 30.0,
        _ => 10.0,
    } + if numbers { 24.0 } else { 0.0 };
    let elec_w = match emode { "None" => 0.0, "Named" => 104.0, _ => 40.0 };
    let left = 24.0 + label_w(left_ladder).max(elec_w);
    let ruler_w = if p.b("ruler") { 64.0 } else { 0.0 };
    let mut right = 24.0 + label_w(right_ladder) + ruler_w;
    if lmode == "Names" && !flat {
        // the last rotated name leans right past the gel edge
        let last = lanes[n - 1].label.chars().count() as f64 * 8.6 * 0.72 + 14.0;
        right = right.max(last - 40.0);
    }
    let gel_w = n as f64 * lane_w + 24.0;
    let run = p.f("run_length").clamp(250.0, 520.0);
    let gel_h = run + 40.0;
    let top = 20.0 + if title.is_empty() { 0.0 } else { 36.0 } + name_h;
    let gy = top;
    let well_y = gy + 18.0;
    // answer key
    let kind = p.s("answer_kind");
    let mut answer_lines: Vec<String> = Vec::new();
    if p.b("answer") {
        if kind == "Band sizes" || kind == "Both" {
            for (i, l) in lanes.iter().enumerate() {
                if l.bands.is_empty() { continue; }
                if l.ladder.is_some() && smode != "Blank lines" { continue; }
                let name = if lmode == "Names" && !l.label.trim().is_empty() { l.label.clone() } else { format!("Lane {}", i + 1) };
                answer_lines.push(format!("{name}: {} bp", plain_sizes(&l.bands)));
            }
        }
        if kind != "Band sizes" {
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
            answer_lines.extend(v);
        }
    }
    let ans_w = answer_lines.iter().map(|l| tw(l, 15.0)).fold(0.0, f64::max) + 40.0;
    let w = (left + gel_w + right).max(420.0).max(ans_w);
    let gx = left + (w - left - gel_w - right) / 2.0;
    let h = gy + gel_h + 30.0 + if answer_lines.is_empty() { 0.0 } else { 14.0 + answer_lines.len() as f64 * 22.0 };
    LAST.with(|l| *l.borrow_mut() = Layout {
        w, gx, gy, well_y, run, lane_w, agar: agar.clone(),
        lanes: lanes.iter().map(|l| LaneInfo { ladder: l.ladder.is_some(), bands: l.bands.clone() }).collect(),
    });
    let mut d = Doc::new(w, h);
    if glow {
        d.raw(r#"<defs><filter id="glow" x="-50%" y="-200%" width="200%" height="500%"><feGaussianBlur stdDeviation="2.6"/></filter></defs>"#);
    }
    if !title.is_empty() { bold(&mut d, w / 2.0, 32.0, 20.0, "middle", &title); }
    d.raw(&format!(r##"<rect x="{gx:.1}" y="{gy:.1}" width="{gel_w:.1}" height="{gel_h:.1}" rx="7" fill="{gel_fill}" stroke="{}" stroke-width="2"/>"##, if glow { gel_fill } else { INK }));
    let lane_x = |i: usize| gx + 12.0 + lane_w * (i as f64 + 0.5);
    let bh = p.f("band_height").clamp(0.5, 2.5);
    for (i, l) in lanes.iter().enumerate() {
        let cx = lane_x(i);
        d.raw(&format!(r#"<rect x="{:.1}" y="{:.1}" width="36" height="10" rx="1.5" fill="{well_fill}" stroke="{well_stroke}" stroke-width="1.3"/>"#, cx - 18.0, well_y - 5.0));
        let num_y = gy - 10.0;
        let lab_y = if numbers { gy - 34.0 } else { gy - 10.0 };
        if numbers {
            txt(&mut d, cx, num_y, 14.0, "middle", SANS, "", &(i + 1).to_string());
        }
        match lmode {
            "Names" if flat => bold(&mut d, cx, lab_y, 15.0, "middle", &l.label),
            "Names" => {
                let (tx, ty) = (cx + 4.0, lab_y);
                txt(&mut d, tx, ty, 14.0, "start", SANS, &format!(r#"font-weight="bold" transform="rotate(-45 {tx:.1} {ty:.1})""#), &l.label);
            }
            "Blank lines" => { d.line(cx - 24.0, lab_y - 6.0, cx + 24.0, lab_y - 6.0, INK, 1.4); }
            _ => {}
        }
        // bands, merging those too close for the gel to separate into one thicker band
        let mut bs: Vec<(f64, f64, f64)> = l.bands.iter().map(|(bp, amt)| {
            let y = well_y + 8.0 + run_frac(*bp, &agar) * run;
            let th = (3.2 + 1.6 * amt.min(3.0)) * bh;
            (y - th / 2.0, y + th / 2.0, *amt)
        }).collect();
        bs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut merged: Vec<(f64, f64, f64)> = Vec::new();
        for b in bs {
            match merged.last_mut() {
                Some(m) if b.0 < m.1 + 0.5 => { m.1 = m.1.max(b.1); m.2 += b.2; }
                _ => merged.push(b),
            }
        }
        for (y0, y1, amt) in merged {
            let th = y1 - y0;
            let op = (0.4 + 0.55 * amt).min(1.0);
            let rx = (th / 2.0).min(3.0);
            if glow {
                d.raw(&format!(r##"<rect x="{:.1}" y="{:.1}" width="40" height="{:.1}" rx="{rx:.1}" fill="#ffcf7a" opacity="{:.2}" filter="url(#glow)"/>"##, cx - 20.0, y0 - 1.0, th + 2.0, (0.35 + 0.4 * amt).min(0.9)));
            }
            d.raw(&format!(r#"<rect x="{:.1}" y="{y0:.1}" width="34" height="{th:.1}" rx="{rx:.1}" fill="{band}" opacity="{op:.2}"/>"#, cx - 17.0));
        }
    }
    // ladder size labels beside a ladder in the first / last lane
    for (li, side_left) in [(left_ladder, true), (right_ladder, false)] {
        let Some(li) = li else { continue };
        let mut bands = lanes[li].bands.clone();
        bands.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let edge = if side_left { gx } else { gx + gel_w };
        let sgn = if side_left { -1.0 } else { 1.0 };
        // group bands that land on (almost) the same spot: one label "500/517 bp"
        let mut groups: Vec<(f64, Vec<f64>)> = Vec::new();
        for (bp, _) in bands {
            let y = well_y + 8.0 + run_frac(bp, &agar) * run;
            match groups.last_mut() {
                Some(g) if (y - g.0).abs() < 3.0 => g.1.push(bp),
                _ => groups.push((y, vec![bp])),
            }
        }
        if smode == "Blank lines" {
            let mut last = f64::MIN;
            for (y, _) in &groups {
                if y - last < 9.0 { continue; }
                last = *y;
                d.line(edge + sgn * 3.0, *y, edge + sgn * 92.0, *y, INK, 1.2);
            }
            continue;
        }
        // spread labels at least `gap` apart, then draw angled leaders to the bands
        let gap = 15.0;
        let ys: Vec<f64> = groups.iter().map(|g| g.0).collect();
        let mut ly = ys.clone();
        for _ in 0..60 {
            let mut moved = false;
            for i in 1..ly.len() {
                let over = gap - (ly[i] - ly[i - 1]);
                if over > 0.01 {
                    ly[i - 1] -= over / 2.0;
                    ly[i] += over / 2.0;
                    moved = true;
                }
            }
            if !moved { break; }
        }
        for (i, (y, bps)) in groups.iter().enumerate() {
            let mut v = bps.clone();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let lab = if v.len() == 1 {
                size_label(v[0], units)
            } else {
                let unit = if units == "kb" { " kb" } else { " bp" };
                let parts: Vec<String> = v.iter().map(|bp| size_label(*bp, units).trim_end_matches(unit).to_string()).collect();
                format!("{}{unit}", parts.join("/"))
            };
            txt(&mut d, edge + sgn * 22.0, ly[i] + 4.5, 13.0, if side_left { "end" } else { "start" }, SANS, "", &lab);
            if (ly[i] - y).abs() < 0.5 {
                d.line(edge + sgn * 16.0, *y, edge + sgn * 4.0, *y, INK, 1.2);
            } else {
                d.polyline(&[(edge + sgn * 16.0, ly[i]), (edge + sgn * 9.0, *y), (edge + sgn * 4.0, *y)], INK, 1.2);
            }
        }
    }
    if emode != "None" {
        let ex = gx - 22.0;
        let (ty, by) = (well_y, gy + gel_h - 22.0);
        if emode == "Blank" {
            d.circle(ex, ty, 12.0, "none", INK, 1.8);
            d.circle(ex, by, 12.0, "none", "#dc2626", 1.8);
        } else {
            d.circle(ex, ty, 12.0, INK, INK, 1.0);
            d.line(ex - 6.0, ty, ex + 6.0, ty, "#fff", 2.6);
            d.circle(ex, by, 12.0, "#dc2626", "#dc2626", 1.0);
            d.line(ex - 6.0, by, ex + 6.0, by, "#fff", 2.6);
            d.line(ex, by - 6.0, ex, by + 6.0, "#fff", 2.6);
            if emode == "Named" {
                bold(&mut d, ex - 18.0, ty + 6.0, 16.0, "end", "Cathode");
                bold(&mut d, ex - 18.0, by + 6.0, 16.0, "end", "Anode");
            }
        }
    }
    if p.b("ruler") {
        let rx = gx + gel_w + 14.0 + label_w(right_ladder);
        let mm = 5.0; // px per mm: 80 mm over the default run
        let nmm = (run / mm) as i64;
        d.rect(rx, well_y - 12.0, 34.0, nmm as f64 * mm + 24.0, "#fff", INK, 1.3);
        for i in 0..=nmm {
            let y = well_y + i as f64 * mm;
            let len = if i % 10 == 0 { 12.0 } else if i % 5 == 0 { 8.0 } else { 5.0 };
            d.line(rx, y, rx + len, y, INK, 1.0);
            if i % 10 == 0 { txt(&mut d, rx + 21.0, y + 4.5, 12.0, "middle", SANS, "", &(i / 10).to_string()); }
        }
        txt(&mut d, rx + 17.0, well_y + nmm as f64 * mm + 30.0, 13.0, "middle", SANS, "", "cm");
    }
    for (i, l) in answer_lines.iter().enumerate() {
        let y = gy + gel_h + 36.0 + i as f64 * 22.0;
        if i == 0 { bold(&mut d, w / 2.0, y - 8.0, 14.0, "middle", "Answer key"); }
        txt(&mut d, w / 2.0, y + 14.0, 15.0, "middle", SANS, "", l);
    }
    d.finish()
}

/// "Start from an example" entries (mirroring the reference gallery).
pub fn examples() -> Vec<Example> {
    let st = |v: &str| Value::S(v.into());
    vec![
        Example { name: "DNA fingerprinting gel: crime scene and three suspects", params: vec![("setup", st("Crime scene")), ("look", st("Glowing"))] },
        Example { name: "Paternity test gel: mother, child and two possible fathers", params: vec![
            ("setup", st(SETUPS[0])),
            ("lanes", st("Ladder: ladder 1kb | Mother: 4800, 2900, 1600, 800 | Child: 4800, 3200, 1600, 650 | Father 1: 5600, 3500, 2200, 1100 | Father 2: 4400, 3200, 1900, 650")),
            ("look", st("Blue stain")),
            ("answer_kind", st("Band sizes")),
        ] },
        Example { name: "PTC taster gene PCR and HaeIII digest gel with a 100 bp ladder", params: vec![
            ("setup", st(SETUPS[0])),
            ("lanes", st("Ladder: ladder 100bp | Uncut: 221 | TT: 177, 44 x0.3 | Tt: 221, 177, 44 x0.3 | tt: 221")),
            ("agarose", st("2%")),
            ("lane_numbers", Value::B(true)),
        ] },
        Example { name: "Lambda DNA restriction digest gel with a ruler for a standard curve", params: vec![
            ("setup", st(SETUPS[0])),
            ("lanes", st("Ladder: ladder lambda | Uncut: 48502 x4 | EcoRI: 21226 x3.5, 7421 x1.2, 5804, 5643, 4878 x0.8, 3530 x0.6 | BamHI: 16841 x2.7, 7233 x1.2, 6770 x1.1, 6527 x1.1, 5626 x0.9, 5505 x0.9")),
            ("agarose", st("0.8%")),
            ("size_units", st("kb")),
            ("ruler", Value::B(true)),
        ] },
        Example { name: "Plasmid restriction digest gel worksheet: draw the double digest", params: vec![
            ("setup", st(SETUPS[0])),
            ("lanes", st("Ladder: ladder 1kb | EcoRI: 5000 | HindIII: 3200, 1800 | Both:")),
            ("ladder_sizes", st("Blank lines")),
            ("electrode_style", st("Blank")),
            ("answer", Value::B(true)),
            ("answer_kind", st("Band sizes")),
        ] },
    ]
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

    /// The lane cards read and write the lanes text: a round trip draws the same gel for every
    /// setup and example, and card edits (reorder, delete, add, ladder choice) land in the text.
    #[test]
    fn lane_cards_round_trip_and_edit() {
        let defs = params();
        let mut cases: Vec<Params> = Vec::new();
        for setup in SETUPS {
            let mut p = Params::defaults(&defs);
            p.set_s("setup", setup);
            cases.push(p);
        }
        for ex in examples() {
            let mut p = Params::defaults(&defs);
            for (k, v) in &ex.params { p.0.insert(k.to_string(), v.clone()); }
            cases.push(p);
        }
        let mut odd = Params::defaults(&defs);
        odd.set_s("lanes", "A:500|B:  450 x2 , 1.5 kb\nL: ladder 100 bp\nNoColon");
        cases.push(odd);
        for p in &cases {
            let before = render(p);
            let cards = lane_cards(p);
            let mut q = p.clone();
            set_lane_cards(&mut q, &cards);
            assert_eq!(q.s("setup"), SETUPS[0]);
            assert_eq!(render(&q), before, "round trip changed the gel: {}", p.s("lanes"));
            assert_eq!(lane_cards(&q), cards);
        }
        // a classic setup becomes editable cards (migrated into the lanes text on edit)
        let mut p = Params::defaults(&defs);
        p.set_s("setup", "Paternity test");
        assert_eq!(current_setup(&p), Some("Paternity test"));
        let mut c = lane_cards(&p);
        assert_eq!(c.len(), 5);
        assert_eq!(c[0], LaneCard { label: "Ladder".into(), kind: LaneKind::Ladder(0) });
        assert_eq!(c[1].kind, LaneKind::Sample("800, 500, 300".into()));
        c.swap(1, 2); // Child before Mother
        c.remove(4); // drop Father B
        c.push(LaneCard::sample());
        c.push(LaneCard::ladder());
        c[0].kind = LaneKind::Ladder(2);
        set_lane_cards(&mut p, &c);
        assert_eq!(current_setup(&p), None);
        assert_eq!(p.s("lanes"), "Ladder: ladder lambda | Child: 800, 600, 300 | Mother: 800, 500, 300 | Father A: 700, 600, 200 | : | Ladder: ladder 1kb");
        let l = layout_of(&p);
        assert_eq!(l.lanes.len(), 6);
        assert!(l.lanes[0].ladder && l.lanes[5].ladder && !l.lanes[4].ladder);
        assert!(l.lanes[4].bands.is_empty());
        assert_eq!(lanes_summary(&p), "Ladder, Child, Mother, Father A, Sample, Ladder");
        // separators typed into a label or bands can't split a lane
        set_lane_cards(&mut p, &[LaneCard { label: "a|b:c".into(), kind: LaneKind::Sample("500 | 300".into()) }]);
        assert_eq!(lane_cards(&p), vec![LaneCard { label: "a b c".into(), kind: LaneKind::Sample("500   300".into()) }]);
        // never more lanes than the gel draws
        set_lane_cards(&mut p, &vec![LaneCard::ladder(); 20]);
        assert_eq!(lane_cards(&p).len(), MAX_LANES);
    }

    #[test]
    fn variants() {
        render_case("gel-electrophoresis", "default", &[]);
        render_case("gel-electrophoresis", "answer_glow", &[("look", s("Glowing")), ("answer", Value::B(true)), ("ruler", Value::B(true)), ("title", s("Who was at the scene?"))]);
        render_case("gel-electrophoresis", "paternity", &[("setup", s("Paternity test")), ("agarose", s("2%")), ("look", s("Blue stain"))]);
        render_case("gel-electrophoresis", "digest", &[("setup", s("Restriction digest")), ("agarose", s("0.8%"))]);
        render_case("gel-electrophoresis", "bare", &[("lane_names", Value::B(false)), ("sizes", Value::B(false)), ("electrodes", Value::B(false))]);
        render_case("gel-electrophoresis", "modes", &[("lane_labels", s("Blank lines")), ("lane_numbers", Value::B(true)), ("ladder_sizes", s("Blank lines")), ("electrode_style", s("Named")), ("answer", Value::B(true)), ("answer_kind", s("Both")), ("size_units", s("kb"))]);
        render_case("gel-electrophoresis", "right_ladder", &[("lanes", s("A: 500, 300 | B: 450 | L: ladder 100bp")), ("ruler", Value::B(true))]);
        for ex in examples() {
            let over: Vec<(&str, Value)> = ex.params.clone();
            render_case("gel-electrophoresis", "example", &over);
        }
    }
}
