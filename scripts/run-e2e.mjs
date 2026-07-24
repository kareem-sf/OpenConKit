import { spawn } from "node:child_process";
import { readFile, rm } from "node:fs/promises";
import { createRequire } from "node:module";
import { createServer } from "node:net";
import { dirname, join, resolve } from "node:path";

const root = resolve(".");
const sentinel = join(root, "target", "e2e", "wdio-result.json");
const cli = join(root, "node_modules", "@wdio", "cli", "bin", "wdio.js");
const desktopUi = join(root, "apps", "desktop-ui");
const requireFromDesktopUi = createRequire(join(desktopUi, "package.json"));
const vitePackage = requireFromDesktopUi.resolve("vite/package.json");
const viteCli = join(dirname(vitePackage), "bin", "vite.js");
const devServerHost = "127.0.0.1";
const devServerPort = 1421;
const devServerUrl = `http://${devServerHost}:${devServerPort}/`;
const expectedTests = 4;

await rm(sentinel, { force: true });

function delay(milliseconds) {
  return new Promise((resolveDelay) => {
    setTimeout(resolveDelay, milliseconds);
  });
}

function waitForExit(child) {
  return new Promise((resolveExit, rejectExit) => {
    child.once("error", rejectExit);
    child.once("close", (code) => resolveExit(code ?? 1));
  });
}

function assertDevServerPortAvailable() {
  return new Promise((resolvePort, rejectPort) => {
    const probe = createServer();
    probe.once("error", (error) => {
      rejectPort(
        new Error(`E2E dev-server port ${devServerPort} is unavailable.`, {
          cause: error,
        }),
      );
    });
    probe.listen({ host: devServerHost, port: devServerPort, exclusive: true }, () => {
      probe.close((error) => {
        if (error) {
          rejectPort(error);
        } else {
          resolvePort();
        }
      });
    });
  });
}

async function waitForDevServer(child) {
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error("Vite exited before the E2E dev server became ready.");
    }
    try {
      const response = await fetch(devServerUrl, {
        signal: AbortSignal.timeout(1_000),
      });
      if (response.ok) {
        return;
      }
    } catch {
      // The server is still starting.
    }
    await delay(250);
  }
  throw new Error(`Vite did not become ready at ${devServerUrl} within 60 seconds.`);
}

async function stopChild(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  const closed = new Promise((resolveClose) => {
    child.once("close", resolveClose);
  });
  child.kill("SIGTERM");
  const closedGracefully = await Promise.race([
    closed.then(() => true),
    delay(5_000).then(() => false),
  ]);
  if (!closedGracefully) {
    child.kill("SIGKILL");
    await closed;
  }
}

await assertDevServerPortAvailable();

const vite = spawn(
  process.execPath,
  [viteCli, "--host", devServerHost, "--port", String(devServerPort), "--strictPort"],
  {
    cwd: desktopUi,
    stdio: "inherit",
    windowsHide: true,
  },
);

let exitCode;
try {
  await waitForDevServer(vite);
  const wdio = spawn(process.execPath, [cli, "run", "e2e/wdio.conf.ts"], {
    cwd: root,
    stdio: "inherit",
    windowsHide: true,
  });
  exitCode = await waitForExit(wdio);
} finally {
  await stopChild(vite);
}

if (exitCode !== 0) {
  process.exitCode = exitCode ?? 1;
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
