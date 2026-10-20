//! Figure parameters on untrusted payloads (docs/shared-pool.md, "Figures on received payloads").
//!
//! The figure registry lives in the app (`src/figures`), not here, so the caller supplies it through
//! [`FigureRegistry`]. Two implementations ship with the crate:
//!
//! * [`Structural`] (the default, used by the Worker): it knows no figure, so it cannot clamp. It
//!   keeps a figure whose id looks like a registry slug, keeps only scalar params (finite numbers,
//!   booleans, strings) under well-formed keys, refuses a text param over the text limit, and drops
//!   everything else. The figure never reaches a non-author anyway: `strip` always removes it.
//! * [`Declared`]: a registry given as data (`{"<figure id>": [ParamSpec, ...]}`), which the app
//!   builds from `figures::registry()` and which also travels over the wasm boundary as JSON. With
//!   it, an unknown figure id is dropped, unknown keys are dropped, a value of the wrong type is
//!   dropped (the app's `params_from_spec` would fall back to the default), numbers are clamped into
//!   range, an unknown choice or colour becomes the default, and a text param over 4 KB is refused,
//!   which is `Params::normalize` plus the length cap it lacks.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One declared parameter, mirroring `figures::ParamDef` / `figures::Kind`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamSpec {
    pub key: String,
    #[serde(flatten)]
    pub kind: ParamKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ParamKind {
    Float { min: f64, max: f64 },
    Int { min: i64, max: i64 },
    Bool,
    Choice { options: Vec<String>, default: String },
    /// `Text`: one line.
    Text,
    /// `Long` and `List`: multi-line text.
    Long,
    Color { palette: Vec<String>, default: String },
}

/// What the validator needs to know about figures.
pub trait FigureRegistry {
    /// The declared params of figure `id`, or `None` if this registry does not know it.
    fn params(&self, id: &str) -> Option<&[ParamSpec]>;
    /// Whether a figure this registry does not know is dropped (`true`) or kept with only
    /// structural checks (`false`).
    fn drop_unknown(&self) -> bool;
}

/// The conservative default: no figure is known, figures are kept with structural checks only.
#[derive(Clone, Copy, Debug, Default)]
pub struct Structural;

impl FigureRegistry for Structural {
    fn params(&self, _id: &str) -> Option<&[ParamSpec]> { None }
    fn drop_unknown(&self) -> bool { false }
}

/// A registry given as data. Unknown figure ids are dropped.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Declared(pub BTreeMap<String, Vec<ParamSpec>>);

impl FigureRegistry for Declared {
    fn params(&self, id: &str) -> Option<&[ParamSpec]> { self.0.get(id).map(|v| v.as_slice()) }
    fn drop_unknown(&self) -> bool { true }
}

/// `^[a-z0-9][a-z0-9-]{0,63}$`: the shape of every registry slug ("free-body-diagram").
pub fn is_figure_id(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// `^[a-z][a-z0-9_]{0,63}$`: the shape of every `ParamDef.key` ("answer_text", "xaxis_lo").
pub fn is_param_key(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b[0].is_ascii_lowercase()
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
}
