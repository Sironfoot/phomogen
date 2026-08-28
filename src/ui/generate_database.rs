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

    let videos_to_index: Vec<_> = app.videos.iter()
        .filter(|v| v.indexing_report.is_some())
        .collect();

    let current_video = videos_to_index.iter().enumerate()
        .find(|(_, v)| v.indexing_report.as_ref().is_some_and(|r| r.status == VideoIndexStatus::Running));

    let (index, current_video) = match current_video {
        Some(video) => video,
        None => {
            render_ffmpeg_initialising(frame, inner);
            return;
        }
    };

    let video_position = index + 1;
    let num_videos_indexing = app.videos.iter()
        .filter(|v| v.indexing_report.is_some())
        .count();

    let report = match &current_video.indexing_report {
        Some(report) => report,
        None => {
            render_ffmpeg_initialising(frame, inner);
            return;
        }
    };

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
            Constraint::Length((report.cores.len() as u16) + 1),
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
        Text::styled(format!("Indexing '{}' ({video_position} of {num_videos_indexing})", current_video.metadata.file_name), Style::default().fg(Color::Green))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(video_info, video_info_section);

    // progress
    let mut output = String::new();
    for core in &report.cores {
        let core_message = match core.status {
            VideoIndexStatus::Initialising => {
                format!("* Core {}: Intialising...\n", core.instance_id)
            }
            _ => {
                format!("* Core {}: {} / {} frames ({:.2}%) - {:.2} fps\n",
                    core.instance_id,
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
            report.frames_processed().to_formatted_string(&Locale::en),
            report.total_frames.to_formatted_string(&Locale::en),
            report.percentage_complete(),
            report.average_fps()))
        .percent(report.percentage_complete().round() as u16);

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