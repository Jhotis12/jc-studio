//! Symbol and edge extraction.
//!
//! Parsing is best-effort by design: a file that fails to parse is skipped with a warning
//! instead of failing the index. The index is a navigation aid, not a compiler.

use tree_sitter::{Node, Parser};

use crate::error::{MemoryError, Result};
use crate::model::{Edge, EdgeKind, Extraction, Language, Symbol, SymbolKind};

pub fn extract(language: Language, path: &str, source: &str) -> Result<Extraction> {
    match language {
        Language::Rust => extract_rust(path, source),
        Language::TypeScript => extract_typescript(path, source, false),
        Language::JavaScript => extract_typescript(path, source, true),
        Language::Python => extract_python(path, source),
        Language::Sql => Ok(extract_sql(path, source)),
    }
}

fn text_of(node: Node<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or_default()
        .to_string()
}

fn field_text(node: Node<'_>, source: &str, field: &str) -> Option<String> {
    node.child_by_field_name(field)
        .map(|child| text_of(child, source))
}

fn qualified(parent: Option<&str>, name: &str) -> String {
    match parent {
        Some(parent) if !parent.is_empty() => format!("{parent}::{name}"),
        _ => name.to_string(),
    }
}

fn symbol(
    path: &str,
    language: Language,
    kind: SymbolKind,
    name: &str,
    parent: Option<&str>,
    node: Node<'_>,
    detail: Option<String>,
) -> Symbol {
    Symbol {
        path: path.to_string(),
        language,
        kind,
        name: name.to_string(),
        qualified: qualified(parent, name),
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        detail,
    }
}

fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    if line.chars().count() > 160 {
        line.chars().take(160).collect::<String>() + "…"
    } else {
        line.to_string()
    }
}

/// The last segment of a callee expression: `foo::bar::Baz::new` -> `new`.
fn callee_name(text: &str) -> String {
    let cleaned = text.trim();
    if cleaned.is_empty() {
        return String::new();
    }
    let after_scope = cleaned.rsplit("::").next().unwrap_or(cleaned);
    let after_dot = after_scope.rsplit('.').next().unwrap_or(after_scope);
    after_dot
        .split('(')
        .next()
        .unwrap_or(after_dot)
        .trim()
        .to_string()
}

// ---------------------------------------------------------------- Rust

fn extract_rust(path: &str, source: &str) -> Result<Extraction> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|error| MemoryError::Parse {
            path: path.to_string(),
            reason: format!("cannot load the Rust grammar: {error}"),
        })?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| MemoryError::Parse {
            path: path.to_string(),
            reason: "the parser produced no tree".to_string(),
        })?;

    let mut extraction = Extraction::default();
    walk_rust(tree.root_node(), source, path, None, &mut extraction);
    Ok(extraction)
}

fn walk_rust(
    node: Node<'_>,
    source: &str,
    path: &str,
    context: Option<&str>,
    out: &mut Extraction,
) {
    let kind = node.kind();
    let mut next_context = context.map(str::to_string);

    match kind {
        "use_declaration" => {
            let raw = text_of(node, source);
            let target = raw
                .trim_start_matches("use ")
                .trim_end_matches(';')
                .trim()
                .to_string();
            if !target.is_empty() {
                out.edges.push(Edge {
                    src_path: path.to_string(),
                    src_name: context.map(str::to_string),
                    dst_name: target,
                    kind: EdgeKind::Import,
                });
            }
            return;
        }
        "mod_item" => {
            if let Some(name) = field_text(node, source, "name") {
                out.symbols.push(symbol(
                    path,
                    Language::Rust,
                    SymbolKind::Module,
                    &name,
                    context,
                    node,
                    None,
                ));
                next_context = Some(qualified(context, &name));
            }
        }
        "struct_item" | "union_item" => {
            push_named(path, source, &node, context, SymbolKind::Struct, out);
            if let Some(name) = field_text(node, source, "name") {
                next_context = Some(qualified(context, &name));
            }
        }
        "enum_item" => {
            push_named(path, source, &node, context, SymbolKind::Enum, out);
            if let Some(name) = field_text(node, source, "name") {
                next_context = Some(qualified(context, &name));
            }
        }
        "trait_item" => {
            push_named(path, source, &node, context, SymbolKind::Trait, out);
            if let Some(name) = field_text(node, source, "name") {
                next_context = Some(qualified(context, &name));
            }
        }
        "const_item" | "static_item" => {
            push_named(path, source, &node, context, SymbolKind::Constant, out);
        }
        "type_item" => {
            push_named(path, source, &node, context, SymbolKind::Type, out);
        }
        "impl_item" => {
            if let Some(type_name) = field_text(node, source, "type") {
                next_context = Some(type_name);
            }
        }
        "function_item" => {
            let name = field_text(node, source, "name").unwrap_or_else(|| "<anonymous>".into());
            let is_test = preceded_by_test_attribute(node, source);
            let kind = if is_test {
                SymbolKind::Test
            } else {
                SymbolKind::Function
            };
            out.symbols.push(symbol(
                path,
                Language::Rust,
                kind,
                &name,
                context,
                node,
                Some(first_line(&text_of(node, source))),
            ));
            next_context = Some(qualified(context, &name));
        }
        "call_expression" => {
            if let Some(callee) = node.child_by_field_name("function") {
                let name = callee_name(&text_of(callee, source));
                if !name.is_empty() {
                    out.edges.push(Edge {
                        src_path: path.to_string(),
                        src_name: context.map(str::to_string),
                        dst_name: name,
                        kind: EdgeKind::Call,
                    });
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_rust(child, source, path, next_context.as_deref(), out);
    }
}

fn push_named(
    path: &str,
    source: &str,
    node: &Node<'_>,
    context: Option<&str>,
    kind: SymbolKind,
    out: &mut Extraction,
) {
    if let Some(name) = field_text(*node, source, "name") {
        out.symbols.push(symbol(
            path,
            Language::Rust,
            kind,
            &name,
            context,
            *node,
            None,
        ));
    }
}

fn preceded_by_test_attribute(node: Node<'_>, source: &str) -> bool {
    let row = node.start_position().row;
    let lines: Vec<&str> = source.lines().collect();
    let start = row.saturating_sub(4);
    lines[start..row].iter().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("#[test]")
            || trimmed.starts_with("#[tokio::test")
            || trimmed.starts_with("#[async_std::test")
    })
}

// ------------------------------------------------- TypeScript / JavaScript

fn extract_typescript(path: &str, source: &str, javascript: bool) -> Result<Extraction> {
    let mut parser = Parser::new();

    let language = if javascript {
        tree_sitter_javascript::LANGUAGE.into()
    } else if path.ends_with(".tsx") || path.ends_with(".jsx") {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };

    parser
        .set_language(&language)
        .map_err(|error| MemoryError::Parse {
            path: path.to_string(),
            reason: format!("cannot load the TypeScript grammar: {error}"),
        })?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| MemoryError::Parse {
            path: path.to_string(),
            reason: "the parser produced no tree".to_string(),
        })?;

    let mut extraction = Extraction::default();
    let language = if javascript {
        Language::JavaScript
    } else {
        Language::TypeScript
    };
    walk_js(
        tree.root_node(),
        source,
        path,
        language,
        None,
        &mut extraction,
    );
    Ok(extraction)
}

fn walk_js(
    node: Node<'_>,
    source: &str,
    path: &str,
    language: Language,
    context: Option<&str>,
    out: &mut Extraction,
) {
    let kind = node.kind();
    let mut next_context = context.map(str::to_string);

    match kind {
        "import_statement" => {
            let raw = text_of(node, source);
            let target = raw
                .split("from")
                .nth(1)
                .unwrap_or_default()
                .trim()
                .trim_matches(|c: char| c == '\'' || c == '"' || c == ';')
                .to_string();
            if !target.is_empty() {
                out.edges.push(Edge {
                    src_path: path.to_string(),
                    src_name: context.map(str::to_string),
                    dst_name: target,
                    kind: EdgeKind::Import,
                });
            }
            return;
        }
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name) = field_text(node, source, "name") {
                out.symbols.push(symbol(
                    path,
                    language,
                    SymbolKind::Function,
                    &name,
                    context,
                    node,
                    Some(first_line(&text_of(node, source))),
                ));
                next_context = Some(qualified(context, &name));
            }
        }
        "class_declaration" | "abstract_class_declaration" => {
            if let Some(name) = field_text(node, source, "name") {
                out.symbols.push(symbol(
                    path,
                    language,
                    SymbolKind::Class,
                    &name,
                    context,
                    node,
                    None,
                ));
                next_context = Some(qualified(context, &name));
            }
        }
        "interface_declaration" => {
            if let Some(name) = field_text(node, source, "name") {
                out.symbols.push(symbol(
                    path,
                    language,
                    SymbolKind::Interface,
                    &name,
                    context,
                    node,
                    None,
                ));
            }
        }
        "type_alias_declaration" => {
            if let Some(name) = field_text(node, source, "name") {
                out.symbols.push(symbol(
                    path,
                    language,
                    SymbolKind::Type,
                    &name,
                    context,
                    node,
                    None,
                ));
            }
        }
        "enum_declaration" => {
            if let Some(name) = field_text(node, source, "name") {
                out.symbols.push(symbol(
                    path,
                    language,
                    SymbolKind::Enum,
                    &name,
                    context,
                    node,
                    None,
                ));
            }
        }
        "method_definition" => {
            if let Some(name) = field_text(node, source, "name") {
                out.symbols.push(symbol(
                    path,
                    language,
                    SymbolKind::Function,
                    &name,
                    context,
                    node,
                    Some(first_line(&text_of(node, source))),
                ));
            }
        }
        "variable_declarator" => {
            // `const render = () => ...` is a function to every reader.
            let value_is_function = node
                .child_by_field_name("value")
                .map(|value| matches!(value.kind(), "arrow_function" | "function_expression"))
                .unwrap_or(false);
            if value_is_function {
                if let Some(name) = field_text(node, source, "name") {
                    out.symbols.push(symbol(
                        path,
                        language,
                        SymbolKind::Function,
                        &name,
                        context,
                        node,
                        Some(first_line(&text_of(node, source))),
                    ));
                }
            }
        }
        "call_expression" => {
            if let Some(callee) = node.child_by_field_name("function") {
                let name = callee_name(&text_of(callee, source));
                if !name.is_empty() {
                    let is_test_call =
                        matches!(name.as_str(), "describe" | "it" | "test" | "specify");
                    out.edges.push(Edge {
                        src_path: path.to_string(),
                        src_name: context.map(str::to_string),
                        dst_name: name.clone(),
                        kind: if is_test_call {
                            EdgeKind::Tests
                        } else {
                            EdgeKind::Call
                        },
                    });
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_js(child, source, path, language, next_context.as_deref(), out);
    }
}

// ---------------------------------------------------------------- Python

fn extract_python(path: &str, source: &str) -> Result<Extraction> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|error| MemoryError::Parse {
            path: path.to_string(),
            reason: format!("cannot load the Python grammar: {error}"),
        })?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| MemoryError::Parse {
            path: path.to_string(),
            reason: "the parser produced no tree".to_string(),
        })?;

    let mut extraction = Extraction::default();
    walk_python(tree.root_node(), source, path, None, &mut extraction);
    Ok(extraction)
}

fn walk_python(
    node: Node<'_>,
    source: &str,
    path: &str,
    context: Option<&str>,
    out: &mut Extraction,
) {
    let kind = node.kind();
    let mut next_context = context.map(str::to_string);

    match kind {
        "import_statement" | "import_from_statement" => {
            let raw = text_of(node, source);
            let target = raw
                .trim_start_matches("from ")
                .trim_start_matches("import ")
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            if !target.is_empty() {
                out.edges.push(Edge {
                    src_path: path.to_string(),
                    src_name: context.map(str::to_string),
                    dst_name: target,
                    kind: EdgeKind::Import,
                });
            }
            return;
        }
        "class_definition" => {
            if let Some(name) = field_text(node, source, "name") {
                out.symbols.push(symbol(
                    path,
                    Language::Python,
                    SymbolKind::Class,
                    &name,
                    context,
                    node,
                    None,
                ));
                next_context = Some(qualified(context, &name));
            }
        }
        "function_definition" => {
            if let Some(name) = field_text(node, source, "name") {
                let is_test = name.starts_with("test_") || name.starts_with("test");
                out.symbols.push(symbol(
                    path,
                    Language::Python,
                    if is_test {
                        SymbolKind::Test
                    } else {
                        SymbolKind::Function
                    },
                    &name,
                    context,
                    node,
                    Some(first_line(&text_of(node, source))),
                ));
                next_context = Some(qualified(context, &name));
            }
        }
        "call" => {
            if let Some(callee) = node.child_by_field_name("function") {
                let name = callee_name(&text_of(callee, source));
                if !name.is_empty() {
                    out.edges.push(Edge {
                        src_path: path.to_string(),
                        src_name: context.map(str::to_string),
                        dst_name: name,
                        kind: EdgeKind::Call,
                    });
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_python(child, source, path, next_context.as_deref(), out);
    }
}

// ------------------------------------------------------------------- SQL

/// SQL is scanned line by line: DDL is regular enough that a full grammar would add a
/// dependency without adding information.
fn extract_sql(path: &str, source: &str) -> Extraction {
    let mut out = Extraction::default();
    let mut current_table: Option<String> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        let upper = line.to_ascii_uppercase();
        let line_number = index as u32 + 1;

        if upper.starts_with("CREATE TABLE") || upper.starts_with("ALTER TABLE") {
            if let Some(name) = identifier_after(&upper, line, "TABLE") {
                current_table = Some(name.clone());
                out.symbols.push(table_symbol(path, &name, line_number));
            }
        } else if upper.starts_with("CREATE INDEX") || upper.starts_with("CREATE UNIQUE INDEX") {
            let keyword = if upper.starts_with("CREATE UNIQUE INDEX") {
                "UNIQUE INDEX"
            } else {
                "INDEX"
            };
            if let Some(name) = identifier_after(&upper, line, keyword) {
                out.symbols.push(Symbol {
                    path: path.to_string(),
                    language: Language::Sql,
                    kind: SymbolKind::Index,
                    name: name.clone(),
                    qualified: name.clone(),
                    start_line: line_number,
                    end_line: line_number,
                    detail: Some(first_line(line)),
                });
            }
        } else if upper.contains("REFERENCES") {
            if let Some(position) = upper.find("REFERENCES") {
                let rest = &line[position + "REFERENCES".len()..];
                let target: String = rest
                    .trim()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !target.is_empty() {
                    out.edges.push(Edge {
                        src_path: path.to_string(),
                        src_name: current_table.clone(),
                        dst_name: target,
                        kind: EdgeKind::References,
                    });
                }
            }
        }

        if upper.starts_with(')') || upper.ends_with(';') {
            current_table = None;
        }
    }

    out
}

fn table_symbol(path: &str, name: &str, line: u32) -> Symbol {
    Symbol {
        path: path.to_string(),
        language: Language::Sql,
        kind: SymbolKind::Table,
        name: name.to_string(),
        qualified: name.to_string(),
        start_line: line,
        end_line: line,
        detail: None,
    }
}

/// Reads the first identifier after `keyword`, tolerating `IF NOT EXISTS`.
fn identifier_after(upper: &str, original: &str, keyword: &str) -> Option<String> {
    let position = upper.find(keyword)? + keyword.len();
    let rest = original.get(position..)?.trim_start();
    let rest = rest
        .strip_prefix("IF NOT EXISTS")
        .or_else(|| rest.strip_prefix("if not exists"))
        .unwrap_or(rest)
        .trim_start();

    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '"' || *c == '.')
        .collect();

    let name = name.trim_matches('"').to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_rust_fixture() -> Extraction {
        let source = r#"
use std::fmt;
use crate::store::Store;

pub struct Widget {
    pub name: String,
}

impl Widget {
    pub fn render(&self) -> String {
        format!("{}", self.name)
    }
}

#[test]
fn renders() {
    let widget = Widget { name: "x".into() };
    let _ = widget.render();
}
"#;
        extract(Language::Rust, "src/widget.rs", source).unwrap()
    }

    #[test]
    fn rust_symbols_include_methods_with_their_container() {
        let extraction = extract_rust_fixture();
        let names: Vec<&str> = extraction
            .symbols
            .iter()
            .map(|symbol| symbol.qualified.as_str())
            .collect();

        assert!(names.contains(&"Widget"), "{names:?}");
        assert!(names.contains(&"Widget::render"), "{names:?}");
        assert!(names.contains(&"renders"), "{names:?}");
    }

    #[test]
    fn rust_tests_are_recognised_by_their_attribute() {
        let extraction = extract_rust_fixture();
        let test = extraction
            .symbols
            .iter()
            .find(|symbol| symbol.name == "renders")
            .unwrap();
        assert_eq!(test.kind, SymbolKind::Test);
    }

    #[test]
    fn rust_imports_and_calls_are_edges() {
        let extraction = extract_rust_fixture();
        assert!(extraction
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Import && edge.dst_name.contains("std::fmt")));
        assert!(extraction.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Call
                && edge.dst_name == "render"
                && edge.src_name.as_deref() == Some("renders")
        }));
    }

    #[test]
    fn typescript_symbols_and_imports() {
        let source = r#"
import { api } from './api';
export interface User { id: string }
export type Id = string;
export class Service {
  handle(user: User): void { api(user); }
}
const render = (u: User) => u.id;
describe('service', () => { it('works', () => {}); });
"#;
        let extraction = extract(Language::TypeScript, "src/service.ts", source).unwrap();
        let names: Vec<&str> = extraction.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"User"), "{names:?}");
        assert!(names.contains(&"Id"), "{names:?}");
        assert!(names.contains(&"Service"), "{names:?}");
        assert!(names.contains(&"render"), "{names:?}");

        assert!(extraction
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Import && edge.dst_name == "./api"));
        assert!(extraction
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Tests && edge.dst_name == "describe"));
    }

    #[test]
    fn python_symbols_and_tests() {
        let source = r#"
import os
from app.models import User

class Service:
    def handle(self, user):
        return user

def test_handle():
    Service().handle(None)
"#;
        let extraction = extract(Language::Python, "app/service.py", source).unwrap();
        let names: Vec<&str> = extraction
            .symbols
            .iter()
            .map(|symbol| symbol.qualified.as_str())
            .collect();
        assert!(names.contains(&"Service"), "{names:?}");
        assert!(names.contains(&"Service::handle"), "{names:?}");
        assert!(names.contains(&"test_handle"), "{names:?}");

        let test = extraction
            .symbols
            .iter()
            .find(|symbol| symbol.name == "test_handle")
            .unwrap();
        assert_eq!(test.kind, SymbolKind::Test);
    }

    #[test]
    fn sql_tables_indexes_and_references() {
        let source = r#"
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    email TEXT NOT NULL
);
CREATE TABLE orders (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT REFERENCES users(id)
);
CREATE INDEX idx_orders_user ON orders(user_id);
"#;
        let extraction = extract(Language::Sql, "db/schema.sql", source).unwrap();
        let tables: Vec<&str> = extraction
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == SymbolKind::Table)
            .map(|symbol| symbol.name.as_str())
            .collect();
        assert!(tables.contains(&"users"), "{tables:?}");
        assert!(tables.contains(&"orders"), "{tables:?}");
        assert!(extraction
            .symbols
            .iter()
            .any(|symbol| symbol.kind == SymbolKind::Index && symbol.name == "idx_orders_user"));
        assert!(extraction
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::References && edge.dst_name == "users"));
    }

    #[test]
    fn callee_names_are_reduced_to_the_last_segment() {
        assert_eq!(callee_name("foo::bar::Baz::new"), "new");
        assert_eq!(callee_name("self.render"), "render");
        assert_eq!(callee_name("handler"), "handler");
        assert_eq!(callee_name(""), "");
    }
}
