//! Retrieval: lexical, semantic and structural, merged into one ranked list.
//!
//! The three signals answer different questions and fail differently:
//!   * BM25 finds the words you used, even in a comment;
//!   * embeddings find the idea even when the words differ;
//!   * the graph finds the code that touches the code you found.
//!
//! Merging them is what makes retrieval usable on a real codebase; any one alone has a
//! predictable blind spot.

use std::collections::HashMap;

use jcs_core::ProjectId;
use jcs_store::Store;
use sqlx::QueryBuilder;

use crate::embeddings::{cosine, decode_vector, Embeddings, SEMANTIC_CANDIDATE_LIMIT};
use crate::error::Result;
use crate::model::{HitSource, SearchHit};

/// How many lexical results are considered before merging.
const LEXICAL_LIMIT: i64 = 40;
/// How many paths the graph step expands from.
const GRAPH_SEEDS: usize = 5;
/// How many related paths the graph step adds.
const GRAPH_LIMIT: i64 = 8;

pub async fn search(
    store: &Store,
    embeddings: Option<&Embeddings>,
    project_id: &ProjectId,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let mut merged: HashMap<i64, SearchHit> = HashMap::new();

    for hit in symbol_hits(store, project_id, query, 10).await? {
        merge(&mut merged, hit);
    }

    for hit in lexical(store, project_id, query).await? {
        merge(&mut merged, hit);
    }

    if let Some(provider) = embeddings {
        for hit in semantic(store, provider, project_id, query).await? {
            merge(&mut merged, hit);
        }
    }

    if merged.is_empty() {
        for hit in fallback(store, project_id, query).await? {
            merge(&mut merged, hit);
        }
    }

    for hit in graph(store, project_id, &merged).await? {
        merge(&mut merged, hit);
    }

    let mut hits: Vec<SearchHit> = merged.into_values().collect();
    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.start_line.cmp(&right.start_line))
    });
    hits.truncate(limit.clamp(1, 50));
    Ok(hits)
}

/// Keeps the best score for a chunk, with a small bonus when several signals agree.
fn merge(merged: &mut HashMap<i64, SearchHit>, candidate: SearchHit) {
    match merged.get_mut(&candidate.chunk_id) {
        Some(existing) => {
            existing.score = existing.score.max(candidate.score) + 0.10;
            // A lexical hit is more explainable than a graph one; keep the stronger source.
            if candidate.score > existing.score {
                existing.source = candidate.source;
            }
        }
        None => {
            merged.insert(candidate.chunk_id, candidate);
        }
    }
}

async fn chunk_by_id(store: &Store, id: i64) -> Result<Option<SearchHit>> {
    let row: Option<(i64, String, i64, i64, String)> = sqlx::query_as(
        "SELECT id, path, start_line, end_line, content FROM memory_chunks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(store.pool())
    .await?;

    Ok(row.map(|(id, path, start, end, content)| SearchHit {
        chunk_id: id,
        path,
        start_line: start as u32,
        end_line: end as u32,
        score: 0.0,
        source: HitSource::Lexical,
        snippet: String::new(),
        content,
    }))
}

/// Exact symbol names, which BM25 handles poorly (`Store` matches many words).
async fn symbol_hits(
    store: &Store,
    project_id: &ProjectId,
    query: &str,
    limit: i64,
) -> Result<Vec<SearchHit>> {
    let needle = query.trim();
    if needle.is_empty() {
        return Ok(Vec::new());
    }

    let rows: Vec<(String, i64, i64, String, String)> = sqlx::query_as(
        "SELECT path, start_line, end_line, name, kind FROM memory_nodes \
         WHERE project_id = ? AND (name = ? OR qualified = ?) LIMIT ?",
    )
    .bind(project_id.as_str())
    .bind(needle)
    .bind(needle)
    .bind(limit)
    .fetch_all(store.pool())
    .await?;

    let mut hits = Vec::new();
    for (path, start, end, name, kind) in rows {
        let chunk: Option<(i64, String)> = sqlx::query_as(
            "SELECT id, content FROM memory_chunks \
             WHERE project_id = ? AND path = ? AND start_line <= ? AND end_line >= ? \
             ORDER BY start_line LIMIT 1",
        )
        .bind(project_id.as_str())
        .bind(&path)
        .bind(end)
        .bind(start)
        .fetch_optional(store.pool())
        .await?;

        if let Some((id, content)) = chunk {
            hits.push(SearchHit {
                chunk_id: id,
                path,
                start_line: start as u32,
                end_line: end as u32,
                score: 0.9,
                source: HitSource::Lexical,
                snippet: format!("{kind} {name}"),
                content,
            });
        }
    }

    Ok(hits)
}

async fn lexical(store: &Store, project_id: &ProjectId, query: &str) -> Result<Vec<SearchHit>> {
    let Some(match_query) = fts_query(query) else {
        return Ok(Vec::new());
    };

    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT rowid, path, snippet(memory_fts, 0, '', '', '…', 12) \
         FROM memory_fts \
         WHERE memory_fts MATCH ? AND project_id = ? \
         ORDER BY bm25(memory_fts) LIMIT ?",
    )
    .bind(match_query)
    .bind(project_id.as_str())
    .bind(LEXICAL_LIMIT)
    .fetch_all(store.pool())
    .await
    .unwrap_or_default();

    let mut hits = Vec::new();
    for (rank, (id, _path, snippet)) in rows.into_iter().enumerate() {
        if let Some(mut hit) = chunk_by_id(store, id).await? {
            hit.score = 1.0 / (1.0 + rank as f32 * 0.25);
            hit.source = HitSource::Lexical;
            hit.snippet = snippet;
            hits.push(hit);
        }
    }
    Ok(hits)
}

async fn semantic(
    store: &Store,
    provider: &Embeddings,
    project_id: &ProjectId,
    query: &str,
) -> Result<Vec<SearchHit>> {
    let embedded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memory_chunks WHERE project_id = ? AND embedding IS NOT NULL",
    )
    .bind(project_id.as_str())
    .fetch_one(store.pool())
    .await?;

    if embedded == 0 {
        return Ok(Vec::new());
    }

    let vectors = provider.embed(&[query.to_string()]).await?;
    let Some(query_vector) = vectors.first() else {
        return Ok(Vec::new());
    };

    let rows: Vec<(i64, String, i64, i64, String, Vec<u8>)> = sqlx::query_as(
        "SELECT id, path, start_line, end_line, content, embedding FROM memory_chunks \
         WHERE project_id = ? AND embedding IS NOT NULL LIMIT ?",
    )
    .bind(project_id.as_str())
    .bind(SEMANTIC_CANDIDATE_LIMIT)
    .fetch_all(store.pool())
    .await?;

    let mut scored: Vec<(f32, SearchHit)> = Vec::new();
    for (id, path, start, end, content, bytes) in rows {
        let similarity = cosine(query_vector, &decode_vector(&bytes));
        if similarity <= 0.0 {
            continue;
        }
        scored.push((
            similarity,
            SearchHit {
                chunk_id: id,
                path,
                start_line: start as u32,
                end_line: end as u32,
                score: similarity,
                source: HitSource::Semantic,
                snippet: String::new(),
                content,
            },
        ));
    }

    scored.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(20);
    Ok(scored.into_iter().map(|(_, hit)| hit).collect())
}

async fn fallback(store: &Store, project_id: &ProjectId, query: &str) -> Result<Vec<SearchHit>> {
    let token = query
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|c: char| !c.is_alphanumeric());
    if token.len() < 3 {
        return Ok(Vec::new());
    }

    let rows: Vec<(i64, String, i64, i64, String)> = sqlx::query_as(
        "SELECT id, path, start_line, end_line, content FROM memory_chunks \
         WHERE project_id = ? AND content LIKE ? LIMIT 20",
    )
    .bind(project_id.as_str())
    .bind(format!("%{token}%"))
    .fetch_all(store.pool())
    .await?;

    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(rank, (id, path, start, end, content))| SearchHit {
            chunk_id: id,
            path,
            start_line: start as u32,
            end_line: end as u32,
            score: 0.5 / (1.0 + rank as f32 * 0.3),
            source: HitSource::Fallback,
            snippet: String::new(),
            content,
        })
        .collect())
}

/// Adds files that call into the code we already found.
async fn graph(
    store: &Store,
    project_id: &ProjectId,
    current: &HashMap<i64, SearchHit>,
) -> Result<Vec<SearchHit>> {
    let mut seeds: Vec<String> = current
        .values()
        .filter(|hit| hit.source != HitSource::Graph)
        .map(|hit| hit.path.clone())
        .collect();
    seeds.sort();
    seeds.dedup();
    seeds.truncate(GRAPH_SEEDS);

    if seeds.is_empty() {
        return Ok(Vec::new());
    }

    // QueryBuilder keeps dynamic `IN (...)` lists parameterised; string-building SQL is
    // rejected by sqlx for good reason.
    let mut builder = QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT DISTINCT name FROM memory_nodes WHERE project_id = ",
    );
    builder.push_bind(project_id.as_str());
    builder.push(" AND path IN (");
    let mut separated = builder.separated(", ");
    for seed in &seeds {
        separated.push_bind(seed);
    }
    separated.push_unseparated(")");

    let names: Vec<String> = builder
        .build_query_scalar()
        .fetch_all(store.pool())
        .await
        .unwrap_or_default();

    if names.is_empty() {
        return Ok(Vec::new());
    }

    let mut builder = QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT DISTINCT src_path FROM memory_edges WHERE project_id = ",
    );
    builder.push_bind(project_id.as_str());
    builder.push(" AND dst_name IN (");
    let mut separated = builder.separated(", ");
    for name in &names {
        separated.push_bind(name);
    }
    separated.push_unseparated(") AND src_path NOT IN (");
    let mut separated = builder.separated(", ");
    for seed in &seeds {
        separated.push_bind(seed);
    }
    separated.push_unseparated(") LIMIT ");
    builder.push_bind(GRAPH_LIMIT);

    let related: Vec<String> = builder
        .build_query_scalar()
        .fetch_all(store.pool())
        .await
        .unwrap_or_default();

    let mut hits = Vec::new();
    for path in related {
        let chunk: Option<(i64, i64, i64, String)> = sqlx::query_as(
            "SELECT id, start_line, end_line, content FROM memory_chunks \
             WHERE project_id = ? AND path = ? ORDER BY start_line LIMIT 1",
        )
        .bind(project_id.as_str())
        .bind(&path)
        .fetch_optional(store.pool())
        .await?;

        if let Some((id, start, end, content)) = chunk {
            hits.push(SearchHit {
                chunk_id: id,
                path,
                start_line: start as u32,
                end_line: end as u32,
                score: 0.45,
                source: HitSource::Graph,
                snippet: String::new(),
                content,
            });
        }
    }

    Ok(hits)
}

/// Turns free text into a safe FTS5 expression, or `None` when nothing usable remains.
fn fts_query(query: &str) -> Option<String> {
    let tokens: Vec<String> = query
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|token| token.chars().count() >= 2)
        .take(8)
        .map(|token| format!("\"{}\"", token.to_lowercase()))
        .collect();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" OR "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts_queries_are_quoted_and_bounded() {
        assert_eq!(
            fts_query("Store::open the database"),
            Some("\"store\" OR \"open\" OR \"the\" OR \"database\"".to_string())
        );
        assert_eq!(fts_query("a"), None);
        assert_eq!(fts_query("   ,, "), None);
    }

    #[test]
    fn fts_queries_drop_punctuation_that_would_break_syntax() {
        let query = fts_query("fn main() { \"unbalanced").unwrap();
        assert!(!query.contains('('), "{query}");
        assert!(!query.contains('{'), "{query}");
        assert_eq!(query, "\"fn\" OR \"main\" OR \"unbalanced\"");
    }

    #[test]
    fn merging_keeps_the_best_score_and_rewards_agreement() {
        let mut merged = HashMap::new();
        merge(
            &mut merged,
            SearchHit {
                chunk_id: 1,
                path: "a.rs".into(),
                start_line: 1,
                end_line: 2,
                score: 0.4,
                source: HitSource::Lexical,
                snippet: String::new(),
                content: String::new(),
            },
        );
        merge(
            &mut merged,
            SearchHit {
                chunk_id: 1,
                path: "a.rs".into(),
                start_line: 1,
                end_line: 2,
                score: 0.3,
                source: HitSource::Semantic,
                snippet: String::new(),
                content: String::new(),
            },
        );

        let hit = &merged[&1];
        assert!((hit.score - 0.5).abs() < 1e-6, "{}", hit.score);
        assert_eq!(hit.source, HitSource::Lexical);
    }
}
