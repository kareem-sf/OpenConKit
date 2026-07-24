#!/usr/bin/env node
// High-confidence local secret scan over tracked and untracked repository
// files. GitHub secret scanning should also remain enabled at repository
// level; this deterministic gate gives pull requests immediate feedback.

import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const listing = spawnSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], {
  encoding: "utf8",
});
if (listing.error || listing.status !== 0) {
  process.stderr.write(listing.stderr ?? "");
  console.error("secret-scan: could not enumerate repository files");
  process.exit(1);
}

const patterns = [
  ["private key", /-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----/g],
  ["GitHub token", /\bgh[pousr]_[A-Za-z0-9]{36,255}\b/g],
  ["GitHub fine-grained token", /\bgithub_pat_[A-Za-z0-9_]{40,255}\b/g],
  ["AWS access key", /\bAKIA[0-9A-Z]{16}\b/g],
  ["OpenAI API key", /\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b/g],
  ["Slack token", /\bxox[baprs]-[A-Za-z0-9-]{20,}\b/g],
  ["Google API key", /\bAIza[0-9A-Za-z_-]{35}\b/g],
  ["Stripe live secret", /\bsk_live_[0-9A-Za-z]{20,}\b/g],
];

const ignoredFiles = new Set(["scripts/secret-scan.mjs"]);
const findings = [];
for (const file of listing.stdout.split("\0").filter(Boolean)) {
  const normalized = file.replaceAll("\\", "/");
  if (ignoredFiles.has(normalized)) {
    continue;
  }
  // `git ls-files --cached` includes tracked paths staged for deletion.
  if (!existsSync(file)) {
    continue;
  }
  const bytes = readFileSync(file);
  if (bytes.includes(0)) {
    continue;
  }
  const contents = bytes.toString("utf8");
  for (const [label, pattern] of patterns) {
    pattern.lastIndex = 0;
    for (const match of contents.matchAll(pattern)) {
      const line = contents.slice(0, match.index).split("\n").length;
      findings.push(`${normalized}:${line}: possible ${label}`);
    }
  }
}

if (findings.length > 0) {
  console.error(`secret-scan: ${findings.length} possible secret(s) found:`);
  for (const finding of findings) {
    console.error(`  ${finding}`);
  }
  process.exit(1);
}

console.log("secret-scan: no high-confidence secrets found.");
