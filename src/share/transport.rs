//! The HTTP seam of the shared-pool client.
//!
//! The client builds a [`Request`] and awaits a [`Response`] through [`Transport`]; nothing else in
//! `share` touches the network, so every flow runs against a mock in tests. Two real transports:
//! - [`NativeTransport`] (ureq): sends `X-Client: native` and never an `Origin` header, which is
//!   what makes the server return refresh and device tokens in bodies (and what it requires: a
//!   native request with an `Origin` is refused).
//! - [`WebTransport`] (`fetch`): `credentials: "include"` so the HttpOnly refresh and device cookies
//!   travel, and no `X-Client` or `X-Device-Token` (the browser path is cookie-only).
//!
//! Requests and responses redact their bodies and credential headers in `Debug`.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use super::secret::Secret;

/// A boxed future that need not be `Send` (browser futures are not).
pub type LocalFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Delete => "DELETE",
        }
    }
}

/// Headers whose values are credentials. Never printed.
const SECRET_HEADERS: [&str; 3] = ["authorization", "x-device-token", "x-sudo-token"];

pub struct Request {
    pub method: Method,
    /// Absolute URL: the configured base plus the route path.
    pub url: String,
    pub headers: Vec<(String, Secret)>,
    /// JSON text. It can hold a password or token, so it is a [`Secret`] too.
    pub body: Option<Secret>,
    /// A shorter time limit than [`DEFAULT_TIMEOUT`] (refresh: see `client::REFRESH_TIMEOUT`).
    pub timeout: Option<Duration>,
}

/// Every route but refresh.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.expose())
    }

    /// The route part of the URL (`/auth/login`), for mocks and messages.
    pub fn path(&self) -> &str {
        let after_scheme = self.url.split_once("://").map_or(self.url.as_str(), |(_, r)| r);
        let p = after_scheme.find('/').map_or("", |i| &after_scheme[i..]);
        p.split(['?', '#']).next().unwrap_or("")
    }

    pub fn json(&self) -> Option<serde_json::Value> {
        self.body.as_ref().and_then(|b| serde_json::from_str(b.expose()).ok())
    }
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let headers: Vec<String> = self
            .headers
            .iter()
            .map(|(k, v)| {
                if SECRET_HEADERS.contains(&k.to_ascii_lowercase().as_str()) {
                    format!("{k}: <redacted>")
                } else {
                    format!("{k}: {}", v.expose())
                }
            })
            .collect();
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &headers)
            .field("body", &self.body.as_ref().map(|b| format!("<{} bytes redacted>", b.expose().len())))
            .field("timeout", &self.timeout)
            .finish()
    }
}

pub struct Response {
    pub status: u16,
    /// `Retry-After` in seconds, when the server sent one (429s).
    pub retry_after: Option<u64>,
    /// JSON text (or empty for 204). It can hold tokens, so it is a [`Secret`].
    pub body: Secret,
}

impl Response {
    pub fn new(status: u16, body: &str) -> Response {
        Response { status, retry_after: None, body: Secret::new(body) }
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(self.body.expose()).unwrap_or(serde_json::Value::Null)
    }
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("retry_after", &self.retry_after)
            .field("body", &format!("<{} bytes redacted>", self.body.expose().len()))
            .finish()
    }
}

/// The request never got an HTTP answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetError {
    pub kind: NetKind,
    /// A sentence naming the host and the cause; never a token.
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetKind {
    /// No connection: offline, DNS, refused, reset, timeout, or (in a browser) a CORS refusal,
    /// which a page cannot tell apart.
    Offline,
    /// A connection, but no secure one: a certificate this computer does not trust (school
    /// networks that inspect traffic install their own) or a broken TLS handshake.
    Tls,
}

impl NetError {
    pub fn offline(text: impl Into<String>) -> NetError {
        NetError { kind: NetKind::Offline, text: text.into() }
    }
    pub fn tls(text: impl Into<String>) -> NetError {
        NetError { kind: NetKind::Tls, text: text.into() }
    }
}

pub trait Transport: Send + Sync {
    fn send<'a>(&'a self, req: Request) -> LocalFuture<'a, Result<Response, NetError>>;
}

/// Which delivery the server uses for this client (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Native,
    Web,
}

impl Mode {
    /// The build's own mode.
    pub fn current() -> Mode {
        if cfg!(target_arch = "wasm32") { Mode::Web } else { Mode::Native }
    }
}

/// The header policy both real transports apply on top of what the client set: native always
/// identifies itself and never claims an origin; the browser never sends the native-only headers
/// (the server's preflight would refuse them anyway).
pub fn finalize_headers(mode: Mode, headers: &mut Vec<(String, Secret)>) {
    headers.retain(|(k, _)| {
        let k = k.to_ascii_lowercase();
        match mode {
            Mode::Native => k != "origin" && k != "x-client",
            Mode::Web => k != "x-client" && k != "x-device-token" && k != "origin",
        }
    });
    if mode == Mode::Native {
        headers.push(("X-Client".into(), Secret::new("native")));
    }
}

fn parse_retry_after(v: Option<&str>) -> Option<u64> {
    v.and_then(|s| s.trim().parse::<u64>().ok())
}

/// Native: one blocking ureq call per request. The future is ready when polled, which is fine
/// because native jobs run on their own thread (see `model::Runner`).
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeTransport {
    agent: ureq::Agent,
    /// Same settings with the short limit, for requests that ask for one (refresh).
    short: ureq::Agent,
    short_limit: Duration,
}

#[cfg(not(target_arch = "wasm32"))]
fn native_agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        // An auth API never redirects; following one could carry a body to another host.
        .max_redirects(0)
        // The OS trust store, not a bundled root list: school networks that inspect TLS install
        // their own root certificate there, and a bundled list would refuse every request.
        .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeTransport {
    fn default() -> Self {
        let short_limit = super::client::REFRESH_TIMEOUT;
        NativeTransport { agent: native_agent(DEFAULT_TIMEOUT), short: native_agent(short_limit), short_limit }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeTransport {
    fn send_blocking(&self, mut req: Request) -> Result<Response, NetError> {
        finalize_headers(Mode::Native, &mut req.headers);
        let host = host_of(&req.url);
        let net = |e: ureq::Error| classify(&host, &e);
        let agent = match req.timeout {
            Some(t) if t <= self.short_limit => &self.short,
            _ => &self.agent,
        };
        let resp = match req.method {
            Method::Get => {
                let mut b = agent.get(&req.url);
                for (k, v) in &req.headers {
                    b = b.header(k.as_str(), v.expose());
                }
                b.call()
            }
            Method::Post | Method::Delete => {
                let mut b = if req.method == Method::Post {
                    agent.post(&req.url)
                } else {
                    agent.delete(&req.url).force_send_body()
                };
                for (k, v) in &req.headers {
                    b = b.header(k.as_str(), v.expose());
                }
                match &req.body {
                    Some(body) => b.header("Content-Type", "application/json").send(body.expose()),
                    None => b.send_empty(),
                }
            }
        };
        let mut resp = resp.map_err(net)?;
        let status = resp.status().as_u16();
        let retry_after = parse_retry_after(resp.headers().get("retry-after").and_then(|v| v.to_str().ok()));
        // The status is the answer; a body that can't be read (a 204 labelled gzip with nothing in
        // it, a truncated error page) is an empty body, not "offline": the request did arrive.
        let body = if status == 204 {
            String::new()
        } else {
            resp.body_mut().with_config().limit(64 * 1024).read_to_string().unwrap_or_default()
        };
        Ok(Response { status, retry_after, body: Secret::new(body) })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Transport for NativeTransport {
    fn send<'a>(&'a self, req: Request) -> LocalFuture<'a, Result<Response, NetError>> {
        let out = self.send_blocking(req);
        Box::pin(std::future::ready(out))
    }
}

/// Sorts a ureq failure into offline vs. TLS, with a short reason (its full text can include the
/// URL, never anything secret, but a short reason reads better in a dialog).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn classify(host: &str, e: &ureq::Error) -> NetError {
    let tls_text = |why: &str| NetError::tls(format!("No secure connection to {host} could be made ({why})."));
    match e {
        ureq::Error::Rustls(r) => tls_text(&short(&r.to_string())),
        ureq::Error::Tls(why) => tls_text(why),
        ureq::Error::Pem(_) => tls_text("certificate"),
        ureq::Error::Timeout(_) => NetError::offline(format!("Can't reach {host}: the connection timed out.")),
        ureq::Error::HostNotFound => NetError::offline(format!("Can't reach {host}: the server name was not found.")),
        ureq::Error::ConnectionFailed => NetError::offline(format!("Can't reach {host}: the connection failed.")),
        // rustls reports handshake and certificate failures through the stream as I/O errors.
        ureq::Error::Io(io) if is_tls_io(io) => tls_text(&short(&io.to_string())),
        ureq::Error::Io(io) => NetError::offline(format!("Can't reach {host}: {}.", io.kind())),
        other => NetError::offline(format!("Can't reach {host}: {}.", short(&other.to_string()))),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn is_tls_io(io: &std::io::Error) -> bool {
    let inner = io.get_ref().map(|e| e.to_string().to_ascii_lowercase()).unwrap_or_default();
    io.kind() == std::io::ErrorKind::InvalidData
        || ["certificate", "handshake", "tls", "alert"].iter().any(|w| inner.contains(w))
}

#[cfg(not(target_arch = "wasm32"))]
fn short(s: &str) -> String {
    s.chars().take(120).collect()
}

pub(crate) fn host_of(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_else(|| "the server".into())
}

/// Browser: `fetch` with CORS and credentials, 30 s timeout. The access token is only ever in the
/// `Authorization` header of a request; nothing is written to any browser storage.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct WebTransport;

#[cfg(target_arch = "wasm32")]
impl Transport for WebTransport {
    fn send<'a>(&'a self, mut req: Request) -> LocalFuture<'a, Result<Response, NetError>> {
        Box::pin(async move {
            use wasm_bindgen::JsCast as _;
            finalize_headers(Mode::Web, &mut req.headers);
            let host = host_of(&req.url);
            let unreachable = || {
                NetError::offline(format!(
                    "Can't reach {host}. You may be offline, or the server does not allow requests from this page."
                ))
            };
            let headers = web_sys::Headers::new().map_err(|_| unreachable())?;
            for (k, v) in &req.headers {
                headers.set(k, v.expose()).map_err(|_| unreachable())?;
            }
            let init = web_sys::RequestInit::new();
            init.set_method(req.method.as_str());
            init.set_mode(web_sys::RequestMode::Cors);
            init.set_credentials(web_sys::RequestCredentials::Include);
            // Like the native agent (`max_redirects(0)`): an auth API never redirects, and
            // following one could carry a body to another host.
            init.set_redirect(web_sys::RequestRedirect::Error);
            if let Some(body) = &req.body {
                headers.set("Content-Type", "application/json").map_err(|_| unreachable())?;
                init.set_body(&wasm_bindgen::JsValue::from_str(body.expose()));
            }
            init.set_headers(&headers);
            let limit = req.timeout.unwrap_or(DEFAULT_TIMEOUT).as_millis() as u32;
            let signal = web_sys::AbortSignal::timeout_with_u32(limit);
            init.set_signal(Some(&signal));
            let window = web_sys::window().ok_or_else(unreachable)?;
            let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init(&req.url, &init))
                .await
                .map_err(|_| unreachable())?;
            let resp: web_sys::Response = resp.dyn_into().map_err(|_| unreachable())?;
            let status = resp.status();
            let retry_after = parse_retry_after(resp.headers().get("Retry-After").ok().flatten().as_deref());
            let text = match resp.text() {
                Ok(p) => wasm_bindgen_futures::JsFuture::from(p).await.ok().and_then(|v| v.as_string()).unwrap_or_default(),
                Err(_) => String::new(),
            };
            Ok(Response { status, retry_after, body: Secret::new(text) })
        })
    }
}

/// Runs a future to completion on the current thread, parking between polls. Native jobs use
/// it on their worker thread; tests use it to drive the client. (The browser uses
/// `spawn_local` instead and never blocks.)
#[cfg(not(target_arch = "wasm32"))]
pub fn block_on<F: Future>(fut: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}
