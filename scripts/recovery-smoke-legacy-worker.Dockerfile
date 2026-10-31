FROM rust:1.96.1-bookworm AS build
WORKDIR /candidate
ARG AQ_SOURCE_REVISION
ARG AQ_WORKER_ARCHIVE_SHA256
COPY vendor/aq-worker-source.tar.gz vendor/worker-candidate.sha256 vendor/worker-candidate.json ./
RUN sha256sum --check worker-candidate.sha256 \
    && test "$AQ_WORKER_ARCHIVE_SHA256" = "$(cut -d ' ' -f 1 worker-candidate.sha256)" \
    && test -n "$AQ_SOURCE_REVISION" \
    && grep -F "\"source_revision\": \"$AQ_SOURCE_REVISION\"" worker-candidate.json \
    && mkdir /build && tar -xzf aq-worker-source.tar.gz -C /build
WORKDIR /build
ENV AQ_SOURCE_REVISION=${AQ_SOURCE_REVISION}
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,id=aq-worker-${AQ_WORKER_ARCHIVE_SHA256},sharing=locked,target=/build/target \
    cargo build --locked --release --bins \
    && mkdir /out \
    && cp target/release/aq-worker target/release/aq-fixture-agent /out/

FROM debian:bookworm-slim AS worker
ARG AQ_SOURCE_REVISION
ARG AQ_VERSION=0.1.0
ARG AQ_STATE_FORMAT=1
ARG AQ_PROTOCOL_MAX=1
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 aq && useradd --uid 10001 --gid aq --create-home --home-dir /home/aq aq \
    && mkdir /config /state && chown 10001:10001 /config /state
LABEL org.opencontainers.image.title="aq-worker" \
      org.opencontainers.image.revision="${AQ_SOURCE_REVISION}" \
      org.opencontainers.image.version="${AQ_VERSION}" \
      io.agent-q.protocol-max="${AQ_PROTOCOL_MAX}" \
      io.agent-q.state-format="${AQ_STATE_FORMAT}"
COPY --from=build /out/aq-worker /out/aq-fixture-agent /usr/local/bin/
COPY vendor/worker-candidate.json /usr/share/agent-q/worker-candidate.json
ENV XDG_CONFIG_HOME=/config
VOLUME /config /state
USER 10001:10001
ENTRYPOINT ["aq-worker"]
CMD ["run"]
