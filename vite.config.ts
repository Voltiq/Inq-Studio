import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { cpSync, existsSync } from "node:fs";

const host = process.env.TAURI_DEV_HOST;

/**
 * pdf.js needs its character maps (CJK text), the standard 14 fonts (for
 * PDFs that don't embed them) and its image decoders next to the app.
 * They are copied from node_modules into public/pdfjs on every build.
 */
function pdfjsAssets(): Plugin {
  return {
    name: "inq-pdfjs-assets",
    buildStart() {
      for (const dir of ["cmaps", "standard_fonts", "wasm"]) {
        const src = `node_modules/pdfjs-dist/${dir}`;
        if (existsSync(src)) cpSync(src, `public/pdfjs/${dir}`, { recursive: true });
      }
    },
  };
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [pdfjsAssets(), react()],

  // pdf.js uses top-level await and modern syntax; WebView2 is evergreen Chromium.
  build: { target: "es2022", chunkSizeWarningLimit: 1600 },
  optimizeDeps: { esbuildOptions: { target: "es2022" } },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
