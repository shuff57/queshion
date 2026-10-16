//! V4 widget kit: the shared pills, headers and toggles every screen uses so the
//! chrome stays uniform ("Study Studio in paper tones"). Pure egui, painter-drawn.

use egui::{Color32, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme;

/// Cards/List view choice shared by Home and the Bank screens.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ViewMode {
    #[default]
    Cards,
    List,
}

/// Card radius (cards, tiles, popovers).
pub const R_CARD: u8 = 16;
/// Field radius.
pub const R_FIELD: u8 = 12;

fn pill_radius(h: f32) -> CornerRadius {
    CornerRadius::same((h / 2.0) as u8)
}

/// The one primary button: filled accent pill.
pub fn primary_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).family(theme::bold_family()).color(theme::on_accent_col()))
            .fill(theme::accent_col())
            .stroke(Stroke::NONE)
            .corner_radius(pill_radius(34.0))
            .min_size(Vec2::new(0.0, 34.0)),
    )
}

/// Ghost/outline pill for every secondary action.
pub fn ghost_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).family(theme::bold_family()).color(theme::ink()))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme::rule()))
            .corner_radius(pill_radius(34.0))
            .min_size(Vec2::new(0.0, 34.0)),
    )
}

/// Danger text button (Delete).
pub fn danger_text_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).family(theme::bold_family()).color(theme::bad()))
            .frame(false),
    )
}

/// Small-caps label: 10–12 px, bold, uppercase, letter-spaced look, muted.
pub fn small_caps(ui: &mut Ui, text: &str) {
    ui.label(small_caps_text(text));
}

pub fn small_caps_text(text: &str) -> RichText {
    // egui has no letter-spacing; thin spaces between letters approximate .09em.
    let spaced: String = text
        .to_uppercase()
        .chars()
        .flat_map(|c| [c, '\u{200A}'])
        .collect();
    RichText::new(spaced).family(theme::bold_family()).size(10.5).color(theme::muted_col())
}

/// Filled chip: pill with a wash fill and bold small label.
pub fn pill(ui: &mut Ui, label: &str, fill: Color32, fg: Color32) -> egui::Response {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(9, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(label).family(theme::bold_family()).size(11.0).color(fg));
        })
        .response
}

/// Type pill (MC, TF, FILL…): accent wash with accent text.
pub fn type_pill(ui: &mut Ui, label: &str) {
    pill(ui, label, theme::accent_faded(), theme::accent_col());
}

/// Marker pill ("Needs AI", "Just added"): highlighter fill, dark ink text.
pub fn marker_pill(ui: &mut Ui, label: &str) {
    pill(ui, label, theme::highlighter(), theme::on_highlighter());
}

/// Outline chip used for filters/topics; `on` fills it with the accent wash.
pub fn chip_toggle(ui: &mut Ui, label: &str, on: bool) -> egui::Response {
    let fill = if on { theme::accent_faded() } else { Color32::TRANSPARENT };
    let stroke = if on { Stroke::new(1.0, theme::accent_col()) } else { Stroke::new(1.0, theme::border()) };
    let fg = if on { theme::accent_col() } else { theme::text_col() };
    ui.add(
        egui::Button::new(RichText::new(label).family(theme::bold_family()).size(12.0).color(fg))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(255))
            .min_size(Vec2::new(0.0, 26.0)),
    )
}

/// Cards/List two-segment pill. Returns true when the mode changed.
pub fn view_toggle(ui: &mut Ui, mode: &mut ViewMode) -> bool {
    let size = Vec2::new(72.0, 30.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(rect, pill_radius(30.0), theme::card(), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
    let mut changed = false;
    for (i, m) in [ViewMode::Cards, ViewMode::List].into_iter().enumerate() {
        let seg = Rect::from_min_size(rect.min + Vec2::new(i as f32 * 36.0, 0.0), Vec2::new(36.0, 30.0)).shrink(2.0);
        let resp = ui.interact(seg, ui.id().with(("view_seg", i)), Sense::click());
        let on = *mode == m;
        if on {
            ui.painter().rect_filled(seg, pill_radius(26.0), theme::accent_faded());
        } else if resp.hovered() {
            ui.painter().rect_filled(seg, pill_radius(26.0), theme::hover_bg());
        }
        let col = if on { theme::accent_col() } else { theme::muted_col() };
        let c = seg.center();
        if i == 0 {
            for (dx, dy) in [(-3.5, -3.5), (3.5, -3.5), (-3.5, 3.5), (3.5, 3.5)] {
                ui.painter().rect_filled(Rect::from_center_size(c + Vec2::new(dx, dy), Vec2::splat(5.0)), CornerRadius::same(1), col);
            }
        } else {
            for dy in [-4.5, 0.0, 4.5] {
                ui.painter().line_segment([c + Vec2::new(-6.0, dy), c + Vec2::new(6.0, dy)], Stroke::new(1.8, col));
            }
        }
        if resp.clicked() && !on {
            *mode = m;
            changed = true;
        }
    }
    changed
}

/// Screen header: small breadcrumb line, then a serif title (34–40 px) and an
/// optional muted subtitle. Replaces the old top bar.
pub fn page_header(ui: &mut Ui, crumbs: &[&str], title: &str, subtitle: Option<&str>) {
    if !crumbs.is_empty() {
        ui.label(
            RichText::new(crumbs.join("  /  ")).size(11.5).color(theme::muted_col()),
        );
    }
    ui.label(theme::serif(title).size(36.0));
    if let Some(sub) = subtitle {
        ui.label(theme::muted(sub).size(13.0));
    }
}

/// Card frame: radius 16, 1 px border, no heavy shadow.
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(R_CARD))
}

/// Round answer-correct circle: filled accent with a check when `on`.
pub fn check_circle(ui: &mut Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let c = rect.center();
    if on {
        ui.painter().circle_filled(c, 10.0, theme::accent_col());
        let s = Stroke::new(2.0, theme::on_accent_col());
        ui.painter().line_segment([c + Vec2::new(-4.5, 0.5), c + Vec2::new(-1.5, 3.5)], s);
        ui.painter().line_segment([c + Vec2::new(-1.5, 3.5), c + Vec2::new(4.5, -3.0)], s);
    } else {
        ui.painter().circle_stroke(c, 10.0, Stroke::new(1.5, if resp.hovered() { theme::accent_col() } else { theme::rule() }));
    }
    resp
}

/// Text at a pt-size on the exam page etc. is the caller's business; this just
/// builds a UI-font id used by dock/labels.
pub fn ui_font(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Proportional)
}

/// Used by painter code that needs a centered label.
pub fn center_text(ui: &Ui, pos: Pos2, text: &str, size: f32, col: Color32) {
    ui.painter().text(pos, egui::Align2::CENTER_CENTER, text, ui_font(size), col);
}
