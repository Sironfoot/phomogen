use crate::app::frame_data::Color;

pub struct TerminalTheme {
    pub mode: TerminalThemeMode,
    pub foreground_color: Color,
    pub background_color: Color,
}

#[derive(PartialEq, Clone, Debug)]
pub enum TerminalThemeMode {
    Light,
    Dark,
}