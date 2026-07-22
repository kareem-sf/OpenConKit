#!/usr/bin/env node
// generate-icons: rasterize the brand icon and generate the Tauri icon set.
//
//   node scripts/generate-icons.mjs
//
// Steps:
//   1. Rasterize branding/icon.svg to a 1024x1024 PNG (via sharp).
//   2. Run `@tauri-apps/cli icon` to generate the icon set into
//      crates/openconkit-desktop/icons/.
//
// Requires devDependencies installed (sharp, @tauri-apps/cli).

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const ICON_SOURCE = join(REPO_ROOT, "branding", "icon.svg");
const ICON_PNG = join(REPO_ROOT, "branding", "icon-1024.png");
const ICONS_DIR = join(REPO_ROOT, "crates", "openconkit-desktop", "icons");

if (!existsSync(ICON_SOURCE)) {
  console.error(`generate-icons: missing ${ICON_SOURCE}`);
  process.exit(1);
}

console.log("generate-icons: rasterizing branding/icon.svg (1024x1024)...");
const { default: sharp } = await import("sharp").catch(() => {
  console.error("generate-icons: sharp is not installed. Run `pnpm install` first.");
  process.exit(1);
});

await sharp(ICON_SOURCE, { density: 384 })
  .resize(1024, 1024, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
  .png()
  .toFile(ICON_PNG);

mkdirSync(ICONS_DIR, { recursive: true });

console.log("generate-icons: running tauri icon generator...");
const command = ["pnpm", "exec", "tauri", "icon", `"${ICON_PNG}"`, "-o", `"${ICONS_DIR}"`].join(
  " ",
);
const result = spawnSync(command, {
  cwd: REPO_ROOT,
  stdio: "inherit",
  shell: true,
});

if (result.status !== 0) {
  console.error("generate-icons: tauri icon failed. You can run it manually:");
  console.error(`  pnpm exec tauri icon "${ICON_PNG}" -o "${ICONS_DIR}"`);
  process.exit(result.status ?? 1);
}

console.log(`generate-icons: icon set written to ${ICONS_DIR}`);
