# Multi-stage Dockerfile for langgraph-server release build.
#
# Build (from project root): docker build -f langgraph-server/Dockerfile -t langgraph-server .
# Run:   docker run -p 8123:8123 -e OPENAI_API_KEY=sk-xxx langgraph-server
#
# To extract binary only:
#   docker create --name tmp langgraph-server && \
#   docker cp tmp:/usr/local/bin/langgraph-server . && \
#   docker rm tmp

# -----------------------------------------------------------------------------
# Stage 1: Build
# -----------------------------------------------------------------------------
FROM rust:1-bookworm AS builder

WORKDIR /build

# Copy workspace (langgraph-server depends on langgraph; workspace has other members)
COPY . .

RUN cargo build --release -p langgraph-server

# -----------------------------------------------------------------------------
# Stage 2: Runtime
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/langgraph-server /usr/local/bin/langgraph-server

ENV LISTEN=0.0.0.0:8123
EXPOSE 8123

ENTRYPOINT ["langgraph-server"]
