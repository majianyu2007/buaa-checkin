# syntax=docker/dockerfile:1

# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1.85-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/

# Build release binary
RUN cargo build --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/buaa-checkin /app/buaa-checkin
COPY static/ /app/static/

RUN mkdir -p /app/data

ENV PORT=3000
ENV DATA_DIR=/app/data
ENV RUST_LOG=buaa_checkin=info

EXPOSE 3000

CMD ["/app/buaa-checkin"]
