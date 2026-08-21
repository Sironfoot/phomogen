use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Padding, Paragraph}
};

pub fn render(frame: &mut Frame, main: Rect) {
    let main_block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default());

    let text = Paragraph::new(
        Text::styled("---- UNDER CONSTRUCTION ----", Style::default().bg(Color::Red))
    )
    .alignment(ratatui::layout::HorizontalAlignment::Center)
    .block(main_block.clone());

    frame.render_widget(text, main);
}