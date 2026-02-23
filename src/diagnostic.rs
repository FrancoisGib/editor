use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
};

#[derive(Clone, PartialEq)]
pub enum DiagnosticLevel {
    Warning,
    Error,
}

#[derive(Clone)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Clone)]
pub struct DiagnosticState {
    pub diagnostics: Vec<Diagnostic>,
    pub is_running: bool,
}

impl DiagnosticState {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            is_running: false,
        }
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Warning)
            .count()
    }
}

fn find_project_dir(file: &Path) -> Option<PathBuf> {
    let mut dir = file.parent().map(|p| p.to_path_buf());
    loop {
        match &dir {
            Some(d) => {
                if d.join("Cargo.toml").exists() {
                    return Some(d.clone());
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
            None => return None,
        }
    }
}

fn parse_diagnostics(
    output: &str,
    target_file: &Path,
    project_dir: Option<&Path>,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for line in output.lines() {
        let Ok(json) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if json.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }
        let Some(message) = json.get("message") else {
            continue;
        };

        let level = match message.get("level").and_then(|l| l.as_str()).unwrap_or("") {
            "error" => DiagnosticLevel::Error,
            "warning" => DiagnosticLevel::Warning,
            _ => continue,
        };

        let msg = message
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        let span = message
            .get("spans")
            .and_then(|s| s.as_array())
            .and_then(|s| s.first());

        let span_file = span
            .and_then(|s| s.get("file_name"))
            .and_then(|f| f.as_str());

        let matches = match span_file {
            Some(f) => {
                let resolved = if let Some(dir) = project_dir {
                    dir.join(f).canonicalize().ok()
                } else {
                    PathBuf::from(f).canonicalize().ok()
                };
                resolved.map_or_else(|| target_file.ends_with(f), |p| p == target_file)
            }
            None => level == DiagnosticLevel::Error,
        };

        if !matches {
            continue;
        }

        let (ln, col) = span
            .map(|s| {
                let l = s
                    .get("line_start")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                let c = s
                    .get("column_start")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                (l, c)
            })
            .unwrap_or((None, None));

        diags.push(Diagnostic {
            level,
            message: msg,
            line: ln.map(|l| l.saturating_sub(1)),
            column: col,
        });
    }

    diags
}

fn run_cargo_check(state: Arc<Mutex<DiagnosticState>>, file: PathBuf) {
    if let Ok(mut s) = state.lock() {
        s.is_running = true;
    }

    let project_dir = find_project_dir(&file);

    let mut cmd = Command::new("cargo");
    cmd.args(["clippy", "--message-format=json", "--color=never"]);
    if let Some(dir) = &project_dir {
        cmd.current_dir(dir);
    }

    let diags = match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            parse_diagnostics(&stdout, &file, project_dir.as_deref())
        }
        Err(e) => vec![Diagnostic {
            level: DiagnosticLevel::Error,
            message: format!("cargo check failed: {}", e),
            line: None,
            column: None,
        }],
    };

    if let Ok(mut s) = state.lock() {
        s.diagnostics = diags;
        s.is_running = false;
    }
}

pub fn spawn_cargo_check(state: &Arc<Mutex<DiagnosticState>>, file: &Path) {
    if let Ok(s) = state.lock()
        && s.is_running
    {
        return;
    }
    let state = Arc::clone(state);
    let file = file.to_path_buf();
    thread::spawn(move || run_cargo_check(state, file));
}
