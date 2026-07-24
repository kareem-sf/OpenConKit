#!/usr/bin/env node

import { readFile, rename, rm, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
const requiredPlatforms = [
  "windows-x86_64-nsis",
  "linux-x86_64-appimage",
  "darwin-aarch64-app",
  "darwin-x86_64-app",
];
const projectApiAssetPrefix = "https://api.github.com/repos/kareem-sf/OpenConKit/releases/assets/";
const projectDownloadPrefix = "https://github.com/kareem-sf/OpenConKit/releases/download/";
const maximumNotesCharacters = 16_384;

function fail(message) {
  throw new Error(`prepare-update-feed: ${message}`);
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (["--latest", "--release", "--output"].includes(argument) && value) {
      options[argument.slice(2)] = resolve(value);
      index += 1;
    } else {
      fail(`unsupported argument: ${argument ?? ""}`);
    }
  }
  for (const field of ["latest", "release", "output"]) {
    if (typeof options[field] !== "string") {
      fail(`missing --${field}`);
    }
  }
  return options;
}

function isSemanticVersion(value) {
  return /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/u.test(
    value,
  );
}

function validateRelease(release) {
  if (
    typeof release !== "object" ||
    release === null ||
    typeof release.tag_name !== "string" ||
    !release.tag_name.startsWith("v") ||
    !isSemanticVersion(release.tag_name.slice(1)) ||
    !Array.isArray(release.assets)
  ) {
    fail("release metadata is invalid");
  }
}

function releaseAssetsByUrl(release) {
  const assets = new Map();
  for (const asset of release.assets) {
    if (
      typeof asset !== "object" ||
      asset === null ||
      !Number.isSafeInteger(asset.id) ||
      asset.id <= 0 ||
      typeof asset.browser_download_url !== "string" ||
      !Number.isSafeInteger(asset.size) ||
      asset.size <= 0
    ) {
      fail("release contains invalid asset metadata");
    }
    assets.set(`${projectApiAssetPrefix}${asset.id}`, asset);
    assets.set(asset.browser_download_url, asset);
  }
  return assets;
}

function validateLatest(latest, release) {
  if (
    typeof latest !== "object" ||
    latest === null ||
    !isSemanticVersion(latest.version) ||
    latest.version !== release.tag_name.slice(1) ||
    typeof latest.notes !== "string" ||
    typeof latest.pub_date !== "string" ||
    Number.isNaN(Date.parse(latest.pub_date)) ||
    typeof latest.platforms !== "object" ||
    latest.platforms === null ||
    Array.isArray(latest.platforms)
  ) {
    fail("latest.json is invalid or does not match the release tag");
  }
}

function enrichPlatforms(latest, assets) {
  const platforms = {};
  const entries = Object.entries(latest.platforms).sort(([left], [right]) =>
    left.localeCompare(right),
  );
  for (const [platform, value] of entries) {
    if (!/^(?:windows|linux|darwin)-[a-z0-9_-]+(?:-[a-z0-9_-]+)?$/u.test(platform)) {
      fail(`unsafe platform key: ${platform}`);
    }
    if (
      typeof value !== "object" ||
      value === null ||
      typeof value.signature !== "string" ||
      value.signature.trim().length === 0 ||
      value.signature.length > 8_192 ||
      typeof value.url !== "string" ||
      (!value.url.startsWith(projectApiAssetPrefix) && !value.url.startsWith(projectDownloadPrefix))
    ) {
      fail(`invalid updater metadata for ${platform}`);
    }
    const asset = assets.get(value.url);
    if (!asset) {
      fail(`updater URL for ${platform} is not a release asset`);
    }
    platforms[platform] = {
      signature: value.signature.trim(),
      url: value.url,
      size: asset.size,
    };
  }
  for (const platform of requiredPlatforms) {
    if (!(platform in platforms)) {
      fail(`latest.json is missing required platform ${platform}`);
    }
  }
  return platforms;
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const [latest, release] = await Promise.all([
    readFile(options.latest, "utf8").then(JSON.parse),
    readFile(options.release, "utf8").then(JSON.parse),
  ]);
  validateRelease(release);
  validateLatest(latest, release);
  const assets = releaseAssetsByUrl(release);
  const feed = {
    version: latest.version,
    notes: [...latest.notes].slice(0, maximumNotesCharacters).join(""),
    pub_date: new Date(latest.pub_date).toISOString(),
    platforms: enrichPlatforms(latest, assets),
  };
  const outputName = basename(options.output);
  if (!/^latest-(?:stable|beta)\.json$/u.test(outputName)) {
    fail("output must be latest-stable.json or latest-beta.json");
  }
  const temporary = `${options.output}.tmp-${process.pid}`;
  await rm(temporary, { force: true });
  await writeFile(temporary, `${JSON.stringify(feed, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
  });
  await rm(options.output, { force: true });
  await rename(temporary, options.output);
  process.stdout.write(
    `Prepared ${options.output} for ${feed.version} (${Object.keys(feed.platforms).length} platforms)\n`,
  );
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
