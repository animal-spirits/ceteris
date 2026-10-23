import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The map reads the pack's web tables as static files. Outside the desktop app, cost
// queries go to the development engine (`cargo run -p ceteris-geo --example engine`).
export default defineConfig({
  plugins: [react()],
  publicDir: "../data/packs/2000/web",
  clearScreen: false,
  // Pre-bundling moves MapLibre away from its worker module, which it locates relative to itself.
  optimizeDeps: { exclude: ["maplibre-gl"] },
  server: {
    port: 1420,
    strictPort: true,
    proxy: { "/engine": "http://127.0.0.1:7878" },
  },
});
