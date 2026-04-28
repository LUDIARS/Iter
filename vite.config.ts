import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Tauri 2 の dev server 設定。Tauri が起動する windows の指す URL は
// `tauri.conf.json` の `build.devUrl` (= http://localhost:5173) と
// 一致させる必要がある。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: false,
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        file: resolve(__dirname, "file-window.html"),
      },
    },
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
});
