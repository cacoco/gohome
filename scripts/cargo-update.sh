#!/usr/bin/env bash
set -euo pipefail

# shellcheck disable=SC1091
# shellcheck disable=SC1090
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)/install-toml.sh"

CURL_VERSION=$(curl --version | head -n 1)
USER_AGENT="GoHome-DependencyChecker/1.0 ($CURL_VERSION)"

# Normalize "1.71" -> "1.71.0" so sort -V compares correctly
normalize_version() {
	local v="$1"
	local dots
	dots=$(printf '%s' "$v" | tr -cd '.' | wc -c | tr -d ' ')
	if [ "$dots" -eq 1 ]; then
		printf '%s.0' "$v"
	else
		printf '%s' "$v"
	fi
}

# Returns 0 (true) if $1 <= $2 using semantic version ordering
version_lte() {
	[ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n1)" = "$1" ]
}

main() {
	local STABLE_ONLY=false
	local UPDATE=false
	local DRY_RUN=false
	local MSRV=""
	local SKIP_DEPS=()
	local POSITIONAL=()

	while [[ $# -gt 0 ]]; do
		case "$1" in
		--stable-only)
			STABLE_ONLY=true
			shift
			;;
		--update)
			UPDATE=true
			shift
			;;
		--dry-run)
			DRY_RUN=true
			shift
			;;
		--rust-version)
			MSRV="$2"
			shift 2
			;;
		--skip)
			IFS=',' read -ra _skip_list <<<"$2"
			SKIP_DEPS+=("${_skip_list[@]}")
			shift 2
			;;
		-h | --help)
			echo "Usage: $(basename "$0") --rust-version <VERSION> [--stable-only] [--update] [--dry-run] [--skip DEP1,DEP2,...] [CARGO_TOML]"
			echo ""
			echo "Options:"
			echo "  --rust-version VERSION Minimum supported Rust version (required)"
			echo "  --stable-only          Skip pre-release versions (alpha, beta, dev, rc)"
			echo "  --update               Apply updates via cargo upgrade"
			echo "  --dry-run              Show what cargo upgrade would do without applying (implies --update)"
			echo "  --skip DEP1,DEP2,...   Comma-separated list of dependencies to skip"
			echo "  -h, --help             Show this help"
			return 0
			;;
		*)
			POSITIONAL+=("$1")
			shift
			;;
		esac
	done

	if [ -z "$MSRV" ]; then
		echo "Error: --rust-version is required" >&2
		return 1
	fi

	if [[ ${CI:-false} == "true" ]]; then
		install_binary
		TOML_BINARY_PATH="/opt/${TOML_BINARY_NAME}-${TOML_CLI_VERSION}/${TOML_BINARY_NAME}"
	fi

	if [ "$DRY_RUN" = true ]; then
		UPDATE=true
	fi

	local CARGO_TOML="${POSITIONAL[0]:-Cargo.toml}"

	if [ ! -f "$CARGO_TOML" ]; then
		echo "Error: $CARGO_TOML not found" >&2
		return 1
	fi

	local REQUIRED_CMDS=("$TOML_BINARY_PATH" jq curl)
	if [ "$UPDATE" = true ]; then
		REQUIRED_CMDS+=(cargo)
	fi

	for cmd in "${REQUIRED_CMDS[@]}"; do
		if ! command -v "$cmd" &>/dev/null; then
			echo "Error: $cmd is required but not found" >&2
			return 1
		fi
	done

	echo "Cargo.toml:       $CARGO_TOML"
	echo "rust-version:     $MSRV"
	echo "stable-only:      $STABLE_ONLY"
	echo "update:           $UPDATE"
	echo "dry-run:          $DRY_RUN"
	echo "skip:             ${SKIP_DEPS[*]:-none}"
	echo "---"

	# Dependency sections to inspect (matches [*.dependencies], [*.dev-dependencies], [*.build-dependencies])
	local SECTIONS=(
		"dependencies"
		"dev-dependencies"
		"build-dependencies"
		"workspace.dependencies"
	)

	# Collect external dependencies: name -> current_version, crate_name (for crates.io lookup)
	declare -A DEP_VERSIONS
	declare -A DEP_CRATE_NAMES

	local section json key version crate_name
	for section in "${SECTIONS[@]}"; do
		json=$("$TOML_BINARY_PATH" get "$CARGO_TOML" "$section" 2>/dev/null) || continue

		# Extract dependency key, version, and optional package name.
		# Skip path-only deps and workspace = true refs (no version to compare).
		while IFS='|' read -r key version crate_name; do
			[ -z "$key" ] && continue
			[ "$version" = "null" ] && continue
			DEP_VERSIONS["$key"]="$version"
			if [ "$crate_name" != "null" ] && [ -n "$crate_name" ]; then
				DEP_CRATE_NAMES["$key"]="$crate_name"
			else
				DEP_CRATE_NAMES["$key"]="$key"
			fi
		done < <(echo "$json" | jq -r '
      to_entries[] |
      select(
        ((.value | type) == "string") or
        ((.value | type) == "object" and (.value | has("path") | not) and (.value | has("workspace") | not))
      ) |
      .key + "|" +
      (if (.value | type) == "string" then .value
       elif (.value | type) == "object" then (.value.version // "null")
       else "null" end) + "|" +
      (if (.value | type) == "object" then (.value.package // "null")
       else "null" end)
    ')
	done

	if [ ${#DEP_VERSIONS[@]} -eq 0 ]; then
		echo "No external dependencies found."
		return 0
	fi

	echo "Checking ${#DEP_VERSIONS[@]} dependencies..."
	echo ""

	local MSRV_NORM
	MSRV_NORM=$(normalize_version "$MSRV")
	local updates=0
	local up_to_date=0
	local warnings=0
	local skipped=0

	local current api_url api_response latest_compatible rv rv_norm upgrade_args skip_it
	for key in $(printf '%s\n' "${!DEP_VERSIONS[@]}" | sort); do
		current="${DEP_VERSIONS[$key]}"
		crate_name="${DEP_CRATE_NAMES[$key]}"

		skip_it=false
		if [ ${#SKIP_DEPS[@]} -gt 0 ]; then
			for _s in "${SKIP_DEPS[@]}"; do
				if [ "$_s" = "$key" ]; then
					skip_it=true
					break
				fi
			done
		fi
		if [ "$skip_it" = true ]; then
			printf "SKIP  %-35s %s\n" "$key" "$current"
			skipped=$((skipped + 1))
			continue
		fi

		api_url="https://crates.io/api/v1/crates/${crate_name}"
		api_response=$(curl -sf -H "User-Agent: $USER_AGENT" "$api_url" 2>/dev/null) || {
			echo "WARN  $key ($crate_name): failed to fetch from crates.io"
			warnings=$((warnings + 1))
			sleep 0.2
			continue
		}

		# Get the latest version from the crate metadata
		if [ "$STABLE_ONLY" = true ]; then
			latest_compatible=$(echo "$api_response" | jq -r '.crate.max_stable_version // .crate.default_version // empty')
		else
			latest_compatible=$(echo "$api_response" | jq -r '.crate.max_version // .crate.default_version // empty')
		fi

		# Check MSRV compatibility for the selected version
		if [ -n "$latest_compatible" ]; then
			rv=$(echo "$api_response" | jq -r --arg v "$latest_compatible" \
				'first(.versions[] | select(.num == $v)) | .rust_version')
			if [ "$rv" != "null" ] && [ -n "$rv" ]; then
				rv_norm=$(normalize_version "$rv")
				if ! version_lte "$rv_norm" "$MSRV_NORM"; then
					printf "WARN  %-35s %s -> %s (requires Rust %s, have %s)\n" "$key" "$current" "$latest_compatible" "$rv" "$MSRV"
					warnings=$((warnings + 1))
					sleep 1
					continue
				fi
			fi
		fi

		# Strip build metadata (e.g. "+spec-1.1.0") from the version
		latest_compatible="${latest_compatible%%+*}"

		if [ -z "$latest_compatible" ]; then
			printf "WARN  %-35s %s -> (no compatible version for Rust %s)\n" "$key" "$current" "$MSRV"
			warnings=$((warnings + 1))
		elif [ "$current" = "$latest_compatible" ]; then
			printf "OK    %-35s %s\n" "$key" "$current"
			up_to_date=$((up_to_date + 1))
		else
			printf "UPDATE %-34s %s -> %s\n" "$key" "$current" "$latest_compatible"
			echo "::notice file=${CARGO_TOML}::${key} ${current} -> ${latest_compatible}"
			updates=$((updates + 1))

			if [ "$UPDATE" = true ]; then
				upgrade_args=(-p "${crate_name}@${latest_compatible}" --incompatible allow --pinned allow)
				if [ "$DRY_RUN" = true ]; then
					upgrade_args+=(--dry-run)
				fi
				cargo upgrade "${upgrade_args[@]}" 2>&1 | sed 's/^/       /'
			fi
		fi

		# crates.io rate limit: max 1 req/sec for unauthenticated
		sleep 1
	done

	echo ""
	echo "---"
	echo "Summary: $up_to_date up to date, $updates updates available, $warnings warnings, $skipped skipped"

	if [ "$UPDATE" = true ] && [ "$DRY_RUN" = false ] && [ "$updates" -gt 0 ]; then
		echo ""
		echo "Running cargo update to sync lockfile..."
		cargo update 2>&1 | sed 's/^/       /'
	fi
}

main "$@"
