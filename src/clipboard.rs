use anyhow::Context;

/// Copy text to the system clipboard.
///
/// Returns an error if the clipboard is unavailable (e.g. no display server
/// in a headless SSH session). Callers should handle this gracefully.
pub fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
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
    let mut clipboard = arboard::Clipboard::new().context("failed to access system clipboard")?;
    let text = clipboard
        .get_text()
        .context("failed to read text from clipboard")?;
    Ok(text)
}
