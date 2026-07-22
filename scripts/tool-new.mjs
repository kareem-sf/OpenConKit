#!/usr/bin/env node
// tool-new: scaffold a new tool crate + UI registration.
//
// The full scaffolder (crate generation, contract wiring, i18n keys,
// registry registration) is implemented in the core-architecture phase
// (see ROADMAP.md, phase 3). For now this entry point validates its
// arguments and fails loudly with guidance instead of half-scaffolding.

const KEBAB_CASE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

function fail(message) {
  console.error(`tool-new: error: ${message}`);
  process.exit(1);
}

const args = process.argv.slice(2);

if (args.includes("--help") || args.includes("-h")) {
  console.log(`Usage: node scripts/tool-new.mjs <tool-id>

  <tool-id>   kebab-case identifier, e.g. "boq-inspector"

Scaffolds crates/openconkit-tool-<tool-id> and its UI registration.
Implemented in phase 3 (core architecture); see ROADMAP.md.`);
  process.exit(0);
}

if (args.length !== 1) {
  fail(`expected exactly one argument (the tool id), got ${args.length}. See --help.`);
}

const [toolId] = args;
if (!KEBAB_CASE.test(toolId)) {
  fail(`invalid tool id "${toolId}": must be kebab-case (e.g. "boq-inspector").`);
}

fail(
  `"${toolId}" is valid, but the scaffolder is not implemented yet. ` +
    `It lands in phase 3 (core architecture) - see ROADMAP.md and docs/tool-authoring.md.`,
);
