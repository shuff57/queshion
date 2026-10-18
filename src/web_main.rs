//! Browser entry point for the standalone web build (`trunk build --release`).

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("vershion-web is a wasm binary; build it with `trunk build --release`.");
}
