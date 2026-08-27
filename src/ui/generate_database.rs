use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Gauge, Padding, Paragraph, Wrap}
};

use num_format::{Locale, ToFormattedString};

use crate::app::{App, VideoIndexStatus};

pub fn render(frame: &mut Frame, main: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default());

    let inner = block.inner(main);
    frame.render_widget(block, main);

    let mut current_video = app.video_indexing_report.iter()
        .find(|r| r.status == VideoIndexStatus::Running);

    if current_video.is_none() {
        current_video = app.video_indexing_report.first();
    }

    let Some(current_video) = current_video else {
        render_ffmpeg_initialising(frame, inner);
        return;
    };

    let video_position = app.video_indexing_report.iter()
        .position(|r| r.file_name == current_video.file_name)
        .unwrap_or(0) + 1;

    let [
        header_section,
        video_info_section,
        progress_section,
        total_section,
        _,
        progress_bar_section
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length((current_video.cores.len() as u16) + 1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .spacing(1)
        .areas(inner);

    // title
    let title = Paragraph::new(
        Text::styled("Generating Mosaic Database", Style::default().bg(Color::Red))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(title, header_section);

    // video info
    let video_info = Paragraph::new(
        Text::styled(format!("Indexing '{}' ({video_position} of {})", current_video.file_name, app.video_indexing_report.len()), Style::default().fg(Color::Green))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(video_info, video_info_section);

    // progress
    let mut output = String::new();
    for core in &current_video.cores {
        let core_message = match core.status {
            VideoIndexStatus::Initialising => {
                format!("* Core {}: Intialising...\n", core.core_id)
            }
            _ => {
                format!("* Core {}: {} / {} frames ({:.2}%) - {:.2} fps\n",
                    core.core_id,
                    core.frames_processed.to_formatted_string(&Locale::en),
                    core.total_frames.to_formatted_string(&Locale::en),
                    core.percentage_complete(),
                    core.average_fps)
            }
        };

        output.push_str(&core_message); 
    }

    let progress_report = Paragraph::new(
        Text::styled(output, Style::default().fg(Color::White))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Left);

    frame.render_widget(progress_report, progress_section);

    // total
    let video_progress_guage = Gauge::default()
        .style(Modifier::BOLD)
        .gauge_style(Style::new().blue().on_black())
        .label(format!("Total: {} / {} frames ({:.2}%) - {:.2} fps",
            current_video.frames_processed().to_formatted_string(&Locale::en),
            current_video.total_frames().to_formatted_string(&Locale::en),
            current_video.percentage_complete(),
            current_video.average_fps()))
        .percent(current_video.percentage_complete().round() as u16);

    frame.render_widget(video_progress_guage, total_section);

    // progress bar
    let total_progress = app.total_video_indexing_progress();
    let total_progress_guage = Gauge::default()
        .style(Modifier::BOLD)
        .gauge_style(Style::new().red().on_black())
        .label(format!("Total Progress ({:.2}%)", total_progress))
        .percent(total_progress.round() as u16);
    frame.render_widget(total_progress_guage, progress_bar_section);
}

fn render_ffmpeg_initialising(frame: &mut Frame, inner: Rect) {
    let [
        header_section,
        message_section,
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .spacing(1)
        .areas(inner);

    // title
    let title = Paragraph::new(
        Text::styled("Generating Mosaic Database", Style::default().bg(Color::Red))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(title, header_section);

    // video info
    let video_info = Paragraph::new(
        Text::styled("Initialising FFMPEG", Style::default().fg(Color::Green))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(video_info, message_section);
}