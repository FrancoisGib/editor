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
    fn monokai() -> Self {
        let mut styles = HashMap::new();
        let s = |fg: Color| Style::default().fg(fg);
        let sb = |fg: Color| Style::default().fg(fg).add_modifier(Modifier::BOLD);

        for kw in [
            "fn", "let", "mut", "if", "else", "match", "for", "while", "loop", "return", "use",
            "mod", "pub", "struct", "enum", "impl", "trait", "where", "as", "in", "ref", "self",
            "super", "crate", "const", "static", "type", "unsafe", "async", "await", "move",
            "break", "continue", "dyn",
        ] {
            styles.insert(kw, sb(Color::Red));
        }

        styles.insert("keyword", sb(Color::Red));
        styles.insert("function", s(Color::Green));
        styles.insert("function.method", s(Color::Green));
        styles.insert("type", s(Color::Cyan));
        styles.insert("type_identifier", s(Color::Cyan));
        styles.insert("primitive_type", sb(Color::Cyan));
        styles.insert("string_literal", s(Color::Yellow));
        styles.insert("string_content", s(Color::Yellow));
        styles.insert("char_literal", s(Color::Yellow));
        styles.insert("integer_literal", s(Color::Magenta));
        styles.insert("float_literal", s(Color::Magenta));
        styles.insert("boolean_literal", sb(Color::Magenta));
        styles.insert("line_comment", s(Color::DarkGray));
        styles.insert("block_comment", s(Color::DarkGray));
        styles.insert("attribute_item", s(Color::LightBlue));
        styles.insert("macro_invocation", s(Color::LightRed));
        styles.insert("identifier", s(Color::White));
        styles.insert("field_identifier", s(Color::LightCyan));
        styles.insert("lifetime", s(Color::LightMagenta));
        styles.insert("mutable_specifier", sb(Color::Red));
        styles.insert("self", sb(Color::Red));
        styles.insert("true", sb(Color::Magenta));
        styles.insert("false", sb(Color::Magenta));

        Self {
            styles,
            default: Style::default().fg(Color::White),
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
            theme: Theme::monokai(),
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

    fn collect_leaf_styles(
        &self,
        node: ts::Node,
        line_idx: usize,
        spans: &mut Vec<(usize, usize, Style)>,
    ) {
        let start_line = node.start_position().row;
        let end_line = node.end_position().row;

        if end_line < line_idx || start_line > line_idx {
            return;
        }

        if node.child_count() == 0 {
            let kind = node.kind();

            let style = if let Some(s) = self.theme.styles.get(kind) {
                *s
            } else if let Some(parent) = node.parent() {
                match (parent.kind(), kind) {
                    ("function_item", "identifier") => self.theme.style_for("function"),
                    ("call_expression", "identifier") => self.theme.style_for("function"),
                    ("macro_invocation", "identifier") => self.theme.style_for("macro_invocation"),
                    ("field_expression", "field_identifier") => {
                        self.theme.style_for("field_identifier")
                    }
                    ("scoped_identifier", "identifier") => self.theme.style_for("type"),
                    ("use_declaration", _) => self.theme.style_for("type"),
                    _ => self.theme.default,
                }
            } else {
                self.theme.default
            };

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
