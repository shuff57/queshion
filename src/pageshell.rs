//! The reference sites' page-shell layout for the app's own screens: soft-shadow cards with 16 px
//! corners and 20 px padding, and 40 px buttons with 12 px corners. Colours and fonts stay the
//! app's own (cream page, green accent, serif headings).
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui, Vec2};

use crate::theme;

/// One white card: 16 px corners, hairline border, soft shadow, 20 px padding.
pub fn card<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(16))
        .shadow(egui::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(if theme::is_dark() { 60 } else { 10 }) })
        .inner_margin(Margin::same(20))
        .show(ui, |ui| {
            ui.set_width(w - 42.0);
            f(ui)
        })
        .inner
}

/// A card's heading.
pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).family(theme::serif_family()).size(19.0).color(theme::ink()));
}

/// The page title with its breadcrumb above.
pub fn title(ui: &mut Ui, crumbs: &str, title: &str, sub: &str) {
    ui.label(RichText::new(crumbs).size(12.8).color(theme::muted_col()));
    ui.label(RichText::new(title).family(theme::serif_family()).size(34.0).color(theme::ink()));
    ui.label(theme::muted(sub).size(14.0));
}

fn button(ui: &mut Ui, text: &str, fill: Color32, stroke: Stroke, fg: Color32) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = Vec2::new(18.0, 4.0);
        ui.add(
            egui::Button::new(RichText::new(text).family(theme::bold_family()).size(15.2).color(fg))
                .fill(fill)
                .stroke(stroke)
                .corner_radius(CornerRadius::same(12))
                .min_size(Vec2::new(0.0, 40.0)),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

/// The solid accent action.
pub fn primary(ui: &mut Ui, text: &str) -> egui::Response {
    button(ui, text, theme::accent_col(), Stroke::NONE, theme::on_accent_col())
}

/// The outlined action.
pub fn outline(ui: &mut Ui, text: &str) -> egui::Response {
    button(ui, text, theme::card(), Stroke::new(1.0, theme::rule()), theme::ink())
}
