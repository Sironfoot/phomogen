use ratatui::{
    Frame, layout::{Constraint, Direction, HorizontalAlignment, Layout, Rect, Size}, style::{Color, Modifier, Style}, text::Text, widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap}
};
use ratatui_image::{FilterType, Image, Resize, picker::Picker};

use crate::app::App;

pub fn render(frame: &mut Frame, main: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default());

    let inner = block.inner(main);
    frame.render_widget(block, main);

    let list_item_height = app.images.len();

    let [
        header_section,
        list_section,
        instructions_section,
        image_preview_section,
        continue_section,
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(list_item_height as u16),
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .spacing(1)
        .areas(inner);

    // title
    let title = Paragraph::new(
        Text::styled("Please select an image", Style::default().bg(Color::Red))
    )
    .wrap(Wrap::default())
    .alignment(ratatui::layout::HorizontalAlignment::Center);

    frame.render_widget(title, header_section);

    // list
    let list_items = app.images.iter()
        .map(|i| {
            let marker = if i.is_chosen { "[X]" } else { "[ ]" };
            let style_color = if i.is_chosen { Color::White } else { Color::DarkGray };
            let style = Style::default().fg(style_color);

            ListItem::new(format!("{marker} {}", i.file_name)).style(style)
        })
        .collect::<Vec<ListItem>>();

    let mut list_state = ListState::default().with_selected(Some(app.current_image_index as usize));

    let list = List::new(list_items)
        .style(Color::White)
        .highlight_style(Modifier::REVERSED)
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_section, &mut list_state);

    // instructions
    let instructions_text = indoc::indoc! {"
        Use (Up) & (Down) arrows. Press (Space) to select image.
    "};

    let instructions =  Paragraph::new(
        Text::styled(instructions_text, Style::default().fg(Color::Green))
    )
    .wrap(Wrap::default())
    .alignment(HorizontalAlignment::Center);

    frame.render_widget(instructions, instructions_section);

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