//! The pedigree's Family card and person editor, as on the reference: Classic setups,
//! Inheritance, Generations and Family size, "New pedigree", then either the "Change someone"
//! picker or the selected person's panel (sex, status, deceased, proband, partners related, twin
//! of the next sibling, + Child / + Partner / + Sibling / Remove). People are selected by clicking
//! them on the preview (highlighted there) or from the picker. Every edit is one undo step and is
//! stored in the figure's `family` param, so exports and share links carry it.

use super::*;
use crate::figures::biology::pedigree as ped;

/// What the selection does after an edit.
enum After {
    Keep,
    Select(usize),
    Clear,
}

/// One row of buttons where exactly one is chosen; disabled options grey out. Returns a new pick.
fn seg_pick(ui: &mut Ui, opts: &[(&str, bool)], cur: usize) -> Option<usize> {
    let mut out = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
        for (i, (o, on)) in opts.iter().enumerate() {
            let sel = i == cur;
            let b = Button::new(RichText::new(*o).size(12.0).color(if sel { theme::accent_col() } else { theme::muted_col() }))
                .fill(if sel { theme::accent_faded() } else { egui::Color32::TRANSPARENT })
                .stroke(Stroke::new(1.0, if sel { theme::accent_col() } else { theme::border() }))
                .corner_radius(CornerRadius::same(8));
            if ui.add_enabled(*on, b).clicked() && !sel {
                out = Some(i);
            }
        }
    });
    out
}

fn label_of(labels: &[String], i: usize) -> String {
    labels.get(i).cloned().unwrap_or_else(|| format!("#{}", i + 1))
}

impl Workspace {
    /// Applies an edit to the family (folding any typed overrides into it first) as one undo step.
    fn ped_apply(&mut self, f: impl FnOnce(&mut ped::Family) -> Option<After>) {
        let mut fam = ped::family_of(&self.params);
        let Some(after) = f(&mut fam) else { return };
        let mut p = self.params.clone();
        ped::store(&mut p, &fam);
        self.set_params_with_history(p);
        match after {
            After::Keep => {}
            After::Select(i) => self.ped_sel = Some(i),
            After::Clear => self.ped_sel = None,
        }
    }

    /// One person-panel action on the selected person (the panel's buttons and the
    /// `VERSHION_PED_DO` test hook share it): child, partner, sibling, remove, male, female,
    /// unknown, unaffected, carrier, affected, deceased / alive, proband / not_proband, related /
    /// unrelated, twin_no, twin_fraternal, twin_identical.
    pub(super) fn ped_action(&mut self, op: &str) {
        let Some(i) = self.ped_sel else { return };
        let person = |f: &mut ped::Family, g: &dyn Fn(&mut ped::Member) -> bool| -> Option<After> {
            let q = f.person_mut(i)?;
            g(q).then_some(After::Keep)
        };
        match op {
            "child" => self.ped_apply(|f| f.add_child(i).map(After::Select)),
            "partner" => self.ped_apply(|f| f.add_partner(i).map(After::Select)),
            "sibling" => self.ped_apply(|f| f.add_sibling(i).map(After::Select)),
            "remove" => self.ped_apply(|f| f.remove(i).then_some(After::Clear)),
            "related" | "unrelated" => self.ped_apply(|f| (f.related(i) != (op == "related") && f.set_related(i, op == "related")).then_some(After::Keep)),
            "male" | "female" | "unknown" => {
                let sex = match op { "male" => 0, "female" => 1, _ => 2 };
                self.ped_apply(|f| {
                    if sex == 2 && !f.caps(i)?.unknown {
                        return None;
                    }
                    person(f, &|q| std::mem::replace(&mut q.sex, sex) != sex)
                })
            }
            "unaffected" | "carrier" | "affected" => {
                let st = match op { "unaffected" => 0, "carrier" => 1, _ => 2 };
                self.ped_apply(|f| person(f, &|q| std::mem::replace(&mut q.status, st) != st))
            }
            "deceased" | "alive" => self.ped_apply(|f| person(f, &|q| std::mem::replace(&mut q.dead, op == "deceased") != (op == "deceased"))),
            "proband" | "not_proband" => self.ped_apply(|f| person(f, &|q| std::mem::replace(&mut q.proband, op == "proband") != (op == "proband"))),
            "twin_no" | "twin_fraternal" | "twin_identical" => {
                let t = match op { "twin_fraternal" => 1, "twin_identical" => 2, _ => 0 };
                self.ped_apply(|f| {
                    if t > 0 && !f.caps(i)?.twin {
                        return None;
                    }
                    person(f, &|q| std::mem::replace(&mut q.twin, t) != t)
                })
            }
            _ => {}
        }
    }

    /// Test hooks: `VERSHION_PED_SEL=III-2` selects that person; `VERSHION_PED_DO=child;partner`
    /// then runs panel actions in order (each new person becomes the selection, as in the UI).
    pub(super) fn ped_qa_hooks(&mut self) {
        if !self.sel.is_some_and(|si| reg()[si].id == "pedigree") {
            return;
        }
        if let Ok(l) = crate::env_var("VERSHION_PED_SEL") {
            self.ped_sel = ped::labels(&self.params).iter().position(|x| x.eq_ignore_ascii_case(l.trim()));
        }
        if let Ok(ops) = crate::env_var("VERSHION_PED_DO") {
            for op in ops.split([';', ',']).map(str::trim).filter(|o| !o.is_empty()) {
                if let Some(who) = op.strip_prefix("sel=") {
                    self.ped_sel = ped::labels(&self.params).iter().position(|x| x.eq_ignore_ascii_case(who));
                } else {
                    self.ped_action(op);
                }
            }
            self.reset_history();
        }
    }

    /// The Family card (replaces the generic sections for the family setup).
    pub(super) fn pedigree_card(&mut self, ui: &mut Ui) {
        let Some(si) = self.sel else { return };
        let def = &reg()[si];
        let pdefs = (def.params)();
        let fam = ped::family_of(&self.params);
        if self.ped_sel.is_some_and(|i| i >= fam.len()) {
            self.ped_sel = None;
        }
        let hand = ped::hand_changed(&self.params);
        let labels = ped::labels(&self.params);
        let family_before = self.params.s("family").to_string();
        Frame::new()
            .fill(theme::card())
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                crate::kit::small_caps(ui, "Family");
                let sub = if hand {
                    format!("Changed by hand, {}", ped::mode_name(&self.params))
                } else {
                    format!("{}, {} generations", self.params.s("inherit"), self.params.i("gens"))
                };
                ui.label(theme::muted(sub).size(11.0));
                ui.add_space(6.0);
                self.classic_combo(ui, def.id);
                if self.params.s("family") != family_before {
                    self.ped_sel = None;
                }

                // Inheritance: a family changed by hand is shaded again to fit
                ui.label(theme::muted("Inheritance").size(11.0));
                let modes: Vec<&'static str> = pdefs.iter().find(|d| d.key == "inherit").and_then(|d| if let Kind::Choice(o) = &d.kind { Some(o.clone()) } else { None }).unwrap_or_default();
                let cur = self.params.s("inherit").to_string();
                let mut pick: Option<&'static str> = None;
                ComboBox::from_id_salt("ped_inherit").selected_text(cur.as_str()).width(ui.available_width().min(260.0)).show_ui(ui, |ui| {
                    for m in &modes {
                        if ui.selectable_label(cur == *m, *m).clicked() && cur != *m {
                            pick = Some(m);
                        }
                    }
                });
                if let Some(m) = pick {
                    let mut p = self.params.clone();
                    p.set_s("inherit", m);
                    ped::reshade_params(&mut p);
                    self.set_params_with_history(p);
                }
                ui.add_space(6.0);

                // Generations and family size start a random family again
                let mut setup: Option<(&str, Value)> = None;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(theme::muted("Generations").size(11.0));
                        let g = self.params.i("gens").clamp(2, 4) as usize;
                        if let Some(k) = seg_pick(ui, &[("2", true), ("3", true), ("4", true)], g - 2) {
                            setup = Some(("gens", Value::I(k as i64 + 2)));
                        }
                    });
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.label(theme::muted("Family size").size(11.0));
                        let sizes = ["Small", "Medium", "Large"];
                        let c = sizes.iter().position(|s| *s == self.params.s("size")).unwrap_or(1);
                        if let Some(k) = seg_pick(ui, &[("Small", true), ("Medium", true), ("Large", true)], c) {
                            setup = Some(("size", Value::S(sizes[k].to_string())));
                        }
                    });
                });
                if let Some((k, v)) = setup {
                    let mut p = self.params.clone();
                    p.0.insert(k.to_string(), v);
                    ped::drop_hand_changes(&mut p);
                    self.set_params_with_history(p);
                    self.ped_sel = None;
                }
                ui.add_space(6.0);
                let nb = Button::new(RichText::new("New pedigree").size(13.0).color(egui::Color32::WHITE)).fill(theme::accent_col()).corner_radius(CornerRadius::same(8));
                if ui.add_sized(Vec2::new(ui.available_width(), 28.0), nb).on_hover_text("Draw a new random family (drops changes made by hand)").clicked() {
                    let mut p = self.params.clone();
                    ped::new_pedigree(&mut p, (random_seed() % 9999) as i64 + 1);
                    self.set_params_with_history(p);
                    self.ped_sel = None;
                }
                if hand {
                    ui.add_space(4.0);
                    ui.label(theme::muted("Changed by hand. New pedigree, or a change of generations or size, starts a random family; changing the inheritance shades this one again to fit it.").size(11.0));
                }
                ui.add_space(8.0);
                match self.ped_sel {
                    Some(i) => self.person_panel(ui, i, &fam, &labels),
                    None => self.change_someone(ui, &labels),
                }
                ui.add_space(4.0);
                egui::CollapsingHeader::new(RichText::new("Advanced").size(12.0)).id_salt("ped_advanced").default_open(false).show(ui, |ui| {
                    for key in ["seed", "changes"] {
                        if let Some(pd) = pdefs.iter().find(|d| d.key == key) {
                            param_control(ui, pd, &mut self.params);
                            ui.add_space(4.0);
                        }
                    }
                    ui.label(theme::muted("Typed overrides are folded into the family the next time you change someone.").size(11.0));
                });
            });
        ui.add_space(8.0);
    }

    /// "Change someone": pick a person without clicking the figure.
    fn change_someone(&mut self, ui: &mut Ui, labels: &[String]) {
        ui.label(theme::muted("Change someone").size(11.0));
        let (_, _, whos) = ped::people_of(&self.params);
        let mut order: Vec<usize> = (0..whos.len()).collect();
        order.sort_by_key(|&i| (whos[i].gi, whos[i].num));
        let mut pick = None;
        ComboBox::from_id_salt("ped_change_someone").selected_text("Click them in the pedigree, or pick here").width(ui.available_width().min(260.0)).show_ui(ui, |ui| {
            for i in order {
                if ui.selectable_label(false, label_of(labels, i)).clicked() {
                    pick = Some(i);
                }
            }
        });
        if pick.is_some() {
            self.ped_sel = pick;
        }
    }

    /// The selected person's panel.
    fn person_panel(&mut self, ui: &mut Ui, i: usize, fam: &ped::Family, labels: &[String]) {
        let (Some(caps), Some(who)) = (fam.caps(i), fam.person(i).cloned()) else {
            self.ped_sel = None;
            return;
        };
        let name = label_of(labels, i);
        let mut op: Option<&'static str> = None;
        Frame::new()
            .fill(theme::accent_faded().gamma_multiply(0.35))
            .stroke(Stroke::new(1.0, theme::accent_col().gamma_multiply(0.5)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(theme::serif_strong(name.as_str()).size(15.0));
                    match caps.role {
                        ped::Role::Founder => {
                            ui.label(theme::muted("founder").size(11.0));
                        }
                        ped::Role::MarriedIn => {
                            ui.label(theme::muted("married in").size(11.0));
                        }
                        ped::Role::Blood => {}
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(Button::new(RichText::new(egui_phosphor::regular::X).size(15.0)).frame(false)).on_hover_text(format!("Done changing {name}")).clicked() {
                            op = Some("done");
                        }
                    });
                });
                ui.add_space(2.0);
                if let Some(k) = seg_pick(ui, &[("Male", true), ("Female", true), ("Unknown", caps.unknown)], who.sex.min(2) as usize) {
                    op = Some(["male", "female", "unknown"][k]);
                }
                ui.add_space(2.0);
                if let Some(k) = seg_pick(ui, &[("Unaffected", true), ("Carrier", true), ("Affected", true)], who.status.min(2) as usize) {
                    op = Some(["unaffected", "carrier", "affected"][k]);
                }
                if who.status == 1 && !ped::carriers_visible(&self.params) {
                    ui.label(theme::muted("Carriers are drawn only when Symbols shows them.").size(11.0));
                }
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    let mut dead = who.dead;
                    if ui.checkbox(&mut dead, "Deceased").changed() {
                        op = Some(if dead { "deceased" } else { "alive" });
                    }
                    let mut pb = who.proband;
                    if ui.checkbox(&mut pb, "Proband").changed() {
                        op = Some(if pb { "proband" } else { "not_proband" });
                    }
                });
                if caps.has_mate {
                    let mut rel = fam.related(i);
                    if ui.checkbox(&mut rel, "Partners are related").on_hover_text("Consanguinity: a double line between the partners").changed() {
                        op = Some(if rel { "related" } else { "unrelated" });
                    }
                }
                if caps.twin {
                    ui.add_space(2.0);
                    ui.label(theme::strong("Twin of the next sibling").size(11.5));
                    if let Some(k) = seg_pick(ui, &[("No", true), ("Fraternal", true), ("Identical", true)], who.twin.min(2) as usize) {
                        op = Some(["twin_no", "twin_fraternal", "twin_identical"][k]);
                    }
                }
                ui.add_space(4.0);
                let bw = (ui.available_width() - 6.0) / 2.0;
                ui.horizontal(|ui| {
                    if ui.add_enabled(caps.child, Button::new(RichText::new("+ Child").size(12.0)).min_size(Vec2::new(bw, 24.0))).on_hover_text("Add a child to this couple (someone single gets a partner first)").clicked() {
                        op = Some("child");
                    }
                    if ui.add_enabled(caps.partner, Button::new(RichText::new("+ Partner").size(12.0)).min_size(Vec2::new(bw, 24.0))).on_hover_text("Add a partner who marries in").clicked() {
                        op = Some("partner");
                    }
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(caps.sibling, Button::new(RichText::new("+ Sibling").size(12.0)).min_size(Vec2::new(bw, 24.0))).on_hover_text("Add a brother or sister right after").clicked() {
                        op = Some("sibling");
                    }
                    let rm = Button::new(RichText::new("Remove").size(12.0).color(if caps.remove { theme::red() } else { theme::muted_col() })).min_size(Vec2::new(bw, 24.0));
                    if ui.add_enabled(caps.remove, rm).clicked() {
                        op = Some("remove");
                    }
                });
                let help = match caps.role {
                    ped::Role::Founder => "The founder stays; change or remove anyone else.",
                    ped::Role::MarriedIn => "Removing a partner removes their children too.",
                    ped::Role::Blood if caps.has_mate || caps.has_kids => "Removing someone removes their partner and children too.",
                    ped::Role::Blood => "",
                };
                if !help.is_empty() {
                    ui.add_space(2.0);
                    ui.label(theme::muted(help).size(11.0));
                }
            });
        match op {
            Some("done") => self.ped_sel = None,
            Some(o) => self.ped_action(o),
            None => {}
        }
    }

    /// Canvas layer: click a person to select them; the selection (and the person under the
    /// pointer) is highlighted. Drawn over the preview only, never into exports.
    pub(super) fn pedigree_canvas(&mut self, ui: &mut Ui, img: egui::Rect, k: f64, si: usize) {
        let (_, _, whos) = ped::people_of(&self.params);
        let labels = ped::labels(&self.params);
        if self.ped_sel.is_some_and(|i| i >= whos.len()) {
            self.ped_sel = None;
        }
        let painter = ui.painter_at(img.expand(8.0));
        let acc = theme::accent_col();
        for (i, w) in whos.iter().enumerate() {
            let c = egui::pos2(img.left() + (w.x * k) as f32, img.top() + (w.y * k) as f32);
            let half = ((w.r * if w.sex == 2 { 1.25 } else { 1.0 } + 4.0) * k).max(9.0) as f32;
            let rect = egui::Rect::from_center_size(c, Vec2::splat(half * 2.0));
            let r = ui.interact(rect, UiId::new(("ped_person", si, i)), Sense::click());
            let selected = self.ped_sel == Some(i);
            if selected {
                let wash = egui::Color32::from_rgba_unmultiplied(acc.r(), acc.g(), acc.b(), 34);
                painter.rect(rect, 6.0, wash, Stroke::new(2.0, acc), egui::StrokeKind::Outside);
            } else if r.hovered() {
                painter.rect(rect, 6.0, egui::Color32::TRANSPARENT, Stroke::new(1.5, acc.gamma_multiply(0.7)), egui::StrokeKind::Outside);
            }
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                self.ped_sel = Some(i);
            }
            r.on_hover_text(format!("Click to change {}", label_of(&labels, i)));
        }
    }
}
