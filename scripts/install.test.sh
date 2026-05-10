#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/jin-install-test.XXXXXX")"
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT

package="$tmp/package"
mkdir -p "$package/bin" "$package/share/jin/web-client"
for bin in jin-server jin-web jin-supervisor jin-web-client; do
  printf '#!/usr/bin/env sh\nexit 0\n' >"$package/bin/$bin"
  chmod +x "$package/bin/$bin"
done
printf 'ok\n' >"$package/share/jin/web-client/index.html"
tar -C "$package" -czf "$tmp/jin.tar.gz" bin share

mock_bin="$tmp/bin"
mkdir -p "$mock_bin"
cat >"$mock_bin/curl" <<'CURL'
#!/usr/bin/env sh
set -eu

for arg in "$@"; do
  case "$arg" in
    https://api.github.com/repos/*/releases/latest)
      printf '{"tag_name":"v0.0.1"}\n'
      exit 0
      ;;
  esac
done

out=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "-o" ]; then
    out="$arg"
  fi
  prev="$arg"
done

url="${1}"
for arg in "$@"; do
  case "$arg" in
    http://*|https://*) url="$arg" ;;
  esac
done
printf '%s\n' "$url" >>"${JIN_INSTALL_TEST_URL_LOG}"
cp "${JIN_INSTALL_TEST_ARCHIVE}" "$out"
CURL
chmod +x "$mock_bin/curl"

url_log="$tmp/urls.log"
PATH="$mock_bin:$PATH" \
PREFIX="$tmp/install" \
JIN_TARGET="darwin-arm64" \
JIN_INSTALL_TEST_ARCHIVE="$tmp/jin.tar.gz" \
JIN_INSTALL_TEST_URL_LOG="$url_log" \
  sh "$repo_root/scripts/install.sh" >/dev/null

if ! grep -q 'releases/download/v0.0.1/jin-0.0.1-darwin-arm64.tar.gz' "$url_log"; then
  echo "installer did not default to latest v0.0.1 release asset" >&2
  cat "$url_log" >&2
  exit 1
fi

echo "install.sh default release test passed"
