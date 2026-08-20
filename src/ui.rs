use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Text},
    widgets::{Block, Borders, Paragraph}
};

use crate::app::App;

pub fn render_ui(frame: &mut Frame, app: &App){
    // Header
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let title = Paragraph::new(
        Text::styled("PhoMoGen\nA PHOto MOsaic GENerator written in Rust", Style::default().fg(Color::Green))
    )
    .alignment(ratatui::layout::HorizontalAlignment::Center)
    .block(header_block);

    frame.render_widget(title, chunks[0]);

    // Main section
    let main_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let main_text = Paragraph::new(
        Text::styled("Main Content Here", Style::default().bg(Color::Red))
    )
    .alignment(ratatui::layout::HorizontalAlignment::Center)
    .block(main_block);

    frame.render_widget(main_text, chunks[1]);

    // Footer
    let footer_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let footer_text = Paragraph::new(
        Text::styled("Project by Dominic Pettifer. Press (q) to Quit.", Style::default().bg(Color::Blue))
    )
    .alignment(ratatui::layout::HorizontalAlignment::Center)
    .block(footer_block);

    frame.render_widget(footer_text, chunks[2]);
}