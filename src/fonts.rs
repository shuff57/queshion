//! The print serif: Source Serif 4 (Adobe, SIL Open Font License 1.1, see
//! `assets/fonts/OFL-SourceSerif4.txt` and `docs/fonts.md`) in its four real faces.
//!
//! The faces are compiled into the binary so the PDF export (which must not touch the file
//! system), the page preview and the card thumbnails always use the same outlines and the
//! same advance widths. The PDF writer measures text with [`FaceMetrics::widths`] (WinAnsi
//! byte -> advance in 1/1000 em, read from the font's own `hmtx` through its `cmap`) and
//! writes the very same numbers into the font's `/Widths` array, so line breaking and
//! drawing can never disagree. [`subset`] keeps only the glyphs a document uses.

use std::sync::OnceLock;

pub static SERIF_REGULAR: &[u8] = include_bytes!("../assets/fonts/SourceSerif4-Regular.ttf");
pub static SERIF_BOLD: &[u8] = include_bytes!("../assets/fonts/SourceSerif4-Bold.ttf");
pub static SERIF_ITALIC: &[u8] = include_bytes!("../assets/fonts/SourceSerif4-It.ttf");
pub static SERIF_BOLD_ITALIC: &[u8] = include_bytes!("../assets/fonts/SourceSerif4-BoldIt.ttf");
/// The print mono for `` `code` `` spans: Liberation Mono Regular 2.1.5 (SIL OFL 1.1, see
/// `assets/fonts/OFL-LiberationMono.txt`), metric-compatible with Courier New, the face the
/// DOCX names, so PDF and Word lines measure alike.
/// Liberation Sans Regular and Bold (SIL OFL 1.1, `assets/fonts/OFL-LiberationSans.txt`),
/// metric-compatible with Arial: the chemistry figure SVGs ask for Arial, and the browser has
/// no system fonts, so text measures like the reference's.
pub static LIBERATION_SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");
pub static LIBERATION_SANS_BOLD: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Bold.ttf");
/// Liberation Serif Regular and Italic, subset to Latin (SIL OFL 1.1, `assets/fonts/OFL-LiberationSerif.txt`),
/// metric-compatible with Times New Roman: the biology figure SVGs set genotypes in
/// "Georgia, 'Times New Roman', Times, serif", which the reference's browser draws in this face.
pub static LIBERATION_SERIF_REGULAR: &[u8] = include_bytes!("../assets/fonts/LiberationSerif-Regular.ttf");
pub static LIBERATION_SERIF_ITALIC: &[u8] = include_bytes!("../assets/fonts/LiberationSerif-Italic.ttf");
pub static LIBERATION_SERIF_BOLD: &[u8] = include_bytes!("../assets/fonts/LiberationSerif-Bold.ttf");
pub static NOTO_REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
pub static NOTO_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/NotoSans-SemiBold.ttf");
pub static NOTO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/NotoSans-Medium.ttf");
pub static NOTO_EXTRABOLD: &[u8] = include_bytes!("../assets/fonts/NotoSans-ExtraBold.ttf");
pub static NOTO_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSans-Bold.ttf");
pub static MONO_REGULAR: &[u8] = include_bytes!("../assets/fonts/LiberationMono-Regular.ttf");

/// UI faces (Source Sans 3, SIL OFL 1.1). Compiled in so the app, the web build and the
/// embedded officecraft tab all get identical typography with no filesystem access.
pub static SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/SourceSans3-Regular.ttf");
pub static SANS_ITALIC: &[u8] = include_bytes!("../assets/fonts/SourceSans3-It.ttf");
pub static SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/SourceSans3-Semibold.ttf");
pub static SANS_BOLD: &[u8] = include_bytes!("../assets/fonts/SourceSans3-Bold.ttf");
/// The semibold print face used by headings; the other four serif faces are above.
pub static SERIF_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/SourceSerif4-Semibold.ttf");

/// The family name the faces carry in their `name` table (used by SVG `font-family`).
pub const SERIF_FAMILY: &str = "Source Serif 4";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Style {
    Regular,
    Bold,
    Italic,
    BoldItalic,
    /// The print mono (Liberation Mono Regular), for code spans. Not in [`Style::ALL`],
    /// which lists the serif faces.
    Mono,
}

impl Style {
    pub const ALL: [Style; 4] = [Style::Regular, Style::Bold, Style::Italic, Style::BoldItalic];
    pub fn of(bold: bool, italic: bool) -> Style {
        match (bold, italic) {
            (false, false) => Style::Regular,
            (true, false) => Style::Bold,
            (false, true) => Style::Italic,
            (true, true) => Style::BoldItalic,
        }
    }
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Style::Regular => SERIF_REGULAR,
            Style::Bold => SERIF_BOLD,
            Style::Italic => SERIF_ITALIC,
            Style::BoldItalic => SERIF_BOLD_ITALIC,
            Style::Mono => MONO_REGULAR,
        }
    }
    pub fn bold(self) -> bool {
        matches!(self, Style::Bold | Style::BoldItalic)
    }
    pub fn italic(self) -> bool {
        matches!(self, Style::Italic | Style::BoldItalic)
    }
    fn idx(self) -> usize {
        self as usize
    }
}

/// WinAnsi (cp1252) byte -> Unicode. `None` for the five undefined bytes and controls.
pub fn winansi_char(b: u8) -> Option<char> {
    const HI: [u16; 32] = [
        0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017D, 0,
        0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
    ];
    match b {
        0x20..=0x7e | 0xa0..=0xff => Some(b as char),
        0x80..=0x9f => match HI[(b - 0x80) as usize] {
            0 => None,
            u => char::from_u32(u as u32),
        },
        _ => None,
    }
}

/// Unicode -> WinAnsi byte for the cp1252 punctuation zone (0x80..0x9F).
pub fn winansi_byte_hi(c: char) -> Option<u8> {
    (0x80u8..=0x9f).find(|&b| winansi_char(b) == Some(c))
}

/// One face's metrics, all in 1/1000 em (PDF glyph space).
#[derive(Debug)]
pub struct FaceMetrics {
    /// PostScript name (`SourceSerif4-Regular`).
    pub ps_name: String,
    /// Advance width of every WinAnsi byte (0 for bytes with no character).
    pub widths: [u16; 256],
    /// Glyph id of every WinAnsi byte (0 = .notdef).
    pub gids: [u16; 256],
    pub ascent: i32,
    pub descent: i32,
    pub cap_height: i32,
    pub bbox: [i32; 4],
    pub italic_angle: f32,
}

fn scale(v: f64, upem: f64) -> i32 {
    (v * 1000.0 / upem).round() as i32
}

fn load(s: Style) -> FaceMetrics {
    let f = ttf_parser::Face::parse(s.bytes(), 0).expect("bundled face parses");
    let upem = f.units_per_em() as f64;
    let mut widths = [0u16; 256];
    let mut gids = [0u16; 256];
    for b in 0..=255u8 {
        if let Some(c) = winansi_char(b)
            && let Some(g) = f.glyph_index(c)
        {
            gids[b as usize] = g.0;
            widths[b as usize] = scale(f.glyph_hor_advance(g).unwrap_or(0) as f64, upem).max(0) as u16;
        }
    }
    let ps_name = f
        .names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME && n.is_unicode())
        .and_then(|n| n.to_string())
        .unwrap_or_else(|| format!("SourceSerif4-{s:?}"));
    let bb = f.global_bounding_box();
    FaceMetrics {
        ps_name,
        widths,
        gids,
        ascent: scale(f.ascender() as f64, upem),
        descent: scale(f.descender() as f64, upem),
        cap_height: scale(f.capital_height().unwrap_or(660) as f64, upem),
        bbox: [scale(bb.x_min as f64, upem), scale(bb.y_min as f64, upem), scale(bb.x_max as f64, upem), scale(bb.y_max as f64, upem)],
        italic_angle: f.italic_angle(),
    }
}

pub fn metrics(s: Style) -> &'static FaceMetrics {
    static M: [OnceLock<FaceMetrics>; 5] = [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
    M[s.idx()].get_or_init(|| load(s))
}

// ------------------------------------------------------------------ subsetting

fn be16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
}
fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*d.get(at)?, *d.get(at + 1)?, *d.get(at + 2)?, *d.get(at + 3)?]))
}

fn checksum(d: &[u8]) -> u32 {
    let mut sum = 0u32;
    for ch in d.chunks(4) {
        let mut w = [0u8; 4];
        w[..ch.len()].copy_from_slice(ch);
        sum = sum.wrapping_add(u32::from_be_bytes(w));
    }
    sum
}

/// Tables a PDF TrueType font program needs (the rest -- layout, colour, variations --
/// is never read by a PDF viewer).
const KEEP: [&[u8; 4]; 13] = [b"OS/2", b"cmap", b"cvt ", b"fpgm", b"gasp", b"glyf", b"head", b"hhea", b"hmtx", b"loca", b"maxp", b"name", b"prep"];

/// A TrueType font program holding only the glyphs of the WinAnsi bytes marked in `used`
/// (plus `.notdef` and the components of composite glyphs). Glyph ids are kept (unused
/// glyphs become empty), so `cmap`, `hmtx` and the PDF `/Widths` stay valid as they are.
/// Falls back to the whole font if the font cannot be parsed this way.
pub fn subset(s: Style, used: &[bool; 256]) -> Vec<u8> {
    subset_with(s, used, &[])
}

/// [`subset`] that also keeps the glyphs `extra` (glyph ids drawn through the face's
/// Identity-H CID font, for characters outside WinAnsi).
pub fn subset_with(s: Style, used: &[bool; 256], extra: &[u16]) -> Vec<u8> {
    subset_bytes(s.bytes(), &metrics(s).gids, used, extra).unwrap_or_else(|| s.bytes().to_vec())
}

fn subset_bytes(font: &[u8], gids: &[u16; 256], used: &[bool; 256], extra: &[u16]) -> Option<Vec<u8>> {
    let n_tables = be16(font, 4)? as usize;
    let mut tables: Vec<([u8; 4], &[u8])> = Vec::new();
    for i in 0..n_tables {
        let rec = 12 + 16 * i;
        let tag: [u8; 4] = font.get(rec..rec + 4)?.try_into().ok()?;
        let off = be32(font, rec + 8)? as usize;
        let len = be32(font, rec + 12)? as usize;
        tables.push((tag, font.get(off..off + len)?));
    }
    let get = |t: &[u8; 4]| tables.iter().find(|(tag, _)| tag == t).map(|(_, d)| *d);
    let head = get(b"head")?;
    let maxp = get(b"maxp")?;
    let loca = get(b"loca")?;
    let glyf = get(b"glyf")?;
    let long = be16(head, 50)? == 1;
    let n_glyphs = be16(maxp, 4)? as usize;
    let off = |g: usize| -> Option<usize> {
        if long { be32(loca, g * 4).map(|v| v as usize) } else { be16(loca, g * 2).map(|v| v as usize * 2) }
    };
    // Glyphs to keep: .notdef, every used byte's glyph, and composite components.
    let mut keep = vec![false; n_glyphs];
    let mut stack: Vec<usize> = vec![0];
    for b in 0..256 {
        if used[b] && gids[b] != 0 {
            stack.push(gids[b] as usize);
        }
    }
    stack.extend(extra.iter().map(|&g| g as usize));
    while let Some(g) = stack.pop() {
        if g >= n_glyphs || keep[g] {
            continue;
        }
        keep[g] = true;
        let (a, z) = (off(g)?, off(g + 1)?);
        if z <= a {
            continue;
        }
        let gd = glyf.get(a..z)?;
        if (be16(gd, 0)? as i16) < 0 {
            // Composite: walk the component records.
            let mut p = 10;
            loop {
                let flags = be16(gd, p)?;
                stack.push(be16(gd, p + 2)? as usize);
                p += 4 + if flags & 0x0001 != 0 { 4 } else { 2 };
                p += if flags & 0x0008 != 0 { 2 } else if flags & 0x0040 != 0 { 4 } else if flags & 0x0080 != 0 { 8 } else { 0 };
                if flags & 0x0020 == 0 {
                    break;
                }
            }
        }
    }
    let mut new_glyf: Vec<u8> = Vec::new();
    let mut new_loca: Vec<u8> = Vec::with_capacity((n_glyphs + 1) * 4);
    for (g, k) in keep.iter().enumerate() {
        new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
        if *k {
            let (a, z) = (off(g)?, off(g + 1)?);
            if z > a {
                new_glyf.extend_from_slice(glyf.get(a..z)?);
                while new_glyf.len() % 4 != 0 {
                    new_glyf.push(0);
                }
            }
        }
    }
    new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
    let mut new_head = head.to_vec();
    new_head[8..12].copy_from_slice(&[0; 4]); // checkSumAdjustment, fixed below
    new_head[50..52].copy_from_slice(&1u16.to_be_bytes()); // long loca offsets

    let mut out_tables: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    for t in KEEP {
        let data = match t {
            b"glyf" => new_glyf.clone(),
            b"loca" => new_loca.clone(),
            b"head" => new_head.clone(),
            _ => match get(t) {
                Some(d) => d.to_vec(),
                None => continue,
            },
        };
        out_tables.push((*t, data));
    }
    let n = out_tables.len() as u16;
    let mut es = 0u16;
    while (1u16 << (es + 1)) <= n {
        es += 1;
    }
    let sr = (1u16 << es) * 16;
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&sr.to_be_bytes());
    out.extend_from_slice(&es.to_be_bytes());
    out.extend_from_slice(&(n * 16 - sr).to_be_bytes());
    let mut at = 12 + 16 * out_tables.len();
    let mut dir = Vec::new();
    let mut body = Vec::new();
    let mut head_at = 0;
    for (tag, data) in &out_tables {
        dir.extend_from_slice(tag);
        dir.extend_from_slice(&checksum(data).to_be_bytes());
        dir.extend_from_slice(&(at as u32).to_be_bytes());
        dir.extend_from_slice(&(data.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_at = at;
        }
        body.extend_from_slice(data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
        at = 12 + 16 * out_tables.len() + body.len();
    }
    out.extend_from_slice(&dir);
    out.extend_from_slice(&body);
    let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
    out[head_at + 8..head_at + 12].copy_from_slice(&adj.to_be_bytes());
    Some(out)
}

// ------------------------------------------------------------------ beyond WinAnsi

/// Every character a font maps, with its glyph id and advance (1/1000 em), and the way back
/// (glyph id -> the lowest character mapped to it).
#[derive(Debug, Default)]
pub struct CharMap {
    map: std::collections::HashMap<char, (u16, u16)>,
    rev: std::collections::HashMap<u16, char>,
}

impl CharMap {
    /// Glyph id and advance (1/1000 em) of `c`; None when the font has no glyph for it.
    pub fn get(&self, c: char) -> Option<(u16, u16)> {
        self.map.get(&c).copied()
    }
    /// The character a glyph id was mapped from (for reading the CID text back).
    pub fn char_of(&self, gid: u16) -> Option<char> {
        self.rev.get(&gid).copied()
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

fn build_charmap(bytes: &[u8]) -> CharMap {
    let mut cm = CharMap::default();
    let Ok(f) = ttf_parser::Face::parse(bytes, 0) else { return cm };
    let upem = f.units_per_em() as f64;
    let Some(cmap) = f.tables().cmap else { return cm };
    for st in cmap.subtables {
        if !st.is_unicode() {
            continue;
        }
        let mut cps = Vec::new();
        st.codepoints(|cp| cps.push(cp));
        for cp in cps {
            let (Some(c), Some(g)) = (char::from_u32(cp), st.glyph_index(cp)) else { continue };
            if g.0 == 0 || cm.map.contains_key(&c) {
                continue;
            }
            let adv = scale(f.glyph_hor_advance(g).unwrap_or(0) as f64, upem).max(0) as u16;
            cm.map.insert(c, (g.0, adv));
            let e = cm.rev.entry(g.0).or_insert(c);
            if c < *e {
                *e = c;
            }
        }
    }
    cm
}

/// The character map of one face (what its Identity-H CID font can print).
pub fn charmap(s: Style) -> &'static CharMap {
    static M: [OnceLock<CharMap>; 5] = [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
    M[s.idx()].get_or_init(|| build_charmap(s.bytes()))
}

/// The fallback face for characters Source Serif 4 lacks (`⇒`, `∈`, `∠`, `⊥`, ...): STIX Two
/// Math Regular (SIL OFL 1.1), the very bytes equaSHion compiles in for its equations (so it
/// adds nothing to the binary). Its glyphs print as a Type 3 font built from the outlines.
pub fn fallback_bytes() -> Option<&'static [u8]> {
    static B: OnceLock<Option<&'static [u8]>> = OnceLock::new();
    *B.get_or_init(|| {
        equashion_font::bundled().iter().copied().find(|b| {
            ttf_parser::Face::parse(b, 0).is_ok_and(|f| {
                f.names().into_iter().any(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME && n.to_string().is_some_and(|s| s.starts_with("STIXTwoMath")))
            })
        })
    })
}

/// PostScript name of the fallback face.
pub const FALLBACK_PS_NAME: &str = "STIXTwoMath-Regular";

pub fn fallback_charmap() -> &'static CharMap {
    static M: OnceLock<CharMap> = OnceLock::new();
    M.get_or_init(|| fallback_bytes().map(build_charmap).unwrap_or_default())
}

/// PDF path operators drawing glyph `gid` of the fallback face in 1/1000 em, and its bounding box.
struct PathOps {
    out: String,
    k: f32,
    last: (f32, f32),
}

impl PathOps {
    fn p(&mut self, x: f32, y: f32) {
        let f = |v: f32| {
            let s = format!("{:.1}", v);
            s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
        };
        self.out.push_str(&f(x * self.k));
        self.out.push(' ');
        self.out.push_str(&f(y * self.k));
        self.out.push(' ');
    }
}

impl ttf_parser::OutlineBuilder for PathOps {
    fn move_to(&mut self, x: f32, y: f32) {
        self.p(x, y);
        self.out.push_str("m ");
        self.last = (x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.p(x, y);
        self.out.push_str("l ");
        self.last = (x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x0, y0) = self.last;
        self.p(x0 + 2.0 / 3.0 * (x1 - x0), y0 + 2.0 / 3.0 * (y1 - y0));
        self.p(x + 2.0 / 3.0 * (x1 - x), y + 2.0 / 3.0 * (y1 - y));
        self.p(x, y);
        self.out.push_str("c ");
        self.last = (x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.p(x1, y1);
        self.p(x2, y2);
        self.p(x, y);
        self.out.push_str("c ");
        self.last = (x, y);
    }
    fn close(&mut self) {
        self.out.push_str("h ");
    }
}

/// A Type 3 glyph procedure for fallback glyph `gid`: `d1` metrics then its outline, filled
/// (1/1000 em glyph space), and the glyph's bounding box. An empty glyph (a space) draws nothing.
pub fn fallback_glyph_proc(gid: u16, advance: u16) -> (String, [i32; 4]) {
    let empty = (format!("{advance} 0 0 0 0 0 d1\n"), [0; 4]);
    let Some(f) = fallback_bytes().and_then(|b| ttf_parser::Face::parse(b, 0).ok()) else { return empty };
    let k = 1000.0 / f.units_per_em() as f32;
    let mut b = PathOps { out: String::new(), k, last: (0.0, 0.0) };
    match f.outline_glyph(ttf_parser::GlyphId(gid), &mut b) {
        Some(r) => {
            let bb = [
                (r.x_min as f32 * k).floor() as i32,
                (r.y_min as f32 * k).floor() as i32,
                (r.x_max as f32 * k).ceil() as i32,
                (r.y_max as f32 * k).ceil() as i32,
            ];
            (format!("{advance} 0 {} {} {} {} d1\n{}f\n", bb[0], bb[1], bb[2], bb[3], b.out), bb)
        }
        None => empty,
    }
}

/// A ToUnicode CMap mapping each code (`bytes` wide: 1 for a Type 3 font, 2 for Identity-H)
/// to its character, so viewers can copy and search the text.
pub fn to_unicode_cmap(entries: &[(u16, char)], bytes: usize) -> String {
    let hex = |v: u16| if bytes == 1 { format!("{:02X}", v) } else { format!("{:04X}", v) };
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n",
    );
    s.push_str(&format!("<{}> <{}>\nendcodespacerange\n", hex(0), if bytes == 1 { "FF".to_string() } else { "FFFF".to_string() }));
    for chunk in entries.chunks(100) {
        s.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (code, c) in chunk {
            let mut u = [0u16; 2];
            let utf16: String = c.encode_utf16(&mut u).iter().map(|w| format!("{w:04X}")).collect();
            s.push_str(&format!("<{}> <{utf16}>\n", hex(*code)));
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMapResource defineresource pop\nend\nend\n");
    s
}

/// zlib (FlateDecode) compression of a PDF stream.
pub fn deflate(data: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = e.write_all(data);
    e.finish().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_real_faces_with_full_winansi_coverage() {
        let mut names = std::collections::HashSet::new();
        for s in Style::ALL {
            let m = metrics(s);
            assert!(names.insert(m.ps_name.clone()), "distinct faces");
            assert!(m.ps_name.starts_with("SourceSerif4"), "{}", m.ps_name);
            for b in 0..=255u8 {
                if winansi_char(b).is_some() {
                    assert!(m.gids[b as usize] != 0, "{s:?} lacks a glyph for byte {b:#x}");
                    assert!(m.widths[b as usize] > 0, "{s:?} byte {b:#x} has no width");
                }
            }
            assert_eq!(m.italic_angle != 0.0, s.italic(), "{s:?} italic angle {}", m.italic_angle);
            assert!(m.ascent > 600 && m.descent < 0 && m.cap_height > 500);
        }
        // Real italics have their own (narrower) advances, not the regular's.
        assert_ne!(metrics(Style::Italic).widths[b'a' as usize], metrics(Style::Regular).widths[b'a' as usize]);
        assert_ne!(metrics(Style::Bold).widths[b'a' as usize], metrics(Style::Regular).widths[b'a' as usize]);
    }

    #[test]
    fn mono_face_covers_winansi_with_one_advance() {
        let m = metrics(Style::Mono);
        assert!(m.ps_name.starts_with("LiberationMono"), "{}", m.ps_name);
        let adv = m.widths[b'i' as usize];
        assert_eq!(adv, 600, "Courier New metrics");
        for b in 0x20..=0x7eu8 {
            assert!(m.gids[b as usize] != 0, "mono lacks byte {b:#x}");
            assert_eq!(m.widths[b as usize], adv, "mono byte {b:#x} not fixed pitch");
        }
        assert!(!Style::Mono.bold() && !Style::Mono.italic() && !Style::ALL.contains(&Style::Mono));
        let mut used = [false; 256];
        b"let x = 1;".iter().for_each(|&b| used[b as usize] = true);
        let sub = subset(Style::Mono, &used);
        assert!(sub.len() < MONO_REGULAR.len() / 3);
        let f = ttf_parser::Face::parse(&sub, 0).expect("mono subset parses");
        assert!(f.glyph_bounding_box(f.glyph_index('x').unwrap()).is_some());
        assert_eq!(checksum(&sub), 0xB1B0_AFBA);
    }

    #[test]
    fn subset_keeps_used_glyphs_and_parses() {
        let mut used = [false; 256];
        for b in b"Hello, World 123 ()" {
            used[*b as usize] = true;
        }
        for s in Style::ALL {
            let sub = subset(s, &used);
            assert!(sub.len() < s.bytes().len() / 3, "{s:?}: {} vs {}", sub.len(), s.bytes().len());
            let f = ttf_parser::Face::parse(&sub, 0).expect("subset parses");
            let full = ttf_parser::Face::parse(s.bytes(), 0).unwrap();
            for c in "HeloWrd1".chars() {
                let g = f.glyph_index(c).unwrap();
                assert_eq!(f.glyph_hor_advance(g), full.glyph_hor_advance(g));
                assert!(f.glyph_bounding_box(g).is_some(), "{s:?} '{c}' outline kept");
            }
            let z = f.glyph_index('Z').unwrap();
            assert!(f.glyph_bounding_box(z).is_none(), "{s:?} unused glyph emptied");
            assert_eq!(checksum(&sub), 0xB1B0_AFBA, "whole-font checksum");
        }
    }

    #[test]
    fn winansi_table_round_trips() {
        for b in 0..=255u8 {
            if let Some(c) = winansi_char(b)
                && (0x80..=0x9f).contains(&b)
            {
                assert_eq!(winansi_byte_hi(c), Some(b));
            }
        }
        assert_eq!(winansi_char(0x80), Some('€'));
        assert_eq!(winansi_char(0x97), Some('—'));
        assert_eq!(winansi_char(0x81), None);
    }
}
