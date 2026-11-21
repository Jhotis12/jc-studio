//! Chunking.
//!
//! Chunks are symbol-aligned: a function, a class, a table. That is the unit a human would
//! paste into a prompt, and it gives every retrieval result a citation that points at a
//! coherent piece of code instead of an arbitrary window.

use crate::model::{Chunk, Symbol, MAX_CHUNKS_PER_FILE, MAX_CHUNK_LINES};
use crate::util::hash_text;

/// Splits a file into chunks, preferring symbol boundaries.
pub fn chunk_file(path: &str, source: &str, symbols: &[Symbol]) -> Vec<Chunk> {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let last_line = lines.len() as u32;

    // Symbol ranges, dropping ones fully contained in another (a method inside a class is
    // already part of the class chunk).
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    let mut sorted: Vec<&Symbol> = symbols.iter().collect();
    sorted.sort_by_key(|symbol| (symbol.start_line, std::cmp::Reverse(symbol.end_line)));

    for symbol in sorted {
        let start = symbol.start_line.clamp(1, last_line);
        let end = symbol.end_line.clamp(start, last_line);
        let contained = ranges
            .iter()
            .any(|(existing_start, existing_end)| start >= *existing_start && end <= *existing_end);
        if !contained {
            ranges.push((start, end));
        }
    }

    // A file header carries module docs, imports and licence text that no symbol covers.
    // It must stop before the first symbol, or the same lines would appear twice.
    match ranges.first().copied() {
        Some((first_start, _)) if first_start > 1 => {
            ranges.insert(0, (1, (first_start - 1).min(25)));
        }
        Some(_) => {}
        None => ranges.push((1, last_line.min(25))),
    }

    let mut chunks = Vec::new();
    for (start, end) in ranges {
        for (chunk_start, chunk_end) in split_range(start, end) {
            if chunks.len() >= MAX_CHUNKS_PER_FILE {
                break;
            }
            let content = lines[chunk_start as usize - 1..chunk_end as usize].join("\n");
            if content.trim().is_empty() {
                continue;
            }
            let content_hash = hash_text(&format!("{path}:{chunk_start}:{chunk_end}:{content}"));
            chunks.push(Chunk {
                path: path.to_string(),
                start_line: chunk_start,
                end_line: chunk_end,
                content,
                content_hash,
            });
        }
    }

    chunks
}

/// Splits an oversized range with a small overlap so a boundary does not cut a statement
/// out of every chunk that mentions it.
fn split_range(start: u32, end: u32) -> Vec<(u32, u32)> {
    let length = end.saturating_sub(start) + 1;
    if length <= MAX_CHUNK_LINES {
        return vec![(start, end)];
    }

    let mut ranges = Vec::new();
    let mut cursor = start;
    while cursor <= end {
        let chunk_end = (cursor + MAX_CHUNK_LINES - 1).min(end);
        ranges.push((cursor, chunk_end));
        if chunk_end == end {
            break;
        }
        cursor = chunk_end.saturating_sub(10) + 1;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Language;
    use crate::model::SymbolKind;

    fn symbol(name: &str, start: u32, end: u32) -> Symbol {
        Symbol {
            path: "src/x.rs".into(),
            language: Language::Rust,
            kind: SymbolKind::Function,
            name: name.into(),
            qualified: name.into(),
            start_line: start,
            end_line: end,
            detail: None,
        }
    }

    #[test]
    fn a_file_without_symbols_still_yields_a_header_chunk() {
        let source = (1..=40)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_file("src/x.rs", &source, &[]);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 25);
    }

    #[test]
    fn chunks_are_aligned_to_symbols_and_contained_symbols_are_skipped() {
        let source = (1..=60)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let symbols = vec![symbol("outer", 10, 40), symbol("inner", 12, 20)];
        let chunks = chunk_file("src/x.rs", &source, &symbols);

        assert!(chunks
            .iter()
            .any(|chunk| chunk.start_line == 10 && chunk.end_line == 40));
        // The nested symbol is inside the outer chunk.
        assert!(!chunks.iter().any(|chunk| chunk.start_line == 12));
        // Header chunk because the first symbol starts after line 3.
        assert!(chunks.iter().any(|chunk| chunk.start_line == 1));
    }

    #[test]
    fn oversized_symbols_are_split_with_overlap() {
        let source = (1..=400)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let symbols = vec![symbol("huge", 1, 300)];
        let chunks = chunk_file("src/x.rs", &source, &symbols);

        assert!(
            chunks.len() >= 3,
            "expected several chunks, got {}",
            chunks.len()
        );
        assert!(chunks
            .iter()
            .all(|chunk| chunk.end_line - chunk.start_line < MAX_CHUNK_LINES));
        // Consecutive chunks overlap by ~10 lines.
        assert!(chunks[1].start_line <= chunks[0].end_line);
    }

    #[test]
    fn chunk_hashes_change_with_content_and_position() {
        let source = "fn a() {}\nfn b() {}\n";
        let a = chunk_file("src/x.rs", source, &[symbol("a", 1, 1)]);
        let b = chunk_file("src/y.rs", source, &[symbol("a", 1, 1)]);
        assert_ne!(a[0].content_hash, b[0].content_hash);
    }

    #[test]
    fn a_header_comment_before_the_first_symbol_is_chunked() {
        let source = "//! Registration endpoint validation.\n\npub struct V;\n";
        let symbols = vec![symbol("V", 3, 3)];
        let chunks = chunk_file("src/auth.rs", source, &symbols);

        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 2);
        assert!(chunks[0]
            .content
            .to_lowercase()
            .contains("registration endpoint"));
        // No duplication: the struct chunk starts after the header.
        assert!(chunks.iter().any(|chunk| chunk.start_line == 3));
    }

    #[test]
    fn a_symbol_starting_on_line_one_gets_no_empty_header() {
        let source = "pub struct V;\n";
        let symbols = vec![symbol("V", 1, 1)];
        let chunks = chunk_file("src/auth.rs", source, &symbols);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn chunking_is_deterministic() {
        let source = (1..=80)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let symbols = vec![symbol("a", 5, 20), symbol("b", 30, 50)];
        let first = chunk_file("src/x.rs", &source, &symbols);
        let second = chunk_file("src/x.rs", &source, &symbols);
        assert_eq!(first, second);
    }
}
