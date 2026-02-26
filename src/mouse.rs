use anyhow::Result;
use crossterm::event::{MouseEvent, MouseEventKind};

use crate::editor::Editor;

#[derive(Debug, Clone)]
pub struct MouseConfig {
    pub scroll_lines: usize,
}

impl Default for MouseConfig {
    fn default() -> Self {
        Self { scroll_lines: 3 }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MouseHandler {
    pub config: MouseConfig,
}

impl MouseHandler {
    pub fn new(config: MouseConfig) -> Self {
        Self { config }
    }

    pub fn handle_mouse(&self, event: MouseEvent, editor: &mut Editor) -> Result<()> {
        match event.kind {
            MouseEventKind::ScrollUp => {
                if let Some(buf) = editor.buf_mut() {
                    buf.move_up(self.config.scroll_lines);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(buf) = editor.buf_mut() {
                    buf.move_down(self.config.scroll_lines);
                }
            }
            MouseEventKind::Down(button) if button.is_left() => {
                if let Some(buf) = editor.buf_mut() {
                    buf.cursor_y = event.row as usize + buf.scroll_y - 2; // -2 equals top of window
                    // TODO: set cursor x with the area in displayer and replace -2 with const
                }
            }
            // MouseEventKind::Down(MouseButton::Left) => {
            // }
            _ => {}
        }
        Ok(())
    }
}
