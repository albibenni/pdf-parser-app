/// <reference types="vitest" />

import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import cssInjectedByJsPlugin from "vite-plugin-css-injected-by-js";

// @ts-expect-error process is a Node.js global.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  base: "./",
  plugins: [react(), cssInjectedByJsPlugin()],
  clearScreen: false,
  server: {
    // @ts-expect-error process is a Node.js global.
    port: process.env.PORT ? parseInt(process.env.PORT, 10) : 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/setupTests.ts"],
    globals: true,
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
  },
});
