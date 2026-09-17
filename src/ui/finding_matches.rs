use std::time::Duration;

use ratatui::{
    Frame, layout::{
        HorizontalAlignment, Constraint, Direction, Layout, Rect, Size
    }, style::{
        Color,
        Modifier,
        Style
    }, text::{Text}, widgets::{
        Block,
        BorderType,
        Borders,
        Gauge,
        Padding,
        Paragraph,
        Wrap
    }
};
use ratatui_image::{FilterType, Image, Resize, picker::Picker};

use crate::app::{App, AppStage};

const MAX_IMAGE_HEIGHT: u16 = 30;

pub fn render(frame: &mut Frame, main: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("  Image > Options > Videos > Finding Matches  ")
        .padding(Padding::uniform(1))
        .style(Style::default());

    let inner = block.inner(main);
    frame.render_widget(block, main);

    let Some(selected_image) = app.images.iter()
        .find(|i| i.is_chosen) else { return; };

    let Some(image) = &selected_image.preview else { return; };

    let selected_shape = &app.selected_tile_shape;
    let Some(tiling_options) = selected_image.tiling_options
        .get(selected_shape) else { return; };

    let selected_tiling_options = tiling_options.iter()
        .filter(|t| t.is_chosen)
        .collect::<Vec<_>>();

    // calculate max height for preview image
    let ratio = selected_image.width as f64 / selected_image.height as f64;
    let image_container_width = inner.width - 2;
    let image_container_height = ((image_container_width as f64 / ratio) / 2.0).round() as u16;
    let max_image_container_height = image_container_height.min(MAX_IMAGE_HEIGHT);

    // calculate height for tile options section
    let num_options = (selected_tiling_options.len() as u16) * 2;

    let [
        header_section,
        image_preview_section,
        progress_section,
        total_progress_section,
        timer_section,
        continue_section,
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(max_image_container_height),
            Constraint::Length(num_options),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(2)
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

    let progress_image = image.progress_image().unwrap_or(image.image());
    let protocol = picker.new_protocol(progress_image.clone(), size, Resize::Scale(Some(FilterType::Nearest))).unwrap();
    let image_widget = Image::new(&protocol);

    frame.render_widget(image_widget, image_area);

    let mut total_mosaic_tiles: u32 = 0;
    let mut total_tiles_complete: u32 = 0;

    // progress section
    let progress_layouts = Layout::default()
        .direction(Direction::Vertical)
        .constraints(selected_tiling_options.iter().map(|_| Constraint::Length(1)))
        .spacing(1)
        .split(progress_section);

    for (i, tiling_option) in selected_tiling_options.iter().enumerate() {
        let num_mosaic_tiles = tiling_option.num_tiles_x as u32 * tiling_option.num_tiles_y as u32;
        let num_tiles_complete = tiling_option.matched_tiles.as_ref()
            .map_or(0, |t| t.len()) as u32;

        total_mosaic_tiles += num_mosaic_tiles;
        total_tiles_complete += num_tiles_complete;

        let percentage = (100.0 / num_mosaic_tiles as f64) * num_tiles_complete as f64;

        let label = format!("{:>2} x {:<2} - Tiles: {num_tiles_complete:>4} / {num_mosaic_tiles:<4} - {:>3.0}%",
            tiling_option.num_tiles_x, tiling_option.num_tiles_y, percentage);

        let tile_match_progress_guage = Gauge::default()
            .style(Modifier::BOLD)
            .gauge_style(Style::new().blue().on_black())
            
            .label(label)
            .percent(percentage.round() as u16);

        frame.render_widget(tile_match_progress_guage, progress_layouts[i]);
    }

    // total progress section
    let total_percentage = (100.0 / total_mosaic_tiles as f64) * total_tiles_complete as f64;

    let total_progress_guage = Gauge::default()
        .style(Modifier::BOLD)
        .gauge_style(Style::new().red().on_black())
        .label(format!("Total Progress ({:.2}%)", total_percentage))
        .percent(total_percentage.round() as u16);
    frame.render_widget(total_progress_guage, total_progress_section);

    // timer section
    let elapsed = app.timer_ellapsed();
    let timer = format_duration(elapsed);

    let timer_text = format!("Time elapsed: {timer}");
    let timer = Paragraph::new(
        Text::styled(timer_text, Style::default().fg(Color::White))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(timer, timer_section);

    // continue or wait section
    if app.stage == AppStage::FindingMatches {
        let wait_text = Paragraph::new(
            Text::styled("Please wait...", Style::default().fg(Color::Green))
        )
        .wrap(Wrap::default())
        .alignment(ratatui::layout::HorizontalAlignment::Center);

        frame.render_widget(wait_text, continue_section);
    }
    else {
        let text = "Finished! Press (Enter) to continue.\nPress (Backspace) to go back.";

        let continue_text = Paragraph::new(
            Text::styled(text, Style::default().fg(Color::Green))
        )
        .wrap(Wrap::default())
        .alignment(HorizontalAlignment::Center);

        frame.render_widget(continue_text, continue_section);
    }
}

const SECONDS_PER_HOUR: f64 = 3600.0;

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs() as f64;

    let hours = f64::floor(total_seconds / SECONDS_PER_HOUR) as u32;
    let minutes = f64::floor((total_seconds % SECONDS_PER_HOUR) / 60.0) as u32;
    let seconds = f64::floor(total_seconds % 60.0) as u32;

    format!("{hours}h {minutes}m {seconds}s")
}