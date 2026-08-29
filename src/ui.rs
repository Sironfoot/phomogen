pub mod under_construction;
pub mod initial_loading;
pub mod video_selection;
pub mod generate_database;
pub mod load_databaqse;
pub mod image_selection;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph}
};

use crate::{app::{App, AppStage}};

pub fn render_ui(frame: &mut Frame, app: &mut App){
    // Header
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = chunks[0];
    let main_content = chunks[1];
    let footer = chunks[2];

    let header_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let title = Paragraph::new(
        Text::styled("PhoMoGen\nA PHOto MOsaic GENerator written in Rust", Style::default().fg(Color::Green))
    )
    .alignment(ratatui::layout::HorizontalAlignment::Center)
    .block(header_block);

    frame.render_widget(title, header);

    // Main section
    match app.stage {
        AppStage::Initial => initial_loading::render(frame, main_content),
        AppStage::VideoSelect => video_selection::render(frame, main_content, app),
        AppStage::GenerateMosaicDatabase => generate_database::render(frame, main_content, app),
        AppStage::LoadMosaicDatabase => load_databaqse::render(frame, main_content, app),
        AppStage::ImageSelect => image_selection::render(frame, main_content, app),
        _ => under_construction::render(frame, main_content),
    };

    // Footer
    let footer_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let footer_text = Paragraph::new(
        Text::styled("by Dominic Pettifer (v0.1.0). Press (q) to Quit.", Style::default().bg(Color::Blue))
    )
    .alignment(ratatui::layout::HorizontalAlignment::Center)
    .block(footer_block);

    frame.render_widget(footer_text, footer);
}