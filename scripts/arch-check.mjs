#!/usr/bin/env node
// arch-check: enforce the layered architecture rules from AGENTS.md and
// docs/architecture.md across the Rust workspace.
//
// Dependency direction is strictly one-way:
//
//   desktop (Tauri host) -> tool crates -> application -> domain
//                                      \-> infrastructure (storage, spreadsheet, ...)
//   tool-sdk <- tool crates, desktop
//
// The script parses each crates/*\/Cargo.toml with a minimal line-based TOML
// reader (no npm dependencies), classifies every crate, checks the rules for
// its layer, and finally runs a cycle detection over the internal dependency
// graph.
//
// Usage: node scripts/arch-check.mjs   (wired into `pnpm lint`)

import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const CRATES_DIR = join(REPO_ROOT, "crates");

const INTERNAL_PREFIX = "openconkit-";
const TOOL_PREFIX = "openconkit-tool-";
const SDK_CRATE = "openconkit-tool-sdk";
const DOMAIN_CRATE = "openconkit-domain";
const APPLICATION_CRATE = "openconkit-application";
const DESKTOP_CRATE = "openconkit-desktop";
// Build/dev tooling binaries that sit outside the runtime layer graph.
// The contracts exporter may inspect compiled tool DTOs as a build-time-only
// edge (ADR 0011); this does not permit runtime infrastructure to depend on
// individual tools.
const TOOLING_CRATES = new Set(["openconkit-contracts-export"]);

// Crates a pure domain layer must never touch (infra + UI host).
const DOMAIN_FORBIDDEN = ["tauri", "rusqlite", "calamine", "rust_xlsxwriter", "typst"];

// Dependency sections of a Cargo.toml that carry crate dependencies.
const DEP_SECTIONS = new Set(["dependencies", "dev-dependencies", "build-dependencies"]);

const DEP_KEY = /^([A-Za-z0-9_-]+)\s*(?:\.[A-Za-z0-9_-]+)?\s*=/;

/**
 * Minimal TOML reader: extract the package name and the set of dependency
 * names declared in [dependencies] / [dev-dependencies] / [build-dependencies].
 * Dependency names appear either as `name = ...` or `name.workspace = true`.
 *
 * @param {string} toml raw Cargo.toml content
 * @returns {{ name: string | null, dependencies: string[] }}
 */
function parseCargoToml(toml) {
  let name = null;
  const dependencies = [];
  let section = null;
  for (const line of toml.split("\n")) {
    const trimmed = line.trim();
    if (trimmed === "" || trimmed.startsWith("#")) {
      continue;
    }
    const header = /^\[([^\]]+)\]$/.exec(trimmed);
    if (header) {
      section = header[1];
      continue;
    }
    if (section === "package") {
      const match = /^name\s*=\s*"([^"]+)"/.exec(trimmed);
      if (match) {
        name = match[1];
      }
      continue;
    }
    if (section !== null && DEP_SECTIONS.has(section)) {
      const match = DEP_KEY.exec(trimmed);
      if (match) {
        dependencies.push(match[1]);
      }
    }
  }
  return { name, dependencies };
}

/**
 * @param {string} name crate name
 * @returns {"domain" | "application" | "sdk" | "desktop" | "tool" | "tooling" | "infra"}
 */
function classify(name) {
  if (name === DOMAIN_CRATE) {
    return "domain";
  }
  if (name === APPLICATION_CRATE) {
    return "application";
  }
  if (name === SDK_CRATE) {
    return "sdk";
  }
  if (name === DESKTOP_CRATE) {
    return "desktop";
  }
  if (TOOLING_CRATES.has(name)) {
    return "tooling";
  }
  if (name.startsWith(TOOL_PREFIX)) {
    return "tool";
  }
  return "infra";
}

// Internal crates a tooling binary may depend on.
const TOOLING_ALLOWED = new Set([SDK_CRATE, DOMAIN_CRATE, APPLICATION_CRATE]);

// Internal crates a tool crate may depend on.
const TOOL_ALLOWED = new Set([
  SDK_CRATE,
  DOMAIN_CRATE,
  APPLICATION_CRATE,
  "openconkit-storage",
  "openconkit-spreadsheet",
  "openconkit-reporting",
  "openconkit-ai-codex",
]);

/**
 * Collect every crates/*\/Cargo.toml in the workspace.
 * @returns {{ path: string, relative: string, name: string, dependencies: string[] }[]}
 */
function readWorkspaceCrates() {
  const crates = [];
  for (const entry of readdirSync(CRATES_DIR, { withFileTypes: true })) {
    if (!entry.isDirectory()) {
      continue;
    }
    const path = join(CRATES_DIR, entry.name, "Cargo.toml");
    let toml;
    try {
      toml = readFileSync(path, "utf8");
    } catch {
      continue; // directory without a Cargo.toml
    }
    const { name, dependencies } = parseCargoToml(toml);
    if (name === null) {
      continue;
    }
    crates.push({
      path,
      relative: `crates/${entry.name}/Cargo.toml`,
      name,
      dependencies,
    });
  }
  return crates;
}

/** @returns {string[]} human-readable violations */
function checkLayerRules(crates) {
  const violations = [];
  for (const crate of crates) {
    const internal = crate.dependencies.filter((dep) => dep.startsWith(INTERNAL_PREFIX));
    const hasTauri = crate.dependencies.some(
      (dep) => dep === "tauri" || dep === "tauri-build" || dep.startsWith("tauri-plugin-"),
    );
    const layer = classify(crate.name);
    const at = `${crate.relative} (${crate.name})`;

    if (hasTauri && crate.name !== DESKTOP_CRATE) {
      violations.push(
        `${at}: depends on tauri/tauri-plugin-* - only ${DESKTOP_CRATE} may depend on Tauri. ` +
          `Move the host integration into the desktop crate.`,
      );
    }

    switch (layer) {
      case "domain": {
        if (internal.length > 0) {
          violations.push(
            `${at}: openconkit-domain must not depend on other workspace crates ` +
              `(found: ${internal.join(", ")}). Keep the domain pure.`,
          );
        }
        const forbidden = crate.dependencies.filter((dep) => DOMAIN_FORBIDDEN.includes(dep));
        if (forbidden.length > 0) {
          violations.push(
            `${at}: openconkit-domain must not depend on infrastructure/UI libraries ` +
              `(found: ${forbidden.join(", ")}). Domain code has no IO.`,
          );
        }
        break;
      }
      case "application": {
        const extra = internal.filter((dep) => dep !== DOMAIN_CRATE);
        if (extra.length > 0) {
          violations.push(
            `${at}: openconkit-application may only depend on ${DOMAIN_CRATE} internally ` +
              `(found: ${extra.join(", ")}). Infrastructure is injected via ports (traits).`,
          );
        }
        break;
      }
      case "sdk": {
        const extra = internal.filter((dep) => dep !== DOMAIN_CRATE);
        if (extra.length > 0) {
          violations.push(
            `${at}: openconkit-tool-sdk may only depend on ${DOMAIN_CRATE} internally ` +
              `(found: ${extra.join(", ")}).`,
          );
        }
        break;
      }
      case "tool": {
        const forbidden = internal.filter((dep) => !TOOL_ALLOWED.has(dep));
        if (forbidden.length > 0) {
          violations.push(
            `${at}: tool crates may only depend on ${[...TOOL_ALLOWED].join(", ")} ` +
              `(found: ${forbidden.join(", ")}). Never depend on another openconkit-tool-* ` +
              `crate or on the desktop host.`,
          );
        }
        break;
      }
      case "infra": {
        // tool-sdk is the contract crate (openconkit-tool-sdk), not a hosted
        // tool; only openconkit-tool-<slug> crates and the desktop host are
        // forbidden for infrastructure.
        const forbidden = internal.filter(
          (dep) => dep === DESKTOP_CRATE || (dep.startsWith(TOOL_PREFIX) && dep !== SDK_CRATE),
        );
        if (forbidden.length > 0) {
          violations.push(
            `${at}: infrastructure crates must not depend on tool crates or the desktop host ` +
              `(found: ${forbidden.join(", ")}). Use cases import infrastructure, not the reverse.`,
          );
        }
        break;
      }
      case "tooling": {
        const forbidden = internal.filter(
          (dep) =>
            !TOOLING_ALLOWED.has(dep) &&
            !(
              crate.name === "openconkit-contracts-export" &&
              dep.startsWith(TOOL_PREFIX) &&
              dep !== SDK_CRATE
            ),
        );
        if (forbidden.length > 0) {
          violations.push(
            `${at}: tooling binaries may only depend on ${[...TOOLING_ALLOWED].join(", ")}; ` +
              `the contracts exporter may additionally inspect compiled tool DTOs per ADR 0011 ` +
              `(found: ${forbidden.join(", ")}).`,
          );
        }
        break;
      }
      case "desktop":
        break; // the host composes everything
    }
  }
  return violations;
}

/**
 * Depth-first cycle detection over the internal dependency graph.
 * @returns {string[]} one message per cycle found
 */
function checkCycles(crates) {
  const graph = new Map();
  for (const crate of crates) {
    graph.set(
      crate.name,
      crate.dependencies.filter((dep) => dep.startsWith(INTERNAL_PREFIX)),
    );
  }

  const violations = [];
  const state = new Map(); // name -> "visiting" | "done"
  const stack = [];

  function visit(name) {
    state.set(name, "visiting");
    stack.push(name);
    for (const dep of graph.get(name) ?? []) {
      if (!graph.has(dep)) {
        continue; // dep declared but crate not present in the workspace
      }
      if (state.get(dep) === "visiting") {
        const cycle = [...stack.slice(stack.indexOf(dep)), dep];
        violations.push(
          `circular dependency detected: ${cycle.join(" -> ")}. ` +
            `Break the cycle (dependency direction must stay one-way; see docs/architecture.md).`,
        );
      } else if (state.get(dep) === undefined) {
        visit(dep);
      }
    }
    stack.pop();
    state.set(name, "done");
  }

  for (const crate of crates) {
    if (state.get(crate.name) === undefined) {
      visit(crate.name);
    }
  }
  return violations;
}

const crates = readWorkspaceCrates();
const violations = [...checkLayerRules(crates), ...checkCycles(crates)];

if (violations.length > 0) {
  console.error(`arch-check: ${violations.length} architecture violation(s):`);
  for (const violation of violations) {
    console.error(`  - ${violation}`);
  }
  process.exit(1);
}

console.log(`arch-check: ${crates.length} workspace crates conform to the layered architecture.`);
