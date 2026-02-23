// mode.rs
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::style::{Color, Style};
use std::fmt::Display;

use crate::editor::Editor;

const MOUSE_SCROLL: usize = 3;
const CONTROL_SCROLL: usize = 10;

#[derive(Debug, Clone)]
pub enum EditorMode {
    Nav,
    Insert,
    TreeNav,
    Command {
        command_str: String,
        former_mode: Box<EditorMode>,
    },
}

impl PartialEq for EditorMode {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

impl EditorMode {
    pub fn command(former: EditorMode) -> Self {
        Self::Command {
            command_str: String::new(),
            former_mode: Box::new(former),
        }
    }

    pub fn get_style(&self) -> Style {
        match self {
            Self::Nav => Style::default().fg(Color::Cyan),
            Self::Insert => Style::default().fg(Color::Yellow),
            Self::TreeNav => Style::default().fg(Color::Black).bg(Color::Cyan),
            Self::Command { .. } => Style::default().fg(Color::Red),
        }
    }

    pub fn handle_key(mut self, key: KeyEvent, editor: &mut Editor) -> Result<Self> {
        match &mut self {
            Self::Nav => Self::handle_nav(key, editor),
            Self::Insert => Self::handle_insert(key, editor),
            Self::TreeNav => Self::handle_tree(key, editor),
            Self::Command {
                command_str,
                former_mode,
            } => Self::handle_command(key, editor, command_str, former_mode),
        }
    }

    pub fn handle_navigation_key(key: KeyEvent, editor: &mut Editor) -> Result<()> {
        let buf = if let Some(buf) = editor.buf_mut() {
            buf
        } else {
            return Ok(());
        };

        match key.code {
            KeyCode::Up => {
                let jump = if key.modifiers == KeyModifiers::CONTROL {
                    CONTROL_SCROLL
                } else {
                    1
                };
                buf.move_up(jump);
            }
            KeyCode::Down => {
                let jump = if key.modifiers == KeyModifiers::CONTROL {
                    CONTROL_SCROLL
                } else {
                    1
                };
                buf.move_down(jump);
            }
            KeyCode::Left => {
                buf.move_left();
            }
            KeyCode::Right => {
                buf.move_right();
            }
            _ => {}
        }
        Ok(())
    }

    pub fn handle_mouse(self, mouse_event: MouseEvent, editor: &mut Editor) -> Result<Self> {
        match &self {
            EditorMode::Nav | EditorMode::Insert => {}
            _ => {
                return Ok(self);
            }
        }

        let buf = if let Some(buf) = editor.buf_mut() {
            buf
        } else {
            return Ok(self);
        };

        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                buf.move_up(MOUSE_SCROLL);
            }
            MouseEventKind::ScrollDown => {
                buf.move_down(MOUSE_SCROLL);
            }
            _ => {}
        }
        Ok(self)
    }

    fn handle_nav(key: KeyEvent, editor: &mut Editor) -> Result<Self> {
        match key.code {
            KeyCode::Char('x') if key.modifiers == KeyModifiers::CONTROL => {
                editor.show_tree = true;
                Ok(Self::TreeNav)
            }
            KeyCode::Char('n') if key.modifiers == KeyModifiers::CONTROL => {
                editor.next_buffer();
                Ok(Self::Nav)
            }
            KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
                editor.prev_buffer();
                Ok(Self::Nav)
            }
            KeyCode::Char('w') if key.modifiers == KeyModifiers::CONTROL => {
                if let Some(i) = editor.active_buffer {
                    editor.close_buffer(i);
                }
                Ok(Self::Nav)
            }
            KeyCode::Down | KeyCode::Up | KeyCode::Left | KeyCode::Right => {
                Self::handle_navigation_key(key, editor)?;
                Ok(Self::Nav)
            }
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                editor.should_quit = true;
                Ok(Self::Nav)
            }
            KeyCode::Char('i') => Ok(Self::Insert),
            KeyCode::Char(':') => Ok(Self::command(Self::Nav)),
            _ => Ok(Self::Nav),
        }
    }

    fn handle_insert(key: KeyEvent, editor: &mut Editor) -> Result<Self> {
        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Left | KeyCode::Right => {
                Self::handle_navigation_key(key, editor)?;
            }
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                editor.should_quit = true;
            }
            KeyCode::Char(c) => {
                editor.insert_char(c);
            }
            KeyCode::Backspace => {
                editor.delete_char();
            }
            KeyCode::Enter => {
                editor.insert_newline();
            }
            KeyCode::Esc => {
                return Ok(Self::Nav);
            }
            _ => {}
        }
        Ok(Self::Insert)
    }

    fn handle_tree(key: KeyEvent, editor: &mut Editor) -> Result<Self> {
        match key.code {
            KeyCode::Up => editor.file_tree.move_up(),
            KeyCode::Down => editor.file_tree.move_down(),
            KeyCode::Enter => {
                if let Some(path) = editor.file_tree.enter() {
                    editor.open_file(&path)?;
                    return Ok(Self::Nav);
                }
            }
            KeyCode::Left => editor.file_tree.collapse_selected(),
            KeyCode::Right => editor.file_tree.expand_selected(),
            KeyCode::Esc => return Ok(Self::Nav),
            KeyCode::Char('x') if key.modifiers == KeyModifiers::CONTROL => {
                editor.show_tree = false;
                return Ok(Self::Nav);
            }
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                editor.should_quit = true;
            }
            KeyCode::Char(':') => {
                return Ok(Self::command(Self::TreeNav));
            }
            _ => {}
        }
        Ok(Self::TreeNav)
    }

    fn handle_command(
        key: KeyEvent,
        editor: &mut Editor,
        command_str: &mut String,
        former_mode: &mut Box<EditorMode>,
    ) -> Result<Self> {
        match key.code {
            KeyCode::Char(':') => command_str.clear(),
            KeyCode::Char(c) => command_str.push(c),
            KeyCode::Backspace => {
                command_str.pop();
            }
            KeyCode::Esc => {
                return Ok(*former_mode.clone());
            }
            KeyCode::Enter => {
                let next = Self::execute_command(command_str, editor, former_mode)?;
                return Ok(next);
            }
            _ => {}
        }
        Ok(Self::Command {
            command_str: command_str.clone(),
            former_mode: former_mode.clone(),
        })
    }

    fn execute_command(cmd: &str, editor: &mut Editor, former_mode: &EditorMode) -> Result<Self> {
        match cmd {
            "q" => {
                editor.should_quit = true;
                Ok(Self::Nav)
            }
            "w" => {
                editor.save_and_check()?;
                Ok(former_mode.clone())
            }
            "wq" => {
                editor.save_and_check()?;
                editor.should_quit = true;
                Ok(Self::Nav)
            }
            "x" => {
                editor.show_tree = true;
                if *former_mode == Self::TreeNav && editor.active_buffer.is_some() {
                    Ok(Self::Nav)
                } else {
                    Ok(Self::TreeNav)
                }
            }
            "bd" | "close" => {
                if let Some(i) = editor.active_buffer {
                    if let Some(buf) = editor.buf_mut() {
                        buf.save()?;
                    }
                    editor.close_buffer(i);
                }
                Ok(Self::Nav)
            }
            "bn" | "next" => {
                editor.next_buffer();
                Ok(former_mode.clone())
            }
            "bp" | "prev" => {
                editor.prev_buffer();
                Ok(former_mode.clone())
            }
            s => {
                if let Ok(line) = s.parse::<usize>()
                    && let Some(buf) = editor.buf_mut()
                {
                    buf.jump_to_line(line);
                }
                Ok(former_mode.clone())
            }
        }
    }
}

impl Display for EditorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Nav => "NAV",
            Self::Insert => "INSERT",
            Self::TreeNav => "TREE",
            Self::Command { .. } => "COMMAND",
        })
    }
}
