#!/usr/bin/env sh
set -eu

repo="${JIN_REPO:-burniq/jin}"
version="${JIN_VERSION:-}"
release_tag="${JIN_RELEASE_TAG:-}"

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "jin installer: missing required command: $1" >&2
    exit 1
  fi
}

need cp
need curl
need id
need install
need mkdir
need mktemp
need rm
need sed
need tar
need uname

resolve_release() {
  if [ -n "$version" ]; then
    if [ -z "$release_tag" ]; then
      release_tag="v$version"
    fi
    return
  fi

  if [ -n "$release_tag" ]; then
    version="${release_tag#v}"
    return
  fi

  latest_url="${JIN_LATEST_RELEASE_URL:-https://api.github.com/repos/$repo/releases/latest}"
  echo "jin installer: resolving latest release from $latest_url"
  latest_json="$(curl -fsSL "$latest_url")"
  release_tag="$(printf '%s\n' "$latest_json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | sed -n '1p')"
  if [ -z "$release_tag" ]; then
    echo "jin installer: failed to resolve latest release tag" >&2
    exit 1
  fi
  version="${release_tag#v}"
}

install_privilege() {
  bin_dir="$1/bin"
  share_dir="$1/share/jin"
  if [ "$(id -u)" -eq 0 ]; then
    mkdir -p "$bin_dir" "$share_dir"
    echo ""
    return
  fi

  if can_write_dir "$bin_dir" && can_write_dir "$share_dir"; then
    echo ""
    return
  fi

  if ! command -v sudo >/dev/null 2>&1; then
    echo "jin installer: $1 is not writable and sudo is not available" >&2
    exit 1
  fi

  sudo mkdir -p "$bin_dir" "$share_dir"
  echo "sudo"
}

can_write_dir() {
  dir="$1"
  if mkdir -p "$dir" 2>/dev/null; then
    probe="$dir/.jin-install-write-test.$$"
    if ( : >"$probe" ) 2>/dev/null; then
      rm -f "$probe"
      return 0
    fi
  fi
  return 1
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Linux:x86_64|Linux:amd64) echo "linux-x86_64" ;;
    Linux:aarch64|Linux:arm64) echo "linux-aarch64" ;;
    Darwin:arm64) echo "darwin-arm64" ;;
    Darwin:x86_64) echo "darwin-x86_64" ;;
    *)
      echo "jin installer: unsupported target $os/$arch" >&2
      echo "jin installer: set JIN_TARGET to override" >&2
      exit 1
      ;;
  esac
}

target="${JIN_TARGET:-$(detect_target)}"
resolve_release
case "$target" in
  linux-*) default_prefix="/usr" ;;
  darwin-*) default_prefix="/usr/local" ;;
  *) default_prefix="/usr/local" ;;
esac

install_prefix() {
  if [ -n "${PREFIX:-}" ]; then
    echo "$PREFIX"
    return
  fi

  if command -v jin-server >/dev/null 2>&1; then
    existing_jin="$(command -v jin-server)"
    existing_dir="${existing_jin%/*}"
    case "$existing_dir" in
      */bin)
        existing_prefix="${existing_dir%/bin}"
        if [ -z "$existing_prefix" ]; then
          existing_prefix="/"
        fi
        echo "$existing_prefix"
        return
        ;;
    esac
  fi

  echo "$default_prefix"
}

prefix="$(install_prefix)"
if [ -z "${PREFIX:-}" ] && command -v jin-server >/dev/null 2>&1; then
  echo "jin installer: updating existing $(command -v jin-server)"
fi

install_package_dir() {
  package_dir="$1"
  sudo_cmd="$(install_privilege "$prefix")"

  echo "jin installer: installing commands into $prefix/bin"
  for bin in jin-server jin-web jin-supervisor jin-web-client; do
    $sudo_cmd install -m 0755 "$package_dir/bin/$bin" "$prefix/bin/$bin"
  done

  if [ -d "$package_dir/share/jin/web-client" ]; then
    echo "jin installer: installing web client assets into $prefix/share/jin/web-client"
    $sudo_cmd mkdir -p "$prefix/share/jin"
    $sudo_cmd rm -rf "$prefix/share/jin/web-client"
    $sudo_cmd cp -R "$package_dir/share/jin/web-client" "$prefix/share/jin/web-client"
  fi
}

workdir="$(mktemp -d "${TMPDIR:-/tmp}/jin-install.XXXXXX")"
cleanup() {
  rm -rf "$workdir"
}
trap cleanup EXIT INT TERM

if [ "${JIN_INSTALL_FROM_SOURCE:-}" = "1" ]; then
  need bash
  need cargo
  need find
  need npm
  need sed

  ref="${JIN_REF:-main}"
  archive="$workdir/source.tar.gz"
  url="https://github.com/$repo/archive/$ref.tar.gz"

  echo "jin installer: downloading $url"
  curl -fsSL "$url" -o "$archive"
  tar -xzf "$archive" -C "$workdir"

  src_dir="$(find "$workdir" -mindepth 1 -maxdepth 1 -type d | sed -n '1p')"
  if [ -z "$src_dir" ]; then
    echo "jin installer: failed to locate unpacked source directory" >&2
    exit 1
  fi

  echo "jin installer: building release package from source"
  JIN_VERSION="$version" JIN_TARGET="$target" \
    bash "$src_dir/scripts/package.sh" "$workdir/package" "$workdir/release"
  install_package_dir "$workdir/package"
else
  asset="jin-$version-$target.tar.gz"
  archive="$workdir/jin.tar.gz"
  release_base_url="${JIN_RELEASE_BASE_URL:-https://github.com/$repo/releases/download/$release_tag}"
  url="$release_base_url/$asset"

  echo "jin installer: downloading $url"
  curl -fsSL "$url" -o "$archive"
  tar -xzf "$archive" -C "$workdir"
  install_package_dir "$workdir"
fi

echo "jin installer: installed jin into $prefix"
echo "next step: start jin-server, then run jin-web-client"
