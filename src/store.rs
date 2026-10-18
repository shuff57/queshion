//! Local-only persistence. JSON documents in the store dir; images under media/ keyed by sha256.
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{de::DeserializeOwned, Serialize};
use crate::model::{Exam, Id, Question, QuestionBank};

fn data_base() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn store_dir() -> PathBuf {
    data_base().join("vershion")
}

/// One-time, non-destructive copy of the pre-rename data dir (`…/queshion`).
/// Runs only when `…/vershion` is absent or empty; the old dir is left untouched.
#[cfg(not(target_arch = "wasm32"))]
pub fn migrate_legacy_dir() {
    let base = data_base();
    let old = base.join("queshion");
    let new = base.join("vershion");
    if !old.is_dir() {
        return;
    }
    let new_empty = std::fs::read_dir(&new)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true);
    if !new_empty {
        return;
    }
    if let Err(e) = copy_tree(&old, &new) {
        eprintln!("verSHion: could not copy legacy data from {}: {e}", old.display());
    } else {
        eprintln!("verSHion: copied legacy data from {} to {}", old.display(), new.display());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&src, &dst)?;
        } else if entry.file_type()?.is_file() {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Atomic JSON save: write `file.tmp` then rename. Creates parent dirs.
pub fn save_json<T: Serialize>(name: &str, value: &T) -> Result<(), String> {
    let dir = store_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(name);
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn load_json<T: DeserializeOwned>(name: &str) -> Option<T> {
    let bytes = std::fs::read(store_dir().join(name)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Stores image bytes under media/<sha256><ext> and returns the ImageRef.
pub fn put_image(bytes: &[u8]) -> Result<crate::model::ImageRef, String> {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(bytes);
    let hex = format!("{hash:x}");
    let ext = sniff_ext(bytes);
    let file = format!("media/{hex}{ext}");
    let dir = store_dir();
    let out = dir.join(&file);
    if !out.exists() {
        std::fs::create_dir_all(dir.join("media")).map_err(|e| e.to_string())?;
        std::fs::write(&out, bytes).map_err(|e| e.to_string())?;
    }
    let (w, h) = imagesize(bytes).ok_or("unreadable image")?;
    Ok(crate::model::ImageRef { file, width: w, height: h })
}

/// Where automatic backups go: `<store>/backups`.
pub fn backup_dir() -> PathBuf {
    store_dir().join("backups")
}

/// "20261017-235201" for a unix time (UTC), for backup file names.
pub fn backup_stamp(secs: i64) -> String {
    let (y, m, d) = crate::screens::civil_from_days(secs.div_euclid(86400));
    let t = secs.rem_euclid(86400);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", t / 3600, t % 3600 / 60, t % 60)
}

/// Saves a copy of the banks before a bulk change: `banks.json` from the store as it is on
/// disk (or `banks` serialized, when there is no file yet) to `<backups>/banks-<stamp>.json`,
/// never overwriting an earlier backup. Returns the backup's path.
pub fn backup_banks_in(store: &std::path::Path, backups: &std::path::Path, banks: &[QuestionBank], secs: i64) -> Result<PathBuf, String> {
    std::fs::create_dir_all(backups).map_err(|e| e.to_string())?;
    let stamp = backup_stamp(secs);
    let mut path = backups.join(format!("banks-{stamp}.json"));
    let mut n = 2;
    while path.exists() {
        path = backups.join(format!("banks-{stamp}-{n}.json"));
        n += 1;
    }
    let bytes = match std::fs::read(store.join("banks.json")) {
        Ok(b) => b,
        Err(_) => serde_json::to_vec(banks).map_err(|e| e.to_string())?,
    };
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// [`backup_banks_in`] for the app's own store.
pub fn backup_banks(banks: &[QuestionBank]) -> Result<PathBuf, String> {
    backup_banks_in(&store_dir(), &backup_dir(), banks, now())
}

/// The store directory as text (headless hooks print it so tests can find media files).
pub fn store_dir_string() -> String {
    store_dir().to_string_lossy().into_owned()
}

pub fn media_path(image: &crate::model::ImageRef) -> PathBuf {
    store_dir().join(&image.file)
}

pub fn sniff_ext(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") { ".png" }
    else if bytes.starts_with(b"\xff\xd8") { ".jpg" }
    else if bytes.starts_with(b"GIF8") { ".gif" }
    else if bytes.starts_with(&[0x49, 0x49, 0x2a]) || bytes.starts_with(&[0x4d, 0x4d, 0x00]) { ".tiff" }
    else if bytes.starts_with(b"RIFF") { ".webp" }
    else { ".bin" }
}

fn imagesize(bytes: &[u8]) -> Option<(u32, u32)> {
    // PNG
    if bytes.starts_with(b"\x89PNG") && bytes.len() >= 24 {
        let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        return Some((be(&bytes[16..20]), be(&bytes[20..24])));
    }
    // GIF
    if bytes.starts_with(b"GIF8") && bytes.len() >= 10 {
        let le = |b: &[u8]| u32::from(u16::from_le_bytes([b[0], b[1]]));
        return Some((le(&bytes[6..8]), le(&bytes[8..10])));
    }
    // JPEG SOF scan
    if bytes.starts_with(b"\xff\xd8") {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xff { i += 1; continue; }
            let marker = bytes[i + 1];
            if (0xc0..=0xc3).contains(&marker) {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
                return Some((w, h));
            }
            let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            i += 2 + len;
        }
    }
    None
}

/// One shared Store the whole app reads and mutates. Saves the affected file on every mutation.
#[derive(Default)]
pub struct Store {
    pub banks: Vec<QuestionBank>,
    pub exams: Vec<Exam>,
    pub imports: Vec<crate::model::Import>,
    pub exports: Vec<crate::model::ExportRecord>,
}

impl Store {
    /// Loads all four JSON documents; missing or corrupt files yield empty vectors.
    pub fn load() -> Store {
        Store {
            banks: load_json("banks.json").unwrap_or_default(),
            exams: load_json("exams.json").unwrap_or_default(),
            imports: load_json("imports.json").unwrap_or_default(),
            exports: load_json("exports.json").unwrap_or_default(),
        }
    }

    pub fn save_banks(&self) { let _ = save_json("banks.json", &self.banks); }
    pub fn save_exams(&self) { let _ = save_json("exams.json", &self.exams); }
    pub fn save_imports(&self) { let _ = save_json("imports.json", &self.imports); }
    pub fn save_exports(&self) { let _ = save_json("exports.json", &self.exports); }

    pub fn bank(&self, id: &Id) -> Option<&QuestionBank> {
        self.banks.iter().find(|b| b.id == *id)
    }

    pub fn bank_mut(&mut self, id: &Id) -> Option<&mut QuestionBank> {
        self.banks.iter_mut().find(|b| b.id == *id)
    }

    pub fn exam(&self, id: &Id) -> Option<&Exam> {
        self.exams.iter().find(|e| e.id == *id)
    }

    pub fn exam_mut(&mut self, id: &Id) -> Option<&mut Exam> {
        self.exams.iter_mut().find(|e| e.id == *id)
    }

    pub fn question(&self, bank_id: &Id, question_id: &Id) -> Option<&Question> {
        self.bank(bank_id)?.questions.iter().find(|q| q.id == *question_id)
    }

    pub fn question_mut(&mut self, bank_id: &Id, question_id: &Id) -> Option<&mut Question> {
        self.bank_mut(bank_id)?.questions.iter_mut().find(|q| q.id == *question_id)
    }
}