use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};
use std::collections::HashMap;
use tree_sitter as ts;

struct Theme {
    styles: HashMap<&'static str, Style>,
    default: Style,
}

impl Theme {
    fn vscode_dark_modern() -> Self {
        let mut styles = HashMap::new();

        // ── VS Code Dark Modern — faithful palette ──────────────────────
        let keyword = Color::Rgb(86, 156, 214); // #569CD6  — keywords
        let control = Color::Rgb(197, 134, 192); // #C586C0  — control flow
        let function = Color::Rgb(220, 220, 170); // #DCDCAA  — functions / macros
        let type_c = Color::Rgb(78, 201, 176); // #4EC9B0  — types / traits
        let string = Color::Rgb(206, 145, 120); // #CE9178  — strings
        let escape = Color::Rgb(215, 186, 125); // #D7BA7D  — escape sequences
        let number = Color::Rgb(181, 206, 168); // #B5CEA8  — numeric literals
        let comment = Color::Rgb(106, 153, 85); // #6A9955  — comments (italic)
        let doc_com = Color::Rgb(127, 178, 103); // #7FB267  — doc comments (brighter green)
        let variable = Color::Rgb(156, 220, 254); // #9CDCFE  — variables / params
        let constant = Color::Rgb(79, 193, 255); // #4FC1FF  — constants / enum variants
        let attribute = Color::Rgb(156, 220, 254); // #9CDCFE  — attributes
        let lifetime = Color::Rgb(86, 156, 214); // #569CD6  — lifetimes
        let punct = Color::Rgb(212, 212, 212); // #D4D4D4  — punctuation / operators
        let default = Color::Rgb(212, 212, 212); // #D4D4D4  — plain text
        let namespace = Color::Rgb(78, 201, 176); // #4EC9B0  — modules / crates

        // Helper — VS Code Dark Modern uses NO bold for most tokens
        let s = |fg: Color| Style::default().fg(fg);
        let si = |fg: Color| Style::default().fg(fg).add_modifier(Modifier::ITALIC);

        // ── Keywords ────────────────────────────────────────────────────
        for kw in [
            "fn", "let", "use", "mod", "pub", "struct", "enum", "impl", "trait", "where", "as",
            "in", "ref", "const", "static", "type", "unsafe", "async", "await", "move", "dyn",
            "crate", "super", "extern", "mut",
        ] {
            styles.insert(kw, s(keyword));
        }

        // ── Control flow ────────────────────────────────────────────────
        for kw in [
            "if", "else", "match", "for", "while", "loop", "return", "break", "continue",
        ] {
            styles.insert(kw, s(control));
        }

        // ── Tree-sitter node kinds ──────────────────────────────────────

        // Generic keyword fallback
        styles.insert("keyword", s(keyword));

        // Functions
        styles.insert("function", s(function));
        styles.insert("function.method", s(function));

        // Types
        styles.insert("type", s(type_c));
        styles.insert("type_identifier", s(type_c));
        styles.insert("primitive_type", s(type_c));
        styles.insert("scoped_type_identifier", s(type_c));
        styles.insert("generic_type", s(type_c));

        // Strings
        styles.insert("string_literal", s(string));
        styles.insert("string_content", s(string));
        styles.insert("char_literal", s(string));
        styles.insert("raw_string_literal", s(string));
        styles.insert("escape_sequence", s(escape));

        // Numbers
        styles.insert("integer_literal", s(number));
        styles.insert("float_literal", s(number));

        // Booleans — VS Code treats true/false as keyword-blue
        styles.insert("boolean_literal", s(keyword));
        styles.insert("true", s(keyword));
        styles.insert("false", s(keyword));

        // Comments — italic, no bold
        styles.insert("line_comment", si(comment));
        styles.insert("block_comment", si(comment));
        styles.insert("doc_comment", si(doc_com));

        // Identifiers
        styles.insert("identifier", s(variable));
        styles.insert("field_identifier", s(variable));
        styles.insert("shorthand_field_identifier", s(variable));

        // Constants / enum variants
        styles.insert("enum_variant", s(constant));

        // Attributes
        styles.insert("attribute_item", si(attribute));
        styles.insert("inner_attribute_item", si(attribute));
        styles.insert("attribute", si(attribute));

        // Macros
        styles.insert("macro_invocation", s(function));
        styles.insert("macro_definition", s(function));

        // Lifetimes
        styles.insert("lifetime", s(lifetime));

        // self / Self
        styles.insert("self", s(keyword));
        styles.insert("metavariable", s(keyword));

        // mut specifier
        styles.insert("mutable_specifier", s(keyword));

        // Modules / namespaces
        styles.insert("scoped_identifier", s(namespace));

        // Operators / punctuation — same as default text
        styles.insert("operator", s(punct));
        styles.insert("::", s(punct));
        styles.insert("->", s(punct));
        styles.insert("=>", s(punct));
        styles.insert("&", s(punct));
        styles.insert("*", s(punct));
        styles.insert("!", s(punct));

        // Punctuation brackets
        styles.insert("(", s(punct));
        styles.insert(")", s(punct));
        styles.insert("{", s(punct));
        styles.insert("}", s(punct));
        styles.insert("[", s(punct));
        styles.insert("]", s(punct));
        styles.insert("<", s(punct));
        styles.insert(">", s(punct));
        styles.insert(",", s(punct));
        styles.insert(";", s(punct));
        styles.insert(":", s(punct));
        styles.insert(".", s(punct));

        Self {
            styles,
            default: Style::default().fg(default),
        }
    }

    fn style_for(&self, node_kind: &str) -> Style {
        self.styles.get(node_kind).copied().unwrap_or(self.default)
    }
}

pub struct Highlighter {
    parser: ts::Parser,
    tree: Option<ts::Tree>,
    theme: Theme,
    source_cache: String,
}

impl Highlighter {
    pub fn new() -> Self {
        let mut parser = ts::Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        parser
            .set_language(&language.into())
            .expect("Error loading Rust grammar");

        Self {
            parser,
            tree: None,
            theme: Theme::vscode_dark_modern(),
            source_cache: String::new(),
        }
    }

    pub fn update(&mut self, source: &str) {
        self.source_cache = source.to_string();
        self.tree = self.parser.parse(source, None);
    }

    pub fn highlight_line(&self, line_idx: usize, line_text: &str) -> Vec<Span<'static>> {
        let Some(tree) = &self.tree else {
            return vec![Span::raw(line_text.to_string())];
        };

        let mut spans: Vec<(usize, usize, Style)> = Vec::new();
        self.collect_leaf_styles(tree.root_node(), line_idx, &mut spans);

        if spans.is_empty() {
            return vec![Span::styled(line_text.to_string(), self.theme.default)];
        }

        spans.sort_by_key(|(start, _, _)| *start);

        let len = line_text.len();
        let mut result = Vec::new();
        let mut pos = 0;

        for (start, end, style) in &spans {
            let s = (*start).min(len);
            let e = (*end).min(len);

            if s > pos {
                result.push(Span::styled(
                    line_text[pos..s].to_string(),
                    self.theme.default,
                ));
            }
            if e > s && s >= pos {
                result.push(Span::styled(line_text[s..e].to_string(), *style));
                pos = e;
            }
        }

        if pos < len {
            result.push(Span::styled(
                line_text[pos..].to_string(),
                self.theme.default,
            ));
        }

        if result.is_empty() {
            vec![Span::styled(line_text.to_string(), self.theme.default)]
        } else {
            result
        }
    }

    /// Resolve the style for a leaf node with semantic context from its parent.
    ///
    /// This is the key to getting close to VS Code's behaviour: tree-sitter
    /// gives us syntax nodes, but the *meaning* often depends on where the
    /// identifier sits in the tree (function name vs variable vs type …).
    fn resolve_semantic_style(&self, node: ts::Node) -> Style {
        let kind = node.kind();

        // 1. Direct match on the node kind (keywords, literals, comments …)
        if let Some(&s) = self.theme.styles.get(kind) {
            // For "identifier" we want to fall through to semantic checks
            if kind == "identifier" {
                // fall through
            } else if kind == "line_comment" || kind == "block_comment" {
                // Distinguish doc comments (/// //! /** /*!)
                let text = node.utf8_text(self.source_cache.as_bytes()).unwrap_or("");
                if text.starts_with("///")
                    || text.starts_with("//!")
                    || text.starts_with("/**")
                    || text.starts_with("/*!")
                {
                    return self.theme.style_for("doc_comment");
                }
                return s;
            } else {
                return s;
            }
        }

        // 2. Semantic disambiguation via parent (and grandparent)
        if let Some(parent) = node.parent() {
            let pk = parent.kind();
            let gp_kind = parent.parent().map(|gp| gp.kind());

            // Helper: is this node the "name" child that sits right before
            // an argument list inside a call_expression?
            let is_call_target = |p: &str, gp: Option<&str>| -> bool {
                // Direct: call_expression > identifier
                if p == "call_expression" {
                    return true;
                }
                // Scoped: call_expression > scoped_identifier > identifier "new"
                //   e.g. Vec::new(), String::from()
                if p == "scoped_identifier" && gp == Some("call_expression") {
                    // Only the last segment (the function name)
                    return node.next_sibling().is_none();
                }
                // Generic: call_expression > generic_function > identifier
                if p == "generic_function" && gp == Some("call_expression") {
                    return true;
                }
                if p == "generic_function" {
                    return true;
                }
                false
            };

            match (pk, kind) {
                // ── Function definitions ────────────────────────────
                ("function_item", "identifier") => {
                    return self.theme.style_for("function");
                }

                // ── Function / method calls ─────────────────────────
                (_, "identifier") if is_call_target(pk, gp_kind) => {
                    return self.theme.style_for("function");
                }

                // ── Macro name (before the `!`) ────────────────────
                ("macro_invocation", "identifier") => {
                    return self.theme.style_for("macro_invocation");
                }

                // ── Methods via field_expression ────────────────────
                // e.g. foo.bar()  →  call_expression > field_expression > field_identifier "bar"
                // e.g. foo.bar    →  field_expression > field_identifier "bar"  (field access)
                ("field_expression", "field_identifier") => {
                    if gp_kind == Some("call_expression") {
                        return self.theme.style_for("function");
                    }
                    return self.theme.style_for("field_identifier");
                }

                // ── Types ──────────────────────────────────────────
                ("type_identifier", _)
                | ("scoped_type_identifier", "identifier")
                | ("struct_item", "type_identifier")
                | ("enum_item", "type_identifier")
                | ("impl_item", "type_identifier")
                | ("trait_item", "type_identifier")
                | ("type_arguments", "type_identifier")
                | ("function_item", "type_identifier") => {
                    return self.theme.style_for("type");
                }

                // use declarations — colour path segments as namespace/type
                ("use_declaration", _)
                | ("use_as_clause", "identifier")
                | ("scoped_use_list", "identifier")
                | ("use_wildcard", _)
                | ("use_list", "identifier") => {
                    return self.theme.style_for("type");
                }

                // Scoped identifiers: std::io::Result, MyEnum::Variant
                // If inside a call_expression the last segment was already
                // caught above; otherwise treat as type/namespace.
                ("scoped_identifier", "identifier") => {
                    return self.theme.style_for("type");
                }

                // ── Parameters ─────────────────────────────────────
                ("parameter", "identifier") | ("closure_parameters", "identifier") => {
                    return self.theme.style_for("identifier");
                }

                _ => {}
            }
        }

        // 3. Fallback — known identifier → variable colour, else default
        if kind == "identifier" {
            return self.theme.style_for("identifier");
        }

        self.theme.default
    }

    fn collect_leaf_styles(
        &self,
        node: ts::Node,
        line_idx: usize,
        spans: &mut Vec<(usize, usize, Style)>,
    ) {
        let start_line = node.start_position().row;
        let end_line = node.end_position().row;

        // Prune branches that don't touch this line
        if end_line < line_idx || start_line > line_idx {
            return;
        }

        let kind = node.kind();

        if node.child_count() == 0 || kind == "line_comment" || kind == "block_comment" {
            let style = self.resolve_semantic_style(node);

            if node.start_position().row == line_idx {
                let start_col = node.start_position().column;
                let end_col = if node.end_position().row == line_idx {
                    node.end_position().column
                } else {
                    usize::MAX
                };
                spans.push((start_col, end_col, style));
            } else if node.end_position().row == line_idx {
                spans.push((0, node.end_position().column, style));
            } else {
                spans.push((0, usize::MAX, style));
            }
        } else {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.collect_leaf_styles(child, line_idx, spans);
            }
        }
    }
}
