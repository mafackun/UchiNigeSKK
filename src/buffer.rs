use crate::util::{ClosedInterval, push_itoa_usize_to_string};
#[derive(Debug, Clone)]
pub struct Buffer {
    lines: Vec<Vec<char>>,
    row: usize,
    col: usize,
    selection_origin: Option<usize>,
    dirty: bool,
}

impl Default for Buffer {
    fn default() -> Self {
        Self {
            lines: vec![Vec::new()],
            row: 0,
            col: 0,
            selection_origin: None,
            dirty: false,
        }
    }
}

type IsOperationDone = bool;
impl Buffer {
    // --- getters (UI用) ---
    pub fn status_as_string(&self) -> String {
        let mut out = String::new();
        out.push('(');
        push_itoa_usize_to_string(&mut out, self.row + 1, 10);
        out.push('/');
        push_itoa_usize_to_string(&mut out, self.line_count(), 10);
        out.push(',');
        if let Some(origin) = self.selection_origin {
            push_itoa_usize_to_string(&mut out, origin + 1, 10);
            out.push(':');
        }
        push_itoa_usize_to_string(&mut out, self.col + 1, 10);
        out.push(')');
        out
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn selection(&self) -> ClosedInterval<usize> {
        match self.selection_origin {
            Some(origin) => ClosedInterval(origin.min(self.col), origin.max(self.col)),
            None => ClosedInterval(self.col, self.col),
        }
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn has_more_line(&self, row: usize) -> bool {
        row < self.line_count() - 1
    }

    pub fn line(&self, row: usize) -> &[char] {
        &self.lines[row]
    }

    pub fn cursor_as_char(&self) -> Option<&char> {
        if self.selection_origin.is_some() {
            return None;
        }
        self.lines[self.row].get(self.col)
    }

    pub fn selected_as_string(&self) -> Option<String> {
        let ClosedInterval(start, end) = self.selection();
        self.lines
            .get(self.row)
            .and_then(|line| line.get(start..=end))
            .map(|v| v.iter().collect())
    }

    pub fn as_string(&self) -> String {
        // クリップボード送信用（最終出力）
        // 改行は '\n' 統一
        let mut out = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.extend(line.iter());
        }
        out
    }
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    // --- editing primitives ---
    pub fn clear(&mut self) {
        self.set_dirty();
        self.lines.clear();
        self.lines.push(Vec::new());
        self.row = 0;
        self.col = 0;
        self.clear_selection_origin();
    }

    pub fn insert_char(&mut self, c: char) {
        self.set_dirty();
        if c == '\n' {
            self.newline();
            return;
        }
        if self.selection_origin.is_some() {
            self.delete_range();
        }
        let line = &mut self.lines[self.row];
        line.insert(self.col, c);
        self.col += 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        // insert_charでdirtyになる
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    pub fn backspace(&mut self) {
        // delete_rangeかdeleteでdirtyになる
        if self.selection_origin.is_some() {
            self.delete_range();
            return;
        }
        if self.move_left() {
            self.delete();
        }
    }

    pub fn delete(&mut self) {
        self.set_dirty();
        if self.selection_origin.is_some() {
            self.delete_range();
            return;
        }
        if !self.delete_on_cursor() {
            self.concatenate_cur_next_lines();
        }
    }

    pub fn delete_range(&mut self) {
        self.set_dirty();
        if let Some(origin) = self.selection_origin {
            let diff = self.col.abs_diff(origin);
            self.col = self.col.min(origin);
            for _ in 0..=diff {
                self.delete_on_cursor();
            }
            self.clear_selection_origin();
        }
    }

    pub fn move_left(&mut self) -> IsOperationDone {
        self.set_dirty();
        self.clear_selection_origin();
        if self.col > 0 {
            self.col -= 1;
        } else if self.move_up() {
            self.to_line_tail();
        } else {
            return false;
        }
        true
    }

    pub fn move_right(&mut self) -> IsOperationDone {
        self.set_dirty();
        self.clear_selection_origin();
        if self.col < self.lines[self.row].len() {
            self.col += 1;
        } else if self.move_down() {
            self.to_line_head();
        } else {
            return false;
        }
        true
    }

    pub fn move_up(&mut self) -> IsOperationDone {
        self.set_dirty();
        self.clear_selection_origin();
        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.lines[self.row].len());
            true
        } else {
            false
        }
    }

    pub fn move_down(&mut self) -> IsOperationDone {
        self.set_dirty();
        self.clear_selection_origin();
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = self.col.min(self.lines[self.row].len());
            true
        } else {
            false
        }
    }

    pub fn rapid_up(&mut self) {
        self.set_dirty();
        self.clear_selection_origin();
        self.rapid_move(Self::move_up);
    }

    pub fn rapid_down(&mut self) {
        self.set_dirty();
        self.clear_selection_origin();
        self.rapid_move(Self::move_down);
    }

    pub fn to_line_head(&mut self) {
        self.set_dirty();
        self.clear_selection_origin();
        self.col = 0;
    }

    pub fn to_line_tail(&mut self) {
        self.set_dirty();
        self.clear_selection_origin();
        self.col = self.lines[self.row].len();
    }

    pub fn select_right(&mut self) {
        self.set_dirty();
        if self.col < self.lines[self.row].len().saturating_sub(1) {
            self.set_selection_origin();
            self.col += 1;
        }
    }

    pub fn select_left(&mut self) {
        self.set_dirty();
        if self.col > 0 {
            self.set_selection_origin();
            self.col -= 1;
        }
    }

    // --- helpers ---
    fn set_dirty(&mut self) {
        self.dirty = true;
    }

    fn newline(&mut self) {
        self.clear_selection_origin();
        let line = &mut self.lines[self.row];
        let right = line.split_off(self.col);

        self.row += 1;
        self.col = 0;
        self.lines.insert(self.row, right);
    }

    fn delete_on_cursor(&mut self) -> IsOperationDone {
        let line = &mut self.lines[self.row];
        if self.col < line.len() {
            line.remove(self.col);
            true
        } else {
            false
        }
    }

    fn concatenate_cur_next_lines(&mut self) {
        if self.row < self.line_count() - 1 {
            let next = self.lines.remove(self.row + 1);
            let cur = &mut self.lines[self.row];
            cur.extend(next);
        }
    }

    fn rapid_move<F: Fn(&mut Self) -> bool>(&mut self, f: F) {
        self.clear_selection_origin();
        let max_scroll = (self.line_count() / 10).max(5);
        for _ in 0..max_scroll {
            if !f(self) {
                break;
            }
        }
    }

    fn set_selection_origin(&mut self) {
        if self.selection_origin.is_none() {
            self.selection_origin =
                Some(self.col.min(self.lines[self.row].len().saturating_sub(1)));
        }
    }

    fn clear_selection_origin(&mut self) {
        self.selection_origin = None;
    }
}
