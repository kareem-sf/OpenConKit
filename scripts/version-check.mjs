#!/usr/bin/env node
// version-check: fail if any version target drifts from the canonical
// VERSION file. Run in CI and before releases.
// Usage: node scripts/version-check.mjs

import { readCanonicalVersion, run } from "./lib/version-targets.mjs";

const { errors } = run("check");

if (errors.length > 0) {
  console.error(`version-check: version mismatch against VERSION (${readCanonicalVersion()}):`);
  for (const error of errors) {
    console.error(`  ${error}`);
  }
  console.error("Run `pnpm version:sync` to fix.");
  process.exit(1);
}

console.log(`version-check: all targets match VERSION (${readCanonicalVersion()})`);
