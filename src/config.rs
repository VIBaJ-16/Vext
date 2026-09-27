use ratatui::prelude::Color;

pub struct Config {
    pub theme: Theme,
    pub tab_size: std::num::NonZero<u8>,
    pub tabs_to_spaces: bool,
    pub auto_close: bool,
    pub surround: bool,
    pub backspace_pair: bool,
    pub vertical_scroll_margin: u8,
    pub horizontal_scroll_margin: u8
}

impl Default for Config {
    fn default() -> Self {
        Self{
            theme: Theme::default(),
            tab_size: unsafe { std::num::NonZero::new_unchecked(4) },
            tabs_to_spaces: true,
            auto_close: true,
            surround: true,
            backspace_pair: true,
            vertical_scroll_margin: 3,
            horizontal_scroll_margin: 4
        }
    }
}

pub struct Theme {
    pub frame_bg: Color,
    pub frame_fg: Color,
    pub text_bg: Color,
    pub text_fg: Color,
    pub line_nums_bg: Color,
    pub line_nums_fg: Color,
    pub scrollbar_thumb: Color,
    pub scrollbar_track: Color,
    pub scrollbar_arrows: Color,
    pub main_cursor_bg: Color,
    pub other_cursor_bg: Color,
    pub cursor_fg: Color,
    pub main_selection_bg: Color,
    pub other_selection_bg: Color,
    pub selection_fg: Color
}

impl Default for Theme {
    fn default() -> Self {
        Self{
            frame_bg:           Color::from_u32(0x303030),
            frame_fg:           Color::from_u32(0xC0C0C0),
            text_bg:            Color::from_u32(0x000000),
            text_fg:            Color::from_u32(0xFFFFFF),
            line_nums_bg:       Color::from_u32(0x181818),
            line_nums_fg:       Color::from_u32(0x808080),
            scrollbar_thumb:    Color::from_u32(0xC0C0C0),
            scrollbar_track:    Color::from_u32(0x181818),
            scrollbar_arrows:   Color::from_u32(0xC0C0C0),
            main_cursor_bg:     Color::from_u32(0xFFFFFF),
            other_cursor_bg:    Color::from_u32(0xC0C0C0),
            cursor_fg:          Color::from_u32(0x000000),
            main_selection_bg:  Color::from_u32(0xB0B0FF),
            other_selection_bg: Color::from_u32(0x8080FF),
            selection_fg:       Color::from_u32(0x000000)
        }
    }
}