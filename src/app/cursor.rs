use std::cmp::Ordering;
use unicode_width::UnicodeWidthStr;

use super::utility::*;

#[derive(Clone)]
pub struct Cursor {
    // If `position != selection_anchor`, this cursor is selecting the text between `position` and
    // `selection_anchor`.
    pub position: TextPosition,
    pub selection_anchor: TextPosition,
    // The column is saved when moving vertically, so if the cursor is forced to move to the left
    // due to moving vertically to a line that's too short, it can return to this column when
    // it moves vertically back to a long enough line.
    saved_column: usize
}

impl Ord for Cursor {
    fn cmp(&self, other: &Self) -> Ordering {
        if (self.position == self.selection_anchor) == (other.position == other.selection_anchor) {
            return self.position.cmp(&other.position);
        }
        let ((self_start, self_end), (other_start, other_end)) = (self.get_selected_region(), other.get_selected_region());
        if self_end <= other_start {
            Ordering::Less
        } else if self_start >= other_end {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Cursor {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Cursor {}

impl Cursor {
    pub fn new(position: TextPosition) -> Self {
        Self{position, selection_anchor: position, saved_column: position.column}
    }

    // Returns `(start, end)` of the selected text.
    pub fn get_selected_region(&self) -> (TextPosition, TextPosition) {
        if self.position <= self.selection_anchor {
            (self.position, self.selection_anchor)
        } else {
            (self.selection_anchor, self.position)
        }
    }

    pub fn reset_saved_column(&mut self) {
        self.saved_column = self.position.column;
    }

    pub fn move_left(&mut self, text: &[String]) {
        if self.position.column > 0 {
            self.position.column -= grapheme_at(&text[self.position.line], self.position.column - 1).width();
        } else if self.position.line > 0 {
            self.position.line -= 1;
            self.position.column = text[self.position.line].width();
        }
        self.reset_saved_column();
    }

    pub fn move_right(&mut self, text: &[String]) {
        let line = &text[self.position.line];
        if self.position.column < line.width() {
            self.position.column += grapheme_at(line, self.position.column).width();
        } else if self.position.line < text.len() - 1 {
            self.position.line += 1;
            self.position.column = 0;
        }
        self.reset_saved_column();
    }

    pub fn move_left_word(&mut self, text: &[String]) {
        'movement: {
            if self.position.column == 0 {
                if self.position.line == 0 {
                    break 'movement;
                }
                self.position.line -= 1;
                self.position.column = text[self.position.line].width();
            }
            self.position.column = prev_and_next_word_bounds(&text[self.position.line], self.position.column).0;
        }
        self.reset_saved_column();
    }

    pub fn move_right_word(&mut self, text: &[String]) {
        'movement: {
            if self.position.column == text[self.position.line].width() {
                if self.position.line == text.len() - 1 {
                    break 'movement;
                }
                self.position.line += 1;
                self.position.column = 0;
            }
            self.position.column = prev_and_next_word_bounds(&text[self.position.line], self.position.column).1;
        }
        self.reset_saved_column();
    }

    pub fn move_up(&mut self, text: &[String]) {
        if self.position.line == 0 {
            self.position.column = 0;
            self.reset_saved_column();
            return;
        }
        self.position.line -= 1;
        self.position.column = self.saved_column;
        self.position.fix(text);
    }

    pub fn move_down(&mut self, text: &[String]) {
        if self.position.line == text.len() - 1 {
            self.position.column = text[self.position.line].width();
            self.reset_saved_column();
            return;
        }
        self.position.line += 1;
        self.position.column = self.saved_column;
        self.position.fix(text);
    }

    // If `self` and `other` overlap, merges `other` into `self` and returns `true`, otherwise returns `false`.
    pub fn merge(&mut self, other: &Self) -> bool {
        if self.position == self.selection_anchor && other.position == other.selection_anchor {
            return self.position == other.position;
        }
        let ((mut self_start, mut self_end), (other_start, other_end)) = (self.get_selected_region(), other.get_selected_region());
        if self_end <= other_start || other_end <= self_start {
            return false;
        }
        if self_start > other_start {
            self_start = other_start;
        }
        if self_end < other_end {
            self_end = other_end;
        }
        let cursor = if self.position != self.selection_anchor { self as &Self } else { other };
        (self.position, self.selection_anchor) = if cursor.position < cursor.selection_anchor { (self_start, self_end) } else { (self_end, self_start) };
        self.reset_saved_column();
        true
    }
}