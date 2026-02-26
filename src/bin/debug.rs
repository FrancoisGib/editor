use std::path::Path;

use text_editor::lsp::LspClient;

fn main() {
    // Point to your Cargo project root
    let project_dir = Path::new("/home/francois/foutoir/text-editor");

    let line: usize = 3;
    let character: usize = 19;

    // Write the content to a real file (rust-analyzer needs it on disk)
    let filepath = project_dir.join("src/bin/test.rs");
    // std::fs::write(&filepath, text).expect("Cannot write file");

    let text = std::fs::read_to_string(&filepath).unwrap();

    println!("Starting rust-analyzer...");
    let mut client =
        LspClient::start(&filepath, project_dir).expect("Failed to start rust-analyzer");

    client.initialize();

    println!("Waiting for rust-analyzer to index...");
    std::thread::sleep(std::time::Duration::from_secs(1));

    let file_uri = format!("file://{}", filepath.display());
    client.did_open(&file_uri, &text);
    std::thread::sleep(std::time::Duration::from_secs(1));

    println!("Requesting completion at line {line}, char {character}...");
    let req_id = client.request_completion(line, character);

    match client.wait_response(req_id, 15000) {
        Some(response) => {
            println!("{}\n\n\n", response);
            if let Some(result) = response.get("result") {
                let items = match result {
                    serde_json::Value::Array(arr) => arr.clone(),
                    obj if obj.get("items").is_some() => {
                        obj["items"].as_array().cloned().unwrap_or_default()
                    }
                    _ => vec![],
                };

                println!("\n=== {} completion(s) ===\n", items.len());
                for (i, item) in items.iter().enumerate().take(1) {
                    // println!("res {:?}\n\n", item);
                    // println!("{}", item);
                    let label = item["label"].as_str().unwrap_or("?");
                    let kind = item.get("kind").and_then(|k| k.as_i64()).unwrap_or(0);
                    let data = item.get("data").unwrap_or(&serde_json::Value::Null);
                    println!("d {:?}", data);
                    let detail = data.get("detail").and_then(|d| d.as_str()).unwrap_or("");
                    let imports = data.get("imports")
                    .and_then(|i| i.as_array())
                    .map(|i| i.first().unwrap_or(&serde_json::Value::Null))
                    .map(|i| i.get("full_import_path").and_then(|i| i.as_str()).unwrap_or(""))
                    .unwrap_or("");
                    // println!("i {:?}", imports);

                    let kind_str = match kind {
                        2 => "method",
                        3 => "function",
                        5 => "field",
                        6 => "variable",
                        9 => "module",
                        13 => "enum",
                        14 => "keyword",
                        22 => "struct",
                        _ => "other",
                    };

                    let doc_id = client.resolve_completion(item);
                    let doc = client.wait_response(doc_id, 15000);
                    println!("\n\ndoc {:?}", doc.unwrap().get("result").unwrap().get("documentation").unwrap());

                    println!("{:>3}. [{:<10}] {:<30} {} {:?}", i + 1, kind_str, label, detail, imports);
                }
            } else if let Some(err) = response.get("error") {
                eprintln!("LSP error: {}", err);
            }
        }
        None => eprintln!("Timeout: no response from rust-analyzer"),
    }
}
