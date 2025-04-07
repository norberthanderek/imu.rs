#!/bin/bash
set -e

function stop() {
    CONTAINER_NAME_PART="imu-dev"
    CONTAINER_ID=$(docker ps --format '{{.ID}} {{.Names}}' | grep "$CONTAINER_NAME_PART" | awk '{print $1}')

    if [[ -n "$CONTAINER_ID" ]]; then
        echo "Found container with ID '$CONTAINER_ID'. Stopping..."
        docker stop "$CONTAINER_ID"
    else
        echo "No container found matching '${CONTAINER_NAME_PART}'."
    fi
}

stop 

