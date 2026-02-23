use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, MouseEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};

use crate::{buffer::Buffer, displayer::Displayer, mode::EditorMode, tree::FileTree};

pub struct Editor {
    pub buffers: Vec<Buffer>,
    pub active_buffer: Option<usize>,
    pub should_quit: bool,
    pub mode: EditorMode,
    pub file_tree: FileTree,
    pub show_tree: bool,
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
            file_tree: FileTree::new(&project_dir),
            show_tree: true,
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

    pub fn buf_mut(&mut self) -> Option<&mut Buffer> {
        self.active_buffer
            .and_then(|active_buffer| self.buffers.get_mut(active_buffer))
    }

    pub fn open_file(&mut self, path: &Path) -> Result<()> {
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

    pub fn close_buffer(&mut self, idx: usize) {
        if self.active_buffer.is_none() {
            return;
        }
        self.buffers.remove(idx);
        if !self.buffers.is_empty() {
            self.active_buffer = Some(self.buffers.len() - 1);
        }
    }

    pub fn next_buffer(&mut self) {
        let nb_buffers = self.buffers.len();
        if let Some(active) = self.active_buffer
            && nb_buffers > 1
        {
            self.active_buffer = Some((active + 1) % nb_buffers);
        }
    }

    pub fn prev_buffer(&mut self) {
        let nb_buffer = self.buffers.len();
        if let Some(active) = self.active_buffer
            && nb_buffer > 1
        {
            self.active_buffer = Some((active + nb_buffer - 1) % nb_buffer);
        }
    }

    pub fn save_file(&mut self) -> Result<()> {
        if let Some(buf) = self.buf_mut() {
            buf.save()?;
        }
        Ok(())
    }

    pub fn insert_char(&mut self, c: char) {
        if let Some(buf) = self.buf_mut() {
            buf.insert_char(c);
        }
    }

    pub fn delete_char(&mut self) {
        if let Some(buf) = self.buf_mut() {
            buf.delete_char();
        }
    }

    pub fn insert_newline(&mut self) {
        if let Some(buf) = self.buf_mut() {
            buf.newline();
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        let mode = std::mem::replace(&mut self.mode, EditorMode::Nav);
        self.mode = mode.handle_key(key, self)?;
        Ok(())
    }

    fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Result<()> {
        let mode = std::mem::replace(&mut self.mode, EditorMode::Nav);
        self.mode = mode.handle_mouse(mouse_event, self)?;
        Ok(())
    }
}
