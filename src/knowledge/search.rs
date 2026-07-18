use std::collections::HashMap;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use super::store::{Chunk, KnowledgeBase};

/// BM25 term-frequency saturation constant — the standard default (used by
/// Lucene/Elasticsearch). Higher values let repeated matches keep adding
/// score for longer before saturating.
const BM25_K1: f64 = 1.5;

/// BM25 length-normalisation weight — the standard default. 0 disables length
/// normalisation entirely; 1 applies it fully. This (not an unbounded log
/// penalty) is what lets a long, genuinely relevant chunk still clear a fixed
/// score threshold: length can pull a score down towards a floor, but never
/// below it, unlike `score / ln(word_count)` which has no floor at all.
const BM25_B: f64 = 0.75;

/// Weight applied to an exact (word-boundary) match, scaled by IDF and BM25
/// length normalisation.
const EXACT_MATCH_WEIGHT: f64 = 10.0;

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

    // Corpus-wide statistics feed BM25: the average chunk length (for
    // saturating length normalisation) and, per query word, inverse document
    // frequency — a term that shows up in nearly every chunk ("project",
    // "nerve") is worth far less than one that shows up in only a handful
    // ("EXCLUDE constraint").
    let total_chunks = kb.chunks.len();
    let avg_word_count = kb
        .chunks
        .iter()
        .map(|c| c.content.split_whitespace().count().max(1) as f64)
        .sum::<f64>()
        / total_chunks as f64;
    let idf_by_word: HashMap<String, f64> = query_words
        .iter()
        .map(|w| {
            let word_lower = w.to_lowercase();
            let df = kb
                .chunks
                .iter()
                .filter(|c| contains_word(&c.content.to_lowercase(), &word_lower))
                .count();
            (word_lower, idf(df, total_chunks))
        })
        .collect();

    let mut results: Vec<SearchResult> = kb
        .chunks
        .iter()
        .filter_map(|chunk| {
            let score = calculate_score(
                &chunk.content,
                &query_words,
                &matcher,
                &idf_by_word,
                avg_word_count,
            );
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

/// Inverse document frequency, BM25-style ("+1" variant, which keeps the
/// value non-negative even when a term appears in every document — a plain
/// `ln(n/df)` goes negative there, which would let a common word SUBTRACT
/// score). `df` is how many chunks in the corpus contain the term at least
/// once; `n` is the total chunk count. A term in nearly every chunk scores
/// close to zero; a term in only a handful of chunks scores much higher —
/// this is what lets a decisive keyword like "EXCLUDE constraint" outweigh a
/// common one like "project".
fn idf(df: usize, n: usize) -> f64 {
    let n = n.max(1) as f64;
    let df = df as f64;
    (((n - df + 0.5) / (df + 0.5)) + 1.0).ln()
}

/// A character that counts as part of a "word" for exact matching: letters,
/// digits, and underscore (so identifier-style tokens like `topic_5` are one
/// word, not two).
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// True when `word` occurs in `text` as a whole word, not merely as a
/// substring of a longer word — the query word `ui` must not match inside
/// "build", "require", or "guide". Both arguments must already be lowercased.
fn contains_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !is_word_char(c)).any(|w| w == word)
}

/// Count how many times `word` occurs in `text` as a whole word — feeds BM25
/// term-frequency saturation. Both arguments must already be lowercased.
fn word_occurrences(text: &str, word: &str) -> usize {
    text.split(|c: char| !is_word_char(c))
        .filter(|w| *w == word)
        .count()
}

/// Calculate a relevance score for a chunk against the query words.
///
/// BM25-style: each query word's contribution is its corpus-wide IDF (rare
/// terms count for more than common ones) times a term-frequency term that
/// SATURATES rather than growing without bound, normalised by this chunk's
/// length relative to the corpus's AVERAGE length (`avg_word_count`). This
/// saturating normalisation — not the old `score / (1.0 + word_count.ln())` —
/// is what lets a long, genuinely relevant chunk still clear a fixed score
/// threshold: that formula punished length with no floor (a decisive 100-word
/// match scored `10 / (1 + ln 100) = 1.78`, below a 2.5 auto-recall threshold,
/// while a 5-word chunk that only vaguely mentioned the same word scored
/// 3.83 — the detailed entry lost to the vague one on every decisive match).
/// Falls back to fuzzy (subsequence/typo) matching only when a word has no
/// exact match, and weights that fallback small so it can never outscore a
/// real word-boundary match.
fn calculate_score(
    chunk: &str,
    query_words: &[&str],
    matcher: &SkimMatcherV2,
    idf_by_word: &HashMap<String, f64>,
    avg_word_count: f64,
) -> f64 {
    let chunk_lower = chunk.to_lowercase();
    let doc_len = chunk.split_whitespace().count().max(1) as f64;
    let length_norm = 1.0 - BM25_B + BM25_B * (doc_len / avg_word_count.max(1.0));
    let mut score = 0.0;

    for word in query_words {
        let word_lower = word.to_lowercase();
        let term_idf = idf_by_word.get(&word_lower).copied().unwrap_or(1.0);
        let tf = word_occurrences(&chunk_lower, &word_lower) as f64;

        if tf > 0.0 {
            let saturated = (tf * (BM25_K1 + 1.0)) / (tf + BM25_K1 * length_norm);
            score += EXACT_MATCH_WEIGHT * term_idf * saturated;
        } else if let Some(fuzzy_score) = matcher.fuzzy_match(&chunk_lower, &word_lower) {
            // Fuzzy fallback for typos/near-misses only — kept an order of
            // magnitude below an exact match so a subsequence hit (e.g. "ui"
            // as a subsequence of "build") can never outrank a real
            // word-boundary match (e.g. "ui" as its own word).
            score += (fuzzy_score as f64 / 100.0) * term_idf.max(1.0) * 0.1;
        }
    }

    score
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

    /// Score a single document against a hand-built corpus, mirroring what
    /// `search_knowledge` does internally (per-word IDF over the corpus, BM25
    /// length normalisation against the corpus average) without needing a
    /// full `KnowledgeBase`. Include `doc` in `corpus` when the test cares
    /// about realistic length normalisation.
    fn score_for_test(query: &str, doc: &str, corpus: &[&str]) -> f64 {
        let matcher = SkimMatcherV2::default();
        let query_words: Vec<&str> = query
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty())
            .collect();
        let total = corpus.len().max(1);
        let avg_word_count = corpus
            .iter()
            .map(|c| c.split_whitespace().count().max(1) as f64)
            .sum::<f64>()
            / total as f64;
        let idf_by_word: HashMap<String, f64> = query_words
            .iter()
            .map(|w| {
                let word_lower = w.to_lowercase();
                let df = corpus
                    .iter()
                    .filter(|c| contains_word(&c.to_lowercase(), &word_lower))
                    .count();
                (word_lower, idf(df, total))
            })
            .collect();
        calculate_score(doc, &query_words, &matcher, &idf_by_word, avg_word_count)
    }

    #[test]
    fn score_empty_chunk_does_not_panic() {
        // An empty chunk has 0 words — division by avg word count must not
        // panic or produce a non-finite score.
        let score = score_for_test("anything", "", &[""]);
        assert!(score.is_finite());
    }

    #[test]
    fn score_whitespace_only_chunk_does_not_panic() {
        let score = score_for_test("test", "   \n\t  ", &["   \n\t  "]);
        assert!(score.is_finite());
    }

    #[test]
    fn shorter_focused_chunk_scores_higher() {
        let short = "rust programming";
        let long = "rust is a systems programming language that runs blazingly fast and prevents segfaults";
        let corpus = [short, long];
        let short_score = score_for_test("rust", short, &corpus);
        let long_score = score_for_test("rust", long, &corpus);
        // BM25 length normalisation still favours the shorter, more focused
        // chunk relative to the corpus average — but boundedly (see the
        // BM25_B comment on `calculate_score`), never with the old unbounded
        // log penalty that made a genuinely relevant long chunk unrecoverable.
        assert!(
            short_score >= long_score,
            "short={short_score}, long={long_score}: shorter chunk should score >= longer"
        );
    }

    #[test]
    fn exact_match_scores_positive() {
        let score = score_for_test("rust", "rust is great", &["rust is great"]);
        assert!(
            score > 0.0,
            "exact keyword match should be positive: {score}"
        );
    }

    #[test]
    fn no_match_scores_zero_or_near_zero() {
        let no_match = "python django flask";
        let exact = "rust programming language";
        let corpus = [no_match, exact];
        let score = score_for_test("rust", no_match, &corpus);
        let exact_score = score_for_test("rust", exact, &corpus);
        assert!(
            score < exact_score,
            "no-match={score} should be less than exact={exact_score}"
        );
    }

    #[test]
    fn score_is_additive_per_query_word() {
        // Each query word contributes independently: the same word listed
        // twice yields exactly double the score of listing it once — both
        // occurrences look up the same corpus IDF and the same saturated
        // term-frequency term for a fixed chunk.
        let doc = "rust programming";
        let corpus = [doc];
        let single = score_for_test("rust", doc, &corpus);
        let double = score_for_test("rust rust", doc, &corpus);
        assert!(single > 0.0);
        assert!(
            (double - 2.0 * single).abs() < 1e-9,
            "double={double} should be exactly 2x single={single}"
        );
    }

    #[test]
    fn score_counts_word_presence_not_frequency_in_chunk() {
        // Unlike the old scheme, BM25 term frequency DOES reward a second
        // occurrence of the same word — but it SATURATES (see BM25_K1), so the
        // gap between "mentioned once" and "mentioned twice" stays small
        // relative to a full extra exact-match weight.
        let once = "rust alpha beta gamma";
        let twice = "rust rust beta gamma";
        let corpus = [once, twice];
        let once_score = score_for_test("rust", once, &corpus);
        let twice_score = score_for_test("rust", twice, &corpus);
        assert!(
            (once_score - twice_score).abs() < 5.0,
            "once={once_score} twice={twice_score}"
        );
    }

    #[test]
    fn a_detailed_entry_outranks_a_vague_one_on_a_decisive_term() {
        // The detailed entry is long and genuinely about the query. The vague
        // one is short and merely mentions a common word. Today the length
        // penalty puts the vague one on top; that is the bug.
        let detailed = "The appointments table uses a Postgres EXCLUDE constraint \
            with btree_gist so two overlapping bookings for the same studio are \
            rejected by the database itself rather than by application logic, which \
            is what makes the double booking fix correct under concurrency and not \
            merely unlikely under load in practice today";
        let vague = "the project uses a database";
        let corpus = [detailed, vague];
        let d = score_for_test("EXCLUDE constraint", detailed, &corpus);
        let v = score_for_test("EXCLUDE constraint", vague, &corpus);
        assert!(d > v, "detailed {d} must outrank vague {v}");
    }

    #[test]
    fn a_long_relevant_entry_clears_the_auto_recall_threshold() {
        // The regression that made memory write-only: a rich entry scored 1.78
        // against a threshold of 2.5 and was silently never injected.
        let detailed = "The appointments table uses a Postgres EXCLUDE constraint \
            with btree_gist so two overlapping bookings for the same studio are \
            rejected by the database itself rather than by application logic, which \
            is what makes the double booking fix correct under concurrency and not \
            merely unlikely under load in practice today";
        let corpus = [detailed, "the project uses a database"];
        let s = score_for_test("EXCLUDE constraint", detailed, &corpus);
        assert!(
            s >= crate::memory_recall::AUTO_RECALL_MIN_SCORE,
            "detailed entry scored {s}, below the auto-recall threshold"
        );
    }

    #[test]
    fn a_query_word_does_not_match_inside_an_unrelated_word() {
        // `ui` must not match "build" / "require" / "guide".
        let corpus = ["we build and require the guide", "the ui components"];
        let bogus = score_for_test("ui", "we build and require the guide", &corpus);
        let real = score_for_test("ui", "the ui components", &corpus);
        assert!(real > bogus, "substring match beat a real word match");
    }
}
