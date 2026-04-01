use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub root: PathBuf,
    pub project_type: ProjectType,
    pub name: String,
    pub description: String,
    pub key_files: Vec<String>,
    pub tech_stack: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Java,
    Ruby,
    Elixir,
    Zig,
    CSharp,
    Cpp,
    Unknown,
}

/// Detect the workspace from the current directory
pub fn detect_workspace() -> Option<WorkspaceInfo> {
    let cwd = std::env::current_dir().ok()?;
    detect_workspace_at(&cwd)
}

/// Detect workspace at a specific path
pub fn detect_workspace_at(path: &Path) -> Option<WorkspaceInfo> {
    // Walk up from path to find project root
    let mut current = path.to_path_buf();
    loop {
        if let Some(info) = try_detect(&current) {
            return Some(info);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn try_detect(dir: &Path) -> Option<WorkspaceInfo> {
    // Check for project markers in priority order

    // Rust
    if dir.join("Cargo.toml").exists() {
        return Some(detect_rust(dir));
    }
    // Node.js
    if dir.join("package.json").exists() {
        return Some(detect_node(dir));
    }
    // Python
    if dir.join("pyproject.toml").exists()
        || dir.join("setup.py").exists()
        || dir.join("requirements.txt").exists()
    {
        return Some(detect_python(dir));
    }
    // Go
    if dir.join("go.mod").exists() {
        return Some(detect_go(dir));
    }
    // Java
    if dir.join("pom.xml").exists()
        || dir.join("build.gradle").exists()
        || dir.join("build.gradle.kts").exists()
    {
        return Some(detect_java(dir));
    }
    // Ruby
    if dir.join("Gemfile").exists() {
        return Some(detect_ruby(dir));
    }
    // Elixir
    if dir.join("mix.exs").exists() {
        return Some(detect_elixir(dir));
    }
    // Zig
    if dir.join("build.zig").exists() {
        return Some(detect_zig(dir));
    }
    // C#
    if has_extension(dir, "csproj") || has_extension(dir, "sln") {
        return Some(detect_csharp(dir));
    }
    // C/C++ (CMake or Makefile)
    if dir.join("CMakeLists.txt").exists() || dir.join("Makefile").exists() {
        return Some(detect_cpp(dir));
    }

    None
}

fn has_extension(dir: &Path, ext: &str) -> bool {
    fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some(ext))
        })
        .unwrap_or(false)
}

fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

// ─── Rust ──────────────────────────────────────────────────────────────────

fn detect_rust(dir: &Path) -> WorkspaceInfo {
    let mut name = dir_name(dir);
    let mut description = String::new();
    let mut tech_stack = vec!["Rust".to_string()];
    let mut key_files = vec!["Cargo.toml".to_string()];

    // Parse Cargo.toml for name and description
    if let Ok(content) = fs::read_to_string(dir.join("Cargo.toml"))
        && let Ok(parsed) = content.parse::<toml::Table>()
    {
        if let Some(pkg) = parsed.get("package").and_then(|v| v.as_table()) {
            if let Some(n) = pkg.get("name").and_then(|v| v.as_str()) {
                name = n.to_string();
            }
            if let Some(d) = pkg.get("description").and_then(|v| v.as_str()) {
                description = d.to_string();
            }
        }
        // Detect key dependencies
        if let Some(deps) = parsed.get("dependencies").and_then(|v| v.as_table()) {
            for dep in [
                "tokio",
                "actix-web",
                "axum",
                "rocket",
                "warp",
                "ratatui",
                "serde",
                "diesel",
                "sqlx",
                "reqwest",
            ] {
                if deps.contains_key(dep) {
                    tech_stack.push(dep.to_string());
                }
            }
        }
    }

    // Detect key files
    for f in [
        "src/main.rs",
        "src/lib.rs",
        "README.md",
        ".gitignore",
        "Dockerfile",
    ] {
        if dir.join(f).exists() {
            key_files.push(f.to_string());
        }
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::Rust,
        name,
        description,
        key_files,
        tech_stack,
    }
}

// ─── Node.js ───────────────────────────────────────────────────────────────

fn detect_node(dir: &Path) -> WorkspaceInfo {
    let mut name = dir_name(dir);
    let mut description = String::new();
    let mut tech_stack = vec!["Node.js".to_string()];
    let mut key_files = vec!["package.json".to_string()];

    if let Ok(content) = fs::read_to_string(dir.join("package.json"))
        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content)
    {
        if let Some(n) = parsed["name"].as_str() {
            name = n.to_string();
        }
        if let Some(d) = parsed["description"].as_str() {
            description = d.to_string();
        }
        // Detect framework
        if let Some(deps) = parsed["dependencies"].as_object() {
            for fw in [
                "next", "react", "vue", "angular", "express", "fastify", "nest", "svelte",
            ] {
                if deps.contains_key(fw) {
                    tech_stack.push(fw.to_string());
                }
            }
        }
        if let Some(deps) = parsed["devDependencies"].as_object() {
            for tool in [
                "typescript",
                "vite",
                "webpack",
                "jest",
                "vitest",
                "tailwindcss",
            ] {
                if deps.contains_key(tool) {
                    tech_stack.push(tool.to_string());
                }
            }
        }
    }

    for f in [
        "src/index.ts",
        "src/index.js",
        "src/App.tsx",
        "README.md",
        "tsconfig.json",
        "Dockerfile",
    ] {
        if dir.join(f).exists() {
            key_files.push(f.to_string());
        }
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::Node,
        name,
        description,
        key_files,
        tech_stack,
    }
}

// ─── Python ────────────────────────────────────────────────────────────────

fn detect_python(dir: &Path) -> WorkspaceInfo {
    let mut name = dir_name(dir);
    let mut tech_stack = vec!["Python".to_string()];
    let mut key_files = Vec::new();

    // Check for pyproject.toml
    if dir.join("pyproject.toml").exists() {
        key_files.push("pyproject.toml".into());
        if let Ok(content) = fs::read_to_string(dir.join("pyproject.toml"))
            && let Ok(parsed) = content.parse::<toml::Table>()
            && let Some(proj) = parsed.get("project").and_then(|v| v.as_table())
            && let Some(n) = proj.get("name").and_then(|v| v.as_str())
        {
            name = n.to_string();
        }
    }

    // Check for common frameworks
    if dir.join("manage.py").exists() {
        tech_stack.push("Django".into());
    }
    if dir.join("app.py").exists() || dir.join("wsgi.py").exists() {
        tech_stack.push("Flask".into());
    }

    for f in [
        "requirements.txt",
        "setup.py",
        "README.md",
        "Dockerfile",
        "main.py",
        "app.py",
    ] {
        if dir.join(f).exists() {
            key_files.push(f.to_string());
        }
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::Python,
        name,
        description: String::new(),
        key_files,
        tech_stack,
    }
}

// ─── Go ────────────────────────────────────────────────────────────────────

fn detect_go(dir: &Path) -> WorkspaceInfo {
    let mut name = dir_name(dir);
    let tech_stack = vec!["Go".to_string()];
    let key_files = vec!["go.mod".into()];

    if let Ok(content) = fs::read_to_string(dir.join("go.mod"))
        && let Some(line) = content.lines().next()
        && let Some(module) = line.strip_prefix("module ")
    {
        name = module.trim().to_string();
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::Go,
        name,
        description: String::new(),
        key_files,
        tech_stack,
    }
}

// ─── Java ──────────────────────────────────────────────────────────────────

fn detect_java(dir: &Path) -> WorkspaceInfo {
    let name = dir_name(dir);
    let tech_stack = vec!["Java".to_string()];
    let mut key_files = Vec::new();

    for f in [
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "README.md",
        "Dockerfile",
    ] {
        if dir.join(f).exists() {
            key_files.push(f.to_string());
        }
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::Java,
        name,
        description: String::new(),
        key_files,
        tech_stack,
    }
}

// ─── Ruby ──────────────────────────────────────────────────────────────────

fn detect_ruby(dir: &Path) -> WorkspaceInfo {
    let name = dir_name(dir);
    let tech_stack = vec!["Ruby".to_string()];
    let mut key_files = vec!["Gemfile".to_string()];

    for f in ["Rakefile", "README.md", "Dockerfile", "config.ru"] {
        if dir.join(f).exists() {
            key_files.push(f.to_string());
        }
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::Ruby,
        name,
        description: String::new(),
        key_files,
        tech_stack,
    }
}

// ─── Elixir ────────────────────────────────────────────────────────────────

fn detect_elixir(dir: &Path) -> WorkspaceInfo {
    let name = dir_name(dir);
    let tech_stack = vec!["Elixir".to_string()];
    let mut key_files = vec!["mix.exs".to_string()];

    for f in ["README.md", "Dockerfile", "config/config.exs"] {
        if dir.join(f).exists() {
            key_files.push(f.to_string());
        }
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::Elixir,
        name,
        description: String::new(),
        key_files,
        tech_stack,
    }
}

// ─── Zig ───────────────────────────────────────────────────────────────────

fn detect_zig(dir: &Path) -> WorkspaceInfo {
    let name = dir_name(dir);
    let tech_stack = vec!["Zig".to_string()];
    let mut key_files = vec!["build.zig".to_string()];

    for f in ["README.md", "build.zig.zon"] {
        if dir.join(f).exists() {
            key_files.push(f.to_string());
        }
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::Zig,
        name,
        description: String::new(),
        key_files,
        tech_stack,
    }
}

// ─── C# ────────────────────────────────────────────────────────────────────

fn detect_csharp(dir: &Path) -> WorkspaceInfo {
    let name = dir_name(dir);
    let tech_stack = vec!["C#".to_string()];
    let mut key_files = Vec::new();

    for f in ["README.md", "Dockerfile"] {
        if dir.join(f).exists() {
            key_files.push(f.to_string());
        }
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::CSharp,
        name,
        description: String::new(),
        key_files,
        tech_stack,
    }
}

// ─── C/C++ ─────────────────────────────────────────────────────────────────

fn detect_cpp(dir: &Path) -> WorkspaceInfo {
    let name = dir_name(dir);
    let tech_stack = vec!["C/C++".to_string()];
    let mut key_files = Vec::new();

    for f in [
        "CMakeLists.txt",
        "Makefile",
        "README.md",
        "Dockerfile",
    ] {
        if dir.join(f).exists() {
            key_files.push(f.to_string());
        }
    }

    WorkspaceInfo {
        root: dir.to_path_buf(),
        project_type: ProjectType::Cpp,
        name,
        description: String::new(),
        key_files,
        tech_stack,
    }
}

// ─── Project map generation ───────────────────────────────────────────────

/// Generate a compact project map: file tree with key symbols
pub fn generate_project_map(root: &std::path::Path, max_depth: usize) -> String {
    let mut map = String::new();
    map.push_str(&format!("Project: {}\n", root.display()));
    map.push_str(&format!("{}\n\n", "=".repeat(40)));

    // File tree
    map.push_str("File structure:\n");
    build_tree(root, root, &mut map, 0, max_depth);

    // Key symbols (functions, structs, etc.) from important files
    map.push_str("\nKey definitions:\n");
    extract_key_symbols(root, &mut map);

    map
}

pub(crate) fn build_tree(
    root: &std::path::Path,
    dir: &std::path::Path,
    output: &mut String,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }

    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files/dirs, target/, node_modules/, __pycache__, .git
        if name.starts_with('.')
            || name == "target"
            || name == "node_modules"
            || name == "__pycache__"
            || name == "vendor"
            || name == "dist"
            || name == "build"
            || name == ".git"
        {
            continue;
        }

        let indent = "  ".repeat(depth);
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if is_dir {
            output.push_str(&format!("{indent}{name}/\n"));
            build_tree(root, &entry.path(), output, depth + 1, max_depth);
        } else {
            // Show file with size
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let size_str = if size < 1024 {
                format!("{size}B")
            } else if size < 1024 * 1024 {
                format!("{}K", size / 1024)
            } else {
                format!("{}M", size / (1024 * 1024))
            };
            output.push_str(&format!("{indent}{name}  ({size_str})\n"));
        }
    }
}

pub(crate) fn extract_key_symbols(root: &std::path::Path, output: &mut String) {
    // Scan important source files for key definitions
    let extensions = ["rs", "py", "js", "ts", "go", "java", "rb"];

    let mut files_to_scan: Vec<std::path::PathBuf> = Vec::new();
    collect_source_files(root, &mut files_to_scan, &extensions, 0, 3);

    // Limit to first 20 files to keep context manageable
    files_to_scan.truncate(20);

    for file in &files_to_scan {
        let rel_path = file.strip_prefix(root).unwrap_or(file);
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");

        if let Ok(content) = std::fs::read_to_string(file) {
            let symbols = extract_symbols_from_content(&content, ext);
            if !symbols.is_empty() {
                output.push_str(&format!("\n{}:\n", rel_path.display()));
                for sym in &symbols {
                    output.push_str(&format!("  {sym}\n"));
                }
            }
        }
    }
}

fn collect_source_files(
    dir: &std::path::Path,
    files: &mut Vec<std::path::PathBuf>,
    extensions: &[&str],
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || name == "target"
                || name == "node_modules"
                || name == "__pycache__"
            {
                continue;
            }

            let path = entry.path();
            if path.is_dir() {
                collect_source_files(&path, files, extensions, depth + 1, max_depth);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    files.push(path);
                }
            }
        }
    }
}

pub(crate) fn extract_symbols_from_content(content: &str, ext: &str) -> Vec<String> {
    let mut symbols = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        match ext {
            "rs" => {
                if trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("pub async fn ")
                {
                    if let Some(sig) = trimmed.split('{').next() {
                        symbols.push(sig.trim().to_string());
                    }
                } else if trimmed.starts_with("pub struct ")
                    || trimmed.starts_with("pub enum ")
                    || trimmed.starts_with("pub trait ")
                {
                    if let Some(sig) = trimmed
                        .split('{')
                        .next()
                        .or_else(|| trimmed.split(';').next())
                    {
                        symbols.push(sig.trim().to_string());
                    }
                }
            }
            "py" => {
                if trimmed.starts_with("def ")
                    || trimmed.starts_with("class ")
                    || trimmed.starts_with("async def ")
                {
                    if let Some(sig) = trimmed.split(':').next() {
                        symbols.push(sig.trim().to_string());
                    }
                }
            }
            "js" | "ts" | "jsx" | "tsx" => {
                if trimmed.starts_with("export function ")
                    || trimmed.starts_with("export class ")
                    || trimmed.starts_with("export default function ")
                    || trimmed.starts_with("export const ")
                {
                    let sig: String = trimmed.chars().take(80).collect();
                    symbols.push(sig);
                }
            }
            "go" => {
                if trimmed.starts_with("func ") || trimmed.starts_with("type ") {
                    if let Some(sig) = trimmed.split('{').next() {
                        symbols.push(sig.trim().to_string());
                    }
                }
            }
            _ => {}
        }
    }

    // Limit to 15 symbols per file
    symbols.truncate(15);
    symbols
}

// ─── System prompt generation ──────────────────────────────────────────────

impl WorkspaceInfo {
    /// Generate a system prompt that gives the AI context about the project
    pub fn to_system_prompt(&self) -> String {
        let mut prompt = format!(
            "You are assisting with the project \"{}\".\n\
             Project type: {:?}\n\
             Root: {}\n",
            self.name,
            self.project_type,
            self.root.display()
        );

        if !self.description.is_empty() {
            prompt.push_str(&format!("Description: {}\n", self.description));
        }

        if !self.tech_stack.is_empty() {
            prompt.push_str(&format!("Tech stack: {}\n", self.tech_stack.join(", ")));
        }

        if !self.key_files.is_empty() {
            prompt.push_str(&format!("Key files: {}\n", self.key_files.join(", ")));
        }

        prompt.push_str(
            "\nWhen writing code, follow the conventions and patterns of this project. ",
        );
        prompt.push_str("Use the project's existing dependencies and style.\n");

        prompt
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detect_rust_project() {
        let dir = std::env::temp_dir().join("nerve_test_rust");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"myapp\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Rust);
        assert_eq!(ws.name, "myapp");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_node_project() {
        let dir = std::env::temp_dir().join("nerve_test_node");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("package.json"),
            r#"{"name":"myapp","description":"test"}"#,
        )
        .unwrap();

        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Node);
        assert_eq!(ws.name, "myapp");
        assert_eq!(ws.description, "test");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_python_project() {
        let dir = std::env::temp_dir().join("nerve_test_python");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"mypyapp\"",
        )
        .unwrap();

        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Python);
        assert_eq!(ws.name, "mypyapp");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_go_project() {
        let dir = std::env::temp_dir().join("nerve_test_go");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("go.mod"), "module github.com/user/mygoapp\n\ngo 1.21\n").unwrap();

        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Go);
        assert_eq!(ws.name, "github.com/user/mygoapp");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_project_detected() {
        let dir = std::env::temp_dir().join("nerve_test_empty");
        fs::create_dir_all(&dir).unwrap();
        // Remove any marker files that might exist from other tests
        for f in ["Cargo.toml", "package.json", "pyproject.toml", "go.mod"] {
            fs::remove_file(dir.join(f)).ok();
        }

        // Create an inner directory so walk-up doesn't find our own project
        let inner = dir.join("inner");
        fs::create_dir_all(&inner).unwrap();

        let _ws = detect_workspace_at(&inner);
        // It may detect a parent project — check that it doesn't detect within inner
        // The empty inner dir itself should not be detected
        assert!(try_detect(&inner).is_none());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn system_prompt_includes_project_info() {
        let ws = WorkspaceInfo {
            root: PathBuf::from("/tmp/test"),
            project_type: ProjectType::Rust,
            name: "testproject".into(),
            description: "A test project".into(),
            key_files: vec!["Cargo.toml".into(), "src/main.rs".into()],
            tech_stack: vec!["Rust".into(), "tokio".into()],
        };

        let prompt = ws.to_system_prompt();
        assert!(prompt.contains("testproject"));
        assert!(prompt.contains("Rust"));
        assert!(prompt.contains("A test project"));
        assert!(prompt.contains("tokio"));
        assert!(prompt.contains("Cargo.toml"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("conventions and patterns"));
    }

    #[test]
    fn system_prompt_omits_empty_description() {
        let ws = WorkspaceInfo {
            root: PathBuf::from("/tmp/test"),
            project_type: ProjectType::Node,
            name: "myapp".into(),
            description: String::new(),
            key_files: vec!["package.json".into()],
            tech_stack: vec!["Node.js".into()],
        };

        let prompt = ws.to_system_prompt();
        assert!(!prompt.contains("Description:"));
        assert!(prompt.contains("myapp"));
        assert!(prompt.contains("Node"));
    }

    #[test]
    fn detect_rust_with_dependencies() {
        let dir = std::env::temp_dir().join("nerve_test_rust_deps");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"webserver\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\ntokio = \"1\"\naxum = \"0.7\"\nserde = \"1\"\n",
        )
        .unwrap();

        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Rust);
        assert_eq!(ws.name, "webserver");
        assert!(ws.tech_stack.contains(&"tokio".to_string()));
        assert!(ws.tech_stack.contains(&"axum".to_string()));
        assert!(ws.tech_stack.contains(&"serde".to_string()));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_node_with_frameworks() {
        let dir = std::env::temp_dir().join("nerve_test_node_fw");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("package.json"),
            r#"{"name":"mywebapp","dependencies":{"react":"18","next":"14"},"devDependencies":{"typescript":"5"}}"#,
        )
        .unwrap();

        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Node);
        assert!(ws.tech_stack.contains(&"react".to_string()));
        assert!(ws.tech_stack.contains(&"next".to_string()));
        assert!(ws.tech_stack.contains(&"typescript".to_string()));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn malformed_cargo_toml_still_detects() {
        let dir = std::env::temp_dir().join("nerve_test_rust_bad");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Cargo.toml"), "this is not valid toml {{{}").unwrap();

        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Rust);
        // Name falls back to directory name
        assert_eq!(ws.name, "nerve_test_rust_bad");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn malformed_package_json_still_detects() {
        let dir = std::env::temp_dir().join("nerve_test_node_bad");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), "not json at all").unwrap();

        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Node);
        // Name falls back to directory name
        assert_eq!(ws.name, "nerve_test_node_bad");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_go_project_parses_module() {
        let dir = std::env::temp_dir().join("nerve_test_go_module");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("go.mod"), "module github.com/user/myapp\n\ngo 1.21\n").unwrap();
        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Go);
        assert_eq!(ws.name, "github.com/user/myapp");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_python_with_django() {
        let dir = std::env::temp_dir().join("nerve_test_django");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("requirements.txt"), "django==4.2\n").unwrap();
        fs::write(dir.join("manage.py"), "#!/usr/bin/env python\n").unwrap();
        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Python);
        assert!(ws.tech_stack.iter().any(|t| t == "Django"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn workspace_key_files_detected() {
        let dir = std::env::temp_dir().join("nerve_test_keyfiles");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(dir.join("README.md"), "# Test").unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
        let ws = detect_workspace_at(&dir).unwrap();
        assert!(ws.key_files.contains(&"README.md".to_string()));
        assert!(ws.key_files.contains(&"src/main.rs".to_string()));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_java_maven_project() {
        let dir = std::env::temp_dir().join("nerve_test_java");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("pom.xml"), "<project></project>").unwrap();
        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Java);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_ruby_project() {
        let dir = std::env::temp_dir().join("nerve_test_ruby");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Gemfile"), "source 'https://rubygems.org'").unwrap();
        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Ruby);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_elixir_project() {
        let dir = std::env::temp_dir().join("nerve_test_elixir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mix.exs"), "defmodule MyApp do end").unwrap();
        let ws = detect_workspace_at(&dir).unwrap();
        assert_eq!(ws.project_type, ProjectType::Elixir);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn system_prompt_contains_tech_stack() {
        let ws = WorkspaceInfo {
            root: std::path::PathBuf::from("/tmp"),
            project_type: ProjectType::Rust,
            name: "myapp".into(),
            description: "A test app".into(),
            key_files: vec!["Cargo.toml".into()],
            tech_stack: vec!["Rust".into(), "tokio".into(), "axum".into()],
        };
        let prompt = ws.to_system_prompt();
        assert!(prompt.contains("tokio"));
        assert!(prompt.contains("axum"));
        assert!(prompt.contains("myapp"));
    }

    #[test]
    fn node_detects_typescript() {
        let dir = std::env::temp_dir().join("nerve_test_node_ts");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("package.json"),
            r#"{"name":"tsapp","devDependencies":{"typescript":"5.0"}}"#,
        )
        .unwrap();
        let ws = detect_workspace_at(&dir).unwrap();
        assert!(ws.tech_stack.iter().any(|t| t == "typescript"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn generate_project_map_current_dir() {
        let root = std::env::current_dir().unwrap();
        let map = generate_project_map(&root, 2);
        assert!(map.contains("Cargo.toml"));
        assert!(map.contains("src/"));
        assert!(!map.contains("target/")); // Should be excluded
        assert!(!map.contains(".git")); // Should be excluded
    }

    #[test]
    fn extract_rust_symbols() {
        let content = r#"
pub fn hello() {
    println!("hi");
}

pub struct App {
    field: String,
}

fn private_fn() {}

pub enum Mode {
    Normal,
    Insert,
}
"#;
        let symbols = extract_symbols_from_content(content, "rs");
        assert!(symbols.iter().any(|s| s.contains("pub fn hello")));
        assert!(symbols.iter().any(|s| s.contains("pub struct App")));
        assert!(symbols.iter().any(|s| s.contains("pub enum Mode")));
        assert!(!symbols.iter().any(|s| s.contains("private_fn"))); // Private excluded
    }

    #[test]
    fn extract_python_symbols() {
        let content = "def hello():\n    pass\n\nclass MyClass:\n    pass\n";
        let symbols = extract_symbols_from_content(content, "py");
        assert!(symbols.iter().any(|s| s.contains("def hello")));
        assert!(symbols.iter().any(|s| s.contains("class MyClass")));
    }

    #[test]
    fn build_tree_excludes_hidden() {
        let root = std::env::current_dir().unwrap();
        let mut output = String::new();
        build_tree(&root, &root, &mut output, 0, 1);
        assert!(!output.contains(".git"));
        assert!(!output.contains("target/"));
    }
}
