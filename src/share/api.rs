//! Wire types of the shared-pool API (server/auth/README.md) and their human-facing text.
//!
//! Error wording rule: the server deliberately answers sign-up, resend and forgot the same way for
//! every address, and login the same way for an unknown address, a wrong password and an account
//! that is not finished. No message here may turn one of those into a hint about whether an
//! account exists ("no such account", "not registered", ...). The tests check every code.

use serde_json::Value;

use super::secret::Secret;
use super::transport::{NetError, NetKind, Response};

/// Which call failed, for wording that fits the screen it shows on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    SignUp,
    Resend,
    Confirm,
    SignIn,
    Enrol,
    Refresh,
    SignOut,
    SignOutEverywhere,
    Forgot,
    Reset,
    Delete,
    Status,
    Reapply,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiError {
    /// No server address saved yet.
    NotConfigured,
    /// No HTTP answer (offline, DNS, timeout, CORS).
    Offline(String),
    /// A connection but no secure one (an untrusted certificate, a TLS-inspecting network).
    Tls(String),
    /// The server did its part, but this computer's keychain would not delete a saved sign-in.
    /// The token is blocked from use and deletion is retried.
    LocalCleanup,
    /// Another copy of verSHion held the sign-in lock too long (it may be waiting for the
    /// keychain). Nothing changed; trying again is safe.
    LockBusy,
    /// The session is gone (refresh refused, or no session on this device).
    SignedOut,
    /// An HTTP error with the server's `{"error": code}` body.
    Http { status: u16, code: String, field: Option<String>, reason: Option<String>, retry_after: Option<u64> },
    /// A 2xx whose body was not what the API documents.
    BadReply(String),
}

impl From<NetError> for ApiError {
    fn from(e: NetError) -> Self {
        match e.kind {
            NetKind::Offline => ApiError::Offline(e.text),
            NetKind::Tls => ApiError::Tls(e.text),
        }
    }
}

impl ApiError {
    /// Builds the error for a non-2xx response. Only short ASCII identifiers are kept from the
    /// body; free text is never copied into an error.
    pub fn from_response(r: &Response) -> ApiError {
        let v = r.json();
        let ident = |key: &str| v.get(key).and_then(Value::as_str).filter(|s| is_ident(s)).map(str::to_string);
        ApiError::Http {
            status: r.status,
            code: ident("error").unwrap_or_default(),
            field: ident("field"),
            reason: ident("reason"),
            retry_after: r.retry_after,
        }
    }

    pub fn code(&self) -> &str {
        match self {
            ApiError::Http { code, .. } => code,
            _ => "",
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            ApiError::Http { status, .. } => *status,
            _ => 0,
        }
    }

    /// A login refused for too many attempts (an account lockdown, or this network's limit: the
    /// server does not say which, and the app does not guess).
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, ApiError::Http { status: 429, .. })
    }

    /// The reset or confirm form must also carry the profile (the account was never finished).
    pub fn needs_profile(&self) -> bool {
        matches!(self, ApiError::Http { status: 400, code, field: Some(f), .. }
            if code == "invalid_request" && matches!(f.as_str(), "display_name" | "school" | "adult_school_employee" | "profile"))
    }

    /// The sentence shown to the person.
    pub fn message(&self, flow: Flow) -> String {
        match self {
            ApiError::NotConfigured => "Sharing is not set up: add the server address first.".into(),
            ApiError::Offline(why) => format!("{why} Nothing changed; your work on this device is unaffected."),
            // Never a password hint: the request did not reach the server's checks at all.
            ApiError::Tls(why) => format!(
                "{why} The network may be inspecting secure connections (common on school networks), or this \
                 computer does not trust the server's certificate. Ask your school's IT team, or try another \
                 network. Nothing was sent; your work on this device is unaffected."
            ),
            ApiError::LockBusy => "Another copy of verSHion is busy with your sign-in (it may be waiting for the keychain to \
                 unlock). Nothing changed. Close the other copy or finish its prompt, then try again."
                .into(),
            ApiError::LocalCleanup => "The server finished, but this computer's keychain would not let verSHion delete the saved \
                 sign-in. It is blocked from use, and verSHion keeps trying to delete it while this window is open \
                 and the next time you open it. You can also remove the \"verSHion shared pool\" entries in your \
                 keychain app."
                .into(),
            ApiError::SignedOut => "You are signed out. Sign in again to continue.".into(),
            ApiError::BadReply(_) => "The server sent a reply the app does not understand. Check the server address, or try again later.".into(),
            ApiError::Http { status, code, field, reason, retry_after } => {
                http_message(flow, *status, code, field.as_deref(), reason.as_deref(), *retry_after)
            }
        }
    }
}

fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn field_label(field: &str) -> &'static str {
    match field {
        "email" => "email address",
        "password" => "password",
        "display_name" => "display name",
        "school" => "school",
        "adult_school_employee" => "adult school employee statement",
        "token" | "code" => "link",
        "profile" => "name and school",
        _ => "form",
    }
}

/// "in 3 minutes" / "in about 2 hours" / "later".
pub fn wait_text(secs: Option<u64>) -> String {
    match secs {
        None | Some(0) => "later".into(),
        Some(s) if s < 90 => "in about a minute".into(),
        Some(s) if s < 90 * 60 => format!("in about {} minutes", s.div_ceil(60)),
        Some(s) => format!("in about {} hours", s.div_ceil(3600)),
    }
}

fn weak_password_text(reason: Option<&str>) -> String {
    match reason {
        Some("too_short") => "That password is too short: use at least 10 characters.".into(),
        Some("too_long") => "That password is too long.".into(),
        Some("common") => "That password is too common or too easy to guess (a common word, a run like 1234, or a repeat). Choose a longer, less predictable one.".into(),
        Some("contains_email") => "The password must not contain your email address.".into(),
        _ => "The server did not accept that password. Choose a longer, less predictable one.".into(),
    }
}

fn http_message(flow: Flow, status: u16, code: &str, field: Option<&str>, reason: Option<&str>, retry: Option<u64>) -> String {
    let link_word = if flow == Flow::Enrol { "code" } else { "link" };
    match code {
        "invalid_credentials" => match flow {
            // The uniform login answer: unknown address, wrong password and unfinished account
            // all land here, so the text covers all three without picking one.
            Flow::SignIn => "Sign-in failed. Check the email address and password. If you signed up recently, finish with the link in the confirmation email first; if you forgot the password, reset it.".into(),
            Flow::Enrol => "That password is not right for the account this code was sent for.".into(),
            Flow::Delete => "That password is not right. Nothing was deleted.".into(),
            _ => "That password is not right.".into(),
        },
        "rate_limited" => match flow {
            Flow::SignIn => format!(
                "Sign-in is paused for this account or this network after too many failed attempts. Try again {}. \
                 To get in now, reset your password from the email, or use a device code from that email.",
                wait_text(retry)
            ),
            Flow::Delete => format!("Too many wrong passwords. Nothing was deleted. Try again {}, or reset your password.", wait_text(retry)),
            _ => format!("Too many requests from here. Try again {}.", wait_text(retry)),
        },
        "cooldown" => format!("You can apply again {}.", wait_text(retry)),
        "weak_password" => weak_password_text(reason),
        "invalid_token" => format!(
            "This {link_word} can't be used: it was already used, has expired, or was copied incompletely. Ask for a new one."
        ),
        "in_progress" => format!("This {link_word} is being used right now (perhaps a double click). Wait a few seconds, then try again."),
        "invalid_request" => match (field, reason) {
            (Some("adult_school_employee"), _) => "Confirm that you are an adult employee of a school.".into(),
            (Some(f), Some("required_plain_text")) => format!("Enter your {} as plain text.", field_label(f)),
            (Some(f), Some("invalid_characters_or_length")) => format!(
                "Your {} has characters the server does not accept (line breaks or invisible characters), or is too long.",
                field_label(f)
            ),
            (Some(f), _) => format!("Check the {}.", field_label(f)),
            (None, _) => "The server did not accept the form. Check every field.".into(),
        },
        "account_suspended" => "This account is suspended. The email you received explains why.".into(),
        "unauthorized" | "invalid_refresh_token" => "Your session has ended. Sign in again.".into(),
        "refresh_token_required" => {
            "Signing out everywhere needs this device's own session. Sign in again here, then try.".into()
        }
        "known_device_required" => "This needs a device you have signed in on before.".into(),
        "not_eligible" => "There is nothing to apply for right now.".into(),
        "client_ip_required" | "origin_not_allowed" | "origin_required" | "native_client_with_origin" | "invalid_client" => {
            "The server refused this app's request. The server address may be wrong, or the server is not set up for this app.".into()
        }
        "unsupported_media_type" | "body_too_large" | "invalid_json" => {
            "The server could not read the app's request. Check for updates to verSHion.".into()
        }
        _ if status >= 500 => "The server had a problem. Try again later.".into(),
        _ if status == 404 || status == 405 => {
            "That server does not offer verSHion accounts. Check the server address.".into()
        }
        _ => format!("The server refused the request (HTTP {status}). Try again later."),
    }
}

/// What a person enters about themselves. Sent as `display_name`, `school`, `adult_school_employee`
/// (server/worker/src/index.ts `profile`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub display_name: String,
    pub school: String,
    pub adult_school_employee: bool,
}

impl Profile {
    pub fn add_to(&self, body: &mut serde_json::Map<String, Value>) {
        body.insert("display_name".into(), Value::from(self.display_name.trim()));
        body.insert("school".into(), Value::from(self.school.trim()));
        body.insert("adult_school_employee".into(), Value::from(self.adult_school_employee));
    }

    /// What is missing, checked before anything is sent.
    pub fn problem(&self) -> Option<&'static str> {
        if self.display_name.trim().is_empty() {
            Some("Enter a display name.")
        } else if self.school.trim().is_empty() {
            Some("Enter your school.")
        } else if !self.adult_school_employee {
            Some("Confirm that you are an adult employee of a school.")
        } else {
            None
        }
    }
}

/// The minimum the server accepts. The full policy (common words, runs, your address) is checked
/// by the server; this only saves a round trip.
pub const MIN_PASSWORD_CHARS: usize = 10;

pub fn password_problem(p: &Secret) -> Option<String> {
    if p.is_empty() {
        Some("Enter a password.".into())
    } else if p.chars() < MIN_PASSWORD_CHARS {
        Some(format!("Use at least {MIN_PASSWORD_CHARS} characters."))
    } else {
        None
    }
}

/// `status` from `GET /me`. Reviewer notes are never sent by the server and never parsed here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Pending,
    Approved,
    /// Rejected or revoked: the reviewer's reason (shown as plain text) and when re-applying opens.
    Rejected { revoked: bool, reason: Option<String>, reapply_after_ms: Option<i64>, can_reapply: bool },
    Suspended { reason: Option<String> },
    Unverified,
    /// A state this build does not know; shown neutrally.
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Me {
    pub email: String,
    pub display_name: String,
    pub school: String,
    pub status: Status,
}

impl Me {
    pub fn parse(v: &Value) -> Option<Me> {
        let s = |v: Option<&Value>| v.and_then(Value::as_str).map(plain).unwrap_or_default();
        let st = v.get("status");
        let state = st.and_then(|x| x.get("state")).and_then(Value::as_str).unwrap_or("");
        let reason = st.and_then(|x| x.get("reason")).and_then(Value::as_str).map(plain).filter(|r| !r.is_empty());
        let status = match state {
            "pending" => Status::Pending,
            "approved" => Status::Approved,
            "rejected" | "revoked" => Status::Rejected {
                revoked: state == "revoked",
                reason,
                reapply_after_ms: st.and_then(|x| x.get("reapply_after")).and_then(Value::as_i64),
                can_reapply: st.and_then(|x| x.get("can_reapply")).and_then(Value::as_bool).unwrap_or(false),
            },
            "suspended" => Status::Suspended { reason },
            "unverified" => Status::Unverified,
            "" if v.get("trust").is_some() => match v.get("trust").and_then(Value::as_str) {
                Some("teacher" | "admin") => Status::Approved,
                Some("pending") => Status::Pending,
                _ => Status::Other(String::new()),
            },
            other => Status::Other(plain(other)),
        };
        if !v.is_object() {
            return None;
        }
        Some(Me {
            email: s(v.get("email")),
            display_name: s(v.get("display_name")),
            school: s(v.pointer("/profile/school")),
            status,
        })
    }
}

/// Server text made safe to show as one plain line: control, format and bidi characters dropped
/// (the server already refuses them in names; this is the client's own guarantee for reasons and
/// anything added later), whitespace collapsed, length capped.
pub fn plain(s: &str) -> String {
    let kept: String = s
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|&c| !c.is_control() && !is_format_char(c))
        .collect();
    let collapsed = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(500).collect()
}

fn is_format_char(c: char) -> bool {
    matches!(c as u32,
        0x00AD | 0x061C | 0x180E | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB)
}

/// Days since 1970 -> "2026-10-26" (UTC), for re-apply dates.
pub fn date_text(ms: i64) -> String {
    let (y, m, d) = crate::time::civil_from_days(ms.div_euclid(86_400_000));
    format!("{y:04}-{m:02}-{d:02}")
}

// ---- mailed links ------------------------------------------------------------------------

/// What a mailed link is for, from its path (`/account/verify`, `/account/reset`, `/account/enrol`;
/// server/worker/src/mail.ts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Verify,
    Reset,
    Enrol,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// None when only the bare code was pasted: the screen it was pasted on decides.
    pub kind: Option<LinkKind>,
    pub token: Secret,
    /// The link's host (lower case, punycode), when a whole link was pasted.
    pub host: Option<String>,
}

/// Every token and code the server mails is 32 random bytes in base64url: 43 characters.
pub fn is_token(s: &str) -> bool {
    s.len() == 43 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Reads a token out of whatever was pasted: the whole link, the link wrapped across lines or in
/// `<...>` by a mail client, `#token=...`, `token=...`, or the bare 43-character code.
pub fn parse_link(pasted: &str) -> Option<Link> {
    // Mail clients wrap long links (raw quoted-printable shows the wrap as "=" at a line end) and
    // add quotes or angle brackets; none of those characters can be part of a link we send.
    let unwrapped = pasted.replace("=\r\n", "").replace("=\n", "");
    let joined: String = unwrapped
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '<' | '>' | '"' | '\'' | '\u{200B}' | '\u{FEFF}'))
        .collect();
    let joined = joined.trim_end_matches(['.', ',', ';', ')', ']']);
    let kind = if joined.contains("/account/verify") {
        Some(LinkKind::Verify)
    } else if joined.contains("/account/reset") {
        Some(LinkKind::Reset)
    } else if joined.contains("/account/enrol") {
        Some(LinkKind::Enrol)
    } else {
        None
    };
    let token = match joined.rfind("token=") {
        Some(i) => {
            let rest = &joined[i + "token=".len()..];
            rest.split(['&', '#', '?', '/']).next().unwrap_or("")
        }
        None => joined.rsplit(['/', '#', '=']).next().unwrap_or(""),
    };
    let host = joined
        .find("://")
        .and_then(|_| url::Url::parse(joined.split('#').next().unwrap_or("")).ok())
        .and_then(|u| u.host_str().map(str::to_string));
    is_token(token).then(|| Link { kind, token: Secret::new(token), host })
}

const LOCAL: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

/// Whether a mailed link's host belongs with the pool server's host. The links point at the web
/// app, which shares the API's registrable domain (docs/shared-pool.md "Stack"), so the two hosts
/// must be equal, one the parent of the other, or siblings under one parent of at least two
/// labels (`app.school.org` and `pool.school.org`). Local development hosts match each other.
/// Without a public-suffix list this is deliberately strict: `a.co.uk` and `b.co.uk` do not match.
pub fn link_host_matches(link_host: &str, api_host: &str) -> bool {
    let (l, a) = (link_host.to_ascii_lowercase(), api_host.to_ascii_lowercase());
    if l == a || (LOCAL.contains(&l.as_str()) && LOCAL.contains(&a.as_str())) {
        return true;
    }
    // On shared hosting only the exact host is the same owner.
    if on_shared_hosting(&l) || on_shared_hosting(&a) {
        return false;
    }
    let parent = |h: &str| h.split_once('.').map(|(_, p)| p.to_string()).unwrap_or_default();
    let labels = |h: &str| h.split('.').count();
    let (pl, pa) = (parent(&l), parent(&a));
    let suffix_of = |child: &str, p: &str| child.ends_with(&format!(".{p}"));
    (labels(&l) >= 3 && labels(&a) >= 3 && pl == pa && labels(&pl) >= 2 && !is_two_level_suffix(&pl))
        || (suffix_of(&l, &a) && labels(&a) >= 2 && !is_two_level_suffix(&a))
        || (suffix_of(&a, &l) && labels(&l) >= 2 && !is_two_level_suffix(&l))
}

/// Shared-hosting domains where every customer gets a subdomain (`anyone.pages.dev`): two hosts
/// under one of these are different owners, so they never count as one site.
const SHARED_HOSTING: [&str; 32] = [
    "pages.dev", "workers.dev", "r2.dev", "github.io", "gitlab.io", "netlify.app", "netlify.com", "vercel.app",
    "now.sh", "herokuapp.com", "appspot.com", "web.app", "firebaseapp.com", "azurewebsites.net", "azurestaticapps.net",
    "cloudfront.net", "amazonaws.com", "onrender.com", "fly.dev", "glitch.me", "repl.co", "replit.app", "surge.sh",
    "blogspot.com", "wordpress.com", "ngrok.io", "ngrok-free.app", "ngrok.app", "trycloudflare.com", "deno.dev",
    "railway.app", "codeberg.page",
];

fn on_shared_hosting(host: &str) -> bool {
    SHARED_HOSTING.iter().any(|s| host == *s || host.ends_with(&format!(".{s}")))
}

/// The common public suffixes with two labels, which are never a shared site.
fn is_two_level_suffix(d: &str) -> bool {
    let mut it = d.rsplitn(2, '.');
    let (tld, second) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
    tld.len() == 2 && matches!(second, "co" | "com" | "ac" | "edu" | "gov" | "org" | "net" | "sch" | "k12" | "nhs" | "ltd" | "plc")
}

/// The server as people should read it before trusting it with a password: the exact scheme,
/// host and port in ASCII (punycode for an international name), with the Unicode form beside it
/// when it differs, so a look-alike name can't pass for the real one.
pub fn server_label(base: &str) -> String {
    let Ok(u) = url::Url::parse(base) else { return base.to_string() };
    let host = u.host_str().unwrap_or("");
    let port = u.port().map(|p| format!(":{p}")).unwrap_or_default();
    let ascii = format!("{}://{host}{port}{}", u.scheme(), u.path().trim_end_matches('/'));
    let (unicode, ok) = idna::domain_to_unicode(host);
    if ok.is_ok() && unicode != host { format!("{ascii}  (shown in Unicode as {unicode})") } else { ascii }
}

/// Builds the link for a page path and fragment as the browser reports them (web deep links).
pub fn link_from_location(pathname: &str, hash: &str) -> Option<Link> {
    let link = parse_link(&format!("{pathname}{hash}"))?;
    link.kind.is_some().then_some(link)
}

// ---- server address ----------------------------------------------------------------------

/// Checks and normalises the server address: `https://` anywhere, `http://` only for this machine
/// (`localhost`, `127.0.0.1`, `[::1]`). No user name or password, query or fragment. A path
/// prefix is kept; the trailing slash is dropped. Returns the address requests are built on.
pub fn validate_base_url(input: &str) -> Result<String, String> {
    let t = input.trim();
    if t.is_empty() {
        return Err("Enter the server address, for example https://pool.example.org".into());
    }
    if t.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("The address must not contain spaces.".into());
    }
    let u = url::Url::parse(t).map_err(|_| "That is not a web address. Start it with https://".to_string())?;
    let host = u.host_str().unwrap_or("").to_string();
    if host.is_empty() {
        return Err("The address has no server name.".into());
    }
    let local = matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]");
    match u.scheme() {
        "https" => {}
        "http" if local => {}
        "http" => return Err("Use https:// (plain http:// is only allowed for localhost while testing).".into()),
        _ => return Err("Use an https:// address.".into()),
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err("The address must not contain a user name or password.".into());
    }
    if u.query().is_some() || u.fragment().is_some() {
        return Err("The address must not contain ? or #.".into());
    }
    Ok(u.as_str().trim_end_matches('/').to_string())
}
