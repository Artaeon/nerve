use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub source_path: String,
    pub ingested_at: DateTime<Utc>,
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub document_id: String,
    pub content: String,
    pub index: usize,
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeBase {
    pub name: String,
    pub documents: Vec<Document>,
    pub chunks: Vec<Chunk>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl KnowledgeBase {
    /// Create a new, empty knowledge base with the given name.
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            name,
            documents: Vec::new(),
            chunks: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a document and its chunks to the knowledge base.
    pub fn add_document(&mut self, doc: Document, chunks: Vec<Chunk>) {
        self.documents.push(doc);
        self.chunks.extend(chunks);
        self.updated_at = Utc::now();
    }

    /// Remove a document and all its chunks by document ID.
    pub fn remove_document(&mut self, doc_id: &str) {
        self.documents.retain(|d| d.id != doc_id);
        self.chunks.retain(|c| c.document_id != doc_id);
        self.updated_at = Utc::now();
    }

    /// Return the storage directory for knowledge bases.
    fn storage_dir() -> anyhow::Result<PathBuf> {
        let base = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine local data directory"))?;
        let dir = base.join("nerve").join("knowledge");
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Persist this knowledge base to disk as JSON.
    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::storage_dir()?;
        let path = dir.join(format!("{}.json", self.name));
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load a knowledge base from disk by name.
    pub fn load(name: &str) -> anyhow::Result<Self> {
        let dir = Self::storage_dir()?;
        let path = dir.join(format!("{name}.json"));
        let data = std::fs::read_to_string(&path)?;
        let kb: Self = serde_json::from_str(&data)?;
        Ok(kb)
    }

    /// List all knowledge base names (derived from JSON filenames on disk).
    pub fn list_all() -> anyhow::Result<Vec<String>> {
        let dir = match Self::storage_dir() {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };
        let mut names = Vec::new();
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Total number of chunks in this knowledge base.
    pub fn total_chunks(&self) -> usize {
        self.chunks.len()
    }

    /// Total word count across all documents.
    pub fn total_words(&self) -> usize {
        self.documents.iter().map(|d| d.word_count).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_kb() -> KnowledgeBase {
        let mut kb = KnowledgeBase::new("test".into());
        let doc = Document {
            id: "doc1".into(),
            title: "Test Doc".into(),
            source_path: "/tmp/test.md".into(),
            ingested_at: chrono::Utc::now(),
            word_count: 100,
        };
        let chunks = vec![
            Chunk { id: "c1".into(), document_id: "doc1".into(), content: "hello world".into(), index: 0, word_count: 2 },
            Chunk { id: "c2".into(), document_id: "doc1".into(), content: "foo bar baz".into(), index: 1, word_count: 3 },
        ];
        kb.add_document(doc, chunks);
        kb
    }

    #[test]
    fn new_kb_is_empty() {
        let kb = KnowledgeBase::new("test".into());
        assert_eq!(kb.name, "test");
        assert!(kb.documents.is_empty());
        assert!(kb.chunks.is_empty());
        assert_eq!(kb.total_chunks(), 0);
        assert_eq!(kb.total_words(), 0);
    }

    #[test]
    fn new_kb_has_timestamps() {
        let before = chrono::Utc::now();
        let kb = KnowledgeBase::new("ts_test".into());
        let after = chrono::Utc::now();
        assert!(kb.created_at >= before && kb.created_at <= after);
        assert!(kb.updated_at >= before && kb.updated_at <= after);
        assert_eq!(kb.created_at, kb.updated_at);
    }

    #[test]
    fn add_document_increases_counts() {
        let kb = make_test_kb();
        assert_eq!(kb.documents.len(), 1);
        assert_eq!(kb.chunks.len(), 2);
        assert_eq!(kb.total_chunks(), 2);
        assert_eq!(kb.total_words(), 100);
    }

    #[test]
    fn add_document_updates_timestamp() {
        let mut kb = KnowledgeBase::new("ts".into());
        let initial = kb.updated_at;
        let doc = Document {
            id: "d".into(),
            title: "T".into(),
            source_path: "/tmp/t".into(),
            ingested_at: chrono::Utc::now(),
            word_count: 1,
        };
        kb.add_document(doc, vec![]);
        assert!(kb.updated_at >= initial);
    }

    #[test]
    fn remove_document_clears_chunks() {
        let mut kb = make_test_kb();
        kb.remove_document("doc1");
        assert!(kb.documents.is_empty());
        assert!(kb.chunks.is_empty());
        assert_eq!(kb.total_chunks(), 0);
        assert_eq!(kb.total_words(), 0);
    }

    #[test]
    fn remove_nonexistent_document_is_noop() {
        let mut kb = make_test_kb();
        kb.remove_document("no_such_id");
        assert_eq!(kb.documents.len(), 1);
        assert_eq!(kb.chunks.len(), 2);
    }

    #[test]
    fn total_chunks_sums_correctly() {
        let mut kb = make_test_kb();
        let doc2 = Document {
            id: "doc2".into(),
            title: "Second".into(),
            source_path: "/tmp/second.md".into(),
            ingested_at: chrono::Utc::now(),
            word_count: 50,
        };
        let chunks2 = vec![
            Chunk { id: "c3".into(), document_id: "doc2".into(), content: "alpha beta".into(), index: 0, word_count: 2 },
        ];
        kb.add_document(doc2, chunks2);
        assert_eq!(kb.total_chunks(), 3);
    }

    #[test]
    fn total_words_sums_across_documents() {
        let mut kb = make_test_kb();
        let doc2 = Document {
            id: "doc2".into(),
            title: "Second".into(),
            source_path: "/tmp/s.md".into(),
            ingested_at: chrono::Utc::now(),
            word_count: 50,
        };
        kb.add_document(doc2, vec![]);
        assert_eq!(kb.total_words(), 150);
    }

    #[test]
    fn serialization_roundtrip() {
        let kb = make_test_kb();
        let json = serde_json::to_string(&kb).expect("serialize");
        let deserialized: KnowledgeBase = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.name, kb.name);
        assert_eq!(deserialized.documents.len(), kb.documents.len());
        assert_eq!(deserialized.chunks.len(), kb.chunks.len());
        assert_eq!(deserialized.documents[0].id, "doc1");
        assert_eq!(deserialized.chunks[0].content, "hello world");
        assert_eq!(deserialized.chunks[1].content, "foo bar baz");
    }

    #[test]
    fn document_fields_preserved() {
        let now = chrono::Utc::now();
        let doc = Document {
            id: "id123".into(),
            title: "My Title".into(),
            source_path: "/some/path.rs".into(),
            ingested_at: now,
            word_count: 42,
        };
        assert_eq!(doc.id, "id123");
        assert_eq!(doc.title, "My Title");
        assert_eq!(doc.source_path, "/some/path.rs");
        assert_eq!(doc.ingested_at, now);
        assert_eq!(doc.word_count, 42);
    }

    #[test]
    fn chunk_fields_preserved() {
        let chunk = Chunk {
            id: "ch1".into(),
            document_id: "doc99".into(),
            content: "some text here".into(),
            index: 7,
            word_count: 3,
        };
        assert_eq!(chunk.id, "ch1");
        assert_eq!(chunk.document_id, "doc99");
        assert_eq!(chunk.content, "some text here");
        assert_eq!(chunk.index, 7);
        assert_eq!(chunk.word_count, 3);
    }
}
