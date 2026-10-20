//! Eval runner for the figure agent: a cases file (`tests/figure_agent_cases.json`) of requests with
//! what a correct figure must contain, run several times each because the model is not deterministic.
//!
//! A case passes when the agent finishes with an acceptable generator and every check holds on the
//! final parameters (or, for `"expect": "decline"`, when it says no generator can make the request).
//! Reference-image cases draw the picture from the registry itself, so the ground truth is exact
//! and no image files are needed.
//!
//! Headless: `VERSHION_FIGURE_EVAL=<cases.json>` [`VERSHION_EVAL_REPEATS`=3] [`VERSHION_EVAL_ONLY`=<name part>]
//! [`VERSHION_EVAL_OUT`=<report.json>]. `validate` checks every case against the live registry so a
//! typo in the cases file fails a test instead of silently failing every model run.

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::figure_agent::{self, AgentRun, Chat};
use crate::figures::Kind;

#[derive(Clone, Debug, Deserialize)]
pub struct Case {
    pub name: String,
    pub level: String,
    /// Text of the request. May be empty for a reference-image case.
    #[serde(default)]
    pub request: String,
    /// Draw this registry figure and hand the picture to the agent as the reference.
    #[serde(default)]
    pub reference: Option<Reference>,
    /// Generators that count as the right choice. Empty means any.
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default)]
    pub expect: Expect,
    #[serde(default)]
    pub checks: Vec<Check>,
}

/// The picture handed to the agent: a registry figure (`id` + `params`, exact ground truth), or a
/// real image `file` (path relative to the cases file), optionally made worse with `degrade`.
#[derive(Clone, Debug, Deserialize)]
pub struct Reference {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub params: Json,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub degrade: Option<Degrade>,
}

/// Makes a clean render look like a photo or scan, in this order: shrink, rotate, darken, blur,
/// noise, JPEG round trip.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Degrade {
    /// Fraction of the original size, 0.2..1.
    pub scale: Option<f32>,
    /// Degrees, counterclockwise.
    pub rotate: Option<f32>,
    /// Brightness multiplier, 0.5..1 (grey paper).
    pub shade: Option<f32>,
    /// Gaussian sigma in pixels.
    pub blur: Option<f32>,
    /// Peak per-channel noise, 0..60.
    pub noise: Option<u8>,
    /// JPEG quality, 5..95.
    pub jpeg: Option<u8>,
}

fn rotate_rgb(img: &image::RgbImage, deg: f32) -> image::RgbImage {
    let (w, h) = img.dimensions();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let (s, c) = deg.to_radians().sin_cos();
    let mut out = image::RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
    for y in 0..h {
        for x in 0..w {
            // Inverse map: where in the source does this destination pixel come from.
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            let (sx, sy) = (cx + dx * c - dy * s, cy + dx * s + dy * c);
            if sx >= 0.0 && sy >= 0.0 && sx < (w - 1) as f32 && sy < (h - 1) as f32 {
                out.put_pixel(x, y, *img.get_pixel(sx.round() as u32, sy.round() as u32));
            }
        }
    }
    out
}

/// Applies `d` to PNG bytes and returns PNG bytes. Deterministic: the noise uses a fixed seed.
pub fn degrade(png: &[u8], d: &Degrade) -> Result<Vec<u8>, String> {
    let mut img = image::load_from_memory(png).map_err(|e| e.to_string())?.to_rgb8();
    if let Some(s) = d.scale {
        let (w, h) = img.dimensions();
        let (nw, nh) = (((w as f32 * s.clamp(0.2, 1.0)) as u32).max(8), ((h as f32 * s.clamp(0.2, 1.0)) as u32).max(8));
        img = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
    }
    if let Some(deg) = d.rotate {
        img = rotate_rgb(&img, deg);
    }
    if let Some(m) = d.shade {
        let m = m.clamp(0.5, 1.0);
        for p in img.pixels_mut() {
            for c in &mut p.0 {
                *c = (*c as f32 * m) as u8;
            }
        }
    }
    if let Some(b) = d.blur {
        img = image::imageops::blur(&img, b.max(0.0));
    }
    if let Some(n) = d.noise {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        for p in img.pixels_mut() {
            for c in &mut p.0 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let r = (state % (2 * n as u64 + 1)) as i32 - n as i32;
                *c = (*c as i32 + r).clamp(0, 255) as u8;
            }
        }
    }
    if let Some(q) = d.jpeg {
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, q.clamp(5, 95)).encode_image(&img).map_err(|e| e.to_string())?;
        img = image::load_from_memory(&buf).map_err(|e| e.to_string())?.to_rgb8();
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The reference picture of a case: a file or a registry render, then degraded if asked.
pub fn reference_png(r: &Reference, base: &std::path::Path) -> Result<Vec<u8>, String> {
    let clean = match (&r.file, &r.id) {
        (Some(f), _) => std::fs::read(base.join(f)).map_err(|e| format!("cannot read reference {f}: {e}"))?,
        (None, Some(id)) => figure_agent::render_png(id, &r.params)?.1,
        (None, None) => return Err("a reference needs an id or a file".to_string()),
    };
    match &r.degrade {
        Some(d) => degrade(&clean, d),
        None => Ok(clean),
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Expect {
    #[default]
    Figure,
    Decline,
    /// Either is right: a figure that passes the checks, or a decline whose reason mentions every
    /// `note_contains` word (used where the honest answer is "this generator cannot do that").
    Either,
}

/// One condition on the final parameters. Every operation that is present must hold.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Check {
    /// Parameter the operations apply to.
    #[serde(default)]
    pub key: String,
    pub eq: Option<Json>,
    pub not_eq: Option<Json>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub one_of: Option<Vec<Json>>,
    /// Text parameter contains this, ignoring case and whitespace.
    pub contains: Option<String>,
    /// Any of `keys` holds each of these values (e.g. the three side parameters hold 3 and 4).
    #[serde(default)]
    pub keys: Vec<String>,
    pub contains_values: Option<Vec<Json>>,
    /// Any of `keys` contains any of these (ignoring case, whitespace and subscript digits), for an
    /// answer that may sit in either of two parameters (a formula typed in, or a preset picked).
    pub any_contains: Option<Vec<String>>,
    /// The model's final note mentions each of these (case-insensitive).
    pub note_contains: Option<Vec<String>>,
}

// ---------- scoring ----------

fn num(v: &Json) -> Option<f64> {
    match v {
        Json::Number(n) => n.as_f64(),
        Json::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Like [`squash`], with subscript digits written as plain digits (`CH₃COCl` becomes `ch3cocl`).
fn plain(s: &str) -> String {
    squash(s).chars().map(|c| if ('\u{2080}'..='\u{2089}').contains(&c) { char::from(b'0' + (c as u32 - 0x2080) as u8) } else { c }).collect()
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect()
}

/// Numbers compare as numbers (`"90"` equals `90`), everything else as text without case or spaces.
fn loose_eq(a: &Json, b: &Json) -> bool {
    match (num(a), num(b)) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => match (a, b) {
            (Json::String(x), Json::String(y)) => squash(x) == squash(y),
            _ => a == b,
        },
    }
}

fn show(v: &Json) -> String {
    v.to_string()
}

fn check_one(c: &Check, params: &serde_json::Map<String, Json>, note: &str) -> Vec<String> {
    let mut bad = Vec::new();
    if let Some(words) = &c.note_contains {
        for w in words {
            if !note.to_lowercase().contains(&w.to_lowercase()) {
                bad.push(format!("the note does not mention {w:?} (note: {note:?})"));
            }
        }
    }
    if let Some(words) = &c.any_contains {
        let texts: Vec<String> = c.keys.iter().filter_map(|k| params.get(k)).map(|v| plain(&v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect();
        if !words.iter().any(|w| texts.iter().any(|t| t.contains(&plain(w)))) {
            bad.push(format!("none of {:?} contains any of {:?}", c.keys, words));
        }
    }
    if let Some(vals) = &c.contains_values {
        for want in vals {
            if !c.keys.iter().any(|k| params.get(k).is_some_and(|v| loose_eq(v, want))) {
                bad.push(format!("none of {:?} holds {}", c.keys, show(want)));
            }
        }
    }
    if c.key.is_empty() {
        return bad;
    }
    let Some(got) = params.get(&c.key) else {
        bad.push(format!("{}: not set", c.key));
        return bad;
    };
    let k = &c.key;
    if let Some(w) = &c.eq {
        if !loose_eq(got, w) {
            bad.push(format!("{k}: wanted {}, got {}", show(w), show(got)));
        }
    }
    if let Some(w) = &c.not_eq {
        if loose_eq(got, w) {
            bad.push(format!("{k}: must not be {}", show(w)));
        }
    }
    if let Some(opts) = &c.one_of {
        if !opts.iter().any(|o| loose_eq(got, o)) {
            bad.push(format!("{k}: wanted one of {}, got {}", show(&Json::Array(opts.clone())), show(got)));
        }
    }
    if c.min.is_some() || c.max.is_some() {
        match num(got) {
            Some(x) => {
                if c.min.is_some_and(|m| x < m) || c.max.is_some_and(|m| x > m) {
                    bad.push(format!("{k}: {x} outside {:?}..{:?}", c.min, c.max));
                }
            }
            None => bad.push(format!("{k}: {} is not a number", show(got))),
        }
    }
    if let Some(sub) = &c.contains {
        let text = got.as_str().map(squash).unwrap_or_else(|| squash(&got.to_string()));
        if !text.contains(&squash(sub)) {
            bad.push(format!("{k}: {} does not contain {sub:?}", show(got)));
        }
    }
    bad
}

/// Why a finished run fails its case. Empty means it passes.
pub fn evaluate(case: &Case, run: &AgentRun) -> Vec<String> {
    if case.expect == Expect::Decline {
        return match (&run.declined, &run.spec) {
            (Some(_), None) => Vec::new(),
            (_, Some(s)) => vec![format!("made a {} figure instead of declining", s.id)],
            (None, None) => vec![format!("neither declined nor finished ({})", run.error.as_deref().unwrap_or("no reason"))],
        };
    }
    if let Some(why) = &run.declined {
        if case.expect == Expect::Either {
            let words: Vec<&String> = case.checks.iter().flat_map(|c| c.note_contains.iter().flatten()).collect();
            let missing: Vec<&&String> = words.iter().filter(|w| !why.to_lowercase().contains(&w.to_lowercase())).collect();
            return if missing.is_empty() { Vec::new() } else { vec![format!("declined without mentioning {missing:?}: {why}")] };
        }
        return vec![format!("declined a request that has an answer: {why}")];
    }
    let Some(spec) = &run.spec else {
        return vec![format!("no figure ({})", run.error.as_deref().unwrap_or("no reason"))];
    };
    let mut bad = Vec::new();
    if !case.ids.is_empty() && !case.ids.contains(&spec.id) {
        bad.push(format!("chose {} (wanted {})", spec.id, case.ids.join(" or ")));
    }
    let params: serde_json::Map<String, Json> = spec.params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    for c in &case.checks {
        bad.extend(check_one(c, &params, &run.note));
    }
    bad
}

// ---------- cases file validation ----------

/// Problems with a case itself: unknown generators or keys, impossible values. Empty means usable.
pub fn validate(case: &Case, base: &std::path::Path) -> Vec<String> {
    let mut bad = Vec::new();
    let reg = figure_agent::registry();
    let defs_of = |id: &str| reg.iter().find(|d| d.id == id).map(|d| (d.params)());
    for id in case.ids.iter().chain(case.reference.iter().filter_map(|r| r.id.as_ref())) {
        if defs_of(id).is_none() {
            bad.push(format!("unknown figure id {id:?}"));
        }
    }
    if case.request.trim().is_empty() && case.reference.is_none() {
        bad.push("needs a request or a reference".to_string());
    }
    if case.expect == Expect::Decline && (!case.checks.is_empty() || !case.ids.is_empty()) {
        bad.push("a decline case has no ids or checks".to_string());
    }
    if let Some(r) = &case.reference {
        match (&r.id, &r.file) {
            (None, None) => bad.push("a reference needs an id or a file".to_string()),
            (Some(_), Some(_)) => bad.push("a reference has an id or a file, not both".to_string()),
            _ => {}
        }
        if let Some(f) = &r.file {
            if !base.join(f).is_file() {
                bad.push(format!("reference file {f:?} not found under {}", base.display()));
            }
        }
        if let Some(defs) = r.id.as_deref().and_then(defs_of) {
            for (k, v) in r.params.as_object().into_iter().flatten() {
                let Some(def) = defs.iter().find(|d| d.key == k) else {
                    bad.push(format!("reference param {k:?} is not a parameter of {}", r.id.as_deref().unwrap_or("?")));
                    continue;
                };
                if let (Kind::Choice(opts), Some(t)) = (&def.kind, v.as_str()) {
                    if !opts.contains(&t) {
                        bad.push(format!("reference param {k:?}: {t:?} is not one of {opts:?}"));
                    }
                }
            }
        }
    }
    for c in &case.checks {
        let keys: Vec<&String> = if c.key.is_empty() { c.keys.iter().collect() } else { vec![&c.key] };
        for k in keys {
            if case.ids.is_empty() {
                bad.push(format!("check on {k:?} needs ids"));
                continue;
            }
            // The key must exist in every acceptable generator, or the check can never pass for one.
            for id in &case.ids {
                let Some(defs) = defs_of(id) else { continue };
                let Some(def) = defs.iter().find(|d| d.key == k.as_str()) else {
                    bad.push(format!("{id} has no parameter {k:?}"));
                    continue;
                };
                let numeric = matches!(def.kind, Kind::Float { .. } | Kind::Int { .. });
                if (c.min.is_some() || c.max.is_some()) && !numeric && c.key == *k {
                    bad.push(format!("{id}.{k} is not numeric but the check has min/max"));
                }
                if let Kind::Float { min, max, .. } = def.kind {
                    for w in c.eq.iter().filter_map(num) {
                        if w < min || w > max {
                            bad.push(format!("{id}.{k}: eq {w} is outside {min}..{max}"));
                        }
                    }
                }
                if let Kind::Choice(opts) = &def.kind {
                    let wants = c.eq.iter().chain(c.not_eq.iter()).chain(c.one_of.iter().flatten());
                    for w in wants.filter_map(Json::as_str) {
                        if !opts.contains(&w) {
                            bad.push(format!("{id}.{k}: {w:?} is not one of {opts:?}"));
                        }
                    }
                }
            }
        }
        let empty = c.eq.is_none()
            && c.not_eq.is_none()
            && c.min.is_none()
            && c.max.is_none()
            && c.one_of.is_none()
            && c.contains.is_none()
            && c.contains_values.is_none()
            && c.any_contains.is_none()
            && c.note_contains.is_none();
        if empty {
            bad.push("a check with no operation".to_string());
        }
    }
    bad
}

// ---------- running ----------

pub use crate::figure_agent::{Judge, ModelJudge, Verdict};

#[derive(Clone, Debug, Serialize)]
pub struct RunResult {
    pub pass: bool,
    pub steps: usize,
    pub renders: usize,
    pub figure: Option<String>,
    pub note: String,
    pub failures: Vec<String>,
    pub judge: Option<Verdict>,
    /// What the model said it saw in the reference image.
    pub sees: Option<String>,
    /// The final picture, kept so a human can look at it (`VERSHION_EVAL_PICS`).
    #[serde(skip)]
    pub png: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseResult {
    pub name: String,
    pub level: String,
    pub runs: Vec<RunResult>,
}

impl CaseResult {
    pub fn passed(&self) -> usize {
        self.runs.iter().filter(|r| r.pass).count()
    }
}

fn failed(msg: String) -> RunResult {
    RunResult { pass: false, steps: 0, renders: 0, figure: None, note: String::new(), failures: vec![msg], judge: None, sees: None, png: None }
}

/// One attempt at one case. `base` is the cases file's folder (for reference image files).
pub fn run_case(case: &Case, chat: &mut dyn Chat, base: &std::path::Path, judge: Option<&mut dyn Judge>) -> RunResult {
    run_case_tries(case, chat, base, judge, 1)
}

/// [`run_case`] with the agent's retry loop: up to `tries` attempts, each reviewed by `judge`.
pub fn run_case_tries(case: &Case, chat: &mut dyn Chat, base: &std::path::Path, judge: Option<&mut dyn Judge>, tries: usize) -> RunResult {
    let png = match case.reference.as_ref().map(|r| reference_png(r, base)) {
        Some(Ok(png)) => Some(png),
        Some(Err(e)) => return failed(format!("reference failed: {e}")),
        None => None,
    };
    let request = if case.request.trim().is_empty() { "Recreate the attached figure." } else { case.request.as_str() };
    let (run, verdict) = if tries > 1 {
        let run = figure_agent::run_with_retries(request, png.as_deref(), chat, judge, tries);
        let verdict = run.review.clone();
        (run, verdict)
    } else {
        let run = figure_agent::run_with_reference(request, png.as_deref(), chat);
        // Only a finished figure is worth showing to a judge.
        let verdict = match (judge, &run.png) {
            (Some(j), Some(fig)) => j.judge(request, fig, png.as_deref()).ok(),
            _ => None,
        };
        (run, verdict)
    };
    let failures = evaluate(case, &run);
    RunResult {
        pass: failures.is_empty(),
        steps: run.steps.len(),
        renders: run.steps.iter().filter(|s| s.tool == "render_figure").count(),
        figure: run.spec.as_ref().map(|s| s.id.clone()),
        note: run.note,
        failures,
        judge: verdict,
        sees: run.sees,
        png: run.png,
    }
}

pub fn run_all(
    cases: &[Case],
    repeats: usize,
    base: &std::path::Path,
    chat: &mut dyn Chat,
    judge: Option<&mut dyn Judge>,
    progress: impl FnMut(&str),
) -> Vec<CaseResult> {
    run_all_tries(cases, repeats, base, chat, judge, 1, progress)
}

/// [`run_all`] where each run may take up to `tries` attempts (see `figure_agent::run_with_retries`).
pub fn run_all_tries(
    cases: &[Case],
    repeats: usize,
    base: &std::path::Path,
    chat: &mut dyn Chat,
    mut judge: Option<&mut dyn Judge>,
    tries: usize,
    mut progress: impl FnMut(&str),
) -> Vec<CaseResult> {
    let mut results = Vec::new();
    for c in cases {
        let mut runs = Vec::new();
        for i in 0..repeats {
            progress(&format!("{} ({}/{})", c.name, i + 1, repeats));
            let j: Option<&mut dyn Judge> = match judge.as_mut() {
                Some(j) => Some(&mut **j),
                None => None,
            };
            runs.push(run_case_tries(c, chat, base, j, tries));
        }
        results.push(CaseResult { name: c.name.clone(), level: c.level.clone(), runs });
    }
    results
}

pub fn parse_cases(json: &str) -> Result<Vec<Case>, String> {
    serde_json::from_str(json).map_err(|e| format!("cases file: {e}"))
}

fn avg(xs: impl Iterator<Item = usize>, n: usize) -> f64 {
    if n == 0 { 0.0 } else { xs.sum::<usize>() as f64 / n as f64 }
}

pub fn format_report(results: &[CaseResult]) -> String {
    let mut out = String::new();
    for r in results {
        let n = r.runs.len();
        out.push_str(&format!(
            "{} {}/{}  [{}] {}  steps {:.1}  renders {:.1}\n",
            if r.passed() == n { "PASS" } else if r.passed() == 0 { "FAIL" } else { "PART" },
            r.passed(),
            n,
            r.level,
            r.name,
            avg(r.runs.iter().map(|x| x.steps), n),
            avg(r.runs.iter().map(|x| x.renders), n),
        ));
        let judged: Vec<&Verdict> = r.runs.iter().filter_map(|x| x.judge.as_ref()).collect();
        if !judged.is_empty() {
            out.push_str(&format!("      judge: looks right in {}/{} judged\n", judged.iter().filter(|v| v.matches).count(), judged.len()));
        }
        // The judge disagreeing with passing checks is the interesting case: checks missed something.
        for (i, x) in r.runs.iter().enumerate() {
            if let (true, Some(v)) = (x.pass, &x.judge) {
                if !v.matches {
                    out.push_str(&format!("      ! run {} passed its checks but the judge saw: {}\n", i + 1, v.problems.join("; ")));
                }
            }
        }
        let mut seen: Vec<&String> = Vec::new();
        for f in r.runs.iter().flat_map(|x| &x.failures) {
            if !seen.contains(&f) {
                seen.push(f);
                out.push_str(&format!("      - {f}\n"));
            }
        }
    }
    let mut levels: Vec<&str> = results.iter().map(|r| r.level.as_str()).collect();
    levels.dedup();
    out.push_str("\nby level:\n");
    let mut done: Vec<&str> = Vec::new();
    for l in levels {
        if done.contains(&l) {
            continue;
        }
        done.push(l);
        let (p, t) = results.iter().filter(|r| r.level == l).fold((0, 0), |(p, t), r| (p + r.passed(), t + r.runs.len()));
        out.push_str(&format!("  {l:<10} {p}/{t}\n"));
    }
    let (p, t) = results.iter().fold((0, 0), |(p, t), r| (p + r.passed(), t + r.runs.len()));
    out.push_str(&format!("  {:<10} {p}/{t}\n", "all"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::Turn;
    use crate::model::FigureSpec;
    use serde_json::json;

    const CASES: &str = include_str!("../tests/figure_agent_cases.json");

    fn base() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
    }

    fn spec(id: &str, params: Json) -> FigureSpec {
        FigureSpec { id: id.to_string(), params: params.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect() }
    }

    fn run_with(spec: Option<FigureSpec>, note: &str) -> AgentRun {
        AgentRun { spec, note: note.to_string(), ..Default::default() }
    }

    #[test]
    fn the_cases_file_parses_and_every_case_is_valid_against_the_registry() {
        let cases = parse_cases(CASES).expect("parses");
        assert!(cases.len() >= 15, "{} cases", cases.len());
        let mut names: Vec<&str> = cases.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), cases.len(), "duplicate case names");
        let problems: Vec<String> = cases.iter().flat_map(|c| validate(c, &base()).into_iter().map(move |p| format!("{}: {p}", c.name))).collect();
        assert!(problems.is_empty(), "{problems:#?}");
    }

    #[test]
    fn reference_pictures_render() {
        for c in parse_cases(CASES).unwrap() {
            if let Some(r) = c.reference {
                let png = reference_png(&r, &base()).unwrap_or_else(|e| panic!("{}: {e}", c.name));
                assert!(png.starts_with(b"\x89PNG"), "{}", c.name);
            }
        }
    }

    #[test]
    fn numbers_compare_as_numbers_and_text_loosely() {
        assert!(loose_eq(&json!("90"), &json!(90)));
        assert!(loose_eq(&json!(30.0), &json!(30)));
        assert!(loose_eq(&json!("Up the ramp"), &json!("up the  ramp")));
        assert!(!loose_eq(&json!("Up the ramp"), &json!("Down the ramp")));
        assert!(!loose_eq(&json!(true), &json!(false)));
    }

    #[test]
    fn checks_report_what_was_wrong() {
        let case = Case {
            name: "t".into(),
            level: "x".into(),
            request: "r".into(),
            reference: None,
            ids: vec!["inclined-plane".into()],
            expect: Expect::Figure,
            checks: vec![
                Check { key: "angle".into(), eq: Some(json!(30)), ..Default::default() },
                Check { key: "friction".into(), not_eq: Some(json!("None")), ..Default::default() },
                Check { key: "obj_lab".into(), contains: Some("5 kg".into()), ..Default::default() },
            ],
        };
        let good = run_with(Some(spec("inclined-plane", json!({"angle": 30.0, "friction": "Up the ramp", "obj_lab": "5kg"}))), "");
        assert!(evaluate(&case, &good).is_empty());
        let bad = run_with(Some(spec("inclined-plane", json!({"angle": 60.0, "friction": "None", "obj_lab": "m"}))), "");
        assert_eq!(evaluate(&case, &bad).len(), 3);
        let wrong = run_with(Some(spec("free-body-diagram", json!({"angle": 30.0, "friction": "Up the ramp", "obj_lab": "5kg"}))), "");
        assert!(evaluate(&case, &wrong)[0].contains("chose free-body-diagram"));
    }

    #[test]
    fn declining_is_scored_both_ways() {
        let case = Case { name: "d".into(), level: "x".into(), request: "r".into(), reference: None, ids: vec![], expect: Expect::Decline, checks: vec![] };
        let said_no = AgentRun { declined: Some("no".into()), ..Default::default() };
        assert!(evaluate(&case, &said_no).is_empty());
        assert!(!evaluate(&case, &run_with(Some(spec("triangle", json!({}))), "")).is_empty());
        let normal = Case { expect: Expect::Figure, ..case };
        assert!(evaluate(&normal, &said_no)[0].contains("declined"));
    }

    #[test]
    fn either_accepts_a_figure_or_an_informative_decline() {
        let case = Case {
            name: "e".into(),
            level: "x".into(),
            request: "r".into(),
            reference: None,
            ids: vec!["regular-polygon".into()],
            expect: Expect::Either,
            checks: vec![Check { key: "n".into(), eq: Some(json!(20)), ..Default::default() }, Check { note_contains: Some(vec!["20".into()]), ..Default::default() }],
        };
        let clamp = run_with(Some(spec("regular-polygon", json!({"n": 20}))), "Used the 20-side limit.");
        assert!(evaluate(&case, &clamp).is_empty());
        let said_limit = AgentRun { declined: Some("only 3 to 20 sides".into()), ..Default::default() };
        assert!(evaluate(&case, &said_limit).is_empty());
        let vague = AgentRun { declined: Some("cannot".into()), ..Default::default() };
        assert!(!evaluate(&case, &vague).is_empty());
    }

    #[test]
    fn set_and_note_checks() {
        let case = Case {
            name: "s".into(),
            level: "x".into(),
            request: "r".into(),
            reference: None,
            ids: vec!["triangle".into()],
            expect: Expect::Figure,
            checks: vec![
                Check { keys: vec!["m_ab".into(), "m_bc".into(), "m_ca".into()], contains_values: Some(vec![json!(3), json!(4)]), ..Default::default() },
                Check { note_contains: Some(vec!["limit".into()]), ..Default::default() },
            ],
        };
        let ok = run_with(Some(spec("triangle", json!({"m_ab": "3", "m_bc": "4", "m_ca": ""}))), "Used the limit.");
        assert!(evaluate(&case, &ok).is_empty());
        let no = run_with(Some(spec("triangle", json!({"m_ab": "3", "m_bc": "5", "m_ca": ""}))), "ok");
        assert_eq!(evaluate(&case, &no).len(), 2);
    }

    #[test]
    fn validation_catches_typos() {
        let bad = Case {
            name: "b".into(),
            level: "x".into(),
            request: "r".into(),
            reference: None,
            ids: vec!["inclined-plane".into()],
            expect: Expect::Figure,
            checks: vec![
                Check { key: "angel".into(), eq: Some(json!(30)), ..Default::default() },
                Check { key: "friction".into(), eq: Some(json!("Uphill")), ..Default::default() },
                Check { key: "angle".into(), eq: Some(json!(300)), ..Default::default() },
            ],
        };
        assert_eq!(validate(&bad, &base()).len(), 3, "{:?}", validate(&bad, &base()));
    }

    /// A model that always gives the same canned reply, for runner plumbing.
    struct Fixed(String);
    impl Chat for Fixed {
        fn chat(&mut self, _: &[Turn]) -> Result<String, String> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn the_runner_scores_a_whole_case_and_reports_it() {
        let case = Case {
            name: "poly".into(),
            level: "easy".into(),
            request: "a hexagon".into(),
            reference: None,
            ids: vec!["regular-polygon".into()],
            expect: Expect::Figure,
            checks: vec![Check { key: "n".into(), eq: Some(json!(6)), ..Default::default() }],
        };
        let mut chat = Fixed(r#"{"final":{"id":"regular-polygon","params":{"n":6},"note":"hexagon"}}"#.to_string());
        let res = run_all(&[case], 2, &base(), &mut chat, None, |_| {});
        assert_eq!(res[0].passed(), 2);
        let text = format_report(&res);
        assert!(text.contains("PASS 2/2") && text.contains("all"), "{text}");
    }

    #[test]
    fn a_reference_case_hands_the_agent_a_picture() {
        struct Spy(std::cell::Cell<usize>);
        impl Chat for Spy {
            fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
                self.0.set(turns.iter().map(|t| t.images.len()).sum());
                Ok(r#"{"cannot":"spy"}"#.to_string())
            }
        }
        let case = Case {
            name: "ref".into(),
            level: "image".into(),
            request: String::new(),
            reference: Some(Reference { id: Some("regular-polygon".into()), params: json!({"n": 5}), file: None, degrade: None }),
            ids: vec![],
            expect: Expect::Decline,
            checks: vec![],
        };
        let mut spy = Spy(std::cell::Cell::new(0));
        let r = run_case(&case, &mut spy, &base(), None);
        assert_eq!(spy.0.get(), 1);
        assert!(r.pass);
    }

    #[test]
    fn degrading_is_deterministic_valid_and_actually_changes_the_picture() {
        let (_, clean) = figure_agent::render_png("regular-polygon", &json!({"n": 5})).unwrap();
        let d = Degrade { scale: Some(0.5), rotate: Some(3.0), shade: Some(0.85), blur: Some(1.0), noise: Some(12), jpeg: Some(40) };
        let a = degrade(&clean, &d).unwrap();
        let b = degrade(&clean, &d).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, clean);
        let img = image::load_from_memory(&a).unwrap();
        let orig = image::load_from_memory(&clean).unwrap();
        assert!(img.width() < orig.width());
    }

    /// A model that gets everything wrong must fail the suite: proof the checks can fail.
    #[test]
    fn a_broken_model_cannot_pass_the_suite() {
        let cases = parse_cases(CASES).unwrap();
        let mut wrong = Fixed(r#"{"final":{"id":"triangle","params":{},"note":"x"}}"#.to_string());
        let res = run_all(&cases, 1, &base(), &mut wrong, None, |_| {});
        let passed: Vec<&str> = res.iter().filter(|r| r.passed() > 0).map(|r| r.name.as_str()).collect();
        assert!(passed.is_empty(), "a model that always draws a default triangle passed {passed:?}");
    }

    #[test]
    fn a_model_that_always_declines_passes_only_the_decline_cases() {
        let cases = parse_cases(CASES).unwrap();
        let mut refuse = Fixed(r#"{"cannot":"no"}"#.to_string());
        let res = run_all(&cases, 1, &base(), &mut refuse, None, |_| {});
        let passed: Vec<&str> = res.iter().filter(|r| r.passed() > 0).map(|r| r.name.as_str()).collect();
        let expected: Vec<&str> = cases.iter().filter(|c| c.expect == Expect::Decline).map(|c| c.name.as_str()).collect();
        assert_eq!(passed, expected);
    }

    #[test]
    fn garbage_replies_pass_nothing() {
        let cases = parse_cases(CASES).unwrap();
        let mut junk = Fixed("lorem ipsum".to_string());
        let res = run_all(&cases, 1, &base(), &mut junk, None, |_| {});
        assert!(res.iter().all(|r| r.passed() == 0));
    }

    struct Always(bool);
    impl Judge for Always {
        fn judge(&mut self, _: &str, _: &[u8], reference: Option<&[u8]>) -> Result<Verdict, String> {
            Ok(Verdict { matches: self.0, problems: if self.0 { vec![] } else { vec![format!("ref={}", reference.is_some())] } })
        }
    }

    #[test]
    fn the_report_flags_a_judge_that_disagrees_with_passing_checks() {
        let case = Case {
            name: "poly".into(),
            level: "easy".into(),
            request: "a hexagon".into(),
            reference: None,
            ids: vec!["regular-polygon".into()],
            expect: Expect::Figure,
            checks: vec![Check { key: "n".into(), eq: Some(json!(6)), ..Default::default() }],
        };
        let mut chat = Fixed(r#"{"final":{"id":"regular-polygon","params":{"n":6},"note":"hexagon"}}"#.to_string());
        let mut no = Always(false);
        let res = run_all(&[case], 1, &base(), &mut chat, Some(&mut no), |_| {});
        assert!(res[0].runs[0].pass);
        let text = format_report(&res);
        assert!(text.contains("passed its checks but the judge saw"), "{text}");
    }

    #[test]
    fn any_contains_reads_either_parameter_and_ignores_subscripts() {
        let check = Check { keys: vec!["formula".into(), "preset".into()], any_contains: Some(vec!["CH3COCl".into(), "acetyl chloride".into()]), ..Default::default() };
        let params = |v: Json| v.as_object().unwrap().clone();
        assert!(check_one(&check, &params(json!({"formula": "CH3COCl", "preset": "None (use the formula)"})), "").is_empty());
        assert!(check_one(&check, &params(json!({"formula": "", "preset": "CH\u{2083}COCl, acetyl chloride"})), "").is_empty());
        assert!(!check_one(&check, &params(json!({"formula": "H2O", "preset": "None"})), "").is_empty());
    }

    #[test]
    fn a_missing_reference_file_is_a_validation_error() {
        let case = Case {
            name: "f".into(),
            level: "image".into(),
            request: String::new(),
            reference: Some(Reference { id: None, params: Json::Null, file: Some("figure_images/nope.png".into()), degrade: None }),
            ids: vec![],
            expect: Expect::Figure,
            checks: vec![],
        };
        assert!(validate(&case, &base()).iter().any(|p| p.contains("not found")));
    }
}
