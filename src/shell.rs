use std::fmt::Write as _;

use anyhow::Context;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

/// Maximum number of output lines before truncation.
const MAX_OUTPUT_LINES: usize = 5000;

/// Default timeout for shell commands in seconds.
pub const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 30;

/// Interval between child process polls while waiting for completion.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Drain a child process pipe into a byte buffer, ignoring read errors
/// (partial data is still useful when a pipe breaks mid-read).
fn drain_pipe(pipe: Option<impl std::io::Read>) -> Vec<u8> {
    pipe.map(|mut r| {
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut r, &mut buf).ok();
        buf
    })
    .unwrap_or_default()
}

/// Wrap a value in single quotes with proper escaping for safe shell usage.
///
/// This is the standard POSIX approach: replace `'` with `'\''` and wrap
/// the whole string in single quotes.
pub fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Check if user input matches a slash command (exact or with arguments).
///
/// Returns `true` for `/cmd` (exact) or `/cmd args...` (with space separator).
/// Returns `false` for `/cmdx` (different command that shares a prefix).
pub fn matches_command(input: &str, cmd: &str) -> bool {
    input == cmd || input.starts_with(&format!("{cmd} "))
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
    /// Whether the command was killed due to timeout.
    pub timed_out: bool,
    /// How long the command took to complete (or until it was killed).
    pub elapsed: Duration,
}

/// Truncate text to at most `max_lines` lines, appending a notice if trimmed.
fn truncate_output(text: &str, max_lines: usize) -> String {
    let total_lines = text.lines().count();
    if total_lines <= max_lines {
        return text.to_string();
    }
    let mut truncated: String = text.lines().take(max_lines).collect::<Vec<_>>().join("\n");
    let _ = write!(
        truncated,
        "\n\n... ({} lines truncated, {total_lines} total)",
        total_lines - max_lines
    );
    truncated
}

/// Run a shell command and capture its output (no timeout).
pub fn run_command(cmd: &str) -> anyhow::Result<CommandResult> {
    let start = Instant::now();
    let output = Command::new("sh")
        .args(["-c", cmd])
        .output()
        .with_context(|| format!("Failed to execute: {cmd}"))?;

    let elapsed = start.elapsed();
    let stdout = truncate_output(&String::from_utf8_lossy(&output.stdout), MAX_OUTPUT_LINES);
    let stderr = truncate_output(&String::from_utf8_lossy(&output.stderr), MAX_OUTPUT_LINES);

    Ok(CommandResult {
        command: cmd.to_string(),
        stdout,
        stderr,
        exit_code: output.status.code().unwrap_or(-1),
        success: output.status.success(),
        timed_out: false,
        elapsed,
    })
}

/// Run a shell command with a timeout. If the process doesn't finish within
/// `timeout_secs`, it is killed and a `CommandResult` with `timed_out = true`
/// is returned.
///
/// A `timeout_secs` of `0` means no timeout (equivalent to `run_command`).
pub fn run_command_with_timeout(cmd: &str, timeout_secs: u64) -> anyhow::Result<CommandResult> {
    // No timeout requested -- fall through to the regular path.
    if timeout_secs == 0 {
        return run_command(cmd);
    }

    let start = Instant::now();

    let mut cmd_builder = Command::new(if cfg!(windows) { "cmd" } else { "sh" });
    if cfg!(windows) {
        cmd_builder.args(["/C", cmd]);
    } else {
        cmd_builder.args(["-c", cmd]);
    }
    cmd_builder
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // On Unix, put the child in its own process group so we can kill the
    // entire tree (shell + its children) on timeout.
    #[cfg(unix)]
    unsafe {
        cmd_builder.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    let mut child = cmd_builder
        .spawn()
        .with_context(|| format!("Failed to spawn: {cmd}"))?;

    let timeout = Duration::from_secs(timeout_secs);

    // Poll the child in a tight loop with short sleeps.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process finished within the timeout.
                let elapsed = start.elapsed();
                let stdout_raw = drain_pipe(child.stdout.take());
                let stderr_raw = drain_pipe(child.stderr.take());

                let stdout =
                    truncate_output(&String::from_utf8_lossy(&stdout_raw), MAX_OUTPUT_LINES);
                let stderr =
                    truncate_output(&String::from_utf8_lossy(&stderr_raw), MAX_OUTPUT_LINES);

                return Ok(CommandResult {
                    command: cmd.to_string(),
                    stdout,
                    stderr,
                    exit_code: status.code().unwrap_or(-1),
                    success: status.success(),
                    timed_out: false,
                    elapsed,
                });
            }
            Ok(None) => {
                // Still running -- check timeout.
                if start.elapsed() >= timeout {
                    // Kill the entire process tree on timeout.
                    #[cfg(unix)]
                    unsafe {
                        // Kill the process group (shell + children).
                        let pid = child.id() as libc::pid_t;
                        libc::kill(-pid, libc::SIGKILL);
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = child.kill();
                    }
                    let _ = child.wait(); // Reap the zombie.

                    // Capture any partial output produced before the timeout.
                    let partial_stdout = drain_pipe(child.stdout.take());
                    let partial_stderr = drain_pipe(child.stderr.take());

                    let stdout = truncate_output(
                        &String::from_utf8_lossy(&partial_stdout),
                        MAX_OUTPUT_LINES,
                    );
                    let mut stderr = String::from_utf8_lossy(&partial_stderr).to_string();
                    if !stderr.is_empty() {
                        stderr.push('\n');
                    }
                    let _ = write!(stderr, "Command timed out after {timeout_secs}s");

                    let elapsed = start.elapsed();
                    return Ok(CommandResult {
                        command: cmd.to_string(),
                        stdout,
                        stderr,
                        exit_code: -1,
                        success: false,
                        timed_out: true,
                        elapsed,
                    });
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error waiting for process: {e}"));
            }
        }
    }
}

/// Format command output for display in conversation.
pub fn format_command_output(result: &CommandResult) -> String {
    let mut output = format!("$ {}\n", result.command);

    if result.timed_out {
        let _ = write!(
            output,
            "\nCommand timed out after {:.1}s\n",
            result.elapsed.as_secs_f64()
        );
        return output;
    }

    if !result.stdout.is_empty() {
        output.push_str(&result.stdout);
        if !result.stdout.ends_with('\n') {
            output.push('\n');
        }
    }

    if !result.stderr.is_empty() {
        let _ = write!(output, "\nstderr:\n{}", result.stderr);
    }

    if !result.success {
        let _ = write!(output, "\nExit code: {}", result.exit_code);
    }

    output
}

/// Format command output as context for AI (in a code block).
pub fn format_command_for_context(result: &CommandResult) -> String {
    let status = if result.success {
        "success".to_string()
    } else {
        format!("failed (exit {})", result.exit_code)
    };
    let mut content = format!("Command: `{}` ({})\n\n```\n", result.command, status);

    if !result.stdout.is_empty() {
        content.push_str(&result.stdout);
    }
    if !result.stderr.is_empty() {
        if !result.stdout.is_empty() {
            content.push('\n');
        }
        content.push_str("--- stderr ---\n");
        content.push_str(&result.stderr);
    }

    content.push_str("\n```");
    content
}

// ─── Security helpers ───────────────────────────────────────────────────────

/// Commands that should NEVER be run (substring matches).
const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    // Reversed flag order (`-fr`) is equally destructive.
    "rm -fr /",
    "rm -fr /*",
    "rm -fr ~",
    "mkfs",
    "dd if=",
    "> /dev/sd",
    "> /dev/nvm",
    "chmod -r 777 /",
    ":(){ :|:& };:", // fork bomb
    "eval $(",
    "eval `",
    "> /etc/",
    "sudo rm",
    "sudo dd",
    "sudo mkfs",
    // Installing system packages as root is not something an autonomous agent
    // should ever do on its own: it mutates the host outside the repo and can
    // pull arbitrary code. (A project-local `npm install` is untouched — this is
    // specifically privileged, system-wide installation.) Previously unguarded:
    // nerve's own test suite ran `sudo apt-get install malware` FOR REAL on
    // whatever machine ran `cargo test`, and it executed as root on the server.
    "sudo apt-get install",
    "sudo apt install",
    "sudo yum install",
    "sudo dnf install",
    "sudo pacman -s",
    "sudo snap install",
    "sudo pip install",
    "tee /etc/",
    "passwd",
    "chpasswd",
    "shutdown",
    "reboot",
    "poweroff",
    "halt",
    "init 0",
    "init 6",
    "systemctl poweroff",
    "systemctl reboot",
    "systemctl halt",
    "shred",
    "wipe",
];

/// Piped download-to-execution patterns: (prefix, pipe target).
/// Matches when the command contains the prefix somewhere before `| target`.
const BLOCKED_PIPE_PATTERNS: &[(&str, &str)] = &[
    ("curl", "| sh"),
    ("curl", "| bash"),
    ("curl", "| zsh"),
    ("curl", "| ksh"),
    ("curl", "| dash"),
    ("curl", "| python"),
    ("curl", "| perl"),
    ("curl", "| ruby"),
    ("wget", "| sh"),
    ("wget", "| bash"),
    ("wget", "| zsh"),
    ("wget", "| python"),
    ("wget", "| perl"),
];

/// Returns `true` if the command matches a well-known destructive pattern
/// that should never be run from within Nerve.
pub fn is_dangerous_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    // Collapse runs of whitespace (spaces, tabs, newlines) to a single space so
    // trivial spacing variants like "rm  -rf   /" or "rm\t-rf /" still match the
    // substring patterns. Defense-in-depth behind the confirmation gate, not a
    // full shell parser.
    let normalized: String = lower.split_whitespace().collect::<Vec<_>>().join(" ");

    if BLOCKED_PATTERNS.iter().any(|p| normalized.contains(p)) {
        return true;
    }

    // Check piped download-to-exec patterns (e.g. "curl url | bash")
    for (prefix, pipe) in BLOCKED_PIPE_PATTERNS {
        if let Some(prefix_pos) = normalized.find(prefix)
            && let Some(pipe_pos) = normalized.find(pipe)
            && pipe_pos > prefix_pos
        {
            return true;
        }
    }

    // Structural layer: catch a catastrophic recursive-force delete of a system
    // root / home regardless of flag spelling or order, or a path-prefixed
    // binary — bypasses the fixed substring patterns miss, e.g.
    // `rm --recursive --force /`, `rm -r -f ~`, `/bin/rm -rf /`.
    if has_catastrophic_rm(&normalized) {
        return true;
    }

    false
}

/// Detect `rm` invoked with BOTH recursive and force, targeting a catastrophic
/// root (`/`, `/*`, `~`, `$HOME`, or a top-level system dir), across command
/// segments. Resistant to flag order (`-rf`/`-fr`/`-r -f`), long flags
/// (`--recursive --force`), and an absolute path to the binary (`/bin/rm`).
fn has_catastrophic_rm(normalized: &str) -> bool {
    // Split into simple-command segments on shell operators.
    for segment in normalized.split(['|', ';', '&', '\n']) {
        let mut tokens = segment.split_whitespace().peekable();
        // Skip common privilege/wrapper prefixes and VAR=val assignments.
        while let Some(&t) = tokens.peek() {
            if t == "sudo" || t == "env" || t == "nice" || t == "nohup" || t.contains('=') {
                tokens.next();
            } else {
                break;
            }
        }
        let Some(cmd0) = tokens.next() else { continue };
        // Basename of argv[0] so `/bin/rm` and `/usr/bin/rm` are caught.
        let base = cmd0.rsplit('/').next().unwrap_or(cmd0);
        if base != "rm" {
            continue;
        }
        let mut recursive = false;
        let mut force = false;
        let mut targets: Vec<&str> = Vec::new();
        for tok in tokens {
            if let Some(long) = tok.strip_prefix("--") {
                match long {
                    "recursive" => recursive = true,
                    "force" => force = true,
                    _ => {}
                }
            } else if let Some(short) = tok.strip_prefix('-') {
                // Combined short flags like -rf / -fr / -Rf (already lowercased).
                if short.contains('r') {
                    recursive = true;
                }
                if short.contains('f') {
                    force = true;
                }
            } else {
                targets.push(tok);
            }
        }
        if recursive && force && targets.iter().any(|t| is_catastrophic_rm_target(t)) {
            return true;
        }
    }
    false
}

/// True for delete targets that would be catastrophic: filesystem root, home,
/// or a top-level system directory (with optional trailing `/` or `/*`).
/// `t` is expected lowercased. Deep paths like `/usr/local/myapp` are NOT
/// flagged — only the roots themselves — to avoid false positives.
fn is_catastrophic_rm_target(t: &str) -> bool {
    if matches!(
        t,
        "/" | "/*" | "~" | "~/" | "~/*" | "$home" | "${home}" | "$home/" | "$home/*"
    ) {
        return true;
    }
    let base = t.trim_end_matches("/*").trim_end_matches('/');
    const ROOTS: &[&str] = &[
        "/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64", "/var", "/boot", "/sys", "/proc",
        "/dev", "/root", "/home", "/opt",
    ];
    ROOTS.contains(&base)
}

/// Paths that should never be written to by the agent.
pub fn is_protected_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");

    // Allow writes inside the operating system's temporary directory. On macOS
    // this lives under /var/folders/ (or /private/var/folders/), which would
    // otherwise be caught by the "/var/" rule below. Comparing against
    // std::env::temp_dir() keeps this correct cross-platform.
    let temp_dir = std::env::temp_dir();
    if let Some(temp) = temp_dir.to_str() {
        let temp = temp.replace('\\', "/");
        // Normalize trailing slash for prefix comparison.
        let temp_prefix = temp.trim_end_matches('/');
        // macOS reports the temp dir under /var/folders but the real path is
        // /private/var/folders (and vice versa via the /private symlink).
        let strip_private = |p: &str| -> String {
            p.strip_prefix("/private")
                .map(|s| s.to_string())
                .unwrap_or_else(|| p.to_string())
        };
        let candidate = strip_private(&normalized);
        let temp_norm = strip_private(temp_prefix);
        let is_under = |base: &str| -> bool {
            !base.is_empty()
                && (normalized == base
                    || normalized.starts_with(&format!("{base}/"))
                    || candidate == base
                    || candidate.starts_with(&format!("{base}/")))
        };
        if is_under(temp_prefix) || is_under(&temp_norm) {
            return false;
        }
    }

    let protected = [
        "/etc/", "/usr/", "/bin/", "/sbin/", "/boot/", "/dev/", "/proc/", "/sys/", "/var/",
    ];
    // Check both the raw path and common prefixes
    protected
        .iter()
        .any(|p| normalized.starts_with(p) || normalized == p.trim_end_matches('/'))
}

/// Files that may contain secrets and should not be read by the agent.
pub fn is_sensitive_file(path: &str) -> bool {
    let sensitive = [
        ".env",
        ".env.local",
        ".env.production",
        "id_rsa",
        "id_ed25519",
        "id_ecdsa",
        ".ssh/",
        ".gnupg/",
        ".aws/credentials",
        ".netrc",
        ".pgpass",
    ];
    sensitive.iter().any(|s| path.contains(s))
}

/// High-value targets the agent must never WRITE, even when they live in the
/// user's home or a repo rather than a system dir. These are the classic
/// persistence / privilege / exfiltration vectors a prompt-injected model would
/// aim for. Checked (in `validate_write_path`) on the normalized + canonical
/// path so `..` and symlink tricks can't sneak past it.
pub fn is_protected_write_target(path: &str) -> bool {
    // Git hooks execute arbitrary code on the next git operation.
    if path.contains("/.git/hooks/") {
        return true;
    }
    // Project memory (.nerve/) is injected into every future prompt — letting
    // the agent write it directly would allow prompt-injection to plant
    // persistent instructions. Writes go through the ProjectStore API only
    // (user commands + the `remember` tool, which appends sanitized bullets).
    if path.contains("/.nerve/")
        || path.starts_with(".nerve/")
        || path.ends_with("/.nerve")
        || path == ".nerve"
    {
        return true;
    }
    // Shell startup files — persistence across every new shell.
    const RC_FILES: &[&str] = &[
        "/.bashrc",
        "/.bash_profile",
        "/.bash_login",
        "/.profile",
        "/.zshrc",
        "/.zshenv",
        "/.zprofile",
        "/.config/fish/config.fish",
    ];
    if RC_FILES.iter().any(|f| path.ends_with(f)) {
        return true;
    }
    // SSH: authorized_keys grants remote access; config can silently reroute
    // connections. Credentials files should never be overwritten by the agent.
    const SUFFIXES: &[&str] = &[
        "/.ssh/authorized_keys",
        "/.ssh/config",
        "/.aws/credentials",
        "/.netrc",
        "/.pgpass",
    ];
    SUFFIXES.iter().any(|s| path.ends_with(s))
}

/// Mask an API key for display, showing only the first 4 and last 4 characters.
#[allow(dead_code)]
pub fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".into();
    }
    let prefix = &key[..4];
    let suffix = &key[key.len() - 4..];
    format!("{prefix}...{suffix}")
}

/// Detect the project's test command based on what files exist.
pub fn detect_test_command() -> &'static str {
    let cwd = std::env::current_dir().unwrap_or_default();

    if cwd.join("Cargo.toml").exists() {
        return "cargo test";
    }
    if cwd.join("package.json").exists() {
        return "npm test";
    }
    if cwd.join("go.mod").exists() {
        return "go test ./...";
    }
    if cwd.join("pyproject.toml").exists() {
        return "pytest";
    }
    if cwd.join("Gemfile").exists() {
        return "bundle exec rspec";
    }
    if cwd.join("mix.exs").exists() {
        return "mix test";
    }
    if cwd.join("build.gradle").exists() || cwd.join("build.gradle.kts").exists() {
        return "gradle test";
    }
    if cwd.join("pom.xml").exists() {
        return "mvn test";
    }
    if cwd.join("Makefile").exists() {
        return "make test";
    }

    "echo 'No test command detected'"
}

/// Detect the project's build command.
pub fn detect_build_command() -> &'static str {
    let cwd = std::env::current_dir().unwrap_or_default();

    if cwd.join("Cargo.toml").exists() {
        return "cargo build";
    }
    if cwd.join("package.json").exists() {
        return "npm run build";
    }
    if cwd.join("go.mod").exists() {
        return "go build ./...";
    }
    if cwd.join("Makefile").exists() {
        return "make";
    }
    if cwd.join("build.gradle").exists() || cwd.join("build.gradle.kts").exists() {
        return "gradle build";
    }
    if cwd.join("pom.xml").exists() {
        return "mvn package";
    }
    if cwd.join("CMakeLists.txt").exists() {
        return "cmake --build build";
    }

    "echo 'No build command detected'"
}

/// Detect the project's lint command.
pub fn detect_lint_command() -> &'static str {
    let cwd = std::env::current_dir().unwrap_or_default();

    if cwd.join("Cargo.toml").exists() {
        return "cargo clippy 2>&1";
    }
    if cwd.join("package.json").exists() {
        return "npx eslint . 2>&1";
    }
    if cwd.join("go.mod").exists() {
        return "golangci-lint run 2>&1";
    }
    if cwd.join("pyproject.toml").exists() || cwd.join("setup.py").exists() {
        return "ruff check . 2>&1";
    }
    if cwd.join("Gemfile").exists() {
        return "bundle exec rubocop 2>&1";
    }
    if cwd.join("mix.exs").exists() {
        return "mix credo 2>&1";
    }

    "echo 'No lint command detected'"
}

/// Detect the project's format command.
pub fn detect_format_command() -> &'static str {
    let cwd = std::env::current_dir().unwrap_or_default();

    if cwd.join("Cargo.toml").exists() {
        return "cargo fmt";
    }
    if cwd.join("package.json").exists() {
        return "npx prettier --write .";
    }
    if cwd.join("go.mod").exists() {
        return "gofmt -w .";
    }
    if cwd.join("pyproject.toml").exists() || cwd.join("setup.py").exists() {
        return "ruff format .";
    }
    if cwd.join("Gemfile").exists() {
        return "bundle exec rubocop -A";
    }
    if cwd.join("mix.exs").exists() {
        return "mix format";
    }

    "echo 'No format command detected'"
}

/// Get the git diff.  Arguments are passed through shell escaping to prevent
/// injection via metacharacters like `$(...)`, backticks, pipes, etc.
pub fn git_diff(args: &str) -> anyhow::Result<CommandResult> {
    let cmd = if args.is_empty() {
        "git diff".to_string()
    } else {
        // Escape each argument individually to prevent shell injection.
        let safe_args: Vec<String> = args.split_whitespace().map(shell_escape).collect();
        format!("git diff {}", safe_args.join(" "))
    };
    run_command(&cmd)
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
