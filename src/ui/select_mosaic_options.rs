use ratatui::{
    Frame, layout::{
        Constraint, Direction, Flex, HorizontalAlignment, Layout, Rect, Size,
    }, style::{
        Color, Modifier, Style,
    }, text::{Line, Span, Text}, widgets::{
        Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Table, TableState, Wrap
    }
};
use ratatui_image::{FilterType, Image, Resize, picker::Picker};

use crate::app::{App, AppStage};

const MAX_IMAGE_HEIGHT: u16 = 30;

pub fn render(frame: &mut Frame, main: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("  Image > Mosaic Options  ")
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

    // calculate max height for preview image
    let ratio = selected_image.width as f64 / selected_image.height as f64;
    let image_container_width = inner.width - 2;
    let image_container_height = ((image_container_width as f64 / ratio) / 2.0).round() as u16;
    let max_image_container_height = image_container_height.min(MAX_IMAGE_HEIGHT);

    // calculate height for tile options section
    let num_options = tiling_options.len() as u16;
    let tabs_block_padding: u16 = 2;
    let options_table_header_margin: u16 = 1;
    let option_section_height = (tabs_block_padding * 2) + num_options + options_table_header_margin + 3;
    
    let [
        header_section,
        image_preview_section,
        instructions_section,
        options_section,
        continue_section,
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(max_image_container_height),
            Constraint::Length(2),
            Constraint::Length(option_section_height),
            Constraint::Length(2)
        ])
        .spacing(1)
        .flex(Flex::Start)
        .areas(inner);

    // title
    let title = Paragraph::new(
        Text::styled("Select Mosaic Options", Style::default().bg(Color::Red))
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

    let protocol = picker.new_protocol(image.image().clone(), size, Resize::Scale(Some(FilterType::Nearest))).unwrap();
    let image_widget = Image::new(&protocol);

    frame.render_widget(image_widget, image_area);

    // instructions
    let instructions_text = indoc::indoc! {"
        Use (Up) & (Down) arrows. Press (Space) to select tile option.
    "};

    let instructions =  Paragraph::new(
        Text::styled(instructions_text, Style::default().fg(Color::Green))
    )
    .wrap(Wrap::default())
    .alignment(HorizontalAlignment::Center);

    frame.render_widget(instructions, instructions_section);

    // Mosaic Options
    let tabs_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Line::from(vec![
            Span::from("  "),
            Span::styled("Landscape 16:9", Style::default().fg(Color::Magenta).bg(Color::Black)),
            Span::from(" * "),
            Span::styled("Square 1x1", Style::default()),
            Span::from(" * "),
            Span::styled("Portrait 9:16", Style::default()),
            Span::from("  "),
        ]))
        .padding(Padding::uniform(tabs_block_padding))
        .style(Style::default());

    let tabs_inner = tabs_block.inner(options_section);
    frame.render_widget(tabs_block, options_section);

    let highlight_symbol = "> ";

    let select_heading = "[ ]";
    let tile_heading = "Tile Layout";
    let print_heading = "Suitable Paper Size";
    let dimensions_heading = "Print Dimensions";
    let db_heading = "DB";

    let header = Row::new([
        Cell::from(select_heading),
        Cell::from(Text::from(tile_heading).left_aligned()),
        Cell::from(Text::from(print_heading).left_aligned()),
        Cell::from(Text::from(dimensions_heading).left_aligned()),
        Cell::from(Text::from(db_heading).right_aligned()),
    ])
    .style(Style::default().bold())
    .bottom_margin(options_table_header_margin);

    let stripe_color = &app.terminal_palette.background_color
        .blend_toward(&app.terminal_palette.foreground_color, 5);

    let rows = tiling_options
        .iter().enumerate()
        .map(|(i, mt)| {
            let marker = if mt.is_chosen { "[X]" } else { "[ ]" };
            let tile = format!("{}x{}", mt.num_tiles_x, mt.num_tiles_y);
            let print = format!("{}", mt.ideal_print_size);

            let is_even = i % 2 == 0;

            let mut style = Style::default();
            style = if mt.is_chosen { style } else { style.add_modifier(Modifier::DIM) };

            if is_even {
                style = style.bg(stripe_color.to_ratatui_color());
            }

            let (mm_width, mm_height) = mt.ideal_print_size
                .measurements_for_contained_image(selected_image.width, selected_image.height);
            let cm_width = mm_width as f64 / 10.0;
            let cm_height = mm_height as f64 / 10.0;
            
            let dimensions = format!("{cm_width:>5.1} x {cm_height} cm");

            let db_loaded = if mt.image_tiles.is_some() { "✔" } else { "" };

            Row::new([
                Cell::from(marker),
                Cell::from(tile),
                Cell::from(print),
                Cell::from(dimensions),
                Cell::from(Text::from(db_loaded).right_aligned()),
            ]).style(style)
        });

    let column_widths = [
        Constraint::Length(select_heading.len() as u16),
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Min(1),
        Constraint::Length(4),
    ];

    let table = Table::new(rows, column_widths)
        .header(header)
        .column_spacing(2)
        .style(Color::White)
        .row_highlight_style(Modifier::REVERSED)
        .highlight_symbol(highlight_symbol);

    let mut table_state = TableState::default()
        .with_selected(selected_image.selected_tiling_option_index);
    
    frame.render_stateful_widget(table, tabs_inner, &mut table_state);

    // continue instructions
    if app.stage == AppStage::SelectMosaicOptions {
        let can_continue = tiling_options.iter().any(|t| t.is_chosen);

        let mut continue_line = Line::from("Press (Enter) to continue.");
        if !can_continue {
            continue_line = continue_line.style(Modifier::DIM);
        }

        let backspace_line = Line::from("Press (Backspace) to go back.");

        let continue_instructions =  Paragraph::new(vec![continue_line, backspace_line])
            .wrap(Wrap::default())
            .alignment(HorizontalAlignment::Center);

        frame.render_widget(continue_instructions, continue_section);
    }
    else {
        let processing_message =  Paragraph::new("Processing image colors. Please wait...")
            .wrap(Wrap::default())
            .alignment(HorizontalAlignment::Center);

        frame.render_widget(processing_message, continue_section);
    }
}