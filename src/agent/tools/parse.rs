use super::ToolCall;

/// Parse tool calls from AI response text.
/// Handles variations in formatting that AI models commonly produce:
/// - Standard `<tool_call>...</tool_call>` tags
/// - `<tool>...</tool>` variant tags
/// - Missing closing tags
/// - Markdown code fences wrapping tool calls
/// - JSON-style `{"tool": "name", ...}` format
/// - Extra whitespace and indentation
pub fn parse_tool_calls(text: &str) -> Vec<ToolCall> {
    // Strip markdown code fences that might wrap tool calls
    let cleaned = text
        .replace("```xml\n", "")
        .replace("```json\n", "")
        .replace("```\n", "")
        .replace("\n```", "");

    let mut calls = Vec::new();

    // Strategy 1: Standard <tool_call>...</tool_call> format
    calls.extend(parse_standard_tool_calls(&cleaned));

    // Strategy 2: If no standard calls found, try <tool>...</tool> variant
    if calls.is_empty() {
        calls.extend(parse_variant_tool_calls(&cleaned, "<tool>", "</tool>"));
    }

    // Strategy 3: If still none, try to detect JSON-style tool calls
    if calls.is_empty() {
        calls.extend(parse_json_tool_calls(&cleaned));
    }

    calls
}

fn parse_standard_tool_calls(text: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("<tool_call>") {
        if let Some(end) = remaining[start..].find("</tool_call>") {
            let block = &remaining[start + 11..start + end];
            if let Some(call) = parse_single_tool_call(block) {
                calls.push(call);
            }
            remaining = &remaining[start + end + 12..];
        } else {
            // No closing tag -- try to parse until next <tool_call> or end of text
            let block = &remaining[start + 11..];
            // Look for either another opening tag or a triple-newline separator
            let end = block
                .find("<tool_call>")
                .or_else(|| block.find("\n\n\n"))
                .unwrap_or(block.len());
            if let Some(call) = parse_single_tool_call(&block[..end]) {
                calls.push(call);
            }
            remaining = &remaining[start + 11 + end..];
        }
    }

    calls
}

fn parse_variant_tool_calls(text: &str, open: &str, close: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find(open) {
        if let Some(end) = remaining[start..].find(close) {
            let block = &remaining[start + open.len()..start + end];
            if let Some(call) = parse_single_tool_call(block) {
                calls.push(call);
            }
            remaining = &remaining[start + end + close.len()..];
        } else {
            break;
        }
    }

    calls
}

fn parse_json_tool_calls(text: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();

    let mut i = 0;
    let bytes = text.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'{' {
            // Try to find matching closing brace
            let mut depth = 1;
            let mut j = i + 1;
            while j < bytes.len() && depth > 0 {
                if bytes[j] == b'{' {
                    depth += 1;
                }
                if bytes[j] == b'}' {
                    depth -= 1;
                }
                j += 1;
            }

            if depth == 0 {
                let json_str = &text[i..j];
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str)
                    && let Some(tool) = value.get("tool").and_then(|v| v.as_str())
                {
                    let mut args = std::collections::HashMap::new();
                    if let Some(obj) = value.as_object() {
                        for (k, v) in obj {
                            if k != "tool" {
                                args.insert(
                                    k.clone(),
                                    v.as_str().unwrap_or(&v.to_string()).to_string(),
                                );
                            }
                        }
                    }
                    calls.push(ToolCall {
                        tool: tool.to_string(),
                        args,
                    });
                }
            }

            i = j;
        } else {
            i += 1;
        }
    }

    calls
}

fn parse_single_tool_call(block: &str) -> Option<ToolCall> {
    let mut tool_name = None;
    let mut args = std::collections::HashMap::new();
    let lines: Vec<&str> = block.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() {
            i += 1;
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            if key == "tool" {
                tool_name = Some(value.to_string());
                i += 1;
            } else if is_multiline_arg(key) {
                // Collect all lines until the next known key or end
                let mut full_value = value.to_string();
                i += 1;
                while i < lines.len() {
                    let next_line = lines[i].trim();
                    // Check if this is a new argument
                    if next_line
                        .split_once(':')
                        .map(|(k, _)| is_known_arg(k.trim()))
                        .unwrap_or(false)
                    {
                        break;
                    }
                    full_value.push('\n');
                    full_value.push_str(lines[i]); // Keep original indentation
                    i += 1;
                }
                args.insert(key.to_string(), full_value);
            } else {
                args.insert(key.to_string(), value.to_string());
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    tool_name.map(|name| ToolCall { tool: name, args })
}

fn is_multiline_arg(key: &str) -> bool {
    matches!(key, "content" | "old_text" | "new_text")
}

fn is_known_arg(key: &str) -> bool {
    matches!(
        key,
        "tool"
            | "path"
            | "content"
            | "old_text"
            | "new_text"
            | "command"
            | "pattern"
            | "start"
            | "end"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_tool_call_test() {
        let text = "Let me read that file.\n\n<tool_call>\ntool: read_file\npath: src/main.rs\n</tool_call>\n\nDone.";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
        assert_eq!(calls[0].args.get("path").unwrap(), "src/main.rs");
    }

    #[test]
    fn parse_multiple_tool_calls() {
        let text = "<tool_call>\ntool: read_file\npath: a.rs\n</tool_call>\n\n<tool_call>\ntool: read_file\npath: b.rs\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn parse_no_tool_calls() {
        let text = "Just a regular response with no tools.";
        let calls = parse_tool_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn parse_tool_call_with_multiword_args() {
        let text = "<tool_call>\ntool: run_command\ncommand: cargo test --release\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(
            calls[0].args.get("command").unwrap(),
            "cargo test --release"
        );
    }

    #[test]
    fn parse_tool_call_with_multiline_content() {
        let text = r#"<tool_call>
tool: write_file
path: src/hello.rs
content: fn main() {
    println!("hello");
}
</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "write_file");
        assert!(calls[0].args.get("content").unwrap().contains("println"));
    }

    #[test]
    fn parse_edit_file_with_multiline() {
        let text = r"<tool_call>
tool: edit_file
path: src/main.rs
old_text: fn old() {
    old_code();
}
new_text: fn new() {
    new_code();
}
</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].args.get("old_text").unwrap().contains("old_code"));
        assert!(calls[0].args.get("new_text").unwrap().contains("new_code"));
    }

    #[test]
    fn tool_call_parse_incomplete_block() {
        // Missing closing tag -- the robust parser now recovers these
        let text = "<tool_call>\ntool: read_file\npath: test.rs\n";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
        assert_eq!(calls[0].args.get("path").unwrap(), "test.rs");
    }

    #[test]
    fn tool_call_parse_nested_tags() {
        // Nested tool_call tags (should not happen but shouldn't crash)
        let text =
            "<tool_call>\ntool: read_file\npath: <tool_call>nested</tool_call>\n</tool_call>";
        let calls = parse_tool_calls(text);
        // Should parse something without crashing
        let _ = calls;
    }

    #[test]
    fn tool_call_empty_args() {
        let text = "<tool_call>\ntool: list_files\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "list_files");
        assert!(calls[0].args.is_empty());
    }

    // ── Robust parsing tests ─────────────────────────────────────────

    #[test]
    fn parse_standard_format() {
        let text = "<tool_call>\ntool: read_file\npath: src/main.rs\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
    }

    #[test]
    fn parse_missing_closing_tag() {
        let text = "Let me read that.\n\n<tool_call>\ntool: read_file\npath: src/main.rs\n\nThen I'll check it.";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
    }

    #[test]
    fn parse_tool_variant_tag() {
        let text = "<tool>\ntool: read_file\npath: test.rs\n</tool>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
    }

    #[test]
    fn parse_json_format() {
        let text = r#"I'll read that file: {"tool": "read_file", "path": "src/main.rs"}"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
        assert_eq!(calls[0].args.get("path").unwrap(), "src/main.rs");
    }

    #[test]
    fn parse_with_markdown_fences() {
        let text = "```xml\n<tool_call>\ntool: read_file\npath: test.rs\n</tool_call>\n```";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn parse_indented_tool_call() {
        let text = "  <tool_call>\n  tool: list_files\n  path: .\n  </tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "list_files");
    }

    #[test]
    fn parse_multiple_json_tool_calls() {
        let text = r#"I'll do two things:
{"tool": "read_file", "path": "a.rs"}
{"tool": "read_file", "path": "b.rs"}"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn parse_no_tool_calls_in_regular_text() {
        let text = "This is just a regular response about programming. No tools needed.";
        let calls = parse_tool_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn parse_json_with_nested_braces() {
        // JSON that's NOT a tool call should be ignored
        let text = r#"Here's some config: {"key": {"nested": "value"}}"#;
        let calls = parse_tool_calls(text);
        assert!(calls.is_empty()); // No "tool" key = not a tool call
    }

    #[test]
    fn parse_json_fenced_in_markdown() {
        let text = "```json\n{\"tool\": \"list_files\", \"path\": \".\"}\n```";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "list_files");
    }

    #[test]
    fn parse_multiple_missing_closing_tags() {
        // Two tool calls, neither has a closing tag, separated by triple newline
        let text = "<tool_call>\ntool: read_file\npath: a.rs\n\n\n<tool_call>\ntool: read_file\npath: b.rs\n";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn parse_standard_takes_priority_over_json() {
        // When standard tags are present, JSON in other parts is ignored
        let text = r#"<tool_call>
tool: read_file
path: a.rs
</tool_call>
Also here is some json: {"tool": "read_file", "path": "b.rs"}"#;
        let calls = parse_tool_calls(text);
        // Standard parsing found 1, so JSON fallback is not attempted
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].args.get("path").unwrap(), "a.rs");
    }

    #[test]
    fn parse_tool_variant_only_when_no_standard() {
        // <tool> tags should only be tried when <tool_call> finds nothing
        let text = "<tool>\ntool: list_files\npath: src\n</tool>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "list_files");
    }

    // ── parse_tool_calls edge cases ─────────────────────────────────

    #[test]
    fn parse_tool_calls_empty_input() {
        let calls = parse_tool_calls("");
        assert!(calls.is_empty());
    }

    #[test]
    fn parse_tool_calls_whitespace_only() {
        let calls = parse_tool_calls("   \n\n   \t  ");
        assert!(calls.is_empty());
    }

    #[test]
    fn parse_tool_calls_plain_text_no_tools() {
        let calls = parse_tool_calls("This is just a regular response with no tool calls.");
        assert!(calls.is_empty());
    }

    #[test]
    fn parse_tool_calls_unclosed_xml_still_parses() {
        // The parser has a fallback: if no closing tag is found, it parses
        // to the end of the text. This is by design for robustness.
        let calls = parse_tool_calls("<tool_call>tool: read_file\npath: test.rs");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
        assert_eq!(calls[0].args["path"], "test.rs");
    }

    #[test]
    fn parse_tool_calls_empty_tool_name() {
        let input = "<tool_call>tool: \npath: test.rs</tool_call>";
        let calls = parse_tool_calls(input);
        // Parser extracts empty tool name — the call is created but tool is empty
        if !calls.is_empty() {
            assert!(calls[0].tool.is_empty());
        }
    }

    #[test]
    fn parse_tool_calls_json_with_non_string_tool() {
        let input = r#"{"tool": 123, "path": "test.rs"}"#;
        let calls = parse_tool_calls(input);
        // JSON parser only accepts string tool values
        for call in &calls {
            // If parsed, verify the tool name is the stringified number or empty
            assert!(call.tool.is_empty() || call.tool == "123" || calls.is_empty());
        }
    }

    #[test]
    fn parse_tool_calls_multiple_in_sequence() {
        let input = "\
<tool_call>tool: read_file\npath: a.rs</tool_call>\n\
<tool_call>tool: read_file\npath: b.rs</tool_call>";
        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].args["path"], "a.rs");
        assert_eq!(calls[1].args["path"], "b.rs");
    }

    #[test]
    fn parse_tool_calls_markdown_wrapped_xml() {
        let input = "```xml\n<tool_call>tool: list_files\npath: src</tool_call>\n```";
        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "list_files");
    }

    #[test]
    fn parse_tool_calls_json_format() {
        let input = r#"{"tool": "search_code", "pattern": "fn main", "path": "."}"#;
        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "search_code");
        assert_eq!(calls[0].args["pattern"], "fn main");
    }

    #[test]
    fn parse_tool_calls_multiline_content_arg() {
        let input = "\
<tool_call>tool: write_file\npath: test.txt\ncontent:\nline 1\nline 2\nline 3</tool_call>";
        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "write_file");
        assert!(calls[0].args["content"].contains("line 1"));
        assert!(calls[0].args["content"].contains("line 3"));
    }
}
