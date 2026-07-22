// Shared logic for scripts/version-sync.mjs and scripts/version-check.mjs.
//
// The root VERSION file is the canonical version source. These are the
// targets it propagates to:
//   - Cargo.toml                  ([workspace.package] version -> all crates)
//   - apps/desktop-ui/package.json
//   - packages/<name>/package.json (all packages)
//   - crates/openconkit-desktop/tauri.conf.json

import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

export const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
export const VERSION_FILE = join(REPO_ROOT, "VERSION");

export function readCanonicalVersion() {
  return readFileSync(VERSION_FILE, "utf8").trim();
}

function packageJsonTargets() {
  const targets = [join(REPO_ROOT, "apps", "desktop-ui", "package.json")];
  const packagesDir = join(REPO_ROOT, "packages");
  for (const entry of readdirSync(packagesDir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      targets.push(join(packagesDir, entry.name, "package.json"));
    }
  }
  return targets;
}

function checkJsonVersion(path, expected) {
  const json = JSON.parse(readFileSync(path, "utf8"));
  return json.version === expected ? null : `expected ${expected}, found ${json.version}`;
}

function syncJsonVersion(path, expected) {
  const json = JSON.parse(readFileSync(path, "utf8"));
  if (json.version !== expected) {
    json.version = expected;
    writeFileSync(path, `${JSON.stringify(json, null, 2)}\n`, "utf8");
    return true;
  }
  return false;
}

const CARGO_TOML = join(REPO_ROOT, "Cargo.toml");
const SECTION = "[workspace.package]";
const VERSION_LINE = /^version\s*=\s*"([^"]*)"/;

function workspacePackageVersion(toml) {
  let inSection = false;
  for (const line of toml.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.startsWith("[")) {
      inSection = trimmed === SECTION;
      continue;
    }
    if (inSection) {
      const match = VERSION_LINE.exec(trimmed);
      if (match) {
        return match[1];
      }
    }
  }
  return null;
}

function checkCargoVersion(expected) {
  const found = workspacePackageVersion(readFileSync(CARGO_TOML, "utf8"));
  return found === expected ? null : `expected ${expected}, found ${found}`;
}

function syncCargoVersion(expected) {
  const toml = readFileSync(CARGO_TOML, "utf8");
  let inSection = false;
  let changed = false;
  const lines = toml.split("\n").map((line) => {
    const trimmed = line.trim();
    if (trimmed.startsWith("[")) {
      inSection = trimmed === SECTION;
      return line;
    }
    if (inSection && VERSION_LINE.test(trimmed)) {
      const current = VERSION_LINE.exec(trimmed)[1];
      if (current !== expected) {
        changed = true;
        return line.replace(VERSION_LINE, `version = "${expected}"`);
      }
    }
    return line;
  });
  if (changed) {
    writeFileSync(CARGO_TOML, lines.join("\n"), "utf8");
  }
  return changed;
}

const TAURI_CONF = join(REPO_ROOT, "crates", "openconkit-desktop", "tauri.conf.json");

/**
 * @param {"sync" | "check"} mode
 * @returns {{ errors: string[], changed: string[] }}
 */
export function run(mode) {
  const expected = readCanonicalVersion();
  const errors = [];
  const changed = [];

  const targets = [
    ...packageJsonTargets().map((path) => ({
      path,
      check: () => checkJsonVersion(path, expected),
      sync: () => syncJsonVersion(path, expected),
    })),
    {
      path: CARGO_TOML,
      check: () => checkCargoVersion(expected),
      sync: () => syncCargoVersion(expected),
    },
    {
      path: TAURI_CONF,
      check: () => checkJsonVersion(TAURI_CONF, expected),
      sync: () => syncJsonVersion(TAURI_CONF, expected),
    },
  ];

  for (const target of targets) {
    const relative = target.path.replace(REPO_ROOT, "").replaceAll("\\", "/").replace(/^\//, "");
    if (mode === "check") {
      const problem = target.check();
      if (problem) {
        errors.push(`${relative}: ${problem}`);
      }
    } else if (target.sync()) {
      changed.push(relative);
    }
  }
  return { errors, changed };
}
