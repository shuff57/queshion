//! Web-only how-to video overlay. The clips live in `assets/help`; Trunk copies them to
//! `help/` next to the page.

use wasm_bindgen::JsCast as _;
use wasm_bindgen::closure::Closure;

const OVERLAY_ID: &str = "vershion_help";

/// Shows the clip for `provider` (the same provider names `platform::help_clip_name` maps).
pub fn open(provider: &str) {
    let Some(clip) = crate::platform::help_clip_name(provider) else { return };
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    close(&document);

    let Ok(overlay) = document.create_element("div") else { return };
    overlay.set_id(OVERLAY_ID);
    overlay.set_attribute("role", "dialog").ok();
    overlay.set_attribute("aria-modal", "true").ok();
    overlay.set_attribute("aria-label", "How to get an API key").ok();
    overlay
        .set_attribute(
            "style",
            "position:fixed;inset:0;z-index:20;display:grid;place-items:center;\
             background:rgba(0,0,0,.65);padding:24px",
        )
        .ok();

    let Ok(panel) = document.create_element("div") else { return };
    panel
        .set_attribute("style", "position:relative;width:min(960px,100%)")
        .ok();

    let Ok(video) = document.create_element("video") else { return };
    video.set_attribute("src", &format!("help/{clip}.mp4")).ok();
    video.set_attribute("controls", "").ok();
    video.set_attribute("autoplay", "").ok();
    video.set_attribute("playsinline", "").ok();
    video
        .set_attribute(
            "style",
            "width:100%;display:block;border-radius:10px;background:#000",
        )
        .ok();

    let Ok(close_btn) = document.create_element("button") else { return };
    close_btn.set_attribute("type", "button").ok();
    close_btn.set_attribute("aria-label", "Close").ok();
    close_btn
        .set_attribute(
            "style",
            "position:absolute;top:-12px;right:-12px;border:0;background:#2b211c;color:#fff;\
             border-radius:999px;width:34px;height:34px;cursor:pointer;font-size:20px;\
             line-height:1;box-shadow:0 2px 8px rgba(0,0,0,.4)",
        )
        .ok();
    close_btn.set_inner_html("&times;");

    // Backdrop closes; the panel swallows clicks so the video controls never close it.
    if let Some(target) = overlay.dyn_ref::<web_sys::EventTarget>() {
        let f = Closure::once_into_js(move |_: web_sys::Event| close_by_id());
        let _ = target.add_event_listener_with_callback("click", f.unchecked_ref());
    }
    if let Some(target) = panel.dyn_ref::<web_sys::EventTarget>() {
        let f = Closure::once_into_js(move |ev: web_sys::Event| ev.stop_propagation());
        let _ = target.add_event_listener_with_callback("click", f.unchecked_ref());
    }
    if let Some(target) = close_btn.dyn_ref::<web_sys::EventTarget>() {
        let f = Closure::once_into_js(move |ev: web_sys::Event| {
            ev.stop_propagation();
            close_by_id();
        });
        let _ = target.add_event_listener_with_callback("click", f.unchecked_ref());
    }

    panel.append_child(video.unchecked_ref::<web_sys::Node>()).ok();
    panel.append_child(close_btn.unchecked_ref::<web_sys::Node>()).ok();
    overlay.append_child(panel.unchecked_ref::<web_sys::Node>()).ok();
    if let Some(body) = document.body() {
        body.append_child(overlay.unchecked_ref::<web_sys::Node>()).ok();
    }
}

fn close(document: &web_sys::Document) {
    if let Some(el) = document.get_element_by_id(OVERLAY_ID) {
        el.remove();
    }
}

fn close_by_id() {
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        close(&document);
    }
}
