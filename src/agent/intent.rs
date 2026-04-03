//! Intent detection for auto-agent mode.
//!
//! Analyses user messages to determine whether the AI will likely need tool
//! access (file read/write, shell commands, git, etc.) to fulfil the request.

/// Returns `true` when the user message strongly implies that tool usage is
/// needed (file I/O, running commands, git operations, etc.).
///
/// Uses keyword matching with word-boundary awareness so that e.g. "bread" does
/// not match the keyword "read".  The check is intentionally conservative:
/// ambiguous messages default to `false`.
pub fn needs_tools(message: &str) -> bool {
    let lower = message.to_lowercase();

    // ── Explicit tool requests ────────────────────────────────────────────
    if contains_phrase(&lower, "use tools")
        || contains_phrase(&lower, "agent mode")
        || contains_phrase(&lower, "with tools")
    {
        return true;
    }

    // ── File write / create / edit operations ─────────────────────────────
    for kw in &[
        "write to",
        "write a file",
        "create file",
        "create a file",
        "create a new file",
        "save to",
        "save file",
        "add to file",
        "update the code",
        "update the file",
        "modify the",
        "edit the",
        "change the file",
        "overwrite",
        "append to",
    ] {
        if contains_phrase(&lower, kw) {
            return true;
        }
    }

    // Single-word action verbs that imply file mutation — but only when
    // followed by a path-like or code-like context.
    if (contains_word(&lower, "write") || contains_word(&lower, "edit")) && has_path_like(&lower) {
        return true;
    }

    // ── File read operations ──────────────────────────────────────────────
    for kw in &[
        "read the file",
        "read the code",
        "read file",
        "show me the file",
        "show me the code",
        "look at the file",
        "look at the code",
        "check the code",
        "check the file",
        "cat the file",
        "open the file",
        "print the file",
        "display the file",
    ] {
        if contains_phrase(&lower, kw) {
            return true;
        }
    }

    // "read" / "show me" + a file path
    if (contains_word(&lower, "read") || lower.contains("show me")) && has_path_like(&lower) {
        return true;
    }

    // "what's in" + path/file context
    if lower.contains("what's in") && has_path_like(&lower) {
        return true;
    }

    // ── Run / execute operations ──────────────────────────────────────────
    for kw in &[
        "run the",
        "run this",
        "execute",
        "compile",
        "build the",
        "build this",
        "install the",
        "install this",
        "cargo test",
        "cargo build",
        "cargo run",
        "cargo clippy",
        "cargo fmt",
        "npm run",
        "npm install",
        "npm test",
        "make ",
        "cmake",
        "go build",
        "go test",
        "pytest",
        "python -m",
    ] {
        if contains_phrase(&lower, kw) {
            return true;
        }
    }

    // Bare verbs that strongly suggest execution
    if contains_word(&lower, "execute") || contains_word(&lower, "compile") {
        return true;
    }

    // "run" at the start of the message followed by a command-like token
    if lower.starts_with("run ") {
        return true;
    }

    // ── Git operations ────────────────────────────────────────────────────
    for kw in &[
        "git commit",
        "git push",
        "git pull",
        "git diff",
        "git add",
        "git stash",
        "git log",
        "git status",
        "git checkout",
        "git branch",
        "git merge",
        "git rebase",
    ] {
        if contains_phrase(&lower, kw) {
            return true;
        }
    }

    // "commit", "push", "diff" as standalone words in the right context
    if contains_word(&lower, "commit") && lower.contains("change") {
        return true;
    }

    // ── Project-level actions ─────────────────────────────────────────────
    // These verbs almost always require reading/writing files.
    for kw in &[
        "refactor",
        "fix the bug",
        "fix this bug",
        "fix the error",
        "fix this error",
        "implement the",
        "implement a ",
        "implement this",
        "add a feature",
        "add feature",
        "add the feature",
        "debug the",
        "debug this",
        "scaffold",
        "generate a",
        "generate the",
        "set up",
        "set up the",
        "deploy",
    ] {
        if contains_phrase(&lower, kw) {
            return true;
        }
    }

    // "create" + a code artifact (not "create a poem")
    if contains_word(&lower, "create")
        && (has_path_like(&lower)
            || contains_word(&lower, "function")
            || contains_word(&lower, "module")
            || contains_word(&lower, "struct")
            || contains_word(&lower, "class")
            || contains_word(&lower, "component")
            || contains_word(&lower, "test")
            || contains_word(&lower, "endpoint")
            || contains_word(&lower, "directory")
            || contains_word(&lower, "dir"))
    {
        return true;
    }

    // "fix" + path or code context
    if contains_word(&lower, "fix") && has_path_like(&lower) {
        return true;
    }

    // "write" + a code artifact
    if contains_word(&lower, "write")
        && (contains_word(&lower, "function")
            || contains_word(&lower, "module")
            || contains_word(&lower, "struct")
            || contains_word(&lower, "class")
            || contains_word(&lower, "test")
            || contains_word(&lower, "script"))
    {
        return true;
    }

    // "delete" / "remove" + file path
    if (contains_word(&lower, "delete") || contains_word(&lower, "remove")) && has_path_like(&lower)
    {
        return true;
    }

    false
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Check whether `haystack` contains `word` at a word boundary.
/// A word boundary is the start/end of string or a non-alphanumeric character.
fn contains_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    let wbytes = word.as_bytes();
    if wbytes.len() > bytes.len() {
        return false;
    }
    let mut pos = 0;
    while let Some(idx) = haystack[pos..].find(word) {
        let abs = pos + idx;
        let before_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        let after = abs + wbytes.len();
        let after_ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        pos = abs + 1;
    }
    false
}

/// Check whether `haystack` contains the multi-word `phrase` as a substring.
/// For phrases we do a simple substring match (already has enough context).
fn contains_phrase(haystack: &str, phrase: &str) -> bool {
    haystack.contains(phrase)
}

/// Heuristic: does the message look like it references a file path?
/// Matches things like `main.rs`, `src/lib.rs`, `./foo`, `/tmp/bar`,
/// `utils.py`, `index.ts`, etc.
fn has_path_like(text: &str) -> bool {
    for token in text.split_whitespace() {
        // Contains a slash — likely a path
        if token.contains('/') && token.len() > 1 {
            return true;
        }
        // Starts with ./ or ../
        if token.starts_with("./") || token.starts_with("../") {
            return true;
        }
        // Has a code-file extension
        let extensions = [
            ".rs", ".py", ".js", ".ts", ".tsx", ".jsx", ".go", ".c", ".cpp", ".h", ".hpp", ".java",
            ".rb", ".sh", ".toml", ".yaml", ".yml", ".json", ".html", ".css", ".scss", ".md",
            ".txt", ".cfg", ".ini", ".xml", ".sql", ".lua", ".zig", ".ex", ".exs", ".kt", ".swift",
        ];
        for ext in &extensions {
            if token.ends_with(ext) && token.len() > ext.len() {
                return true;
            }
        }
    }
    false
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive cases: should trigger tools ──────────────────────────────

    #[test]
    fn write_a_function() {
        assert!(needs_tools("write a function to sort arrays"));
    }

    #[test]
    fn fix_the_bug() {
        assert!(needs_tools("fix the bug in main.rs"));
    }

    #[test]
    fn run_cargo_test() {
        assert!(needs_tools("run cargo test"));
    }

    #[test]
    fn create_new_file() {
        assert!(needs_tools("create a new file called utils.rs"));
    }

    #[test]
    fn edit_the_config() {
        assert!(needs_tools("edit the config.toml file"));
    }

    #[test]
    fn implement_feature() {
        assert!(needs_tools("implement the login feature"));
    }

    #[test]
    fn read_the_file() {
        assert!(needs_tools("read the file src/main.rs"));
    }

    #[test]
    fn show_me_file() {
        assert!(needs_tools("show me src/main.rs"));
    }

    #[test]
    fn refactor_code() {
        assert!(needs_tools("refactor the authentication module"));
    }

    #[test]
    fn run_build() {
        assert!(needs_tools("run the build"));
    }

    #[test]
    fn git_commit() {
        assert!(needs_tools("git commit -m 'fix typo'"));
    }

    #[test]
    fn write_test() {
        assert!(needs_tools("write a test for the parser"));
    }

    #[test]
    fn compile_project() {
        assert!(needs_tools("compile the project"));
    }

    #[test]
    fn add_feature() {
        assert!(needs_tools("add a feature for dark mode"));
    }

    #[test]
    fn debug_this() {
        assert!(needs_tools("debug this crash in app.rs"));
    }

    #[test]
    fn execute_command() {
        assert!(needs_tools("execute the migration script"));
    }

    #[test]
    fn save_to_file() {
        assert!(needs_tools("save to output.json"));
    }

    #[test]
    fn update_the_code() {
        assert!(needs_tools("update the code to use async"));
    }

    #[test]
    fn create_directory() {
        assert!(needs_tools("create a new directory called src/utils"));
    }

    #[test]
    fn modify_the_struct() {
        assert!(needs_tools("modify the struct in models.rs"));
    }

    #[test]
    fn use_tools_explicit() {
        assert!(needs_tools("use tools to fix this"));
    }

    #[test]
    fn npm_install() {
        assert!(needs_tools("npm install express"));
    }

    #[test]
    fn check_whats_in_file() {
        assert!(needs_tools("what's in src/lib.rs"));
    }

    #[test]
    fn generate_a_module() {
        assert!(needs_tools("generate a new API module"));
    }

    #[test]
    fn delete_file() {
        assert!(needs_tools("delete the old config.yaml"));
    }

    #[test]
    fn write_class() {
        assert!(needs_tools("write a class for user authentication"));
    }

    // ── Negative cases: should NOT trigger tools ──────────────────────────

    #[test]
    fn explain_sorting() {
        assert!(!needs_tools("explain how sorting works"));
    }

    #[test]
    fn what_is_rust() {
        assert!(!needs_tools("what is Rust?"));
    }

    #[test]
    fn translate_to_french() {
        assert!(!needs_tools("translate this to French"));
    }

    #[test]
    fn review_code() {
        assert!(!needs_tools("review this code"));
    }

    #[test]
    fn help_understand() {
        assert!(!needs_tools("help me understand closures"));
    }

    #[test]
    fn summarize_text() {
        assert!(!needs_tools("summarize this article"));
    }

    #[test]
    fn compare_languages() {
        assert!(!needs_tools("compare Rust and Go"));
    }

    #[test]
    fn bread_not_read() {
        assert!(!needs_tools("I like bread and butter"));
    }

    #[test]
    fn creative_writing() {
        assert!(!needs_tools("write a poem about the ocean"));
    }

    #[test]
    fn greeting() {
        assert!(!needs_tools("hello, how are you?"));
    }

    #[test]
    fn opinion_question() {
        assert!(!needs_tools("what do you think about microservices?"));
    }

    #[test]
    fn explain_concept() {
        assert!(!needs_tools("explain dependency injection"));
    }

    #[test]
    fn list_differences() {
        assert!(!needs_tools("list the differences between TCP and UDP"));
    }

    #[test]
    fn suggest_approach() {
        assert!(!needs_tools("suggest a good approach for caching"));
    }

    // ── Edge cases ────────────────────────────────────────────────────────

    #[test]
    fn empty_message() {
        assert!(!needs_tools(""));
    }

    #[test]
    fn whitespace_only() {
        assert!(!needs_tools("   "));
    }

    #[test]
    fn mixed_case() {
        assert!(needs_tools("READ the file src/main.rs"));
    }

    #[test]
    fn fix_with_path() {
        assert!(needs_tools("fix src/app.rs"));
    }

    #[test]
    fn scaffold_project() {
        assert!(needs_tools("scaffold a new REST API project"));
    }

    // ── Helper function tests ─────────────────────────────────────────────

    #[test]
    fn contains_word_basic() {
        assert!(contains_word("read the file", "read"));
        assert!(contains_word("please read it", "read"));
        assert!(contains_word("read", "read"));
    }

    #[test]
    fn contains_word_no_partial() {
        assert!(!contains_word("bread", "read"));
        assert!(!contains_word("reading", "read"));
        assert!(!contains_word("already", "read"));
    }

    #[test]
    fn contains_word_punctuation_boundary() {
        assert!(contains_word("please read, then write", "read"));
        assert!(contains_word("(read)", "read"));
    }

    #[test]
    fn has_path_like_basic() {
        assert!(has_path_like("look at src/main.rs"));
        assert!(has_path_like("the file ./foo.txt"));
        assert!(has_path_like("check utils.py"));
    }

    #[test]
    fn has_path_like_no_match() {
        assert!(!has_path_like("explain sorting"));
        assert!(!has_path_like("what is rust"));
    }
}
