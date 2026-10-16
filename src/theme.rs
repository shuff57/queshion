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

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin,
    RichText, Stroke, TextStyle, ThemePreference,
};

// ---------- palette ----------

/// One palette of the whole app. `light` is the Test Parrot cream/brown house
/// palette; `dark` is the same warm hue family for dark rooms. All surfaces,
/// text roles and accents come from here — no screen keeps its own hexes.
#[derive(Clone, Copy)]
pub struct Palette {
    /// Whole-app background (warm cream / warm near-black).
    pub cream: Color32,
    /// Cards, windows, widgets (slightly lighter than the page).
    pub card: Color32,
    /// Hairline borders around cards and windows.
    pub border: Color32,
    /// Ruled answer lines on printed forms and dialogs.
    pub rule: Color32,
    /// Headings and emphasized text.
    pub ink: Color32,
    /// Regular body text (warm brown).
    pub text: Color32,
    /// De-emphasized text (captions, counters).
    pub muted: Color32,
    /// Buttons / filled actions.
    pub accent: Color32,
    pub accent_hover: Color32,
    /// Soft accent wash (chips, selection, active widgets).
    pub accent_faded: Color32,
    /// Hover state on cards and widgets.
    pub hover_bg: Color32,
    /// Correct-answer marks.
    pub green: Color32,
    /// Errors and destructive confirmations.
    pub red: Color32,
    /// Inverse text for filled (accent) buttons and chips.
    pub on_accent: Color32,
}

pub const fn light_palette() -> Palette {
    Palette {
        cream: Color32::from_rgb(0xf7, 0xf4, 0xed),
        card: Color32::from_rgb(0xff, 0xfd, 0xf8),
        border: Color32::from_rgb(0xe0, 0xd3, 0xc2),
        rule: Color32::from_rgb(0xbd, 0xa9, 0x95),
        ink: Color32::from_rgb(0x33, 0x2a, 0x24),
        text: Color32::from_rgb(0x6e, 0x5b, 0x4d),
        muted: Color32::from_rgb(0x7c, 0x5f, 0x43),
        accent: Color32::from_rgb(0x9f, 0x50, 0x37),
        accent_hover: Color32::from_rgb(0xb6, 0x5f, 0x43),
        accent_faded: Color32::from_rgb(0xf4, 0xe0, 0xcf),
        hover_bg: Color32::from_rgb(0xf2, 0xe6, 0xd8),
        green: Color32::from_rgb(0x3e, 0x6b, 0x48),
        red: Color32::from_rgb(0xa3, 0x3b, 0x2e),
        on_accent: Color32::from_rgb(0xfd, 0xf6, 0xf0),
    }
}

pub const fn dark_palette() -> Palette {
    Palette {
        cream: Color32::from_rgb(0x21, 0x1c, 0x18),
        card: Color32::from_rgb(0x2b, 0x25, 0x20),
        border: Color32::from_rgb(0x4b, 0x3f, 0x35),
        rule: Color32::from_rgb(0x6d, 0x5c, 0x4b),
        ink: Color32::from_rgb(0xf2, 0xe8, 0xdd),
        text: Color32::from_rgb(0xd3, 0xc2, 0xb2),
        muted: Color32::from_rgb(0xa9, 0x93, 0x7e),
        accent: Color32::from_rgb(0xcd, 0x74, 0x51),
        accent_hover: Color32::from_rgb(0xdb, 0x86, 0x64),
        accent_faded: Color32::from_rgb(0x50, 0x39, 0x2b),
        hover_bg: Color32::from_rgb(0x38, 0x30, 0x28),
        green: Color32::from_rgb(0x8c, 0xbf, 0x99),
        red: Color32::from_rgb(0xe0, 0x82, 0x6e),
        on_accent: Color32::from_rgb(0xff, 0xf8, 0xf2),
    }
}

static DARK_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Is the dark palette active?
pub fn is_dark() -> bool {
    DARK_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Activate a palette mode and (if `ctx` given) restyle the running app.
/// The caller persists the choice in settings.
pub fn set_dark(dark: bool, ctx: Option<&Context>) {
    DARK_MODE.store(dark, std::sync::atomic::Ordering::Relaxed);
    if let Some(ctx) = ctx {
        setup_style(ctx);
        ctx.request_repaint();
    }
}

fn pal() -> Palette {
    if is_dark() { dark_palette() } else { light_palette() }
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

/// Aliases other screens reference for the same tokens.
/// The warm accent-brown used on page numbers and grips.
pub fn brown() -> Color32 { pal().accent }

/// Named family: the house serif (headings, hero titles, exam preview).
pub fn serif_family() -> FontFamily {
    FontFamily::Name("serif".into())
}
/// Named family: heavier serif for large headings.
pub fn serif_bold_family() -> FontFamily {
    FontFamily::Name("serif-bold".into())
}
/// Named family: real bold sans (used by [`strong`]).
pub fn bold_family() -> FontFamily {
    FontFamily::Name("sans-bold".into())
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

static FONTS: OnceLock<Option<FontDefinitions>> = OnceLock::new();

/// Font directory: `<project>/assets/fonts` when run from the project root,
/// or `../assets/fonts` relative to the executable (target/debug/...).
pub fn fonts_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    if cwd.join("assets/fonts").is_dir() {
        return Some(cwd.join("assets/fonts"));
    }
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let up = exe_dir.join("../../assets/fonts");
    if up.is_dir() { Some(up) } else { None }
}

/// Loads the house faces once. Returns `None` (and the app then quietly keeps
/// egui's default fonts) when the font directory is missing or the sans regular
/// face is absent — the app must start without it.
fn load_fonts() -> Option<FontDefinitions> {
    let dir = fonts_dir()?;
    let read = |name: &str| std::fs::read(dir.join(name));

    // The sans regular face is the one thing we can't do without.
    let sans = read("SourceSans3-Regular.ttf").ok()?;
    let mut defs = FontDefinitions::default();
    let put = |defs: &mut FontDefinitions, name: &str, bytes: Vec<u8>| {
        defs.font_data
            .insert(name.to_string(), Arc::new(FontData::from_owned(bytes)));
    };

    put(&mut defs, "SourceSans3-Regular", sans);
    if let Ok(b) = read("SourceSans3-It.ttf") {
        put(&mut defs, "SourceSans3-It", b);
    }
    if let Ok(b) = read("SourceSans3-Semibold.ttf") {
        put(&mut defs, "SourceSans3-Semibold", b);
    }
    if let Ok(b) = read("SourceSans3-Bold.ttf") {
        put(&mut defs, "SourceSans3-Bold", b);
    }
    if let Ok(b) = read("SourceSerif4-Regular.ttf") {
        put(&mut defs, "SourceSerif4-Regular", b);
    }
    if let Ok(b) = read("SourceSerif4-Semibold.ttf") {
        put(&mut defs, "SourceSerif4-Semibold", b);
    }
    if let Ok(b) = read("SourceSerif4-Bold.ttf") {
        put(&mut defs, "SourceSerif4-Bold", b);
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
    // Named families: ordered glyph-coverage fallbacks, egui defaults behind them.
    let named = |name: &'static str, list: Vec<&'static str>| {
        (
            FontFamily::Name(Arc::<str>::from(name)),
            list.iter()
                .filter(|f| defs.font_data.contains_key(**f))
                .map(|f| f.to_string())
                .chain(fallback.iter().cloned())
                .collect::<Vec<String>>(),
        )
    };
    let (fam, fonts) = named("sans-bold", vec!["SourceSans3-Bold", "SourceSans3-Semibold", "SourceSans3-Regular"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("serif", vec!["SourceSerif4-Regular", "SourceSerif4-Semibold", "SourceSans3-Regular"]);
    defs.families.insert(fam, fonts);
    let (fam, fonts) = named("serif-bold", vec!["SourceSerif4-Bold", "SourceSerif4-Semibold", "SourceSerif4-Regular"]);
    defs.families.insert(fam, fonts);
    Some(defs)
}

/// Registered font set, initialized once on first use.
pub fn fonts() -> &'static Option<FontDefinitions> {
    FONTS.get_or_init(load_fonts)
}

pub fn setup(ctx: &Context) {
    setup_fonts(ctx);
    setup_style(ctx);
}

fn setup_fonts(ctx: &Context) {
    if let Some(defs) = fonts() {
        ctx.set_fonts(defs.clone());
    }
    // Missing fonts: silently keep egui defaults; screens still read fine.
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
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.faint_bg_color = p.cream;
        v.extreme_bg_color = if is_dark() { p.cream } else { p.card }; // text edit backgrounds
        v.text_edit_bg_color = Some(p.card);
        v.code_bg_color = p.accent_faded;
        v.hyperlink_color = p.accent;
        v.warn_fg_color = p.muted;
        v.error_fg_color = p.red;
        v.selection.bg_fill = p.accent_faded;
        v.selection.stroke = Stroke::new(0.0, p.ink);

        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.inactive.weak_bg_fill = p.card;
        v.widgets.inactive.bg_fill = p.card;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.hovered.weak_bg_fill = p.hover_bg;
        v.widgets.hovered.bg_fill = p.hover_bg;
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.hovered.corner_radius = CornerRadius::same(6);
        v.widgets.active.weak_bg_fill = p.accent_faded;
        v.widgets.active.bg_fill = p.accent_faded;
        v.widgets.active.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
        v.widgets.active.corner_radius = CornerRadius::same(6);
        v.widgets.open.weak_bg_fill = p.hover_bg;
        v.widgets.open.bg_fill = p.hover_bg;
        v.widgets.open.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.open.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.open.corner_radius = CornerRadius::same(6);

        style.text_styles.clear();
        style.text_styles.insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Button, FontId::new(13.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Small, FontId::new(11.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Name("Heading2".into()),
            FontId::new(17.0, FontFamily::Proportional),
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

    /// When run from the project root (cargo's default cwd), the bundled Source
    /// fonts are present and registered. The named serif/bold families and the
    /// Proportional list must all reference real font_data entries.
    #[test]
    fn bundled_fonts_register_named_families() {
        if fonts_dir().is_none() {
            // Not run from the project root (e.g. packaged): house fonts are an
            // optional startup asset; the fallback path keeps egui defaults.
            assert!(load_fonts().is_none());
            return;
        }
        let defs = load_fonts().expect("fonts dir present but load failed");
        for key in [
            "SourceSans3-Regular",
            "SourceSans3-It",
            "SourceSans3-Semibold",
            "SourceSans3-Bold",
            "SourceSerif4-Regular",
            "SourceSerif4-Semibold",
            "SourceSerif4-Bold",
        ] {
            assert!(defs.font_data.contains_key(key), "missing font {key}");
        }
        for family in ["sans-bold", "serif", "serif-bold"] {
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

    /// The date math behind the Imports list: known dates (math lives in screens.rs).
    #[test]
    fn civil_days_matches_known_dates() {
        assert_eq!(crate::screens::civil_from_days(0), (1970, 1, 1));
        assert_eq!(crate::screens::civil_from_days(19_468), (2023, 4, 21));
        assert_eq!(crate::screens::civil_from_days(19_782), (2024, 2, 29)); // leap day
    }
}