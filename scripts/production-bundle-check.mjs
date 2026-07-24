#!/usr/bin/env node

import { readdir, readFile, stat } from "node:fs/promises";
import { extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const distributionRoot = join(repositoryRoot, "apps", "desktop-ui", "dist");
const forbiddenText = [
  ["E2E query marker", "openconkit-e2e"],
  ["E2E readiness event", "openconkit:e2e"],
  ["development fixture loader", "previewData"],
  ["source map reference", "sourceMappingURL"],
];

function fail(message) {
  throw new Error(`production-bundle-check: ${message}`);
}

async function listFiles(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFiles(path)));
    } else if (entry.isFile()) {
      files.push(path);
    } else {
      fail(`distribution contains a non-regular entry: ${path}`);
    }
  }
  return files;
}

async function main() {
  let metadata;
  try {
    metadata = await stat(distributionRoot);
  } catch {
    fail(`production distribution is missing: ${distributionRoot}`);
  }
  if (!metadata.isDirectory()) {
    fail(`production distribution is not a directory: ${distributionRoot}`);
  }

  const files = (await listFiles(distributionRoot)).sort();
  if (files.length === 0) {
    fail("production distribution is empty");
  }
  if (!files.includes(join(distributionRoot, "index.html"))) {
    fail("production distribution has no index.html");
  }

  for (const path of files) {
    const name = relative(distributionRoot, path).replaceAll("\\", "/");
    if (extname(path).toLowerCase() === ".map") {
      fail(`source map would be shipped: ${name}`);
    }
    if (!/\.(?:css|html|js)$/u.test(path)) {
      continue;
    }
    const contents = await readFile(path, "utf8");
    for (const [label, marker] of forbiddenText) {
      if (contents.includes(marker)) {
        fail(`${label} remains in ${name}`);
      }
    }
  }

  process.stdout.write(
    `production-bundle-check: ${files.length} release file(s) contain no source maps or development controls.\n`,
  );
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
