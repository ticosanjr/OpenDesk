# syntax=docker/dockerfile:1
# Single OpenDesk sandbox image for `sandbox.mode=container`.
#
# Harness binaries are intentionally installed at image build time so a run
# never depends on host PATH inside the container. Extend this image (or set
# `sandbox.image` to another tag) when a harness needs extra tooling.
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        jq \
        nodejs \
        npm \
        python3 \
        ripgrep \
        bubblewrap \
    && rm -rf /var/lib/apt/lists/*

# OpenCode CLI is the default harness. Pin the version in build pipelines.
RUN npm install -g opencode-ai@latest

RUN useradd --create-home --uid 1000 opendesk
WORKDIR /workspace
USER opendesk

ENTRYPOINT []
