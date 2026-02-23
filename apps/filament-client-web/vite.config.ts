import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import UnoCSS from "unocss/vite";

function readPositiveIntEnv(name: string, fallback: number): number {
  const value = process.env[name];
  if (!value) {
    return fallback;
  }
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    return fallback;
  }
  return parsed;
}

function readCsvEnv(name: string, fallback: readonly string[]): string[] {
  const value = process.env[name];
  if (!value) {
    return [...fallback];
  }
  const values = value
    .split(",")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
  if (values.length < 1) {
    return [...fallback];
  }
  return values;
}

const DEV_SERVER_HOST = process.env.VITE_DEV_SERVER_HOST ?? "0.0.0.0";
const DEV_SERVER_PORT = readPositiveIntEnv("VITE_DEV_SERVER_PORT", 4173);
const DEV_ALLOWED_HOSTS = readCsvEnv("VITE_DEV_ALLOWED_HOSTS", [
  "localhost",
  "127.0.0.1",
]);
const DEV_HMR_CLIENT_PORT = readPositiveIntEnv("VITE_DEV_HMR_CLIENT_PORT", 443);
const DEV_API_PROXY_TARGET =
  process.env.VITE_DEV_API_PROXY_TARGET ?? "http://localhost:8080";
const DEV_GATEWAY_PROXY_TARGET =
  process.env.VITE_DEV_GATEWAY_PROXY_TARGET ?? "ws://localhost:8080";

export default defineConfig({
  envDir: "../../infra",
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
    host: DEV_SERVER_HOST,
    port: DEV_SERVER_PORT,
    strictPort: true,
    allowedHosts: DEV_ALLOWED_HOSTS,
    hmr: {
      clientPort: DEV_HMR_CLIENT_PORT,
    },
    proxy: {
      "/api": {
        target: DEV_API_PROXY_TARGET,
        changeOrigin: false,
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
      "/gateway/ws": {
        target: DEV_GATEWAY_PROXY_TARGET,
        ws: true,
        changeOrigin: false,
      },
    },
  },
  preview: {
    host: DEV_SERVER_HOST,
    port: DEV_SERVER_PORT,
    strictPort: true,
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./tests/setup.ts",
    include: ["tests/**/*.test.ts", "tests/**/*.test.tsx"],
  },
});
