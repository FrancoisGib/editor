use ropey::Rope;
use std::path::{Path, PathBuf};

use crate::highlighter::Highlighter;

pub struct Buffer {
    pub text: Rope,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub scroll_y: usize,
    pub filepath: Option<PathBuf>,
    pub name: String,
    pub modified: bool,
    pub highlighter: Highlighter,
}

impl Buffer {
    pub fn from_file(path: &Path) -> Self {
        let text = std::fs::read_to_string(path)
            .map(|s| Rope::from_str(&s))
            .unwrap_or_else(|_| Rope::new());

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let mut highlighter = Highlighter::new();
        highlighter.update(&text.to_string());

        Self {
            text,
            cursor_x: 0,
            cursor_y: 0,
            scroll_y: 0,
            filepath: Some(path.to_path_buf()),
            name,
            modified: false,
            highlighter,
        }
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(ref path) = self.filepath {
            std::fs::write(path, self.text.to_string())?;
            self.modified = false;
        }
        Ok(())
    }

    pub fn display_name(&self) -> String {
        if self.modified {
            format!("{}*", self.name)
        } else {
            self.name.clone()
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = self.visible_line_len(self.cursor_y);
        }
    }

    pub fn move_right(&mut self) {
        let vis_len = self.visible_line_len(self.cursor_y);
        if self.cursor_x < vis_len {
            self.cursor_x += 1;
        } else if self.cursor_y + 1 < self.text.len_lines() {
            self.cursor_y += 1;
            self.cursor_x = 0;
        }
    }

    pub fn move_up(&mut self, scroll: usize) {
        let jump = self.cursor_y.min(scroll);
        if jump > 0 {
            self.cursor_y -= jump;
            self.cursor_x = self.cursor_x.min(self.visible_line_len(self.cursor_y));
        } else if self.cursor_y == 0 {
            self.cursor_x = 0;
        }
    }

    pub fn move_down(&mut self, scroll: usize) {
        let nb_lines = self.text.len_lines();
        let jump = (nb_lines - 1 - self.cursor_y).min(scroll);
        if jump > 0 {
            self.cursor_y += jump;
            self.cursor_x = self.cursor_x.min(self.visible_line_len(self.cursor_y));
        } else if self.cursor_y + 1 == nb_lines {
            self.cursor_x = self.visible_line_len(self.cursor_y);
        }
    }

    pub fn on_text_changed(&mut self) {
        self.modified = true;
        self.highlighter.update(&self.text.to_string());
    }

    pub fn insert_char(&mut self, c: char) {
        let pos = self.text.line_to_char(self.cursor_y) + self.cursor_x;
        self.text.insert_char(pos, c);
        self.cursor_x += 1;
        self.on_text_changed();
    }

    pub fn delete_char(&mut self) {
        if self.cursor_x > 0 {
            let pos = self.text.line_to_char(self.cursor_y) + self.cursor_x;
            let line = self.text.line(self.cursor_y);
            let nb_spaces: usize = line
                .chars()
                .take_while(|c| *c == ' ' && *c != '\t')
                .map(|c| if c == ' ' { 1 } else { 0 })
                .sum();

            let chars_to_remove = if nb_spaces.is_multiple_of(4) {
                4
            } else if nb_spaces > 0 {
                nb_spaces % 4
            } else {
                1
            };

            self.text.remove(pos - chars_to_remove..pos);
            self.cursor_x -= chars_to_remove;
            self.on_text_changed();
        } else if self.cursor_y > 0 {
            let pos = self.text.line_to_char(self.cursor_y);
            let prev_len = self.visible_line_len(self.cursor_y - 1);
            self.text.remove(pos - 1..pos);
            self.cursor_y -= 1;
            self.cursor_x = prev_len;
            self.on_text_changed();
        }
    }

    pub fn newline(&mut self) {
        let pos = self.text.line_to_char(self.cursor_y) + self.cursor_x;
        let indent = self.indent_after(self.cursor_y);

        self.text.insert_char(pos, '\n');
        if indent > 0 {
            self.text.insert(pos + 1, &" ".repeat(indent));
        }

        self.cursor_y += 1;
        self.cursor_x = indent;
        self.on_text_changed();
    }

    pub fn jump_to_line(&mut self, line: usize) {
        let nb = self.text.len_lines();
        if line < nb {
            self.cursor_y = line;
        }
    }

    pub fn jump_to_line_end(&mut self) {
        self.cursor_x = self.visible_line_len(self.cursor_y);
    }

    pub fn jump_to_line_indent(&mut self) {
        self.cursor_x = self.line_indent(self.cursor_y);
    }

    pub fn compute_scroll(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        if self.cursor_y < self.scroll_y {
            self.scroll_y = self.cursor_y;
        } else if self.cursor_y >= self.scroll_y + viewport_height {
            self.scroll_y = self.cursor_y - viewport_height + 1;
        }
    }

    pub fn move_word_left(&mut self) {
        let s = self.line_content(self.cursor_y);

        if self.cursor_x > 0 {
            let chars: Vec<char> = s[..self.cursor_x].chars().collect();
            let mut i = chars.len();

            while i > 0 && chars[i - 1].is_whitespace() {
                i -= 1;
            }

            if i > 0 {
                let target = char_class(chars[i - 1]);
                while i > 0 && char_class(chars[i - 1]) == target {
                    i -= 1;
                }
            }

            self.cursor_x = chars[..i].iter().collect::<String>().len();
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = self.line_content(self.cursor_y).len();
        }
    }

    pub fn move_word_right(&mut self) {
        let s = self.line_content(self.cursor_y);
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();

        if self.cursor_x < s.len() {
            let ci = s[..self.cursor_x].chars().count();
            let mut i = ci;

            let target = char_class(chars[i]);
            while i < len && char_class(chars[i]) == target {
                i += 1;
            }

            while i < len && chars[i].is_whitespace() {
                i += 1;
            }

            self.cursor_x = chars[..i].iter().collect::<String>().len();
        } else if self.cursor_y < self.text.len_lines() - 1 {
            self.cursor_y += 1;
            self.cursor_x = 0;
        }
    }

    pub fn visible_line_len(&self, line_idx: usize) -> usize {
        let len = self.text.line(line_idx).len_chars();
        if line_idx + 1 < self.text.len_lines() {
            len - 1
        } else {
            len
        }
    }

    /// Indent to use for a new line inserted below `ref_line`.
    fn indent_after(&self, ref_line: usize) -> usize {
        let s = self.text.line(ref_line).as_str().unwrap_or("");
        let trimmed = s.trim();

        if trimmed.is_empty() && ref_line > 0 {
            return self.indent_after(ref_line - 1);
        }

        let leading = s.len() - trimmed.len() - 1;

        if trimmed.ends_with('{') {
            return leading + 4;
        }

        if trimmed.starts_with('.') && trimmed.ends_with(';') {
            return self.skip_chain(ref_line);
        }

        leading
    }

    /// Where to place the cursor on the current line.
    /// Non-empty -> start of content. Empty -> derive from above.
    fn line_indent(&self, line: usize) -> usize {
        let s = self.text.line(line).as_str().unwrap_or("");
        let trimmed = s.trim_start();

        if !trimmed.is_empty() {
            return s.len() - trimmed.len();
        }

        if line > 0 {
            return self.indent_after(line - 1);
        }

        0
    }

    fn skip_chain(&self, line: usize) -> usize {
        if line == 0 {
            return 0;
        }
        let above = line - 1;
        let s = self.text.line(above).as_str().unwrap_or("");
        let trimmed = s.trim_start();

        if trimmed.starts_with('.') {
            return self.skip_chain(above);
        }

        s.len() - trimmed.len()
    }

    fn line_content(&self, y: usize) -> String {
        let s = self.text.line(y).to_string();
        s.trim_end_matches('\n').trim_end_matches('\r').to_string()
    }
}

/// Character classification for word boundary detection.
fn char_class(c: char) -> u8 {
    if c.is_alphanumeric() || c == '_' {
        0 // word
    } else if c.is_whitespace() {
        1 // whitespace
    } else {
        2 // punctuation / operator
    }
}
