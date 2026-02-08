/// Text chunking for RAG document indexing.
///
/// Splits source text into overlapping chunks that respect sentence boundaries
/// (via `unicode-segmentation`). Each chunk carries its source path and byte
/// offset so search results can be traced back to the original file.

use unicode_segmentation::UnicodeSegmentation;

use crate::spi::rag::DocChunk;

/// Configuration controlling how text is split into chunks.
#[derive(Debug, Clone)]
pub struct ChunkerConfig {
    /// Target chunk size in characters.
    pub chunk_size: usize,
    /// Overlap between consecutive chunks in characters.
    pub overlap: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            chunk_size: 2000,
            overlap: 200,
        }
    }
}

/// Split `text` from `source_path` into overlapping [`DocChunk`]s for the
/// given `agent_id`.
///
/// The algorithm collects whole sentences until the target `chunk_size` is
/// reached, then emits a chunk. The next chunk starts `overlap` characters
/// before the end of the previous one, rewound to a sentence boundary so
/// chunks never split mid-sentence.
pub fn chunk_text(
    text: &str,
    source_path: &str,
    agent_id: &str,
    config: &ChunkerConfig,
) -> Vec<DocChunk> {
    if text.is_empty() {
        return Vec::new();
    }

    let sentences: Vec<&str> = text.unicode_sentences().collect();
    if sentences.is_empty() {
        // Fallback: no sentence boundaries detected — emit the whole text
        // as a single chunk (or split by raw char count if huge).
        return chunk_raw(text, source_path, agent_id, config);
    }

    // If we have only one "sentence" that exceeds chunk_size, fall back to
    // raw chunking. This handles text with no real sentence boundaries where
    // unicode_sentences returns the entire text as one segment.
    if sentences.len() == 1 && sentences[0].len() > config.chunk_size {
        return chunk_raw(text, source_path, agent_id, config);
    }

    let mut chunks = Vec::new();
    let mut idx = 0; // index into `sentences`

    while idx < sentences.len() {
        let mut chunk_chars = 0usize;
        let mut chunk_end_idx = idx;

        // Accumulate sentences until we reach chunk_size.
        while chunk_end_idx < sentences.len() {
            let sent_len = sentences[chunk_end_idx].len();
            if chunk_chars + sent_len > config.chunk_size && chunk_chars > 0 {
                break;
            }
            chunk_chars += sent_len;
            chunk_end_idx += 1;
        }

        // Build the chunk content.
        let content: String = sentences[idx..chunk_end_idx].concat();
        let byte_offset = byte_offset_of(text, &sentences, idx);

        let chunk_id = format!("{}:{}:{}", agent_id, source_path, byte_offset);
        chunks.push(DocChunk {
            id: chunk_id,
            content,
            source_path: source_path.to_string(),
            byte_offset,
            agent_id: agent_id.to_string(),
        });

        if chunk_end_idx >= sentences.len() {
            break;
        }

        // Advance: rewind by `overlap` chars to find the next start sentence.
        let next_start = find_overlap_start(
            &sentences,
            idx,
            chunk_end_idx,
            config.overlap,
        );
        idx = next_start;
    }

    chunks
}

/// Find the sentence index where the next chunk should start so that
/// approximately `overlap` characters from the end of the current chunk
/// are repeated.
fn find_overlap_start(
    sentences: &[&str],
    chunk_start: usize,
    chunk_end: usize,
    overlap: usize,
) -> usize {
    let mut chars_from_end = 0usize;
    let mut start = chunk_end;
    while start > 0 {
        start -= 1;
        chars_from_end += sentences[start].len();
        if chars_from_end >= overlap {
            break;
        }
    }
    // Ensure forward progress: the next chunk must start after chunk_start.
    // If overlap would put us at or before the current start, advance by one
    // sentence instead.
    if start <= chunk_start {
        chunk_start + 1
    } else {
        start
    }
}

/// Compute the byte offset of `sentences[idx]` within `text`.
fn byte_offset_of(text: &str, sentences: &[&str], idx: usize) -> usize {
    // Sum lengths of all prior sentences.
    let prefix: String = sentences[..idx].concat();
    // Find the actual byte position — sentence boundaries may have
    // whitespace that concat skips, so search for the first sentence
    // in the text after the prefix length.
    text.find(&sentences[idx][..sentences[idx].len().min(40)])
        .unwrap_or(prefix.len())
        .max(if idx == 0 { 0 } else { prefix.len().saturating_sub(idx) })
}

/// Fallback chunker for text with no detectable sentence boundaries.
/// Splits on raw character boundaries with overlap.
fn chunk_raw(
    text: &str,
    source_path: &str,
    agent_id: &str,
    config: &ChunkerConfig,
) -> Vec<DocChunk> {
    let mut chunks = Vec::new();
    let bytes = text.as_bytes();
    let mut pos = 0usize;

    while pos < bytes.len() {
        let end = (pos + config.chunk_size).min(bytes.len());
        // Snap to char boundary.
        let end = snap_to_char_boundary(text, end);
        let content = &text[pos..end];

        let chunk_id = format!("{}:{}:{}", agent_id, source_path, pos);
        chunks.push(DocChunk {
            id: chunk_id,
            content: content.to_string(),
            source_path: source_path.to_string(),
            byte_offset: pos,
            agent_id: agent_id.to_string(),
        });

        if end >= bytes.len() {
            break;
        }

        let next = if end > config.overlap {
            end - config.overlap
        } else {
            end
        };
        // Ensure forward progress.
        pos = snap_to_char_boundary(text, next).max(pos + 1);
    }

    chunks
}

/// Round a byte position up to the nearest UTF-8 character boundary.
fn snap_to_char_boundary(text: &str, pos: usize) -> usize {
    if pos >= text.len() {
        return text.len();
    }
    let mut p = pos;
    while !text.is_char_boundary(p) && p < text.len() {
        p += 1;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_produces_no_chunks() {
        let chunks = chunk_text("", "file.md", "agent1", &ChunkerConfig::default());
        assert!(chunks.is_empty());
    }

    #[test]
    fn short_text_produces_single_chunk() {
        let text = "Hello world. This is a test.";
        let chunks = chunk_text(text, "file.md", "agent1", &ChunkerConfig::default());
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].agent_id, "agent1");
        assert_eq!(chunks[0].source_path, "file.md");
    }

    #[test]
    fn long_text_produces_multiple_chunks() {
        // Create text with many sentences that exceeds chunk_size.
        let sentence = "This is a test sentence with some content. ";
        let text: String = sentence.repeat(100); // ~4400 chars

        let config = ChunkerConfig {
            chunk_size: 500,
            overlap: 100,
        };
        let chunks = chunk_text(&text, "docs/big.md", "agent1", &config);

        assert!(chunks.len() > 1, "expected multiple chunks, got {}", chunks.len());

        // Every chunk should have the correct agent_id and source_path.
        for chunk in &chunks {
            assert_eq!(chunk.agent_id, "agent1");
            assert_eq!(chunk.source_path, "docs/big.md");
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn chunks_have_unique_ids() {
        let sentence = "Sentence number one. Sentence number two. Sentence number three. ";
        let text: String = sentence.repeat(50);

        let config = ChunkerConfig {
            chunk_size: 300,
            overlap: 50,
        };
        let chunks = chunk_text(&text, "file.md", "agent1", &config);

        let ids: Vec<&str> = chunks.iter().map(|c| c.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "chunk IDs must be unique");
    }

    #[test]
    fn raw_fallback_handles_no_sentences() {
        // Text with no sentence-ending punctuation.
        let text = "abcdef".repeat(500);
        let config = ChunkerConfig {
            chunk_size: 200,
            overlap: 50,
        };
        let chunks = chunk_text(&text, "raw.txt", "a", &config);
        assert!(chunks.len() > 1);
        // First chunk should start at offset 0.
        assert_eq!(chunks[0].byte_offset, 0);
    }

    #[test]
    fn chunk_config_defaults_are_reasonable() {
        let config = ChunkerConfig::default();
        assert_eq!(config.chunk_size, 2000);
        assert_eq!(config.overlap, 200);
    }

    #[test]
    fn snap_to_char_boundary_works() {
        let text = "hello";
        assert_eq!(snap_to_char_boundary(text, 0), 0);
        assert_eq!(snap_to_char_boundary(text, 5), 5);
        assert_eq!(snap_to_char_boundary(text, 100), 5);
    }

    #[test]
    fn multibyte_text_does_not_panic() {
        let text = "Héllo wörld. Ünïcödé text hëre. Another séntence.";
        let config = ChunkerConfig {
            chunk_size: 20,
            overlap: 5,
        };
        let chunks = chunk_text(text, "utf8.md", "a", &config);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            // Verify content is valid UTF-8 (it is, since it's a String).
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn single_oversized_sentence_falls_back_to_raw_chunking() {
        // One long sentence (no periods except at end) that exceeds chunk_size.
        // Without the fix, this would return a single chunk with all content.
        let text = "This is one very long sentence without any breaks ".repeat(20);
        let config = ChunkerConfig {
            chunk_size: 100,
            overlap: 20,
        };
        let chunks = chunk_text(&text, "long.txt", "a", &config);

        // Should produce multiple chunks via raw fallback, not one huge chunk.
        assert!(
            chunks.len() > 1,
            "expected multiple chunks from raw fallback, got {}",
            chunks.len()
        );

        // Each chunk should respect approximate chunk_size (with some tolerance for overlap).
        for chunk in &chunks {
            assert!(
                chunk.content.len() <= config.chunk_size + config.overlap,
                "chunk too large: {} bytes",
                chunk.content.len()
            );
        }
    }

    #[test]
    fn sentences_larger_than_chunk_size_do_not_infinite_loop() {
        // Multiple sentences, each larger than chunk_size.
        // Without the fix to find_overlap_start, this would infinite loop.
        let text = "First sentence that is definitely longer than twenty chars. \
                    Second sentence also exceeds the small chunk size limit. \
                    Third sentence completes our test of the overlap logic.";
        let config = ChunkerConfig {
            chunk_size: 20,
            overlap: 10,
        };

        // This should complete without hanging.
        let chunks = chunk_text(text, "big_sentences.md", "a", &config);

        assert!(!chunks.is_empty(), "should produce at least one chunk");
        // Verify we covered all sentences (content should span the text).
        let total_unique_content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            total_unique_content.contains("First") && total_unique_content.contains("Third"),
            "chunks should cover the full text"
        );
    }

    #[test]
    fn find_overlap_start_guarantees_forward_progress() {
        let sentences = &["Short. ", "Another short one. ", "And a third. "];

        // When overlap would put us at or before chunk_start, we must advance.
        let result = find_overlap_start(sentences, 1, 2, 1000);
        assert!(
            result > 1,
            "find_overlap_start must return > chunk_start, got {}",
            result
        );

        // Normal case: overlap within bounds.
        let result = find_overlap_start(sentences, 0, 2, 5);
        assert!(
            result > 0,
            "find_overlap_start must return > chunk_start, got {}",
            result
        );
    }

    #[test]
    fn overlap_larger_than_chunk_still_progresses() {
        // Edge case: overlap >= chunk_size (misconfiguration, but shouldn't hang).
        let text = "One sentence here. Two sentence here. Three sentence here.";
        let config = ChunkerConfig {
            chunk_size: 20,
            overlap: 50, // overlap larger than chunk_size
        };

        let chunks = chunk_text(text, "edge.md", "a", &config);
        assert!(!chunks.is_empty(), "should produce chunks even with large overlap");
    }
}
