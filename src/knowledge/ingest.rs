use std::path::Path;

use chrono::Utc;

use super::store::{Chunk, Document, KnowledgeBase};

/// File extensions that are considered ingestible text.
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "txt", "md", "rs", "py", "go", "js", "ts", "toml", "yaml", "json", "html",
];

/// Maximum file size we will ingest (10 MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Default chunk size in words.
const CHUNK_SIZE: usize = 500;

/// Default overlap in words between consecutive chunks.
const CHUNK_OVERLAP: usize = 50;

/// Recursively ingest all supported files from a directory into the knowledge
/// base. Returns the number of new documents added.
pub fn ingest_directory(dir: &Path, kb: &mut KnowledgeBase) -> anyhow::Result<usize> {
    if !dir.is_dir() {
        anyhow::bail!("{} is not a directory", dir.display());
    }

    let mut count = 0;
    visit_dir(dir, kb, &mut count)?;
    Ok(count)
}

/// Walk a directory tree, skipping hidden entries and binary/oversized files.
fn visit_dir(dir: &Path, kb: &mut KnowledgeBase, count: &mut usize) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("skipping directory {}: {e}", dir.display());
            return Ok(());
        }
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip hidden files/directories (name starts with '.').
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.starts_with('.')
        {
            continue;
        }

        if path.is_dir() {
            visit_dir(&path, kb, count)?;
        } else if path.is_file() {
            match ingest_file(&path, kb) {
                Ok(n) => *count += n,
                Err(e) => {
                    tracing::warn!("skipping {}: {e}", path.display());
                }
            }
        }
    }
    Ok(())
}

/// Ingest a single file into the knowledge base. Returns 1 on success, 0 if
/// the file was skipped (unsupported extension, too large, etc.).
pub fn ingest_file(path: &Path, kb: &mut KnowledgeBase) -> anyhow::Result<usize> {
    // Check extension.
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !SUPPORTED_EXTENSIONS.contains(&ext) {
        return Ok(0);
    }

    // Check file size.
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_FILE_SIZE {
        anyhow::bail!("file exceeds 10 MB limit");
    }

    // Read content — bail on non-UTF-8 (likely binary).
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(0), // Skip binary / unreadable files.
    };

    if content.trim().is_empty() {
        return Ok(0);
    }

    let doc_id = uuid::Uuid::new_v4().to_string();
    let title = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("untitled")
        .to_string();
    let word_count = content.split_whitespace().count();

    let raw_chunks = chunk_text(&content, CHUNK_SIZE, CHUNK_OVERLAP);
    let chunks: Vec<Chunk> = raw_chunks
        .into_iter()
        .enumerate()
        .map(|(idx, text)| {
            let wc = text.split_whitespace().count();
            Chunk {
                id: uuid::Uuid::new_v4().to_string(),
                document_id: doc_id.clone(),
                content: text,
                index: idx,
                word_count: wc,
            }
        })
        .collect();

    let doc = Document {
        id: doc_id,
        title,
        source_path: path.to_string_lossy().into_owned(),
        ingested_at: Utc::now(),
        word_count,
    };

    kb.add_document(doc, chunks);
    Ok(1)
}

/// Split text into overlapping chunks of approximately `chunk_size` words.
pub(crate) fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < words.len() {
        let end = (start + chunk_size).min(words.len());
        let chunk = words[start..end].join(" ");
        chunks.push(chunk);
        start += chunk_size.saturating_sub(overlap);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_empty_text() {
        let chunks = chunk_text("", 500, 50);
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_whitespace_only() {
        let chunks = chunk_text("   \n\t  ", 500, 50);
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_short_text_single_chunk() {
        let chunks = chunk_text("hello world this is short", 500, 50);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello world this is short");
    }

    #[test]
    fn chunk_text_exactly_at_chunk_size_no_overlap() {
        // 10 words, chunk_size=10, overlap=0 => 1 chunk
        let text = "one two three four five six seven eight nine ten";
        let chunks = chunk_text(text, 10, 0);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn chunk_long_text_multiple_chunks() {
        let words: Vec<String> = (0..100).map(|i| format!("w{i}")).collect();
        let text = words.join(" ");
        let chunks = chunk_text(&text, 30, 10);
        assert!(chunks.len() > 1);
        // Each chunk should have at most 30 words
        for chunk in &chunks {
            let wc = chunk.split_whitespace().count();
            assert!(wc <= 30, "chunk has {wc} words, expected <= 30");
        }
    }

    #[test]
    fn chunk_overlap_is_correct() {
        let words: Vec<String> = (0..60).map(|i| format!("w{i}")).collect();
        let text = words.join(" ");
        let chunks = chunk_text(&text, 30, 10);
        assert!(chunks.len() >= 2);

        // Last 10 words of first chunk should appear at the start of second chunk
        let first_words: Vec<&str> = chunks[0].split_whitespace().collect();
        let second_words: Vec<&str> = chunks[1].split_whitespace().collect();
        let overlap_from_first = &first_words[first_words.len() - 10..];
        let overlap_from_second = &second_words[..10];
        assert_eq!(overlap_from_first, overlap_from_second);
    }

    #[test]
    fn chunk_very_long_text() {
        let words: Vec<&str> = (0..5000).map(|_| "word").collect();
        let text = words.join(" ");
        let chunks = chunk_text(&text, 500, 50);
        // 5000 words, step = 450, so ceil(5000/450) = 12 chunks
        // First chunk covers 0..500, next 450..950, etc.
        assert!(chunks.len() >= 10);
        // All text should be covered
        let last_chunk_words: Vec<&str> = chunks.last().unwrap().split_whitespace().collect();
        assert!(!last_chunk_words.is_empty());
    }

    #[test]
    fn supported_extension_accepted() {
        let tmp = tempfile::Builder::new()
            .suffix(".md")
            .tempfile()
            .expect("create tmp");
        std::fs::write(tmp.path(), "Hello world test content").expect("write");
        let mut kb = KnowledgeBase::new("test".into());
        let result = ingest_file(tmp.path(), &mut kb).expect("ingest");
        assert_eq!(result, 1);
        assert_eq!(kb.documents.len(), 1);
        assert!(!kb.chunks.is_empty());
    }

    #[test]
    fn unsupported_extension_skipped() {
        let tmp = tempfile::Builder::new()
            .suffix(".exe")
            .tempfile()
            .expect("create tmp");
        std::fs::write(tmp.path(), "some binary content").expect("write");
        let mut kb = KnowledgeBase::new("test".into());
        let result = ingest_file(tmp.path(), &mut kb).expect("ingest");
        assert_eq!(result, 0);
        assert!(kb.documents.is_empty());
    }

    #[test]
    fn empty_file_skipped() {
        let tmp = tempfile::Builder::new()
            .suffix(".txt")
            .tempfile()
            .expect("create tmp");
        std::fs::write(tmp.path(), "").expect("write");
        let mut kb = KnowledgeBase::new("test".into());
        let result = ingest_file(tmp.path(), &mut kb).expect("ingest");
        assert_eq!(result, 0);
    }

    #[test]
    fn ingest_file_word_count_matches() {
        let tmp = tempfile::Builder::new()
            .suffix(".txt")
            .tempfile()
            .expect("create tmp");
        std::fs::write(tmp.path(), "alpha beta gamma delta epsilon").expect("write");
        let mut kb = KnowledgeBase::new("test".into());
        ingest_file(tmp.path(), &mut kb).expect("ingest");
        assert_eq!(kb.documents[0].word_count, 5);
    }

    #[test]
    fn chunk_single_word() {
        let chunks = chunk_text("hello", 500, 50);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello");
    }

    #[test]
    fn chunk_preserves_all_words() {
        let text = "one two three four five six seven eight nine ten";
        let chunks = chunk_text(text, 5, 2);
        // All words should appear in at least one chunk
        for word in text.split_whitespace() {
            assert!(
                chunks.iter().any(|c| c.contains(word)),
                "Word '{word}' missing from chunks"
            );
        }
    }

    #[test]
    fn ingest_directory_skips_hidden() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("visible.txt"), "hello").unwrap();
        std::fs::write(dir.path().join(".hidden.txt"), "secret").unwrap();

        let mut kb = KnowledgeBase::new("test".into());
        let count = ingest_directory(dir.path(), &mut kb).unwrap();

        // Should have ingested visible.txt but not .hidden.txt
        assert_eq!(count, 1);
    }
}
