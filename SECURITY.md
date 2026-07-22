# Security Policy

## Supported versions

| Version      | Supported                           |
| ------------ | ----------------------------------- |
| 0.0.x (main) | Yes, until the first stable release |

OpenConKit is pre-release software. Security fixes land on `main` and are
included in the next release.

## Reporting a vulnerability

Please do NOT open a public issue for security problems.

- Report privately through GitHub Security Advisories:
  https://github.com/kareem-sf/openconkit/security/advisories/new
- If that is unavailable, contact the maintainer listed as `@kareem-sf`
  in `.github/CODEOWNERS` through a private GitHub message.

Please include: affected version/commit, reproduction steps, impact, and any
mitigation you are aware of. We aim to acknowledge reports within 72 hours.

## Scope notes

The threat model in `docs/threat-model.md` describes what we defend against,
including malicious spreadsheets, formula injection in exports, path
traversal, sidecar command execution, updater compromise, and supply-chain
risks. Reports outside that model are still welcome.

OpenConKit is local-first: it does not run a network service, and user
documents never leave the machine (except when the user explicitly uses the
optional AI features). Vulnerabilities that break these properties are
treated as high severity.
