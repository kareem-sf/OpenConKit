#!/usr/bin/env node

import { resolve } from "node:path";

import { Launcher } from "@wdio/cli";

const [configPath, ...specFiles] = process.argv.slice(2);
if (!configPath || specFiles.length === 0) {
  console.error("run-wdio: expected a config path and at least one spec file");
  process.exit(1);
}

const launcher = new Launcher(resolve(configPath), {
  spec: specFiles.map((specFile) => resolve(specFile)),
  logLevel: process.env.CI ? "trace" : "info",
});

let exitCode = 1;
const keepAlive = setInterval(() => {}, 1_000);
try {
  exitCode = await launcher.run();
} catch (error) {
  console.error(error);
} finally {
  clearInterval(keepAlive);
}
process.exit(exitCode);
