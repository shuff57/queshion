//! The figure-specific cards of the side panel (`figures::cards`), as on the reference
//! generators: gel lane cards, per-force / per-vector cards with quick directions, the
//! free-body Motion card, population-growth and predator-prey readouts with "Fit now" and
//! axis-fit boxes that keep the range, line-spectrum strip cards and particle kind cards.
//! Structural edits (add, remove, move, setups, Fit now) are one undo step each; typing and
//! sliders settle into undo steps like any other control.

use super::*;
use crate::figures::biology::{gel, popdyn};
use crate::figures::cards::{self, Card};
use crate::figures::chemistry::{atoms, spectra};
use crate::figures::items;

const LABEL_MODES: [&str; 3] = ["Text", "Blank line", "None"];
const ARROWS: [&str; 3] = ["Drawn", "Dashed", "Left off"];

/// A one-line text box that keeps exactly what is typed (spaces included) while it has focus,
/// even though the stored value is tidied (trimmed) on every keystroke.
fn buffered_edit(ui: &mut Ui, id: egui::Id, current: &str, width: f32, hint: &str) -> Option<String> {
    let focused = ui.memory(|m| m.has_focus(id));
    let mut buf = if focused { ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| current.to_string()) } else { current.to_string() };
    let r = ui.add(TextEdit::singleline(&mut buf).id(id).desired_width(width).hint_text(hint));
    ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    r.changed().then_some(buf)
}

/// A small frameless icon button (move / remove).
fn icon(ui: &mut Ui, glyph: &str, on: bool, hover: &str) -> bool {
    let b = Button::new(RichText::new(glyph).size(14.0)).frame(false).min_size(Vec2::splat(22.0));
    ui.add_enabled(on, b).on_hover_text(hover).on_disabled_hover_text(hover).clicked()
}

/// A bordered card for one item.
fn item_frame(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    // no fill: slider tracks are drawn in the card colour and would vanish on it
    Frame::new()
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            f(ui);
        });
    ui.add_space(5.0);
}

/// The card title on the left and its buttons (laid out right to left) on the right.
fn header(ui: &mut Ui, title: &str, buttons: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(theme::strong(title).size(13.0));
        ui.with_layout(Layout::right_to_left(Align::Center), buttons);
    });
}

/// A row of choices as toggle buttons; returns the newly picked one.
fn seg(ui: &mut Ui, opts: &[&'static str], cur: &str) -> Option<String> {
    let mut c = cur.to_string();
    segmented(ui, opts, &mut c);
    (c != cur).then_some(c)
}

/// "+ Sample": a stroke plus icon, then the label in bold.
fn plus_job(ui: &Ui, label: &str) -> egui::text::LayoutJob {
    let col = ui.visuals().widgets.inactive.fg_stroke.color;
    let mut job = egui::text::LayoutJob::default();
    job.append(egui_phosphor::regular::PLUS, 0.0, egui::TextFormat { font_id: theme::icon_font(18.0), color: col, ..Default::default() });
    job.append(label, 9.0, egui::TextFormat { font_id: egui::FontId::new(14.0, theme::bold_family()), color: col, ..Default::default() });
    job
}

/// The gel chip: a stroke plus icon and a bold blue label.
fn chip_job(label: &str) -> egui::text::LayoutJob {
    let col = theme::accent_col();
    let mut job = egui::text::LayoutJob::default();
    job.append(egui_phosphor::regular::PLUS, 0.0, egui::TextFormat { font_id: theme::icon_font(16.0), color: col, ..Default::default() });
    job.append(label, 11.0, egui::TextFormat { font_id: egui::FontId::new(14.4, theme::bold_family()), color: col, ..Default::default() });
    job
}

fn small(ui: &mut Ui, text: &str) {
    ui.label(theme::muted(text).size(11.0));
}

/// The reference's direction control: a 0–360° slider, the angle in degrees and the
/// Up / Down / Left / Right quick buttons (the current one highlighted). Returns a new angle.
fn direction(ui: &mut Ui, ang: f64) -> Option<f64> {
    let mut out = None;
    small(ui, "Direction");
    ui.horizontal(|ui| {
        let mut v = ang;
        ui.spacing_mut().slider_width = 130.0;
        if ui.add(egui::Slider::new(&mut v, 0.0..=360.0).step_by(1.0).show_value(false)).changed() {
            out = Some(v);
        }
        let mut d = ang;
        if ui.add(egui::DragValue::new(&mut d).range(0.0..=360.0).speed(1.0).max_decimals(1).suffix("°")).changed() {
            out = Some(d);
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for (name, a) in items::QUICK_DIRS {
            let on = (ang.rem_euclid(360.0) - a).abs() < 1e-6;
            let b = Button::new(RichText::new(name).size(12.0).color(if on { theme::accent_col() } else { theme::muted_col() }))
                .fill(if on { theme::accent_faded() } else { egui::Color32::TRANSPARENT })
                .stroke(Stroke::new(1.0, if on { theme::accent_col() } else { theme::border() }))
                .corner_radius(CornerRadius::same(8));
            if ui.add(b).on_hover_text(format!("Point it {}", name.to_lowercase())).clicked() && !on {
                out = Some(a);
            }
        }
    });
    out
}

/// Label: Text / Blank line / None, with the text box when it is "Text".
fn label_control(ui: &mut Ui, p: &mut Params, mode_key: &str, text_key: &str, title: &str) {
    small(ui, title);
    if let Some(m) = seg(ui, &LABEL_MODES, p.s(mode_key)) {
        p.set_s(mode_key, &m);
    }
    if p.s(mode_key) == "Text" {
        let mut t = p.s(text_key).to_string();
        if ui.add(TextEdit::singleline(&mut t).desired_width(160.0)).changed() {
            p.set_s(text_key, &t);
        }
    }
}

/// Bulleted readout lines.
fn readout(ui: &mut Ui, lines: &[String]) -> Option<egui::Response> {
    if theme::chem_skin() && !lines.is_empty() {
        // the biology page's note: a pale blue rounded box of semibold 13.6 px lines on a 20 px line
        let fr = egui::Frame::new().fill(theme::accent_faded()).corner_radius(egui::CornerRadius::same(10)).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 3.0;
            for l in lines {
                ui.add(egui::Label::new(RichText::new(l.as_str()).size(13.6).family(theme::semibold_family()).color(theme::ink()).line_height(Some(20.0))).wrap());
            }
        });
        return Some(fr.response);
    }
    for l in lines {
        ui.horizontal_top(|ui| {
            ui.label(theme::muted("•").size(12.0));
            ui.add(egui::Label::new(theme::text(l.as_str()).size(12.0)).wrap());
        });
    }
    None
}

impl Workspace {
    /// Commits a card edit: structural ones as their own undo step, the rest settle as usual.
    fn card_commit(&mut self, q: Params, step: bool) {
        if step {
            self.set_params_with_history(q);
        } else {
            self.params = q;
        }
    }

    /// Draws the card that replaces param `key` (true), or nothing (false).
    pub(super) fn card_control(&mut self, ui: &mut Ui, fig: &str, key: &str, pdefs: &[ParamDef]) -> bool {
        let Some(card) = cards::replaces(fig, key) else { return false };
        self.draw_card(ui, fig, card, pdefs);
        true
    }

    /// Draws the card that follows param `key`, if any.
    pub(super) fn card_after(&mut self, ui: &mut Ui, fig: &str, key: &str, pdefs: &[ParamDef]) {
        if let Some(card) = cards::after(fig, key) {
            self.draw_card(ui, fig, card, pdefs);
        }
    }

    fn draw_card(&mut self, ui: &mut Ui, fig: &str, card: Card, pdefs: &[ParamDef]) {
        match card {
            Card::GelLanes => self.gel_lanes_card(ui),
            Card::Forces => self.forces_card(ui, pdefs),
            Card::Motion => self.motion_card(ui),
            Card::Vectors => self.vectors_card(ui, pdefs),
            Card::GrowthFit => self.growth_fit_card(ui, fig),
            Card::PpFit(x) => self.pp_fit_box(ui, x),
            Card::PpReadout => {
                ui.add_space(2.0);
                readout(ui, &cards::readout(fig, &self.params));
                ui.label(theme::muted("How the cycle is worked out").size(11.0).underline()).on_hover_text(
                    "From the Lotka–Volterra model: prey grow and are eaten, predators grow on the prey they eat and die off. The populations swing around their averages, the predators’ peaks a little after the prey’s.",
                );
            }
            Card::TitReadout => {
                let skin = theme::chem_skin();
                if self.params.s("source") == "Key points" {
                    ui.add(egui::Label::new(theme::muted(crate::figures::chemistry::tit_end_note(&self.params)).size(11.0)).wrap());
                    ui.add_space(4.0);
                }
                if super::chem_shell::chem_page(ui.ctx()) {
                    // 12 px under the number boxes
                    ui.add_space(5.0);
                }
                Frame::new()
                    .fill(theme::accent_faded())
                    .corner_radius(CornerRadius::same(if skin { 10 } else { 8 }))
                    .inner_margin(if skin { Margin::symmetric(12, 9) } else { Margin::same(8) })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let chem = super::chem_shell::chem_page(ui.ctx());
                        if skin {
                            ui.spacing_mut().item_spacing.y = if chem { 2.5 } else { 3.0 };
                        }
                        for l in cards::readout(fig, &self.params) {
                            // the chemistry reference sets each line in 13.6 px on a 20 px line
                            let t = if chem { theme::strong(l).size(13.6).line_height(Some(20.0)) } else { theme::strong(l).size(if skin { 13.6 } else { 12.0 }) };
                            ui.add(egui::Label::new(t).wrap());
                        }
                    });
            }
            Card::Strips => self.strips_card(ui),
            Card::Particles => self.particles_card(ui, pdefs),
        }
    }

    /// Resets the params a section's cards own (the section's Reset button).
    pub(super) fn reset_card_keys(&mut self, fig: &str, anchors: &[&str], pdefs: &[ParamDef], start: &Params) {
        let owned: Vec<Card> = anchors.iter().filter_map(|k| cards::replaces(fig, k)).collect();
        for d in pdefs {
            if cards::owner(fig, d.key).is_some_and(|c| owned.contains(&c)) {
                self.params.0.insert(d.key.to_string(), start.0.get(d.key).cloned().unwrap_or_else(|| d.default.clone()));
            }
        }
        if owned.contains(&Card::Particles) {
            self.kinds_keep = [false; 4];
        }
    }

    // ------------------------------------------------------------ gel lanes

    /// The biology page's Lanes card: bold labels, 40 px boxes, lanes split by rules, icon buttons.
    fn gel_lanes_card_skin(&mut self, ui: &mut Ui) {
        let noto = || egui::FontFamily::Name("vs-noto".into());
        let cur = gel::current_setup(&self.params);
        ui.spacing_mut().item_spacing.y = 0.0;
        param_label(ui, "Classic setups", "");
        ui.add_space(5.0);
        let mut pick = None;
        let combo = |ui: &mut Ui, id: egui::Id, shown: &str, f: &mut dyn FnMut(&mut Ui)| {
            ui.scope(|ui| {
                ui.spacing_mut().button_padding = Vec2::new(15.0, 9.5);
                ui.spacing_mut().interact_size.y = 39.5;
                let w = ui.available_width();
                ComboBox::from_id_salt(id).icon(super::chem_shell::select_chevron).selected_text(RichText::new(shown).size(15.2).family(egui::FontFamily::Name("vs-noto".into()))).width(w).show_ui(ui, |ui| f(ui));
            });
        };
        combo(ui, egui::Id::new("gel_setup"), cur.unwrap_or("Choose one…"), &mut |ui| {
            for s in gel::setup_names() {
                if ui.selectable_label(cur == Some(*s), *s).clicked() {
                    pick = Some(*s);
                }
            }
        });
        ui.add_space(8.0);
        ui.add(egui::Label::new(theme::muted(gel::setup_note(cur)).size(13.0).family(noto())).wrap());
        ui.add_space(16.0);
        if let Some(s) = pick {
            let mut q = self.params.clone();
            q.set_s("setup", s);
            self.card_commit(q, true);
        }
        let lanes = gel::lane_cards(&self.params);
        let n = lanes.len();
        let mut edit: Option<(Vec<gel::LaneCard>, bool)> = None;
        let rule = |ui: &mut Ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::border()));
        };
        let field = |ui: &mut Ui, id: egui::Id, cur: &str, hint: &str| -> Option<String> {
            let focused = ui.memory(|m| m.has_focus(id));
            let mut buf = if focused { ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| cur.to_string()) } else { cur.to_string() };
            let r = ui.add(TextEdit::singleline(&mut buf).id(id).desired_width(f32::INFINITY).margin(Margin::symmetric(11, 10)).hint_text(hint).font(egui::FontId::new(15.0, egui::FontFamily::Name("vs-noto".into()))));
            ui.data_mut(|d| d.insert_temp(id, buf.clone()));
            r.changed().then_some(buf)
        };
        for (i, lane) in lanes.iter().enumerate() {
            rule(ui);
            ui.add_space(15.0);
            let ladder = matches!(lane.kind, gel::LaneKind::Ladder(_));
            let name = if lane.label.is_empty() { format!("Sample {}", i + 1) } else { lane.label.clone() };
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size.y = 24.0;
                ui.label(theme::strong(format!("{}. {}", i + 1, if ladder { "Ladder" } else { "Sample" })).size(14.7));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 13.0;
                    let big = |ui: &mut Ui, g: &str, on: bool, tip: &str| {
                        let b = Button::new(RichText::new(g).size(18.0).color(theme::ink())).frame(false).min_size(Vec2::splat(30.0));
                        ui.add_enabled(on, b).on_hover_text(tip).on_disabled_hover_text(tip).clicked()
                    };
                    ui.add_space(4.0);
                    if big(ui, egui_phosphor::regular::TRASH, n > 1, &format!("Remove {name}")) {
                        let mut v = lanes.clone();
                        v.remove(i);
                        edit = Some((v, true));
                    }
                    if big(ui, egui_phosphor::regular::ARROW_RIGHT, i + 1 < n, &format!("Move {name} right")) {
                        let mut v = lanes.clone();
                        v.swap(i, i + 1);
                        edit = Some((v, true));
                    }
                    if big(ui, egui_phosphor::regular::ARROW_LEFT, i > 0, &format!("Move {name} left")) {
                        let mut v = lanes.clone();
                        v.swap(i, i - 1);
                        edit = Some((v, true));
                    }
                });
            });
            ui.add_space(11.0);
            param_label(ui, "Label", "");
            ui.add_space(5.0);
            let hint = if ladder { "e.g. Ladder" } else { "e.g. Suspect 1" };
            if let Some(t) = field(ui, ui.make_persistent_id(("gel_lab", i)), &lane.label, hint) {
                let mut v = lanes.clone();
                v[i].label = t;
                edit = Some((v, false));
            }
            ui.add_space(8.0);
            match &lane.kind {
                gel::LaneKind::Ladder(k) => {
                    param_label(ui, "Ladder", "");
                    ui.add_space(5.0);
                    let mut k2 = *k;
                    combo(ui, egui::Id::new(("gel_ladder", i)), gel::LADDERS[(*k).min(2)], &mut |ui| {
                        for (j, l) in gel::LADDERS.iter().enumerate() {
                            ui.selectable_value(&mut k2, j, *l);
                        }
                    });
                    if k2 != *k {
                        let mut v = lanes.clone();
                        v[i].kind = gel::LaneKind::Ladder(k2);
                        edit = Some((v, true));
                    }
                }
                gel::LaneKind::Sample(b) => {
                    param_label(ui, "Bands|in bp, separated by commas", "");
                    ui.add_space(5.0);
                    if let Some(t) = field(ui, ui.make_persistent_id(("gel_bands", i)), b, "e.g. 1200, 450, 300") {
                        let mut v = lanes.clone();
                        v[i].kind = gel::LaneKind::Sample(t);
                        edit = Some((v, false));
                    }
                }
            }
            ui.add_space(13.0);
        }
        rule(ui);
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 9.0;
            let room = n < gel::MAX_LANES;
            let chip = |ui: &mut Ui, text: &str, tip: &str| ui.add_enabled(room, Button::new(chip_job(text)).fill(egui::Color32::TRANSPARENT).stroke(Stroke::new(1.0, super::chem_shell::pill_border())).corner_radius(CornerRadius::same(20)).min_size(Vec2::new(0.0, 34.0))).on_hover_text(tip).clicked();
            if chip(ui, "Sample", "Add a sample lane") {
                let mut v = lanes.clone();
                v.push(gel::LaneCard::sample());
                edit = Some((v, true));
            }
            if chip(ui, "Ladder", "Add a ladder lane") {
                let mut v = lanes.clone();
                v.push(gel::LaneCard::ladder());
                edit = Some((v, true));
            }
        });
        ui.add_space(10.0);
        ui.add(egui::Label::new(theme::muted("Type “x2” after a size for twice as much DNA, a darker band (e.g. 450 x2), or “x0.5” for a fainter one. Sizes can be in kb, e.g. 1.5 kb.").size(13.0).family(noto())).wrap());
        ui.add_space(6.0);
        if let Some((v, step)) = edit {
            let mut q = self.params.clone();
            gel::set_lane_cards(&mut q, &v);
            self.card_commit(q, step);
        }
    }

    fn gel_lanes_card(&mut self, ui: &mut Ui) {
        if theme::chem_skin() {
            return self.gel_lanes_card_skin(ui);
        }
        let cur = gel::current_setup(&self.params);
        small(ui, "Classic setups");
        let mut pick = None;
        ComboBox::from_id_salt("gel_setup")
            .selected_text(cur.unwrap_or("Choose one…"))
            .width(ui.available_width().min(240.0))
            .show_ui(ui, |ui| {
                for s in gel::setup_names() {
                    if ui.selectable_label(cur == Some(*s), *s).clicked() {
                        pick = Some(*s);
                    }
                }
            });
        ui.add(egui::Label::new(theme::muted(gel::setup_note(cur)).size(11.0)).wrap());
        ui.add_space(4.0);
        if let Some(s) = pick {
            let mut q = self.params.clone();
            q.set_s("setup", s);
            self.card_commit(q, true);
        }
        let lanes = gel::lane_cards(&self.params);
        let n = lanes.len();
        let mut edit: Option<(Vec<gel::LaneCard>, bool)> = None;
        for (i, lane) in lanes.iter().enumerate() {
            let ladder = matches!(lane.kind, gel::LaneKind::Ladder(_));
            let name = if lane.label.is_empty() { format!("Sample {}", i + 1) } else { lane.label.clone() };
            item_frame(ui, |ui| {
                header(ui, &format!("{}. {}", i + 1, if ladder { "Ladder" } else { "Sample" }), |ui| {
                    if icon(ui, egui_phosphor::regular::X, n > 1, &format!("Remove {name}")) {
                        let mut v = lanes.clone();
                        v.remove(i);
                        edit = Some((v, true));
                    }
                    if icon(ui, egui_phosphor::regular::ARROW_RIGHT, i + 1 < n, &format!("Move {name} right")) {
                        let mut v = lanes.clone();
                        v.swap(i, i + 1);
                        edit = Some((v, true));
                    }
                    if icon(ui, egui_phosphor::regular::ARROW_LEFT, i > 0, &format!("Move {name} left")) {
                        let mut v = lanes.clone();
                        v.swap(i, i - 1);
                        edit = Some((v, true));
                    }
                });
                small(ui, "Label");
                let hint = if ladder { "e.g. Ladder" } else { "e.g. Suspect 1" };
                if let Some(t) = buffered_edit(ui, ui.make_persistent_id(("gel_lab", i)), &lane.label, 200.0, hint) {
                    let mut v = lanes.clone();
                    v[i].label = t;
                    edit = Some((v, false));
                }
                match &lane.kind {
                    gel::LaneKind::Ladder(k) => {
                        small(ui, "Ladder");
                        let mut k2 = *k;
                        ComboBox::from_id_salt(("gel_ladder", i)).selected_text(gel::LADDERS[(*k).min(2)]).width(200.0).show_ui(ui, |ui| {
                            for (j, l) in gel::LADDERS.iter().enumerate() {
                                ui.selectable_value(&mut k2, j, *l);
                            }
                        });
                        if k2 != *k {
                            let mut v = lanes.clone();
                            v[i].kind = gel::LaneKind::Ladder(k2);
                            edit = Some((v, true));
                        }
                    }
                    gel::LaneKind::Sample(b) => {
                        small(ui, "Bands in bp, separated by commas");
                        if let Some(t) = buffered_edit(ui, ui.make_persistent_id(("gel_bands", i)), b, 200.0, "e.g. 1200, 450, 300") {
                            let mut v = lanes.clone();
                            v[i].kind = gel::LaneKind::Sample(t);
                            edit = Some((v, false));
                        }
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            let room = n < gel::MAX_LANES;
            if ui.add_enabled(room, Button::new(plus_job(ui, "Sample")).min_size(Vec2::new(0.0, 34.0)).corner_radius(CornerRadius::same(17))).on_hover_text("Add a sample lane").clicked() {
                let mut v = lanes.clone();
                v.push(gel::LaneCard::sample());
                edit = Some((v, true));
            }
            if ui.add_enabled(room, Button::new(plus_job(ui, "Ladder")).min_size(Vec2::new(0.0, 34.0)).corner_radius(CornerRadius::same(17))).on_hover_text("Add a ladder lane").clicked() {
                let mut v = lanes.clone();
                v.push(gel::LaneCard::ladder());
                edit = Some((v, true));
            }
            ui.label(theme::muted(format!("{n} of {}", gel::MAX_LANES)).size(11.0));
        });
        ui.add(egui::Label::new(theme::muted("Type “x2” after a size for twice as much DNA, a darker band (e.g. 450 x2), or “x0.5” for a fainter one. Sizes can be in kb, e.g. 1.5 kb.").size(11.0)).wrap());
        if let Some((v, step)) = edit {
            let mut q = self.params.clone();
            gel::set_lane_cards(&mut q, &v);
            self.card_commit(q, step);
        }
    }

    // ------------------------------------------------------------ forces, motion, vectors

    /// The "Add a force / vector" chips inside the card (one undo step each).
    fn add_chips(&mut self, ui: &mut Ui, fig: &str, pdefs: &[ParamDef]) {
        let Some(card) = figures::chips_for(fig) else { return };
        let n = self.params.i(card.count_key);
        ui.horizontal(|ui| {
            crate::kit::small_caps(ui, card.title);
            ui.label(theme::muted(format!("{n} of {}", card.max)).size(11.0));
        });
        let mut pick = None;
        ui.horizontal_wrapped(|ui| {
            for (ci, chip) in card.chips.iter().enumerate() {
                let label = if card.chips.len() == 1 { format!("+ Add a {}", chip.label.to_lowercase()) } else { format!("+ {}", chip.label) };
                if ui.add_enabled(n < card.max, Button::new(RichText::new(label).size(12.0)).corner_radius(CornerRadius::same(10))).clicked() {
                    pick = Some(ci);
                }
            }
        });
        if let Some(ci) = pick {
            let mut q = self.params.clone();
            apply_chip(&mut q, pdefs, &card, &card.chips[ci]);
            self.card_commit(q, true);
        }
    }

    fn forces_card(&mut self, ui: &mut Ui, pdefs: &[ParamDef]) {
        let n = self.params.i("n").clamp(1, 8) as usize;
        let mut remove = None;
        for i in 1..=n {
            let k = |s: &str| format!("f{i}_{s}");
            item_frame(ui, |ui| {
                header(ui, &format!("Force {i}"), |ui| {
                    if icon(ui, egui_phosphor::regular::X, n > 1, &format!("Remove force {i}")) {
                        remove = Some(i);
                    }
                });
                if let Some(a) = direction(ui, self.params.f(&k("ang"))) {
                    self.params.set_f(&k("ang"), a.clamp(0.0, 360.0));
                }
                small(ui, "Length");
                ui.horizontal(|ui| {
                    let mut v = self.params.f(&k("len"));
                    ui.spacing_mut().slider_width = 130.0;
                    if ui.add(egui::Slider::new(&mut v, 0.25..=2.0).step_by(0.05).show_value(false)).changed() {
                        self.params.set_f(&k("len"), v);
                    }
                    ui.label(theme::muted(format!("{v:.2}×")).size(11.0));
                });
                label_control(ui, &mut self.params, &k("lab_mode"), &k("lab"), "Label");
                egui::CollapsingHeader::new(RichText::new("Angle and components").size(11.5)).id_salt(("fbd_more", i)).default_open(self.params.b(&k("mark")) || self.params.b(&k("comps"))).show(ui, |ui| {
                    let mut m = self.params.b(&k("mark"));
                    if ui.checkbox(&mut m, "Mark its angle").changed() {
                        self.params.set_b(&k("mark"), m);
                    }
                    if m {
                        small(ui, "Measured from");
                        if let Some(v) = seg(ui, &["Horizontal", "Vertical"], self.params.s(&k("from"))) {
                            self.params.set_s(&k("from"), &v);
                        }
                        label_control(ui, &mut self.params, &k("alab_mode"), &k("alab"), "Angle label");
                    }
                    let mut c = self.params.b(&k("comps"));
                    if ui.checkbox(&mut c, "Show its components").changed() {
                        self.params.set_b(&k("comps"), c);
                    }
                });
            });
        }
        if let Some(i) = remove {
            let mut q = self.params.clone();
            items::remove_item(&mut q, pdefs, "f", "n", i);
            self.card_commit(q, true);
        }
        self.add_chips(ui, "free-body-diagram", pdefs);
        ui.add_space(4.0);
    }

    fn motion_card(&mut self, ui: &mut Ui) {
        let m = self.params.s("motion").to_string();
        let (mut v, mut a) = (m == "Velocity" || m == "Both", m == "Acceleration" || m == "Both");
        let (v0, a0) = (v, a);
        ui.checkbox(&mut v, "Velocity");
        if v {
            ui.indent("fbd_vel", |ui| {
                if let Some(d) = direction(ui, self.params.f("mdir")) {
                    self.params.set_f("mdir", d.clamp(0.0, 360.0));
                }
                label_control(ui, &mut self.params, "mv_lab_mode", "mv_lab", "Label");
            });
        }
        ui.checkbox(&mut a, "Acceleration");
        if a {
            ui.indent("fbd_acc", |ui| {
                // -1 = "same as the velocity" in figures saved before the Motion card
                let cur = if self.params.f("adir") < 0.0 { self.params.f("mdir") } else { self.params.f("adir") };
                if let Some(d) = direction(ui, cur) {
                    self.params.set_f("adir", d.clamp(0.0, 360.0));
                }
                label_control(ui, &mut self.params, "ma_lab_mode", "ma_lab", "Label");
            });
        }
        if (v, a) != (v0, a0) {
            let mut q = self.params.clone();
            q.set_s("motion", match (v, a) { (true, true) => "Both", (true, false) => "Velocity", (false, true) => "Acceleration", _ => "None" });
            self.card_commit(q, true);
        }
    }

    fn vectors_card(&mut self, ui: &mut Ui, pdefs: &[ParamDef]) {
        let n = self.params.i("n").clamp(1, 4) as usize;
        let (mut remove, mut swap) = (None, None);
        for i in 1..=n {
            let k = |s: &str| format!("v{i}_{s}");
            let summary = format!("“{}” · {} squares at {}°", self.params.s(&k("lab")), fmt_measure(self.params.f(&k("mag"))), fmt_measure(self.params.f(&k("ang"))));
            let id = ui.make_persistent_id(("vec_card", i));
            item_frame(ui, |ui| {
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                    .show_header(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.label(theme::strong(format!("Vector {i}")).size(13.0));
                            ui.label(theme::muted(summary.as_str()).size(11.0));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icon(ui, egui_phosphor::regular::X, n > 1, &format!("Remove vector {i}")) {
                                remove = Some(i);
                            }
                            if icon(ui, egui_phosphor::regular::ARROW_DOWN, i < n, &format!("Move vector {i} later")) {
                                swap = Some((i, i + 1));
                            }
                            if icon(ui, egui_phosphor::regular::ARROW_UP, i > 1, &format!("Move vector {i} earlier")) {
                                swap = Some((i, i - 1));
                            }
                        });
                    })
                    .body(|ui| {
                        small(ui, "Magnitude");
                        ui.horizontal(|ui| {
                            let mut v = self.params.f(&k("mag"));
                            ui.spacing_mut().slider_width = 130.0;
                            if ui.add(egui::Slider::new(&mut v, 0.0..=12.0).step_by(0.5).show_value(false)).changed() {
                                self.params.set_f(&k("mag"), v);
                            }
                            ui.label(theme::muted(format!("{} squares", fmt_measure(v))).size(11.0));
                        });
                        if let Some(a) = direction(ui, self.params.f(&k("ang"))) {
                            self.params.set_f(&k("ang"), a.clamp(0.0, 360.0));
                        }
                        small(ui, "Arrow");
                        if let Some(v) = seg(ui, &ARROWS, self.params.s(&k("style"))) {
                            self.params.set_s(&k("style"), &v);
                        }
                        label_control(ui, &mut self.params, &k("lab_mode"), &k("lab"), "Label");
                    });
            });
        }
        if let Some(i) = remove {
            let mut q = self.params.clone();
            items::remove_item(&mut q, pdefs, "v", "n", i);
            self.card_commit(q, true);
        } else if let Some((a, b)) = swap {
            let mut q = self.params.clone();
            items::swap_items(&mut q, pdefs, "v", a, b);
            self.card_commit(q, true);
        }
        self.add_chips(ui, "vector-diagram", pdefs);
        ui.add_space(4.0);
    }

    // ------------------------------------------------------------ population readouts and fit

    fn growth_fit_card(&mut self, ui: &mut Ui, fig: &str) {
        ui.add_space(2.0);
        let tip =

            "Exponential growth is dN/dt = rN; logistic growth is dN/dt = rN(K − N)/K, leveling off at the carrying capacity K. Overshoot is logistic growth that feels crowding a lag τ late, so it swings around K. Boom and crash is a population using up food that doesn’t grow back.";
        let note = readout(ui, &cards::readout(fig, &self.params));
        if theme::chem_skin() {
            // the biology page keeps the explanation on the note itself
            if let Some(r) = note {
                r.on_hover_text(tip);
            }
            ui.add_space(0.0);
            let mut fit = self.params.b("fit");
            let mut now = false;
            ui.horizontal(|ui| {
                ui.set_height(28.5);
                if chem_shell::fit_checkbox(ui, &mut fit, "Fit the axes as these change").changed() {
                    let mut q = if fit { self.params.clone() } else { popdyn::growth_fit_now(&self.params) };
                    q.set_b("fit", fit);
                    self.card_commit(q, true);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    now = chem_shell::fit_now_button(ui).on_hover_text("Fit the x- and y-axis ranges to the curve now").clicked();
                });
            });
            if now {
                let q = popdyn::growth_fit_now(&self.params);
                self.card_commit(q, true);
            }
            return;
        }
        let how = ui.label(theme::muted("How the curves are worked out").size(11.0).underline());
        how.on_hover_text(tip);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut fit = self.params.b("fit");
            if ui.checkbox(&mut fit, "Fit the axes as these change").changed() {
                // unticking keeps the axes where they are (their boxes take the fitted range)
                let mut q = if fit { self.params.clone() } else { popdyn::growth_fit_now(&self.params) };
                q.set_b("fit", fit);
                self.card_commit(q, true);
            }
            if ui.button(RichText::new("Fit now").size(12.0)).on_hover_text("Fit the x- and y-axis ranges to the curve now").clicked() {
                let q = popdyn::growth_fit_now(&self.params);
                self.card_commit(q, true);
            }
        });
    }

    fn pp_fit_box(&mut self, ui: &mut Ui, x_axis: bool) {
        let key = if x_axis { "x_fit" } else { "y_fit" };
        let mut on = self.params.b(key) || !self.params.0.contains_key(key);
        let label = if x_axis { "Fit to the time shown" } else { "Fit to the populations" };
        let r = ui.checkbox(&mut on, label);
        if !x_axis {
            small(ui, "room for both lines up the side");
        }
        if r.changed() {
            let q = if on {
                let mut q = self.params.clone();
                q.set_b(key, true);
                q
            } else {
                popdyn::pp_unfit(&self.params, x_axis)
            };
            self.card_commit(q, true);
        }
    }

    // ------------------------------------------------------------ line spectrum strips

    fn strips_card(&mut self, ui: &mut Ui) {
        use spectra::StripCard as C;
        let strips = spectra::strip_cards(&self.params);
        let n = strips.len();
        let (lo, hi) = spectra::strip_axis(&self.params);
        let elements = spectra::strip_elements();
        let mut edit: Option<(Vec<C>, bool)> = None;
        for (i, st) in strips.iter().enumerate() {
            let kind = match st { C::Element(_) => "Element", C::Own { .. } => "Your own lines", C::Mixture { .. } => "Mixture" };
            let name = if st.name().is_empty() { format!("strip {}", i + 1) } else { st.name().to_string() };
            item_frame(ui, |ui| {
                header(ui, &format!("{}. {kind}", i + 1), |ui| {
                    if icon(ui, egui_phosphor::regular::X, n > 1, &format!("Remove {name}")) {
                        let mut v = strips.clone();
                        v.remove(i);
                        edit = Some((v, true));
                    }
                    if icon(ui, egui_phosphor::regular::ARROW_DOWN, i + 1 < n, &format!("Move {name} down")) {
                        let mut v = strips.clone();
                        v.swap(i, i + 1);
                        edit = Some((v, true));
                    }
                    if icon(ui, egui_phosphor::regular::ARROW_UP, i > 0, &format!("Move {name} up")) {
                        let mut v = strips.clone();
                        v.swap(i, i - 1);
                        edit = Some((v, true));
                    }
                });
                match st {
                    C::Element(el) => {
                        ui.horizontal(|ui| {
                            let shown = elements.iter().find(|(_, nm)| nm == el).map(|(s, nm)| format!("{s} – {nm}")).unwrap_or_else(|| el.clone());
                            let mut pick = None;
                            ComboBox::from_id_salt(("strip_el", i)).selected_text(shown).width(150.0).show_ui(ui, |ui| {
                                for (s, nm) in &elements {
                                    if ui.selectable_label(nm == el, format!("{s} – {nm}")).clicked() {
                                        pick = Some(nm.to_string());
                                    }
                                }
                            });
                            if let Some(nm) = pick.filter(|nm| nm != el) {
                                let mut v = strips.clone();
                                v[i] = C::Element(nm);
                                edit = Some((v, true));
                            }
                            if ui.button(RichText::new("Edit lines").size(12.0)).on_hover_text("Copy this element’s lines into your own, to change them").clicked() {
                                let mut v = strips.clone();
                                v[i] = C::Own { name: el.clone(), lines: spectra::element_lines_text(el) };
                                edit = Some((v, true));
                            }
                        });
                    }
                    C::Own { name: nm, lines } => {
                        small(ui, "Name");
                        if let Some(t) = buffered_edit(ui, ui.make_persistent_id(("strip_name", i)), nm, 200.0, "e.g. Element X") {
                            let mut v = strips.clone();
                            // mixtures made of this strip follow the new name
                            for s in v.iter_mut() {
                                if let C::Mixture { parts, .. } = s {
                                    for x in parts.iter_mut().filter(|x| x.eq_ignore_ascii_case(nm)) {
                                        *x = t.trim().to_string();
                                    }
                                }
                            }
                            v[i] = C::Own { name: t, lines: lines.clone() };
                            edit = Some((v, false));
                        }
                        small(ui, "Wavelengths in nm, separated by commas");
                        if let Some(t) = buffered_edit(ui, ui.make_persistent_id(("strip_lines", i)), lines, 200.0, "e.g. 450, 520.5, 610") {
                            let mut v = strips.clone();
                            v[i] = C::Own { name: nm.clone(), lines: t };
                            edit = Some((v, false));
                        }
                    }
                    C::Mixture { name: nm, parts } => {
                        small(ui, "Name");
                        if let Some(t) = buffered_edit(ui, ui.make_persistent_id(("strip_name", i)), nm, 200.0, "e.g. Unknown") {
                            let mut v = strips.clone();
                            v[i] = C::Mixture { name: t, parts: parts.clone() };
                            edit = Some((v, false));
                        }
                        small(ui, "Made of");
                        ui.horizontal_wrapped(|ui| {
                            for (j, other) in strips.iter().enumerate() {
                                if j == i || matches!(other, C::Mixture { .. }) || other.name().trim().is_empty() {
                                    continue;
                                }
                                // your own lines are looked up among the strips above the mixture
                                let usable = matches!(other, C::Element(_)) || j < i;
                                let mut on = parts.iter().any(|x| x.eq_ignore_ascii_case(other.name()));
                                let r = ui.add_enabled(usable, egui::Checkbox::new(&mut on, other.name()));
                                let r = r.on_disabled_hover_text("Move it above the mixture to include it");
                                if r.changed() {
                                    let mut np: Vec<String> = parts.iter().filter(|x| !x.eq_ignore_ascii_case(other.name())).cloned().collect();
                                    if on {
                                        np.push(other.name().to_string());
                                    }
                                    let mut v = strips.clone();
                                    v[i] = C::Mixture { name: nm.clone(), parts: np };
                                    edit = Some((v, true));
                                }
                            }
                        });
                    }
                }
                let out = spectra::strip_outside(&self.params, i);
                if out > 0 {
                    let what = if out == 1 { "1 line is".to_string() } else { format!("{out} lines are") };
                    ui.add(egui::Label::new(theme::muted(format!("{what} outside {}–{} nm, so not drawn. Widen the axis to show it.", fmt_measure(lo), fmt_measure(hi))).size(11.0)).wrap());
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            let used: Vec<&str> = strips.iter().map(|s| s.name()).collect();
            if ui.add(Button::new(RichText::new("+ Element").size(12.0)).corner_radius(CornerRadius::same(10))).clicked() {
                let next = elements.iter().map(|(_, nm)| *nm).find(|nm| !used.contains(nm)).unwrap_or("Hydrogen");
                let mut v = strips.clone();
                v.push(C::Element(next.to_string()));
                edit = Some((v, true));
            }
            if ui.add(Button::new(RichText::new("+ Your own lines").size(12.0)).corner_radius(CornerRadius::same(10))).clicked() {
                let mut v = strips.clone();
                v.push(C::Own { name: String::new(), lines: String::new() });
                edit = Some((v, true));
            }
            if ui.add(Button::new(RichText::new("+ Mixture").size(12.0)).corner_radius(CornerRadius::same(10))).clicked() {
                let mut v = strips.clone();
                v.push(C::Mixture { name: "Unknown".into(), parts: Vec::new() });
                edit = Some((v, true));
            }
        });
        ui.add(egui::Label::new(theme::muted("Elements show a few of their strongest visible lines, not every one. A mixture shows every line of the strips in it.").size(11.0)).wrap());
        if let Some((v, step)) = edit {
            let mut q = self.params.clone();
            spectra::set_strip_cards(&mut q, &v);
            self.card_commit(q, step);
        }
    }

    // ------------------------------------------------------------ particle kinds

    /// Kinds with a card: those that draw something, plus any added or emptied this session.
    fn shown_kinds(&self) -> Vec<usize> {
        let v: Vec<usize> = (0..4).filter(|&i| atoms::kind_used(&self.params, i) || self.kinds_keep[i]).collect();
        if v.is_empty() { vec![0] } else { v }
    }

    fn particles_card(&mut self, ui: &mut Ui, pdefs: &[ParamDef]) {
        let shown = self.shown_kinds();
        for &i in &shown {
            self.kinds_keep[i] = true;
        }
        let before_after = self.params.s("boxes") == "Before and after";
        let mut remove = None;
        for (pos, &i) in shown.iter().enumerate() {
            let k = atoms::PARTICLE_KINDS[i];
            let key = |f: &str| format!("{k}_{f}");
            let title = format!("{} {}", atoms::kind_noun(&self.params, i), pos + 1);
            item_frame(ui, |ui| {
                header(ui, &title, |ui| {
                    if icon(ui, egui_phosphor::regular::X, shown.len() > 1, &format!("Remove {title}")) {
                        remove = Some(i);
                    }
                });
                ui.horizontal(|ui| {
                    let counts: Vec<(&str, String)> = if before_after { vec![("Before", key("count")), ("After", format!("{k}2_count"))] } else { vec![("How many", key("count"))] };
                    for (lab, ck) in counts {
                        ui.label(theme::muted(lab).size(11.0));
                        let mut c = self.params.i(&ck);
                        if ui.add(egui::DragValue::new(&mut c).range(0..=24)).changed() {
                            self.params.set_i(&ck, c);
                        }
                    }
                });
                small(ui, "Shape");
                if let Some(v) = seg(ui, &atoms::PARTICLE_SHAPES, self.params.s(&key("shape"))) {
                    self.params.set_s(&key("shape"), &v);
                }
                let molecule = self.params.s(&key("shape")) != "Alone";
                let groups: Vec<(&str, [&str; 3])> = if molecule {
                    vec![("Center", ["size", "shade", "charge"]), ("Outer", ["osize", "oshade", "ocharge"])]
                } else {
                    vec![("", ["size", "shade", "charge"])]
                };
                for (g, fields) in groups {
                    if !g.is_empty() {
                        ui.label(theme::strong(g).size(12.0));
                    }
                    for (f, opts, lab) in [(fields[0], &atoms::PARTICLE_SIZES[..], "Size"), (fields[1], &atoms::PARTICLE_SHADES[..], "Shade"), (fields[2], &atoms::PARTICLE_CHARGES[..], "Charge")] {
                        small(ui, lab);
                        if let Some(v) = seg(ui, opts, self.params.s(&key(f))) {
                            self.params.set_s(&key(f), &v);
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(theme::muted(if molecule { "Key names" } else { "Key name" }).size(11.0));
                    let mut t = self.params.s(&key("name")).to_string();
                    if ui.add(TextEdit::singleline(&mut t).desired_width(80.0).hint_text(if molecule { "center" } else { "e.g. Na⁺ ion" })).changed() {
                        self.params.set_s(&key("name"), &t);
                    }
                    if molecule {
                        let mut t = self.params.s(&key("oname")).to_string();
                        if ui.add(TextEdit::singleline(&mut t).desired_width(80.0).hint_text("outer")).changed() {
                            self.params.set_s(&key("oname"), &t);
                        }
                    }
                });
            });
        }
        if let Some(i) = remove {
            let mut q = self.params.clone();
            atoms::remove_kind(&mut q, pdefs, i);
            self.kinds_keep.copy_within(i + 1.., i);
            self.kinds_keep[3] = false;
            self.card_commit(q, true);
        }
        ui.horizontal(|ui| {
            let free = (0..4).find(|&i| !shown.contains(&i));
            if ui.add_enabled(free.is_some(), Button::new(RichText::new("+ Add a kind").size(12.0)).corner_radius(CornerRadius::same(10))).on_hover_text("Up to four kinds").clicked()
                && let Some(i) = free
            {
                let mut q = self.params.clone();
                atoms::reset_kind(&mut q, i);
                self.kinds_keep[i] = true;
                self.card_commit(q, true);
            }
            if ui.add(Button::new(RichText::new("Shuffle").size(12.0)).corner_radius(CornerRadius::same(10))).on_hover_text("Scatter the particles again").clicked() {
                let mut q = self.params.clone();
                q.set_i("shuffle", (random_seed() % 999) as i64 + 1);
                self.card_commit(q, true);
            }
        });
    }
}
