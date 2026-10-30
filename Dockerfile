FROM rust:1.96.1-bookworm AS build
WORKDIR /build
ARG AQ_SOURCE_REVISION=unknown
ENV AQ_SOURCE_REVISION=${AQ_SOURCE_REVISION}
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN cargo build --locked --release --bin aq-server --bin aq-worker

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 aq && useradd --uid 10001 --gid aq --create-home --home-dir /home/aq aq

FROM runtime AS server
ARG AQ_SOURCE_REVISION=unknown
LABEL org.opencontainers.image.title="agent-q-server" \
      org.opencontainers.image.revision="${AQ_SOURCE_REVISION}" \
      io.agent-q.protocol-max="1" \
      io.agent-q.state-format="1"
COPY --from=build /build/target/release/aq-server /usr/local/bin/aq-server
USER 10001:10001
EXPOSE 8080 8081
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=5 CMD ["aq-server", "healthcheck"]
ENTRYPOINT ["aq-server"]
CMD ["serve", "--vault", "/vault"]

FROM runtime AS worker
ARG AQ_SOURCE_REVISION=unknown
LABEL org.opencontainers.image.title="agent-q-worker" \
      org.opencontainers.image.revision="${AQ_SOURCE_REVISION}" \
      io.agent-q.protocol-max="1" \
      io.agent-q.state-format="1"
COPY --from=build /build/target/release/aq-worker /usr/local/bin/aq-worker
# The config directory is created and owned by the runtime user so a named
# volume mounted here inherits that ownership.
RUN mkdir -p /config && chown 10001:10001 /config
ENV XDG_CONFIG_HOME=/config
VOLUME /config
USER 10001:10001
ENTRYPOINT ["aq-worker"]
CMD ["run"]
