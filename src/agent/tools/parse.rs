use super::ToolCall;

pub fn parse_tool_calls(text: &str) -> Vec<ToolCall> {
    let cleaned = text
        .replace("```xml\n", "")
        .replace("```json\n", "")
        .replace("```\n", "")
        .replace("\n```", "");

    let mut calls = Vec::new();

    calls.extend(parse_standard_tool_calls(&cleaned));

    if calls.is_empty() {
        calls.extend(parse_variant_tool_calls(&cleaned, "<tool>", "</tool>"));
    }

    if calls.is_empty() {
        calls.extend(parse_json_tool_calls(&cleaned));
    }

    calls
}

fn parse_standard_tool_calls(text: &str) -> Vec<ToolCall> {
    const OPEN: &str = "<tool_call>";
    const CLOSE: &str = "</tool_call>";
    let mut calls = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find(OPEN) {
        if let Some(end) = remaining[start..].find(CLOSE) {
            let block = &remaining[start + OPEN.len()..start + end];
            if let Some(call) = parse_single_tool_call(block) {
                calls.push(call);
            }
            remaining = &remaining[start + end + CLOSE.len()..];
        } else {
            let block = &remaining[start + OPEN.len()..];
            let end = block
                .find(OPEN)
                .or_else(|| block.find("\n\n\n"))
                .unwrap_or(block.len());
            if let Some(call) = parse_single_tool_call(&block[..end]) {
                calls.push(call);
            }
            remaining = &remaining[start + OPEN.len() + end..];
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
                    && let Some(found_tool) = value.get("tool").and_then(|v| v.as_str())
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
                    let found_tool_name = found_tool.to_string();
                    calls.push(ToolCall {
                        tool: found_tool_name,
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
                let mut full_value = value.to_string();
                i += 1;
                // SILENT-CORRUPTION BUG (job e8279faa): the old version of this
                // loop stopped accumulating as soon as it saw ANY line whose
                // `key:` prefix matched a known arg name (tool, path, content,
                // old_text, new_text, command, pattern, start, end). That is
                // satisfiable by completely ordinary file content, e.g. this
                // Zod error object:
                //     throw new ZodError([{
                //       code: 'custom',
                //       path: ['serviceId'],
                //       message: 'x',
                //     }]);
                // The indented `  path: ['serviceId'],` line was mistaken for a
                // new tool argument: `content` got silently truncated right
                // before `}]);`, and `path` got hijacked to the literal string
                // `['serviceId'],`. Nerve then wrote the truncated content to a
                // file with that garbage name and reported success — no error,
                // no warning. The same shape produced a junk file named
                // `text('path').notNull(),` on 2026-07-04. This is not exotic:
                // `content:` alone breaks every CSS file with a pseudo-element
                // rule (`content: "";`), `path:` breaks JS/TS object literals
                // and YAML, `command:` breaks docker-compose/CI files, and
                // `start:`/`end:` break ordinary JS date-range objects.
                //
                // THE FIX: per `tools_system_prompt` (see `tools/mod.rs`), a
                // tool call is taught as flush-left `key: value` lines, with
                // at most one multiline arg (`content` or `new_text`) emitted
                // LAST. So a multiline arg's value is "everything from here to
                // the end of the block" — it must be read LITERALLY and never
                // re-scanned for keys. The ONE exception the protocol actually
                // requires is `edit_file`, whose `old_text` is immediately
                // followed by a `new_text:` line — that is the only case where
                // a multiline arg is not last. We special-case ONLY that exact
                // transition, and only when the candidate line is flush-left
                // (zero leading whitespace) AND its key is exactly `new_text`.
                // Ordinary code/CSS/YAML/JSON embedded as arg content is
                // nested inside braces/blocks and is therefore indented in
                // virtually all real-world formatting, so it cannot satisfy
                // this check; a real protocol arg line, per the system prompt,
                // is never indented. `content` (and `new_text` itself) get NO
                // lookahead at all — they run to the end of the block.
                while i < lines.len() {
                    let raw_line = lines[i];
                    if key == "old_text" && is_unindented_new_text_key(raw_line) {
                        break;
                    }
                    full_value.push('\n');
                    full_value.push_str(raw_line);
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

/// True only if `raw_line` is flush-left (no leading space/tab) AND its
/// `key:` prefix is exactly `new_text`. This is the ONLY lookahead the
/// multiline-arg accumulator performs, and it is only ever consulted while
/// reading `old_text` (see the comment in `parse_single_tool_call` above,
/// and job e8279faa / the 2026-07-04 incident for why a broader "any known
/// key" check corrupts ordinary files). Requiring zero indentation is what
/// makes this safe: real protocol arg lines, as taught by
/// `tools_system_prompt`, are always emitted flush-left, while a `new_text:`
/// (or `path:`, `content:`, ...) -shaped line occurring inside ordinary file
/// content is virtually always nested inside braces/blocks and therefore
/// indented.
fn is_unindented_new_text_key(raw_line: &str) -> bool {
    if raw_line.starts_with(' ') || raw_line.starts_with('\t') {
        return false;
    }
    raw_line
        .split_once(':')
        .map(|(k, _)| k == "new_text")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_tool_call() {
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
        let text = "<tool_call>\ntool: write_file\npath: src/hello.rs\ncontent: fn main() {\n    println!(\"hello\");\n}\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "write_file");
        assert!(calls[0].args.get("content").unwrap().contains("println"));
    }

    #[test]
    fn parse_edit_file_with_multiline() {
        let text = "<tool_call>\ntool: edit_file\npath: src/main.rs\nold_text: fn old() {\n    old_code();\n}\nnew_text: fn new() {\n    new_code();\n}\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].args.get("old_text").unwrap().contains("old_code"));
        assert!(calls[0].args.get("new_text").unwrap().contains("new_code"));
    }

    #[test]
    fn tool_call_parse_incomplete_block() {
        let text = "<tool_call>\ntool: read_file\npath: test.rs\n";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
        assert_eq!(calls[0].args.get("path").unwrap(), "test.rs");
    }

    #[test]
    fn tool_call_parse_nested_tags() {
        let text =
            "<tool_call>\ntool: read_file\npath: <tool_call>nested</tool_call>\n</tool_call>";
        let calls = parse_tool_calls(text);
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
        let text = "I'll do two things:\n{\"tool\": \"read_file\", \"path\": \"a.rs\"}\n{\"tool\": \"read_file\", \"path\": \"b.rs\"}";
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
        let text = r#"Here's some config: {"key": {"nested": "value"}}"#;
        let calls = parse_tool_calls(text);
        assert!(calls.is_empty());
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
        let text = "<tool_call>\ntool: read_file\npath: a.rs\n\n\n<tool_call>\ntool: read_file\npath: b.rs\n";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn parse_standard_takes_priority_over_json() {
        let text = "<tool_call>\ntool: read_file\npath: a.rs\n</tool_call>\nAlso here is some json: {\"tool\": \"read_file\", \"path\": \"b.rs\"}";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].args.get("path").unwrap(), "a.rs");
    }

    #[test]
    fn parse_tool_variant_only_when_no_standard() {
        let text = "<tool>\ntool: list_files\npath: src\n</tool>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "list_files");
    }

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
        let calls = parse_tool_calls("<tool_call>tool: read_file\npath: test.rs");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
        assert_eq!(calls[0].args["path"], "test.rs");
    }

    #[test]
    fn parse_tool_calls_empty_tool_name() {
        let input = "<tool_call>tool: \npath: test.rs</tool_call>";
        let calls = parse_tool_calls(input);
        if !calls.is_empty() {
            assert!(calls[0].tool.is_empty());
        }
    }

    #[test]
    fn parse_tool_calls_json_with_non_string_tool() {
        let input = r#"{"tool": 123, "path": "test.rs"}"#;
        let calls = parse_tool_calls(input);
        for call in &calls {
            assert!(call.tool.is_empty() || call.tool == "123" || calls.is_empty());
        }
    }

    #[test]
    fn parse_tool_calls_multiple_in_sequence() {
        let input = "<tool_call>tool: read_file\npath: a.rs</tool_call>\n<tool_call>tool: read_file\npath: b.rs</tool_call>";
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
        let input = "<tool_call>tool: write_file\npath: test.txt\ncontent:\nline 1\nline 2\nline 3</tool_call>";
        let calls = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "write_file");
        assert!(calls[0].args["content"].contains("line 1"));
        assert!(calls[0].args["content"].contains("line 3"));
    }
}
