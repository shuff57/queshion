# parity-leftovers progress

Baseline on `parity-loop` (from `web-port` b89fea2): 528 unit + 6 integration tests, 3 ignored;
`cargo check --workspace` reported 9 dead-code warnings.

- Step 1: zero compiler warnings. Removed dead wrappers/fields in `bankexport.rs`, `figuresui.rs`,
  `bankui.rs`, `examui.rs`, `figures/biology/{pedigree,plot,punnett}.rs` and
  `figuresui/composite_canvas.rs`; test-only helpers (`popover_copy_text`, `SnapDoc::from_parts`,
  `Canvas::label_centres`) are now `#[cfg(test)]`. `cargo check --workspace`: 0 warnings.
  `cargo test --workspace`: 528 passed, 6 integration passed, 0 failed, 3 ignored.
