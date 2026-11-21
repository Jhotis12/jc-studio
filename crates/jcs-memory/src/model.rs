//! Memory data model.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Languages with a real parser. SQL is handled separately because a grammar is not
/// needed to read DDL: tables, indexes and references are line-oriented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Sql,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "rs" => Some(Self::Rust),
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "py" => Some(Self::Python),
            "sql" => Some(Self::Sql),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Sql => "sql",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "rust" => Some(Self::Rust),
            "typescript" => Some(Self::TypeScript),
            "javascript" => Some(Self::JavaScript),
            "python" => Some(Self::Python),
            "sql" => Some(Self::Sql),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Class,
    Interface,
    Type,
    Module,
    Constant,
    Test,
    Table,
    Index,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Type => "type",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::Test => "test",
            Self::Table => "table",
            Self::Index => "index",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "function" => Self::Function,
            "struct" => Self::Struct,
            "enum" => Self::Enum,
            "trait" => Self::Trait,
            "class" => Self::Class,
            "interface" => Self::Interface,
            "type" => Self::Type,
            "module" => Self::Module,
            "constant" => Self::Constant,
            "test" => Self::Test,
            "table" => Self::Table,
            "index" => Self::Index,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Import,
    Call,
    References,
    Tests,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Call => "call",
            Self::References => "references",
            Self::Tests => "tests",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "import" => Self::Import,
            "call" => Self::Call,
            "references" => Self::References,
            "tests" => Self::Tests,
            _ => return None,
        })
    }
}

/// A definition found in a file. Lines are 1-based and inclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub path: String,
    pub language: Language,
    pub kind: SymbolKind,
    pub name: String,
    /// Name including its container, e.g. `Widget::render`.
    pub qualified: String,
    pub start_line: u32,
    pub end_line: u32,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub src_path: String,
    pub src_name: Option<String>,
    pub dst_name: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub content: String,
    pub content_hash: String,
}

/// Everything extracted from one file.
#[derive(Debug, Clone, Default)]
pub struct Extraction {
    pub symbols: Vec<Symbol>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexReport {
    pub files_seen: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub files_removed: usize,
    pub symbols: usize,
    pub chunks: usize,
    pub embeddings: usize,
    pub duration_ms: i64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitSource {
    Lexical,
    Semantic,
    Graph,
    Fallback,
}

impl HitSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Semantic => "semantic",
            Self::Graph => "graph",
            Self::Fallback => "fallback",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub chunk_id: i64,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub score: f32,
    pub source: HitSource,
    pub snippet: String,
    pub content: String,
}

impl SearchHit {
    /// The citation form used everywhere: `path:start-end`.
    pub fn citation(&self) -> String {
        format!("{}:{}-{}", self.path, self.start_line, self.end_line)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    pub files: i64,
    pub symbols: i64,
    pub chunks: i64,
    pub embedded_chunks: i64,
    pub edges: i64,
    pub last_indexed_at: Option<String>,
}

/// Files larger than this are indexed only as a chunk, never parsed.
pub const MAX_PARSED_BYTES: u64 = 512 * 1024;
/// Hard cap on chunks per file, so a generated 10k-line file cannot flood the index.
pub const MAX_CHUNKS_PER_FILE: usize = 120;
/// Maximum lines in one chunk.
pub const MAX_CHUNK_LINES: u32 = 120;
