//! Small shared helpers.

use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256.
pub fn hash_text(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Rough token estimate used for context budgets: four characters per token.
///
/// A heuristic is acceptable here because the budget is a safety rail, not an accounting
/// system, and every provider tokenizes differently anyway.
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}
