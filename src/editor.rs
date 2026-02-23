use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};

use crate::{buffer::Buffer, displayer::Displayer, mode::EditorMode, tree::FileTree};

const CONTROL_SCROLL: usize = 10;
const MOUSE_SCROLL: usize = 3;

pub struct Editor {
    pub buffers: Vec<Buffer>,
    pub active_buffer: Option<usize>,
    should_quit: bool,
    pub mode: EditorMode,
    pub command_str: String,
    pub file_tree: FileTree,
    pub show_tree: bool,
    former_mode: EditorMode,
}

impl Editor {
    pub fn new(path: &str) -> Result<Self> {
        let canon_path = PathBuf::from(path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(path));

        let (buffers, project_dir) = if canon_path.is_dir() {
            (vec![], canon_path.clone())
        } else {
            let project_dir = canon_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            (vec![Buffer::from_file(&canon_path)], project_dir)
        };

        let active_buffer = if buffers.is_empty() { None } else { Some(0) };

        let mode = if canon_path.is_dir() {
            EditorMode::TreeNav
        } else {
            EditorMode::Nav
        };

        Ok(Self {
            buffers,
            active_buffer,
            should_quit: false,
            mode,
            command_str: String::new(),
            file_tree: FileTree::new(&project_dir),
            show_tree: true,
            former_mode: mode,
        })
    }

    pub fn run(mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            SetCursorStyle::SteadyBar
        )?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let mut displayer = Displayer::new(terminal);

        loop {
            if self.should_quit {
                break;
            }

            let vh = displayer.viewport_height();
            if let Some(buf) = self.buf_mut() {
                buf.compute_scroll(vh);
            }

            displayer.draw(&self)?;

            if event::poll(Duration::from_millis(1))? {
                let event = event::read()?;
                self.handle_event(event)?;
            }
        }

        disable_raw_mode()?;
        execute!(
            displayer.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            SetCursorStyle::DefaultUserShape
        )?;
        Ok(())
    }

    pub fn buf(&self) -> Option<&Buffer> {
        self.active_buffer
            .and_then(|active_buffer| self.buffers.get(active_buffer))
    }

    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key)?,
            Event::Mouse(mouse) => self.handle_mouse(mouse)?,
            _ => {}
        }
        Ok(())
    }

    fn buf_mut(&mut self) -> Option<&mut Buffer> {
        self.active_buffer
            .and_then(|active_buffer| self.buffers.get_mut(active_buffer))
    }

    fn open_file(&mut self, path: &Path) -> Result<()> {
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        for (i, buf) in self.buffers.iter().enumerate() {
            if let Some(path) = &buf.filepath
                && *path == canon
            {
                self.active_buffer = Some(i);
                return Ok(());
            }
        }

        self.buffers.push(Buffer::from_file(&canon));
        self.active_buffer = Some(self.buffers.len() - 1);

        Ok(())
    }

    fn close_buffer(&mut self, idx: usize) {
        if self.active_buffer.is_none() {
            return;
        }
        self.buffers.remove(idx);
        if !self.buffers.is_empty() {
            self.active_buffer = Some(self.buffers.len() - 1);
        }
    }

    fn next_buffer(&mut self) {
        let nb_buffers = self.buffers.len();
        if let Some(active) = self.active_buffer
            && nb_buffers > 1
        {
            self.active_buffer = Some((active + 1) % nb_buffers);
        }
    }

    fn prev_buffer(&mut self) {
        let nb_buffer = self.buffers.len();
        if let Some(active) = self.active_buffer
            && nb_buffer > 1
        {
            self.active_buffer = Some((active + nb_buffer - 1) % nb_buffer);
        }
    }

    fn save_file(&mut self) -> Result<()> {
        if let Some(buf) = self.buf_mut() {
            buf.save()?;
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            EditorMode::Nav => self.handle_nav_mode_key(key),
            EditorMode::Command => self.handle_command_mode_key(key),
            EditorMode::Insert => self.handle_insert_mode_key(key),
            EditorMode::TreeNav => self.handle_tree_nav_key(key),
        }
    }

    fn handle_navigation_key(&mut self, key: KeyEvent) -> Result<()> {
        let buf = if let Some(buf) = self.buf_mut() {
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

    fn handle_nav_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('x') if key.modifiers == KeyModifiers::CONTROL => {
                self.show_tree = true;
                self.mode = EditorMode::TreeNav;
            }
            KeyCode::Char('n') if key.modifiers == KeyModifiers::CONTROL => {
                self.next_buffer();
            }
            KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
                self.prev_buffer();
            }
            KeyCode::Char('w') if key.modifiers == KeyModifiers::CONTROL => {
                if let Some(active_buffer) = self.active_buffer {
                    self.close_buffer(active_buffer);
                }
            }
            KeyCode::Down | KeyCode::Up | KeyCode::Left | KeyCode::Right => {
                self.handle_navigation_key(key)?;
            }
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            KeyCode::Char('i') => {
                self.mode = EditorMode::Insert;
            }
            KeyCode::Char(':') => {
                self.mode = EditorMode::Command;
                self.former_mode = EditorMode::Nav;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_insert_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        let buf = if let Some(buf) = self.buf_mut() {
            buf
        } else {
            return Ok(());
        };
        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Left | KeyCode::Right => {
                self.handle_navigation_key(key)?;
            }
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            KeyCode::Char(c) => {
                buf.insert_char(c);
            }
            KeyCode::Backspace => {
                buf.delete_char();
            }
            KeyCode::Enter => {
                buf.newline();
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
                self.command_str.clear();
            }
            KeyCode::Char(c) => {
                self.command_str.push(c);
            }
            KeyCode::Esc => {
                self.command_str.clear();
                self.mode = self.former_mode;
            }
            KeyCode::Backspace => {
                self.command_str.pop();
            }
            KeyCode::Enter => {
                let cmd = self.command_str.clone();
                match cmd.as_str() {
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
                    "x" => {
                        self.show_tree = true;
                        self.mode = if self.former_mode == EditorMode::TreeNav
                            && self.active_buffer.is_some()
                        {
                            EditorMode::Nav
                        } else {
                            EditorMode::TreeNav
                        };
                        self.former_mode = self.mode;
                        self.command_str.clear();
                        return Ok(());
                    }
                    "bd" | "close" => {
                        if let Some(i) = self.active_buffer
                            && let Some(buf) = self.buf_mut()
                        {
                            buf.save()?;
                            self.close_buffer(i);
                        }
                    }
                    "bn" | "next" => {
                        self.next_buffer();
                    }
                    "bp" | "prev" => {
                        self.prev_buffer();
                    }
                    str => {
                        if let Ok(line) = str.parse::<usize>()
                            && let Some(buf) = self.buf_mut()
                        {
                            buf.jump_to_line(line);
                        }
                    }
                }
                self.command_str.clear();
                if self.mode != EditorMode::TreeNav {
                    self.mode = EditorMode::Nav;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_tree_nav_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up => self.file_tree.move_up(),
            KeyCode::Down => self.file_tree.move_down(),
            KeyCode::Left => self.file_tree.collapse_selected(),
            KeyCode::Right => self.file_tree.expand_selected(),
            KeyCode::Enter => {
                if let Some(path) = self.file_tree.enter() {
                    self.open_file(&path)?;
                    self.mode = EditorMode::Nav;
                }
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Nav;
            }
            KeyCode::Char(':') => {
                self.mode = EditorMode::Command;
                self.former_mode = EditorMode::TreeNav;
            }
            KeyCode::Char('b') if key.modifiers == KeyModifiers::CONTROL => {
                self.show_tree = false;
                self.mode = EditorMode::Nav;
            }
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Result<()> {
        let buf = if let Some(buf) = self.buf_mut() {
            buf
        } else {
            return Ok(());
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
        Ok(())
    }
}
