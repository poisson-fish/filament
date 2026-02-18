import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import UnoCSS from "unocss/vite";

export default defineConfig({
  plugins: [UnoCSS(), solid()],
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          livekit: ["livekit-client"],
        },
      },
    },
  },
  server: {
    host: "0.0.0.0",
    port: 4173,
    strictPort: true,
    allowedHosts: ["filamentapp.net", "localhost"],
    hmr: {
      clientPort: 443,
    },
    proxy: {
      "/api": {
        target: "http://127.0.0.1:8080",
        changeOrigin: false,
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
      "/gateway/ws": {
        target: "ws://127.0.0.1:8080",
        ws: true,
        changeOrigin: false,
      },
    },
  },
  preview: {
    host: "0.0.0.0",
    port: 4173,
    strictPort: true,
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./tests/setup.ts",
    include: ["tests/**/*.test.ts", "tests/**/*.test.tsx"],
  },
});
