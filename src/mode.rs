use std::fmt::Display;

use ratatui::style::{Color, Style};

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
