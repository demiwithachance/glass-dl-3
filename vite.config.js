import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  publicDir: "assets",
  build: {
    target: "esnext",
  },
  server: {
    port: 1420,
    strictPort: true,
  },
});
