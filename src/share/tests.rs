//! Shared-pool client tests. No network: a fake server behind the [`Transport`] seam, plus one
//! loopback socket to check the real ureq transport's headers.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::Poll;
use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use super::api::{self, ApiError, Flow, LinkKind, Profile, Status};
use super::client::{Client, REFRESH_RETRY_WINDOW};
use super::model::{AccountModel, MailKind, Notice, Page, Poller, Runner, SettingsIo};
use super::secret::Secret;
use super::transport::{LocalFuture, Method, Mode, NetError, Request, Response, Transport, block_on, finalize_headers};
use super::vault::{MemoryVault, Slot, Vault, VaultError};

const EMAIL: &str = "teacher@school.example.org";
const PASSWORD: &str = "correct horse battery staple";
const BASE: &str = "https://pool.test";
/// The fake server's one account, and a second one for the two-account tests.
const UID: &str = "11111111-2222-4333-8444-555555555555";
const UID2: &str = "99999999-8888-4777-8666-555555555555";

/// A device token in the server's format (`v1.<user>.<epoch>.<expiry>.<nonce>.<mac>`).
fn dev_token(uid: &str, n: u32) -> String {
    format!("v1.{uid}.1.9999999999.{n:0>12}.{}", "m".repeat(43))
}

// ---- a fake pool server ---------------------------------------------------------------

#[derive(Clone, Debug)]
struct Logged {
    method: Method,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Value>,
    timeout: Option<Duration>,
}

impl Logged {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

type OnRequest = Box<dyn Fn(&str) + Send + Sync>;

/// Just enough of server/auth to exercise every client flow: one active account, rotating
/// refresh tokens with the grace window, device tokens, a lockdown switch, and scripted overrides.
struct Fake {
    password: String,
    access: Option<String>,
    refresh: Option<String>,
    prev_refresh: Option<String>,
    device: Option<String>,
    n: u32,
    lockdown: bool,
    status: Value,
    /// Scripted answers for a path, used before the fake's own logic.
    script: VecDeque<(String, Result<Response, NetError>)>,
    log: Vec<Logged>,
    refresh_calls: usize,
    /// Sleep inside /auth/refresh (widens the window for the concurrency tests).
    refresh_delay: Duration,
    /// Return Pending once from /auth/refresh (single-thread interleaving).
    refresh_yields: bool,
    /// Called on each request, e.g. to move a test clock.
    on_request: Option<OnRequest>,
    /// Verify/reset links belong to this account instead of the fake's own.
    link_account: Option<String>,
    /// Enrolment codes belong to this account instead of the fake's own.
    enrol_account: Option<String>,
    /// Every refresh token presented, in order.
    presented: Vec<String>,
    /// Presentations of an already-rotated token (answered from the grace window).
    replays: usize,
    /// Rotate on the next refresh but lose the reply.
    lose_rotation_reply: bool,
}

impl Default for Fake {
    fn default() -> Self {
        Fake {
            password: PASSWORD.into(),
            access: None,
            refresh: None,
            prev_refresh: None,
            device: None,
            n: 0,
            lockdown: false,
            status: json!({"state": "pending"}),
            script: VecDeque::new(),
            log: Vec::new(),
            refresh_calls: 0,
            refresh_delay: Duration::ZERO,
            refresh_yields: false,
            on_request: None,
            link_account: None,
            enrol_account: None,
            presented: Vec::new(),
            replays: 0,
            lose_rotation_reply: false,
        }
    }
}

fn token(prefix: &str, n: u32) -> String {
    // 43 characters, like the server's tokens.
    format!("{prefix}{n:0>width$}", width = 43 - prefix.len())
}

fn ok(status: u16, body: Value) -> Response {
    Response::new(status, &body.to_string())
}

fn err(status: u16, code: &str) -> Response {
    ok(status, json!({"error": code}))
}

impl Fake {
    fn issue(&mut self) -> Value {
        self.n += 1;
        self.access = Some(token("at", self.n));
        self.prev_refresh = self.refresh.take();
        self.refresh = Some(token("rt", self.n));
        json!({"access_token": self.access, "token_type": "Bearer", "expires_in": 900, "refresh_token": self.refresh,
               "refresh_expires_in": 2_592_000, "user": {"id": UID, "trust": "pending"}})
    }

    fn bearer_ok(&self, req: &Logged) -> bool {
        req.header("Authorization").is_some_and(|h| Some(h.trim_start_matches("Bearer ")) == self.access.as_deref())
    }

    fn handle(&mut self, req: &Logged) -> Result<Response, NetError> {
        if let Some(i) = self.script.iter().position(|(p, _)| *p == req.path) {
            return self.script.remove(i).expect("present").1;
        }
        let body = req.body.clone().unwrap_or(Value::Null);
        let s = |k: &str| body.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Ok(match req.path.as_str() {
            "/auth/signup" | "/auth/resend" | "/auth/forgot" => ok(202, json!({"ok": true})),
            "/auth/verify" | "/auth/reset" => {
                if !api::is_token(&s("token")) {
                    err(400, "invalid_token")
                } else if s("password").chars().count() < 10 {
                    ok(400, json!({"error": "weak_password", "reason": "too_short"}))
                } else {
                    if self.link_account.is_none() {
                        self.password = s("password");
                    }
                    let uid = self.link_account.clone().unwrap_or_else(|| UID.to_string());
                    let d = dev_token(&uid, self.n + 100);
                    if uid == UID {
                        self.access = None;
                        self.refresh = None;
                        self.device = Some(d.clone());
                    }
                    ok(200, json!({"ok": true, "password_set": true, "device_token": d}))
                }
            }
            "/auth/login" => {
                let known = req.header("X-Device-Token").is_some() && req.header("X-Device-Token") == self.device.as_deref();
                if self.lockdown && !known {
                    let mut r = err(429, "rate_limited");
                    r.retry_after = Some(600);
                    r
                } else if s("email") != EMAIL || s("password") != self.password {
                    err(401, "invalid_credentials")
                } else {
                    let mut v = self.issue();
                    self.device = Some(dev_token(UID, self.n));
                    v["device_token"] = json!(self.device);
                    ok(200, v)
                }
            }
            "/auth/refresh" => {
                self.refresh_calls += 1;
                std::thread::sleep(self.refresh_delay);
                let presented = s("refresh_token");
                self.presented.push(presented.clone());
                if self.refresh.as_deref() == Some(presented.as_str()) {
                    let r = ok(200, self.issue());
                    if std::mem::take(&mut self.lose_rotation_reply) {
                        // Rotated on the server, but the reply never arrives.
                        return Err(NetError::offline("Can't reach pool.test: the connection timed out."));
                    }
                    r
                } else if self.prev_refresh.as_deref() == Some(presented.as_str()) {
                    self.replays += 1;
                    // Inside the grace window: the same successor.
                    ok(200, json!({"access_token": self.access, "expires_in": 900, "refresh_token": self.refresh}))
                } else {
                    err(401, "invalid_refresh_token")
                }
            }
            "/auth/device/enrol" => {
                if s("code") != token("code", 1) {
                    err(400, "invalid_token")
                } else if s("password") != self.password {
                    err(401, "invalid_credentials")
                } else {
                    self.device = Some(dev_token(self.enrol_account.as_deref().unwrap_or(UID), 900));
                    ok(200, json!({"ok": true, "device_token": self.device}))
                }
            }
            "/auth/logout" => {
                if body.get("scope").and_then(Value::as_str) == Some("global") && s("refresh_token").is_empty() {
                    err(401, "refresh_token_required")
                } else {
                    self.access = None;
                    self.refresh = None;
                    Response::new(204, "")
                }
            }
            "/me" if self.bearer_ok(req) => ok(
                200,
                json!({"status": self.status, "id": UID, "email": EMAIL, "email_verified": true,
                       "display_name": "Ms Frizzle", "profile": {"school": "Walkerville Elementary"}, "trust": "pending",
                       "created_at": 1, "admin_note": "SECRET REVIEWER NOTE"}),
            ),
            "/account" if req.method == Method::Delete && self.bearer_ok(req) => {
                if s("password") == self.password {
                    Response::new(204, "")
                } else {
                    err(401, "invalid_credentials")
                }
            }
            "/account/reapply" if self.bearer_ok(req) => ok(202, json!({"ok": true})),
            "/me" | "/account" | "/account/reapply" => err(401, "unauthorized"),
            _ => err(404, "not_found"),
        })
    }
}

#[derive(Clone, Default)]
struct Server(Arc<Mutex<Fake>>, Arc<std::sync::atomic::AtomicBool>);

impl Server {
    fn with(&self, f: impl FnOnce(&mut Fake)) {
        f(&mut self.0.lock().unwrap());
    }
    fn log(&self) -> Vec<Logged> {
        self.0.lock().unwrap().log.clone()
    }
    fn count(&self) -> usize {
        self.0.lock().unwrap().log.len()
    }
    fn paths(&self) -> Vec<String> {
        self.log().into_iter().map(|l| l.path).collect()
    }
    fn refresh_calls(&self) -> usize {
        self.0.lock().unwrap().refresh_calls
    }
    fn last(&self, path: &str) -> Logged {
        self.log().into_iter().rev().find(|l| l.path == path).unwrap_or_else(|| panic!("no request to {path}"))
    }
}

/// The transport the client sees: applies the real header policy, then the fake.
struct MockTransport {
    server: Server,
    mode: Mode,
}

impl Transport for MockTransport {
    fn send<'a>(&'a self, mut req: Request) -> LocalFuture<'a, Result<Response, NetError>> {
        finalize_headers(self.mode, &mut req.headers);
        let logged = Logged {
            method: req.method,
            path: req.path().to_string(),
            headers: req.headers.iter().map(|(k, v)| (k.clone(), v.expose().to_string())).collect(),
            body: req.json(),
            timeout: req.timeout,
        };
        // Panics outside the fake's lock (a panic under it would poison the fake, not the client).
        if logged.path == "/auth/refresh" && self.server.1.swap(false, Ordering::SeqCst) {
            panic!("transport blew up mid-refresh");
        }
        let yields = logged.path == "/auth/refresh" && self.server.0.lock().unwrap().refresh_yields;
        let server = self.server.clone();
        Box::pin(async move {
            if yields {
                let mut once = false;
                std::future::poll_fn(|cx| {
                    if once {
                        Poll::Ready(())
                    } else {
                        once = true;
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                })
                .await;
            }
            // The callback runs without the lock (it may move a clock).
            let cb = server.0.lock().unwrap().on_request.take();
            if let Some(cb) = &cb {
                cb(&logged.path);
            }
            let mut f = server.0.lock().unwrap();
            if cb.is_some() {
                f.on_request = cb;
            }
            f.log.push(logged.clone());
            f.handle(&logged)
        })
    }
}

/// A vault the test can look into after the client took ownership.
#[derive(Clone, Default)]
struct SharedVault {
    inner: Arc<MemoryVault>,
    writes: Arc<AtomicUsize>,
    broken: bool,
    /// Deletes fail while set (a locked keychain).
    deletes_fail: Arc<std::sync::atomic::AtomicBool>,
    /// Refresh-token writes fail while set.
    refresh_writes_fail: Arc<std::sync::atomic::AtomicBool>,
}

impl Vault for SharedVault {
    fn load(&self, slot: &Slot) -> Result<Option<Secret>, VaultError> {
        self.inner.load(slot)
    }
    fn save(&self, slot: &Slot, value: &Secret) -> Result<(), VaultError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.broken || (self.refresh_writes_fail.load(Ordering::SeqCst) && matches!(slot, Slot::Refresh(_))) {
            return Err(VaultError("locked".into()));
        }
        self.inner.save(slot, value)
    }
    fn clear(&self, slot: &Slot) -> Result<(), VaultError> {
        if self.deletes_fail.load(Ordering::SeqCst) {
            return Err(VaultError("locked".into()));
        }
        self.inner.clear(slot)
    }
    fn persistent(&self) -> bool {
        true
    }
}

impl SharedVault {
    fn get(&self, slot: Slot) -> Option<String> {
        self.inner.load(&slot).unwrap().map(|s| s.expose().to_string())
    }
    fn refresh(&self) -> Option<String> {
        self.get(Slot::Refresh(UID.into()))
    }
    fn device(&self) -> Option<String> {
        self.get(Slot::Device(UID.into()))
    }
    /// A session saved by an earlier run: index plus refresh token.
    fn seed_session(&self, refresh: &str) {
        self.inner.save(&Slot::Index, &Secret::new(json!({"current": UID}).to_string())).unwrap();
        self.inner.save(&Slot::Refresh(UID.into()), &Secret::new(refresh)).unwrap();
    }
}

fn client(mode: Mode, server: &Server, vault: &SharedVault) -> Client {
    Client::new(BASE, mode, Box::new(MockTransport { server: server.clone(), mode }), Box::new(vault.clone()))
}

fn native() -> (Client, Server, SharedVault) {
    let (s, v) = (Server::default(), SharedVault::default());
    (client(Mode::Native, &s, &v), s, v)
}

fn signed_in() -> (Client, Server, SharedVault) {
    let (c, s, v) = native();
    block_on(c.sign_in(EMAIL, &Secret::new(PASSWORD))).unwrap();
    (c, s, v)
}

fn profile() -> Profile {
    Profile { display_name: "Ms Frizzle".into(), school: "Walkerville Elementary".into(), adult_school_employee: true }
}

fn pw(s: &str) -> Secret {
    Secret::new(s)
}

fn link_token() -> String {
    "x9Kq-_".repeat(7) + "A"
}

// ---- headers: native vs web -----------------------------------------------------------

#[test]
fn native_requests_identify_themselves_and_never_send_origin() {
    let (c, s, _) = native();
    block_on(c.sign_up(EMAIL, &pw(PASSWORD), &profile())).unwrap();
    let req = s.last("/auth/signup");
    assert_eq!(req.header("X-Client"), Some("native"));
    assert_eq!(req.header("Origin"), None);
    // Even if something upstream tried to add one, the policy strips it.
    let mut h = vec![("Origin".to_string(), Secret::new("https://evil.example")), ("x-client".into(), Secret::new("web"))];
    finalize_headers(Mode::Native, &mut h);
    assert_eq!(h.len(), 1);
    assert_eq!((h[0].0.as_str(), h[0].1.expose()), ("X-Client", "native"));
}

#[test]
fn web_requests_send_no_native_headers_and_keep_nothing() {
    let (s, v) = (Server::default(), SharedVault::default());
    let c = client(Mode::Web, &s, &v);
    // The web fake still answers with a body token; the web client must not keep it anywhere.
    block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap();
    let login = s.last("/auth/login");
    assert_eq!(login.header("X-Client"), None);
    assert_eq!(login.header("X-Device-Token"), None);
    assert_eq!(v.writes.load(Ordering::SeqCst), 0, "the browser build never writes a token to storage");
    let mut h = vec![("X-Device-Token".to_string(), Secret::new("d")), ("X-Client".into(), Secret::new("native"))];
    finalize_headers(Mode::Web, &mut h);
    assert!(h.is_empty());
}

#[test]
fn web_refresh_sends_no_body_the_cookie_carries_the_token() {
    let (s, v) = (Server::default(), SharedVault::default());
    let c = client(Mode::Web, &s, &v);
    s.with(|f| f.script.push_back(("/auth/refresh".into(), Ok(ok(200, json!({"access_token": "at-web", "expires_in": 900}))))));
    s.with(|f| f.access = Some("at-web".into()));
    let me = block_on(c.restore()).unwrap();
    assert_eq!(me.email, EMAIL);
    let r = s.last("/auth/refresh");
    assert!(r.body.is_none(), "web refresh must not put a token in the body");
    assert_eq!(s.last("/me").header("Authorization"), Some("Bearer at-web"));
}

#[test]
fn web_restore_without_a_cookie_is_quietly_signed_out() {
    let (s, v) = (Server::default(), SharedVault::default());
    let c = client(Mode::Web, &s, &v);
    assert_eq!(block_on(c.restore()), Err(ApiError::SignedOut));
    assert_eq!(s.paths(), vec!["/auth/refresh"]);
}

// ---- sign up, confirm -----------------------------------------------------------------

#[test]
fn sign_up_sends_the_profile_and_never_signs_in() {
    let (c, s, v) = native();
    block_on(c.sign_up(" Teacher@School.example.org ", &pw(PASSWORD), &profile())).unwrap();
    let b = s.last("/auth/signup").body.unwrap();
    assert_eq!(b["email"], "Teacher@School.example.org");
    assert_eq!(b["password"], PASSWORD);
    assert_eq!(b["display_name"], "Ms Frizzle");
    assert_eq!(b["school"], "Walkerville Elementary");
    assert_eq!(b["adult_school_employee"], true);
    assert!(!c.has_session());
    assert_eq!(v.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn sign_up_errors_map_to_specific_text() {
    let (c, s, _) = native();
    s.with(|f| f.script.push_back(("/auth/signup".into(), Ok(ok(400, json!({"error": "weak_password", "reason": "common"}))))));
    let e = block_on(c.sign_up(EMAIL, &pw("password1234"), &profile())).unwrap_err();
    assert!(e.message(Flow::SignUp).contains("too common"));
    s.with(|f| {
        f.script.push_back((
            "/auth/signup".into(),
            Ok(ok(400, json!({"error": "invalid_request", "field": "school", "reason": "required_plain_text"}))),
        ))
    });
    let e = block_on(c.sign_up(EMAIL, &pw(PASSWORD), &profile())).unwrap_err();
    assert_eq!(e.message(Flow::SignUp), "Enter your school as plain text.");
}

#[test]
fn confirm_sets_password_and_profile_and_keeps_the_device_token() {
    let (c, s, v) = native();
    let r = block_on(c.confirm(&Secret::new(link_token()), &pw("a fresh password!"), &profile())).unwrap();
    assert!(r.password_set && r.device_known);
    let b = s.last("/auth/verify").body.unwrap();
    assert_eq!(b["token"], link_token());
    assert_eq!(b["password"], "a fresh password!");
    assert_eq!(b["adult_school_employee"], true);
    assert!(v.device().is_some());
    assert!(!c.has_session(), "confirming does not sign in");
}

#[test]
fn confirm_errors() {
    let (c, s, _) = native();
    let t = Secret::new(link_token());
    for (resp, needle) in [
        (err(400, "invalid_token"), "already used, has expired"),
        (err(409, "in_progress"), "being used right now"),
        (ok(400, json!({"error": "weak_password", "reason": "contains_email"})), "must not contain your email"),
        (ok(400, json!({"error": "invalid_request", "field": "adult_school_employee", "reason": "must_confirm"})), "adult employee"),
        (ok(400, json!({"error": "invalid_request", "field": "display_name", "reason": "invalid_characters_or_length"})), "display name has characters"),
    ] {
        s.with(|f| f.script.push_back(("/auth/verify".into(), Ok(resp))));
        let e = block_on(c.confirm(&t, &pw(PASSWORD), &profile())).unwrap_err();
        assert!(e.message(Flow::Confirm).contains(needle), "{e:?} -> {}", e.message(Flow::Confirm));
    }
}

// ---- sign in, lockdown, enrolment -----------------------------------------------------

#[test]
fn sign_in_keeps_refresh_and_device_in_the_vault_and_access_in_memory_only() {
    let (c, _s, v) = signed_in();
    assert!(c.has_session());
    assert_eq!(v.refresh(), Some(token("rt", 1)));
    assert_eq!(v.device(), Some(dev_token(UID, 1)));
    // Only two slots exist: the access token has nowhere to go but memory.
    assert_eq!(c.access_for_test(), Some(token("at", 1)));
}

#[test]
fn sign_in_sends_the_saved_device_token() {
    let (s, v) = (Server::default(), SharedVault::default());
    v.save(&Slot::Device(UID.into()), &Secret::new(dev_token(UID, 7))).unwrap();
    v.save(&Slot::Index, &Secret::new(json!({"emails": {EMAIL: UID}}).to_string())).unwrap();
    let c = client(Mode::Native, &s, &v);
    let _ = block_on(c.sign_in(EMAIL, &pw(PASSWORD)));
    assert_eq!(s.last("/auth/login").header("X-Device-Token"), Some(dev_token(UID, 7).as_str()));
}

#[test]
fn wrong_password_message_does_not_say_whether_the_account_exists() {
    let (c, _, _) = native();
    let e = block_on(c.sign_in(EMAIL, &pw("not the password"))).unwrap_err();
    assert_eq!(e.code(), "invalid_credentials");
    let unknown = block_on(c.sign_in("nobody@example.org", &pw(PASSWORD))).unwrap_err();
    assert_eq!(e.message(Flow::SignIn), unknown.message(Flow::SignIn));
    assert!(!c.has_session());
}

#[test]
fn lockdown_429_leads_to_enrolment_then_sign_in_works() {
    let (c, s, v) = native();
    s.with(|f| f.lockdown = true);
    let e = block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap_err();
    assert!(e.is_rate_limited());
    assert!(e.message(Flow::SignIn).contains("in about 10 minutes"), "{}", e.message(Flow::SignIn));
    assert!(e.message(Flow::SignIn).contains("device code"));
    // The mailed code turns this into a known device ...
    block_on(c.enrol(&Secret::new(token("code", 1)), &pw(PASSWORD))).unwrap();
    assert_eq!(v.device(), Some(dev_token(UID, 900)));
    let b = s.last("/auth/device/enrol");
    assert_eq!(b.header("X-Client"), Some("native"));
    assert_eq!(b.body.unwrap()["code"], token("code", 1));
    // ... and the device token gets it past the lockdown.
    block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap();
    assert!(c.has_session());
}

#[test]
fn enrol_errors() {
    let (c, s, _) = native();
    let e = block_on(c.enrol(&Secret::new(token("code", 2)), &pw(PASSWORD))).unwrap_err();
    assert!(e.message(Flow::Enrol).contains("This code can't be used"));
    let e = block_on(c.enrol(&Secret::new(token("code", 1)), &pw("wrong wrong wrong"))).unwrap_err();
    assert!(e.message(Flow::Enrol).contains("not right for the account"));
    s.with(|f| f.script.push_back(("/auth/device/enrol".into(), Ok(err(409, "in_progress")))));
    let e = block_on(c.enrol(&Secret::new(token("code", 1)), &pw(PASSWORD))).unwrap_err();
    assert!(e.message(Flow::Enrol).contains("code is being used"));
    // A 200 without the device token is not success for the native app.
    s.with(|f| f.script.push_back(("/auth/device/enrol".into(), Ok(ok(200, json!({"ok": true}))))));
    assert!(matches!(block_on(c.enrol(&Secret::new(token("code", 1)), &pw(PASSWORD))), Err(ApiError::BadReply(_))));
}

#[test]
fn suspended_sign_in() {
    let (c, s, _) = native();
    s.with(|f| f.script.push_back(("/auth/login".into(), Ok(err(403, "account_suspended")))));
    let e = block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap_err();
    assert!(e.message(Flow::SignIn).contains("suspended"));
}

// ---- refresh --------------------------------------------------------------------------

#[test]
fn a_401_refreshes_once_rotates_and_retries() {
    let (c, s, v) = signed_in();
    s.with(|f| f.access = Some("server-forgot-it".into())); // the access token is no longer accepted
    let me = block_on(c.me()).unwrap();
    assert_eq!(me.display_name, "Ms Frizzle");
    assert_eq!(s.refresh_calls(), 1);
    assert_eq!(s.last("/auth/refresh").body.unwrap()["refresh_token"], token("rt", 1));
    assert_eq!(v.refresh(), Some(token("rt", 2)), "the rotated token is saved");
    assert_eq!(s.paths(), vec!["/auth/login", "/me", "/auth/refresh", "/me"]);
}

#[test]
fn a_second_401_after_refreshing_signs_out_without_looping() {
    let (c, s, _) = signed_in();
    s.with(|f| {
        f.access = Some("x".into());
        f.script.push_back(("/me".into(), Ok(err(401, "unauthorized"))));
        f.script.push_back(("/me".into(), Ok(err(401, "unauthorized"))));
    });
    assert_eq!(block_on(c.me()), Err(ApiError::SignedOut));
    assert_eq!(s.refresh_calls(), 1);
}

#[test]
fn refresh_refused_ends_the_session_but_keeps_the_device() {
    let (c, s, v) = signed_in();
    s.with(|f| {
        f.access = None;
        f.refresh = Some("revoked family".into());
        f.prev_refresh = None;
    });
    assert_eq!(block_on(c.me()), Err(ApiError::SignedOut));
    assert!(!c.has_session());
    assert_eq!(v.refresh(), None);
    assert!(v.device().is_some(), "the known-device token survives an ended session");
}

#[test]
fn concurrent_401s_on_threads_share_one_refresh() {
    let (c, s, v) = signed_in();
    s.with(|f| {
        f.access = Some("stale".into());
        f.refresh_delay = Duration::from_millis(150);
    });
    let c = Arc::new(c);
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let c = c.clone();
            std::thread::spawn(move || block_on(c.me()))
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap().unwrap().email, EMAIL);
    }
    assert_eq!(s.refresh_calls(), 1, "exactly one refresh for eight concurrent 401s");
    assert_eq!(v.refresh(), Some(token("rt", 2)));
    let refreshes = s.log().iter().filter(|l| l.path == "/auth/refresh").count();
    assert_eq!(refreshes, 1);
}

#[test]
fn concurrent_401s_on_one_thread_share_one_refresh() {
    // The browser's case: one thread, interleaved futures.
    let (c, s, _) = signed_in();
    s.with(|f| {
        f.access = Some("stale".into());
        f.refresh_yields = true;
    });
    let futs: Vec<LocalFuture<'_, Result<api::Me, ApiError>>> = (0..5).map(|_| Box::pin(c.me()) as LocalFuture<'_, _>).collect();
    let results = block_on(join_all(futs));
    assert!(results.iter().all(|r| r.as_ref().is_ok_and(|m| m.email == EMAIL)), "{results:?}");
    assert_eq!(s.refresh_calls(), 1);
}

/// Polls every future until all are done (a minimal `join_all`).
fn join_all<'a, T>(mut futs: Vec<LocalFuture<'a, T>>) -> impl std::future::Future<Output = Vec<T>> + 'a
where
    T: 'a,
{
    let mut out: Vec<Option<T>> = (0..futs.len()).map(|_| None).collect();
    std::future::poll_fn(move |cx| {
        for (i, f) in futs.iter_mut().enumerate() {
            if out[i].is_none()
                && let Poll::Ready(v) = f.as_mut().poll(cx)
            {
                out[i] = Some(v);
            }
        }
        if out.iter().all(Option::is_some) { Poll::Ready(out.iter_mut().map(|o| o.take().unwrap()).collect()) } else { Poll::Pending }
    })
}

#[test]
fn a_call_that_lost_the_race_retries_with_the_new_token_instead_of_refreshing_again() {
    let (c, s, _) = signed_in();
    let before = c.generation();
    s.with(|f| f.access = Some("stale".into()));
    block_on(c.me()).unwrap(); // refreshes: generation moves on
    assert!(c.generation() > before);
    // A late 401 for the old generation must not refresh again.
    s.with(|f| f.script.push_back(("/me".into(), Ok(err(401, "unauthorized")))));
    block_on(c.me()).unwrap();
    assert_eq!(s.refresh_calls(), 2, "a fresh 401 on the current token does refresh");
}

fn test_clock() -> (Arc<Mutex<Instant>>, super::client::Clock) {
    let t = Arc::new(Mutex::new(Instant::now()));
    let t2 = t.clone();
    (t, Box::new(move || *t2.lock().unwrap()))
}

#[test]
fn a_lost_refresh_reply_is_retried_once_with_the_same_token_inside_the_grace_window() {
    let (s, v) = (Server::default(), SharedVault::default());
    v.seed_session(&token("rt", 0));
    s.with(|f| {
        f.refresh = Some(token("rt", 0));
        f.script.push_back(("/auth/refresh".into(), Err(NetError::offline("connection reset"))));
    });
    let c = client(Mode::Native, &s, &v);
    let me = block_on(c.restore()).unwrap();
    assert_eq!(me.email, EMAIL);
    let sent: Vec<Value> = s.log().into_iter().filter(|l| l.path == "/auth/refresh").map(|l| l.body.unwrap()).collect();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0]["refresh_token"], sent[1]["refresh_token"], "the retry presents the same token");
}

#[test]
fn a_lost_refresh_reply_is_not_replayed_after_the_grace_window() {
    let (s, v) = (Server::default(), SharedVault::default());
    v.seed_session(&token("rt", 0));
    let (clock, clock_fn) = test_clock();
    s.with(|f| {
        f.refresh = Some(token("rt", 0));
        f.script.push_back(("/auth/refresh".into(), Err(NetError::offline("timed out"))));
        // The failed attempt took longer than the retry window.
        let clock = clock.clone();
        f.on_request = Some(Box::new(move |_| *clock.lock().unwrap() += REFRESH_RETRY_WINDOW + Duration::from_secs(1)));
    });
    let c = Client::with_clock(BASE, Mode::Native, Box::new(MockTransport { server: s.clone(), mode: Mode::Native }), Box::new(v.clone()), clock_fn);
    let e = block_on(c.restore()).unwrap_err();
    assert!(matches!(e, ApiError::Offline(_)));
    assert_eq!(s.refresh_calls(), 0, "the scripted failure is the only refresh sent");
    assert_eq!(s.count(), 1);
    assert!(c.has_session(), "the token is kept for later, not thrown away");
    assert!(e.message(Flow::Status).contains("your work on this device is unaffected"));
}

#[test]
fn an_expiring_access_token_is_refreshed_before_use() {
    let (s, v) = (Server::default(), SharedVault::default());
    let (clock, clock_fn) = test_clock();
    let c = Client::with_clock(BASE, Mode::Native, Box::new(MockTransport { server: s.clone(), mode: Mode::Native }), Box::new(v.clone()), clock_fn);
    block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap();
    *clock.lock().unwrap() += Duration::from_secs(900);
    block_on(c.me()).unwrap();
    assert_eq!(s.paths(), vec!["/auth/login", "/auth/refresh", "/me"], "no wasted 401");
}

#[test]
fn a_keychain_that_refuses_writes_keeps_tokens_in_memory_and_says_so() {
    let s = Server::default();
    let v = SharedVault { broken: true, ..Default::default() };
    let c = client(Mode::Native, &s, &v);
    block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap();
    assert!(c.has_session());
    assert_eq!(c.persist_note(), Some(super::vault::KEYCHAIN_FAILED));
    assert!(block_on(c.me()).is_ok());
}

// ---- sign out -------------------------------------------------------------------------

#[test]
fn sign_out_sends_the_refresh_token_and_keeps_the_device() {
    let (c, s, v) = signed_in();
    block_on(c.sign_out(false, false)).unwrap();
    let b = s.last("/auth/logout").body.unwrap();
    assert_eq!(b["refresh_token"], token("rt", 1));
    assert!(b.get("scope").is_none() && b.get("forget_device").is_none());
    assert!(!c.has_session());
    assert_eq!(v.refresh(), None);
    assert!(v.device().is_some());
}

#[test]
fn sign_out_and_forget_this_device() {
    let (c, s, v) = signed_in();
    block_on(c.sign_out(false, true)).unwrap();
    assert_eq!(s.last("/auth/logout").body.unwrap()["forget_device"], true);
    assert_eq!(v.device(), None);
}

#[test]
fn sign_out_offline_still_ends_the_local_session() {
    let (c, s, v) = signed_in();
    s.with(|f| f.script.push_back(("/auth/logout".into(), Err(NetError::offline("offline")))));
    assert!(matches!(block_on(c.sign_out(false, false)), Err(ApiError::Offline(_))));
    assert!(!c.has_session());
    assert_eq!(v.refresh(), None);
}

#[test]
fn sign_out_when_the_server_already_forgot_the_session_is_fine() {
    let (c, s, _) = signed_in();
    s.with(|f| f.script.push_back(("/auth/logout".into(), Ok(err(401, "invalid_refresh_token")))));
    assert_eq!(block_on(c.sign_out(false, false)), Ok(()));
    assert!(!c.has_session());
}

#[test]
fn sign_out_everywhere_needs_the_refresh_token_and_forgets_the_device() {
    let (c, s, v) = signed_in();
    block_on(c.sign_out(true, false)).unwrap();
    let b = s.last("/auth/logout").body.unwrap();
    assert_eq!(b["scope"], "global");
    assert_eq!(b["refresh_token"], token("rt", 1));
    assert_eq!(v.device(), None, "logout everywhere revokes every device token anyway");
}

#[test]
fn sign_out_everywhere_failing_still_leaves_this_device_signed_out() {
    let (c, s, v) = signed_in();
    s.with(|f| f.script.push_back(("/auth/logout".into(), Err(NetError::offline("offline")))));
    assert!(matches!(block_on(c.sign_out(true, false)), Err(ApiError::Offline(_))));
    assert!(!c.has_session(), "never left signed in (or saved) while waiting on the server");
    assert_eq!((v.refresh(), v.device()), (None, None));
    // The server got the refresh token in the body (it lived only for that call).
    assert_eq!(s.last("/auth/logout").body.unwrap()["refresh_token"], token("rt", 1));
    let e = ApiError::Http { status: 401, code: "refresh_token_required".into(), field: None, reason: None, retry_after: None };
    assert!(e.message(Flow::SignOutEverywhere).contains("Sign in again here"));
}

// ---- forgot, reset --------------------------------------------------------------------

#[test]
fn forgot_and_resend_are_202_for_any_address() {
    let (c, s, _) = native();
    block_on(c.forgot(EMAIL)).unwrap();
    block_on(c.forgot("nobody@example.org")).unwrap();
    block_on(c.resend("nobody@example.org")).unwrap();
    assert_eq!(s.last("/auth/forgot").body.unwrap(), json!({"email": "nobody@example.org"}));
    s.with(|f| {
        let mut r = err(429, "rate_limited");
        r.retry_after = Some(3600);
        f.script.push_back(("/auth/forgot".into(), Ok(r)));
    });
    let e = block_on(c.forgot(EMAIL)).unwrap_err();
    assert_eq!(e.message(Flow::Forgot), "Too many requests from here. Try again in about 60 minutes.");
}

#[test]
fn reset_ends_the_local_session_and_keeps_the_new_device_token() {
    let (c, s, v) = signed_in();
    let r = block_on(c.reset(&Secret::new(link_token()), &pw("brand new password"), None)).unwrap();
    assert!(r.device_known);
    let b = s.last("/auth/reset").body.unwrap();
    assert!(b.get("display_name").is_none(), "no profile unless asked for");
    assert!(!c.has_session());
    assert_eq!(v.device(), Some(dev_token(UID, 101)));
}

#[test]
fn reset_for_an_unfinished_account_asks_for_the_profile() {
    let (c, s, _) = native();
    s.with(|f| {
        f.script.push_back((
            "/auth/reset".into(),
            Ok(ok(400, json!({"error": "invalid_request", "field": "display_name", "reason": "required_plain_text"}))),
        ))
    });
    let e = block_on(c.reset(&Secret::new(link_token()), &pw("brand new password"), None)).unwrap_err();
    assert!(e.needs_profile());
    block_on(c.reset(&Secret::new(link_token()), &pw("brand new password"), Some(&profile()))).unwrap();
    assert_eq!(s.last("/auth/reset").body.unwrap()["school"], "Walkerville Elementary");
}

#[test]
fn reset_errors() {
    let (c, s, _) = native();
    let e = block_on(c.reset(&Secret::new("short"), &pw("brand new password"), None)).unwrap_err();
    assert_eq!(e.code(), "invalid_token");
    let e = block_on(c.reset(&Secret::new(link_token()), &pw("short"), None)).unwrap_err();
    assert!(e.message(Flow::Reset).contains("at least 10 characters"));
    s.with(|f| f.script.push_back(("/auth/reset".into(), Ok(err(409, "in_progress")))));
    assert_eq!(block_on(c.reset(&Secret::new(link_token()), &pw("brand new password"), None)).unwrap_err().code(), "in_progress");
}

// ---- delete, status -------------------------------------------------------------------

#[test]
fn delete_sends_the_password_with_the_bearer_and_forgets_everything() {
    let (c, s, v) = signed_in();
    block_on(c.delete_account(&pw(PASSWORD))).unwrap();
    let d = s.last("/account");
    assert_eq!(d.method, Method::Delete);
    assert_eq!(d.header("Authorization"), Some(format!("Bearer {}", token("at", 1)).as_str()));
    assert_eq!(d.body.unwrap()["password"], PASSWORD);
    assert!(!c.has_session());
    assert_eq!((v.refresh(), v.device()), (None, None));
}

#[test]
fn delete_errors_keep_the_account_and_session() {
    let (c, s, _) = signed_in();
    let e = block_on(c.delete_account(&pw("not my password"))).unwrap_err();
    assert_eq!(e.message(Flow::Delete), "That password is not right. Nothing was deleted.");
    assert!(c.has_session());
    assert_eq!(s.refresh_calls(), 0, "a wrong password is not an expired token");
    s.with(|f| {
        let mut r = err(429, "rate_limited");
        r.retry_after = Some(1200);
        f.script.push_back(("/account".into(), Ok(r)));
    });
    let e = block_on(c.delete_account(&pw(PASSWORD))).unwrap_err();
    assert!(e.message(Flow::Delete).contains("Nothing was deleted"));
}

#[test]
fn me_parses_every_status_and_ignores_anything_else() {
    let (c, s, _) = signed_in();
    let cases = [
        (json!({"state": "pending"}), Status::Pending),
        (json!({"state": "approved"}), Status::Approved),
        (
            json!({"state": "rejected", "reason": "Not on the staff list", "reapply_after": 1_793_000_000_000i64, "can_reapply": false}),
            Status::Rejected { revoked: false, reason: Some("Not on the staff list".into()), reapply_after_ms: Some(1_793_000_000_000), can_reapply: false },
        ),
        (
            json!({"state": "revoked", "reason": null, "reapply_after": 5, "can_reapply": true}),
            Status::Rejected { revoked: true, reason: None, reapply_after_ms: Some(5), can_reapply: true },
        ),
        (json!({"state": "suspended", "reason": "Spam"}), Status::Suspended { reason: Some("Spam".into()) }),
        (json!({"state": "something_new"}), Status::Other("something_new".into())),
    ];
    for (st, want) in cases {
        s.with(|f| f.status = st.clone());
        let me = block_on(c.me()).unwrap();
        assert_eq!(me.status, want);
        assert!(!format!("{me:?}").contains("SECRET REVIEWER NOTE"));
    }
}

#[test]
fn server_text_is_shown_as_one_plain_line() {
    assert_eq!(api::plain("Not\u{202E}evil\u{200B} on\nthe   list\u{7}"), "Notevil on the list");
    let v = json!({"email": EMAIL, "display_name": "A\u{2066}B", "profile": {"school": "S\tchool"},
                   "status": {"state": "rejected", "reason": "line1\r\nline2"}});
    let me = api::Me::parse(&v).unwrap();
    assert_eq!(me.display_name, "AB");
    assert_eq!(me.school, "S chool");
    assert!(matches!(me.status, Status::Rejected { reason: Some(ref r), .. } if r == "line1 line2"));
    assert_eq!(api::date_text(1_793_000_000_000), "2026-10-26");
}

#[test]
fn reapply_posts_a_fresh_profile() {
    let (c, s, _) = signed_in();
    block_on(c.reapply(&profile())).unwrap();
    assert_eq!(s.last("/account/reapply").body.unwrap()["display_name"], "Ms Frizzle");
    s.with(|f| {
        let mut r = ok(429, json!({"error": "cooldown", "reapply_after": 99}));
        r.retry_after = Some(2 * 86400);
        f.script.push_back(("/account/reapply".into(), Ok(r)));
    });
    let e = block_on(c.reapply(&profile())).unwrap_err();
    assert_eq!(e.message(Flow::Reapply), "You can apply again in about 48 hours.");
}

// ---- error text -----------------------------------------------------------------------

const EVERY_CODE: [(u16, &str); 27] = [
    (400, "invalid_token"),
    (400, "weak_password"),
    (401, "invalid_credentials"),
    (401, "invalid_refresh_token"),
    (401, "refresh_token_required"),
    (401, "unauthorized"),
    (403, "account_suspended"),
    (403, "client_ip_required"),
    (403, "origin_required"),
    (403, "origin_not_allowed"),
    (403, "known_device_required"),
    (403, "forbidden"),
    (409, "in_progress"),
    (409, "not_eligible"),
    (429, "rate_limited"),
    (429, "cooldown"),
    (400, "invalid_client"),
    (400, "invalid_json"),
    (400, "invalid_request"),
    (400, "native_client_with_origin"),
    (413, "body_too_large"),
    (415, "unsupported_media_type"),
    (404, "not_found"),
    (405, "method_not_allowed"),
    (500, "server_error"),
    (502, ""),
    (418, "something_new"),
];

const FLOWS: [Flow; 13] = [
    Flow::SignUp,
    Flow::Resend,
    Flow::Confirm,
    Flow::SignIn,
    Flow::Enrol,
    Flow::Refresh,
    Flow::SignOut,
    Flow::SignOutEverywhere,
    Flow::Forgot,
    Flow::Reset,
    Flow::Delete,
    Flow::Status,
    Flow::Reapply,
];

/// Phrases that would turn a uniform server answer into an account-existence hint.
const EXISTENCE_HINTS: [&str; 12] = [
    "no such account",
    "no account",
    "not registered",
    "does not exist",
    "doesn't exist",
    "unknown email",
    "unknown address",
    "already exists",
    "already registered",
    "is taken",
    "not found",
    "isn't registered",
];

#[test]
fn every_error_code_has_text_and_none_hints_at_account_existence() {
    let mut extra: Vec<ApiError> = vec![
        ApiError::NotConfigured,
        ApiError::Offline("Can't reach pool.test: the connection failed.".into()),
        ApiError::SignedOut,
        ApiError::BadReply("x".into()),
    ];
    for (status, code) in EVERY_CODE {
        for field in [None, Some("email"), Some("school"), Some("display_name"), Some("adult_school_employee"), Some("password")] {
            for reason in [None, Some("too_short"), Some("common"), Some("contains_email"), Some("required_plain_text")] {
                extra.push(ApiError::Http {
                    status,
                    code: code.into(),
                    field: field.map(Into::into),
                    reason: reason.map(Into::into),
                    retry_after: Some(120),
                });
            }
        }
    }
    for e in &extra {
        for flow in FLOWS {
            let m = e.message(flow);
            assert!(m.len() > 10, "{e:?} {flow:?}: {m}");
            let lower = m.to_lowercase();
            for hint in EXISTENCE_HINTS {
                assert!(!lower.contains(hint), "{e:?} {flow:?} says {hint:?}: {m}");
            }
        }
    }
}

#[test]
fn error_bodies_never_leak_free_text_into_messages() {
    let r = Response::new(400, r#"{"error":"Your account does not exist <b>","field":"email\n","reason":"nope nope"}"#);
    let e = ApiError::from_response(&r);
    assert_eq!(e, ApiError::Http { status: 400, code: String::new(), field: None, reason: None, retry_after: None });
    assert!(!e.message(Flow::SignIn).contains("exist"));
    let e = ApiError::from_response(&Response::new(500, "<html>oops</html>"));
    assert_eq!(e.message(Flow::Status), "The server had a problem. Try again later.");
}

#[test]
fn wait_text_rounds_sensibly() {
    assert_eq!(api::wait_text(None), "later");
    assert_eq!(api::wait_text(Some(30)), "in about a minute");
    assert_eq!(api::wait_text(Some(600)), "in about 10 minutes");
    assert_eq!(api::wait_text(Some(7200)), "in about 2 hours");
}

// ---- redaction ------------------------------------------------------------------------

struct Capture(Mutex<Vec<String>>);

impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        self.0.lock().unwrap().push(format!("{} {}", r.target(), r.args()));
    }
    fn flush(&self) {}
}

fn capture() -> &'static Capture {
    static CAP: OnceLock<&'static Capture> = OnceLock::new();
    CAP.get_or_init(|| {
        let cap: &'static Capture = Box::leak(Box::new(Capture(Mutex::new(Vec::new()))));
        let _ = log::set_logger(cap);
        log::set_max_level(log::LevelFilter::Trace);
        cap
    })
}

#[test]
fn secrets_never_appear_in_debug_output_or_logs() {
    let cap = capture();
    let (c, s, v) = signed_in();
    s.with(|f| f.access = Some("stale".into()));
    block_on(c.me()).unwrap();
    block_on(c.sign_out(false, false)).unwrap();
    let secrets = [PASSWORD.to_string(), token("at", 2), token("rt", 2), dev_token(UID, 1), token("at", 1), token("rt", 1)];
    let mut dumps = vec![format!("{c:?}"), format!("{:?}", v.inner), format!("{:?}", Secret::new(PASSWORD))];
    let req = Request {
        method: Method::Post,
        url: format!("{BASE}/auth/login"),
        headers: vec![
            ("Authorization".into(), Secret::new(format!("Bearer {}", token("at", 1)))),
            ("X-Device-Token".into(), Secret::new(dev_token(UID, 1))),
            ("X-Client".into(), Secret::new("native")),
        ],
        body: Some(Secret::new(json!({"email": EMAIL, "password": PASSWORD}).to_string())),
        timeout: None,
    };
    dumps.push(format!("{req:?}"));
    dumps.push(format!("{:?}", Response::new(200, &json!({"refresh_token": token("rt", 1)}).to_string())));
    let model_err = ApiError::from_response(&Response::new(401, &json!({"error": "invalid_credentials", "token": token("rt", 1)}).to_string()));
    dumps.push(format!("{model_err:?}"));
    dumps.push(format!("{:?}", api::parse_link(&format!("https://a/account/verify#token={}", link_token())).unwrap()));
    dumps.extend(cap.0.lock().unwrap().iter().cloned());
    for d in &dumps {
        for secret in secrets.iter().chain([link_token()].iter()) {
            assert!(!d.contains(secret.as_str()), "secret leaked in: {d}");
        }
    }
    assert!(format!("{req:?}").contains("X-Client: native"), "non-secret headers stay readable");
}

#[test]
fn secret_take_wipes_the_form_field() {
    let mut field = String::from(PASSWORD);
    let s = Secret::take(&mut field);
    assert!(field.is_empty());
    assert_eq!(s.expose(), PASSWORD);
    let mut forms = super::model::Forms { password: "abc".into(), password2: "abc".into(), paste: "x".into(), ..Default::default() };
    forms.wipe_secrets();
    assert!(forms.password.is_empty() && forms.password2.is_empty() && forms.paste.is_empty());
}

// ---- server address, links ------------------------------------------------------------

#[test]
fn server_address_validation() {
    for (input, want) in [
        ("https://pool.example.org", "https://pool.example.org"),
        ("https://pool.example.org/", "https://pool.example.org"),
        (" https://Pool.Example.ORG/api/ ", "https://pool.example.org/api"),
        ("http://localhost:8787", "http://localhost:8787"),
        ("http://127.0.0.1:18811/", "http://127.0.0.1:18811"),
        ("http://[::1]:8080", "http://[::1]:8080"),
        ("https://203.0.113.9:8443", "https://203.0.113.9:8443"),
    ] {
        assert_eq!(api::validate_base_url(input).as_deref(), Ok(want), "{input}");
    }
    for bad in [
        "",
        "   ",
        "pool.example.org",
        "http://pool.example.org",
        "http://localhost.evil.example",
        "http://127.0.0.2",
        "http://192.168.1.10",
        "ftp://pool.example.org",
        "javascript:alert(1)",
        "https://user:pw@pool.example.org",
        "https://user@pool.example.org",
        "https://pool.example.org/?a=1",
        "https://pool.example.org/#x",
        "https://pool example.org",
        "https://",
        "file:///etc/passwd",
    ] {
        assert!(api::validate_base_url(bad).is_err(), "{bad:?} should be refused");
    }
}

#[test]
fn pasted_link_variants() {
    let t = link_token();
    assert_eq!(t.len(), 43);
    let cases: Vec<(String, Option<LinkKind>)> = vec![
        (format!("https://app.example.org/account/verify#token={t}"), Some(LinkKind::Verify)),
        (format!("<https://app.example.org/account/verify#token={t}>."), Some(LinkKind::Verify)),
        (format!("  \"http://localhost/account/reset#token={t}\"\n"), Some(LinkKind::Reset)),
        (format!("https://app.example.org/account/en\r\nrol#token={}\r\n{}", &t[..20], &t[20..]), Some(LinkKind::Enrol)),
        (format!("https://app.example.org/account/verify#token={}=\r\n{}", &t[..30], &t[30..]), Some(LinkKind::Verify)),
        (format!("https://app.example.org/account/reset#token={t}&utm=x"), Some(LinkKind::Reset)),
        (format!("https://app.example.org/account/reset?token={t}"), Some(LinkKind::Reset)),
        (format!("#token={t}"), None),
        (format!("token={t}"), None),
        (t.clone(), None),
        (format!("\n {t} \n"), None),
    ];
    for (input, kind) in cases {
        let l = api::parse_link(&input).unwrap_or_else(|| panic!("{input:?} not parsed"));
        assert_eq!(l.kind, kind, "{input:?}");
        assert_eq!(l.token.expose(), t, "{input:?}");
    }
    for bad in [
        String::new(),
        t[..42].to_string(),
        format!("{t}x"),
        "https://app.example.org/account/verify".to_string(),
        format!("https://app.example.org/account/verify#token={}", t.replace('-', "+")),
        "token=".to_string(),
    ] {
        assert!(api::parse_link(&bad).is_none(), "{bad:?} should not parse");
    }
    assert_eq!(api::link_from_location("/account/verify", &format!("#token={t}")).unwrap().kind, Some(LinkKind::Verify));
    assert_eq!(api::link_from_location("/app/account/enrol", &format!("#token={t}")).unwrap().kind, Some(LinkKind::Enrol));
    assert!(api::link_from_location("/", &format!("#token={t}")).is_none(), "a token on the home page is not a deep link");
    assert!(api::link_from_location("/account/verify", "").is_none());
}

// ---- the Account screen model ---------------------------------------------------------

struct Harness {
    model: AccountModel,
    /// Stands in for share.json.
    file: Arc<Mutex<super::ShareSettings>>,
    keychain: Keychain,
    server: Server,
    vault: SharedVault,
    built: Arc<AtomicUsize>,
    now: Instant,
}

/// Like the keychain: entries are per server, except the one trusted-server entry.
#[derive(Clone, Default)]
struct Keychain {
    per_server: Arc<Mutex<std::collections::HashMap<String, SharedVault>>>,
    global: SharedVault,
}

impl Keychain {
    fn for_server(&self, base: &str) -> SharedVault {
        self.per_server.lock().unwrap().entry(base.to_string()).or_insert_with(|| {
            // The pool.test vault shares the failure switches the tests flip.
            SharedVault { deletes_fail: self.global.deletes_fail.clone(), refresh_writes_fail: self.global.refresh_writes_fail.clone(), ..Default::default() }
        }).clone()
    }
}

struct ScopedVault {
    server: SharedVault,
    global: SharedVault,
}

impl Vault for ScopedVault {
    fn load(&self, slot: &Slot) -> Result<Option<Secret>, VaultError> {
        if *slot == Slot::TrustedServer { self.global.load(slot) } else { self.server.load(slot) }
    }
    fn save(&self, slot: &Slot, value: &Secret) -> Result<(), VaultError> {
        if *slot == Slot::TrustedServer { self.global.save(slot, value) } else { self.server.save(slot, value) }
    }
    fn clear(&self, slot: &Slot) -> Result<(), VaultError> {
        if *slot == Slot::TrustedServer { self.global.clear(slot) } else { self.server.clear(slot) }
    }
    fn persistent(&self) -> bool {
        true
    }
}

fn keychain_factory(server: Server, keychain: Keychain, mode: Mode, built: Arc<AtomicUsize>) -> super::model::Factory {
    Arc::new(move |base: &str| {
        built.fetch_add(1, Ordering::SeqCst);
        let v = ScopedVault { server: keychain.for_server(base), global: keychain.global.clone() };
        (Client::new(base, mode, Box::new(MockTransport { server: server.clone(), mode }), Box::new(v)), None)
    })
}

fn harness(mode: Mode) -> Harness {
    let (server, keychain, built) = (Server::default(), Keychain::default(), Arc::new(AtomicUsize::new(0)));
    let vault = keychain.for_server(BASE);
    let factory = keychain_factory(server.clone(), keychain.clone(), mode, built.clone());
    let file = Arc::new(Mutex::new(super::ShareSettings::default()));
    Harness {
        model: AccountModel::new(factory, Runner::Inline, SettingsIo::Memory(file.clone())),
        server,
        vault,
        keychain,
        built,
        file,
        now: Instant::now(),
    }
}

impl Harness {
    fn configure(&mut self) {
        self.model.open();
        self.frame();
        assert_eq!(self.model.page, Page::NotConfigured);
        self.model.forms.server = "https://pool.test/".into();
        self.model.save_server();
        self.frame();
        assert_eq!(self.model.base_url(), "https://pool.test");
        assert!(!self.model.trusted, "a new address is never trusted until confirmed");
        self.model.trust_server();
        assert!(self.model.trusted);
    }
    fn frame(&mut self) {
        self.model.frame(self.now);
    }
    fn sign_in(&mut self) {
        self.model.forms.email = EMAIL.into();
        self.model.forms.password = PASSWORD.into();
        self.model.submit_sign_in();
        self.frame();
    }
    fn notice(&self) -> String {
        match &self.model.notice {
            Some(Notice::Info(s)) | Some(Notice::Error(s)) => s.clone(),
            None => String::new(),
        }
    }
}

#[test]
fn a_closed_account_screen_does_nothing_at_all() {
    let mut h = harness(Mode::Native);
    // A saved session exists, the app runs for a long time, the screen is never opened.
    h.vault.seed_session(&token("rt", 0));
    for _ in 0..500 {
        h.now += Duration::from_secs(60);
        h.frame();
    }
    assert_eq!(h.server.count(), 0, "no request");
    assert_eq!(h.built.load(Ordering::SeqCst), 0, "no client (so no keychain read either)");
    assert!(!h.model.open);
}

#[test]
fn not_configured_shows_setup_and_sends_nothing() {
    let mut h = harness(Mode::Native);
    h.model.open();
    h.frame();
    assert_eq!(h.model.page, Page::NotConfigured);
    h.model.forms.server = "http://pool.example.org".into();
    h.model.save_server();
    h.frame();
    assert!(matches!(h.model.notice, Some(Notice::Error(_))));
    assert_eq!(h.model.page, Page::NotConfigured);
    // Pressing a form button anyway does not reach anything.
    h.model.forms.email = EMAIL.into();
    h.model.submit_forgot();
    assert_eq!(h.server.count(), 0);
    assert_eq!(h.built.load(Ordering::SeqCst), 0);
}

#[test]
fn opening_without_a_session_shows_sign_in_and_sends_nothing() {
    let mut h = harness(Mode::Native);
    h.configure();
    assert_eq!(h.model.page, Page::SignIn);
    assert_eq!(h.server.count(), 0);
}

#[test]
fn sign_in_then_poll_only_while_open_with_backoff() {
    let mut h = harness(Mode::Native);
    h.configure();
    h.sign_in();
    assert_eq!(h.model.page, Page::Status);
    assert_eq!(h.model.me.as_ref().unwrap().status, Status::Pending);
    assert!(h.model.forms.password.is_empty(), "the password field is wiped after success");
    let after_login = h.server.count();
    // Not due yet.
    h.now += Duration::from_secs(30);
    h.frame();
    assert_eq!(h.server.count(), after_login);
    // Due: one /me.
    h.now += Duration::from_secs(31);
    h.frame(); // spawns and (inline) completes
    h.frame(); // applies
    assert_eq!(h.server.count(), after_login + 1);
    // Closed: nothing, however long.
    h.model.close();
    for _ in 0..100 {
        h.now += Duration::from_secs(600);
        h.frame();
    }
    assert_eq!(h.server.count(), after_login + 1);
    // Errors back off.
    h.model.open = true;
    h.model.poller.after_failure(h.now);
    let first = h.model.poller.next_at().unwrap() - h.now;
    h.model.poller.after_failure(h.now);
    let second = h.model.poller.next_at().unwrap() - h.now;
    assert!(second > first);
}

#[test]
fn poller_schedule() {
    let now = Instant::now();
    let mut p = Poller::default();
    assert!(!p.due(now));
    p.after_success(now, &Status::Pending);
    assert!(!p.due(now + Duration::from_secs(59)) && p.due(now + Duration::from_secs(60)));
    p.after_success(now, &Status::Approved);
    assert!(!p.due(now + Duration::from_secs(100_000)), "a settled state is not polled");
    let mut delays = Vec::new();
    for _ in 0..8 {
        p.after_failure(now);
        delays.push((p.next_at().unwrap() - now).as_secs());
    }
    assert_eq!(delays, vec![120, 240, 480, 900, 900, 900, 900, 900]);
    p.stop();
    assert!(!p.due(now + Duration::from_secs(100_000)));
}

#[test]
fn reopening_with_a_saved_session_checks_it_once() {
    let mut h = harness(Mode::Native);
    h.configure();
    h.sign_in();
    h.model.close();
    let n = h.server.count();
    h.model.open();
    h.frame();
    h.frame();
    assert_eq!(h.server.count(), n + 1, "one /me on open");
    assert_eq!(h.model.page, Page::Status);
}

#[test]
fn model_lockdown_offers_enrolment_and_enrolment_signs_in() {
    let mut h = harness(Mode::Native);
    h.configure();
    h.server.with(|f| f.lockdown = true);
    h.sign_in();
    assert!(h.model.lockdown);
    assert_eq!(h.model.page, Page::SignIn);
    assert!(h.notice().contains("paused"));
    h.model.go(Page::Enrol);
    assert!(h.model.lockdown, "still offered on the enrol page");
    h.model.forms.paste = format!("https://app.example.org/account/enrol#token={}", token("code", 1));
    h.model.forms.password = PASSWORD.into();
    h.model.submit_enrol();
    h.frame();
    assert_eq!(h.model.page, Page::Status, "{}", h.notice());
    assert!(h.server.paths().ends_with(&["/auth/device/enrol".into(), "/auth/login".into(), "/me".into()]));
}

#[test]
fn model_sign_up_check_email_paste_confirm_then_sign_in() {
    let mut h = harness(Mode::Native);
    h.configure();
    h.model.go(Page::SignUp);
    h.model.forms.email = EMAIL.into();
    h.model.forms.password = "short".into();
    h.model.submit_sign_up();
    assert!(h.notice().contains("10 characters"));
    assert_eq!(h.server.count(), 0, "checked before sending");
    h.model.forms.password = PASSWORD.into();
    h.model.forms.display_name = "Ms Frizzle".into();
    h.model.forms.school = "Walkerville".into();
    h.model.submit_sign_up();
    assert!(h.notice().contains("Enter") || h.notice().contains("Confirm"), "adult box unticked: {}", h.notice());
    h.model.forms.adult = true;
    h.model.submit_sign_up();
    h.frame();
    assert_eq!(h.model.page, Page::CheckEmail(MailKind::Verify));
    let sent_text = h.notice();
    assert!(sent_text.starts_with("If that address can receive mail"));
    // A bare code pasted on the check-email page opens the confirm page.
    h.model.forms.paste = link_token();
    h.model.submit_paste();
    assert_eq!(h.model.page, Page::Confirm);
    h.model.forms.password = "a fresh password!".into();
    h.model.forms.password2 = "a fresh password?".into();
    h.model.forms.display_name = "Ms Frizzle".into();
    h.model.forms.school = "Walkerville".into();
    h.model.forms.adult = true;
    h.model.submit_confirm();
    assert!(h.notice().contains("not the same"));
    h.model.forms.password2 = "a fresh password!".into();
    h.model.submit_confirm();
    h.frame();
    assert_eq!(h.model.page, Page::SignIn);
    assert!(h.notice().contains("confirmed"));
    h.model.forms.email = EMAIL.into();
    h.model.forms.password = "a fresh password!".into();
    h.model.submit_sign_in();
    h.frame();
    assert_eq!(h.model.page, Page::Status);
}

#[test]
fn model_success_texts_are_the_same_for_any_address() {
    let mut texts = Vec::new();
    for email in [EMAIL, "nobody@nowhere.example"] {
        let mut h = harness(Mode::Native);
        h.configure();
        h.model.go(Page::Forgot);
        h.model.forms.email = email.into();
        h.model.submit_forgot();
        h.frame();
        assert_eq!(h.model.page, Page::CheckEmail(MailKind::Reset));
        texts.push(h.notice());
    }
    assert_eq!(texts[0], texts[1]);
}

#[test]
fn model_reset_link_asks_for_the_profile_when_the_server_needs_it() {
    let mut h = harness(Mode::Native);
    h.configure();
    h.model.open_link(api::parse_link(&format!("https://app.pool.test/account/reset#token={}", link_token())).unwrap());
    h.frame();
    assert_eq!(h.model.page, Page::Reset);
    h.server.with(|f| {
        f.script.push_back((
            "/auth/reset".into(),
            Ok(ok(400, json!({"error": "invalid_request", "field": "school", "reason": "required_plain_text"}))),
        ))
    });
    h.model.forms.password = "brand new password".into();
    h.model.forms.password2 = "brand new password".into();
    h.model.submit_reset();
    h.frame();
    assert!(h.model.reset_needs_profile);
    assert!(h.notice().contains("never finished"));
    assert_eq!(h.model.forms.password, "brand new password", "kept for the second try");
    h.model.forms.display_name = "Ms Frizzle".into();
    h.model.forms.school = "Walkerville".into();
    h.model.forms.adult = true;
    h.model.submit_reset();
    h.frame();
    assert_eq!(h.model.page, Page::SignIn);
    assert!(h.model.forms.password.is_empty());
    assert_eq!(h.server.last("/auth/reset").body.unwrap()["school"], "Walkerville");
}

#[test]
fn a_deep_link_before_setup_waits_for_the_server_address() {
    let mut h = harness(Mode::Web);
    h.model.open_link(api::link_from_location("/account/verify", &format!("#token={}", link_token())).unwrap());
    h.frame();
    assert_eq!(h.model.page, Page::NotConfigured);
    h.model.forms.server = "https://pool.test".into();
    h.model.save_server();
    h.frame();
    assert_eq!(h.model.page, Page::Confirm);
    assert!(h.model.has_link_token());
    assert_eq!(h.server.count(), 0, "following a link sends nothing until the form is submitted");
}

#[test]
fn model_sign_out_everywhere_and_delete() {
    let mut h = harness(Mode::Native);
    h.configure();
    h.sign_in();
    h.model.submit_sign_out(true);
    h.frame();
    assert_eq!(h.model.page, Page::SignIn);
    assert!(h.notice().contains("every device"));
    h.sign_in();
    h.model.go(Page::Delete);
    h.model.forms.password = PASSWORD.into();
    h.model.submit_delete();
    assert!(h.notice().contains("Tick the box"));
    h.model.forms.delete_ack = true;
    h.model.submit_delete();
    h.frame();
    assert_eq!(h.model.page, Page::SignIn);
    assert!(h.notice().contains("deleted"));
    assert_eq!(h.vault.device(), None);
}

#[test]
fn model_offline_keeps_the_session_and_the_app_usable() {
    let mut h = harness(Mode::Native);
    h.configure();
    h.sign_in();
    h.model.close();
    h.server.with(|f| {
        f.access = Some("stale".into());
        f.script.push_back(("/auth/refresh".into(), Err(NetError::offline("Can't reach pool.test: the connection failed"))));
        f.script.push_back(("/auth/refresh".into(), Err(NetError::offline("Can't reach pool.test: the connection failed"))));
    });
    h.model.open();
    h.frame();
    h.frame();
    assert_eq!(h.model.page, Page::Status);
    assert!(h.notice().contains("unaffected"), "{}", h.notice());
    assert!(h.model.client().unwrap().has_session());
}

#[test]
fn changing_the_server_forgets_the_old_session() {
    let mut h = harness(Mode::Native);
    h.configure();
    h.sign_in();
    h.model.edit_server();
    h.model.forms.server = "https://other.test".into();
    h.model.save_server();
    h.frame();
    assert_eq!(h.model.base_url(), "https://other.test");
    assert_eq!(h.vault.refresh(), None);
    assert_eq!(h.model.page, Page::SignIn);
}

// ---- the real native transport, on a loopback socket ----------------------------------

#[test]
fn native_transport_sends_x_client_no_origin_and_reads_retry_after() {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for _ in 0..2 {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut len = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                head.push_str(&line);
            }
            let mut body = vec![0; len];
            reader.read_exact(&mut body).unwrap();
            seen.push((head, String::from_utf8(body).unwrap()));
            let reply = r#"{"error":"rate_limited"}"#;
            let mut s = stream;
            write!(
                s,
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 42\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
        }
        seen
    });
    let t = super::transport::NativeTransport::default();
    let base = format!("http://127.0.0.1:{port}");
    let req = Request {
        method: Method::Post,
        url: format!("{base}/auth/login"),
        headers: vec![("X-Device-Token".into(), Secret::new("dev-token")), ("Origin".into(), Secret::new("https://evil"))],
        body: Some(Secret::new(r#"{"email":"a@b.c","password":"p"}"#)),
        timeout: None,
    };
    let r = block_on(t.send(req)).unwrap();
    assert_eq!((r.status, r.retry_after), (429, Some(42)));
    assert_eq!(ApiError::from_response(&r).code(), "rate_limited");
    let del = Request { method: Method::Delete, url: format!("{base}/account"), headers: vec![], body: Some(Secret::new(r#"{"password":"p"}"#)), timeout: None };
    block_on(t.send(del)).unwrap();
    let seen = server.join().unwrap();
    let (head, body) = &seen[0];
    let lower = head.to_ascii_lowercase();
    assert!(lower.contains("x-client: native"), "{head}");
    assert!(!lower.contains("origin:"), "{head}");
    assert!(lower.contains("x-device-token: dev-token"));
    assert!(lower.contains("content-type: application/json"));
    assert_eq!(body, r#"{"email":"a@b.c","password":"p"}"#);
    assert!(seen[1].0.starts_with("DELETE /account"));
    assert_eq!(seen[1].1, r#"{"password":"p"}"#, "DELETE carries its JSON body");
}

#[test]
fn native_transport_reports_unreachable_as_offline() {
    // A port nothing listens on (bound then dropped).
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let t = super::transport::NativeTransport::default();
    let req = Request { method: Method::Get, url: format!("http://127.0.0.1:{port}/me"), headers: vec![], body: None, timeout: None };
    let e = block_on(t.send(req)).unwrap_err();
    assert!(e.text.starts_with("Can't reach 127.0.0.1"), "{}", e.text);
    assert_eq!(e.kind, super::transport::NetKind::Offline);
}

#[test]
fn native_transport_treats_an_empty_gzip_204_as_success() {
    // What the local Worker sends for a logout: 204, labelled gzip, no body.
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" || line.is_empty() {
                break;
            }
        }
        let mut s = stream;
        write!(s, "HTTP/1.1 204 No Content\r\nContent-Encoding: gzip\r\nConnection: close\r\n\r\n").unwrap();
    });
    let t = super::transport::NativeTransport::default();
    let req = Request { method: Method::Get, url: format!("http://127.0.0.1:{port}/x"), headers: vec![], body: None, timeout: None };
    let r = block_on(t.send(req)).unwrap();
    assert_eq!(r.status, 204);
    server.join().unwrap();
}

// ==== fix round 2 ========================================================================

fn trusted_harness() -> Harness {
    let mut h = harness(Mode::Native);
    h.configure();
    h
}

fn file_base(h: &Harness, base: &str) {
    h.file.lock().unwrap().base_url = base.into();
}

// ---- 1. the server address -------------------------------------------------------------

#[test]
fn a_built_in_server_ignores_share_json_and_cannot_be_changed() {
    let mut h = harness(Mode::Native);
    h.model = std::mem::replace(&mut h.model, AccountModel::new(Arc::new(|_| unreachable!()), Runner::Inline, SettingsIo::memory()))
        .with_built_in(Some("https://pool.test".into()));
    file_base(&h, "https://evil.example");
    h.model.open();
    h.frame();
    assert_eq!(h.model.base_url(), "https://pool.test");
    assert!(h.model.server_locked());
    assert!(h.model.trusted, "the build's own server needs no confirmation");
    assert_eq!(h.model.page, Page::SignIn);
    h.model.edit_server();
    assert_eq!(h.model.page, Page::SignIn, "no way to the address form");
    h.model.forms.server = "https://evil.example".into();
    h.model.save_server();
    h.frame();
    assert_eq!(h.model.base_url(), "https://pool.test");
    assert_eq!(h.file.lock().unwrap().base_url, "https://evil.example", "and nothing was written either");
    h.sign_in();
    assert!(h.server.log().iter().all(|l| l.path.starts_with('/')), "requests went to the built-in server");
    assert_eq!(h.model.page, Page::Status);
}

#[test]
fn an_edited_share_json_needs_confirming_before_any_password_is_sent() {
    let mut h = trusted_harness();
    h.sign_in();
    h.model.close();
    let before = h.server.count();
    // Someone at the keyboard (or a script) points the file at their own server.
    file_base(&h, "https://evil.example");
    h.model.open();
    h.frame();
    assert_eq!(h.model.base_url(), "https://evil.example");
    assert!(!h.model.trusted);
    h.model.go(Page::SignIn);
    h.model.forms.email = EMAIL.into();
    h.model.forms.password = PASSWORD.into();
    h.model.submit_sign_in();
    h.frame();
    assert_eq!(h.server.count(), before, "the password went nowhere");
    assert!(h.notice().contains("Confirm the server first") && h.notice().contains("https://evil.example"), "{}", h.notice());
    assert_eq!(h.vault.refresh(), Some(token("rt", 1)), "the real server's saved session is untouched");
}

#[test]
fn a_typed_new_server_resets_trust_and_every_password_page_waits_for_it() {
    let mut h = trusted_harness();
    h.model.edit_server();
    h.model.forms.server = "https://other.example".into();
    h.model.save_server();
    h.frame();
    assert!(!h.model.trusted);
    let n = h.server.count();
    h.model.forms.email = EMAIL.into();
    h.model.forms.password = PASSWORD.into();
    h.model.forms.password2 = PASSWORD.into();
    h.model.forms.display_name = "x".into();
    h.model.forms.school = "y".into();
    h.model.forms.adult = true;
    h.model.forms.delete_ack = true;
    h.model.forms.paste = token("code", 1);
    h.model.submit_sign_in();
    h.model.submit_sign_up();
    h.model.open_link(api::parse_link(&link_token()).unwrap());
    h.frame();
    h.model.submit_confirm();
    h.model.submit_reset();
    h.model.submit_enrol();
    h.model.submit_delete();
    h.frame();
    assert_eq!(h.server.count(), n, "no password page sent anything to an unconfirmed server");
    // Confirming moves the trust: the old address is no longer trusted.
    h.model.trust_server();
    assert!(h.model.trusted);
    h.model.edit_server();
    h.model.forms.server = "https://pool.test".into();
    h.model.save_server();
    h.frame();
    assert!(!h.model.trusted, "switching back needs confirming again");
    // Away and straight back, never confirming the other address: still reset.
    h.model.trust_server();
    for base in ["https://elsewhere.example", "https://pool.test"] {
        h.model.edit_server();
        h.model.forms.server = base.into();
        h.model.save_server();
        h.frame();
    }
    assert!(!h.model.trusted, "any change of address resets trust");
}

#[test]
fn a_link_from_a_foreign_host_is_refused_and_never_changes_the_server() {
    let mut h = trusted_harness();
    let t = link_token();
    h.model.open_link(api::parse_link(&format!("https://evil.example/account/reset#token={t}")).unwrap());
    h.frame();
    assert_eq!(h.model.page, Page::SignIn);
    assert!(!h.model.has_link_token());
    assert!(h.notice().contains("was not used"), "{}", h.notice());
    assert_eq!(h.model.base_url(), "https://pool.test");
    // The app's own site (same parent domain) and a bare code are fine.
    h.model.open_link(api::parse_link(&format!("https://app.pool.test/account/reset#token={t}")).unwrap());
    h.frame();
    assert_eq!(h.model.page, Page::Reset);
    h.model.go(Page::CheckEmail(MailKind::Verify));
    h.model.open_link(api::parse_link(&t).unwrap());
    h.frame();
    assert_eq!(h.model.page, Page::Confirm);
}

#[test]
fn a_foreign_deep_link_before_setup_is_refused_once_the_server_is_known() {
    let mut h = harness(Mode::Web);
    let mut link = api::link_from_location("/account/verify", &format!("#token={}", link_token())).unwrap();
    link.host = Some("evil.example".into());
    h.model.open_link(link);
    h.frame();
    h.model.forms.server = "https://pool.test".into();
    h.model.save_server();
    h.frame();
    assert_ne!(h.model.page, Page::Confirm);
    assert!(!h.model.has_link_token());
}

#[test]
fn link_hosts_match_only_the_servers_own_site() {
    for (l, a) in [
        ("pool.test", "pool.test"),
        ("app.example.org", "pool.example.org"),
        ("example.org", "api.example.org"),
        ("app.school.k12.ca.us", "pool.school.k12.ca.us"),
        ("localhost", "127.0.0.1"),
    ] {
        assert!(api::link_host_matches(l, a), "{l} vs {a}");
    }
    for (l, a) in [
        ("evil.example", "pool.test"),
        ("app.evil.co.uk", "pool.school.co.uk"),
        ("example.org.evil.net", "pool.example.org"),
        ("a.com", "b.com"),
        ("localhost", "pool.test"),
    ] {
        assert!(!api::link_host_matches(l, a), "{l} vs {a}");
    }
}

#[test]
fn an_international_server_name_shows_punycode_and_unicode() {
    let base = api::validate_base_url("https://аррӏе.com/pool").unwrap();
    assert_eq!(base, "https://xn--80ak6aa92e.com/pool", "stored and sent in ASCII");
    let label = api::server_label(&base);
    assert!(label.starts_with("https://xn--80ak6aa92e.com/pool"), "{label}");
    assert!(label.contains("аррӏе.com"), "{label}");
    assert_eq!(api::server_label("https://pool.example.org"), "https://pool.example.org");
    assert_eq!(api::server_label("http://127.0.0.1:18811"), "http://127.0.0.1:18811");
}

// ---- 2. sign-out when the keychain will not delete -------------------------------------

#[test]
fn a_failed_keychain_delete_is_reported_blocks_the_token_and_is_retried() {
    let (c, s, v) = signed_in();
    v.deletes_fail.store(true, Ordering::SeqCst);
    assert_eq!(block_on(c.sign_out(false, false)), Err(ApiError::LocalCleanup));
    assert!(!c.has_session());
    assert_eq!(c.pending_purge(), vec![UID.to_string()]);
    assert!(v.refresh().is_some(), "the keychain still holds it");
    // Next run, keychain still locked: the saved token is blocked, nothing is sent.
    let next = client(Mode::Native, &s, &v);
    let n = s.count();
    assert_eq!(next.purge(&[UID.to_string()]), vec![UID.to_string()]);
    assert!(!next.has_session());
    assert_eq!(block_on(next.restore()), Err(ApiError::SignedOut));
    assert_eq!(s.count(), n);
    // The keychain opens up: the retry deletes it.
    v.deletes_fail.store(false, Ordering::SeqCst);
    assert!(next.retry_purge().is_empty());
    assert_eq!(v.refresh(), None);
    assert!(!ApiError::LocalCleanup.message(Flow::SignOut).to_lowercase().starts_with("signed out"));
}

#[test]
fn the_model_never_says_signed_out_until_the_delete_worked_and_remembers_it_across_runs() {
    let mut h = trusted_harness();
    h.sign_in();
    h.vault.deletes_fail.store(true, Ordering::SeqCst);
    h.model.submit_sign_out(false);
    h.frame();
    assert!(matches!(h.model.notice, Some(Notice::Error(_))), "{:?}", h.model.notice);
    assert!(!h.notice().contains("Signed out."));
    assert_eq!(
        h.file.lock().unwrap().purge,
        vec![super::PurgeEntry { server: BASE.into(), account: UID.into() }],
        "recorded for the next run, with its server"
    );
    // "Next run": a fresh model on the same file and keychain.
    let file = h.file.clone();
    let factory = keychain_factory(h.server.clone(), h.keychain.clone(), Mode::Native, Arc::new(AtomicUsize::new(0)));
    let mut next = AccountModel::new(factory, Runner::Inline, SettingsIo::Memory(file.clone()));
    let n = h.server.count();
    next.open();
    next.frame(h.now);
    assert_eq!(h.server.count(), n, "the blocked token is not used");
    assert_eq!(next.page, Page::SignIn);
    h.vault.deletes_fail.store(false, Ordering::SeqCst);
    next.frame(h.now + Duration::from_secs(3));
    assert!(file.lock().unwrap().purge.is_empty());
    assert_eq!(h.vault.refresh(), None);
}

#[test]
fn a_rotated_token_that_cannot_be_saved_leaves_no_old_token_behind() {
    let (c, s, v) = signed_in();
    v.refresh_writes_fail.store(true, Ordering::SeqCst);
    s.with(|f| f.access = Some("stale".into()));
    block_on(c.me()).unwrap();
    assert_eq!(v.refresh(), None, "rt1 would be replayed next run and revoke the family");
    assert!(c.has_session(), "this run carries on in memory");
    assert!(c.pending_purge().is_empty());
    // If even the delete fails, the account is blocked for the next run instead.
    v.deletes_fail.store(true, Ordering::SeqCst);
    s.with(|f| f.access = Some("stale again".into()));
    block_on(c.me()).unwrap();
    assert_eq!(c.pending_purge(), vec![UID.to_string()]);
    assert!(c.has_session(), "the in-memory token is the live one; only the saved copy is stale");
    // Once a rotated token is saved again, the slot is good and must not be deleted.
    v.refresh_writes_fail.store(false, Ordering::SeqCst);
    s.with(|f| f.access = Some("stale a third time".into()));
    block_on(c.me()).unwrap();
    assert!(c.pending_purge().is_empty());
    v.deletes_fail.store(false, Ordering::SeqCst);
    c.retry_purge();
    assert_eq!(v.refresh(), Some(token("rt", 4)));
}

#[test]
fn a_pending_purge_blocks_a_saved_session_even_if_the_index_still_points_at_it() {
    // The keychain refused the delete AND the index update: the next run must still not use it.
    let (s, v) = (Server::default(), SharedVault::default());
    v.seed_session(&token("rt", 0));
    v.deletes_fail.store(true, Ordering::SeqCst);
    let c = client(Mode::Native, &s, &v);
    assert!(c.has_session());
    assert_eq!(c.purge(&[UID.to_string()]), vec![UID.to_string()]);
    assert!(!c.has_session());
    assert_eq!(block_on(c.restore()), Err(ApiError::SignedOut));
    assert_eq!(s.count(), 0);
}

// ---- 3. accounts never overwrite each other ---------------------------------------------

#[test]
fn a_reset_link_for_another_account_keeps_this_session_and_device() {
    let (c, s, v) = signed_in();
    let teacher_device = v.device();
    s.with(|f| f.link_account = Some(UID2.into()));
    let r = block_on(c.reset(&Secret::new(link_token()), &pw("student password 1"), None)).unwrap();
    assert!(r.other_account);
    assert!(c.has_session(), "the teacher's session is not dropped");
    assert_eq!(v.device(), teacher_device, "nor their device token replaced");
    assert!(v.get(Slot::Device(UID2.into())).is_some(), "the other account's token is kept apart");
    assert!(block_on(c.me()).is_ok());
    // The same for a confirmation link.
    let r = block_on(c.confirm(&Secret::new(link_token()), &pw("student password 1"), &profile())).unwrap();
    assert!(r.other_account && c.has_session());
    assert_eq!(v.device(), teacher_device);
}

#[test]
fn a_reset_link_for_this_account_ends_the_local_session() {
    let (c, _s, v) = signed_in();
    let r = block_on(c.reset(&Secret::new(link_token()), &pw("brand new password"), None)).unwrap();
    assert!(!r.other_account);
    assert!(!c.has_session());
    assert_eq!(v.device(), Some(dev_token(UID, 101)));
}

#[test]
fn a_device_token_for_another_account_is_never_stored_as_this_ones() {
    let (c, s, v) = native();
    s.with(|f| {
        let mut body = f.issue();
        body["device_token"] = json!(dev_token(UID2, 5));
        f.script.push_back(("/auth/login".into(), Ok(ok(200, body))));
    });
    block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap();
    assert_eq!(v.device(), None);
    assert_eq!(v.get(Slot::Device(UID2.into())), None);
}

#[test]
fn signing_in_as_another_account_ends_the_first_ones_local_session() {
    let (c, s, v) = signed_in();
    s.with(|f| {
        let mut body = f.issue();
        body["user"]["id"] = json!(UID2);
        f.script.push_back(("/auth/login".into(), Ok(ok(200, body))));
    });
    block_on(c.sign_in("other@school.example.org", &pw(PASSWORD))).unwrap();
    assert_eq!(c.account().as_deref(), Some(UID2));
    assert_eq!(v.refresh(), None);
    assert!(v.get(Slot::Refresh(UID2.into())).is_some());
    assert!(v.device().is_some(), "the first account's device token stays");
}

#[test]
fn an_enrolment_code_for_another_account_is_reported_and_changes_nothing_here() {
    let mut h = trusted_harness();
    h.sign_in();
    let teacher_device = h.vault.device();
    h.server.with(|f| f.enrol_account = Some(UID2.into()));
    h.model.go(Page::Enrol);
    h.model.forms.paste = token("code", 1);
    h.model.forms.email = EMAIL.into();
    h.model.forms.password = PASSWORD.into();
    h.model.submit_enrol();
    h.frame();
    assert!(h.notice().contains("different account"), "{}", h.notice());
    assert_eq!(h.vault.device().map(|d| d != dev_token(UID2, 900)), Some(true));
    assert_eq!(teacher_device.is_some(), h.vault.device().is_some());
}

// ---- 4. two copies of the app ---------------------------------------------------------

fn lock_path() -> std::path::PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!("vershion-share-test-{}-{}.lock", std::process::id(), N.fetch_add(1, Ordering::SeqCst)))
}

#[test]
fn a_second_process_re_reads_the_rotated_token_instead_of_replaying_it() {
    let (s, v) = (Server::default(), SharedVault::default());
    v.seed_session(&token("rt", 0));
    s.with(|f| f.refresh = Some(token("rt", 0)));
    let path = lock_path();
    let p1 = client(Mode::Native, &s, &v).with_lock(Box::new(super::vault::FileLock::new(path.clone())));
    let p2 = client(Mode::Native, &s, &v).with_lock(Box::new(super::vault::FileLock::new(path.clone())));
    block_on(p1.restore()).unwrap(); // rt0 -> rt1
    // Much later, the other process (still holding rt0 in memory) needs a token.
    block_on(p2.restore()).unwrap();
    let f = s.0.lock().unwrap();
    assert_eq!(f.presented, vec![token("rt", 0), token("rt", 1)]);
    assert_eq!(f.replays, 0, "rt0 was never presented twice");
    let _ = std::fs::remove_file(path);
}

#[test]
fn two_processes_refreshing_together_never_present_one_token_twice() {
    let (s, v) = (Server::default(), SharedVault::default());
    v.seed_session(&token("rt", 0));
    s.with(|f| {
        f.refresh = Some(token("rt", 0));
        f.refresh_delay = Duration::from_millis(150);
    });
    let path = lock_path();
    let clients: Vec<Arc<Client>> = (0..2)
        .map(|_| Arc::new(client(Mode::Native, &s, &v).with_lock(Box::new(super::vault::FileLock::new(path.clone())))))
        .collect();
    let handles: Vec<_> = clients.iter().map(|c| {
        let c = c.clone();
        std::thread::spawn(move || block_on(c.restore()))
    }).collect();
    for h in handles {
        assert_eq!(h.join().unwrap().unwrap().email, EMAIL);
    }
    let f = s.0.lock().unwrap();
    assert_eq!(f.replays, 0, "presented: {:?}", f.presented);
    // (The fake accepts only the newest access token, so a process may refresh a second time
    // with the token it re-read; the property is that no token is ever presented twice.)
    let distinct: std::collections::BTreeSet<_> = f.presented.iter().collect();
    assert_eq!(distinct.len(), f.presented.len(), "presented: {:?}", f.presented);
    assert!(f.presented.len() >= 2);
    let _ = std::fs::remove_file(path);
}

// ---- 5. the refresh time limit ----------------------------------------------------------

#[test]
fn refresh_uses_the_short_time_limit_and_other_routes_do_not() {
    let (c, s, _) = signed_in();
    s.with(|f| f.access = Some("stale".into()));
    block_on(c.me()).unwrap();
    assert_eq!(s.last("/auth/refresh").timeout, Some(super::client::REFRESH_TIMEOUT));
    assert_eq!(s.last("/me").timeout, None);
    assert_eq!(s.last("/auth/login").timeout, None);
}

#[test]
fn a_reply_lost_after_rotation_is_recovered_inside_the_grace_window() {
    let (s, v) = (Server::default(), SharedVault::default());
    v.seed_session(&token("rt", 0));
    let (clock, clock_fn) = test_clock();
    let start = *clock.lock().unwrap();
    let times = Arc::new(Mutex::new(Vec::new()));
    s.with(|f| {
        f.refresh = Some(token("rt", 0));
        f.lose_rotation_reply = true;
        let (clock, times) = (clock.clone(), times.clone());
        // Each refresh attempt costs the full time limit (the lost reply times out).
        f.on_request = Some(Box::new(move |p| {
            if p == "/auth/refresh" {
                let mut t = clock.lock().unwrap();
                times.lock().unwrap().push(*t);
                *t += super::client::REFRESH_TIMEOUT;
            }
        }));
    });
    let c = Client::with_clock(BASE, Mode::Native, Box::new(MockTransport { server: s.clone(), mode: Mode::Native }), Box::new(v.clone()), clock_fn);
    assert_eq!(block_on(c.restore()).unwrap().email, EMAIL);
    let t = times.lock().unwrap();
    assert_eq!(t.len(), 2, "one retry");
    // The server rotated during the first try; the retry arrives well inside its grace.
    assert!(t[1] - start < super::client::SERVER_REUSE_GRACE);
    let f = s.0.lock().unwrap();
    assert_eq!(f.presented, vec![token("rt", 0), token("rt", 0)]);
    assert_eq!(f.replays, 1, "answered from the grace window, not treated as theft");
    assert_eq!(v.refresh(), Some(token("rt", 1)));
}

#[test]
fn the_native_transport_honours_the_short_limit() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (_stream, _) = listener.accept().unwrap();
        std::thread::sleep(Duration::from_secs(8)); // never answers in time
    });
    let t = super::transport::NativeTransport::default();
    let started = std::time::Instant::now();
    let req = Request {
        method: Method::Post,
        url: format!("http://127.0.0.1:{port}/auth/refresh"),
        headers: vec![],
        body: Some(Secret::new("{}")),
        timeout: Some(super::client::REFRESH_TIMEOUT),
    };
    let e = block_on(t.send(req)).unwrap_err();
    let took = started.elapsed();
    assert!(took < super::client::REFRESH_TIMEOUT + Duration::from_secs(2), "took {took:?}");
    assert_eq!(e.kind, super::transport::NetKind::Offline);
}

// ---- 6. TLS failures are not "offline" or "wrong password" -----------------------------

#[test]
fn tls_failures_have_their_own_message() {
    let e: ApiError = NetError::tls("No secure connection to pool.test could be made (invalid peer certificate).").into();
    assert!(matches!(e, ApiError::Tls(_)));
    for flow in FLOWS {
        let m = e.message(flow).to_lowercase();
        assert!(m.contains("certificate") && m.contains("network"), "{m}");
        assert!(!m.contains("password"), "never a password hint: {m}");
    }
}

#[test]
fn the_native_transport_reports_a_broken_handshake_as_tls() {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        std::thread::sleep(Duration::from_millis(300));
    });
    let t = super::transport::NativeTransport::default();
    let req = Request { method: Method::Get, url: format!("https://127.0.0.1:{port}/me"), headers: vec![], body: None, timeout: None };
    let e = block_on(t.send(req)).unwrap_err();
    assert_eq!(e.kind, super::transport::NetKind::Tls, "{}", e.text);
}

// ---- 7. late results after the server changed -------------------------------------------

#[test]
fn the_server_cannot_change_while_a_request_runs() {
    let mut h = trusted_harness();
    h.model.forms.email = EMAIL.into();
    h.model.forms.password = PASSWORD.into();
    h.model.submit_sign_in(); // finished, not yet applied: still busy
    assert!(h.model.busy());
    h.model.forms.server = "https://other.example".into();
    h.model.save_server();
    h.frame(); // what Enter in the address field does
    assert_eq!(h.model.base_url(), "https://pool.test");
    h.frame();
    assert_eq!(h.model.page, Page::Status);
}

#[test]
fn a_result_from_the_old_server_is_dropped_and_writes_nothing() {
    let mut h = trusted_harness();
    h.model.forms.email = EMAIL.into();
    h.model.forms.password = PASSWORD.into();
    h.model.submit_sign_in();
    h.model.close();
    file_base(&h, "https://other.example");
    h.model.open();
    h.frame();
    h.frame();
    assert!(h.model.me.is_none(), "the old server's account is not shown under the new address");
    assert_ne!(h.model.page, Page::Status);
    // A retired client writes nothing more.
    let (c, _s, v) = native();
    c.forget_local();
    block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap();
    assert_eq!(v.refresh(), None);
}

// ---- 8. a panic mid-refresh -----------------------------------------------------------

#[test]
fn a_panic_while_refreshing_does_not_hang_later_callers() {
    let (c, s, _) = signed_in();
    let c = Arc::new(c);
    s.with(|f| f.access = Some("stale".into()));
    s.1.store(true, Ordering::SeqCst);
    let c1 = c.clone();
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| block_on(c1.me())));
    assert!(first.is_err(), "the transport panicked");
    let (tx, rx) = std::sync::mpsc::channel();
    let c2 = c.clone();
    std::thread::spawn(move || {
        let _ = tx.send(block_on(c2.me()));
    });
    let second = rx.recv_timeout(Duration::from_secs(5)).expect("a later caller must not hang");
    assert_eq!(second.unwrap().email, EMAIL);
}

// ---- 9. request bodies ------------------------------------------------------------------

#[test]
fn request_bodies_are_built_without_leftover_copies() {
    let mut v = json!({"password": PASSWORD, "nested": [{"token": "abc"}], "n": 1});
    super::client::wipe(&mut v);
    assert_eq!(v, json!({"password": "", "nested": [{"token": ""}], "n": 1}));
    let b = super::client::json_body(json!({"email": EMAIL, "password": "pässwörd \"x\""}));
    let back: Value = serde_json::from_str(b.expose()).unwrap();
    assert_eq!(back["password"], "pässwörd \"x\"");
}

#[test]
fn the_delete_body_is_serialised_once_and_sent_per_try() {
    let (c, s, _) = signed_in();
    s.with(|f| f.access = Some("stale".into()));
    block_on(c.delete_account(&pw(PASSWORD))).unwrap();
    let deletes: Vec<Logged> = s.log().into_iter().filter(|l| l.path == "/account").collect();
    assert_eq!(deletes.len(), 2, "one 401, one retry");
    assert_eq!(deletes[0].body, deletes[1].body);
}

// ---- 10. the paste box ------------------------------------------------------------------

#[test]
fn the_paste_box_is_masked_by_default_and_cleared_once_used() {
    let mut h = trusted_harness();
    assert!(!h.model.forms.show_paste);
    h.model.go(Page::CheckEmail(MailKind::Verify));
    h.model.forms.show_paste = true;
    h.model.forms.paste = link_token();
    h.model.submit_paste();
    assert!(h.model.forms.paste.is_empty());
    h.model.go(Page::SignIn);
    assert!(!h.model.forms.show_paste, "masked again when the page changes");
}

// ==== fix round 3 ========================================================================

/// Hangs one route until the test releases it (or forever, standing in for a killed app),
/// then answers like the fake would.
struct Hang {
    inner: MockTransport,
    path: &'static str,
    reached: Mutex<std::sync::mpsc::Sender<()>>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}

impl Transport for Hang {
    fn send<'a>(&'a self, req: Request) -> LocalFuture<'a, Result<Response, NetError>> {
        if req.path() == self.path {
            let _ = self.reached.lock().unwrap().send(());
            // Err when the test drops its sender: that is the release.
            let _ = self.release.lock().unwrap().recv();
        }
        self.inner.send(req)
    }
}

struct HangHandle {
    reached: std::sync::mpsc::Receiver<()>,
    release: Option<std::sync::mpsc::Sender<()>>,
}

impl HangHandle {
    fn wait_reached(&self) {
        self.reached.recv_timeout(Duration::from_secs(10)).expect("the request never reached the network");
    }
    fn release(&mut self) {
        self.release.take();
    }
}

fn hang(server: &Server, mode: Mode, path: &'static str) -> (Hang, HangHandle) {
    let (reached_tx, reached_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    (
        Hang { inner: MockTransport { server: server.clone(), mode }, path, reached: Mutex::new(reached_tx), release: Mutex::new(release_rx) },
        HangHandle { reached: reached_rx, release: Some(release_tx) },
    )
}

/// Polls `cond` with frames until true (background jobs), or fails after 10 s.
fn settle(m: &mut AccountModel, cond: impl Fn(&AccountModel) -> bool) {
    let until = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        m.frame(Instant::now());
        if cond(m) {
            return;
        }
        assert!(std::time::Instant::now() < until, "timed out; page {:?}, notice {:?}", m.page, m.notice);
        std::thread::sleep(Duration::from_millis(10));
    }
}

// ---- 1. sign-out: local first ------------------------------------------------------------

#[test]
fn the_saved_session_is_gone_before_sign_out_touches_the_network() {
    let (s, v) = (Server::default(), SharedVault::default());
    let (t, mut h) = hang(&s, Mode::Native, "/auth/logout");
    let c = Arc::new(Client::new(BASE, Mode::Native, Box::new(t), Box::new(v.clone())));
    block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap();
    let c2 = c.clone();
    let job = std::thread::spawn(move || block_on(c2.sign_out(false, false)));
    h.wait_reached();
    // The request hangs: this is the moment the app could be killed.
    assert_eq!(v.refresh(), None, "the refresh token was deleted before the network call");
    assert!(!c.has_session());
    let next_run = client(Mode::Native, &s, &v);
    assert!(!next_run.has_session(), "a restart here is signed out");
    h.release();
    assert!(job.join().unwrap().is_ok());
}

#[test]
fn sign_out_everywhere_keeps_the_token_in_memory_only_while_waiting() {
    let (s, v) = (Server::default(), SharedVault::default());
    let (t, mut h) = hang(&s, Mode::Native, "/auth/logout");
    let c = Arc::new(Client::new(BASE, Mode::Native, Box::new(t), Box::new(v.clone())));
    block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap();
    let c2 = c.clone();
    let job = std::thread::spawn(move || block_on(c2.sign_out(true, false)));
    h.wait_reached();
    assert_eq!((v.refresh(), v.device()), (None, None), "nothing saved while the server is asked");
    assert!(!client(Mode::Native, &s, &v).has_session());
    h.release();
    assert!(job.join().unwrap().is_ok());
    assert_eq!(s.last("/auth/logout").body.unwrap()["scope"], "global");
}

#[test]
fn a_login_or_refresh_answered_after_sign_out_keeps_nothing() {
    // A refresh stuck on the network while the person signs out.
    let (s, v) = (Server::default(), SharedVault::default());
    v.seed_session(&token("rt", 0));
    s.with(|f| f.refresh = Some(token("rt", 0)));
    let (t, mut h) = hang(&s, Mode::Native, "/auth/refresh");
    let c = Arc::new(Client::new(BASE, Mode::Native, Box::new(t), Box::new(v.clone())));
    let c2 = c.clone();
    let job = std::thread::spawn(move || block_on(c2.restore()));
    h.wait_reached();
    let so = c.begin_sign_out(false);
    c.wipe_signed_out(&so, false).unwrap();
    h.release(); // the server rotates and answers with fresh tokens
    assert_eq!(job.join().unwrap(), Err(ApiError::SignedOut));
    assert_eq!(v.refresh(), None, "the late rotation was not saved");
    assert!(!c.has_session());
}

#[test]
fn the_model_signs_out_while_a_request_hangs_and_nothing_comes_back() {
    let (server, keychain) = (Server::default(), Keychain::default());
    let vault = keychain.for_server(BASE);
    vault.seed_session(&token("rt", 0));
    server.with(|f| f.refresh = Some(token("rt", 0)));
    let (t, mut h) = hang(&server, Mode::Native, "/auth/refresh");
    let t = Arc::new(t);
    let (kc, tt) = (keychain.clone(), t.clone());
    struct Shared(Arc<Hang>);
    impl Transport for Shared {
        fn send<'a>(&'a self, req: Request) -> LocalFuture<'a, Result<Response, NetError>> {
            self.0.send(req)
        }
    }
    let factory: super::model::Factory = Arc::new(move |base: &str| {
        let v = ScopedVault { server: kc.for_server(base), global: kc.global.clone() };
        (Client::new(base, Mode::Native, Box::new(Shared(tt.clone())), Box::new(v)), None)
    });
    let file = Arc::new(Mutex::new(super::ShareSettings { base_url: BASE.into(), purge: vec![] }));
    let mut m = AccountModel::new(factory, Runner::Background, SettingsIo::Memory(file));
    m.open();
    settle(&mut m, |m| m.page == Page::Status && m.busy());
    h.wait_reached(); // the session check is stuck in a refresh
    m.submit_sign_out(false);
    settle(&mut m, |m| m.page == Page::SignIn);
    assert_eq!(vault.refresh(), None);
    assert!(!m.busy(), "signing out is not held up by the stuck request");
    h.release();
    std::thread::sleep(Duration::from_millis(100));
    m.frame(Instant::now());
    assert_eq!(vault.refresh(), None, "the stuck refresh, answered late, saved nothing");
    assert!(m.me.is_none());
}

#[test]
fn delete_wipes_locally_only_after_the_server_confirmed() {
    let (s, v) = (Server::default(), SharedVault::default());
    let (t, mut h) = hang(&s, Mode::Native, "/account");
    let c = Arc::new(Client::new(BASE, Mode::Native, Box::new(t), Box::new(v.clone())));
    block_on(c.sign_in(EMAIL, &pw(PASSWORD))).unwrap();
    let c2 = c.clone();
    let job = std::thread::spawn(move || block_on(c2.delete_account(&Secret::new(PASSWORD))));
    h.wait_reached();
    assert!(v.refresh().is_some(), "not deleted yet: a failed deletion must leave the session usable");
    h.release();
    job.join().unwrap().unwrap();
    assert_eq!((v.refresh(), v.device()), (None, None));
}

// ---- 2. the server mode ----------------------------------------------------------------

#[test]
fn the_server_mode_is_decided_by_the_build() {
    use super::model::{ServerMode, server_mode};
    assert_eq!(server_mode(None, false), ServerMode::NotSetUp);
    assert_eq!(server_mode(Some(""), false), ServerMode::NotSetUp);
    assert_eq!(server_mode(None, true), ServerMode::Custom);
    assert_eq!(server_mode(Some("https://API.example.org/"), false), ServerMode::BuiltIn("https://api.example.org".into()));
    assert_eq!(server_mode(Some("https://api.example.org"), true), ServerMode::BuiltIn("https://api.example.org".into()));
    // Never a fallback to typed addresses because the built-in one was bad.
    assert_eq!(server_mode(Some("http://evil.example"), true), ServerMode::NotSetUp);
}

#[cfg(not(any(debug_assertions, feature = "pool-custom-server")))]
#[test]
fn a_release_build_has_no_typed_servers() {
    const { assert!(!super::model::CUSTOM_SERVER_ALLOWED) };
    if super::ShareSettings::built_in().is_none() {
        let mut m = AccountModel::default();
        assert_eq!(*m.server_mode(), super::model::ServerMode::NotSetUp);
        m.open();
        assert_eq!(m.page, Page::NotSetUp);
    }
}

#[cfg(debug_assertions)]
#[test]
fn a_debug_build_allows_typed_servers() {
    const { assert!(super::model::CUSTOM_SERVER_ALLOWED) };
}

#[test]
fn not_set_up_offers_no_password_page_and_sends_nothing() {
    let mut h = harness(Mode::Native);
    h.model = std::mem::replace(&mut h.model, AccountModel::new(Arc::new(|_| unreachable!()), Runner::Inline, SettingsIo::memory()))
        .with_server_mode(super::model::ServerMode::NotSetUp);
    file_base(&h, "https://evil.example");
    h.model.open();
    h.frame();
    assert_eq!(h.model.page, Page::NotSetUp);
    for page in [Page::SignIn, Page::SignUp, Page::Confirm, Page::Reset, Page::Enrol, Page::Delete] {
        h.model.go(page);
        h.model.forms.email = EMAIL.into();
        h.model.forms.password = PASSWORD.into();
        h.model.submit_sign_in();
        h.model.submit_sign_up();
        h.model.submit_delete();
    }
    h.model.edit_server();
    h.model.forms.server = "https://evil.example".into();
    h.model.save_server();
    h.model.close();
    h.model.open_link(api::parse_link(&format!("https://pool.test/account/reset#token={}", link_token())).unwrap());
    assert_eq!(h.model.page, Page::NotSetUp);
    assert_eq!(h.server.count(), 0);
    assert_eq!(h.model.base_url(), "");
}

#[test]
fn a_built_in_server_needs_no_confirmation_and_custom_mode_does() {
    let mut h = harness(Mode::Native);
    let factory = keychain_factory(h.server.clone(), h.keychain.clone(), Mode::Native, h.built.clone());
    h.model = AccountModel::new(factory, Runner::Inline, SettingsIo::memory()).with_built_in(Some(BASE.into()));
    h.model.open();
    h.frame();
    assert!(h.model.trusted && !h.model.custom_server());
    h.sign_in();
    assert_eq!(h.model.page, Page::Status);
    assert!(h.keychain.global.get(Slot::TrustedServer).is_none(), "nothing to confirm, nothing stored");
}

// ---- 3. the lock waits, then gives up ---------------------------------------------------

#[test]
fn a_lock_held_elsewhere_times_out_with_a_retryable_error() {
    let path = lock_path();
    let other = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path).unwrap();
    other.lock().unwrap(); // another copy of the app, stuck at a keychain prompt
    let lock = super::vault::FileLock { path: path.clone(), wait: Duration::from_millis(300) };
    let started = std::time::Instant::now();
    let r = block_on(super::vault::SessionLock::acquire(&lock));
    assert!(matches!(r, Err(ApiError::LockBusy)));
    assert!(started.elapsed() < Duration::from_secs(3));
    // The client reports it and keeps the session for a retry.
    let (s, v) = (Server::default(), SharedVault::default());
    v.seed_session(&token("rt", 0));
    let c = client(Mode::Native, &s, &v).with_lock(Box::new(super::vault::FileLock { path: path.clone(), wait: Duration::from_millis(300) }));
    assert_eq!(block_on(c.restore()), Err(ApiError::LockBusy));
    assert!(c.has_session());
    assert_eq!(s.count(), 0);
    assert!(ApiError::LockBusy.message(Flow::Status).contains("try again"));
    drop(other);
    s.with(|f| f.refresh = Some(token("rt", 0)));
    assert!(block_on(c.restore()).is_ok(), "works once the other copy lets go");
    let _ = std::fs::remove_file(path);
}

// ---- 4. no keychain call on the UI thread ----------------------------------------------

#[derive(Clone)]
struct ThreadSpy {
    inner: ScopedVault2,
    threads: Arc<Mutex<Vec<std::thread::ThreadId>>>,
}

#[derive(Clone)]
struct ScopedVault2(SharedVault);

impl ThreadSpy {
    fn note(&self) {
        self.threads.lock().unwrap().push(std::thread::current().id());
    }
}

impl Vault for ThreadSpy {
    fn load(&self, slot: &Slot) -> Result<Option<Secret>, VaultError> {
        self.note();
        self.inner.0.load(slot)
    }
    fn save(&self, slot: &Slot, value: &Secret) -> Result<(), VaultError> {
        self.note();
        self.inner.0.save(slot, value)
    }
    fn clear(&self, slot: &Slot) -> Result<(), VaultError> {
        self.note();
        self.inner.0.clear(slot)
    }
    fn persistent(&self) -> bool {
        true
    }
}

#[test]
fn every_keychain_call_runs_off_the_ui_thread() {
    let (server, vault) = (Server::default(), SharedVault::default());
    let threads = Arc::new(Mutex::new(Vec::new()));
    let (s, v, t) = (server.clone(), vault.clone(), threads.clone());
    let factory: super::model::Factory = Arc::new(move |base: &str| {
        let spy = ThreadSpy { inner: ScopedVault2(v.clone()), threads: t.clone() };
        (Client::new(base, Mode::Native, Box::new(MockTransport { server: s.clone(), mode: Mode::Native }), Box::new(spy)), None)
    });
    let file = Arc::new(Mutex::new(super::ShareSettings { base_url: BASE.into(), purge: vec![] }));
    let mut m = AccountModel::new(factory, Runner::Background, SettingsIo::Memory(file));
    let ui = std::thread::current().id();
    m.open();
    settle(&mut m, |m| m.page == Page::SignIn);
    m.trust_server();
    settle(&mut m, |m| !m.busy());
    m.forms.email = EMAIL.into();
    m.forms.password = PASSWORD.into();
    m.submit_sign_in();
    settle(&mut m, |m| m.page == Page::Status && !m.busy());
    vault.deletes_fail.store(true, Ordering::SeqCst);
    m.submit_sign_out(false);
    settle(&mut m, |m| m.page == Page::SignIn);
    vault.deletes_fail.store(false, Ordering::SeqCst);
    settle(&mut m, |m| m.client().is_some_and(|c| c.pending_purge().is_empty()));
    let seen = threads.lock().unwrap().clone();
    assert!(!seen.is_empty());
    assert!(!seen.contains(&ui), "a keychain call ran on the UI thread");
}

// ---- 5. purge entries belong to a server ------------------------------------------------

#[test]
fn purge_entries_are_kept_per_server() {
    let mut h = harness(Mode::Native);
    let other = super::PurgeEntry { server: "https://other.example".into(), account: UID.into() };
    {
        let mut f = h.file.lock().unwrap();
        f.base_url = BASE.into();
        f.purge = vec![other.clone()];
    }
    h.vault.seed_session(&token("rt", 0));
    h.server.with(|f| f.refresh = Some(token("rt", 0)));
    h.model.open();
    h.frame();
    h.frame();
    assert_eq!(h.model.page, Page::Status, "another server's entry does not touch this one's session");
    assert!(h.vault.refresh().is_some());
    assert_eq!(h.file.lock().unwrap().purge, vec![other], "and it is kept for that server");
}

// ---- 6. shared hosting -----------------------------------------------------------------

#[test]
fn shared_hosting_subdomains_are_different_sites() {
    for (l, a) in [
        ("evil.pages.dev", "vershion.pages.dev"),
        ("evil.github.io", "vershion.github.io"),
        ("x.workers.dev", "pool.vershion.workers.dev"),
        ("evil.vercel.app", "vershion.vercel.app"),
        ("evil.netlify.app", "vershion.netlify.app"),
        ("pages.dev", "vershion.pages.dev"),
    ] {
        assert!(!api::link_host_matches(l, a), "{l} vs {a}");
    }
    assert!(api::link_host_matches("vershion.pages.dev", "vershion.pages.dev"), "the exact host is fine");
}

// ---- 8. token text in parsed replies ----------------------------------------------------

#[test]
fn tokens_are_moved_out_of_parsed_replies() {
    let mut v = json!({"access_token": "aaa", "user": {"id": "x"}, "device_token": ""});
    let t = super::client::take_str(&mut v, "/access_token").unwrap();
    assert_eq!(t.expose(), "aaa");
    assert_eq!(v["access_token"], "", "no copy left in the parsed reply");
    assert!(super::client::take_str(&mut v, "/device_token").is_none());
    assert!(super::client::take_str(&mut v, "/missing").is_none());
}

// ---- 9. two app copies through the model ------------------------------------------------

#[test]
fn two_app_copies_restoring_through_the_model_never_replay() {
    let (server, keychain) = (Server::default(), Keychain::default());
    keychain.for_server(BASE).seed_session(&token("rt", 0));
    server.with(|f| f.refresh = Some(token("rt", 0)));
    let path = lock_path();
    let model = |kc: Keychain, s: Server, p: std::path::PathBuf| {
        let factory: super::model::Factory = Arc::new(move |base: &str| {
            let v = ScopedVault { server: kc.for_server(base), global: kc.global.clone() };
            let c = Client::new(base, Mode::Native, Box::new(MockTransport { server: s.clone(), mode: Mode::Native }), Box::new(v));
            (c.with_lock(Box::new(super::vault::FileLock::new(p.clone()))), None)
        });
        let file = Arc::new(Mutex::new(super::ShareSettings { base_url: BASE.into(), purge: vec![] }));
        AccountModel::new(factory, Runner::Inline, SettingsIo::Memory(file))
    };
    let mut a = model(keychain.clone(), server.clone(), path.clone());
    let mut b = model(keychain.clone(), server.clone(), path.clone());
    for m in [&mut a, &mut b] {
        m.open();
        m.frame(Instant::now());
        m.frame(Instant::now());
        assert_eq!(m.page, Page::Status);
        assert!(m.me.is_some());
    }
    let f = server.0.lock().unwrap();
    assert_eq!(f.replays, 0, "presented: {:?}", f.presented);
    assert_eq!(f.presented, vec![token("rt", 0), token("rt", 1)]);
    let _ = std::fs::remove_file(path);
}

// ---- 10. neutral link messages ----------------------------------------------------------

#[test]
fn link_results_do_not_claim_the_account_is_yours() {
    let mut h = trusted_harness();
    h.model.open_link(api::parse_link(&format!("https://pool.test/account/verify#token={}", link_token())).unwrap());
    h.frame();
    h.model.forms.password = "a fresh password!".into();
    h.model.forms.password2 = "a fresh password!".into();
    h.model.forms.display_name = "x".into();
    h.model.forms.school = "y".into();
    h.model.forms.adult = true;
    h.model.submit_confirm();
    h.frame();
    assert_eq!(h.notice(), super::model::LINK_CONFIRMED);
    for text in [super::model::LINK_CONFIRMED, super::model::LINK_RESET] {
        assert!(!text.to_lowercase().contains("your address"), "{text}");
        assert!(text.contains("If you did not request this on this device, ignore it."));
    }
}
