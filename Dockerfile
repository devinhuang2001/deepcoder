FROM node:22-bookworm-slim AS web-builder

WORKDIR /src/deepcoder-web
COPY deepcoder-web/package.json deepcoder-web/package-lock.json ./
RUN npm ci
COPY deepcoder-web/ ./
RUN VITE_DEEPCODER_REQUIRE_AUTH=true npm run build

FROM rust:1.96-bookworm AS rust-builder

WORKDIR /src/deepcoder
COPY deepcoder/ ./
RUN cargo build --release -p deepcoder-cli

FROM caddy:2-alpine AS caddy-binary

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates gosu \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 deepcoder \
    && useradd --uid 10001 --gid deepcoder --create-home --shell /bin/bash deepcoder \
    && mkdir -p /app /srv/deepcoder /var/data/deepcoder /var/data/workspace \
    && chown -R deepcoder:deepcoder /srv/deepcoder /var/data /home/deepcoder

COPY --from=caddy-binary /usr/bin/caddy /usr/local/bin/caddy
COPY --from=rust-builder /src/deepcoder/target/release/deepcoder /app/deepcoder
COPY --from=web-builder /src/deepcoder-web/dist/ /srv/deepcoder/
COPY deploy/Caddyfile /etc/caddy/Caddyfile
COPY deploy/entrypoint.sh /app/entrypoint.sh

RUN caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile \
    && chmod 0755 /app/deepcoder /app/entrypoint.sh /usr/local/bin/caddy

ENV PORT=10000 \
    DEEPCODER_ENV=production \
    DEEPCODER_DATA_DIR=/var/data/deepcoder \
    DEEPCODER_WORKSPACE_DIR=/var/data/workspace \
    RUST_LOG=info

EXPOSE 10000
STOPSIGNAL SIGTERM
ENTRYPOINT ["/app/entrypoint.sh"]
