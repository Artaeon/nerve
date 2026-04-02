# Contributing to Nerve

Thank you for your interest in contributing to Nerve. This guide covers the process for submitting changes.

## Development Setup

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build
cargo test
```

Requires Rust stable (latest). Install via [rustup](https://rustup.rs/).

## Code Standards

- **Formatting:** Run `cargo fmt` before committing. All code must be formatted.
- **Linting:** Run `cargo clippy` and resolve all warnings. Zero warnings policy.
- **Testing:** New features must include tests. Bug fixes should include a regression test where practical.
- **Documentation:** Public APIs should have doc comments.

## Pull Request Process

1. **Open an issue first** for large changes or new features. Discuss the approach before writing code.
2. Fork the repository and create a feature branch from `master`.
3. Keep PRs focused: one logical change per pull request.
4. Write a clear PR description explaining what changed and why.
5. Ensure all CI checks pass before requesting review.

## CI Checks

Every PR is validated against the following:

- `cargo build` (debug and release)
- `cargo test`
- `cargo clippy` (zero warnings)
- `cargo fmt --check`
- Security audit (`cargo audit`)

Your PR will not be merged until all checks pass.

## Commit Style

- Use imperative mood: "Add feature" not "Added feature" or "Adds feature".
- Keep the subject line concise (under 72 characters).
- Reference related issues where applicable (e.g., `Fixes #42`).

## Reporting Bugs

Open a GitHub issue using the bug report template. Include reproduction steps, expected vs. actual behavior, and your environment details.

## Feature Requests

Open a GitHub issue using the feature request template. Describe the use case and proposed solution.

## Security Vulnerabilities

Do **not** report security issues via public GitHub issues. See [SECURITY.md](SECURITY.md) for responsible disclosure instructions.

## Code of Conduct

All contributors are expected to follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (MIT).
