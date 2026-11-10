#!/bin/bash
# dstack 0.5.12 sources this script in app-compose.sh after systemd loads
# encrypted environment variables. Its exports are used by Docker Compose.
set +x
set -euo pipefail
umask 077

VERIFIER_SERVICE_IMAGE=ghcr.io/dylansius11/constitution-verifier-service@sha256:0e528a8567df25613e219d57f941e74559cb42ad672d987442271225f8d2c171
VERIFIER_TOOL_IMAGE=ghcr.io/dylansius11/constitution-verifier-tool@sha256:4e733a569d80f739bc7939be24817265e1caed4ab1558f449c3ec5f618a7e2f3
: "${DSTACK_DOCKER_USERNAME:?missing pull-only registry username}"
: "${DSTACK_DOCKER_PASSWORD:?missing pull-only registry credential}"
: "${DSTACK_DOCKER_REGISTRY:?missing registry}"
: "${NEAR_RELAY_CREDENTIAL_JSON:?missing relay credential}"

if [[ "$DSTACK_DOCKER_REGISTRY" != ghcr.io ]]; then
    echo 'pre-launch requires ghcr.io' >&2
    exit 1
fi

# The privileged guest bootstrap prepares only host inputs. No helper container
# receives either socket or any credential. The service runs as UID/GID 10001.
for socket in /var/run/docker.sock /var/run/dstack.sock; do
    if [[ ! -S "$socket" ]]; then
        echo "required socket unavailable: $socket" >&2
        exit 1
    fi
done
export DOCKER_GID="$(stat -c '%g' /var/run/docker.sock)"
export DSTACK_GID="$(stat -c '%g' /var/run/dstack.sock)"

mkdir -p /var/lib/constitution/work /run/constitution
chown 10001:10001 /var/lib/constitution/work /run/constitution
chmod 0700 /var/lib/constitution/work /run/constitution
# Use compact JSON in the encrypted secret: systemd's EnvironmentFile does not
# preserve literal newlines. Do not echo or pass the credential in arguments.
printf '%s' "$NEAR_RELAY_CREDENTIAL_JSON" | jq -e 'type == "object"' >/dev/null
printf '%s' "$NEAR_RELAY_CREDENTIAL_JSON" > /run/constitution/relay.json
chown 10001:10001 /run/constitution/relay.json
chmod 0400 /run/constitution/relay.json
unset NEAR_RELAY_CREDENTIAL_JSON

# Keep the pull-only login out of the service and remove Docker's credential
# store even when a pull fails. A failed login or pull aborts the boot.
(
    export DOCKER_CONFIG
    DOCKER_CONFIG="$(mktemp -d /run/constitution-docker.XXXXXX)"
    trap 'rm -rf "$DOCKER_CONFIG"' EXIT
    printf '%s' "$DSTACK_DOCKER_PASSWORD" | docker login ghcr.io --username "$DSTACK_DOCKER_USERNAME" --password-stdin >/dev/null
    docker pull --platform linux/amd64 "$VERIFIER_SERVICE_IMAGE"
    docker pull --platform linux/amd64 "$VERIFIER_TOOL_IMAGE"
)
unset DSTACK_DOCKER_USERNAME DSTACK_DOCKER_PASSWORD DSTACK_DOCKER_REGISTRY

# dstack 0.5.12 prunes images after compose startup. A stopped, unprivileged
# container retains the tool image without running code or mounting sockets.
tool_id="$(docker image inspect --format '{{.Id}}' "$VERIFIER_TOOL_IMAGE")"
if docker container inspect constitution-tool-image-pin >/dev/null 2>&1; then
    pin="$(docker container inspect --format '{{.Image}} {{.State.Running}}' constitution-tool-image-pin)"
    if [[ "$pin" != "$tool_id false" ]]; then
        echo 'tool image pin conflicts with the measured tool image' >&2
        exit 1
    fi
else
    docker create --name constitution-tool-image-pin --network none --read-only \
        --cap-drop ALL --security-opt no-new-privileges --user 10001:10001 \
        --entrypoint /bin/true "$VERIFIER_TOOL_IMAGE" >/dev/null
fi
