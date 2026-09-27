use unicode_width::UnicodeWidthStr;

use super::{utility::*, cursor::Cursor};

fn replace_text(text: &mut Vec<String>, position: TextPosition, remove_to: TextPosition, inserted_text: Vec<String>, cursors: &mut [Cursor]) {
    /*
           | removed text | inserted text | save end | splice first line | splice text | append end
    -------+--------------+---------------+----------+-------------------+-------------+-----------
    case 1 | ...~~~...    | ***           | ...      | ...***            | ...***      | ...***...
    -------+--------------+---------------+----------+-------------------+-------------+-----------
    case 2 | ...~~~       | ***           | ...      | ...***            | ...***      | ...***...
           | ~~~~~~       |               |          | ~~~~~~            |             |
           | ~~~...       |               |          | ~~~...            |             |
    -------+--------------+---------------+----------+-------------------+-------------+-----------
    case 3 | ...~~~...    | ***           | ...      | ...***            | ...***      | ...***
           |              | ******        |          |                   | ******      | ******
           |              | ***           |          |                   | ***         | ***...
    -------+--------------+---------------+----------+-------------------+-------------+-----------
    case 4 | ...~~~       | ***           | ...      | ...***            | ...***      | ...***
           | ~~~~~~       | ******        |          | ~~~~~~            | ******      | ******
           | ~~~...       | ***           |          | ~~~...            | ***         | ***...
    */

    // save end
    let last_line = &mut text[remove_to.line];
    let end = last_line.split_off(byte_index(last_line, remove_to.column));
    // splice first line
    let first_line = &mut text[position.line];
    first_line.replace_range(byte_index(first_line, position.column) .., &inserted_text[0]);
    // splice text
    text.splice(position.line + 1 ..= remove_to.line, inserted_text[1 ..].iter().cloned());
    // append end
    text[position.line + inserted_text.len() - 1] += &end;

    // move cursors
    let start = Cursor::new(position);
    let start = cursors.partition_point(|cursor| cursor < &start);
    move_positions(position, remove_to, &inserted_text, &mut cursors[start ..].iter_mut()
        .flat_map(|c| if c.selection_anchor <= c.position { [&mut c.selection_anchor, &mut c.position] } else { [&mut c.position, &mut c.selection_anchor] }).collect::<Vec<_>>());
    for cursor in cursors.iter_mut() {
        cursor.reset_saved_column();
    }
}

fn move_positions(position: TextPosition, remove_to: TextPosition, inserted_text: &[String], positions: &mut [&mut TextPosition]) {
    let mut i = positions.partition_point(|p| **p < position);
    // move positions within replaced text
    if i == positions.len() {
        return;
    }
    let mut curr_position = &mut positions[i];
    let end_position = position.add_text(inserted_text);
    while **curr_position <= remove_to {
        **curr_position = end_position;
        i += 1;
        if i == positions.len() {
            return;
        }
        curr_position = &mut positions[i];
    }
    // move positions on last line of replaced text
    let line_offset = inserted_text.len() as isize - remove_to.line as isize + position.line as isize - 1;
    let mut column_offset = inserted_text.last().unwrap().width() as isize - remove_to.column as isize;
    if inserted_text.len() == 1 {
        column_offset += position.column as isize;
    }
    while curr_position.line == remove_to.line {
        let line = &mut curr_position.line;
        *line = (*line as isize + line_offset) as usize;
        let column = &mut curr_position.column;
        *column = (*column as isize + column_offset) as usize;
        i += 1;
        if i == positions.len() {
            return;
        }
        curr_position = &mut positions[i];
    }
    // move positions on lines after replaced text
    if line_offset == 0 {
        return;
    }
    while i < positions.len() {
        let line = &mut positions[i].line;
        *line = (*line as isize + line_offset) as usize;
        i += 1;
    }
}

pub struct Change {
    position: TextPosition,
    removed_text: Vec<String>,
    pub inserted_text: Vec<String>
}

impl Change {
    pub fn new(text: &[String], position: TextPosition, remove_to: TextPosition, inserted_text: Vec<String>) -> Self {
        Self{position, removed_text: position.get_text_to(text, remove_to), inserted_text}
    }

    pub fn is_empty(&self) -> bool {
        text_is_empty(&self.removed_text) && text_is_empty(&self.inserted_text)
    }
}

pub enum ActionType {
    Normal,
    Surround
}

struct Action {
    action_type: ActionType,
    init_cursors: Vec<Cursor>,
    init_main_cursor_index: usize,
    init_window_position: TextPosition,
    changes: Vec<Change>
}

impl Action {
    fn redo(&self, app: &mut super::App) {
        app.cursors = self.init_cursors.clone();
        app.main_cursor_index = self.init_main_cursor_index;
        app.window_position = self.init_window_position;
        for change in self.changes.iter().rev() {
            replace_text(&mut app.text, change.position, change.position.add_text(&change.removed_text), change.inserted_text.clone(), &mut app.cursors);
        }
        if let ActionType::Surround = self.action_type {
            let mut change_index = 0;
            for cursor in &mut app.cursors {
                if cursor.position == cursor.selection_anchor {
                    if self.changes[change_index].inserted_text[0].len() == 2 {
                        cursor.position.column -= 1;
                        cursor.selection_anchor.column -= 1;
                    }
                    change_index += 1;
                } else {
                    change_index += 2;
                }
            }
        }
    }

    fn undo(&self, app: &mut super::App) {
        for change in &self.changes {
            replace_text(&mut app.text, change.position, change.position.add_text(&change.inserted_text), change.removed_text.clone(), &mut app.cursors);
        }
        app.cursors = self.init_cursors.clone();
        app.main_cursor_index = self.init_main_cursor_index;
        app.window_position = self.init_window_position;
    }
}

struct HistoryNode {
    action: Action,
    prev: usize,
    next: usize,
    depth: usize
}

pub struct History {
    tree: Vec<HistoryNode>,
    pub pos: usize
}

impl History {
    pub fn new() -> Self {
        Self{
            tree: vec![HistoryNode{
                action: Action{action_type: ActionType::Normal, init_cursors: Vec::new(), init_main_cursor_index: 0, init_window_position: TextPosition::new(0, 0), changes: Vec::new()},
                prev: 0,
                next: 0,
                depth: 0
            }],
            pos: 0
        }
    }

    fn do_action(&mut self, action: Action, app: &mut super::App) {
        action.redo(app);
        app.update_text_width();
        app.scroll_to_main_cursor = true;
        self.tree.push(HistoryNode{action, prev: self.pos, next: 0, depth: self.tree[self.pos].depth + 1});
        let new_pos = self.tree.len() - 1;
        self.tree[self.pos].next = new_pos;
        self.pos = new_pos;
    }

    fn undo_unchecked(&mut self, app: &mut super::App) {
        let node = &self.tree[self.pos];
        node.action.undo(app);
        self.pos = node.prev;
    }

    fn redo_unchecked(&mut self, app: &mut super::App) {
        self.pos = self.tree[self.pos].next;
        self.tree[self.pos].action.redo(app);
    }

    fn undo(&mut self, app: &mut super::App) {
        if self.pos != 0 {
            self.undo_unchecked(app);
            app.update_text_width();
            app.scroll_to_main_cursor = true;
        }
    }

    fn redo(&mut self, app: &mut super::App) {
        if self.tree[self.pos].next != 0 {
            self.redo_unchecked(app);
            app.update_text_width();
            app.scroll_to_main_cursor = true;
        }
    }

    fn backtrack_new_branch(&mut self, other_pos: usize) -> usize {
        let new_other_pos = self.tree[other_pos].prev;
        self.tree[new_other_pos].next = other_pos;
        new_other_pos
    }

    fn set_pos(&mut self, new_pos: usize, app: &mut super::App) {
        let mut other_pos = new_pos;
        let (curr_depth, new_depth) = (self.tree[self.pos].depth, self.tree[new_pos].depth);
        if new_depth < curr_depth {
            for _ in new_depth .. curr_depth {
                self.undo_unchecked(app);
            }
        } else {
            for _ in curr_depth .. new_depth {
                other_pos = self.backtrack_new_branch(other_pos);
            }
        }
        while self.pos != other_pos {
            self.undo_unchecked(app);
            other_pos = self.backtrack_new_branch(other_pos);
        }
        while self.pos != new_pos {
            self.redo_unchecked(app);
        }
        app.update_text_width();
        app.scroll_to_main_cursor = true;
    }

    fn prev_pos(&mut self, app: &mut super::App) {
        if self.pos != 0 {
            self.set_pos(self.pos - 1, app);
        }
    }

    fn next_pos(&mut self, app: &mut super::App) {
        if self.pos != self.tree.len() - 1 {
            self.set_pos(self.pos + 1, app);
        }
    }
}

impl<'a> super::App<'a> {
    pub(in super) fn do_action(&mut self, action_type: ActionType, changes: Vec<Change>) {
        let action = Action{
            action_type,
            init_cursors: self.cursors.clone(),
            init_main_cursor_index: self.main_cursor_index,
            init_window_position: self.window_position,
            changes
        };
        unsafe { (*(self as *mut Self)).history.do_action(action, self); }
    }

    pub(in super) fn undo(&mut self) {
        unsafe { (*(self as *mut Self)).history.undo(self); }
    }

    pub(in super) fn redo(&mut self) {
        unsafe { (*(self as *mut Self)).history.redo(self); }
    }

    pub(in super) fn history_prev_pos(&mut self) {
        unsafe { (*(self as *mut Self)).history.prev_pos(self); }
    }

    pub(in super) fn history_next_pos(&mut self) {
        unsafe { (*(self as *mut Self)).history.next_pos(self); }
    }
}