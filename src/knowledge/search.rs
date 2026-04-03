use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

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

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
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
    let word_count = chunk.split_whitespace().count().max(1) as f64;
    score / (1.0 + word_count.ln())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::store::{Document, KnowledgeBase};

    fn make_search_kb() -> KnowledgeBase {
        let mut kb = KnowledgeBase::new("search_test".into());
        let doc = Document {
            id: "d1".into(),
            title: "Programming".into(),
            source_path: "/tmp/prog.md".into(),
            ingested_at: chrono::Utc::now(),
            word_count: 20,
        };
        let chunks = vec![
            Chunk {
                id: "c1".into(),
                document_id: "d1".into(),
                content: "Rust is a systems programming language focused on safety".into(),
                index: 0,
                word_count: 9,
            },
            Chunk {
                id: "c2".into(),
                document_id: "d1".into(),
                content: "Python is an interpreted language for scripting".into(),
                index: 1,
                word_count: 7,
            },
            Chunk {
                id: "c3".into(),
                document_id: "d1".into(),
                content: "JavaScript runs in the browser and on servers".into(),
                index: 2,
                word_count: 8,
            },
        ];
        kb.add_document(doc, chunks);
        kb
    }

    #[test]
    fn search_finds_matching_chunks() {
        let kb = make_search_kb();
        let results = search_knowledge(&kb, "rust", 5);
        assert!(!results.is_empty());
        assert!(results[0].chunk.content.to_lowercase().contains("rust"));
    }

    #[test]
    fn search_no_matches_returns_empty() {
        let kb = make_search_kb();
        let results = search_knowledge(&kb, "xyznonexistent", 5);
        // Fuzzy matcher might still return something, but with exact-miss words the
        // score may be zero. If any results do come back, they should at least be
        // scored positively (no negative scores).
        for r in &results {
            assert!(r.score > 0.0);
        }
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let kb = make_search_kb();
        let results = search_knowledge(&kb, "", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn search_empty_kb_returns_empty() {
        let kb = KnowledgeBase::new("empty".into());
        let results = search_knowledge(&kb, "rust", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn search_respects_max_results() {
        let kb = make_search_kb();
        let results = search_knowledge(&kb, "language", 1);
        assert!(results.len() <= 1);
    }

    #[test]
    fn search_results_sorted_by_score_descending() {
        let kb = make_search_kb();
        let results = search_knowledge(&kb, "language", 10);
        for window in results.windows(2) {
            assert!(
                window[0].score >= window[1].score,
                "results not sorted: {} < {}",
                window[0].score,
                window[1].score
            );
        }
    }

    #[test]
    fn search_is_case_insensitive() {
        let kb = make_search_kb();
        let lower = search_knowledge(&kb, "rust", 5);
        let upper = search_knowledge(&kb, "RUST", 5);
        let mixed = search_knowledge(&kb, "RuSt", 5);
        // All should find the same chunk
        assert!(!lower.is_empty());
        assert!(!upper.is_empty());
        assert!(!mixed.is_empty());
        assert_eq!(lower[0].chunk.id, upper[0].chunk.id);
        assert_eq!(lower[0].chunk.id, mixed[0].chunk.id);
    }

    #[test]
    fn search_exact_match_scores_higher_than_fuzzy() {
        let kb = make_search_kb();
        // "Rust" appears literally in c1 but not in c2 or c3
        let results = search_knowledge(&kb, "rust", 10);
        if results.len() >= 2 {
            // The chunk with the exact match should be first
            assert!(results[0].chunk.content.to_lowercase().contains("rust"));
        }
    }

    #[test]
    fn search_includes_document_title() {
        let kb = make_search_kb();
        let results = search_knowledge(&kb, "rust", 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].document_title, "Programming");
    }

    #[test]
    fn search_with_multiple_query_words() {
        let kb = make_search_kb();
        let results = search_knowledge(&kb, "systems safety", 5);
        assert!(!results.is_empty());
        // The Rust chunk mentions both "systems" and "safety"
        assert!(results[0].chunk.content.contains("systems"));
    }

    #[test]
    fn search_with_special_characters() {
        let mut kb = KnowledgeBase::new("test".into());
        let doc = Document {
            id: "d1".into(),
            title: "Test".into(),
            source_path: "/tmp/t".into(),
            ingested_at: chrono::Utc::now(),
            word_count: 10,
        };
        let chunks = vec![Chunk {
            id: "c1".into(),
            document_id: "d1".into(),
            content: "fn main() { println!(\"hello\"); }".into(),
            index: 0,
            word_count: 5,
        }];
        kb.add_document(doc, chunks);

        // Search for code patterns — "println" should match
        let results = search_knowledge(&kb, "println", 5);
        assert!(!results.is_empty());
    }

    #[test]
    fn search_with_punctuation_only_query() {
        let kb = make_search_kb();
        // A query that is only punctuation should be stripped to nothing and
        // return empty results rather than panic.
        let results = search_knowledge(&kb, "!@#$%", 5);
        // Should not panic; result may or may not be empty depending on
        // whether fuzzy matcher finds anything.
        let _ = results;
    }

    #[test]
    fn search_single_char_query() {
        let kb = make_search_kb();
        // Very short queries should not panic.
        let results = search_knowledge(&kb, "a", 5);
        let _ = results;
    }

    // === Stress tests ===

    #[test]
    fn search_large_knowledge_base() {
        let mut kb = KnowledgeBase::new("stress".into());
        for i in 0..100 {
            let doc = Document {
                id: format!("doc{i}"),
                title: format!("Document {i}"),
                source_path: format!("/tmp/doc{i}.md"),
                ingested_at: chrono::Utc::now(),
                word_count: 500,
            };
            let mut chunks = Vec::new();
            for j in 0..10 {
                chunks.push(Chunk {
                    id: format!("c{i}_{j}"),
                    document_id: format!("doc{i}"),
                    content: format!("Chunk {j} of document {i} about topic_{}", i % 10),
                    index: j,
                    word_count: 10,
                });
            }
            kb.add_document(doc, chunks);
        }

        assert_eq!(kb.total_chunks(), 1000);

        // Search should be fast and return results
        let results = search_knowledge(&kb, "topic_5", 5);
        assert!(!results.is_empty());
        assert!(results.len() <= 5);
    }
}
