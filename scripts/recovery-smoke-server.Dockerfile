# Dedicated fixture artifact. The production Dockerfile never enables this feature.
FROM rust:1.96.1-bookworm AS build
WORKDIR /build
ARG AQ_SOURCE_REVISION
ENV AQ_SOURCE_REVISION=${AQ_SOURCE_REVISION}
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY vendor ./vendor
COPY protocol ./protocol
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,sharing=locked,target=/build/target \
    cargo clean --release -p aq-server -p aq-release -p aq-protocol -p aq-worker-protocol \
    && cargo build --locked --release --bin aq-server --features recovery-test-support \
    && mkdir /out && cp target/release/aq-server /out/

FROM debian:bookworm-slim AS server
ARG AQ_SOURCE_REVISION
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 aq && useradd --uid 10001 --gid aq --create-home aq
LABEL org.opencontainers.image.revision="${AQ_SOURCE_REVISION}" \
      io.agent-q.recovery-fixture="true"
COPY --from=build /out/aq-server /usr/local/bin/aq-server
ENV AQ_VAULT_DIR=/vault
USER 10001:10001
HEALTHCHECK --interval=2s --timeout=3s --start-period=5s --retries=30 CMD ["aq-server", "healthcheck"]
ENTRYPOINT ["aq-server"]
CMD ["serve", "--vault", "/vault"]
