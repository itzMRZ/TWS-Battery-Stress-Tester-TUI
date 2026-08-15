#!/bin/sh
# Install or update tws-tester from the latest GitHub release.
# The binary is SHA-256 checked before anything on disk is replaced.
#
#   curl -sSfL https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/releases/latest/download/install.sh | sh

set -eu

REPO="${TWS_TESTER_REPO:-https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI}"
DEST_DIR="${TWS_TESTER_BIN:-${HOME}/.local/bin}"
DEST="${DEST_DIR}/tws-tester"
UA="tws-tester-install (+${REPO})"

say() { printf '%s\n' "$*"; }
die() { say "$*" >&2; exit 1; }

if ! command -v curl >/dev/null 2>&1; then
  die "need curl"
fi

os=$(uname -s | tr 'A-Z' 'a-z')
arch=$(uname -m)
case "${os}-${arch}" in
  linux-x86_64|linux-amd64)
    asset="tws-tester-x86_64-unknown-linux-gnu"
    ;;
  *)
    die "no GitHub binary for ${os}/${arch} (Linux x86_64 only). Build from source: cargo build --release"
    ;;
esac

file_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    die "need sha256sum (coreutils) or shasum to verify the download"
  fi
}

fetch() {
  url=$1
  out=$2
  curl -fL --proto '=https' --tlsv1.2 --retry 3 --max-time 120 --max-filesize 104857600 \
    -A "$UA" -o "$out" "$url" \
    || die "could not download ${url}
Publish a GitHub release (tag vX.Y.Z matching Cargo.toml) or build from source: cargo build --release"
}

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

bin="${workdir}/${asset}"
sumfile="${workdir}/${asset}.sha256"
url="${REPO}/releases/latest/download/${asset}"

say "downloading ${url}"
fetch "$url" "$bin"
fetch "${url}.sha256" "$sumfile"

got=$(file_sha256 "$bin" | tr 'A-F' 'a-f')
want=$(tr -d '\r' < "$sumfile" | awk '{print $1}' | tr 'A-F' 'a-f')
if [ -z "$want" ] || [ "${#want}" -ne 64 ]; then
  die "checksum file is not a SHA-256 hex digest"
fi
if [ "$got" != "$want" ]; then
  die "SHA-256 mismatch (got ${got}, expected ${want}). Left the installed binary alone."
fi

if command -v od >/dev/null 2>&1; then
  hdr=$(od -An -N4 -tx1 "$bin" | tr -d ' \n')
  if [ "$hdr" != "7f454c46" ]; then
    die "downloaded file is not an ELF executable"
  fi
fi

chmod 755 "$bin"
ver=$("$bin" --version) || die "downloaded binary is not runnable"
case "$ver" in
  tws-tester\ *) ;;
  *) die "downloaded binary did not report tws-tester --version" ;;
esac

mkdir -p "$DEST_DIR"
new="${DEST_DIR}/.tws-tester.new.$$"
cp "$bin" "$new"
chmod 755 "$new"
mv "$new" "$DEST"

say "SHA-256 ok"
say "$ver"
say "installed ${DEST}"
case ":${PATH}:" in
  *":${DEST_DIR}:"*) ;;
  *) say "add ${DEST_DIR} to PATH" ;;
esac
