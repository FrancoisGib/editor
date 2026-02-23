use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct FileEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    depth: usize,
    expanded: bool,
}

pub struct FileTree {
    entries: Vec<FileEntry>,
    selected: usize,
    scroll: usize,
}

impl FileTree {
    pub fn new(root: &Path) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let mut tree = Self {
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
        };

        tree.scan_dir(&root, 1);
        tree
    }

    fn scan_dir(&mut self, dir: &Path, depth: usize) {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.starts_with('.') || name == "target" {
                continue;
            }

            let is_dir = path.is_dir();
            let file_entry = FileEntry {
                path,
                name,
                is_dir,
                depth,
                expanded: false,
            };

            if is_dir {
                dirs.push(file_entry);
            } else {
                files.push(file_entry);
            }
        }

        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        for entry in dirs.into_iter().chain(files.into_iter()) {
            self.entries.push(entry);
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    pub fn enter(&mut self) -> Option<PathBuf> {
        let entry = &self.entries[self.selected];

        if entry.is_dir {
            if entry.expanded {
                self.collapse(self.selected);
            } else {
                self.expand(self.selected);
            }
            None
        } else {
            Some(entry.path.clone())
        }
    }

    fn expand(&mut self, idx: usize) {
        self.entries[idx].expanded = true;
        let dir = self.entries[idx].path.clone();
        let depth = self.entries[idx].depth + 1;

        let mut children = Vec::new();
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            return;
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            let is_dir = path.is_dir();
            let fe = FileEntry {
                path,
                name,
                is_dir,
                depth,
                expanded: false,
            };
            if is_dir {
                dirs.push(fe);
            } else {
                files.push(fe);
            }
        }

        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        children.extend(dirs);
        children.extend(files);

        let insert_pos = idx + 1;
        for (i, child) in children.into_iter().enumerate() {
            self.entries.insert(insert_pos + i, child);
        }
    }

    fn collapse(&mut self, idx: usize) {
        self.entries[idx].expanded = false;
        let depth = self.entries[idx].depth;

        let mut remove_count = 0;
        for i in (idx + 1)..self.entries.len() {
            if self.entries[i].depth > depth {
                remove_count += 1;
            } else {
                break;
            }
        }
        self.entries.drain((idx + 1)..(idx + 1 + remove_count));

        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;

        let scroll = if self.selected < self.scroll {
            self.selected
        } else if self.selected >= self.scroll + inner_height {
            self.selected - inner_height + 1
        } else {
            self.scroll
        };

        let lines: Vec<Line> = self
            .entries
            .iter()
            .enumerate()
            .skip(scroll)
            .take(inner_height)
            .map(|(i, entry)| {
                let indent = "  ".repeat(entry.depth);
                let icon = if entry.is_dir {
                    if entry.expanded { "▼ " } else { "▶ " }
                } else {
                    "  "
                };

                let name_style = if i == self.selected {
                    if entry.is_dir {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Black).bg(Color::White)
                    }
                } else if entry.is_dir {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                Line::from(vec![
                    Span::styled(indent, Style::default()),
                    Span::styled(icon, Style::default().fg(Color::DarkGray)),
                    Span::styled(&entry.name, name_style),
                ])
            })
            .collect();

        let block = Block::default()
            .title(" Files ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        f.render_widget(Paragraph::new(lines).block(block), area);
    }

    pub fn expand_selected(&mut self) {
        if self.entries[self.selected].is_dir && !self.entries[self.selected].expanded {
            self.expand(self.selected);
        }
    }

    pub fn collapse_selected(&mut self) {
        if self.entries[self.selected].is_dir && self.entries[self.selected].expanded {
            self.collapse(self.selected);
        } else {
            let depth = self.entries[self.selected].depth;
            if depth > 0 {
                for i in (0..self.selected).rev() {
                    if self.entries[i].is_dir && self.entries[i].depth < depth {
                        self.selected = i;
                        break;
                    }
                }
            }
        }
    }
}
