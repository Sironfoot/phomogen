use ratatui::{
    Frame, layout::{Constraint, Direction, Layout, Rect, Size}, style::{Color, Modifier, Style}, text::Text, widgets::{Block, Borders, Gauge, Padding, Paragraph, Wrap}
};
use ratatui_image::{FilterType, Image, Resize, picker::Picker};

use crate::app::{App};

pub fn render(frame: &mut Frame, main: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default());

    let inner = block.inner(main);
    frame.render_widget(block, main);


    let [
        header_section,
        image_preview_section,
        progress_section,
        timer_section,
        continue_section,
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(1)
        ])
        .spacing(1)
        .areas(inner);

    // title
    let title = Paragraph::new(
        Text::styled("Finding Frame Matches", Style::default().bg(Color::Red))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(title, header_section);

    // image preview
    if let Some(selected_image) = app.images.iter().find(|i| i.is_chosen) {
        if let Some(image) = &selected_image.preview {
            let preview_area_width = image_preview_section.width - 2; // add padding to left/right
            let preview_area_height = image_preview_section.height;

            let mut image_width = preview_area_width;

            let mut image_height = (
                image_width as f64
                * selected_image.height as f64
                / selected_image.width as f64
                / 2.0
            ).round() as u16;

            if image_height > preview_area_height {
                image_height = preview_area_height;

                image_width = (
                    image_height as f64
                    * selected_image.width as f64
                    / selected_image.height as f64
                    * 2.0
                ).round() as u16;
            }

            let rect_x = image_preview_section.x +
                image_preview_section.width.saturating_sub(image_width) / 2;

            let image_area = Rect {
                x: rect_x,
                y: image_preview_section.y,
                width: image_width,
                height: image_height,
            };

            let size = Size::new(image_width, image_height);
            let picker = Picker::halfblocks();
            let protocol = picker.new_protocol(image.clone(), size, Resize::Scale(Some(FilterType::Nearest))).unwrap();
            let image_widget = Image::new(&protocol);

            frame.render_widget(image_widget, image_area);
        }
    }

    let mut is_finished = false;

    let choden_image = app.images.iter()
        .find(|i| i.is_chosen && i.image_tiles.is_some());

    if let Some(image) = choden_image {
        if let Some(image_tiles) = image.image_tiles.as_ref() {
            // progress
            let num_tiles_to_match = image_tiles.len() as u32;
            let mut num_tiles_matched: u32 = 0;

            if let Some(matched_tiles) = image.matched_tiles.as_ref() {
                num_tiles_matched = matched_tiles.len() as u32;
            }

            let percentage = (100.0 / num_tiles_to_match as f64) * num_tiles_matched as f64;

            let video_progress_guage = Gauge::default()
                .style(Modifier::BOLD)
                .gauge_style(Style::new().blue().on_black())
                .label(format!("Progress: {num_tiles_matched} / {num_tiles_to_match} tiles ({:.2}%)", percentage))
                .percent(percentage.round() as u16);

            frame.render_widget(video_progress_guage, progress_section);

            is_finished = percentage == 100.0;

            // timer
            let timer_text = format!("Timer: {:.2} s", app.timer_ellapsed().as_secs_f64());
            let timer = Paragraph::new(
                Text::styled(timer_text, Style::default().fg(Color::White))
            )
            .wrap(Wrap::default())
            .alignment(ratatui::layout::HorizontalAlignment::Center);

            frame.render_widget(timer, timer_section);
        }
    }

    if is_finished {
        let continue_text = Paragraph::new(
            Text::styled("Press (Space) to continue", Style::default().fg(Color::Green))
        )
        .wrap(Wrap::default())
        .alignment(ratatui::layout::HorizontalAlignment::Center);

        frame.render_widget(continue_text, continue_section);
    }
}