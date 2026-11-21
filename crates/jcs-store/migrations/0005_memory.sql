-- Phase 4: code memory.
--
-- Three layers that answer different questions:
--   * nodes/edges: "what is this symbol and what does it touch?" (structure)
--   * chunks:      "show me the code around this idea" (retrieval unit)
--   * FTS:         "find it by words" (lexical recall, no external service)
--
-- Embeddings are stored as raw f32 bytes in `memory_chunks`. They are optional: without a
-- provider the system still answers from structure and BM25, and says so.

CREATE TABLE memory_files (
    project_id   TEXT NOT NULL,
    path         TEXT NOT NULL,
    language     TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    size_bytes   INTEGER NOT NULL,
    indexed_at   TEXT NOT NULL,
    PRIMARY KEY (project_id, path)
);

CREATE TABLE memory_nodes (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    path       TEXT NOT NULL,
    language   TEXT NOT NULL,
    kind       TEXT NOT NULL,
    name       TEXT NOT NULL,
    qualified  TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line   INTEGER NOT NULL,
    detail     TEXT
);
CREATE INDEX idx_memory_nodes_path ON memory_nodes(project_id, path);
CREATE INDEX idx_memory_nodes_name ON memory_nodes(project_id, name);
CREATE INDEX idx_memory_nodes_qualified ON memory_nodes(project_id, qualified);

CREATE TABLE memory_edges (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    src_path   TEXT NOT NULL,
    src_name   TEXT,
    dst_name   TEXT NOT NULL,
    kind       TEXT NOT NULL
);
CREATE INDEX idx_memory_edges_src ON memory_edges(project_id, src_path);
CREATE INDEX idx_memory_edges_dst ON memory_edges(project_id, dst_name);

CREATE TABLE memory_chunks (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id      TEXT NOT NULL,
    path            TEXT NOT NULL,
    start_line      INTEGER NOT NULL,
    end_line        INTEGER NOT NULL,
    content         TEXT NOT NULL,
    content_hash    TEXT NOT NULL,
    embedding       BLOB,
    embedding_model TEXT
);
CREATE INDEX idx_memory_chunks_path ON memory_chunks(project_id, path);
CREATE INDEX idx_memory_chunks_hash ON memory_chunks(project_id, content_hash);

-- Standalone FTS5 table; `rowid` is the chunk id, so search joins by id.
CREATE VIRTUAL TABLE memory_fts USING fts5(
    content,
    path UNINDEXED,
    project_id UNINDEXED,
    tokenize = 'unicode61'
);
