FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        cc65 \
        make \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
