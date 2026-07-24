#!/usr/bin/env node
// tool-new: scaffold a new OpenConKit tool.
//
// Usage: pnpm tool:new <slug>     (slug is kebab-case, e.g. "takeoff-assistant")
//
// Generates:
//   crates/openconkit-tool-<slug>/          compiling crate (SDK contract)
//   apps/desktop-ui/src/routes/<Slug>Page.tsx  accessible route stub
//   apps/desktop-ui/src/App.tsx             route registration
//   packages/i18n/src/locales/{en,ar}/common.json  i18n keys (parity-safe)
//   docs/tools/<slug>.md                    tool documentation stub
//   desktop registry registration (or a pending entry in
//   crates/openconkit-desktop/TOOL-REGISTRATIONS.md while the composition
//   root does not exist yet)
//
// Every generated placeholder carries a `SCAFFOLD:` marker; the
// `pnpm tool:completeness` gate lists them before release.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const KEBAB_CASE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

function fail(message) {
  console.error(`tool-new: error: ${message}`);
  process.exit(1);
}

const args = process.argv.slice(2);

if (args.includes("--help") || args.includes("-h")) {
  console.log(`Usage: node scripts/tool-new.mjs <slug>

  <slug>   kebab-case tool identifier, e.g. "takeoff-assistant"

Scaffolds the tool crate, route stub, i18n keys, docs stub and registry
registration. See docs/tool-authoring.md.`);
  process.exit(0);
}

if (args.length !== 1) {
  fail(`expected exactly one argument (the tool slug), got ${args.length}. See --help.`);
}

const [slug] = args;
if (!KEBAB_CASE.test(slug)) {
  fail(`invalid tool slug "${slug}": must be kebab-case (e.g. "takeoff-assistant").`);
}
if (slug === "sdk") {
  fail(`"sdk" is reserved: openconkit-tool-sdk is the contract crate, not a tool.`);
}

const words = slug.split("-");
const pascal = words.map((word) => word[0].toUpperCase() + word.slice(1)).join("");
const camel =
  words[0] +
  words
    .slice(1)
    .map((word) => word[0].toUpperCase() + word.slice(1))
    .join("");
const snake = slug.replaceAll("-", "_");
const title = words.map((word) => word[0].toUpperCase() + word.slice(1)).join(" ");
const crateName = `openconkit-tool-${slug}`;

const paths = {
  crateDir: join(REPO_ROOT, "crates", crateName),
  crateToml: join(REPO_ROOT, "crates", crateName, "Cargo.toml"),
  crateLib: join(REPO_ROOT, "crates", crateName, "src", "lib.rs"),
  routePage: join(REPO_ROOT, "apps", "desktop-ui", "src", "routes", `${pascal}Page.tsx`),
  appTsx: join(REPO_ROOT, "apps", "desktop-ui", "src", "App.tsx"),
  localeEn: join(REPO_ROOT, "packages", "i18n", "src", "locales", "en", "common.json"),
  localeAr: join(REPO_ROOT, "packages", "i18n", "src", "locales", "ar", "common.json"),
  docsStub: join(REPO_ROOT, "docs", "tools", `${slug}.md`),
  desktopDir: join(REPO_ROOT, "crates", "openconkit-desktop"),
  desktopToml: join(REPO_ROOT, "crates", "openconkit-desktop", "Cargo.toml"),
  registrationsMd: join(REPO_ROOT, "crates", "openconkit-desktop", "TOOL-REGISTRATIONS.md"),
};

const created = [];
const modified = [];

// ---------------------------------------------------------------------------
// Up-front validation: fail before writing anything.
// ---------------------------------------------------------------------------

if (existsSync(paths.crateDir)) {
  fail(`crate directory crates/${crateName} already exists.`);
}
for (const target of [paths.routePage, paths.docsStub]) {
  if (existsSync(target)) {
    fail(`target already exists: ${target.replace(REPO_ROOT, "").replaceAll("\\", "/")}`);
  }
}

const APP_IMPORT_ANCHOR = `import { HomePage } from "./routes/HomePage";`;
const APP_ROUTE_ANCHOR = `<Route path="/" element={<HomePage />} />`;
const appTsx = readFileSync(paths.appTsx, "utf8");
if (!appTsx.includes(APP_IMPORT_ANCHOR) || !appTsx.includes(APP_ROUTE_ANCHOR)) {
  fail(
    `apps/desktop-ui/src/App.tsx does not contain the expected router anchors ` +
      `(HomePage import and "/" route). Update the scaffolder or register the route by hand.`,
  );
}

const localeEn = JSON.parse(readFileSync(paths.localeEn, "utf8"));
const localeAr = JSON.parse(readFileSync(paths.localeAr, "utf8"));
for (const [label, locale] of [
  ["en", localeEn],
  ["ar", localeAr],
]) {
  if (typeof locale.tools !== "object" || locale.tools === null) {
    fail(`packages/i18n locale ${label}: missing top-level "tools" object.`);
  }
  if (locale.tools[camel] !== undefined) {
    fail(`packages/i18n locale ${label}: key tools.${camel} already exists.`);
  }
}

// ---------------------------------------------------------------------------
// a. Tool crate
// ---------------------------------------------------------------------------

const crateToml = `[package]
name = "${crateName}"
description = "OpenConKit ${title} tool."
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[lints.rust]
unsafe_code = "forbid"

[dependencies]
openconkit-tool-sdk.workspace = true
openconkit-domain.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
`;

const crateLib = `//! ${title}: OpenConKit tool scaffolded by \`pnpm tool:new ${slug}\`.
//!
//! Replace the SCAFFOLD-marked sections with the real implementation;
//! see docs/tool-authoring.md and docs/tools/${slug}.md.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde::{Deserialize, Serialize};

use openconkit_domain::{Finding, WorkbookDiagnostics};
use openconkit_tool_sdk::{
    CancellationToken, InputCapabilities, ProgressCallback, Tool, ToolEngine, ToolError,
    ToolManifest, ToolPermissions, ToolProgress, ToolRunContext, TypedEngineAdapter,
    TypedToolEngine, TOOL_CONTRACT_VERSION,
};

/// Stable identifier of the ${title} tool.
pub const TOOL_ID: &str = "${slug}";

/// Typed run input for the ${title} engine.
// SCAFFOLD: replace with the tool's real input model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ${pascal}Input {
    /// Source revision being analyzed.
    pub source_revision_id: String,
    /// Rule ids to apply (empty = the tool's default rule set).
    pub rules: Vec<String>,
}

/// Typed run settings for the ${title} engine.
// SCAFFOLD: replace with the tool's real settings model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ${pascal}Settings {
    /// Locale used for user-facing output, e.g. "en" or "ar".
    pub locale: String,
}

/// Typed run output of the ${title} engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ${pascal}Output {
    /// Findings produced by the analysis rules.
    pub findings: Vec<Finding>,
    /// Structural diagnostics captured while reading the workbook.
    pub diagnostics: WorkbookDiagnostics,
}

/// The typed analysis engine.
struct ${pascal}Engine;

impl TypedToolEngine for ${pascal}Engine {
    type Input = ${pascal}Input;
    type Settings = ${pascal}Settings;
    type Output = ${pascal}Output;

    fn run_typed(
        &self,
        _context: &ToolRunContext,
        _input: Self::Input,
        _settings: Self::Settings,
        progress: ProgressCallback<'_>,
        cancel: &CancellationToken,
    ) -> Result<Self::Output, ToolError> {
        // SCAFFOLD: replace the pass-through body with the real engine (Phase 4+).
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        progress(ToolProgress {
            phase_key: "tools.${camel}.progress.scaffold".into(),
            fraction: 1.0,
            detail: None,
        });
        Ok(${pascal}Output {
            findings: Vec::new(),
            diagnostics: WorkbookDiagnostics::default(),
        })
    }
}

/// The ${title} tool hosted by the OpenConKit shell.
pub struct ${pascal}Tool {
    engine: TypedEngineAdapter<${pascal}Engine>,
}

impl ${pascal}Tool {
    /// Create the tool instance.
    pub fn new() -> Self {
        Self {
            engine: TypedEngineAdapter::new(${pascal}Engine),
        }
    }
}

impl Default for ${pascal}Tool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ${pascal}Tool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            id: TOOL_ID.to_string(),
            contract_version: TOOL_CONTRACT_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            name_key: "tools.${camel}.name".to_string(),
            description_key: "tools.${camel}.description".to_string(),
            icon: "tools/${slug}.svg".to_string(),
            route: "/tools/${slug}".to_string(),
        }
    }

    fn input_capabilities(&self) -> InputCapabilities {
        // SCAFFOLD: review the accepted extensions and the size limit.
        InputCapabilities {
            accepted_extensions: vec![".xls".to_string(), ".xlsx".to_string()],
            max_file_size_bytes: 64 * 1024 * 1024,
            accepts_multiple: false,
        }
    }

    fn permissions(&self) -> ToolPermissions {
        // SCAFFOLD: review declared permissions. network/ai must stay false
        // unless the tool ships an explicitly user-invoked AI feature
        // (product invariant: local-first, no telemetry).
        ToolPermissions {
            reads_source_files: true,
            writes_exports: true,
            network: false,
            ai: false,
        }
    }

    fn engine(&self) -> &dyn ToolEngine {
        &self.engine
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    fn sample_context() -> ToolRunContext {
        ToolRunContext {
            run_id: "run-1".to_string(),
            project_id: "project-1".to_string(),
            source_revision_id: "rev-1".to_string(),
            workbook_path: std::path::PathBuf::from("stored/workbook.xlsx"),
            app_version: "0.0.1".to_string(),
        }
    }

    #[test]
    fn manifest_targets_current_contract() {
        let manifest = ${pascal}Tool::new().manifest();
        assert_eq!(manifest.id, TOOL_ID);
        assert_eq!(manifest.contract_version, TOOL_CONTRACT_VERSION);
        assert_eq!(manifest.route, "/tools/${slug}");
        assert_eq!(manifest.name_key, "tools.${camel}.name");
        assert_eq!(manifest.description_key, "tools.${camel}.description");
    }

    #[test]
    fn capabilities_accept_xlsx_case_insensitively() {
        let capabilities = ${pascal}Tool::new().input_capabilities();
        assert!(capabilities.accepts(".XLSX"));
        assert!(capabilities.accepts("xls"));
        assert!(!capabilities.accepts(".csv"));
    }

    #[test]
    fn engine_pass_through_returns_empty_findings_and_default_diagnostics() {
        let tool = ${pascal}Tool::new();
        let output = tool
            .engine()
            .run(
                &sample_context(),
                &json!({ "source_revision_id": "rev-1", "rules": [] }),
                &json!({ "locale": "en" }),
                &|_| {},
                &CancellationToken::new(),
            )
            .expect("run succeeds");
        assert_eq!(output["findings"], json!([]));
        assert_eq!(output["diagnostics"], json!({ "sheets": [], "tables": [] }));
    }

    #[test]
    fn cancellation_yields_cancelled_error() {
        let tool = ${pascal}Tool::new();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = tool
            .engine()
            .run(
                &sample_context(),
                &json!({ "source_revision_id": "rev-1", "rules": [] }),
                &json!({ "locale": "en" }),
                &|_| {},
                &cancel,
            )
            .expect_err("cancelled run fails");
        assert_eq!(err, ToolError::Cancelled);
    }
}
`;

mkdirSync(join(paths.crateDir, "src"), { recursive: true });
writeFileSync(paths.crateToml, crateToml, "utf8");
writeFileSync(paths.crateLib, crateLib, "utf8");
created.push(`crates/${crateName}/Cargo.toml`, `crates/${crateName}/src/lib.rs`);

// ---------------------------------------------------------------------------
// b. Frontend route stub + registration
// ---------------------------------------------------------------------------

// Prettier (printWidth 100) inlines a JSX element with a single expression
// child when it fits, and breaks it otherwise; emit the matching form so the
// generated files are prettier-clean for any slug length.
const PRINT_WIDTH = 100;

function jsxExpressionElement(indent, openTag, expression, tag) {
  const oneLine = `${indent}<${openTag}>{${expression}}</${tag}>`;
  if (oneLine.length <= PRINT_WIDTH) {
    return oneLine;
  }
  return `${indent}<${openTag}>\n${indent}  {${expression}}\n${indent}</${tag}>`;
}

const heading = jsxExpressionElement(
  "      ",
  'h1 className="text-3xl font-semibold text-content-primary"',
  `t("tools.${camel}.name")`,
  "h1",
);
const tagline = jsxExpressionElement(
  "      ",
  'p className="max-w-md text-lg text-content-secondary"',
  `t("tools.${camel}.description")`,
  "p",
);

const routePage = `import { useTranslation } from "react-i18next";

/**
 * ${title} tool route.
 */
export function ${pascal}Page() {
  const { t } = useTranslation();

  // SCAFFOLD: replace with the real tool UI; see docs/tool-authoring.md.
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 bg-surface-base px-8 text-center">
${heading}
${tagline}
    </main>
  );
}
`;

writeFileSync(paths.routePage, routePage, "utf8");
created.push(`apps/desktop-ui/src/routes/${pascal}Page.tsx`);

// Insert the route with the same indentation as the existing "/" route, and
// break the <Route> props the way prettier does when the line overflows.
const routeAnchorLine = appTsx.split("\n").find((line) => line.includes(APP_ROUTE_ANCHOR));
const routeIndent = /^(\s*)/.exec(routeAnchorLine)[1];
const routeOneLine = `${routeIndent}<Route path="/tools/${slug}" element={<${pascal}Page />} />`;
const routeElement =
  routeOneLine.length <= PRINT_WIDTH
    ? routeOneLine
    : `${routeIndent}<Route\n${routeIndent}  path="/tools/${slug}"\n${routeIndent}  element={<${pascal}Page />}\n${routeIndent}/>`;

const nextAppTsx = appTsx
  .replace(
    APP_IMPORT_ANCHOR,
    `${APP_IMPORT_ANCHOR}\nimport { ${pascal}Page } from "./routes/${pascal}Page";`,
  )
  .replace(
    `${routeIndent}${APP_ROUTE_ANCHOR}`,
    `${routeIndent}${APP_ROUTE_ANCHOR}\n${routeIndent}{/* SCAFFOLD: route registered by tool-new */}\n${routeElement}`,
  );
writeFileSync(paths.appTsx, nextAppTsx, "utf8");
modified.push("apps/desktop-ui/src/App.tsx");

// ---------------------------------------------------------------------------
// c. i18n stubs (parsed + re-stringified, never regex-edited)
// ---------------------------------------------------------------------------

const enDescription = `Automated ${title.toLowerCase()} quality review.`;
const enScaffold = "Running the scaffolded pass-through engine.";

localeEn.tools[camel] = {
  name: title,
  description: enDescription,
  progress: { scaffold: enScaffold },
};
localeAr.tools[camel] = {
  name: `TODO(ar): ${title}`,
  description: `TODO(ar): ${enDescription}`,
  progress: { scaffold: `TODO(ar): ${enScaffold}` },
};

writeFileSync(paths.localeEn, `${JSON.stringify(localeEn, null, 2)}\n`, "utf8");
writeFileSync(paths.localeAr, `${JSON.stringify(localeAr, null, 2)}\n`, "utf8");
modified.push(
  "packages/i18n/src/locales/en/common.json",
  "packages/i18n/src/locales/ar/common.json",
);

// ---------------------------------------------------------------------------
// d. Documentation stub
// ---------------------------------------------------------------------------

const docsStub = `# ${title}

## Overview

<!-- SCAFFOLD: one paragraph on what the tool does, for users and reviewers. -->

## Rules

<!-- SCAFFOLD: table of rule ids (kebab-case), severity, and what each rule
     detects. Rules ship with a semver rule_set_version per tool. -->

## Fixtures

<!-- SCAFFOLD: the synthetic fixtures covering this tool (specs in
     fixtures/source-specs/, see fixtures/README.md) and their planted
     defects. -->

## Exports

<!-- SCAFFOLD: export formats the tool produces (xlsx/pdf), their structure,
     and localization notes (en + ar). -->

## AI

<!-- SCAFFOLD: optional AI capability, or "none". AI output is always a
     suggestion grounded in extracted facts, never silently applied. -->
`;

mkdirSync(dirname(paths.docsStub), { recursive: true });
writeFileSync(paths.docsStub, docsStub, "utf8");
created.push(`docs/tools/${slug}.md`);

// ---------------------------------------------------------------------------
// e. Registry registration
// ---------------------------------------------------------------------------

const REGISTER_MARKER = "// tool-new: register here";
const registerSnippet = `registry.register(Box::new(openconkit_tool_${snake}::${pascal}Tool::new()))?;`;
const cargoDepLine = `${crateName} = { path = "../${crateName}" }`;

// Candidate composition modules inside the desktop host.
const compositionCandidates = ["registry.rs", "composition.rs", "compose.rs", "tools.rs"].map(
  (file) => join(paths.desktopDir, "src", file),
);

let registeredInCode = false;
for (const candidate of compositionCandidates) {
  if (!existsSync(candidate)) {
    continue;
  }
  const source = readFileSync(candidate, "utf8");
  if (!source.includes(REGISTER_MARKER)) {
    continue;
  }
  const nextSource = source.replace(REGISTER_MARKER, `${REGISTER_MARKER}\n    ${registerSnippet}`);
  writeFileSync(candidate, nextSource, "utf8");
  modified.push(candidate.replace(REPO_ROOT, "").replaceAll("\\", "/").replace(/^\//, ""));

  // Add the crate to the desktop host's [dependencies].
  const desktopToml = readFileSync(paths.desktopToml, "utf8");
  if (!desktopToml.includes(`${crateName} `)) {
    const lines = desktopToml.split("\n");
    const depStart = lines.findIndex((line) => line.trim() === "[dependencies]");
    if (depStart === -1) {
      fail(`crates/openconkit-desktop/Cargo.toml has no [dependencies] section.`);
    }
    let insertAt = lines.length;
    for (let index = depStart + 1; index < lines.length; index += 1) {
      if (lines[index].trim().startsWith("[")) {
        insertAt = index;
        break;
      }
    }
    lines.splice(insertAt, 0, cargoDepLine);
    writeFileSync(paths.desktopToml, lines.join("\n"), "utf8");
    modified.push("crates/openconkit-desktop/Cargo.toml");
  }
  registeredInCode = true;
  break;
}

if (!registeredInCode) {
  const relative = (path) => path.replace(REPO_ROOT, "").replaceAll("\\", "/").replace(/^\//, "");
  const header = `# Pending tool registrations

The desktop composition root does not exist yet, so \`pnpm tool:new\` records
registrations here instead of editing code. When the composition module lands
(with a \`// tool-new: register here\` marker), move each entry below into it,
add the Cargo.toml dependency, and delete the entry from this file.
`;
  const entry = `
## ${slug}

- Dependency, add to \`crates/openconkit-desktop/Cargo.toml\` under \`[dependencies]\`:
  \`${cargoDepLine}\`
- Registration, add at the \`// tool-new: register here\` marker:
  \`${registerSnippet}\`
`;
  const existed = existsSync(paths.registrationsMd);
  const previous = existed ? readFileSync(paths.registrationsMd, "utf8") : header;
  writeFileSync(paths.registrationsMd, `${previous.trimEnd()}\n${entry}`, "utf8");
  (existed ? modified : created).push(relative(paths.registrationsMd));
  console.log(
    `tool-new: note: no desktop composition module with a "${REGISTER_MARKER}" marker ` +
      `exists yet; recorded the registration in ${relative(paths.registrationsMd)} instead.`,
  );
}

// ---------------------------------------------------------------------------
// f. Summary
// ---------------------------------------------------------------------------

console.log(`\ntool-new: scaffolded "${slug}" (${crateName})`);
console.log("  created:");
for (const path of created) {
  console.log(`    + ${path}`);
}
if (modified.length > 0) {
  console.log("  modified:");
  for (const path of modified) {
    console.log(`    ~ ${path}`);
  }
}
console.log(`
Next steps:
  1. cargo test -p ${crateName}
  2. Replace the SCAFFOLD-marked sections (see docs/tool-authoring.md).
  3. Translate the TODO(ar) values in packages/i18n/src/locales/ar/common.json.
  4. \`pnpm tool:completeness\` must be clean before release.`);
