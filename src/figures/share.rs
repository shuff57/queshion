//! Offline share links: `queshion://figure/<id>?p=<base64url>`. The payload is the compact JSON
//! object of only the params that differ from the figure's opening state (the reference sites do
//! the same with a query string, e.g. `motion-graphs?letters=1`), base64url-encoded without
//! padding. No server is involved: the link carries everything needed to rebuild the figure.
//! See docs/figure-share.md.

use super::{Params, Value};

pub const SCHEME: &str = "queshion://figure/";

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url without padding (RFC 4648 §5).
pub fn b64url_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

/// Decodes base64url (also accepts the standard `+/` alphabet and stray `=` padding).
pub fn b64url_decode(s: &str) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Result<u32, String> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return Err(format!("unexpected character '{}' in the link", c as char)),
        } as u32)
    };
    let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
    if bytes.len() % 4 == 1 {
        return Err("the link looks cut off".into());
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

fn to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::F(f) => serde_json::Number::from_f64(*f).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
        Value::I(i) => serde_json::Value::from(*i),
        Value::B(b) => serde_json::Value::Bool(*b),
        Value::S(s) => serde_json::Value::String(s.clone()),
    }
}

/// The share link for figure `id` showing `cur`, relative to its opening state `base`.
pub fn encode(id: &str, cur: &Params, base: &Params) -> String {
    let diff: serde_json::Map<String, serde_json::Value> = cur
        .0
        .iter()
        .filter(|(k, v)| base.0.get(*k) != Some(*v))
        .map(|(k, v)| (k.clone(), to_json(v)))
        .collect();
    if diff.is_empty() {
        return format!("{SCHEME}{id}?v={VERSION}");
    }
    let json = serde_json::to_string(&serde_json::Value::Object(diff)).unwrap_or_default();
    format!("{SCHEME}{id}?p={}&v={VERSION}", b64url_encode(json.as_bytes()))
}

/// Link format version written by `encode` (`&v=3`). Links without it are v1. A link reads the
/// opening-state keys newer than its version (`start::newer_than`) as plain defaults, so it
/// opens as it did when made. v2: round 5 (vector resultant dashed); v3: P4 (orbital diagram
/// without the configuration line, bare angles and parallel lines).
pub const VERSION: u32 = 3;

/// Pasted text with whitespace dropped and HTML-escaped ampersands (`&amp;`, from a link copied
/// out of a mail or web page) turned back into `&`.
fn clean(text: &str) -> String {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    t.replace("&amp;", "&").replace("&#38;", "&")
}

/// The format version a pasted link was written with: its `v=` value, 1 when it has none.
pub fn version(text: &str) -> u32 {
    let t = clean(text);
    let q = t.split_once('?').map(|(_, q)| q).unwrap_or("");
    q.split('&').find_map(|kv| kv.strip_prefix("v=")).and_then(|v| v.trim_end_matches(|c: char| !c.is_ascii_digit()).parse().ok()).unwrap_or(1)
}

/// Reads a pasted link back into (figure id, changed params as JSON). Lenient about what
/// surrounds it: whitespace and line breaks (mail clients wrap long links), angle brackets,
/// a missing scheme (`figure/<id>?p=...`) and extra `&key=value` pairs are all accepted.
pub fn decode(text: &str) -> Result<(String, serde_json::Map<String, serde_json::Value>), String> {
    let t = clean(text);
    let t = t.trim_matches(|c| c == '<' || c == '>' || c == '"' || c == '\'');
    if t.is_empty() {
        return Err("paste a queshion://figure/... link first".into());
    }
    // the scheme and "figure/" in any case (some apps capitalise a pasted link's start)
    let rest = if let Some(i) = t.to_ascii_lowercase().find("figure/") { &t[i + "figure/".len()..] } else {
        return Err("not a queShion figure link (it should start with queshion://figure/)".into());
    };
    let (id, query) = match rest.split_once('?') {
        Some((a, b)) => (a, b),
        None => (rest, ""),
    };
    let id = id.trim_end_matches('/');
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("the link has no figure name".into());
    }
    let mut params = serde_json::Map::new();
    for pair in query.split('&').filter(|s| !s.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == "p" && !v.is_empty() {
            let raw = b64url_decode(v)?;
            let txt = String::from_utf8(raw).map_err(|_| "the link is damaged (not text)".to_string())?;
            let val: serde_json::Value = serde_json::from_str(&txt).map_err(|_| "the link is damaged (bad settings)".to_string())?;
            match val {
                serde_json::Value::Object(o) => params.extend(o),
                _ => return Err("the link is damaged (bad settings)".into()),
            }
        }
    }
    Ok((id.to_ascii_lowercase(), params))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{examples_for, registry, start_params, Params, Value};

    #[test]
    fn base64url_round_trips_every_length() {
        for n in 0..70usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            let e = b64url_encode(&data);
            assert!(e.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{e}");
            assert_eq!(b64url_decode(&e).unwrap(), data, "n={n}");
        }
        assert_eq!(b64url_encode(b"Man"), "TWFu");
        assert_eq!(b64url_encode(b"\xfb\xff"), "-_8");
        assert_eq!(b64url_decode("-_8=").unwrap(), b"\xfb\xff");
        assert!(b64url_decode("abc$").is_err());
        assert!(b64url_decode("a").is_err());
    }

    #[test]
    fn unchanged_figure_is_a_bare_link() {
        let d = registry().into_iter().find(|d| d.id == "waves").unwrap();
        let p = start_params(&d);
        assert_eq!(encode(d.id, &p, &p), format!("queshion://figure/waves?v={VERSION}"));
        let (id, m) = decode("queshion://figure/waves").unwrap();
        assert_eq!(id, "waves");
        assert!(m.is_empty());
    }

    #[test]
    fn links_carry_their_version() {
        let d = registry().into_iter().find(|d| d.id == "waves").unwrap();
        let p = start_params(&d);
        assert_eq!(version(&encode(d.id, &p, &p)), VERSION);
        let mut q = p.clone();
        q.set_s("title", "x");
        assert_eq!(version(&encode(d.id, &q, &p)), VERSION);
        assert_eq!(version("queshion://figure/waves"), 1);
        assert_eq!(version("queshion://figure/waves?p=e30"), 1);
        assert_eq!(version("<queshion://figure/waves?p=e30&v=2>"), 2);
    }

    #[test]
    fn only_changed_params_travel_and_unicode_survives() {
        let d = registry().into_iter().find(|d| d.id == "motion-graphs").unwrap();
        let base = start_params(&d);
        let mut p = base.clone();
        p.set_b("letters", !base.b("letters"));
        p.set_s("title", "Car — ½ m/s² ✓");
        let link = encode(d.id, &p, &base);
        assert!(link.starts_with("queshion://figure/motion-graphs?p="), "{link}");
        assert!(link.len() < 120, "compact: {link}");
        let (id, m) = decode(&link).unwrap();
        assert_eq!(id, "motion-graphs");
        assert_eq!(m.len(), 2);
        assert_eq!(m["title"], serde_json::json!("Car — ½ m/s² ✓"));
        // wrapped across lines, in angle brackets, without the scheme
        let wrapped = format!("<{}\n  {}>", &link[..30], &link[30..]);
        assert_eq!(decode(&wrapped).unwrap().1, m);
        let bare = link.trim_start_matches("queshion://");
        assert_eq!(decode(bare).unwrap().1, m);
    }

    #[test]
    fn garbage_links_are_rejected() {
        assert!(decode("").is_err());
        assert!(decode("https://example.com/").is_err());
        assert!(decode("queshion://figure/?p=e30").is_err());
        assert!(decode("queshion://figure/waves?p=!!!").is_err());
        assert!(decode("queshion://figure/waves?p=WzFd").is_err()); // "[1]" is not an object
        assert!(decode("queshion://figure/wa ves").is_ok()); // whitespace is dropped
    }

    /// Links copied out of HTML (`&amp;`) keep their version, and a capitalised scheme or id
    /// still opens.
    #[test]
    fn escaped_and_capitalised_links() {
        let link = format!("queshion://figure/motion-graphs?p={}&v=2", b64url_encode(br#"{"letters":true}"#));
        let html = link.replace('&', "&amp;");
        assert_eq!(version(&html), 2);
        assert_eq!(version(&link.replace('&', "&#38;")), 2);
        assert_eq!(decode(&html).unwrap(), decode(&link).unwrap());
        let caps = link.replace("queshion://figure/motion-graphs", "Queshion://Figure/Motion-Graphs");
        let (id, m) = decode(&caps).unwrap();
        assert_eq!(id, "motion-graphs");
        assert_eq!(m["letters"], serde_json::json!(true));
        // the payload itself stays case-sensitive
        assert!(decode(&link.to_ascii_uppercase()).map(|(_, m)| m != decode(&link).unwrap().1).unwrap_or(true));
    }

    /// Every example of every figure (and an edit of every param type) survives the link.
    #[test]
    fn every_example_round_trips() {
        for d in registry() {
            let defs = (d.params)();
            let base = start_params(&d);
            let mut cases: Vec<Params> = examples_for(d.id)
                .iter()
                .map(|e| {
                    let mut p = Params::defaults(&defs);
                    for (k, v) in &e.params {
                        p.0.insert(k.to_string(), v.clone());
                    }
                    p.normalize(&defs, false);
                    p
                })
                .collect();
            let mut tweaked = base.clone();
            for pd in &defs {
                let v = match tweaked.0.get(pd.key) {
                    Some(Value::F(f)) => Value::F(f + 0.1),
                    Some(Value::I(i)) => Value::I(i + 1),
                    Some(Value::B(b)) => Value::B(!b),
                    Some(Value::S(s)) => Value::S(format!("{s}x")),
                    None => continue,
                };
                tweaked.0.insert(pd.key.to_string(), v);
            }
            cases.push(tweaked);
            for p in cases {
                let link = encode(d.id, &p, &base);
                let (id, m) = decode(&link).unwrap_or_else(|e| panic!("{}: {e}", d.id));
                assert_eq!(id, d.id);
                let mut back = base.clone();
                for (k, j) in m {
                    let v = match (base.0.get(&k), j) {
                        (Some(Value::F(_)), j) => Value::F(j.as_f64().unwrap()),
                        (Some(Value::I(_)), j) => Value::I(j.as_i64().unwrap()),
                        (Some(Value::B(_)), j) => Value::B(j.as_bool().unwrap()),
                        (_, j) => Value::S(j.as_str().unwrap().to_string()),
                    };
                    back.0.insert(k, v);
                }
                assert_eq!(format!("{:?}", back.0), format!("{:?}", p.0), "{} via {link}", d.id);
            }
        }
    }
}
