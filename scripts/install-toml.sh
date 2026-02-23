#!/usr/bin/env bash
set -euo pipefail

TOML_CLI_VERSION=${TOML_CLI_VERSION:-0.2.3}
readonly TOML_CLI_VERSION
export TOML_CLI_VERSION
TOML_BINARY_NAME="toml"
readonly TOML_BINARY_NAME
export TOML_BINARY_NAME
TOML_BINARY_PATH="$TOML_BINARY_NAME"
export TOML_BINARY_PATH

install_binary() {
	TARBALL_FILE="${TOML_BINARY_NAME}-${TOML_CLI_VERSION}-x86_64-linux.tar.gz"
	readonly TARBALL_FILE
	DOWNLOAD_URL="https://github.com/gnprice/toml-cli/releases/download/v${TOML_CLI_VERSION}/${TOML_BINARY_NAME}-${TOML_CLI_VERSION}-x86_64-linux.tar.gz"
	readonly DOWNLOAD_URL
	INSTALL_DIR="/opt/${TOML_BINARY_NAME}-${TOML_CLI_VERSION}"

	command_to_check="/opt/${TOML_BINARY_NAME}-${TOML_CLI_VERSION}/${TOML_BINARY_NAME}"
	if command -v "$command_to_check" >/dev/null 2>&1; then
		printf "toml-cli is installed ✅ \n"
	else
		printf "installing toml-cli \n"
		curl -sL -o "$TARBALL_FILE" "$DOWNLOAD_URL"
		mkdir -p "$INSTALL_DIR"
		tar -xzf "${TARBALL_FILE}" -C "$INSTALL_DIR" --strip-components=1
		chmod +x "${INSTALL_DIR}/${TOML_BINARY_NAME}"
		rm "${TARBALL_FILE}"
		printf "toml-cli is installed ✅ \n"
	fi
}
