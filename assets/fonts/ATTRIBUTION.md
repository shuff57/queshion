All fonts in assets/fonts are under the SIL Open Font License 1.1: the Adobe Source fonts and
Liberation Mono.

- Source Sans 3 (Regular, It, Semibold, Bold) and Source Serif 4 (Regular, Semibold, Bold):
  copied from the wordcraft assets (storytold/wordcraft). Licence: OFL-SourceSans3.txt,
  OFL-SourceSerif4.txt.
- Source Serif 4 It and BoldIt (added for the print serif's real italics): copied from the
  sibling equaSHion repo (../equaSHion/assets/fonts, same Adobe release 4.005; Regular and
  Bold there are byte-identical to the ones here). Licence: OFL-SourceSerif4.txt.

- STIX Two Math Regular (the PDF fallback face for symbols Source Serif 4 lacks) is not a file
  here: queShion uses the copy the equashion-font crate already compiles in
  (../equaSHion/assets/fonts/STIXTwoMath-Regular.otf). Its licence is copied here as
  OFL-STIXTwoMath.txt (SIL OFL 1.1, Copyright The STIX Fonts Project Authors).

- Liberation Mono Regular 2.1.5 (the PDF face of `code` spans): copied unmodified from Fedora's
  liberation-mono-fonts 2.1.5 package (/usr/share/fonts/liberation-mono-fonts/
  LiberationMono-Regular.ttf, sha256 f2b83c76...d922c1). Licence: OFL-LiberationMono.txt (SIL
  OFL 1.1, Copyright 2010 Google Corporation with Reserved Font Arimo, Tinos and Cousine;
  Copyright 2012 Red Hat, Inc. with Reserved Font Name Liberation). The name is unchanged
  because the font is unmodified; the PDF embeds a subset, which the OFL allows when the
  document is not a font product.

See docs/fonts.md for which face is used where.
