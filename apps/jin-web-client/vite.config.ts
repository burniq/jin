import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": {
        target: "http://127.0.0.1:8787",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ""),
        configure: (proxy) => {
          proxy.on("error", (_error, _request, response) => {
            if (!response || response.headersSent) {
              return;
            }
            response.writeHead(502, { "content-type": "application/json" });
            response.end(
              JSON.stringify({
                error:
                  "Jin backend is unavailable at http://127.0.0.1:8787. Start jin-server and refresh.",
              }),
            );
          });
        },
      },
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
  },
});
