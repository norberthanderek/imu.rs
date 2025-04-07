#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

function setup_env_if_needed() {
    if [ ! -f "$SCRIPT_DIR/.env" ]; then
        echo "No .env file found. Running setup script..."
        source "$SCRIPT_DIR/setup-env.sh"
    else
        echo ".env file already exists. Skipping setup."
    fi
}

function start_or_attach() {
    setup_env_if_needed

    DOCKER_COMPOSE_PATH="$SCRIPT_DIR/docker-compose.yaml"
    CONTAINER_NAME_PART="imu-dev"
    CONTAINER_ID=$(docker ps --format '{{.ID}} {{.Names}}' | grep "$CONTAINER_NAME_PART" | awk '{print $1}')

    if [[ -n "$CONTAINER_ID" ]]; then
        echo "Found container with ID '$CONTAINER_ID'. Attaching..."
        docker exec -it "$CONTAINER_ID" /bin/bash
    else
        echo "No container found matching '${CONTAINER_NAME_PART}'. Starting it..."
        docker compose -f ${DOCKER_COMPOSE_PATH} up --build --detach
        echo "Waiting for container to start..."

        until CONTAINER_ID=$(docker ps --format '{{.ID}} {{.Names}}' | grep "$CONTAINER_NAME_PART" | awk '{print $1}'); do
            sleep 2
        done

        echo "Attaching to container '$CONTAINER_ID'..."
        docker exec -it "$CONTAINER_ID" /bin/bash
    fi
}

start_or_attach 

