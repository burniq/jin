# Jin

Jin is a local master-agent workspace for controlling development agents from a
phone-friendly web UI and HTTP API.

The goal is simple: keep one always-on agent hub on your machine or server, open
it from a phone, choose a project, and drive tools such as Codex or shell from a
single chat interface.

Jin is an early MVP. The current implementation focuses on:

- project-scoped chat sessions;
- a React web client on a separate frontend port;
- native-wrapper Codex integration through the local Codex CLI/app server;
- shell runner with approval gates;
- local filesystem project registry;
- durable file-backed state;
- global settings such as public host rewriting for localhost links;
- supervisor commands for stable/candidate promotion and rollback.

## Install

From the GitHub project. The installer downloads prebuilt release archives; it
does not require Rust, Cargo, or npm on the target machine:

```sh
curl -fsSL https://raw.githubusercontent.com/burniq/jin/main/scripts/install.sh | sh
```

For a fork or private repo:

```sh
curl -fsSL https://raw.githubusercontent.com/burniq/jin/main/scripts/install.sh | JIN_REPO=burniq/jin sh
```

The installer downloads `jin-${JIN_VERSION:-0.1.0}-${target}.tar.gz` from
GitHub Releases. If `jin-server` is already in `PATH`, the installer updates
that existing location. Otherwise it installs into `/usr/bin` on Linux and
`/usr/local/bin` on macOS by default:

- `jin-server`
- `jin-web`
- `jin-supervisor`
- `jin-web-client`

The packaged `jin-web-client` command serves the built React client and proxies
`/api/*` to the backend. It uses the system `node` runtime.

Use a specific release or target when needed:

```sh
curl -fsSL https://raw.githubusercontent.com/burniq/jin/main/scripts/install.sh | \
  JIN_VERSION=0.1.0 JIN_TARGET=linux-x86_64 sh
```

Source builds are an explicit fallback for development machines with `bash`,
Rust/Cargo, Node.js, and npm:

```sh
curl -fsSL https://raw.githubusercontent.com/burniq/jin/main/scripts/install.sh | \
  JIN_INSTALL_FROM_SOURCE=1 JIN_REF=main sh
```

Use another install prefix when needed:

```sh
curl -fsSL https://raw.githubusercontent.com/burniq/jin/main/scripts/install.sh | PREFIX="$HOME/.local" sh
```

When using a custom prefix, make sure the install bin directory is in `PATH`.
For example:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Release archives are published automatically by GitHub Actions when a tag like
`v0.1.0` is pushed. The release workflow uploads:

- `jin-<version>-linux-x86_64.tar.gz`
- `jin-<version>-linux-aarch64.tar.gz`
- `jin-<version>-darwin-arm64.tar.gz`
- `jin-<version>-darwin-x86_64.tar.gz`

## Quick Start

Start the backend API:

```sh
jin-server --addr 127.0.0.1:8787 --state "$HOME/.jin/state/state.json"
```

Start the web client in another terminal:

```sh
jin-web-client
```

Open:

```text
http://127.0.0.1:8790
```

The React web client proxies `/api/*` to `http://127.0.0.1:8787/*`, so the
backend must be running first. Override the backend URL when needed:

```sh
JIN_API_BASE=http://127.0.0.1:8787 jin-web-client --host 127.0.0.1 --port 8790
```

For the current MVP, start the React web client with a backend that has no
`JIN_API_TOKEN`. Token support exists on the backend, but the React client does
not yet inject an authorization header.

## Codex Setup

To use Codex chats, install and authenticate the Codex CLI on the same machine
where `jin-server` runs:

```sh
codex --version
codex login
```

Jin discovers available Codex models from the local Codex installation and
starts native Codex sessions in the selected project directory.

## Projects

Add projects from the web UI, or register one through the API:

```sh
curl -sS -X POST http://127.0.0.1:8787/projects \
  -H 'content-type: application/json' \
  -d '{"name":"jin","root":"/Users/nikita/dev/homeworks/jin"}'
```

Chats are grouped by project in the sidebar. Project groups can be collapsed,
and the collapsed state is saved locally in the browser.

## Public Host

If Jin is opened from another device, localhost links produced by tools are not
useful as-is. Set a global public host in `Settings`.

Example:

```text
jin.example.com
```

Then a tool link such as:

```text
http://localhost:54418/content/design.html
```

is rendered in chat as:

```text
http://jin.example.com:54418/content/design.html
```

This is useful for Superpowers design docs and other local preview servers.

## API Token Mode

Backend token protection can be enabled for API or server-rendered usage:

```sh
JIN_API_TOKEN=dev-secret jin-server --addr 127.0.0.1:8787
```

Then protected API calls require:

```text
Authorization: Bearer dev-secret
```

The legacy Rust web frontend keeps the token server-side:

```sh
JIN_API_BASE=http://127.0.0.1:8787 \
JIN_API_TOKEN=dev-secret \
jin-web --addr 127.0.0.1:8788
```

## Useful Commands

Run tests:

```sh
cargo test --workspace
cd apps/jin-web-client && npm test
```

Build the web client:

```sh
cd apps/jin-web-client && npm run build
```

Build a local release archive:

```sh
JIN_VERSION=0.1.0 JIN_TARGET=darwin-arm64 bash scripts/package.sh
```

Run the backend from source during development:

```sh
cargo run --bin jin-server -- --addr 127.0.0.1:8787 --state .jin/state/state.json
```

Run the frontend from source during development:

```sh
cd apps/jin-web-client
npm install
npm run dev
```

## Supervisor Rollback

Jin includes a small supervisor CLI for stable/candidate artifact bookkeeping:

```sh
jin-supervisor init-stable \
  --state "$HOME/.jin/state/versions.json" \
  --name stable-a \
  --source-ref "$(git rev-parse HEAD)" \
  --artifact-path "$(command -v jin-server)"

jin-supervisor set-candidate \
  --state "$HOME/.jin/state/versions.json" \
  --name candidate-b \
  --source-ref "$(git rev-parse HEAD)" \
  --artifact-path "$(command -v jin-server)"

jin-supervisor promote --state "$HOME/.jin/state/versions.json"
jin-supervisor rollback --state "$HOME/.jin/state/versions.json"
```

`rollback` prints the artifact path that an external process supervisor should
restart.

## Repository

```text
https://github.com/burniq/jin
```
