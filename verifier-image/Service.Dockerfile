ARG TOOLS_IMAGE=constitution-verifier-spike:local
FROM sourcescan/cargo-near@sha256:c9655eaf7436b90c87ea8b25739f91063032c5c30ba24cfeafe72c142943b5bb AS build
USER root
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ crates/
COPY contracts/src/demo/ contracts/src/demo/
COPY rules/ rules/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    CARGO_BUILD_JOBS=1 cargo build --locked --release -p constitution-verifier --bin constitution-verifier \
    && install -m 0755 target/release/constitution-verifier /constitution-verifier

FROM ${TOOLS_IMAGE} AS sidecar
USER root
ENV HOME=/root
WORKDIR /sidecar
COPY verifier-image/ai-sidecar/package.json verifier-image/ai-sidecar/pnpm-lock.yaml ./
RUN npm exec --yes --package=pnpm@12.15.0 -- pnpm install --prod --frozen-lockfile --ignore-scripts --store-dir /tmp/pnpm-store \
    && rm -rf /tmp/pnpm-store /root/.npm

FROM docker:28.5.1-cli@sha256:9190b0613792e658a7783cf14b2d5ace5941bb68ede7276922ea36ee457d76ad AS docker-cli
FROM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3t64 libstdc++6 libatomic1 passwd \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 verifier \
    && useradd --uid 10001 --gid 10001 --no-create-home verifier \
    && install -d -o 10001 -g 10001 /work/home
COPY --from=docker-cli /usr/local/bin/docker /usr/local/bin/docker
COPY --from=sidecar /opt/node-v24.22.1-linux-x64/ /opt/node/
COPY --from=build /constitution-verifier /usr/local/bin/constitution-verifier
WORKDIR /opt/constitution
COPY --from=sidecar /sidecar/node_modules/ verifier-image/ai-sidecar/node_modules/
COPY verifier-image/ai-sidecar/server.mjs verifier-image/ai-sidecar/review.mjs verifier-image/ai-sidecar/package.json verifier-image/ai-sidecar/
COPY verifier-image/run-service.mjs verifier-image/run-service.mjs
COPY rules/ rules/
COPY contracts/src/demo/ contracts/src/demo/
ENV PATH="/opt/node/bin:${PATH}" HOME=/work/home
USER 10001:10001
EXPOSE 8787
ENTRYPOINT ["node", "verifier-image/run-service.mjs"]
CMD []
