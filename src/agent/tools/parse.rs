use super::ToolCall;

// CONTROL/DATA SEPARATION IS THE WHOLE POINT OF THIS MODULE.
//
// Every silent-corruption bug this parser has ever had came from one mistake:
// treating file CONTENT as if it were protocol CONTROL. The model emits the
// text of a file it wants written; if the parser scans that text for its own
// markers, ordinary source code silently changes what nerve does.
//
// There used to be a pre-pass here that stripped markdown fences from the
// WHOLE response before any structure was identified:
//     text.replace("```xml\n", "").replace("```\n", "").replace("\n```", "")
// Its stated purpose was to ignore a fence a model had wrapped a tool call in.
// But a blanket replace cannot tell a fence that WRAPS a call from a fence
// INSIDE the file being written, so it ate both. Writing this very file's
// documentation was impossible: ```rust fences vanished and the code inside
// them became loose prose. Nothing caught it — no gate checks markdown.
//
// It is now gone, and nothing replaced it, because it was never needed: a
// fence that wraps a tool call lies OUTSIDE the `<tool_call>`/`</tool_call>`
// markers (and outside the braces of the JSON form), so all three strategies
// below already skip it. The pre-pass only ever had the power to do harm.
pub fn parse_tool_calls(text: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();

    calls.extend(parse_standard_tool_calls(text));

    if calls.is_empty() {
        calls.extend(parse_variant_tool_calls(text, "<tool>", "</tool>"));
    }

    if calls.is_empty() {
        calls.extend(parse_json_tool_calls(text));
    }

    calls
}

const OPEN: &str = "<tool_call>";
const CLOSE: &str = "</tool_call>";

/// Byte offset, within `body` (the text following an `OPEN` marker), of the
/// `CLOSE` that ends this block — or `None` if the model never closed it.
///
/// Two rules, both there because CONTENT can contain the terminator (nerve
/// documenting its own tool protocol is a routine task, and every `.nerve/`
/// memory file does it):
///
/// 1. A block never extends past the start of the NEXT call, so the search is
///    bounded by the next `OPEN`. Without this bound rule 2 would merge two
///    adjacent calls into one.
/// 2. Within that window, prefer a `CLOSE` sitting ALONE on its line — that is
///    how the protocol is taught and how a real terminator appears. An inline
///    `</tool_call>` is far more likely to be prose being written to a file.
///    Only if no own-line terminator exists do we fall back to the first inline
///    one, because models do legitimately emit `path: a.rs</tool_call>`.
///
/// This is a heuristic, not a guarantee: content whose own line is exactly
/// `</tool_call>` still ends the block early. The protocol has no escape
/// mechanism, so that case is unfixable here — but it is far rarer than the
/// inline mention this rule rescues.
fn find_block_terminator(body: &str) -> Option<usize> {
    let window_end = body.find(OPEN).unwrap_or(body.len());
    let window = &body[..window_end];

    let mut offset = 0;
    for line in window.split_inclusive('\n') {
        if line.trim() == CLOSE {
            // Point at the marker itself, not at any indentation before it.
            return Some(offset + (line.len() - line.trim_start().len()));
        }
        offset += line.len();
    }

    window.find(CLOSE)
}

fn parse_standard_tool_calls(text: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find(OPEN) {
        let body = &remaining[start + OPEN.len()..];

        match find_block_terminator(body) {
            Some(end) => {
                if let Some(call) = parse_single_tool_call(&body[..end]) {
                    calls.push(call);
                }
                remaining = &body[end + CLOSE.len()..];
            }
            None => {
                // Unterminated block. It runs to the next call, or to the end
                // of the message. There is deliberately no blank-line
                // heuristic here: the old code ended the block at "\n\n\n",
                // which silently truncated any content containing two blank
                // lines — PEP 8 *mandates* those between top-level defs, so it
                // fired constantly on ordinary Python. Trailing prose swept
                // into a file is visible; silent truncation is not.
                let end = body.find(OPEN).unwrap_or(body.len());
                if let Some(call) = parse_single_tool_call(&body[..end]) {
                    calls.push(call);
                }
                remaining = &body[end..];
            }
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

/// True if only whitespace separates `i` from the end of its line.
///
/// `i` is only ever the index just past a `}` — ASCII — so the slicing here
/// can never split a multi-byte character.
fn is_at_line_end(text: &str, i: usize) -> bool {
    text[i..]
        .split('\n')
        .next()
        .is_none_or(|seg| seg.trim().is_empty())
}

/// Last-resort strategy: a bare `{"tool": "...", ...}` object with no
/// `<tool_call>` wrapper at all.
///
/// It runs ONLY when the other two strategies found nothing — which is exactly
/// the situation where nerve has written a plain final answer and called no
/// tool. That made an unbounded brace-scan actively dangerous: this sentence
///
///     For reference the JSON form looks like
///     {"tool": "run_command", "command": "rm -rf build"}.
///
/// used to parse as a live `run_command`, so nerve explaining its own protocol,
/// quoting a config file, or echoing a tool result back in prose could execute
/// it. Content re-read as control, with arbitrary command execution attached.
///
/// A candidate object must therefore END ITS LINE: nothing but whitespace may
/// follow the closing brace. A referenced object is punctuated (`}.`, `}` in a
/// sentence, `},` in a list); an object the model is actually INVOKING is the
/// last thing it writes. Multi-line pretty-printed objects still parse, since
/// only the closing boundary is checked.
///
/// Why not also require the object to START its line, which would be stricter?
/// Because `I'll read that file: {"tool": "read_file", ...}` is a real, tested,
/// supported shape (see `parse_json_format`), and refusing it would silently
/// stop executing valid calls — a regression that presents as "nerve did
/// nothing", which is exactly the class of silent failure this module keeps
/// producing.
///
/// THIS NARROWS THE HOLE, IT DOES NOT SEAL IT. An unpunctuated mention that
/// happens to end its line still parses; see
/// `json_mention_ending_its_line_is_a_known_residual_gap`, which pins that
/// behaviour deliberately so nobody discovers it by accident. Sealing it
/// properly needs a protocol-level fix (an explicit marker for the bare-JSON
/// form), not a smarter heuristic here.
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

            if depth == 0 && is_at_line_end(text, j) {
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

    // ---- Control/data separation ------------------------------------------
    //
    // Each test below is a REPRODUCTION of a silent corruption that shipped.
    // Every one of them passed through a green `cargo test` + `cargo clippy`
    // gate undetected, because nothing in a Rust gate inspects the bytes the
    // agent writes to a file. These are the only thing standing between the
    // protocol and the next one.

    /// A markdown code fence inside CONTENT must survive verbatim.
    ///
    /// The blanket `.replace("```\n", "")` pre-pass turned this exact input
    /// into `# Doc\nrust\nfn main() {}\n\nEnd.` — both fences gone and the code
    /// demoted to prose. It made writing any doc with a code example
    /// impossible, and burned whole iteration caps: the agent wrote the fence,
    /// re-read the mangled file, and retried a fight it could not win.
    #[test]
    fn fence_inside_content_is_preserved() {
        let text = "<tool_call>\ntool: write_file\npath: README.md\ncontent: # Doc\n\n```rust\nfn main() {}\n```\n\nEnd.\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        let content = calls[0].args.get("content").unwrap();
        assert!(
            content.contains("```rust"),
            "opening fence lost: {content:?}"
        );
        assert!(
            content.matches("```").count() == 2,
            "expected both fences, got: {content:?}"
        );
        assert!(content.contains("fn main() {}"));
    }

    /// A fence WRAPPING the call is still ignored — it lies outside the
    /// markers, which is why removing the pre-pass cost nothing.
    #[test]
    fn fence_wrapping_the_call_is_still_ignored() {
        let text = "```xml\n<tool_call>\ntool: read_file\npath: a.rs\n</tool_call>\n```";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].args.get("path").unwrap(), "a.rs");
    }

    /// An INLINE `</tool_call>` inside content must not end the block.
    ///
    /// This truncated the file to the 4 bytes `Emit` and reported success.
    /// Directly reachable: nerve documents its own tool protocol in `.nerve/`
    /// memory files and in this repository's docs.
    #[test]
    fn inline_terminator_in_content_does_not_end_the_block() {
        let text = "<tool_call>\ntool: write_file\npath: docs/protocol.md\ncontent: Emit </tool_call> to end.\nMore text here.\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        let content = calls[0].args.get("content").unwrap();
        assert!(
            content.contains("More text here."),
            "content truncated at the inline terminator: {content:?}"
        );
    }

    /// The own-line preference must not merge two adjacent calls, which is why
    /// the terminator search is bounded by the next `<tool_call>`.
    #[test]
    fn inline_terminator_still_closes_when_no_own_line_one_exists() {
        let text = "<tool_call>tool: read_file\npath: a.rs</tool_call>\n<tool_call>tool: read_file\npath: b.rs</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].args.get("path").unwrap(), "a.rs");
        assert_eq!(calls[1].args.get("path").unwrap(), "b.rs");
    }

    /// Two blank lines inside content must not truncate an unterminated block.
    /// PEP 8 mandates them between top-level defs, so the old `"\n\n\n"`
    /// heuristic fired on ordinary Python.
    #[test]
    fn blank_lines_do_not_truncate_an_unterminated_block() {
        let text = "<tool_call>\ntool: write_file\npath: a.py\ncontent: def one():\n    pass\n\n\ndef two():\n    pass";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        let content = calls[0].args.get("content").unwrap();
        assert!(
            content.contains("def two()"),
            "content truncated at the blank lines: {content:?}"
        );
    }

    /// A tool-shaped JSON object MENTIONED IN PROSE must never execute.
    ///
    /// This is the one finding whose consequence was arbitrary command
    /// execution rather than corruption: the JSON strategy runs precisely when
    /// nerve wrote a final answer and called no tool, and it used to
    /// brace-scan the entire message. Nerve documents its own tool protocol in
    /// `.nerve/` memory files, so this is a shape it genuinely writes.
    #[test]
    fn json_tool_call_mentioned_in_prose_is_not_executed() {
        let text = "I did not call any tool. For reference the JSON form looks like {\"tool\": \"run_command\", \"command\": \"rm -rf build\"}.";
        let calls = parse_tool_calls(text);
        assert!(
            calls.is_empty(),
            "prose mention parsed as a live call: {calls:?}"
        );
    }

    /// KNOWN RESIDUAL GAP — pinned deliberately, not an endorsement.
    ///
    /// The guard keys on punctuation after the closing brace, so an
    /// UNPUNCTUATED mention that ends its line is still executed. This test
    /// exists so the limit is discovered by reading the suite rather than by
    /// an incident. Closing it needs an explicit protocol marker for the
    /// bare-JSON form; a cleverer heuristic here would just move the seam.
    #[test]
    fn json_mention_ending_its_line_is_a_known_residual_gap() {
        let text = "For reference the JSON form looks like\n{\"tool\": \"run_command\", \"command\": \"echo hi\"}";
        let calls = parse_tool_calls(text);
        assert_eq!(
            calls.len(),
            1,
            "behaviour changed — if this is now empty the gap is CLOSED; \
             update the docs on parse_json_tool_calls and delete this test"
        );
    }

    /// ...but a genuine bare JSON call, owning its line, still parses.
    #[test]
    fn bare_json_tool_call_on_its_own_line_still_parses() {
        let text = "I'll do two things:\n{\"tool\": \"read_file\", \"path\": \"a.rs\"}\n{\"tool\": \"read_file\", \"path\": \"b.rs\"}";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].args.get("path").unwrap(), "a.rs");
        assert_eq!(calls[1].args.get("path").unwrap(), "b.rs");
    }

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
