//! Standalone browser shell: IndexedDB store, eframe WebRunner, and the loading splash.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::egui;
use vershion::fonts::Face;
use wasm_bindgen::JsCast as _;

const CANVAS_ID: &str = "vershion_canvas";
const LOADING_ID: &str = "vershion_loading";
/// One pass of the loading animation (the SVG's duration) and the fade that follows it.
const LOADER_CYCLE_MS: f64 = 3500.0;
const LOADER_FADE_MS: i32 = 350;

thread_local! {
    static CTX: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
}

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("verSHion: no document");
            return;
        };
        let Some(canvas) = document
            .get_element_by_id(CANVAS_ID)
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        else {
            log::error!("verSHion: missing <canvas id=\"{CANVAS_ID}\">");
            return;
        };

        // The faces the wasm does not compile in, requested now so they download while storage
        // opens (index.html preloads them alongside the wasm).
        let fonts = fetch_fonts();

        let repaint: Rc<dyn Fn()> = Rc::new(|| {
            CTX.with(|c| {
                if let Some(ctx) = &*c.borrow() {
                    ctx.request_repaint();
                }
            });
        });

        // The store must be ready before App::new_from reads settings and documents.
        let backend = match vershion::store::web::IdbBackend::open("vershion-standalone", repaint).await {
            Ok(b) => b,
            Err(e) => {
                show_error(&document, &format!("Browser storage could not open: {e}"));
                return;
            }
        };
        vershion::store::init(Box::new(backend));

        // Every font consumer (egui, PDF metrics, resvg, the PDF writer) reads its faces from
        // here on, so they are installed before the app exists; any that did not arrive are drawn
        // with compiled-in stand-ins for the whole session.
        install_fonts(fonts.await);

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    CTX.with(|c| *c.borrow_mut() = Some(cc.egui_ctx.clone()));
                    let mut app = vershion::App::new_from(&cc.egui_ctx);
                    // The page owns the window; the bank pop-over stays in the same viewport.
                    app.allow_viewports = false;
                    Ok(Box::new(WebApp { app }))
                }),
            )
            .await;

        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => {
                    finish_loader_cycle().await;
                    el.remove();
                }
                Err(e) => show_error(&document, &format!("verSHion failed to start: {e:?}")),
            }
        }
    });
}

/// Longest the start waits for the fetched fonts; past it the app starts with stand-ins.
const FONT_WAIT_MS: i32 = 15_000;

/// A font file the wasm does not compile in.
#[derive(Clone, Copy)]
enum Wanted {
    /// One of the app's faces (`fonts/<file>`).
    Face(Face),
    /// One of hayro's standard PDF fonts (`pdf-fonts/<file>`).
    Pdf(&'static str),
}

impl Wanted {
    fn url(self) -> String {
        match self {
            Wanted::Face(f) => format!("fonts/{}", f.file()),
            Wanted::Pdf(file) => format!("pdf-fonts/{file}"),
        }
    }
}

/// Starts fetching every font file the wasm does not compile in (copied next to index.html by
/// Trunk) and returns a future of those that arrived within [`FONT_WAIT_MS`].
fn fetch_fonts() -> impl std::future::Future<Output = Vec<(Wanted, Vec<u8>)>> {
    let got: Rc<RefCell<Vec<(Wanted, Vec<u8>)>>> = Rc::default();
    let pending = js_sys::Array::new();
    let faces = Face::ALL.iter().filter(|f| f.compiled_in().is_none()).map(|&f| Wanted::Face(f));
    let pdf = vershion::fonts::PDF_STANDARD_FONTS.into_iter().map(Wanted::Pdf);
    for wanted in faces.chain(pdf) {
        let got = got.clone();
        pending.push(&wasm_bindgen_futures::future_to_promise(async move {
            let url = wanted.url();
            match fetch_bytes(&url).await {
                Ok(bytes) => got.borrow_mut().push((wanted, bytes)),
                Err(e) => log::warn!("verSHion: font {url} not loaded ({e})"),
            }
            Ok(wasm_bindgen::JsValue::UNDEFINED)
        }));
    }
    async move {
        if let Some(window) = web_sys::window() {
            let timeout = js_sys::Promise::new(&mut |resolve, _| {
                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, FONT_WAIT_MS);
            });
            let all = js_sys::Promise::all(&pending);
            let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::race(&js_sys::Array::of2(&all, &timeout))).await;
        }
        std::mem::take(&mut *got.borrow_mut())
    }
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    use wasm_bindgen_futures::JsFuture;
    let err = |e: wasm_bindgen::JsValue| e.as_string().unwrap_or_else(|| format!("{e:?}"));
    let window = web_sys::window().ok_or("no window")?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(url)).await.map_err(err)?.dyn_into().map_err(err)?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let buf = JsFuture::from(resp.array_buffer().map_err(err)?).await.map_err(err)?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

fn install_fonts(fetched: Vec<(Wanted, Vec<u8>)>) {
    let (mut faces, mut pdf) = (Vec::new(), Vec::new());
    for (wanted, bytes) in fetched {
        match wanted {
            Wanted::Face(f) => faces.push((f, bytes)),
            Wanted::Pdf(file) => pdf.push((file, bytes)),
        }
    }
    let used = vershion::fonts::install_fetched(faces);
    let missing: Vec<&str> = Face::ALL
        .iter()
        .filter(|f| f.compiled_in().is_none() && !used.contains(f))
        .map(|f| f.file())
        .collect();
    if !missing.is_empty() {
        log::warn!("verSHion: drawing with stand-in faces for {}", missing.join(", "));
    }
    let used = vershion::fonts::install_fetched_pdf_fonts(pdf);
    let missing: Vec<&str> = vershion::fonts::PDF_STANDARD_FONTS.into_iter().filter(|f| !used.contains(f)).collect();
    if !missing.is_empty() {
        log::warn!("verSHion: imported PDFs draw these standard fonts with house faces instead: {}", missing.join(", "));
    }
}

/// Holds the loading screen until its animation reaches the end of the cycle it is in, then fades
/// it out, so the logo always finishes a full pass before the app appears. Skipped when the
/// visitor prefers reduced motion (the logo is then a still image).
async fn finish_loader_cycle() {
    let Some(window) = web_sys::window() else { return };
    let reduced = window
        .match_media("(prefers-reduced-motion: reduce)")
        .ok()
        .flatten()
        .is_some_and(|m| m.matches());
    if reduced {
        return;
    }
    // `performance.now()` shares its origin with the document timeline the CSS animation runs on.
    let now = window.performance().map_or(0.0, |p| p.now());
    let remaining = LOADER_CYCLE_MS - now % LOADER_CYCLE_MS;
    sleep_ms(&window, remaining as i32).await;
    if let Some(el) = window
        .document()
        .and_then(|d| d.get_element_by_id(LOADING_ID))
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = el.style().set_property("opacity", "0");
        sleep_ms(&window, LOADER_FADE_MS).await;
    }
}

async fn sleep_ms(window: &web_sys::Window, ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms.max(0));
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

fn show_error(document: &web_sys::Document, message: &str) {
    if let Some(el) = document.get_element_by_id(LOADING_ID) {
        el.set_inner_html(&format!(
            "<p>{}</p><p>A browser with WebGL2 is required.</p>",
            html_escape(message)
        ));
    }
    log::error!("{message}");
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// eframe adapter for the host-agnostic app.
struct WebApp {
    app: vershion::App,
}

impl eframe::App for WebApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(ctx, raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }
}
