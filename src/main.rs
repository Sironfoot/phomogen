pub mod app;
pub mod ui;

use std::io;

use ratatui::Terminal;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::crossterm::{event, execute};
use ratatui::crossterm::terminal::{
    EnterAlternateScreen,
    enable_raw_mode,
    LeaveAlternateScreen,
    disable_raw_mode
};

use anyhow::Result;

use crate::app::App;
use crate::ui::render_ui;

fn main() -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    run_app(&mut terminal, &mut app)?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;

    terminal.show_cursor()?;

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()>
where
    io::Error: From<B::Error>
{
    loop {
        terminal.draw(|frame| render_ui(frame, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => {
                    return Ok(())
                },
                _ => {}
            }
        }
    }
}