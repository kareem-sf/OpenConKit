#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  chmod,
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  stat,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { Readable, Transform } from "node:stream";
import { pipeline } from "node:stream/promises";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createWriteStream } from "node:fs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = join(repositoryRoot, "tools", "codex-version.json");
const defaultStageDirectory = join(repositoryRoot, "crates", "openconkit-desktop", "binaries");
const defaultResourceDirectory = join(
  repositoryRoot,
  "crates",
  "openconkit-desktop",
  "resources",
  "codex",
);
const releaseOrigin = "https://github.com/openai/codex/releases/download";
const sourceOrigin = "https://raw.githubusercontent.com/openai/codex";
const universalTarget = "universal-apple-darwin";
const universalSourceTargets = ["x86_64-apple-darwin", "aarch64-apple-darwin"];

function fail(message) {
  throw new Error(`fetch-codex: ${message}`);
}

function parseArguments(argv) {
  const options = {
    target: detectTarget(),
    stageDirectory: defaultStageDirectory,
    resourceDirectory: defaultResourceDirectory,
    resourcesOnly: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (argument === "--target" && value) {
      options.target = value;
      index += 1;
    } else if (argument === "--stage-dir" && value) {
      options.stageDirectory = resolve(value);
      index += 1;
    } else if (argument === "--resource-dir" && value) {
      options.resourceDirectory = resolve(value);
      index += 1;
    } else if (argument === "--resources-only") {
      options.resourcesOnly = true;
    } else {
      fail(`unsupported argument: ${argument ?? ""}`);
    }
  }
  return options;
}

function detectTarget() {
  const key = `${process.platform}/${process.arch}`;
  const targets = {
    "win32/x64": "x86_64-pc-windows-msvc",
    "linux/x64": "x86_64-unknown-linux-gnu",
    "darwin/x64": "x86_64-apple-darwin",
    "darwin/arm64": "aarch64-apple-darwin",
  };
  const target = targets[key];
  if (!target) {
    fail(`unsupported build host ${key}; pass --target explicitly`);
  }
  return target;
}

function validateManifest(manifest) {
  if (
    typeof manifest !== "object" ||
    manifest === null ||
    typeof manifest.version !== "string" ||
    !/^\d+\.\d+\.\d+$/.test(manifest.version) ||
    typeof manifest.releaseTag !== "string" ||
    manifest.releaseTag !== `rust-v${manifest.version}` ||
    typeof manifest.resources !== "object" ||
    manifest.resources === null ||
    typeof manifest.targets !== "object" ||
    manifest.targets === null
  ) {
    fail("invalid tools/codex-version.json");
  }
}

function validateResource(name, record) {
  if (
    typeof record !== "object" ||
    record === null ||
    typeof record.sourcePath !== "string" ||
    typeof record.output !== "string" ||
    !Number.isSafeInteger(record.size) ||
    record.size <= 0 ||
    typeof record.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(record.sha256)
  ) {
    fail(`invalid pinned resource record for ${name}`);
  }
  const sourceComponents = record.sourcePath.split("/");
  if (
    record.sourcePath.length === 0 ||
    record.sourcePath.startsWith("/") ||
    record.sourcePath.includes("\\") ||
    sourceComponents.some((component) => component.length === 0 || component === "..") ||
    record.output.length === 0 ||
    record.output !== basename(record.output) ||
    record.output.includes("..") ||
    record.output.includes("/") ||
    record.output.includes("\\")
  ) {
    fail(`unsafe pinned resource path for ${name}`);
  }
}

function validateTarget(target, record) {
  if (
    typeof record !== "object" ||
    record === null ||
    typeof record.asset !== "string" ||
    typeof record.archiveEntry !== "string" ||
    !Number.isSafeInteger(record.assetSize) ||
    record.assetSize <= 0 ||
    typeof record.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(record.sha256)
  ) {
    fail(`invalid release record for ${target}`);
  }
  for (const value of [record.asset, record.archiveEntry]) {
    if (
      value.length === 0 ||
      value !== basename(value) ||
      value.includes("..") ||
      value.includes("/") ||
      value.includes("\\")
    ) {
      fail(`unsafe release filename for ${target}`);
    }
  }
  if (!record.asset.endsWith(".tar.gz")) {
    fail(`unsupported archive format for ${target}`);
  }
}

async function run(command, args) {
  return await new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, {
      shell: false,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      const result = {
        code,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
      };
      if (code === 0) {
        resolvePromise(result);
      } else {
        reject(
          new Error(
            `${command} exited ${String(code)}: ${result.stderr.trim() || "no diagnostic"}`,
          ),
        );
      }
    });
  });
}

async function download(url, destination, expectedSize, expectedSha256) {
  const response = await fetch(url, {
    headers: {
      "User-Agent": "OpenConKit-sidecar-fetch",
    },
    redirect: "follow",
  });
  if (!response.ok || !response.body) {
    fail(`download failed with HTTP ${response.status}`);
  }

  const hash = createHash("sha256");
  let byteCount = 0;
  const digestingStream = new Transform({
    transform(chunk, _encoding, callback) {
      byteCount += chunk.length;
      hash.update(chunk);
      callback(null, chunk);
    },
  });
  await pipeline(
    Readable.fromWeb(response.body),
    digestingStream,
    createWriteStream(destination, { flags: "wx", mode: 0o600 }),
  );

  const digest = hash.digest("hex");
  if (byteCount !== expectedSize) {
    fail(`asset size mismatch: expected ${expectedSize}, received ${byteCount}`);
  }
  if (digest !== expectedSha256) {
    fail(`asset checksum mismatch: expected ${expectedSha256}, received ${digest}`);
  }
}

function normalizeArchiveEntry(entry) {
  return entry
    .replaceAll("\\", "/")
    .replace(/^\.\/+/, "")
    .replace(/\/+$/, "");
}

function validateArchiveEntry(entry, expected) {
  const normalized = normalizeArchiveEntry(entry);
  if (
    normalized.length === 0 ||
    normalized.startsWith("/") ||
    /^[A-Za-z]:/.test(normalized) ||
    normalized.split("/").some((component) => component === "..") ||
    normalized !== expected
  ) {
    fail(`unexpected or unsafe archive entry: ${entry}`);
  }
}

async function extractVerifiedBinary(archive, extractionDirectory, expectedEntry) {
  const listing = await run("tar", ["-tzf", archive]);
  const entries = listing.stdout
    .split(/\r?\n/u)
    .map((entry) => entry.trim())
    .filter(Boolean);
  if (entries.length !== 1) {
    fail(`release archive must contain exactly one file, found ${entries.length}`);
  }
  validateArchiveEntry(entries[0], expectedEntry);

  await run("tar", ["-xzf", archive, "-C", extractionDirectory, entries[0]]);
  const extracted = resolve(extractionDirectory, normalizeArchiveEntry(entries[0]));
  const extractionRoot = `${resolve(extractionDirectory)}${sep}`;
  if (!extracted.startsWith(extractionRoot)) {
    fail("archive extraction escaped the temporary directory");
  }
  const metadata = await lstat(extracted);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    fail("release archive entry is not a regular file");
  }
  return extracted;
}

function stagedBinaryName(target) {
  return `codex-app-server-${target}${target.includes("windows") ? ".exe" : ""}`;
}

async function stageBinary(source, stageDirectory, target) {
  await mkdir(stageDirectory, { recursive: true, mode: 0o700 });
  const destination = resolve(stageDirectory, stagedBinaryName(target));
  const stageRoot = `${resolve(stageDirectory)}${sep}`;
  if (
    !destination.startsWith(stageRoot) ||
    relative(stageDirectory, destination).startsWith("..")
  ) {
    fail("staged binary path escaped the stage directory");
  }
  const temporary = `${destination}.tmp-${process.pid}`;
  await rm(temporary, { force: true });
  await copyFile(source, temporary);
  if (process.platform !== "win32") {
    await chmod(temporary, 0o755);
  }
  await rm(destination, { force: true });
  await rename(temporary, destination);
  return destination;
}

async function stageResource(source, resourceDirectory, output) {
  await mkdir(resourceDirectory, { recursive: true, mode: 0o700 });
  const destination = resolve(resourceDirectory, output);
  const resourceRoot = `${resolve(resourceDirectory)}${sep}`;
  if (
    !destination.startsWith(resourceRoot) ||
    relative(resourceDirectory, destination).startsWith("..")
  ) {
    fail("staged resource path escaped the resource directory");
  }
  const temporary = `${destination}.tmp-${process.pid}`;
  await rm(temporary, { force: true });
  await copyFile(source, temporary);
  await rm(destination, { force: true });
  await rename(temporary, destination);
  return destination;
}

async function verifyNativeVersion(staged, target, version) {
  const nativeTarget = detectTarget();
  const isNativeUniversal = process.platform === "darwin" && target === universalTarget;
  if (target !== nativeTarget && !isNativeUniversal) {
    return;
  }
  const result = await run(staged, ["--version"]);
  const expected = `codex-app-server ${version}`;
  if (result.stdout.trim() !== expected) {
    fail(`staged executable version mismatch: expected "${expected}"`);
  }
}

async function fetchPinnedBinary(manifest, target, temporaryRoot) {
  const targetRecord = manifest.targets[target];
  validateTarget(target, targetRecord);
  const archive = join(temporaryRoot, targetRecord.asset);
  const extractionDirectory = join(temporaryRoot, `extracted-${target}`);
  await mkdir(extractionDirectory, { mode: 0o700 });
  const url = `${releaseOrigin}/${manifest.releaseTag}/${targetRecord.asset}`;
  process.stdout.write(`Fetching Codex app-server ${manifest.version} for ${target}...\n`);
  await download(url, archive, targetRecord.assetSize, targetRecord.sha256);
  return extractVerifiedBinary(archive, extractionDirectory, targetRecord.archiveEntry);
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  validateManifest(manifest);
  for (const [name, resource] of Object.entries(manifest.resources)) {
    validateResource(name, resource);
  }
  if (options.target === universalTarget && process.platform !== "darwin") {
    fail("the universal macOS sidecar can only be assembled on macOS");
  }
  if (options.target !== universalTarget) {
    validateTarget(options.target, manifest.targets[options.target]);
  } else {
    for (const target of universalSourceTargets) {
      validateTarget(target, manifest.targets[target]);
    }
  }

  const temporaryRoot = await mkdtemp(join(tmpdir(), "openconkit-codex-"));
  try {
    if (!options.resourcesOnly) {
      let source;
      if (options.target === universalTarget) {
        const architectureBinaries = [];
        for (const target of universalSourceTargets) {
          architectureBinaries.push(await fetchPinnedBinary(manifest, target, temporaryRoot));
        }
        source = join(temporaryRoot, "codex-app-server-universal");
        await run("lipo", ["-create", ...architectureBinaries, "-output", source]);
        const architectureResult = await run("lipo", ["-archs", source]);
        const architectures = new Set(architectureResult.stdout.trim().split(/\s+/u));
        if (!architectures.has("x86_64") || !architectures.has("arm64")) {
          fail(`universal binary has unexpected architectures: ${architectureResult.stdout}`);
        }
      } else {
        source = await fetchPinnedBinary(manifest, options.target, temporaryRoot);
      }
      const staged = await stageBinary(source, options.stageDirectory, options.target);
      await verifyNativeVersion(staged, options.target, manifest.version);
      const stagedSize = (await stat(staged)).size;
      process.stdout.write(`Staged ${staged} (${stagedSize} bytes)\n`);
    }

    for (const [name, resource] of Object.entries(manifest.resources)) {
      const downloaded = join(temporaryRoot, `resource-${name}`);
      const url = `${sourceOrigin}/${manifest.releaseTag}/${resource.sourcePath}`;
      await download(url, downloaded, resource.size, resource.sha256);
      const stagedResource = await stageResource(
        downloaded,
        options.resourceDirectory,
        resource.output,
      );
      process.stdout.write(`Staged ${stagedResource} (${resource.size} bytes)\n`);
    }
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
