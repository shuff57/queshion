//! Host edge: file pickers, saving, opening paths, and PDF page rasterization.
//!
//! Screens call these instead of `rfd`, `std::fs` and `std::process` directly, so one set of
//! screens compiles for the desktop and the browser. Native keeps synchronous dialogs; the
//! wasm build installs browser implementations here (async pickers + downloads) when the web
//! entry is built.
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_files(filter: &str, exts: &[&str]) -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new().add_filter(filter, exts).pick_files()
}

#[cfg(target_arch = "wasm32")]
pub fn pick_files(_filter: &str, _exts: &[&str]) -> Option<Vec<PathBuf>> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_file(filter: &str, exts: &[&str]) -> Option<PathBuf> {
    rfd::FileDialog::new().add_filter(filter, exts).pick_file()
}

#[cfg(target_arch = "wasm32")]
pub fn pick_file(_filter: &str, _exts: &[&str]) -> Option<PathBuf> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_folder() -> Option<PathBuf> {
    rfd::FileDialog::new().pick_folder()
}

#[cfg(target_arch = "wasm32")]
pub fn pick_folder() -> Option<PathBuf> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_path(default_name: &str, filter: &str, exts: &[&str]) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_file_name(default_name)
        .add_filter(filter, exts)
        .save_file()
}

#[cfg(target_arch = "wasm32")]
pub fn save_path(default_name: &str, _filter: &str, _exts: &[&str]) -> Option<PathBuf> {
    // No filesystem path in the browser; the web layer turns the suggested name into a download.
    Some(PathBuf::from(default_name))
}

pub fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, bytes)
}

pub fn create_dir_all(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

/// Opens a folder/file in the system file manager. No-op in the browser.
pub fn open_path(path: &Path) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
    }
}

/// Rasterizes every page of a PDF to PNG. Native uses `pdftoppm`; the web build swaps in the
/// pure-Rust hayro renderer.
pub fn render_pdf_pages(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::screens::pdf_pages_png(data)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = data;
        Err("PDF page rendering is not available in this build yet".to_string())
    }
}
