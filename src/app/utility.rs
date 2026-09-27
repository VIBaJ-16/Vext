use ratatui::{prelude::*, layout::Offset};
use unicode_width::UnicodeWidthStr;
use unicode_segmentation::UnicodeSegmentation;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct TextPosition {
    pub line: usize,
    pub column: usize
}

impl TextPosition {
    pub fn new(line: usize, column: usize) -> Self {
        Self{line, column}
    }

    // creates a `TextPosition` from its corresponding position on the terminal
    pub fn from(position: &Position, window_position: &TextPosition, window_area: &Rect) -> Self {
        let position = position
            .offset(-Offset::new(window_area.x as i32, window_area.y as i32))
            .offset(Offset::new(window_position.column as i32, window_position.line as i32));
        Self{line: position.y as usize, column: position.x as usize}
    }

    // makes the `TextPosition` valid; this is, for example, used to put the cursor at a valid position when it's moved by clicking the mouse
    pub fn fix(&mut self, text: &[String]) {
        if self.line >= text.len() {
            self.line = text.len() - 1;
            self.column = text[self.line].width();
            return;
        }
        let line = &text[self.line];
        let line_length = line.width();
        if self.column > line_length {
            self.column = line_length;
            return;
        }
        self.column = valid_offset(line, self.column);
    }

    // returns the text between this `TextPosition` and `end`
    pub fn get_text_to(&self, text: &[String], end: Self) -> Vec<String> {
        let mut result = text[self.line ..= end.line].to_vec();
        let last_line = result.last_mut().unwrap();
        last_line.truncate(byte_index(last_line, end.column));
        let first_line = &mut result[0];
        first_line.replace_range(.. byte_index(first_line, self.column), "");
        result
    }

    // returns where this `TextPosition` would be pushed to if `text` was inserted here
    pub fn add_text(&self, text: &[String]) -> Self {
        TextPosition::new(self.line + text.len() - 1, if text.len() == 1 { self.column + text[0].width() } else { text.last().unwrap().width() })
    }
}

pub fn text_is_empty(text: &[String]) -> bool {
    text.len() == 1 && text[0].is_empty()
}

// Takes an offset (in terms of actual displayed distance) and returns the nearest (to the left) offset aligned to a grapheme and its corresponding byte index and grapheme.
pub fn valid_offset_byte_index_and_grapheme_at(string: &str, mut offset: usize) -> (usize, usize, &str) {
    let input_offset = offset;
    for (i, grapheme) in string.grapheme_indices(true) {
        let grapheme_width = grapheme.width();
        if grapheme_width > offset {
            return (input_offset - offset, i, grapheme);
        }
        offset -= grapheme_width;
    }
    (string.width(), string.len(), "")
}

pub fn valid_offset(string: &str, offset: usize) -> usize {
    valid_offset_byte_index_and_grapheme_at(string, offset).0
}

pub fn byte_index(string: &str, offset: usize) -> usize {
    valid_offset_byte_index_and_grapheme_at(string, offset).1
}

pub fn grapheme_at(string: &str, offset: usize) -> &str {
    valid_offset_byte_index_and_grapheme_at(string, offset).2
}

// Returns the corresponding displayed offset of `byte_index` in `string`.
pub fn byte_to_offset(string: &str, byte_index: usize) -> usize {
    string[.. byte_index].width()
}

pub fn prev_and_next_word_bounds(string: &str, offset: usize) -> (usize, usize) {
    let i = byte_index(string, offset);
    let mut prev = 0;
    for (start, word) in string.unicode_word_indices() {
        let end = start + word.len();
        if end <= i {
            prev = start;
            continue;
        }
        if start < i {
            prev = start;
        }
        return (byte_to_offset(string, prev), byte_to_offset(string, end));
    }
    (byte_to_offset(string, prev), string.width())
}