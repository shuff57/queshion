//! Figure agent: a small tool-calling loop that drives the figure engine from a plain request
//! ("a free-body diagram of a 5 kg block on a 30 degree ramp").
//!
//! The model never draws. It picks a generator from [`figures::registry`], fills its parameters,
//! looks at the rendered PNG, and adjusts until the figure fits the request. The reply protocol
//! is one JSON object per turn so any provider works, with or without native tool calling:
//!
//! - `{"tool": "list_figures" | "get_figure" | "render_figure", "args": {...}}` runs a tool.
//! - `{"final": {"id": "...", "params": {...}, "note": "..."}}` ends the run.
//! - `{"cannot": "reason"}` ends the run when no generator can make the request.
//!
//! A reference image (a photo or scan of the figure to reproduce) can ride along with the request;
//! the model then compares each render with it.
//!
//! The loop only needs a [`Chat`] and the registry, so it is a plain function and can later run
//! somewhere other than the desktop app (a Worker relaying tool calls back to a client).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Serialize;
use serde_json::{Value as Json, json};

use crate::ai::{AiCfg, Turn};
use crate::figures::spec::{flatten_white, spec_for, value_to_json};
use crate::figures::{self, FigureDef, Kind, ParamDef, Params, Subject, Value};
use crate::model::FigureSpec;

/// Model calls plus tool calls the loop may spend before giving up.
pub const MAX_STEPS: usize = 8;
/// Replies that were not a valid command in a row before the run is abandoned.
const MAX_BAD_REPLIES: usize = 2;

/// One model round trip. A trait so tests (and a future remote host) can stand in for Ollama.
pub trait Chat {
    fn chat(&mut self, turns: &[Turn]) -> Result<String, String>;
}

/// The configured provider (`ai.rs`), blocking.
pub struct ModelChat<'a> {
    pub cfg: &'a AiCfg,
}

impl Chat for ModelChat<'_> {
    fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
        crate::ai::chat_turns(self.cfg, turns, 0.2)
    }
}

/// One line of the run log shown to the user (and printed by the headless hook).
#[derive(Clone, Debug, PartialEq)]
pub struct StepLog {
    pub tool: String,
    pub summary: String,
}

#[derive(Debug, Default)]
pub struct AgentRun {
    /// The figure the model settled on, ready to store on a question.
    pub spec: Option<FigureSpec>,
    /// That figure rendered, flattened on white.
    pub png: Option<Vec<u8>>,
    /// The model's one-line description of what it made.
    pub note: String,
    pub steps: Vec<StepLog>,
    /// Why there is no `spec`, when there is none.
    pub error: Option<String>,
    /// Set when the model said no generator can make the request (and why).
    pub declined: Option<String>,
    /// What the model said it saw in the reference image, from its first reply (if it was asked).
    pub sees: Option<String>,
    /// How many attempts were made (1 unless [`run_with_retries`] went round again).
    pub tries: usize,
    /// The reviewer's last verdict on `spec`, when a reviewer ran.
    pub review: Option<Verdict>,
}

pub(crate) fn registry() -> &'static [FigureDef] {
    static R: OnceLock<Vec<FigureDef>> = OnceLock::new();
    R.get_or_init(figures::registry)
}

fn find(id: &str) -> Option<&'static FigureDef> {
    registry().iter().find(|d| d.id == id)
}

fn subject_named(s: &str) -> Option<Subject> {
    Subject::ALL.into_iter().find(|x| x.name().eq_ignore_ascii_case(s.trim()))
}

// ---------- tools ----------

/// `id | Title | blurb`, one per line, optionally one subject only.
fn tool_list(args: &Json) -> String {
    let want = args.get("subject").and_then(Json::as_str).and_then(subject_named);
    let mut out = String::new();
    for d in registry() {
        if want.is_some_and(|s| !figures::listed_under(d, s)) {
            continue;
        }
        out.push_str(&format!("{} | {} | {}\n", d.id, d.title, d.blurb));
    }
    if out.is_empty() { "no figures match".to_string() } else { out }
}

fn describe_param(d: &ParamDef) -> String {
    let (ty, range) = match &d.kind {
        Kind::Float { min, max, .. } => ("number", format!("{min}..{max}")),
        Kind::Int { min, max } => ("integer", format!("{min}..{max}")),
        Kind::Bool => ("bool", "true/false".to_string()),
        Kind::Choice(opts) => ("choice", opts.join(" | ")),
        Kind::Text => ("text", String::new()),
        Kind::Long => ("multi-line text", String::new()),
        Kind::List => ("list text", String::new()),
        Kind::Color(opts) => ("color", opts.join(" | ")),
    };
    let default = match &d.default {
        Value::S(s) => format!("{s:?}"),
        v => value_to_json(v).to_string(),
    };
    let hint = if d.hint.is_empty() { String::new() } else { format!(" - {}", d.hint) };
    format!("{} ({}): {ty} {range}, default {default}{hint}", d.key, d.label)
}

/// Every figure with its parameters, as `get_figure` shows them (the `VERSHION_FIGURE_CATALOG`
/// hook prints this; handy when writing eval cases).
pub fn catalog_text() -> String {
    registry().iter().map(|d| tool_get(&json!({"id": d.id})).unwrap_or_default()).collect::<Vec<_>>().join("\n")
}

fn tool_get(args: &Json) -> Result<String, String> {
    let id = args.get("id").and_then(Json::as_str).ok_or("get_figure needs args.id")?;
    let d = find(id).ok_or_else(|| format!("no figure with id {id:?}; call list_figures"))?;
    let mut out = format!("{} - {}\n{}\nparams:\n", d.id, d.title, d.blurb);
    for p in (d.params)() {
        out.push_str(&format!("  {}\n", describe_param(&p)));
    }
    Ok(out)
}

/// Starts from the figure's opening state, then applies `over` (a JSON object) coerced to each
/// parameter's own type and clamped to its range. Returns the params and what could not be applied.
fn build_params(d: &FigureDef, over: &Json) -> (Params, Vec<String>) {
    let defs = (d.params)();
    let mut p = figures::start_params(d);
    let mut notes = Vec::new();
    let Some(obj) = over.as_object() else {
        if !over.is_null() {
            notes.push("params must be a JSON object".to_string());
        }
        return (p, notes);
    };
    let mut asked: BTreeMap<&str, &Json> = BTreeMap::new();
    for (k, v) in obj {
        let Some(def) = defs.iter().find(|x| x.key == k) else {
            notes.push(format!("unknown param {k:?} ignored"));
            continue;
        };
        let coerced = match (&def.default, v) {
            (Value::F(_), Json::Number(n)) => n.as_f64().map(Value::F),
            (Value::I(_), Json::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)).map(Value::I),
            (Value::B(_), Json::Bool(b)) => Some(Value::B(*b)),
            (Value::S(_), Json::String(s)) => Some(Value::S(s.clone())),
            // Lists the model wrote as JSON arrays: the engine parses comma-separated text.
            (Value::S(_), Json::Array(a)) => Some(Value::S(
                a.iter().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).collect::<Vec<_>>().join(", "),
            )),
            (Value::S(_), Json::Number(n)) => Some(Value::S(n.to_string())),
            _ => None,
        };
        match coerced {
            Some(c) => {
                p.0.insert(def.key.to_string(), c);
                asked.insert(def.key, v);
            }
            None => notes.push(format!("param {k:?} has the wrong type (expected {}), kept the default", describe_param(def))),
        }
    }
    p.normalize(&defs, false);
    for (k, want) in asked {
        let got = value_to_json(&p.0[k]);
        let same = match (want.as_f64(), got.as_f64()) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            _ => want == &got || matches!(want, Json::Array(_)),
        };
        if !same {
            notes.push(format!("param {k:?}: asked {want}, used {got} (out of range or not an option)"));
        }
    }
    (p, notes)
}

struct Rendered {
    spec: FigureSpec,
    png: Vec<u8>,
    notes: Vec<String>,
}

fn render(id: &str, over: &Json) -> Result<Rendered, String> {
    let d = find(id).ok_or_else(|| format!("no figure with id {id:?}; call list_figures"))?;
    let (p, notes) = build_params(d, over);
    let svg = (d.render)(&p);
    let png = figures::to_png(&svg, 1.0).map_err(|e| format!("render failed: {e}"))?;
    let png = flatten_white(&png).unwrap_or(png);
    Ok(Rendered { spec: spec_for(d.id, &p), png, notes })
}

/// Renders a figure from JSON params the way a tool call does; the eval uses it to draw reference images.
pub(crate) fn render_png(id: &str, over: &Json) -> Result<(FigureSpec, Vec<u8>), String> {
    render(id, over).map(|r| (r.spec, r.png))
}

fn render_args(args: &Json) -> Result<Rendered, String> {
    let id = args.get("id").and_then(Json::as_str).ok_or("render_figure needs args.id")?;
    render(id, args.get("params").unwrap_or(&Json::Null))
}

// ---------- the loop ----------

fn system_prompt(has_reference: bool) -> String {
    let mut catalog = String::new();
    for d in registry() {
        catalog.push_str(&format!("{} | {}\n", d.id, d.title));
    }
    let mut p = format!(
        "You make figures for exam questions by driving a figure generator. You cannot draw; you \
choose a generator and set its parameters.\n\n\
Reply with exactly one JSON object per turn, nothing else.\n\
Run a tool:  {{\"tool\": \"<name>\", \"args\": {{...}}}}\n\
  list_figures  args: {{\"subject\": \"Physics|Biology|Chemistry|Math\"}} (optional) - id | title | blurb\n\
  get_figure    args: {{\"id\": \"...\"}} - the parameters, ranges and defaults\n\
  render_figure args: {{\"id\": \"...\", \"params\": {{...}}}} - draws it and shows you the picture\n\
Finish:      {{\"final\": {{\"id\": \"...\", \"params\": {{...}}, \"note\": \"one sentence\"}}}}\n\
Give up:     {{\"cannot\": \"why no generator here can make this\"}}\n\n\
Rules:\n\
- Use only ids from the catalog and parameter keys from get_figure. Never invent either.\n\
- Pick the generator whose own parameters cover what the request asks for (a ramp problem needs a \
ramp angle and force toggles, not a generic diagram you would have to bend). Read get_figure for \
the likely candidates before choosing, and prefer one that fits directly.\n\
- Set only the parameters the request needs; the rest keep their defaults. Quantities in the \
request (angles, lengths, counts) must appear in the parameters, in the units the parameter uses.\n\
- After each render_figure you get the picture and a list of values that were clamped or ignored. \
Look at the picture: are the right quantities, labels, forces and orientation there? If anything \
the request needs is missing or wrong, fix it and render again. Do not finish on a figure you have \
not checked.\n\
- If a request asks for a value beyond a generator's range (more sides, a longer ruler, a steeper \
angle than it allows), do not decline: use the nearest allowed value and say so in the note, naming \
the value asked for and the one used. Say cannot only when no generator in the catalog draws that \
kind of thing at all; never force a wrong figure.\n\
- The note says in one sentence what you made and any compromise. You have at most {MAX_STEPS} turns.\n"
    );
    if has_reference {
        p.push_str(
            "\nThe first message has a reference image. Reproduce it: choose the generator that matches, \
then set parameters so your render has the same shapes, labels, numbers and layout. Compare each \
render with the reference image, not only with the request text.\n\
Your first reply must also carry a \"sees\" field next to the tool call: a literal description of \
what is drawn (the letters, symbols, labels, numbers and how they are joined) before you choose \
anything. Read the letters. Chemical element symbols joined by lines or double lines, with dots \
beside them, are a molecule's Lewis structure, not forces; arrows labelled F or with a mass are \
forces. Choose the generator from what you read, not from the overall shape.\n",
        );
    }
    p.push_str(&format!("\nCatalog (id | title):\n{catalog}"));
    p
}

/// Parses a reply into its first JSON object, tolerating code fences, prose around it, and a second
/// object after it (models sometimes send a tool call and a guess at its result together; only the
/// first is acted on).
fn parse_reply(raw: &str) -> Result<Json, String> {
    let start = raw.find('{').ok_or("the reply had no JSON object")?;
    match serde_json::Deserializer::from_str(&raw[start..]).into_iter::<Json>().next() {
        Some(Ok(v)) if v.is_object() => Ok(v),
        Some(Err(e)) => Err(format!("the reply was not valid JSON ({e})")),
        _ => Err("the reply had no JSON object".to_string()),
    }
}

fn brief(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= n { one } else { format!("{}...", one.chars().take(n).collect::<String>()) }
}

pub fn run(request: &str, chat: &mut dyn Chat) -> AgentRun {
    run_with_reference(request, None, chat)
}

/// [`run`] with an optional reference image (PNG/JPEG bytes) the figure should reproduce.
pub fn run_with_reference(request: &str, reference: Option<&[u8]>, chat: &mut dyn Chat) -> AgentRun {
    let mut out = AgentRun::default();
    let mut first = Turn::new("user", format!("Request: {request}"));
    if let Some(img) = reference {
        first.images.push(img.to_vec());
    }
    let mut turns = vec![Turn::new("system", system_prompt(reference.is_some())), first];
    let mut bad = 0;
    let mut asked_note = false;
    let mut last: Option<Rendered> = None;

    for step in 0..MAX_STEPS {
        let raw = match chat.chat(&turns) {
            Ok(r) => r,
            Err(e) => {
                out.error = Some(format!("the model call failed: {e}"));
                return out;
            }
        };
        if crate::env_var("VERSHION_AGENT_DEBUG").as_deref() == Ok("1") {
            eprintln!("[agent reply {}] {}", step + 1, brief(&raw, 600));
        }
        turns.push(Turn::new("assistant", raw.clone()));

        let cmd = match parse_reply(&raw) {
            Ok(c) => {
                if out.sees.is_none() {
                    out.sees = c.get("sees").and_then(Json::as_str).map(str::to_string);
                }
                c
            }
            Err(e) => {
                bad += 1;
                if bad > MAX_BAD_REPLIES {
                    out.error = Some(format!("the model kept sending invalid replies ({e})"));
                    return out;
                }
                out.steps.push(StepLog { tool: "invalid".into(), summary: e.clone() });
                turns.push(Turn::new("user", format!("Error: {e}. Reply with exactly one JSON object as described.")));
                continue;
            }
        };

        if let Some(why) = cmd.get("cannot") {
            let why = why.as_str().map(str::to_string).unwrap_or_else(|| why.to_string());
            out.steps.push(StepLog { tool: "cannot".into(), summary: brief(&why, 120) });
            out.declined = Some(why);
            return out;
        }

        if let Some(fin) = cmd.get("final") {
            let id = fin.get("id").and_then(Json::as_str).unwrap_or("");
            // The note belongs inside `final`, but models often put it beside it; take either.
            let note = fin.get("note").or_else(|| cmd.get("note")).and_then(Json::as_str).unwrap_or("").to_string();
            // A silent compromise (a value the generator could not take) is the failure to prevent:
            // ask once for the note, which must name anything asked for that was not delivered.
            if note.trim().is_empty() && !asked_note {
                asked_note = true;
                out.steps.push(StepLog { tool: "final".into(), summary: "asked for a note".into() });
                turns.push(Turn::new(
                    "user",
                    "Your final reply has an empty note. Send the same final again with a one-sentence note saying what you made. \
If anything the request asked for is not in the figure (a value beyond the generator's range, a missing element), name what was asked and what you used.",
                ));
                continue;
            }
            match render(id, fin.get("params").unwrap_or(&Json::Null)) {
                Ok(r) => {
                    // Clamped or ignored values are always flagged, whether or not the model said so.
                    let note = if r.notes.is_empty() {
                        note
                    } else if note.is_empty() {
                        format!("Adjusted: {}.", r.notes.join("; "))
                    } else {
                        format!("{note} Adjusted: {}.", r.notes.join("; "))
                    };
                    out.steps.push(StepLog { tool: "final".into(), summary: format!("{id}: {}", brief(&note, 120)) });
                    out.note = note;
                    out.spec = Some(r.spec);
                    out.png = Some(r.png);
                    return out;
                }
                Err(e) => {
                    bad += 1;
                    if bad > MAX_BAD_REPLIES {
                        out.error = Some(format!("the final figure was invalid ({e})"));
                        return out;
                    }
                    out.steps.push(StepLog { tool: "final".into(), summary: format!("rejected: {e}") });
                    turns.push(Turn::new("user", format!("Error: {e}. Fix the final figure or keep working.")));
                    continue;
                }
            }
        }

        let tool = cmd.get("tool").and_then(Json::as_str).unwrap_or("");
        let args = cmd.get("args").cloned().unwrap_or_else(|| json!({}));
        let remaining = MAX_STEPS - step - 1;
        let (text, image, summary) = match tool {
            "list_figures" => {
                let t = tool_list(&args);
                let n = t.lines().count();
                (t, None, format!("{n} figures"))
            }
            "get_figure" => match tool_get(&args) {
                Ok(t) => {
                    let id = args.get("id").and_then(Json::as_str).unwrap_or("");
                    (t, None, id.to_string())
                }
                Err(e) => (format!("Error: {e}"), None, format!("error: {e}")),
            },
            "render_figure" => match render_args(&args) {
                Ok(r) => {
                    let id = args.get("id").and_then(Json::as_str).unwrap_or("");
                    let mut t = format!("Rendered {id}. Values used: {}.", serde_json::to_string(&r.spec.params).unwrap_or_default());
                    if r.notes.is_empty() {
                        t.push_str(" Every value was applied as asked.");
                    } else {
                        t.push_str(&format!(" Problems: {}.", r.notes.join("; ")));
                    }
                    let summary = if r.notes.is_empty() { id.to_string() } else { format!("{id} ({} adjusted)", r.notes.len()) };
                    let png = r.png.clone();
                    last = Some(r);
                    (t, Some(png), summary)
                }
                Err(e) => (format!("Error: {e}"), None, format!("error: {e}")),
            },
            other => {
                bad += 1;
                if bad > MAX_BAD_REPLIES {
                    out.error = Some(format!("the model kept calling unknown tools ({other:?})"));
                    return out;
                }
                let msg = if other.is_empty() {
                    "Error: your reply had no \"tool\", \"final\" or \"cannot\" key. Send exactly one JSON object, e.g. {\"tool\": \"get_figure\", \"args\": {\"id\": \"...\"}}.".to_string()
                } else {
                    format!("Error: unknown tool {other:?}. Tools: list_figures, get_figure, render_figure, or reply with final.")
                };
                (msg, None, format!("unknown tool {other:?}"))
            }
        };
        out.steps.push(StepLog { tool: tool.to_string(), summary });
        let mut t = Turn::new("user", format!("{text}\n({remaining} turns left)"));
        if let Some(png) = image {
            t.images.push(png);
        }
        turns.push(t);
    }

    // Out of turns: keep the last figure the model rendered, so the work is not thrown away.
    match last {
        Some(r) => {
            out.note = "Stopped at the turn limit; this is the last figure rendered.".to_string();
            out.spec = Some(r.spec);
            out.png = Some(r.png);
        }
        None => out.error = Some("the model used all its turns without rendering a figure".to_string()),
    }
    out
}

// ---------- reviewing a figure against its reference ----------

/// A vision model's read of one finished figure.
#[derive(Clone, Debug, Serialize)]
pub struct Verdict {
    pub matches: bool,
    pub problems: Vec<String>,
}

/// Looks at a finished figure (and the reference, if there was one) and says whether it fits the request.
pub trait Judge {
    fn judge(&mut self, request: &str, figure: &[u8], reference: Option<&[u8]>) -> Result<Verdict, String>;
}

/// The configured provider as judge. Pass a different model in `cfg` for a second opinion.
pub struct ModelJudge<'a> {
    pub cfg: &'a AiCfg,
}

impl Judge for ModelJudge<'_> {
    fn judge(&mut self, request: &str, figure: &[u8], reference: Option<&[u8]>) -> Result<Verdict, String> {
        let mut images: Vec<&[u8]> = vec![figure];
        let mut prompt = format!(
            r#"You check a figure made for an exam question. Request: {request:?}
Image 1 is the figure that was made."#
        );
        if let Some(r) = reference {
            images.push(r);
            prompt.push_str(" Image 2 is the reference picture it was supposed to reproduce; compare the two shapes, labels, numbers and layout.");
        }
        prompt.push_str(
            r#" Judge only what you can see. Does the figure show everything the request asks for, with the right quantities, labels and orientation, and nothing misleading? Reply with one JSON object: {"matches": true or false, "problems": ["short problem", ...]}. Use an empty list when it matches."#,
        );
        let raw = crate::ai::ask_images(&images, self.cfg, &prompt, 0.0)?;
        let start = raw.find('{').ok_or("the judge sent no JSON")?;
        let v: Json = serde_json::Deserializer::from_str(&raw[start..]).into_iter::<Json>().next().ok_or("the judge sent no JSON")?.map_err(|e| e.to_string())?;
        Ok(Verdict {
            matches: v.get("matches").and_then(Json::as_bool).ok_or("the judge's reply has no matches field")?,
            problems: v.get("problems").and_then(Json::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        })
    }
}

// ---------- trying again ----------

/// Attempts made at an image-to-figure request before settling.
pub const MAX_TRIES: usize = 3;

/// The same request again, told what the earlier attempts drew and what went wrong with them.
fn retry_request(request: &str, earlier: &[String]) -> String {
    let mut s = request.to_string();
    s.push_str("\n\nEarlier attempts did not work:");
    for (i, e) in earlier.iter().enumerate() {
        s.push_str(&format!("\n- Attempt {}: {e}", i + 1));
    }
    s.push_str("\nLook at the reference picture again, slowly, and change what was wrong rather than repeating it. Read counts and labels from the picture itself.");
    s
}

fn describe(spec: &FigureSpec) -> String {
    format!("{} {}", spec.id, serde_json::to_string(&spec.params).unwrap_or_default())
}

/// [`run_with_reference`] with up to `tries` attempts. An attempt is tried again when it ends in an
/// error, or when `judge` says the figure does not match the reference; the next attempt is told what
/// the earlier ones drew and what the reviewer saw. It stops early when the model declines (that is an
/// answer), when the reviewer cannot be reached, or when an attempt redraws a figure already tried (the
/// model is standing by it). The result is the last attempt that made a figure, else the last attempt.
pub fn run_with_retries(
    request: &str,
    reference: Option<&[u8]>,
    chat: &mut dyn Chat,
    mut judge: Option<&mut dyn Judge>,
    tries: usize,
) -> AgentRun {
    let tries = tries.max(1);
    let mut earlier: Vec<String> = Vec::new();
    let mut drawn: Vec<FigureSpec> = Vec::new();
    let mut best: Option<AgentRun> = None;
    let mut last = AgentRun::default();
    for n in 1..=tries {
        let ask = if earlier.is_empty() { request.to_string() } else { retry_request(request, &earlier) };
        let mut run = run_with_reference(&ask, reference, chat);
        run.tries = n;
        if run.declined.is_some() {
            return run;
        }
        let Some(spec) = run.spec.clone() else {
            earlier.push(format!("no figure ({})", run.error.as_deref().unwrap_or("none made")));
            last = run;
            continue;
        };
        let seen = drawn.contains(&spec);
        drawn.push(spec.clone());
        let verdict = match (judge.as_mut(), run.png.as_deref()) {
            (Some(j), Some(fig)) if reference.is_some() => j.judge(request, fig, reference).ok(),
            _ => None,
        };
        run.review = verdict.clone();
        match verdict {
            // No reviewer, or none reachable: the first figure is all there is to go on.
            None => return run,
            Some(v) if v.matches => return run,
            Some(v) => {
                if seen || n == tries {
                    return run;
                }
                let why = if v.problems.is_empty() { "it does not match".to_string() } else { v.problems.join("; ") };
                earlier.push(format!("drew {}; a reviewer comparing it with the reference said: {why}", describe(&spec)));
                best = Some(run);
            }
        }
    }
    // Ran out of tries without a figure on the last one: the last drawn figure beats nothing.
    best.map(|mut b| {
        b.tries = tries;
        b
    })
    .unwrap_or(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replays canned replies and records what the loop sent.
    struct Script {
        replies: Vec<String>,
        seen: Vec<Vec<Turn>>,
    }

    impl Script {
        fn new(replies: &[&str]) -> Script {
            Script { replies: replies.iter().rev().map(|s| s.to_string()).collect(), seen: Vec::new() }
        }
    }

    impl Chat for Script {
        fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
            self.seen.push(turns.to_vec());
            self.replies.pop().ok_or_else(|| "script ran out".to_string())
        }
    }

    /// A figure with at least one numeric parameter, so range checks have something to bite on.
    fn numeric_figure() -> (&'static FigureDef, ParamDef) {
        registry()
            .iter()
            .find_map(|d| (d.params)().into_iter().find(|p| matches!(p.kind, Kind::Float { .. })).map(|p| (d, p)))
            .expect("some figure has a float param")
    }

    #[test]
    fn full_loop_lists_reads_renders_and_finishes() {
        let (d, p) = numeric_figure();
        let id = d.id;
        let render = format!(r#"{{"tool":"render_figure","args":{{"id":"{id}","params":{{}}}}}}"#);
        let fin = format!(r#"{{"final":{{"id":"{id}","params":{{}},"note":"done"}}}}"#);
        let get = format!(r#"{{"tool":"get_figure","args":{{"id":"{id}"}}}}"#);
        let mut chat = Script::new(&[r#"{"tool":"list_figures","args":{}}"#, &get, &render, &fin]);
        let run = run("anything", &mut chat);
        assert!(run.error.is_none(), "{:?}", run.error);
        let spec = run.spec.expect("spec");
        assert_eq!(spec.id, id);
        assert!(spec.params.contains_key(p.key));
        assert!(run.png.as_ref().is_some_and(|b| b.starts_with(b"\x89PNG")));
        assert_eq!(run.note, "done");
        assert_eq!(run.steps.iter().map(|s| s.tool.as_str()).collect::<Vec<_>>(), ["list_figures", "get_figure", "render_figure", "final"]);
        // The render result came back to the model with the picture attached.
        let last = chat.seen.last().unwrap().last().unwrap();
        assert_eq!(last.role, "user");
        assert_eq!(last.images.len(), 1);
    }

    #[test]
    fn out_of_range_and_unknown_params_are_reported_back() {
        let (d, p) = numeric_figure();
        let Kind::Float { max, .. } = p.kind else { unreachable!() };
        let over = json!({ p.key: max + 1000.0, "no_such_param": 1 });
        let (params, notes) = build_params(d, &over);
        assert_eq!(params.f(p.key), max);
        assert!(notes.iter().any(|n| n.contains("no_such_param")), "{notes:?}");
        assert!(notes.iter().any(|n| n.contains(p.key) && n.contains("asked")), "{notes:?}");
    }

    #[test]
    fn wrong_type_keeps_default_and_says_so() {
        let (d, p) = numeric_figure();
        let (params, notes) = build_params(d, &json!({ p.key: "lots" }));
        assert_eq!(params.f(p.key), figures::start_params(d).f(p.key));
        assert!(notes.iter().any(|n| n.contains("wrong type")), "{notes:?}");
    }

    #[test]
    fn bad_replies_get_one_correction_then_recover() {
        let (d, _) = numeric_figure();
        let fin = format!(r#"{{"final":{{"id":"{}","params":{{}},"note":"ok"}}}}"#, d.id);
        let mut chat = Script::new(&["I will draw a graph!", &fin]);
        let run = run("x", &mut chat);
        assert!(run.spec.is_some());
        assert_eq!(run.steps[0].tool, "invalid");
        assert!(chat.seen[1].last().unwrap().text.starts_with("Error:"));
    }

    #[test]
    fn gives_up_after_repeated_garbage() {
        let mut chat = Script::new(&["nope", "still nope", "no json here either", "{}"]);
        let run = run("x", &mut chat);
        assert!(run.spec.is_none());
        assert!(run.error.as_deref().is_some_and(|e| e.contains("invalid replies")), "{:?}", run.error);
    }

    #[test]
    fn unknown_figure_id_is_an_error_the_model_can_read() {
        let mut chat = Script::new(&[r#"{"tool":"get_figure","args":{"id":"nope"}}"#, r#"{"tool":"list_figures"}"#]);
        let run = run("x", &mut chat);
        assert!(run.error.is_some()); // script ran out: the loop was still going, which is the point
        assert!(chat.seen[1].last().unwrap().text.contains("no figure with id"));
    }

    #[test]
    fn turn_limit_keeps_the_last_render() {
        let (d, _) = numeric_figure();
        let render = format!(r#"{{"tool":"render_figure","args":{{"id":"{}"}}}}"#, d.id);
        let replies: Vec<&str> = std::iter::repeat_n(render.as_str(), MAX_STEPS).collect();
        let mut chat = Script::new(&replies);
        let run = run("x", &mut chat);
        assert!(run.error.is_none());
        assert!(run.spec.is_some());
        assert!(run.note.contains("turn limit"));
    }

    #[test]
    fn model_failure_surfaces_cleanly() {
        let mut chat = Script::new(&[]);
        let run = run("x", &mut chat);
        assert!(run.error.as_deref().is_some_and(|e| e.contains("model call failed")));
    }

    #[test]
    fn a_clamped_final_is_flagged_in_the_note_even_if_the_model_said_nothing() {
        let (d, p) = numeric_figure();
        let Kind::Float { max, .. } = p.kind else { unreachable!() };
        let fin = format!(r#"{{"final":{{"id":"{}","params":{{"{}":{}}},"note":""}}}}"#, d.id, p.key, max + 500.0);
        let run = run("x", &mut Script::new(&[&fin, &fin]));
        assert!(run.note.starts_with("Adjusted:") && run.note.contains(p.key), "{:?}", run.note);
        let said = format!(r#"{{"final":{{"id":"{}","params":{{"{}":{}}},"note":"Used the limit."}}}}"#, d.id, p.key, max + 500.0);
        let run = super::run("x", &mut Script::new(&[&said]));
        assert!(run.note.starts_with("Used the limit. Adjusted:"), "{:?}", run.note);
    }

    #[test]
    fn an_empty_final_note_is_asked_for_once_and_then_accepted() {
        let (d, _) = numeric_figure();
        let empty = format!(r#"{{"final":{{"id":"{}","params":{{}},"note":""}}}}"#, d.id);
        let noted = format!(r#"{{"final":{{"id":"{}","params":{{}},"note":"Made it."}}}}"#, d.id);
        let mut chat = Script::new(&[&empty, &noted]);
        let r = run("x", &mut chat);
        assert_eq!(r.note, "Made it.");
        assert!(chat.seen[1].last().unwrap().text.contains("empty note"));
        // A model that stays silent after being asked still gets its figure.
        let r = super::run("x", &mut Script::new(&[&empty, &empty]));
        assert!(r.spec.is_some() && r.note.is_empty());
    }

    #[test]
    fn a_note_beside_final_counts_as_the_note() {
        let (d, _) = numeric_figure();
        let beside = format!(r#"{{"final":{{"id":"{}","params":{{}}}},"note":"Made it."}}"#, d.id);
        let mut chat = Script::new(&[&beside]);
        let r = run("x", &mut chat);
        assert_eq!(r.note, "Made it.");
        assert_eq!(chat.seen.len(), 1, "no extra turn spent asking for a note");
    }

    #[test]
    fn a_reply_with_no_command_gets_a_specific_correction() {
        let (d, _) = numeric_figure();
        let fin = format!(r#"{{"final":{{"id":"{}","params":{{}},"note":"ok"}}}}"#, d.id);
        let mut chat = Script::new(&[r#"{"sees":"a thing"}"#, &fin]);
        let run = run("x", &mut chat);
        assert!(run.spec.is_some());
        assert!(chat.seen[1].last().unwrap().text.contains("no \"tool\""));
    }

    #[test]
    fn a_second_object_after_the_first_is_ignored() {
        let v = parse_reply(r#"{"tool":"get_figure","args":{"id":"triangle"}} {"final":{"id":"x"}}"#).unwrap();
        assert_eq!(v["tool"], "get_figure");
        assert!(parse_reply("{broken").is_err());
    }

    #[test]
    fn fenced_replies_parse() {
        let v = parse_reply("```json\n{\"tool\":\"list_figures\"}\n```").unwrap();
        assert_eq!(v["tool"], "list_figures");
    }

    // ---------- retries ----------

    /// Answers the queued verdicts in order; `None` entries are a reviewer that cannot be reached.
    struct Reviewer {
        verdicts: Vec<Option<(bool, &'static str)>>,
        asked: usize,
    }

    impl Judge for Reviewer {
        fn judge(&mut self, _: &str, _: &[u8], _: Option<&[u8]>) -> Result<Verdict, String> {
            self.asked += 1;
            match self.verdicts.remove(0) {
                Some((matches, why)) => Ok(Verdict { matches, problems: if why.is_empty() { vec![] } else { vec![why.to_string()] } }),
                None => Err("unreachable".into()),
            }
        }
    }

    const PIC: &[u8] = b"reference";

    fn final_for(id: &str, params: &str) -> String {
        format!(r#"{{"final":{{"id":"{id}","params":{params},"note":"ok"}}}}"#)
    }

    /// Two different figures to tell attempts apart.
    fn two_figures() -> (String, String) {
        let mut it = registry().iter();
        (it.next().unwrap().id.to_string(), it.next().unwrap().id.to_string())
    }

    #[test]
    fn a_reviewed_mismatch_is_tried_again_and_told_what_went_wrong() {
        let (a, b) = two_figures();
        let mut chat = Script::new(&[&final_for(&a, "{}"), &final_for(&b, "{}")]);
        let mut rev = Reviewer { verdicts: vec![Some((false, "drew four atoms, the picture has three")), Some((true, ""))], asked: 0 };
        let r = run_with_retries("copy it", Some(PIC), &mut chat, Some(&mut rev), 3);
        assert_eq!(r.tries, 2);
        assert_eq!(r.spec.as_ref().map(|s| s.id.as_str()), Some(b.as_str()));
        assert!(r.review.as_ref().is_some_and(|v| v.matches));
        let second = &chat.seen[1][1].text;
        assert!(second.starts_with("Request: copy it"), "{second}");
        assert!(second.contains("Attempt 1: drew") && second.contains(&a) && second.contains("drew four atoms"), "{second}");
    }

    #[test]
    fn it_stops_after_three_tries_and_keeps_the_last_figure() {
        let (a, b) = two_figures();
        let c = registry().iter().nth(2).unwrap().id.to_string();
        let mut chat = Script::new(&[&final_for(&a, "{}"), &final_for(&b, "{}"), &final_for(&c, "{}")]);
        let mut rev = Reviewer { verdicts: vec![Some((false, "x")), Some((false, "y")), Some((false, "z"))], asked: 0 };
        let r = run_with_retries("r", Some(PIC), &mut chat, Some(&mut rev), MAX_TRIES);
        assert_eq!((r.tries, rev.asked), (3, 3));
        assert_eq!(r.spec.as_ref().map(|s| s.id.as_str()), Some(c.as_str()));
        assert!(r.review.as_ref().is_some_and(|v| !v.matches));
    }

    #[test]
    fn a_model_that_redraws_the_same_figure_is_not_asked_again() {
        let (a, _) = two_figures();
        let mut chat = Script::new(&[&final_for(&a, "{}"), &final_for(&a, "{}"), &final_for(&a, "{}")]);
        let mut rev = Reviewer { verdicts: vec![Some((false, "x")), Some((false, "x")), Some((false, "x"))], asked: 0 };
        let r = run_with_retries("r", Some(PIC), &mut chat, Some(&mut rev), 3);
        assert_eq!(r.tries, 2, "the second identical figure ends it");
    }

    #[test]
    fn a_decline_an_unreachable_reviewer_and_a_text_request_never_retry() {
        let (a, _) = two_figures();
        let r = run_with_retries("r", Some(PIC), &mut Script::new(&[r#"{"cannot":"no generator"}"#]), None, 3);
        assert!(r.declined.is_some() && r.tries == 1);
        let mut rev = Reviewer { verdicts: vec![None], asked: 0 };
        let r = run_with_retries("r", Some(PIC), &mut Script::new(&[&final_for(&a, "{}")]), Some(&mut rev), 3);
        assert!(r.spec.is_some() && r.tries == 1 && r.review.is_none());
        // Without a reference there is nothing to compare against, so the reviewer is not asked.
        let mut rev = Reviewer { verdicts: vec![Some((false, "x"))], asked: 0 };
        let r = run_with_retries("r", None, &mut Script::new(&[&final_for(&a, "{}")]), Some(&mut rev), 3);
        assert!(r.spec.is_some() && rev.asked == 0);
    }

    #[test]
    fn a_failed_attempt_is_tried_again_and_an_all_failed_run_reports_the_last_error() {
        let (a, _) = two_figures();
        let mut chat = Script::new(&["junk", "junk", "junk", &final_for(&a, "{}")]);
        let mut rev = Reviewer { verdicts: vec![Some((true, ""))], asked: 0 };
        let r = run_with_retries("r", Some(PIC), &mut chat, Some(&mut rev), 3);
        assert!(r.spec.is_some() && r.tries == 2);
        assert!(chat.seen[3][1].text.contains("no figure"), "{}", chat.seen[3][1].text);
        let r = run_with_retries("r", Some(PIC), &mut Script::new(&["junk"; 9]), None, 3);
        assert!(r.spec.is_none() && r.error.is_some());
    }
}
