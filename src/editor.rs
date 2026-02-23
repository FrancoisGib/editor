use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{Frame, layout::Rect, style::{Color, Style}, text::{Line, Span}, widgets::Paragraph};
use ropey::Rope;

use crate::mode::EditorMode;

pub struct Editor {
    pub text: Rope,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub filename: String,
    _filepath: PathBuf,
    pub should_quit: bool,
    pub scroll_y: usize,
    pub mode: EditorMode,
    command_str: String,
}

impl Editor {
    pub fn new(filename: &str) -> Self {
        let text = std::fs::read_to_string(filename)
            .map(|s| Rope::from_str(&s))
            .unwrap_or_else(|_| Rope::new());
        let filepath = PathBuf::from(filename)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(filename));

        Self {
            text,
            cursor_x: 0,
            cursor_y: 0,
            filename: filename.into(),
            _filepath: filepath,
            should_quit: false,
            scroll_y: 0,
            mode: EditorMode::Nav,
            command_str: String::new(),
        }
    }

    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => {
                self.handle_key(key)?;
            }
            Event::Mouse(mouse) => {
                self.handle_mouse(mouse)?;
            },
            _ => {}
        }
        Ok(())
    }

    pub fn render_status(&self, f: &mut Frame, rect: Rect) {
        let mut components = vec![
            Span::styled(
                format!(" {} ", self.filename),
                Style::default().fg(Color::Black).bg(Color::White),
            ),
            Span::raw(format!(
                "  {}:{} ",
                self.cursor_y + 1,
                self.cursor_x + 1
            )),
            Span::styled(
                format!(" {} ", self.mode), 
                self.mode.get_style(),
            )
        ];
        if self.mode == EditorMode::Command {
            components.push(Span::raw(format!(" :{} ", self.command_str)))
        }

        let status = Line::from(components);
        f.render_widget(Paragraph::new(status), rect);
    }

    fn insert_char(&mut self, c: char) {
        let pos = self.line_start() + self.cursor_x;
        self.text.insert_char(pos, c);
        self.cursor_x += 1;
    }

    fn delete_char(&mut self) {
        if self.cursor_x > 0 {
            let pos = self.line_start() + self.cursor_x;
            self.text.remove(pos - 1..pos);
            self.cursor_x -= 1;
        } else if self.cursor_y > 0 {
            let pos = self.text.line_to_char(self.cursor_y);
            let prev_line = self.text.line(self.cursor_y - 1);
            self.cursor_y -= 1;
            self.cursor_x = prev_line.len_chars() - 1;
            self.text.remove(pos - 1..pos);
        }
    }

    fn line_start(&self) -> usize {
        self.text.line_to_char(self.cursor_y)
    }

    fn save_file(&self) -> Result<()> {
        std::fs::write(&self.filename, self.text.to_string())?;
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            EditorMode::Nav => self.handle_nav_mode_key(key),
            EditorMode::Command => self.handle_command_mode_key(key),
            EditorMode::Insert => self.handle_insert_mode_key(key),
        }
    }

    fn handle_navigation_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Down => {
                if self.text.len_lines() - 1 > self.cursor_y {
                    self.cursor_y += 1;
                    let new_current_line_len = self.text.line(self.cursor_y).len_chars();
                    if self.cursor_x > new_current_line_len - 1 {
                        self.cursor_x = new_current_line_len - 1;
                    }
                }
            }
            KeyCode::Up => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    let new_current_line_len = self.text.line(self.cursor_y).len_chars();
                    if self.cursor_x > new_current_line_len - 1 {
                        self.cursor_x = new_current_line_len - 1;
                    }
                }
            }
            KeyCode::Left => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                } else if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    self.cursor_x = self.text.line(self.cursor_y).len_chars() - 1;
                }
            }
            KeyCode::Right => {
                let current_line_len = self.text.line(self.cursor_y).len_chars();
                if self.cursor_x < current_line_len - 1 {
                    self.cursor_x += 1;
                } else if self.cursor_y < self.text.len_lines() - 1 {
                    self.cursor_y += 1;
                    self.cursor_x = 0;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_nav_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Left | KeyCode::Right => self.handle_navigation_key(key)?,
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            KeyCode::Char('i') => {
                self.mode = EditorMode::Insert;
            }
            KeyCode::Char(':') => {
                self.mode = EditorMode::Command;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_insert_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Left | KeyCode::Right => self.handle_navigation_key(key)?,
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            KeyCode::Char(c) => {
                self.insert_char(c);
            }
            KeyCode::Backspace => {
                self.delete_char();
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Nav;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_command_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char(':') => {
                self.command_str = String::new();
            }
            KeyCode::Char(c) => {
                self.command_str.push(c);
            }
            KeyCode::Esc => {
                self.command_str = String::new();
                self.mode = EditorMode::Nav;
            }
            KeyCode::Backspace => {
                self.command_str.pop();
            }
            KeyCode::Enter => {
                match self.command_str.as_str() {
                    "q" => {
                        self.should_quit = true;
                    }
                    "w" => {
                        self.save_file()?;
                    }
                    "wq" => {
                        self.save_file()?;
                        self.should_quit = true;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Result<()> {
        match mouse_event.kind {
            MouseEventKind::ScrollDown => {
                if self.cursor_y < self.text.len_lines() - 3 {
                    self.cursor_y += 3;
                }
            },
            MouseEventKind::ScrollUp => {
                if self.cursor_y >= 3 {
                    self.cursor_y -= 3;
                } else {
                    self.cursor_y = 0;
                }
            },
            _ => {}
        }
        Ok(())
    }
}
