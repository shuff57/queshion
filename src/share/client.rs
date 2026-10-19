//! The shared-pool account client: every auth route of server/auth/README.md, with no UI.
//!
//! Sessions:
//! - Native: login returns a refresh token and a known-device token in the body; both go to the
//!   [`Vault`] (the OS keychain) under the server AND the account id, and stay in memory for this
//!   run. The device token rides as `X-Device-Token` on later logins, which lets the owner in
//!   during an attack lockdown. A token issued for one account never replaces another's.
//! - Web: the refresh and device tokens are HttpOnly cookies the page never sees. The access token
//!   is held in memory only (gone on reload; the account screen restores it with one refresh).
//! - Access tokens live 15 minutes. An authenticated call that gets `401 unauthorized` refreshes
//!   once and retries once. Refreshes are single-flight within the process, and serialised across
//!   processes (and browser tabs) by a [`SessionLock`]; under that lock the saved token is read
//!   again, so a token another copy of the app already rotated is never replayed.
//! - Rotation grace: the server hands back the same successor for a rotated token presented again
//!   within its `reuseGraceSeconds` (10 s). A refresh that got no answer is retried once with the
//!   same token, only within [`REFRESH_RETRY_WINDOW`], and each try is cut off after
//!   [`REFRESH_TIMEOUT`], so the retry always reaches the server inside the grace.
//! - Signing out deletes the saved token. If the keychain refuses, the account is put on a purge
//!   list ([`Client::pending_purge`]): its token is never used again, the app keeps retrying the
//!   delete, and the screen does not claim "signed out" until it succeeds.
//!
//! Nothing here runs until a caller asks: building a [`Client`] reads the vault and does no
//! network I/O, and the app builds one only when the account screen opens.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;
use std::task::{Poll, Waker};
use std::time::Duration;

use serde_json::{Map, Value, json};
use web_time::Instant;
use zeroize::Zeroize;

use super::api::{ApiError, Me, Profile};
use super::secret::Secret;
use super::transport::{Method, Mode, Request, Response, Transport};
use super::vault::{KEYCHAIN_FAILED, ProcessLocal, SessionLock, Slot, Vault};

/// The server's refresh-token reuse grace (`reuseGraceSeconds` in server/auth, default 10 s). If
/// the server's setting changes, these two must change with it.
pub const SERVER_REUSE_GRACE: Duration = Duration::from_secs(10);
/// Each refresh try is cut off after this long (other routes allow 30 s).
pub const REFRESH_TIMEOUT: Duration = Duration::from_secs(4);
/// A refresh that got no answer is retried with the same token only if the first try started
/// less than this long ago. The retry then reaches the server at most this long (plus transit)
/// after the server rotated the token, which must stay inside [`SERVER_REUSE_GRACE`].
pub const REFRESH_RETRY_WINDOW: Duration = Duration::from_secs(5);
const _: () = assert!(REFRESH_RETRY_WINDOW.as_secs() + 3 <= SERVER_REUSE_GRACE.as_secs());
const _: () = assert!(REFRESH_TIMEOUT.as_secs() <= REFRESH_RETRY_WINDOW.as_secs());

/// Refresh this long before the access token's stated expiry instead of waiting for a `401`.
const ACCESS_SLACK: Duration = Duration::from_secs(30);

pub type Clock = Box<dyn Fn() -> Instant + Send + Sync>;

/// What the vault's `Index` slot holds for one server: whose session it is, and which address
/// goes with which account id (to pick the device token at login).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Index {
    #[serde(default)]
    current: Option<String>,
    #[serde(default)]
    emails: BTreeMap<String, String>,
    /// The account of the newest device token, offered at a login whose address is not mapped
    /// yet (just after a reset or enrolment). The server ignores it for any other account.
    #[serde(default)]
    latest_device: Option<String>,
}

#[derive(Default)]
struct Inner {
    access: Option<Secret>,
    access_until: Option<Instant>,
    /// Bumped whenever the access token changes; a `401` on an older one means "retry", not
    /// "refresh again".
    generation: u64,
    /// Account id of the session (native; web learns it from `/me` but needs nothing else).
    account: Option<String>,
    /// Native only (web: an HttpOnly cookie).
    refresh: Option<Secret>,
    /// The last refresh-token write reached the vault (so the vault copy can be trusted).
    refresh_saved: bool,
    /// Native only, by account id (web: an HttpOnly cookie).
    devices: HashMap<String, Secret>,
    index: Index,
    /// Accounts whose saved refresh token must be deleted and never used.
    purge: BTreeSet<String>,
    refreshing: bool,
    /// Counts finished refreshes, so a waiter can tell its refresh has happened.
    refresh_round: u64,
    last_refresh: Option<Result<(), ApiError>>,
    waiters: Vec<Waker>,
    /// Set when the keychain refused a write: tokens are then in memory only.
    persist_note: Option<&'static str>,
    /// The server address changed: nothing more may be written for this client.
    retired: bool,
    /// Bumped by every sign-out. A login or refresh that started under an older epoch finishes
    /// without keeping anything, so a reply arriving after "Sign out" cannot sign back in.
    epoch: u64,
}

/// A session taken out of memory by [`Client::begin_sign_out`], still to be wiped from the vault
/// and reported to the server.
pub struct SignOut {
    account: Option<String>,
    refresh: Option<Secret>,
    access: Option<Secret>,
    forget_device: bool,
}

impl std::fmt::Debug for SignOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignOut").field("account", &self.account).field("forget_device", &self.forget_device).finish_non_exhaustive()
    }
}

/// Moves a token string out of a parsed reply (no copy) and leaves an empty string behind.
pub(crate) fn take_str(v: &mut Value, pointer: &str) -> Option<Secret> {
    match v.pointer_mut(pointer) {
        Some(Value::String(s)) if !s.is_empty() => Some(Secret::new(std::mem::take(s))),
        _ => None,
    }
}

pub struct Client {
    base: String,
    mode: Mode,
    transport: Box<dyn Transport>,
    vault: Box<dyn Vault>,
    lock: Box<dyn SessionLock>,
    clock: Clock,
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let i = self.lock();
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("mode", &self.mode)
            .field("account", &i.account)
            .field("access", &i.access.as_ref().map(|_| "<redacted>"))
            .field("refresh", &i.refresh.as_ref().map(|_| "<redacted>"))
            .field("devices", &i.devices.keys().collect::<Vec<_>>())
            .field("purge", &i.purge)
            .field("generation", &i.generation)
            .finish()
    }
}

/// What a confirmation or reset told us.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirmed {
    /// The link set the password (confirm with a password always does).
    pub password_set: bool,
    /// This device is now a known device of the link's account.
    pub device_known: bool,
    /// The link belonged to an account other than the one signed in here (whose session was
    /// left alone).
    pub other_account: bool,
}

/// The account id inside a device token (`v1.<user id>.<epoch>.<expiry>.<nonce>.<mac>`).
pub fn device_account(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    (parts.next() == Some("v1")).then_some(())?;
    let id = parts.next()?;
    let ok = id.len() == 36 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    ok.then(|| id.to_ascii_lowercase())
}

/// Serialises a JSON body without leaving copies of its strings behind: the text is written
/// into a buffer of exactly the right size (no reallocation leaves an old copy in freed memory)
/// and every string in the value is overwritten afterwards. serde_json's and the HTTP stack's own
/// internal buffers are outside our reach.
pub fn json_body(mut v: Value) -> Secret {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0 += b.len();
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut n = Count(0);
    let _ = serde_json::to_writer(&mut n, &v);
    let mut buf = Vec::with_capacity(n.0);
    let _ = serde_json::to_writer(&mut buf, &v);
    wipe(&mut v);
    match String::from_utf8(buf) {
        Ok(s) => Secret::new(s),
        Err(e) => {
            let mut b = e.into_bytes();
            b.zeroize();
            Secret::default()
        }
    }
}

pub(crate) fn wipe(v: &mut Value) {
    match v {
        Value::String(s) => s.zeroize(),
        Value::Array(a) => a.iter_mut().for_each(wipe),
        Value::Object(m) => m.values_mut().for_each(wipe),
        _ => {}
    }
}

/// Resets the single-flight state if a refresh ends any way but normally (a panic, or the future
/// being dropped), so later callers neither hang nor wait for nothing.
struct RefreshFlag<'a> {
    client: &'a Client,
    done: bool,
}

impl RefreshFlag<'_> {
    fn finish(mut self, result: &Result<(), ApiError>) {
        self.done = true;
        self.client.end_refresh(result.clone());
    }
}

impl Drop for RefreshFlag<'_> {
    fn drop(&mut self) {
        if !self.done {
            self.client.end_refresh(Err(ApiError::Offline("The sign-in check was interrupted.".into())));
        }
    }
}

impl Client {
    /// `base` must already be validated ([`super::api::validate_base_url`]). Reads the vault (the
    /// keychain on native); does not touch the network. No cross-process lock: see
    /// [`Client::with_lock`].
    pub fn new(base: &str, mode: Mode, transport: Box<dyn Transport>, vault: Box<dyn Vault>) -> Client {
        Client::with_clock(base, mode, transport, vault, Box::new(Instant::now))
    }

    pub fn with_clock(base: &str, mode: Mode, transport: Box<dyn Transport>, vault: Box<dyn Vault>, clock: Clock) -> Client {
        let c = Client {
            base: base.trim_end_matches('/').to_string(),
            mode,
            transport,
            vault,
            lock: Box::new(ProcessLocal),
            clock,
            inner: Mutex::new(Inner::default()),
        };
        if mode == Mode::Native {
            c.load();
        }
        c
    }

    /// Serialise refreshes with other processes (or tabs) through `lock`.
    pub fn with_lock(mut self, lock: Box<dyn SessionLock>) -> Client {
        self.lock = lock;
        self
    }

    fn load(&self) {
        let index: Index = self
            .vault
            .load(&Slot::Index)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(s.expose()).ok())
            .unwrap_or_default();
        // Keychain calls first, the lock only to publish the results: the keychain may be slow
        // (an unlock prompt), and the UI thread reads this state.
        let refresh = index.current.as_ref().and_then(|a| self.vault.load(&Slot::Refresh(a.clone())).ok().flatten());
        let mut devices = HashMap::new();
        for a in index.emails.values().chain(index.latest_device.iter()) {
            if let Ok(Some(d)) = self.vault.load(&Slot::Device(a.clone())) {
                devices.insert(a.clone(), d);
            }
        }
        let mut i = self.lock();
        if let (Some(a), Some(r)) = (&index.current, refresh) {
            i.refresh = Some(r);
            i.refresh_saved = true;
            i.account = Some(a.clone());
        }
        i.devices = devices;
        i.index = index;
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Native: a usable refresh token is held. Web: an access token is held (a cookie session may
    /// still exist without one; [`Client::restore`] finds out).
    pub fn has_session(&self) -> bool {
        let i = self.lock();
        match self.mode {
            Mode::Native => i.refresh.is_some(),
            Mode::Web => i.access.is_some(),
        }
    }

    /// The account id of the session, if known.
    pub fn account(&self) -> Option<String> {
        self.lock().account.clone()
    }

    pub fn knows_device(&self) -> bool {
        let i = self.lock();
        match &i.account {
            Some(a) => i.devices.contains_key(a),
            None => !i.devices.is_empty(),
        }
    }

    pub fn knows_device_for(&self, account: &str) -> bool {
        self.lock().devices.contains_key(account)
    }

    /// Whether the person confirmed that this exact server address may receive their password
    /// (kept in the keychain, so editing `share.json` can't forge it).
    pub fn is_trusted(&self) -> bool {
        matches!(self.vault.load(&Slot::TrustedServer), Ok(Some(t)) if t.expose() == self.base)
    }

    pub fn set_trusted(&self) {
        let _ = self.vault.save(&Slot::TrustedServer, &Secret::new(self.base.clone()));
    }

    /// Forgets the trusted address (the address changed).
    pub fn clear_trusted(&self) {
        let _ = self.vault.clear(&Slot::TrustedServer);
    }

    /// Set when the keychain refused a write during this run.
    pub fn persist_note(&self) -> Option<&'static str> {
        self.lock().persist_note
    }

    pub fn vault_persistent(&self) -> bool {
        self.vault.persistent()
    }

    // ---- local storage -----------------------------------------------------------------

    /// Accounts whose saved sign-in still has to be deleted (the app records them in
    /// `share.json`, so the next run deletes them before anything else).
    pub fn pending_purge(&self) -> Vec<String> {
        self.lock().purge.iter().cloned().collect()
    }

    /// Deletes the saved sign-in of `accounts`, and drops it from memory so it is never used
    /// (called when a client is built, with what an earlier run could not delete). Returns what is
    /// still pending.
    pub fn purge(&self, accounts: &[String]) -> Vec<String> {
        {
            let mut i = self.lock();
            for a in accounts {
                i.purge.insert(a.clone());
                if i.account.as_deref() == Some(a.as_str()) {
                    i.refresh = None;
                    i.access = None;
                    i.generation += 1;
                }
            }
        }
        self.retry_purge()
    }

    /// One more attempt at every pending delete. Returns what is still pending.
    pub fn retry_purge(&self) -> Vec<String> {
        for a in self.pending_purge() {
            if self.vault.clear(&Slot::Refresh(a.clone())).is_ok() {
                self.lock().purge.remove(&a);
            }
        }
        self.pending_purge()
    }

    fn save_index(&self) {
        let (index, retired) = {
            let i = self.lock();
            (i.index.clone(), i.retired)
        };
        if retired {
            return;
        }
        if let Ok(text) = serde_json::to_string(&index) {
            let _ = self.vault.save(&Slot::Index, &Secret::new(text));
        }
    }

    /// Keeps a rotated or new refresh token for `account`. If the vault refuses, the old saved
    /// token is deleted (it would be replayed next run and end the session as "stolen"); if even
    /// that fails, the account goes on the purge list.
    fn store_refresh(&self, account: &str, token: Secret, epoch: u64) -> bool {
        let retired = self.lock().retired;
        if self.lock().epoch != epoch {
            return false;
        }
        let saved = !retired && self.vault.save(&Slot::Refresh(account.to_string()), &token).is_ok();
        if saved && self.lock().epoch != epoch {
            // Signed out while the keychain was writing: take it back out.
            if self.vault.clear(&Slot::Refresh(account.to_string())).is_err() {
                self.lock().purge.insert(account.to_string());
            }
            return false;
        }
        if !saved && !retired && self.vault.persistent() && self.vault.clear(&Slot::Refresh(account.to_string())).is_err() {
            self.lock().purge.insert(account.to_string());
        }
        let mut i = self.lock();
        if !saved && !retired && self.vault.persistent() {
            i.persist_note = Some(KEYCHAIN_FAILED);
        }
        if saved {
            // The slot now holds the live token: deleting it later would end a good session.
            i.purge.remove(account);
        }
        if i.epoch != epoch {
            return false;
        }
        i.refresh = Some(token);
        i.refresh_saved = saved;
        i.account = Some(account.to_string());
        i.index.current = Some(account.to_string());
        true
    }

    /// Deletes the saved refresh token of `account`. False if the vault refused (the account is
    /// then on the purge list).
    fn clear_refresh(&self, account: &str) -> bool {
        let ok = self.vault.clear(&Slot::Refresh(account.to_string())).is_ok();
        if !ok {
            self.lock().purge.insert(account.to_string());
        }
        ok
    }

    fn store_device(&self, account: &str, token: Secret) {
        let retired = self.lock().retired;
        if !retired && self.vault.save(&Slot::Device(account.to_string()), &token).is_err() && self.vault.persistent() {
            self.lock().persist_note = Some(KEYCHAIN_FAILED);
        }
        {
            let mut i = self.lock();
            i.devices.insert(account.to_string(), token);
            i.index.latest_device = Some(account.to_string());
        }
        self.save_index();
    }

    /// Ends the local session (access and refresh) of the current account. The device token
    /// stays unless `forget_device`. False if a keychain delete failed.
    fn drop_session(&self, forget_device: bool) -> bool {
        let account = {
            let mut i = self.lock();
            i.access = None;
            i.access_until = None;
            i.generation += 1;
            i.refresh = None;
            i.refresh_saved = false;
            i.account.take()
        };
        if self.mode != Mode::Native {
            return true;
        }
        let Some(a) = account else { return true };
        let mut ok = self.clear_refresh(&a);
        if forget_device {
            ok &= self.vault.clear(&Slot::Device(a.clone())).is_ok();
            let mut i = self.lock();
            i.devices.remove(&a);
            if i.index.latest_device.as_deref() == Some(a.as_str()) {
                i.index.latest_device = None;
            }
            i.index.emails.retain(|_, id| *id != a);
        }
        if self.lock().index.current.as_deref() == Some(a.as_str()) {
            self.lock().index.current = None;
        }
        self.save_index();
        ok
    }

    // ---- plumbing ----------------------------------------------------------------------

    fn request(&self, method: Method, path: &str, body: Option<Secret>) -> Request {
        Request { method, url: format!("{}{}", self.base, path), headers: Vec::new(), body, timeout: None }
    }

    async fn send(&self, req: Request) -> Result<Response, ApiError> {
        Ok(self.transport.send(req).await?)
    }

    /// One unauthenticated call; any non-2xx becomes an [`ApiError`].
    async fn call(&self, method: Method, path: &str, body: Option<Secret>) -> Result<Response, ApiError> {
        let resp = self.send(self.request(method, path, body)).await?;
        if (200..300).contains(&resp.status) { Ok(resp) } else { Err(ApiError::from_response(&resp)) }
    }

    fn set_access(&self, access: Secret, expires_in: Option<u64>) {
        let now = (self.clock)();
        let mut i = self.lock();
        i.access = Some(access);
        // Refresh a little early, but never so early that a short-lived token is stale on arrival.
        i.access_until = expires_in.map(|s| {
            let life = Duration::from_secs(s);
            now + life.saturating_sub(ACCESS_SLACK.min(life / 2))
        });
        i.generation += 1;
    }

    /// Stores what a token response carries (login, refresh). `email` is the address used to
    /// sign in (login only).
    fn take_tokens(&self, resp: &Response, email: Option<&str>, epoch: u64) -> Result<(), ApiError> {
        let mut v = resp.json();
        let r = self.take_tokens_from(&mut v, email, epoch);
        // Whatever token text is left in the parsed reply is overwritten (residual: serde_json's
        // own parse buffers, and the response bytes ureq or the browser held).
        wipe(&mut v);
        r
    }

    fn take_tokens_from(&self, v: &mut Value, email: Option<&str>, epoch: u64) -> Result<(), ApiError> {
        let Some(access) = take_str(v, "/access_token") else { return Err(ApiError::BadReply("no access token".into())) };
        if self.lock().epoch != epoch {
            // Signed out while this was on its way: keep nothing.
            return Err(ApiError::SignedOut);
        }
        if self.mode == Mode::Native {
            let Some(refresh) = take_str(v, "/refresh_token") else {
                return Err(ApiError::BadReply("no refresh token".into()));
            };
            let account = v
                .pointer("/user/id")
                .and_then(Value::as_str)
                .map(str::to_ascii_lowercase)
                .or_else(|| self.lock().account.clone());
            let Some(account) = account else { return Err(ApiError::BadReply("no account id".into())) };
            // Signing in as someone else ends the previous account's local session first.
            let previous = self.lock().account.clone();
            if let Some(p) = previous.filter(|p| *p != account) {
                self.clear_refresh(&p);
            }
            // The rotated token is kept before anything else happens: losing it ends the session.
            if !self.store_refresh(&account, refresh, epoch) {
                return Err(ApiError::SignedOut);
            }
            if let Some(e) = email {
                self.lock().index.emails.insert(e.trim().to_ascii_lowercase(), account.clone());
            }
            if let Some(d) = take_str(v, "/device_token") {
                // Only a token issued for this very account is kept for it.
                if device_account(d.expose()).as_deref() == Some(account.as_str()) {
                    self.store_device(&account, d);
                }
            }
            self.save_index();
        }
        let expires = v.get("expires_in").and_then(Value::as_u64);
        let mut i = self.lock();
        if i.epoch != epoch {
            return Err(ApiError::SignedOut);
        }
        drop(i.access.take());
        drop(i);
        self.set_access(access, expires);
        Ok(())
    }

    /// Native: keeps a device token from a body (verify, reset, enrol) under the account it was
    /// issued for. Returns that account.
    fn take_device(&self, v: &mut Value) -> Option<String> {
        if self.mode != Mode::Native {
            return None;
        }
        let d = take_str(v, "/device_token")?;
        let account = device_account(d.expose())?;
        self.store_device(&account, d);
        Some(account)
    }

    // ---- refresh -----------------------------------------------------------------------

    /// One refresh, under the cross-process lock, with the lost-reply retry from the module docs.
    async fn refresh_once(&self) -> Result<(), ApiError> {
        let epoch = self.lock().epoch;
        let _held = self.lock.acquire().await?;
        let token = match self.mode {
            Mode::Native => {
                let (account, mem, saved) = {
                    let i = self.lock();
                    (i.account.clone(), i.refresh.clone(), i.refresh_saved)
                };
                let Some(account) = account else { return Err(ApiError::SignedOut) };
                // Another copy of the app may have rotated the token, or signed out, since we
                // read it. The saved copy is the truth whenever ours reached the vault.
                let token = if saved {
                    match self.vault.load(&Slot::Refresh(account.clone())) {
                        Ok(Some(t)) => t,
                        Ok(None) => {
                            self.lock().refresh = None;
                            self.lock().account = None;
                            return Err(ApiError::SignedOut);
                        }
                        Err(_) => match mem {
                            Some(t) => t,
                            None => return Err(ApiError::SignedOut),
                        },
                    }
                } else {
                    match mem {
                        Some(t) => t,
                        None => return Err(ApiError::SignedOut),
                    }
                };
                self.lock().refresh = Some(token.clone());
                Some(token)
            }
            Mode::Web => None,
        };
        let started = (self.clock)();
        let mut retried = false;
        loop {
            let body = token.as_ref().map(|t| json_body(json!({ "refresh_token": t.expose() })));
            let mut req = self.request(Method::Post, "/auth/refresh", body);
            req.timeout = Some(REFRESH_TIMEOUT);
            match self.send(req).await {
                Ok(resp) if resp.status == 200 => return self.take_tokens(&resp, None, epoch),
                Ok(resp) if resp.status == 401 => {
                    self.drop_session(false);
                    return Err(ApiError::SignedOut);
                }
                // 429, 5xx: the token is still good; keep it.
                Ok(resp) => return Err(ApiError::from_response(&resp)),
                Err(e @ ApiError::Offline(_)) => {
                    if retried || (self.clock)().saturating_duration_since(started) >= REFRESH_RETRY_WINDOW {
                        return Err(e);
                    }
                    retried = true;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn end_refresh(&self, result: Result<(), ApiError>) {
        let waiters = {
            let mut i = self.lock();
            i.refreshing = false;
            i.refresh_round += 1;
            i.last_refresh = Some(result);
            std::mem::take(&mut i.waiters)
        };
        for w in waiters {
            w.wake();
        }
    }

    /// Makes sure an access token newer than `seen` exists, refreshing at most once across every
    /// concurrent caller.
    async fn refresh_after(&self, seen: u64) -> Result<(), ApiError> {
        let round = {
            let mut i = self.lock();
            if i.generation != seen && i.access.is_some() {
                return Ok(());
            }
            if i.refreshing {
                Some(i.refresh_round)
            } else {
                i.refreshing = true;
                None
            }
        };
        if let Some(round) = round {
            return self.wait_for_refresh(round).await;
        }
        let flag = RefreshFlag { client: self, done: false };
        let result = self.refresh_once().await;
        flag.finish(&result);
        result
    }

    /// Resolves with the outcome of the refresh that was in flight at `round`.
    async fn wait_for_refresh(&self, round: u64) -> Result<(), ApiError> {
        std::future::poll_fn(|cx| {
            let mut i = self.lock();
            if i.refresh_round > round {
                return Poll::Ready(i.last_refresh.clone().unwrap_or(Err(ApiError::SignedOut)));
            }
            i.waiters.push(cx.waker().clone());
            Poll::Pending
        })
        .await
    }

    /// The access token to use, refreshing first when there is none or it is about to expire.
    async fn access(&self) -> Result<(Secret, u64), ApiError> {
        let seen = {
            let i = self.lock();
            let fresh = i.access_until.is_none_or(|t| (self.clock)() < t);
            if let (Some(a), true) = (&i.access, fresh) {
                return Ok((a.clone(), i.generation));
            }
            if self.mode == Mode::Native && i.refresh.is_none() {
                return Err(ApiError::SignedOut);
            }
            i.generation
        };
        self.refresh_after(seen).await?;
        // Whatever the refresh produced is used as is (no second refresh for a short-lived token).
        let i = self.lock();
        i.access.clone().map(|a| (a, i.generation)).ok_or(ApiError::SignedOut)
    }

    /// An authenticated call: bearer token, one refresh-and-retry on `401 unauthorized`. The body
    /// is serialised once by the caller; each try sends a copy that is wiped when dropped.
    async fn authed(&self, method: Method, path: &str, body: Option<Secret>) -> Result<Response, ApiError> {
        for attempt in 0..2 {
            let (token, generation) = self.access().await?;
            let mut req = self.request(method, path, body.clone());
            req.headers.push(("Authorization".into(), Secret::new(format!("Bearer {}", token.expose()))));
            let resp = self.send(req).await?;
            // Only the bearer check's 401 means "token expired". A route's own 401 (a wrong
            // password on DELETE /account is `invalid_credentials`) is that route's answer.
            let expired = resp.status == 401 && matches!(ApiError::from_response(&resp).code(), "unauthorized" | "");
            if expired && attempt == 0 {
                self.refresh_after(generation).await?;
                continue;
            }
            if expired {
                self.drop_session(false);
                return Err(ApiError::SignedOut);
            }
            return if (200..300).contains(&resp.status) { Ok(resp) } else { Err(ApiError::from_response(&resp)) };
        }
        Err(ApiError::SignedOut)
    }

    // ---- routes ------------------------------------------------------------------------

    /// `POST /auth/signup`. Always `202` whatever the address; it never signs in.
    pub async fn sign_up(&self, email: &str, password: &Secret, profile: &Profile) -> Result<(), ApiError> {
        let mut body = Map::new();
        body.insert("email".into(), Value::from(email.trim()));
        body.insert("password".into(), Value::from(password.expose()));
        profile.add_to(&mut body);
        self.call(Method::Post, "/auth/signup", Some(json_body(Value::Object(body)))).await.map(|_| ())
    }

    /// `POST /auth/resend`: another confirmation mail (same answer for every address).
    pub async fn resend(&self, email: &str) -> Result<(), ApiError> {
        self.call(Method::Post, "/auth/resend", Some(json_body(json!({ "email": email.trim() })))).await.map(|_| ())
    }

    /// After a verify or reset: the server ended every session of the link's account. The local
    /// session is dropped only if it belongs to that account (known from the device token); a
    /// link for some other account leaves it alone.
    fn after_link(&self, v: &mut Value) -> Confirmed {
        let link_account = self.take_device(v);
        let current = self.lock().account.clone();
        let other_account = match (&link_account, &current) {
            (Some(l), Some(c)) => l != c,
            _ => false,
        };
        if self.mode == Mode::Native && link_account.is_some() && !other_account {
            self.drop_session(false);
        }
        if self.mode == Mode::Web {
            // The page cannot see whose link it was. The server cleared this browser's refresh
            // cookie with the reset, so the in-memory access token is all that is left: drop it.
            self.drop_session(false);
        }
        Confirmed {
            password_set: v.get("password_set").and_then(Value::as_bool).unwrap_or(true),
            device_known: link_account.is_some() || self.mode == Mode::Web,
            other_account,
        }
    }

    /// `POST /auth/verify` with the new password and profile: activates the account in one step.
    /// It does not sign in.
    pub async fn confirm(&self, token: &Secret, password: &Secret, profile: &Profile) -> Result<Confirmed, ApiError> {
        let mut body = Map::new();
        body.insert("token".into(), Value::from(token.expose()));
        body.insert("password".into(), Value::from(password.expose()));
        profile.add_to(&mut body);
        let resp = self.call(Method::Post, "/auth/verify", Some(json_body(Value::Object(body)))).await?;
        let mut v = resp.json();
        let password_set = v.get("password_set").and_then(Value::as_bool).unwrap_or(false);
        let mut c = self.after_link(&mut v);
        wipe(&mut v);
        c.password_set = password_set;
        Ok(c)
    }

    /// `POST /auth/login`. Native sends the device token of the account it knows for this
    /// address (or the newest one, right after a reset or enrolment; the server ignores a token
    /// of any other account).
    pub async fn sign_in(&self, email: &str, password: &Secret) -> Result<(), ApiError> {
        let epoch = self.lock().epoch;
        let body = json_body(json!({ "email": email.trim(), "password": password.expose() }));
        let mut req = self.request(Method::Post, "/auth/login", Some(body));
        if self.mode == Mode::Native {
            let device = {
                let i = self.lock();
                let key = email.trim().to_ascii_lowercase();
                i.index.emails.get(&key).or(i.index.latest_device.as_ref()).and_then(|a| i.devices.get(a)).cloned()
            };
            if let Some(d) = device {
                req.headers.push(("X-Device-Token".into(), d));
            }
        }
        let resp = self.send(req).await?;
        if resp.status != 200 {
            return Err(ApiError::from_response(&resp));
        }
        self.take_tokens(&resp, Some(email), epoch)
    }

    /// `POST /auth/device/enrol`: redeems the mailed code with the password; this device becomes
    /// known for the code's account (returned), so the next sign-in passes an attack lockdown.
    pub async fn enrol(&self, code: &Secret, password: &Secret) -> Result<Option<String>, ApiError> {
        let body = json_body(json!({ "code": code.expose(), "password": password.expose() }));
        let resp = self.call(Method::Post, "/auth/device/enrol", Some(body)).await?;
        let mut v = resp.json();
        let account = self.take_device(&mut v);
        wipe(&mut v);
        match account {
            Some(a) => Ok(Some(a)),
            None if self.mode == Mode::Web => Ok(None),
            None => Err(ApiError::BadReply("no device token".into())),
        }
    }

    /// Step 1 of signing out, on the caller's thread and without I/O: the session leaves memory
    /// at once, and any login or refresh still running will keep nothing it gets back.
    pub fn begin_sign_out(&self, forget_device: bool) -> SignOut {
        let mut i = self.lock();
        i.epoch += 1;
        i.generation += 1;
        i.access_until = None;
        let account = i.account.take();
        if i.index.current.is_some() && i.index.current == account {
            i.index.current = None;
        }
        SignOut { account, refresh: i.refresh.take(), access: i.access.take(), forget_device }
    }

    /// Step 2: deletes the saved session (and, if asked, this device's token) from the vault,
    /// before anything goes over the network. If the keychain refuses, the account goes on the
    /// purge list and the result is [`ApiError::LocalCleanup`].
    pub fn wipe_signed_out(&self, s: &SignOut, everywhere: bool) -> Result<(), ApiError> {
        if self.mode != Mode::Native {
            return Ok(());
        }
        let Some(a) = &s.account else { return Ok(()) };
        let mut ok = self.clear_refresh(a);
        if everywhere || s.forget_device {
            ok &= self.vault.clear(&Slot::Device(a.clone())).is_ok();
            let mut i = self.lock();
            i.devices.remove(a);
            if i.index.latest_device.as_deref() == Some(a.as_str()) {
                i.index.latest_device = None;
            }
            i.index.emails.retain(|_, id| id != a);
        }
        self.save_index();
        if ok { Ok(()) } else { Err(ApiError::LocalCleanup) }
    }

    /// Step 3, best effort: `POST /auth/logout` with the tokens taken in step 1, which exist only
    /// in this call by now. `everywhere` ends every session and known device of the account (the
    /// server needs the refresh token for that).
    pub async fn tell_server_signed_out(&self, s: SignOut, everywhere: bool) -> Result<(), ApiError> {
        let mut body = Map::new();
        if let Some(r) = &s.refresh {
            body.insert("refresh_token".into(), Value::from(r.expose()));
        }
        if everywhere {
            body.insert("scope".into(), Value::from("global"));
        }
        if s.forget_device || everywhere {
            body.insert("forget_device".into(), Value::from(true));
        }
        let mut req = self.request(Method::Post, "/auth/logout", Some(json_body(Value::Object(body))));
        if let Some(a) = &s.access {
            req.headers.push(("Authorization".into(), Secret::new(format!("Bearer {}", a.expose()))));
        }
        match self.send(req).await {
            Ok(r) if (200..300).contains(&r.status) => Ok(()),
            // The server no longer knew the session: signed out all the same.
            Ok(r) if r.status == 401 && !everywhere => Ok(()),
            Ok(r) => Err(ApiError::from_response(&r)),
            Err(e) => Err(e),
        }
    }

    /// All three steps. The saved session is gone before the network is touched, so quitting the
    /// app at any moment leaves this device signed out (never "signing out" with a token still
    /// saved). For `everywhere` the refresh token lives only in memory for the server call; if
    /// that call fails, this device is signed out anyway and the person signs in again to retry.
    /// Returns [`ApiError::LocalCleanup`] if the keychain would not delete, else the server's
    /// answer (for a plain sign-out an error there changes nothing locally).
    pub async fn sign_out(&self, everywhere: bool, forget_device: bool) -> Result<(), ApiError> {
        let s = self.begin_sign_out(forget_device);
        let local = self.wipe_signed_out(&s, everywhere);
        let server = self.tell_server_signed_out(s, everywhere).await;
        local.and(server)
    }

    /// `POST /auth/forgot` (same answer for every address).
    pub async fn forgot(&self, email: &str) -> Result<(), ApiError> {
        self.call(Method::Post, "/auth/forgot", Some(json_body(json!({ "email": email.trim() })))).await.map(|_| ())
    }

    /// `POST /auth/reset`. `profile` is needed only for an account that was never finished; the
    /// server says so with `invalid_request` on a profile field ([`ApiError::needs_profile`])
    /// before using the link.
    pub async fn reset(&self, token: &Secret, password: &Secret, profile: Option<&Profile>) -> Result<Confirmed, ApiError> {
        let mut body = Map::new();
        body.insert("token".into(), Value::from(token.expose()));
        body.insert("password".into(), Value::from(password.expose()));
        if let Some(p) = profile {
            p.add_to(&mut body);
        }
        let resp = self.call(Method::Post, "/auth/reset", Some(json_body(Value::Object(body)))).await?;
        let mut v = resp.json();
        let c = self.after_link(&mut v);
        wipe(&mut v);
        Ok(c)
    }

    /// `GET /me`.
    pub async fn me(&self) -> Result<Me, ApiError> {
        let resp = self.authed(Method::Get, "/me", None).await?;
        let v = resp.json();
        if self.mode == Mode::Native
            && let Some(id) = v.get("id").and_then(Value::as_str)
            && let Some(email) = v.get("email").and_then(Value::as_str)
            && self.lock().account.as_deref() == Some(id.to_ascii_lowercase().as_str())
        {
            self.lock().index.emails.insert(email.to_ascii_lowercase(), id.to_ascii_lowercase());
        }
        Me::parse(&v).ok_or_else(|| ApiError::BadReply("account".into()))
    }

    /// Picks up an existing session when the account screen opens. Native: only if a refresh
    /// token was saved (otherwise no request at all). Web: one refresh with the cookie, if any.
    pub async fn restore(&self) -> Result<Me, ApiError> {
        if self.mode == Mode::Native && !self.has_session() {
            return Err(ApiError::SignedOut);
        }
        self.me().await
    }

    /// `POST /account/reapply` after a rejection and the waiting period.
    pub async fn reapply(&self, profile: &Profile) -> Result<(), ApiError> {
        let mut body = Map::new();
        profile.add_to(&mut body);
        self.authed(Method::Post, "/account/reapply", Some(json_body(Value::Object(body)))).await.map(|_| ())
    }

    /// `DELETE /account` with the password. Everything local goes too, the device token included.
    pub async fn delete_account(&self, password: &Secret) -> Result<(), ApiError> {
        let body = json_body(json!({ "password": password.expose() }));
        self.authed(Method::Delete, "/account", Some(body)).await?;
        if !self.drop_session(true) {
            return Err(ApiError::LocalCleanup);
        }
        Ok(())
    }

    /// The server address changed: forgets this server's session on this device without telling
    /// the server, and makes sure a job still running on this client can write nothing more.
    pub fn forget_local(&self) {
        self.drop_session(true);
        self.lock().retired = true;
    }

    /// Stops this client from writing anything more (another client took over), without
    /// touching what is saved: the saved session still belongs to its server.
    pub fn retire(&self) {
        self.lock().retired = true;
    }

    #[cfg(test)]
    pub(crate) fn generation(&self) -> u64 {
        self.lock().generation
    }

    #[cfg(test)]
    pub(crate) fn access_for_test(&self) -> Option<String> {
        self.lock().access.as_ref().map(|s| s.expose().to_string())
    }
}
