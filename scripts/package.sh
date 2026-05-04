#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="${1:-$repo_root/dist/package}"
release_dir="${2:-$repo_root/dist}"
version="${JIN_VERSION:-0.1.0}"

detect_target() {
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64|Linux:amd64) echo "linux-x86_64" ;;
    Linux:aarch64|Linux:arm64) echo "linux-aarch64" ;;
    Darwin:arm64) echo "darwin-arm64" ;;
    Darwin:x86_64) echo "darwin-x86_64" ;;
    *)
      echo "unsupported target $(uname -s)/$(uname -m)" >&2
      exit 1
      ;;
  esac
}

target="${JIN_TARGET:-$(detect_target)}"

cd "$repo_root"

cargo build --release -p jin-server -p jin-web -p jin-supervisor
(
  cd apps/jin-web-client
  npm ci
  npm run build
)

case "$out_dir" in
  ""|"/")
    echo "refusing to package into unsafe output path: '$out_dir'" >&2
    exit 1
    ;;
esac

rm -rf "$out_dir"
mkdir -p "$out_dir/bin" "$out_dir/share/jin/web-client"

install -m 0755 target/release/jin-server "$out_dir/bin/jin-server"
install -m 0755 target/release/jin-web "$out_dir/bin/jin-web"
install -m 0755 target/release/jin-supervisor "$out_dir/bin/jin-supervisor"
install -m 0755 packaging/bin/jin-web-client "$out_dir/bin/jin-web-client"

cp -R apps/jin-web-client/dist/. "$out_dir/share/jin/web-client/"
install -m 0644 packaging/web-client/server.mjs \
  "$out_dir/share/jin/web-client/server.mjs"

mkdir -p "$release_dir"
tar -C "$out_dir" -czf "$release_dir/jin-$version-$target.tar.gz" bin share
