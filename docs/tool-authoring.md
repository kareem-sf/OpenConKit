# Tool authoring (skeleton)

How to add a new tool to the OpenConKit shell. The full guide lands with the
core-architecture phase; this skeleton fixes the contract so tools built now
will not need rework.

## The contract

A tool is a Rust crate `crates/openconkit-tool-<id>` that:

1. Implements `openconkit_tool_sdk::Tool` and returns a `ToolDescriptor`
   with a unique kebab-case `id`, the current `TOOL_CONTRACT_VERSION`, and
   i18n keys (`tools.<camelId>.name` / `.description`).
2. Depends on `openconkit-application` / infrastructure crates for its
   engine - never on the desktop host.
3. Registers itself in the compile-time registry used by the host
   (ADR 0003). There is no runtime plugin loading.

## UI side

- Tool UI lives in `apps/desktop-ui/src/tools/<id>/` and is mounted under a
  route derived from the tool id.
- All user-facing strings go into `packages/i18n` (en + ar); the parity
  test enforces key equality.
- Shared visual primitives come from `@openconkit/ui` and its design tokens.

## Scaffolding

`pnpm tool:new <id>` will generate the crate, i18n keys, route stub and
registry entry. (Implemented in phase 3; the command currently validates its
arguments and points here.)

## Ground rules for tools

- Never modify source documents; produce new artifacts.
- Every user-visible finding must be explainable (rule id + evidence cells).
- Respect the test tiers in `AGENTS.md`; detection rules ship with synthetic
  fixtures (`fixtures/`).
