//! Unicode-version canary. The character rules depend on general categories (Cn = unassigned is
//! refused) and NFC on the normalisation tables; the app and the Worker share this crate, so they
//! agree as long as the pinned versions (`Cargo.toml`, `=`) do not move. If a dependency bump
//! changes the tables, this fails and says so; update the expectations deliberately, re-run every
//! fixture, and ship the app and the Worker together.
use share_core::text::{Class, Max, clean};

#[test]
fn table_versions_are_the_pinned_ones() {
    assert_eq!(unicode_general_category_version(), (16, 0, 0), "general categories");
    assert_eq!(unicode_normalization::UNICODE_VERSION, (17, 0, 0), "normalisation tables");
}

fn unicode_general_category_version() -> (u64, u64, u64) {
    share_core::text::UNICODE_VERSION
}

#[test]
fn recently_assigned_and_unassigned_code_points() {
    let ok = |c: char| clean(&format!("a{c}b"), Class::Block, Max::Bytes(64)).is_ok();
    // Assigned in 15.1 (U+2FFC IDEOGRAPHIC DESCRIPTION CHARACTER) and 16.0 (U+1C89 CYRILLIC
    // CAPITAL LETTER TJE, U+1CC00 UP-POINTING GO-KART): accepted.
    for c in ['\u{2FFC}', '\u{1C89}', '\u{1CC00}'] {
        assert!(ok(c), "U+{:04X} should be assigned", c as u32);
    }
    // Never assigned (U+0378), unassigned as of 16.0 (U+1C8B, U+31350..), noncharacters: refused.
    for c in ['\u{0378}', '\u{1C8B}', '\u{3134B}', '\u{FDEF}', '\u{1FFFE}'] {
        assert!(!ok(c), "U+{:04X} should be unassigned", c as u32);
    }
}

/// The std functions the rules rely on, over a fixed corpus. Their answers come from the
/// compiler's Unicode tables (pinned by `rust-toolchain.toml`); a toolchain bump that changes any
/// of them fails here first.
#[test]
fn std_unicode_functions_on_a_fixed_corpus() {
    let corpus: Vec<char> = [
        0x09, 0x0A, 0x20, 0x41, 0x61, 0xA0, 0xC5, 0xDF, 0x130, 0x1C5, 0x2126, 0x212B, 0x1680, 0x180E, 0x2000, 0x2007, 0x200B, 0x2028, 0x202F,
        0x205F, 0x3000, 0x3164, 0xFEFF, 0x10400, 0x1D465, 0x1E900, 0x1C89, 0x2FFC, 0x3B1, 0x430, 0x5D0, 0x661, 0x30A2, 0x24B6, 0x2160,
    ]
    .iter()
    .map(|u| char::from_u32(*u).unwrap())
    .collect();
    let mut sig = String::new();
    for c in &corpus {
        sig.push_str(&format!(
            "{:X}:{}{}{}{}{}:{};",
            *c as u32,
            c.is_whitespace() as u8,
            c.is_alphabetic() as u8,
            c.is_uppercase() as u8,
            c.is_lowercase() as u8,
            c.is_alphanumeric() as u8,
            c.to_lowercase().map(|l| format!("{:X}", l as u32)).collect::<Vec<_>>().join("+")
        ));
    }
    let digest = share_core::hash::sha256_hex(sig.as_bytes());
    assert_eq!(digest, CANARY, "std Unicode answers changed:\n{sig}");
}

/// Recorded with Rust 1.99.0.
const CANARY: &str = "eb34615e9192fc5a5f038261f042beef3410a52a0a3a0b2af31db3b4210332d9";
