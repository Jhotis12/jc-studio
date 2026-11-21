//! Context broker.
//!
//! Builds the smallest useful context package for a task: the code that matters, cited,
//! under a token budget. Every snippet carries `path:start-end` so the agent can be told
//! exactly where a claim came from.

use jcs_core::ProjectId;
use jcs_store::Store;

use crate::embeddings::Embeddings;
use crate::error::Result;
use crate::model::SearchHit;
use crate::search;
use crate::util::estimate_tokens;

#[derive(Debug, Clone)]
pub struct ContextOptions {
    pub query: String,
    /// Approximate token ceiling for the rendered package.
    pub budget_tokens: usize,
    pub limit: usize,
}

impl ContextOptions {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            budget_tokens: 2_000,
            limit: 8,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ContextPackage {
    pub text: String,
    pub citations: Vec<String>,
    pub used_tokens: usize,
    pub hits: usize,
}

impl ContextPackage {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Builds a cited context block, filling the budget in relevance order.
///
/// `extra_sections` are appended after the code if budget remains: policy rules, previous
/// failure output, anything the caller wants the agent to see with the code.
pub async fn build(
    store: &Store,
    embeddings: Option<&Embeddings>,
    project_id: &ProjectId,
    options: ContextOptions,
    extra_sections: &[(String, String)],
) -> Result<ContextPackage> {
    let hits = search::search(
        store,
        embeddings,
        project_id,
        &options.query,
        options.limit.max(1),
    )
    .await?;

    if hits.is_empty() {
        return Ok(ContextPackage::default());
    }

    let mut package = ContextPackage::default();
    let mut lines: Vec<String> = vec!["## Relevant code".to_string(), String::new()];
    let mut used = estimate_tokens(&lines.join("\n"));

    for hit in &hits {
        if used >= options.budget_tokens {
            break;
        }

        let block = render_hit(hit);
        let cost = estimate_tokens(&block);
        if used + cost > options.budget_tokens && package.hits > 0 {
            break;
        }

        used += cost;
        package.hits += 1;
        package.citations.push(hit.citation());
        lines.push(block);
    }

    for (title, body) in extra_sections {
        let block = format!("## {title}\n\n{}\n", body.trim());
        let cost = estimate_tokens(&block);
        if used + cost > options.budget_tokens {
            continue;
        }
        used += cost;
        lines.push(block);
    }

    package.used_tokens = used;
    package.text = lines.join("\n").trim().to_string();
    Ok(package)
}

fn render_hit(hit: &SearchHit) -> String {
    let fence = language_fence(&hit.path);
    format!(
        "### {} ({})\n\n```{fence}\n{}\n```\n",
        hit.citation(),
        hit.source.as_str(),
        hit.content.trim_end()
    )
}

fn language_fence(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "sql" => "sql",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(path: &str, start: u32, end: u32, content: &str) -> SearchHit {
        SearchHit {
            chunk_id: 1,
            path: path.to_string(),
            start_line: start,
            end_line: end,
            score: 0.9,
            source: crate::model::HitSource::Lexical,
            snippet: String::new(),
            content: content.to_string(),
        }
    }

    #[test]
    fn a_hit_renders_with_a_citation_and_the_right_fence() {
        let rendered = render_hit(&hit("src/lib.rs", 10, 20, "fn a() {}"));
        assert!(rendered.contains("src/lib.rs:10-20"));
        assert!(rendered.contains("```rust"));
        assert!(rendered.contains("fn a() {}"));
    }

    #[test]
    fn unknown_extensions_get_a_plain_fence() {
        let rendered = render_hit(&hit("Makefile", 1, 2, "all:"));
        assert!(rendered.contains("```\n"), "{rendered}");
    }
}
