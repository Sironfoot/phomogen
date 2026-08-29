use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Padding, Paragraph, Wrap}
};

use num_format::{Locale, ToFormattedString};

use crate::app::{App, VideoFile};

pub fn render(frame: &mut Frame, main: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default());

    let inner = block.inner(main);
    frame.render_widget(block, main);

    let videos = app.videos.iter()
        .filter(|v| v.is_chosen && (v.database.is_none() || v.total_database_frames_loaded > 0))
        .collect::<Vec<&VideoFile>>();

    let [
        header_section,
        progress_section,
        continue_section,
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(videos.len() as u16),
            Constraint::Length(2),
        ])
        .spacing(1)
        .areas(inner);

    // title
    let title = Paragraph::new(
        Text::styled("Loading Mosaic Database - Please wait", Style::default().bg(Color::Red))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(title, header_section);

    // progress report
    let mut output = String::new();

    for video in videos {
        let mut line = format!(" - {}: {} / {} frames processed",
            video.metadata.file_name,
            video.total_database_frames_loaded.to_formatted_string(&Locale::en),
            video.metadata.total_frames.to_formatted_string(&Locale::en));

        if video.total_dropped_frames > 0 {
            line.push_str(&format!(" - DROPPED FRAMES: {}", video.total_dropped_frames));
        }

        line.push_str("\n");

        output.push_str(&line);
    }

    let progress_report = Paragraph::new(
        Text::styled(output, Style::default().fg(Color::White))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Left);

    frame.render_widget(progress_report, progress_section);

    // continue
    let continue_message = Paragraph::new(
        Text::styled("Will continue to next screen when done", Style::default().fg(Color::White))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Left);

    frame.render_widget(continue_message, continue_section);
}