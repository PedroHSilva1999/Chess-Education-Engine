# syntax=docker/dockerfile:1.7
FROM rust:1.97-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY proto ./proto
RUN --mount=type=cache,id=chess-education-cargo,target=/usr/local/cargo/registry \
    --mount=type=cache,id=chess-education-target,target=/app/target \
    cargo build --locked --release -p chess-api \
    && cp /app/target/release/chess-api /tmp/chess-education-engine

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home chess
COPY --from=builder /tmp/chess-education-engine /usr/local/bin/chess-education-engine
USER chess
ENV PORT=8080 \
    GRPC_ADDR=0.0.0.0:50051 \
    RUST_LOG=chess_api=info,tower_http=info
EXPOSE 8080 50051
HEALTHCHECK --interval=15s --timeout=3s --start-period=3s --retries=3 \
    CMD ["sh", "-c", "curl --fail --silent \"http://127.0.0.1:${PORT:-8080}/health\""]
ENTRYPOINT ["/usr/local/bin/chess-education-engine"]
