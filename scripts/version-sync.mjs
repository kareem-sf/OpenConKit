#!/usr/bin/env node
// version-sync: propagate the canonical VERSION file into all version
// targets (Cargo workspace, package.json files, tauri.conf.json).
// Usage: node scripts/version-sync.mjs

import { readCanonicalVersion, run } from "./lib/version-targets.mjs";

const { changed } = run("sync");
const version = readCanonicalVersion();

if (changed.length === 0) {
  console.log(`version-sync: all targets already at ${version}`);
} else {
  console.log(`version-sync: updated to ${version}:`);
  for (const path of changed) {
    console.log(`  ${path}`);
  }
}
