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
}

// ---------- screen 1: the Question Banks list ----------

struct BankRow {
    id: Id,
    name: String,
    nq: usize,
    topics: usize,
    modified: i64,
    mix: [usize; 6],
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
                let resp = ui.add(
                    TextEdit::singleline(&mut name_buf)
                        .font(egui::FontId::new(34.0, theme::serif_family()))
                        .text_color(theme::ink())
                        .frame(Frame::new())
                        .hint_text("Untitled bank")
                        .desired_width(name_w),
                );
                if resp.changed() && !name_buf.trim().is_empty() && name_buf.trim() != bank_name {
                    let t = now_secs();
                    if let Some(b) = self.store.bank_mut(bank_id) {
                        b.name = name_buf.trim().to_string();
                        b.modified = t;
                    }
                    self.store.save_banks();
                }
                ui.label(
                    theme::muted(format!(
                        "{} question{} · {} topic{} · edited {}",
                        questions.len(),
                        if questions.len() == 1 { "" } else { "s" },
                        topics.len(),
                        if topics.len() == 1 { "" } else { "s" },
                        rel_time(modified),
                    ))
                    .size(13.0),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if kit::primary_button(ui, "Add question").clicked() {
                    self.qedit = Some(qedit::Draft::new(bank_id.clone()));
                }
                if !questions.is_empty() && kit::ghost_button(ui, "Export").clicked() {
                    export_bank(self, bank_id);
                }
                if kit::ghost_button(ui, "Import").clicked() {
                    self.screen = crate::Screen::Imports;
                }
            });
        });
        ui.add_space(12.0);

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
                    if kit::chip_toggle(ui, &format!("{t}  {n}"), on).clicked() {
                        let sel = if on { None } else { Some(t.clone()) };
                        with_state(ctx, key, |s: &mut BankEditState| s.topic = sel);
                    }
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

/// "..." menu with Rename / Delete; returns (clicked item, pointer over the button).
fn bank_menu(ui: &mut Ui) -> (Option<bool>, bool) {
    let mut act = None;
    let r = ui.menu_button(RichText::new("…").size(14.0).color(theme::muted_col()), |ui| {
        if ui.button("Rename").clicked() {
            act = Some(false);
            ui.close();
        }
        if ui.button("Delete…").clicked() {
            act = Some(true);
            ui.close();
        }
    });
    (act, r.response.contains_pointer())
}

fn apply_bank_menu(ctx: &egui::Context, key: UiId, r: &BankRow, act: Option<bool>) {
    if let Some(del) = act {
        with_state(ctx, key, |s: &mut BanksState| {
            if del {
                s.confirm_delete = Some(r.id.clone());
            } else {
                s.rename = Some((r.id.clone(), r.name.clone()));
            }
        });
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
    let mut menu_act = None;
    let mut over_menu = false;
    let resp = kit::card().inner_margin(Margin::same(14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(theme::serif(truncate(&r.name, 30)).size(18.0));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                let (a, o) = bank_menu(ui);
                menu_act = a;
                over_menu = o;
            });
        });
        ui.label(theme::muted(counts_line(r)).size(12.5));
        ui.add_space(8.0);
        paint_mix_bar(ui, ui.available_width(), &r.mix);
        ui.add_space(8.0);
        ui.label(theme::muted(format!("Edited {}", rel_time(r.modified))).size(11.5));
    });
    apply_bank_menu(ctx, key, r, menu_act);
    let click = ui.interact(resp.response.rect, ui.id().with(("bank_card", &r.id)), Sense::click());
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

fn bank_list_row(ui: &mut Ui, app: &mut App, ctx: &egui::Context, key: UiId, r: &BankRow) {
    let mut menu_act = None;
    let mut over_menu = false;
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
            ui.label(theme::muted(format!("Edited {}", rel_time(r.modified))).size(12.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (a, o) = bank_menu(ui);
                menu_act = a;
                over_menu = o;
            });
        });
    });
    apply_bank_menu(ctx, key, r, menu_act);
    let click = ui.interact(resp.response.rect, ui.id().with(("bank_row", &r.id)), Sense::click());
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

fn stem_job(ui: &Ui, text: &str, size: f32, rows: usize, width: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(
        crate::equation::stem_preview(text),
        egui::FontId::new(size, theme::serif_family()),
        theme::ink(),
        width,
    );
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = false;
    job.wrap.overflow_character = Some('…');
    let _ = ui;
    job
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
                    let sw = (ui.available_width() - 270.0).max(100.0);
                    ui.allocate_ui(Vec2::new(sw, 20.0), |ui| {
                        let job = stem_job(ui, &q.stem, 13.5, 1, sw);
                        ui.add(egui::Label::new(job).truncate());
                    });
                    if let Some(t) = q.topics.iter().map(|t| t.trim()).find(|t| !t.is_empty()) {
                        meta_chip(ui, &truncate(t, 16));
                    }
                    if let Some(d) = q.difficulty {
                        ui.label(theme::muted(diff_label(d)).size(11.5));
                    }
                    if let Some(p) = points_label(q.points) {
                        ui.label(RichText::new(p).family(theme::bold_family()).size(12.0).color(theme::ink()));
                    }
                    paint_chevron(ui, false);
                });
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
                        if let Some(p) = q.and_then(|q| points_label(q.points)) {
                            ui.label(RichText::new(p).family(theme::bold_family()).size(12.0).color(theme::ink()));
                        }
                    });
                });
                if let (false, Some(q)) = (open, q) {
                    ui.add_space(5.0);
                    let job = stem_job(ui, &q.stem, 14.0, 2, ui.available_width());
                    ui.label(job);
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                        for t in q.topics.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).take(2) {
                            meta_chip(ui, &truncate(t, 18));
                        }
                        if let Some(d) = q.difficulty {
                            ui.label(theme::muted(diff_label(d)).size(11.5));
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
        if hr.clicked() {
            toggled = true;
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
    fn points_label_works() {
        assert_eq!(points_label(None), None);
        assert_eq!(points_label(Some(2.0)), Some("2 pt".to_string()));
        assert_eq!(points_label(Some(2.5)), Some("2.5 pt".to_string()));
    }
}
