// vite.config.js
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(async () => ({
  plugins: [react()],

  // Tauri: dev server must be on a fixed port
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Tell Vite to ignore watching the Rust source
      ignored: ["**/src-tauri/**"],
    },
  },

  // Prevent Vite from obscuring Rust compile errors in the console
  clearScreen: false,
}));
