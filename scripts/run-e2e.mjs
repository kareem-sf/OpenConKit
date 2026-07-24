import { spawn } from "node:child_process";
import { readFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";

const root = resolve(".");
const sentinel = join(root, "target", "e2e", "wdio-result.json");
const cli = join(root, "node_modules", "@wdio", "cli", "bin", "wdio.js");
const expectedTests = 4;

await rm(sentinel, { force: true });

const exitCode = await new Promise((resolveExit, rejectExit) => {
  const child = spawn(process.execPath, [cli, "run", "e2e/wdio.conf.ts"], {
    cwd: root,
    stdio: "inherit",
  });
  child.once("error", rejectExit);
  child.once("close", (code) => resolveExit(code ?? 1));
});

if (exitCode !== 0) {
  process.exitCode = exitCode;
} else {
  let completedTests = 0;
  try {
    const parsed = JSON.parse(await readFile(sentinel, "utf8"));
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      "completedTests" in parsed &&
      typeof parsed.completedTests === "number"
    ) {
      completedTests = parsed.completedTests;
    }
  } catch {
    // A missing or malformed sentinel means the worker never completed.
  }
  if (completedTests !== expectedTests) {
    console.error(
      `E2E gate failed: expected ${expectedTests} completed tests, observed ${completedTests}.`,
    );
    process.exitCode = 1;
  }
}
