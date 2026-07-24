import { existsSync } from "node:fs";
import { join, resolve } from "node:path";

const projectRoot = resolve(".");
const devServerUrl = "http://127.0.0.1:1421/?openconkit-e2e=1";
const chromedriverDirectory = process.env.CHROMEWEBDRIVER;
const preinstalledChromedriver = chromedriverDirectory
  ? join(chromedriverDirectory, process.platform === "win32" ? "chromedriver.exe" : "chromedriver")
  : undefined;
const chromedriverBinary =
  preinstalledChromedriver && existsSync(preinstalledChromedriver)
    ? { binary: preinstalledChromedriver }
    : {};

export const config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  autoXvfb: false,
  capabilities: [
    {
      browserName: "tauri",
      "goog:chromeOptions": {
        args: ["--headless=new", "--no-sandbox", "--disable-dev-shm-usage"],
      },
      "wdio:enforceWebDriverClassic": true,
      "wdio:chromedriverOptions": {
        cacheDir: join(projectRoot, "target", "wdio-driver-cache"),
        ...chromedriverBinary,
      },
    },
  ],
  services: [
    [
      "tauri",
      {
        mode: "browser",
        devServerUrl,
      },
    ],
  ],
  framework: "mocha",
  reporters: ["spec"],
  logLevel: "info",
  outputDir: join(projectRoot, "target", "e2e", "logs"),
  waitforTimeout: 10_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 2,
  mochaOpts: {
    ui: "bdd",
    timeout: 30_000,
  },
};
