# syntax=docker/dockerfile:1

FROM rust:1.100.0-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY crates ./crates

# Remove Windows-specific toolchain target override if present in context
RUN rm -f rust-toolchain.toml \
    && cargo build --release --locked -p swaggen-server

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system swaggen \
    && useradd --system --gid swaggen --home-dir /var/lib/swaggen --create-home swaggen

COPY --from=builder /app/target/release/swaggen-server /usr/local/bin/swaggen-server

ENV SWAGGEN_BIND=0.0.0.0:8080
ENV SWAGGEN_MONGO_DB=swaggen

USER swaggen
WORKDIR /var/lib/swaggen

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/v1/health/ready || exit 1

ENTRYPOINT ["/usr/local/bin/swaggen-server"]
