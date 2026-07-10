#!/bin/bash
set -Eeuo pipefail

required=(DEEPCODER_ACCESS_TOKEN DEEPCODER_ALLOWED_ORIGINS DEEPSEEK_API_KEY)
for name in "${required[@]}"; do
    if [[ -z "${!name:-}" ]]; then
        echo "fatal: $name must be configured" >&2
        exit 64
    fi
done

if (( ${#DEEPCODER_ACCESS_TOKEN} < 32 || ${#DEEPCODER_ACCESS_TOKEN} > 256 )); then
    echo "fatal: DEEPCODER_ACCESS_TOKEN must contain 32-256 characters" >&2
    exit 64
fi
if [[ ! "$DEEPCODER_ACCESS_TOKEN" =~ ^[A-Za-z0-9_-]+$ ]]; then
    echo "fatal: DEEPCODER_ACCESS_TOKEN must use only A-Z, a-z, 0-9, - or _" >&2
    exit 64
fi

IFS=',' read -ra allowed_origins <<< "$DEEPCODER_ALLOWED_ORIGINS"
for origin in "${allowed_origins[@]}"; do
    origin="${origin#"${origin%%[![:space:]]*}"}"
    origin="${origin%"${origin##*[![:space:]]}"}"
    if [[ ! "$origin" =~ ^https://[^/]+$ ]] \
        && [[ ! "$origin" =~ ^http://(127\.0\.0\.1|localhost|\[::1\])(:[0-9]+)?$ ]]; then
        echo "fatal: every DEEPCODER_ALLOWED_ORIGINS entry must be an exact https origin or loopback http origin" >&2
        exit 64
    fi
done

if [[ ! "${PORT:-}" =~ ^[0-9]+$ ]] || (( PORT < 1 || PORT > 65535 )); then
    echo "fatal: PORT must be a valid TCP port" >&2
    exit 64
fi

data_dir="${DEEPCODER_DATA_DIR:-/var/data/deepcoder}"
workspace_dir="${DEEPCODER_WORKSPACE_DIR:-/var/data/workspace}"
mkdir -p "$data_dir" "$workspace_dir"
chown -R deepcoder:deepcoder "$data_dir" "$workspace_dir"

app_pid=''
caddy_pid=''
shutdown() {
    trap - TERM INT
    [[ -n "$caddy_pid" ]] && kill -TERM "$caddy_pid" 2>/dev/null || true
    [[ -n "$app_pid" ]] && kill -TERM "$app_pid" 2>/dev/null || true
    [[ -n "$caddy_pid" ]] && wait "$caddy_pid" 2>/dev/null || true
    [[ -n "$app_pid" ]] && wait "$app_pid" 2>/dev/null || true
}
trap shutdown TERM INT

gosu deepcoder bash -c 'cd "$1" && exec /app/deepcoder app-server --ws 127.0.0.1:8081' -- "$workspace_dir" &
app_pid=$!

backend_ready=false
for _ in {1..100}; do
    if ! kill -0 "$app_pid" 2>/dev/null; then
        wait "$app_pid"
        exit $?
    fi
    if (exec 3<>/dev/tcp/127.0.0.1/8081) 2>/dev/null; then
        exec 3>&-
        exec 3<&-
        backend_ready=true
        break
    fi
    sleep 0.1
done
if [[ "$backend_ready" != true ]]; then
    echo "fatal: DeepCoder AppServer did not become ready" >&2
    shutdown
    exit 70
fi

gosu deepcoder caddy run --config /etc/caddy/Caddyfile --adapter caddyfile &
caddy_pid=$!

set +e
wait -n "$app_pid" "$caddy_pid"
status=$?
set -e
shutdown
exit "$status"
