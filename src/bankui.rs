//! Question Banks screens (V4): the bank list and the bank editor. The editor is a
//! grid of question cards; clicking a card expands it in place into the two-column
//! editor from `qedit` (one open at a time: the open card is `App.qedit`'s draft).
//! Also hosts the shared `fmt_date` / kind-chip helpers other modules import.

use egui::{
    Align, Button, Color32, ComboBox, CornerRadius, Frame, Layout, Margin, Pos2, Rect, RichText,
    Sense, Stroke, TextEdit, Ui, Vec2,
};
// egui's widget id type vs the store's bank/question id (a String).
use egui::Id as UiId;

use crate::kit::{self, ViewMode};
use crate::model::{Difficulty, Id, QKind, Question};
use crate::screens::now_secs;
use crate::{qedit, screens, theme, App, Screen};

// ---------- shared helpers (pub: other workers may use these) ----------

/// Formats a unix-seconds timestamp as a short date, "Oct 15, 2026".
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

/// "just now", "2 hours ago", ... falling back to a date after a month.
pub fn rel_time(epoch: i64) -> String {
    if epoch <= 0 {
        return "—".to_string();
    }
    let d = (now_secs() - epoch).max(0);
    let plural = |n: i64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    if d < 60 {
        "just now".to_string()
    } else if d < 3600 {
        plural(d / 60, "minute")
    } else if d < 86_400 {
        plural(d / 3600, "hour")
    } else if d < 30 * 86_400 {
        plural(d / 86_400, "day")
    } else {
        fmt_date(epoch)
    }
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
pub fn texture_for(
    app: &mut App,
    ctx: &egui::Context,
    img: &crate::model::ImageRef,
) -> Option<egui::TextureHandle> {
    app.image_texture(ctx, &img.file)
}

fn diff_label(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Low => "Easy",
        Difficulty::Medium => "Medium",
        Difficulty::High => "Hard",
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

/// Reads/edits per-frame UI state kept in egui context memory.
fn with_state<T, R>(ctx: &egui::Context, key: UiId, f: impl FnOnce(&mut T) -> R) -> R
where
    T: Default + Clone + Send + Sync + 'static,
{
    ctx.data_mut(|d| f(d.get_temp_mut_or_default::<T>(key)))
}

fn view_of(list: bool) -> ViewMode {
    if list { ViewMode::List } else { ViewMode::Cards }
}

fn save_settings(app: &App) {
    let _ = crate::store::save_json("settings.json", &app.settings);
}

const MAX_W: f32 = 1040.0;

/// Draws a clickable crumb "Banks" that returns to the list.
fn crumb_row(ui: &mut Ui, app: &mut App, tail: Option<&str>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let b = ui.add(
            Button::new(RichText::new("Banks").size(11.5).color(theme::muted_col())).frame(false),
        );
        if tail.is_some() && b.clicked() {
            app.screen = Screen::Banks;
            app.bank = None;
            app.qedit = None;
        }
        if let Some(t) = tail {
            ui.label(RichText::new("/").size(11.5).color(theme::muted_col()));
            ui.label(RichText::new(t).size(11.5).color(theme::muted_col()));
        }
    });
}

/// Painter-drawn search pill with a magnifier; returns true when text changed.
fn search_pill(ui: &mut Ui, text: &mut String, w: f32, hint: &str) -> bool {
    let mut changed = false;
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(13, 5))
        .show(ui, |ui| {
            ui.set_width(w - 28.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
                let s = Stroke::new(1.6, theme::muted_col());
                ui.painter().circle_stroke(r.center() + Vec2::new(-1.0, -1.0), 5.0, s);
                ui.painter().line_segment(
                    [r.center() + Vec2::new(2.8, 2.8), r.center() + Vec2::new(6.0, 6.0)],
                    s,
                );
                changed = ui
                    .add(
                        TextEdit::singleline(text)
                            .frame(Frame::new())
                            .hint_text(hint)
                            .desired_width(ui.available_width()),
                    )
                    .changed();
            });
        });
    changed
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
pub(crate) enum Sort {
    #[default]
    Newest,
    Oldest,
    Kind,
    Difficulty,
    Topic,
}

impl Sort {
    fn all() -> [Sort; 5] {
        [Sort::Newest, Sort::Oldest, Sort::Kind, Sort::Difficulty, Sort::Topic]
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
    /// Type filter chip (None = All).
    kind: Option<QKind>,
    sort: Sort,
    /// Last deleted question + its index, for the Undo strip.
    last_deleted: Option<(Question, usize)>,
    /// Open card seen last frame (scroll into view when it changes).
    last_open: Option<String>,
    /// Topic rename / merge / delete dialog.
    topic_edit: Option<TopicEdit>,
    /// "Question Bank details" dialog draft (None = closed).
    details: Option<crate::model::BankDetails>,
    /// "Tidy stems" dialog (None = closed).
    tidy: Option<TidyDialog>,
    /// The last applied tidy, for its Undo strip.
    last_tidy: Option<TidyUndo>,
}

/// The "Tidy stems" preview: every question tidying would change, and which are ticked.
#[derive(Clone)]
struct TidyDialog {
    items: Vec<crate::tidy::QuestionTidy>,
    picked: Vec<bool>,
}

/// Questions as they were before the last tidy, and where the backup went.
#[derive(Clone)]
struct TidyUndo {
    before: Vec<Question>,
    backup: String,
}

/// Opens the "Tidy stems" dialog on the bank's current questions.
fn open_tidy(app: &App, ctx: &egui::Context, key: UiId, bank_id: &Id) {
    let items = app.store.bank(bank_id).map(|b| crate::tidy::tidy_bank(&b.questions)).unwrap_or_default();
    let picked = vec![true; items.len()];
    with_state(ctx, key, |s: &mut BankEditState| s.tidy = Some(TidyDialog { items, picked }));
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TopicMode {
    Rename,
    Merge,
    Delete,
}

/// The topic dialog: which topic, what to do, the typed / chosen target.
#[derive(Clone, Debug)]
struct TopicEdit {
    from: String,
    mode: TopicMode,
    text: String,
}

/// "Used in N exams" list dialog: title + exam ids (shared by banks list and editor).
#[derive(Clone, Default)]
struct UsageDialog {
    open: Option<(String, Vec<Id>)>,
}

fn usage_key() -> UiId {
    UiId::new("bank_usage_dialog")
}

fn open_usage(ctx: &egui::Context, title: String, exams: Vec<Id>) {
    with_state(ctx, usage_key(), |s: &mut UsageDialog| s.open = Some((title, exams)));
}

/// Lists the exams that use a bank / question; a row opens that exam.
fn draw_usage_dialog(app: &mut App, ctx: &egui::Context) {
    let Some((title, ids)) = with_state(ctx, usage_key(), |s: &mut UsageDialog| s.open.clone()) else {
        return;
    };
    let mut close = false;
    let mut go: Option<Id> = None;
    let modal = egui::Modal::new(UiId::new("bank_usage_modal")).show(ctx, |ui| {
        ui.set_min_width(380.0);
        ui.set_max_width(460.0);
        ui.label(theme::serif(title.clone()).size(20.0));
        ui.add_space(2.0);
        ui.label(theme::muted(crate::bankops::usage_label(ids.len())).size(12.5));
        ui.add_space(8.0);
        if ids.is_empty() {
            ui.label(theme::muted("No exam places these questions yet. Add them from an exam's question shelf.").size(12.5));
        }
        for id in &ids {
            let Some(e) = app.store.exam(id) else { continue };
            let resp = kit::card().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(theme::strong(truncate(&e.name, 44)));
                        ui.label(
                            theme::muted(format!(
                                "{} question{} · edited {}",
                                e.items.len(),
                                if e.items.len() == 1 { "" } else { "s" },
                                rel_time(e.modified)
                            ))
                            .size(11.5),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(theme::accent("Open").size(12.5));
                    });
                });
            });
            let r = ui.interact(resp.response.rect, ui.id().with(("usage_row", id)), Sense::click());
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                go = Some(id.clone());
            }
            ui.add_space(6.0);
        }
        ui.add_space(6.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if kit::ghost_button(ui, "Close").clicked() {
                close = true;
            }
        });
    });
    if let Some(id) = &go {
        app.qedit = None;
        app.open_exam(id);
    }
    if close || go.is_some() || modal.should_close() {
        with_state(ctx, usage_key(), |s: &mut UsageDialog| s.open = None);
    }
}

/// Accent "Used in N exams" link text; returns its response (hover sense only: the
/// caller's card owns the click and checks the pointer against this rect).
fn usage_link(ui: &mut Ui, n: usize, size: f32) -> egui::Response {
    let txt = crate::bankops::usage_label(n);
    let rt = if n > 0 { RichText::new(txt).size(size).color(theme::accent_col()).underline() } else { theme::muted(txt).size(size) };
    let r = ui.add(egui::Label::new(rt).extend().sense(Sense::hover()));
    if n > 0 && r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r
}

// ---------- screen 1: the Question Banks list ----------

struct BankRow {
    id: Id,
    name: String,
    nq: usize,
    topics: usize,
    modified: i64,
    mix: [usize; 6],
    /// Exams placing at least one of its questions.
    used_in: Vec<Id>,
}

fn mix_index(k: QKind) -> usize {
    match k {
        QKind::Mc => 0,
        QKind::Tf => 1,
        QKind::Multipart => 2,
        QKind::Matching => 3,
        QKind::Blank => 4,
        QKind::Short => 5,
    }
}

fn unique_topics(qs: &[Question]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for q in qs {
        for t in &q.topics {
            let t = t.trim();
            if !t.is_empty() && !v.iter().any(|x| x == t) {
                v.push(t.to_string());
            }
        }
    }
    v.sort();
    v
}

impl App {
    pub fn draw_banks(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let key = UiId::new("testparrot_banks_state");
        screens::desk_panel(ui, 20, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("banks_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.scope(|ui| {
                        ui.set_max_width(ui.available_width().min(MAX_W));
                        self.banks_body(ui, &ctx, key);
                    });
                });
        });
        draw_bank_overlays(self, &ctx, key);
        draw_usage_dialog(self, &ctx);
    }

    fn banks_body(&mut self, ui: &mut Ui, ctx: &egui::Context, key: UiId) {
        let total_q: usize = self.store.banks.iter().map(|b| b.questions.len()).sum();
        let sub = format!(
            "{} bank{} · {} question{}",
            self.store.banks.len(),
            if self.store.banks.len() == 1 { "" } else { "s" },
            total_q,
            if total_q == 1 { "" } else { "s" }
        );
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                kit::page_header(ui, &["Banks"], "Question banks", Some(&sub));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if kit::primary_button(ui, "New bank").clicked() {
                    let id = self.new_bank("Untitled Question Bank");
                    self.open_bank(&id);
                }
                if kit::ghost_button(ui, "Import").clicked() {
                    self.screen = crate::Screen::Imports;
                }
            });
        });
        ui.add_space(12.0);

        // Toolbar.
        let mut search = with_state(ctx, key, |s: &mut BanksState| s.search.clone());
        let mut list = self.settings.banks_list_view;
        ui.horizontal(|ui| {
            if search_pill(ui, &mut search, 300.0, "Search banks") {
                with_state(ctx, key, |s: &mut BanksState| s.search = search.clone());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut mode = view_of(list);
                if kit::view_toggle(ui, &mut mode) {
                    list = mode == ViewMode::List;
                }
            });
        });
        if list != self.settings.banks_list_view {
            self.settings.banks_list_view = list;
            save_settings(self);
        }
        ui.add_space(12.0);

        if self.store.banks.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(theme::serif("Nothing here yet").size(22.0));
                ui.add_space(4.0);
                ui.label(
                    theme::muted(
                        "Create your first Question Bank, or hand a test you already have to the importer.",
                    )
                    .size(13.0),
                );
            });
            return;
        }

        let query = search.trim().to_lowercase();
        let rows: Vec<BankRow> = self
            .store
            .banks
            .iter()
            .filter(|b| query.is_empty() || b.name.to_lowercase().contains(&query))
            .map(|b| {
                let mut mix = [0usize; 6];
                for q in &b.questions {
                    mix[mix_index(q.kind.qkind())] += 1;
                }
                BankRow {
                    id: b.id.clone(),
                    name: b.name.clone(),
                    nq: b.questions.len(),
                    topics: unique_topics(&b.questions).len(),
                    modified: b.modified,
                    mix,
                    used_in: crate::bankops::exams_using_bank(&self.store.exams, &b.id).iter().map(|e| e.id.clone()).collect(),
                }
            })
            .collect();
        if rows.is_empty() {
            ui.label(theme::muted(format!("No banks match \"{search}\".")));
            return;
        }

        if list {
            // Header row.
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                kit::small_caps(ui, "Bank");
            });
            ui.add_space(2.0);
            for r in &rows {
                bank_list_row(ui, self, ctx, key, r);
                ui.add_space(6.0);
            }
        } else {
            let avail = ui.available_width();
            let gap = 12.0;
            let cols = if avail >= 760.0 { 3 } else if avail >= 500.0 { 2 } else { 1 };
            let cw = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).floor();
            for chunk in rows.chunks(cols) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for r in chunk {
                        ui.allocate_ui_with_layout(
                            Vec2::new(cw, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                ui.set_width(cw);
                                bank_card(ui, self, ctx, key, r);
                            },
                        );
                    }
                });
                ui.add_space(gap);
            }
        }
    }

    /// The bank editor: breadcrumb, serif editable name, subtitle, toolbar (search,
    /// type chips, topic chips, sort/difficulty, view toggle) and a grid of question
    /// cards that expand in place into the editor.
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
        // A draft belonging to another bank never carries over.
        if self.qedit.as_ref().is_some_and(|d| d.bank_id != bank_id) {
            self.qedit = None;
        }

        let key = UiId::new("testparrot_bank_edit_state").with(&bank_id);
        // Headless test hook: QUESHION_OPEN_Q=<n> opens the n-th question once.
        if let Ok(v) = std::env::var("QUESHION_OPEN_Q") {
            let done = ctx.data_mut(|d| d.get_temp::<bool>(UiId::new("qopen_hook")).unwrap_or(false));
            if !done {
                ctx.data_mut(|d| d.insert_temp(UiId::new("qopen_hook"), true));
                let n: usize = v.parse().unwrap_or(1);
                let q = self.store.bank(&bank_id).and_then(|b| b.questions.get(n.saturating_sub(1)).cloned());
                if let Some(q) = q {
                    self.qedit = Some(qedit::Draft::from_question(bank_id.clone(), &q));
                }
            }
        }
        screens::desk_panel(ui, 20, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("bank_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.scope(|ui| {
                        ui.set_max_width(ui.available_width().min(MAX_W));
                        self.bank_edit_body(ui, &ctx, key, &bank_id);
                    });
                });
        });

        // Insert-figure modal stacks on top of the open editor.
        self.draw_figure_picker(&ctx);
        draw_topic_dialog(self, &ctx, key, &bank_id);
        draw_details_dialog(self, &ctx, key, &bank_id);
        // Headless/QA hook: QUESHION_TIDY=1 opens the bank's "Tidy stems" dialog once.
        if std::env::var("QUESHION_TIDY").is_ok() {
            let hid = UiId::new("qs_tidy_hook");
            if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
                ctx.data_mut(|d| d.insert_temp(hid, true));
                open_tidy(self, &ctx, key, &bank_id);
            }
        }
        draw_tidy_dialog(self, &ctx, key, &bank_id);
        draw_usage_dialog(self, &ctx);
        // Headless/QA hook: QUESHION_IMPORT_MODAL=1 opens the bank's Import modal once.
        if std::env::var("QUESHION_IMPORT_MODAL").is_ok() {
            let hid = UiId::new("qs_import_modal_hook");
            if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
                ctx.data_mut(|d| d.insert_temp(hid, true));
                crate::importui::open_import_modal(&ctx, &bank_id);
            }
        }
        self.draw_import_modal(&ctx);
        self.draw_resolve_images(&ctx, &bank_id);
        draw_bank_export(self, &ctx);
    }

    fn bank_edit_body(&mut self, ui: &mut Ui, ctx: &egui::Context, key: UiId, bank_id: &Id) {
        let (bank_name, questions, modified) = match self.store.bank(bank_id) {
            Some(b) => (b.name.clone(), b.questions.clone(), b.modified),
            None => return,
        };
        let topics = unique_topics(&questions);

        // Header: breadcrumb, serif name + subtitle, actions.
        crumb_row(ui, self, Some(&bank_name));
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let name_w = (ui.available_width() - 340.0).clamp(220.0, 680.0);
            ui.vertical(|ui| {
                let mut name_buf = bank_name.clone();
                let name_font = egui::FontId::new(34.0, theme::serif_family());
                // The name field hugs its text so the rename pencil sits right after it.
                let text_w = ui.fonts_mut(|f| f.layout_no_wrap(name_buf.clone(), name_font.clone(), theme::ink()).size().x);
                let field_w = (text_w + 12.0).clamp(160.0, name_w);
                let resp = ui
                    .horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let resp = ui.add(
                            TextEdit::singleline(&mut name_buf)
                                .id(ui.id().with(("bank_name_field", bank_id.as_str())))
                                .font(name_font.clone())
                                .text_color(theme::ink())
                                .frame(Frame::new())
                                .hint_text("Untitled bank")
                                .desired_width(field_w),
                        );
                        if !resp.has_focus() && pencil_button(ui).on_hover_text("Rename this bank").clicked() {
                            resp.request_focus();
                        }
                        resp
                    })
                    .inner;
                if resp.changed() && !name_buf.trim().is_empty() && name_buf.trim() != bank_name {
                    let t = now_secs();
                    if let Some(b) = self.store.bank_mut(bank_id) {
                        b.name = name_buf.trim().to_string();
                        b.modified = t;
                    }
                    self.store.save_banks();
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(
                        theme::muted(format!(
                            "{} question{} · {} topic{} · edited {} · ",
                            questions.len(),
                            if questions.len() == 1 { "" } else { "s" },
                            topics.len(),
                            if topics.len() == 1 { "" } else { "s" },
                            rel_time(modified),
                        ))
                        .size(13.0),
                    );
                    let used: Vec<Id> = crate::bankops::exams_using_bank(&self.store.exams, bank_id).iter().map(|e| e.id.clone()).collect();
                    let r = usage_link(ui, used.len(), 13.0);
                    if !used.is_empty() && ui.interact(r.rect, ui.id().with("bank_usage_link"), Sense::click()).clicked() {
                        open_usage(ctx, bank_name.clone(), used);
                    }
                    let pics: usize = questions.iter().map(crate::bankops::pictures_needed).sum();
                    if pics > 0 {
                        ui.label(theme::muted(" · ").size(13.0));
                        let link = ui
                            .add(
                                egui::Label::new(
                                    RichText::new(format!("{pics} picture{} needed", if pics == 1 { "" } else { "s" }))
                                        .size(13.0)
                                        .color(theme::bad())
                                        .underline(),
                                )
                                .sense(Sense::click()),
                            )
                            .on_hover_text("Add the missing pictures: upload, crop from the original, or describe them");
                        if link.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if link.clicked() {
                            crate::importui::open_resolve_images(self, bank_id);
                        }
                    }
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if kit::primary_button(ui, "Add question").clicked() {
                    self.qedit = Some(qedit::Draft::new(bank_id.clone()));
                }
                if !questions.is_empty() && kit::ghost_button(ui, "Export").on_hover_text("Preview and download the bank file (PDF, answers included) or its JSON").clicked() {
                    open_bank_export(ctx, bank_id);
                }
                if kit::ghost_button(ui, "Import").on_hover_text("Import files or pasted questions into this bank").clicked() {
                    crate::importui::open_import_modal(ctx, bank_id);
                }
                if kit::ghost_button(ui, "Pop-over").on_hover_text("Open this bank in its own window").clicked() {
                    open_popover(ctx, bank_id);
                }
                if kit::ghost_button(ui, "Details").on_hover_text("Description, author and license").clicked() {
                    let d = self.store.bank(bank_id).map(|b| b.details.clone()).unwrap_or_default();
                    with_state(ctx, key, |s: &mut BankEditState| s.details = Some(d));
                }
                // (Details has its own button next to this menu; the menu keeps what has none.)
                ui.menu_button(RichText::new("…").size(16.0).color(theme::muted_col()), |ui| {
                    ui.set_min_width(170.0);
                    let tidy = ui
                        .add_enabled(!questions.is_empty(), Button::new("Tidy stems…"))
                        .on_hover_text("Clean up stems from older imports: tables of blanks, line breaks, spacing. Shows a preview first.");
                    if tidy.clicked() {
                        open_tidy(self, ctx, key, bank_id);
                        ui.close();
                    }
                })
                .response
                .on_hover_text("More bank actions");
            });
        });
        let desc = self.store.bank(bank_id).map(|b| b.details.description.trim().to_string()).unwrap_or_default();
        if !desc.is_empty() {
            ui.add_space(2.0);
            ui.add(egui::Label::new(theme::text(desc).size(13.0)).wrap());
        }
        ui.add_space(12.0);
        // Usage counts per question for the cards (one pass per frame).
        let usage = crate::bankops::question_usage(&self.store.exams, bank_id);
        ctx.data_mut(|d| d.insert_temp(key.with("usage"), usage));
        let wide = ui.available_width() >= 860.0;
        if wide {
            let side_w = 200.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                ui.allocate_ui_with_layout(Vec2::new(side_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(side_w);
                    outline_panel(ui, ctx, key, &questions);
                });
                let rest = ui.available_width();
                ui.allocate_ui_with_layout(Vec2::new(rest, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(rest);
                    self.bank_edit_main(ui, ctx, key, bank_id, questions, topics, true);
                });
            });
        } else {
            self.bank_edit_main(ui, ctx, key, bank_id, questions, topics, false);
        }
    }

    /// Toolbar, filters and the question cards (right of the outline when wide).
    #[allow(clippy::too_many_arguments)]
    fn bank_edit_main(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        key: UiId,
        bank_id: &Id,
        questions: Vec<Question>,
        topics: Vec<String>,
        wide: bool,
    ) {
        // Toolbar row 1: search, sort, difficulty, view toggle.
        let st = with_state(ctx, key, |s: &mut BankEditState| s.clone());
        let mut search = st.search.clone();
        let mut list = self.settings.bank_edit_list_view;
        ui.horizontal(|ui| {
            if search_pill(ui, &mut search, 280.0, "Search questions") {
                with_state(ctx, key, |s: &mut BankEditState| s.search = search.clone());
            }
            let mut sort_val = st.sort;
            ComboBox::from_id_salt("banks_sort")
                .selected_text(theme::text(format!("Sort: {}", sort_val.label())).size(12.0))
                .show_ui(ui, |ui| {
                    for s in Sort::all() {
                        ui.selectable_value(&mut sort_val, s, s.label());
                    }
                });
            if sort_val != st.sort {
                with_state(ctx, key, |s: &mut BankEditState| s.sort = sort_val);
            }
            let mut diff_val = st.diff;
            ComboBox::from_id_salt("banks_diff")
                .selected_text(
                    theme::text(match diff_val {
                        Some(d) => format!("Difficulty: {}", diff_label(d)),
                        None => "Difficulty: All".to_string(),
                    })
                    .size(12.0),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut diff_val, None, "All");
                    for d in [Difficulty::Low, Difficulty::Medium, Difficulty::High] {
                        ui.selectable_value(&mut diff_val, Some(d), diff_label(d));
                    }
                });
            if diff_val != st.diff {
                with_state(ctx, key, |s: &mut BankEditState| s.diff = diff_val);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut mode = view_of(list);
                if kit::view_toggle(ui, &mut mode) {
                    list = mode == ViewMode::List;
                }
            });
        });
        if list != self.settings.bank_edit_list_view {
            self.settings.bank_edit_list_view = list;
            save_settings(self);
        }
        ui.add_space(6.0);

        // Narrow windows: type + topic chips stand in for the outline panel.
        if !wide {
        // Toolbar row 2: type chips with counts.
        let mut counts = [0usize; 6];
        for q in &questions {
            counts[mix_index(q.kind.qkind())] += 1;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            if kit::chip_toggle(ui, &format!("All  {}", questions.len()), st.kind.is_none()).clicked() {
                with_state(ctx, key, |s: &mut BankEditState| s.kind = None);
            }
            for k in [
                QKind::Mc,
                QKind::Tf,
                QKind::Multipart,
                QKind::Matching,
                QKind::Blank,
                QKind::Short,
            ] {
                let n = counts[mix_index(k)];
                if n == 0 && st.kind != Some(k) {
                    continue;
                }
                if kit::chip_toggle(ui, &format!("{}  {n}", qedit::type_tag(k)), st.kind == Some(k))
                    .clicked()
                {
                    let sel = if st.kind == Some(k) { None } else { Some(k) };
                    with_state(ctx, key, |s: &mut BankEditState| s.kind = sel);
                }
            }
        });
        // Row 3: topic chips (replaces the old outline panel).
        if !topics.is_empty() {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                kit::small_caps(ui, "Topics");
                for t in &topics {
                    let n = questions
                        .iter()
                        .filter(|q| q.topics.iter().any(|x| x.trim() == t))
                        .count();
                    let on = st.topic.as_deref() == Some(t.as_str());
                    let chip = kit::chip_toggle(ui, &format!("{t}  {n}"), on).on_hover_text("Right-click to rename, merge or delete");
                    if chip.clicked() {
                        let sel = if on { None } else { Some(t.clone()) };
                        with_state(ctx, key, |s: &mut BankEditState| s.topic = sel);
                    }
                    // Narrow layout has no outline panel: the topic menu lives on the chip.
                    chip.context_menu(|ui| {
                        let mut act = None;
                        if ui.button("Rename…").clicked() {
                            act = Some(TopicMode::Rename);
                        }
                        if topics.len() > 1 && ui.button("Merge into…").clicked() {
                            act = Some(TopicMode::Merge);
                        }
                        if ui.button("Delete…").clicked() {
                            act = Some(TopicMode::Delete);
                        }
                        if let Some(mode) = act {
                            let text = if mode == TopicMode::Rename { t.clone() } else { String::new() };
                            with_state(ctx, key, |s: &mut BankEditState| s.topic_edit = Some(TopicEdit { from: t.clone(), mode, text }));
                            ui.close();
                        }
                    });
                }
            });
        }
        }
        // Active filter note with a clear link (the outline shows the choice too).
        if wide && (st.topic.is_some() || st.kind.is_some()) {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let mut parts: Vec<String> = Vec::new();
                if let Some(k) = st.kind {
                    parts.push(k.label().to_string());
                }
                if let Some(t) = &st.topic {
                    parts.push(format!("topic “{t}”"));
                }
                ui.label(theme::muted(format!("Showing {}", parts.join(" · "))).size(12.0));
                if ui.add(Button::new(theme::accent("Clear").size(12.0)).frame(false)).clicked() {
                    with_state(ctx, key, |s: &mut BankEditState| {
                        s.kind = None;
                        s.topic = None;
                    });
                }
            });
        }
        ui.add_space(10.0);

        // Undo strip for the last deleted question.
        if let Some((dq, idx)) = st.last_deleted.clone() {
            kit::card().inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(theme::text(format!("Question deleted: {}", truncate(&crate::equation::stem_preview(&dq.stem), 60))).size(12.5));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(Button::new(RichText::new("Dismiss").color(theme::muted_col())).frame(false))
                            .clicked()
                        {
                            with_state(ctx, key, |s: &mut BankEditState| s.last_deleted = None);
                        }
                        if kit::ghost_button(ui, "Undo").clicked() {
                            if let Some(b) = self.store.bank_mut(bank_id) {
                                let at = idx.min(b.questions.len());
                                b.questions.insert(at, dq.clone());
                                b.modified = now_secs();
                            }
                            self.store.save_banks();
                            with_state(ctx, key, |s: &mut BankEditState| s.last_deleted = None);
                            self.toast("Question restored");
                        }
                    });
                });
            });
            ui.add_space(10.0);
        }

        // Undo strip for the last "Tidy stems".
        if let Some(u) = st.last_tidy.clone() {
            kit::card().inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let file = std::path::Path::new(&u.backup).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    let n = u.before.len();
                    ui.label(theme::text(format!("Tidied {n} question{} · backup saved as {file}", if n == 1 { "" } else { "s" })).size(12.5))
                        .on_hover_text(u.backup.clone());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(Button::new(RichText::new("Dismiss").color(theme::muted_col())).frame(false)).clicked() {
                            with_state(ctx, key, |s: &mut BankEditState| s.last_tidy = None);
                        }
                        if kit::ghost_button(ui, "Undo").on_hover_text("Put these questions back as they were before tidying").clicked() {
                            let t = now_secs();
                            if let Some(b) = self.store.bank_mut(bank_id) {
                                crate::tidy::undo(&mut b.questions, &u.before);
                                b.modified = t;
                            }
                            self.store.save_banks();
                            with_state(ctx, key, |s: &mut BankEditState| s.last_tidy = None);
                            self.toast("Tidy undone");
                        }
                    });
                });
            });
            ui.add_space(10.0);
        }

        // Filter + order.
        let query = search.trim().to_lowercase();
        let mut order: Vec<usize> = questions
            .iter()
            .enumerate()
            .filter(|(_, q)| {
                (query.is_empty() || q.stem.to_lowercase().contains(&query) || crate::equation::stem_preview(&q.stem).to_lowercase().contains(&query))
                    && st.diff.is_none_or(|d| Some(d) == q.difficulty)
                    && st.kind.is_none_or(|k| k == q.kind.qkind())
                    && st
                        .topic
                        .as_deref()
                        .is_none_or(|t| q.topics.iter().any(|x| x.trim() == t))
            })
            .map(|(i, _)| i)
            .collect();
        order.sort_by(|&a, &b| {
            let (qa, qb) = (&questions[a], &questions[b]);
            match st.sort {
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

        let open_qid: Option<String> = self.qedit.as_ref().and_then(|d| d.qid.clone());
        let new_open = self.qedit.as_ref().is_some_and(|d| d.qid.is_none());

        // Scroll the newly opened card into view once.
        let open_marker = open_qid.clone().or(new_open.then(|| "<new>".to_string()));
        let scroll_now = open_marker.is_some() && st.last_open != open_marker;
        if st.last_open != open_marker {
            with_state(ctx, key, |s: &mut BankEditState| s.last_open = open_marker.clone());
        }

        let avail = ui.available_width();
        let gap = 10.0;
        let list = self.settings.bank_edit_list_view;
        let cols = if list { 1 } else if avail >= 820.0 { 3 } else if avail >= 540.0 { 2 } else { 1 };
        let cw = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).floor();

        if new_open {
            draw_card(ui, self, ctx, key, bank_id, avail, None, 0, list, &topics, scroll_now);
            ui.add_space(gap);
        }
        if questions.is_empty() && !new_open {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(theme::serif("No questions yet").size(22.0));
                ui.add_space(4.0);
                ui.label(theme::muted("Use Add question to write the first one.").size(13.0));
            });
        } else if order.is_empty() {
            ui.label(theme::muted("No questions match those filters.").size(12.5));
        }

        let mut pending: Vec<usize> = Vec::new();
        for &i in &order {
            let is_open = open_qid.as_deref() == Some(questions[i].id.as_str());
            if is_open {
                flush_row(ui, self, ctx, key, bank_id, &questions, &mut pending, cw, gap, list, &topics);
                draw_card(ui, self, ctx, key, bank_id, avail, Some(&questions[i]), i + 1, list, &topics, scroll_now);
                ui.add_space(if list { 6.0 } else { gap });
            } else {
                pending.push(i);
                if pending.len() == cols {
                    flush_row(ui, self, ctx, key, bank_id, &questions, &mut pending, cw, gap, list, &topics);
                }
            }
        }
        flush_row(ui, self, ctx, key, bank_id, &questions, &mut pending, cw, gap, list, &topics);

        if !questions.is_empty() {
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    theme::muted(format!(
                        "Showing {} of {} · open a card to edit it here, press Done to fold it back up",
                        order.len(),
                        questions.len()
                    ))
                    .size(12.0),
                );
            });
        }
    }
}

/// Draws the pending collapsed cards as one grid row, then clears `pending`.
#[allow(clippy::too_many_arguments)]
fn flush_row(
    ui: &mut Ui,
    app: &mut App,
    ctx: &egui::Context,
    key: UiId,
    bank_id: &str,
    questions: &[Question],
    pending: &mut Vec<usize>,
    cw: f32,
    gap: f32,
    list: bool,
    pool: &[String],
) {
    if pending.is_empty() {
        return;
    }
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for &i in pending.iter() {
            ui.allocate_ui_with_layout(Vec2::new(cw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(cw);
                draw_card(ui, app, ctx, key, bank_id, cw, Some(&questions[i]), i + 1, list, pool, false);
            });
        }
    });
    ui.add_space(if list { 6.0 } else { gap });
    pending.clear();
}

// ---------- banks list pieces ----------

const MIX_ALPHA: [u8; 6] = [255, 205, 160, 120, 85, 55];

fn paint_mix_bar(ui: &mut Ui, w: f32, mix: &[usize; 6]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 6.0), Sense::hover());
    let total: usize = mix.iter().sum();
    ui.painter().rect_filled(rect, CornerRadius::same(3), theme::field());
    if total == 0 {
        return;
    }
    let mut x = rect.left();
    let ac = theme::accent_col();
    for (i, n) in mix.iter().enumerate() {
        if *n == 0 {
            continue;
        }
        let seg_w = rect.width() * (*n as f32 / total as f32);
        let col = Color32::from_rgba_unmultiplied(ac.r(), ac.g(), ac.b(), MIX_ALPHA[i]);
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(seg_w.max(2.0), 6.0)),
            CornerRadius::same(3),
            col,
        );
        x += seg_w;
    }
}

/// What the bank "..." menu asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BankAct {
    Open,
    Popover,
    Export,
    Rename,
    Delete,
}

/// The bank "..." menu items in order (as on the reference: Open, Pop-over, Export, then Delete
/// after a separator; Rename is ours). `None` marks the separator; the bool says enabled
/// (Export needs at least one question).
fn bank_menu_items(nq: usize) -> Vec<Option<(BankAct, &'static str, bool)>> {
    vec![
        Some((BankAct::Open, "Open", true)),
        Some((BankAct::Popover, "Pop-over", true)),
        Some((BankAct::Export, "Export…", nq > 0)),
        Some((BankAct::Rename, "Rename", true)),
        None,
        Some((BankAct::Delete, "Delete…", true)),
    ]
}

/// Room for the "..." button in a card header (the button itself is drawn by [`bank_menu`]
/// after the card's click area, so it sits on top and gets its own clicks).
const MENU_BTN: Vec2 = Vec2::new(30.0, 24.0);

/// The "..." menu drawn into `rect`; returns (clicked item, pointer over the button or menu).
fn bank_menu(ui: &mut Ui, rect: Rect, r: &BankRow) -> (Option<BankAct>, bool) {
    let mut act = None;
    let resp = ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(Layout::right_to_left(Align::Center)), |ui| {
        ui.spacing_mut().button_padding = Vec2::new(8.0, 3.0);
        ui.menu_button(RichText::new("…").size(14.0).color(theme::muted_col()), |ui| {
            ui.set_min_width(150.0);
            for it in bank_menu_items(r.nq) {
                match it {
                    None => {
                        ui.separator();
                    }
                    Some((a, label, enabled)) => {
                        let text = if a == BankAct::Delete { RichText::new(label).color(theme::bad()) } else { RichText::new(label) };
                        let b = ui.add_enabled(enabled, egui::Button::new(text));
                        let b = if a == BankAct::Export && !enabled { b.on_disabled_hover_text("Add a question first") } else { b };
                        if b.clicked() {
                            act = Some(a);
                            ui.close();
                        }
                    }
                }
            }
        })
    });
    let over = resp.inner.response.contains_pointer();
    (act, over)
}

fn apply_bank_menu(app: &mut App, ctx: &egui::Context, key: UiId, r: &BankRow, act: Option<BankAct>) {
    match act {
        Some(BankAct::Open) => app.open_bank(&r.id),
        Some(BankAct::Popover) => open_popover(ctx, &r.id),
        Some(BankAct::Export) => open_bank_export(ctx, &r.id),
        Some(BankAct::Rename) => with_state(ctx, key, |s: &mut BanksState| s.rename = Some((r.id.clone(), r.name.clone()))),
        Some(BankAct::Delete) => with_state(ctx, key, |s: &mut BanksState| s.confirm_delete = Some(r.id.clone())),
        None => {}
    }
}

fn counts_line(r: &BankRow) -> String {
    format!(
        "{} question{} · {} topic{}",
        r.nq,
        if r.nq == 1 { "" } else { "s" },
        r.topics,
        if r.topics == 1 { "" } else { "s" }
    )
}

/// One bank tile: serif name, counts, type-mix bar, edited time.
fn bank_card(ui: &mut Ui, app: &mut App, ctx: &egui::Context, key: UiId, r: &BankRow) {
    let mut link: Option<Rect> = None;
    let mut menu_rect = Rect::NOTHING;
    let resp = kit::card().inner_margin(Margin::same(14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(theme::serif(truncate(&r.name, 30)).size(18.0));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                menu_rect = ui.allocate_exact_size(MENU_BTN, Sense::hover()).0;
            });
        });
        ui.label(theme::muted(counts_line(r)).size(12.5));
        ui.add_space(8.0);
        paint_mix_bar(ui, ui.available_width(), &r.mix);
        ui.add_space(8.0);
        link = Some(usage_link(ui, r.used_in.len(), 12.0).rect);
        ui.add_space(2.0);
        ui.label(theme::muted(format!("Edited {}", rel_time(r.modified))).size(11.5));
    });
    let click = ui.interact(resp.response.rect, ui.id().with(("bank_card", &r.id)), Sense::click());
    let (menu_act, over_menu) = bank_menu(ui, menu_rect, r);
    if menu_act.is_some() {
        apply_bank_menu(app, ctx, key, r, menu_act);
        return;
    }
    if usage_clicked(&click, link, r) {
        open_usage(ctx, r.name.clone(), r.used_in.clone());
        return;
    }
    if click.hovered() {
        ui.painter().rect_stroke(
            resp.response.rect,
            CornerRadius::same(kit::R_CARD),
            Stroke::new(1.0, theme::accent_col()),
            egui::StrokeKind::Inside,
        );
    }
    if click.clicked() && !over_menu {
        app.open_bank(&r.id);
    }
}

/// True when the card click landed on its "Used in N exams" link (and there are exams).
fn usage_clicked(click: &egui::Response, link: Option<Rect>, r: &BankRow) -> bool {
    click.clicked() && !r.used_in.is_empty() && link.zip(click.interact_pointer_pos()).is_some_and(|(l, p)| l.expand(2.0).contains(p))
}

fn bank_list_row(ui: &mut Ui, app: &mut App, ctx: &egui::Context, key: UiId, r: &BankRow) {
    let mut link: Option<Rect> = None;
    let mut menu_rect = Rect::NOTHING;
    let resp = kit::card().inner_margin(Margin::symmetric(14, 9)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let total = ui.available_width();
            ui.allocate_ui(Vec2::new((total * 0.38).floor(), 22.0), |ui| {
                ui.label(theme::serif(truncate(&r.name, 40)).size(16.0));
            });
            ui.allocate_ui(Vec2::new((total * 0.27).floor(), 22.0), |ui| {
                ui.label(theme::muted(counts_line(r)).size(12.5));
            });
            ui.allocate_ui(Vec2::new(90.0, 22.0), |ui| {
                paint_mix_bar(ui, 80.0, &r.mix);
            });
            ui.allocate_ui(Vec2::new(118.0, 22.0), |ui| {
                link = Some(usage_link(ui, r.used_in.len(), 12.0).rect);
            });
            ui.label(theme::muted(format!("Edited {}", rel_time(r.modified))).size(12.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                menu_rect = ui.allocate_exact_size(MENU_BTN, Sense::hover()).0;
            });
        });
    });
    let click = ui.interact(resp.response.rect, ui.id().with(("bank_row", &r.id)), Sense::click());
    let (menu_act, over_menu) = bank_menu(ui, menu_rect, r);
    if menu_act.is_some() {
        apply_bank_menu(app, ctx, key, r, menu_act);
        return;
    }
    if usage_clicked(&click, link, r) {
        open_usage(ctx, r.name.clone(), r.used_in.clone());
        return;
    }
    if click.hovered() {
        ui.painter().rect_stroke(
            resp.response.rect,
            CornerRadius::same(kit::R_CARD),
            Stroke::new(1.0, theme::accent_col()),
            egui::StrokeKind::Inside,
        );
    }
    if click.clicked() && !over_menu {
        app.open_bank(&r.id);
    }
}

/// Rename and delete-confirmation modals for the banks list.
fn draw_bank_overlays(app: &mut App, ctx: &egui::Context, key: UiId) {
    draw_bank_export(app, ctx);
    let st = with_state(ctx, key, |s: &mut BanksState| s.clone());

    // Rename.
    if let Some((id, ref rename_text)) = st.rename {
        let mut close = false;
        let mut commit = false;
        let mut commit_text = rename_text.clone();
        let modal = egui::Modal::new(UiId::new("testparrot_rename_bank")).show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.label(theme::serif("Rename bank").size(20.0));
            ui.add_space(6.0);
            let r = ui.add(TextEdit::singleline(&mut commit_text).desired_width(ui.available_width()));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                commit = true;
            }
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if kit::primary_button(ui, "Rename").clicked() {
                    commit = true;
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
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
            ui.label(theme::serif("Delete this bank?").size(20.0));
            ui.add_space(6.0);
            if let Some(b) = &bank {
                ui.label(
                    theme::text(format!(
                        "This permanently deletes \"{}\" and its {} question{}. It is removed from your \
                         machine, and exams stop being able to pull new copies of its questions.",
                        b.name,
                        b.questions.len(),
                        if b.questions.len() == 1 { "" } else { "s" },
                    ))
                    .size(12.5),
                );
            }
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        Button::new(RichText::new("Delete").family(theme::bold_family()).color(theme::on_accent_col()))
                            .fill(theme::bad())
                            .corner_radius(CornerRadius::same(255))
                            .min_size(Vec2::new(0.0, 34.0)),
                    )
                    .clicked()
                {
                    confirmed = true;
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
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

/// Saves a bank as the JSON question-bank file (asks where; the write goes through `rfd` + fs).
pub fn export_bank(app: &mut App, bank_id: &Id) {
    let Some(bank) = app.store.bank(bank_id).cloned() else {
        app.toast_err("That bank is no longer there.");
        return;
    };
    let path = rfd::FileDialog::new()
        .set_file_name(format!("{}.json", crate::bankexport::file_stem(&bank.name)))
        .add_filter("Question bank JSON", &["json"])
        .save_file();
    let Some(path) = path else { return };
    let doc = crate::bankexport::bank_data(&bank);
    let written = serde_json::to_string_pretty(&doc)
        .map_err(|e| e.to_string())
        .and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
    match written {
        Ok(()) => app.toast(format!("Exported to {}", path.display())),
        Err(e) => app.toast_err(format!("Export failed: {e}")),
    }
}

fn bank_export_key() -> UiId {
    UiId::new("qs_bank_export_dialog")
}

/// Opens the "Export question bank" dialog (preview, then PDF or JSON).
pub fn open_bank_export(ctx: &egui::Context, bank_id: &Id) {
    ctx.data_mut(|d| d.insert_temp(bank_export_key(), Some(bank_id.clone())));
}

/// The rendered bank file for the open dialog: which bank / revision, the PDF, its pages.
struct BankExportCache {
    bank: Id,
    modified: i64,
    pdf: Result<Vec<u8>, String>,
    pages: Vec<egui::TextureHandle>,
}

thread_local! {
    static BANK_EXPORT: std::cell::RefCell<Option<BankExportCache>> = const { std::cell::RefCell::new(None) };
}

/// Test Parrot's "Export Question Bank": a preview of the printable bank file (answers
/// included), then Download PDF, or the JSON file.
fn draw_bank_export(app: &mut App, ctx: &egui::Context) {
    if let Ok(v) = std::env::var("QUESHION_BANK_EXPORT") {
        let hid = UiId::new("qs_bank_export_hook");
        if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
            ctx.data_mut(|d| d.insert_temp(hid, true));
            let n: usize = v.trim().parse().unwrap_or(1);
            if let Some(b) = app.store.banks.get(n.saturating_sub(1)) {
                open_bank_export(ctx, &b.id.clone());
            }
        }
    }
    let Some(bank_id) = ctx.data(|d| d.get_temp::<Option<Id>>(bank_export_key())).flatten() else { return };
    let Some(bank) = app.store.bank(&bank_id).cloned() else {
        ctx.data_mut(|d| d.insert_temp::<Option<Id>>(bank_export_key(), None));
        return;
    };
    let fresh = BANK_EXPORT.with(|c| c.borrow().as_ref().is_some_and(|c| c.bank == bank_id && c.modified == bank.modified));
    if !fresh {
        let pdf = crate::bankexport::bank_pdf(&bank);
        let mut pages = Vec::new();
        if let Ok(bytes) = &pdf {
            if let Ok(pngs) = crate::screens::pdf_pages_png(bytes) {
                for (i, png) in pngs.iter().enumerate() {
                    if let Ok(img) = image::load_from_memory(png) {
                        let img = img.thumbnail(900, 1200).to_rgba8();
                        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                        pages.push(ctx.load_texture(format!("bank_export_page_{i}"), ci, egui::TextureOptions::LINEAR));
                    }
                }
            }
        }
        // QA hook: QUESHION_BANK_EXPORT_OUT=<file> also writes the PDF there.
        if let (Ok(out), Ok(bytes)) = (std::env::var("QUESHION_BANK_EXPORT_OUT"), &pdf) {
            let _ = std::fs::write(out, bytes);
        }
        BANK_EXPORT.with(|c| *c.borrow_mut() = Some(BankExportCache { bank: bank_id.clone(), modified: bank.modified, pdf, pages }));
    }
    let screen = ctx.content_rect();
    let w = 700.0_f32.min(screen.width() - 40.0);
    let mut close = false;
    let mut save_pdf = false;
    let mut save_json = false;
    let modal = egui::Modal::new(UiId::new("qs_bank_export_modal")).show(ctx, |ui| {
        ui.set_width(w);
        ui.label(theme::serif("Export question bank").size(22.0).color(theme::ink()));
        ui.add(egui::Label::new(theme::muted("For teachers: this file contains correct answers and suggested answers. It is separate from printing an exam.").size(12.5)).wrap());
        ui.add_space(8.0);
        BANK_EXPORT.with(|c| {
            let c = c.borrow();
            let Some(c) = c.as_ref() else { return };
            match &c.pdf {
                Err(e) => {
                    ui.label(RichText::new(format!("The PDF couldn't be made: {e}")).color(theme::bad()));
                }
                Ok(_) if c.pages.is_empty() => {
                    ui.label(theme::muted("Preview unavailable (pdftoppm is not installed); the PDF itself is ready.").size(12.5));
                }
                Ok(_) => {
                    Frame::new().fill(theme::field()).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical().id_salt("bank_export_preview").max_height((screen.height() - 260.0).max(240.0)).auto_shrink([false, false]).show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                for t in &c.pages {
                                    let sz = t.size_vec2();
                                    let k = ((ui.available_width() - 8.0) / sz.x).min(1.0);
                                    ui.add(egui::Image::new(t).fit_to_exact_size(sz * k));
                                    ui.add_space(8.0);
                                }
                            });
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(theme::muted(format!(
                        "{} · {} question{} · the bank's data rides along inside the PDF, so it imports back as it is",
                        plural_pages(c.pages.len()),
                        bank.questions.len(),
                        if bank.questions.len() == 1 { "" } else { "s" }
                    )).size(11.5));
                }
            }
        });
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| {
            if kit::ghost_button(ui, "Save JSON file…").on_hover_text("The bank as a question-bank JSON file (for other apps and backups)").clicked() {
                save_json = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ok = BANK_EXPORT.with(|c| c.borrow().as_ref().is_some_and(|c| c.pdf.is_ok()));
                if ui.add_enabled_ui(ok, |ui| kit::primary_button(ui, "Download PDF")).inner.clicked() {
                    save_pdf = true;
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
    });
    if save_json {
        export_bank(app, &bank_id);
    }
    if save_pdf {
        let path = rfd::FileDialog::new()
            .set_file_name(format!("{}.pdf", crate::bankexport::file_stem(&bank.name)))
            .add_filter("PDF", &["pdf"])
            .save_file();
        if let Some(path) = path {
            let bytes = BANK_EXPORT.with(|c| c.borrow().as_ref().and_then(|c| c.pdf.as_ref().ok().cloned()));
            match bytes.map(|b| std::fs::write(&path, b)) {
                Some(Ok(())) => {
                    app.toast(format!("Saved {}", path.display()));
                    close = true;
                }
                Some(Err(e)) => app.toast_err(format!("Couldn't save: {e}")),
                None => {}
            }
        }
    }
    if close || modal.should_close() {
        ctx.data_mut(|d| d.insert_temp::<Option<Id>>(bank_export_key(), None));
        BANK_EXPORT.with(|c| *c.borrow_mut() = None);
    }
}

fn plural_pages(n: usize) -> String {
    format!("{n} page{}", if n == 1 { "" } else { "s" })
}

/// The exported bank document. Details travel only when the teacher entered some.
pub(crate) fn bank_export_json(bank: &crate::model::QuestionBank) -> serde_json::Value {
    let mut doc = serde_json::json!({
        "kind": "test-parrot-question-bank",
        "version": 1,
        "name": bank.name,
        "questions": bank.questions,
    });
    if !bank.details.is_empty() {
        doc["details"] = serde_json::to_value(bank.details.trimmed()).unwrap_or_default();
    }
    doc
}

/// "Where your work is stored" info popup; clears its flag on Close/backdrop/Esc.
impl App {
    pub(crate) fn storage_info_modal(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let modal = egui::Modal::new(UiId::new("testparrot_storage_info")).show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.label(theme::serif("Where your work is stored").size(20.0));
            ui.add_space(6.0);
            ui.label(
                theme::text(
                    "Every bank, exam and export lives on your machine, in plain files you can back up:",
                )
                .size(12.5),
            );
            ui.add_space(6.0);
            Frame::new()
                .fill(theme::field())
                .corner_radius(CornerRadius::same(kit::R_FIELD))
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
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if kit::primary_button(ui, "Close").clicked() {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            self.storage_info = false;
        }
    }
}

// ---------- bank editor: outline panel (types + topics) ----------

/// One outline row: label left, count right; filled when `on`. Leaves `right_pad` free
/// at the right edge (for a menu button). Returns the row's click response.
fn side_row(ui: &mut Ui, label: &str, n: usize, on: bool, right_pad: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, CornerRadius::same(8), theme::accent_faded());
        p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, theme::accent_col().gamma_multiply(0.45)), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(8), theme::hover_bg());
    }
    let fg = if on { theme::accent_col() } else { theme::text_col() };
    let font = egui::FontId::new(12.5, if on { theme::bold_family() } else { egui::FontFamily::Proportional });
    let cg = p.layout_no_wrap(n.to_string(), egui::FontId::new(11.5, theme::bold_family()), theme::muted_col());
    let cx = rect.right() - 10.0 - right_pad;
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), font, fg);
    job.wrap.max_width = (cx - cg.size().x - 8.0 - (rect.left() + 10.0)).max(20.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let lg = p.layout_job(job);
    p.galley(Pos2::new(rect.left() + 10.0, rect.center().y - lg.size().y / 2.0), lg, fg);
    p.galley(Pos2::new(cx - cg.size().x, rect.center().y - cg.size().y / 2.0), cg, theme::muted_col());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Left panel of the bank editor: Outline (all + per type, counts) and Topics (counts,
/// click to filter, "…" menu to rename / merge / delete across the bank).
fn outline_panel(ui: &mut Ui, ctx: &egui::Context, key: UiId, questions: &[Question]) {
    let st = with_state(ctx, key, |s: &mut BankEditState| s.clone());
    ui.spacing_mut().item_spacing.y = 2.0;
    kit::small_caps(ui, "Outline");
    ui.add_space(4.0);
    if side_row(ui, "All questions", questions.len(), st.kind.is_none() && st.topic.is_none(), 0.0).clicked() {
        with_state(ctx, key, |s: &mut BankEditState| {
            s.kind = None;
            s.topic = None;
        });
    }
    let counts = crate::bankops::type_counts(questions);
    for (k, n) in crate::bankops::KINDS.iter().copied().zip(counts) {
        if n == 0 && st.kind != Some(k) {
            continue;
        }
        let on = st.kind == Some(k);
        if side_row(ui, k.label(), n, on, 0.0).clicked() {
            with_state(ctx, key, |s: &mut BankEditState| s.kind = if on { None } else { Some(k) });
        }
    }
    ui.add_space(16.0);
    kit::small_caps(ui, "Topics");
    ui.add_space(4.0);
    let tc = crate::bankops::topic_counts(questions);
    if tc.is_empty() {
        ui.add(
            egui::Label::new(theme::muted("No topics yet. Topics added to a question are listed here.").size(12.0)).wrap(),
        );
        return;
    }
    for (t, n) in &tc {
        let on = st.topic.as_deref() == Some(t.as_str());
        let resp = side_row(ui, t, *n, on, 18.0);
        // "…" menu, placed over the row's right edge (added last, so it gets its clicks).
        let mrect = Rect::from_center_size(Pos2::new(resp.rect.right() - 13.0, resp.rect.center().y), Vec2::new(20.0, 22.0));
        let mut act: Option<TopicMode> = None;
        // A child ui that takes no room in the list (the row already has it).
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(mrect).layout(Layout::left_to_right(Align::Center)));
        child.style_mut().visuals.button_frame = false;
        let over_menu = {
            let ui = &mut child;
            {
                ui.menu_button(RichText::new("…").size(14.0).color(if on { theme::accent_col() } else { theme::muted_col() }), |ui| {
                    if ui.button("Rename…").clicked() {
                        act = Some(TopicMode::Rename);
                        ui.close();
                    }
                    if tc.len() > 1 && ui.button("Merge into…").clicked() {
                        act = Some(TopicMode::Merge);
                        ui.close();
                    }
                    if ui.button("Delete…").clicked() {
                        act = Some(TopicMode::Delete);
                        ui.close();
                    }
                })
                .response
                .contains_pointer()
            }
        };
        if let Some(mode) = act {
            let text = if mode == TopicMode::Rename { t.clone() } else { String::new() };
            with_state(ctx, key, |s: &mut BankEditState| s.topic_edit = Some(TopicEdit { from: t.clone(), mode, text }));
        } else if resp.clicked() && !over_menu {
            let sel = if on { None } else { Some(t.clone()) };
            with_state(ctx, key, |s: &mut BankEditState| s.topic = sel);
        }
    }
}

/// Rename / merge / delete a topic on every question of the bank.
fn draw_topic_dialog(app: &mut App, ctx: &egui::Context, key: UiId, bank_id: &Id) {
    let Some(mut te) = with_state(ctx, key, |s: &mut BankEditState| s.topic_edit.clone()) else {
        return;
    };
    let Some(bank) = app.store.bank(bank_id) else { return };
    let tc = crate::bankops::topic_counts(&bank.questions);
    let n_from = tc.iter().find(|(t, _)| *t == te.from).map(|(_, n)| *n).unwrap_or(0);
    let others: Vec<String> = tc.iter().map(|(t, _)| t.clone()).filter(|t| *t != te.from).collect();
    let qs = |n: usize| format!("{n} question{}", if n == 1 { "" } else { "s" });

    let mut close = false;
    let mut apply = false;
    let target = crate::bankops::canonical_topic(&others, &te.text);
    let merging = others.contains(&target);
    let modal = egui::Modal::new(UiId::new("bank_topic_dialog")).show(ctx, |ui| {
        ui.set_min_width(400.0);
        ui.set_max_width(440.0);
        match te.mode {
            TopicMode::Rename => {
                ui.label(theme::serif("Rename topic").size(20.0));
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        theme::text(format!("“{}” is on {}. The new name replaces it on all of them.", te.from, qs(n_from))).size(12.5),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);
                // Read Enter before the field runs (it may consume the key when it hands back focus).
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let r = ui.add(TextEdit::singleline(&mut te.text).desired_width(ui.available_width()).hint_text("New topic name"));
                // Focus the field once when the dialog opens.
                let fid = UiId::new("bank_topic_dialog_focused");
                if !ui.ctx().data(|d| d.get_temp::<bool>(fid).unwrap_or(false)) {
                    ui.ctx().data_mut(|d| d.insert_temp(fid, true));
                    r.request_focus();
                }
                if enter && (r.lost_focus() || r.has_focus()) {
                    apply = true;
                }
                let sugg = crate::bankops::suggest_topics(&others, &te.text, &[], 6);
                if !sugg.is_empty() && te.text.trim() != te.from {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        ui.label(theme::muted("Existing:").size(11.5));
                        for t in sugg {
                            if kit::chip_toggle(ui, &t, false).clicked() {
                                te.text = t.clone();
                            }
                        }
                    });
                }
                if merging {
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            theme::muted(format!("“{target}” already exists, so the two topics merge into one.")).size(12.0),
                        )
                        .wrap(),
                    );
                }
            }
            TopicMode::Merge => {
                ui.label(theme::serif(format!("Merge “{}” into…", te.from)).size(20.0));
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        theme::text(format!("Its {} {} to the topic you pick; “{}” goes away.", qs(n_from), if n_from == 1 { "moves" } else { "move" }, te.from)).size(12.5),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for (t, n) in tc.iter().filter(|(t, _)| *t != te.from) {
                        let on = te.text == *t;
                        if side_row(ui, t, *n, on, 0.0).clicked() {
                            te.text = t.clone();
                        }
                    }
                });
            }
            TopicMode::Delete => {
                ui.label(theme::serif(format!("Delete topic “{}”?", te.from)).size(20.0));
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        theme::text(format!(
                            "It is removed from {}. {}",
                            qs(n_from),
                            if n_from == 1 { "The question itself stays." } else { "The questions themselves stay." }
                        )).size(12.5),
                    )
                    .wrap(),
                );
            }
        }
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let ok = match te.mode {
                TopicMode::Rename => !te.text.trim().is_empty() && target != te.from,
                TopicMode::Merge => others.contains(&te.text),
                TopicMode::Delete => true,
            };
            let label = match te.mode {
                TopicMode::Rename if merging => "Merge",
                TopicMode::Rename => "Rename",
                TopicMode::Merge => "Merge",
                TopicMode::Delete => "Delete topic",
            };
            let clicked = if te.mode == TopicMode::Delete {
                ui.add(
                    Button::new(RichText::new(label).family(theme::bold_family()).color(theme::on_accent_col()))
                        .fill(theme::bad())
                        .corner_radius(CornerRadius::same(255))
                        .min_size(Vec2::new(0.0, 34.0)),
                )
                .clicked()
            } else {
                ui.add_enabled_ui(ok, |ui| kit::primary_button(ui, label)).inner.clicked()
            };
            if clicked && ok {
                apply = true;
            }
            if kit::ghost_button(ui, "Cancel").clicked() {
                close = true;
            }
        });
    });

    if apply {
        let to = if te.mode == TopicMode::Merge { te.text.clone() } else { target.clone() };
        let valid = match te.mode {
            TopicMode::Rename => !to.trim().is_empty() && to != te.from,
            TopicMode::Merge => others.contains(&to),
            TopicMode::Delete => true,
        };
        if valid {
            let t = now_secs();
            let mut n = 0;
            if let Some(b) = app.store.bank_mut(bank_id) {
                n = match te.mode {
                    TopicMode::Delete => crate::bankops::delete_topic(&mut b.questions, &te.from),
                    _ => crate::bankops::rename_topic(&mut b.questions, &te.from, &to),
                };
                b.modified = t;
            }
            // An open draft carries its own topic list: keep it in step.
            if let Some(d) = app.qedit.as_mut().filter(|d| &d.bank_id == bank_id) {
                let mut one = vec![Question { topics: d.topics.clone(), ..placeholder_question() }];
                match te.mode {
                    TopicMode::Delete => crate::bankops::delete_topic(&mut one, &te.from),
                    _ => crate::bankops::rename_topic(&mut one, &te.from, &to),
                };
                d.topics = one.remove(0).topics;
            }
            app.store.save_banks();
            let from = te.from.clone();
            with_state(ctx, key, |s: &mut BankEditState| {
                if s.topic.as_deref() == Some(from.as_str()) {
                    s.topic = if te.mode == TopicMode::Delete { None } else { Some(to.clone()) };
                }
            });
            let qn = qs(n);
            app.toast(match te.mode {
                TopicMode::Delete => format!("Removed “{from}” from {qn}"),
                TopicMode::Merge => format!("Merged “{from}” into “{to}” on {qn}"),
                TopicMode::Rename if merging => format!("Merged “{from}” into “{to}” on {qn}"),
                TopicMode::Rename => format!("Renamed “{from}” to “{to}” on {qn}"),
            });
            close = true;
        }
    }
    if close || modal.should_close() {
        ctx.data_mut(|d| d.remove_temp::<bool>(UiId::new("bank_topic_dialog_focused")));
        with_state(ctx, key, |s: &mut BankEditState| s.topic_edit = None);
    } else {
        with_state(ctx, key, |s: &mut BankEditState| s.topic_edit = Some(te));
    }
}

/// A throwaway question (only `topics` matters) for running bankops on a draft's list.
fn placeholder_question() -> Question {
    Question {
        id: String::new(),
        stem: String::new(),
        kind: crate::model::Kind::Short { sample: String::new() },
        points: None,
        difficulty: None,
        topics: Vec::new(),
        image: None,
        explanation: None,
        figure: None,
        created: 0,
        modified: 0,
    }
}

// ---------- question cards ----------

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

fn paint_chevron(ui: &mut Ui, open: bool) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
    let s = Stroke::new(1.8, theme::muted_col());
    let c = r.center();
    let (a, b) = if open { (2.0, -2.0) } else { (-2.0, 2.0) };
    ui.painter().line_segment([c + Vec2::new(-4.5, a), c + Vec2::new(0.0, b)], s);
    ui.painter().line_segment([c + Vec2::new(0.0, b), c + Vec2::new(4.5, a)], s);
}

fn meta_chip(ui: &mut Ui, text: &str) {
    kit::pill(ui, text, theme::field(), theme::text_col());
}

/// A fill-in-the-blank key as one line: each blank's accepted answers joined by " / ",
/// blanks separated by "; " (Test Parrot prints "14; 10").
pub(crate) fn blank_answer_text(accept: &[Vec<String>]) -> String {
    accept
        .iter()
        .map(|alts| alts.iter().map(|a| a.trim()).filter(|a| !a.is_empty()).collect::<Vec<_>>().join(" / "))
        .map(|t| if t.is_empty() { "—".to_string() } else { t })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Dashed "Picture needed" stand-in for a `[picture]` marker on a bank card.
fn card_picture_box(ui: &mut Ui, alt: Option<&str>, size: f32) {
    let w = ui.available_width().clamp(120.0, 520.0);
    let h = size * 3.4;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter();
    let red = theme::red();
    p.rect_filled(rect, CornerRadius::same(6), red.gamma_multiply(0.08));
    p.extend(egui::Shape::dashed_line(
        &[rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()],
        Stroke::new(1.0, red),
        4.0,
        3.0,
    ));
    let alt = alt.unwrap_or("").trim();
    let cy = if alt.is_empty() { rect.center().y } else { rect.center().y - size * 0.55 };
    p.text(Pos2::new(rect.center().x, cy), egui::Align2::CENTER_CENTER, "Picture needed", egui::FontId::new(size * 0.9, theme::bold_family()), red);
    if !alt.is_empty() {
        let max = ((w / (size * 0.42)) as usize).max(12);
        p.text(
            Pos2::new(rect.center().x, cy + size * 1.15),
            egui::Align2::CENTER_CENTER,
            truncate(alt, max),
            egui::FontId::new(size * 0.75, theme::serif_family()),
            theme::muted_col(),
        );
    }
    ui.add_space(4.0);
}

/// Rich text with its `[picture]` markers drawn as "Picture needed" boxes; the question's
/// attached picture stands in for the first marker (`picture` is taken once used).
fn card_rich(ui: &mut Ui, app: &mut App, text: &str, rich: &crate::equation::Rich, picture: &mut Option<crate::model::ImageRef>) {
    use crate::bankops::{split_pictures, StemPart};
    for part in split_pictures(text) {
        match part {
            StemPart::Text(t) => {
                crate::equation::rich_ui(ui, &t, rich);
            }
            StemPart::Picture(alt) => match picture.take() {
                Some(img) => card_image(ui, app, &img),
                None => card_picture_box(ui, alt.as_deref(), rich.size),
            },
        }
    }
}

fn card_image(ui: &mut Ui, app: &mut App, img: &crate::model::ImageRef) {
    let ctx = ui.ctx().clone();
    if let Some(tex) = texture_for(app, &ctx, img) {
        let max_w = ui.available_width().min(360.0);
        let sz = tex.size_vec2();
        let k = (max_w / sz.x.max(1.0)).min(1.0).min(220.0 / sz.y.max(1.0));
        ui.add_space(3.0);
        ui.add(egui::Image::new(&tex).fit_to_exact_size(sz * k));
        ui.add_space(3.0);
    } else {
        card_picture_box(ui, Some("picture file missing"), 13.0);
    }
}

/// "SUGGESTED ANSWER"-style block: small caps title, left rule, content.
fn answer_block(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(5.0);
    let out = Frame::new().inner_margin(Margin { left: 12, right: 0, top: 1, bottom: 2 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(kit::small_caps_text(title));
        ui.add_space(2.0);
        add(ui);
    });
    let r = out.response.rect;
    ui.painter().vline(r.left() + 1.0, r.y_range(), Stroke::new(2.0, theme::border()));
}

/// Choices with the correct ones marked (green, bold letter, check mark), as Test Parrot's
/// bank list shows them.
fn card_choices(ui: &mut Ui, app: &mut App, answers: &[crate::model::Answer], size: f32, indent: f32) {
    use crate::equation::Rich;
    for (i, a) in answers.iter().enumerate() {
        let letter = format!("{}. ", char::from(b'A' + (i % 26) as u8));
        ui.horizontal_top(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| {
                let col = if a.correct { theme::green() } else { theme::text_col() };
                let mut rich = Rich::new(size, col).lead(letter.clone(), size, if a.correct { theme::green() } else { theme::ink() });
                if a.correct {
                    rich.family = rich.bold_family.clone();
                }
                // The check mark follows the choice's last line ("A. Ca ✓").
                let text = if a.correct { format!("{} ✓", a.text.trim_end()) } else { a.text.clone() };
                let mut none = None;
                card_rich(ui, app, &text, &rich, &mut none);
            });
        });
    }
}

/// The whole question on a collapsed bank card (Test Parrot's bank list): the stem as it
/// prints (rich, pictures), choices with the correct ones marked, the fill-in answers, the
/// suggested answer, matching pairs, every multipart part with its own answers, and the
/// teacher's explanation.
fn card_full_question(ui: &mut Ui, app: &mut App, q: &Question, size: f32) {
    use crate::equation::Rich;
    use crate::model::{Kind, PartKind};
    let rich = Rich::new(size, theme::ink());
    let mut picture = q.image.clone();
    card_rich(ui, app, &q.stem, &rich, &mut picture);
    if let Some(img) = picture.take() {
        card_image(ui, app, &img);
    }
    let small = size - 1.0;
    let tf = |ui: &mut Ui, correct: bool, indent: f32| {
        ui.horizontal(|ui| {
            ui.add_space(indent);
            for (v, label) in [(true, "True"), (false, "False")] {
                let on = v == correct;
                let t = RichText::new(if on { format!("{label} ✓") } else { label.to_string() }).size(size);
                ui.label(if on { t.color(theme::green()).family(theme::bold_family()) } else { t.color(theme::text_col()) });
                ui.add_space(10.0);
            }
        });
    };
    match &q.kind {
        Kind::Mc { answers } => {
            ui.add_space(3.0);
            card_choices(ui, app, answers, size, 2.0);
        }
        Kind::Tf { correct } => {
            ui.add_space(3.0);
            tf(ui, *correct, 2.0);
        }
        Kind::Matching { pairs, distractors } => {
            answer_block(ui, "MATCHES", |ui| {
                for (i, p) in pairs.iter().enumerate() {
                    let left = crate::equation::stem_preview(&p.left);
                    let right = crate::equation::stem_preview(&p.right);
                    ui.add(egui::Label::new(theme::text(format!("{}. {left}  →  {right}", i + 1)).size(small)).wrap());
                }
                if !distractors.is_empty() {
                    let d: Vec<String> = distractors.iter().map(|d| crate::equation::stem_preview(d)).collect();
                    ui.add(egui::Label::new(theme::muted(format!("Also listed: {}", d.join(", "))).size(small - 1.0)).wrap());
                }
            });
        }
        Kind::Blank { accept } => {
            if !accept.is_empty() {
                answer_block(ui, "ANSWERS", |ui| {
                    ui.add(egui::Label::new(theme::text(blank_answer_text(accept)).size(small)).wrap());
                });
            }
        }
        Kind::Short { sample } => {
            if !sample.trim().is_empty() {
                answer_block(ui, "SUGGESTED ANSWER", |ui| {
                    let mut none = None;
                    card_rich(ui, app, sample, &Rich::new(small, theme::text_col()), &mut none);
                });
            }
        }
        Kind::Multipart { parts } => {
            for (pi, p) in parts.iter().enumerate() {
                ui.add_space(5.0);
                let label = part_label(pi, &p.stem);
                let r = Rich::new(size, theme::ink());
                let r = if label.is_empty() { r } else { r.lead(label, size, theme::ink()) };
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        let mut none = None;
                        card_rich(ui, app, &p.stem, &r, &mut none);
                        match &p.kind {
                            PartKind::Mc { answers } => card_choices(ui, app, answers, small, 14.0),
                            PartKind::Tf { correct } => tf(ui, *correct, 14.0),
                            PartKind::Short { sample } => {
                                if !sample.trim().is_empty() {
                                    answer_block(ui, "SUGGESTED ANSWER", |ui| {
                                        let mut none = None;
                                        card_rich(ui, app, sample, &Rich::new(small, theme::text_col()), &mut none);
                                    });
                                }
                            }
                        }
                    });
                });
            }
        }
    }
    if let Some(e) = q.explanation.as_deref().filter(|e| !e.trim().is_empty()) {
        answer_block(ui, "EXPLANATION", |ui| {
            let mut none = None;
            card_rich(ui, app, e, &Rich::new(small, theme::text_col()), &mut none);
        });
    }
}

/// One question card. `q == None` is the unsaved new-question draft (always open).
#[allow(clippy::too_many_arguments)]
fn draw_card(
    ui: &mut Ui,
    app: &mut App,
    ctx: &egui::Context,
    key: UiId,
    bank_id: &str,
    width: f32,
    q: Option<&Question>,
    n: usize,
    list: bool,
    pool: &[String],
    scroll_to: bool,
) {
    let open = match q {
        None => true,
        Some(q) => app.qedit.as_ref().and_then(|d| d.qid.as_deref()) == Some(q.id.as_str()),
    };
    let mut toggled = false;
    let used = q
        .and_then(|q| ctx.data(|d| d.get_temp::<std::collections::HashMap<Id, usize>>(key.with("usage"))).and_then(|m| m.get(&q.id).copied()))
        .unwrap_or(0);
    let mut usage_rect: Option<Rect> = None;
    let mut copy_rect: Option<Rect> = None;

    let frame = kit::card()
        .stroke(Stroke::new(1.0, if open { theme::accent_col() } else { theme::border() }));
    let out = frame.show(ui, |ui| {
        ui.set_width(width - 2.0);

        // ----- header (the click target) -----
        let hdr = Frame::new().inner_margin(Margin::symmetric(13, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let kind = q
                .map(|q| q.kind.qkind())
                .or_else(|| app.qedit.as_ref().map(|d| d.qkind()))
                .unwrap_or(QKind::Mc);
            if list && !open {
                let q = q.unwrap();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.allocate_ui(Vec2::new(54.0, 18.0), |ui| kit::type_pill(ui, qedit::type_tag(kind)));
                    ui.label(RichText::new(format!("Q{n}")).family(theme::bold_family()).size(11.5).color(theme::muted_col()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        paint_chevron(ui, false);
                        copy_rect = Some(copy_icon(ui));
                        if let Some(p) = points_label(q.points) {
                            ui.label(RichText::new(p).family(theme::bold_family()).size(12.0).color(theme::ink()));
                        }
                        if used > 0 {
                            let r = ui.add(
                                egui::Label::new(
                                    RichText::new(format!("in {used} exam{}", if used == 1 { "" } else { "s" }))
                                        .size(11.5)
                                        .color(theme::accent_col())
                                        .underline(),
                                )
                                .sense(Sense::hover()),
                            );
                            usage_rect = Some(r.rect);
                        }
                        if let Some(d) = q.difficulty {
                            ui.label(theme::muted(diff_label(d)).size(11.5));
                        }
                        if let Some(t) = q.topics.iter().map(|t| t.trim()).find(|t| !t.is_empty()) {
                            meta_chip(ui, &truncate(t, 16));
                        }
                    });
                });
                ui.add_space(4.0);
                card_full_question(ui, app, q, 14.0);
            } else {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    kit::type_pill(ui, qedit::type_tag(kind));
                    match q {
                        Some(_) => {
                            ui.label(RichText::new(format!("Q{n}")).family(theme::bold_family()).size(11.5).color(theme::muted_col()));
                        }
                        None => {
                            ui.label(theme::serif("New question").size(15.0));
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        paint_chevron(ui, open);
                        if q.is_some() && !open {
                            copy_rect = Some(copy_icon(ui));
                        }
                        if let Some(p) = q.and_then(|q| points_label(q.points)) {
                            ui.label(RichText::new(p).family(theme::bold_family()).size(12.0).color(theme::ink()));
                        }
                    });
                });
                if let (false, Some(q)) = (open, q) {
                    ui.add_space(5.0);
                    card_full_question(ui, app, q, 14.0);
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                        for t in q.topics.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).take(2) {
                            meta_chip(ui, &truncate(t, 18));
                        }
                        if let Some(d) = q.difficulty {
                            ui.label(theme::muted(diff_label(d)).size(11.5));
                        }
                        if used > 0 {
                            usage_rect = Some(usage_link(ui, used, 11.5).rect);
                        }
                    });
                }
            }
        });
        let hid = ui.id().with(("qhdr", q.map(|q| q.id.as_str()).unwrap_or("<new>")));
        let hr = ui.interact(hdr.response.rect, hid, Sense::click());
        if hr.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if let Some(q) = q {
            hr.context_menu(|ui| {
                if ui.button("Duplicate").clicked() {
                    duplicate_question(app, bank_id, &q.id);
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    if open {
                        app.qedit = None;
                    }
                    delete_question(app, ctx, key, bank_id, &q.id);
                    ui.close();
                }
            });
        }
        if hr.clicked() {
            let on_usage = usage_rect.zip(hr.interact_pointer_pos()).is_some_and(|(r, p)| r.expand(2.0).contains(p));
            let on_copy = copy_rect.zip(hr.interact_pointer_pos()).is_some_and(|(r, p)| r.contains(p));
            match q {
                Some(q) if on_copy => duplicate_question(app, bank_id, &q.id),
                Some(q) if on_usage => {
                    let ids: Vec<Id> = crate::bankops::exams_using_question(&app.store.exams, bank_id, &q.id).iter().map(|e| e.id.clone()).collect();
                    open_usage(ctx, format!("Q{n} · {}", truncate(&crate::equation::stem_preview(&q.stem), 40)), ids);
                }
                _ => toggled = true,
            }
        }
        if open && scroll_to {
            hr.scroll_to_me(Some(Align::TOP));
        }

        // ----- expanded editor body -----
        if open {
            let y = ui.cursor().top();
            ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::rule()));
            let mut action = qedit::QAction::None;
            let mut draft = app.qedit.take();
            if let Some(d) = draft.as_mut() {
                Frame::new()
                    .inner_margin(Margin { left: 16, right: 16, top: 14, bottom: 12 })
                    .show(ui, |ui| {
                        let mut textures = std::mem::take(&mut app.textures);
                        action = qedit::draw_inline(ui, d, pool, &mut textures);
                        app.textures = textures;
                    });
            }
            match (action, draft) {
                (qedit::QAction::Done, Some(d)) => app.commit_draft(d),
                (qedit::QAction::Cancel, _) => {}
                (qedit::QAction::Delete, Some(d)) => {
                    if let Some(qid) = d.qid.clone() {
                        delete_question(app, ctx, key, bank_id, &qid);
                    }
                }
                (_, d) => app.qedit = d,
            }
        }
    });

    if !open && out.response.hovered() {
        ui.painter().rect_stroke(
            out.response.rect,
            CornerRadius::same(kit::R_CARD),
            Stroke::new(1.0, theme::accent_col()),
            egui::StrokeKind::Inside,
        );
    }

    if toggled {
        if open {
            // Collapse = Cancel.
            app.qedit = None;
        } else if let Some(q) = q {
            app.qedit = Some(qedit::Draft::from_question(bank_id.to_string(), q));
        }
    }
}

/// Copies question `qid` of `bank` (Test Parrot's Copy): same content under a fresh id
/// and fresh timestamps, inserted right after the original. Returns the copy's id.
pub(crate) fn duplicate_in_bank(bank: &mut crate::model::QuestionBank, qid: &str) -> Option<Id> {
    let idx = bank.questions.iter().position(|x| x.id == qid)?;
    let now = now_secs();
    let copy = Question { id: crate::model::new_id(), created: now, modified: now, ..bank.questions[idx].clone() };
    let id = copy.id.clone();
    bank.questions.insert(idx + 1, copy);
    bank.modified = now;
    Some(id)
}

/// Duplicates a stored question in place (bank card Copy button / right-click menu).
fn duplicate_question(app: &mut App, bank_id: &str, qid: &str) {
    let done = app.store.bank_mut(&bank_id.to_string()).and_then(|b| duplicate_in_bank(b, qid)).is_some();
    if done {
        app.store.save_banks();
    }
}

/// The card's Copy affordance: a small two-sheets icon. Only paints and reports its rect;
/// the header's click handler routes a click inside it (the header owns the clicks).
fn copy_icon(ui: &mut Ui) -> Rect {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::hover());
    let hot = resp.hovered();
    let col = if hot { theme::accent_col() } else { theme::muted_col() };
    if hot {
        ui.painter().rect_filled(r, CornerRadius::same(5), theme::accent_faded());
    }
    let s = Stroke::new(1.3, col);
    let c = r.center();
    let back = Rect::from_center_size(c + Vec2::new(-1.8, -1.8), Vec2::new(8.0, 9.5));
    let front = Rect::from_center_size(c + Vec2::new(1.8, 1.8), Vec2::new(8.0, 9.5));
    ui.painter().rect_stroke(back, CornerRadius::same(1), s, egui::StrokeKind::Middle);
    ui.painter().rect_filled(front, CornerRadius::same(1), theme::card());
    ui.painter().rect_stroke(front, CornerRadius::same(1), s, egui::StrokeKind::Middle);
    resp.on_hover_text("Copy question");
    r
}

/// Removes a stored question and parks it in the Undo strip.
fn delete_question(app: &mut App, ctx: &egui::Context, key: UiId, bank_id: &str, qid: &str) {
    let mut removed: Option<(Question, usize)> = None;
    if let Some(bank) = app.store.bank_mut(&bank_id.to_string()) {
        if let Some(idx) = bank.questions.iter().position(|x| x.id == qid) {
            removed = Some((bank.questions.remove(idx), idx));
            bank.modified = now_secs();
        }
    }
    if let Some(r) = removed {
        app.store.save_banks();
        with_state(ctx, key, |s: &mut BankEditState| s.last_deleted = Some(r));
    }
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

// ---------- bank details, rename pencil, Pop-over window ----------

/// Small borderless pencil icon button (the rename affordance after the bank name).
fn pencil_button(ui: &mut Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    let col = if resp.hovered() { theme::accent_col() } else { theme::muted_col() };
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), theme::hover_bg());
    }
    let c = rect.center();
    let s = Stroke::new(1.6, col);
    // Pencil: a slanted outline from the lower-left tip to the upper-right end, with a cap line.
    let u = Vec2::new(1.0, -1.0).normalized();
    let v = Vec2::new(1.0, 1.0).normalized();
    let tip = c - u * 7.5;
    let end = c + u * 7.5;
    let shoulder = tip + u * 3.5;
    let pts = vec![tip, shoulder + v * 2.6, end + v * 2.6, end - v * 2.6, shoulder - v * 2.6, tip];
    ui.painter().add(egui::Shape::line(pts, s));
    ui.painter().line_segment([end - u * 3.0 + v * 2.6, end - u * 3.0 - v * 2.6], s);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Writes edited details onto a bank; returns true when anything changed.
pub(crate) fn apply_details(bank: &mut crate::model::QuestionBank, draft: &crate::model::BankDetails, now: i64) -> bool {
    let d = draft.trimmed();
    if bank.details == d {
        return false;
    }
    bank.details = d;
    bank.modified = now;
    true
}

/// Test Parrot's "Question Bank details" dialog: description, declared author, license.
fn draw_details_dialog(app: &mut App, ctx: &egui::Context, key: UiId, bank_id: &Id) {
    let Some(mut d) = with_state(ctx, key, |s: &mut BankEditState| s.details.clone()) else {
        return;
    };
    let mut close = false;
    let mut save = false;
    let modal = egui::Modal::new(UiId::new("bank_details_dialog")).show(ctx, |ui| {
      Frame::new().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
        ui.set_min_width(520.0);
        ui.set_max_width(560.0);
        ui.label(theme::serif("Question bank details").size(22.0));
        ui.add_space(4.0);
        ui.label(theme::text("Only details you enter are included when this bank is exported and shared.").size(12.5));
        ui.add_space(10.0);
        let w = ui.available_width();
        let field = |ui: &mut Ui, label: &str| {
            ui.label(theme::strong(label).size(13.0));
            ui.add_space(2.0);
        };
        field(ui, "Description");
        ui.add(TextEdit::multiline(&mut d.description).desired_rows(4).desired_width(w).hint_text("What these questions cover, grade level, …"));
        ui.add_space(8.0);
        field(ui, "Declared author");
        ui.add(TextEdit::singleline(&mut d.author).desired_width(w));
        ui.add_space(8.0);
        field(ui, "License name");
        ui.add(TextEdit::singleline(&mut d.license_name).desired_width(w).hint_text("e.g. CC BY 4.0"));
        ui.add_space(8.0);
        field(ui, "License URL");
        ui.add(TextEdit::singleline(&mut d.license_url).desired_width(w).hint_text("https://"));
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(6.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if kit::primary_button(ui, "Save details").clicked() {
                save = true;
            }
            if kit::ghost_button(ui, "Cancel").clicked() {
                close = true;
            }
        });
      });
    });
    if save {
        let changed = app.store.bank_mut(bank_id).is_some_and(|b| apply_details(b, &d, now_secs()));
        if changed {
            app.store.save_banks();
            app.toast("Details saved");
        }
    }
    if save || close || modal.should_close() {
        with_state(ctx, key, |s: &mut BankEditState| s.details = None);
    } else {
        with_state(ctx, key, |s: &mut BankEditState| s.details = Some(d));
    }
}

/// "Tidy stems": before / after of every question tidying would change, a tick per question,
/// and Apply (after a backup of banks.json; the bank page then offers Undo).
fn draw_tidy_dialog(app: &mut App, ctx: &egui::Context, key: UiId, bank_id: &Id) {
    use crate::equation::{rich_ui, Rich};
    use crate::model::Kind;
    let Some(mut td) = with_state(ctx, key, |s: &mut BankEditState| s.tidy.clone()) else {
        return;
    };
    let mut close = false;
    let mut apply = false;
    let screen = ctx.content_rect();
    let w = (screen.width() - 120.0).clamp(420.0, 940.0);
    let modal = egui::Modal::new(UiId::new("bank_tidy_dialog")).show(ctx, |ui| {
        Frame::new().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_width(w);
            ui.label(theme::serif("Tidy stems").size(22.0));
            ui.add_space(4.0);
            let n = td.items.len();
            if n == 0 {
                ui.add(egui::Label::new(theme::text("Every stem in this bank is already tidy. Nothing to change.").size(12.5)).wrap());
            } else {
                ui.add(
                    egui::Label::new(
                        theme::text(format!(
                            "{n} question{} from older imports can be cleaned up: tables of blanks, line breaks, wrapped lines and spacing. Nothing changes until you apply, and fill-in-the-blank answers stay with their blanks.",
                            if n == 1 { "" } else { "s" }
                        ))
                        .size(12.5),
                    )
                    .wrap(),
                );
            }
            ui.add_space(10.0);
            egui::ScrollArea::vertical().max_height((screen.height() - 260.0).max(200.0)).auto_shrink([false, true]).show(ui, |ui| {
                // Keep the cards clear of the scroll bar.
                ui.set_max_width(ui.available_width() - 12.0);
                for (i, t) in td.items.iter().enumerate() {
                    kit::card().inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut td.picked[i], theme::strong(format!("Question {}", t.index + 1)).size(13.5));
                            ui.label(kit::small_caps_text(&t.before.kind.qkind().label().to_uppercase()));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let fixes: Vec<&str> = t.fixes.iter().map(|f| f.label()).collect();
                                ui.label(theme::muted(fixes.join(" · ")).size(11.5));
                            });
                        });
                        for f in &t.fields {
                            ui.add_space(6.0);
                            if t.fields.len() > 1 || f.label != "Stem" {
                                ui.label(kit::small_caps_text(&f.label.to_uppercase()));
                                ui.add_space(2.0);
                            }
                            ui.columns(2, |cols| {
                                for (c, (head, text)) in cols.iter_mut().zip([("Before", &f.before), ("After", &f.after)]) {
                                    c.label(theme::muted(head).size(11.0));
                                    Frame::new()
                                        .fill(if head == "After" { theme::accent_col().gamma_multiply(0.06) } else { Color32::TRANSPARENT })
                                        .stroke(Stroke::new(1.0, theme::border()))
                                        .corner_radius(CornerRadius::same(6))
                                        .inner_margin(Margin::symmetric(8, 6))
                                        .show(c, |ui| {
                                            ui.set_width(ui.available_width());
                                            rich_ui(ui, text, &Rich::new(13.0, theme::text_col()));
                                        });
                                }
                            });
                        }
                        if let (Kind::Blank { accept: a }, Kind::Blank { accept: b }) = (&t.before.kind, &t.after.kind)
                            && a != b
                        {
                            ui.add_space(4.0);
                            ui.label(
                                theme::muted(format!(
                                    "Answers: {} entries for {} blanks → {} for {} (the header row's stray blanks go; every answer stays with its blank)",
                                    a.len(),
                                    t.before.stem.matches("___").count(),
                                    b.len(),
                                    t.after.stem.matches("___").count(),
                                ))
                                .size(11.5),
                            );
                        }
                        if let Some(note) = &t.note {
                            ui.add_space(4.0);
                            ui.add(egui::Label::new(RichText::new(note).size(11.5).color(theme::bad())).wrap());
                        }
                    });
                    ui.add_space(8.0);
                }
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            let k = td.picked.iter().filter(|p| **p).count();
            ui.horizontal(|ui| {
                if n > 0 {
                    ui.label(theme::muted(format!("{k} of {n} selected")).size(12.0));
                    if ui.add(Button::new(RichText::new("All").size(12.0).color(theme::accent_col())).frame(false)).clicked() {
                        td.picked.iter_mut().for_each(|p| *p = true);
                    }
                    if ui.add(Button::new(RichText::new("None").size(12.0).color(theme::accent_col())).frame(false)).clicked() {
                        td.picked.iter_mut().for_each(|p| *p = false);
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if n > 0 {
                        let label = format!("Apply to {k} question{}", if k == 1 { "" } else { "s" });
                        if ui.add_enabled_ui(k > 0, |ui| kit::primary_button(ui, &label)).inner.clicked() && k > 0 {
                            apply = true;
                        }
                    }
                    if kit::ghost_button(ui, if n == 0 { "Close" } else { "Cancel" }).clicked() {
                        close = true;
                    }
                });
            });
            if n > 0 {
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        theme::muted("Your banks are backed up first (backups folder in the store). Undo then appears on the bank page.").size(11.5),
                    )
                    .wrap(),
                );
            }
        });
    });
    if apply {
        let picked: Vec<&crate::tidy::QuestionTidy> = td.items.iter().zip(&td.picked).filter(|(_, p)| **p).map(|(t, _)| t).collect();
        match crate::store::backup_banks(&app.store.banks) {
            Err(e) => app.toast_err(format!("Couldn't back up the banks, so nothing was changed: {e}")),
            Ok(path) => {
                let t = now_secs();
                let before = app
                    .store
                    .bank_mut(bank_id)
                    .map(|b| {
                        let u = crate::tidy::apply(&mut b.questions, &picked, t);
                        if !u.is_empty() {
                            b.modified = t;
                        }
                        u
                    })
                    .unwrap_or_default();
                let n = before.len();
                if n > 0 {
                    app.store.save_banks();
                    let backup = path.display().to_string();
                    with_state(ctx, key, |s: &mut BankEditState| s.last_tidy = Some(TidyUndo { before, backup }));
                    app.toast(format!("Tidied {n} question{}", if n == 1 { "" } else { "s" }));
                } else {
                    app.toast_err("Nothing was tidied: those questions changed since the preview");
                }
            }
        }
    }
    if apply || close || modal.should_close() {
        with_state(ctx, key, |s: &mut BankEditState| s.tidy = None);
    } else {
        with_state(ctx, key, |s: &mut BankEditState| s.tidy = Some(td));
    }
}

fn popover_key() -> UiId {
    UiId::new("qs_bank_popover")
}

/// Opens (or re-targets) the bank Pop-over window.
pub fn open_popover(ctx: &egui::Context, bank_id: &Id) {
    ctx.data_mut(|d| d.insert_temp(popover_key(), Some(bank_id.clone())));
}

#[derive(Clone, Default)]
struct PopoverState {
    search: String,
    /// Open banks (tabs), in order; the active one is the Pop-over key's bank.
    tabs: Vec<Id>,
    /// Filters row shown.
    filters: bool,
    /// Type filter (empty = every type).
    kinds: Vec<QKind>,
    diff: Option<Difficulty>,
    topic: Option<String>,
    sort: Sort,
    /// Answer lines chosen per question (copied with it and used when it is added to an exam).
    lines: std::collections::HashMap<String, u8>,
    /// Copy math as LaTeX `$…$` instead of plain Unicode text.
    latex: bool,
    settings: bool,
    /// Answer columns chosen per multiple-choice question (1, 2 or 4; shown, copied, and
    /// kept when it is added to an exam).
    cols: std::collections::HashMap<String, u8>,
}

/// Answer-line choices on the Pop-over cards (as on the reference).
pub(crate) const ANSWER_LINES: [u8; 5] = [0, 2, 4, 6, 10];

fn lines_label(n: u8) -> String {
    if n == 0 { "No lines".to_string() } else { format!("{n} lines") }
}

/// Indices of `qs` the Pop-over lists: search over the stem text, type / difficulty / topic
/// filters, then the sort.
pub(crate) fn popover_order(qs: &[Question], search: &str, kinds: &[QKind], diff: Option<Difficulty>, topic: Option<&str>, sort: Sort) -> Vec<usize> {
    let query = search.trim().to_lowercase();
    let mut order: Vec<usize> = qs
        .iter()
        .enumerate()
        .filter(|(_, q)| {
            (query.is_empty() || q.stem.to_lowercase().contains(&query) || crate::equation::stem_preview(&q.stem).to_lowercase().contains(&query))
                && (kinds.is_empty() || kinds.contains(&q.kind.qkind()))
                && diff.is_none_or(|d| Some(d) == q.difficulty)
                && topic.is_none_or(|t| q.topics.iter().any(|x| x.trim() == t))
        })
        .map(|(i, _)| i)
        .collect();
    order.sort_by(|&a, &b| {
        let (qa, qb) = (&qs[a], &qs[b]);
        match sort {
            Sort::Newest => qb.created.cmp(&qa.created),
            Sort::Oldest => qa.created.cmp(&qb.created),
            Sort::Kind => mix_index(qa.kind.qkind()).cmp(&mix_index(qb.kind.qkind())).then_with(|| qb.created.cmp(&qa.created)),
            Sort::Difficulty => diff_rank(qa.difficulty).cmp(&diff_rank(qb.difficulty)).then_with(|| qb.created.cmp(&qa.created)),
            Sort::Topic => first_topic(qa).cmp(&first_topic(qb)).then_with(|| qb.created.cmp(&qa.created)),
        }
    });
    order
}

/// Lines of stem markup as plain text: lists keep their markers, tables their ` | ` cells,
/// side-by-side panels follow each other; math as `$…$` (`latex`) or its Unicode text.
fn markup_lines(text: &str, latex: bool) -> Vec<String> {
    use crate::richtext::{line_text, parse, Block};
    fn walk(blocks: &[Block], out: &mut Vec<String>) {
        for (k, b) in blocks.iter().enumerate() {
            if k > 0 && !out.last().is_some_and(|l| l.is_empty()) && matches!(b, Block::Heading(..) | Block::Table { .. }) {
                out.push(String::new());
            }
            match b {
                Block::Para(ls) => out.extend(ls.iter().map(line_text)),
                Block::Bullets(ls) => out.extend(ls.iter().map(|l| format!("• {}", line_text(l)))),
                Block::Numbered(ls) => out.extend(ls.iter().enumerate().map(|(i, l)| format!("{}. {}", i + 1, line_text(l)))),
                Block::Quote(ls) => out.extend(ls.iter().map(|l| format!("> {}", line_text(l)))),
                Block::Table { rows, .. } => out.extend(rows.iter().map(|r| r.iter().map(line_text).collect::<Vec<_>>().join(" | "))),
                Block::Heading(_, l) => out.push(line_text(l)),
                Block::Divider => out.push("———".to_string()),
                Block::Side(panels) => {
                    for p in panels {
                        walk(p, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&parse(text), &mut out);
    if latex { out } else { out.into_iter().map(|l| crate::equation::stem_to_plain(&l)).collect() }
}

/// A question as text for the clipboard: stem, choices (A. B. ...), matching columns, parts,
/// and `lines` answer lines under a short answer (or each short part).
pub(crate) fn popover_copy_text(q: &Question, lines: u8, latex: bool) -> String {
    popover_copy_text_cols(q, lines, latex, 1)
}

/// Choice texts laid out in `cols` columns (column-major, as the exam prints them): one line
/// per row, each column padded to its widest entry.
fn choice_rows(items: &[String], cols: u8) -> Vec<String> {
    if cols <= 1 || items.len() < 2 {
        return items.to_vec();
    }
    let grid = crate::layout::choice_grid(items.len(), cols);
    let ncols = grid.iter().map(|r| r.len()).max().unwrap_or(1);
    let width: Vec<usize> = (0..ncols).map(|c| grid.iter().filter_map(|r| r.get(c)).map(|&i| items[i].chars().count()).max().unwrap_or(0)).collect();
    grid.iter()
        .map(|r| {
            let cells: Vec<String> = r.iter().enumerate().map(|(c, &i)| format!("{:<w$}", items[i], w = width[c])).collect();
            cells.join("    ").trim_end().to_string()
        })
        .collect()
}

/// [`popover_copy_text`] with the multiple-choice answers in `cols` columns.
pub(crate) fn popover_copy_text_cols(q: &Question, lines: u8, latex: bool, cols: u8) -> String {
    use crate::model::{Kind, PartKind};
    let one = |t: &str| markup_lines(t, latex).join(" ");
    let rule = "_".repeat(48);
    let mut out = markup_lines(&q.stem, latex);
    let push_lines = |out: &mut Vec<String>| {
        for _ in 0..lines {
            out.push(rule.clone());
        }
    };
    let abc = |i: usize| char::from(b'A' + (i % 26) as u8);
    match &q.kind {
        Kind::Mc { answers } => {
            let items: Vec<String> = answers.iter().enumerate().map(|(i, a)| format!("{}. {}", abc(i), one(&a.text))).collect();
            out.extend(choice_rows(&items, cols));
        }
        Kind::Tf { .. } => out.push("True / False".to_string()),
        Kind::Matching { pairs, distractors } => {
            out.extend(pairs.iter().enumerate().map(|(i, p)| format!("{}. {}", i + 1, one(&p.left))));
            let rights: Vec<&String> = pairs.iter().map(|p| &p.right).chain(distractors.iter()).collect();
            out.extend(rights.iter().enumerate().map(|(i, r)| format!("{}. {}", abc(i), one(r))));
        }
        Kind::Blank { .. } => {}
        Kind::Short { .. } => push_lines(&mut out),
        Kind::Multipart { parts } => {
            for (pi, p) in parts.iter().enumerate() {
                let pl = markup_lines(&p.stem, latex);
                out.push(format!("{}{}", part_label(pi, &p.stem), pl.first().cloned().unwrap_or_default()));
                out.extend(pl.into_iter().skip(1));
                match &p.kind {
                    PartKind::Mc { answers } => out.extend(answers.iter().enumerate().map(|(i, a)| format!("   {}. {}", abc(i), one(&a.text)))),
                    PartKind::Tf { .. } => out.push("   True / False".to_string()),
                    PartKind::Short { .. } => push_lines(&mut out),
                }
            }
        }
    }
    out.join("\n")
}

/// "a. " for part `pi`, or nothing when the part's text already starts with its own label
/// ("a. NH₃", "b) ...").
fn part_label(pi: usize, stem: &str) -> String {
    let t = stem.trim_start();
    let mut c = t.chars();
    let own = matches!((c.next(), c.next()), (Some(l), Some('.' | ')')) if l.is_ascii_alphabetic());
    if own { String::new() } else { format!("{}. ", char::from(b'a' + (pi % 26) as u8)) }
}

/// Does the question take answer lines (a short answer, or a multipart with a short part)?
fn takes_lines(q: &Question) -> bool {
    use crate::model::{Kind, PartKind};
    match &q.kind {
        Kind::Short { .. } => true,
        Kind::Multipart { parts } => parts.iter().any(|p| matches!(p.kind, PartKind::Short { .. })),
        _ => false,
    }
}

/// One Pop-over card: type label, answer-lines choice, the stem as it prints (rich), choices
/// or parts, and the actions.
#[allow(clippy::too_many_arguments)]
fn popover_card(ui: &mut Ui, q: &Question, n: usize, ps: &mut PopoverState, exam_id: Option<&Id>, in_exam: bool, add: &mut Option<Id>, edit: &mut Option<Id>) {
    use crate::equation::{rich_ui, Rich};
    use crate::model::{Kind, PartKind};
    let size = 13.0;
    kit::card().inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let mut lines = ps.lines.get(&q.id).copied().unwrap_or(0);
        ui.horizontal(|ui| {
            ui.label(theme::muted(format!("{n}.")).size(11.5));
            ui.label(kit::small_caps_text(&q.kind.qkind().label().to_uppercase()));
            if let Some(p) = points_label(q.points) {
                ui.label(theme::muted(p).size(11.0));
            }
            if let Kind::Mc { .. } = &q.kind {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut c = ps.cols.get(&q.id).copied().unwrap_or(1);
                    let r = crate::examui::answer_columns_switch(ui, &mut c).on_hover_text("Answer columns: how the choices are laid out, copied and printed when added to an exam");
                    if r.changed() || c != ps.cols.get(&q.id).copied().unwrap_or(1) {
                        if c <= 1 {
                            ps.cols.remove(&q.id);
                        } else {
                            ps.cols.insert(q.id.clone(), c);
                        }
                    }
                });
            }
            if takes_lines(q) {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ComboBox::from_id_salt(("pop_lines", &q.id)).width(84.0).selected_text(RichText::new(lines_label(lines)).size(11.5)).show_ui(ui, |ui| {
                        for v in ANSWER_LINES {
                            ui.selectable_value(&mut lines, v, lines_label(v));
                        }
                    })
                    .response
                    .on_hover_text("Answer lines: copied under the question, and the work space it gets when added to an exam");
                });
            }
        });
        if lines == 0 {
            ps.lines.remove(&q.id);
        } else {
            ps.lines.insert(q.id.clone(), lines);
        }
        ui.add_space(2.0);
        let rich = Rich::new(size, theme::text_col());
        rich_ui(ui, &q.stem, &rich);
        let abc = |i: usize| format!("{}. ", char::from(b'A' + (i % 26) as u8));
        let sep = |ui: &mut Ui| {
            ui.add_space(4.0);
            let r = ui.available_rect_before_wrap();
            ui.painter().extend(egui::Shape::dashed_line(&[Pos2::new(r.left(), r.top()), Pos2::new(r.right(), r.top())], Stroke::new(1.0, theme::border()), 3.0, 3.0));
            ui.add_space(4.0);
        };
        match &q.kind {
            Kind::Mc { answers } => {
                sep(ui);
                let cols = ps.cols.get(&q.id).copied().unwrap_or(1);
                if cols <= 1 {
                    for (i, a) in answers.iter().enumerate() {
                        rich_ui(ui, &a.text, &Rich::new(size, theme::text_col()).lead(abc(i), size, theme::ink()));
                    }
                } else {
                    // Column-major grid of equal-width columns (A B down the first column).
                    let grid = crate::layout::choice_grid(answers.len(), cols);
                    let ncols = grid.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
                    let cw = (ui.available_width() / ncols as f32).floor();
                    for row in grid {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for i in row {
                                ui.allocate_ui_with_layout(Vec2::new(cw, 0.0), Layout::top_down(Align::Min), |ui| {
                                    ui.set_width(cw - 6.0);
                                    rich_ui(ui, &answers[i].text, &Rich::new(size, theme::text_col()).lead(abc(i), size, theme::ink()));
                                });
                            }
                        });
                    }
                }
            }
            Kind::Tf { .. } => {
                sep(ui);
                ui.label(theme::text("True  /  False").size(size));
            }
            Kind::Matching { pairs, distractors } => {
                sep(ui);
                for (i, p) in pairs.iter().enumerate() {
                    rich_ui(ui, &p.left, &Rich::new(size, theme::text_col()).lead(format!("{}. ", i + 1), size, theme::ink()));
                }
                ui.add_space(3.0);
                for (i, r) in pairs.iter().map(|p| &p.right).chain(distractors.iter()).enumerate() {
                    rich_ui(ui, r, &Rich::new(size, theme::text_col()).lead(abc(i), size, theme::ink()));
                }
            }
            Kind::Blank { .. } | Kind::Short { .. } => {}
            Kind::Multipart { parts } => {
                sep(ui);
                for (pi, p) in parts.iter().enumerate() {
                    let label = part_label(pi, &p.stem);
                    let r = Rich::new(size, theme::text_col());
                    rich_ui(ui, &p.stem, &if label.is_empty() { r } else { r.lead(label, size, theme::ink()) });
                    if let PartKind::Mc { answers } = &p.kind {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(16.0);
                            for (i, a) in answers.iter().enumerate() {
                                ui.label(theme::text(format!("{}{}", abc(i), crate::equation::stem_preview(&a.text))).size(size - 1.0));
                            }
                        });
                    }
                    ui.add_space(3.0);
                }
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if exam_id.is_some() {
                let label = if in_exam { "Add again" } else { "Add to exam" };
                if kit::ghost_button(ui, label).clicked() {
                    *add = Some(q.id.clone());
                }
            }
            if ui.add(Button::new(theme::accent("Copy text").size(12.0)).frame(false)).on_hover_text("Copy the question (with its answer lines) to paste anywhere").clicked() {
                ui.ctx().copy_text(popover_copy_text_cols(q, lines, ps.latex, ps.cols.get(&q.id).copied().unwrap_or(1)));
            }
            if ui.add(Button::new(theme::accent("Edit").size(12.0)).frame(false)).clicked() {
                *edit = Some(q.id.clone());
            }
        });
    });
}

/// The bank Pop-over: questions of one or more banks (tabs) in a separate native window (an
/// embedded egui window where the platform has no multi-viewport support) so a teacher can
/// keep banks in view while building an exam: search and filter, set answer lines, copy a
/// question as text, add it to the open exam, or jump back to edit it. Called every frame from
/// the screen router.
pub fn draw_popover(app: &mut App, ctx: &egui::Context) {
    // Headless/QA hook: QUESHION_POPOVER=<n> opens the n-th bank's Pop-over once.
    if let Ok(v) = std::env::var("QUESHION_POPOVER") {
        let hid = UiId::new("qs_popover_hook");
        if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
            ctx.data_mut(|d| d.insert_temp(hid, true));
            let n: usize = v.trim().parse().unwrap_or(1);
            if let Some(b) = app.store.banks.get(n.saturating_sub(1)) {
                open_popover(ctx, &b.id.clone());
            }
            if std::env::var("QUESHION_EMBED_VIEWPORTS").as_deref() == Ok("1") {
                ctx.set_embed_viewports(true);
            }
            // QUESHION_POPOVER_FILTERS=1 shows the filters row, =settings the copy settings
            match std::env::var("QUESHION_POPOVER_FILTERS").as_deref() {
                Ok("1") => ctx.data_mut(|d| d.get_temp_mut_or_default::<PopoverState>(UiId::new("qs_popover_state")).filters = true),
                Ok("settings") => ctx.data_mut(|d| d.get_temp_mut_or_default::<PopoverState>(UiId::new("qs_popover_state")).settings = true),
                _ => {}
            }
        }
    }
    let Some(bank_id) = ctx.data(|d| d.get_temp::<Option<Id>>(popover_key())).flatten() else { return };
    let sk = UiId::new("qs_popover_state");
    let mut ps: PopoverState = ctx.data(|d| d.get_temp(sk)).unwrap_or_default();
    ps.tabs.retain(|t| app.store.bank(t).is_some());
    if app.store.bank(&bank_id).is_none() {
        // the active bank went away: the next tab, or close
        match ps.tabs.first().cloned() {
            Some(t) => open_popover(ctx, &t),
            None => {
                ctx.data_mut(|d| d.insert_temp::<Option<Id>>(popover_key(), None));
            }
        }
        ctx.data_mut(|d| d.insert_temp(sk, ps));
        return;
    }
    if !ps.tabs.contains(&bank_id) {
        ps.tabs.push(bank_id.clone());
    }
    let bank = app.store.bank(&bank_id).cloned().unwrap();
    let names: Vec<(Id, String)> = app.store.banks.iter().map(|b| (b.id.clone(), b.name.clone())).collect();
    let exam_id = match &app.screen {
        Screen::ExamEdit(id) => Some(id.clone()),
        _ => None,
    };
    let exam_items: Vec<Id> = exam_id.as_ref().and_then(|id| app.store.exam(id)).map(|e| e.items.iter().map(|it| it.question_id.clone()).collect()).unwrap_or_default();
    let exam_name = exam_id.as_ref().and_then(|id| app.store.exam(id)).map(|e| e.name.clone());
    let vid = egui::ViewportId::from_hash_of("qs_bank_popover_window");
    let builder = egui::ViewportBuilder::default()
        .with_title(format!("{} · queShion", bank.name))
        .with_inner_size([460.0, 720.0])
        .with_min_inner_size([340.0, 380.0]);
    let mut close = false;
    let mut add: Option<Id> = None;
    let mut edit: Option<Id> = None;
    let mut switch: Option<Id> = None;
    let mut close_tab: Option<Id> = None;
    ctx.show_viewport_immediate(vid, builder, |ui, class| {
        let pctx = ui.ctx().clone();
        if pctx.input(|i| i.viewport().close_requested()) {
            close = true;
        }
        let embedded = class == egui::ViewportClass::EmbeddedWindow;
        let body = |ui: &mut Ui, ps: &mut PopoverState, close: &mut bool, add: &mut Option<Id>, edit: &mut Option<Id>, switch: &mut Option<Id>, close_tab: &mut Option<Id>| {
            // tabs: open banks, + to open another, copy settings
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for t in ps.tabs.clone() {
                    let name = names.iter().find(|(id, _)| *id == t).map(|(_, n)| n.clone()).unwrap_or_default();
                    let on = t == bank_id;
                    Frame::new()
                        .fill(if on { theme::card() } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, if on { theme::border() } else { Color32::TRANSPARENT }))
                        .corner_radius(CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 })
                        .inner_margin(Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let label = RichText::new(truncate(&name, 22)).size(12.5).color(if on { theme::ink() } else { theme::muted_col() });
                                if ui.add(Button::new(if on { label.strong() } else { label }).frame(false)).on_hover_text(&name).clicked() && !on {
                                    *switch = Some(t.clone());
                                }
                                if ui.add(Button::new(RichText::new("×").size(12.0).color(theme::muted_col())).frame(false)).on_hover_text(format!("Close {name}")).clicked() {
                                    *close_tab = Some(t.clone());
                                }
                            });
                        });
                }
                let others: Vec<&(Id, String)> = names.iter().filter(|(id, _)| !ps.tabs.contains(id)).collect();
                if !others.is_empty() {
                    ui.menu_button(RichText::new("+").size(15.0), |ui| {
                        ui.label(theme::muted("Open a question bank").size(11.5));
                        for (id, n) in others {
                            if ui.button(n).clicked() {
                                *switch = Some(id.clone());
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text("Open another question bank here");
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if embedded && kit::ghost_button(ui, "Close").clicked() {
                        *close = true;
                    }
                    if ui.add(Button::new(RichText::new("⚙").size(15.0)).frame(false).selected(ps.settings)).on_hover_text("Copy settings").clicked() {
                        ps.settings = !ps.settings;
                    }
                });
            });
            if ps.settings {
                kit::card().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(theme::strong("Copy settings").size(13.5));
                    ui.label(kit::small_caps_text("MATHEMATICS"));
                    ui.radio_value(&mut ps.latex, false, RichText::new("Plain text").strong()).on_hover_text("x² , ½ , √ … pastes anywhere (Google Docs, email)");
                    ui.label(theme::muted("Equations as Unicode text. For Google Docs, and anywhere else.").size(11.0));
                    ui.radio_value(&mut ps.latex, true, RichText::new("LaTeX").strong()).on_hover_text("$x^2$ … type it into Word's equation editor (Alt+=) or a LaTeX document");
                    ui.label(theme::muted("Equations as $…$ source, for Word's equation editor (Alt+=) and LaTeX.").size(11.0));
                });
                ui.add_space(4.0);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 92.0).max(160.0);
                search_pill(ui, &mut ps.search, w, "Search question stems");
                let n_on = ps.kinds.len() + ps.diff.is_some() as usize + ps.topic.is_some() as usize;
                let label = if n_on > 0 { format!("Filters · {n_on}") } else { "Filters".to_string() };
                if ui.add(Button::new(RichText::new(label).size(12.5)).selected(ps.filters).corner_radius(CornerRadius::same(8))).clicked() {
                    ps.filters = !ps.filters;
                }
            });
            if ps.filters {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let kinds_label = if ps.kinds.is_empty() { "Question type".to_string() } else { format!("Type · {}", ps.kinds.len()) };
                    ui.menu_button(RichText::new(kinds_label).size(12.0), |ui| {
                        for k in [QKind::Mc, QKind::Tf, QKind::Matching, QKind::Blank, QKind::Short, QKind::Multipart] {
                            let mut on = ps.kinds.contains(&k);
                            if ui.checkbox(&mut on, k.label()).changed() {
                                if on {
                                    ps.kinds.push(k);
                                } else {
                                    ps.kinds.retain(|x| *x != k);
                                }
                            }
                        }
                    });
                    let dl = ps.diff.map(|d| format!("Difficulty · {}", diff_label(d))).unwrap_or_else(|| "Difficulty".to_string());
                    ui.menu_button(RichText::new(dl).size(12.0), |ui| {
                        ui.selectable_value(&mut ps.diff, None, "All");
                        for d in [Difficulty::Low, Difficulty::Medium, Difficulty::High] {
                            ui.selectable_value(&mut ps.diff, Some(d), diff_label(d));
                        }
                    });
                    let topics = unique_topics(&bank.questions);
                    if !topics.is_empty() || ps.topic.is_some() {
                        let tl = ps.topic.as_deref().map(|t| format!("Topic · {}", truncate(t, 16))).unwrap_or_else(|| "Topic".to_string());
                        ui.menu_button(RichText::new(tl).size(12.0), |ui| {
                            ui.selectable_value(&mut ps.topic, None, "All topics");
                            for t in topics {
                                ui.selectable_value(&mut ps.topic, Some(t.clone()), t);
                            }
                        });
                    }
                    ui.menu_button(RichText::new(format!("Sort: {}", ps.sort.label())).size(12.0), |ui| {
                        for so in Sort::all() {
                            ui.selectable_value(&mut ps.sort, so, so.label());
                        }
                    });
                    if (!ps.kinds.is_empty() || ps.diff.is_some() || ps.topic.is_some()) && ui.add(Button::new(theme::accent("Clear").size(12.0)).frame(false)).clicked() {
                        ps.kinds.clear();
                        ps.diff = None;
                        ps.topic = None;
                    }
                });
            }
            ui.add_space(4.0);
            ui.label(
                theme::muted(match &exam_name {
                    Some(n) => format!("Add sends a question to “{n}”."),
                    None => "Open an exam in the main window to add questions to it.".to_string(),
                })
                .size(11.5),
            );
            ui.add_space(6.0);
            let order = popover_order(&bank.questions, &ps.search, &ps.kinds, ps.diff, ps.topic.as_deref(), ps.sort);
            egui::ScrollArea::vertical().id_salt(("popover_scroll", &bank_id)).auto_shrink([false, false]).show(ui, |ui| {
                for (k, &i) in order.iter().enumerate() {
                    let q = &bank.questions[i];
                    popover_card(ui, q, k + 1, ps, exam_id.as_ref(), exam_items.contains(&q.id), add, edit);
                    ui.add_space(6.0);
                }
                if order.is_empty() {
                    ui.label(theme::muted(if bank.questions.is_empty() { "This bank has no questions yet." } else { "No questions match." }).size(12.5));
                }
            });
        };
        if embedded {
            // Embedded fallback: the viewport is already an egui window; just pad it.
            ui.set_min_size(Vec2::new(420.0, 540.0));
            body(ui, &mut ps, &mut close, &mut add, &mut edit, &mut switch, &mut close_tab);
        } else {
            egui::CentralPanel::default()
                .frame(Frame::new().fill(theme::cream()).inner_margin(Margin::same(12)))
                .show(ui, |ui| body(ui, &mut ps, &mut close, &mut add, &mut edit, &mut switch, &mut close_tab));
        }
    });
    if let Some(t) = switch {
        if !ps.tabs.contains(&t) {
            ps.tabs.push(t.clone());
        }
        open_popover(ctx, &t);
    }
    if let Some(t) = close_tab {
        let at = ps.tabs.iter().position(|x| *x == t).unwrap_or(0);
        ps.tabs.retain(|x| *x != t);
        if t == bank_id {
            match ps.tabs.get(at.min(ps.tabs.len().saturating_sub(1))).cloned() {
                Some(n) => open_popover(ctx, &n),
                None => close = true,
            }
        }
    }
    if let (Some(qid), Some(eid)) = (add, exam_id.as_ref()) {
        if let Some(before) = app.store.exam(eid).map(|e| e.items.clone()) {
            app.undo.push(before);
            app.redo.clear();
            let lines = ps.lines.get(&qid).copied().unwrap_or(0);
            if let Some(ex) = app.store.exam_mut(eid) {
                let cols = ps.cols.get(&qid).copied().unwrap_or(1);
                let mut item = crate::model::ExamItem::new(bank_id.clone(), qid);
                item.work_lines = lines.min(23);
                item.answer_columns = cols;
                ex.items.push(item);
                ex.modified = now_secs();
            }
            app.store.save_exams();
            app.toast("Added to the exam");
        }
    }
    if let Some(qid) = edit {
        if let Some(q) = app.store.question(&bank_id, &qid).cloned() {
            app.open_bank(&bank_id);
            app.qedit = Some(qedit::Draft::from_question(bank_id.clone(), &q));
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
        }
    }
    if close {
        ps.tabs.clear();
        ctx.data_mut(|d| d.insert_temp::<Option<Id>>(popover_key(), None));
    }
    ctx.data_mut(|d| d.insert_temp(sk, ps));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_bank() -> crate::model::QuestionBank {
        crate::model::QuestionBank { id: "b".into(), name: "B".into(), questions: Vec::new(), created: 1, modified: 1, details: Default::default() }
    }

    fn q(stem: &str, kind: crate::model::Kind, created: i64) -> Question {
        let mut q = crate::formats::make_question(stem.to_string(), kind);
        q.created = created;
        q
    }

    fn mc(texts: &[&str]) -> crate::model::Kind {
        crate::model::Kind::Mc { answers: texts.iter().enumerate().map(|(i, t)| crate::model::Answer { text: t.to_string(), correct: i == 0, locked: false }).collect() }
    }

    #[test]
    fn popover_filters_and_sorts() {
        let mut a = q("Which gas is $CO_2$?", mc(&["carbon dioxide", "oxygen"]), 1);
        a.difficulty = Some(Difficulty::Low);
        a.topics = vec!["Gases".into()];
        let b = q("Explain **why**.", crate::model::Kind::Short { sample: String::new() }, 2);
        let mut c = q("The sky is blue.", crate::model::Kind::Tf { correct: true }, 3);
        c.difficulty = Some(Difficulty::High);
        let qs = vec![a, b, c];
        assert_eq!(popover_order(&qs, "", &[], None, None, Sort::Newest), vec![2, 1, 0]);
        assert_eq!(popover_order(&qs, "", &[], None, None, Sort::Oldest), vec![0, 1, 2]);
        assert_eq!(popover_order(&qs, "", &[QKind::Mc, QKind::Tf], None, None, Sort::Oldest), vec![0, 2]);
        assert_eq!(popover_order(&qs, "", &[], Some(Difficulty::High), None, Sort::Newest), vec![2]);
        assert_eq!(popover_order(&qs, "", &[], None, Some("Gases"), Sort::Newest), vec![0]);
        assert_eq!(popover_order(&qs, "why", &[], None, None, Sort::Newest), vec![1]);
        assert_eq!(popover_order(&qs, "co₂", &[], None, None, Sort::Newest), vec![0], "search reads the shown text");
    }

    #[test]
    fn popover_copy_text_carries_choices_lines_and_math_setting() {
        let a = q("Which gas is $CO_2$?\n• gas", mc(&["carbon dioxide", "*oxygen*"]), 1);
        assert_eq!(popover_copy_text(&a, 0, false), "Which gas is CO₂?\n• gas\nA. carbon dioxide\nB. oxygen");
        assert_eq!(popover_copy_text(&a, 0, true), "Which gas is $CO_2$?\n• gas\nA. carbon dioxide\nB. oxygen");
        let b = q("Explain.", crate::model::Kind::Short { sample: String::new() }, 2);
        let t = popover_copy_text(&b, 2, false);
        assert_eq!(t.lines().count(), 3);
        assert!(t.lines().skip(1).all(|l| l.chars().all(|c| c == '_') && l.len() > 20));
        let parts = crate::model::Kind::Multipart {
            parts: vec![
                crate::model::Part { stem: "a. NH_{3}".into(), kind: crate::model::PartKind::Short { sample: String::new() } },
                crate::model::Part { stem: "Second".into(), kind: crate::model::PartKind::Tf { correct: true } },
            ],
        };
        let m = q("Do all:", parts, 3);
        let t = popover_copy_text(&m, 1, false);
        assert_eq!(t.lines().next(), Some("Do all:"));
        assert!(t.contains("\na. NH₃\n____") && t.contains("\nb. Second\n   True / False"), "{t}");
        assert!(takes_lines(&m) && takes_lines(&b) && !takes_lines(&a));
        // headings, dividers and side-by-side panels flatten to lines
        let h = q("# Part A\n---\n::: side\nleft\n::: next\nright\n:::", crate::model::Kind::Short { sample: String::new() }, 4);
        assert_eq!(popover_copy_text(&h, 0, false), "Part A\n———\nleft\nright");
    }

    #[test]
    fn details_save_trimmed_and_only_on_change() {
        let mut b = empty_bank();
        let mut d = crate::model::BankDetails::default();
        assert!(!apply_details(&mut b, &d, 5), "nothing entered, nothing changes");
        assert_eq!(b.modified, 1);
        d.author = "  Ms. Rivera ".into();
        d.license_name = "CC BY 4.0".into();
        assert!(apply_details(&mut b, &d, 7));
        assert_eq!(b.details.author, "Ms. Rivera");
        assert_eq!(b.modified, 7);
        assert!(!apply_details(&mut b, &d, 9), "same details again is a no-op");
    }

    #[test]
    fn bank_menu_matches_the_reference_order() {
        let labels: Vec<&str> = bank_menu_items(3).iter().map(|i| i.map_or("---", |(_, l, _)| l)).collect();
        assert_eq!(labels, vec!["Open", "Pop-over", "Export…", "Rename", "---", "Delete…"]);
        assert!(bank_menu_items(3).iter().flatten().all(|(_, _, en)| *en));
        // an empty bank cannot be exported (as on the reference)
        let export = |n| bank_menu_items(n).into_iter().flatten().find(|(a, _, _)| *a == BankAct::Export).unwrap().2;
        assert!(!export(0) && export(1));
    }

    #[test]
    fn export_carries_details_only_when_entered() {
        let mut b = empty_bank();
        assert!(bank_export_json(&b).get("details").is_none());
        b.details.description = "Unit 4 bonding".into();
        b.details.license_url = "https://creativecommons.org/licenses/by/4.0/".into();
        let v = bank_export_json(&b);
        assert_eq!(v["details"]["description"], "Unit 4 bonding");
        assert_eq!(v["details"]["license_url"], "https://creativecommons.org/licenses/by/4.0/");
        assert!(v["details"].get("author").is_none(), "empty fields stay out");
    }

    #[test]
    fn duplicate_inserts_a_fresh_copy_after_the_original() {
        use crate::formats::make_question;
        use crate::model::{Kind, Part, PartKind, QuestionBank};
        let mut a = make_question("First".into(), Kind::Tf { correct: true });
        a.topics = vec!["Atoms".into()];
        a.points = Some(2.0);
        a.created = 5;
        let b = make_question(
            "Second".into(),
            Kind::Multipart { parts: vec![Part { stem: "a.".into(), kind: PartKind::Short { sample: "x".into() } }] },
        );
        let mut bank: QuestionBank = serde_json::from_value(serde_json::json!({
            "id": "b1", "name": "B", "questions": [], "created": 0, "modified": 0
        }))
        .unwrap();
        bank.questions = vec![a.clone(), b.clone()];
        let id = duplicate_in_bank(&mut bank, &a.id).expect("copied");
        assert_eq!(bank.questions.len(), 3);
        let c = &bank.questions[1];
        assert_eq!(c.id, id);
        assert_ne!(c.id, a.id);
        assert_eq!(c.stem, "First");
        assert_eq!(c.topics, a.topics);
        assert_eq!(c.points, Some(2.0));
        assert!(matches!(c.kind, Kind::Tf { correct: true }));
        assert!(c.created > 5, "fresh timestamps");
        assert_eq!(bank.questions[2].id, b.id, "inserted right after the original");
        assert!(bank.modified > 0);
        assert!(duplicate_in_bank(&mut bank, "missing").is_none());
    }

    #[test]
    fn fmt_date_formats_epoch_days() {
        assert_eq!(fmt_date(0), "—");
        assert_eq!(fmt_date(86_400), "Jan 2, 1970");
        // 1970-01-01 + 56*365 + 14 leap days + day-of-year offset of Oct 15 (287).
        let secs: i64 = ((2026 - 1970) as i64 * 365 + 14 + 287) * 86_400;
        assert_eq!(fmt_date(secs), "Oct 15, 2026");
    }

    #[test]
    fn rel_time_buckets() {
        let now = now_secs();
        assert_eq!(rel_time(now - 5), "just now");
        assert_eq!(rel_time(now - 2 * 3600 - 10), "2 hours ago");
        assert_eq!(rel_time(now - 86_400 - 10), "1 day ago");
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
    fn popover_copies_choices_in_the_chosen_columns() {
        let q = q("Which is least electronegative?", mc(&["Ca", "O", "Ge", "C"]), 1);
        assert_eq!(popover_copy_text_cols(&q, 0, false, 1), popover_copy_text(&q, 0, false));
        assert_eq!(popover_copy_text_cols(&q, 0, false, 2), "Which is least electronegative?\nA. Ca    C. Ge\nB. O     D. C");
        assert_eq!(popover_copy_text_cols(&q, 0, false, 4), "Which is least electronegative?\nA. Ca    B. O    C. Ge    D. C");
        // three choices in two columns: A C / B
        assert_eq!(choice_rows(&["A. x".into(), "B. yy".into(), "C. z".into()], 2), vec!["A. x     C. z".to_string(), "B. yy".to_string()]);
    }

    #[test]
    fn blank_answer_text_reads_like_the_reference_key() {
        let acc = |v: &[&[&str]]| v.iter().map(|a| a.iter().map(|s| s.to_string()).collect()).collect::<Vec<Vec<String>>>();
        assert_eq!(blank_answer_text(&acc(&[&["14"], &["10"]])), "14; 10");
        assert_eq!(blank_answer_text(&acc(&[&["linear", " Linear "], &[]])), "linear / Linear; —");
        assert_eq!(blank_answer_text(&[]), "");
    }

    #[test]
    fn points_label_works() {
        assert_eq!(points_label(None), None);
        assert_eq!(points_label(Some(2.0)), Some("2 pt".to_string()));
        assert_eq!(points_label(Some(2.5)), Some("2.5 pt".to_string()));
    }
}
