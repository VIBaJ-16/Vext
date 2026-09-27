use std::io::{self, stdout};

use ratatui::prelude::Position;
use crossterm::{execute, event::*, clipboard::CopyToClipboard};
use unicode_width::UnicodeWidthStr;

use super::{utility::*, history::{ActionType, Change}, cursor::Cursor};

impl<'a> super::App<'a> {
    pub(in super) fn handle_events(&mut self) -> io::Result<()> {
        let event = read()?;

        if let Some(prompt) = self.curr_prompt.as_ref() {
            match event {
                Event::Key(key_event) if !key_event.is_release() => (prompt.handle_key_event)(self, prompt.options.iter().map(|option| option.1).collect(), &key_event),
                Event::Mouse(mouse_event) => match mouse_event.kind {
                    MouseEventKind::Down(MouseButton::Left) if mouse_event.row == prompt.options_position.y => {
                        let mut option_column = prompt.options_position.x;
                        for option in prompt.options.iter() {
                            if mouse_event.column < option_column {
                                break;
                            }
                            let option_end_column = option_column + option.0.width() as u16 + 2;
                            if mouse_event.column < option_end_column {
                                option.1(self);
                                break;
                            }
                            option_column = option_end_column + 1;
                        }
                    },
                    _ => ()
                },
                Event::Resize(_, height) => self.handle_resize(height),
                _ => ()
            }
            return Ok(());
        }

        match event {
            Event::Key(key_event) if !key_event.is_release() => self.handle_key_event(&key_event),
            Event::Paste(paste_text) => if !paste_text.is_empty() { self.cursors_do_insertion(paste_text.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(|s| s.to_string()).collect()) },
            Event::Mouse(mouse_event) => self.handle_mouse_event(&mouse_event),
            Event::Resize(_, height) => self.handle_resize(height),
            _ => ()
        }

        // merge overlapping cursors
        for i in (1 .. self.cursors.len()).rev() {
            let (left, right) = self.cursors.split_at_mut(i);
            if left[i - 1].merge(&right[0]) {
                self.cursors.remove(i);
                if i <= self.main_cursor_index {
                    self.main_cursor_index -= 1;
                }
            }
        }

        Ok(())
    }

    fn cursors_do_action(&mut self, action_type: ActionType, mut action: impl FnMut(&Cursor, &mut Vec<Change>, &[String])) {
        let mut changes = Vec::with_capacity(self.cursors.len());
        self.cursors.iter().for_each(|cursor| action(cursor, &mut changes, &self.text));
        changes.retain(|change| !change.is_empty());
        if changes.is_empty() {
            return;
        }
        changes.shrink_to_fit();
        self.do_action(action_type, changes);
    }

    fn cursors_do_insertion(&mut self, inserted_text: Vec<String>) {
        self.cursors_do_action(ActionType::Normal, |cursor, changes, text| {
            let (start, end) = cursor.get_selected_region();
            changes.push(Change::new(text, start, end, inserted_text.clone()));
        });
    }

    // Surround selections with `start_char` and `end_char`.
    // If `auto_close`, surround non-selecting cursors too, otherwise just insert `start_char` for those.
    // () [] {} "" '' ``
    // <>
    fn surround(&mut self, start_char: char, end_char: char, auto_close: bool) {
        let config_auto_close = self.config.auto_close;
        let surround = self.config.surround;
        self.cursors_do_action(ActionType::Surround, |cursor, changes, text| {
            let (start, end) = cursor.get_selected_region();
            changes.push(Change::new(text, start, if surround { start } else { end }, vec![start_char.to_string()]));
            if start == end || !surround && config_auto_close {
                if config_auto_close && auto_close && matches!(grapheme_at(&text[end.line], end.column), "" | " " | ")" | "]" | "}") {
                    changes.last_mut().unwrap().inserted_text[0].push(end_char);
                }
            } else if surround {
                changes.push(Change::new(text, end, end, vec![end_char.to_string()]));
            }
        });
    }

    fn copy(&mut self) {
        self.clipboards = Vec::with_capacity(self.cursors.len());
        for cursor in &self.cursors {
            if cursor.position == cursor.selection_anchor {
                continue;
            }
            let (start, end) = cursor.get_selected_region();
            self.clipboards.push(start.get_text_to(&self.text, end));
        }
        self.clipboards.shrink_to_fit();
    }

    fn indentation(s: &str) -> usize {
        s.find(|c: char| !c.is_ascii_whitespace()).unwrap_or(s.len())
    }

    fn handle_key_event(&mut self, key_event: &KeyEvent) {
        let shift_pressed = key_event.modifiers.contains(KeyModifiers::SHIFT);
        let control_pressed = key_event.modifiers.contains(KeyModifiers::CONTROL);
        let alt_pressed = key_event.modifiers.contains(KeyModifiers::ALT);
        match key_event.code {
            KeyCode::Char(char) => match key_event.modifiers {
                KeyModifiers::NONE | KeyModifiers::SHIFT => match char {
                    '(' => self.surround('(', ')', true),
                    '[' => self.surround('[', ']', true),
                    '{' => self.surround('{', '}', true),
                    '"' | '\'' | '`' => self.surround(char, char, true),
                    '<' => self.surround('<', '>', false),
                    '/' if self.config.surround => self.cursors_do_action(ActionType::Surround, |cursor, changes, text| {
                        if cursor.position == cursor.selection_anchor {
                            changes.push(Change::new(text, cursor.position, cursor.position, vec!["/".to_string()]));
                            return;
                        }
                        // block comment
                        let (start, end) = cursor.get_selected_region();
                        changes.push(Change::new(text, start, start, vec![if start.column == text[start.line].width() { "/*" } else { "/* " }.to_string()]));
                        changes.push(Change::new(text, end, end, vec![if end.column == 0 { "*/" } else { " */" }.to_string()]));
                    }),
                    _ => self.cursors_do_insertion(vec![char.to_string()])
                },
                KeyModifiers::CONTROL => match char {
                    'h' => self.cursors_do_action(ActionType::Normal, |cursor, changes, text| { // Ctrl + Backspace
                        let (mut start, end) = cursor.get_selected_region();
                        if start == end {
                            let mut temp = cursor.clone();
                            temp.move_left_word(text);
                            start = temp.position;
                        }
                        changes.push(Change::new(text, start, end, vec![String::new()]));
                    }),
                    'q' => self.exit(false),
                    's' if key_event.is_press() => self.save(),
                    'z' => self.undo(),
                    'y' => self.redo(),
                    'a' => { // select all
                        let mut cursor = Cursor::new(TextPosition::new(self.text.len() - 1, self.text.last().unwrap().width()));
                        (cursor.selection_anchor.line, cursor.selection_anchor.column) = (0, 0);
                        self.cursors = vec![cursor];
                        self.main_cursor_index = 0;
                        self.scroll_to_main_cursor();
                    },
                    'c' => self.copy(),
                    'x' => { // cut
                        self.copy();
                        self.cursors_do_action(ActionType::Normal, |cursor, changes, text| {
                            if cursor.position != cursor.selection_anchor {
                                let (start, end) = cursor.get_selected_region();
                                changes.push(Change::new(text, start, end, vec![String::new()]));
                            }
                        });
                    },
                    'v' => if !self.clipboards.is_empty() { // paste
                        if self.cursors.len() == self.clipboards.len() {
                            let mut i = 0;
                            unsafe { (&raw mut *self).as_mut().unwrap() }.cursors_do_action(ActionType::Normal, |cursor, changes, text| {
                                let (start, end) = cursor.get_selected_region();
                                changes.push(Change::new(text, start, end, self.clipboards[i].clone()));
                                i += 1;
                            });
                        } else {
                            self.cursors_do_insertion(self.clipboards.concat());
                        }
                    },
                    '7' => {
                        let mut lines = Vec::new();
                        let last_cursor = self.cursors.last().unwrap().clone();
                        self.cursors_do_action(ActionType::Normal, |cursor, changes, text| {
                            let (start, end) = cursor.get_selected_region();
                            if lines.last().is_none_or(|&line| line != start.line) {
                                let mut line = start.line ..;
                                lines.resize_with(lines.len() + end.line - start.line + (start == end || end.column != 0) as usize, || line.next().unwrap());
                            }
                            if cursor != &last_cursor {
                                return;
                            }
                            if lines.iter().any(|&line| !text[line].trim_ascii_start().starts_with("//")) { // comment
                                let min_indentation = lines.iter().map(|&line| Self::indentation(&text[line])).min().unwrap();
                                for &line in &lines {
                                    let position = TextPosition::new(line, min_indentation);
                                    changes.push(Change::new(text, position, position, vec!["// ".to_string()]));
                                }
                            } else { // uncomment
                                for &line in &lines {
                                    let start = TextPosition::new(line, text[line].find('/').unwrap());
                                    let mut end = start;
                                    end.column += 2;
                                    if text[line].as_bytes().get(end.column).is_some_and(|&c| c == b' ') {
                                        end.column += 1;
                                    }
                                    changes.push(Change::new(text, start, end, vec![String::new()]));
                                }
                            }
                        });
                    },
                    'f' => todo!("search"),
                    _ => ()
                },
                KeyModifiers::ALT => match char {
                    'z' => self.history_prev_pos(),
                    'y' => self.history_next_pos(),
                    // copy to system clipboard
                    'c' => execute!(stdout(), CopyToClipboard::to_clipboard_from(&self.cursors.iter().filter_map(|cursor| if cursor.position == cursor.selection_anchor { None } else {
                        let (start, end) = cursor.get_selected_region();
                        Some(start.get_text_to(&self.text, end).join("\n"))
                    }).collect::<Vec<_>>().join("\n"))).unwrap(),
                    _ => ()
                },
                _ => ()
            },
            KeyCode::Backspace => {
                let backspace_pair = self.config.backspace_pair;
                self.cursors_do_action(ActionType::Normal, |cursor, changes, text| {
                    let (mut start, mut end) = cursor.get_selected_region();
                    if start == end {
                        let line = &text[start.line];
                        let i = byte_index(line, start.column);
                        if backspace_pair && i > 0 && i < line.len() && matches!(line.get(i - 1 ..= i), Some("()" | "[]" | "{}" | "\"\"" | "''" | "``" | "<>")) {
                            start.column -= 1;
                            end.column += 1;
                        } else {
                            let mut temp = cursor.clone();
                            temp.move_left(text);
                            start = temp.position;
                        }
                    }
                    changes.push(Change::new(text, start, end, vec![String::new()]));
                });
            },
            KeyCode::Enter => self.cursors_do_action(ActionType::Normal, |cursor, changes, text| {
                let (start, end) = cursor.get_selected_region();
                changes.push(Change::new(text, start, end, vec![String::new(); 2]));
            }),
            KeyCode::Left => {
                if !alt_pressed {
                    if control_pressed {
                        for cursor in &mut self.cursors {
                            cursor.move_left_word(&self.text);
                        }
                    } else {
                        for cursor in &mut self.cursors {
                            if cursor.position == cursor.selection_anchor || shift_pressed {
                                cursor.move_left(&self.text);
                            }
                        }
                    }
                    if !shift_pressed {
                        for cursor in self.cursors.iter_mut() {
                            if cursor.position <= cursor.selection_anchor {
                                cursor.selection_anchor = cursor.position;
                            } else {
                                cursor.position = cursor.selection_anchor;
                            }
                        }
                    }
                }
                self.scroll_to_main_cursor();
            },
            KeyCode::Right => {
                if !alt_pressed {
                    if control_pressed {
                        for cursor in &mut self.cursors {
                            cursor.move_right_word(&self.text);
                        }
                    } else {
                        for cursor in &mut self.cursors {
                            if cursor.position == cursor.selection_anchor || shift_pressed {
                                cursor.move_right(&self.text);
                            }
                        }
                    }
                    if !shift_pressed {
                        for cursor in self.cursors.iter_mut() {
                            if cursor.position <= cursor.selection_anchor {
                                cursor.position = cursor.selection_anchor;
                            } else {
                                cursor.selection_anchor = cursor.position;
                            }
                        }
                    }
                }
                self.scroll_to_main_cursor();
            },
            KeyCode::Up => {
                if !alt_pressed {
                    if control_pressed {
                        todo!("prev paragraph");
                    } else {
                        for cursor in self.cursors.iter_mut() {
                            cursor.move_up(&self.text);
                        }
                    }
                    if !shift_pressed {
                        for cursor in self.cursors.iter_mut() {
                            cursor.selection_anchor = cursor.position;
                        }
                    }
                }
                self.scroll_to_main_cursor();
            },
            KeyCode::Down => {
                if !alt_pressed {
                    if control_pressed {
                        todo!("next paragraph");
                    } else {
                        for cursor in self.cursors.iter_mut() {
                            cursor.move_down(&self.text);
                        }
                    }
                    if !shift_pressed {
                        for cursor in self.cursors.iter_mut() {
                            cursor.selection_anchor = cursor.position;
                        }
                    }
                }
                self.scroll_to_main_cursor();
            },
            KeyCode::Tab => todo!("Tab"),
            KeyCode::BackTab => todo!("Shift + Tab"), // Shift + Tab
            KeyCode::Esc if key_event.is_press() => {
                self.cursors = vec![self.cursors[self.main_cursor_index].clone()];
                self.main_cursor_index = 0;
                self.scroll_to_main_cursor();
            },
            _ => ()
        }
    }

    fn handle_mouse_event(&mut self, mouse_event: &MouseEvent) {
        match mouse_event.kind {
            MouseEventKind::Down(mouse_button) => if mouse_button == MouseButton::Left {
                let position = Position::new(mouse_event.column, mouse_event.row);
                if self.window_area.contains(position) {
                    let mut text_position = TextPosition::from(&position, &self.window_position, &self.window_area);
                    text_position.fix(&self.text);
                    match mouse_event.modifiers {
                        KeyModifiers::NONE => {
                            self.cursors = vec![Cursor::new(text_position)];
                            self.main_cursor_index = 0;
                            self.scroll_to_main_cursor();
                        },
                        KeyModifiers::CONTROL => {
                            let cursor = Cursor::new(text_position);
                            match self.cursors.binary_search(&cursor) {
                                Err(i) => {
                                    self.cursors.insert(i, cursor);
                                    if i <= self.main_cursor_index {
                                        self.main_cursor_index += 1;
                                    }
                                },
                                Ok(i) if i != self.main_cursor_index => {
                                    self.cursors.remove(i);
                                    if i < self.main_cursor_index {
                                        self.main_cursor_index -= 1;
                                    }
                                },
                                _ => ()
                            }
                        },
                        KeyModifiers::ALT => todo!("Alt + left click"),
                        _ => ()
                    }
                } else if self.vertical_scrollbar_area.contains(position) {
                    if !mouse_event.modifiers.is_empty() || self.vertical_scrollbar_area.height < 2 {
                        return;
                    }
                    let y = position.y - self.vertical_scrollbar_area.y;
                    if y == 0 {
                        if self.window_position.line > 0 {
                            self.window_position.line -= 1;
                        }
                        return;
                    }
                    if y == self.vertical_scrollbar_area.height - 1 {
                        self.window_position.line += 1;
                        return;
                    }
                } else if self.horizontal_scrollbar_area.contains(position) {
                    if !mouse_event.modifiers.is_empty() || self.horizontal_scrollbar_area.width < 2 {
                        return;
                    }
                    let x = position.x - self.horizontal_scrollbar_area.x;
                    if x == 0 {
                        if self.window_position.column > 0 {
                            self.window_position.column -= 1;
                        }
                        return;
                    }
                    if x == self.horizontal_scrollbar_area.width - 1 {
                        self.window_position.column += 1;
                        return;
                    }
                    if self.horizontal_scrollbar_area.width == 2 || x == 1 || x == self.horizontal_scrollbar_area.width - 2 {
                        return;
                    }
                }
            },
            MouseEventKind::ScrollDown => self.window_position.line += 1,
            MouseEventKind::ScrollUp => if self.window_position.line > 0 {
                self.window_position.line -= 1;
            },
            MouseEventKind::ScrollLeft => if self.window_position.column > 0 {
                self.window_position.column -= 1;
            },
            MouseEventKind::ScrollRight => self.window_position.column += 1,
            _ => ()
        }
    }

    fn handle_resize(&mut self, height: u16) {
        self.window_position.line = self.window_position.line.saturating_sub((height as usize).saturating_sub(2).saturating_sub(self.text.len() - self.window_position.line));
    }
}