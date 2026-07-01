use anyhow::Context;

/// Copy text to the system clipboard.
///
/// Returns an error if the clipboard is unavailable (e.g. no display server
/// in a headless SSH session). Callers should handle this gracefully.
pub fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    // Never touch the real OS clipboard from a test build. Unit tests that
    // exercise /copy, the clipboard manager, Ctrl+Y, etc. would otherwise
    // hijack the developer's actual system clipboard on every `cargo test`.
    if cfg!(test) {
        return Ok(());
    }
    let mut clipboard = arboard::Clipboard::new().context("failed to access system clipboard")?;
    clipboard
        .set_text(text)
        .context("failed to copy text to clipboard")?;
    Ok(())
}

/// Read text from the system clipboard.
///
/// Returns an error if the clipboard is unavailable or does not contain
/// text content.
pub fn paste_from_clipboard() -> anyhow::Result<String> {
    // Symmetric with copy_to_clipboard: tests must not read the real OS
    // clipboard either (non-deterministic, and pastes another app's content).
    if cfg!(test) {
        return Ok(String::new());
    }
    let mut clipboard = arboard::Clipboard::new().context("failed to access system clipboard")?;
    let text = clipboard
        .get_text()
        .context("failed to read text from clipboard")?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_and_paste_are_isolated_in_tests() {
        // The guards must make these no-ops under `cargo test` so unit tests
        // (e.g. /copy) can never hijack the real OS clipboard.
        assert!(copy_to_clipboard("must not reach the OS clipboard").is_ok());
        assert_eq!(paste_from_clipboard().unwrap(), "");
    }
}
