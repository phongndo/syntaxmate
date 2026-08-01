# Security policy

## Supported versions

Until 1.0, security fixes are released from the latest published minor line.
After 1.0, the latest major release and the immediately previous major release
receive fixes when a practical backport exists.

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| Older 0.x minors | No |

## Reporting a vulnerability

Use GitHub's private vulnerability reporting form:

<https://github.com/phongndo/syntaxmate/security/advisories/new>

Do not open a public issue for a suspected denial of service, panic reachable
through untrusted grammar/theme/source input, terminal or HTML injection,
bundle-validation flaw, or vulnerable dependency. Include a minimal reproducer,
impact assessment, affected version, and any proposed mitigation. Maintainers
will acknowledge a complete report within seven days and coordinate disclosure
and a release when the issue is confirmed.

## Security model

Syntaxmate treats grammar JSON, theme JSON, source text, and renderer input as
untrusted. The tokenizer applies bounded fallback-regex and per-line work
limits. `HighlightStatus::Degraded` reports when those limits prevent complete
highlighting. HTML output escapes source and attributes; ANSI output sanitizes
source control characters by default.

The release library is safe Rust and performs no network, filesystem, or
process-environment access. Bundled assets are committed, checksummed, licensed,
and built offline. Node dependencies under `tools/golden-oracle` are development-only and are not
present in the crate archive.
