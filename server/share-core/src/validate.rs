//! `validate` and `validate_for_publish`: the gate every pool payload passes (docs/shared-pool.md,
//! "Only the server strips", "Atomic import", "Limits").
//!
//! * [`validate`] is what the **importing client** runs on every payload it receives, and the first
//!   half of what the server runs: strict reading along the schema (`json`), the Limits table, the
//!   character rules (`text`), figure params reduced by the caller's [`FigureRegistry`], and
//!   **well-formedness** (below). It accepts keyless (stripped) questions.
//! * [`validate_for_publish`] is what the **server** runs on an upload: `validate`, plus **key
//!   completeness** for every question that is not keyless, plus the keyless rule.
//!
//! The accepted shape is exactly the app's serde shape (same required fields, same enum tagging,
//! `null` only where the app writes it). Unknown fields are dropped and reported by the path of
//! their object, never by name; schema-v2 fields are unknown in a v1 payload. Every error has a
//! code and a path made of schema names and indices, plus at most a numeric limit, a character
//! offset and a code point (`"U+202E"`): **no error echoes input text** (ASVS 5.0 16.5.1).
//!
//! # Well-formedness (both entry points)
//! | Where | Rule | Code |
//! |---|---|---|
//! | every stem, part stem, answer text, match item, distractor, accepted blank | at least one visible character (`text::has_visible`) | `blank_text` |
//! | `Mc` (and an `Mc` part) | at least 2 answers | `too_few` (limit 2) |
//! | `Matching` | at least 2 pairs; every right side visible unless the question is keyless | `too_few`, `blank_text` |
//! | `Blank` | `accept.len()` equals the number of `___` in the stem (as the app counts them, `str::matches`), at least 1 | `blank_count_mismatch`, `too_few` |
//! | `Multipart` | at least 1 part | `too_few` |
//!
//! The app's editor (`src/qedit.rs`, `Draft::validate`) enforces the same before it saves, except
//! that it lets a multipart `Mc` part have fewer than two answers; a pool payload may not.
//!
//! # Key completeness (`validate_for_publish`, questions without `keyless`)
//! | Kind | Rule | Code |
//! |---|---|---|
//! | `Mc` and `Mc` parts | at least one `correct` | `no_correct` |
//! | `Blank` | every accept list non-empty | `no_correct` |
//! | `Matching` | (rights visible, above) | |
//!
//! # The keyless rule (`validate_for_publish`)
//! A question with `keyless: true`, or whose explanation is the keyless sentinel, has no real key.
//! It may be published only with `hide_key` (the stripped form is all anyone gets); otherwise it is
//! refused with `keyless_needs_hide_key`, so a placeholder key is never served as a real one.
use crate::figures::{FigureRegistry, ParamKind, is_figure_id, is_param_key};
use crate::json::{self, Node, Obj, ParseError};
use crate::limits::Limits;
use crate::text::{self, Class, Max, TextError};
use crate::wire::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Licences a publish may carry (docs/shared-pool.md, "Sharing model").
pub const LICENSES: [&str; 6] = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0", "circle-only"];

/// More errors than this are not collected.
pub const MAX_ERRORS: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ValidationError {
    pub code: &'static str,
    /// Schema field names and indices only, e.g. `bank.questions[3].kind.Mc.answers[0].text`.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    /// `forbidden_char`, `mark_run`: index of the character in the field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    /// The same position in UTF-16 code units, for JavaScript callers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset16: Option<u64>,
    /// `forbidden_char`: the refused code point, e.g. `"U+202E"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cp: Option<String>,
}

impl ValidationError {
    pub fn new(code: &'static str, path: &str) -> Self {
        ValidationError { code, path: path.to_string(), limit: None, offset: None, offset16: None, cp: None }
    }
}

/// Something `validate` removed or changed instead of refusing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Dropped {
    pub path: String,
    /// `unknown_field`, `figure_unknown`, `param_unknown`, `param_type`, `param_clamped`,
    /// `param_reset`, `param_spec_invalid`.
    pub reason: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Validated {
    pub payload: Payload,
    pub dropped: Vec<Dropped>,
}

/// What the server knows about a publish that the payload does not say.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublishOptions {
    pub hide_key: bool,
    /// The author's label decisions (`labels`); required for every finding when `hide_key`.
    pub labels: Vec<crate::strip::LabelDecision>,
}

struct Cx<'a> {
    lim: &'a Limits,
    reg: &'a dyn FigureRegistry,
    v2: bool,
    errors: Vec<ValidationError>,
    dropped: Vec<Dropped>,
    /// Bytes of normalised text kept so far (NFC can triple a text): at most `body_bytes`.
    text_bytes: usize,
    text_over: bool,
    /// Declared registry: key -> spec index, per figure id seen.
    def_index: std::collections::HashMap<String, std::collections::HashMap<String, usize>>,
}

fn at(path: &str, key: &str) -> String {
    format!("{path}.{key}")
}
fn idx(path: &str, i: usize) -> String {
    format!("{path}[{i}]")
}

fn canonical_number(n: f64) -> serde_json::Value {
    let n = if n == 0.0 { 0.0 } else { n }; // -0.0 becomes 0.0
    if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_992.0 {
        serde_json::Value::from(n as i64)
    } else {
        serde_json::Number::from_f64(n).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null)
    }
}

impl Cx<'_> {
    fn push(&mut self, e: ValidationError) {
        if self.errors.len() < MAX_ERRORS {
            self.errors.push(e);
        }
    }
    fn err(&mut self, code: &'static str, path: &str) {
        self.push(ValidationError::new(code, path));
    }
    fn err_lim(&mut self, code: &'static str, path: &str, limit: u64) {
        self.push(ValidationError { limit: Some(limit), ..ValidationError::new(code, path) });
    }
    fn drop_(&mut self, path: &str, reason: &'static str) {
        if self.dropped.len() < MAX_ERRORS {
            self.dropped.push(Dropped { path: path.to_string(), reason });
        }
    }
    fn text_err(&mut self, e: TextError, path: &str, max: Max) {
        match e {
            TextError::Forbidden { offset, offset16, cp } => self.push(ValidationError {
                offset: Some(offset as u64),
                offset16: Some(offset16 as u64),
                cp: Some(format!("U+{cp:04X}")),
                ..ValidationError::new("forbidden_char", path)
            }),
            TextError::MarkRun { offset, offset16 } => self.push(ValidationError {
                offset: Some(offset as u64),
                offset16: Some(offset16 as u64),
                limit: Some(text::MAX_MARK_RUN as u64),
                ..ValidationError::new("mark_run", path)
            }),
            TextError::TooLong => self.err_lim("too_long", path, max.limit()),
        }
    }

    /// `v` as an object; keys outside `allowed` (or `allowed_v2` in a v2 payload) are dropped.
    fn object<'v>(&mut self, v: &'v Node, path: &str, allowed: &[&str], allowed_v2: &[&str]) -> Option<&'v Obj> {
        let Some(m) = v.as_object() else {
            self.err("expected_object", path);
            return None;
        };
        let stray = m.entries.iter().any(|(k, _)| match k.as_str() {
            Some(k) => !allowed.contains(&k) && !(self.v2 && allowed_v2.contains(&k)),
            None => true,
        });
        if m.dropped || stray {
            self.drop_(path, "unknown_field");
        }
        Some(m)
    }

    fn required<'v>(&mut self, m: &'v Obj, key: &str, path: &str) -> Option<&'v Node> {
        let v = m.get(key);
        if v.is_none() {
            self.err("missing", &at(path, key));
        }
        v
    }

    fn string(&mut self, v: &Node, path: &str, class: Class, max: Max) -> String {
        let Some(s) = v.as_str() else {
            self.err("expected_string", path);
            return String::new();
        };
        // NFC may replace a character with its canonical equivalent (OHM SIGN with omega, a
        // CJK compatibility ideograph with the unified one): it is stored normalised and
        // reported, so the author knows what was stored.
        if !unicode_normalization::is_nfc(s) {
            self.drop_(path, "nfc_changed");
        }
        match text::clean(s, class, max) {
            Ok(c) => {
                self.text_bytes = self.text_bytes.saturating_add(c.len());
                if self.text_bytes > self.lim.body_bytes && !self.text_over {
                    self.text_over = true;
                    self.err_lim("normalized_too_large", "", self.lim.body_bytes as u64);
                }
                c
            }
            Err(e) => {
                self.text_err(e, path, max);
                String::new()
            }
        }
    }

    /// A string that must also contain a visible character.
    fn visible(&mut self, v: &Node, path: &str, max: Max) -> String {
        let s = self.string(v, path, Class::Block, max);
        if v.as_str().is_some() && !text::has_visible(&s) && self.errors.last().is_none_or(|e| e.path != path) {
            self.err("blank_text", path);
        }
        s
    }

    fn req_string(&mut self, m: &Obj, key: &str, path: &str, class: Class, max: Max) -> String {
        match self.required(m, key, path) {
            Some(v) => self.string(v, &at(path, key), class, max),
            None => String::new(),
        }
    }

    fn req_visible(&mut self, m: &Obj, key: &str, path: &str, max: Max) -> String {
        match self.required(m, key, path) {
            Some(v) => self.visible(v, &at(path, key), max),
            None => String::new(),
        }
    }

    fn opt_string(&mut self, m: &Obj, key: &str, path: &str, class: Class, max: Max) -> String {
        match m.get(key) {
            Some(v) => self.string(v, &at(path, key), class, max),
            None => String::new(),
        }
    }

    fn id(&mut self, v: &Node, path: &str) -> String {
        let Some(s) = v.as_str() else {
            self.err("expected_string", path);
            return String::new();
        };
        if !is_id(s, self.lim.id_chars) {
            self.err("bad_id", path);
            return String::new();
        }
        s.to_string()
    }

    fn req_id(&mut self, m: &Obj, key: &str, path: &str) -> String {
        match self.required(m, key, path) {
            Some(v) => self.id(v, &at(path, key)),
            None => String::new(),
        }
    }

    fn boolean(&mut self, v: &Node, path: &str) -> bool {
        match v.as_bool() {
            Some(b) => b,
            None => {
                self.err("expected_bool", path);
                false
            }
        }
    }

    fn opt_bool(&mut self, m: &Obj, key: &str, path: &str) -> bool {
        match m.get(key) {
            Some(v) => self.boolean(v, &at(path, key)),
            None => false,
        }
    }

    fn i64_(&mut self, m: &Obj, key: &str, path: &str) -> i64 {
        match self.required(m, key, path) {
            Some(v) => match v.as_i64() {
                Some(n) => n,
                None => {
                    self.err("expected_integer", &at(path, key));
                    0
                }
            },
            None => 0,
        }
    }

    fn u32_(&mut self, m: &Obj, key: &str, path: &str) -> u32 {
        match self.required(m, key, path) {
            Some(v) => match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
                Some(n) => n,
                None => {
                    self.err("expected_integer", &at(path, key));
                    0
                }
            },
            None => 0,
        }
    }

    fn array<'v>(&mut self, v: &'v Node, path: &str, max: usize) -> &'v [Node] {
        match v.as_array() {
            Some(a) if a.len() > max => {
                self.err_lim("too_many", path, max as u64);
                &[]
            }
            Some(a) => a,
            None => {
                self.err("expected_array", path);
                &[]
            }
        }
    }

    fn req_array<'v>(&mut self, m: &'v Obj, key: &str, path: &str, max: usize) -> &'v [Node] {
        match self.required(m, key, path) {
            Some(v) => self.array(v, &at(path, key), max),
            None => &[],
        }
    }

    fn at_least(&mut self, n: usize, min: usize, path: &str) {
        if n < min {
            self.err_lim("too_few", path, min as u64);
        }
    }

    // ---- schema -------------------------------------------------------------------------------

    fn answers(&mut self, v: &Node, path: &str) -> Vec<Answer> {
        let max = self.lim.mc_answers;
        let len = Max::Bytes(self.lim.answer_bytes);
        let arr = self.array(v, path, max);
        let mut out = Vec::with_capacity(arr.len());
        for (i, a) in arr.iter().enumerate() {
            let p = idx(path, i);
            let Some(m) = self.object(a, &p, &["text", "correct", "locked"], &[]) else { continue };
            let text = self.req_visible(m, "text", &p, len);
            let correct = self.opt_bool(m, "correct", &p);
            let locked = self.opt_bool(m, "locked", &p);
            out.push(Answer { text, correct, locked });
        }
        if v.as_array().is_some_and(|a| a.len() <= max) {
            self.at_least(arr.len(), 2, path);
        }
        out
    }

    /// An externally tagged enum value: exactly one known key.
    fn tagged<'v>(&mut self, v: &'v Node, path: &str, tags: &[&'static str]) -> Option<(&'static str, &'v Node)> {
        let Some(m) = v.as_object() else {
            self.err("expected_object", path);
            return None;
        };
        if m.len() != 1 || m.dropped {
            self.err(if m.is_empty() { "missing" } else { "ambiguous_kind" }, path);
            return None;
        }
        let (k, inner) = m.entries.first()?;
        match k.as_str().and_then(|k| tags.iter().find(|t| **t == k)) {
            Some(t) => Some((t, inner)),
            None => {
                self.err("unknown_kind", path);
                None
            }
        }
    }

    fn mc(&mut self, inner: &Node, p: &str) -> Option<Vec<Answer>> {
        let m = self.object(inner, p, &["answers"], &[])?;
        Some(match self.required(m, "answers", p) {
            Some(a) => self.answers(a, &at(p, "answers")),
            None => Vec::new(),
        })
    }

    fn tf(&mut self, inner: &Node, p: &str) -> Option<bool> {
        let m = self.object(inner, p, &["correct"], &[])?;
        Some(match self.required(m, "correct", p) {
            Some(c) => self.boolean(c, &at(p, "correct")),
            None => false,
        })
    }

    fn short(&mut self, inner: &Node, p: &str) -> Option<String> {
        let m = self.object(inner, p, &["sample"], &[])?;
        let len = Max::Bytes(self.lim.answer_bytes);
        Some(self.req_string(m, "sample", p, Class::Block, len))
    }

    fn part_kind(&mut self, v: &Node, path: &str) -> PartKind {
        let fallback = PartKind::Short { sample: String::new() };
        let Some((tag, inner)) = self.tagged(v, path, &["Mc", "Tf", "Short"]) else { return fallback };
        let p = at(path, tag);
        match tag {
            "Mc" => self.mc(inner, &p).map(|answers| PartKind::Mc { answers }),
            "Tf" => self.tf(inner, &p).map(|correct| PartKind::Tf { correct }),
            _ => self.short(inner, &p).map(|sample| PartKind::Short { sample }),
        }
        .unwrap_or(fallback)
    }

    fn kind(&mut self, v: &Node, path: &str, keyless: bool, stem: &str) -> Kind {
        let fallback = Kind::Short { sample: String::new() };
        let tags = ["Mc", "Tf", "Matching", "Blank", "Short", "Multipart"];
        let Some((tag, inner)) = self.tagged(v, path, &tags) else { return fallback };
        let p = at(path, tag);
        let ans = Max::Bytes(self.lim.answer_bytes);
        match tag {
            "Mc" => self.mc(inner, &p).map(|answers| Kind::Mc { answers }).unwrap_or(fallback),
            "Tf" => self.tf(inner, &p).map(|correct| Kind::Tf { correct }).unwrap_or(fallback),
            "Short" => self.short(inner, &p).map(|sample| Kind::Short { sample }).unwrap_or(fallback),
            "Matching" => {
                let Some(m) = self.object(inner, &p, &["pairs", "distractors"], &[]) else { return fallback };
                let max_pairs = self.lim.matching_pairs;
                let pp = at(&p, "pairs");
                let parr = self.req_array(m, "pairs", &p, max_pairs);
                let mut pairs = Vec::with_capacity(parr.len());
                for (i, pv) in parr.iter().enumerate() {
                    let ip = idx(&pp, i);
                    let Some(pm) = self.object(pv, &ip, &["left", "right"], &[]) else { continue };
                    let left = self.req_visible(pm, "left", &ip, ans);
                    // Keyless: "" (served) or a visible item (the importer's layout); never blank.
                    let right = if keyless {
                        let r = self.req_string(pm, "right", &ip, Class::Block, ans);
                        if !r.is_empty() && !text::has_visible(&r) {
                            self.err("blank_text", &at(&ip, "right"));
                        }
                        r
                    } else {
                        self.req_visible(pm, "right", &ip, ans)
                    };
                    pairs.push(Pair { left, right });
                }
                if m.get("pairs").and_then(Node::as_array).is_some_and(|a| a.len() <= max_pairs) {
                    self.at_least(parr.len(), 2, &pp);
                }
                // Right-hand items: the pairs' rights plus distractors, at most pairs + the
                // distractor limit, whatever `keyless` says (a stripped word bank fits, and stays
                // valid after the key is re-entered or `keyless` is lost).
                let max_d = parr.len().saturating_add(self.lim.matching_distractors);
                let dp = at(&p, "distractors");
                let darr = self.req_array(m, "distractors", &p, max_d);
                let mut distractors = Vec::with_capacity(darr.len());
                for (i, d) in darr.iter().enumerate() {
                    let v = self.visible(d, &idx(&dp, i), ans);
                    // A keyless word bank lists each item once, and not again among the rights.
                    if keyless && !v.is_empty() && (distractors.contains(&v) || pairs.iter().any(|p: &Pair| p.right == v)) {
                        self.err("duplicate_choice", &idx(&dp, i));
                    }
                    distractors.push(v);
                }
                // The word bank strip serves is the distinct visible right items and distractors,
                // so that union (not just the distractor list) must fit pairs + the distractor
                // limit, or the stripped form would be refused by every importer.
                let cap = parr.len().saturating_add(self.lim.matching_distractors);
                let union: BTreeSet<&str> = pairs.iter().map(|p| p.right.as_str()).chain(distractors.iter().map(String::as_str)).filter(|s| text::has_visible(s)).collect();
                if union.len() > cap && darr.len() <= max_d {
                    self.err_lim("too_many", &dp, cap as u64);
                }
                Kind::Matching { pairs, distractors }
            }
            "Blank" => {
                let Some(m) = self.object(inner, &p, &["accept"], &[]) else { return fallback };
                let (max_b, max_v) = (self.lim.blanks, self.lim.accept_variants);
                let ap = at(&p, "accept");
                let barr = self.req_array(m, "accept", &p, max_b);
                let mut accept = Vec::with_capacity(barr.len());
                for (i, b) in barr.iter().enumerate() {
                    let bp = idx(&ap, i);
                    let varr = self.array(b, &bp, max_v);
                    let mut vs = Vec::with_capacity(varr.len());
                    for (j, s) in varr.iter().enumerate() {
                        vs.push(self.visible(s, &idx(&bp, j), ans));
                    }
                    accept.push(vs);
                }
                if m.get("accept").and_then(Node::as_array).is_some_and(|a| a.len() <= max_b) {
                    let markers = stem.matches("___").count();
                    if markers == 0 {
                        self.err_lim("too_few", &ap, 1);
                    } else if markers != accept.len() {
                        self.err_lim("blank_count_mismatch", &ap, markers as u64);
                    }
                }
                Kind::Blank { accept }
            }
            _ => {
                let Some(m) = self.object(inner, &p, &["parts"], &[]) else { return fallback };
                let max = self.lim.parts;
                let stem_len = Max::Bytes(self.lim.stem_bytes);
                let pp = at(&p, "parts");
                let arr = self.req_array(m, "parts", &p, max);
                let mut parts = Vec::with_capacity(arr.len());
                for (i, pv) in arr.iter().enumerate() {
                    let ip = idx(&pp, i);
                    let Some(pm) = self.object(pv, &ip, &["stem", "kind"], &[]) else { continue };
                    let stem = self.req_visible(pm, "stem", &ip, stem_len);
                    let kind = match self.required(pm, "kind", &ip) {
                        Some(k) => self.part_kind(k, &at(&ip, "kind")),
                        None => PartKind::Short { sample: String::new() },
                    };
                    parts.push(Part { stem, kind });
                }
                if m.get("parts").and_then(Node::as_array).is_some_and(|a| a.len() <= max) {
                    self.at_least(arr.len(), 1, &pp);
                }
                Kind::Multipart { parts }
            }
        }
    }

    fn image(&mut self, v: &Node, path: &str) -> Option<ImageRef> {
        if v.is_null() {
            return None;
        }
        let m = self.object(v, path, &["file", "width", "height"], &[])?;
        let file = match self.required(m, "file", path) {
            Some(f) => match f.as_str() {
                Some(s) if is_media_file(s) => s.to_string(),
                Some(_) => {
                    self.err("bad_image_file", &at(path, "file"));
                    String::new()
                }
                None => {
                    self.err("expected_string", &at(path, "file"));
                    String::new()
                }
            },
            None => String::new(),
        };
        let width = self.side(m, "width", path);
        let height = self.side(m, "height", path);
        Some(ImageRef { file, width, height })
    }

    fn side(&mut self, m: &Obj, key: &str, path: &str) -> u32 {
        let max = self.lim.image_side;
        match self.required(m, key, path) {
            Some(v) => match v.as_u64() {
                Some(n) if (1..=max as u64).contains(&n) => n as u32,
                Some(_) => {
                    self.err_lim("out_of_range", &at(path, key), max as u64);
                    0
                }
                None => {
                    self.err("expected_integer", &at(path, key));
                    0
                }
            },
            None => 0,
        }
    }

    fn figure(&mut self, v: &Node, path: &str) -> Option<FigureSpec> {
        if v.is_null() {
            return None;
        }
        let m = self.object(v, path, &["id", "params"], &[])?;
        let id = match self.required(m, "id", path)?.as_str() {
            Some(s) if is_figure_id(s) => s.to_string(),
            Some(_) => {
                self.err("bad_figure_id", &at(path, "id"));
                return None;
            }
            None => {
                self.err("expected_string", &at(path, "id"));
                return None;
            }
        };
        let pp = at(path, "params");
        let empty = Obj::default();
        let raw: &Obj = match m.get("params") {
            None => &empty,
            Some(Node::Obj(o)) => o,
            Some(_) => {
                self.err("expected_object", &pp);
                return None;
            }
        };
        if raw.len() > self.lim.figure_params {
            self.err_lim("too_many", &pp, self.lim.figure_params as u64);
            return None;
        }
        let text_len = Max::Bytes(self.lim.figure_text_bytes);
        let text_path = at(&pp, "<text>");
        let mut params = BTreeMap::new();
        // The registry's specs are borrowed (no copy per figure), and looked up by key through an
        // index built once per figure id, so the work is linear in params, not params x specs.
        let reg: &dyn FigureRegistry = self.reg;
        let defs = reg.params(&id);
        if let Some(d) = defs
            && !self.def_index.contains_key(&id)
        {
            let idx: std::collections::HashMap<String, usize> = d.iter().enumerate().map(|(i, s)| (s.key.clone(), i)).collect();
            self.def_index.insert(id.clone(), idx);
        }
        match defs {
            None if self.reg.drop_unknown() => {
                self.drop_(path, "figure_unknown");
                return None;
            }
            None => {
                for (k, val) in &raw.entries {
                    let Some(k) = k.as_str().filter(|k| is_param_key(k)) else {
                        self.drop_(&pp, "param_unknown");
                        continue;
                    };
                    let kept = match val {
                        Node::Bool(b) => Some(serde_json::Value::Bool(*b)),
                        Node::Num(n) => match (n.as_u64(), n.as_i64(), n.as_f64()) {
                            (Some(u), _, _) => Some(serde_json::Value::from(u)),
                            (_, Some(i), _) => Some(serde_json::Value::from(i)),
                            (_, _, Some(f)) if f.is_finite() => Some(canonical_number(f)),
                            _ => None,
                        },
                        Node::Str(_) => Some(serde_json::Value::String(self.string(val, &text_path, Class::Block, text_len))),
                        _ => None,
                    };
                    match kept {
                        Some(x) => {
                            params.insert(k.to_string(), x);
                        }
                        None => self.drop_(&pp, "param_type"),
                    }
                }
            }
            Some(defs) => {
                let mut unknown = false;
                let found: Vec<(usize, &Node)> = {
                    let idx = &self.def_index[&id];
                    raw.entries.iter().filter_map(|(k, v)| match k.as_str().and_then(|k| idx.get(k)) {
                        Some(&i) => Some((i, v)),
                        None => {
                            unknown = true;
                            None
                        }
                    }).collect()
                };
                if unknown {
                    self.drop_(&pp, "param_unknown");
                }
                for (di, val) in found {
                    let d = &defs[di];
                    let spec_ok = match &d.kind {
                        ParamKind::Float { min, max } => min <= max,
                        ParamKind::Int { min, max } => min <= max,
                        ParamKind::Choice { options, default } => options.contains(default),
                        // The app's registry has colour defaults outside their palette (the
                        // mapping diagram's `arrow_color`); a `#rrggbb` default is accepted.
                        ParamKind::Color { palette, default } => palette.contains(default) || is_hex_color(default),
                        _ => true,
                    };
                    if !spec_ok {
                        self.drop_(&pp, "param_spec_invalid");
                        continue;
                    }
                    let kept = match &d.kind {
                        ParamKind::Float { min, max } => val.as_f64().and_then(|x| {
                            let c = x.clamp(*min, *max);
                            if c != x {
                                self.drop_(&pp, "param_clamped");
                            }
                            let c = if c == 0.0 { 0.0 } else { c };
                            serde_json::Number::from_f64(c).map(serde_json::Value::Number)
                        }),
                        ParamKind::Int { min, max } => val.as_i64().or_else(|| val.as_f64().map(|f| f.round() as i64)).map(|x| {
                            let c = x.clamp(*min, *max);
                            if c != x {
                                self.drop_(&pp, "param_clamped");
                            }
                            serde_json::Value::from(c)
                        }),
                        ParamKind::Bool => val.as_bool().map(serde_json::Value::Bool),
                        ParamKind::Choice { options: o, default } | ParamKind::Color { palette: o, default } => val.as_str().map(|s| {
                            if o.iter().any(|x| x == s) || s == default {
                                serde_json::Value::String(s.to_string())
                            } else {
                                self.drop_(&pp, "param_reset");
                                serde_json::Value::String(default.clone())
                            }
                        }),
                        ParamKind::Text | ParamKind::Long => val.as_str().is_some().then(|| serde_json::Value::String(self.string(val, &text_path, Class::Block, text_len))),
                    };
                    match kept {
                        Some(x) => {
                            params.insert(d.key.clone(), x);
                        }
                        None => self.drop_(&pp, "param_type"),
                    }
                }
            }
        }
        let bytes = serde_json::to_string(&params).map(|s| s.len()).unwrap_or(usize::MAX);
        if bytes > self.lim.figure_params_bytes {
            self.err_lim("too_long", &pp, self.lim.figure_params_bytes as u64);
            return None;
        }
        Some(FigureSpec { id, params })
    }

    fn chain_link(&mut self, v: &Node, path: &str) -> Option<ChainLink> {
        let m = self.object(v, path, &["author_name", "item_id", "qid", "version", "license", "modified"], &[])?;
        let author = Max::Chars(self.lim.author_chars);
        Some(ChainLink {
            author_name: self.req_string(m, "author_name", path, Class::Line, author),
            item_id: self.req_id(m, "item_id", path),
            qid: self.req_id(m, "qid", path),
            version: self.u32_(m, "version", path),
            license: self.license(m, "license", path),
            modified: match self.required(m, "modified", path) {
                Some(b) => self.boolean(b, &at(path, "modified")),
                None => false,
            },
        })
    }

    fn license(&mut self, m: &Obj, key: &str, path: &str) -> String {
        match self.required(m, key, path) {
            Some(Node::Str(s)) if LICENSES.contains(&&**s) => s.to_string(),
            Some(_) => {
                self.err("bad_license", &at(path, key));
                String::new()
            }
            None => String::new(),
        }
    }

    fn hash_field(&mut self, m: &Obj, key: &str, path: &str) -> String {
        match m.get(key) {
            None => String::new(),
            Some(Node::Str(s)) if is_hex64(s) => s.to_string(),
            Some(_) => {
                self.err("bad_hash", &at(path, key));
                String::new()
            }
        }
    }

    fn origin(&mut self, v: &Node, path: &str, question: bool) -> Option<Origin> {
        if v.is_null() {
            return None;
        }
        let keys = ["item_id", "qid", "publish_id", "version", "author_name", "license", "form", "served_sha256", "match_hash", "chain"];
        let m = self.object(v, path, &keys, &[])?;
        let qid = if question { self.req_id(m, "qid", path) } else { String::new() };
        let form = match self.required(m, "form", path) {
            Some(Node::Str(s)) if &**s == "full" || &**s == "stripped" => s.to_string(),
            Some(_) => {
                self.err("bad_form", &at(path, "form"));
                String::new()
            }
            None => String::new(),
        };
        let mut chain = Vec::new();
        if let Some(c) = m.get("chain") {
            let max = self.lim.chain_links;
            let cp = at(path, "chain");
            let arr = self.array(c, &cp, max);
            for (i, l) in arr.iter().enumerate() {
                if let Some(l) = self.chain_link(l, &idx(&cp, i)) {
                    chain.push(l);
                }
            }
        }
        let author = Max::Chars(self.lim.author_chars);
        Some(Origin {
            item_id: self.req_id(m, "item_id", path),
            qid,
            publish_id: self.req_id(m, "publish_id", path),
            version: self.u32_(m, "version", path),
            author_name: self.req_string(m, "author_name", path, Class::Line, author),
            license: self.license(m, "license", path),
            form,
            served_sha256: self.hash_field(m, "served_sha256", path),
            match_hash: self.hash_field(m, "match_hash", path),
            chain,
        })
    }

    fn published(&mut self, v: &Node, path: &str, question: bool) -> Option<Published> {
        if v.is_null() {
            return None;
        }
        let m = self.object(v, path, &["item_id", "qid", "publish_id", "version"], &[])?;
        Some(Published {
            item_id: self.req_id(m, "item_id", path),
            qid: if question { self.req_id(m, "qid", path) } else { String::new() },
            publish_id: self.req_id(m, "publish_id", path),
            version: self.u32_(m, "version", path),
        })
    }

    /// A keyless question carries no key: no `correct: true`, no Tf `true`, no accepted blanks,
    /// no samples, no figure, and no explanation but the sentinel.
    fn no_key_material(&mut self, kind: &Kind, explanation: Option<&str>, figure: bool, path: &str) {
        let k = at(path, "kind");
        let bad = |cx: &mut Self, p: String| cx.err("keyless_key_material", &p);
        match kind {
            Kind::Mc { answers } if answers.iter().any(|a| a.correct) => bad(self, format!("{k}.Mc.answers")),
            Kind::Tf { correct: true } => bad(self, format!("{k}.Tf.correct")),
            Kind::Blank { accept } if accept.iter().any(|a| !a.is_empty()) => bad(self, format!("{k}.Blank.accept")),
            Kind::Short { sample } if !sample.is_empty() => bad(self, format!("{k}.Short.sample")),
            Kind::Multipart { parts } => {
                for (i, p) in parts.iter().enumerate() {
                    let pk = format!("{k}.Multipart.parts[{i}].kind");
                    match &p.kind {
                        PartKind::Mc { answers } if answers.iter().any(|a| a.correct) => bad(self, format!("{pk}.Mc.answers")),
                        PartKind::Tf { correct: true } => bad(self, format!("{pk}.Tf.correct")),
                        PartKind::Short { sample } if !sample.is_empty() => bad(self, format!("{pk}.Short.sample")),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        if explanation.is_some_and(|e| e != KEYLESS_SENTINEL) {
            bad(self, at(path, "explanation"));
        }
        if figure {
            bad(self, at(path, "figure"));
        }
    }

    fn question(&mut self, v: &Node, path: &str) -> Option<Question> {
        let keys = ["id", "stem", "kind", "points", "difficulty", "topics", "image", "explanation", "figure", "created", "modified"];
        let m = self.object(v, path, &keys, &["keyless", "keep_order", "origin", "published"])?;
        let keyless = self.v2 && self.opt_bool(m, "keyless", path);
        // An old payload claiming to be keyless, or asking that its order be kept, cannot be
        // honoured (v1 has neither) and must not silently lose the request.
        for f in ["keyless", "keep_order"] {
            if !self.v2 && m.get(f).and_then(Node::as_bool) == Some(true) {
                self.err("v2_field_in_v1", &at(path, f));
            }
        }
        let id = self.req_id(m, "id", path);
        let stem = self.req_visible(m, "stem", path, Max::Bytes(self.lim.stem_bytes));
        let kind = match self.required(m, "kind", path) {
            Some(k) => self.kind(k, &at(path, "kind"), keyless, &stem),
            None => Kind::Short { sample: String::new() },
        };
        let points = match m.get("points") {
            None | Some(Node::Null) => None,
            Some(p) => match p.as_f64() {
                Some(x) if (0.0..=self.lim.max_points).contains(&x) => Some(if x == 0.0 { 0.0 } else { x }),
                Some(_) => {
                    self.err_lim("out_of_range", &at(path, "points"), self.lim.max_points as u64);
                    None
                }
                None => {
                    self.err("expected_number", &at(path, "points"));
                    None
                }
            },
        };
        let difficulty = match m.get("difficulty") {
            None | Some(Node::Null) => None,
            Some(Node::Str(s)) if &**s == "Low" => Some(Difficulty::Low),
            Some(Node::Str(s)) if &**s == "Medium" => Some(Difficulty::Medium),
            Some(Node::Str(s)) if &**s == "High" => Some(Difficulty::High),
            Some(_) => {
                self.err("unknown_kind", &at(path, "difficulty"));
                None
            }
        };
        let mut topics = Vec::new();
        if let Some(t) = m.get("topics") {
            let tp = at(path, "topics");
            let (max, len) = (self.lim.topics, Max::Chars(self.lim.tag_chars));
            // Trimmed (Line), empties dropped, duplicates dropped; the count is checked first on
            // what was sent, so a flood of empties is still refused.
            let arr = self.array(t, &tp, max);
            for (i, s) in arr.iter().enumerate() {
                let c = self.string(s, &idx(&tp, i), Class::Line, len);
                if !c.is_empty() && !topics.contains(&c) {
                    topics.push(c);
                }
            }
        }
        let image = m.get("image").and_then(|i| self.image(i, &at(path, "image")));
        let explanation = match m.get("explanation") {
            None | Some(Node::Null) => None,
            Some(e) => Some(self.string(e, &at(path, "explanation"), Class::Block, Max::Bytes(self.lim.explanation_bytes))),
        };
        let figure = m.get("figure").and_then(|f| self.figure(f, &at(path, "figure")));
        let created = self.i64_(m, "created", path);
        let modified = self.i64_(m, "modified", path);
        let (origin, published) = if self.v2 {
            (
                m.get("origin").and_then(|o| self.origin(o, &at(path, "origin"), true)),
                m.get("published").and_then(|p| self.published(p, &at(path, "published"), true)),
            )
        } else {
            (None, None)
        };
        if keyless {
            self.no_key_material(&kind, explanation.as_deref(), figure.is_some(), path);
        }
        let keep_order = self.v2 && self.opt_bool(m, "keep_order", path);
        Some(Question { id, stem, kind, points, difficulty, topics, image, explanation, figure, created, modified, keyless, keep_order, origin, published })
    }

    /// Takes the bank by value: each question's subtree is freed as soon as it has been read, so
    /// the parsed tree and the typed bank are never both whole in memory.
    fn bank(&mut self, mut v: Node, path: &str) -> Option<QuestionBank> {
        self.object(&v, path, &["id", "name", "questions", "created", "modified", "details"], &["origin", "published"])?;
        let max = self.lim.questions_per_bank;
        let qp = at(path, "questions");
        let mut questions = Vec::new();
        match v.take("questions") {
            None => self.err("missing", &qp),
            Some(Node::Arr(arr)) if arr.len() > max => self.err_lim("too_many", &qp, max as u64),
            Some(Node::Arr(arr)) => {
                questions.reserve(arr.len());
                for (i, q) in arr.into_iter().enumerate() {
                    if let Some(q) = self.question(&q, &idx(&qp, i)) {
                        questions.push(q);
                    }
                }
            }
            Some(_) => self.err("expected_array", &qp),
        }
        let m = v.as_object()?;
        let id = self.req_id(m, "id", path);
        let name = self.req_string(m, "name", path, Class::Line, Max::Chars(self.lim.title_chars));
        let created = self.i64_(m, "created", path);
        let modified = self.i64_(m, "modified", path);
        let details = match m.get("details") {
            None => BankDetails::default(),
            Some(d) => {
                let dp = at(path, "details");
                match self.object(d, &dp, &["description", "author", "license_name", "license_url"], &[]) {
                    None => BankDetails::default(),
                    Some(dm) => {
                        let desc = Max::Bytes(self.lim.description_bytes);
                        let author = Max::Chars(self.lim.author_chars);
                        let url = Max::Chars(self.lim.license_url_chars);
                        let description = self.opt_string(dm, "description", &dp, Class::Block, desc);
                        let author_ = self.opt_string(dm, "author", &dp, Class::Line, author);
                        let license_name = self.opt_string(dm, "license_name", &dp, Class::Line, author);
                        let license_url = self.opt_string(dm, "license_url", &dp, Class::Line, url);
                        if !license_url.is_empty() && !is_web_url(&license_url) {
                            self.err("bad_url", &at(&dp, "license_url"));
                        }
                        BankDetails { description, author: author_, license_name, license_url }
                    }
                }
            }
        };
        let (origin, published) = if self.v2 {
            (
                m.get("origin").and_then(|o| self.origin(o, &at(path, "origin"), false)),
                m.get("published").and_then(|p| self.published(p, &at(path, "published"), false)),
            )
        } else {
            (None, None)
        };
        Some(QuestionBank { id, name, questions, created, modified, details, origin, published })
    }
}

/// `^[A-Za-z0-9_-]{1,max}$`: local ids (`model::new_id`), server ids, qids and publish ids.
pub fn is_id(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// `^media/[0-9a-f]{64}\.(png|jpg|gif|webp)$` (the local content-addressed store's names).
pub fn is_media_file(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("media/") else { return false };
    let Some((hash, ext)) = rest.split_once('.') else { return false };
    is_hex64(hash) && matches!(ext, "png" | "jpg" | "gif" | "webp")
}

/// `#rrggbb`.
pub fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s.bytes().skip(1).all(|b| b.is_ascii_hexdigit())
}

/// 64 lowercase hex digits.
pub fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `https://` only (the spec names no other scheme), a host of letters, digits, dots and hyphens
/// with an optional port, no userinfo (`user@`), then printable ASCII without quotes, angle
/// brackets, backslashes or backticks.
pub fn is_web_url(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("https://") else { return false };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    // Every dot-separated label 1-63 letters, digits or hyphens, not starting or ending with a
    // hyphen; at least two labels; no empty label ("a..b", ".a", "a.").
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && host.split('.').count() >= 2
        && host.split('.').all(|l| {
            !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });
    let port_ok = port.is_none_or(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u32>().is_ok_and(|n| (1..=65535).contains(&n)));
    let tail_ok = tail.bytes().all(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b'\'' | b'<' | b'>' | b'\\' | b'`'));
    host_ok && port_ok && tail_ok
}

fn parse_err(e: ParseError, limits: &Limits) -> Vec<ValidationError> {
    let limit = match e {
        ParseError::TooLarge => Some(limits.body_bytes as u64),
        ParseError::Depth => Some(limits.max_depth as u64),
        ParseError::TooManyValues => Some(limits.max_values as u64),
        ParseError::TooManyObjects => Some(limits.max_objects as u64),
        ParseError::OverBudget => Some(limits.parse_budget_bytes as u64),
        _ => None,
    };
    vec![ValidationError { limit, ..ValidationError::new(e.code(), "") }]
}

/// Validates and canonicalises a payload envelope (see the module docs). For received payloads.
pub fn validate(json: &str, limits: &Limits, figures: &dyn FigureRegistry) -> Result<Validated, Vec<ValidationError>> {
    let limits = limits.sane();
    let root = json::parse(json, &json::ENVELOPE, limits.budget(), &limits).map_err(|e| parse_err(e, &limits))?;
    validate_tree(root, &limits, figures)
}

/// `validate` for UTF-8 bytes (what the wasm entry points take): invalid UTF-8 is refused.
pub fn validate_bytes(json: &[u8], limits: &Limits, figures: &dyn FigureRegistry) -> Result<Validated, Vec<ValidationError>> {
    let limits = limits.sane();
    let root = json::parse_bytes(json, &json::ENVELOPE, limits.budget(), &limits).map_err(|e| parse_err(e, &limits))?;
    validate_tree(root, &limits, figures)
}

/// `validate`, then key completeness and the keyless rule (see the module docs). For uploads.
pub fn validate_for_publish(json: &[u8], limits: &Limits, figures: &dyn FigureRegistry, opts: &PublishOptions) -> Result<Validated, Vec<ValidationError>> {
    let v = validate_bytes(json, limits, figures)?;
    let errors = publish_errors(&v.payload, opts);
    if errors.is_empty() { Ok(v) } else { Err(errors) }
}

/// The publish-only checks over an already validated payload.
pub fn publish_errors(p: &Payload, opts: &PublishOptions) -> Vec<ValidationError> {
    let base = match &p.body {
        Body::Bank(_) => "bank.questions",
        Body::Question(_) => "question",
    };
    let mut errors = Vec::new();
    let mut err = |code, path: String| {
        if errors.len() < MAX_ERRORS {
            errors.push(ValidationError::new(code, &path));
        }
    };
    for (i, q) in p.questions().iter().enumerate() {
        let qp = if matches!(p.body, Body::Bank(_)) { format!("{base}[{i}]") } else { base.to_string() };
        let sentinel = q.explanation.as_deref() == Some(KEYLESS_SENTINEL);
        if q.keyless || sentinel {
            if !opts.hide_key {
                err("keyless_needs_hide_key", qp);
            }
            continue;
        }
        let k = format!("{qp}.kind");
        let dup = |answers: &[Answer]| {
            let mut t: Vec<String> = answers.iter().map(|a| text::hash_form(&a.text)).collect();
            t.sort();
            t.windows(2).any(|w| w[0] == w[1])
        };
        match &q.kind {
            Kind::Mc { answers } if dup(answers) => err("duplicate_choice", format!("{k}.Mc.answers")),
            Kind::Multipart { parts } if parts.iter().any(|p| matches!(&p.kind, PartKind::Mc { answers } if dup(answers))) => {
                err("duplicate_choice", format!("{k}.Multipart.parts"))
            }
            _ => {}
        }
        match &q.kind {
            Kind::Mc { answers } if !answers.iter().any(|a| a.correct) => err("no_correct", format!("{k}.Mc.answers")),
            Kind::Blank { accept } => {
                for (b, a) in accept.iter().enumerate() {
                    if a.is_empty() {
                        err("no_correct", format!("{k}.Blank.accept[{b}]"));
                    }
                }
            }
            Kind::Multipart { parts } => {
                for (pi, part) in parts.iter().enumerate() {
                    if let PartKind::Mc { answers } = &part.kind
                        && !answers.iter().any(|a| a.correct)
                    {
                        err("no_correct", format!("{k}.Multipart.parts[{pi}].kind.Mc.answers"));
                    }
                }
            }
            _ => {}
        }
    }
    // A hide_key publish needs exactly one decision for every label finding (and none for
    // anything else), Strip only where allowed, and no key carried by formatting.
    if opts.hide_key {
        let mut push = |e: ValidationError| {
            if errors.len() < MAX_ERRORS {
                errors.push(e);
            }
        };
        for e in crate::strip::decision_errors(p, &opts.labels) {
            push(e);
        }
        for (i, q) in p.questions().iter().enumerate() {
            let qp = if matches!(p.body, Body::Bank(_)) { format!("{base}[{i}]") } else { base.to_string() };
            let mut check = |answers: &[Answer], at: String| {
                for k in crate::labels::key_in_formatting(&answers.iter().map(|a| (a.text.as_str(), a.correct)).collect::<Vec<_>>()) {
                    push(ValidationError::new("key_in_formatting", &format!("{at}[{k}]")));
                }
            };
            match &q.kind {
                Kind::Mc { answers } => check(answers, format!("{qp}.kind.Mc.answers")),
                Kind::Multipart { parts } => {
                    for (pi, part) in parts.iter().enumerate() {
                        if let PartKind::Mc { answers } = &part.kind {
                            check(answers, format!("{qp}.kind.Multipart.parts[{pi}].kind.Mc.answers"));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    errors
}

fn validate_tree(mut root: Node, limits: &Limits, figures: &dyn FigureRegistry) -> Result<Validated, Vec<ValidationError>> {
    let mut cx = Cx { lim: limits, reg: figures, v2: false, errors: Vec::new(), dropped: Vec::new(), text_bytes: 0, text_over: false, def_index: Default::default() };
    let Some(m) = cx.object(&root, "", &["schema_version", "question", "bank"], &[]) else { return Err(cx.errors) };
    let schema_version = match m.get("schema_version").and_then(Node::as_u64) {
        Some(n) if (SCHEMA_MIN as u64..=SCHEMA_MAX as u64).contains(&n) => n as u32,
        Some(_) => {
            cx.err_lim("schema_version_unsupported", "schema_version", SCHEMA_MAX as u64);
            return Err(cx.errors);
        }
        None => {
            cx.err(if m.contains_key("schema_version") { "expected_integer" } else { "missing" }, "schema_version");
            return Err(cx.errors);
        }
    };
    cx.v2 = schema_version >= 2;
    let (has_q, has_b) = (m.contains_key("question"), m.contains_key("bank"));
    let body = match (has_q, has_b) {
        (true, false) => root.take("question").and_then(|q| cx.question(&q, "question")).map(Body::Question),
        (false, true) => root.take("bank").and_then(|b| cx.bank(b, "bank")).map(Body::Bank),
        (true, true) => {
            cx.err("ambiguous_kind", "");
            None
        }
        (false, false) => {
            cx.err("missing", "question");
            None
        }
    };
    match body {
        Some(body) if cx.errors.is_empty() => {
            let payload = Payload { schema_version, body };
            // What is stored and served must itself fit the body limit (defaults are written out
            // and NFC can expand text), or every client would refuse it.
            let n = payload.json_len();
            if n > limits.body_bytes {
                return Err(vec![ValidationError { limit: Some(limits.body_bytes as u64), ..ValidationError::new("output_too_large", "") }]);
            }
            Ok(Validated { payload, dropped: cx.dropped })
        }
        _ => Err(cx.errors),
    }
}

/// Publish metadata (docs/shared-pool.md, "Sharing model": discovery metadata).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Meta {
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
    /// Lower-cased, de-duplicated, sorted.
    pub tags: Vec<String>,
    /// Upper-cased, whitespace collapsed, de-duplicated, sorted.
    pub standards: Vec<String>,
}

/// Checks the publish metadata object `{title, subject?, course?, grade?, tags?, standards?}` with
/// the Line rule and NFKC (`text::clean_meta`). Unknown keys are dropped.
pub fn validate_meta(json: &str, limits: &Limits) -> Result<Meta, Vec<ValidationError>> {
    let limits = limits.sane();
    let budget = json::Budget { max_bytes: 64 * 1024, max_depth: 3, max_values: 1000, max_objects: 1, max_alloc: 1 << 20 };
    let root = json::parse(json, &json::META, budget, &limits).map_err(|e| parse_err(e, &limits))?;
    let mut errors: Vec<ValidationError> = Vec::new();
    let Some(m) = root.as_object() else {
        return Err(vec![ValidationError::new("expected_object", "")]);
    };
    let mut push = |e: ValidationError| {
        if errors.len() < MAX_ERRORS {
            errors.push(e)
        }
    };
    let text_err = |e: TextError, path: String, max: usize| match e {
        TextError::Forbidden { offset, offset16, cp } => {
            ValidationError { offset: Some(offset as u64), offset16: Some(offset16 as u64), cp: Some(format!("U+{cp:04X}")), ..ValidationError::new("forbidden_char", &path) }
        }
        TextError::MarkRun { offset, offset16 } => ValidationError { offset: Some(offset as u64), offset16: Some(offset16 as u64), ..ValidationError::new("mark_run", &path) },
        TextError::TooLong => ValidationError { limit: Some(max as u64), ..ValidationError::new("too_long", &path) },
    };
    let line = |key: &str, max: usize, required: bool, push: &mut dyn FnMut(ValidationError)| -> String {
        match m.get(key) {
            None | Some(Node::Null) if !required => String::new(),
            None | Some(Node::Null) => {
                push(ValidationError::new("missing", key));
                String::new()
            }
            Some(Node::Str(s)) => match text::clean_meta(s, max) {
                Err(e) => {
                    push(text_err(e, key.to_string(), max));
                    String::new()
                }
                Ok(c) if required && !text::has_visible(&c) => {
                    push(ValidationError::new("blank_text", key));
                    String::new()
                }
                Ok(c) => c,
            },
            Some(_) => {
                push(ValidationError::new("expected_string", key));
                String::new()
            }
        }
    };
    let title = line("title", limits.title_chars, true, &mut push);
    let subject = line("subject", limits.meta_chars, false, &mut push);
    let course = line("course", limits.meta_chars, false, &mut push);
    let grade = line("grade", limits.meta_chars, false, &mut push);
    let list = |key: &str, max: usize, upper: bool, push: &mut dyn FnMut(ValidationError)| -> Vec<String> {
        let mut out = BTreeSet::new();
        match m.get(key) {
            None | Some(Node::Null) => {}
            Some(Node::Arr(a)) if a.len() > max => push(ValidationError { limit: Some(max as u64), ..ValidationError::new("too_many", key) }),
            Some(Node::Arr(a)) => {
                for (i, v) in a.iter().enumerate() {
                    let p = format!("{key}[{i}]");
                    match v.as_str().map(|s| text::clean_meta(s, limits.tag_chars)) {
                        None => push(ValidationError::new("expected_string", &p)),
                        Some(Err(e)) => push(text_err(e, p, limits.tag_chars)),
                        Some(Ok(c)) if !text::has_visible(&c) => push(ValidationError::new("blank_text", &p)),
                        Some(Ok(c)) => {
                            out.insert(if upper { c.to_uppercase() } else { c.to_lowercase() });
                        }
                    }
                }
            }
            Some(_) => push(ValidationError::new("expected_array", key)),
        }
        out.into_iter().collect()
    };
    let tags = list("tags", limits.tags, false, &mut push);
    let standards = list("standards", limits.standards, true, &mut push);
    if errors.is_empty() { Ok(Meta { title, subject, course, grade, tags, standards }) } else { Err(errors) }
}
