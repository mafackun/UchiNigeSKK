#[derive(Debug, Clone)]
pub struct Buffer {
    lines: Vec<Vec<char>>,
    row: usize,
    col: usize,
}

impl Default for Buffer {
    fn default() -> Self {
        Self {
            lines: vec![Vec::new()],
            row: 0,
            col: 0,
        }
    }
}

use std::fmt::{self, Display, Formatter};
impl Display for Buffer {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "カーソル位置: ({}, {})  総行数: {}", self.row + 1, self.col + 1, self.line_count())
    }
}

impl Buffer {
    // --- getters (UI用) ---

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line_as_string(&self, row: usize) -> String {
        self.lines
            .get(row)
            .map(|v| v.iter().collect::<String>())
            .unwrap_or_default()
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

    pub fn clear(&mut self) {
        self.lines.clear();
        self.lines.push(Vec::new());
        self.row = 0;
        self.col = 0;
    }

    // --- editing primitives ---

    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            self.newline();
            return;
        }

        self.ensure_invariants();

        let line = &mut self.lines[self.row];
        line.insert(self.col, c);
        self.col += 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    pub fn newline(&mut self) {
        self.ensure_invariants();

        let line = &mut self.lines[self.row];
        let right = line.split_off(self.col);

        self.row += 1;
        self.col = 0;
        self.lines.insert(self.row, right);
    }

    pub fn backspace(&mut self) {
        self.ensure_invariants();

        if self.col > 0 {
            // 行内の1文字削除（カーソル左の文字）
            let line = &mut self.lines[self.row];
            line.remove(self.col - 1);
            self.col -= 1;
            return;
        }

        // col == 0 → 行頭。前行があれば結合
        if self.row > 0 {
            let cur = self.lines.remove(self.row);
            self.row -= 1;

            let prev = &mut self.lines[self.row];
            self.col = prev.len();
            prev.extend(cur);
        }
    }

    pub fn delete(&mut self) {
        if let Ok(()) = self.move_right() {
            self.backspace();
        }
    }

    pub fn move_left(&mut self) -> Result<(), ()> {
        self.ensure_invariants();

        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].len();
        } else {
            return Err(());
        }
        Ok(())
    }

    pub fn move_right(&mut self) -> Result<(), ()> {
        self.ensure_invariants();

        let line_len = self.lines[self.row].len();
        if self.col < line_len {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        } else {
            return Err(());
        }
        Ok(())
    }

    pub fn move_up(&mut self) -> Result<(), ()> {
        self.ensure_invariants();

        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.lines[self.row].len());
        } else {
            return Err(());
        }
        Ok(())

    }

    pub fn move_down(&mut self) -> Result<(), ()> {
        self.ensure_invariants();

        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = self.col.min(self.lines[self.row].len());
        } else {
            return Err(());
        }
        Ok(())

    }
    pub fn move_line_head(&mut self) {
        self.ensure_invariants();
        self.col = 0;
    }

    pub fn move_line_tail(&mut self) {
        self.ensure_invariants();
        self.col = self.lines[self.row].len();
    }

    fn ensure_invariants(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
            self.row = 0;
            self.col = 0;
            return;
        }

        if self.row >= self.lines.len() {
            self.row = self.lines.len() - 1;
        }

        let len = self.lines[self.row].len();
        if self.col > len {
            self.col = len;
        }
    }
}

