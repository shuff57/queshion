//! Host edge: file pickers, saving, opening paths, and PDF page rasterization.
//!
//! Screens call these instead of `rfd`, `std::fs` and `std::process` directly, so one set of
//! screens compiles for the desktop and the browser. Native keeps synchronous dialogs; the
//! wasm build installs browser implementations here (async pickers + downloads) when the web
//! entry is built.
use std::path::{Path, PathBuf};

/// A file the browser picked: bytes and the name the user's machine had for it.
#[derive(Clone, Debug)]
pub struct PickedFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Files chosen through the Import pickers, drained by the Import screen each frame.
    static PICKED: std::cell::RefCell<Vec<PickedFile>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Single-file picks (attach a picture); a separate queue so they never become imports.
    static PICKED_SINGLE: std::cell::RefCell<Vec<PickedFile>> = const { std::cell::RefCell::new(Vec::new()) };
    static REPAINT: std::cell::RefCell<Option<egui::Context>> = const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
pub fn set_repaint(ctx: egui::Context) {
    REPAINT.with(|r| *r.borrow_mut() = Some(ctx));
}

#[cfg(target_arch = "wasm32")]
fn wake() {
    REPAINT.with(|r| {
        if let Some(ctx) = &*r.borrow() {
            ctx.request_repaint();
        }
    });
}

/// Files the Import pickers finished reading. Empty on native.
pub fn take_picked() -> Vec<PickedFile> {
    #[cfg(target_arch = "wasm32")]
    {
        PICKED.with(|q| std::mem::take(&mut *q.borrow_mut()))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Vec::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_files(filter: &str, exts: &[&str]) -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new().add_filter(filter, exts).pick_files()
}

/// No synchronous picker in a browser: the input opens and results arrive through [`take_picked`].
#[cfg(target_arch = "wasm32")]
pub fn pick_files(_filter: &str, exts: &[&str]) -> Option<Vec<PathBuf>> {
    open_input(true, exts, false);
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_file(filter: &str, exts: &[&str]) -> Option<PathBuf> {
    rfd::FileDialog::new().add_filter(filter, exts).pick_file()
}

#[cfg(target_arch = "wasm32")]
pub fn pick_file(_filter: &str, exts: &[&str]) -> Option<PathBuf> {
    open_input(false, exts, true);
    None
}

#[cfg(target_arch = "wasm32")]
fn open_input(multiple: bool, exts: &[&str], single: bool) {
    use wasm_bindgen::JsCast as _;
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let Ok(el) = document.create_element("input") else { return };
    let Ok(input) = el.dyn_into::<web_sys::HtmlInputElement>() else { return };
    input.set_type("file");
    input.set_multiple(multiple);
    if !exts.is_empty() {
        let accept = exts.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(",");
        input.set_accept(&accept);
    }
    // Keep the input in the document (hidden): some browsers ignore clicks on detached inputs,
    // and a visible-in-DOM input is also scriptable for tests.
    let _ = input.style().set_property("display", "none");
    if let Some(body) = document.body() {
        let _ = body.append_child(&input);
    }
    let input2 = input.clone();
    let handler = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
        let Some(files) = input2.files() else { return };
        for i in 0..files.length() {
            let Some(file) = files.get(i) else { continue };
            wasm_bindgen_futures::spawn_local(async move {
                let promise = file.array_buffer();
                if let Ok(value) = wasm_bindgen_futures::JsFuture::from(promise).await {
                    let bytes = js_sys::Uint8Array::new(&value).to_vec();
                    let picked = PickedFile { name: file.name(), bytes };
                    if single {
                        PICKED_SINGLE.with(|q| q.borrow_mut().push(picked));
                    } else {
                        PICKED.with(|q| q.borrow_mut().push(picked));
                    }
                    wake();
                }
            });
        }
    });
    let _ = input.add_event_listener_with_callback("change", handler.as_ref().unchecked_ref());
    handler.forget();
    input.click();
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

#[cfg(not(target_arch = "wasm32"))]
pub fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

#[cfg(target_arch = "wasm32")]
pub fn read(_path: &Path) -> std::io::Result<Vec<u8>> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "a page has no file paths"))
}

pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, bytes)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("download.bin");
        download(name, bytes);
        Ok(())
    }
}

pub fn create_dir_all(path: &Path) -> std::io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::create_dir_all(path)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Ok(())
    }
}

/// Hands bytes to the browser's download UI.
#[cfg(target_arch = "wasm32")]
fn download(name: &str, bytes: &[u8]) {
    use wasm_bindgen::JsCast as _;
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array);
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type("application/octet-stream");
    let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &bag) else { return };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
    if let Ok(el) = document.create_element("a")
        && let Ok(a) = el.dyn_into::<web_sys::HtmlAnchorElement>()
    {
        a.set_href(&url);
        a.set_download(name);
        a.click();
    }
    let _ = web_sys::Url::revoke_object_url(&url);
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

/// The clip that shows how to obtain a key for `provider`, if we have one.
pub fn help_clip_name(provider: &str) -> Option<&'static str> {
    match provider {
        "cloud" => Some("ollama-cloud-api-key"),
        "openrouter" => Some("openrouter-free-model-router"),
        _ => None,
    }
}

/// Shows the how-to clip for `provider`: an in-page overlay on the web, the system video player
/// on the desktop (the clips are compiled into the desktop binary; the web build copies them to
/// `help/` with Trunk).
pub fn open_help_video(provider: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        crate::helpweb::open(provider);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(name) = help_clip_name(provider) else { return };
        let bytes: &'static [u8] = match name {
            "openrouter-free-model-router" => include_bytes!("../assets/help/openrouter-free-model-router.mp4"),
            _ => include_bytes!("../assets/help/ollama-cloud-api-key.mp4"),
        };
        let path = std::env::temp_dir().join(format!("vershion-help-{name}.mp4"));
        if std::fs::write(&path, bytes).is_ok() {
            open_path(&path);
        }
    }
}

/// Rasterizes every page of a PDF to PNG with the pure-Rust hayro renderer. Native falls back to
/// `pdftoppm` for files hayro cannot render.
pub fn render_pdf_pages(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    match render_with_hayro(data) {
        Ok(pages) if !pages.is_empty() => Ok(pages),
        hayro_result => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                match crate::screens::pdf_pages_png(data) {
                    Ok(pages) => Ok(pages),
                    Err(pdftoppm) => hayro_result.map_err(|e| format!("{e} (pdftoppm: {pdftoppm})")),
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                hayro_result
            }
        }
    }
}

fn render_with_hayro(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::Pdf;
    use hayro::vello_cpu::color::palette::css::WHITE;
    use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
    const MAX_PDF_PAGES: usize = 30;
    const DPI: f32 = 150.0;

    let pdf = Pdf::new(data.to_vec()).map_err(|e| format!("{e:?}"))?;
    let interpreter = InterpreterSettings::default();
    let cache = RenderCache::new();
    let settings = RenderSettings::default();
    let pixmap = PixmapSettings { x_scale: DPI / 72.0, y_scale: DPI / 72.0, bg_color: WHITE };
    let mut pages = Vec::new();
    for page in pdf.pages().iter().take(MAX_PDF_PAGES) {
        let rendered = render(page, &cache, &interpreter, &settings, &pixmap);
        pages.push(rendered.into_png().map_err(|e| format!("{e:?}"))?);
    }
    if pages.is_empty() {
        Err("the PDF has no pages".to_string())
    } else {
        Ok(pages)
    }
}
