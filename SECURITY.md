# Security Policy

## Reporting Vulnerabilities

**Do not report security vulnerabilities via public GitHub issues.**

To report a vulnerability, email **raphael.lugmayr@stoicera.com** with:

- A clear description of the vulnerability.
- Steps to reproduce the issue.
- An assessment of the potential impact.
- Any suggested fixes, if applicable.

You will receive an initial response within **48 hours**. We will work with you to understand the issue and coordinate a fix and disclosure timeline.

## Supported Versions

| Version | Support Level |
|---------|--------------|
| Latest release | Full security support |
| Previous releases | Critical fixes only, on a case-by-case basis |

## Built-in Security Features

Nerve includes several security measures by design:

- **Shell injection blocking** — User inputs are sanitized before any shell interaction.
- **Protected system paths** — Critical system directories are shielded from write operations.
- **Sensitive file detection** — Nerve identifies and warns about sensitive files (credentials, keys, environment files) before processing.
- **Rate limiting** — API interactions are rate-limited to prevent abuse and runaway costs.

## Disclosure Policy

We follow responsible disclosure practices. Once a fix is available, we will publish a security advisory and credit the reporter (unless anonymity is requested).
