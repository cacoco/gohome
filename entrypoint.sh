#!/bin/bash
set -euo pipefail

readonly BINARY_NAME="gohome"

cleanup() {
    echo ""
    echo -e "\e[38;5;241m$(date --utc +'%Y-%m-%dT%H:%M:%S.%6NZ')\e[0m  \e[32mINFO\e[0m \e[38;5;241m${BINARY_NAME//-/_}::entrypoint.sh\e[0m received termination...shutting down gracefully"
    if [[ -n "$BINARY_PID" ]]; then
        echo -e "\e[38;5;241m$(date --utc +'%Y-%m-%dT%H:%M:%S.%6NZ')\e[0m  \e[32mINFO\e[0m \e[38;5;241m${BINARY_NAME//-/_}::entrypoint.sh\e[0m shutdown grace period: ${SHUTDOWN_GRACE_PERIOD}s"
        kill -SIGINT "$BINARY_PID"
        # give the binary processs time to gracefully exit
        sleep "${SHUTDOWN_GRACE_PERIOD}s"
    fi
    echo -e "\e[38;5;241m$(date --utc +'%Y-%m-%dT%H:%M:%S.%6NZ')\e[0m  \e[32mINFO\e[0m \e[38;5;241m${BINARY_NAME//-/_}::entrypoint.sh\e[0m binary has shutdown down gracefully"
    # optional: perform other cleanup tasks here
    echo -e "\e[38;5;241m$(date --utc +'%Y-%m-%dT%H:%M:%S.%6NZ')\e[0m  \e[32mINFO\e[0m \e[38;5;241m${BINARY_NAME//-/_}::entrypoint.sh\e[0m program has exited"
    exit 0
}

main() {
    RUST_LOG=debug /usr/src/${BINARY_NAME//-/_} "$@" &
    BINARY_PID=$!

    # keep the script running while the binary is active
    wait "$BINARY_PID"
}

SHUTDOWN_GRACE_PERIOD="${SHUTDOWN_GRACE_PERIOD:-5}"
trap cleanup SIGTERM SIGINT
main "$@"
