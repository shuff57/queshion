//! House theme: the Test Parrot cream/brown palette, Source fonts, and widget chrome.
//!
//! Usage from any screen module:
//! ```ignore
//! ui.label(theme::serif("Turn anything into an exam.").size(34.0)); // house serif heading
//! ui.label(theme::strong("Bold-ish label"));                        // real bold sans
//! ui.label(theme::muted("secondary text"));                         // muted brown
//! ```
//! Colors are public constants so every worker styles directly; fonts are registered here
//! at startup from `assets/fonts/` (falling back to egui's defaults when missing).

use std::sync::{Arc, OnceLock};

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    RichText, Stroke, TextStyle, ThemePreference,
};

// ---------- palette ----------

/// One palette of the whole app: the V4 "Study Studio in paper tones" tokens
/// (cream/brown neutrals, chalkboard-green accent, highlighter-yellow marker).
/// All surfaces, text roles and accents come from here — no screen keeps its own hexes.
#[derive(Clone, Copy)]
pub struct Palette {
    /// Desk: whole-app background (under the faint graph-paper grid).
    pub cream: Color32,
    /// Left dock.
    pub rail: Color32,
    /// Cards, windows, popovers.
    pub card: Color32,
    /// Inputs and inset areas.
    pub field: Color32,
    /// Hairline borders around cards and windows.
    pub border: Color32,
    /// Stronger divider / grip dots / checkbox outline.
    pub rule: Color32,
    /// Headings and emphasized text.
    pub ink: Color32,
    /// Regular body text (warm brown).
    pub text: Color32,
    /// De-emphasized text (captions, counters).
    pub muted: Color32,
    /// Primary button, links, active controls (chalkboard green).
    pub accent: Color32,
    pub accent_hover: Color32,
    /// Soft accent wash (selected chips, type pills, icon tiles).
    pub accent_faded: Color32,
    /// Inverse text for filled (accent) buttons and chips.
    pub on_accent: Color32,
    /// Marker: active dock item, "just added", "needs AI" tags. Always dark ink text.
    pub highlighter: Color32,
    /// Hover state on cards and widgets.
    pub hover_bg: Color32,
    /// Correct-answer marks (same family as the accent).
    pub green: Color32,
    /// Errors and destructive confirmations.
    pub red: Color32,
    /// Exam desk behind the paper.
    pub table: Color32,
    /// 24 px graph-paper grid line (alpha-blended).
    pub grid_line: Color32,
}

pub const fn light_palette() -> Palette {
    Palette {
        cream: Color32::from_rgb(0xf7, 0xf4, 0xed),
        rail: Color32::from_rgb(0x33, 0x2a, 0x24),
        card: Color32::from_rgb(0xff, 0xfd, 0xf8),
        field: Color32::from_rgb(0xf3, 0xee, 0xe3),
        border: Color32::from_rgb(0xe0, 0xd3, 0xc2),
        rule: Color32::from_rgb(0xbd, 0xa9, 0x95),
        ink: Color32::from_rgb(0x33, 0x2a, 0x24),
        text: Color32::from_rgb(0x5e, 0x4c, 0x40),
        muted: Color32::from_rgb(0x7a, 0x63, 0x50),
        accent: Color32::from_rgb(0x2f, 0x61, 0x44),
        accent_hover: Color32::from_rgb(0x3b, 0x74, 0x53),
        accent_faded: Color32::from_rgb(0xdf, 0xe9, 0xd4),
        on_accent: Color32::from_rgb(0xf7, 0xfb, 0xf3),
        highlighter: Color32::from_rgb(0xf3, 0xdc, 0x6a),
        hover_bg: Color32::from_rgb(0xef, 0xe8, 0xdb),
        green: Color32::from_rgb(0x2f, 0x61, 0x44),
        red: Color32::from_rgb(0xa3, 0x3b, 0x2e),
        table: Color32::from_rgb(0xe8, 0xdf, 0xce),
        grid_line: Color32::from_rgba_premultiplied(3, 2, 2, 14),
    }
}

pub const fn dark_palette() -> Palette {
    Palette {
        cream: Color32::from_rgb(0x21, 0x1c, 0x18),
        rail: Color32::from_rgb(0x14, 0x10, 0x0d),
        card: Color32::from_rgb(0x2b, 0x25, 0x20),
        field: Color32::from_rgb(0x21, 0x1c, 0x18),
        border: Color32::from_rgb(0x4b, 0x3f, 0x35),
        rule: Color32::from_rgb(0x6d, 0x5c, 0x4b),
        ink: Color32::from_rgb(0xf2, 0xe8, 0xdd),
        text: Color32::from_rgb(0xd3, 0xc2, 0xb2),
        muted: Color32::from_rgb(0xa9, 0x93, 0x7e),
        accent: Color32::from_rgb(0x9d, 0xcb, 0xa0),
        accent_hover: Color32::from_rgb(0xb3, 0xdc, 0xb5),
        accent_faded: Color32::from_rgb(0x2d, 0x3b, 0x2c),
        on_accent: Color32::from_rgb(0x0e, 0x20, 0x12),
        highlighter: Color32::from_rgb(0xe9, 0xcf, 0x55),
        hover_bg: Color32::from_rgb(0x38, 0x30, 0x28),
        green: Color32::from_rgb(0x9d, 0xcb, 0xa0),
        red: Color32::from_rgb(0xe0, 0x82, 0x6e),
        table: Color32::from_rgb(0x16, 0x12, 0x10),
        grid_line: Color32::from_rgba_premultiplied(2, 2, 2, 11),
    }
}

thread_local! {
    /// The palette mode for the app being drawn. Per-context, not process-global, so the
    /// embedded officecraft tab cannot change the suite's palette.
    static DARK_MODE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Is the dark palette active?
pub fn is_dark() -> bool {
    DARK_MODE.with(|d| d.get())
}

/// Sets the palette for this app's next frame. Call at the top of every frame; cheap.
pub fn begin_frame(dark: bool) {
    DARK_MODE.with(|d| d.set(dark));
}

/// Activate a palette mode and (if `ctx` given) restyle the running app.
/// The caller persists the choice in settings.
pub fn set_dark(dark: bool, ctx: Option<&Context>) {
    begin_frame(dark);
    if let Some(ctx) = ctx {
        setup_style(ctx);
        ctx.request_repaint();
    }
}

thread_local! {
    /// The rectangle the app may draw in. Standalone this is the viewport; in officecraft it
    /// is the tab's content rect, so panels and overlays stay inside the tab.
    static AREA: std::cell::Cell<egui::Rect> = const { std::cell::Cell::new(egui::Rect::NOTHING) };
}

pub fn set_area(rect: egui::Rect) {
    AREA.with(|a| a.set(rect));
}

/// The host-provided drawing rect, falling back to `default` before the first frame.
pub fn area_or(default: egui::Rect) -> egui::Rect {
    AREA.with(|a| {
        let r = a.get();
        if r.is_finite() && r.width() > 0.0 && r.height() > 0.0 { r } else { default }
    })
}

fn pal() -> Palette {
    match (chem_skin(), is_dark()) {
        (true, false) => chem_light_palette(),
        (true, true) => chem_dark_palette(),
        (false, true) => dark_palette(),
        (false, false) => light_palette(),
    }
}

thread_local! {
    /// The chemistry-figures page skin (cool grey page, blue accent, Noto Sans): on while a
    /// chemistry figure is open, off everywhere else.
    static CHEM_SKIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn chem_skin() -> bool {
    CHEM_SKIN.with(|c| c.get())
}

/// Switches the chemistry skin and restyles `ctx` when it changed.
pub fn set_chem_skin(on: bool, ctx: &Context) {
    if chem_skin() != on {
        CHEM_SKIN.with(|c| c.set(on));
        setup_style(ctx);
        ctx.request_repaint();
    }
}

/// The reference chemistry site's tokens: page #f6f7fb, ink #1f2937, hairline #e3e4ea,
/// blue #2563eb with #1d4ed8 text, #bfdbfe pill borders, #eff6ff washes.
pub const fn chem_light_palette() -> Palette {
    Palette {
        cream: Color32::from_rgb(0xf6, 0xf7, 0xfb),
        rail: Color32::from_rgb(0x1f, 0x29, 0x37),
        card: Color32::from_rgb(0xff, 0xff, 0xff),
        field: Color32::from_rgb(0xff, 0xff, 0xff),
        border: Color32::from_rgb(0xe3, 0xe4, 0xea),
        rule: Color32::from_rgb(0xbf, 0xdb, 0xfe),
        ink: Color32::from_rgb(0x1f, 0x29, 0x37),
        text: Color32::from_rgb(0x37, 0x41, 0x51),
        muted: Color32::from_rgb(0x6b, 0x72, 0x80),
        accent: Color32::from_rgb(0x25, 0x63, 0xeb),
        accent_hover: Color32::from_rgb(0x1d, 0x4e, 0xd8),
        accent_faded: Color32::from_rgb(0xef, 0xf6, 0xff),
        on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
        highlighter: Color32::from_rgb(0xdb, 0xea, 0xfe),
        hover_bg: Color32::from_rgb(0xef, 0xf6, 0xff),
        green: Color32::from_rgb(0x15, 0x80, 0x3d),
        red: Color32::from_rgb(0xb9, 0x1c, 0x1c),
        table: Color32::from_rgb(0xe5, 0xe7, 0xeb),
        grid_line: Color32::from_rgba_premultiplied(0, 0, 3, 10),
    }
}

pub const fn chem_dark_palette() -> Palette {
    Palette {
        cream: Color32::from_rgb(0x11, 0x16, 0x21),
        rail: Color32::from_rgb(0x0b, 0x0f, 0x17),
        card: Color32::from_rgb(0x1a, 0x21, 0x31),
        field: Color32::from_rgb(0x11, 0x16, 0x21),
        border: Color32::from_rgb(0x2e, 0x38, 0x4d),
        rule: Color32::from_rgb(0x3b, 0x4a, 0x68),
        ink: Color32::from_rgb(0xf3, 0xf4, 0xf6),
        text: Color32::from_rgb(0xd1, 0xd5, 0xdb),
        muted: Color32::from_rgb(0x9c, 0xa3, 0xaf),
        accent: Color32::from_rgb(0x60, 0xa5, 0xfa),
        accent_hover: Color32::from_rgb(0x93, 0xc5, 0xfd),
        accent_faded: Color32::from_rgb(0x1e, 0x2a, 0x4a),
        on_accent: Color32::from_rgb(0x0b, 0x12, 0x20),
        highlighter: Color32::from_rgb(0x1e, 0x3a, 0x8a),
        hover_bg: Color32::from_rgb(0x23, 0x2d, 0x44),
        green: Color32::from_rgb(0x86, 0xef, 0xac),
        red: Color32::from_rgb(0xfc, 0xa5, 0xa5),
        table: Color32::from_rgb(0x0b, 0x0f, 0x17),
        grid_line: Color32::from_rgba_premultiplied(2, 2, 3, 11),
    }
}

/// The print-work surface (exports, page preview) stays paper-white in both
/// modes — it previews a white sheet.
pub fn paper() -> Color32 {
    Color32::from_rgb(0xff, 0xfd, 0xf8)
}
/// Border of the paper sheet (slightly stronger than ui borders on cream).
pub fn paper_edge() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x8a, 0x76, 0x63)
    } else {
        Color32::from_rgb(0xc9, 0xb8, 0xa5)
    }
}

/// Paint the faint 24 px graph-paper grid across `rect` (clipped to the painter).
pub fn paint_grid(painter: &egui::Painter, rect: egui::Rect) {
    let step = 24.0;
    let stroke = Stroke::new(1.0, grid_line());
    let mut x = (rect.left() / step).floor() * step;
    while x <= rect.right() {
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], stroke);
        x += step;
    }
    let mut y = (rect.top() / step).floor() * step;
    while y <= rect.bottom() {
        painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)], stroke);
        y += step;
    }
}

/// Aliases other screens reference for the same tokens.
/// The warm accent-brown used on page numbers and grips.
pub fn brown() -> Color32 { pal().accent }

/// Named family: the house serif (headings, hero titles, exam preview).
pub fn serif_family() -> FontFamily {
    FontFamily::Name("vs-serif".into())
}
/// Named family: heavier serif for large headings.
pub fn serif_bold_family() -> FontFamily {
    FontFamily::Name("vs-serif-bold".into())
}
pub fn serif_strong_family() -> FontFamily {
    serif_bold_family()
}
/// The real italic face of `family` (`serif` -> `serif-italic`, `serif-bold` ->
/// `serif-bold-italic`, proportional -> `sans-italic`), or `None` when `ctx` does not have
/// the house fonts (then callers fall back to egui's synthetic slant).
pub fn italic_family(ctx: &Context, family: &FontFamily) -> Option<FontFamily> {
    let name = match family {
        FontFamily::Proportional => "vs-sans-italic",
        FontFamily::Name(n) => match n.as_ref() {
            "vs-serif" | "vs-serif-italic" => "vs-serif-italic",
            "vs-serif-bold" | "vs-serif-bold-italic" => "vs-serif-bold-italic",
            "vs-sans-italic" => "vs-sans-italic",
            _ => return None,
        },
        _ => return None,
    };
    let fam = FontFamily::Name(name.into());
    ctx.fonts(|f| f.definitions().families.contains_key(&fam)).then_some(fam)
}
/// The chemistry skin's semi-bold (breadcrumb, header pills); plain bold elsewhere.
pub fn semibold_family() -> FontFamily {
    if chem_skin() { FontFamily::Name("vs-noto-semibold".into()) } else { bold_family() }
}
/// The chemistry skin's medium (500) and extra-bold (800) faces; the nearest plain faces elsewhere.
pub fn medium_family() -> FontFamily {
    if chem_skin() { FontFamily::Name("vs-noto-medium".into()) } else { bold_family() }
}
pub fn xbold_family() -> FontFamily {
    if chem_skin() { FontFamily::Name("vs-noto-xbold".into()) } else { bold_family() }
}
/// Named family: real bold sans (used by [`strong`]).
pub fn bold_family() -> FontFamily {
    FontFamily::Name(if chem_skin() { "vs-noto-bold" } else { "vs-sans-bold" }.into())
}

// ---------- text helpers ----------

// (Manual impls instead of a blanket over `Into<String>`: rustc's future-compat
// rule flags the overlap with a possible upstream `impl From<RichText> for String`.)
pub trait IntoText {
    fn into_text(self) -> RichText;
}

impl IntoText for &str {
    fn into_text(self) -> RichText {
        RichText::new(self)
    }
}

impl IntoText for &String {
    fn into_text(self) -> RichText {
        RichText::new(self)
    }
}

impl IntoText for String {
    fn into_text(self) -> RichText {
        RichText::new(self)
    }
}

impl IntoText for RichText {
    fn into_text(self) -> RichText {
        self
    }
}

/// House-serif heading text. Sizes are set by the caller (`.size(34.0)`).
pub fn serif(text: impl IntoText) -> RichText {
    text.into_text().family(serif_family()).color(pal().ink)
}

/// Heavier serif (large headings, the logo wordmark).
pub fn serif_strong(text: impl IntoText) -> RichText {
    text.into_text().family(serif_bold_family()).color(pal().ink)
}

/// True bold sans. Use instead of `RichText::strong()`, which in egui
/// only darkens the color rather than changing the weight.
pub fn strong(text: impl IntoText) -> RichText {
    text.into_text().family(bold_family()).color(pal().ink)
}

/// Regular body text.
pub fn text(text: impl IntoText) -> RichText {
    text.into_text().color(pal().text)
}

/// Muted caption text.
pub fn muted(text: impl IntoText) -> RichText {
    text.into_text().color(pal().muted)
}

/// Accent-colored text (selected nav items, links).
pub fn accent(text: impl IntoText) -> RichText {
    text.into_text().color(pal().accent)
}

/// Inverse text for filled (accent) buttons and chips.
pub fn on_accent(text: impl IntoText) -> RichText {
    text.into_text().color(pal().on_accent)
}

// ---------- palette accessors (the old constants, now mode-dependent) ----------
// Names avoid colliding with the RichText helpers above.

pub fn cream() -> Color32 { pal().cream }
pub fn card() -> Color32 { pal().card }
pub fn border() -> Color32 { pal().border }
pub fn rule() -> Color32 { pal().rule }
pub fn ink() -> Color32 { pal().ink }
pub fn text_col() -> Color32 { pal().text }
pub fn muted_col() -> Color32 { pal().muted }
pub fn accent_col() -> Color32 { pal().accent }
pub fn accent_hover() -> Color32 { pal().accent_hover }
pub fn accent_faded() -> Color32 { pal().accent_faded }
pub fn hover_bg() -> Color32 { pal().hover_bg }
pub fn rail() -> Color32 { pal().rail }
pub fn field() -> Color32 { pal().field }
pub fn highlighter() -> Color32 { pal().highlighter }
pub fn table() -> Color32 { pal().table }
pub fn grid_line() -> Color32 { pal().grid_line }
/// Dark ink for text painted on the highlighter (same in both modes).
pub fn on_highlighter() -> Color32 { Color32::from_rgb(0x33, 0x2a, 0x24) }
/// Dock label color (AA on the rail in both modes).
pub fn dock_text() -> Color32 { Color32::from_rgb(0xe9, 0xdf, 0xd2) }
pub fn green() -> Color32 { pal().green }
pub fn red() -> Color32 { pal().red }
pub fn bad() -> Color32 { pal().red }
/// Text painted directly on an accent-filled shape.
pub fn on_accent_col() -> Color32 { pal().on_accent }
/// Soft red wash behind error toasts.
pub fn error_wash() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x50, 0x30, 0x2a)
    } else {
        Color32::from_rgb(0xfa, 0xe6, 0xe1)
    }
}

// ---------- fonts ----------

static FONTS: OnceLock<FontDefinitions> = OnceLock::new();

/// egui font-data key of the symbol fallback face (STIX Two Math).
const FALLBACK_FACE: &str = "STIXTwoMath-Regular";

/// Named egui family holding the Phosphor icon face.
pub const ICON_FAMILY: &str = "vs-phosphor";

/// Font for one Phosphor glyph. The icons live in the Private Use Area, so no text face can
/// shadow them; naming the family keeps them independent of the proportional chain.
pub fn icon_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, FontFamily::Name(ICON_FAMILY.into()))
}

// The Phosphor icons the app draws, cut from the full regular face at compile time (that face
// holds ~1 500 icons and weighs 477 KiB). Code keeps naming icons `egui_phosphor::regular::X`
// (or `ph::X`); an icon missing here draws as an empty box, and the test
// `every_phosphor_icon_used_is_in_the_subset` fails until it is added.
egui_phosphor::subset! {
    pub mod icons {
        use regular::{
            ARROW_CLOCKWISE, ARROW_COUNTER_CLOCKWISE, ARROW_DOWN, ARROW_LEFT, ARROW_RIGHT, ARROW_UP,
            ARROW_UP_RIGHT, ARROW_U_UP_LEFT, ARROW_U_UP_RIGHT, ATOM, BEZIER_CURVE, BOOKMARK_SIMPLE,
            BOOKS, BUG, CAMERA, CARET_DOWN, CARET_RIGHT, CARET_UP, CHART_BAR, CHART_LINE,
            CHART_LINE_UP, CHART_SCATTER, COFFEE, COPY, COPY_SIMPLE, DNA, DROP, ENVELOPE_SIMPLE,
            EXAM, EXPORT, EYE, FILE, FILE_ARROW_DOWN, FLASK, GEAR, GRID_FOUR, GRID_NINE, HOUSE,
            IMAGE, IMAGES, INFO, KEY, LIST_BULLETS, LOCK_SIMPLE, MAGNIFYING_GLASS, MAP_PIN, MOON,
            PALETTE, PAPERCLIP, PENCIL_SIMPLE, PENCIL_SIMPLE_LINE, PLAY_CIRCLE, PLUS, PRINTER,
            PROHIBIT, QUESTION, ROWS, RULER, SELECTION, SHAPES, SLIDERS_HORIZONTAL, SPARKLE, SUN,
            TABLE, TAG, TEXT_AA, TEXT_H, TEXT_T, THERMOMETER, TRASH, TREE_STRUCTURE,
            UPLOAD_SIMPLE, WAVE_SINE, X,
        };
    }
}

/// Builds the house faces from the bundled bytes (src/fonts.rs: compiled in natively; on the web
/// partly fetched before the app starts). No filesystem access, so the same faces are available
/// on the desktop, in the browser and inside officecraft.
fn load_fonts() -> FontDefinitions {
    use crate::fonts::Face;
    let mut defs = FontDefinitions::default();
    // A face the web build could not fetch is registered under its own name with its
    // stand-in's bytes (`Face::bytes`), so every family below keeps its order.
    let put = |defs: &mut FontDefinitions, name: &str, face: Face| {
        defs.font_data
            .insert(name.to_string(), Arc::new(FontData::from_static(face.bytes())));
    };

    put(&mut defs, "SourceSans3-Regular", Face::SansRegular);
    put(&mut defs, "SourceSans3-It", Face::SansItalic);
    put(&mut defs, "SourceSans3-Semibold", Face::SansSemibold);
    put(&mut defs, "SourceSans3-Bold", Face::SansBold);
    put(&mut defs, "NotoSans-Regular", Face::NotoRegular);
    put(&mut defs, "NotoSans-SemiBold", Face::NotoSemibold);
    put(&mut defs, "NotoSans-Bold", Face::NotoBold);
    put(&mut defs, "NotoSans-Medium", Face::NotoMedium);
    put(&mut defs, "NotoSans-ExtraBold", Face::NotoExtraBold);
    // The print serif (src/fonts.rs): the page preview, the card thumbnails and the PDF all use
    // the very same faces.
    put(&mut defs, "SourceSerif4-Regular", Face::SerifRegular);
    put(&mut defs, "SourceSerif4-Bold", Face::SerifBold);
    put(&mut defs, "SourceSerif4-It", Face::SerifItalic);
    put(&mut defs, "SourceSerif4-BoldIt", Face::SerifBoldItalic);
    put(&mut defs, "SourceSerif4-Semibold", Face::SerifSemibold);
    // STIX Two Math (OFL; the bytes equaSHion already compiles in) right behind the house
    // faces, for symbols they lack (⇒ ∈ ∠ ⊥ ...): the same fallback the PDF prints with.
    if let Some(b) = crate::fonts::fallback_bytes() {
        defs.font_data.insert(FALLBACK_FACE.to_string(), Arc::new(FontData::from_static(b)));
    }

    // egui's default fallback chain (emoji + icon font) kept behind our faces.
    let fallback = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let with = |mut ours: Vec<&'static str>, defs: &FontDefinitions| {
        let rest: Vec<String> = fallback
            .iter()
            .filter(|f| defs.font_data.contains_key(f.as_str()))
            .cloned()
            .collect();
        ours.push(FALLBACK_FACE);
        ours.retain(|f| defs.font_data.contains_key(*f));
        ours.into_iter()
            .map(str::to_string)
            .chain(rest)
            .collect::<Vec<String>>()
    };

    defs.families.insert(
        FontFamily::Proportional,
        with(vec!["SourceSans3-Regular", "SourceSans3-It", "SourceSans3-Bold"], &defs),
    );
    // Named families: ordered glyph-coverage fallbacks, egui defaults behind them. The
    // `vs-` prefix keeps them from colliding with the host suite's families in a shared
    // egui context.
    let named = |name: &'static str, list: Vec<&'static str>| {
        (
            FontFamily::Name(Arc::<str>::from(name)),
            list.iter()
                .chain(std::iter::once(&FALLBACK_FACE))
                .filter(|f| defs.font_data.contains_key(**f))
                .map(|f| f.to_string())
                .chain(fallback.iter().cloned())
                .collect::<Vec<String>>(),
        )
    };
    let (fam, fonts) = named("vs-sans-bold", vec!["SourceSans3-Bold", "SourceSans3-Semibold", "SourceSans3-Regular"]);
    defs.families.insert(fam, fonts);
    // the chemistry page skin's sans (Source Sans stays behind it for glyphs it lacks)
    let (fam, fonts) = named("vs-noto", vec!["NotoSans-Regular", "SourceSans3-Regular"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("vs-noto-bold", vec!["NotoSans-Bold", "NotoSans-SemiBold", "SourceSans3-Bold"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("vs-noto-semibold", vec!["NotoSans-SemiBold", "NotoSans-Bold", "SourceSans3-Semibold"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("vs-noto-medium", vec!["NotoSans-Medium", "NotoSans-Regular", "SourceSans3-Regular"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("vs-noto-xbold", vec!["NotoSans-ExtraBold", "NotoSans-Bold", "SourceSans3-Bold"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("vs-serif", vec!["SourceSerif4-Regular", "SourceSerif4-Semibold", "SourceSans3-Regular"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("vs-serif-bold", vec!["SourceSerif4-Bold", "SourceSerif4-Semibold", "SourceSerif4-Regular"]);
    defs.families.insert(fam, fonts);
    // Real italic faces (egui has no style linking: an italic run picks one of these
    // families instead of slanting the upright face, see `italic_family`).
    let (fam, fonts) = named("vs-serif-italic", vec!["SourceSerif4-It", "SourceSerif4-Regular"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("vs-serif-bold-italic", vec!["SourceSerif4-BoldIt", "SourceSerif4-Bold", "SourceSerif4-It"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("vs-sans-italic", vec!["SourceSans3-It", "SourceSans3-Regular"]);
    defs.families.insert(fam, fonts);
    // Phosphor icons: registered as a named family and as a proportional fallback, so icon
    // glyphs can also appear in ordinary labels.
    egui_phosphor::add_font_bytes_to_fonts(&mut defs, ICON_FAMILY, &icons::regular::FONT);
    // Every named family must reach the icon face too, or an icon inside a bold/serif label
    // renders as tofu.
    for fonts in defs.families.values_mut() {
        if !fonts.iter().any(|f| f.as_str() == ICON_FAMILY) {
            fonts.insert(fonts.len().min(1), ICON_FAMILY.to_string());
        }
    }
    defs
}

/// Registered font set, initialized once on first use.
pub fn fonts() -> &'static FontDefinitions {
    FONTS.get_or_init(load_fonts)
}

pub fn setup(ctx: &Context) {
    setup_fonts(ctx);
    setup_style(ctx);
}

fn setup_fonts(ctx: &Context) {
    ctx.set_fonts(fonts().clone());
}

fn setup_style(ctx: &Context) {
    let p = pal();
    ctx.set_theme(if is_dark() { ThemePreference::Dark } else { ThemePreference::Light });
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.dark_mode = is_dark();
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.ink);
        v.panel_fill = p.cream;
        v.window_fill = p.card;
        v.window_stroke = Stroke::new(1.0, p.border);
        v.window_corner_radius = CornerRadius::same(16);
        v.menu_corner_radius = CornerRadius::same(12);
        v.faint_bg_color = p.cream;
        v.extreme_bg_color = p.field; // text edit backgrounds
        v.text_edit_bg_color = Some(p.field);
        v.code_bg_color = p.accent_faded;
        v.hyperlink_color = p.accent;
        v.warn_fg_color = p.muted;
        v.error_fg_color = p.red;
        v.selection.bg_fill = p.accent_faded;
        v.selection.stroke = Stroke::new(0.0, p.ink);

        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.noninteractive.corner_radius = CornerRadius::same(12);
        v.widgets.inactive.corner_radius = CornerRadius::same(12);
        v.widgets.inactive.weak_bg_fill = p.card;
        v.widgets.inactive.bg_fill = p.card;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.hovered.weak_bg_fill = p.hover_bg;
        v.widgets.hovered.bg_fill = p.hover_bg;
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.hovered.corner_radius = CornerRadius::same(12);
        v.widgets.active.weak_bg_fill = p.accent_faded;
        v.widgets.active.bg_fill = p.accent_faded;
        v.widgets.active.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
        v.widgets.active.corner_radius = CornerRadius::same(12);
        v.widgets.open.weak_bg_fill = p.hover_bg;
        v.widgets.open.bg_fill = p.hover_bg;
        v.widgets.open.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.open.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.open.corner_radius = CornerRadius::same(12);

        style.text_styles.clear();
        let sans = if chem_skin() { FontFamily::Name("vs-noto".into()) } else { FontFamily::Proportional };
        style.text_styles.insert(TextStyle::Body, FontId::new(13.0, sans.clone()));
        style.text_styles.insert(TextStyle::Button, FontId::new(13.0, sans.clone()));
        style.text_styles.insert(TextStyle::Small, FontId::new(11.0, sans.clone()));
        style.text_styles.insert(TextStyle::Heading, FontId::new(22.0, sans.clone()));
        style.text_styles.insert(
            TextStyle::Name("Heading2".into()),
            FontId::new(17.0, sans),
        );
        style.text_styles.insert(TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace));

        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        // One button size everywhere: 34 app px tall (TP parity at this font scale).
        style.spacing.button_padding = egui::vec2(12.0, 9.0);
        style.spacing.interact_size.y = 34.0;
        style.visuals.button_frame = true;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compiled-in Source faces are always present. The named serif/bold families and
    /// the Proportional list must all reference real font_data entries.
    #[test]
    fn bundled_fonts_register_named_families() {
        let defs = load_fonts();
        for key in [
            "SourceSans3-Regular",
            "SourceSans3-It",
            "SourceSans3-Semibold",
            "SourceSans3-Bold",
            "SourceSerif4-Regular",
            "SourceSerif4-Semibold",
            "SourceSerif4-Bold",
            "SourceSerif4-It",
            "SourceSerif4-BoldIt",
        ] {
            assert!(defs.font_data.contains_key(key), "missing font {key}");
        }
        for family in ["vs-sans-bold", "vs-serif", "vs-serif-bold", "vs-serif-italic", "vs-serif-bold-italic", "vs-sans-italic"] {
            let list = defs
                .families
                .get(&FontFamily::Name(family.into()))
                .unwrap_or_else(|| panic!("missing family {family}"));
            assert!(!list.is_empty(), "empty family {family}");
            for f in list {
                assert!(
                    defs.font_data.contains_key(f),
                    "family {family} references unknown font {f}"
                );
            }
        }
        let prop = &defs.families[&FontFamily::Proportional];
        assert_eq!(prop.first().map(String::as_str), Some("SourceSans3-Regular"));
    }

    /// Every Phosphor icon the code names (`egui_phosphor::regular::X`, or `ph::X` where a file
    /// aliases `egui_phosphor::regular as ph`) must be in the compile-time subset (`icons`), or it
    /// would draw as an empty box.
    #[test]
    fn every_phosphor_icon_used_is_in_the_subset() {
        fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    rust_files(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let names_after = |src: &str, prefix: &str| -> Vec<String> {
            src.match_indices(prefix)
                .filter(|(i, _)| *i == 0 || !src[..*i].ends_with(|c: char| c.is_alphanumeric() || c == '_'))
                .map(|(i, _)| src[i + prefix.len()..].chars().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_').collect::<String>())
                .filter(|n| n.starts_with(|c: char| c.is_ascii_uppercase()))
                .collect()
        };
        let this = include_str!("theme.rs");
        let list_at = this.find("use regular::{").unwrap();
        let list = &this[list_at + "use regular::{".len()..];
        let subset: Vec<&str> = list[..list.find('}').unwrap()].split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        assert!(subset.len() > 50, "{subset:?}");
        let mut files = Vec::new();
        rust_files(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut used = std::collections::BTreeSet::new();
        for f in &files {
            let src = std::fs::read_to_string(f).unwrap();
            used.extend(names_after(&src, "egui_phosphor::regular::"));
            if src.contains("egui_phosphor::regular as ph") {
                used.extend(names_after(&src, "ph::"));
            }
        }
        assert!(used.contains("HOUSE") && used.contains("TRASH"), "{used:?}");
        let missing: Vec<&String> = used.iter().filter(|n| !subset.contains(&n.as_str())).collect();
        assert!(missing.is_empty(), "add these to theme::icons: {missing:?}");
        // The subset really holds a glyph for each icon.
        let face = ttf_parser::Face::parse(&icons::regular::FONT, 0).unwrap();
        for icon in [icons::regular::HOUSE, icons::regular::X, icons::regular::WAVE_SINE] {
            let c = icon.chars().next().unwrap();
            assert!(face.glyph_index(c).is_some_and(|g| face.glyph_bounding_box(g).is_some()), "{c:?}");
        }
    }
}