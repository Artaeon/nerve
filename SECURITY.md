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

Nerve includes defense-in-depth security measures:

- **Shell injection blocking** — All user inputs are shell-escaped via POSIX single-quoting before any shell interaction. 30+ dangerous command patterns blocked (rm -rf, mkfs, fork bombs, poweroff, shred, etc.). Pipe-to-exec patterns blocked for 8+ interpreters.
- **Protected system paths** — Agent tools cannot write to /etc, /usr, /bin, /sbin, /boot, /proc, /sys, /dev. Path traversal via `..` segments is normalized before checking. Symlink attacks defeated via canonicalization.
- **SSRF protection** — URLs are parsed (not string-matched) to block private IPs, IPv6 loopback, link-local, ULA, multicast, IPv4-mapped/compatible IPv6, and CGN ranges. Final URL re-validated after redirect chains.
- **Device file protection** — Character devices (/dev/zero, /dev/random), block devices, FIFOs, and sockets are blocked from file read operations to prevent DoS.
- **Sensitive file detection** — Nerve identifies and warns about credentials, SSH keys, .env files, .aws/credentials before processing.
- **ANSI sanitization** — Plugin output is stripped of all ANSI escape sequences (CSI, OSC, SS2/SS3) to prevent terminal manipulation.
- **History ID sanitization** — Conversation IDs are restricted to alphanumeric characters and hyphens to prevent path traversal.
- **Rate limiting** — Tool executions capped at 100/session. API retries use exponential backoff with jitter.
- **Clipboard security** — Files written with 0600 permissions on Unix. Entry size capped at 1MB.
- **Plugin sandboxing** — 30-second timeout, output sanitized, control characters stripped.

## Disclosure Policy

We follow responsible disclosure practices. Once a fix is available, we will publish a security advisory and credit the reporter (unless anonymity is requested).
