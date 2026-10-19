//! End-to-end check of the shared-pool client against a real local Worker (server/worker).
//! Ignored by default: it needs the Worker running with log-mode mail, and reads the mailed
//! links from the Worker's output.
//!
//! ```sh
//! cd server/worker
//! printf 'MAIL_MODE=log\nAPP_URL=http://localhost\nALLOWED_ORIGINS=http://localhost\nAUTH_SECRET=%s\n' \
//!   "$(head -c 32 /dev/urandom | base64)" > .dev.vars          # git-ignored; delete afterwards
//! wrangler d1 migrations apply DB --local --persist-to /tmp/pool-e2e
//! wrangler dev --port 18811 --persist-to /tmp/pool-e2e > /tmp/pool-e2e.log 2>&1 &
//! cd ../..
//! VERSHION_POOL_E2E=http://127.0.0.1:18811 VERSHION_POOL_E2E_LOG=/tmp/pool-e2e.log \
//!   cargo test --test share_e2e -- --ignored --nocapture
//! ```
//!
//! What this run covers: the client (`share::client::Client`) against the real server for
//! sign-up, confirm, sign-in, restore with a saved refresh token, sign-out, sign-out everywhere,
//! forgot/reset, a real 429 followed by device enrolment, and deletion.
//!
//! What it does NOT cover (each is covered by unit tests in `src/share/tests.rs`, against a fake
//! server): the Account screen model and its trust gate and server modes; the cross-process lock
//! (two clients over a real file lock); sign-out while a request hangs or the app is killed;
//! keychain failures; TLS errors; the browser build (nothing here runs in a browser); the account
//! lockdown after 30 failures an hour (this run trips the per-network limit, which answers the
//! same 429); and the approval workflow (pending -> approved/rejected needs an admin).
#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use vershion::share::api::{Flow, Profile, Status, parse_link};
use vershion::share::client::Client;
use vershion::share::secret::Secret;
use vershion::share::transport::{Mode, NativeTransport, block_on};
use vershion::share::vault::{MemoryVault, Slot, Vault};

/// The newest link of `kind` ("verify", "reset", "enrol") mailed to `to`, from the Worker's log.
fn mailed(log: &str, to: &str, kind: &str) -> Secret {
    for _ in 0..50 {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        let mut found = None;
        let mut current = false;
        for line in text.lines() {
            if line.contains("[dev mail] to=") {
                current = line.contains(&format!("to={to} "));
            } else if current && line.contains(&format!("/account/{kind}#token=")) {
                found = parse_link(line);
            }
        }
        if let Some(l) = found {
            return l.token;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("no {kind} mail to {to} in {log}");
}

fn client(base: &str) -> Client {
    Client::new(base, Mode::Native, Box::new(NativeTransport::default()), Box::new(MemoryVault::default()))
}

#[test]
#[ignore = "needs a local Worker; see the module docs"]
fn pool_end_to_end() {
    let (Ok(base), Ok(log)) = (std::env::var("VERSHION_POOL_E2E"), std::env::var("VERSHION_POOL_E2E_LOG")) else {
        panic!("set VERSHION_POOL_E2E and VERSHION_POOL_E2E_LOG");
    };
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let email = format!("e2e{stamp}@school.example.org");
    let profile = Profile { display_name: "E2E Teacher".into(), school: "Loopback High".into(), adult_school_employee: true };
    let first = Secret::new("typed at sign-up 1");
    let pw = Secret::new("confirmed password 2");
    let pw2 = Secret::new("after the reset 3!");
    let c = client(&base);

    // Sign-up never signs in, and the sign-up password never works.
    block_on(c.sign_up(&email, &first, &profile)).unwrap();
    assert!(!c.has_session());
    let e = block_on(c.sign_in(&email, &first)).unwrap_err();
    assert_eq!(e.code(), "invalid_credentials");
    println!("unconfirmed sign-in: {}", e.message(Flow::SignIn));

    // Confirm with the password and profile: a device token comes back.
    let verify = mailed(&log, &email, "verify");
    let r = block_on(c.confirm(&verify, &pw, &profile)).unwrap();
    assert!(r.password_set && r.device_known);
    let e = block_on(c.confirm(&verify, &pw, &profile)).unwrap_err();
    assert_eq!(e.code(), "invalid_token", "a link works once");

    // Sign in; status is pending; the profile is the one set at confirmation.
    block_on(c.sign_in(&email, &pw)).unwrap();
    let me = block_on(c.me()).unwrap();
    assert_eq!((me.email.as_str(), me.status.clone()), (email.as_str(), Status::Pending));
    assert_eq!(me.school, "Loopback High");

    // A second client restores the session from the saved refresh token (one rotation).
    let (account, refresh) = refresh_of(&base, &email, &pw);
    let vault = MemoryVault::default();
    vault.save(&Slot::Index, &Secret::new(format!(r#"{{"current":"{account}"}}"#))).unwrap();
    vault.save(&Slot::Refresh(account.clone()), &Secret::new(refresh)).unwrap();
    let c2 = Client::new(&base, Mode::Native, Box::new(NativeTransport::default()), Box::new(vault));
    assert_eq!(block_on(c2.restore()).unwrap().email, email);

    // Plain sign-out, then sign-out everywhere (needs the refresh token).
    block_on(c2.sign_out(false, false)).unwrap();
    assert!(!c2.has_session());
    block_on(c.sign_out(true, false)).unwrap();
    assert!(!c.has_session() && !c.knows_device());

    // Forgot -> reset: every session ends, a device token and an enrolment code are issued.
    block_on(c.forgot(&email)).unwrap();
    block_on(c.forgot("nobody-at-all@school.example.org")).unwrap();
    let reset = mailed(&log, &email, "reset");
    let weak = block_on(c.reset(&reset, &Secret::new("short"), None)).unwrap_err();
    assert_eq!(weak.code(), "weak_password");
    block_on(c.reset(&reset, &pw2, None)).unwrap();
    assert!(c.knows_device());
    let code = mailed(&log, &email, "enrol");

    // A fresh device: wrong passwords until the server refuses with 429 ...
    let fresh = client(&base);
    let mut limited = None;
    for _ in 0..12 {
        match block_on(fresh.sign_in(&email, &Secret::new("wrong password here"))) {
            Err(e) if e.is_rate_limited() => {
                limited = Some(e);
                break;
            }
            Err(e) => assert_eq!(e.code(), "invalid_credentials"),
            Ok(()) => panic!("wrong password accepted"),
        }
    }
    let limited = limited.expect("the server never rate-limited");
    println!("429: {}", limited.message(Flow::SignIn));
    let still = block_on(fresh.sign_in(&email, &pw2)).unwrap_err();
    assert!(still.is_rate_limited(), "the right password is refused too while limited");
    // ... and the mailed enrolment code makes it a known device that gets in.
    let enrolled = block_on(fresh.enrol(&code, &pw2)).unwrap();
    assert!(enrolled.is_some(), "the device token names its account");
    block_on(fresh.sign_in(&email, &pw2)).unwrap();
    assert_eq!(fresh.account(), enrolled, "the enrolled device and the session are one account");
    assert_eq!(block_on(fresh.me()).unwrap().email, email);

    // Delete: a wrong password changes nothing and keeps the session; the right one deletes.
    let e = block_on(fresh.delete_account(&Secret::new("not the password!"))).unwrap_err();
    assert_eq!(e.code(), "invalid_credentials");
    assert!(fresh.has_session());
    block_on(fresh.delete_account(&pw2)).unwrap();
    assert!(!fresh.has_session());
    let e = block_on(c.sign_in(&email, &pw2)).unwrap_err();
    assert_eq!(e.code(), "invalid_credentials");
    println!("end to end: ok");
}

/// Signs in once more to get the account id and a refresh token for the restore check.
fn refresh_of(base: &str, email: &str, pw: &Secret) -> (String, String) {
    use std::sync::{Arc, Mutex};
    use vershion::share::vault::VaultError;
    #[derive(Clone, Default)]
    struct Spy(Arc<Mutex<Option<String>>>);
    impl Vault for Spy {
        fn load(&self, _: &Slot) -> Result<Option<Secret>, VaultError> {
            Ok(None)
        }
        fn save(&self, slot: &Slot, v: &Secret) -> Result<(), VaultError> {
            if matches!(slot, Slot::Refresh(_)) {
                *self.0.lock().unwrap() = Some(v.expose().to_string());
            }
            Ok(())
        }
        fn clear(&self, _: &Slot) -> Result<(), VaultError> {
            Ok(())
        }
        fn persistent(&self) -> bool {
            false
        }
    }
    let spy = Spy::default();
    let c = Client::new(base, Mode::Native, Box::new(NativeTransport::default()), Box::new(spy.clone()));
    block_on(c.sign_in(email, pw)).unwrap();
    (c.account().expect("an account id"), spy.0.lock().unwrap().clone().expect("a refresh token"))
}

/// The real OS keychain round trip (Linux: needs a running Secret Service).
#[test]
#[ignore = "touches the OS keychain"]
fn keychain_round_trip() {
    use vershion::share::vault::KeyringVault;
    let v = KeyringVault::open("https://keychain-probe.invalid").expect("keychain available");
    let slot = Slot::Device("00000000-0000-4000-8000-000000000000".into());
    v.save(&slot, &Secret::new("probe-value")).unwrap();
    assert_eq!(v.load(&slot).unwrap().unwrap().expose(), "probe-value");
    v.clear(&slot).unwrap();
    assert!(v.load(&slot).unwrap().is_none());
}
