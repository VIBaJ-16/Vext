use std::{
    fs::File,
    io::{self, stdout, BufReader, BufRead, Write}
};

use ratatui::{prelude::*, DefaultTerminal};
use crossterm::{execute, event::*};
use unicode_width::UnicodeWidthStr;

use crate::config::*;

mod utility;
mod history;
mod cursor;
mod render;
mod events;

use self::{utility::*, history::History, cursor::Cursor};

pub struct App<'a> {
    exit: bool,
    config: Config,
    file_path: &'a str,
    saved: usize, // saved position in history
    text: Vec<String>,
    text_width: usize,
    window_position: TextPosition,
    window_area: Rect,
    vertical_scrollbar_area: Rect,
    horizontal_scrollbar_area: Rect,
    cursors: Vec<Cursor>,
    main_cursor_index: usize,
    scroll_to_main_cursor: bool,
    curr_prompt: Option<Prompt>,
    clipboards: Vec<Vec<String>>,
    history: History
}

struct Prompt {
    description: String,
    options: Vec<(String, fn(&mut App))>,
    options_position: Position,
    handle_key_event: fn(&mut App, Vec<fn(&mut App)>, &KeyEvent)
}

impl<'a> App<'a> {
    pub fn new(file_path: &'a str) -> io::Result<Self> {
        let text = if let Ok(file) = File::open(file_path) {
            let mut file = BufReader::new(file);
            let mut result = Vec::new();
            let mut line = String::new();
            while file.read_line(&mut line)? != 0 && line.ends_with('\n') {
                line.pop();
                result.push(line);
                line = String::new();
            }
            result.push(line);
            result
        } else {
            vec![String::new()]
        };
        let mut result = Self{
            exit: false,
            config: Config::default(),
            file_path,
            saved: 0,
            text,
            text_width: 0,
            window_position: TextPosition::new(0, 0),
            window_area: Default::default(),
            vertical_scrollbar_area: Default::default(),
            horizontal_scrollbar_area: Default::default(),
            cursors: vec![Cursor::new(TextPosition::new(0, 0))],
            main_cursor_index: 0,
            scroll_to_main_cursor: false,
            curr_prompt: None,
            clipboards: Vec::new(),
            history: History::new()
        };
        result.update_text_width();
        Ok(result)
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        execute!(
             stdout(),
             EnableBracketedPaste,
             EnableMouseCapture
        )?;
        while !self.exit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        execute!(
             stdout(),
             DisableBracketedPaste,
             DisableMouseCapture
        )?;
        Ok(())
    }

    fn exit(&mut self, force: bool) {
        if self.history.pos == self.saved || force {
            self.exit = true;
            return;
        }
        self.prompt(String::from("You have unsaved changes"), vec![
            (String::from("Quit without saving (Ctrl Q)"), |app| app.exit(true)),
            (String::from("Save and quit (Ctrl S)"), |app| { app.save(); app.exit(false); } ),
            (String::from("Cancel (Esc)"), |app| app.curr_prompt = None)
        ], |app, option_effects, key_event| match key_event.code {
            KeyCode::Char(char) if key_event.modifiers == KeyModifiers::CONTROL => match char {
                'q' => option_effects[0](app),
                's' => option_effects[1](app),
                _ => ()
            },
            KeyCode::Esc => option_effects[2](app),
            _ => ()
        });
    }

    fn save(&mut self) {
        let _ = File::create(self.file_path).unwrap().write(self.text.join("\n").as_bytes()).unwrap();
        self.saved = self.history.pos;
    }

    fn prompt(&mut self, description: String, options: Vec<(String, fn(&mut App))>, handle_key_event: fn(&mut App, Vec<fn(&mut App)>, &KeyEvent)) {
        self.curr_prompt = Some(Prompt{description, options, options_position: Position::default(), handle_key_event});
    }

    fn update_text_width(&mut self) {
        self.text_width = self.text.iter().max_by_key(|line| line.width()).unwrap().width().max(1);
    }

    // fix being scrolled too far
    fn fix_window_position(&mut self) {
        let max_line = self.text.len().saturating_sub(self.window_area.height as usize);
        if self.window_position.line > max_line {
            self.window_position.line = max_line;
        }
        let max_column = (self.text_width + 1).saturating_sub(self.window_area.width as usize);
        if self.window_position.column > max_column {
            self.window_position.column = max_column;
        }
    }

    fn scroll_to_main_cursor(&mut self) {
        let (window_line, window_column) = (self.window_position.line, self.window_position.column);
        let (window_height, window_width) = (self.window_area.height as usize, self.window_area.width as usize);
        let (text_len, text_width) = (self.text.len(), self.text_width);
        let cursor = &self.cursors[self.main_cursor_index];
        let (cursor_line, cursor_column) = (cursor.position.line, cursor.position.column);
        let (vertical_margin, horizontal_margin) = (self.config.vertical_scroll_margin, self.config.horizontal_scroll_margin);

        if text_len > window_height {
            let max_line = cursor_line.saturating_sub(vertical_margin as usize);
            if window_line > max_line {
                self.window_position.line = max_line;
            } else {
                let min_line = (cursor_line + vertical_margin as usize).min(text_len - 1).saturating_sub(window_height - 1);
                if window_line < min_line {
                    self.window_position.line = min_line;
                }
            }
        }

        if text_width >= window_width {
            let max_column = cursor_column.saturating_sub(horizontal_margin as usize);
            if window_column > max_column {
                self.window_position.column = max_column;
            } else {
                let min_column = (cursor_column + horizontal_margin as usize).min(text_width).saturating_sub(window_width - 1);
                if window_column < min_column {
                    self.window_position.column = min_column;
                }
            }
        }
    }
}