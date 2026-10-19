//! A string that must never be shown: passwords, access/refresh/device tokens, mailed codes.
//!
//! `Debug` prints a fixed placeholder, there is no `Display`, and the bytes are overwritten when
//! the value is dropped. Getting at the text needs an explicit [`Secret::expose`], so every place a
//! secret leaves this type is easy to find by searching for it.

use zeroize::Zeroize;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Secret {
        Secret(s.into())
    }

    /// Takes the text out of a form field and wipes the field, so the only copy is this one.
    pub fn take(field: &mut String) -> Secret {
        let s = Secret(field.clone());
        field.zeroize();
        s
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Length in code points (the server's password policy counts these, after NFKC).
    pub fn chars(&self) -> usize {
        self.0.chars().count()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
