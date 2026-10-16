# queShion

Turn anything into an exam — a pure-Rust desktop clone of [testparrot.com](https://www.testparrot.com/):
question banks, printable exams, and one-shot AI conversion, all local-first.

- **egui + wgpu** (eframe 0.36) single binary; house theme (cream/terracotta, Source Serif/Sans).
- **Question banks**: six question types (multiple choice, true/false, matching, fill-in-the-blank,
  short answer, multipart), topic tags, difficulty, points, images.
- **Exam editor**: page preview with header line, numbered questions and an answer section; open a
  bank in the side panel and click or drag questions onto the exam; undo/redo; heading/text size,
  margins and ruled blanks per exam.
- **Export**: PDF (hand-rolled, base-14 fonts, embedded images) and DOCX, student test + answer key,
  shuffle question/answer order into named Versions; every export is snapshotted and re-exportable
  from Export History.
- **Imports**: Blackboard test-generator text (`MC/MA/TF/ESS/NUM/MAT/FIB/ORD`, `*` = correct), Aiken,
  CSV, and real `.parrot.json` Test Parrot packages; paste or file.
- **AI conversion**: a photo/scan/screenshot is transcribed to import text by
  **Ollama Cloud** (`gemma4:31b` — default), local **Ollama**, **OpenRouter**, or any custom
  OpenAI-compatible endpoint. Keys are read from the environment or the ollama-cloud credentials
  file at request time and never stored.

Data lives in `${XDG_DATA_HOME:-~/.local/share}/queshion/`. Headless helpers:
`QUESHION_SELFTEST=1`, `QUESHION_SEED=1`, `QUESHION_CONVERT=<file>` (prints import text).

Clone provenance: the original app's AI instructions document is archived at
`assets/reference-test-parrot-ai-instructions.md` as the source-of-truth recon artifact.