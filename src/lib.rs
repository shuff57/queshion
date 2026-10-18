//! verSHion as a library: the whole app UI, host-agnostic.
//!
//! The desktop binary wraps [`App`] in an `eframe::App`; officecraft embeds it as a tab; the
//! web entry drives the same `new_from`/`logic`/`ui` methods. Native-only work (dialogs,
//! filesystem, HTTP, headless hooks) lives behind [`platform`] and [`store`] backends.

/// Reads `VERSHION_*`, falling back to the pre-rename `QUESHION_*` for one release so
/// existing QA scripts keep working.
pub fn env_var(name: &str) -> Result<String, std::env::VarError> {
    std::env::var(name)
        .or_else(|first| std::env::var(name.replacen("VERSHION_", "QUESHION_", 1)).map_err(|_| first))
}

pub fn env_var_os(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name)
        .or_else(|| std::env::var_os(name.replacen("VERSHION_", "QUESHION_", 1)))
}

pub mod ai;
pub mod app;
pub mod bankexport;
pub mod bankops;
pub mod bankui;
pub mod ditto;
pub mod examui;
pub mod equation;
pub mod equation_text;
pub mod export;
pub mod figfit;
pub mod figures;
pub mod figuresui;
pub mod fonts;
pub mod formats;
pub mod importui;
pub mod kit;
pub mod layout;
pub mod model;
pub mod platform;
pub mod qedit;
pub mod richtext;
pub mod screens;
pub mod store;
pub mod theme;
pub mod time;
pub mod tidy;
pub mod wysiwyg;

pub use app::{App, Screen, Settings};
