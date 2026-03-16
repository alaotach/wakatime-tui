use std::io;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
    execute,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};
use std::time::Duration;
use crate::config::Config;
mod config;
mod api;
use crate::api::wakatime::{fetch_summary, fetch_weekly_stats, Item};

const HMAPR: usize = 7;
const HMAPC: usize = 28;

fn main() -> Result<(), io::Error> {
    let config = Config::from_env();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(io::Error::other)?;

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

    // let heatmap = build_hmap(&languages, &projects);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(9),
                    Constraint::Length(15),
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
                    Constraint::Min(0),
                ]).split(weekly_inner);
            let weekly = Paragraph::new(weekly_header.as_str()).wrap(Wrap { trim: true });
            f.render_widget(weekly, weekly_layout[0]);
            rdr_gg(f, weekly_layout[1], &languages, Color::Green);
            rdr_gg(f, weekly_layout[2], &projects, Color::Cyan);
            // let hmap_widget = Paragraph::new(rdr_hmap(&heatmap)).block(Block::default().borders(Borders::ALL).title("Coding Streak"));

            // f.render_widget(hmap_widget, chunks[3]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
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

// fn build_hmap(activity) -> Vec<Vec<u8>> {
//     let mut grid = vec![vec![0u8; HMAPR]; HMAPC];
//     let values: Vec<u8> = activity
//         .iter()
//         .chain(projects.iter())
//         .map(|item| {
//             let pct = item.percent.clamp(0.0, 100.0);
//             match pct {
//                 p if p <= 0.0 => 0,
//                 p if p <= 25.0 => 1,
//                 p if p <= 50.0 => 2,
//                 p if p <= 75.0 => 3,
//                 _ => 4,
//             }
//         })
//         .collect();
//     if values.is_empty() {
//         return grid;
//     }
//     for col in 0..HMAPC {
//         let level = values[col % values.len()];
//         for row in 0..HMAPR {
//             let wave = ((col + row) % 5) as u8;
//             grid[col][row] = level.saturating_sub(wave / 3);
//         }
//     }
//     grid
// }

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