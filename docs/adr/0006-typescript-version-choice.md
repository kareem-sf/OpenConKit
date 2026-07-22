# 0006. TypeScript version choice

- Status: Accepted
- Date: 2026-07-23

## Context

`typescript@latest` on npm is 7.0.2 (the native-preview line). The lint
toolchain must support the chosen compiler: typescript-eslint 8.65.0
declares a peer range of `typescript >=4.8.4 <6.1.0`, so TypeScript 7 (and
any 6.1+) is outside its supported range.

## Decision

Pin **TypeScript 6.0.3** - the newest version inside typescript-eslint's
supported range - across the root and all workspace packages. ESLint stays
on 9.x with the flat config (widely supported by all plugins in use).

## Consequences

- Positive: fully supported lint + typecheck path today; no peer warnings,
  no unsupported-version diagnostics from typescript-eslint.
- Negative: not on the absolute latest compiler; upgrade to 7.x is
  deliberate future work once typescript-eslint (and the wider plugin
  ecosystem) declares support.
- Review trigger: a typescript-eslint release whose peer range includes
  TypeScript 7.
