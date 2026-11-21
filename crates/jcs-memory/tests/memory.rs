//! Indexing, retrieval and incremental updates against real files.

use std::path::Path;

use jcs_core::ProjectId;
use jcs_memory::{context, indexer, search};
use jcs_store::Store;

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

async fn fixture() -> (tempfile::TempDir, Store, ProjectId) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    write(
        root,
        "src/auth.rs",
        r#"
//! Email validation for the registration endpoint.

pub struct Validator;

impl Validator {
    /// Rejects addresses without a domain.
    pub fn validate_email(&self, value: &str) -> bool {
        value.contains('@')
    }
}

#[test]
fn rejects_addresses_without_a_domain() {
    assert!(!Validator.validate_email("nope"));
}
"#,
    );

    write(
        root,
        "src/store.rs",
        r#"
//! Persistent storage. Opening the database happens once per process.

pub struct Store {
    path: String,
}

impl Store {
    /// Opens the database, creating it when missing.
    pub fn open(path: &str) -> Store {
        Store { path: path.to_string() }
    }
}
"#,
    );

    write(
        root,
        "db/schema.sql",
        r#"
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    email TEXT NOT NULL
);
CREATE INDEX idx_users_email ON users(email);
"#,
    );

    write(root, "README.md", "# Not indexed\n");

    let store = Store::in_memory().await.unwrap();
    let project = ProjectId::new();
    (directory, store, project)
}

#[tokio::test]
async fn indexing_extracts_symbols_chunks_and_sql_tables() {
    let (directory, store, project) = fixture().await;

    let report = indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();
    assert_eq!(report.files_seen, 3, "markdown must not be indexed");
    assert_eq!(report.files_indexed, 3);
    assert_eq!(report.files_skipped, 0);
    assert!(report.symbols >= 5, "{report:?}");
    assert!(report.chunks >= 3, "{report:?}");
    assert!(report.errors.is_empty(), "{report:?}");

    let stats = indexer::stats(&store, &project).await.unwrap();
    assert_eq!(stats.files, 3);
    assert!(stats.symbols >= 5);
    assert_eq!(stats.embedded_chunks, 0);
}

#[tokio::test]
async fn a_second_index_run_skips_everything_unchanged() {
    let (directory, store, project) = fixture().await;
    indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    let report = indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    assert_eq!(report.files_indexed, 0);
    assert_eq!(report.files_skipped, 3);
    assert_eq!(report.files_removed, 0);
}

#[tokio::test]
async fn only_changed_files_are_reindexed() {
    let (directory, store, project) = fixture().await;
    indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    write(
        directory.path(),
        "src/auth.rs",
        "//! Email validation.\n\npub fn validate_email(value: &str) -> bool { value.contains('@') }\n",
    );

    let report = indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    assert_eq!(report.files_indexed, 1);
    assert_eq!(report.files_skipped, 2);
}

#[tokio::test]
async fn deleted_files_disappear_from_the_index() {
    let (directory, store, project) = fixture().await;
    indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    std::fs::remove_file(directory.path().join("db/schema.sql")).unwrap();

    let report = indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();
    assert_eq!(report.files_removed, 1);

    let stats = indexer::stats(&store, &project).await.unwrap();
    assert_eq!(stats.files, 2);
}

#[tokio::test]
async fn searching_by_symbol_name_returns_its_file_with_a_citation() {
    let (directory, store, project) = fixture().await;
    indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    let hits = search::search(&store, None, &project, "Validator", 5)
        .await
        .unwrap();
    assert!(!hits.is_empty(), "no hits for a symbol name");
    assert_eq!(hits[0].path, "src/auth.rs");
    assert!(hits[0].citation().starts_with("src/auth.rs:"));
}

#[tokio::test]
async fn searching_by_words_finds_comments() {
    let (directory, store, project) = fixture().await;
    indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    let hits = search::search(&store, None, &project, "opening the database", 5)
        .await
        .unwrap();
    assert!(
        hits.iter().any(|hit| hit.path == "src/store.rs"),
        "expected src/store.rs, got {:?}",
        hits.iter().map(|hit| &hit.path).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn searching_finds_sql_tables() {
    let (directory, store, project) = fixture().await;
    indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    let hits = search::search(&store, None, &project, "users", 5)
        .await
        .unwrap();
    assert!(hits.iter().any(|hit| hit.path == "db/schema.sql"));
}

#[tokio::test]
async fn an_empty_or_missing_index_returns_no_hits_instead_of_failing() {
    let (_directory, store, project) = fixture().await;

    let hits = search::search(&store, None, &project, "anything", 5)
        .await
        .unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn context_packages_are_cited_and_budgeted() {
    let (directory, store, project) = fixture().await;
    indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    let mut options = context::ContextOptions::new("email validation");
    options.budget_tokens = 400;

    let package = context::build(&store, None, &project, options, &[])
        .await
        .unwrap();

    assert!(!package.is_empty());
    assert!(package.text.contains("## Relevant code"));
    assert!(!package.citations.is_empty());
    assert!(package
        .citations
        .iter()
        .all(|citation| citation.contains(':')));
    assert!(
        package.used_tokens <= 450,
        "budget exceeded: {} tokens",
        package.used_tokens
    );
}

#[tokio::test]
async fn extra_sections_are_included_when_the_budget_allows() {
    let (directory, store, project) = fixture().await;
    indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    let mut options = context::ContextOptions::new("email validation");
    options.budget_tokens = 4_000;

    let package = context::build(
        &store,
        None,
        &project,
        options,
        &[(
            "Project rules".to_string(),
            "No unwrap outside tests.".to_string(),
        )],
    )
    .await
    .unwrap();

    assert!(package.text.contains("## Project rules"));
    assert!(package.text.contains("No unwrap outside tests."));
}

/// Retrieval quality is a measurement, not an opinion: every query must find its file in
/// the top three results.
#[tokio::test]
async fn recall_at_three_is_total_on_the_fixture() {
    let (directory, store, project) = fixture().await;
    indexer::index_project(&store, None, &project, directory.path(), false)
        .await
        .unwrap();

    let cases = [
        ("Validator", "src/auth.rs"),
        ("validate_email", "src/auth.rs"),
        ("email validation", "src/auth.rs"),
        ("opening the database", "src/store.rs"),
        ("Store", "src/store.rs"),
        ("users", "db/schema.sql"),
        ("idx_users_email", "db/schema.sql"),
    ];

    let mut found = 0usize;
    for (query, expected) in cases {
        let hits = search::search(&store, None, &project, query, 3)
            .await
            .unwrap();
        let hit = hits.iter().take(3).any(|hit| hit.path == expected);
        if hit {
            found += 1;
        } else {
            eprintln!(
                "MISS {query:?} -> {:?}",
                hits.iter().map(|hit| &hit.path).collect::<Vec<_>>()
            );
        }
    }

    let recall = found as f32 / cases.len() as f32;
    assert!(
        recall >= 0.99,
        "recall@3 was {recall:.2} ({found}/{}), expected every query to hit",
        cases.len()
    );
}
