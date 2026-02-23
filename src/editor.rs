use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
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

use crate::{
    buffer::Buffer,
    completion::{CompletionState, parse_completions, parse_resolve_doc},
    diagnostic::{DiagnosticState, spawn_cargo_check},
    displayer::Displayer,
    lsp::LspClient,
    mode::EditorMode,
    tree::FileTree,
};

pub struct Editor {
    pub buffers: Vec<Buffer>,
    pub active_buffer: Option<usize>,
    pub should_quit: bool,
    pub mode: EditorMode,
    pub file_tree: FileTree,
    pub show_tree: bool,
    pub diag_state: Arc<Mutex<DiagnosticState>>,
    pub lsp: Option<LspClient>,
    pub completion: CompletionState,
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

        let diag_state = Arc::new(Mutex::new(DiagnosticState::new()));

        let mut lsp = LspClient::start(&canon_path, &project_dir);
        if let Some(ref mut client) = lsp {
            client.initialize();
            if !canon_path.is_dir() {
                let text = buffers
                    .first()
                    .map(|b| b.text.to_string())
                    .unwrap_or_default();
                client.did_open(&format!("file://{}", canon_path.display()), &text);
            }
        }

        if !canon_path.is_dir() {
            spawn_cargo_check(&diag_state, &canon_path);
        }

        Ok(Self {
            buffers,
            active_buffer,
            should_quit: false,
            mode,
            file_tree: FileTree::new(&project_dir),
            show_tree: true,
            diag_state,
            lsp,
            completion: CompletionState::new(),
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

            self.poll_completion();

            let vh = displayer.viewport_height();
            if let Some(buf) = self.buf_mut() {
                buf.compute_scroll(vh);
            }

            displayer.draw(&self)?;

            if event::poll(Duration::from_millis(50))? {
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

    pub fn diag_snapshot(&self) -> DiagnosticState {
        self.diag_state
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| DiagnosticState::new())
    }

    pub fn run_check(&self) {
        if let Some(buf) = self.buf()
            && let Some(path) = &buf.filepath
        {
            spawn_cargo_check(&self.diag_state, path);
        }
    }

    pub fn save_and_check(&mut self) -> Result<()> {
        self.save_file()?;
        self.run_check();
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

        self.notify_lsp_open(&canon);
        spawn_cargo_check(&self.diag_state, &canon);

        Ok(())
    }

    pub fn close_buffer(&mut self, idx: usize) {
        if self.active_buffer.is_none() {
            return;
        }
        self.buffers.remove(idx);
        if !self.buffers.is_empty() {
            self.active_buffer = Some(self.buffers.len() - 1);
        } else {
            self.active_buffer = None;
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
        self.notify_lsp_change();
    }

    pub fn delete_char(&mut self) {
        if let Some(buf) = self.buf_mut() {
            buf.delete_char();
        }
        self.notify_lsp_change();
    }

    pub fn insert_newline(&mut self) {
        if let Some(buf) = self.buf_mut() {
            buf.newline();
        }
        self.notify_lsp_change();
    }

    pub fn notify_lsp_change(&mut self) {
        if let Some(ref mut client) = self.lsp
            && let Some(buf) = self.buffers.get(self.active_buffer.unwrap_or(0))
        {
            client.did_change(&buf.text.to_string());
        }
    }

    pub fn notify_lsp_open(&mut self, path: &Path) {
        let mut lsp = self.lsp.take();
        if let Some(client) = &mut lsp
            && let Some(buf) = self.buf()
        {
            let uri = format!("file://{}", path.display());
            client.did_open(&uri, &buf.text.to_string());
        }
        self.lsp = lsp;
    }

    pub fn trigger_completion(&mut self) {
        let mut lsp = self.lsp.take();
        if let Some(client) = &mut lsp
            && let Some(buf) = self.buf()
        {
            let id = client.request_completion(buf.cursor_y, buf.cursor_x);
            self.completion.request_id = Some(id);
            self.completion.prefix = self.get_word_before_cursor();
        }
        self.lsp = lsp;
    }

    pub fn request_resolve(&mut self) {
        if let Some(item) = self.completion.selected_item()
            && let Some(ref mut client) = self.lsp
        {
            let id = client.resolve_completion(&item.raw);
            self.completion.resolve_id = Some((id, self.completion.selected));
            self.completion.doc = None;
        }
    }

    pub fn get_word_before_cursor(&self) -> String {
        let Some(buf) = self.buf() else {
            return String::new();
        };
        if buf.cursor_y >= buf.text.len_lines() {
            return String::new();
        }
        let line: String = buf
            .text
            .line(buf.cursor_y)
            .chars()
            .take(buf.cursor_x)
            .collect();
        line.chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn apply_completion(&mut self) {
        let Some(item) = self.completion.selected_item().cloned() else {
            return;
        };

        let prefix_len = self.completion.prefix.len();

        if let Some(buf) = self.buf_mut() {
            for _ in 0..prefix_len {
                if buf.cursor_x > 0 {
                    let pos = buf.text.line_to_char(buf.cursor_y) + buf.cursor_x;
                    buf.text.remove(pos - 1..pos);
                    buf.cursor_x -= 1;
                }
            }
            let pos = buf.text.line_to_char(buf.cursor_y) + buf.cursor_x;
            buf.text.insert(pos, &item.insert_text);
            buf.cursor_x += item.insert_text.len();
            buf.on_text_changed();
        }

        self.completion.clear();
        self.notify_lsp_change();
    }

    pub fn poll_completion(&mut self) {
        if let Some((resolve_id, idx)) = self.completion.resolve_id
            && let Some(ref client) = self.lsp
            && let Some(resp) = client.get_response(resolve_id)
        {
            self.completion.resolve_id = None;
            if idx == self.completion.selected {
                self.completion.doc = parse_resolve_doc(&resp);
            }
        }

        let Some(req_id) = self.completion.request_id else {
            return;
        };
        let resp = if let Some(ref client) = self.lsp {
            client.get_response(req_id)
        } else {
            None
        };

        if let Some(resp) = resp {
            self.completion.request_id = None;
            let items = parse_completions(&resp);
            if items.is_empty() {
                self.completion.clear();
            } else {
                self.completion.items = items;
                self.completion.selected = 0;
                self.completion.doc = None;
                self.mode = EditorMode::Autocomplete;
                self.request_resolve();
            }
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
