use std::io::Stdout;

use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Direction, Layout, Position, Rect},
    prelude::CrosstermBackend,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    diagnostic::{DiagnosticLevel, DiagnosticState},
    editor::Editor,
    mode::EditorMode,
};

pub struct Displayer {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Displayer {
    pub fn new(terminal: Terminal<CrosstermBackend<Stdout>>) -> Self {
        Self { terminal }
    }

    pub fn backend_mut(&mut self) -> &mut CrosstermBackend<Stdout> {
        self.terminal.backend_mut()
    }

    pub fn draw(&mut self, editor: &Editor) -> anyhow::Result<()> {
        let is_cursor_visible = editor.mode == EditorMode::Nav
            || editor.mode == EditorMode::Insert
            || editor.mode == EditorMode::Autocomplete;
        if is_cursor_visible {
            self.terminal.show_cursor()?;
        } else {
            self.terminal.hide_cursor()?;
        }

        let diag = editor.diag_snapshot();

        self.terminal.draw(|f| {
            let size = f.area();

            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(size);

            Self::render_tab_bar(editor, f, vertical[0]);
            Self::render_status(editor, &diag, f, vertical[2]);

            let main_h = if editor.show_tree {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Length(25),
                        Constraint::Min(1),
                        Constraint::Percentage(30),
                    ])
                    .split(vertical[1])
            } else {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(1), Constraint::Percentage(30)])
                    .split(vertical[1])
            };

            if editor.show_tree {
                editor.file_tree.render(f, main_h[0]);
            }

            let editor_area = if editor.show_tree {
                main_h[1]
            } else {
                main_h[0]
            };
            let side_panel = if editor.show_tree {
                main_h[2]
            } else {
                main_h[1]
            };

            Self::render_editor(editor, &diag, f, editor_area, is_cursor_visible);

            if editor.completion.is_active() && editor.completion.doc.is_some() {
                Self::render_completion_doc(editor, f, side_panel);
            } else {
                Self::render_diagnostics(&diag, f, side_panel);
            }
        })?;

        Ok(())
    }

    pub fn viewport_height(&self) -> usize {
        let size = self.terminal.size().unwrap_or_default();
        size.height.saturating_sub(4) as usize // tab + borders + status
    }

    fn render_editor(
        editor: &Editor,
        diag: &DiagnosticState,
        f: &mut Frame,
        area: Rect,
        show_cursor: bool,
    ) {
        let visible_height = area.height.saturating_sub(2) as usize;
        let Some(buf) = editor.buf() else { return };

        let lines: Vec<Line> = (buf.scroll_y
            ..buf.text.len_lines().min(buf.scroll_y + visible_height))
            .map(|i| {
                let has_err = diag
                    .diagnostics
                    .iter()
                    .any(|d| d.line == Some(i) && d.level == DiagnosticLevel::Error);
                let has_warn = diag
                    .diagnostics
                    .iter()
                    .any(|d| d.line == Some(i) && d.level == DiagnosticLevel::Warning);
                let num_color = if has_err {
                    Color::Red
                } else if has_warn {
                    Color::Yellow
                } else {
                    Color::DarkGray
                };

                let num = Span::styled(format!("{:>4} │ ", i), Style::default().fg(num_color));

                let mut text = buf.text.line(i).to_string();
                if text.ends_with('\n') {
                    text.pop();
                }

                let mut spans = vec![num];
                spans.extend(buf.highlighter.highlight_line(i, &text));

                Line::from(spans)
            })
            .collect();

        f.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .title(format!(" {} ", buf.display_name()))
                    .borders(Borders::ALL),
            ),
            area,
        );

        if show_cursor {
            let gutter_width: u16 = 7;
            let cursor_x = buf.cursor_x as u16 + gutter_width + area.x + 1;
            let cursor_y = (buf.cursor_y - buf.scroll_y) as u16 + area.y + 1;
            f.set_cursor_position(Position::new(cursor_x, cursor_y));
        }

        Self::render_completion(editor, f, area);
    }

    fn render_tab_bar(editor: &Editor, f: &mut Frame, rect: Rect) {
        let mut spans: Vec<Span> = Vec::new();
        for (i, buf) in editor.buffers.iter().enumerate() {
            let is_active = editor.active_buffer.map(|ab| ab == i).unwrap_or(false);
            let style = if is_active {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(format!(" {} ", buf.display_name()), style));
            spans.push(Span::raw("│"));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), rect);
    }

    fn render_status(editor: &Editor, diag: &DiagnosticState, f: &mut Frame, rect: Rect) {
        let mut components = if let Some(buf) = editor.buf()
            && let Some(active_buffer) = editor.active_buffer
        {
            let diag_info = if diag.is_running {
                Span::styled(" [checking...] ", Style::default().fg(Color::Gray))
            } else {
                let e = diag.error_count();
                let w = diag.warning_count();
                if e > 0 || w > 0 {
                    Span::styled(
                        format!(" [E:{} W:{}] ", e, w),
                        Style::default().fg(if e > 0 { Color::Red } else { Color::Yellow }),
                    )
                } else {
                    Span::styled(" [✓] ", Style::default().fg(Color::Green))
                }
            };

            vec![
                Span::styled(format!(" {} ", editor.mode), editor.mode.get_style()),
                Span::styled(
                    format!(" {} ", buf.display_name()),
                    Style::default().fg(Color::Black).bg(Color::White),
                ),
                Span::raw(format!("  {}:{} ", buf.cursor_y + 1, buf.cursor_x + 1)),
                Span::raw(format!(
                    " [{}/{}] ",
                    active_buffer + 1,
                    editor.buffers.len()
                )),
                diag_info,
            ]
        } else {
            vec![Span::styled(
                format!(" {} ", editor.mode),
                editor.mode.get_style(),
            )]
        };

        if let EditorMode::Command { command_str, .. } = &editor.mode {
            components.push(Span::raw(format!(" :{} ", command_str)));
        }

        f.render_widget(Paragraph::new(Line::from(components)), rect);
    }

    fn render_diagnostics(diag: &DiagnosticState, f: &mut Frame, area: Rect) {
        let title = if diag.is_running {
            " Diagnostics (checking...) "
        } else if diag.diagnostics.is_empty() {
            " Diagnostics ✓ "
        } else {
            " Diagnostics "
        };

        let mut lines: Vec<Line> = Vec::new();

        if diag.is_running {
            lines.push(Line::from(Span::styled(
                "⟳ Running cargo check...",
                Style::default().fg(Color::Gray),
            )));
        } else if diag.diagnostics.is_empty() {
            lines.push(Line::from(Span::styled(
                "✓ No errors or warnings",
                Style::default().fg(Color::Green),
            )));
        } else {
            let e = diag.error_count();
            let w = diag.warning_count();
            let mut summary = Vec::new();
            if e > 0 {
                summary.push(Span::styled(
                    format!(" {} error{} ", e, if e > 1 { "s" } else { "" }),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
            }
            if w > 0 {
                summary.push(Span::styled(
                    format!(" {} warning{} ", w, if w > 1 { "s" } else { "" }),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(summary));
            lines.push(Line::from(
                "─".repeat(area.width.saturating_sub(2) as usize),
            ));

            for d in &diag.diagnostics {
                let (icon, color) = match d.level {
                    DiagnosticLevel::Error => ("✗", Color::Red),
                    DiagnosticLevel::Warning => ("▲", Color::Yellow),
                };
                let loc = match (d.line, d.column) {
                    (Some(l), Some(c)) => format!(" L{}:{}", l, c),
                    (Some(l), None) => format!(" L{}", l),
                    _ => String::new(),
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", icon), Style::default().fg(color)),
                    Span::styled(loc, Style::default().fg(Color::DarkGray)),
                ]));

                let max_w = area.width.saturating_sub(4) as usize;
                if max_w > 0 {
                    for chunk in d
                        .message
                        .chars()
                        .collect::<Vec<_>>()
                        .chunks(max_w)
                        .map(|c| c.iter().collect::<String>())
                    {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", chunk),
                            Style::default().fg(color),
                        )));
                    }
                }
                lines.push(Line::from(""));
            }
        }

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        f.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(ratatui::widgets::Wrap { trim: false }),
            area,
        );
    }

    fn render_completion(editor: &Editor, f: &mut Frame, editor_area: Rect) {
        use ratatui::widgets::Clear;

        if !editor.completion.is_active() {
            return;
        }
        let Some(buf) = editor.buf() else { return };

        let max_visible = 10usize;
        let count = editor.completion.items.len().min(max_visible);

        let gutter: u16 = 7;
        let cursor_screen_y = (buf.cursor_y.saturating_sub(buf.scroll_y)) as u16;
        let popup_x = (buf.cursor_x as u16 + gutter + editor_area.x + 1)
            .min(editor_area.right().saturating_sub(40));
        let popup_y = cursor_screen_y + editor_area.y + 2;

        let max_label = editor
            .completion
            .items
            .iter()
            .take(max_visible)
            .map(|c| c.label.len() + c.kind.len() + 4)
            .max()
            .unwrap_or(20)
            .clamp(20, 40);

        let popup_w = (max_label + 2) as u16;
        let popup_h = count as u16 + 2;

        let x = popup_x.min(editor_area.right().saturating_sub(popup_w));
        let y = if popup_y + popup_h > editor_area.bottom() {
            cursor_screen_y + editor_area.y - popup_h
        } else {
            popup_y
        };

        let area = Rect::new(x, y, popup_w, popup_h);

        let scroll_start = editor
            .completion
            .selected
            .saturating_sub(max_visible - 1)
            .max(if editor.completion.selected >= max_visible {
                editor.completion.selected - max_visible + 1
            } else {
                0
            });

        let items: Vec<Line> = editor
            .completion
            .items
            .iter()
            .skip(scroll_start)
            .take(max_visible)
            .enumerate()
            .map(|(i, item)| {
                let real_idx = scroll_start + i;
                let is_sel = real_idx == editor.completion.selected;
                let kind = Span::styled(
                    format!(" {:>5} ", item.kind),
                    Style::default().fg(Color::DarkGray),
                );
                let label = Span::styled(
                    item.label.clone(),
                    if is_sel {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                );
                let mut spans = vec![kind, Span::raw(" "), label];
                if is_sel && let Some(ref detail) = item.detail {
                    let short: String = detail.chars().take(20).collect();
                    spans.push(Span::styled(
                        format!(" {}", short),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                Line::from(spans)
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Rgb(30, 30, 30)));

        f.render_widget(Clear, area);
        f.render_widget(Paragraph::new(items).block(block), area);
    }

    fn render_completion_doc(editor: &Editor, f: &mut Frame, area: Rect) {
        let Some(ref doc) = editor.completion.doc else {
            return;
        };
        let Some(item) = editor.completion.selected_item() else {
            return;
        };

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(vec![
            Span::styled(
                &item.label,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({})", item.kind),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        if let Some(ref detail) = item.detail {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                detail.clone(),
                Style::default().fg(Color::Yellow),
            )));
        }

        lines.push(Line::from(
            "─".repeat(area.width.saturating_sub(2) as usize),
        ));

        let mut md_text = tui_markdown::from_str(doc);

        for line in &mut md_text.lines {
            for span in &mut line.spans {
                span.style = span.style.bg(Color::Reset);

                if span.style.add_modifier.contains(Modifier::BOLD) {
                    span.style = span.style.fg(Color::Cyan);
                }
            }
        }

        lines.extend(md_text.lines);

        let block = Block::default()
            .title(" Documentation ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        f.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(ratatui::widgets::Wrap { trim: true }),
            area,
        );
    }
}
