use std::iter;

use ratatui::{prelude::*, symbols::scrollbar::*, widgets::ScrollbarOrientation};
use unicode_width::UnicodeWidthStr;

pub struct SmartScrollbar<'a> {
    orientation: ScrollbarOrientation,
    thumb_symbol: &'a str,
    thumb_style: Style,
    track_symbol: Option<&'a str>,
    track_style: Style,
    begin_symbol: Option<&'a str>,
    begin_style: Style,
    end_symbol: Option<&'a str>,
    end_style: Style,
    content_length: usize,
    window_position: usize,
    window_length: usize
}

impl<'a> SmartScrollbar<'a> {
    pub fn new(orientation: ScrollbarOrientation, content_length: usize, window_position: usize, window_length: usize) -> Self {
        let symbols = if orientation.is_vertical() { DOUBLE_VERTICAL } else { DOUBLE_HORIZONTAL };
        Self::with_symbols(orientation, symbols, content_length, window_position, window_length)
    }

    pub fn with_symbols(orientation: ScrollbarOrientation, symbols: Set<'a>, content_length: usize, window_position: usize, window_length: usize) -> Self {
        Self{
            orientation,
            thumb_symbol: symbols.thumb,
            thumb_style: Style::new(),
            track_symbol: Some(symbols.track),
            track_style: Style::new(),
            begin_symbol: Some(symbols.begin),
            begin_style: Style::new(),
            end_symbol: Some(symbols.end),
            end_style: Style::new(),
            content_length,
            window_position,
            window_length
        }
    }

    pub fn symbols(mut self, symbols: Set<'a>) -> Self {
        self.thumb_symbol = symbols.thumb;
        self.track_symbol = Some(symbols.track);
        self.begin_symbol = Some(symbols.begin);
        self.end_symbol = Some(symbols.end);
        self
    }

    pub fn thumb_symbol(mut self, symbol: &'a str) -> Self {
        self.thumb_symbol = symbol;
        self
    }

    pub fn thumb_style(mut self, style: impl Into<Style>) -> Self {
        self.thumb_style = style.into();
        self
    }

    pub fn track_symbol(mut self, symbol: Option<&'a str>) -> Self {
        self.track_symbol = symbol;
        self
    }

    pub fn track_style(mut self, style: impl Into<Style>) -> Self {
        self.track_style = style.into();
        self
    }

    pub fn begin_symbol(mut self, symbol: Option<&'a str>) -> Self {
        self.begin_symbol = symbol;
        self
    }

    pub fn begin_style(mut self, style: impl Into<Style>) -> Self {
        self.begin_style = style.into();
        self
    }

    pub fn end_symbol(mut self, symbol: Option<&'a str>) -> Self {
        self.end_symbol = symbol;
        self
    }

    pub fn end_style(mut self, style: impl Into<Style>) -> Self {
        self.end_style = style.into();
        self
    }

    pub fn state(mut self, content_length: usize, window_position: usize, window_length: usize) -> Self {
        self.content_length = content_length;
        self.window_position = window_position;
        self.window_length = window_length;
        self
    }

    fn render_from_iter(chars: impl Iterator<Item = (Option<&'a str>, Style)>, area: Rect, buf: &mut Buffer) {
        let mut chars = chars.peekable();
        let skip_first = chars.peek().unwrap().0.is_none();
        let chars = chars.skip(skip_first as usize);
        for (position, c) in area.positions().zip(chars) {
            if let (Some(symbol), style) = c {
                buf.set_string(position.x, position.y, symbol, style);
            }
        }
    }
}

impl Default for SmartScrollbar<'_> {
    fn default() -> Self {
        Self::new(ScrollbarOrientation::default(), 0, 0, 0)
    }
}

impl Widget for SmartScrollbar<'_> {
    fn render(mut self, mut area: Rect, buf: &mut Buffer) {
        // ensure the window doesn't go beyond the end of the content
        self.window_length = self.window_length.min(self.content_length - self.window_position);

        if area.area() == 0
                || self.thumb_symbol.width() != 1
                || self.track_symbol.is_some_and(|s| s.width() != 1)
                || self.begin_symbol.is_some_and(|s| s.width() != 1)
                || self.end_symbol.is_some_and(|s| s.width() != 1) {
            return;
        }

        match self.orientation {
            ScrollbarOrientation::VerticalLeft => area.width = 1,
            ScrollbarOrientation::VerticalRight => {
                area.x += area.width - 1;
                area.width = 1;
            },
            ScrollbarOrientation::HorizontalTop => area.height = 1,
            ScrollbarOrientation::HorizontalBottom => {
                area.y += area.height - 1;
                area.height = 1;
            }
        }

        let track_bg = (self.track_symbol.map(|_| " "), self.track_style);

        let scrollbar_length = area.area();

        if self.content_length <= self.window_length {
            Self::render_from_iter(iter::repeat_n(track_bg, scrollbar_length as usize), area, buf);
            return;
        }

        let ends_length = self.begin_symbol.is_some() as u32 + self.end_symbol.is_some() as u32;
        if scrollbar_length < ends_length {
            Self::render_from_iter(iter::once(track_bg), area, buf);
            return;
        }
        let track_length = scrollbar_length.saturating_sub(ends_length) as usize;
        if track_length < 3 {
            Self::render_from_iter(
                iter::once((self.begin_symbol, self.begin_style))
                    .chain(iter::repeat_n(track_bg, track_length))
                    .chain(iter::once((self.end_symbol, self.end_style))),
                area, buf);
            return;
        }

        let mut thumb_length = track_length * self.window_length / self.content_length;
        // `thumb_length` < `track_length`, so the scrollbar always shows there's content outside the window

        // ensure the scrollbar can always show in which directions there's content beyond the window
        let outside_content_length = self.content_length - self.window_length;
        if outside_content_length >= 2 {
            thumb_length = thumb_length.min(track_length - 2);
        }

        // ensure the thumb is always visible
        if thumb_length == 0 {
            thumb_length = 1;
        }

        let mut thumb_position = track_length * self.window_position / self.content_length;

        // ensure the scrollbar always shows whether there's content beyond the window in the negative direction
        if self.window_position > 0 {
            thumb_position = thumb_position.max(1);
        }

        // ensure the scrollbar always shows whether there's content beyond the window in the positive direction
        if self.window_position == outside_content_length {
            thumb_position = track_length - thumb_length;
        } else {
            thumb_position = thumb_position.min(track_length - thumb_length - 1);
        }

        // draw the scrollbar
        let track = (self.track_symbol, self.track_style);
        Self::render_from_iter(
            iter::once((self.begin_symbol, self.begin_style))
                .chain(iter::repeat_n(track, thumb_position))
                .chain(iter::repeat_n((Some(self.thumb_symbol), self.thumb_style), thumb_length))
                .chain(iter::repeat_n(track, track_length - thumb_position - thumb_length))
                .chain(iter::once((self.end_symbol, self.end_style))),
            area, buf
        );
    }
}