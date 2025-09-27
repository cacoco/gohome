#!/usr/bin/env bash
set -euo pipefail

build=false

# Script to run (and optionally build) the application locally
# To use, run from the repository base:
#
# $ ./bin/local-run.sh --build
#
# This will build (via goreleaser) the application and the Docker container, then
# invoke `docker` to start it.

print_usage() {
    echo "Run (and optionally build) the program"
    echo "USAGE: $0 [--build|--help]"
    echo "Options:
  help  Print this message
  build Optionally build in additon to run"
}

function check_arg {
    local arg=$1
    if [[ $arg == --* ]]; then
        echo "Unrecognized argument: $arg" >&2
        print_usage >&2
        exit 1
    fi
}

main() {
    for arg in "$@"; do
    shift
    case "$arg" in
        "-b" | "--build")  build=true ;;
        "-h" | "--help")   print_usage >&2; exit ;;
        *)          check_arg "$arg"
    esac
    done

    if [[ "$build" = true ]]; then
        # check for goreleaser installed
        command_to_check="goreleaser"
        printf "Checking if goreleaser is installed..."
        if command -v "$command_to_check" > /dev/null 2>&1; then
            printf "goreleaser is installed ✅\n"
        else
            printf "goreleaser is required to run ❌\nPlease install goreleaser: \"brew install --cask goreleaser/tap/goreleaser\"\n" >&2
            exit 1
        fi
        # check if cosign installed
        command_to_check="cosign"
        printf "Checking if cosign is installed..."
        if command -v "$command_to_check" > /dev/null 2>&1; then
            printf "cosign is installed ✅ \n"
        else
            printf "cosign is required to run ❌\nPlease install cosign: \"brew install cosign\"\n" >&2
            exit 1
        fi
        # check if zig installed
        command_to_check="zig"
        printf "Checking if zig is installed..."
        if command -v "$command_to_check" > /dev/null 2>&1; then
            printf "zig is installed ✅ \n"
        else
            printf "zig is required to run ❌\nPlease install zig: \"brew install zig\"\n" >&2
            exit 1
        fi
        # check if syft installed
        command_to_check="syft"
        printf "Checking if syft is installed..."
        if command -v "$command_to_check" > /dev/null 2>&1; then
            printf "syft is installed ✅ \n"
        else
            printf "syft is required to run ❌\nPlease install syft: \"brew install syft\"\n" >&2
            exit 1
        fi
        goreleaser release --snapshot --clean
    fi

    base=$(pwd)
    docker run -v "$base:/home/nonroot" -p 8080:8080 -e "RUST_LOG=trace" ghcr.io/angstromio/gohome:latest --domain="localhost:8080" --host="0.0.0.0:8080"
}

main "$@"
