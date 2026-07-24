/// <reference types="vitest/config" />
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri conventions: fixed dev port, no terminal clearing.
// See https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  // Relative base so the bundled app resolves assets under tauri://localhost.
  base: "./",
  envPrefix: ["VITE_", "TAURI_ENV_"],
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // The Rust workspace is built by cargo, not Vite.
      ignored: ["**/crates/**", "**/target/**"],
    },
  },
  build: {
    target: "es2022",
    // Source maps retain tree-shaken development fixtures and E2E controls.
    // They are useful in dev, but must never be shipped inside the desktop app.
    sourcemap: false,
    chunkSizeWarningLimit: 1200,
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
