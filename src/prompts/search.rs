//! BM25 ranking over SmartPrompts for the `/suggest` command.
//!
//! Tokenises a concatenation of `name`, `description`, `category`, and
//! `tags` for each prompt, then ranks the corpus against a free-form
//! query with standard BM25. Pure in-process — no network, no model
//! download, no allocation beyond the per-query working set.

use std::collections::HashMap;

use super::{SmartPrompt, all_prompts};

// BM25 defaults. k1 controls term-frequency saturation; b controls
// length-normalisation strength. These are the canonical Robertson/
// Walker values and perform well on short documents like our prompts.
const K1: f64 = 1.5;
const B: f64 = 0.75;

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|s| {
            if s.len() >= 2 {
                Some(s.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Concatenate the searchable fields of a prompt into a single token stream.
fn prompt_text(p: &SmartPrompt) -> String {
    // Name + tags are weighted higher by repetition — BM25 responds to
    // term frequency so duplicating those fields amounts to a boost.
    format!(
        "{name} {name} {name} {desc} {category} {tags}",
        name = p.name,
        desc = p.description,
        category = p.category,
        tags = p.tags.join(" "),
    )
}

struct Doc {
    tokens: Vec<String>,
    length: f64,
}

struct Index {
    docs: Vec<Doc>,
    df: HashMap<String, usize>,
    avg_dl: f64,
    n: f64,
}

impl Index {
    fn build(prompts: &[SmartPrompt]) -> Self {
        let docs: Vec<Doc> = prompts
            .iter()
            .map(|p| {
                let tokens = tokenize(&prompt_text(p));
                let length = tokens.len() as f64;
                Doc { tokens, length }
            })
            .collect();

        let mut df: HashMap<String, usize> = HashMap::new();
        for doc in &docs {
            // Document frequency: unique tokens per doc.
            let mut seen: HashMap<&String, ()> = HashMap::new();
            for tok in &doc.tokens {
                if seen.insert(tok, ()).is_none() {
                    *df.entry(tok.clone()).or_insert(0) += 1;
                }
            }
        }

        let n = docs.len() as f64;
        let avg_dl = if docs.is_empty() {
            0.0
        } else {
            docs.iter().map(|d| d.length).sum::<f64>() / n
        };

        Self {
            docs,
            df,
            avg_dl,
            n,
        }
    }

    fn score(&self, query_terms: &[String], doc_idx: usize) -> f64 {
        let doc = &self.docs[doc_idx];
        if doc.tokens.is_empty() {
            return 0.0;
        }
        // Pre-compute term frequency for this doc.
        let mut tf: HashMap<&String, f64> = HashMap::new();
        for tok in &doc.tokens {
            *tf.entry(tok).or_insert(0.0) += 1.0;
        }

        let dl = doc.length;
        let avg_dl = self.avg_dl.max(1.0);
        let mut score = 0.0;
        for term in query_terms {
            let df = *self.df.get(term).unwrap_or(&0) as f64;
            if df == 0.0 {
                continue;
            }
            // BM25+ smoothing via `+ 1.0` inside the log to keep IDF
            // non-negative even for terms appearing in > N/2 documents.
            let idf = ((self.n - df + 0.5) / (df + 0.5) + 1.0).ln();
            let f = *tf.get(term).unwrap_or(&0.0);
            let numerator = f * (K1 + 1.0);
            let denominator = f + K1 * (1.0 - B + B * dl / avg_dl);
            if denominator > 0.0 {
                score += idf * numerator / denominator;
            }
        }
        score
    }
}

/// Rank all known prompts (built-in + custom) against a free-form query.
/// Returns the top `top_k` matches by BM25 score, cloned so callers can
/// move them freely. Matches with a non-positive score are dropped —
/// an empty result means no prompt shares a single tokenised term with
/// the query.
pub fn suggest(query: &str, top_k: usize) -> Vec<(SmartPrompt, f64)> {
    let prompts = all_prompts();
    if prompts.is_empty() {
        return Vec::new();
    }
    let query_terms = tokenize(query);
    if query_terms.is_empty() {
        return Vec::new();
    }
    let index = Index::build(&prompts);

    let mut scored: Vec<(usize, f64)> = (0..prompts.len())
        .map(|i| (i, index.score(&query_terms, i)))
        .filter(|(_, s)| *s > 0.0)
        .collect();
    // Sort descending by score; tie-break on original order for stability.
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    scored.truncate(top_k);
    scored
        .into_iter()
        .map(|(i, s)| (prompts[i].clone(), s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_drops_single_chars_and_punctuation() {
        let toks = tokenize("Fix a bug in the code!");
        assert!(toks.contains(&"fix".into()));
        assert!(toks.contains(&"bug".into()));
        assert!(toks.contains(&"code".into()));
        // "a" and "in" are single/short terms — dropped by the len>=2 filter
        // (only "a" here, "in" is kept since len==2).
        assert!(!toks.contains(&"a".into()));
        assert!(toks.contains(&"in".into()));
    }

    #[test]
    fn tokenize_lowercases() {
        let toks = tokenize("REFACTOR Code");
        assert!(toks.contains(&"refactor".into()));
        assert!(toks.contains(&"code".into()));
        assert!(!toks.contains(&"REFACTOR".into()));
    }

    #[test]
    fn suggest_returns_empty_for_empty_query() {
        assert!(suggest("", 5).is_empty());
        assert!(suggest("   ", 5).is_empty());
    }

    #[test]
    fn suggest_respects_top_k() {
        let results = suggest("code", 3);
        assert!(results.len() <= 3);
    }

    #[test]
    fn suggest_prefers_exact_name_match() {
        // "Fix Bug" is one of the built-in prompts. Querying for its
        // name should rank it first.
        let results = suggest("fix bug", 5);
        assert!(!results.is_empty(), "expected at least one match");
        let top = &results[0].0;
        assert_eq!(top.name, "Fix Bug");
    }

    #[test]
    fn suggest_scores_are_descending() {
        let results = suggest("explain code", 10);
        for pair in results.windows(2) {
            assert!(pair[0].1 >= pair[1].1, "scores must be sorted descending");
        }
    }

    #[test]
    fn suggest_no_match_returns_empty() {
        // Gibberish that can't overlap with any prompt text.
        let results = suggest("xyzzyqwerfloop", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn suggest_category_boost_via_description() {
        // A prompt about refactoring should outrank unrelated ones when
        // the query matches "refactor".
        let results = suggest("refactor code", 3);
        assert!(!results.is_empty());
        let top = &results[0].0;
        // The top prompt should have "refactor" in its name, description,
        // or tags — all contribute to the doc text.
        let searchable = format!(
            "{} {} {}",
            top.name.to_lowercase(),
            top.description.to_lowercase(),
            top.tags.join(" ").to_lowercase(),
        );
        assert!(
            searchable.contains("refactor"),
            "top match should relate to refactoring, got {:?}",
            top.name
        );
    }
}
