use std::collections::BTreeMap;
use std::io;
use std::time::Duration as StdDuration;

use chrono::{Duration, Local, NaiveDate, Timelike};
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
    fetch_daily_leaderboard, fetch_projects, fetch_spans, fetch_summary, fetch_weekly_leaderboard,
    fetch_weekly_stats, fetch_hourly, streaks, Item, ProjectDetail as ProjectItem,
};
use crate::config::Config;

mod api;
mod config;

const HMAPR: usize = 9;
const HMAPC: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Main,
    Projects,
    Leaderboard,
    Day,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LbType {
    Daily,
    Weekly,
}

fn main() -> Result<(), io::Error> {
    let mut view = View::Main;
    let mut project_scroll: u16 = 0;
    let mut lb_scroll: u16 = 0;
    let mut lb_period = LbType::Daily;

    let config = Config::from_env();
    let api_missing = config.api_key.is_empty();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(io::Error::other)?;
    let mut day_date: NaiveDate = Local::now().date_naive();
    let mut jump_active = false;
    let mut jump_input = String::new();
    let mut jump_err = String::new();

    let (mut today_text, mut weekly_header, mut languages, mut projects) = if config.api_key.is_empty() {
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
                summary.data.grand_total.text, summary.data.goal.completion_percent,
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

    let mut project_cards: Vec<ProjectItem> = if config.api_key.is_empty() {
        vec![]
    } else {
        runtime.block_on(fetch_projects(&config)).unwrap_or_default()
    };

    let mut daily_lb: Vec<(String, String)> = if config.api_key.is_empty() {
        vec![]
    } else {
        runtime.block_on(fetch_daily_leaderboard(&config)).unwrap_or_default()
    };

    let mut weekly_lb: Vec<(String, String)> = if config.api_key.is_empty() {
        vec![]
    } else {
        runtime.block_on(fetch_weekly_leaderboard(&config)).unwrap_or_default()
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let activity = runtime.block_on(fetch_spans(&config)).unwrap_or_default();
    let mut heatmap = build_hmap(&activity);
    let (mut current_streak, mut longest_streak) = streaks(&activity);
    let mut day_hours: [f64; 24] = if config.api_key.is_empty() {
        [0.0; 24]
    } else {
        runtime
            .block_on(fetch_hourly(&config, day_date))
            .unwrap_or([0.0; 24])
    };
    let mut search_q = String::new();
    let mut search_active = false;
    let mut lb_search_q = String::new();
    let mut lb_search_active = false;
    let refresh_int = std::time::Duration::from_secs(300);
    let mut last_r = std::time::Instant::now();

    loop {
        if last_r.elapsed() >= refresh_int {
            let (ntoday_text, nweekly_header, nlanguages, nprojects, nproject_cards, ndaily_lb, nweekly_lb, nheatmap, ncurrent_streak, nlongest_streak, nday_hours, ) = refresh(&runtime, &config, day_date);
            today_text = ntoday_text;
            weekly_header = nweekly_header;
            languages = nlanguages;
            projects = nprojects;
            project_cards = nproject_cards;
            daily_lb = ndaily_lb;
            weekly_lb = nweekly_lb;
            heatmap = nheatmap;
            current_streak = ncurrent_streak;
            longest_streak = nlongest_streak;
            day_hours = nday_hours;
            last_r = std::time::Instant::now();
        }

        terminal.draw(|f| {
            let size = f.area();

            match view {
                View::Main => {
                    if api_missing {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .margin(1)
                            .constraints([Constraint::Length(3), Constraint::Min(0)])
                            .split(size);
                        
                        let header = Paragraph::new("WakaTime TUI - Setup Required  [q] Quit")
                            .block(Block::default().borders(Borders::ALL).title("Header"));
                        f.render_widget(header, chunks[0]);

                        let setup_text = [
                            "Hackatime is not configured yet. Set up your API key, then restart this TUI.",
                            "",
                            "Windows PowerShell install:",
                            "& ([scriptblock]::Create((irm https://hack.club/setup/install.ps1))) -ApiKey <YOUR_API_KEY>",
                            "",
                            "macOS/Linux install:",
                            "curl -fsSL https://hack.club/setup/install.sh | bash -s -- <YOUR_API_KEY>",
                            "",
                            "Or create ~/.wakatime.cfg (Windows: %USERPROFILE%\\.wakatime.cfg):",
                            "[settings]",
                            "api_url = https://hackatime.hackclub.com/api/hackatime/v1",
                            "api_key = <YOUR_API_KEY>",
                            "heartbeat_rate_limit_seconds = 30",
                            "",
                            "You can also export HACKATIME_API_KEY in your environment.",
                        ]
                        .join("\n");

                        let setup = Paragraph::new(setup_text)
                            .block(Block::default().borders(Borders::ALL).title("Hackatime Setup"))
                            .wrap(Wrap { trim: false });
                        f.render_widget(setup, chunks[1]);
                    } else {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .margin(1)
                            .constraints([
                                Constraint::Length(3),
                                Constraint::Length(3),
                                Constraint::Length(9),
                                Constraint::Length(22),
                                Constraint::Min(0),
                            ])
                            .split(size);

                        let header = Paragraph::new("WakaTime TUI - [p] Projects  [l] Leaderboard  [d] Day  [r] Refresh  [q] Quit")
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
                            ])
                            .split(weekly_inner);
                        let weekly = Paragraph::new(weekly_header.as_str()).wrap(Wrap { trim: true });
                        f.render_widget(weekly, weekly_layout[0]);
                        rdr_gg(f, weekly_layout[1], &languages, Color::Green);
                        rdr_gg(f, weekly_layout[2], &projects, Color::Cyan);

                        let hmap_widget = Paragraph::new(rdr_hmap(&heatmap))
                            .block(Block::default().borders(Borders::ALL).title(format!("Coding Streak (Current: {}d, Longest: {}d)", current_streak, longest_streak)));
                        f.render_widget(hmap_widget, chunks[3]);
                    }
                }

                View::Projects => {
                    let outer = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(3), Constraint::Min(0)])
                        .split(size);

                    let crsr = if search_active {
                        if (chrono::Local::now().timestamp_millis() / 300) % 2 == 0 { "|" } else { " " }
                    } else {
                        ""
                    };
                    let search_text = if search_q.is_empty() && !search_active {
                        "[/] to search, [p] back, [l] leaderboard, [q] quit".to_string()
                    } else {
                        format!("{}{}", search_q, crsr)
                    };
                    let search_style = if search_q.is_empty() && !search_active {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let search_bar = Paragraph::new(Span::styled(search_text, search_style))
                        .block(Block::default().borders(Borders::ALL).title(Span::styled(
                            "Search Projects",
                            Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD),
                        )));
                    f.render_widget(search_bar, outer[0]);

                    let filtered: Vec<&ProjectItem> = if search_q.is_empty() {
                        project_cards.iter().collect()
                    } else {
                        project_cards.iter().filter(|p| p.name.to_lowercase().contains(&search_q.to_lowercase())).collect()
                    };
                    let container = Block::default()
                        .borders(Borders::ALL)
                        .title(Span::styled(
                            format!("My Projects ({}/{}) | [p] back | [up/down] scroll", filtered.len(), project_cards.len()),
                            Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD),
                        ));
                    let inner = container.inner(outer[1]);
                    f.render_widget(container, outer[1]);

                    let tab = Style::default().fg(Color::Black).bg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD);
                    let mut lines: Vec<Line> = vec![
                        Line::from(vec![Span::styled(" Active ", tab)]),
                        Line::from(Span::styled(
                            format!("{} projects", filtered.len()),
                            Style::default().fg(Color::Rgb(201, 158, 170)),
                        )),
                        Line::from(""),
                    ];

                    if filtered.is_empty() {
                        lines.push(Line::from(Span::styled(
                            "No projects found :(",
                            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                        )));
                    } else {
                        for p in &filtered {
                            let h = p.total_seconds / 3600;
                            let m = (p.total_seconds % 3600) / 60;
                            let s = p.total_seconds % 60;
                            let langs = if p.languages.is_empty() {
                                "No languages".to_string()
                            } else {
                                p.languages.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
                            };
                            lines.push(Line::from(Span::styled(
                                "────────────────────────────────────────────────────────",
                                Style::default().fg(Color::Rgb(210, 57, 89)),
                            )));
                            lines.push(Line::from(Span::styled(
                                format!(" {}", p.name),
                                Style::default().fg(Color::Rgb(245, 240, 243)).add_modifier(Modifier::BOLD),
                            )));
                            lines.push(Line::from(Span::styled(
                                format!(" {}h {}m {}s", h, m, s),
                                Style::default().fg(Color::Rgb(233, 72, 103)).add_modifier(Modifier::BOLD),
                            )));
                            lines.push(Line::from(Span::styled(
                                format!(" {} heartbeats | {}", p.total_heartbeats, langs),
                                Style::default().fg(Color::Rgb(173, 151, 160)),
                            )));
                            lines.push(Line::from(""));
                        }
                    }

                    f.render_widget(
                        Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((project_scroll, 0)),
                        inner,
                    );
                }

                View::Leaderboard => {
                    let outer = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(3), Constraint::Min(0)])
                        .split(size);

                    let crsr = if lb_search_active {
                        if (chrono::Local::now().timestamp_millis() / 300) % 2 == 0 {
                            "|"
                        } else {
                            " "
                        }
                    } else {
                        ""
                    };
                    let lb_search_text = if lb_search_q.is_empty() && !lb_search_active {
                        "[/] search username, [tab] switch, [p] projects, [l] main, [q] quit".to_string()
                    } else {
                        format!("{}{}", lb_search_q, crsr)
                    };
                    let dstyle = if lb_period == LbType::Daily {
                        Style::default().fg(Color::Black).bg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    let wstyle = if lb_period == LbType::Weekly {
                        Style::default().fg(Color::Black).bg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    let tab_line = Line::from(vec![
                        Span::styled(" Daily ", dstyle),
                        Span::raw("  "),
                        Span::styled(" Weekly ", wstyle),
                        Span::raw("  "),
                        Span::styled(
                            lb_search_text,
                            if lb_search_q.is_empty() && !lb_search_active {
                                Style::default().fg(Color::DarkGray)
                            } else {
                                Style::default().fg(Color::White)
                            },
                        ),
                    ]);
                    let tab_bar = Paragraph::new(tab_line).block(
                        Block::default().borders(Borders::ALL).title(Span::styled(
                            "Leaderboard",
                            Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD),
                        )),
                    );
                    f.render_widget(tab_bar, outer[0]);

                    let entries = match lb_period {
                        LbType::Daily => &daily_lb,
                        LbType::Weekly => &weekly_lb,
                    };

                    let lb_query = lb_search_q.to_lowercase();
                    let fentries: Vec<(usize, &(String, String))> = if lb_search_q.is_empty() {
                        entries.iter().enumerate().map(|(i, e)| (i + 1, e)).collect()
                    } else {
                        entries.iter().enumerate().filter(|(_, (u, _))| u.to_lowercase().contains(&lb_query)).map(|(i, e)| (i + 1, e)).collect()
                    };

                    let container = Block::default().borders(Borders::ALL).title(Span::styled(
                        format!(
                            "{} | {}/{} entries",
                            if lb_period == LbType::Daily { "Last 24 Hours" } else { "Last 7 Days" },
                            fentries.len(),
                            entries.len()
                        ),
                        Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD),
                    ));
                    let inner = container.inner(outer[1]);
                    f.render_widget(container, outer[1]);

                    let mut lines: Vec<Line> = Vec::new();
                    if fentries.is_empty() {
                        lines.push(Line::from(Span::styled(
                            if lb_search_q.is_empty() {
                                "  No leaderboard data."
                            } else {
                                "  No usernames match your search."
                            },
                            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                        )));
                    } else {
                        for (visible_i, (rank, (username, time_text))) in fentries.iter().enumerate() {
                            let rank = *rank;
                            let (medal, rank_style) = match rank {
                                1 => ("\u{1F947}", Style::default().fg(Color::Rgb(255, 215, 0)).add_modifier(Modifier::BOLD)),
                                2 => ("\u{1F948}", Style::default().fg(Color::Rgb(192, 192, 192)).add_modifier(Modifier::BOLD)),
                                3 => ("\u{1F949}", Style::default().fg(Color::Rgb(205, 127, 50)).add_modifier(Modifier::BOLD)),
                                _ => ("  ", Style::default().fg(Color::Rgb(100, 100, 120))),
                            };
                            let rank_text = if rank <= 3 {
                                format!(" {} ", medal)
                            } else {
                                format!(" {:>3}. ", rank)
                            };
                            let scolor = match rank {
                                1 => Color::Rgb(180, 130, 0),
                                2 => Color::Rgb(120, 120, 120),
                                3 => Color::Rgb(130, 80, 30),
                                _ => Color::Rgb(50, 50, 65),
                            };
                            if visible_i > 0 {
                                lines.push(Line::from(Span::styled(
                                    "─────────────────────────────────────────────────────────────",
                                    Style::default().fg(scolor),
                                )));
                            }
                            lines.push(Line::from(vec![
                                Span::styled(rank_text, rank_style),
                                Span::styled(
                                    format!("{:<35}", username),
                                    if rank <= 3 { rank_style } else { Style::default().fg(Color::Rgb(220, 215, 230)) },
                                ),
                                Span::styled(
                                    format!("{:>10}", time_text),
                                    Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD),
                                ),
                            ]));
                        }
                    }
                    f.render_widget(
                        Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((lb_scroll, 0)),
                        inner,
                    );
                }

                View::Day => {
                    let today = Local::now().date_naive();
                    let chunks = Layout::default().direction(Direction::Vertical).margin(1)
                        .constraints([
                            Constraint::Length(3),
                            Constraint::Length(3),
                            Constraint::Min(0),
                        ]).split(size);

                    let total_today: f64 = day_hours.iter().sum();
                    let h = (total_today / 3600.0) as u64;
                    let m = ((total_today % 3600.0) / 60.0) as u64;
                    let is_today = day_date == today;
                    let max = day_hours.iter().cloned().fold(0./0., f64::max);
                    let max_hr = day_hours.iter().position(|&h| h == max).unwrap_or(0);
                    let nav_text = format!(
                        "[<-] {} [->]   |   Total: {}h {}m   |   Max: {:.1}h   |   Most Productive: {}th Hour   |   [j] jump to date   |   [d/Esc] back",
                        day_date.format("%A, %d %B %Y"), h, m, max / 3600.0, max_hr);
                    let nav = Paragraph::new(nav_text).block(
                        Block::default().borders(Borders::ALL).title(Span::styled(
                            if is_today { "Day View - Today" } else { "Day View" },
                            Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD),
                        )),
                    );
                    f.render_widget(nav, chunks[0]);
                    let crsr = if jump_active {
                        if (chrono::Local::now().timestamp_millis() / 300) % 2 == 0 {
                            "|"
                        } else {
                            " "
                        }
                    } else {
                        ""
                    };
                    let dsearch_text = if jump_active {
                        format!("{}{}", jump_input, crsr)
                    } else if !jump_err.is_empty() {
                        jump_err.clone()
                    } else {
                        "Press [j],  type a date (YYYY-MM-DD) then press enter to jump".to_string()
                    };
                    let dsearch_style = if !jump_err.is_empty() {
                        Style::default().fg(Color::Red)
                    } else if jump_active {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    let dsearch_bar = Paragraph::new(Span::styled(dsearch_text, dsearch_style)).block(Block::default().borders(Borders::ALL).title("Search"));
                    f.render_widget(dsearch_bar, chunks[1]);
                    let block = Block::default().borders(Borders::ALL).title(Span::styled(
                        "Coding Activity by Hour",
                        Style::default().fg(Color::Rgb(231, 76, 125)).add_modifier(Modifier::BOLD),
                    ));
                    let in_chart = block.inner(chunks[2]);
                    f.render_widget(block, chunks[2]);
                    rdr_day(f, in_chart, &day_hours, is_today);
                }
            }
        })?;

        if event::poll(StdDuration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('p') if view == View::Main => {
                        view = View::Projects;
                    }
                    KeyCode::Char('p') if view == View::Projects && !search_active => {
                        view = View::Main;
                        search_active = false;
                        search_q.clear();
                    }
                    KeyCode::Char('l') if view == View::Leaderboard && !lb_search_active => {
                        view = View::Main;
                        lb_scroll = 0;
                    }
                    KeyCode::Char('l') if view == View::Main => {
                        view = View::Leaderboard;
                    }
                    KeyCode::Char('l') if view == View::Projects && !search_active => {
                        view = View::Leaderboard;
                        search_active = false;
                        search_q.clear();
                    }
                    KeyCode::Char('p') if view == View::Leaderboard && !lb_search_active => {
                        view = View::Projects;
                        lb_scroll = 0;
                    }
                    KeyCode::Tab if view == View::Leaderboard => {
                        lb_period = match lb_period {
                            LbType::Daily => LbType::Weekly,
                            LbType::Weekly => LbType::Daily,
                        };
                        lb_scroll = 0;
                    }
                    KeyCode::Down if view == View::Leaderboard => {
                        lb_scroll = lb_scroll.saturating_add(1);
                    }
                    KeyCode::Up if view == View::Leaderboard => {
                        lb_scroll = lb_scroll.saturating_sub(1);
                    }
                    KeyCode::PageDown if view == View::Leaderboard => {
                        lb_scroll = lb_scroll.saturating_add(10);
                    }
                    KeyCode::PageUp if view == View::Leaderboard => {
                        lb_scroll = lb_scroll.saturating_sub(10);
                    }
                    KeyCode::Char('/') if view == View::Leaderboard => {
                        lb_search_active = true;
                    }
                    KeyCode::Char(c) if view == View::Leaderboard && lb_search_active => {
                        lb_search_q.push(c);
                        lb_scroll = 0;
                    }
                    KeyCode::Backspace if view == View::Leaderboard && lb_search_active => {
                        lb_search_q.pop();
                        lb_scroll = 0;
                    }
                    KeyCode::Esc if view == View::Leaderboard => {
                        lb_search_active = false;
                        lb_search_q.clear();
                        lb_scroll = 0;
                    }
                    KeyCode::Enter if view == View::Leaderboard && lb_search_active => {
                        lb_search_active = false;
                    }
                    KeyCode::Down if view == View::Projects => {
                        project_scroll = project_scroll.saturating_add(1);
                    }
                    KeyCode::Up if view == View::Projects => {
                        project_scroll = project_scroll.saturating_sub(1);
                    }
                    KeyCode::PageDown if view == View::Projects => {
                        project_scroll = project_scroll.saturating_add(8);
                    }
                    KeyCode::PageUp if view == View::Projects => {
                        project_scroll = project_scroll.saturating_sub(8);
                    }
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
                    KeyCode::Char('d') if view == View::Main => {
                        view = View::Day;
                    }
                    KeyCode::Char('d') | KeyCode::Esc if view == View::Day && !jump_active => {
                        view = View::Main;
                    }
                    KeyCode::Char('j') if view == View::Day && !jump_active => {
                        jump_active = true;
                        jump_input.clear();
                        jump_err.clear();
                    }
                    KeyCode::Char(c) if view == View::Day && jump_active => {
                        jump_input.push(c);
                    }
                    KeyCode::Backspace if view == View::Day && jump_active => {
                        jump_input.pop();
                        jump_err.clear();
                    }
                    KeyCode::Enter if view == View::Day && jump_active => {
                        match NaiveDate::parse_from_str(jump_input.trim(), "%Y-%m-%d") {
                            Ok(parsed) => {
                                let today = Local::now().date_naive();
                                if parsed <= today {
                                    day_date = parsed;
                                    day_hours = runtime.block_on(fetch_hourly(&config, day_date)).unwrap_or([0.0; 24]);
                                    jump_active = false;
                                    jump_input.clear();
                                    jump_err.clear();
                                } else {
                                    jump_err = "Can't jump to a future date".to_string();
                                }
                            }
                            Err(_) => {
                                jump_err = "Invalid format. Use YYYY-MM-DD".to_string();
                            }
                        }
                    }
                    KeyCode::Esc if view == View::Day && jump_active => {
                        jump_active = false;
                        jump_input.clear();
                        jump_err.clear();
                    }
                    KeyCode::Left if view == View::Day && !jump_active => {
                        day_date = day_date - chrono::Duration::days(1);
                        day_hours = runtime.block_on(fetch_hourly(&config, day_date)).unwrap_or([0.0; 24]);
                    }
                    KeyCode::Right if view == View::Day && !jump_active => {
                        let today = Local::now().date_naive();
                        if day_date < today {
                            day_date = day_date + chrono::Duration::days(1);
                            day_hours = runtime.block_on(fetch_hourly(&config, day_date)).unwrap_or([0.0; 24]);
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        if last_r.elapsed() > std::time::Duration::from_secs(60) {
                            let (ntoday_text, nweekly_header, nlanguages, nprojects, nproject_cards, ndaily_lb, nweekly_lb, nheatmap, ncurrent_streak, nlongest_streak, nday_hours, ) = refresh(&runtime, &config, day_date);
                            today_text = ntoday_text;
                            weekly_header = nweekly_header;
                            languages = nlanguages;
                            projects = nprojects;
                            project_cards = nproject_cards;
                            daily_lb = ndaily_lb;
                            weekly_lb = nweekly_lb;
                            heatmap = nheatmap;
                            current_streak = ncurrent_streak;
                            longest_streak = nlongest_streak;
                            day_hours = nday_hours;
                            last_r = std::time::Instant::now();
                        }
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

fn build_hmap(activity: &BTreeMap<NaiveDate, f64>) -> Vec<Vec<u8>> {
    let mut grid = vec![vec![0u8; HMAPR]; HMAPC];
    let today = Local::now().date_naive();
    for i in 0..HMAPC {
        for j in 0..HMAPR {
            let idx = i * HMAPR + j;
            let days_ago = (HMAPC * HMAPR - 1) - idx;
            let day = today - Duration::days(days_ago as i64);
            let secs = activity.get(&day).copied().unwrap_or(0.0);
            let level = if secs == 0.0 {
                0
            } else {
                let hrs = secs / 3600.0;
                match hrs {
                    h if h < 0.25 => 1,
                    h if h < 1.0 => 2,
                    h if h < 3.0 => 3,
                    _ => 4,
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
            spans.push(Span::styled("  ", Style::default().bg(heat_color(level))));
            spans.push(Span::styled(" ", Style::default().fg(Color::Reset).bg(Color::Reset)));
        }
        lines.push(Line::from(spans));
        if row + 1 < HMAPR {
            lines.push(Line::from(" "));
        }
    }
    lines
}

fn rdr_day(f: &mut Frame, area: Rect, hours: &[f64; 24], is_today: bool) {
    if area.height < 4 || area.width < 10 {
        return;
    }
    let now_hour = Local::now().hour() as usize;
    let max_secs = hours.iter().cloned().fold(0.0f64, f64::max).max(1.0);
    let y_axis_w: u16 = 6;
    let label_h: u16 = 1;
    let chart_h = area.height.saturating_sub(label_h);
    let chart_w = area.width.saturating_sub(y_axis_w);
    let w = ((chart_w as usize) / 24).max(1) as u16;
    let mut lines: Vec<Line<'static>> = Vec::new();
    for i in 0..chart_h {
        let row_frac = 1.0 - (i as f64 / chart_h as f64);
        let y_label = if i == 0 {
            let m = (max_secs / 60.0) as u64;
            if m >= 60 {
                format!("{:>4}h  ", m / 60)
            } else {
                format!("{:>4}m  ", m)
            }
        } else if i == chart_h / 2 {
            let m = (max_secs / 2.0 / 60.0) as u64;
            if m >= 60 {
                format!("{:>4}h  ", m / 60)
            } else {
                format!("{:>4}m  ", m)
            }
        } else if i == chart_h - 1 {
            "   0   ".to_string()
        } else {
            "       ".to_string()
        };
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::styled(
            y_label,
            Style::default().fg(Color::Rgb(100, 100, 120)),
        ));

        for h in 0..24 {
            let frac = (hours[h] / max_secs).min(1.0);
            let filled = frac >= row_frac;
            let color = if filled {
                if is_today && h == now_hour {
                    Color::Rgb(255, 130, 160)
                } else if hours[h] > 0.0 {
                    let bb = (frac * 255.0) as u8;
                    Color::Rgb(bb.max(80), 30, 70)
                } else {
                    Color::Rgb(40, 40, 55)
                }
            } else {
                Color::Rgb(28, 28, 38)
            };
            let charr = if filled { "#" } else { "." };
            let cell: String = charr.repeat(w as usize);
            spans.push(Span::styled(cell, Style::default().fg(color)));
            if w > 1 {
                spans.push(Span::styled(" ", Style::default()));
            }
        }
        lines.push(Line::from(spans));
    }

    let mut lspans: Vec<Span<'static>> = Vec::new();
    lspans.push(Span::raw("       "));
    for h in 0..24 {
        let label = if w >= 2 {
            format!("{:02}", h)
        } else if h % 2 == 0 {
            format!("{:02}", h)
        } else {
            "  ".to_string()
        };
        let style = if is_today && h == now_hour {
            Style::default().fg(Color::Rgb(255, 130, 160)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(100, 100, 120))
        };
        let padded = format!("{:<width$}", label, width = w as usize);
        lspans.push(Span::styled(padded, style));
        if w > 1 {
            lspans.push(Span::raw(" "));
        }
    }
    lines.push(Line::from(lspans));
    f.render_widget(Paragraph::new(lines), area);
}

fn refresh( runtime: &tokio::runtime::Runtime, config: &Config, day_date: NaiveDate, ) -> ( String, String, Vec<Item>, Vec<Item>, Vec<ProjectItem>, Vec<(String, String)>, Vec<(String, String)>, Vec<Vec<u8>>, u64, u64, [f64; 24], ) {
    if config.api_key.is_empty() {
        let activity = BTreeMap::new();
        return ( "set 'HACKATIME_API_KEY' in env to fetch coding stats".to_string(), String::new(), vec![], vec![], vec![], vec![], vec![], build_hmap(&activity), 0, 0, [0.0; 24], );
    }

    let today_text = match runtime.block_on(fetch_summary(config)) {
        Ok(summary) => format!(
            "Today: {} ({}% completed)",
            summary.data.grand_total.text, summary.data.goal.completion_percent,
        ),
        Err(e) => format!("{}", e),
    };

    let (weekly_header, languages, projects) = match runtime.block_on(fetch_weekly_stats(config)) {
        Ok(stats) => (
            format!(
                "This week: {} ({})",
                stats.data.human_readable_total, stats.data.human_readable_range,
            ),
            stats.data.languages.into_iter().take(8).collect::<Vec<_>>(),
            stats.data.projects.into_iter().take(8).collect::<Vec<_>>(),
        ),
        Err(e) => (format!("{}", e), vec![], vec![]),
    };

    let pcards = runtime.block_on(fetch_projects(config)).unwrap_or_default();
    let daily_lb = runtime.block_on(fetch_daily_leaderboard(config)).unwrap_or_default();
    let weekly_lb = runtime.block_on(fetch_weekly_leaderboard(config)).unwrap_or_default();
    let activity = runtime.block_on(fetch_spans(config)).unwrap_or_default();
    let hmap = build_hmap(&activity);
    let (cstreak, lstreak) = streaks(&activity);
    let day_hours = runtime.block_on(fetch_hourly(config, day_date)).unwrap_or([0.0; 24]);

    ( today_text, weekly_header, languages, projects, pcards, daily_lb, weekly_lb, hmap, cstreak, lstreak, day_hours,
    )
}