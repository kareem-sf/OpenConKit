#!/usr/bin/env node
// contracts:check — drift-check the committed ts-rs bindings.
//
// Regenerates the bindings into a throwaway directory and compares them
// byte-for-byte against the committed files in packages/contracts/src/generated.
// Exits 1 (and lists the differing files) if the Rust types and the committed
// bindings disagree. See docs/adr/0005-ts-rs-generated-contracts.md.

import { spawnSync } from "node:child_process";
import { exit } from "node:process";

const result = spawnSync("cargo", ["run", "-p", "openconkit-contracts-export", "--", "--check"], {
  stdio: "inherit",
  shell: process.platform === "win32",
});

if (result.error) {
  console.error("contracts:check: failed to spawn cargo:", result.error.message);
  exit(1);
}
exit(result.status ?? 0);
