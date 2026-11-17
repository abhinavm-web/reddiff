#!/usr/bin/env sh
set -eu

REPO="abhinavm-web/reddiff"
BIN_NAME="reddiff"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}:${ARCH}" in
  Linux:x86_64|Linux:amd64)
    TARGET="x86_64-unknown-linux-gnu"
    ;;
  Darwin:arm64|Darwin:aarch64)
    TARGET="aarch64-apple-darwin"
    ;;
  *)
    echo "Unsupported prebuilt target: ${OS} ${ARCH}" >&2
    echo "Build from source instead: cargo install --git https://github.com/${REPO}" >&2
    exit 1
    ;;
esac

ARCHIVE="${BIN_NAME}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT INT TERM

echo "Downloading ${BIN_NAME} (${TARGET}) from ${URL}..."
curl -fsSL "${URL}" -o "${TMP_DIR}/${ARCHIVE}"
tar -xzf "${TMP_DIR}/${ARCHIVE}" -C "${TMP_DIR}"

mkdir -p "${INSTALL_DIR}"
install -m 755 "${TMP_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"

echo "Installed ${BIN_NAME} to ${INSTALL_DIR}/${BIN_NAME}"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo "Add ${INSTALL_DIR} to your PATH to run '${BIN_NAME}' from anywhere:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac
