//! LLM conversion of a test photo/scan/screenshot into Blackboard import text (see
//! `formats::parse_blackboard` for the exact grammar the model must produce).
//!
//! One conversion job at a time: `send_image` spawns a `std::thread` (egui never blocks)
//! and keeps the result channel in [`Ai`]; the Imports screen calls `poll` each frame and
//! the thread calls `ctx.request_repaint()` when it finishes, so the parsed questions show
//! up on the next frame. API keys come from, in order: a key saved in the app (`ai-keys.json`,
//! mode 0600, never in settings.json or backups), the provider's environment variable, and
//! (Ollama Cloud) the ollama-cloud tool's credentials file. They are read at request time
//! and never logged, echoed, put in errors or Debug output.

pub use crate::Settings;

use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

const OLLAMA_DEFAULT_BASE: &str = "http://127.0.0.1:11434";
const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";
/// Small local vision model that a laptop GPU can carry through a slow image round trip.
const OLLAMA_DEFAULT_MODEL: &str = "gemma4:31b";
/// OpenRouter default mirrors the local Ollama model on the free tier; set `ai_model` to override.
const OPENROUTER_DEFAULT_MODEL: &str = "qwen/qwen3-vl-8b-instruct:free";
/// Custom OpenAI-compatible endpoints default to the same model as local Ollama unless
/// `ai_model` says otherwise (the base URL decides what that model id actually means).
const CUSTOM_DEFAULT_MODEL: &str = "gemma4:31b";
/// Images are slow, on small local GPUs especially.
const TIMEOUT: Duration = Duration::from_secs(300);

const OPENROUTER_KEY_ENV: &str = "OPENROUTER_API_KEY";
const CUSTOM_KEY_ENV: &str = "VERSHION_AI_KEY";
/// Ollama's hosted service: same OpenAI-compatible surface (`/v1`), keyed.
const OLLAMA_CLOUD_BASE: &str = "https://ollama.com/v1";
const OLLAMA_CLOUD_DEFAULT_MODEL: &str = "gemma4:31b";
/// Ollama Cloud's key: `OLLAMA_API_KEY`, else the ollama-cloud tool's own credentials file.
const CLOUD_KEY_ENV: &str = "OLLAMA_API_KEY";
const CLOUD_KEY_FILE: &str = ".config/ollama-cloud/credentials.json";

/// The conversion prompt: one page in, one verSHion Ditto package (`docs/ditto-format.md`) out.
const TRANSCRIBE_PROMPT: &str = include_str!("../prompts/ditto-convert.md");

/// The instruction plus the user's typed note about the source, if any.
/// Public so the headless `VERSHION_CONVERT` path shares the UI's prompt.
pub fn prompt_for(hint: &str) -> String {
    transcription_prompt(hint)
}

fn transcription_prompt(hint: &str) -> String {
    let hint = hint.trim();
    if hint.is_empty() {
        return TRANSCRIBE_PROMPT.to_string();
    }
    format!("{TRANSCRIBE_PROMPT}\n\nUser note about this source:\n{hint}")
}

#[derive(Clone, Debug)]
pub struct AiCfg {
    /// Normalized provider: "ollama" (the default), "openrouter", or "custom".
    pub provider: String,
    pub base: String,
    pub model: String,
}

impl AiCfg {
    pub fn from(s: &Settings) -> AiCfg {
        let provider = match s.ai_provider.trim().to_ascii_lowercase().as_str() {
            "openrouter" => "openrouter",
            "custom" => "custom",
            "ollama-cloud" | "cloud" | "ollama cloud" => "cloud",
            // Empty or unknown settings fall back to local Ollama.
            _ => "ollama",
        };
        let model: String = if s.ai_model.trim().is_empty() {
            match provider {
                "openrouter" => OPENROUTER_DEFAULT_MODEL.to_string(),
                "custom" => CUSTOM_DEFAULT_MODEL.to_string(),
                "cloud" => OLLAMA_CLOUD_DEFAULT_MODEL.to_string(),
                _ => OLLAMA_DEFAULT_MODEL.to_string(),
            }
        } else {
            s.ai_model.trim().to_string()
        };
        let base: String = match provider {
            "openrouter" => OPENROUTER_BASE.to_string(),
            "cloud" => OLLAMA_CLOUD_BASE.to_string(),
            "custom" => s.ai_base.trim().trim_end_matches('/').to_string(),
            _ => OLLAMA_DEFAULT_BASE.to_string(),
        };
        AiCfg { provider: provider.to_string(), base, model }
    }

    /// One human sentence about where the API key for this provider comes from.
    /// Order: a key saved in the app wins, then the environment variable, then (Ollama
    /// Cloud only) the ollama-cloud credentials file.
    pub fn key_source_help(&self) -> String {
        match self.provider.as_str() {
            "openrouter" => format!(
                "OpenRouter needs an API key. A key saved here is used first, otherwise the {OPENROUTER_KEY_ENV} environment variable."
            ),
            "cloud" => format!(
                "Ollama Cloud needs an API key. A key saved here is used first, then the {CLOUD_KEY_ENV} environment variable, then ~/{CLOUD_KEY_FILE}."
            ),
            "custom" => format!(
                "A custom endpoint may need an API key. A key saved here is used first, otherwise the {CUSTOM_KEY_ENV} environment variable."
            ),
            _ => "Ollama runs on the local machine and needs no API key.".to_string(),
        }
    }

    /// True for providers that take an API key (everything but local Ollama).
    pub fn uses_key(&self) -> bool {
        self.key_env().is_some()
    }

    /// Where the key would come from right now, for the Settings status line.
    pub fn key_status(&self) -> KeySource {
        resolve_key(self, &load_keys(), &|v| std::env::var(v).ok(), home_dir().as_deref()).map(|(_, s)| s).unwrap_or(KeySource::None)
    }

    /// True when a request would carry a key (or the provider needs none).
    pub fn has_key_or_needs_none(&self) -> bool {
        !self.uses_key() || self.provider == "custom" || self.key_status() != KeySource::None
    }

    fn key_env(&self) -> Option<&'static str> {
        match self.provider.as_str() {
            "openrouter" => Some(OPENROUTER_KEY_ENV),
            "custom" => Some(CUSTOM_KEY_ENV),
            "cloud" => Some(CLOUD_KEY_ENV),
            _ => None,
        }
    }
}

/// Where a resolved key came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    Saved,
    Env(&'static str),
    CredentialsFile,
    None,
}

impl KeySource {
    pub fn describe(self) -> String {
        match self {
            KeySource::Saved => "Key saved on this computer".to_string(),
            KeySource::Env(v) => format!("Using {v} from the environment"),
            KeySource::CredentialsFile => format!("Using ~/{CLOUD_KEY_FILE}"),
            KeySource::None => "No key".to_string(),
        }
    }
}

/// Provider -> key, as stored in `ai-keys.json`. `Debug` is deliberately redacted.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Keys(std::collections::BTreeMap<String, String>);

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keys({} saved, redacted)", self.0.len())
    }
}

impl Keys {
    pub fn get(&self, provider: &str) -> Option<&str> {
        self.0.get(provider).map(String::as_str).filter(|k| !k.is_empty())
    }
}

const KEYS_FILE: &str = "ai-keys.json";

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

pub fn load_keys() -> Keys {
    load_keys_in(&crate::store::store_dir())
}
pub fn load_keys_in(dir: &std::path::Path) -> Keys {
    std::fs::read(dir.join(KEYS_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Saves (`Some`) or removes (`None`) one provider's key.
pub fn set_key(provider: &str, key: Option<&str>) -> Result<(), String> {
    set_key_in(&crate::store::store_dir(), provider, key)
}

/// Atomic write (temp + rename) of the whole map, mode 0600 on unix from creation.
pub fn set_key_in(dir: &std::path::Path, provider: &str, key: Option<&str>) -> Result<(), String> {
    use std::io::Write as _;
    let mut keys = load_keys_in(dir);
    match key.map(str::trim).filter(|k| !k.is_empty()) {
        Some(k) => {
            keys.0.insert(provider.to_string(), k.to_string());
        }
        None => {
            keys.0.remove(provider);
        }
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't save the key: {}", e.kind()))?;
    let tmp = dir.join(format!("{KEYS_FILE}.tmp"));
    let path = dir.join(KEYS_FILE);
    let body = serde_json::to_vec(&keys).map_err(|_| "Couldn't save the key.".to_string())?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let res = (|| -> std::io::Result<()> {
        let mut f = opts.open(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(&body)?;
        f.sync_all()?;
        std::fs::rename(&tmp, &path)
    })();
    res.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Couldn't save the key: {}", e.kind())
    })
}

/// Pure resolution: saved key, then env var, then (cloud) the credentials file under `home`.
fn resolve_key(
    cfg: &AiCfg,
    saved: &Keys,
    env: &dyn Fn(&str) -> Option<String>,
    home: Option<&std::path::Path>,
) -> Option<(String, KeySource)> {
    let var = cfg.key_env()?;
    if let Some(k) = saved.get(&cfg.provider) {
        return Some((k.to_string(), KeySource::Saved));
    }
    if let Some(k) = env(var).filter(|k| !k.is_empty()) {
        return Some((k, KeySource::Env(var)));
    }
    if cfg.provider == "cloud"
        && let Some(bytes) = home.and_then(|h| std::fs::read(h.join(CLOUD_KEY_FILE)).ok())
        && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
        && let Some(k) = v.get("apiKey").and_then(Value::as_str)
        && !k.is_empty()
    {
        return Some((k.to_string(), KeySource::CredentialsFile));
    }
    None
}

/// The API key for a request, resolved at request time. Never logged or echoed.
fn request_key(cfg: &AiCfg) -> Option<String> {
    resolve_key(cfg, &load_keys(), &|v| std::env::var(v).ok(), home_dir().as_deref()).map(|(k, _)| k)
}

/// Cheap authenticated probe for the Settings "Test connection" button: GET {base}/models.
/// Blocking; call from a thread. The reply never contains the key.
pub fn test_connection(cfg: &AiCfg) -> Result<(), String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = if cfg.provider == "ollama" { format!("{base}/api/tags") } else { format!("{base}/models") };
    let mut req = agent().get(&url);
    if let Some(k) = request_key(cfg) {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let resp = req.call().map_err(|_| format!("Can't reach {base}."))?;
    match resp.status().as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err("Key rejected (401).".to_string()),
        404 => Err("Reached the server, but it has no /models endpoint (404). The key may still work.".to_string()),
        n => Err(format!("The server answered {n}.")),
    }
}

/// One pending conversion job; `Default` starts with none.
#[derive(Default)]
pub struct Ai {
    job: Option<Receiver<Result<Converted, String>>>,
}

impl Ai {
    /// Starts converting one image in the background. The channel delivers the model's
    /// reply with [`extract_import_text`] already applied, so `poll` returns import-ready
    /// text; the thread asks egui to repaint the moment it is done.
    pub fn send_image(&mut self, bytes: Vec<u8>, mime: &str, hint: String, page_no: usize, cfg: &AiCfg, ctx: &egui::Context) {
        let (tx, rx) = channel();
        self.job = Some(rx);
        let cfg = cfg.clone();
        let mime = mime.to_string();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(convert_page(&bytes, &mime, &hint, &cfg, page_no));
            ctx.request_repaint();
        });
    }

    /// Picks up a finished job, if any. Called every frame from the Imports screen.
    pub fn poll(&mut self) -> Option<Result<Converted, String>> {
        let Some(rx) = self.job.as_mut() else { return None };
        match rx.try_recv() {
            Ok(r) => {
                self.job = None;
                Some(r)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.job = None;
                Some(Err("the AI job ended without sending a result".to_string()))
            }
        }
    }
}

/// One round trip: Ollama's native `/api/chat` or an OpenAI-compatible `/chat/completions`.
/// Also the entry point for the headless `VERSHION_CONVERT` path.
pub fn transcribe(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str) -> Result<String, String> {
    transcribe_at(bytes, mime, cfg, prompt, 0.0)
}

fn transcribe_at(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    match cfg.provider.as_str() {
        "ollama" => ollama_chat(cfg, prompt, &[bytes], temp),
        _ => openai_chat(cfg, prompt, mime, &[bytes], temp),
    }
}

/// A question to the model about one or more PNG images, answered with JSON.
pub fn ask_images(images: &[&[u8]], cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    match cfg.provider.as_str() {
        "ollama" => ollama_chat(cfg, prompt, images, temp),
        _ => openai_chat(cfg, prompt, "image/png", images, temp),
    }
}

/// What one converted page yields.
#[derive(Debug)]
pub struct Converted {
    pub doc: crate::formats::ImportDoc,
    /// Skipped questions, dropped figures, figures kept as plain images.
    pub warnings: Vec<String>,
    /// Pictures cropped from the page, and how many of those became editable Figures-tab figures.
    pub cropped: usize,
    pub redrawn: usize,
}

/// The whole Import path for one page image: model reply -> Ditto parse (figures cropped) ->
/// figure fitting (crops become editable Figures-tab figures when a generator matches).
pub fn convert_page(bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize) -> Result<Converted, String> {
    let raw = transcribe_checked(bytes, mime, cfg, &transcription_prompt(hint))?;
    let text = extract_import_text(&raw);
    if text.is_empty() {
        return Err("The model returned no text.".to_string());
    }
    if !crate::ditto::is_ditto(&text) {
        let doc = crate::formats::detect_and_parse(text.as_bytes(), "converted")?;
        return Ok(Converted { doc, warnings: Vec::new(), cropped: 0, redrawn: 0 });
    }
    let mut r = crate::ditto::import_text(&text, Some(bytes), page_no, "converted")?;
    let mut redrawn = 0;
    for (i, q) in r.doc.questions.iter_mut().enumerate() {
        if q.image.is_none() || q.figure.is_some() {
            continue;
        }
        let crop = std::fs::read(crate::store::media_path(q.image.as_ref().expect("checked"))).unwrap_or_default();
        match crate::figfit::fit(&crop, &q.stem, cfg) {
            Ok(Some(f)) => match crate::store::put_image(&f.png) {
                Ok(img) => {
                    q.image = Some(img);
                    q.figure = Some(f.spec);
                    redrawn += 1;
                }
                Err(e) => r.warnings.push(format!("Question {}: couldn't save the redrawn figure ({e}).", i + 1)),
            },
            Ok(None) => r.warnings.push(format!("Question {}: no Figures-tab generator matched its picture, kept the cropped image.", i + 1)),
            Err(e) => r.warnings.push(format!("Question {}: figure fitting failed ({e}), kept the cropped image.", i + 1)),
        }
    }
    Ok(Converted { doc: r.doc, warnings: r.warnings, cropped: r.figures, redrawn })
}

/// `transcribe`, then up to two retries when the reply isn't valid JSON: the error is quoted
/// back to the model and the temperature nudged up so it doesn't repeat the same slip.
pub fn transcribe_checked(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str) -> Result<String, String> {
    let mut reply = transcribe_at(bytes, mime, cfg, prompt, 0.0)?;
    for attempt in 1..=2 {
        let Some(err) = crate::ditto::json_error(&reply) else { break };
        let again = format!("{prompt}\n\nYour previous reply was not valid JSON ({err}). Output the complete package again as strictly valid JSON: commas between members, no trailing commas, no comments.");
        match transcribe_at(bytes, mime, cfg, &again, 0.2 * attempt as f64) {
            Ok(r) => reply = r,
            Err(_) => break,
        }
    }
    Ok(reply)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).http_status_as_error(false).build().into()
}

/// Ollama's native chat API: images ride in the message's `images` array as raw base64.
fn ollama_chat(cfg: &AiCfg, prompt: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    let url = format!("{}/api/chat", cfg.base.trim_end_matches('/'));
    let body = json!({
        "model": cfg.model,
        "messages": [{"role": "user", "content": prompt, "images": images.iter().map(|b| base64_encode(b)).collect::<Vec<_>>()}],
        "stream": false,
        "format": "json",
        "options": {"temperature": temp},
    });
    let reply = post_json(&url, body, None)?;
    reply
        .pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The Ollama reply had no message content.".to_string())
}

/// OpenRouter and any custom OpenAI-compatible endpoint, with the image as a data URI.
fn openai_chat(cfg: &AiCfg, prompt: &str, mime: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL for the custom AI endpoint in Settings.".to_string());
    }
    let url = format!("{base}/chat/completions");
    let mut content = vec![json!({"type": "text", "text": prompt})];
    for b in images {
        content.push(json!({"type": "image_url", "image_url": {"url": format!("data:{mime};base64,{}", base64_encode(b))}}));
    }
    let body = json!({
        "model": cfg.model,
        "stream": false,
        "temperature": temp,
        "response_format": {"type": "json_object"},
        "messages": [{"role": "user", "content": content}],
    });
    let key = request_key(cfg);
    let reply = post_json(&url, body, key.as_deref())?;
    reply
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The reply had no message content.".to_string())
}

/// One JSON POST, with the same error shapes for both providers; answers with the body.
fn post_json(url: &str, body: Value, key: Option<&str>) -> Result<Value, String> {
    let mut req = agent().post(url).header("Content-Type", "application/json");
    if let Some(k) = key {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = req.send(body.to_string()).map_err(|e| format!("Can't reach {url}: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from {url}: {e}"))?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{url} did not answer with JSON ({e}): {}", text.chars().take(160).collect::<String>()))?;
    if !(200..300).contains(&status) {
        let msg = v.pointer("/error/message").and_then(Value::as_str).or_else(|| v.get("error").and_then(Value::as_str)).unwrap_or("request failed");
        return Err(format!("{url} answered {status}: {msg}"));
    }
    Ok(v)
}

/// Strips an accidental markdown fence (with its language tag) and outer whitespace; the
/// `formats` parsers tolerate everything else a model might add around the import text.
pub fn extract_import_text(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("```")
        && let Some((_, body)) = rest.split_once('\n')
    {
        let body = body.trim_end();
        let body = body.strip_suffix("```").map_or(body, str::trim_end);
        s = body;
    }
    s.trim().to_string()
}

/// Standard base64 (RFC 4648, with padding). Small local encoder; base64 is not a crate here.
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let a = u32::from(chunk[0]);
        let b = chunk.get(1).copied().map_or(0, u32::from);
        let c = chunk.get(2).copied().map_or(0, u32::from);
        let n = (a << 16) | (b << 8) | c;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(provider: &str, model: &str, base: &str) -> Settings {
        Settings { ai_provider: provider.into(), ai_model: model.into(), ai_base: base.into(), dark: bool::default(), ..Default::default() }
    }

    #[test]
    fn prompt_names_every_type_and_the_figure_rules() {
        let p = transcription_prompt("");
        for ty in ["multiple-choice", "true-false", "matching", "fill-in-the-blank", "short-answer", "multipart"] {
            assert!(p.contains(ty), "prompt must name {ty}");
        }
        for must in ["___", "\"box\"", "queshion/ditto", "No markdown fences", "FRACTIONS"] {
            assert!(p.contains(must), "prompt must mention {must}");
        }
    }

    #[test]
    fn prompt_appends_the_user_hint() {
        let p = transcription_prompt("page 2 is multiple choice");
        assert!(p.contains("User note about this source:\npage 2 is multiple choice"), "{p}");
        // Empty hint changes nothing.
        assert_eq!(transcription_prompt("  "), TRANSCRIBE_PROMPT);
    }

    #[test]
    fn extract_strips_fences_and_whitespace() {
        assert_eq!(extract_import_text("  \n```text\nMC\nQ?\n*a. 1\n```\n  "), "MC\nQ?\n*a. 1");
        assert_eq!(extract_import_text("```\nMC\nQ?\n*a. 1\n```"), "MC\nQ?\n*a. 1");
        assert_eq!(extract_import_text(" MC\nQ? "), "MC\nQ?");
        // No content line after the fence: left as-is for the parsers to report.
        assert_eq!(extract_import_text("```junk"), "```junk");
    }

    #[test]
    fn cfg_maps_the_three_providers() {
        let c = AiCfg::from(&settings("ollama", "", ""));
        assert_eq!(c.provider, "ollama");
        assert_eq!(c.base, OLLAMA_DEFAULT_BASE);
        assert_eq!(c.model, OLLAMA_DEFAULT_MODEL);

        let c = AiCfg::from(&settings("openrouter", "vendor/my-model", ""));
        assert_eq!(c.provider, "openrouter");
        assert_eq!(c.base, OPENROUTER_BASE);
        assert_eq!(c.model, "vendor/my-model");

        let c = AiCfg::from(&settings("custom", "", "https://api.example.com/v1/"));
        assert_eq!(c.provider, "custom");
        assert_eq!(c.base, "https://api.example.com/v1");
        assert_eq!(c.model, CUSTOM_DEFAULT_MODEL);

        // Empty or unknown provider settings fall back to local Ollama.
        let c = AiCfg::from(&Settings::default());
        assert_eq!(c.provider, "ollama");
        assert_eq!(c.model, OLLAMA_DEFAULT_MODEL);
    }

    #[test]
    fn key_help_names_the_env_var() {
        assert!(AiCfg::from(&settings("openrouter", "", "")).key_source_help().contains(OPENROUTER_KEY_ENV));
        assert!(AiCfg::from(&settings("custom", "", "")).key_source_help().contains(CUSTOM_KEY_ENV));
        assert!(AiCfg::from(&settings("ollama", "", "")).key_source_help().contains("no API key"));
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn poll_reports_a_job_that_ended_without_a_result() {
        let (tx, rx) = channel::<Result<Converted, String>>();
        drop(tx);
        let mut ai = Ai { job: Some(rx) };
        assert!(matches!(ai.poll(), Some(Err(_))), "a dropped sender is an error, not silence");
        assert!(ai.poll().is_none(), "the ended job must not stick");
    }

    #[test]
    fn poll_silently_waits_for_a_running_job() {
        let (tx, rx) = channel::<Result<Converted, String>>();
        let mut ai = Ai { job: Some(rx) };
        assert!(ai.poll().is_none());
        assert!(ai.job.is_some());
        let doc = crate::formats::detect_and_parse(b"MC\nQ?\n*a. 1\nb. 2\n", "x").unwrap();
        tx.send(Ok(Converted { doc, warnings: Vec::new(), cropped: 0, redrawn: 0 })).unwrap();
        let _ = tx; // keep the channel alive until the reply is read
        assert_eq!(ai.poll().unwrap().unwrap().doc.questions.len(), 1);
        assert!(ai.poll().is_none(), "the job is consumed once read");
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vershion-keytest-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn key_store_roundtrip_remove_and_perms() {
        let d = tmpdir("rt");
        assert!(load_keys_in(&d).get("cloud").is_none());
        set_key_in(&d, "cloud", Some("  sk-dummy-1 ")).unwrap();
        set_key_in(&d, "openrouter", Some("sk-dummy-2")).unwrap();
        let k = load_keys_in(&d);
        assert_eq!(k.get("cloud"), Some("sk-dummy-1"));
        assert_eq!(k.get("openrouter"), Some("sk-dummy-2"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let m = std::fs::metadata(d.join(KEYS_FILE)).unwrap().permissions().mode();
            assert_eq!(m & 0o777, 0o600);
        }
        assert!(!d.join("ai-keys.json.tmp").exists());
        set_key_in(&d, "cloud", None).unwrap();
        assert!(load_keys_in(&d).get("cloud").is_none());
        assert_eq!(load_keys_in(&d).get("openrouter"), Some("sk-dummy-2"));
        assert!(!format!("{:?}", load_keys_in(&d)).contains("sk-dummy"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn key_resolution_order_saved_env_file() {
        let home = tmpdir("home");
        std::fs::create_dir_all(home.join(".config/ollama-cloud")).unwrap();
        std::fs::write(home.join(CLOUD_KEY_FILE), r#"{"apiKey":"from-file"}"#).unwrap();
        let cfg = AiCfg::from(&settings("cloud", "", ""));
        let env_set = |v: &str| (v == CLOUD_KEY_ENV).then(|| "from-env".to_string());
        let env_none = |_: &str| None;
        let mut saved = Keys::default();
        // file only
        let r = resolve_key(&cfg, &saved, &env_none, Some(&home)).unwrap();
        assert_eq!((r.0.as_str(), r.1), ("from-file", KeySource::CredentialsFile));
        // env beats file
        let r = resolve_key(&cfg, &saved, &env_set, Some(&home)).unwrap();
        assert_eq!((r.0.as_str(), r.1), ("from-env", KeySource::Env(CLOUD_KEY_ENV)));
        // saved beats env
        saved.0.insert("cloud".into(), "from-app".into());
        let r = resolve_key(&cfg, &saved, &env_set, Some(&home)).unwrap();
        assert_eq!((r.0.as_str(), r.1), ("from-app", KeySource::Saved));
        // openrouter ignores the credentials file; local ollama never has a key
        let or = AiCfg::from(&settings("openrouter", "", ""));
        assert!(resolve_key(&or, &Keys::default(), &env_none, Some(&home)).is_none());
        let local = AiCfg::from(&settings("ollama", "", ""));
        assert!(resolve_key(&local, &saved, &env_set, Some(&home)).is_none());
        let _ = std::fs::remove_dir_all(&home);
    }
}
