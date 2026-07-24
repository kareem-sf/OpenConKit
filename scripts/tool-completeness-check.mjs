#!/usr/bin/env node
// Release-readiness gate for every compiled-in OpenConKit tool.
//
// This deliberately goes beyond SCAFFOLD comments: a hand-written
// ToolError::NotReady, missing schemas/test hooks, or missing route/docs must
// never be reported as a complete production tool.

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const CRATES_DIR = join(REPO_ROOT, "crates");

/**
 * Recursively collect files under `dir` matching `extensions`.
 * @param {string} dir
 * @param {string[]} extensions
 * @returns {string[]}
 */
function collectFiles(dir, extensions) {
  if (!existsSync(dir)) {
    return [];
  }
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectFiles(path, extensions));
    } else if (extensions.some((extension) => entry.name.endsWith(extension))) {
      files.push(path);
    }
  }
  return files;
}

/** @param {string} path */
function relative(path) {
  return path.replace(REPO_ROOT, "").replaceAll("\\", "/").replace(/^\//, "");
}

/**
 * @param {string} file
 * @param {string} needle
 * @param {string} message
 * @returns {string[]}
 */
function lineFindings(file, needle, message) {
  const findings = [];
  readFileSync(file, "utf8")
    .split("\n")
    .forEach((line, index) => {
      if (line.includes(needle)) {
        findings.push(`${relative(file)}:${index + 1}: ${message}`);
      }
    });
  return findings;
}

/** @param {string} slug */
function pascalCase(slug) {
  return slug
    .split("-")
    .map((word) => `${word[0].toUpperCase()}${word.slice(1)}`)
    .join("");
}

const toolDirectories = readdirSync(CRATES_DIR, { withFileTypes: true })
  .filter(
    (entry) =>
      entry.isDirectory() &&
      entry.name.startsWith("openconkit-tool-") &&
      entry.name !== "openconkit-tool-sdk",
  )
  .map((entry) => join(CRATES_DIR, entry.name));

const findings = [];
const desktopRegistryPath = join(REPO_ROOT, "crates", "openconkit-desktop", "src", "registry.rs");
const desktopCargoPath = join(REPO_ROOT, "crates", "openconkit-desktop", "Cargo.toml");
const desktopRegistry = existsSync(desktopRegistryPath)
  ? readFileSync(desktopRegistryPath, "utf8")
  : "";
const desktopCargo = existsSync(desktopCargoPath) ? readFileSync(desktopCargoPath, "utf8") : "";

for (const toolDirectory of toolDirectories) {
  const crateName = toolDirectory.split(/[\\/]/).at(-1);
  const slug = crateName.replace("openconkit-tool-", "");
  const snake = slug.replaceAll("-", "_");
  const rustFiles = collectFiles(join(toolDirectory, "src"), [".rs"]);
  const rustSource = rustFiles.map((file) => readFileSync(file, "utf8")).join("\n");

  for (const file of rustFiles) {
    findings.push(...lineFindings(file, "SCAFFOLD:", "scaffold marker remains"));
    findings.push(
      ...lineFindings(file, "ToolError::NotReady", "placeholder engine/capability remains"),
    );
  }

  for (const method of ["input_schema", "settings_schema", "output_schema", "test_hooks"]) {
    if (!rustSource.includes(`fn ${method}(`)) {
      findings.push(`${relative(toolDirectory)}: missing Tool::${method} implementation`);
    }
  }
  if (
    /writes_exports\s*:\s*true/.test(rustSource) &&
    !rustSource.includes("fn export_providers(")
  ) {
    findings.push(
      `${relative(toolDirectory)}: writes_exports is true but no export_providers implementation exists`,
    );
  }
  if (/\bai\s*:\s*true/.test(rustSource) && !rustSource.includes("fn ai_capability(")) {
    findings.push(
      `${relative(toolDirectory)}: ai permission is true but no ai_capability implementation exists`,
    );
  }

  const routePath = join(
    REPO_ROOT,
    "apps",
    "desktop-ui",
    "src",
    "routes",
    `${pascalCase(slug)}Page.tsx`,
  );
  if (!existsSync(routePath)) {
    findings.push(`${relative(routePath)}: required tool route is missing`);
  }
  const docsPath = join(REPO_ROOT, "docs", "tools", `${slug}.md`);
  if (!existsSync(docsPath)) {
    findings.push(`${relative(docsPath)}: required tool documentation is missing`);
  }
  if (!desktopRegistry.includes(`openconkit_tool_${snake}::`)) {
    findings.push(`${relative(desktopRegistryPath)}: ${slug} is not registered`);
  }
  if (!desktopCargo.includes(`${crateName} =`)) {
    findings.push(`${relative(desktopCargoPath)}: ${crateName} dependency is missing`);
  }
}

for (const file of collectFiles(join(REPO_ROOT, "apps", "desktop-ui", "src", "routes"), [
  ".ts",
  ".tsx",
])) {
  findings.push(...lineFindings(file, "SCAFFOLD:", "scaffold marker remains"));
}
for (const file of collectFiles(join(REPO_ROOT, "docs", "tools"), [".md"])) {
  findings.push(...lineFindings(file, "SCAFFOLD:", "scaffold marker remains"));
}
for (const file of collectFiles(join(REPO_ROOT, "packages", "i18n", "src", "locales"), [".json"])) {
  findings.push(...lineFindings(file, "TODO(ar):", "placeholder Arabic translation remains"));
}

if (findings.length > 0) {
  console.error(
    `tool-completeness-check: ${findings.length} release blocker(s) remain across ` +
      `${toolDirectories.length} tool(s):`,
  );
  for (const finding of findings) {
    console.error(`  ${finding}`);
  }
  process.exit(1);
}

console.log(
  `tool-completeness-check: ${toolDirectories.length} tool(s) have complete engines, ` +
    "schemas, test hooks, routes, docs, registrations, and translations.",
);
