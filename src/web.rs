//! Standalone browser shell: IndexedDB store, eframe WebRunner, and the loading splash.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::egui;
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
