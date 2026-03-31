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
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
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
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
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
fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
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
