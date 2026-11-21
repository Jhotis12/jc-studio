//! Code memory for JC Studio.
//!
//! Three layers, each answering a different question:
//!   * **graph** (`memory_nodes`/`memory_edges`): what is defined, and what touches it;
//!   * **chunks + FTS5**: show me the code around an idea, and find it by words;
//!   * **embeddings** (optional): find it even when the words differ.
//!
//! Everything is project-scoped and incremental by content hash, so indexing can run after
//! every merge without re-parsing the world.

pub mod chunk;
pub mod context;
pub mod embeddings;
pub mod error;
pub mod indexer;
pub mod languages;
pub mod model;
pub mod search;
pub mod util;

pub use context::{build as build_context, ContextOptions, ContextPackage};
pub use embeddings::Embeddings;
pub use error::{MemoryError, Result};
pub use indexer::{clear_project, index_project, stats};
pub use model::{
    Edge, EdgeKind, HitSource, IndexReport, Language, MemoryStats, SearchHit, Symbol, SymbolKind,
};
pub use search::search;
