use ratatui::{prelude::*, widgets::*, symbols::block::FULL, layout::Flex};
use unicode_width::UnicodeWidthStr;

use super::utility::*;

mod smart_scrollbar;
use smart_scrollbar::SmartScrollbar;

impl<'a> super::App<'a> {
    pub(in super) fn render(&mut self, frame: &mut Frame) {
        fn style_span(mut position: TextPosition, mut length: usize, style: Style, text: &Rect, window_position: TextPosition, frame: &mut Frame) {
            if position.line < window_position.line || position.line >= window_position.line + text.height as usize
                    || position.column + length <= window_position.column || position.column >= window_position.column + text.width as usize {
                return;
            }
            if position.column < window_position.column {
                length -= window_position.column - position.column;
                position.column = window_position.column;
            }
            if position.column + length > window_position.column + text.width as usize {
                length = window_position.column + text.width as usize - position.column;
            }
            frame.render_widget(
                Block::new().style(style),
                Rect::new(text.x + position.column.saturating_sub(window_position.column) as u16, 1 + (position.line - window_position.line) as u16, length as u16, 1)
            );
        }

        fn style_region(start: TextPosition, end: TextPosition, style: Style, text: &Rect, app: &super::App, frame: &mut Frame) {
            if start.line == end.line {
                style_span(start, end.column - start.column, style, text, app.window_position, frame);
                return;
            }
            style_span(start, app.text[start.line].width() - start.column + 1, style, text, app.window_position, frame);
            for line in start.line + 1 .. end.line {
                style_span(TextPosition::new(line, 0), app.text[line].width() + 1, style, text, app.window_position, frame);
            }
            style_span(TextPosition::new(end.line, 0), end.column, style, text, app.window_position, frame);
        }

        let [header, main, mut footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1)
        ]).areas(frame.area());
        let num_lines = self.text.len();
        let num_lines_str = num_lines.to_string();
        let line_nums_width = num_lines_str.len() as u16 + 1;
        let [line_nums, main, vertical_scrollbar] = Layout::horizontal([
            Constraint::Length(line_nums_width),
            Constraint::Fill(1),
            Constraint::Length(if self.text.len() > (main.height - (self.text_width as u16 >= main.width - line_nums_width) as u16) as usize { 1 } else { 0 })
        ]).areas(main);
        let horizontal_scrollbar_exists = self.text_width >= main.width as usize;
        let [text, mut horizontal_scrollbar] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(if horizontal_scrollbar_exists { 1 } else { 0 })
        ]).areas(main);
        let [vertical_scrollbar, scrollbars_corner] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(if horizontal_scrollbar_exists { 1 } else { 0 })
        ]).areas(vertical_scrollbar);

        self.window_area = text;
        self.vertical_scrollbar_area = vertical_scrollbar;
        self.horizontal_scrollbar_area = horizontal_scrollbar;

        if self.scroll_to_main_cursor {
            self.scroll_to_main_cursor();
            self.scroll_to_main_cursor = false;
        }
        self.fix_window_position();

        let theme = &self.config.theme;

        let frame_block = Block::new().style(Style::new().bg(theme.frame_bg).fg(theme.frame_fg));

        let main_cursor = &self.cursors[self.main_cursor_index];

        // header
        frame.render_widget(&frame_block, header);
        frame.render_widget(Line::from(if self.history.pos == self.saved { "Saved" } else { "Unsaved" }), header);
        frame.render_widget(Line::from(format!("{} ({num_lines_str} line{})", self.file_path, if num_lines == 1 { "" } else { "s" })).centered(), header);
        frame.render_widget(Line::from(format!("({}, {})", main_cursor.position.line + 1, main_cursor.position.column + 1)).right_aligned(), header);

        // footer
        frame.render_widget(&frame_block, footer);
        if !self.clipboards.is_empty() {
            let mut spans = Vec::with_capacity(1 + 2 * self.clipboards.iter().fold(0, |acc, clipboard| acc + clipboard.len()));
            spans.push(Span::raw(format!("Clipboard{}:", if self.clipboards.len() == 1 { "" } else { "s" })));
            for clipboard in &self.clipboards {
                spans.push(Span::raw(" "));
                spans.push(clipboard[0].clone().underlined());
                let newline = "↵".bold().underlined();
                for line in &clipboard[1 ..] {
                    spans.push(newline.clone());
                    spans.push(line.clone().underlined());
                }
            }
            let mut line = Line::from(spans);
            line = if line.width() <= self.window_area.width as usize {
                line.right_aligned()
            } else {
                frame.render_widget(Line::from("...").right_aligned(), footer);
                footer.width -= 3;
                line.left_aligned()
            };
            frame.render_widget(line, footer);
        }

        // scrollbars corner
        frame.render_widget(&frame_block, scrollbars_corner);

        let visible_lines = self.window_position.line .. (self.window_position.line + text.height as usize).min(self.text.len());

        // line numbers
        frame.render_widget(
            Paragraph::new(Text::from_iter(visible_lines.clone().map(|n| (n + 1).to_string())))
                .right_aligned()
                .block(Block::new().style(Style::new().bg(theme.line_nums_bg).fg(theme.line_nums_fg)).padding(Padding::right(1))),
            line_nums
        );

        let scrollbar_thumb_style = Style::new().bg(theme.frame_bg).fg(theme.scrollbar_thumb);
        let scrollbar_track_style = Style::new().bg(theme.frame_bg).fg(theme.scrollbar_track);
        let scrollbar_arrows_style = Style::new().bg(theme.frame_bg).fg(theme.scrollbar_arrows);

        // vertical scrollbar
        frame.render_widget(
            SmartScrollbar::default()
                .thumb_style(scrollbar_thumb_style)
                .track_symbol(Some(FULL))
                .track_style(scrollbar_track_style)
                .begin_style(scrollbar_arrows_style)
                .end_style(scrollbar_arrows_style)
                .state(self.text.len(), visible_lines.start, text.height as usize),
            vertical_scrollbar
        );

        // horizontal scrollbar
        frame.render_widget(
            SmartScrollbar::new(ScrollbarOrientation::HorizontalBottom, 1, 0, 0)
                .begin_style(scrollbar_arrows_style)
                .end_style(scrollbar_arrows_style),
            horizontal_scrollbar
        );
        horizontal_scrollbar.x += 1;
        horizontal_scrollbar.width = horizontal_scrollbar.width.saturating_sub(2);
        frame.render_widget(
            SmartScrollbar::new(ScrollbarOrientation::HorizontalBottom, self.text_width + 1, self.window_position.column, text.width as usize)
                .thumb_style(scrollbar_thumb_style)
                .track_symbol(Some(FULL))
                .track_style(scrollbar_track_style)
                .begin_symbol(Some(" "))
                .begin_style(scrollbar_arrows_style)
                .end_symbol(Some(" "))
                .end_style(scrollbar_arrows_style),
            horizontal_scrollbar
        );

        // text
        frame.render_widget(
            Paragraph::new(Text::from_iter(self.text[visible_lines.clone()].iter().map(|s| {
                let (valid_offset, byte_index, grapheme) = valid_offset_byte_index_and_grapheme_at(s, self.window_position.column);
                let line = if valid_offset == self.window_position.column { grapheme.to_string() } else { " ".repeat(self.window_position.column - valid_offset) };
                line + &s[byte_index + grapheme.len() ..]
            }))).block(Block::new().style(Style::new().bg(theme.text_bg).fg(theme.text_fg))),
            text
        );

        // cursors
        for (i, cursor) in self.cursors.iter().enumerate().rev() {
            let (mut start, mut end) = cursor.get_selected_region();
            if end.line < visible_lines.start || start.line >= visible_lines.end {
                continue;
            }

            if start == end {
                style_span(
                    start, 1,
                    Style::new().bg(if i == self.main_cursor_index { theme.main_cursor_bg } else { theme.other_cursor_bg }).fg(theme.cursor_fg),
                    &text, self.window_position, frame
                );
                continue;
            }

            if start.line < visible_lines.start {
                (start.line, start.column) = (visible_lines.start, 0);
            }
            if end.line >= visible_lines.end {
                end.line = visible_lines.end - 1;
                end.column = self.text[end.line].width() + 1;
            }

            style_region(
                start, end,
                Style::new().bg(if i == self.main_cursor_index { theme.main_selection_bg } else { theme.other_selection_bg }).fg(theme.selection_fg),
                &text, self, frame
            );
        }

        // prompt
        if let Some(prompt) = self.curr_prompt.as_mut() {
            let options = Line::from(String::from(" ") + &prompt.options.iter().map(|option| option.0.clone()).collect::<Vec<_>>().join(" █ ") + " ").style(Style::new().reversed());
            let width = options.width().max(prompt.description.width()) as u16 + 4;
            let paragraph = Paragraph::new(Text::from(vec![Line::from(prompt.description.clone()).centered(), Line::default(), options])).wrap(Wrap{trim: false});
            let prompt_area = Layout::vertical([Constraint::Length(paragraph.line_count(width) as u16 + 2)]).flex(Flex::Center).areas::<1>(
                Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center).areas::<1>(frame.area())[0])[0];
            prompt.options_position = Position::new(prompt_area.x + 2, prompt_area.bottom() - 2);
            frame.render_widget(Clear, prompt_area);
            const ACTUAL_BORDER: symbols::border::Set = symbols::border::Set{
                top_right: "🭾",
                top_left: "🭽",
                bottom_right: "🭿",
                bottom_left: "🭼",
                vertical_left: symbols::border::ONE_EIGHTH_LEFT_EIGHT,
                vertical_right: symbols::border::ONE_EIGHTH_RIGHT_EIGHT,
                horizontal_top: symbols::border::ONE_EIGHTH_TOP_EIGHT,
                horizontal_bottom: symbols::border::ONE_EIGHTH_BOTTOM_EIGHT
            };
            frame.render_widget(paragraph.block(frame_block.borders(Borders::ALL).border_set(ACTUAL_BORDER).padding(Padding::horizontal(1))), prompt_area);
        }
    }
}