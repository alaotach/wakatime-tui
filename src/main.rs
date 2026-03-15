use std::io;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
    execute,
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    layout::{Layout, Constraint, Direction},
};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::time::Duration;

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    loop {
        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([Constraint::Length(3),Constraint::Min(0)]).split(size);
            let header = Paragraph::new("WakaTime TUI — Press q to quit").block(Block::default().borders(Borders::ALL).title("Header"));

            f.render_widget(header, chunks[0]);
        })?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    disable_raw_mode()?;
                    execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
                    terminal.show_cursor()?;
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