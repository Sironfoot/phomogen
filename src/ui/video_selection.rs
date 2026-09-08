use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, HorizontalAlignment, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap}
};

use num_format::{Locale, ToFormattedString};

use crate::app::{App, AppStage};

pub fn render(frame: &mut Frame, main: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default());

    let inner = block.inner(main);
    frame.render_widget(block, main);

    let list_item_height = app.videos.len();
    let container_width = inner.width;
    let is_loading = app.stage == AppStage::LoadMosaicDatabase;

    let [
        header_section,
        column_headings_section,
        list_section,
        instructions_section,
        status_section,
        continue_section,
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(list_item_height as u16),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .spacing(1)
        .areas(inner);
    
    // title
    let title_text = if app.stage == AppStage::LoadMosaicDatabase
        { "Loading databases..." } else { "Please select 1 or more videos from the list" };

    let title = Paragraph::new(
        Text::styled(title_text, Style::default().bg(Color::Red))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(title, header_section);

    let frames_col_width = app.videos.iter()
        .max_by_key(|v| v.metadata.total_frames)
        .map(|v| v.metadata.total_frames)
        .unwrap_or(0)
        .to_formatted_string(&Locale::en)
        .len() as u16;

    let duration_col_width = app.videos.iter()
        .max_by_key(|v| v.metadata.duration)
        .map(|v| format_duration(v.metadata.duration))
        .unwrap_or_else(|| format_duration(Duration::new(0, 0)))
        .len() as u16;

    let db_loaded_symbol = "- ✔";
    let db_not_loaded_symbol = "   ";
    let db_col_width = db_loaded_symbol.len() as u16;

    let col_gap: u16 = 3;

    // columns headings
    let prefix_heading = "  [ ]   ";
    let prefix_heading_width = prefix_heading.len() as u16;

    let filename_col_width = container_width - (
        prefix_heading_width +
        frames_col_width +
        duration_col_width +
        db_col_width
    );
    let video_heading = format!("{:<spaces$}", "Video", spaces = (filename_col_width - col_gap) as usize);
    let frames_heading = format!("{:<spaces$}", "Frames", spaces = (frames_col_width + col_gap) as usize);
    let duration_heading = format!("{:<spaces$}", "Length", spaces = (duration_col_width + col_gap) as usize);
    let loaded_heading = format!("{:<spaces$}", "DB", spaces = db_col_width as usize);
    

    let column_headings = Paragraph::new(
        Text::styled(format!("{prefix_heading}{video_heading}{frames_heading}{duration_heading}{loaded_heading}"), Style::default().add_modifier(Modifier::UNDERLINED))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Left);

    frame.render_widget(column_headings, column_headings_section);

    // video list
    let fake_cursor_space = if is_loading { "  " } else { "" };

    let stripe_color = &app.terminal_palette.background_color
        .blend_toward(&app.terminal_palette.foreground_color, 5);

    let list_items = app.videos.iter().enumerate()
        .map(|(i, v)| {
            let is_even = i % 2 == 0;

            // left aligned parts
            let marker = if v.is_chosen { "[X]" } else { "[ ]" };
            let data_exists_flag = if v.database_path.is_some() { "✔" } else { " " };

            let filename = &v.metadata.file_name;

            let left_aligned = format!("{fake_cursor_space}{marker} {data_exists_flag} {filename}");

            // right aligned parts
            let total_frames = v.metadata.total_frames.to_formatted_string(&Locale::en);
            let total_frames = format!("{:>spaces$}", total_frames, spaces = frames_col_width as usize);

            let duration = format_duration(v.metadata.duration);
            let duration = format!("{:>spaces$}", duration, spaces = duration_col_width as usize);

            let database_loaded = if v.database.is_some() { db_loaded_symbol} else { db_not_loaded_symbol };

            let right_aligned = format!("{total_frames}   {duration}   {database_loaded}");

            let remaining_space = container_width - (left_aligned.len() + right_aligned.len()) as u16;
            let spaces = " ".repeat(remaining_space as usize);

            let mut style = Style::default();
            style = if v.is_chosen { style } else { style.add_modifier(Modifier::DIM) };
  
            if is_even {
                style = style.bg(stripe_color.to_ratatui_color());
            }
            
            ListItem::new(format!("{left_aligned}{spaces}{right_aligned}")).style(style)
        })
        .collect::<Vec<ListItem>>();

    let mut list_state = if !is_loading {
        ListState::default().with_selected(Some(app.current_video_index as usize))
    }
    else {
        ListState::default()
    };

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

    let [length_section, checkmark_section, warning_section] =
        Layout::vertical([
            Constraint::Length(1),
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

    let at_least_one_video_has_database = app.videos.iter().any(|v| v.database_path.is_some());
    let checkmark_message_color = if at_least_one_video_has_database { Color::White } else { Color::DarkGray };

    let checkmark_info =  Paragraph::new(
        Text::styled("✔ = Colour index database already generated", Style::default().fg(checkmark_message_color))
    )
    .wrap(Wrap::default())
    .alignment(HorizontalAlignment::Center);

    frame.render_widget(checkmark_info, checkmark_section);

    let num_chosen_with_vfr = app.videos.iter()
        .filter(|v| v.is_chosen && v.metadata.is_variable_frame_rate)
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
    let at_least_one_selected = app.videos.iter().any(|v| v.is_chosen);
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