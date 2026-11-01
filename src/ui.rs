use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, Tabs, Wrap,
};

use crate::app::{App, Tab};
use crate::repo::{heatmap_grid, DayBucket};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let outer = Layout::vertical([
        Constraint::Length(3), // tab bar
        Constraint::Min(0),    // body
        Constraint::Length(1), // status bar
    ])
    .split(frame.area());

    draw_tabs(frame, outer[0], app);
    match app.tab {
        Tab::Overview => draw_overview(frame, outer[1], app),
        Tab::Commits => draw_commits(frame, outer[1], app),
        Tab::Authors => draw_authors(frame, outer[1], app),
        Tab::Heatmap => draw_heatmap(frame, outer[1], app),
        Tab::Files => draw_files(frame, outer[1], app),
    }
    draw_status(frame, outer[2], app);

    if app.show_help {
        draw_help(frame);
    }
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| {
            Line::from(vec![
                Span::styled(format!(" {} ", i + 1), Style::default().fg(Color::DarkGray)),
                Span::styled(t.title(), Style::default().fg(Color::White)),
                Span::raw(" "),
            ])
        })
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Line::from(vec![
                    Span::raw(" "),
                    Span::styled("gitpulse", Style::default().fg(Color::Magenta).bold()),
                    Span::raw(" "),
                ])),
        )
        .highlight_style(Style::default().fg(Color::Magenta).bold())
        .divider("│");

    frame.render_widget(tabs, area);
}

fn draw_overview(frame: &mut Frame, area: Rect, app: &App) {
    let s = &app.stats;
    let repo_path = s.repo_path.display().to_string();
    let remote = s.remote_url.clone().unwrap_or_else(|| "(none)".into());

    let age = match (s.first_commit_date, s.last_commit_date) {
        (Some(a), Some(b)) => {
            let days = (b.date_naive() - a.date_naive()).num_days();
            format!(
                "{} ({} → {}, {} days)",
                humanize_days(days),
                a.format("%Y-%m-%d"),
                b.format("%Y-%m-%d"),
                days
            )
        }
        _ => "(no commits)".into(),
    };

    let lines: Vec<Line> = vec![
        kv("repo", &repo_path),
        kv("branch", &s.head_branch),
        kv("HEAD", &s.head_short),
        kv("remote", &remote),
        Line::from(""),
        kv("commits", &s.total_commits.to_string()),
        kv("authors", &s.total_authors.to_string()),
        kv("files", &s.total_files.to_string()),
        kv("age", &age),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press ? for help, 1-5 or Tab to switch views, q to quit.",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let p = Paragraph::new(lines)
        .block(bordered_block("Overview"))
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

fn draw_commits(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .stats
        .commits
        .iter()
        .map(|c| {
            let line = Line::from(vec![
                Span::styled(
                    format!("{:7}", c.short_sha),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw("  "),
                Span::styled(
                    c.date.format("%Y-%m-%d").to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(truncate(&c.author, 20), Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::raw(c.subject.clone()),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = format!("Commits ({})", app.stats.commits.len());
    let list = List::new(items)
        .block(bordered_block(&title))
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("▌ ");

    let mut state = ListState::default();
    state.select(Some(app.commits_scroll));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_authors(frame: &mut Frame, area: Rect, app: &mut App) {
    let max = app
        .stats
        .authors
        .first()
        .map(|a| a.commits)
        .unwrap_or(1)
        .max(1);

    let rows: Vec<Row> = app
        .stats
        .authors
        .iter()
        .map(|a| {
            let bar = bar_cell(a.commits, max, 24);
            Row::new(vec![
                Cell::from(truncate(&a.name, 24)).style(Style::default().fg(Color::Cyan)),
                Cell::from(truncate(&a.email, 32)).style(Style::default().fg(Color::DarkGray)),
                Cell::from(format!("{:>5}", a.commits)),
                Cell::from(bar),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(26),
        Constraint::Length(34),
        Constraint::Length(7),
        Constraint::Min(10),
    ];
    let title = format!("Authors ({})", app.stats.authors.len());
    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Name", "Email", "Count", "Share"])
                .style(Style::default().fg(Color::White).bold()),
        )
        .block(bordered_block(&title))
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("▌ ");

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(app.authors_scroll));
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_heatmap(frame: &mut Frame, area: Rect, app: &App) {
    let grid = heatmap_grid(&app.stats.heatmap);
    if grid.is_empty() {
        let p =
            Paragraph::new("No commit history to visualize yet.").block(bordered_block("Heatmap"));
        frame.render_widget(p, area);
        return;
    }

    let title = format!(
        "Heatmap — last 365 days ({} commits)",
        app.stats.heatmap.iter().map(|d| d.count).sum::<usize>()
    );
    let block = bordered_block(&title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // One cell per day. Each cell is 2 columns wide (``) and 1 row tall. Rows are weekdays.
    let cell_w = 2u16;
    let max_weeks = (inner.width / cell_w) as usize;
    let visible = if grid.len() > max_weeks {
        &grid[grid.len() - max_weeks..]
    } else {
        &grid[..]
    };

    // 8 rows: 7 weekdays + 1 legend/footer
    let rows = 8u16.min(inner.height);
    let legend_row = inner.y + rows.saturating_sub(1);

    // Compute max count once for intensity scaling.
    let max_count: usize = app
        .stats
        .heatmap
        .iter()
        .map(|d| d.count)
        .max()
        .unwrap_or(0)
        .max(1);

    for (col_i, column) in visible.iter().enumerate() {
        let x = inner.x + (col_i as u16) * cell_w;
        for (row_i, cell) in column.iter().enumerate().take(7) {
            let y = inner.y + row_i as u16;
            if y >= legend_row {
                break;
            }
            let (ch, style) = heatmap_cell_style(*cell, max_count);
            let span = Span::styled(ch, style);
            let area = Rect {
                x,
                y,
                width: cell_w,
                height: 1,
            };
            frame.render_widget(Paragraph::new(span), area);
        }
    }

    // Legend
    if inner.height >= 8 {
        let legend = legend_line(max_count);
        let legend_area = Rect {
            x: inner.x,
            y: legend_row,
            width: inner.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(legend), legend_area);
    }
}

fn heatmap_cell_style(cell: Option<DayBucket>, max_count: usize) -> (String, Style) {
    match cell {
        None => ("  ".into(), Style::default()),
        Some(b) if b.count == 0 => ("■ ".into(), Style::default().fg(Color::Rgb(40, 40, 40))),
        Some(b) => {
            let intensity = b.count as f32 / max_count as f32;
            let color = heatmap_color(intensity);
            ("■ ".into(), Style::default().fg(color))
        }
    }
}

fn heatmap_color(intensity: f32) -> Color {
    // Four-step green scale, GitHub-ish.
    if intensity > 0.75 {
        Color::Rgb(39, 174, 96)
    } else if intensity > 0.5 {
        Color::Rgb(39, 174, 96).into_mix(0.75)
    } else if intensity > 0.25 {
        Color::Rgb(39, 174, 96).into_mix(0.5)
    } else {
        Color::Rgb(39, 174, 96).into_mix(0.25)
    }
}

trait Mix {
    fn into_mix(self, factor: f32) -> Color;
}
impl Mix for Color {
    fn into_mix(self, factor: f32) -> Color {
        match self {
            Color::Rgb(r, g, b) => Color::Rgb(mix(r, factor), mix(g, factor), mix(b, factor)),
            other => other,
        }
    }
}
fn mix(c: u8, factor: f32) -> u8 {
    let base = 25.0;
    let v = base + (c as f32 - base) * factor;
    v.clamp(0.0, 255.0) as u8
}

fn legend_line(max_count: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled("less ", Style::default().fg(Color::DarkGray)),
        Span::styled("■ ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled(
            "■ ",
            Style::default().fg(Color::Rgb(39, 174, 96).into_mix(0.25)),
        ),
        Span::styled(
            "■ ",
            Style::default().fg(Color::Rgb(39, 174, 96).into_mix(0.5)),
        ),
        Span::styled(
            "■ ",
            Style::default().fg(Color::Rgb(39, 174, 96).into_mix(0.75)),
        ),
        Span::styled("■ ", Style::default().fg(Color::Rgb(39, 174, 96))),
        Span::styled(
            format!("more  (peak day: {} commits)", max_count),
            Style::default().fg(Color::DarkGray),
        ),
    ])
}

fn draw_files(frame: &mut Frame, area: Rect, app: &mut App) {
    let max = app
        .stats
        .hot_files
        .first()
        .map(|f| f.touches)
        .unwrap_or(1)
        .max(1);

    let rows: Vec<Row> = app
        .stats
        .hot_files
        .iter()
        .map(|f| {
            let bar = bar_cell(f.touches, max, 24);
            Row::new(vec![
                Cell::from(truncate(&f.path, 60)).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{:>5}", f.touches)),
                Cell::from(bar),
            ])
        })
        .collect();

    let widths = [
        Constraint::Min(20),
        Constraint::Length(7),
        Constraint::Length(30),
    ];
    let title = format!("Hot files (top {})", app.stats.hot_files.len());
    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Path", "Touches", "Share"])
                .style(Style::default().fg(Color::White).bold()),
        )
        .block(bordered_block(&title))
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("▌ ");

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(app.files_scroll));
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let left = Span::styled(
        format!("  {}  ", app.tab.title()),
        Style::default().bg(Color::Magenta).fg(Color::Black).bold(),
    );
    let right = Span::styled(
        " 1-5 tabs · ↑↓ scroll · ? help · q quit ",
        Style::default().fg(Color::DarkGray),
    );
    let line = Line::from(vec![left, Span::raw("  "), right]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_help(frame: &mut Frame) {
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(Span::styled(
            "Keybindings",
            Style::default().fg(Color::Magenta).bold(),
        )),
        Line::from(""),
        Line::from("  1 / 2 / 3 / 4 / 5   switch tab"),
        Line::from("  Tab / Shift+Tab    next / previous tab"),
        Line::from("  ↑ / ↓   or   j / k   scroll"),
        Line::from("  Home / End          scroll to top / bottom"),
        Line::from("  PgUp / PgDn         page up / down"),
        Line::from("  ?                   toggle this help"),
        Line::from("  q   or   Esc        quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to close.",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let p = Paragraph::new(lines)
        .block(bordered_block("Help"))
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

/* ---------- helpers ---------- */

fn bordered_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title, Style::default().fg(Color::Magenta).bold()),
            Span::raw(" "),
        ]))
}

fn kv(key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:>10} ", key),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  "),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut it = s.chars();
        let head: String = it.by_ref().take(max.saturating_sub(1)).collect();
        format!("{}…", head)
    }
}

fn bar_cell(value: usize, max: usize, width: usize) -> Line<'static> {
    let filled = ((value as f32 / max.max(1) as f32) * width as f32).round() as usize;
    let filled = filled.min(width);
    let bar = "█".repeat(filled);
    let rest = " ".repeat(width - filled);
    Line::from(vec![
        Span::styled(bar, Style::default().fg(Color::Magenta)),
        Span::raw(rest),
    ])
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(v[1])[1]
}

fn humanize_days(days: i64) -> String {
    if days < 30 {
        format!("{} days", days)
    } else if days < 365 {
        format!("{:.1} months", days as f32 / 30.0)
    } else {
        format!("{:.1} years", days as f32 / 365.25)
    }
}
