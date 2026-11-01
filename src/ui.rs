//! Интерфейс Nexora Unlocker: окно, таблица файлов, панели и фоновые задачи.

use crate::fsops::{plan_rename, run_op, Guard, Op};
use crate::settings::{IconSet, Settings};
use crate::sys::{self, Holder};
use eframe::egui::{self, text::LayoutJob, Align, Align2, Color32, FontFamily, FontId, TextFormat};
use egui_extras::{Column, TableBuilder};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

// Глифы Segoe Fluent Icons (Windows 11) / Segoe MDL2 Assets (Windows 10).
mod icon {
    pub const ADD: &str = "\u{E710}";
    pub const FOLDER: &str = "\u{E8B7}";
    pub const FILE: &str = "\u{E8A5}";
    pub const SETTINGS: &str = "\u{E713}";
    pub const DELETE: &str = "\u{E74D}";
    pub const MOVE: &str = "\u{E8DE}";
    pub const COPY: &str = "\u{E8C8}";
    pub const EDIT: &str = "\u{E70F}";
    pub const CANCEL: &str = "\u{E711}";
    pub const CLOCK: &str = "\u{E916}";
    pub const SHIELD: &str = "\u{EA18}";
    pub const OK: &str = "\u{E73E}";
    pub const STOP: &str = "\u{E71A}";
    pub const CHEVRON_DOWN: &str = "\u{E70D}";
    pub const CHEVRON_UP: &str = "\u{E70E}";
}

/// Найден ли шрифт значков; без него значки не рисуются, остаётся только текст.
static HAS_ICONS: AtomicBool = AtomicBool::new(false);

const MAX_LOG: usize = 500;

// ---------- оформление ----------

fn accent(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(38, 132, 255)
    } else {
        Color32::from_rgb(0, 103, 192)
    }
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    if dark {
        v.panel_fill = Color32::from_rgb(32, 32, 32);
        v.window_fill = Color32::from_rgb(40, 40, 40);
        v.extreme_bg_color = Color32::from_rgb(26, 26, 26);
        v.faint_bg_color = Color32::from_rgb(38, 38, 38);
    } else {
        v.panel_fill = Color32::from_rgb(245, 248, 253);
        v.window_fill = Color32::WHITE;
        v.extreme_bg_color = Color32::WHITE;
        v.faint_bg_color = Color32::from_rgb(240, 244, 250);
    }
    v.selection.bg_fill = accent(dark);
    let r = egui::Rounding::same(6.0);
    v.widgets.noninteractive.rounding = r;
    v.widgets.inactive.rounding = r;
    v.widgets.hovered.rounding = r;
    v.widgets.active.rounding = r;
    v.widgets.open.rounding = r;
    v.window_rounding = egui::Rounding::same(10.0);
    ctx.set_visuals(v);
}

fn setup_fonts(ctx: &egui::Context, icons: IconSet) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(bytes) = sys::read_system_font(&["segoeui.ttf"]) {
        fonts.font_data.insert("segoe".into(), egui::FontData::from_owned(bytes));
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "segoe".into());
    }
    let candidates: &[&str] = match icons {
        IconSet::Fluent => &["SegoeIcons.ttf", "segmdl2.ttf"],
        IconSet::Mdl2 => &["segmdl2.ttf", "SegoeIcons.ttf"],
        IconSet::None => &[],
    };
    let icon_list = match sys::read_system_font(candidates) {
        Some(bytes) => {
            fonts.font_data.insert("icons".into(), egui::FontData::from_owned(bytes));
            HAS_ICONS.store(true, Ordering::Relaxed);
            vec!["icons".to_string()]
        }
        None => {
            HAS_ICONS.store(false, Ordering::Relaxed);
            vec![]
        }
    };
    fonts.families.insert(FontFamily::Name("icons".into()), icon_list);
    ctx.set_fonts(fonts);
}

fn icon_set_name(s: IconSet) -> &'static str {
    match s {
        IconSet::Fluent => "Windows 11 (Fluent)",
        IconSet::Mdl2 => "Windows 10 (MDL2)",
        IconSet::None => "Без значков",
    }
}

/// Текст со значком слева (значок рисуется шрифтом значков).
fn rich(ic: &str, text: &str, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let text_fmt = TextFormat {
        font_id: FontId::proportional(14.0),
        color,
        valign: Align::Center,
        ..Default::default()
    };
    if HAS_ICONS.load(Ordering::Relaxed) && !ic.is_empty() {
        let icon_fmt = TextFormat {
            font_id: FontId::new(16.0, FontFamily::Name("icons".into())),
            color,
            valign: Align::Center,
            ..Default::default()
        };
        job.append(ic, 0.0, icon_fmt);
        job.append("  ", 0.0, text_fmt.clone());
    }
    job.append(text, 0.0, text_fmt);
    job
}

fn btn(ui: &mut egui::Ui, ic: &str, text: &str, enabled: bool) -> egui::Response {
    let c = ui.visuals().text_color();
    ui.add_enabled(enabled, egui::Button::new(rich(ic, text, c)).min_size(egui::vec2(0.0, 32.0)))
}

fn btn_colored(
    ui: &mut egui::Ui,
    ic: &str,
    text: &str,
    fg: Color32,
    fill: Color32,
    enabled: bool,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(rich(ic, text, fg)).fill(fill).min_size(egui::vec2(0.0, 32.0)),
    )
}

fn ask(title: &str, text: &str) -> bool {
    MessageDialog::new()
        .set_title(title)
        .set_description(text)
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}

fn format_size(n: u64) -> String {
    const KB: f64 = 1024.0;
    let f = n as f64;
    if f < KB {
        format!("{n} Б")
    } else if f < KB * KB {
        format!("{:.0} КБ", f / KB)
    } else if f < KB * KB * KB {
        format!("{:.1} МБ", f / (KB * KB))
    } else {
        format!("{:.2} ГБ", f / (KB * KB * KB))
    }
}

// ---------- модель ----------

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Checking,
    Unknown,
    Free,
    Busy,
    Protected,
}

struct Item {
    path: PathBuf,
    name: String,
    parent: String,
    size: Option<u64>,
    is_dir: bool,
    blockers: Vec<Holder>,
    scanned: bool,
    scan_failed: bool,
    checked: bool,
}

struct RenameState {
    template: String,
    sources: Vec<PathBuf>,
}

type Job = (PathBuf, Option<PathBuf>);

enum Msg {
    Scanned(PathBuf, Result<Vec<Holder>, u32>),
    Renamed(PathBuf, PathBuf),
    Log(String, String),
    Progress(usize, usize),
    Done,
}

pub struct App {
    items: Vec<Item>,
    selected_procs: HashSet<u32>,
    settings: Settings,
    show_settings: bool,
    rename: Option<RenameState>,
    log: Vec<(String, String, String)>,
    status: String,
    progress: (usize, usize),
    running: bool,
    is_admin: bool,
    cancel: Arc<AtomicBool>,
    guard: Arc<Guard>,
    tx: mpsc::Sender<Msg>,
    rx: mpsc::Receiver<Msg>,
    ctx: egui::Context,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        setup_fonts(&cc.egui_ctx, settings.icons);
        apply_theme(&cc.egui_ctx, settings.dark);
        let (tx, rx) = mpsc::channel();
        App {
            items: Vec::new(),
            selected_procs: HashSet::new(),
            settings,
            show_settings: false,
            rename: None,
            log: Vec::new(),
            status: "Готов к работе".into(),
            progress: (0, 0),
            running: false,
            is_admin: sys::is_admin(),
            cancel: Arc::new(AtomicBool::new(false)),
            guard: Arc::new(Guard::from_env()),
            tx,
            rx,
            ctx: cc.egui_ctx.clone(),
        }
    }

    fn push_log(&mut self, action: impl Into<String>, result: impl Into<String>) {
        self.log.push((sys::local_time(), action.into(), result.into()));
        if self.log.len() > MAX_LOG {
            let extra = self.log.len() - MAX_LOG;
            self.log.drain(0..extra);
        }
    }

    fn checked_paths(&self) -> Vec<PathBuf> {
        self.items.iter().filter(|i| i.checked).map(|i| i.path.clone()).collect()
    }

    // ----- список и сканирование -----

    fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let mut fresh = Vec::new();
        for p in paths {
            if self.items.iter().any(|i| i.path == p) {
                continue;
            }
            let is_dir = p.is_dir();
            let size = if is_dir { None } else { std::fs::metadata(&p).ok().map(|m| m.len()) };
            self.items.push(Item {
                name: p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.display().to_string()),
                parent: p.parent().map(|x| x.display().to_string()).unwrap_or_default(),
                path: p.clone(),
                size,
                is_dir,
                blockers: Vec::new(),
                scanned: false,
                scan_failed: false,
                checked: false,
            });
            fresh.push(p);
        }
        self.spawn_scan(fresh);
    }

    fn spawn_scan(&self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            for p in paths {
                let h = sys::holders(&p);
                if tx.send(Msg::Scanned(p, h)).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
    }

    fn rescan_all(&mut self) {
        let paths: Vec<PathBuf> = self.items.iter().map(|i| i.path.clone()).collect();
        for i in &mut self.items {
            i.scanned = false;
            i.scan_failed = false;
            i.blockers.clear();
        }
        self.spawn_scan(paths);
    }

    fn status_of(&self, it: &Item) -> Status {
        if self.settings.protection && self.guard.is_protected(&it.path) {
            Status::Protected
        } else if !it.scanned {
            Status::Checking
        } else if it.scan_failed {
            Status::Unknown
        } else if it.blockers.is_empty() {
            Status::Free
        } else {
            Status::Busy
        }
    }

    fn drain(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Scanned(path, res) => {
                    if let Some(it) = self.items.iter_mut().find(|i| i.path == path) {
                        it.scanned = true;
                        match res {
                            Ok(h) => it.blockers = h,
                            Err(_) => it.scan_failed = true,
                        }
                    }
                }
                Msg::Renamed(old, new) => {
                    if let Some(it) = self.items.iter_mut().find(|i| i.path == old) {
                        it.name = new.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        it.path = new;
                    }
                }
                Msg::Log(a, r) => self.push_log(a, r),
                Msg::Progress(d, t) => self.progress = (d, t),
                Msg::Done => {
                    self.running = false;
                    self.items.retain(|i| i.path.exists());
                    self.rescan_all();
                    self.status = "Готово".into();
                }
            }
        }
    }

    // ----- запуск операций -----

    fn start_op(&mut self, op: Op) {
        if self.running {
            return;
        }
        let paths = self.checked_paths();
        if paths.is_empty() {
            return;
        }
        let dest = if matches!(op, Op::Move | Op::Copy) {
            match FileDialog::new().set_title("Куда?").pick_folder() {
                Some(d) => Some(d),
                None => return,
            }
        } else {
            None
        };
        if op == Op::Delete
            && !ask("Подтверждение", &format!("Удалить безвозвратно: {} шт.?", paths.len()))
        {
            return;
        }
        let jobs: Vec<Job> = paths.into_iter().map(|p| (p, dest.clone())).collect();
        self.launch(op, jobs);
    }

    /// Общий запуск пакетной операции: проверки безопасности, подтверждения, фоновый поток.
    fn launch(&mut self, op: Op, jobs: Vec<Job>) {
        if self.running || jobs.is_empty() {
            return;
        }
        let protect = self.settings.protection;

        // Защита отключена: отдельное подтверждение для системных путей.
        if !protect {
            let n = jobs.iter().filter(|(p, _)| self.guard.is_protected(p)).count();
            if n > 0
                && !ask(
                    "Системные пути",
                    &format!(
                        "Среди выбранных защищённых системных путей: {n}. Защита отключена, операция может повредить Windows. Продолжить?"
                    ),
                )
            {
                return;
            }
        }

        // Процессы, мешающие выбранным элементам (при защите критичные не трогаем).
        let mut busy: BTreeMap<u32, String> = BTreeMap::new();
        for (src, _) in &jobs {
            if let Some(it) = self.items.iter().find(|i| &i.path == src) {
                for h in &it.blockers {
                    if !(protect && h.critical) {
                        busy.insert(h.pid, h.name.clone());
                    }
                }
            }
        }
        let kill_busy = !busy.is_empty() && {
            let names: Vec<String> = busy.iter().map(|(p, n)| format!("{n} ({p})")).collect();
            ask(
                "Файлы заняты",
                &format!(
                    "Заняты процессами:\n{}\n\nЗавершить их и продолжить? Несохранённые данные будут потеряны.",
                    names.join(", ")
                ),
            )
        };

        self.running = true;
        self.cancel.store(false, Ordering::Relaxed);
        self.progress = (0, jobs.len());
        self.status = format!("{}…", op.label());

        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let cancel = self.cancel.clone();
        let guard = self.guard.clone();
        std::thread::spawn(move || {
            let total = jobs.len();
            for (i, (p, target)) in jobs.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    let _ = tx.send(Msg::Log("Отмена".into(), "Прервано пользователем".into()));
                    break;
                }
                let label = format!("{}: {}", op.label(), p.display());
                if protect && guard.is_protected(p) {
                    let _ = tx.send(Msg::Log(label, "Пропущено: защищённый путь".into()));
                } else {
                    if kill_busy {
                        for h in sys::holders(p).unwrap_or_default() {
                            if !(protect && h.critical) {
                                sys::kill(h.pid);
                            }
                        }
                        std::thread::sleep(Duration::from_millis(150));
                    }
                    let mut res = run_op(op, p, target.as_deref());
                    // Повтор имеет смысл только для операций, безопасных к повторному запуску.
                    if res.is_err() && kill_busy && matches!(op, Op::Trash | Op::Delete | Op::Rename) {
                        std::thread::sleep(Duration::from_millis(300));
                        res = run_op(op, p, target.as_deref());
                    }
                    let ok = res.is_ok();
                    let text = match res {
                        Ok(()) => "Успешно".to_string(),
                        Err(e) => format!("Ошибка: {e}"),
                    };
                    if ok && op == Op::Rename {
                        if let Some(t) = target {
                            let _ = tx.send(Msg::Renamed(p.clone(), t.clone()));
                        }
                    }
                    let _ = tx.send(Msg::Log(label, text));
                }
                let _ = tx.send(Msg::Progress(i + 1, total));
                ctx.request_repaint();
            }
            let _ = tx.send(Msg::Done);
            ctx.request_repaint();
        });
    }

    fn kill_selected(&mut self) {
        if self.selected_procs.is_empty() {
            return;
        }
        if !ask("Подтверждение", "Завершить выбранные процессы? Несохранённые данные в них будут потеряны.") {
            return;
        }
        let pids: Vec<u32> = self.selected_procs.drain().collect();
        for pid in pids {
            let ok = sys::kill(pid);
            self.push_log(
                "Завершение процесса",
                format!("PID {pid}: {}", if ok { "успешно" } else { "не удалось" }),
            );
        }
        self.rescan_all();
    }

    fn open_rename(&mut self) {
        let sources = self.checked_paths();
        if !sources.is_empty() {
            self.rename = Some(RenameState { template: "{имя}".into(), sources });
        }
    }

    fn delete_on_reboot(&mut self) {
        for p in self.checked_paths() {
            let label = format!("Удаление при перезагрузке: {}", p.display());
            if self.settings.protection && self.guard.is_protected(&p) {
                self.push_log(label, "Пропущено: защищённый путь");
            } else if sys::delete_on_reboot(&p) {
                self.push_log(label, "Запланировано");
            } else {
                self.push_log(label, "Ошибка (нужны права администратора)");
            }
        }
    }

    // ----- панели -----

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if btn(ui, icon::ADD, "Добавить файлы", true).clicked() {
                    if let Some(f) = FileDialog::new().pick_files() {
                        self.add_paths(f);
                    }
                }
                if btn(ui, icon::FOLDER, "Добавить папку", true).clicked() {
                    if let Some(f) = FileDialog::new().pick_folders() {
                        self.add_paths(f);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if btn(ui, icon::SETTINGS, "Настройки", true).clicked() {
                        self.show_settings = !self.show_settings;
                    }
                });
            });
            ui.add_space(6.0);
        });
    }

    fn journal_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("journal").show(ctx, |ui| {
            egui::CollapsingHeader::new("Журнал").default_open(false).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(140.0).stick_to_bottom(true).show(ui, |ui| {
                    egui::Grid::new("log_grid").striped(true).num_columns(3).show(ui, |ui| {
                        ui.strong("Время");
                        ui.strong("Действие");
                        ui.strong("Результат");
                        ui.end_row();
                        for (t, a, r) in &self.log {
                            ui.label(t);
                            ui.label(a);
                            ui.label(r);
                            ui.end_row();
                        }
                    });
                });
            });
        });
    }

    fn progress_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("progress").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let (d, t) = self.progress;
                let frac = if t == 0 { 0.0 } else { d as f32 / t as f32 };
                ui.add(egui::ProgressBar::new(frac).desired_width(300.0));
                if t > 0 {
                    ui.label(format!("Обработано {d} из {t}, {}%", (frac * 100.0) as u32));
                }
                if btn(ui, icon::CANCEL, "Отмена", self.running).clicked() {
                    self.cancel.store(true, Ordering::Relaxed);
                }
                ui.label(&self.status);
            });
            ui.add_space(6.0);
        });
    }

    fn actions_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("actions").show(ctx, |ui| {
            let en = self.items.iter().any(|i| i.checked) && !self.running;
            let dark = self.settings.dark;
            let (dfg, dbg) = if dark {
                (Color32::from_rgb(255, 120, 120), Color32::from_rgb(78, 36, 40))
            } else {
                (Color32::from_rgb(180, 20, 20), Color32::from_rgb(253, 226, 226))
            };
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if btn_colored(ui, icon::DELETE, "В Корзину", Color32::WHITE, accent(dark), en).clicked() {
                    self.start_op(Op::Trash);
                }
                if btn_colored(ui, icon::DELETE, "Удалить навсегда", dfg, dbg, en).clicked() {
                    self.start_op(Op::Delete);
                }
                if btn(ui, icon::MOVE, "Переместить…", en).clicked() {
                    self.start_op(Op::Move);
                }
                if btn(ui, icon::COPY, "Копировать…", en).clicked() {
                    self.start_op(Op::Copy);
                }
                if btn(ui, icon::EDIT, "Переименовать…", en).clicked() {
                    self.open_rename();
                }
                if btn(ui, icon::CLOCK, "Удалить при перезагрузке", en).clicked() {
                    self.delete_on_reboot();
                }
            });
            ui.add_space(6.0);
        });
    }

    fn blockers_panel(&mut self, ctx: &egui::Context) {
        let open = self.settings.blockers_open;
        let mut panel = egui::SidePanel::right("blockers");
        panel = if open {
            panel.default_width(270.0).resizable(true)
        } else {
            panel.exact_width(240.0).resizable(false)
        };
        panel.show(ctx, |ui| {
            let mut map: BTreeMap<u32, Holder> = BTreeMap::new();
            for it in &self.items {
                for h in &it.blockers {
                    map.entry(h.pid).or_insert_with(|| h.clone());
                }
            }
            ui.add_space(6.0);
            let chevron = if open { icon::CHEVRON_UP } else { icon::CHEVRON_DOWN };
            let title = format!("Блокирующие процессы ({})", map.len());
            if btn(ui, chevron, &title, true).clicked() {
                self.settings.blockers_open = !open;
                self.settings.save();
            }
            if !open {
                return;
            }
            ui.separator();
            if map.is_empty() {
                ui.weak("Блокировок не найдено");
            }
            let protect = self.settings.protection;
            egui::ScrollArea::vertical().max_height((ui.available_height() - 50.0).max(40.0)).show(ui, |ui| {
                for (pid, h) in &map {
                    let locked = protect && h.critical;
                    ui.add_enabled_ui(!locked, |ui| {
                        let mut sel = self.selected_procs.contains(pid);
                        if ui.checkbox(&mut sel, format!("{} (PID {pid})", h.name)).changed() {
                            if sel {
                                self.selected_procs.insert(*pid);
                            } else {
                                self.selected_procs.remove(pid);
                            }
                        }
                    });
                    if locked {
                        ui.weak("   защищённый процесс");
                    }
                }
            });
            ui.add_space(6.0);
            let en = !self.selected_procs.is_empty() && !self.running;
            if btn(ui, icon::STOP, "Завершить выбранные", en).clicked() {
                self.kill_selected();
            }
        });
    }

    fn table_panel(&mut self, ctx: &egui::Context, hovering: bool) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let dark = self.settings.dark;
            if hovering {
                ui.painter().rect_stroke(
                    ui.max_rect().shrink(2.0),
                    8.0,
                    egui::Stroke::new(2.0, accent(dark)),
                );
            }
            if self.items.is_empty() {
                let rect = ui.available_rect_before_wrap().shrink(12.0);
                let color = ui.visuals().weak_text_color();
                let pts = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
                ui.painter().extend(egui::Shape::dashed_line(&pts, egui::Stroke::new(1.5, color), 8.0, 6.0));
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "Перетащите файлы и папки сюда",
                    FontId::proportional(18.0),
                    color,
                );
                return;
            }

            let statuses: Vec<Status> = self.items.iter().map(|i| self.status_of(i)).collect();
            let n = self.items.len();
            let mut all = self.items.iter().all(|i| i.checked);
            let items = &mut self.items;

            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .column(Column::exact(30.0))
                .column(Column::initial(200.0).at_least(80.0).clip(true))
                .column(Column::initial(240.0).at_least(80.0).clip(true))
                .column(Column::initial(80.0).at_least(50.0))
                .column(Column::remainder().at_least(100.0))
                .header(28.0, |mut h| {
                    h.col(|ui| {
                        if ui.checkbox(&mut all, "").changed() {
                            for it in items.iter_mut() {
                                it.checked = all;
                            }
                        }
                    });
                    h.col(|ui| {
                        ui.strong("Имя");
                    });
                    h.col(|ui| {
                        ui.strong("Путь");
                    });
                    h.col(|ui| {
                        ui.strong("Размер");
                    });
                    h.col(|ui| {
                        ui.strong("Статус");
                    });
                })
                .body(|body| {
                    body.rows(30.0, n, |mut row| {
                        let i = row.index();
                        row.col(|ui| {
                            ui.checkbox(&mut items[i].checked, "");
                        });
                        row.col(|ui| {
                            let ic = if items[i].is_dir { icon::FOLDER } else { icon::FILE };
                            let c = ui.visuals().text_color();
                            ui.label(rich(ic, &items[i].name, c));
                        });
                        row.col(|ui| {
                            ui.label(&items[i].parent);
                        });
                        row.col(|ui| {
                            ui.label(items[i].size.map(format_size).unwrap_or_else(|| "—".into()));
                        });
                        row.col(|ui| pill(ui, statuses[i], dark));
                    });
                });
        });
    }

    fn windows(&mut self, ctx: &egui::Context) {
        self.settings_window(ctx);
        self.rename_window(ctx);
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut open = true;
        let mut s = self.settings.clone();
        let mut restart = false;
        let is_admin = self.is_admin;
        egui::Window::new("Настройки")
            .anchor(Align2::RIGHT_TOP, [-12.0, 52.0])
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Тема:");
                    ui.radio_value(&mut s.dark, true, "Тёмная");
                    ui.radio_value(&mut s.dark, false, "Светлая");
                });
                egui::ComboBox::from_label("Значки")
                    .selected_text(icon_set_name(s.icons))
                    .show_ui(ui, |ui| {
                        for set in [IconSet::Fluent, IconSet::Mdl2, IconSet::None] {
                            ui.selectable_value(&mut s.icons, set, icon_set_name(set));
                        }
                    });
                ui.checkbox(&mut s.protection, "Защита системных папок и процессов");
                ui.separator();
                ui.label(if is_admin { "Права: администратор" } else { "Права: обычный пользователь" });
                if !is_admin && ui.button("Перезапустить от имени администратора").clicked() {
                    restart = true;
                }
            });

        if s.dark != self.settings.dark {
            apply_theme(ctx, s.dark);
        }
        if s.icons != self.settings.icons {
            setup_fonts(ctx, s.icons);
        }
        if !s.protection
            && self.settings.protection
            && !ask(
                "Отключить защиту?",
                "Без защиты можно удалить системные файлы и завершить критичные процессы, что может сломать Windows. Отключить?",
            )
        {
            s.protection = true;
        }
        if s != self.settings {
            self.settings = s;
            self.settings.save();
        }
        self.show_settings = open;

        if restart {
            if sys::restart_as_admin() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.status = "Перезапуск отменён или не удался".into();
            }
        }
    }

    fn rename_window(&mut self, ctx: &egui::Context) {
        let mut action: Option<bool> = None;
        let mut jobs: Vec<Job> = Vec::new();
        if let Some(st) = &mut self.rename {
            let plans = plan_rename(&st.sources, &st.template, &|p| p.exists());
            let has_err = plans.iter().any(|p| p.error.is_some());
            let any_change = plans.iter().any(|p| p.old_name != p.new_name);
            egui::Window::new("Массовое переименование")
                .collapsible(false)
                .resizable(true)
                .default_width(520.0)
                .show(ctx, |ui| {
                    ui.label("Шаблон имени:");
                    ui.text_edit_singleline(&mut st.template);
                    ui.weak("{имя} прежнее имя, {номер} или {номер:3} счётчик, {расш} расширение. Расширение сохраняется само, если в шаблоне нет точки.");
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        egui::Grid::new("rn_grid").striped(true).num_columns(3).show(ui, |ui| {
                            ui.strong("Было");
                            ui.strong("Станет");
                            ui.strong("");
                            ui.end_row();
                            for p in &plans {
                                ui.label(&p.old_name);
                                ui.label(&p.new_name);
                                match p.error {
                                    Some(e) => {
                                        ui.colored_label(Color32::from_rgb(220, 60, 60), e);
                                    }
                                    None => {
                                        ui.label("");
                                    }
                                }
                                ui.end_row();
                            }
                        });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if btn(ui, icon::OK, "Применить", !has_err && any_change).clicked() {
                            jobs = plans
                                .iter()
                                .filter(|p| p.error.is_none() && p.old_name != p.new_name)
                                .map(|p| (p.src.clone(), Some(p.src.with_file_name(&p.new_name))))
                                .collect();
                            action = Some(true);
                        }
                        if btn(ui, icon::CANCEL, "Отмена", true).clicked() {
                            action = Some(false);
                        }
                    });
                });
        }
        match action {
            Some(true) => {
                self.rename = None;
                self.launch(Op::Rename, jobs);
            }
            Some(false) => self.rename = None,
            None => {}
        }
    }
}

fn pill(ui: &mut egui::Ui, st: Status, dark: bool) {
    let (ic, text, fg) = match st {
        Status::Free => (
            icon::OK,
            "Свободен",
            if dark { Color32::from_rgb(74, 222, 128) } else { Color32::from_rgb(22, 128, 56) },
        ),
        Status::Busy => (
            icon::CLOCK,
            "Занят",
            if dark { Color32::from_rgb(255, 159, 10) } else { Color32::from_rgb(204, 102, 0) },
        ),
        Status::Protected => (
            icon::SHIELD,
            "Защищён",
            if dark { Color32::from_rgb(255, 107, 107) } else { Color32::from_rgb(200, 30, 30) },
        ),
        Status::Unknown => ("", "Не проверено", ui.visuals().weak_text_color()),
        Status::Checking => ("", "Проверка…", ui.visuals().weak_text_color()),
    };
    let bg = Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 40);
    egui::Frame::none()
        .fill(bg)
        .rounding(12.0)
        .inner_margin(egui::Margin::symmetric(8.0, 1.0))
        .show(ui, |ui| {
            ui.label(rich(ic, text, fg));
        });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain();

        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            self.add_paths(dropped);
        }
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if self.running {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        // Порядок важен: сначала крайние панели, затем боковая, затем центр.
        self.top_bar(ctx);
        self.journal_panel(ctx);
        self.progress_panel(ctx);
        self.actions_panel(ctx);
        self.blockers_panel(ctx);
        self.table_panel(ctx, hovering);
        self.windows(ctx);
    }
}
