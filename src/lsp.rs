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
    server_requests: Arc<Mutex<Vec<Value>>>,
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
        let server_requests: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));

        let resp = Arc::clone(&responses);
        let srv_req = Arc::clone(&server_requests);
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

                let has_id = json.get("id").is_some();
                let has_method = json.get("method").is_some();
                let has_result_or_error =
                    json.get("result").is_some() || json.get("error").is_some();

                if has_id && has_result_or_error {
                    // Response to our request
                    if let Some(id) = json["id"].as_i64()
                        && let Ok(mut map) = resp.lock()
                    {
                        map.insert(id, json);
                    }
                } else if has_id && has_method {
                    // Server-initiated request (needs a reply)
                    if let Ok(mut reqs) = srv_req.lock() {
                        reqs.push(json);
                    }
                }
                // else: notification from server, ignore
            }
        });

        let uri = format!("file://{}", filepath.display());

        Some(Self {
            _process: child,
            writer,
            responses,
            server_requests,
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
                                "additionalTextEditsSupport": true,
                                "resolveSupport": { "properties": ["detail", "documentation", "additionalTextEdits"] }
                            }
                        }
                    },
                    "workspace": {
                        "configuration": true
                    }
                },
                "initializationOptions": {
                    "completion": {
                        "autoimport": {
                            "enable": true
                        }
                    }
                },
            }),
        );
        self.wait_response(id, 10000);
        self.send_notification("initialized", serde_json::json!({}));

        // After initialized, rust-analyzer will send workspace/configuration
        // requests. Process them.
        // thread::sleep(Duration::from_millis(100));
        // self.process_server_requests();
    }

    /// Handle pending server-initiated requests (workspace/configuration, etc.)
    pub fn process_server_requests(&mut self) {
        let reqs: Vec<Value> = {
            let mut srv = self.server_requests.lock().unwrap();
            std::mem::take(&mut *srv)
        };

        for req in reqs {
            // let method = req["method"].as_str().unwrap_or("");
            let id = &req["id"];
            self.send_response(id, Value::Null);

            // match method {
            //     "workspace/configuration" => {
            //         // Return our rust-analyzer config for each requested section
            //         let items = req["params"]["items"]
            //             .as_array()
            //             .map(|a| a.len())
            //             .unwrap_or(1);

            //         // Send back the config for each item requested
            //         let config = serde_json::json!({
            //             "completion": {
            //                 "autoimport": {
            //                     "enable": true
            //                 }
            //             }
            //         });

            //         let results: Vec<Value> = (0..items).map(|_| config.clone()).collect();
            //         self.send_response(id, serde_json::json!(results));
            //     }
            //     "client/registerCapability" => {
            //         // Just acknowledge
            //         self.send_response(id, Value::Null);
            //     }
            //     "window/workDoneProgress/create" => {
            //         self.send_response(id, Value::Null);
            //     }
            //     _ => {
            //         // Unknown request, respond with null
            //         self.send_response(id, Value::Null);
            //     }
            // }
        }
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
        // Process any pending server requests before completion
        self.process_server_requests();

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

    fn send_response(&mut self, id: &Value, result: Value) {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        });
        self.send_raw(&msg);
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

    pub fn wait_response(&self, id: i64, timeout_ms: u64) -> Option<Value> {
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
