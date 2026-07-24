import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";
import test from "node:test";

const execFileAsync = promisify(execFile);
const script = resolve("scripts", "prepare-update-feed.mjs");
const platforms = [
  "windows-x86_64-nsis",
  "linux-x86_64-appimage",
  "darwin-aarch64-app",
  "darwin-x86_64-app",
];

async function fixture() {
  const directory = await mkdtemp(join(tmpdir(), "openconkit-feed-test-"));
  const assets = platforms.map((platform, index) => ({
    id: index + 1,
    name: `${platform}.archive`,
    size: 1_000 + index,
    browser_download_url: `https://github.com/kareem-sf/OpenConKit/releases/download/v1.2.3/${platform}.archive`,
  }));
  const latest = {
    version: "1.2.3",
    notes: "Release notes",
    pub_date: "2026-07-24T08:00:00Z",
    platforms: Object.fromEntries(
      platforms.map((platform, index) => [
        platform,
        {
          signature: `signature-${platform}`,
          url: `https://api.github.com/repos/kareem-sf/OpenConKit/releases/assets/${index + 1}`,
        },
      ]),
    ),
  };
  const release = { tag_name: "v1.2.3", assets };
  const latestPath = join(directory, "latest.json");
  const releasePath = join(directory, "release.json");
  const outputPath = join(directory, "latest-stable.json");
  await writeFile(latestPath, JSON.stringify(latest));
  await writeFile(releasePath, JSON.stringify(release));
  return { directory, latest, latestPath, releasePath, outputPath };
}

test("prepares a bounded feed with release-owned asset sizes", async () => {
  const data = await fixture();
  try {
    await execFileAsync(process.execPath, [
      script,
      "--latest",
      data.latestPath,
      "--release",
      data.releasePath,
      "--output",
      data.outputPath,
    ]);
    const output = JSON.parse(await readFile(data.outputPath, "utf8"));
    assert.equal(output.version, "1.2.3");
    assert.equal(output.platforms["windows-x86_64-nsis"].size, 1_000);
    assert.equal(Object.keys(output.platforms).length, platforms.length);
  } finally {
    await rm(data.directory, { recursive: true, force: true });
  }
});

test("rejects a platform URL outside the project release", async () => {
  const data = await fixture();
  try {
    data.latest.platforms["windows-x86_64-nsis"].url = "https://attacker.invalid/OpenConKit.zip";
    await writeFile(data.latestPath, JSON.stringify(data.latest));
    await assert.rejects(
      execFileAsync(process.execPath, [
        script,
        "--latest",
        data.latestPath,
        "--release",
        data.releasePath,
        "--output",
        data.outputPath,
      ]),
    );
  } finally {
    await rm(data.directory, { recursive: true, force: true });
  }
});
