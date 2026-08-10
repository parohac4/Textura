import { defineConfig } from "vite";

// Tauri si servíruje frontend z pevného portu a hlídá si vlastní výpis konzole.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    // Tauri v2 cílí na moderní webview, není důvod transpilovat níž.
    target: "es2021",
    sourcemap: true,
  },
});
