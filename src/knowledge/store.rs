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
