#!/usr/bin/env node
// Fail closed when a production npm dependency introduces an unreviewed
// license expression. Rust dependencies are checked separately by cargo-deny.

import { spawnSync } from "node:child_process";

const ALLOWED_LICENSE_EXPRESSIONS = new Set([
  "0BSD",
  "Apache-2.0",
  "Apache-2.0 OR MIT",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "CC0-1.0",
  "ISC",
  "MIT",
  "MIT-0",
  "MIT OR Apache-2.0",
  "MPL-2.0",
  "Unicode-3.0",
  "Unlicense",
  "Zlib",
]);

const command = process.platform === "win32" ? (process.env.ComSpec ?? "cmd.exe") : "pnpm";
const commandArgs =
  process.platform === "win32"
    ? ["/d", "/s", "/c", "pnpm licenses list --prod --json"]
    : ["licenses", "list", "--prod", "--json"];
const result = spawnSync(command, commandArgs, {
  encoding: "utf8",
});
if (result.error || result.status !== 0) {
  process.stderr.write(result.stderr ?? "");
  console.error(
    `license-check: pnpm license inventory failed${
      result.error ? `: ${result.error.message}` : ` with exit code ${result.status}`
    }`,
  );
  process.exit(1);
}

let inventory;
try {
  inventory = JSON.parse(result.stdout);
} catch (error) {
  console.error(`license-check: invalid pnpm JSON output: ${String(error)}`);
  process.exit(1);
}

const rejected = Object.entries(inventory)
  .filter(([license]) => !ALLOWED_LICENSE_EXPRESSIONS.has(license))
  .flatMap(([license, packages]) =>
    packages.map(
      (dependency) => `${dependency.name}@${dependency.versions.join(",")} (${license})`,
    ),
  );

if (rejected.length > 0) {
  console.error("license-check: unreviewed production dependency licenses:");
  for (const dependency of rejected) {
    console.error(`  ${dependency}`);
  }
  process.exit(1);
}

const dependencyCount = Object.values(inventory).reduce(
  (count, dependencies) => count + dependencies.length,
  0,
);
console.log(
  `license-check: ${dependencyCount} production dependencies use reviewed license expressions.`,
);
