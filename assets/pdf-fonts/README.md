The 14 standard PDF fonts (bare CFF programs, named `.pfb`) that hayro draws a PDF's non-embedded text with:
Foxit's fonts as shipped in PDFium, copied unmodified from the `hayro-interpret` 0.8.0 crate's
`assets/` folder. Licence: LICENSE_FOXIT (BSD-style, Copyright 2014 PDFium Authors; original
code copyright 2014 Foxit Software Inc.).

The desktop app gets the same bytes from hayro's `embed-fonts` feature. The web build fetches
these files from `pdf-fonts/` at startup instead of compiling them in (src/fonts.rs,
`PDF_STANDARD_FONTS`; src/platform.rs, `render_with_hayro`).
