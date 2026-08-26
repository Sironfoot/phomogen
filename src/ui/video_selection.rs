use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, HorizontalAlignment, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap}
};

use num_format::{Locale, ToFormattedString};

use crate::app::App;

pub fn render(frame: &mut Frame, main: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default());

    let inner = block.inner(main);
    frame.render_widget(block, main);

    let list_item_height = app.videos.len();

    let [
        header_section,
        list_section,
        instructions_section,
        status_section,
        continue_section,
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(list_item_height as u16),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .spacing(1)
        .areas(inner);
    
    // title
    let title = Paragraph::new(
        Text::styled("Please select 1 or more videos from the list", Style::default().bg(Color::Red))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(title, header_section);

    // list
    let list_items = app.videos.iter()
        .map(|v| {
            let marker = if v.is_selected { "[X]" } else { "[ ]" };
            let style_color = if v.is_selected { Color::White } else { Color::DarkGray };
            let style = Style::default().fg(style_color);

            let duration = format_duration(v.metadata.duration);

            let variable_flag = if v.metadata.is_variable_frame_rate { " - (VRF)" } else { "" };
            
            ListItem::new(format!("{marker} {} - {duration}{variable_flag}", v.metadata.file_name)).style(style)
        })
        .collect::<Vec<ListItem>>();

    let mut list_state = ListState::default().with_selected(Some(app.current_video_index as usize));

    let list = List::new(list_items)
        .style(Color::White)
        .highlight_style(Modifier::REVERSED)
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_section, &mut list_state);

    // instructions
    let instructions_text = indoc::indoc! {"
        Use (Up) & (Down) arrows. Press (Space) to toggle selection.
        Press (a) to select all videos.
    "};

    let instructions =  Paragraph::new(
        Text::styled(instructions_text, Style::default().fg(Color::Green))
    )
    .wrap(Wrap::default())
    .alignment(HorizontalAlignment::Center);

    frame.render_widget(instructions, instructions_section);

    // status
    let total_duration = format_duration(app.total_selected_video_duration());
    let total_frames = app.total_selected_video_frames().to_formatted_string(&Locale::en);
    let status_text = format!("Total video length: {total_duration} - {total_frames} frames");

    let [length_section, warning_section] =
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1)
        ])
        .areas(status_section);

    let status_info =  Paragraph::new(
        Text::styled(status_text, Style::default().fg(Color::White))
    )
    .wrap(Wrap::default())
    .alignment(HorizontalAlignment::Center);

    frame.render_widget(status_info, length_section);

    let num_chosen_with_vfr = app.videos.iter()
        .filter(|v| v.is_selected && v.metadata.is_variable_frame_rate)
        .count();

    if num_chosen_with_vfr > 0 {
        let warning_text = format!("Warning: Variable frame rate (VFR) video{} selected.",
            if num_chosen_with_vfr > 1 { "s" } else {""});

        let warning_info =  Paragraph::new(
            Text::styled(warning_text, Style::default().fg(Color::Red))
        )
        .wrap(Wrap::default())
        .alignment(HorizontalAlignment::Center);

        frame.render_widget(warning_info, warning_section);
    }

    // continue message
    let at_least_one_selected = app.videos.iter().any(|v| v.is_selected);
    let cont_color = if at_least_one_selected { Color::White } else { Color::DarkGray };

    let continue_instructions =  Paragraph::new(
        Text::styled("Press (Enter) to continue.", Style::default().fg(cont_color))
    )
    .wrap(Wrap::default())
    .alignment(HorizontalAlignment::Center);

    frame.render_widget(continue_instructions, continue_section);
}

const SECONDS_PER_HOUR: f64 = 3600.0;

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs() as f64;

    let hours = f64::floor(total_seconds / SECONDS_PER_HOUR) as u32;
    let minutes = f64::floor((total_seconds % SECONDS_PER_HOUR) / 60.0) as u32;
    let seconds = f64::floor(total_seconds % 60.0) as u32;

    format!("{hours}:{:0>2}:{:0>2}", minutes, seconds)
}