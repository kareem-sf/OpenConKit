# 0003. Compile-time tool registry, no runtime plugins

- Status: Accepted
- Date: 2026-07-23

## Context

OpenConKit hosts multiple tools (starting with BOQ Inspector) in one shell.
Tools could be loaded at runtime (plugins/extensions) or wired at compile
time.

## Decision

Tools are Rust crates implementing the versioned `openconkit-tool-sdk`
contract (`Tool` trait + `ToolDescriptor`, `TOOL_CONTRACT_VERSION`). They
are registered in an explicit, static `ToolRegistry` at application
startup. There is no runtime plugin loading, no extension marketplace, and
no dynamic code download.

## Consequences

- Positive: every shipped tool is reviewed, tested and signed as part of
  the release; no plugin sandboxing problem; no supply-chain path through
  third-party extensions; the contract version makes breaking changes
  explicit.
- Negative: third parties cannot ship tools without a pull request; adding
  a tool requires a release.
- Revisit only with a signed-plugin design and a sandbox story (see
  `ROADMAP.md`, deferred features).
