//! Shared-pool accounts: the client side of docs/shared-pool.md "Accounts and auth".
//!
//! Sharing is an opt-in layer. Everything else in verSHion works offline and signed out, and this
//! module does nothing (no network, no keychain) until the person opens the Account screen.
//!
//! - [`transport`]: the HTTP seam ([`transport::Transport`]); ureq natively, `fetch` in the browser.
//! - [`client`]: every auth route, the token lifecycle and single-flight refresh. No UI.
//! - [`vault`]: native refresh and device tokens in the OS keychain (never in a file).
//! - [`api`]: wire types, error text, link and server-address parsing.
//! - [`model`]: the Account screen's state machine (pages, jobs, status polling). No egui.
//! - [`ui`]: the egui Account window.
//!
//! # Which server (decided when the app is built)
//!
//! - **Release:** `VERSHION_POOL_URL=https://api.example.org cargo build --release`, and for the
//!   web `VERSHION_POOL_URL=https://api.example.org trunk build --release`. The app then talks
//!   only to that server; nothing in the app or in `share.json` can change it, and no
//!   confirmation is asked. `build.rs` fails the build if the address is not a plain https URL.
//! - **Release without `VERSHION_POOL_URL`:** sharing is switched off. The Account window says
//!   "Sharing is not set up in this build" and has no page that takes a password.
//! - **Development** (debug builds, or `--features pool-custom-server`, which is off by default
//!   and must never ship): the server address is typed in the app (`share.json`), may be
//!   `http://localhost`, every password page carries a "DEVELOPMENT BUILD" banner, and each
//!   address must be confirmed before any password is sent to it.
//!
//! The owner still has to decide the production API URL (none exists yet).
//!
//! `share.json` (in the store) holds the development address and the list of saved sign-ins the
//! keychain would not delete. Neither is a secret.

pub mod api;
pub mod client;
pub mod model;
pub mod secret;
pub mod transport;
pub mod ui;
pub mod vault;

#[cfg(test)]
mod tests;

pub use model::AccountModel;
pub use ui::AccountUi;

/// `share.json`: kept apart from `settings.json` so the AI settings code never sees it. Neither
/// field is a secret.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShareSettings {
    /// Validated base URL of the pool API, or empty. Ignored when the build has its own
    /// (`VERSHION_POOL_URL`).
    #[serde(default)]
    pub base_url: String,
    /// Saved sign-ins (per server and account) the keychain would not delete: deleted, and never
    /// used, first thing the next time the Account screen opens for that server.
    #[serde(default)]
    pub purge: Vec<PurgeEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PurgeEntry {
    pub server: String,
    pub account: String,
}

/// Shown instead of every account page by a release build made without `VERSHION_POOL_URL`.
pub const NOT_SET_UP: &str = "Sharing is not set up in this build of verSHion. Everything else works as usual. (The \
     person who builds verSHion for your school sets the pool server when building it.)";

pub const SETTINGS_DOC: &str = "share.json";

impl ShareSettings {
    pub fn load() -> ShareSettings {
        ShareSettings::checked(crate::store::load_json(SETTINGS_DOC).unwrap_or_default())
    }

    /// A hand-edited file with a bad address counts as not set up; junk in `purge` is dropped.
    pub fn checked(s: ShareSettings) -> ShareSettings {
        ShareSettings {
            base_url: api::validate_base_url(&s.base_url).unwrap_or_default(),
            purge: s
                .purge
                .into_iter()
                .filter(|e| {
                    api::validate_base_url(&e.server).is_ok_and(|b| b == e.server)
                        && e.account.len() <= 64
                        && e.account.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
                })
                .collect(),
        }
    }

    /// The server this build was made for (`VERSHION_POOL_URL` at compile time). When present it
    /// is the only server the app will talk to. Never read at run time.
    pub fn built_in() -> Option<String> {
        option_env!("VERSHION_POOL_URL").filter(|s| !s.is_empty()).map(str::to_string)
    }

    pub fn save(&self) -> Result<(), String> {
        crate::store::save_json(SETTINGS_DOC, self)
    }

    pub fn configured(&self) -> bool {
        !self.base_url.is_empty()
    }
}

/// Web deep link: the page was opened from a mailed link (`/account/verify#token=...`, `reset`,
/// `enrol`). Returns the link and removes the token from the address bar and history, so it is
/// not left behind in the tab or a bookmark.
#[cfg(target_arch = "wasm32")]
pub fn take_web_deep_link() -> Option<api::Link> {
    let window = web_sys::window()?;
    let loc = window.location();
    let path = loc.pathname().ok()?;
    let hash = loc.hash().ok()?;
    let mut link = api::link_from_location(&path, &hash)?;
    link.host = loc.hostname().ok();
    // Back to the app's own page: the path without its /account/... tail.
    let root = path.find("/account/").map_or(path.as_str(), |i| &path[..i]).to_string();
    let root = if root.is_empty() { "/".to_string() } else { format!("{}/", root.trim_end_matches('/')) };
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&root));
    }
    Some(link)
}
