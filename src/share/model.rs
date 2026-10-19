//! The Account screen's logic: which page shows, the forms, one background job at a time, and
//! status polling. No egui, so tests drive every flow with a mock transport.
//!
//! Network rule: nothing here runs unless [`AccountModel::frame`] is called with the screen open,
//! or the person submits a form on it. Opening the screen builds the [`Client`] (reading the
//! keychain) on first use; closing it stops polling at once. A job already in flight finishes in
//! the background and its result is applied the next time the screen is open.

use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

use web_time::Instant;

use super::{NOT_SET_UP, PurgeEntry, ShareSettings};
use super::api::{self, ApiError, Flow, Link, LinkKind, Me, Profile, Status};
use super::client::{Client, Confirmed};
use super::secret::Secret;
use super::transport::{LocalFuture, Mode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailKind {
    Verify,
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// A release build without a built-in server: sharing is off.
    NotSetUp,
    /// The client (and keychain) is being set up on a job thread.
    Opening,
    /// No server address yet (development builds).
    NotConfigured,
    /// Editing the server address.
    Server,
    SignIn,
    SignUp,
    /// "Check your email": accepts the pasted link or code.
    CheckEmail(MailKind),
    /// From a verify link: new password and profile.
    Confirm,
    Forgot,
    /// From a reset link: new password (and the profile, if the account was never finished).
    Reset,
    /// Device enrolment code (login refused during a lockdown).
    Enrol,
    /// Signed in: approval status and account actions.
    Status,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Info(String),
    Error(String),
}

/// Every text field of every page. Password fields are wiped on success and whenever the screen
/// closes or the page changes.
#[derive(Default)]
pub struct Forms {
    pub server: String,
    pub email: String,
    pub password: String,
    pub password2: String,
    pub display_name: String,
    pub school: String,
    pub adult: bool,
    pub paste: String,
    pub forget_device: bool,
    /// Show the pasted link in clear text (masked by default, like a password).
    pub show_paste: bool,
    pub delete_ack: bool,
}

impl Forms {
    pub fn profile(&self) -> Profile {
        Profile { display_name: self.display_name.clone(), school: self.school.clone(), adult_school_employee: self.adult }
    }

    pub fn wipe_secrets(&mut self) {
        use zeroize::Zeroize;
        self.password.zeroize();
        self.password2.zeroize();
        self.paste.zeroize();
        self.show_paste = false;
    }
}

/// Status polling while the screen is open: every minute while pending, backing off on errors up
/// to 15 minutes, and not at all in a settled state (approved, rejected, suspended) where only
/// "Check now" asks.
#[derive(Clone, Debug, Default)]
pub struct Poller {
    next_at: Option<Instant>,
    failures: u32,
}

pub const POLL_PENDING: Duration = Duration::from_secs(60);
pub const POLL_MAX: Duration = Duration::from_secs(15 * 60);

impl Poller {
    pub fn after_success(&mut self, now: Instant, status: &Status) {
        self.failures = 0;
        self.next_at = matches!(status, Status::Pending).then(|| now + POLL_PENDING);
    }

    pub fn after_failure(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        let secs = POLL_PENDING.as_secs().saturating_mul(1u64 << self.failures.min(8));
        self.next_at = Some(now + Duration::from_secs(secs).min(POLL_MAX));
    }

    pub fn stop(&mut self) {
        self.next_at = None;
    }

    pub fn due(&self, now: Instant) -> bool {
        self.next_at.is_some_and(|t| now >= t)
    }

    pub fn next_at(&self) -> Option<Instant> {
        self.next_at
    }
}

/// A finished job (or an early report from one).
pub enum Outcome {
    /// The client for the server is ready (built on a job thread).
    Opened { client: Arc<Client>, note: Option<&'static str>, trusted: bool },
    /// Keychain upkeep finished (trust saved, an old server forgotten).
    Upkept,
    /// A purge retry ran; what is still owed.
    Purged(Vec<String>),
    /// Sign-out's first step: the saved session is gone from this device (or the keychain
    /// refused). The server is told afterwards.
    SignedOutLocal { everywhere: bool, local: Result<(), ApiError> },
    Restored(Result<Me, ApiError>),
    SignedIn(Result<Me, ApiError>),
    SignedUp(Result<(), ApiError>),
    Resent(Result<(), ApiError>),
    Confirmed(Result<Confirmed, ApiError>),
    Forgot(Result<(), ApiError>),
    Reset(Result<Confirmed, ApiError>),
    /// The account, and whether the code was for an account other than the one signed in with.
    Enrolled(Result<(Me, bool), ApiError>),
    Me(Result<Me, ApiError>),
    SignedOut { everywhere: bool, result: Result<(), ApiError> },
    Deleted(Result<(), ApiError>),
    Reapplied(Result<(), ApiError>),
}

/// How jobs run. `Background`: a thread natively, `spawn_local` in the browser. `Inline`: to
/// completion inside the call (tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runner {
    Background,
    Inline,
}

/// Builds the client for a server address: the real transport and vault, or a test's mock.
/// Returns the notice to show when sign-in will not be remembered (no keychain). Called on a
/// job thread, never the UI thread: opening the keychain can block on an unlock prompt.
pub type Factory = Arc<dyn Fn(&str) -> (Client, Option<&'static str>) + Send + Sync>;

/// Where `share.json` is kept: the app's store, or a shared value standing in for the file
/// (tests edit it the way someone at the keyboard could edit the file).
#[derive(Clone, Debug)]
pub enum SettingsIo {
    Store,
    Memory(Arc<Mutex<ShareSettings>>),
}

impl SettingsIo {
    pub fn memory() -> SettingsIo {
        SettingsIo::Memory(Arc::default())
    }

    fn load(&self) -> ShareSettings {
        match self {
            SettingsIo::Store => ShareSettings::load(),
            SettingsIo::Memory(m) => ShareSettings::checked(m.lock().unwrap_or_else(|p| p.into_inner()).clone()),
        }
    }

    fn save(&self, s: &ShareSettings) -> Result<(), String> {
        match self {
            SettingsIo::Store => s.save(),
            SettingsIo::Memory(m) => {
                *m.lock().unwrap_or_else(|p| p.into_inner()) = s.clone();
                Ok(())
            }
        }
    }
}

/// How this build finds its pool server. Decided when the app is built, never at run time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerMode {
    /// `VERSHION_POOL_URL` was set at build time (and checked by build.rs): the only server.
    BuiltIn(String),
    /// A development build (debug, or the `pool-custom-server` feature): the address is typed and
    /// each one must be confirmed before a password is sent to it.
    Custom,
    /// A release build without a built-in server: sharing is off, no password page exists.
    NotSetUp,
}

/// Typed server addresses exist only in debug builds or with the `pool-custom-server` feature.
pub const CUSTOM_SERVER_ALLOWED: bool = cfg!(any(debug_assertions, feature = "pool-custom-server"));

/// The decision, as a pure function (tests cover every case in any build).
pub fn server_mode(built_in: Option<&str>, custom_allowed: bool) -> ServerMode {
    match built_in.filter(|b| !b.is_empty()) {
        // build.rs refuses an invalid address, so this never fails; if it somehow did, sharing
        // stays off rather than falling back to a typed address.
        Some(b) => api::validate_base_url(b).map(ServerMode::BuiltIn).unwrap_or(ServerMode::NotSetUp),
        None if custom_allowed => ServerMode::Custom,
        None => ServerMode::NotSetUp,
    }
}

/// After a confirmation link. Neutral on purpose: the server does not say whose address the link
/// was for, and a link someone else sent (for their own account) must not read as "yours is done".
pub const LINK_CONFIRMED: &str = "The address from this link is confirmed and its password is set. If you did not request \
     this on this device, ignore it. To use that account, sign in with its address and the new password.";
pub const LINK_RESET: &str = "The password of the account in this link is changed, and its devices were signed out. If you \
     did not request this on this device, ignore it. Otherwise sign in with the new password.";

const OTHER_ACCOUNT: &str = "That link was for a different account than the one signed in here. It was used for that \
     account only; your session here was not changed.";

/// Retry a failed keychain delete this often while the screen is open.
const PURGE_RETRY: Duration = Duration::from_secs(2);

/// What a job may send before it finishes (sign-out reports the local wipe at once).
type Early = Box<dyn Fn(Outcome) + Send>;
type ClientJob = Box<dyn FnOnce(Arc<Client>, Early) -> LocalFuture<'static, Outcome> + Send>;

pub struct AccountModel {
    pub open: bool,
    pub page: Page,
    pub forms: Forms,
    pub notice: Option<Notice>,
    pub me: Option<Me>,
    /// Sign-in was refused with `429`: offer the device code and the reset.
    pub lockdown: bool,
    /// The reset link belongs to an account never finished: the profile fields are required.
    pub reset_needs_profile: bool,
    /// Shown while sign-in is kept in memory only.
    pub keychain_note: Option<&'static str>,
    settings: Option<ShareSettings>,
    settings_io: SettingsIo,
    client: Option<Arc<Client>>,
    factory: Factory,
    runner: Runner,
    /// The running job (and the client generation it belongs to). One at a time.
    job: Option<(u64, Receiver<Outcome>)>,
    /// Keychain upkeep (purge retries, saving trust) beside the main job.
    side: Option<(u64, Receiver<Outcome>)>,
    /// Bumped whenever the client is replaced; results of an older client's job are dropped.
    client_gen: u64,
    server_mode: ServerMode,
    /// The person confirmed this server may receive their password (always, for a built-in one).
    pub trusted: bool,
    last_purge_try: Option<Instant>,
    /// Token from a verify or reset link, or the enrolment code.
    link_token: Option<Secret>,
    /// The page a link asked for (and the link's host), shown once the client is ready.
    link_page: Option<(Page, Option<String>)>,
    pub poller: Poller,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for AccountModel {
    fn default() -> Self {
        AccountModel::new(default_factory(), Runner::Background, SettingsIo::Store)
            .with_server_mode(server_mode(ShareSettings::built_in().as_deref(), CUSTOM_SERVER_ALLOWED))
    }
}

/// The real client: ureq or fetch, the keychain (native) or memory (browser), and the
/// cross-process refresh lock.
pub fn default_factory() -> Factory {
    Arc::new(|base: &str| {
        let (vault, note) = super::vault::open_default(base);
        #[cfg(not(target_arch = "wasm32"))]
        let transport: Box<dyn super::transport::Transport> = Box::new(super::transport::NativeTransport::default());
        #[cfg(target_arch = "wasm32")]
        let transport: Box<dyn super::transport::Transport> = Box::new(super::transport::WebTransport);
        let client = Client::new(base, Mode::current(), transport, vault);
        #[cfg(not(target_arch = "wasm32"))]
        let client = client.with_lock(Box::new(super::vault::FileLock::new(crate::store::store_dir().join("share-refresh.lock"))));
        #[cfg(target_arch = "wasm32")]
        let client = client.with_lock(Box::new(super::vault::WebLock));
        (client, note)
    })
}

impl AccountModel {
    /// Does no I/O: settings, keychain and network wait until the screen opens. Starts in
    /// [`ServerMode::Custom`] (tests); the app uses `Default`, which decides from the build.
    pub fn new(factory: Factory, runner: Runner, settings_io: SettingsIo) -> AccountModel {
        AccountModel {
            open: false,
            page: Page::NotConfigured,
            forms: Forms::default(),
            notice: None,
            me: None,
            lockdown: false,
            reset_needs_profile: false,
            keychain_note: None,
            settings: None,
            settings_io,
            client: None,
            factory,
            runner,
            job: None,
            side: None,
            client_gen: 0,
            server_mode: ServerMode::Custom,
            trusted: false,
            last_purge_try: None,
            link_token: None,
            link_page: None,
            poller: Poller::default(),
            repaint: None,
        }
    }

    pub fn with_server_mode(mut self, mode: ServerMode) -> AccountModel {
        self.server_mode = mode;
        self
    }

    /// Shorthand for a built-in server (tests).
    pub fn with_built_in(self, base: Option<String>) -> AccountModel {
        let mode = server_mode(base.as_deref(), false);
        self.with_server_mode(mode)
    }

    pub fn server_mode(&self) -> &ServerMode {
        &self.server_mode
    }

    /// True unless addresses are typed: no "Change" anywhere.
    pub fn server_locked(&self) -> bool {
        self.server_mode != ServerMode::Custom
    }

    /// A development build talking to a typed address.
    pub fn custom_server(&self) -> bool {
        self.server_mode == ServerMode::Custom
    }

    pub fn set_repaint(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.repaint = Some(f);
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The server in use: the built-in one, else (development builds) the saved one.
    pub fn base_url(&self) -> &str {
        match &self.server_mode {
            ServerMode::BuiltIn(b) => b,
            ServerMode::Custom => self.settings.as_ref().map_or("", |s| s.base_url.as_str()),
            ServerMode::NotSetUp => "",
        }
    }

    pub fn mode(&self) -> Mode {
        self.client.as_ref().map_or(Mode::current(), |c| c.mode())
    }

    pub fn client(&self) -> Option<&Arc<Client>> {
        self.client.as_ref()
    }

    fn settings(&mut self) -> &ShareSettings {
        if self.settings.is_none() {
            self.settings = Some(self.settings_io.load());
        }
        self.settings.as_ref().expect("just set")
    }

    /// Records in `share.json` which saved sign-ins of this server still have to be deleted, so
    /// the next run deletes them (and never uses them) even if this one ends first. Entries of
    /// other servers are kept for when the app talks to them again.
    fn sync_purge(&mut self) {
        let Some(c) = &self.client else { return };
        let server = c.base().to_string();
        let pending: Vec<PurgeEntry> =
            c.pending_purge().into_iter().map(|account| PurgeEntry { server: server.clone(), account }).collect();
        let s = self.settings.get_or_insert_with(ShareSettings::default);
        let mut next: Vec<PurgeEntry> = s.purge.iter().filter(|e| e.server != server).cloned().collect();
        next.extend(pending);
        if s.purge != next {
            s.purge = next;
            let copy = s.clone();
            let _ = self.settings_io.save(&copy);
        }
    }

    /// The person confirmed the server shown may receive their password (development builds).
    pub fn trust_server(&mut self) {
        if !self.custom_server() {
            return;
        }
        if let Some(c) = self.client.clone() {
            self.trusted = true;
            self.notice = None;
            self.spawn_side(c, Box::new(|c, _| Box::pin(async move {
                c.set_trusted();
                Outcome::Upkept
            })));
        }
    }

    /// Password pages refuse to send anything to a server the person has not confirmed.
    fn credentials_allowed(&mut self) -> bool {
        if self.server_mode == ServerMode::NotSetUp {
            self.notice = Some(Notice::Error(NOT_SET_UP.into()));
            return false;
        }
        if self.trusted {
            return true;
        }
        self.notice = Some(Notice::Error(format!(
            "Confirm the server first: your password would be sent to {}.",
            api::server_label(self.base_url())
        )));
        false
    }

    // ---- opening, closing, pages -------------------------------------------------------

    /// Opens the screen. The client (and with it the keychain) is set up on a job thread; the
    /// screen shows "Opening" meanwhile. With a saved session it is then checked (one request);
    /// without one, sign-in shows and nothing is sent (the browser asks its cookie once).
    pub fn open(&mut self) {
        if self.open {
            return;
        }
        self.open = true;
        self.notice = None;
        if self.server_mode == ServerMode::NotSetUp {
            self.page = Page::NotSetUp;
            return;
        }
        // Read share.json again: it may have changed while the screen was closed.
        if self.custom_server() {
            self.settings = None;
        }
        self.settings();
        let base = self.base_url().to_string();
        if base.is_empty() {
            self.page = Page::NotConfigured;
            return;
        }
        match &self.client {
            Some(c) if c.base() == base => self.after_open(),
            _ => self.start_open(base),
        }
    }

    /// Builds the client for `base` on a job thread (keychain I/O), retiring the old one.
    fn start_open(&mut self, base: String) {
        if let Some(old) = self.client.take() {
            old.retire();
        }
        self.client_gen += 1;
        self.me = None;
        self.trusted = false;
        self.page = Page::Opening;
        let pending: Vec<String> = self
            .settings
            .as_ref()
            .map(|s| s.purge.iter().filter(|e| e.server == base).map(|e| e.account.clone()).collect())
            .unwrap_or_default();
        let (factory, check_trust) = (self.factory.clone(), self.custom_server());
        let (tx, rx) = channel();
        self.job = Some((self.client_gen, rx));
        let repaint = self.repaint.clone();
        let work = move || {
            let (client, note) = factory(&base);
            // A sign-out whose keychain delete failed last time is finished before anything else.
            if !pending.is_empty() {
                client.purge(&pending);
            }
            let trusted = !check_trust || client.is_trusted();
            let _ = tx.send(Outcome::Opened { client: Arc::new(client), note, trusted });
            if let Some(r) = repaint {
                r();
            }
        };
        match self.runner {
            Runner::Inline => work(),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(work);
            }
            #[cfg(target_arch = "wasm32")]
            Runner::Background => wasm_bindgen_futures::spawn_local(async move { work() }),
        }
    }

    /// The client is ready: show what a link asked for, or check a saved session, or sign-in.
    fn after_open(&mut self) {
        let Some(client) = self.client.clone() else { return };
        if let Some((page, host)) = self.link_page.take()
            && self.link_allowed(host.as_deref())
        {
            self.go(page);
            return;
        }
        if self.busy() {
            return;
        }
        if client.has_session() || client.mode() == Mode::Web {
            self.page = Page::Status;
            self.spawn(Box::new(|c, _| Box::pin(async move { Outcome::Restored(c.restore().await) })));
        } else {
            self.page = Page::SignIn;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.poller.stop();
        self.forms.wipe_secrets();
    }

    pub fn go(&mut self, page: Page) {
        if page != self.page {
            self.forms.wipe_secrets();
            self.notice = None;
            if !matches!(page, Page::SignIn | Page::Enrol | Page::Forgot) {
                self.lockdown = false;
            }
        }
        self.page = page;
    }

    /// A mailed link opened the app (web deep link) or was pasted.
    pub fn open_link(&mut self, link: Link) {
        if self.server_mode == ServerMode::NotSetUp {
            self.open();
            return;
        }
        let kind = link.kind;
        let host = link.host.clone();
        if self.open && self.client.is_some() && !self.link_allowed(host.as_deref()) {
            return;
        }
        self.link_token = Some(link.token);
        self.reset_needs_profile = false;
        let page = match kind {
            Some(LinkKind::Verify) => Page::Confirm,
            Some(LinkKind::Reset) => Page::Reset,
            Some(LinkKind::Enrol) => Page::Enrol,
            None => match self.page {
                Page::CheckEmail(MailKind::Verify) => Page::Confirm,
                Page::CheckEmail(MailKind::Reset) => Page::Reset,
                _ => Page::Enrol,
            },
        };
        if self.open && self.client.is_some() {
            self.go(page);
        } else {
            // Not open or not ready yet: the page waits for the client.
            self.link_page = Some((page, host));
            self.open = false;
            self.open();
        }
    }

    /// A link never changes the server, and one whose host does not belong with the server is
    /// not used at all (it may come from a look-alike site).
    fn link_allowed(&mut self, link_host: Option<&str>) -> bool {
        let Some(h) = link_host else { return true };
        let api_host = super::transport::host_of(self.base_url());
        if api::link_host_matches(h, &api_host) {
            return true;
        }
        self.link_token = None;
        self.notice = Some(Notice::Error(format!(
            "This link points to {h}, which does not belong to the pool server this app uses ({}). It was not used. \
             If you expected it, check the address the email came from.",
            api::server_label(self.base_url())
        )));
        false
    }

    /// The "paste the link or code" box.
    pub fn submit_paste(&mut self) {
        match api::parse_link(&self.forms.paste) {
            Some(link) => {
                use zeroize::Zeroize;
                self.forms.paste.zeroize();
                self.open_link(link);
            }
            None => {
                self.notice = Some(Notice::Error(
                    "That doesn't look like a link from a verSHion email. Copy the whole link, or just the long code after token=."
                        .into(),
                ))
            }
        }
    }

    pub fn has_link_token(&self) -> bool {
        self.link_token.is_some()
    }

    // ---- server address (development builds only) -------------------------------------

    pub fn save_server(&mut self) {
        if self.server_locked() {
            self.notice = Some(Notice::Error("This copy of verSHion is set up for one server, which can't be changed.".into()));
            return;
        }
        if self.busy() {
            self.notice = Some(Notice::Error("Wait for the current request to finish first.".into()));
            return;
        }
        match api::validate_base_url(&self.forms.server) {
            Ok(base) => {
                if base != self.base_url() {
                    if let Some(c) = self.client.take() {
                        // A new address is never trusted until confirmed again; the old server's
                        // session on this device goes (on a job thread: keychain I/O).
                        c.retire();
                        self.spawn_side(c, Box::new(|c, _| Box::pin(async move {
                            c.clear_trusted();
                            c.forget_local();
                            Outcome::Upkept
                        })));
                    }
                    self.client_gen += 1;
                    self.trusted = false;
                    self.me = None;
                }
                let mut s = self.settings.clone().unwrap_or_default();
                s.base_url = base;
                if let Err(e) = self.settings_io.save(&s) {
                    self.notice = Some(Notice::Error(format!("Couldn't save the server address: {e}")));
                    return;
                }
                self.settings = Some(s);
                self.open = false;
                self.open();
            }
            Err(e) => self.notice = Some(Notice::Error(e)),
        }
    }

    pub fn edit_server(&mut self) {
        if self.server_locked() || self.busy() {
            return;
        }
        self.forms.server = self.base_url().to_string();
        self.go(Page::Server);
    }

    // ---- jobs --------------------------------------------------------------------------

    fn run(&self, client: Arc<Client>, job: ClientJob) -> Receiver<Outcome> {
        let (tx, rx) = channel();
        let repaint = self.repaint.clone();
        let early_tx = tx.clone();
        let early_repaint = repaint.clone();
        let early: Early = Box::new(move |out: Outcome| {
            let _ = early_tx.send(out);
            if let Some(r) = &early_repaint {
                r();
            }
        });
        let done = move |out: Outcome| {
            let _ = tx.send(out);
            if let Some(r) = repaint {
                r();
            }
        };
        match self.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => done(super::transport::block_on(job(client, early))),
            #[cfg(target_arch = "wasm32")]
            Runner::Inline => wasm_bindgen_futures::spawn_local(async move { done(job(client, early).await) }),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || done(super::transport::block_on(job(client, early))));
            }
            #[cfg(target_arch = "wasm32")]
            Runner::Background => wasm_bindgen_futures::spawn_local(async move { done(job(client, early).await) }),
        }
        rx
    }

    fn spawn(&mut self, job: ClientJob) {
        let Some(client) = self.client.clone() else {
            self.notice = Some(Notice::Error(ApiError::NotConfigured.message(Flow::Status)));
            return;
        };
        let rx = self.run(client, job);
        self.job = Some((self.client_gen, rx));
    }

    fn spawn_side(&mut self, client: Arc<Client>, job: ClientJob) {
        let rx = self.run(client, job);
        self.side = Some((self.client_gen, rx));
    }

    /// Takes every message a lane has ready; the lane ends with its last one.
    fn drain(lane: &mut Option<(u64, Receiver<Outcome>)>, current: u64) -> Vec<Outcome> {
        let mut out = Vec::new();
        let Some((generation, rx)) = lane else { return out };
        let mine = *generation == current;
        loop {
            match rx.try_recv() {
                Ok(o) => {
                    if mine {
                        out.push(o);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    *lane = None;
                    break;
                }
            }
        }
        out
    }

    /// Called every frame while the screen is open: applies finished jobs, retries a failed
    /// keychain delete, and polls the approval status when due. Does nothing (no I/O of any
    /// kind) when the screen is closed.
    pub fn frame(&mut self, now: Instant) {
        if !self.open {
            return;
        }
        let main_was_running = self.job.is_some();
        for out in AccountModel::drain(&mut self.job, self.client_gen) {
            self.apply(out, now);
        }
        if main_was_running && self.job.as_ref().is_none() {
            self.sync_purge();
        }
        for out in AccountModel::drain(&mut self.side, self.client_gen) {
            self.apply(out, now);
        }
        // Keychain deletes still owed: retried on the side lane, never on this (UI) thread.
        let pending = self.client.as_ref().is_some_and(|c| !c.pending_purge().is_empty());
        if pending && self.side.is_none() && self.last_purge_try.is_none_or(|t| now.saturating_duration_since(t) >= PURGE_RETRY) {
            self.last_purge_try = Some(now);
            if let Some(c) = self.client.clone() {
                self.spawn_side(c, Box::new(|c, _| Box::pin(async move { Outcome::Purged(c.retry_purge()) })));
                for out in AccountModel::drain(&mut self.side, self.client_gen) {
                    self.apply(out, now);
                }
            }
        }
        if self.job.is_none() && self.page == Page::Status && self.poller.due(now) {
            self.poller.stop();
            self.spawn(Box::new(|c, _| Box::pin(async move { Outcome::Me(c.me().await) })));
        }
    }

    fn fail(&mut self, e: &ApiError, flow: Flow) {
        self.notice = Some(Notice::Error(e.message(flow)));
    }

    fn signed_in(&mut self, me: Me, now: Instant) {
        self.forms.wipe_secrets();
        self.poller.after_success(now, &me.status);
        self.me = Some(me);
        self.lockdown = false;
        self.page = Page::Status;
    }

    fn apply(&mut self, out: Outcome, now: Instant) {
        let session = self.client.as_ref().is_some_and(|c| c.has_session());
        match out {
            Outcome::Opened { client, note, trusted } => {
                self.keychain_note = note;
                self.trusted = trusted;
                self.client = Some(client);
                self.job = None;
                self.sync_purge();
                if self.open {
                    self.after_open();
                }
            }
            Outcome::Upkept => {}
            Outcome::Purged(left) => {
                self.sync_purge();
                if left.is_empty() && matches!(self.notice, Some(Notice::Error(ref t)) if t == &ApiError::LocalCleanup.message(Flow::SignOut)) {
                    self.notice = Some(Notice::Info("Signed out. The saved sign-in on this computer is now deleted.".into()));
                }
            }
            Outcome::SignedOutLocal { everywhere, local } => {
                self.me = None;
                self.poller.stop();
                self.go(Page::SignIn);
                self.notice = Some(match local {
                    Err(e) => Notice::Error(e.message(Flow::SignOut)),
                    Ok(()) if everywhere => Notice::Info("Signed out here. Signing out your other devices…".into()),
                    Ok(()) => Notice::Info("Signed out.".into()),
                });
                self.sync_purge();
                if !everywhere {
                    // Telling the server is best effort and nobody waits for it.
                    self.job = None;
                }
            }
            Outcome::Restored(Ok(me)) | Outcome::Me(Ok(me)) => {
                self.poller.after_success(now, &me.status);
                self.me = Some(me);
                if self.page == Page::SignIn {
                    self.page = Page::Status;
                }
            }
            Outcome::Restored(Err(ApiError::SignedOut)) => {
                self.me = None;
                self.page = Page::SignIn;
            }
            Outcome::Restored(Err(e)) | Outcome::Me(Err(e)) => {
                if matches!(e, ApiError::SignedOut) || !session {
                    self.me = None;
                    self.go(Page::SignIn);
                    if !matches!(e, ApiError::SignedOut) {
                        self.fail(&e, Flow::Status);
                    } else {
                        self.notice = Some(Notice::Info("Your session ended. Sign in again.".into()));
                    }
                } else {
                    self.poller.after_failure(now);
                    self.fail(&e, Flow::Status);
                }
            }
            Outcome::SignedIn(Ok(me)) => self.signed_in(me, now),
            Outcome::Enrolled(Ok((me, other))) => {
                let email = me.email.clone();
                self.signed_in(me, now);
                if other {
                    self.notice = Some(Notice::Info(format!(
                        "Signed in as {email}, but that device link was for a different account, so it did not \
                         change anything for {email}."
                    )));
                }
            }
            Outcome::SignedIn(Err(e)) => {
                self.lockdown = e.is_rate_limited();
                self.fail(&e, Flow::SignIn);
            }
            Outcome::Enrolled(Err(e)) => self.fail(&e, Flow::Enrol),
            Outcome::SignedUp(Ok(())) => {
                self.forms.wipe_secrets();
                self.go(Page::CheckEmail(MailKind::Verify));
                self.notice = Some(Notice::Info(
                    "If that address can receive mail, a confirmation link is on its way. It works once, for 24 hours.".into(),
                ));
            }
            Outcome::SignedUp(Err(e)) => self.fail(&e, Flow::SignUp),
            Outcome::Resent(Ok(())) => {
                self.notice = Some(Notice::Info("If that address has an unconfirmed sign-up, another link is on its way.".into()))
            }
            Outcome::Resent(Err(e)) => self.fail(&e, Flow::Resend),
            Outcome::Confirmed(Ok(c)) => {
                self.link_token = None;
                if c.other_account {
                    self.notice = Some(Notice::Info(OTHER_ACCOUNT.into()));
                    return;
                }
                self.me = None;
                self.go(Page::SignIn);
                let _ = c.device_known;
                self.notice = Some(Notice::Info(LINK_CONFIRMED.into()));
            }
            Outcome::Confirmed(Err(e)) => {
                if e.code() == "invalid_token" {
                    self.link_token = None;
                }
                self.fail(&e, Flow::Confirm);
            }
            Outcome::Forgot(Ok(())) => {
                self.go(Page::CheckEmail(MailKind::Reset));
                self.notice = Some(Notice::Info(
                    "If that address has an account, a reset link is on its way. It works once, for 1 hour.".into(),
                ));
            }
            Outcome::Forgot(Err(e)) => self.fail(&e, Flow::Forgot),
            Outcome::Reset(Ok(c)) if c.other_account => {
                self.link_token = None;
                self.reset_needs_profile = false;
                self.go(Page::Status);
                self.notice = Some(Notice::Info(OTHER_ACCOUNT.into()));
            }
            Outcome::Reset(Ok(_)) => {
                self.link_token = None;
                self.reset_needs_profile = false;
                self.me = None;
                self.poller.stop();
                self.go(Page::SignIn);
                self.notice = Some(Notice::Info(LINK_RESET.into()));
            }
            Outcome::Reset(Err(e)) => {
                if e.needs_profile() && !self.reset_needs_profile {
                    self.reset_needs_profile = true;
                    self.notice = Some(Notice::Info(
                        "This account was never finished. Add your name and school below to finish it.".into(),
                    ));
                } else {
                    if e.code() == "invalid_token" {
                        self.link_token = None;
                    }
                    self.fail(&e, Flow::Reset);
                }
            }
            Outcome::Deleted(Err(ApiError::LocalCleanup)) => {
                // Never "signed out" while the saved sign-in is still on this computer.
                self.me = None;
                self.poller.stop();
                self.go(Page::SignIn);
                self.notice = Some(Notice::Error(format!(
                    "Your account was deleted on the server. {}",
                    ApiError::LocalCleanup.message(Flow::SignOut)
                )));
            }
            // The local wipe was reported already (SignedOutLocal); only "everywhere" waits for the server.
            Outcome::SignedOut { everywhere: false, .. } => {}
            Outcome::SignedOut { everywhere: true, result } => {
                if matches!(self.notice, Some(Notice::Error(_))) {
                    // The local wipe failed; that message stays.
                } else {
                    self.notice = Some(match result {
                        Ok(()) => Notice::Info("Signed out on every device.".into()),
                        Err(e) => Notice::Error(format!(
                            "This device is signed out, but your other devices could not be signed out: {} \
                             Sign in again here and choose \"Sign out everywhere\" to retry.",
                            e.message(Flow::SignOutEverywhere)
                        )),
                    });
                }
            }
            Outcome::Deleted(Ok(())) => {
                self.me = None;
                self.poller.stop();
                self.go(Page::SignIn);
                self.notice = Some(Notice::Info("Your account was deleted and you were signed out.".into()));
            }
            Outcome::Deleted(Err(e)) => {
                if matches!(e, ApiError::SignedOut) {
                    self.me = None;
                    self.go(Page::SignIn);
                }
                self.fail(&e, Flow::Delete);
            }
            Outcome::Reapplied(Ok(())) => {
                self.notice = Some(Notice::Info("Your application was sent again.".into()));
                self.spawn(Box::new(|c, _| Box::pin(async move { Outcome::Me(c.me().await) })));
            }
            Outcome::Reapplied(Err(e)) => self.fail(&e, Flow::Reapply),
        }
    }

    // ---- form submissions (each starts one job) ----------------------------------------

    fn ready(&mut self) -> bool {
        if self.busy() {
            return false;
        }
        self.notice = None;
        true
    }

    fn need(&mut self, problem: Option<impl Into<String>>) -> bool {
        match problem {
            Some(p) => {
                self.notice = Some(Notice::Error(p.into()));
                false
            }
            None => true,
        }
    }

    fn email_problem(&self) -> Option<&'static str> {
        let e = self.forms.email.trim();
        (e.is_empty() || !e.contains('@')).then_some("Enter your email address.")
    }

    fn new_password(&self) -> Option<String> {
        let p = Secret::new(self.forms.password.clone());
        if let Some(problem) = api::password_problem(&p) {
            return Some(problem);
        }
        (self.forms.password != self.forms.password2).then(|| "The two passwords are not the same.".to_string())
    }

    pub fn submit_sign_in(&mut self) {
        if !self.ready() || !self.credentials_allowed() || !self.need(self.email_problem()) {
            return;
        }
        if self.forms.password.is_empty() {
            self.notice = Some(Notice::Error("Enter your password.".into()));
            return;
        }
        let email = self.forms.email.clone();
        let password = Secret::new(self.forms.password.clone());
        self.spawn(Box::new(move |c, _| {
            Box::pin(async move {
                let r = match c.sign_in(&email, &password).await {
                    Ok(()) => c.me().await,
                    Err(e) => Err(e),
                };
                Outcome::SignedIn(r)
            })
        }));
    }

    pub fn submit_sign_up(&mut self) {
        if !self.ready() || !self.credentials_allowed() || !self.need(self.email_problem()) {
            return;
        }
        let pw = Secret::new(self.forms.password.clone());
        if !self.need(api::password_problem(&pw)) || !self.need(self.forms.profile().problem()) {
            return;
        }
        let (email, profile) = (self.forms.email.clone(), self.forms.profile());
        self.spawn(Box::new(move |c, _| Box::pin(async move { Outcome::SignedUp(c.sign_up(&email, &pw, &profile).await) })));
    }

    pub fn submit_resend(&mut self) {
        if !self.ready() || !self.need(self.email_problem()) {
            return;
        }
        let email = self.forms.email.clone();
        self.spawn(Box::new(move |c, _| Box::pin(async move { Outcome::Resent(c.resend(&email).await) })));
    }

    pub fn submit_confirm(&mut self) {
        if !self.ready() || !self.credentials_allowed() {
            return;
        }
        let Some(token) = self.link_token.clone() else {
            self.notice = Some(Notice::Error("Paste the link from the confirmation email first.".into()));
            return;
        };
        if !self.need(self.new_password()) || !self.need(self.forms.profile().problem()) {
            return;
        }
        let (pw, profile) = (Secret::new(self.forms.password.clone()), self.forms.profile());
        self.spawn(Box::new(move |c, _| Box::pin(async move { Outcome::Confirmed(c.confirm(&token, &pw, &profile).await) })));
    }

    pub fn submit_forgot(&mut self) {
        if !self.ready() || !self.need(self.email_problem()) {
            return;
        }
        let email = self.forms.email.clone();
        self.spawn(Box::new(move |c, _| Box::pin(async move { Outcome::Forgot(c.forgot(&email).await) })));
    }

    pub fn submit_reset(&mut self) {
        if !self.ready() || !self.credentials_allowed() {
            return;
        }
        let Some(token) = self.link_token.clone() else {
            self.notice = Some(Notice::Error("Paste the link from the reset email first.".into()));
            return;
        };
        if !self.need(self.new_password()) {
            return;
        }
        let profile = if self.reset_needs_profile {
            if !self.need(self.forms.profile().problem()) {
                return;
            }
            Some(self.forms.profile())
        } else {
            None
        };
        let pw = Secret::new(self.forms.password.clone());
        self.spawn(Box::new(move |c, _| Box::pin(async move { Outcome::Reset(c.reset(&token, &pw, profile.as_ref()).await) })));
    }

    /// Redeems the code, then signs in with it (the email is the sign-in form's).
    pub fn submit_enrol(&mut self) {
        if !self.ready() || !self.credentials_allowed() {
            return;
        }
        if !self.forms.paste.trim().is_empty() {
            match api::parse_link(&self.forms.paste) {
                Some(l) => {
                    use zeroize::Zeroize;
                    self.forms.paste.zeroize();
                    self.link_token = Some(l.token);
                }
                None => {
                    self.notice = Some(Notice::Error("That code is not complete. Copy the whole link from the email.".into()));
                    return;
                }
            }
        }
        let Some(code) = self.link_token.clone() else {
            self.notice = Some(Notice::Error("Paste the link or code from the email.".into()));
            return;
        };
        if !self.need(self.email_problem()) {
            return;
        }
        if self.forms.password.is_empty() {
            self.notice = Some(Notice::Error("Enter your password.".into()));
            return;
        }
        let (email, pw) = (self.forms.email.clone(), Secret::new(self.forms.password.clone()));
        self.spawn(Box::new(move |c, _| {
            Box::pin(async move {
                let r = async {
                    let enrolled = c.enrol(&code, &pw).await?;
                    c.sign_in(&email, &pw).await?;
                    let other = enrolled.is_some_and(|a| c.account().as_deref() != Some(a.as_str()));
                    Ok((c.me().await?, other))
                }
                .await;
                Outcome::Enrolled(r)
            })
        }));
    }

    pub fn check_now(&mut self) {
        if !self.ready() {
            return;
        }
        self.poller.stop();
        self.spawn(Box::new(|c, _| Box::pin(async move { Outcome::Me(c.me().await) })));
    }

    /// Signing out works even while another request runs (a refresh may be stuck waiting for a
    /// keychain or another copy of the app): that request is abandoned, and the session leaves
    /// memory right here, so whatever it brings back is not kept. The keychain wipe runs next, on
    /// a job thread, before the server is told.
    pub fn submit_sign_out(&mut self, everywhere: bool) {
        let Some(client) = self.client.clone() else { return };
        self.job = None;
        self.notice = None;
        let forget = self.forms.forget_device;
        let s = client.begin_sign_out(forget);
        self.me = None;
        self.poller.stop();
        self.spawn(Box::new(move |c, early| {
            Box::pin(async move {
                let local = c.wipe_signed_out(&s, everywhere);
                early(Outcome::SignedOutLocal { everywhere, local });
                Outcome::SignedOut { everywhere, result: c.tell_server_signed_out(s, everywhere).await }
            })
        }));
    }

    pub fn submit_reapply(&mut self) {
        if !self.ready() || !self.need(self.forms.profile().problem()) {
            return;
        }
        let profile = self.forms.profile();
        self.spawn(Box::new(move |c, _| Box::pin(async move { Outcome::Reapplied(c.reapply(&profile).await) })));
    }

    pub fn submit_delete(&mut self) {
        if !self.ready() || !self.credentials_allowed() {
            return;
        }
        if !self.forms.delete_ack {
            self.notice = Some(Notice::Error("Tick the box to confirm that the account should be deleted.".into()));
            return;
        }
        if self.forms.password.is_empty() {
            self.notice = Some(Notice::Error("Enter your password to delete the account.".into()));
            return;
        }
        let pw = Secret::new(self.forms.password.clone());
        self.forms.delete_ack = false;
        self.spawn(Box::new(move |c, _| Box::pin(async move { Outcome::Deleted(c.delete_account(&pw).await) })));
    }
}
