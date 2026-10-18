// queShion V4 palette ("Study Studio in paper tones").
// V1 cream/brown neutrals + chalkboard-green accent + highlighter-yellow marker.
// Values taken from the V4 design boards (light and dark).
use egui::Color32;

pub struct Palette {
    pub desk: Color32,        // app background (under the faint 24px grid)
    pub rail: Color32,        // left dock
    pub card: Color32,        // cards, popovers, top bar
    pub field: Color32,       // inputs, inset areas
    pub border: Color32,
    pub rule: Color32,        // stronger divider / grip dots / checkbox outline
    pub ink: Color32,         // headings, key values
    pub text: Color32,        // body
    pub muted: Color32,       // labels, hints
    pub accent: Color32,      // primary button, links, active controls
    pub accent_hover: Color32,
    pub accent_wash: Color32, // selected chips, type pills, icon tiles
    pub on_accent: Color32,   // text/icons on accent
    pub highlighter: Color32, // active nav item, "just added", AI-needed tags
    pub danger: Color32,
    pub grid_line: Color32,   // 24px graph-paper grid (alpha-blended)
    pub table: Color32,       // exam desk behind the paper
}

pub const LIGHT: Palette = Palette {
    desk: Color32::from_rgb(0xf7, 0xf4, 0xed),
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
    accent_wash: Color32::from_rgb(0xdf, 0xe9, 0xd4),
    on_accent: Color32::from_rgb(0xf7, 0xfb, 0xf3),
    highlighter: Color32::from_rgb(0xf3, 0xdc, 0x6a),
    danger: Color32::from_rgb(0xa3, 0x3b, 0x2e),
    grid_line: Color32::from_rgba_premultiplied(51, 42, 36, 14),
    table: Color32::from_rgb(0xe8, 0xdf, 0xce),
};

pub const DARK: Palette = Palette {
    desk: Color32::from_rgb(0x21, 0x1c, 0x18),
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
    accent_wash: Color32::from_rgb(0x2d, 0x3b, 0x2c),
    on_accent: Color32::from_rgb(0x0e, 0x20, 0x12),
    highlighter: Color32::from_rgb(0xe9, 0xcf, 0x55),
    danger: Color32::from_rgb(0xe0, 0x82, 0x6e),
    grid_line: Color32::from_rgba_premultiplied(11, 10, 9, 11),
    table: Color32::from_rgb(0x16, 0x12, 0x10),
};

// The exam paper is white in both modes.
pub const PAPER: Color32 = Color32::from_rgb(0xff, 0xfd, 0xf8);
pub const PAPER_EDGE: Color32 = Color32::from_rgb(0xc9, 0xb8, 0xa5);
pub const PAPER_TEXT: Color32 = Color32::from_rgb(0x22, 0x22, 0x22);

// Radii (px). Pills use height / 2.
pub const R_CARD: u8 = 18; // cards, tiles, dock, popovers use 16
pub const R_FIELD: u8 = 12;
pub const R_ICON_TILE: u8 = 12;
