use anyhow::Context;

#[derive(Debug, Clone)]
pub struct ScrapeResult {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    pub word_count: usize,
}

/// Maximum number of words to keep in scraped content (rough token budget).
const MAX_WORDS: usize = 4000;

/// Returns true if the URL targets a private/internal network address.
fn is_private_url(url: &str) -> bool {
    // Block file:// and other non-HTTP schemes.
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return true;
    }

    let parsed = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return true, // invalid URL — block it
    };

    let host_str = match parsed.host_str() {
        Some(h) => h.to_lowercase(),
        None => return true,
    };

    // Check domain-based names.
    if host_str == "localhost"
        || host_str.ends_with(".local")
        || host_str.ends_with(".internal")
        || host_str.ends_with(".localhost")
    {
        return true;
    }

    // Try parsing as IPv4 — handles dotted, integer, hex, and octal formats.
    if let Ok(ip) = host_str.parse::<std::net::Ipv4Addr>() {
        return is_private_ipv4(ip);
    }

    // Try parsing as IPv6 (strip brackets if present).
    let bare = host_str.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<std::net::Ipv6Addr>() {
        return is_private_ipv6(ip);
    }

    false
}

fn is_private_ipv4(ip: std::net::Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_unspecified()
        || ip.is_link_local()
        // 100.64.0.0/10 — shared address space (CGN)
        || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xC0) == 64)
        // 169.254.0.0/16 — link-local
        || ip.octets()[0] == 169 && ip.octets()[1] == 254
}

fn is_private_ipv6(ip: std::net::Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    let segs = ip.segments();
    // fe80::/10 — link-local
    if segs[0] & 0xffc0 == 0xfe80 {
        return true;
    }
    // fc00::/7 — unique local address (ULA)
    if segs[0] & 0xfe00 == 0xfc00 {
        return true;
    }
    // ff00::/8 — multicast
    if segs[0] & 0xff00 == 0xff00 {
        return true;
    }
    // ::ffff:x.x.x.x — IPv4-mapped
    if segs[..5] == [0, 0, 0, 0, 0] && segs[5] == 0xffff {
        let mapped = std::net::Ipv4Addr::new(
            (segs[6] >> 8) as u8,
            segs[6] as u8,
            (segs[7] >> 8) as u8,
            segs[7] as u8,
        );
        return is_private_ipv4(mapped);
    }
    // ::x.x.x.x — IPv4-compatible (deprecated but still valid)
    if segs[..6] == [0, 0, 0, 0, 0, 0] {
        let compat = std::net::Ipv4Addr::new(
            (segs[6] >> 8) as u8,
            segs[6] as u8,
            (segs[7] >> 8) as u8,
            segs[7] as u8,
        );
        return is_private_ipv4(compat);
    }
    false
}

/// Fetch a URL and extract readable text content from the HTML.
pub async fn scrape_url(url: &str) -> anyhow::Result<ScrapeResult> {
    if is_private_url(url) {
        anyhow::bail!("Cannot scrape private/internal URLs");
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Nerve/0.1.0")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to fetch {url}"))?;

    // Re-validate the final URL after redirects to prevent SSRF via redirect.
    if is_private_url(response.url().as_str()) {
        anyhow::bail!("Redirect target is a private/internal URL");
    }

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} when fetching {url}", response.status());
    }

    let html = response
        .text()
        .await
        .with_context(|| format!("failed to read response body from {url}"))?;

    let title = extract_title(&html);
    let text = strip_html(&html);
    let text = decode_html_entities(&text);
    let text = collapse_whitespace(&text);
    let word_count = text.split_whitespace().count();
    let content = truncate_words(&text, MAX_WORDS);

    Ok(ScrapeResult {
        url: url.to_string(),
        title,
        content,
        word_count,
    })
}

/// Fetch multiple URLs concurrently, returning results in the same order.
#[allow(dead_code)]
pub async fn scrape_urls(urls: &[&str]) -> Vec<anyhow::Result<ScrapeResult>> {
    let futures: Vec<_> = urls.iter().map(|url| scrape_url(url)).collect();
    futures::future::join_all(futures).await
}

/// Extract the content of the first `<title>` tag, if present.
fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    // Find the closing `>` of the opening tag.
    let tag_end = lower[start..].find('>')? + start + 1;
    let end = lower[tag_end..].find("</title")? + tag_end;
    let raw = &html[tag_end..end];
    let decoded = decode_html_entities(raw);
    let trimmed = decoded.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Strip HTML tags from the input, removing `<script>` and `<style>` blocks
/// entirely (including their content). Uses character-by-character parsing.
fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len() / 2);
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '<' {
            // Check if this is a script or style opening tag.
            if let Some(block_tag) = starts_with_block_tag(&chars, i) {
                // Skip everything until the matching closing tag.
                i = skip_to_closing_tag(&chars, i, &block_tag);
                continue;
            }

            // Skip past the closing `>` of this tag.
            while i < len && chars[i] != '>' {
                i += 1;
            }
            // Skip the `>` itself, and add a space to avoid words merging.
            if i < len {
                i += 1;
            }
            result.push(' ');
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// Check if position `i` in `chars` starts a `<script` or `<style` tag.
/// Returns the block tag name if so.
fn starts_with_block_tag(chars: &[char], i: usize) -> Option<String> {
    for tag in &["script", "style"] {
        let pattern: Vec<char> = format!("<{tag}").chars().collect();
        if i + pattern.len() <= chars.len() {
            let segment: String = chars[i..i + pattern.len()]
                .iter()
                .collect::<String>()
                .to_lowercase();
            if segment == format!("<{tag}") {
                // Make sure it's actually a tag (followed by space, >, or /).
                let next_idx = i + pattern.len();
                if next_idx < chars.len() {
                    let next = chars[next_idx];
                    if next == '>' || next == ' ' || next == '/' || next == '\t' || next == '\n' {
                        return Some(tag.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Skip forward from position `i` until we find and pass `</tag_name>`.
/// Returns the index just after the closing tag.
fn skip_to_closing_tag(chars: &[char], start: usize, tag_name: &str) -> usize {
    let closing = format!("</{tag_name}");
    let closing_chars: Vec<char> = closing.chars().collect();
    let len = chars.len();
    let mut i = start + 1; // skip past the initial `<`

    while i < len {
        if chars[i] == '<' && i + closing_chars.len() <= len {
            let segment: String = chars[i..i + closing_chars.len()]
                .iter()
                .collect::<String>()
                .to_lowercase();
            if segment == closing {
                // Skip to the `>` that closes this tag.
                let mut j = i + closing_chars.len();
                while j < len && chars[j] != '>' {
                    j += 1;
                }
                return j + 1; // past the `>`
            }
        }
        i += 1;
    }

    len // closing tag not found; consume rest of input
}

/// Decode common HTML entities.
fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// Collapse runs of whitespace into single spaces and trim blank lines.
fn collapse_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_newline = false;
    let mut prev_space = false;

    for ch in text.chars() {
        if ch == '\n' || ch == '\r' {
            if !prev_newline {
                result.push('\n');
                prev_newline = true;
            }
            prev_space = false;
        } else if ch.is_whitespace() {
            if !prev_space && !prev_newline {
                result.push(' ');
                prev_space = true;
            }
        } else {
            prev_newline = false;
            prev_space = false;
            result.push(ch);
        }
    }

    // Remove leading/trailing whitespace from each line.
    result
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Truncate content to approximately `max_words` words.
fn truncate_words(text: &str, max_words: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= max_words {
        return text.to_string();
    }
    let mut result = words[..max_words].join(" ");
    result.push_str("\n\n[Content truncated — showing first ");
    result.push_str(&max_words.to_string());
    result.push_str(" of ");
    result.push_str(&words.len().to_string());
    result.push_str(" words]");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_basic() {
        let html = "<p>Hello <b>world</b></p>";
        let text = strip_html(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains('<'));
    }

    #[test]
    fn test_strip_script_blocks() {
        let html = "<p>Before</p><script>var x = 1;</script><p>After</p>";
        let text = strip_html(html);
        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert!(!text.contains("var x"));
    }

    #[test]
    fn test_decode_entities() {
        let text = "Tom &amp; Jerry &lt;3 &quot;cool&quot;";
        let decoded = decode_html_entities(text);
        assert_eq!(decoded, "Tom & Jerry <3 \"cool\"");
    }

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>My Page</title></head><body></body></html>";
        assert_eq!(extract_title(html), Some("My Page".to_string()));
    }

    #[test]
    fn test_extract_title_none() {
        let html = "<html><head></head><body></body></html>";
        assert_eq!(extract_title(html), None);
    }

    #[test]
    fn test_truncate_words() {
        let text = "one two three four five";
        assert_eq!(truncate_words(text, 10), text);
        let truncated = truncate_words(text, 3);
        assert!(truncated.starts_with("one two three"));
        assert!(truncated.contains("[Content truncated"));
    }

    #[test]
    fn test_collapse_whitespace_multiple_spaces() {
        let text = "hello    world   foo";
        let collapsed = collapse_whitespace(text);
        assert_eq!(collapsed, "hello world foo");
    }

    #[test]
    fn test_collapse_whitespace_multiple_newlines() {
        let text = "hello\n\n\n\nworld";
        let collapsed = collapse_whitespace(text);
        assert_eq!(collapsed, "hello\nworld");
    }

    #[test]
    fn test_nested_tags() {
        let html = "<div><p>text</p></div>";
        let text = strip_html(html);
        let text = collapse_whitespace(&text);
        assert!(text.contains("text"));
        assert!(!text.contains('<'));
    }

    #[test]
    fn test_self_closing_tags() {
        let html = "before<br/>middle<img src='x'/>after";
        let text = strip_html(html);
        assert!(text.contains("before"));
        assert!(text.contains("middle"));
        assert!(text.contains("after"));
        assert!(!text.contains('<'));
    }

    #[test]
    fn test_empty_input() {
        let text = strip_html("");
        assert!(text.is_empty());
        let title = extract_title("");
        assert_eq!(title, None);
        let collapsed = collapse_whitespace("");
        assert!(collapsed.is_empty());
    }

    #[test]
    fn test_only_tags_no_text() {
        let html = "<div><span></span><br/></div>";
        let text = strip_html(html);
        let text = collapse_whitespace(&text);
        // Should be empty or only whitespace after collapsing
        assert!(text.trim().is_empty());
    }

    #[test]
    fn test_mixed_content_and_entities() {
        let html = "<p>Tom &amp; Jerry &lt;3 &quot;friends&quot;</p>";
        let text = strip_html(html);
        let text = decode_html_entities(&text);
        assert!(text.contains("Tom & Jerry"));
        assert!(text.contains("<3"));
        assert!(text.contains("\"friends\""));
    }

    #[test]
    fn test_very_long_input() {
        let segment = "<p>word </p>";
        let html: String = std::iter::repeat_n(segment, 5000).collect();
        let text = strip_html(&html);
        let collapsed = collapse_whitespace(&text);
        // Should not panic and should contain the word
        assert!(collapsed.contains("word"));
        let word_count = collapsed.split_whitespace().count();
        assert!(word_count >= 4000, "expected many words, got {word_count}");
    }

    #[test]
    fn test_style_block_stripped() {
        let html = "<p>visible</p><style>body { color: red; }</style><p>also visible</p>";
        let text = strip_html(html);
        assert!(text.contains("visible"));
        assert!(text.contains("also visible"));
        assert!(!text.contains("color"));
    }

    #[test]
    fn test_extract_title_with_entities() {
        let html = "<html><head><title>A &amp; B</title></head></html>";
        let title = extract_title(html);
        assert_eq!(title, Some("A & B".to_string()));
    }

    #[test]
    fn test_truncate_words_exact_boundary() {
        let text = "one two three";
        assert_eq!(truncate_words(text, 3), text);
        assert_eq!(truncate_words(text, 4), text);
    }

    #[test]
    fn test_nbsp_decoded() {
        let text = "hello&nbsp;world";
        let decoded = decode_html_entities(text);
        assert_eq!(decoded, "hello world");
    }

    // ── SSRF protection ────────────────────────────────────────────────

    #[test]
    fn blocks_localhost() {
        assert!(is_private_url("http://localhost:8080/api"));
    }

    #[test]
    fn blocks_loopback_ip() {
        assert!(is_private_url("http://127.0.0.1/secret"));
    }

    #[test]
    fn blocks_private_10() {
        assert!(is_private_url("http://10.0.0.1/internal"));
    }

    #[test]
    fn blocks_private_192() {
        assert!(is_private_url("http://192.168.1.1/admin"));
    }

    #[test]
    fn blocks_private_172() {
        assert!(is_private_url("http://172.16.0.1/db"));
    }

    #[test]
    fn blocks_ipv6_loopback() {
        assert!(is_private_url("http://[::1]/api"));
    }

    #[test]
    fn blocks_file_scheme() {
        assert!(is_private_url("file:///etc/passwd"));
    }

    #[test]
    fn allows_public_url() {
        assert!(!is_private_url("https://example.com/page"));
    }

    #[test]
    fn allows_public_ip() {
        assert!(!is_private_url("http://8.8.8.8/dns"));
    }

    // ── IPv6 bypass tests ─────────────────────────────────────────────

    #[test]
    fn blocks_ipv6_unspecified() {
        assert!(is_private_url("http://[::]/api"));
    }

    #[test]
    fn blocks_ipv6_link_local() {
        assert!(is_private_url("http://[fe80::1]/api"));
    }

    #[test]
    fn blocks_ipv4_mapped_ipv6_loopback() {
        assert!(is_private_url("http://[::ffff:127.0.0.1]/api"));
    }

    #[test]
    fn blocks_ipv4_mapped_ipv6_private() {
        assert!(is_private_url("http://[::ffff:10.0.0.1]/api"));
    }

    #[test]
    fn blocks_zero_ip() {
        assert!(is_private_url("http://0.0.0.0/"));
    }

    #[test]
    fn blocks_link_local_169() {
        assert!(is_private_url("http://169.254.1.1/"));
    }

    #[test]
    fn blocks_cgn_range() {
        assert!(is_private_url("http://100.64.0.1/"));
    }

    #[test]
    fn blocks_dot_localhost_domain() {
        assert!(is_private_url("http://foo.localhost/api"));
    }

    #[test]
    fn blocks_dot_internal_domain() {
        assert!(is_private_url("http://service.internal/api"));
    }

    #[test]
    fn blocks_dot_local_domain() {
        assert!(is_private_url("http://printer.local/api"));
    }

    #[test]
    fn blocks_invalid_url() {
        assert!(is_private_url("not-a-url"));
    }

    #[test]
    fn allows_public_ipv6() {
        assert!(!is_private_url("http://[2607:f8b0:4004:800::200e]/"));
    }

    // ── Extended SSRF edge cases ──────────────────────────────────────

    #[test]
    fn blocks_ipv6_loopback_with_port() {
        assert!(is_private_url("http://[::1]:8080/api"));
    }

    #[test]
    fn blocks_private_range_boundaries() {
        assert!(is_private_url("http://10.0.0.0/"));
        assert!(is_private_url("http://10.255.255.255/"));
        assert!(is_private_url("http://172.16.0.0/"));
        assert!(is_private_url("http://172.31.255.255/"));
        assert!(is_private_url("http://192.168.0.0/"));
        assert!(is_private_url("http://192.168.255.255/"));
    }

    #[test]
    fn allows_public_172_outside_private() {
        // 172.32.0.0 is NOT in the 172.16-31 private range
        assert!(!is_private_url("http://172.32.0.1/"));
    }

    #[test]
    fn blocks_ipv4_mapped_ipv6_private_192() {
        assert!(is_private_url("http://[::ffff:192.168.1.1]/"));
    }

    #[test]
    fn allows_regular_domain() {
        assert!(!is_private_url("https://github.com/"));
        assert!(!is_private_url("https://docs.rs/"));
    }

    #[test]
    fn blocks_no_scheme() {
        assert!(is_private_url("//example.com/path"));
    }

    #[test]
    fn blocks_ftp_scheme() {
        assert!(is_private_url("ftp://example.com/file"));
    }

    #[test]
    fn blocks_data_scheme() {
        assert!(is_private_url("data:text/html,<h1>hi</h1>"));
    }

    // ── IPv6 advanced bypass tests ────────────────────────────────────

    #[test]
    fn blocks_ipv4_compatible_ipv6_loopback() {
        // ::127.0.0.1 — deprecated IPv4-compatible format
        assert!(is_private_url("http://[::127.0.0.1]/"));
    }

    #[test]
    fn blocks_ipv4_compatible_ipv6_private() {
        assert!(is_private_url("http://[::10.0.0.1]/"));
    }

    #[test]
    fn blocks_ipv6_ula() {
        // fc00::/7 — unique local address
        assert!(is_private_url("http://[fd00::1]/"));
        assert!(is_private_url("http://[fc00::1]/"));
    }

    #[test]
    fn blocks_ipv6_multicast() {
        assert!(is_private_url("http://[ff00::1]/"));
        assert!(is_private_url("http://[ff02::1]/"));
    }
}
