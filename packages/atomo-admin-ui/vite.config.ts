import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

// https://vitejs.dev/config/
//
// `base` is set on the CLI at build time (e.g. `vite build --base=/admin/`) so the
// same config serves both root (dev) and the /admin subpath (production image).
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "VITE_");
  // Tests can select an isolated backend without compiling a cross-origin API
  // URL into the client. Both dev and preview inherit this same proxy target.
  const backend = new URL(
    process.env.E2E_API_URL || env.VITE_API_URL || "http://localhost:3000",
  );
  if (
    !["http:", "https:"].includes(backend.protocol) ||
    backend.username ||
    backend.password
  ) {
    throw new Error(
      "E2E_API_URL or VITE_API_URL must be an HTTP(S) backend URL without credentials",
    );
  }
  const target = backend.toString().replace(/\/$/, "");
  const websocket = new URL(backend);
  websocket.protocol = backend.protocol === "https:" ? "wss:" : "ws:";
  return {
    plugins: [react()],
    build: {
      outDir: "dist",
      assetsDir: "assets",
      commonjsOptions: {
        strictRequires: true,
        transformMixedEsModules: true,
      },
    },
    resolve: {
      dedupe: [
        "react",
        "react-dom",
        "react-router-dom",
        "@tanstack/react-query",
      ],
    },
    server: {
      port: 5173,
      host: true,
      proxy: {
        // Proxy all API requests to atomo-server. Vite serves its own assets
        // (/@vite, /src, /node_modules, *.hot-update.*) directly; everything
        // else is an API call that should hit the configured backend, including
        // relative schema-parser requests that do not use the Axios base URL.
        "/api": target,
        "/auth": target,
        "/graphql": target,
        "/schema.ts": target,
        "/meta": target,
        "/health": target,
        "/media": target,
        "/jobs": target,
        "/workflows": target,
        "/audit": target,
        "/storage": target,
        "/oauth": target,
        "/version": target,
        "/info": target,
        "/ready": target,
        "/metrics": target,
        "/ws": { target: websocket.toString().replace(/\/$/, ""), ws: true },
      },
    },
  };
});
