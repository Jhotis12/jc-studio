//! Optional semantic layer.
//!
//! Embeddings are opt-in. Without a provider the index still answers from structure and
//! BM25, and the daemon reports that semantic search is off instead of pretending.
//!
//! Storage is a `BLOB` of little-endian `f32`s and similarity is computed in Rust over a
//! bounded candidate set. That is deliberately simpler than a vector extension: it keeps the
//! build dependency-free, and the bound is explicit in [`SEMANTIC_CANDIDATE_LIMIT`].

use serde_json::json;

use crate::error::{MemoryError, Result};

/// How many embedded chunks a single query compares against.
///
/// Above this, a brute-force scan stops being free and the honest fix is an index, not a
/// bigger constant.
pub const SEMANTIC_CANDIDATE_LIMIT: i64 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingFormat {
    /// `POST {url}/api/embed` with `{model, input}`, as Ollama exposes it.
    Ollama,
    /// `POST {url}/v1/embeddings`, OpenAI-compatible.
    OpenAi,
}

/// A client for an embedding endpoint.
#[derive(Debug, Clone)]
pub struct Embeddings {
    client: reqwest::Client,
    url: String,
    model: String,
    format: EmbeddingFormat,
}

impl Embeddings {
    /// Reads `JCS_EMBEDDING_URL` and `JCS_EMBEDDING_MODEL`; disabled when either is missing.
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("JCS_EMBEDDING_URL").ok()?;
        let model = std::env::var("JCS_EMBEDDING_MODEL").ok()?;
        if url.trim().is_empty() || model.trim().is_empty() {
            return None;
        }

        let format = match std::env::var("JCS_EMBEDDING_FORMAT")
            .unwrap_or_else(|_| "ollama".to_string())
            .as_str()
        {
            "openai" => EmbeddingFormat::OpenAi,
            _ => EmbeddingFormat::Ollama,
        };

        Some(Self {
            client: reqwest::Client::new(),
            url: url.trim_end_matches('/').to_string(),
            model,
            format,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let (endpoint, body) = match self.format {
            EmbeddingFormat::Ollama => (
                format!("{}/api/embed", self.url),
                json!({ "model": self.model, "input": texts }),
            ),
            EmbeddingFormat::OpenAi => (
                format!("{}/v1/embeddings", self.url),
                json!({ "model": self.model, "input": texts }),
            ),
        };

        let response = self
            .client
            .post(&endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|error| MemoryError::Embedding(error.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| MemoryError::Embedding(error.to_string()))?;

        if !status.is_success() {
            return Err(MemoryError::Embedding(format!(
                "{endpoint} returned {status}: {}",
                text.chars().take(200).collect::<String>()
            )));
        }

        let parsed: serde_json::Value = serde_json::from_str(&text)?;
        let vectors = match self.format {
            EmbeddingFormat::Ollama => parsed
                .get("embeddings")
                .and_then(|value| value.as_array())
                .map(|items| parse_vectors(items))
                .unwrap_or_default(),
            EmbeddingFormat::OpenAi => parsed
                .get("data")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.get("embedding").cloned())
                        .collect::<Vec<_>>()
                        .iter()
                        .map(parse_vector)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        };

        if vectors.len() != texts.len() {
            return Err(MemoryError::Embedding(format!(
                "expected {} vectors, received {}",
                texts.len(),
                vectors.len()
            )));
        }

        Ok(vectors)
    }
}

fn parse_vectors(items: &[serde_json::Value]) -> Vec<Vec<f32>> {
    items.iter().map(parse_vector).collect()
}

fn parse_vector(value: &serde_json::Value) -> Vec<f32> {
    value
        .as_array()
        .map(|numbers| {
            numbers
                .iter()
                .filter_map(|number| number.as_f64())
                .map(|number| number as f32)
                .collect()
        })
        .unwrap_or_default()
}

pub fn encode_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub fn decode_vector(bytes: &[u8]) -> Vec<f32> {
    let (groups, _remainder) = bytes.as_chunks::<4>();
    groups
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect()
}

/// Cosine similarity in `[-1, 1]`. Zero when either vector has no magnitude.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (left, right) in a.iter().zip(b) {
        dot += left * right;
        norm_a += left * left;
        norm_b += right * right;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_roundtrip_through_bytes() {
        let vector = vec![0.5f32, -1.25, 3.0];
        let bytes = encode_vector(&vector);
        assert_eq!(bytes.len(), 12);
        assert_eq!(decode_vector(&bytes), vector);
    }

    #[test]
    fn cosine_identifies_identical_and_orthogonal_vectors() {
        assert!((cosine(&[1.0, 2.0], &[1.0, 2.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert!(cosine(&[0.0, 0.0], &[1.0, 0.0]).abs() < 1e-6);
    }

    #[test]
    fn mismatched_dimensions_are_not_compared() {
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
    }
}
