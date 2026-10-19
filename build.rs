//! Checks the shared-pool server address a release is built with (`VERSHION_POOL_URL`, see the
//! `[features]` comment in Cargo.toml and src/share/mod.rs). An invalid address fails the build
//! instead of quietly leaving sharing switched off or pointing somewhere unexpected.

fn main() {
    println!("cargo:rerun-if-env-changed=VERSHION_POOL_URL");
    println!("cargo:rerun-if-changed=build.rs");
    let Ok(raw) = std::env::var("VERSHION_POOL_URL") else { return };
    if raw.is_empty() {
        return;
    }
    let debug = std::env::var("PROFILE").is_ok_and(|p| p == "debug");
    if let Err(why) = check(&raw, debug) {
        panic!(
            "\n\nVERSHION_POOL_URL={raw:?} is not a usable pool server address: {why}.\n\
             Use the API's https address, e.g. VERSHION_POOL_URL=https://api.example.org \
             (http://localhost or http://127.0.0.1 only in debug builds), or unset it.\n\n"
        );
    }
}

/// The same rules as `share::api::validate_base_url`, minus the local http exception in release.
fn check(raw: &str, debug: bool) -> Result<(), String> {
    if raw.trim() != raw || raw.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("it contains spaces".into());
    }
    let u = url::Url::parse(raw).map_err(|e| format!("it is not a URL ({e})"))?;
    let host = u.host_str().unwrap_or("");
    if host.is_empty() {
        return Err("it has no host".into());
    }
    let local = matches!(host, "localhost" | "127.0.0.1" | "[::1]");
    match u.scheme() {
        "https" => {}
        "http" if local && debug => {}
        "http" if local => return Err("plain http is only allowed in debug builds".into()),
        _ => return Err("it must start with https://".into()),
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err("it contains a user name or password".into());
    }
    if u.query().is_some() || u.fragment().is_some() {
        return Err("it contains ? or #".into());
    }
    Ok(())
}
