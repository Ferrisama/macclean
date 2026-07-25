use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::Frame;

use super::app::{App, Tab, UninstallScreen};
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
        Tab::Clean => draw_clean(frame, app, chunks[1]),
        Tab::Uninstall => draw_uninstall(frame, app, chunks[1]),
        Tab::Explore => draw_explore(frame, app, chunks[1]),
    }
    draw_status_bar(frame, app, chunks[2]);
}

fn draw_tab_bar(frame: &mut Frame, app: &mut App, area: Rect) {
    let titles = ["Dashboard", "Clean", "Uninstall", "Explore"];
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 4); 4])
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

fn size_bar(size: u64, max: u64, width: usize) -> String {
    if max == 0 {
        return " ".repeat(width);
    }
    let filled = (((size as f64 / max as f64) * width as f64).round() as usize).min(width);
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

    let block = Block::bordered().title(format!("{} items", app.explore_entries.len()));
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

    let max_size = app
        .explore_entries
        .iter()
        .map(|e| e.size)
        .max()
        .unwrap_or(1)
        .max(1);
    let visible = inner.height as usize;
    let start = app.explore_cursor.saturating_sub(visible.saturating_sub(1));
    let bar_width = 24usize;
    let name_width = (inner.width as usize)
        .saturating_sub(bar_width + 14)
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
        let bar = size_bar(entry.size, max_size, bar_width);
        let line = format!(
            "{:<name_width$} {} {:>10}",
            name,
            bar,
            format_size(entry.size),
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
