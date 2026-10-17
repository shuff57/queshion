//! Headless end-to-end check of every import format: each fixture in
//! `tests/import_fixtures/` goes through the app binary's `QUESHION_PARSE=<file>` hook
//! (the same `formats::detect_and_parse` path the Import screen uses) and the printed
//! JSON is checked. Each run gets its own temporary store (`XDG_DATA_HOME`) so archive
//! images are really written there, never into the user's store, and every run is
//! killed after a timeout.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/import_fixtures").join(name)
}

/// Runs the binary's parse hook on one fixture → (questions JSON array, store dir).
fn parse(name: &str) -> (Vec<serde_json::Value>, PathBuf) {
    let store = std::env::temp_dir().join(format!("queshion-import-test-{}-{}", std::process::id(), name.replace('.', "_")));
    let _ = std::fs::remove_dir_all(&store);
    std::fs::create_dir_all(&store).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_queshion"))
        .env("QUESHION_PARSE", fixture(name))
        .env("XDG_DATA_HOME", &store)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn queshion");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("{name}: QUESHION_PARSE did not finish within 60 s");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{name}: {}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{name}: bad JSON ({e})"));
    let qs = v["questions"].as_array().cloned().unwrap_or_default();
    (qs, store.join("queshion"))
}

fn kind(q: &serde_json::Value) -> String {
    q["kind"].as_object().and_then(|o| o.keys().next().cloned()).unwrap_or_default()
}

fn kinds(qs: &[serde_json::Value]) -> Vec<String> {
    qs.iter().map(kind).collect()
}

fn check(name: &str, want: &[&str]) -> (Vec<serde_json::Value>, PathBuf) {
    let (qs, store) = parse(name);
    assert_eq!(kinds(&qs), want, "{name}");
    (qs, store)
}

fn cleanup(store: &Path) {
    if let Some(parent) = store.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

#[test]
fn gift_headless() {
    let (qs, s) = check("moodle_sample.gift", &["Mc", "Mc", "Tf", "Tf", "Blank", "Blank", "Blank", "Blank", "Blank", "Matching", "Short", "Mc", "Mc", "Mc"]);
    assert_eq!(qs[5]["stem"], "The chemical symbol for sodium is ___ on the periodic table.");
    cleanup(&s);
}

#[test]
fn flashcard_lists_headless() {
    for (f, want) in [("quizlet_terms.txt", vec!["Matching"]), ("anki_notes.txt", vec!["Short"; 3]), ("vocab_list.csv", vec!["Matching"]), ("vocab.xlsx", vec!["Matching"])] {
        let (_, s) = check(f, &want);
        cleanup(&s);
    }
}

#[test]
fn numbered_text_headless() {
    let (_, s) = check("respondus.txt", &["Mc", "Mc", "Tf", "Short", "Blank", "Matching", "Blank", "Mc"]);
    cleanup(&s);
    let (qs, s) = check("text2qti.md", &["Mc", "Mc", "Blank", "Blank", "Blank", "Short", "Short"]);
    assert_eq!(qs[0]["points"], 2.0);
    cleanup(&s);
    let (_, s) = check("aiken.txt", &["Mc", "Mc"]);
    cleanup(&s);
}

#[test]
fn spreadsheets_headless() {
    let (qs, s) = check("questions.xlsx", &["Mc", "Mc", "Tf", "Short", "Blank", "Mc"]);
    assert_eq!(qs[1]["topics"], serde_json::json!(["astronomy", "planets"]));
    cleanup(&s);
    let (_, s) = check("kahoot.xlsx", &["Mc", "Mc", "Tf"]);
    cleanup(&s);
    let (_, s) = check("brightspace.csv", &["Mc", "Tf", "Mc", "Blank", "Short", "Matching", "Blank", "Blank"]);
    cleanup(&s);
}

#[test]
fn moodle_xml_headless_stores_the_embedded_image() {
    let (_, s) = check("moodle_quiz.xml", &["Mc", "Mc", "Tf", "Blank", "Blank", "Blank", "Matching", "Short", "Blank", "Blank", "Mc"]);
    cleanup(&s);
    let (qs, store) = check("moodle_image.xml", &["Tf"]);
    let file = qs[0]["image"]["file"].as_str().expect("the base64 <file> becomes the picture");
    assert!(store.join(file).is_file(), "image written into the temporary store: {file}");
    assert_eq!((qs[0]["image"]["width"].as_u64(), qs[0]["image"]["height"].as_u64()), (Some(4), Some(3)));
    cleanup(&store);
}

#[test]
fn qti_files_and_packages_headless() {
    for f in ["qti12_canvas_quiz.xml", "canvas_qti_export.zip"] {
        let (qs, store) = check(f, &["Mc", "Tf", "Mc", "Blank", "Short", "Matching", "Blank", "Blank", "Blank", "Short"]);
        let has_image = qs[0]["image"].is_object();
        assert_eq!(has_image, f.ends_with(".zip"), "{f}: only the zip carries the image bytes");
        if has_image {
            assert!(store.join(qs[0]["image"]["file"].as_str().unwrap()).is_file());
        }
        cleanup(&store);
    }
    let (_, s) = check("qti12_blackboard.xml", &["Mc", "Tf", "Blank"]);
    cleanup(&s);
    let (qs, store) = check("qti21_package.zip", &["Mc", "Matching", "Blank", "Blank", "Blank", "Short", "Blank", "Multipart"]);
    assert!(store.join(qs[0]["image"]["file"].as_str().expect("flask.png attached")).is_file());
    cleanup(&store);
    let (_, s) = check("qti21_choice_multiple.xml", &["Mc"]);
    cleanup(&s);
    let (_, s) = check("qti3_item.xml", &["Mc"]);
    cleanup(&s);
}
