import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import http from "node:http";
import https from "node:https";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL(".", import.meta.url));
const config = parseArgs(process.argv.slice(2));

const server = http.createServer((request, response) => {
  if (request.url?.startsWith("/api")) {
    proxyApi(request, response);
    return;
  }
  serveStatic(request, response).catch((error) => {
    console.error("jin-web-client:", error);
    sendText(response, 500, "internal server error");
  });
});

server.listen(config.port, config.host, () => {
  console.log(`jin-web-client listening on http://${config.host}:${config.port}`);
  console.log(`jin-web-client proxying /api to ${config.apiBase}`);
});

function parseArgs(args) {
  if (args.includes("--help") || args.includes("-h")) {
    printHelp();
    process.exit(0);
  }

  const config = {
    host: process.env.JIN_WEB_HOST || "127.0.0.1",
    port: Number(process.env.JIN_WEB_PORT || "8790"),
    apiBase: process.env.JIN_API_BASE || "http://127.0.0.1:8787",
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--addr") {
      const next = requireValue(args, ++index, arg);
      const splitAt = next.lastIndexOf(":");
      if (splitAt === -1) {
        throw new Error("--addr must look like 127.0.0.1:8790");
      }
      config.host = next.slice(0, splitAt);
      config.port = Number(next.slice(splitAt + 1));
    } else if (arg === "--host") {
      config.host = requireValue(args, ++index, arg);
    } else if (arg === "--port") {
      config.port = Number(requireValue(args, ++index, arg));
    } else if (arg === "--api-base") {
      config.apiBase = requireValue(args, ++index, arg);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (!Number.isInteger(config.port) || config.port < 1 || config.port > 65535) {
    throw new Error(`invalid port: ${config.port}`);
  }

  return config;
}

function printHelp() {
  console.log(`Usage: jin-web-client [OPTIONS]

Options:
      --addr <ADDR>          Listen address, for example 127.0.0.1:8790
      --host <HOST>          Host to bind [default: 127.0.0.1]
      --port <PORT>          Port to bind [default: 8790]
      --api-base <URL>       Backend API base [default: http://127.0.0.1:8787]
  -h, --help                 Print help

Environment:
      JIN_WEB_HOST
      JIN_WEB_PORT
      JIN_API_BASE`);
}

function requireValue(args, index, flag) {
  const value = args[index];
  if (!value) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

async function serveStatic(request, response) {
  const requestUrl = new URL(request.url || "/", `http://${request.headers.host || "localhost"}`);
  const pathname = decodeURIComponent(requestUrl.pathname);
  const relative = pathname === "/" ? "index.html" : pathname.slice(1);
  let filePath = safeResolve(relative);

  if (!filePath) {
    sendText(response, 403, "forbidden");
    return;
  }

  const fileStat = await stat(filePath).catch(() => null);
  if (!fileStat?.isFile()) {
    if (path.extname(relative)) {
      sendText(response, 404, "not found");
      return;
    }
    filePath = safeResolve("index.html");
  }

  if (!filePath) {
    sendText(response, 404, "not found");
    return;
  }

  response.writeHead(200, {
    "content-type": contentType(filePath),
    "cache-control": cacheControl(filePath),
  });

  if (request.method === "HEAD") {
    response.end();
    return;
  }

  createReadStream(filePath).pipe(response);
}

function proxyApi(request, response) {
  const upstreamUrl = new URL(
    (request.url || "/api").replace(/^\/api/, "") || "/",
    config.apiBase,
  );
  const transport = upstreamUrl.protocol === "https:" ? https : http;
  const headers = { ...request.headers, host: upstreamUrl.host };

  const upstream = transport.request(
    upstreamUrl,
    {
      method: request.method,
      headers,
    },
    (upstreamResponse) => {
      response.writeHead(upstreamResponse.statusCode || 502, upstreamResponse.headers);
      upstreamResponse.pipe(response);
    },
  );

  upstream.on("error", () => {
    if (response.headersSent) {
      response.destroy();
      return;
    }
    response.writeHead(502, { "content-type": "application/json" });
    response.end(
      JSON.stringify({
        error: `Jin backend is unavailable at ${config.apiBase}. Start jin-server and refresh.`,
      }),
    );
  });

  request.pipe(upstream);
}

function safeResolve(relativePath) {
  const resolved = path.resolve(root, relativePath);
  if (resolved === root || resolved.startsWith(root)) {
    return resolved;
  }
  return null;
}

function sendText(response, statusCode, text) {
  response.writeHead(statusCode, { "content-type": "text/plain; charset=utf-8" });
  response.end(text);
}

function contentType(filePath) {
  switch (path.extname(filePath)) {
    case ".css":
      return "text/css; charset=utf-8";
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    case ".json":
      return "application/json; charset=utf-8";
    case ".svg":
      return "image/svg+xml";
    case ".txt":
      return "text/plain; charset=utf-8";
    case ".wasm":
      return "application/wasm";
    default:
      return "application/octet-stream";
  }
}

function cacheControl(filePath) {
  return filePath.includes(`${path.sep}assets${path.sep}`)
    ? "public, max-age=31536000, immutable"
    : "no-cache";
}
