//! share-core: the one implementation of the shared pool's payload rules, linked natively by the
//! app and compiled to wasm for the Worker (docs/shared-pool.md, "One share-core crate").
//!
//! * [`validate`]: strict parsing, the schema, the Limits table, character rules, figure params.
//! * [`strip`]: the stripped (keyless) form, shuffled once with a seed drawn inside the crate;
//!   [`labels`]: what the author must decide about choice labels first.
//! * [`import`]: the importer's conversion of a stripped question, and re-keying.
//! * [`match_hash`], [`full_hash`] / [`served_hash`], [`canonical_json`]: see [`hash`] for exactly
//!   what each hash covers.
//! * [`wasm`]: the same, JSON in and JSON out, for the Worker.
//! * [`argon2id`]: password hashing (phase 1).
pub mod figures;
pub mod hash;
pub mod import;
pub mod json;
pub mod labels;
pub mod limits;
pub mod strip;
pub mod text;
pub mod validate;
pub mod wasm;
pub mod wire;

pub use figures::{Declared, FigureRegistry, ParamKind, ParamSpec, Structural};
pub use hash::{canonical_json, full_hash, match_hash, match_hash_with, match_skeleton, served_hash, write_payload, write_question};
pub use limits::Limits;
pub use labels::LabelChoice;
pub use strip::{LabelDecision, LabelFinding, OrderNote, OrderReason, Pos, QuestionLabels, ShuffleRng, StripOptions, Stripped, label_findings, question_labels, strip, strip_question, strip_with_seed, strip_without_shuffle};
pub use validate::{Dropped, Meta, PublishOptions, Validated, ValidationError, publish_errors, validate, validate_bytes, validate_for_publish, validate_meta};
pub use wire::*;

use argon2::{Algorithm, Argon2, Params, Version};
use wasm_bindgen::prelude::*;

/// Hash `password` with Argon2id; returns raw 32-byte tag. Salt is supplied by the caller.
#[wasm_bindgen]
pub fn argon2id(password: &[u8], salt: &[u8], m_kib: u32, t: u32, p: u32) -> Result<Vec<u8>, JsError> {
    let params = Params::new(m_kib, t, p, Some(32)).map_err(|e| JsError::new(&e.to_string()))?;
    let a = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = vec![0u8; 32];
    a.hash_password_into(password, salt, &mut out).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(out)
}

/// Current wasm linear-memory size in bytes (peak is what matters; wasm memory never shrinks).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn memory_bytes() -> usize {
    core::arch::wasm32::memory_size(0) * 65536
}
