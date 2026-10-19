//! Where the native app keeps its shared-pool tokens between runs, and the lock that keeps two
//! running copies of the app from refreshing the same token.
//!
//! Only the OS keychain, through the `keyring` crate: the macOS Keychain, the Windows Credential
//! Manager, or a Secret Service on Linux (GNOME Keyring, KWallet's Secret Service, KeePassXC). On
//! Linux a desktop session normally runs one; a bare window manager or a headless box may not.
//!
//! When no keychain is usable the app does NOT fall back to a file. It keeps the tokens in memory
//! for this run only and says so ([`KEYCHAIN_MISSING`]); the person signs in again next time. The
//! browser build never stores anything here: its refresh and device tokens are HttpOnly cookies
//! and its access token lives in memory only.
//!
//! Entries are per server AND per account (`refresh:<account id>@<server>`), so a token issued
//! for one account can never overwrite another account's. The access token is never put in a
//! vault on any platform.

use std::collections::HashMap;
use std::sync::Mutex;

use super::api::ApiError;
use super::secret::Secret;
use super::transport::LocalFuture;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    /// Which account holds this server's session, and which address belongs to which account id
    /// (JSON, see `client::Index`). Not a credential, but private, so it lives with them.
    Index,
    /// Rotating refresh token of an account's session.
    Refresh(String),
    /// Known-device token (`X-Device-Token`) of an account; survives a plain sign-out.
    Device(String),
    /// The one server address the person confirmed they trust with their password (independent
    /// of the server: there is one entry, for whichever address was confirmed last).
    TrustedServer,
}

impl Slot {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn key(&self, base: &str) -> String {
        match self {
            Slot::Index => format!("index@{base}"),
            Slot::Refresh(a) => format!("refresh:{a}@{base}"),
            Slot::Device(a) => format!("device:{a}@{base}"),
            Slot::TrustedServer => "trusted-server".into(),
        }
    }
}

/// Why a vault call failed. Carries no token text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultError(pub String);

pub trait Vault: Send + Sync {
    fn load(&self, slot: &Slot) -> Result<Option<Secret>, VaultError>;
    fn save(&self, slot: &Slot, value: &Secret) -> Result<(), VaultError>;
    /// Deleting something that is not there is success.
    fn clear(&self, slot: &Slot) -> Result<(), VaultError>;
    /// Whether what is saved survives a restart.
    fn persistent(&self) -> bool;
}

pub const KEYCHAIN_MISSING: &str = "No keychain is available on this computer, so you will stay signed in only until you \
     close verSHion. (On Linux, sign-in is remembered when a Secret Service such as GNOME Keyring or \
     KWallet is running.) Nothing is written to a file instead.";

pub const KEYCHAIN_FAILED: &str = "The keychain refused to save your sign-in, so you will stay signed in only until you \
     close verSHion. Nothing was written to a file instead.";

/// Memory only: tests, the browser build, and the native fallback when there is no keychain.
#[derive(Default)]
pub struct MemoryVault {
    slots: Mutex<HashMap<Slot, Secret>>,
}

impl Vault for MemoryVault {
    fn load(&self, slot: &Slot) -> Result<Option<Secret>, VaultError> {
        Ok(self.slots.lock().unwrap_or_else(|p| p.into_inner()).get(slot).cloned())
    }
    fn save(&self, slot: &Slot, value: &Secret) -> Result<(), VaultError> {
        self.slots.lock().unwrap_or_else(|p| p.into_inner()).insert(slot.clone(), value.clone());
        Ok(())
    }
    fn clear(&self, slot: &Slot) -> Result<(), VaultError> {
        self.slots.lock().unwrap_or_else(|p| p.into_inner()).remove(slot);
        Ok(())
    }
    fn persistent(&self) -> bool {
        false
    }
}

impl std::fmt::Debug for MemoryVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let held: Vec<Slot> = self.slots.lock().map(|s| s.keys().cloned().collect()).unwrap_or_default();
        f.debug_struct("MemoryVault").field("holds", &held).finish()
    }
}

/// The OS keychain.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct KeyringVault {
    base: String,
}

#[cfg(not(target_arch = "wasm32"))]
const KEYRING_SERVICE: &str = "verSHion shared pool";

#[cfg(not(target_arch = "wasm32"))]
impl KeyringVault {
    fn entry(&self, slot: &Slot) -> Result<keyring::Entry, VaultError> {
        keyring::Entry::new(KEYRING_SERVICE, &slot.key(&self.base)).map_err(kind)
    }

    /// Opens the keychain for `base`, or says why it can't. A missing entry is not a failure.
    pub fn open(base: &str) -> Result<KeyringVault, VaultError> {
        let v = KeyringVault { base: base.to_string() };
        match v.entry(&Slot::Index)?.get_password() {
            Ok(mut s) => {
                zeroize::Zeroize::zeroize(&mut s);
                Ok(v)
            }
            Err(keyring::Error::NoEntry) => Ok(v),
            Err(e) => Err(kind(e)),
        }
    }
}

/// The keyring error's class only (some platform messages are long; none hold the secret).
#[cfg(not(target_arch = "wasm32"))]
fn kind(e: keyring::Error) -> VaultError {
    VaultError(
        match e {
            keyring::Error::PlatformFailure(_) => "keychain platform failure",
            keyring::Error::NoStorageAccess(_) => "no keychain access",
            keyring::Error::NoEntry => "no entry",
            keyring::Error::BadEncoding(_) => "bad encoding",
            keyring::Error::TooLong(..) => "value too long",
            keyring::Error::Invalid(..) => "invalid entry",
            keyring::Error::Ambiguous(_) => "ambiguous entry",
            _ => "keychain error",
        }
        .to_string(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
impl Vault for KeyringVault {
    fn load(&self, slot: &Slot) -> Result<Option<Secret>, VaultError> {
        match self.entry(slot)?.get_password() {
            Ok(mut s) => Ok(Some(Secret::take(&mut s))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(kind(e)),
        }
    }
    fn save(&self, slot: &Slot, value: &Secret) -> Result<(), VaultError> {
        self.entry(slot)?.set_password(value.expose()).map_err(kind)
    }
    fn clear(&self, slot: &Slot) -> Result<(), VaultError> {
        match self.entry(slot)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(kind(e)),
        }
    }
    fn persistent(&self) -> bool {
        true
    }
}

/// The vault this build uses for `base`, plus the notice to show when sign-in will not be
/// remembered. Native: the keychain, else memory with [`KEYCHAIN_MISSING`]. Browser: memory (it
/// is never given a token to keep).
pub fn open_default(base: &str) -> (Box<dyn Vault>, Option<&'static str>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match KeyringVault::open(base) {
            Ok(v) => (Box::new(v), None),
            Err(e) => {
                log::warn!("share: keychain unavailable ({}); sign-in is kept in memory only", e.0);
                (Box::new(MemoryVault::default()), Some(KEYCHAIN_MISSING))
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = base;
        (Box::new(MemoryVault::default()), None)
    }
}

// ---- the refresh lock -------------------------------------------------------------------

/// Held while one process refreshes. Dropping it releases the lock (also on a panic).
pub struct LockGuard(#[allow(dead_code)] Box<dyn std::any::Any>);

impl LockGuard {
    pub fn new(inner: impl std::any::Any) -> LockGuard {
        LockGuard(Box::new(inner))
    }
}

/// Serialises refreshes across every running copy of the app (and every tab, in the browser), so
/// a refresh token rotated by one is never replayed by another: the holder re-reads the saved
/// token under the lock before using it.
pub trait SessionLock: Send + Sync {
    /// The lock, or [`ApiError::LockBusy`] when another copy holds it too long.
    fn acquire(&self) -> LocalFuture<'_, Result<LockGuard, ApiError>>;
}

/// No cross-process lock (one process, or tests of in-process behaviour). The client's own
/// single-flight still applies.
pub struct ProcessLocal;

impl SessionLock for ProcessLocal {
    fn acquire(&self) -> LocalFuture<'_, Result<LockGuard, ApiError>> {
        Box::pin(std::future::ready(Ok(LockGuard::new(()))))
    }
}

/// An advisory lock on a file (`flock` / `LockFileEx` through `std::fs::File::try_lock`). The
/// file is only opened when a refresh starts, never while the Account screen is closed. Another
/// copy of the app may hold it for a while (its keychain may be waiting for an unlock), so this
/// tries for [`FileLock::wait`] and then gives up with a retryable [`ApiError::LockBusy`]. If the
/// file can't be opened at all the refresh goes ahead with the in-process single-flight only.
#[cfg(not(target_arch = "wasm32"))]
pub struct FileLock {
    pub path: std::path::PathBuf,
    pub wait: std::time::Duration,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileLock {
    pub const DEFAULT_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

    pub fn new(path: std::path::PathBuf) -> FileLock {
        FileLock { path, wait: FileLock::DEFAULT_WAIT }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SessionLock for FileLock {
    fn acquire(&self) -> LocalFuture<'_, Result<LockGuard, ApiError>> {
        let open = || {
            if let Some(dir) = self.path.parent() {
                std::fs::create_dir_all(dir).ok()?;
            }
            std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&self.path).ok()
        };
        let result = match open() {
            None => {
                log::warn!("share: refresh lock file unavailable; refreshing with the in-process lock only");
                Ok(LockGuard::new(()))
            }
            Some(f) => {
                let until = std::time::Instant::now() + self.wait;
                loop {
                    match f.try_lock() {
                        // Dropping the File closes it, which releases the lock.
                        Ok(()) => break Ok(LockGuard::new(f)),
                        Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < until => {
                            // Native jobs run on their own thread: a short sleep blocks nothing else.
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        Err(std::fs::TryLockError::WouldBlock) => break Err(ApiError::LockBusy),
                        Err(std::fs::TryLockError::Error(_)) => break Ok(LockGuard::new(())),
                    }
                }
            }
        };
        Box::pin(std::future::ready(result))
    }
}

/// The browser: the Web Locks API (`navigator.locks.request`), shared by every tab of the app's
/// origin. Without it (an old browser) the refresh goes ahead with the in-tab single-flight only.
#[cfg(target_arch = "wasm32")]
pub struct WebLock;

#[cfg(target_arch = "wasm32")]
impl SessionLock for WebLock {
    fn acquire(&self) -> LocalFuture<'_, Result<LockGuard, ApiError>> {
        Box::pin(async {
            use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
            struct Held {
                release: js_sys::Function,
                _cb: Closure<dyn FnMut(JsValue) -> js_sys::Promise>,
            }
            impl Drop for Held {
                fn drop(&mut self) {
                    let _ = self.release.call0(&JsValue::NULL);
                }
            }
            let attempt = async {
                let navigator = web_sys::window()?.navigator();
                let locks = js_sys::Reflect::get(&navigator, &"locks".into()).ok().filter(|v| !v.is_undefined())?;
                let request: js_sys::Function = js_sys::Reflect::get(&locks, &"request".into()).ok()?.dyn_into().ok()?;
                // `release` resolves the promise the lock callback returns; until then the lock is held.
                let mut release: Option<js_sys::Function> = None;
                let held_until = js_sys::Promise::new(&mut |resolve, _| release = Some(resolve));
                let mut granted_fn: Option<js_sys::Function> = None;
                let granted = js_sys::Promise::new(&mut |resolve, _| granted_fn = Some(resolve));
                let granted_fn = granted_fn?;
                let cb = Closure::<dyn FnMut(JsValue) -> js_sys::Promise>::new(move |_lock: JsValue| {
                    let _ = granted_fn.call0(&JsValue::NULL);
                    held_until.clone()
                });
                request.call2(&locks, &"vershion-pool-refresh".into(), cb.as_ref().unchecked_ref()).ok()?;
                wasm_bindgen_futures::JsFuture::from(granted).await.ok()?;
                Some(Held { release: release?, _cb: cb })
            };
            Ok(match attempt.await {
                Some(h) => LockGuard::new(h),
                None => LockGuard::new(()),
            })
        })
    }
}
