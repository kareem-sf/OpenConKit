#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readdir, readFile, stat, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const outputPath = join(repositoryRoot, "THIRD_PARTY_NOTICES.md");
const desktopPackageName = "openconkit-desktop";
const maxLegalFileBytes = 2 * 1024 * 1024;
const legalFilePattern = /^(?:licen[cs]e|copying|copyright|notice)(?:[._-].*)?$/iu;

function fail(message) {
  throw new Error(`generate-notices: ${message}`);
}

function run(command, args) {
  const executable =
    process.platform === "win32" && command === "pnpm"
      ? (process.env.ComSpec ?? "cmd.exe")
      : command;
  const commandArgs =
    process.platform === "win32" && command === "pnpm"
      ? ["/d", "/s", "/c", ["pnpm", ...args].join(" ")]
      : args;
  const result = spawnSync(executable, commandArgs, {
    cwd: repositoryRoot,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    process.stderr.write(result.stderr ?? "");
    fail(
      `${command} failed${
        result.error ? `: ${result.error.message}` : ` with exit code ${String(result.status)}`
      }`,
    );
  }
  return result.stdout;
}

function parseJson(text, label) {
  try {
    return JSON.parse(text);
  } catch (error) {
    fail(`${label} returned invalid JSON: ${String(error)}`);
  }
}

function normalizeLegalText(text) {
  return text
    .replace(/^\uFEFF/u, "")
    .replace(/\r\n?/gu, "\n")
    .replace(/[ \t]+$/gmu, "")
    .trimEnd();
}

function contentHash(content) {
  return createHash("sha256").update(content, "utf8").digest("hex");
}

function htmlEscape(text) {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

function displayUrl(dependency) {
  return dependency.repository ?? dependency.homepage ?? dependency.source ?? "not declared";
}

function rustRuntimeDependencies(metadata) {
  const packagesById = new Map(metadata.packages.map((dependency) => [dependency.id, dependency]));
  const nodesById = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const desktop = metadata.packages.find(
    (dependency) => dependency.name === desktopPackageName && dependency.source === null,
  );
  if (!desktop) {
    fail(`Cargo metadata does not contain the ${desktopPackageName} workspace package`);
  }

  const visited = new Set();
  const queue = [desktop.id];
  while (queue.length > 0) {
    const packageId = queue.pop();
    if (!packageId || visited.has(packageId)) {
      continue;
    }
    visited.add(packageId);
    const node = nodesById.get(packageId);
    if (!node) {
      fail(`Cargo metadata is missing a resolve node for ${packageId}`);
    }
    for (const dependency of node.deps) {
      const isRuntimeDependency = dependency.dep_kinds.some(({ kind }) => kind === null);
      if (isRuntimeDependency) {
        queue.push(dependency.pkg);
      }
    }
  }

  return [...visited]
    .map((packageId) => packagesById.get(packageId))
    .filter((dependency) => dependency && dependency.source !== null)
    .sort(
      (left, right) =>
        left.name.localeCompare(right.name, "en") ||
        left.version.localeCompare(right.version, "en"),
    );
}

async function legalFiles(packageRoot, explicitLicenseFile) {
  const entries = await readdir(packageRoot, { withFileTypes: true });
  const selected = new Set(
    entries
      .filter((entry) => entry.isFile() && legalFilePattern.test(entry.name))
      .map((entry) => entry.name),
  );
  if (explicitLicenseFile) {
    selected.add(
      isAbsolute(explicitLicenseFile)
        ? relative(packageRoot, explicitLicenseFile)
        : explicitLicenseFile,
    );
  }

  const files = [];
  for (const relativePath of [...selected].sort((left, right) => left.localeCompare(right, "en"))) {
    const path = resolve(packageRoot, relativePath);
    const confinedPath = relative(resolve(packageRoot), path);
    if (
      isAbsolute(confinedPath) ||
      confinedPath === ".." ||
      confinedPath.startsWith("../") ||
      confinedPath.startsWith("..\\")
    ) {
      fail(`legal file escapes its package directory: ${path}`);
    }
    let metadata;
    try {
      metadata = await stat(path);
    } catch {
      fail(`declared legal file is missing: ${path}`);
    }
    if (!metadata.isFile()) {
      continue;
    }
    if (metadata.size > maxLegalFileBytes) {
      fail(`legal file exceeds ${String(maxLegalFileBytes)} bytes: ${path}`);
    }
    const content = normalizeLegalText(await readFile(path, "utf8"));
    if (content.length > 0) {
      files.push({ name: confinedPath.replaceAll("\\", "/"), content });
    }
  }
  return files;
}

function addMaterial(materials, dependencyLabel, file) {
  const hash = contentHash(file.content);
  const existing = materials.get(hash) ?? {
    content: file.content,
    names: new Set(),
    users: new Set(),
  };
  existing.names.add(file.name);
  existing.users.add(dependencyLabel);
  materials.set(hash, existing);
  return hash;
}

async function rustInventory(metadata, materials) {
  const inventory = [];
  for (const dependency of rustRuntimeDependencies(metadata)) {
    const label = `${dependency.name}@${dependency.version}`;
    const packageRoot = dirname(dependency.manifest_path);
    const files = await legalFiles(packageRoot, dependency.license_file);
    if (!dependency.license && files.length === 0) {
      fail(`${label} declares neither a license expression nor a legal file`);
    }
    inventory.push({
      label,
      license: dependency.license ?? `see ${dependency.license_file}`,
      url: displayUrl(dependency),
      materialHashes: files.map((file) => addMaterial(materials, label, file)),
    });
  }
  return inventory;
}

async function npmInventory(inventoryByLicense, materials) {
  const inventory = [];
  for (const [license, dependencies] of Object.entries(inventoryByLicense)) {
    for (const dependency of dependencies) {
      const versions = [...dependency.versions].sort((left, right) =>
        left.localeCompare(right, "en"),
      );
      const label = `${dependency.name}@${versions.join(",")}`;
      const files = [];
      for (const packagePath of [...dependency.paths].sort((left, right) =>
        left.localeCompare(right, "en"),
      )) {
        files.push(...(await legalFiles(packagePath)));
      }
      inventory.push({
        label,
        license,
        url: displayUrl(dependency),
        materialHashes: [...new Set(files.map((file) => addMaterial(materials, label, file)))],
      });
    }
  }
  return inventory.sort((left, right) => left.label.localeCompare(right.label, "en"));
}

async function codexInventory(materials) {
  const manifest = parseJson(
    await readFile(join(repositoryRoot, "tools", "codex-version.json"), "utf8"),
    "tools/codex-version.json",
  );
  const label = `OpenAI Codex app-server@${manifest.version}`;
  const resourceRoot = join(repositoryRoot, "crates", "openconkit-desktop", "resources", "codex");
  const files = await legalFiles(resourceRoot);
  if (files.length < 2) {
    fail("the staged Codex runtime must include both upstream license and notice files");
  }
  return {
    label,
    license: "Apache-2.0",
    url: `https://github.com/openai/codex/releases/tag/${manifest.releaseTag}`,
    materialHashes: files.map((file) => addMaterial(materials, label, file)),
    targets: Object.entries(manifest.targets)
      .sort(([left], [right]) => left.localeCompare(right, "en"))
      .map(([target, details]) => ({ target, sha256: details.sha256 })),
  };
}

function assignMaterialIds(materials) {
  const ordered = [...materials.entries()].sort(
    ([leftHash, left], [rightHash, right]) =>
      [...left.names].sort()[0].localeCompare([...right.names].sort()[0], "en") ||
      leftHash.localeCompare(rightHash, "en"),
  );
  return new Map(
    ordered.map(([hash, material], index) => [
      hash,
      {
        ...material,
        hash,
        id: `M-${String(index + 1).padStart(3, "0")}`,
      },
    ]),
  );
}

function dependencyLine(dependency, materialIds) {
  const references = [...new Set(dependency.materialHashes)]
    .map((hash) => materialIds.get(hash)?.id)
    .filter(Boolean)
    .sort();
  return `- \`${dependency.label}\` — license: \`${dependency.license}\`; source: ${dependency.url}; legal materials: ${
    references.length > 0 ? references.join(", ") : "license expression only"
  }`;
}

function render({ rust, npm, codex, materialIds }) {
  const lines = [
    "# Third-Party Notices",
    "",
    "This file is generated deterministically from the locked production dependency graphs by",
    "`pnpm notices:generate`. `pnpm notices:check` fails when the committed inventory drifts.",
    "OpenConKit's own source is licensed separately under Apache-2.0 in `LICENSE`.",
    "",
    "The inventory is intentionally conservative: it includes normal Rust dependencies reachable",
    "from the desktop host across supported targets and all packages reported by pnpm's production",
    "license inventory. Build-only and test-only Rust dependencies are excluded.",
    "",
    "## Bundled runtime components",
    "",
    dependencyLine(codex, materialIds),
    "",
    "The Codex binary is fetched only from its pinned upstream release. The official archive",
    "SHA-256 values accepted by the build are:",
    "",
    ...codex.targets.map(({ target, sha256 }) => `- \`${target}\`: \`${sha256}\``),
    "",
    "The `typst-assets` dependency embeds the fonts used for English and Arabic PDF output.",
    "Its reproduced NOTICE material includes the applicable Libertinus, New Computer Modern,",
    "DejaVu, Foxit, and related font terms. The bundled SQLite amalgamation is public-domain",
    "software; Rust wrapper crates retain the licenses listed below.",
    "",
    `## Rust runtime dependencies (${String(rust.length)})`,
    "",
    ...rust.map((dependency) => dependencyLine(dependency, materialIds)),
    "",
    `## Webview production dependencies (${String(npm.length)})`,
    "",
    ...npm.map((dependency) => dependencyLine(dependency, materialIds)),
    "",
    `## Reproduced license and notice materials (${String(materialIds.size)})`,
    "",
    "Identical texts are deduplicated. Each material lists every dependency that supplied that",
    "exact content; SHA-256 makes the reproduction independently checkable.",
    "",
  ];

  for (const material of [...materialIds.values()].sort((left, right) =>
    left.id.localeCompare(right.id, "en"),
  )) {
    lines.push(
      `### ${material.id}`,
      "",
      `- SHA-256: \`${material.hash}\``,
      `- Source filename(s): ${[...material.names]
        .sort()
        .map((name) => `\`${name}\``)
        .join(", ")}`,
      `- Used by: ${[...material.users]
        .sort()
        .map((user) => `\`${user}\``)
        .join(", ")}`,
      "",
      "<pre>",
      htmlEscape(material.content),
      "</pre>",
      "",
    );
  }

  return `${lines.join("\n").trimEnd()}\n`;
}

async function generate() {
  const cargoMetadata = parseJson(
    run("cargo", ["metadata", "--format-version", "1", "--locked"]),
    "cargo metadata",
  );
  const pnpmLicenses = parseJson(
    run("pnpm", ["licenses", "list", "--prod", "--json"]),
    "pnpm licenses",
  );
  const materials = new Map();
  const rust = await rustInventory(cargoMetadata, materials);
  const npm = await npmInventory(pnpmLicenses, materials);
  const codex = await codexInventory(materials);
  const materialIds = assignMaterialIds(materials);
  return render({ rust, npm, codex, materialIds });
}

async function main() {
  const args = process.argv.slice(2);
  if (args.some((argument) => argument !== "--check") || args.length > 1) {
    fail("usage: node scripts/generate-notices.mjs [--check]");
  }
  const expected = await generate();
  if (args[0] === "--check") {
    let actual;
    try {
      actual = await readFile(outputPath, "utf8");
    } catch {
      fail("THIRD_PARTY_NOTICES.md is missing; run pnpm notices:generate");
    }
    if (actual !== expected) {
      fail("THIRD_PARTY_NOTICES.md is stale; run pnpm notices:generate");
    }
    process.stdout.write("generate-notices: committed third-party inventory is current.\n");
    return;
  }
  await writeFile(outputPath, expected, "utf8");
  process.stdout.write(
    `generate-notices: wrote THIRD_PARTY_NOTICES.md (${String(Buffer.byteLength(expected))} bytes).\n`,
  );
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
