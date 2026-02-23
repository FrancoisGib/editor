use std::fmt::Display;

use ratatui::style::{Color, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Nav,
    Command,
    Insert,
}

impl EditorMode {
    pub fn get_style(&self) -> Style {
        match self {
            EditorMode::Nav => Style::default().fg(Color::Cyan),
            EditorMode::Command => Style::default().fg(Color::Red),
            EditorMode::Insert => Style::default().fg(Color::Yellow),
        }
    }
}

impl Display for EditorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EditorMode::Nav => "NAV",
            EditorMode::Command => "COMMAND",
            EditorMode::Insert => "INSERT",
        })
    }
}
