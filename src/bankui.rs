//! Question Banks screens: the bank list and the bank editor (outline topics panel,
//! filtered/sorted question cards, toolbar with Import/Export/New question, inline
//! name edit), plus the shared `fmt_date` / kind-chip helpers. Owned by the bank
//! worker; the question editor modal lives in `qedit`. Screens draw their own
//! `CentralPanel` content pane; the shell router draws the sidebar around it.

use egui::{
    Align, Button, Color32, ComboBox, CornerRadius, Frame, Layout, Margin, RichText,
    Sense, Stroke, TextEdit, Ui, Vec2,
};
// egui's widget id type vs the store's bank/question id (a String).
use egui::Id as UiId;

use crate::model::{Difficulty, Id, QKind, Question};
use crate::screens::{accent_button, card_frame, now_secs};
use crate::{qedit, screens, theme, App, Screen};

const OUTLINE_W: f32 = 200.0;

// ---------- shared helpers (pub: other workers may use these) ----------

/// Formats a unix-seconds timestamp as a short date, "Oct 15, 2026".
/// Exported from this module; integration can move it into a util module without
/// breaking anything if it also happens to exist elsewhere.
pub fn fmt_date(epoch: i64) -> String {
    if epoch <= 0 {
        return "—".to_string();
    }
    let (y, m, d) = screens::civil_from_days(epoch.div_euclid(86_400));
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{} {}, {}", MONTHS[(m - 1) as usize], d, y)
}

/// Chip color for a question type.
pub fn kind_chip_color(kind: QKind) -> Color32 {
    match kind {
        QKind::Mc => Color32::from_rgb(0x35, 0x60, 0x7c),
        QKind::Tf => Color32::from_rgb(0x3e, 0x6b, 0x48),
        QKind::Matching => Color32::from_rgb(0x6b, 0x53, 0x79),
        QKind::Blank => Color32::from_rgb(0xb5, 0x8a, 0x3d),
        QKind::Short => Color32::from_rgb(0x7c, 0x5b, 0x3a),
        QKind::Multipart => Color32::from_rgb(0x8c, 0x44, 0x44),
    }
}

/// Short chip label for a question type.
pub fn kind_chip_label(kind: QKind) -> &'static str {
    match kind {
        QKind::Mc => "MC",
        QKind::Tf => "T/F",
        QKind::Matching => "Match",
        QKind::Blank => "Blank",
        QKind::Short => "Short",
        QKind::Multipart => "Multi",
    }
}

/// Loads (and caches in `App.textures`) the egui texture for a stored image.
/// Exported; the shell's `App::image_texture` does the actual load, and the
/// exam worker can reuse this wrapper for question images.
pub fn texture_for(
    app: &mut App,
    ctx: &egui::Context,
    img: &crate::model::ImageRef,
) -> Option<egui::TextureHandle> {
    app.image_texture(ctx, &img.file)
}

/// Chip color for a difficulty level.
fn diff_color(difficulty: Difficulty) -> Color32 {
    match difficulty {
        Difficulty::Low => theme::GREEN, // #3e6b48
        Difficulty::Medium => Color32::from_rgb(0xb5, 0x8a, 0x3d),
        Difficulty::High => theme::RED, // #a33b2e
    }
}

fn diff_label(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Low => "Low",
        Difficulty::Medium => "Medium",
        Difficulty::High => "High",
    }
}

/// Points on human form ("2 pt", "2.5 pt"); None when the question has none.
pub fn points_label(points: Option<f64>) -> Option<String> {
    points.map(|p| {
        if p.fract() == 0.0 {
            format!("{} pt", p as i64)
        } else {
            format!("{p} pt")
        }
    })
}

/// Reads/edits per-frame UI state kept in egui context memory (the App struct is
/// not mine to extend).
fn with_state<T, R>(ctx: &egui::Context, key: UiId, f: impl FnOnce(&mut T) -> R) -> R
where
    T: Default + Clone + Send + Sync + 'static,
{
    ctx.data_mut(|d| f(d.get_temp_mut_or_default::<T>(key)))
}

/// Card-ish secondary button.
fn ghost_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        Button::new(theme::text(label))
            .fill(theme::CARD)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(CornerRadius::same(6)),
    )
}

// ---------- transient per-screen state ----------

/// State of the Question Banks list screen.
#[derive(Clone, Default)]
struct BanksState {
    search: String,
    /// Bank id awaiting delete confirmation.
    confirm_delete: Option<Id>,
    /// Bank id being renamed + the current name text.
    rename: Option<(Id, String)>,
}

/// Sort choices for the question card list.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
enum Sort {
    #[default]
    Newest,
    Oldest,
    Kind,
    Difficulty,
    Topic,
}

impl Sort {
    fn all() -> [Sort; 5] {
        [
            Sort::Newest,
            Sort::Oldest,
            Sort::Kind,
            Sort::Difficulty,
            Sort::Topic,
        ]
    }
    fn label(self) -> &'static str {
        match self {
            Sort::Newest => "Newest",
            Sort::Oldest => "Oldest",
            Sort::Kind => "Question type",
            Sort::Difficulty => "Difficulty",
            Sort::Topic => "Topic",
        }
    }
}

/// State of the bank editor screen.
#[derive(Clone, Default)]
struct BankEditState {
    search: String,
    /// Topic the card list is filtered to (None = all).
    topic: Option<String>,
    /// Difficulty filter (None = all).
    diff: Option<Difficulty>,
    sort: Sort,
    /// Question id awaiting delete confirmation.
    confirm_delete: Option<Id>,
}

/// Menu actions shared by the two card types.
#[derive(Clone, Copy, PartialEq)]
enum CardMenu {
    Item,
    Delete,
}

// ---------- screen 1: the Question Banks list ----------

impl App {
    pub fn draw_banks(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let key = UiId::new("testparrot_banks_state");
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(theme::CREAM)
                    .inner_margin(Margin::same(24)),
            )
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Heading + right-aligned actions.
                ui.horizontal(|ui| {
                    ui.label(theme::serif_strong("Question Banks").size(24.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if accent_button(ui, "＋ New bank").clicked() {
                            let id = self.new_bank("Untitled Question Bank");
                            self.open_bank(&id);
                        }
                        ui.add_space(8.0);
                        if ui
                            .add(
                                Button::new(theme::text("Import a test"))
                                    .fill(theme::CARD)
                                    .corner_radius(CornerRadius::same(8)),
                            )
                            .clicked()
                        {
                            self.import_open = true;
                        }
                    });
                });

                ui.add_space(10.0);

                // Search.
                let mut search = with_state(&ctx, key, |s: &mut BanksState| s.search.clone());
                let resp = ui.add(
                    TextEdit::singleline(&mut search)
                        .hint_text("Search Question Banks")
                        .desired_width(ui.available_width()),
                );
                if resp.changed() {
                    with_state(&ctx, key, |s: &mut BanksState| s.search = search.clone());
                }

                // Count line.
                let query = search.trim().to_lowercase();
                let count = if query.is_empty() {
                    self.store.banks.len()
                } else {
                    self.store
                        .banks
                        .iter()
                        .filter(|b| b.name.to_lowercase().contains(&query))
                        .count()
                };
                ui.label(theme::muted(bank_count_line(count)).size(11.5));
                ui.add_space(8.0);

                // Card grid.
                if self.store.banks.is_empty() {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        ui.label(theme::serif_strong("Nothing here yet").size(18.0));
                        ui.add_space(4.0);
                        ui.label(
                            theme::muted(
                                "Create your first Question Bank, or hand a test you already \
                                 have to the importer.",
                            )
                            .size(12.5),
                        );
                    });
                } else {
                    let cards: Vec<(Id, String, usize, i64)> = self
                        .store
                        .banks
                        .iter()
                        .filter(|b| {
                            query.is_empty() || b.name.to_lowercase().contains(&query)
                        })
                        .map(|b| (b.id.clone(), b.name.clone(), b.questions.len(), b.modified))
                        .collect();
                    if cards.is_empty() {
                        ui.label(theme::muted(format!(
                            "No banks match \"{search}\"."
                        )));
                    }
                    egui::Grid::new("banks_grid")
                        .num_columns(3)
                        .spacing([16.0, 16.0])
                        .show(ui, |ui| {
                            for chunk in cards.chunks(3) {
                                for card in chunk {
                                    bank_card(ui, self, &ctx, key, card);
                                }
                                for _ in 0..(3 - chunk.len()) {
                                    ui.label("");
                                }
                                ui.end_row();
                            }
                        });
                }
            });

        // List-screen overlays: rename + delete confirmation.
        draw_bank_overlays(self, &ctx, key);
    }

    /// The bank editor: breadcrumb, inline serif name edit, toolbar, outline topics
    /// panel on the left, filtered question cards on the right; the `qedit` modal
    /// opens on top when set.
    pub fn draw_bank_edit(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let Some(bank_id) = self.bank.clone().or_else(|| match &self.screen {
            Screen::BankEdit(id) => Some(id.clone()),
            _ => None,
        }) else {
            return;
        };
        if self.store.bank(&bank_id).is_none() {
            self.screen = Screen::Banks;
            self.bank = None;
            self.qedit = None;
            return;
        }

        let key = UiId::new("testparrot_bank_edit_state").with(&bank_id);
        let bank_name = self
            .store
            .bank(&bank_id)
            .map(|b| b.name.clone())
            .unwrap_or_default();
        let questions: Vec<Question> = self
            .store
            .bank(&bank_id)
            .map(|b| b.questions.clone())
            .unwrap_or_default();

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(theme::CREAM)
                    .inner_margin(Margin::same(24)),
            )
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Breadcrumb row; storage info at the right.
                ui.horizontal(|ui| {
                    if ui
                        .add(Button::new(RichText::new("Home").size(13.0)).frame(false))
                        .clicked()
                    {
                        self.screen = Screen::Home;
                    }
                    ui.label(theme::muted("/").size(12.0));
                    if ui
                        .add(Button::new(theme::muted("Question Banks").size(13.0)).frame(false))
                        .clicked()
                    {
                        self.screen = Screen::Banks;
                    }
                    ui.label(theme::muted("/").size(12.0));
                    ui.label(theme::muted(bank_name.clone()).size(13.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(Button::new(RichText::new("ⓘ").size(11.0)).frame(false))
                            .clicked()
                        {
                            with_state(&ctx, key.with("storage_info"), |b: &mut bool| {
                                *b = true;
                            });
                        }
                        ui.label(theme::muted("Changes save immediately").size(11.5));
                    });
                });
                ui.add_space(6.0);

                // Inline name (house serif, 18) + right toolbar.
                ui.horizontal(|ui| {
                    let mut name_buf = bank_name.clone();
                    let resp = ui.add(
                        TextEdit::singleline(&mut name_buf)
                            .font(egui::FontId::new(18.0, theme::serif_family()))
                            .text_color(theme::INK)
                            .frame(egui::Frame::new().fill(theme::CREAM))
                            .desired_width(360.0),
                    );
                    let changed_now = resp.changed() && name_buf.trim() != bank_name
                        && !name_buf.trim().is_empty();
                    if changed_now {
                        let t = now_secs();
                        if let Some(b) = self.store.bank_mut(&bank_id) {
                            b.name = name_buf.trim().to_string();
                            b.modified = t;
                        }
                        self.store.save_banks();
                    }
                    ui.label(""); // reserves the gap; toolbar hugs the right edge
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if accent_button(ui, "＋ New question").clicked() {
                            self.qedit = Some(qedit::Draft::new(bank_id.clone()));
                        }
                        ui.add_space(8.0);
                        if ui
                            .add(
                                Button::new(theme::text("Import"))
                                    .fill(theme::CARD)
                                    .corner_radius(CornerRadius::same(8)),
                            )
                            .clicked()
                        {
                            self.import_open = true;
                        }
                        ui.menu_button(RichText::new("⋯").size(14.0), |ui| {
                            if ui.button("Export Question Bank").clicked() {
                                export_bank(self, &bank_id);
                                ui.close();
                            }
                            if ui.button("Import questions").clicked() {
                                self.import_open = true;
                                ui.close();
                            }
                            if ui.button("New question").clicked() {
                                self.qedit = Some(qedit::Draft::new(bank_id.clone()));
                                ui.close();
                            }
                        });
                    });
                });

                ui.add_space(10.0);

                // Outline panel (fixed width) + question card list.
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(OUTLINE_W);
                        ui.set_max_width(OUTLINE_W);
                        draw_outline(ui, &ctx, key, &questions);
                    });
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        draw_question_cards(ui, self, &ctx, key, &bank_id, &questions);
                    });
                });
            });

        // Editor-screen overlays: storage info, question delete confirmation.
        let info_open = with_state(&ctx, key.with("storage_info"), |b: &mut bool| *b);
        if info_open {
            self.storage_info_modal(&ctx, key.with("storage_info"));
        }
        draw_question_delete_confirm(self, &ctx, key);

        // The question editor modal on top.
        if self.qedit.is_some() {
            self.draw_question_editor(ui);
        }
    }
}

// ---------- banks list pieces ----------

fn bank_count_line(count: usize) -> String {
    match count {
        0 => "No question banks yet".to_string(),
        1 => "1 Question Bank".to_string(),
        n => format!("{n} Question Banks"),
    }
}

fn question_count_line(n: usize) -> String {
    format!("{n} question{}", if n == 1 { "" } else { "s" })
}

/// One bank in the grid: name, counts, last-modified, Open, and a ⋯ menu with
/// Rename / Delete (delete asks for confirmation in a modal).
fn bank_card(
    ui: &mut Ui,
    app: &mut App,
    ctx: &egui::Context,
    key: UiId,
    card: &(Id, String, usize, i64),
) {
    let (id, name, nq, modified) = (
        card.0.clone(),
        card.1.clone(),
        card.2,
        card.3,
    );
    let mut action: Option<CardMenu> = None;
    let resp = card_frame()
        .inner_margin(Margin::same(12))
        .outer_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::strong(name.clone()).size(14.0));
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    ui.menu_button(RichText::new("⋯").size(13.0), |ui| {
                        if ui.button("Rename").clicked() {
                            action = Some(CardMenu::Item);
                            ui.close();
                        }
                        if ui.button("Delete…").clicked() {
                            action = Some(CardMenu::Delete);
                            ui.close();
                        }
                    });
                });
            });
            ui.add_space(2.0);
            ui.label(theme::muted(question_count_line(nq)).size(11.5));
            ui.label(
                theme::muted(format!("Modified {}", fmt_date(modified))).size(11.0),
            );
            ui.add_space(6.0);
            if ghost_button(ui, "Open").clicked() {
                app.open_bank(&id);
            }
        });

    if let Some(a) = action {
        with_state(ctx, key, |s: &mut BanksState| match a {
            CardMenu::Item => s.rename = Some((id.clone(), name.clone())),
            CardMenu::Delete => s.confirm_delete = Some(id.clone()),
        });
    }

    // The whole card is clickable too; inner buttons keep their own clicks.
    if resp.response.interact(Sense::click()).clicked() {
        app.open_bank(&id);
    }
}

/// Rename and delete-confirmation modals for the banks list.
fn draw_bank_overlays(app: &mut App, ctx: &egui::Context, key: UiId) {
    let st = with_state(ctx, key, |s: &mut BanksState| s.clone());

    // Rename.
    if let Some((id, ref rename_text)) = st.rename {
        let mut close = false;
        let mut commit = false;
        let mut commit_text = rename_text.clone();
        let modal = egui::Modal::new(UiId::new("testparrot_rename_bank")).show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.horizontal(|ui| {
                ui.label(theme::serif_strong("Rename bank").size(17.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(Button::new(RichText::new("✕").size(13.0)).frame(false)).clicked()
                    {
                        close = true;
                    }
                });
            });
            ui.add_space(4.0);
            let r = ui.add(TextEdit::singleline(&mut commit_text).desired_width(ui.available_width()));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                commit = true;
            }
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        Button::new(theme::on_accent("Rename"))
                            .fill(theme::ACCENT)
                            .corner_radius(CornerRadius::same(8)),
                    )
                    .clicked()
                {
                    commit = true;
                }
                if ui.add(Button::new(theme::text("Cancel"))).clicked() {
                    close = true;
                }
            });
        });
        if commit {
            let t = commit_text.trim().to_string();
            if !t.is_empty() {
                if let Some(b) = app.store.bank_mut(&id) {
                    b.name = t;
                    b.modified = now_secs();
                }
                app.store.save_banks();
            }
        }
        if commit || close || modal.should_close() {
            with_state(ctx, key, |s: &mut BanksState| s.rename = None);
        }
    }

    // Delete confirmation.
    if let Some(id) = st.confirm_delete.clone() {
        let bank = app.store.bank(&id).cloned();
        let mut close = false;
        let mut confirmed = false;
        let modal = egui::Modal::new(UiId::new("testparrot_delete_bank")).show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.label(theme::serif_strong("Delete Question Bank?").size(17.0));
            ui.add_space(4.0);
            if let Some(b) = &bank {
                ui.label(
                    theme::text(format!(
                        "This permanently deletes \"{}\" and its {} question{} — everything \
                         in it is removed from your machine, and exams stop being able to \
                         pull new copies of its questions.",
                        b.name,
                        b.questions.len(),
                        if b.questions.len() == 1 { "" } else { "s" },
                    ))
                    .size(12.5),
                );
            }
            ui.add_space(8.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        Button::new(theme::on_accent("Delete"))
                            .fill(theme::RED)
                            .corner_radius(CornerRadius::same(8)),
                    )
                    .clicked()
                {
                    confirmed = true;
                }
                if ui.add(Button::new(theme::text("Cancel"))).clicked() {
                    close = true;
                }
            });
        });
        if confirmed {
            app.store.banks.retain(|b| b.id != id);
            app.store.save_banks();
            app.toast("Question bank deleted");
            if app.bank.as_deref() == Some(id.as_str()) {
                app.screen = Screen::Banks;
                app.bank = None;
                app.qedit = None;
                app.panel_bank = None;
            }
        }
        if close || confirmed || modal.should_close() {
            with_state(ctx, key, |s: &mut BanksState| s.confirm_delete = None);
        }
    }
}

/// Serializes a bank as a Test Parrot JSON question-bank file and asks where to
/// save it (the user picks the location, so the write goes through `rfd` + fs).
pub fn export_bank(app: &mut App, bank_id: &Id) {
    let Some(bank) = app.store.bank(bank_id).cloned() else {
        app.toast_err("That bank is no longer there.");
        return;
    };
    let path = rfd::FileDialog::new()
        .set_file_name(format!("{}.json", bank.name))
        .add_filter("Question bank JSON", &["json"])
        .save_file();
    let Some(path) = path else { return };
    let doc = serde_json::json!({
        "kind": "test-parrot-question-bank",
        "version": 1,
        "name": bank.name,
        "questions": bank.questions,
    });
    let written = serde_json::to_string_pretty(&doc)
        .map_err(|e| e.to_string())
        .and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
    match written {
        Ok(()) => app.toast(format!("Exported to {}", path.display())),
        Err(e) => app.toast_err(format!("Export failed: {e}")),
    }
}

// ---------- bank editor pieces ----------

/// "Where your work is stored" info popup; clears its flag on Close/backdrop/Esc.
impl App {
    fn storage_info_modal(&mut self, ctx: &egui::Context, key2: UiId) {
        let mut close = false;
        let modal = egui::Modal::new(UiId::new("testparrot_storage_info")).show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.horizontal(|ui| {
                ui.label(theme::serif_strong("Where your work is stored").size(17.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(Button::new(RichText::new("✕").size(13.0)).frame(false)).clicked()
                    {
                        close = true;
                    }
                });
            });
            ui.add_space(6.0);
            ui.label(
                theme::text(
                    "Every bank, exam and export lives on your machine, in plain files you \
                     can back up:",
                )
                .size(12.5),
            );
            ui.add_space(6.0);
            egui::Frame::new()
                .fill(theme::CREAM)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.monospace(crate::store::store_dir().display().to_string());
                });
            ui.add_space(6.0);
            ui.label(
                theme::muted(
                    "banks.json and exams.json hold your data; media/ holds question images; \
                     exports/ holds every exam PDF and Word file ever produced.",
                )
                .size(11.5),
            );
            ui.add_space(8.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(Button::new(theme::text("Close"))).clicked() {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            with_state(ctx, key2, |b: &mut bool| *b = false);
        }
    }
}

fn draw_question_delete_confirm(app: &mut App, ctx: &egui::Context, key: UiId) {
    let Some(qid) = with_state(ctx, key, |s: &mut BankEditState| s.confirm_delete.clone()) else {
        return;
    };
    let bank_id = app
        .bank
        .clone()
        .or_else(|| match &app.screen {
            Screen::BankEdit(id) => Some(id.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let question = app.store.question(&bank_id, &qid).cloned();
    let mut close = false;
    let mut confirmed = false;
    let modal = egui::Modal::new(UiId::new("testparrot_delete_question").with(&qid)).show(ctx, |ui| {
        ui.set_min_width(360.0);
        ui.label(theme::serif_strong("Delete Question?").size(17.0));
        ui.add_space(4.0);
        if let Some(q) = &question {
            ui.label(
                theme::text(format!(
                    "\"{}\" will be removed from the bank. Any exams already using it keep \
                     their copies.",
                    truncate(&q.stem, 80),
                ))
                .size(12.5),
            );
        }
        ui.add_space(8.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(
                    Button::new(theme::on_accent("Delete"))
                        .fill(theme::RED)
                        .corner_radius(CornerRadius::same(8)),
                )
                .clicked()
            {
                confirmed = true;
            }
            if ui.add(Button::new(theme::text("Cancel"))).clicked() {
                close = true;
            }
        });
    });
    if confirmed {
        if let Some(bank) = app.store.bank_mut(&bank_id) {
            bank.questions.retain(|x| x.id != qid);
            bank.modified = now_secs();
        }
        app.store.save_banks();
        if app
            .qedit
            .as_ref()
            .is_some_and(|d| d.qid.as_deref() == Some(qid.as_str()))
        {
            app.qedit = None;
        }
        app.toast("Question deleted");
    }
    if close || confirmed || modal.should_close() {
        with_state(ctx, key, |s: &mut BankEditState| s.confirm_delete = None);
    }
}

/// Left outline: "All questions (N)" plus topic chips with counts; clicking a chip
/// filters the card list.
fn draw_outline(ui: &mut Ui, ctx: &egui::Context, key: UiId, questions: &[Question]) {
    let some_filtered = with_state(ctx, key, |s: &mut BankEditState| {
        s.topic.is_some() || s.diff.is_some() || !s.search.is_empty()
    });
    let label = format!("All questions ({})", questions.len());
    if some_filtered
        && ui
            .add(
                Button::new(theme::on_accent(&label))
                    .fill(theme::ACCENT)
                    .corner_radius(CornerRadius::same(8)),
            )
            .clicked()
    {
        with_state(ctx, key, |s: &mut BankEditState| {
            s.topic = None;
            s.diff = None;
            s.search.clear();
        });
    }
    ui.add_space(8.0);
    ui.label(theme::muted("Topics").size(11.0));

    // Unique topics with counts, alphabetical.
    let mut counts: Vec<(String, usize)> = Vec::new();
    for q in questions {
        for t in &q.topics {
            let t = t.trim().to_string();
            if t.is_empty() {
                continue;
            }
            match counts.iter_mut().find(|(k, _)| *k == t) {
                Some((_, c)) => *c += 1,
                None => counts.push((t, 1)),
            }
        }
    }
    counts.sort();
    for (topic, count) in &counts {
        let selected =
            with_state(ctx, key, |s: &mut BankEditState| s.topic.as_deref() == Some(topic));
        let label = RichText::new(format!("{topic} ({count})"))
            .color(if selected { theme::ACCENT } else { theme::TEXT })
            .size(12.5);
        let btn = Button::new(label)
            .corner_radius(CornerRadius::same(6))
            .min_size(Vec2::new(OUTLINE_W - 20.0, 0.0))
            .fill(if selected { theme::ACCENT_FADED } else { theme::CREAM });
        if ui.add(btn).clicked() {
            let t = topic.clone();
            with_state(ctx, key, |s: &mut BankEditState| {
                s.topic = if selected { None } else { Some(t) };
            });
        }
    }
}

/// Right column: search / difficulty filter / sort, then the question cards.
fn draw_question_cards(
    ui: &mut Ui,
    app: &mut App,
    ctx: &egui::Context,
    key: UiId,
    bank_id: &str,
    questions: &[Question],
) {
    // Toolbar: search left, difficulty + sort right.
    ui.horizontal(|ui| {
        let mut search = with_state(ctx, key, |s: &mut BankEditState| s.search.clone());
        let resp = ui.add(
            TextEdit::singleline(&mut search)
                .hint_text("Search question stems")
                .desired_width((ui.available_width() - 300.0).max(200.0)),
        );
        if resp.changed() {
            with_state(ctx, key, |s: &mut BankEditState| s.search = search.clone());
        }
        ui.label(""); // gap; the two filter widgets hug the right
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut sort_val = with_state(ctx, key, |s: &mut BankEditState| s.sort);
            ComboBox::from_id_salt("banks_sort")
                .selected_text(
                    theme::text(format!("Sort: {}", sort_val.label())).size(12.0),
                )
                .show_ui(ui, |ui| {
                    for s in Sort::all() {
                        ui.selectable_value(&mut sort_val, s, s.label());
                    }
                });
            if sort_val != with_state(ctx, key, |s: &mut BankEditState| s.sort) {
                let sv = sort_val;
                with_state(ctx, key, |s: &mut BankEditState| s.sort = sv);
            }
            ui.add_space(8.0);
            let diff_text = with_state(ctx, key, |s: &mut BankEditState| match s.diff {
                Some(d) => format!("Difficulty: {}", diff_label(d)),
                None => "Difficulty: All".to_string(),
            });
            ui.menu_button(theme::text(&diff_text).size(12.0), |ui| {
                let options: [(Option<Difficulty>, &str); 4] = [
                    (None, "All"),
                    (Some(Difficulty::Low), "Low"),
                    (Some(Difficulty::Medium), "Medium"),
                    (Some(Difficulty::High), "High"),
                ];
                for (d, label) in options {
                    let selected =
                        with_state(ctx, key, |s: &mut BankEditState| s.diff == d);
                    if ui
                        .add(
                            Button::new(if selected {
                                RichText::new(format!("✓ {label}")).color(theme::ACCENT)
                            } else {
                                RichText::new(label)
                            })
                            .frame(false),
                        )
                        .clicked()
                    {
                        let dd = d;
                        with_state(ctx, key, |s: &mut BankEditState| s.diff = dd);
                        ui.close();
                    }
                }
            });
        });
    });

    ui.add_space(4.0);

    // Filter + order.
    let (query, diff, topic, sort) = with_state(ctx, key, |s: &mut BankEditState| {
        (
            s.search.trim().to_lowercase(),
            s.diff,
            s.topic.clone(),
            s.sort,
        )
    });
    let mut order: Vec<usize> = questions
        .iter()
        .enumerate()
        .filter(|(_, q)| {
            (query.is_empty() || q.stem.to_lowercase().contains(&query))
                && diff.is_none_or(|d| Some(d) == q.difficulty)
                && topic
                    .as_deref()
                    .is_none_or(|t| q.topics.iter().any(|x| x.trim() == t))
        })
        .map(|(i, _)| i)
        .collect();
    order.sort_by(|&a, &b| {
        let (qa, qb) = (&questions[a], &questions[b]);
        match sort {
            Sort::Newest => qb.created.cmp(&qa.created),
            Sort::Oldest => qa.created.cmp(&qb.created),
            Sort::Kind => qa
                .kind
                .qkind()
                .label()
                .cmp(qb.kind.qkind().label())
                .then_with(|| qb.created.cmp(&qa.created)),
            Sort::Difficulty => diff_rank(qa.difficulty)
                .cmp(&diff_rank(qb.difficulty))
                .then_with(|| qb.created.cmp(&qa.created)),
            Sort::Topic => first_topic(qa)
                .cmp(&first_topic(qb))
                .then_with(|| qb.created.cmp(&qa.created)),
        }
    });

    if questions.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(theme::serif_strong("No questions yet").size(18.0));
            ui.add_space(4.0);
            ui.label(theme::muted("Use ＋ New question to write the first one.").size(12.5));
        });
    } else if order.is_empty() {
        ui.label(theme::muted("No questions match those filters.").size(12.5));
        ui.add_space(8.0);
    }

    egui::ScrollArea::vertical()
        .id_salt("bank_questions_scroll")
        .show(ui, |ui| {
            for i in order {
                question_card(ui, app, ctx, key, bank_id, &questions[i]);
            }
        });
}

fn diff_rank(d: Option<Difficulty>) -> u8 {
    match d {
        Some(Difficulty::Low) => 0,
        Some(Difficulty::Medium) => 1,
        Some(Difficulty::High) => 2,
        None => 3,
    }
}

fn first_topic(q: &Question) -> String {
    q.topics
        .iter()
        .map(|t| t.trim().to_lowercase())
        .min()
        .unwrap_or_else(|| "~unsorted".to_string())
}

/// One question card: type chip, stem, answer preview, points/difficulty, topics,
/// ⋯ menu (Edit / Delete…); clicking the card also opens the editor.
fn question_card(
    ui: &mut Ui,
    app: &mut App,
    ctx: &egui::Context,
    key: UiId,
    bank_id: &str,
    q: &Question,
) {
    let qid = q.id.clone();
    let bank_id_open = bank_id.to_string();
    let mut action: Option<CardMenu> = None;
    let resp = card_frame()
        .inner_margin(Margin::same(12))
        .outer_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    Button::new(theme::on_accent(kind_chip_label(q.kind.qkind())).size(11.0))
                        .fill(kind_chip_color(q.kind.qkind()))
                        .corner_radius(CornerRadius::same(6)),
                );
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    ui.menu_button(RichText::new("⋯").size(13.0), |ui| {
                        if ui.button("Edit").clicked() {
                            action = Some(CardMenu::Item);
                            ui.close();
                        }
                        if ui.button("Delete…").clicked() {
                            action = Some(CardMenu::Delete);
                            ui.close();
                        }
                    });
                    if let Some(points) = points_label(q.points) {
                        ui.label(theme::muted(points).size(11.0));
                    }
                    match q.difficulty {
                        Some(d) => {
                            ui.label(
                                RichText::new(format!("● {}", diff_label(d)))
                                    .color(diff_color(d))
                                    .size(11.0),
                            );
                        }
                        None => {
                            ui.label(theme::muted("—").size(11.0));
                        }
                    }
                });
            });
            ui.add_space(2.0);
            ui.label(theme::strong(truncate(&q.stem, 160)).size(13.0));
            ui.add_space(4.0);

            // Answer preview: correct answers first-class in green.
            ui.horizontal_wrapped(|ui| {
                for (text, good) in answer_preview(q) {
                    ui.label(
                        RichText::new(text)
                            .color(if good { theme::GREEN } else { theme::MUTED })
                            .size(11.5),
                    );
                    ui.add_space(6.0);
                }
            });

            // Topic chips (click to filter the list to that topic).
            let topics: Vec<String> = q
                .topics
                .iter()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            if !topics.is_empty() {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for t in topics {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(t.clone()).size(10.5).color(theme::MUTED),
                                )
                                .fill(theme::ACCENT_FADED)
                                .corner_radius(CornerRadius::same(8)),
                            )
                            .clicked()
                        {
                            let ft = t.clone();
                            with_state(ctx, key, |s: &mut BankEditState| s.topic = Some(ft));
                        }
                        ui.add_space(2.0);
                    }
                });
            }
        });

    if let Some(a) = action {
        match a {
            CardMenu::Item => {
                app.qedit =
                    Some(qedit::Draft::from_question(bank_id_open.clone(), q));
            }
            CardMenu::Delete => {
                let qq = qid.clone();
                with_state(ctx, key, |s: &mut BankEditState| s.confirm_delete = Some(qq));
            }
        }
    }
    // Clicking anywhere on the card opens the editor; inner buttons keep their clicks.
    if resp.response.interact(Sense::click()).clicked() {
        app.qedit = Some(qedit::Draft::from_question(bank_id_open, q));
    }
}

/// "✓ answer" previews: green when correct, muted otherwise.
fn answer_preview(q: &Question) -> Vec<(String, bool)> {
    let mut lines = Vec::new();
    match &q.kind {
        crate::model::Kind::Mc { answers } => {
            for a in answers {
                if a.text.trim().is_empty() {
                    continue;
                }
                if a.correct {
                    lines.push((format!("✓ {}", a.text.trim()), true));
                } else {
                    lines.push((a.text.trim().to_string(), false));
                }
            }
        }
        crate::model::Kind::Tf { correct } => {
            let word = if *correct { "True" } else { "False" };
            lines.push((format!("✓ {word}"), true));
        }
        crate::model::Kind::Matching { pairs, distractors } => {
            for p in pairs {
                if p.left.trim().is_empty() && p.right.trim().is_empty() {
                    continue;
                }
                lines.push((format!("{} — {}", p.left.trim(), p.right.trim()), false));
            }
            for d in distractors {
                if d.trim().is_empty() {
                    continue;
                }
                lines.push((format!("{} (distractor)", d.trim()), false));
            }
        }
        crate::model::Kind::Blank { accept } => {
            for row in accept {
                if row.is_empty() {
                    continue;
                }
                lines.push((format!("✓ {}", row.join(" OR ")), true));
            }
        }
        crate::model::Kind::Short { sample } => {
            if !sample.trim().is_empty() {
                lines.push((format!("Sample: {}", sample.trim()), false));
            }
        }
        crate::model::Kind::Multipart { parts } => {
            let n = parts.len();
            lines.push((format!("{n} part{}", if n == 1 { "" } else { "s" }), false));
        }
    }
    lines
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars).collect();
        out.push('…');
        out
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_date_formats_epoch_days() {
        assert_eq!(fmt_date(0), "—");
        assert_eq!(fmt_date(86_400), "Jan 2, 1970");
        // 1970-01-01 + 56*365 + 14 leap days + day-of-year offset of Oct 15 (287).
        let secs: i64 = ((2026 - 1970) as i64 * 365 + 14 + 287) * 86_400;
        assert_eq!(fmt_date(secs), "Oct 15, 2026");
    }

    #[test]
    fn chip_colors_are_distinct() {
        let kinds = [
            QKind::Mc,
            QKind::Tf,
            QKind::Matching,
            QKind::Blank,
            QKind::Short,
            QKind::Multipart,
        ];
        let colors: Vec<Color32> = kinds.iter().map(|k| kind_chip_color(*k)).collect();
        assert_eq!(colors.len(), 6);
        for i in 0..colors.len() {
            for j in i + 1..colors.len() {
                assert_ne!(colors[i], colors[j], "kinds share chip colors");
            }
        }
    }

    #[test]
    fn points_label_works() {
        assert_eq!(points_label(None), None);
        assert_eq!(points_label(Some(2.0)), Some("2 pt".to_string()));
        assert_eq!(points_label(Some(2.5)), Some("2.5 pt".to_string()));
    }
}