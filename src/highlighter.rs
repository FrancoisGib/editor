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

        // VS Code Dark Modern palette
        let keyword = Color::Rgb(86, 156, 214); // #569CD6
        let control = Color::Rgb(197, 134, 192); // #C586C0
        let function = Color::Rgb(220, 220, 170); // #DCDCAA
        let type_color = Color::Rgb(78, 201, 176); // #4EC9B0
        let string = Color::Rgb(206, 145, 120); // #CE9178
        let number = Color::Rgb(181, 206, 168); // #B5CEA8
        let comment = Color::Rgb(106, 153, 85); // #6A9955
        let variable = Color::Rgb(156, 220, 254); // #9CDCFE
        let constant = Color::Rgb(100, 150, 224); // #4FC1FF
        let attribute = Color::Rgb(156, 220, 254); // #9CDCFE
        let macro_c = Color::Rgb(220, 220, 170); // #DCDCAA
        let lifetime = Color::Rgb(86, 156, 214); // #569CD6
        let operator = Color::Rgb(212, 212, 212); // #D4D4D4
        let default = Color::Rgb(212, 212, 212); // #D4D4D4

        // let s  = |fg: Color| Style::default().fg(fg);
        let s = |fg: Color| Style::default().fg(fg).add_modifier(Modifier::BOLD);

        for kw in [
            "fn", "let", "use", "mod", "pub", "struct", "enum", "impl", "trait", "where", "as",
            "in", "ref", "const", "static", "type", "unsafe", "async", "await", "move", "dyn",
            "crate", "super", "extern",
        ] {
            styles.insert(kw, s(keyword));
        }

        for kw in [
            "if", "else", "match", "for", "while", "loop", "return", "break", "continue", "mut",
        ] {
            styles.insert(kw, s(control));
        }

        // Tree-sitter node types
        styles.insert("keyword", s(keyword));

        // Fonctions
        styles.insert("function", s(function));
        styles.insert("function.method", s(function));

        // Types
        styles.insert("type", s(type_color));
        styles.insert("type_identifier", s(type_color));
        styles.insert("primitive_type", s(type_color));

        // Strings
        styles.insert("string_literal", s(string));
        styles.insert("string_content", s(string));
        styles.insert("char_literal", s(string));
        styles.insert("raw_string_literal", s(string));

        // Numbers
        styles.insert("integer_literal", s(number));
        styles.insert("float_literal", s(number));

        // Booleans
        styles.insert("boolean_literal", s(constant));
        styles.insert("true", s(constant));
        styles.insert("false", s(constant));

        // Comments
        styles.insert("line_comment", s(comment));
        styles.insert("block_comment", s(comment));

        // Identifiers
        styles.insert("identifier", s(variable));
        styles.insert("field_identifier", s(variable));

        // Attributes
        styles.insert("attribute_item", s(attribute));
        styles.insert("inner_attribute_item", s(attribute));

        // Macros
        styles.insert("macro_invocation", s(macro_c));

        // Lifetimes
        styles.insert("lifetime", s(lifetime));

        // self
        styles.insert("self", s(keyword));
        styles.insert("mutable_specifier", s(control));

        // operators
        styles.insert("operator", s(operator));
        styles.insert("::", s(operator));
        styles.insert("->", s(operator));
        styles.insert("=>", s(operator));

        Self {
            styles,
            default: Style::default().fg(default),
        }
    }

    fn _monokai() -> Self {
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
