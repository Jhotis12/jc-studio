//! Indexing: walk, hash, parse, persist.
//!
//! Incremental by content hash, so a full sweep is cheap and can run on every promotion
//! without re-parsing the world. Every write for a file happens in one transaction, so a
//! crash cannot leave half a file's symbols behind.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use jcs_core::ProjectId;
use jcs_store::Store;

use crate::chunk::chunk_file;
use crate::embeddings::{encode_vector, Embeddings};
use crate::error::{MemoryError, Result};
use crate::languages::extract;
use crate::model::{Chunk, Extraction, IndexReport, Language, MemoryStats, MAX_PARSED_BYTES};
use crate::util::hash_text;

const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "build",
    "coverage",
    "vendor",
    "__pycache__",
    "venv",
    ".venv",
    "out",
    "obj",
    "bin",
    ".git",
    ".jcs",
];

/// Indexes (or re-indexes) one project.
pub async fn index_project(
    store: &Store,
    embeddings: Option<&Embeddings>,
    project_id: &ProjectId,
    root: &Path,
    force: bool,
) -> Result<IndexReport> {
    let started = Instant::now();
    let mut report = IndexReport::default();

    if !root.is_dir() {
        return Err(MemoryError::Invalid(format!(
            "{} is not a directory",
            root.display()
        )));
    }

    let mut seen = BTreeSet::new();

    for path in walk_files(root) {
        report.files_seen += 1;
        match index_file(
            store,
            embeddings,
            project_id,
            root,
            &path,
            force,
            &mut report,
        )
        .await
        {
            Ok(Some(relative)) => {
                seen.insert(relative);
            }
            Ok(None) => {}
            Err(error) => report.errors.push(format!("{}: {error}", path.display())),
        }
    }

    // Files that disappeared from disk must disappear from the index too.
    let indexed: Vec<String> =
        sqlx::query_scalar("SELECT path FROM memory_files WHERE project_id = ?")
            .bind(project_id.as_str())
            .fetch_all(store.pool())
            .await?;

    for relative in indexed {
        if !seen.contains(&relative) {
            remove_file(store, project_id, &relative).await?;
            report.files_removed += 1;
        }
    }

    report.duration_ms = started.elapsed().as_millis() as i64;
    tracing::info!(
        project = %project_id,
        indexed = report.files_indexed,
        skipped = report.files_skipped,
        removed = report.files_removed,
        chunks = report.chunks,
        errors = report.errors.len(),
        duration_ms = report.duration_ms,
        "memory index updated"
    );
    Ok(report)
}

async fn index_file(
    store: &Store,
    embeddings: Option<&Embeddings>,
    project_id: &ProjectId,
    root: &Path,
    path: &Path,
    force: bool,
    report: &mut IndexReport,
) -> Result<Option<String>> {
    let relative = relative_path(root, path)?;
    let metadata = std::fs::metadata(path)?;

    if metadata.len() > MAX_PARSED_BYTES {
        report.errors.push(format!(
            "{relative}: {} bytes is above the {} byte parsing limit; skipped",
            metadata.len(),
            MAX_PARSED_BYTES
        ));
        return Ok(None);
    }

    let bytes = std::fs::read(path)?;
    if bytes.iter().take(4096).any(|byte| *byte == 0) {
        return Ok(None);
    }

    let source = match String::from_utf8(bytes) {
        Ok(source) => source,
        Err(_) => return Ok(None),
    };

    let Some(language) = Language::from_path(path) else {
        return Ok(None);
    };

    let content_hash = hash_text(&source);

    let existing: Option<String> = sqlx::query_scalar(
        "SELECT content_hash FROM memory_files WHERE project_id = ? AND path = ?",
    )
    .bind(project_id.as_str())
    .bind(&relative)
    .fetch_optional(store.pool())
    .await?;

    if !force && existing.as_deref() == Some(content_hash.as_str()) {
        report.files_skipped += 1;
        return Ok(Some(relative));
    }

    let extraction = match extract(language, &relative, &source) {
        Ok(extraction) => extraction,
        Err(error) => {
            // A file that does not parse is still worth retrieving as text.
            report.errors.push(format!("{relative}: {error}"));
            Extraction::default()
        }
    };

    let chunks = chunk_file(&relative, &source, &extraction.symbols);

    persist(
        store,
        project_id,
        &relative,
        language,
        &content_hash,
        metadata.len(),
        &extraction,
        &chunks,
    )
    .await?;

    report.files_indexed += 1;
    report.symbols += extraction.symbols.len();
    report.chunks += chunks.len();

    if let Some(provider) = embeddings {
        if let Err(error) = embed_chunks(store, provider, project_id, &relative, &chunks).await {
            report
                .errors
                .push(format!("{relative}: embeddings unavailable: {error}"));
        } else {
            report.embeddings += chunks.len();
        }
    }

    Ok(Some(relative))
}

#[allow(clippy::too_many_arguments)]
async fn persist(
    store: &Store,
    project_id: &ProjectId,
    relative: &str,
    language: Language,
    content_hash: &str,
    size: u64,
    extraction: &Extraction,
    chunks: &[Chunk],
) -> Result<()> {
    let mut tx = store.pool().begin().await?;

    sqlx::query(
        "DELETE FROM memory_fts WHERE rowid IN \
         (SELECT id FROM memory_chunks WHERE project_id = ? AND path = ?)",
    )
    .bind(project_id.as_str())
    .bind(relative)
    .execute(&mut *tx)
    .await?;

    for statement in [
        "DELETE FROM memory_chunks WHERE project_id = ? AND path = ?",
        "DELETE FROM memory_nodes WHERE project_id = ? AND path = ?",
        "DELETE FROM memory_edges WHERE project_id = ? AND src_path = ?",
    ] {
        sqlx::query(statement)
            .bind(project_id.as_str())
            .bind(relative)
            .execute(&mut *tx)
            .await?;
    }

    for symbol in &extraction.symbols {
        sqlx::query(
            "INSERT INTO memory_nodes \
             (project_id, path, language, kind, name, qualified, start_line, end_line, detail) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(project_id.as_str())
        .bind(&symbol.path)
        .bind(symbol.language.as_str())
        .bind(symbol.kind.as_str())
        .bind(&symbol.name)
        .bind(&symbol.qualified)
        .bind(symbol.start_line as i64)
        .bind(symbol.end_line as i64)
        .bind(symbol.detail.as_deref())
        .execute(&mut *tx)
        .await?;
    }

    for edge in &extraction.edges {
        sqlx::query(
            "INSERT INTO memory_edges (project_id, src_path, src_name, dst_name, kind) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(project_id.as_str())
        .bind(&edge.src_path)
        .bind(edge.src_name.as_deref())
        .bind(&edge.dst_name)
        .bind(edge.kind.as_str())
        .execute(&mut *tx)
        .await?;
    }

    for chunk in chunks {
        let inserted = sqlx::query(
            "INSERT INTO memory_chunks \
             (project_id, path, start_line, end_line, content, content_hash) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(project_id.as_str())
        .bind(&chunk.path)
        .bind(chunk.start_line as i64)
        .bind(chunk.end_line as i64)
        .bind(&chunk.content)
        .bind(&chunk.content_hash)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO memory_fts (rowid, content, path, project_id) VALUES (?, ?, ?, ?)",
        )
        .bind(inserted.last_insert_rowid())
        .bind(&chunk.content)
        .bind(&chunk.path)
        .bind(project_id.as_str())
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO memory_files \
         (project_id, path, language, content_hash, size_bytes, indexed_at) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT(project_id, path) DO UPDATE SET \
           language = excluded.language, \
           content_hash = excluded.content_hash, \
           size_bytes = excluded.size_bytes, \
           indexed_at = excluded.indexed_at",
    )
    .bind(project_id.as_str())
    .bind(relative)
    .bind(language.as_str())
    .bind(content_hash)
    .bind(size as i64)
    .bind(jcs_store::store::encode_timestamp(jiff::Timestamp::now()))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

async fn embed_chunks(
    store: &Store,
    provider: &Embeddings,
    project_id: &ProjectId,
    relative: &str,
    chunks: &[Chunk],
) -> Result<()> {
    if chunks.is_empty() {
        return Ok(());
    }

    let texts: Vec<String> = chunks.iter().map(|chunk| chunk.content.clone()).collect();
    let vectors = provider.embed(&texts).await?;

    for (chunk, vector) in chunks.iter().zip(vectors) {
        sqlx::query(
            "UPDATE memory_chunks SET embedding = ?, embedding_model = ? \
             WHERE project_id = ? AND path = ? AND start_line = ? AND content_hash = ?",
        )
        .bind(encode_vector(&vector))
        .bind(provider.model())
        .bind(project_id.as_str())
        .bind(relative)
        .bind(chunk.start_line as i64)
        .bind(&chunk.content_hash)
        .execute(store.pool())
        .await?;
    }

    Ok(())
}

async fn remove_file(store: &Store, project_id: &ProjectId, relative: &str) -> Result<()> {
    let mut tx = store.pool().begin().await?;

    sqlx::query(
        "DELETE FROM memory_fts WHERE rowid IN \
         (SELECT id FROM memory_chunks WHERE project_id = ? AND path = ?)",
    )
    .bind(project_id.as_str())
    .bind(relative)
    .execute(&mut *tx)
    .await?;

    for statement in [
        "DELETE FROM memory_chunks WHERE project_id = ? AND path = ?",
        "DELETE FROM memory_nodes WHERE project_id = ? AND path = ?",
        "DELETE FROM memory_edges WHERE project_id = ? AND src_path = ?",
        "DELETE FROM memory_files WHERE project_id = ? AND path = ?",
    ] {
        sqlx::query(statement)
            .bind(project_id.as_str())
            .bind(relative)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Removes every memory row for a project.
pub async fn clear_project(store: &Store, project_id: &ProjectId) -> Result<()> {
    let mut tx = store.pool().begin().await?;

    sqlx::query(
        "DELETE FROM memory_fts WHERE rowid IN \
         (SELECT id FROM memory_chunks WHERE project_id = ?)",
    )
    .bind(project_id.as_str())
    .execute(&mut *tx)
    .await?;

    for statement in [
        "DELETE FROM memory_chunks WHERE project_id = ?",
        "DELETE FROM memory_nodes WHERE project_id = ?",
        "DELETE FROM memory_edges WHERE project_id = ?",
        "DELETE FROM memory_files WHERE project_id = ?",
    ] {
        sqlx::query(statement)
            .bind(project_id.as_str())
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn stats(store: &Store, project_id: &ProjectId) -> Result<MemoryStats> {
    let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_files WHERE project_id = ?")
        .bind(project_id.as_str())
        .fetch_one(store.pool())
        .await?;
    let symbols: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_nodes WHERE project_id = ?")
        .bind(project_id.as_str())
        .fetch_one(store.pool())
        .await?;
    let chunks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_chunks WHERE project_id = ?")
        .bind(project_id.as_str())
        .fetch_one(store.pool())
        .await?;
    let embedded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memory_chunks WHERE project_id = ? AND embedding IS NOT NULL",
    )
    .bind(project_id.as_str())
    .fetch_one(store.pool())
    .await?;
    let edges: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_edges WHERE project_id = ?")
        .bind(project_id.as_str())
        .fetch_one(store.pool())
        .await?;
    let last: Option<String> =
        sqlx::query_scalar("SELECT MAX(indexed_at) FROM memory_files WHERE project_id = ?")
            .bind(project_id.as_str())
            .fetch_one(store.pool())
            .await?;

    Ok(MemoryStats {
        files,
        symbols,
        chunks,
        embedded_chunks: embedded,
        edges,
        last_indexed_at: last,
    })
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            MemoryError::Invalid(format!(
                "{} is not under {}",
                path.display(),
                root.display()
            ))
        })
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            if file_type.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_ref()) {
                    continue;
                }
                stack.push(path);
            } else if file_type.is_file() && Language::from_path(&path).is_some() {
                files.push(path);
            }
        }
    }

    // Deterministic order keeps indexing reproducible and diffs of the index meaningless.
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn irrelevant_and_hidden_directories_are_skipped() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::create_dir_all(root.path().join("target")).unwrap();
        std::fs::create_dir_all(root.path().join(".hidden")).unwrap();
        std::fs::create_dir_all(root.path().join("node_modules")).unwrap();

        std::fs::write(root.path().join("src/lib.rs"), "fn a() {}").unwrap();
        std::fs::write(root.path().join("target/lib.rs"), "fn b() {}").unwrap();
        std::fs::write(root.path().join(".hidden/lib.rs"), "fn c() {}").unwrap();
        std::fs::write(root.path().join("node_modules/lib.rs"), "fn d() {}").unwrap();
        std::fs::write(root.path().join("README.md"), "docs").unwrap();

        let files = walk_files(root.path());
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(files[0].ends_with("src/lib.rs"));
    }

    #[test]
    fn file_order_is_deterministic() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("b")).unwrap();
        std::fs::create_dir_all(root.path().join("a")).unwrap();
        std::fs::write(root.path().join("b/z.rs"), "fn z() {}").unwrap();
        std::fs::write(root.path().join("a/y.rs"), "fn y() {}").unwrap();

        let first = walk_files(root.path());
        let second = walk_files(root.path());
        assert_eq!(first, second);
    }
}
