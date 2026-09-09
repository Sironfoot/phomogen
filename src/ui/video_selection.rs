use std::time::Duration;

use ratatui::{
    Frame, layout::{
        Constraint,
        Direction,
        HorizontalAlignment,
        Layout,
        Rect
    }, style::{
        Color,
        Modifier,
        Style
    }, text::Text, widgets::{
        Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Table, TableState, Wrap
    }
};

use num_format::{Locale, ToFormattedString};

use crate::app::{App, AppStage};

pub fn render(frame: &mut Frame, main: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::uniform(1))
        .title("  Select Videos  ")
        .style(Style::default());

    let inner = block.inner(main);
    frame.render_widget(block, main);

    let list_item_height = app.videos.len();
    let is_loading = app.stage == AppStage::LoadMosaicDatabase;

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
            Constraint::Length((list_item_height + 2) as u16),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .spacing(1)
        .areas(inner);
    
    // title
    let title_text = if is_loading { "Loading databases..." } else { "Please select 1 or more videos from the list" };

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

    // render table
    let highlight_symbol = "> ";

    let select_heading = if is_loading { "  [ ]" } else { "[ ]" };
    let video_heading = "Video";
    let frames_heading = "Frames";
    let duration_heading = "Length";
    let db_loaded_heading = "DB";

    let header = Row::new([
        Cell::from(select_heading),
        Cell::from(""), 
        Cell::from(Text::from(video_heading).left_aligned()),
        Cell::from(Text::from(frames_heading).left_aligned()),
        Cell::from(Text::from(duration_heading).left_aligned()),
        Cell::from(Text::from(db_loaded_heading).right_aligned()),
    ])
    .style(Style::default().bold())
    .bottom_margin(1);

    let stripe_color = &app.terminal_palette.background_color
        .blend_toward(&app.terminal_palette.foreground_color, 5);

    let rows = app.videos.iter().enumerate().map(|(i, v)| {
        let marker = if v.is_chosen { "[X]" } else { "[ ]" };
        let marker = if is_loading { format!("  {marker}") } else { String::from(marker) };

        let data_exists_flag = if v.database_path.is_some() { "✔" } else { "" };
        let filename = v.metadata.file_name.as_str();
        let total_frames = v.metadata.total_frames.to_formatted_string(&Locale::en);
        let duration = format_duration(v.metadata.duration);

        let db_loaded = if v.is_loading_database {
            let percentage_complete = ((100.0 / v.metadata.total_frames as f64) * v.total_database_frames_loaded as f64).round() as u8;
            format!("{percentage_complete}%")
        }
        else {
            if v.database.is_some() { String::from(" ✔") } else { String::from("  ") }
        };

        let is_even = i % 2 == 0;

        let mut style = Style::default();
        style = if v.is_chosen { style } else { style.add_modifier(Modifier::DIM) };

        if is_even {
            style = style.bg(stripe_color.to_ratatui_color());
        }

        Row::new([
            Cell::from(marker),
            Cell::from(data_exists_flag),
            Cell::from(filename),
            Cell::from(Text::from(total_frames).right_aligned()),
            Cell::from(Text::from(duration).right_aligned()),
            Cell::from(Text::from(db_loaded).right_aligned()),
        ]).style(style)
    });

    let column_widths = [
        Constraint::Length(select_heading.len() as u16),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(frames_col_width.max(frames_heading.len() as u16)),
        Constraint::Length(duration_col_width.max(duration_heading.len() as u16)),
        Constraint::Length(3),
    ];

    let table = Table::new(rows, column_widths)
        .header(header)
        .column_spacing(2)
        .style(Color::White)
        .row_highlight_style(Modifier::REVERSED)
        .highlight_symbol(highlight_symbol);

    let mut table_state = if is_loading { TableState::default() }
        else { TableState::default().with_selected(app.current_video_index as usize) };

    frame.render_stateful_widget(table, list_section, &mut table_state);

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
    let continue_active = at_least_one_selected && !is_loading;
    let cont_color = if continue_active { Color::White } else { Color::DarkGray };

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