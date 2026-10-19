//! Spike: Argon2id inside a Worker. Real API lands once parameters are chosen.
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
#[wasm_bindgen]
pub fn memory_bytes() -> usize {
    core::arch::wasm32::memory_size(0) * 65536
}
