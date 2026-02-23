use anyhow::Result;

use crate::editor::Editor;

mod buffer;
mod completion;
mod diagnostic;
mod displayer;
mod editor;
mod highlighter;
mod lsp;
mod mode;
mod tree;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let filename = if args.len() > 1 {
        args[1].as_str()
    } else {
        eprintln!(
            "Usage: {} <file or folder>",
            args.first().map(|s| s.as_str()).unwrap_or("editor")
        );
        std::process::exit(1);
    };
    Editor::new(filename)?.run()
}
