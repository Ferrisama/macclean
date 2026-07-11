use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};

use crate::cleaners::{self, health::HealthSnapshot, uninstall};
use crate::core::cmd::is_root;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Clean,
    Uninstall,
    Explore,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UninstallScreen {
    List,
    Reviewing,
}

/// The one case the key/mouse handlers hand back to the event loop directly:
/// running the selected cleaners needs the alternate screen torn down first,
/// since it shells out to the existing, chatty `Cleaner::clean()` code path
/// that prints straight to stdout. Everything else that used to block here
/// (analyze/build-plan/execute/scan) now runs on a background thread instead
/// -- see `BgEvent` -- so Tab/navigation keys are never stuck waiting on a
/// filesystem walk.
pub enum Action {
    None,
    RunClean,
}

/// Results delivered back from background threads. `poll_bg` drains these
/// every tick and applies them to state; the main thread never blocks on
/// filesystem or subprocess work outside of `run_selected_clean`.
pub enum BgEvent {
    CleanSize(usize, Option<u64>),
    CleanAnalyzeDone,
    UninstallPlanReady(Result<uninstall::UninstallPlan, String>),
    UninstallExecuted { app_name: String, dry_run: bool, trashed: usize, failed: usize },
    ExploreScanned(PathBuf, Vec<ExploreEntry>),
    ExploreDeleted { name: String, result: Result<(), String> },
}

pub struct ExploreEntry {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
}

pub struct CleanCategory {
    pub key: &'static str,
    pub label: &'static str,
    pub needs_sudo: bool,
    pub selected: bool,
    pub size: Option<u64>,
}

const CLEAN_CATALOG: &[(&str, &str, bool)] = &[
    ("trash", "Trash", false),
    ("system", "System Caches & Logs", true),
    ("browser", "Browser Caches", false),
    ("brew", "Homebrew", false),
    ("docker", "Docker", false),
    ("xcode", "Xcode Data", false),
    ("node", "Node.js Caches", false),
    ("pip", "pip Cache", false),
    ("cargo", "Cargo Cache", false),
    ("crash-reports", "Crash Reports", false),
    ("projects", "Project Artifacts", false),
];

const QUICK_PRESET: &[&str] = &["trash", "browser", "crash-reports"];
const DEV_PRESET: &[&str] = &["brew", "docker", "node", "pip", "cargo", "xcode", "projects"];
const DEEP_PRESET: &[&str] = &[
    "trash", "system", "browser", "docker", "brew", "xcode", "node", "pip", "cargo",
    "crash-reports", "projects",
];

pub struct App {
    pub tab: Tab,
    pub should_quit: bool,
    pub dry_run: bool,
    pub is_root: bool,
    pub status: String,

    bg_tx: Sender<BgEvent>,
    bg_rx: Receiver<BgEvent>,

    // Dashboard
    pub health: Option<HealthSnapshot>,

    // Clean
    pub clean_categories: Vec<CleanCategory>,
    pub clean_cursor: usize,
    pub clean_analyzed: bool,
    pub clean_loading: bool,

    // Uninstall
    pub uninstall_apps: Vec<(String, PathBuf)>,
    pub uninstall_filter: String,
    pub uninstall_cursor: usize,
    pub uninstall_screen: UninstallScreen,
    pub uninstall_plan: Option<uninstall::UninstallPlan>,
    pub uninstall_loading: bool,

    // Explore
    pub explore_dir: PathBuf,
    pub explore_entries: Vec<ExploreEntry>,
    pub explore_cursor: usize,
    pub explore_history: Vec<PathBuf>,
    pub explore_scanned: bool,
    pub explore_loading: bool,
    pub explore_confirm_delete: Option<usize>,

    // Layout hit-testing, recorded each draw so mouse clicks can be mapped
    // back to what's on screen.
    pub tab_rects: [Rect; 4],
    pub clean_row_rects: Vec<Rect>,
    pub uninstall_row_rects: Vec<(Rect, usize)>,
    pub explore_row_rects: Vec<Rect>,
}

impl App {
    /// `yes` (the CLI's global `--yes`) has no TUI equivalent: every
    /// destructive action here already requires an explicit in-app
    /// confirmation (category selection + Enter for Clean, the Review
    /// screen for Uninstall) that a scripted `--yes` shouldn't bypass.
    pub fn new(dry_run: bool, _yes: bool) -> Self {
        let clean_categories = CLEAN_CATALOG
            .iter()
            .map(|(key, label, needs_sudo)| CleanCategory {
                key,
                label,
                needs_sudo: *needs_sudo,
                selected: false,
                size: None,
            })
            .collect();

        let (bg_tx, bg_rx) = channel();

        App {
            tab: Tab::Dashboard,
            should_quit: false,
            dry_run,
            is_root: is_root(),
            status: String::new(),
            bg_tx,
            bg_rx,
            health: None,
            clean_categories,
            clean_cursor: 0,
            clean_analyzed: false,
            clean_loading: false,
            uninstall_apps: uninstall::list_installed_apps(),
            uninstall_filter: String::new(),
            uninstall_cursor: 0,
            uninstall_screen: UninstallScreen::List,
            uninstall_plan: None,
            uninstall_loading: false,
            explore_dir: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
            explore_entries: Vec::new(),
            explore_cursor: 0,
            explore_history: Vec::new(),
            explore_scanned: false,
            explore_loading: false,
            explore_confirm_delete: None,
            tab_rects: [Rect::default(); 4],
            clean_row_rects: Vec::new(),
            uninstall_row_rects: Vec::new(),
            explore_row_rects: Vec::new(),
        }
    }

    /// Drain any results background threads have finished computing. Called
    /// once per event-loop tick, whether or not a key/mouse event arrived --
    /// this is what makes size numbers and scan results appear live without
    /// ever blocking input.
    pub fn poll_bg(&mut self) {
        while let Ok(event) = self.bg_rx.try_recv() {
            match event {
                BgEvent::CleanSize(i, size) => {
                    if let Some(cat) = self.clean_categories.get_mut(i) {
                        cat.size = size;
                    }
                }
                BgEvent::CleanAnalyzeDone => {
                    self.clean_loading = false;
                    self.clean_analyzed = true;
                    self.status = "Ready.".to_string();
                }
                BgEvent::UninstallPlanReady(result) => {
                    self.uninstall_loading = false;
                    match result {
                        Ok(plan) => {
                            self.uninstall_plan = Some(plan);
                            self.uninstall_screen = UninstallScreen::Reviewing;
                            self.status.clear();
                        }
                        Err(e) => self.status = format!("Error: {}", e),
                    }
                }
                BgEvent::UninstallExecuted { app_name, dry_run, trashed, failed } => {
                    self.uninstall_loading = false;
                    self.uninstall_screen = UninstallScreen::List;
                    self.uninstall_apps = uninstall::list_installed_apps();
                    self.uninstall_cursor = 0;
                    self.status = if dry_run {
                        format!("Dry run -- {} not touched.", app_name)
                    } else {
                        format!("Uninstalled {}: {} moved to Trash, {} failed.", app_name, trashed, failed)
                    };
                }
                BgEvent::ExploreScanned(dir, entries) => {
                    self.explore_loading = false;
                    // Discard results for a directory the user has already
                    // navigated away from.
                    if dir == self.explore_dir {
                        self.explore_entries = entries;
                        self.explore_cursor = 0;
                        self.explore_scanned = true;
                        self.status.clear();
                    }
                }
                BgEvent::ExploreDeleted { name, result } => {
                    self.explore_loading = false;
                    match result {
                        Ok(_) => {
                            self.status = format!("Moved {} to Trash.", name);
                            self.start_explore_scan();
                        }
                        Err(e) => self.status = format!("Failed to trash {}: {}", name, e),
                    }
                }
            }
        }
    }

    pub fn refresh_dashboard(&mut self) {
        self.health = Some(cleaners::health::snapshot());
    }

    pub fn filtered_uninstall(&self) -> Vec<usize> {
        let f = self.uninstall_filter.to_lowercase();
        self.uninstall_apps
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| f.is_empty() || name.to_lowercase().contains(&f))
            .map(|(i, _)| i)
            .collect()
    }

    fn switch_tab(&mut self, tab: Tab) {
        self.tab = tab;
        if tab == Tab::Clean && !self.clean_analyzed {
            self.start_analyze_clean();
        }
        if tab == Tab::Explore && !self.explore_scanned {
            self.start_explore_scan();
        }
    }

    fn next_tab_variant(&self) -> Tab {
        match self.tab {
            Tab::Dashboard => Tab::Clean,
            Tab::Clean => Tab::Uninstall,
            Tab::Uninstall => Tab::Explore,
            Tab::Explore => Tab::Dashboard,
        }
    }

    fn prev_tab_variant(&self) -> Tab {
        match self.tab {
            Tab::Dashboard => Tab::Explore,
            Tab::Clean => Tab::Dashboard,
            Tab::Uninstall => Tab::Clean,
            Tab::Explore => Tab::Uninstall,
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Action::None;
        }
        match key.code {
            KeyCode::Tab => {
                self.switch_tab(self.next_tab_variant());
                return Action::None;
            }
            KeyCode::BackTab => {
                self.switch_tab(self.prev_tab_variant());
                return Action::None;
            }
            _ => {}
        }

        match self.tab {
            Tab::Dashboard => self.on_key_dashboard(key),
            Tab::Clean => self.on_key_clean(key),
            Tab::Uninstall => self.on_key_uninstall(key),
            Tab::Explore => self.on_key_explore(key),
        }
    }

    fn on_key_dashboard(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('r') => self.refresh_dashboard(),
            _ => {}
        }
        Action::None
    }

    fn apply_preset(&mut self, keys: &[&str]) {
        for cat in &mut self.clean_categories {
            cat.selected = keys.contains(&cat.key);
        }
    }

    fn on_key_clean(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => {
                self.clean_cursor = self.clean_cursor.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.clean_cursor + 1 < self.clean_categories.len() {
                    self.clean_cursor += 1;
                }
            }
            KeyCode::Char(' ') => {
                if let Some(cat) = self.clean_categories.get_mut(self.clean_cursor) {
                    cat.selected = !cat.selected;
                }
            }
            KeyCode::Char('a') => {
                for c in &mut self.clean_categories {
                    c.selected = true;
                }
            }
            KeyCode::Char('n') => {
                for c in &mut self.clean_categories {
                    c.selected = false;
                }
            }
            KeyCode::Char('1') => self.apply_preset(QUICK_PRESET),
            KeyCode::Char('2') => self.apply_preset(DEV_PRESET),
            KeyCode::Char('3') => self.apply_preset(DEEP_PRESET),
            KeyCode::Char('R') => self.start_analyze_clean(),
            KeyCode::Enter | KeyCode::Char('c')
                if self.clean_categories.iter().any(|c| c.selected) =>
            {
                return Action::RunClean;
            }
            _ => {}
        }
        Action::None
    }

    fn start_analyze_clean(&mut self) {
        if self.clean_loading {
            return;
        }
        self.clean_loading = true;
        self.status = "Analyzing categories...".to_string();
        let keys: Vec<&'static str> = self.clean_categories.iter().map(|c| c.key).collect();
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            for (i, key) in keys.into_iter().enumerate() {
                let size = cleaners::cleaner_by_name(key).and_then(|c| c.analyze().ok()).map(|r| r.total_bytes());
                if tx.send(BgEvent::CleanSize(i, size)).is_err() {
                    return;
                }
            }
            let _ = tx.send(BgEvent::CleanAnalyzeDone);
        });
    }

    /// Runs after the TUI has torn down the alternate screen -- stdout here
    /// is the user's normal terminal, exactly like running the CLI directly.
    /// Unlike everything else in this file, this one is meant to block: the
    /// user is watching real cleaner output scroll by, not waiting on a
    /// silent background scan.
    pub fn run_selected_clean(&mut self) {
        println!();
        println!("=== Running selected cleaners ===");
        for cat in &self.clean_categories {
            if !cat.selected {
                continue;
            }
            if cat.needs_sudo && !self.is_root {
                println!(
                    "  ! {} needs root -- restart with `sudo macclean` to include it.",
                    cat.label
                );
                continue;
            }
            let Some(cleaner) = cleaners::cleaner_by_name(cat.key) else { continue };
            match cleaner.analyze() {
                Ok(result) => {
                    if let Err(e) = cleaner.clean(&result, self.dry_run, true) {
                        crate::ui::print_warn(&format!("{}: {}", cleaner.display_name(), e));
                    }
                }
                Err(e) => crate::ui::print_err(&format!("Error in {}: {}", cleaner.display_name(), e)),
            }
        }
        println!();
        print!("Press Enter to return to the dashboard...");
        let _ = std::io::stdout().flush();
        let mut buf = String::new();
        let _ = std::io::stdin().read_line(&mut buf);

        self.clean_analyzed = false;
        for cat in &mut self.clean_categories {
            cat.size = None;
            cat.selected = false;
        }
        self.status = "Cleaned. Press 'R' to re-analyze.".to_string();
    }

    fn on_key_uninstall(&mut self, key: KeyEvent) -> Action {
        match self.uninstall_screen {
            UninstallScreen::List => self.on_key_uninstall_list(key),
            UninstallScreen::Reviewing => self.on_key_uninstall_review(key),
        }
    }

    fn on_key_uninstall_list(&mut self, key: KeyEvent) -> Action {
        let filtered = self.filtered_uninstall();
        match key.code {
            KeyCode::Esc => {
                self.uninstall_filter.clear();
                self.uninstall_cursor = 0;
            }
            KeyCode::Up => self.uninstall_cursor = self.uninstall_cursor.saturating_sub(1),
            KeyCode::Down => {
                if self.uninstall_cursor + 1 < filtered.len() {
                    self.uninstall_cursor += 1;
                }
            }
            KeyCode::Backspace => {
                self.uninstall_filter.pop();
                self.uninstall_cursor = 0;
            }
            KeyCode::Enter => {
                if let Some(&idx) = filtered.get(self.uninstall_cursor) {
                    let app_name = self.uninstall_apps[idx].0.clone();
                    self.start_build_uninstall_plan(app_name);
                }
            }
            KeyCode::Char(c) => {
                self.uninstall_filter.push(c);
                self.uninstall_cursor = 0;
            }
            _ => {}
        }
        Action::None
    }

    fn on_key_uninstall_review(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') => {
                self.uninstall_plan = None;
                self.uninstall_screen = UninstallScreen::List;
                self.status.clear();
            }
            KeyCode::Enter | KeyCode::Char('y') => self.start_execute_uninstall(),
            _ => {}
        }
        Action::None
    }

    fn start_build_uninstall_plan(&mut self, app_name: String) {
        if self.uninstall_loading {
            return;
        }
        self.uninstall_loading = true;
        self.status = "Scanning for app traces...".to_string();
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let result = uninstall::build_plan(&app_name).map_err(|e| e.to_string());
            let _ = tx.send(BgEvent::UninstallPlanReady(result));
        });
    }

    fn start_execute_uninstall(&mut self) {
        let Some(plan) = self.uninstall_plan.take() else { return };
        if self.uninstall_loading {
            return;
        }
        self.uninstall_loading = true;
        self.status = "Working...".to_string();
        let dry_run = self.dry_run;
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let app_name = plan.app_name.clone();
            if dry_run {
                let _ = tx.send(BgEvent::UninstallExecuted {
                    app_name,
                    dry_run: true,
                    trashed: 0,
                    failed: 0,
                });
                return;
            }
            let results = uninstall::execute(&plan);
            let trashed = results.iter().filter(|(_, r)| r.is_ok()).count();
            let failed = results.len() - trashed;
            let _ = tx.send(BgEvent::UninstallExecuted { app_name, dry_run: false, trashed, failed });
        });
    }

    fn on_key_explore(&mut self, key: KeyEvent) -> Action {
        if self.explore_confirm_delete.is_some() {
            match key.code {
                KeyCode::Enter | KeyCode::Char('y') => self.start_explore_delete(),
                KeyCode::Esc | KeyCode::Char('n') => {
                    self.explore_confirm_delete = None;
                    self.status.clear();
                }
                _ => {}
            }
            return Action::None;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => {
                self.explore_cursor = self.explore_cursor.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.explore_cursor + 1 < self.explore_entries.len() {
                    self.explore_cursor += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(entry) = self.explore_entries.get(self.explore_cursor) {
                    if entry.is_dir {
                        self.explore_history.push(self.explore_dir.clone());
                        self.explore_dir = entry.path.clone();
                        self.explore_scanned = false;
                        self.start_explore_scan();
                    }
                }
            }
            KeyCode::Backspace | KeyCode::Char('u') => {
                if let Some(parent) = self.explore_history.pop() {
                    self.explore_dir = parent;
                    self.explore_scanned = false;
                    self.start_explore_scan();
                }
            }
            KeyCode::Char('d') if !self.explore_entries.is_empty() => {
                self.explore_confirm_delete = Some(self.explore_cursor);
            }
            _ => {}
        }
        Action::None
    }

    fn start_explore_scan(&mut self) {
        if self.explore_loading {
            return;
        }
        self.explore_loading = true;
        self.status = "Scanning...".to_string();
        let dir = self.explore_dir.clone();
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let mut entries = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for entry in rd.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_dir = path.is_dir() && !path.is_symlink();
                    let size = if is_dir {
                        crate::core::fs::dir_size(&path)
                    } else {
                        entry.metadata().map(|m| m.len()).unwrap_or(0)
                    };
                    entries.push(ExploreEntry { path, name, size, is_dir });
                }
            }
            entries.sort_by_key(|e| std::cmp::Reverse(e.size));
            let _ = tx.send(BgEvent::ExploreScanned(dir, entries));
        });
    }

    fn start_explore_delete(&mut self) {
        let Some(idx) = self.explore_confirm_delete.take() else { return };
        let Some(entry) = self.explore_entries.get(idx) else { return };
        let path = entry.path.clone();
        let name = entry.name.clone();

        if self.dry_run {
            self.status = format!("Dry run -- {} not touched.", name);
            return;
        }

        self.explore_loading = true;
        self.status = "Working...".to_string();
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let result = crate::core::trash::trash_paths(&[path])
                .into_iter()
                .next()
                .map(|(_, r)| r)
                .unwrap_or(Ok(()));
            let _ = tx.send(BgEvent::ExploreDeleted { name, result });
        });
    }

    pub fn on_click(&mut self, col: u16, row: u16) -> Action {
        let point = Position::new(col, row);

        for (i, rect) in self.tab_rects.iter().enumerate() {
            if rect.contains(point) {
                let tab = match i {
                    0 => Tab::Dashboard,
                    1 => Tab::Clean,
                    2 => Tab::Uninstall,
                    _ => Tab::Explore,
                };
                self.switch_tab(tab);
                return Action::None;
            }
        }

        if self.tab == Tab::Clean {
            for (i, rect) in self.clean_row_rects.iter().enumerate() {
                if rect.contains(point) {
                    self.clean_cursor = i;
                    if let Some(c) = self.clean_categories.get_mut(i) {
                        c.selected = !c.selected;
                    }
                    return Action::None;
                }
            }
        }

        if self.tab == Tab::Uninstall && self.uninstall_screen == UninstallScreen::List {
            let filtered = self.filtered_uninstall();
            for &(rect, idx) in &self.uninstall_row_rects {
                if rect.contains(point) {
                    self.uninstall_cursor = idx;
                    if let Some(&fi) = filtered.get(idx) {
                        let app_name = self.uninstall_apps[fi].0.clone();
                        self.start_build_uninstall_plan(app_name);
                    }
                    return Action::None;
                }
            }
        }

        if self.tab == Tab::Explore && self.explore_confirm_delete.is_none() {
            for (i, rect) in self.explore_row_rects.iter().enumerate() {
                if rect.contains(point) {
                    self.explore_cursor = i;
                    if let Some(entry) = self.explore_entries.get(i) {
                        if entry.is_dir {
                            self.explore_history.push(self.explore_dir.clone());
                            self.explore_dir = entry.path.clone();
                            self.explore_scanned = false;
                            self.start_explore_scan();
                        }
                    }
                    return Action::None;
                }
            }
        }

        Action::None
    }
}
