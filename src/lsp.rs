use serde_json::Value;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub struct LspClient {
    _process: Child,
    writer: BufWriter<std::process::ChildStdin>,
    responses: Arc<Mutex<HashMap<i64, Value>>>,
    next_id: i64,
    uri: String,
    version: i64,
}

impl LspClient {
    pub fn start(filepath: &Path, project_dir: &Path) -> Option<Self> {
        let mut child = Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir(project_dir)
            .spawn()
            .ok()?;

        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let writer = BufWriter::new(stdin);
        let responses: Arc<Mutex<HashMap<i64, Value>>> = Arc::new(Mutex::new(HashMap::new()));

        let resp = Arc::clone(&responses);
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut content_length: usize = 0;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) == 0 {
                        return;
                    }
                    let header = header.trim();
                    if header.is_empty() {
                        break;
                    }
                    if let Some(len) = header.strip_prefix("Content-Length: ") {
                        content_length = len.parse().unwrap_or(0);
                    }
                }
                if content_length == 0 {
                    continue;
                }

                let mut body = vec![0u8; content_length];
                if reader.read_exact(&mut body).is_err() {
                    return;
                }
                let Ok(json) = serde_json::from_slice::<Value>(&body) else {
                    continue;
                };

                if let Some(id) = json.get("id").and_then(|i| i.as_i64())
                    && let Ok(mut map) = resp.lock()
                {
                    map.insert(id, json);
                }
            }
        });

        let uri = format!("file://{}", filepath.display());

        Some(Self {
            _process: child,
            writer,
            responses,
            next_id: 1,
            uri,
            version: 0,
        })
    }

    pub fn initialize(&mut self) {
        let id = self.send_request(
            "initialize",
            serde_json::json!({
                "processId": std::process::id(),
                "capabilities": {
                    "textDocument": {
                        "completion": {
                            "completionItem": {
                                "snippetSupport": false,
                                "resolveSupport": { "properties": ["detail", "documentation"] }
                            }
                        }
                    }
                },
                "rootUri": null,
            }),
        );
        self.wait_response(id, 10000);
        self.send_notification("initialized", serde_json::json!({}));
    }

    pub fn did_open(&mut self, uri: &str, text: &str) {
        self.uri = uri.to_string();
        self.version = 1;
        self.send_notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": self.uri,
                    "languageId": "rust",
                    "version": self.version,
                    "text": text,
                }
            }),
        );
    }

    pub fn did_change(&mut self, text: &str) {
        self.version += 1;
        self.send_notification(
            "textDocument/didChange",
            serde_json::json!({
                "textDocument": { "uri": self.uri, "version": self.version },
                "contentChanges": [{ "text": text }]
            }),
        );
    }

    pub fn request_completion(&mut self, line: usize, character: usize) -> i64 {
        self.send_request(
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": self.uri },
                "position": { "line": line, "character": character },
            }),
        )
    }

    pub fn resolve_completion(&mut self, item: &Value) -> i64 {
        self.send_request("completionItem/resolve", item.clone())
    }

    pub fn get_response(&self, id: i64) -> Option<Value> {
        self.responses.lock().ok()?.remove(&id)
    }

    fn send_request(&mut self, method: &str, params: Value) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.send_raw(&msg);
        id
    }

    fn send_notification(&mut self, method: &str, params: Value) {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.send_raw(&msg);
    }

    fn send_raw(&mut self, msg: &Value) {
        let body = serde_json::to_string(msg).unwrap();
        let _ = write!(
            self.writer,
            "Content-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = self.writer.flush();
    }

    fn wait_response(&self, id: i64, timeout_ms: u64) -> Option<Value> {
        let start = Instant::now();
        loop {
            if let Some(resp) = self.get_response(id) {
                return Some(resp);
            }
            if start.elapsed().as_millis() as u64 > timeout_ms {
                return None;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
