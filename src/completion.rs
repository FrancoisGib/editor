use serde_json::Value;

#[derive(Clone)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub kind: String,
    pub insert_text: String,
    pub raw: Value,
}

pub struct CompletionState {
    pub items: Vec<CompletionItem>,
    pub selected: usize,
    pub prefix: String,
    pub request_id: Option<i64>,
    pub doc: Option<String>,
    pub resolve_id: Option<(i64, usize)>,
}

impl CompletionState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            prefix: String::new(),
            request_id: None,
            doc: None,
            resolve_id: None,
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.selected = 0;
        self.prefix.clear();
        self.request_id = None;
        self.doc = None;
        self.resolve_id = None;
    }

    pub fn is_active(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn move_down(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
            self.doc = None;
        }
    }

    pub fn move_up(&mut self) {
        if !self.items.is_empty() {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
            self.doc = None;
        }
    }

    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected)
    }

    pub fn filter(&mut self, prefix: &str) {
        self.prefix = prefix.to_string();
        let lower = prefix.to_lowercase();
        self.items.retain(|item| {
            item.label.to_lowercase().contains(&lower)
                || item.insert_text.to_lowercase().contains(&lower)
        });
        if self.items.is_empty() {
            self.clear();
        } else {
            self.selected = self.selected.min(self.items.len().saturating_sub(1));
        }
    }
}

fn parse_completion_kind(kind_num: u64) -> &'static str {
    match kind_num {
        1 => "txt",
        2 => "meth",
        3 => "fn",
        4 => "ctor",
        5 => "field",
        6 => "var",
        7 => "class",
        8 => "iface",
        9 => "mod",
        10 => "prop",
        11 => "unit",
        12 => "val",
        13 => "enum",
        14 => "kw",
        15 => "snip",
        16 => "color",
        17 => "file",
        21 => "const",
        22 => "struct",
        23 => "event",
        24 => "op",
        25 => "type",
        _ => "?",
    }
}

pub fn parse_completions(response: &Value) -> Vec<CompletionItem> {
    let raw_items = if let Some(arr) = response.get("result").and_then(|r| r.as_array()) {
        arr.clone()
    } else if let Some(arr) = response
        .get("result")
        .and_then(|r| r.get("items"))
        .and_then(|i| i.as_array())
    {
        arr.clone()
    } else {
        return Vec::new();
    };

    raw_items
        .iter()
        .filter_map(|item| {
            let label = item.get("label")?.as_str()?.to_string();
            let detail = item
                .get("detail")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());
            let kind_num = item.get("kind").and_then(|k| k.as_u64()).unwrap_or(0);
            let kind = parse_completion_kind(kind_num).to_string();
            let insert_text = item
                .get("insertText")
                .and_then(|t| t.as_str())
                .or_else(|| {
                    item.get("textEdit")
                        .and_then(|te| te.get("newText"))
                        .and_then(|t| t.as_str())
                })
                .unwrap_or(&label)
                .to_string()
                .replace("$0", "")
                .replace("${0:()}", "()")
                .replace("$1", "");
            Some(CompletionItem {
                label,
                detail,
                kind,
                insert_text,
                raw: item.clone(),
            })
        })
        .collect()
}

pub fn parse_resolve_doc(response: &Value) -> Option<String> {
    response
        .get("result")
        .and_then(|r| r.get("documentation"))
        .and_then(|d| {
            d.as_str().map(|s| s.to_string()).or_else(|| {
                d.get("value")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
        })
}
