import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  publicDir: "assets",
  server: {
    port: 1420,
    strictPort: true,
  },
});
