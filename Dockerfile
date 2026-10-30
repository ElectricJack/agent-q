# Release image: only the signed release manifest and its signature bundle.
# Built with a context holding just those two files:
#   docker build --target release -f Dockerfile <dir with both files>
FROM scratch AS release
ARG AQ_SOURCE_REVISION=unknown
ARG AQ_VERSION=unknown
LABEL org.opencontainers.image.title="agent-q-release" \
      org.opencontainers.image.revision="${AQ_SOURCE_REVISION}" \
      org.opencontainers.image.version="${AQ_VERSION}"
COPY manifest.json manifest.sigstore.json /

FROM rust:1.96.1-bookworm AS build
WORKDIR /build
ARG AQ_SOURCE_REVISION=unknown
ENV AQ_SOURCE_REVISION=${AQ_SOURCE_REVISION}
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
# The registry and target directory are BuildKit cache mounts, so rebuilds
# (another version or revision) recompile only the workspace crates. The
# cached target directory is not part of the image: copy the binaries out.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --locked --release --bin aq-server --bin aq-worker \
    && mkdir -p /out \
    && cp target/release/aq-server target/release/aq-worker /out/

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 aq && useradd --uid 10001 --gid aq --create-home --home-dir /home/aq aq

FROM runtime AS server
ARG AQ_SOURCE_REVISION=unknown
ARG AQ_VERSION=unknown
# The release's state format and newest worker protocol. release.yml passes
# them from the source (STATE_FORMAT_VERSION, SUPPORTED_WORKER_PROTOCOLS) and
# checks the pushed labels against the manifest; the updater refuses a
# server image whose state-format label disagrees with its manifest.
ARG AQ_STATE_FORMAT=1
ARG AQ_PROTOCOL_MAX=1
LABEL org.opencontainers.image.title="agent-q-server" \
      org.opencontainers.image.revision="${AQ_SOURCE_REVISION}" \
      org.opencontainers.image.version="${AQ_VERSION}" \
      io.agent-q.protocol-max="${AQ_PROTOCOL_MAX}" \
      io.agent-q.state-format="${AQ_STATE_FORMAT}"
COPY --from=build /out/aq-server /usr/local/bin/aq-server
ENV AQ_VAULT_DIR=/vault
USER 10001:10001
EXPOSE 8080 8081
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=5 CMD ["aq-server", "healthcheck"]
ENTRYPOINT ["aq-server"]
CMD ["serve", "--vault", "/vault"]

FROM runtime AS worker
ARG AQ_SOURCE_REVISION=unknown
ARG AQ_VERSION=unknown
# The same release labels as the server image.
ARG AQ_STATE_FORMAT=1
ARG AQ_PROTOCOL_MAX=1
LABEL org.opencontainers.image.title="agent-q-worker" \
      org.opencontainers.image.revision="${AQ_SOURCE_REVISION}" \
      org.opencontainers.image.version="${AQ_VERSION}" \
      io.agent-q.protocol-max="${AQ_PROTOCOL_MAX}" \
      io.agent-q.state-format="${AQ_STATE_FORMAT}"
COPY --from=build /out/aq-worker /usr/local/bin/aq-worker
# The config directory is created and owned by the runtime user so a named
# volume mounted here inherits that ownership.
RUN mkdir -p /config && chown 10001:10001 /config
ENV XDG_CONFIG_HOME=/config
VOLUME /config
USER 10001:10001
ENTRYPOINT ["aq-worker"]
CMD ["run"]
