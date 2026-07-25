use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::Frame;

use super::app::{App, Tab, UninstallScreen};
use crate::core::storage::StorageCategory;
use crate::ui::format_size;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_tab_bar(frame, app, chunks[0]);
    match app.tab {
        Tab::Dashboard => draw_dashboard(frame, app, chunks[1]),
        Tab::SystemData => draw_system_data(frame, app, chunks[1]),
        Tab::Clean => draw_clean(frame, app, chunks[1]),
        Tab::Uninstall => draw_uninstall(frame, app, chunks[1]),
        Tab::Explore => draw_explore(frame, app, chunks[1]),
    }
    draw_status_bar(frame, app, chunks[2]);
}

fn draw_tab_bar(frame: &mut Frame, app: &mut App, area: Rect) {
    let titles = ["Dashboard", "System Data", "Clean", "Uninstall", "Explore"];
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 5); 5])
        .split(area);

    for (i, title) in titles.iter().enumerate() {
        app.tab_rects[i] = cols[i];
        let active = i == app.tab as usize;
        let style = if active {
            Style::new().fg(Color::Black).bg(Color::Cyan).bold()
        } else {
            Style::new().fg(Color::Cyan)
        };
        let p = Paragraph::new(format!("[{}] {}", i + 1, title))
            .style(style)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(p, cols[i]);
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let help = match app.tab {
        Tab::Dashboard => "r: refresh   Tab: switch panel   Ctrl+C: quit",
        Tab::SystemData => "r: rescan   Tab: switch panel   Ctrl+C: quit",
        Tab::Clean => {
            "Space: toggle  a: all  n: none  1/2/3: quick/dev/deep preset  Enter: run  R: re-analyze  Tab: switch panel"
        }
        Tab::Uninstall => match app.uninstall_screen {
            UninstallScreen::List => "type to search  Enter: select  Esc: clear filter  Tab: switch panel",
            UninstallScreen::Reviewing => "Enter/y: confirm  Esc/n: cancel",
        },
        Tab::Explore => {
            if app.explore_confirm_delete.is_some() {
                "Enter/y: move to Trash  Esc/n: cancel"
            } else {
                "Enter: open folder  Backspace/u: up  d: trash item  Tab: switch panel"
            }
        }
    };
    let text = if app.status.is_empty() {
        help.to_string()
    } else {
        format!("{}   |   {}", app.status, help)
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::new().fg(Color::DarkGray)),
        area,
    );
}

fn gauge_color(pct: u16) -> Color {
    if pct >= 90 {
        Color::Red
    } else if pct >= 75 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn draw_dashboard(frame: &mut Frame, app: &App, area: Rect) {
    let Some(h) = &app.health else {
        frame.render_widget(Paragraph::new("Loading..."), area);
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let disk_pct = if h.disk_total > 0 {
        ((h.disk_used as f64 / h.disk_total as f64) * 100.0) as u16
    } else {
        0
    };
    let mem_pct = if h.mem_total > 0 {
        ((h.mem_used as f64 / h.mem_total as f64) * 100.0) as u16
    } else {
        0
    };

    frame.render_widget(
        Gauge::default()
            .block(Block::bordered().title(format!("Disk ({} free)", format_size(h.disk_free))))
            .gauge_style(Style::new().fg(gauge_color(disk_pct)))
            .percent(disk_pct.min(100)),
        rows[0],
    );

    frame.render_widget(
        Gauge::default()
            .block(Block::bordered().title(format!(
                "Memory ({} / {})",
                format_size(h.mem_used),
                format_size(h.mem_total)
            )))
            .gauge_style(Style::new().fg(gauge_color(mem_pct)))
            .percent(mem_pct.min(100)),
        rows[1],
    );

    frame.render_widget(
        Paragraph::new(format!(
            "CPU: {} cores  |  Load avg: {}      Battery: {}",
            h.ncpu, h.load_avg, h.battery
        ))
        .block(Block::bordered().title("System")),
        rows[2],
    );

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[3]);

    let sec_lines: Vec<Line> = [
        ("FileVault", h.filevault),
        ("Firewall", h.firewall),
        ("SIP", h.sip),
    ]
    .into_iter()
    .map(|(name, ok)| {
        let (txt, color) = if ok {
            ("OK", Color::Green)
        } else {
            ("OFF", Color::Red)
        };
        Line::from(vec![
            Span::raw(format!("{:<12}", name)),
            Span::styled(txt, Style::new().fg(color)),
        ])
    })
    .collect();
    frame.render_widget(
        Paragraph::new(sec_lines).block(Block::bordered().title("Security")),
        bottom[0],
    );

    let space_lines: Vec<Line> = h
        .top_space
        .iter()
        .map(|(label, sz)| Line::from(format!("{:<14} {}", label, format_size(*sz))))
        .collect();
    frame.render_widget(
        Paragraph::new(space_lines).block(Block::bordered().title("Top Space Users (Home)")),
        bottom[1],
    );
}

fn draw_system_data(frame: &mut Frame, app: &App, area: Rect) {
    let Some(scan) = &app.system_data else {
        let msg = if app.system_data_loading {
            "Scanning System Data buckets..."
        } else {
            "Press r to scan System Data buckets."
        };
        frame.render_widget(
            Paragraph::new(msg)
                .style(Style::new().fg(Color::DarkGray))
                .block(Block::bordered().title("System Data")),
            area,
        );
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .split(area);

    let summary = vec![
        Line::from(vec![
            Span::styled("Known buckets: ", Style::new().fg(Color::DarkGray)),
            Span::styled(format_size(scan.total_bytes), Style::new().bold()),
            Span::raw(format!("   Scan: {} ms", scan.elapsed_ms)),
        ]),
        Line::from(if scan.partial {
            Span::styled(
                "Fast scan hit its budget; numbers are partial. Use `macclean system-data --deep --json` for exact chart data.",
                Style::new().fg(Color::Yellow),
            )
        } else {
            Span::styled(
                "Exact enough for this fast pass.",
                Style::new().fg(Color::Green),
            )
        }),
        Line::from(""),
    ];
    frame.render_widget(
        Paragraph::new(summary).block(Block::bordered().title("System Data Summary")),
        rows[0],
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Share"),
            category_share_line(&scan.categories, 44),
        ])
        .block(Block::bordered().title("Category Mix")),
        rows[1],
    );

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(rows[2]);

    draw_system_data_categories(frame, &scan.categories, bottom[0]);
    draw_system_data_actions(frame, &scan.categories, bottom[1]);
}

fn draw_system_data_categories(frame: &mut Frame, categories: &[StorageCategory], area: Rect) {
    let block = Block::bordered().title("Buckets");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = inner.height as usize;
    for (i, category) in categories
        .iter()
        .filter(|category| category.size_bytes > 0)
        .take(visible)
        .enumerate()
    {
        let rect = Rect {
            x: inner.x,
            y: inner.y + i as u16,
            width: inner.width,
            height: 1,
        };
        let name_width = (inner.width as usize).saturating_sub(34).max(10);
        let line = Line::from(vec![
            Span::styled(
                format!(
                    "{:<name_width$}",
                    truncate(&category.name, name_width),
                    name_width = name_width
                ),
                Style::new().fg(category_color(i)),
            ),
            Span::raw(format!(" {:>9}", format_size(category.size_bytes))),
            Span::raw(format!(" {:>5.1}% ", category.percent_of_total)),
            Span::styled(
                percent_bar(category.percent_of_total, 12),
                Style::new().fg(category_color(i)),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), rect);
    }
}

fn draw_system_data_actions(frame: &mut Frame, categories: &[StorageCategory], area: Rect) {
    let lines: Vec<Line> = categories
        .iter()
        .filter(|category| category.size_bytes > 0)
        .take(area.height as usize)
        .map(|category| {
            Line::from(vec![
                Span::styled(
                    format!("{:<18}", truncate(&category.name, 18)),
                    Style::new().fg(Color::Cyan),
                ),
                Span::raw(truncate(
                    &category.clean_with,
                    area.width.saturating_sub(20) as usize,
                )),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Actions")),
        area,
    );
}

fn category_share_line(categories: &[StorageCategory], width: usize) -> Line<'static> {
    let mut spans = Vec::new();
    let nonzero: Vec<_> = categories
        .iter()
        .filter(|category| category.size_bytes > 0)
        .collect();
    if nonzero.is_empty() {
        return Line::from("No category sizes found.");
    }

    let mut used = 0usize;
    for (i, category) in nonzero.iter().take(8).enumerate() {
        let mut cells = ((category.percent_of_total / 100.0) * width as f64).round() as usize;
        if category.percent_of_total > 0.0 {
            cells = cells.max(1);
        }
        cells = cells.min(width.saturating_sub(used));
        if cells == 0 {
            continue;
        }
        used += cells;
        spans.push(Span::styled(
            "█".repeat(cells),
            Style::new().fg(category_color(i)),
        ));
    }
    if used < width {
        spans.push(Span::styled(
            "░".repeat(width - used),
            Style::new().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}

fn category_color(index: usize) -> Color {
    match index % 8 {
        0 => Color::Cyan,
        1 => Color::Green,
        2 => Color::Yellow,
        3 => Color::Magenta,
        4 => Color::Blue,
        5 => Color::LightRed,
        6 => Color::LightGreen,
        _ => Color::LightCyan,
    }
}

fn draw_clean(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered().title("Clean -- select categories, then Enter to run");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let n = app.clean_categories.len();
    let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Length(1)).collect();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    app.clean_row_rects.clear();
    for (i, cat) in app.clean_categories.iter().enumerate() {
        app.clean_row_rects.push(rows[i]);

        let checkbox = if cat.selected { "[x]" } else { "[ ]" };
        let size_str = cat.size.map(format_size).unwrap_or_else(|| "-".to_string());
        let sudo_note = if cat.needs_sudo && !app.is_root {
            "  (needs sudo)"
        } else {
            ""
        };
        let line = format!(
            "{} {:<28} {:>10}{}",
            checkbox, cat.label, size_str, sudo_note
        );

        let style = if i == app.clean_cursor {
            Style::new().bg(Color::Cyan).fg(Color::Black)
        } else if cat.selected {
            Style::new().fg(Color::Green)
        } else {
            Style::new()
        };
        frame.render_widget(Paragraph::new(line).style(style), rows[i]);
    }
}

fn draw_uninstall(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.uninstall_screen {
        UninstallScreen::List => draw_uninstall_list(frame, app, area),
        UninstallScreen::Reviewing => draw_uninstall_review(frame, app, area),
    }
}

fn draw_uninstall_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    frame.render_widget(
        Paragraph::new(format!("{}_", app.uninstall_filter))
            .block(Block::bordered().title("Search")),
        rows[0],
    );

    let filtered = app.filtered_uninstall();
    let block = Block::bordered().title(format!("Installed Apps ({})", filtered.len()));
    let inner = block.inner(rows[1]);
    frame.render_widget(block, rows[1]);

    app.uninstall_row_rects.clear();

    if filtered.is_empty() {
        frame.render_widget(
            Paragraph::new("No matches.").style(Style::new().fg(Color::DarkGray)),
            inner,
        );
        return;
    }

    let visible = inner.height as usize;
    let start = app
        .uninstall_cursor
        .saturating_sub(visible.saturating_sub(1));

    for (row_i, &idx) in filtered.iter().enumerate().skip(start).take(visible) {
        let y = inner.y + (row_i - start) as u16;
        let rect = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        app.uninstall_row_rects.push((rect, row_i));

        let style = if row_i == app.uninstall_cursor {
            Style::new().bg(Color::Cyan).fg(Color::Black)
        } else {
            Style::new()
        };
        frame.render_widget(
            Paragraph::new(app.uninstall_apps[idx].0.as_str()).style(style),
            rect,
        );
    }
}

fn draw_uninstall_review(frame: &mut Frame, app: &App, area: Rect) {
    let Some(plan) = &app.uninstall_plan else {
        frame.render_widget(Paragraph::new("No plan."), area);
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let mut header_lines = vec![Line::from(format!(
        "App: {}   Bundle ID: {}",
        plan.app_name, plan.bundle_id
    ))];
    if plan.bundle_id_guessed {
        header_lines.push(Line::from(Span::styled(
            "Could not read Info.plist -- bundle ID is a guess. Review paths carefully.",
            Style::new().fg(Color::Yellow),
        )));
    }
    frame.render_widget(
        Paragraph::new(header_lines).block(Block::bordered().title("Uninstall")),
        rows[0],
    );

    let home = dirs::home_dir().unwrap_or_default();
    let lines: Vec<Line> = plan
        .items
        .iter()
        .map(|item| {
            let label = item
                .path
                .strip_prefix(&home)
                .map(|p| format!("~/{}", p.display()))
                .unwrap_or_else(|_| item.path.display().to_string());
            Line::from(format!(
                "{:<70} {:>10}",
                label,
                format_size(item.size_bytes)
            ))
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(format!(
            "Will remove -- total {}",
            format_size(plan.total_size)
        ))),
        rows[1],
    );

    let confirm_text = if app.dry_run {
        "Dry run -- press Enter to simulate (nothing will be deleted). Esc to cancel."
    } else {
        "Press Enter/y to move to Trash (recoverable). Esc/n to cancel."
    };
    frame.render_widget(
        Paragraph::new(confirm_text).style(Style::new().fg(Color::Red).bold()),
        rows[2],
    );
}

fn percent_bar(percent: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if percent <= 0.0 {
        return "░".repeat(width);
    }
    let filled = (((percent / 100.0) * width as f64).round() as usize).clamp(1, width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", head)
    }
}

/// A drill-down, size-sorted explorer -- browse into folders, see
/// proportional size bars, and delete the huge thing you find right there.
/// Not a literal pixel-packed treemap: a sorted bar list reads more clearly
/// in a monospace grid than tiny packed rectangles would.
fn draw_explore(frame: &mut Frame, app: &mut App, area: Rect) {
    let home = dirs::home_dir().unwrap_or_default();
    let path_label = app
        .explore_dir
        .strip_prefix(&home)
        .map(|p| {
            if p.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", p.display())
            }
        })
        .unwrap_or_else(|_| app.explore_dir.display().to_string());

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    frame.render_widget(
        Paragraph::new(path_label).block(Block::bordered().title("Location")),
        rows[0],
    );

    let total_size: u64 = app.explore_entries.iter().map(|entry| entry.size).sum();
    let partial = app.explore_entries.iter().any(|entry| entry.partial);
    let title = if partial {
        format!(
            "{} items  {} shown  partial",
            app.explore_entries.len(),
            format_size(total_size)
        )
    } else {
        format!(
            "{} items  {} shown",
            app.explore_entries.len(),
            format_size(total_size)
        )
    };
    let block = Block::bordered().title(title);
    let inner = block.inner(rows[1]);
    frame.render_widget(block, rows[1]);

    app.explore_row_rects.clear();

    if app.explore_entries.is_empty() {
        let msg = if app.explore_loading {
            "Scanning..."
        } else {
            "Empty."
        };
        frame.render_widget(
            Paragraph::new(msg).style(Style::new().fg(Color::DarkGray)),
            inner,
        );
        return;
    }

    let visible = inner.height as usize;
    let start = app.explore_cursor.saturating_sub(visible.saturating_sub(1));
    let bar_width = 24usize;
    let name_width = (inner.width as usize)
        .saturating_sub(bar_width + 23)
        .max(10);

    for (row_i, entry) in app
        .explore_entries
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
    {
        let y = inner.y + (row_i - start) as u16;
        let rect = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        app.explore_row_rects.push(rect);

        let suffix = if entry.is_dir { "/" } else { "" };
        let name = truncate(&format!("{}{}", entry.name, suffix), name_width);
        let bar = percent_bar(entry.percent, bar_width);
        let partial = if entry.partial { " ~" } else { "  " };
        let line = format!(
            "{:<name_width$} {:>5.1}% {} {:>10}{}",
            name,
            entry.percent,
            bar,
            format_size(entry.size),
            partial,
            name_width = name_width
        );

        let style = if row_i == app.explore_cursor {
            Style::new().bg(Color::Cyan).fg(Color::Black)
        } else if entry.is_dir {
            Style::new().fg(Color::Blue)
        } else {
            Style::new()
        };
        frame.render_widget(Paragraph::new(line).style(style), rect);
    }

    if let Some(idx) = app.explore_confirm_delete {
        if let Some(entry) = app.explore_entries.get(idx) {
            let msg = if app.dry_run {
                format!(
                    "Dry run -- press Enter to simulate trashing '{}'. Esc to cancel.",
                    entry.name
                )
            } else {
                format!(
                    "Move '{}' to Trash? Enter/y confirm, Esc/n cancel.",
                    entry.name
                )
            };
            let popup = Rect {
                x: area.x + 2,
                y: area.y + area.height.saturating_sub(3),
                width: area.width.saturating_sub(4),
                height: 3,
            };
            frame.render_widget(
                Paragraph::new(msg)
                    .style(Style::new().fg(Color::Red).bold())
                    .block(Block::bordered()),
                popup,
            );
        }
    }
}
