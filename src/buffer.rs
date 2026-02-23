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

    fn on_text_changed(&mut self) {
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
            self.text.remove(pos - 1..pos);
            self.cursor_x -= 1;
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
        self.text.insert_char(pos, '\n');
        self.cursor_y += 1;
        self.cursor_x = 0;
        self.on_text_changed();
    }

    pub fn jump_to_line(&mut self, line: usize) {
        let nb = self.text.len_lines();
        if line < nb {
            self.cursor_y = line;
        }
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

    fn visible_line_len(&self, line_idx: usize) -> usize {
        let len = self.text.line(line_idx).len_chars();
        if line_idx + 1 < self.text.len_lines() {
            len - 1
        } else {
            len
        }
    }
}
