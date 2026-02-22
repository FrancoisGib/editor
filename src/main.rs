use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    layout::{Constraint, Direction, Layout, Position},
    prelude::CrosstermBackend,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use ropey::Rope;

struct Editor {
    text: Rope,
    cursor_x: usize,
    cursor_y: usize,
    filename: String,
    _filepath: PathBuf,
    should_quit: bool,
    scroll_y: usize,
}

impl Editor {
    fn new(filename: &str) -> Self {
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
        }
    }

    fn insert(&mut self, c: char) {
        let pos = self.line_start() + self.cursor_x;
        self.text.insert_char(pos, c);
        self.cursor_x += 1;
    }

    fn line_start(&self) -> usize {
        self.text.line_to_char(self.cursor_y)
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            KeyCode::Down => {
                if self.text.len_lines() - 1 > self.cursor_y {
                    self.cursor_y += 1;
                }
            }
            KeyCode::Up => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                }
            }
            KeyCode::Char(c) => {
                self.insert(c);
            }
            _ => {}
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let filename = if args.len() > 1 {
        args[1].as_str()
    } else {
        eprintln!(
            "Usage: {} <fichier>",
            args.first().map(|s| s.as_str()).unwrap_or("editor")
        );
        std::process::exit(1);
    };

    enable_raw_mode()?;

    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut editor = Editor::new(filename);

    loop {
        terminal.draw(|f| {
            let size = f.area();
            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(size);

            let status = Line::from(vec![
                Span::styled(
                    format!(" {} ", editor.filename),
                    Style::default().fg(Color::Black).bg(Color::White),
                ),
                Span::raw(format!(
                    "  L:{} C:{}",
                    editor.cursor_y + 1,
                    editor.cursor_x + 1
                )),
            ]);
            f.render_widget(Paragraph::new(status), vertical[1]);

            let main_h = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                .split(vertical[0]);

            let editor_h = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(6), Constraint::Min(1)])
                .split(main_h[0]);

            let visible_height = editor_h[1].height as usize - 2;

            if editor.cursor_y >= visible_height {
                editor.scroll_y = editor.cursor_y - visible_height + 1;
            } else {
                editor.scroll_y = 0;
            }

            let mut line_nums: Vec<Line> = (editor.scroll_y
                ..editor
                    .text
                    .len_lines()
                    .min(editor.scroll_y + visible_height))
                .map(|i| {
                    Line::from(Span::styled(
                        format!("{:>4} ", i),
                        Style::default().fg(Color::DarkGray),
                    ))
                })
                .collect();

            line_nums.insert(0, Line::from(""));

            f.render_widget(Paragraph::new(line_nums), editor_h[0]);

            let text: Vec<Line> = editor
                .text
                .lines()
                .skip(editor.scroll_y)
                .take(visible_height)
                .map(|l| Line::from(l.to_string()))
                .collect();

            f.render_widget(
                Paragraph::new(text).block(
                    Block::default()
                        .title(format!(" {} ", editor.filename))
                        .borders(Borders::ALL),
                ),
                editor_h[1],
            );

            let cursor_pos = Position::new(
                editor.cursor_x as u16 + 7,
                (editor.cursor_y - editor.scroll_y) as u16 + 1,
            );

            f.set_cursor_position(cursor_pos);
        })?;

        if editor.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    editor.handle_key(key)?;
                }
                Event::Mouse(_mouse) => {}
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    Ok(())
}
