use std::collections::BTreeMap;
use std::io;
use std::time::Duration as StdDuration;

use chrono::{Duration, Local, NaiveDate};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::api::wakatime::{
    fetch_projects, fetch_spans, fetch_summary, fetch_weekly_stats, Item,
    ProjectDetail as ProjectItem,
};
use crate::config::Config;

mod api;
mod config;

const HMAPR: usize = 9;
const HMAPC: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View { Main, Projects }


fn main() -> Result<(), io::Error> {
    let mut view = View::Main;
    let mut project_scroll: u16 = 0;
    let config = Config::from_env();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(io::Error::other)?;

    let (today_text, weekly_header, languages, projects) = if config.api_key.is_empty() {
        (
            "Set HACKATIME_API_KEY in env to fetch coding stats".to_string(),
            String::new(),
            vec![],
            vec![],
        )
    } else {
        let today_text = match runtime.block_on(fetch_summary(&config)) {
            Ok(summary) => format!(
                "Today: {} ({}% completed)",
                summary.data.grand_total.text,
                summary.data.goal.completion_percent,
            ),
            Err(e) => format!("API error: {}", e),
        };

        let (weekly_header, languages, projects) =
            match runtime.block_on(fetch_weekly_stats(&config)) {
                Ok(stats) => (
                    format!(
                        "This week: {} ({})",
                        stats.data.human_readable_total,
                        stats.data.human_readable_range,
                    ),
                    stats.data.languages.into_iter().take(8).collect::<Vec<_>>(),
                    stats.data.projects.into_iter().take(8).collect::<Vec<_>>(),
                ),
                Err(e) => (format!("API error: {}", e), vec![], vec![]),
            };

        (today_text, weekly_header, languages, projects)
    };
    let project_cards: Vec<ProjectItem> = if config.api_key.is_empty() {
        vec![]
    } else {
        runtime.block_on(fetch_projects(&config)).unwrap_or_default()
    };
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let activity = runtime.block_on(fetch_spans(&config)).unwrap_or_default();
    let heatmap = build_hmap(activity);
    let mut search_q = String::new();
    let mut search_active = false;

    loop {
        terminal.draw(|f| {
            let size = f.area();

            match view {
                View::Main => {
                    let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(9),
                    Constraint::Length(22),
                    Constraint::Min(0),
                ]).split(size);
                    let header = Paragraph::new("WakaTime TUI - Press q to quit")
                        .block(Block::default().borders(Borders::ALL).title("Header"));
                    f.render_widget(header, chunks[0]);

                    let today = Paragraph::new(today_text.as_str())
                        .block(Block::default().borders(Borders::ALL).title("Today"));
                    f.render_widget(today, chunks[1]);

                    let weekly_block = Block::default().borders(Borders::ALL).title("This Week");
                    let weekly_inner = weekly_block.inner(chunks[2]);
                    f.render_widget(weekly_block, chunks[2]);
                    let weekly_layout = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(1),
                            Constraint::Length(3),
                            Constraint::Length(3),
                        ]).split(weekly_inner);
                    let weekly = Paragraph::new(weekly_header.as_str()).wrap(Wrap { trim: true });
                    f.render_widget(weekly, weekly_layout[0]);
                    rdr_gg(f, weekly_layout[1], &languages, Color::Green);
                    rdr_gg(f, weekly_layout[2], &projects, Color::Cyan);
                    let hmap_widget = Paragraph::new(rdr_hmap(&heatmap)).block(Block::default().borders(Borders::ALL).title("Coding Streak"));

                    f.render_widget(hmap_widget, chunks[3]);
                }

                View::Projects => {
                    let outer = Layout::default().direction(Direction::Vertical).constraints([
                        Constraint::Length(3),
                        Constraint::Min(0),
                    ]).split(size);
                    let search_brdrstyle = if search_active { Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) };
                    let crsr = if search_active { "|" } else { "" };
                    let search_text = if search_q.is_empty() && !search_active {"/ to search, p back, q quit".to_string()} else { format!("{}{}", search_q, crsr) };
                    let search_style = if search_q.is_empty() && !search_active {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let search_bar = Paragraph::new(Span::styled(search_text, search_style)).block(Block::default().borders(Borders::ALL).title(Span::styled("Search Projects", Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD))));
                    f.render_widget(search_bar, outer[0]);
                    let filtered: Vec<&ProjectItem> = if search_q.is_empty() {
                        project_cards.iter().collect()
                    } else {
                        project_cards.iter().filter(|p| p.name.to_lowercase().contains(&search_q.to_lowercase())).collect()
                    };
                    let container = Block::default()
                        .borders(Borders::ALL)
                        .title(Span::styled(format!("My Projects ({}/{}) | p back | up/down scroll", filtered.len(), project_cards.len()), Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD)));
                    let inner = container.inner(outer[1]);
                    f.render_widget(container, outer[1]);

                    let tab = Style::default().fg(Color::Black).bg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD);

                    let mut lines: Vec<Line> = vec![
                        Line::from(vec![
                            Span::styled(" Active ", tab),
                        ]),
                        Line::from(Span::styled(
                            format!("{} projects", filtered.len()),
                            Style::default().fg(Color::Rgb(201, 158, 170)),
                        )),
                        Line::from(""),
                    ];

                    if filtered.is_empty() {
                        lines.push(Line::from(Span::styled("No projects found :(", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))));
                    } else {
                        for p in &filtered {
                            let h = p.total_seconds / 3600;
                            let m = (p.total_seconds % 3600) / 60;
                            let s = p.total_seconds % 60;
                            let langs = if p.languages.is_empty() { "No languages".to_string() } else { p.languages.iter().take(4).cloned().collect::<Vec<_>>().join(", ") };
                            lines.push(Line::from(Span::styled("────────────────────────────────────────────────────────", Style::default().fg(Color::Rgb(210, 57, 89)))));
                            lines.push(Line::from(Span::styled(format!(" {}", p.name), Style::default().fg(Color::Rgb(245, 240, 243)).add_modifier(Modifier::BOLD))));
                            lines.push(Line::from(Span::styled(format!(" {}h {}m {}s", h, m, s), Style::default().fg(Color::Rgb(233, 72, 103)).add_modifier(Modifier::BOLD))));
                            lines.push(Line::from(Span::styled(format!(" {} heartbeats | {}", p.total_heartbeats, langs), Style::default().fg(Color::Rgb(173, 151, 160)))));
                            lines.push(Line::from(""));
                        }
                    }
                    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((project_scroll, 0)), inner);
                }
            }
        })?;

        if event::poll(StdDuration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('p') if view == View::Main => {
                        view = View::Projects;
                    }
                    KeyCode::Char('p') if view == View::Projects => {
                        view = View::Main;
                    }
                    KeyCode::Down if view == View::Projects => { project_scroll = project_scroll.saturating_add(1); }
                    KeyCode::Up if view == View::Projects => { project_scroll = project_scroll.saturating_sub(1); }
                    KeyCode::PageDown if view == View::Projects => { project_scroll = project_scroll.saturating_add(8); }
                    KeyCode::PageUp if view == View::Projects => { project_scroll = project_scroll.saturating_sub(8); }
                    KeyCode::Char('/') if view == View::Projects => {
                        search_active = true;
                    }
                    KeyCode::Char(c) if view == View::Projects && search_active => {
                        search_q.push(c);
                        project_scroll = 0;
                    }
                    KeyCode::Backspace if view == View::Projects && search_active => {
                        search_q.pop();
                        project_scroll = 0;
                    }
                    KeyCode::Esc if view == View::Projects => {
                        search_active = false;
                        search_q.clear();
                    }
                    KeyCode::Enter if view == View::Projects && search_active => {
                        search_active = false;
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn rdr_gg(f: &mut Frame, area: Rect, items: &[Item], color: Color) {
    if items.is_empty() {
        return;
    }
    let n = items.len() as u32;
    let mut constraints = Vec::new();
    for _ in 0..n {
        constraints.push(Constraint::Ratio(1, n));
    }
    let chunks = Layout::default().direction(Direction::Horizontal).constraints(constraints).split(area);

    for i in 0..items.len() {
        let item = &items[i];
        let pct = item.percent.clamp(0.0, 100.0).round() as u16;
        let gg = Gauge::default().block(Block::default().title(item.name.as_str()).borders(Borders::ALL)).gauge_style(Style::default().fg(color)).percent(pct);
        f.render_widget(gg, chunks[i]);
    }
}

fn heat_color(level: u8) -> Color {
    match level {
        0 => Color::Rgb(40, 40, 40),
        1 => Color::Rgb(0, 80, 0),
        2 => Color::Rgb(0, 140, 0),
        3 => Color::Rgb(0, 200, 0),
        _ => Color::Rgb(0, 255, 0),
    }
}

fn build_hmap(activity: BTreeMap<NaiveDate, f64>) -> Vec<Vec<u8>> {
    let mut grid = vec![vec![0u8; HMAPR]; HMAPC];
    let today = Local::now().date_naive();
    for i in 0..HMAPC {
        for j in 0..HMAPR {
            let idx = i * HMAPR + j;
            let days_ago = (HMAPC * HMAPR - 1) - idx;
            let day = today - Duration::days(days_ago as i64);
            let secs = activity.get(&day).cloned().unwrap_or(0.0);
            let level = if secs == 0.0 {
                0
            } else {
                let hrs = secs / 3600.0;
                match hrs {
                    h if h < 0.25 => 1,
                    h if h < 1.0  => 2,
                    h if h < 3.0  => 3,
                    _             => 4,
                }
            };
            grid[i][j] = level;
        }
    }
    grid
}

fn rdr_hmap(grid: &[Vec<u8>]) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(HMAPR);
    for row in 0..HMAPR {
        let mut spans = Vec::with_capacity(grid.len() * 2);
        for col in 0..grid.len() {
            let level = grid[col][row];
            spans.push(Span::styled("  ",Style::default().bg(heat_color(level))));
            spans.push(Span::styled(" ", Style::default().fg(Color::Reset).bg(Color::Reset)));
        }
        lines.push(Line::from(spans));
        if row + 1 < HMAPR {
            lines.push(Line::from(" "));
        }
    }
    lines
}