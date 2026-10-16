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
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, RichText, Stroke,
    TextStyle, ThemePreference,
};

// ---------- palette ----------

/// Whole-app background (warm cream).
pub const CREAM: Color32 = Color32::from_rgb(0xf7, 0xf4, 0xed);
/// Cards, windows, widgets (slightly whiter than the page).
pub const CARD: Color32 = Color32::from_rgb(0xff, 0xfd, 0xf8);
/// Hairline borders around cards and windows.
pub const BORDER: Color32 = Color32::from_rgb(0xe0, 0xd3, 0xc2);
/// Ruled answer lines on printed forms and dialogs.
pub const RULE: Color32 = Color32::from_rgb(0xbd, 0xa9, 0x95);
/// Headings and emphasized text.
pub const INK: Color32 = Color32::from_rgb(0x33, 0x2a, 0x24);
/// Regular body text (warm brown).
pub const TEXT: Color32 = Color32::from_rgb(0x6e, 0x5b, 0x4d);
/// De-emphasized text (captions, counters).
pub const MUTED: Color32 = Color32::from_rgb(0x8a, 0x6a, 0x4c);
/// Buttons / filled actions.
pub const ACCENT: Color32 = Color32::from_rgb(0x9f, 0x50, 0x37);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0xb6, 0x5f, 0x43);
/// Soft accent wash (chips, selection, active widgets).
pub const ACCENT_FADED: Color32 = Color32::from_rgb(0xf4, 0xe0, 0xcf);
/// Hover state on cards and widgets.
pub const HOVER_BG: Color32 = Color32::from_rgb(0xf2, 0xe6, 0xd8);
/// Correct-answer marks.
pub const GREEN: Color32 = Color32::from_rgb(0x3e, 0x6b, 0x48);
/// Errors and destructive confirmations.
pub const RED: Color32 = Color32::from_rgb(0xa3, 0x3b, 0x2e);

/// Aliases other screens reference for the same tokens.
pub const BAD: Color32 = RED;
/// The warm accent-brown used on page numbers and grips.
pub const BROWN: Color32 = Color32::from_rgb(0x9f, 0x50, 0x37);

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

/// House-serif heading text in ink. Sizes are set by the caller (`.size(34.0)`).
pub fn serif(text: impl IntoText) -> RichText {
    text.into_text().family(serif_family()).color(INK)
}

/// Heavier serif (large headings, the logo wordmark).
pub fn serif_strong(text: impl IntoText) -> RichText {
    text.into_text().family(serif_bold_family()).color(INK)
}

/// True bold sans, in ink. Use instead of `RichText::strong()`, which in egui
/// only darkens the color rather than changing the weight.
pub fn strong(text: impl IntoText) -> RichText {
    text.into_text().family(bold_family()).color(INK)
}

/// Regular body text in the warm brown.
pub fn text(text: impl IntoText) -> RichText {
    text.into_text().color(TEXT)
}

/// Muted caption text.
pub fn muted(text: impl IntoText) -> RichText {
    text.into_text().color(MUTED)
}

/// Accent-colored text (selected nav items, links).
pub fn accent(text: impl IntoText) -> RichText {
    text.into_text().color(ACCENT)
}

/// Inverse text for filled (accent) buttons and chips.
pub fn on_accent(text: impl IntoText) -> RichText {
    text.into_text().color(Color32::from_rgb(0xfd, 0xf6, 0xf0))
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
    // The house palette is a light theme; ignore the system dark preference.
    ctx.set_theme(ThemePreference::Light);
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.dark_mode = false;
        v.panel_fill = CREAM;
        v.window_fill = CARD;
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.faint_bg_color = CREAM;
        v.extreme_bg_color = CARD; // text edit backgrounds
        v.text_edit_bg_color = Some(CARD);
        v.code_bg_color = ACCENT_FADED;
        v.hyperlink_color = ACCENT;
        v.warn_fg_color = MUTED;
        v.error_fg_color = RED;
        v.selection.bg_fill = ACCENT_FADED;
        v.selection.stroke = Stroke::new(0.0, INK);

        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.inactive.weak_bg_fill = CARD;
        v.widgets.inactive.bg_fill = CARD;
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, INK);
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.inactive.corner_radius = CornerRadius::same(6);
        v.widgets.hovered.weak_bg_fill = HOVER_BG;
        v.widgets.hovered.bg_fill = HOVER_BG;
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, INK);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.hovered.corner_radius = CornerRadius::same(6);
        v.widgets.active.weak_bg_fill = ACCENT_FADED;
        v.widgets.active.bg_fill = ACCENT_FADED;
        v.widgets.active.fg_stroke = Stroke::new(1.0, INK);
        v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        v.widgets.active.corner_radius = CornerRadius::same(6);
        v.widgets.open.weak_bg_fill = HOVER_BG;
        v.widgets.open.bg_fill = HOVER_BG;
        v.widgets.open.fg_stroke = Stroke::new(1.0, INK);
        v.widgets.open.bg_stroke = Stroke::new(1.0, BORDER);
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
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.menu_margin = egui::Margin::same(6);
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