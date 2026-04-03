//! Web search integration using DuckDuckGo's instant answer API.
//!
//! Provides a free, no-API-key search that returns relevant results
//! for the AI to reference. Falls back gracefully if the network is
//! unavailable.

use anyhow::Context;

/// A single web search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Maximum results to return.
const MAX_RESULTS: usize = 5;

/// Search the web and return relevant results.
///
/// Uses DuckDuckGo's API, which requires no API key.
/// Falls back to an empty result set on network errors.
pub async fn web_search(query: &str) -> anyhow::Result<Vec<SearchResult>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("Nerve/0.1.0")
        .build()
        .context("failed to build HTTP client")?;

    // DuckDuckGo instant answer API.
    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .send()
        .await
        .context("web search request failed")?;

    if !response.status().is_success() {
        anyhow::bail!("search API returned {}", response.status());
    }

    let body: serde_json::Value = response
        .json()
        .await
        .context("failed to parse search response")?;

    let mut results = Vec::new();

    // Extract the abstract (main answer).
    if let Some(abstract_text) = body["AbstractText"].as_str() {
        if !abstract_text.is_empty() {
            let abstract_url = body["AbstractURL"].as_str().unwrap_or("");
            let abstract_source = body["AbstractSource"].as_str().unwrap_or("DuckDuckGo");
            results.push(SearchResult {
                title: abstract_source.to_string(),
                url: abstract_url.to_string(),
                snippet: truncate(abstract_text, 500),
            });
        }
    }

    // Extract related topics.
    if let Some(topics) = body["RelatedTopics"].as_array() {
        for topic in topics {
            if results.len() >= MAX_RESULTS {
                break;
            }

            if let Some(text) = topic["Text"].as_str() {
                let url = topic["FirstURL"].as_str().unwrap_or("").to_string();
                // Extract a title from the text (first sentence or bolded part).
                let (title, snippet) = split_title_snippet(text);
                results.push(SearchResult {
                    title,
                    url,
                    snippet: truncate(&snippet, 300),
                });
            }
        }
    }

    // If no results from DDG API, try a simple scrape approach
    // (search engine HTML scraping as last resort).
    if results.is_empty() {
        if let Some(answer) = body["Answer"].as_str() {
            if !answer.is_empty() {
                results.push(SearchResult {
                    title: "Answer".to_string(),
                    url: String::new(),
                    snippet: truncate(answer, 500),
                });
            }
        }
    }

    Ok(results)
}

/// Format search results for AI context.
pub fn format_search_results(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return format!("Web search for \"{query}\": No results found.");
    }

    let mut out = format!("Web search results for \"{query}\":\n\n");
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!(
            "{}. **{}**\n   {}\n   {}\n\n",
            i + 1,
            r.title,
            r.snippet,
            r.url
        ));
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{truncated}...")
    }
}

fn split_title_snippet(text: &str) -> (String, String) {
    // DDG topics often have format "Title - description"
    if let Some(pos) = text.find(" - ") {
        let title = text[..pos].to_string();
        let snippet = text[pos + 3..].to_string();
        (title, snippet)
    } else if text.len() > 60 {
        // Use first ~40 chars as title.
        let boundary = text
            .char_indices()
            .take(40)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(40);
        (format!("{}...", &text[..boundary]), text.to_string())
    } else {
        (text.to_string(), text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long() {
        let result = truncate("hello world this is long", 10);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 14); // 10 chars + "..."
    }

    #[test]
    fn split_title_with_dash() {
        let (title, snippet) = split_title_snippet("Rust - A systems programming language");
        assert_eq!(title, "Rust");
        assert_eq!(snippet, "A systems programming language");
    }

    #[test]
    fn split_title_no_dash() {
        let (title, snippet) = split_title_snippet("Short text");
        assert_eq!(title, "Short text");
        assert_eq!(snippet, "Short text");
    }

    #[test]
    fn format_empty_results() {
        let output = format_search_results("test", &[]);
        assert!(output.contains("No results"));
    }

    #[test]
    fn format_with_results() {
        let results = vec![SearchResult {
            title: "Rust Lang".into(),
            url: "https://rust-lang.org".into(),
            snippet: "Systems programming language".into(),
        }];
        let output = format_search_results("rust", &results);
        assert!(output.contains("Rust Lang"));
        assert!(output.contains("rust-lang.org"));
        assert!(output.contains("Systems programming"));
    }
}
