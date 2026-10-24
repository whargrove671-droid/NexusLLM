# NexusLLM Docker Configuration

## Worker Node Dockerfile
FROM rust:1.78-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY proto ./proto

# Build with CUDA support
RUN apt-get update && apt-get install -y \
    nvidia-cuda-toolkit \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo build --release --bin nexus-agent --features cuda

FROM nvidia/cuda:12.3-runtime-ubuntu22.04

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/nexus-agent /usr/local/bin/nexus-agent

# Create non-root user
RUN useradd -m -u 1000 nexus && \
    mkdir -p /home/nexus/model_cache && \
    chown -R nexus:nexus /home/nexus

USER nexus
WORKDIR /home/nexus

EXPOSE 9090

ENTRYPOINT ["nexus-agent"]
CMD ["--config", "/home/nexus/config.toml"]