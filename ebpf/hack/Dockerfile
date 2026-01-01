# eBPF Development Environment
# Based on Ubuntu with Rust nightly and all eBPF dependencies

FROM ubuntu:24.04

# Avoid interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN apt-get update && apt-get install -y \
    # Build essentials
    build-essential \
    pkg-config \
    curl \
    git \
    # eBPF/BPF dependencies
    linux-headers-generic \
    libbpf-dev \
    libelf-dev \
    zlib1g-dev \
    # Network tools for DNS testing
    iputils-ping \
    dnsutils \
    net-tools \
    iproute2 \
    # Useful tools
    linux-tools-generic \
    strace \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install nightly toolchain and components for eBPF
RUN rustup install nightly \
    && rustup default nightly \
    && rustup component add rust-src \
    && rustup component add llvm-tools

# Install bpf-linker using Rust's bundled LLVM
RUN cargo install bpf-linker

# Set working directory
WORKDIR /workspace

# Default command
CMD ["/bin/bash"]
