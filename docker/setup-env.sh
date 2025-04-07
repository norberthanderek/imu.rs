#!/bin/bash
set -e

# Description:
# This script sets up the environment for using devcontainer

function main() {
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    ENV_FILE="$SCRIPT_DIR/.env"

    if [ ! -f "$ENV_FILE" ]; then
        touch "$ENV_FILE"
    fi

    function update_or_append() {
        local VAR_NAME="$1"
        local VAR_VALUE="$2"

        if grep -q "^$VAR_NAME=" "$ENV_FILE"; then
            sed -i ".old" "s/^$VAR_NAME=.*/$VAR_NAME=$VAR_VALUE/" "$ENV_FILE"
        else
            echo "$VAR_NAME=$VAR_VALUE" >> "$ENV_FILE"
        fi
    }

    update_or_append "DOCKER_UID" "$(id -u)"
    update_or_append "DOCKER_GID" "$(id -g)"

    echo -e "\033[0;32mSuccess\033[0m"
}

main

