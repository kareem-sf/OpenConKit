# 0010. Temporary citationberg security revision pin

- Status: Accepted (temporary)
- Date: 2026-07-23

## Context

Typst 0.15.1 depends on `hayagriva` 0.10.1 and `citationberg` 0.7.0.
The `citationberg` 0.7.0 package published on crates.io constrains
`quick-xml` to the vulnerable 0.38 series:

- RUSTSEC-2026-0194: quadratic duplicate-attribute checking
- RUSTSEC-2026-0195: unbounded namespace-declaration allocation

Both defects can cause denial of service while parsing untrusted XML.
`quick-xml` 0.41.0 fixes them. Citationberg upstream commit
`06a591e2f237d25e1dfdedac3f3d1494c496c52d` updates its dependency to
0.41.0, but that fix was not yet available as a newer crates.io release.

## Decision

- Patch `citationberg` to exact upstream revision
  `06a591e2f237d25e1dfdedac3f3d1494c496c52d`.
- Require revision-pinned Git sources in `deny.toml`; branch and floating Git
  dependencies remain forbidden.
- Allow only the exact `https://github.com/typst/citationberg` source.
- Keep `cargo-deny` advisories, sources, bans, and licenses as release gates.

## Removal condition

Remove the `[patch.crates-io]` entry and the Git-source exception as soon as
Typst's compatible dependency graph resolves a crates.io `citationberg`
release using `quick-xml >= 0.41.0`. The replacement must pass the full
all-feature test and `cargo-deny` gates before merge.

## Consequences

- Positive: the optional PDF graph contains neither known `quick-xml`
  advisory and remains reproducible through `Cargo.lock`.
- Negative: builds temporarily depend on one reviewed Git revision in
  addition to crates.io, and dependency updates must explicitly re-evaluate
  the removal condition.
