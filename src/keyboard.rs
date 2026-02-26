use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{editor::Editor, mode::EditorMode};

const DEFAULT_SCROLL_JUMP: usize = 10;

#[derive(Debug, Clone)]
pub struct KeyboardConfig {
    pub scroll_jump: usize,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            scroll_jump: DEFAULT_SCROLL_JUMP,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct KeyboardHandler {
    pub config: KeyboardConfig,
}

impl KeyboardHandler {
    pub fn new(config: KeyboardConfig) -> Self {
        Self { config }
    }

    pub fn handle_key(&self, key: KeyEvent, editor: &mut Editor) -> Result<()> {
        if Self::handle_global(key, editor) {
            return Ok(());
        }

        match editor.mode.clone() {
            EditorMode::Nav => self.handle_nav(key, editor),
            EditorMode::Insert => self.handle_insert(key, editor),
            EditorMode::TreeNav => self.handle_tree(key, editor),
            EditorMode::Command {
                mut command_str,
                former_mode,
            } => Self::handle_command(key, editor, &mut command_str, &former_mode),
        }
    }

    fn handle_global(key: KeyEvent, editor: &mut Editor) -> bool {
        match key.code {
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                editor.should_quit = true;
                true
            }
            _ => false,
        }
    }

    fn handle_arrows(&self, key: KeyEvent, editor: &mut Editor) -> Result<()> {
        let Some(buf) = editor.buf_mut() else {
            return Ok(());
        };

        let ctrl = key.modifiers == KeyModifiers::CONTROL;

        match (key.code, ctrl) {
            (KeyCode::Up, true) => buf.move_up(self.config.scroll_jump),
            (KeyCode::Up, false) => buf.move_up(1),
            (KeyCode::Down, true) => buf.move_down(self.config.scroll_jump),
            (KeyCode::Down, false) => buf.move_down(1),
            (KeyCode::Left, true) => buf.move_word_left(),
            (KeyCode::Left, false) => buf.move_left(),
            (KeyCode::Right, true) => buf.move_word_right(),
            (KeyCode::Right, false) => buf.move_right(),
            _ => {}
        }
        Ok(())
    }

    fn handle_nav(&self, key: KeyEvent, editor: &mut Editor) -> Result<()> {
        match key.code {
            KeyCode::Char('x') if key.modifiers == KeyModifiers::CONTROL => {
                editor.show_tree = true;
                editor.mode = EditorMode::TreeNav;
            }
            KeyCode::Char('n') if key.modifiers == KeyModifiers::CONTROL => {
                editor.next_buffer();
            }
            KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
                editor.prev_buffer();
            }
            KeyCode::Char('w') if key.modifiers == KeyModifiers::CONTROL => {
                if let Some(i) = editor.active_buffer {
                    editor.close_buffer(i);
                }
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                self.handle_arrows(key, editor)?;
            }
            KeyCode::Char('i') => {
                editor.mode = EditorMode::Insert;
            }
            KeyCode::Char('$') => {
                if let Some(buf) = editor.buf_mut() {
                    buf.jump_to_line_end();
                }
            }
            KeyCode::Char('*') => {
                if let Some(buf) = editor.buf_mut() {
                    buf.jump_to_line_indent();
                }
            }
            KeyCode::Char(':') => {
                editor.mode = EditorMode::command(EditorMode::Nav);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_insert(&self, key: KeyEvent, editor: &mut Editor) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                self.handle_arrows(key, editor)?;
            }
            KeyCode::Char(c) => editor.insert_char(c),
            KeyCode::Backspace => editor.delete_char(),
            KeyCode::Enter => editor.insert_newline(),
            KeyCode::Esc => editor.mode = EditorMode::Nav,
            _ => {}
        }
        Ok(())
    }

    fn handle_tree(&self, key: KeyEvent, editor: &mut Editor) -> Result<()> {
        match key.code {
            KeyCode::Up => editor.file_tree.move_up(),
            KeyCode::Down => editor.file_tree.move_down(),
            KeyCode::Left => editor.file_tree.collapse_selected(),
            KeyCode::Right => editor.file_tree.expand_selected(),
            KeyCode::Enter => {
                if let Some(path) = editor.file_tree.enter() {
                    editor.open_file(&path)?;
                    editor.mode = EditorMode::Nav;
                }
            }
            KeyCode::Esc | KeyCode::Char('x')
                if key.code == KeyCode::Esc || key.modifiers == KeyModifiers::CONTROL =>
            {
                editor.show_tree = false;
                editor.mode = EditorMode::Nav;
            }
            KeyCode::Char(':') => {
                editor.mode = EditorMode::command(EditorMode::TreeNav);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_command(
        key: KeyEvent,
        editor: &mut Editor,
        command_str: &mut String,
        former_mode: &EditorMode,
    ) -> Result<()> {
        match key.code {
            KeyCode::Char(':') => command_str.clear(),
            KeyCode::Char(c) => command_str.push(c),
            KeyCode::Backspace => {
                if command_str.pop().is_none() {
                    editor.mode = former_mode.clone();
                    return Ok(());
                }
            }
            KeyCode::Esc => {
                editor.mode = former_mode.clone();
                return Ok(());
            }
            KeyCode::Enter => {
                Self::execute_command(command_str, editor, former_mode)?;
                return Ok(());
            }
            _ => {}
        }

        editor.mode = EditorMode::Command {
            command_str: command_str.clone(),
            former_mode: Box::new(former_mode.clone()),
        };
        Ok(())
    }

    fn execute_command(cmd: &str, editor: &mut Editor, former_mode: &EditorMode) -> Result<()> {
        match cmd {
            "q" => {
                editor.should_quit = true;
                editor.mode = EditorMode::Nav;
            }
            "w" => {
                editor.save_and_check()?;
                editor.mode = former_mode.clone();
            }
            "wq" => {
                editor.save_and_check()?;
                editor.should_quit = true;
                editor.mode = EditorMode::Nav;
            }
            "x" => {
                editor.show_tree = true;
                editor.mode =
                    if *former_mode == EditorMode::TreeNav && editor.active_buffer.is_some() {
                        EditorMode::Nav
                    } else {
                        EditorMode::TreeNav
                    };
            }
            "bd" | "close" => {
                if let Some(i) = editor.active_buffer {
                    if let Some(buf) = editor.buf_mut() {
                        buf.save()?;
                    }
                    editor.close_buffer(i);
                }
                editor.mode = EditorMode::Nav;
            }
            "bn" | "next" => {
                editor.next_buffer();
                editor.mode = former_mode.clone();
            }
            "bp" | "prev" => {
                editor.prev_buffer();
                editor.mode = former_mode.clone();
            }
            other => {
                if let Ok(line) = other.parse::<usize>()
                    && let Some(buf) = editor.buf_mut()
                {
                    buf.jump_to_line(line);
                }
                editor.mode = former_mode.clone();
            }
        }
        Ok(())
    }
}
