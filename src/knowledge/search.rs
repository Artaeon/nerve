use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use super::store::{Chunk, KnowledgeBase};

/// A single search result with its source chunk, document title, and relevance
/// score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk: Chunk,
    pub document_title: String,
    pub score: f64,
}

/// Search the knowledge base for chunks relevant to the given query.
///
/// Returns up to `max_results` results sorted by descending relevance score.
pub fn search_knowledge(kb: &KnowledgeBase, query: &str, max_results: usize) -> Vec<SearchResult> {
    let query_words: Vec<&str> = query
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect();

    if query_words.is_empty() || kb.chunks.is_empty() {
        return Vec::new();
    }

    let matcher = SkimMatcherV2::default();

    let mut results: Vec<SearchResult> = kb
        .chunks
        .iter()
        .filter_map(|chunk| {
            let score = calculate_score(&chunk.content, &query_words, &matcher);
            if score <= 0.0 {
                return None;
            }
            // Look up the document title for this chunk.
            let document_title = kb
                .documents
                .iter()
                .find(|d| d.id == chunk.document_id)
                .map(|d| d.title.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            Some(SearchResult {
                chunk: chunk.clone(),
                document_title,
                score,
            })
        })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(max_results);
    results
}

/// Calculate a relevance score for a chunk against the query words.
///
/// Uses exact keyword matching (weighted heavily) plus fuzzy matching via Skim.
/// The score is normalised by chunk length so shorter, more focused chunks rank
/// higher.
fn calculate_score(chunk: &str, query_words: &[&str], matcher: &SkimMatcherV2) -> f64 {
    let chunk_lower = chunk.to_lowercase();
    let mut score = 0.0;

    for word in query_words {
        let word_lower = word.to_lowercase();

        // Exact word match.
        if chunk_lower.contains(&word_lower) {
            score += 10.0;
        }

        // Fuzzy match on the whole chunk.
        if let Some(fuzzy_score) = matcher.fuzzy_match(&chunk_lower, &word_lower) {
            score += fuzzy_score as f64 / 100.0;
        }
    }

    // Normalise by chunk length (shorter, more focused chunks rank higher).
    let word_count = chunk.split_whitespace().count() as f64;
    score / (1.0 + word_count.ln())
}
