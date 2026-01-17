# Base image: Ubuntu (aligned with GitHub Actions ubuntu-latest)
FROM ubuntu:22.04

# Prevent interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN apt-get update && apt-get install -y \
    # Core build tools
    build-essential \
    curl \
    git \
    wget \
    autoconf \
    perl \
    python3 \
    # RISC-V GCC cross-compiler toolchain
    gcc-riscv64-unknown-elf \
    # Additional development tools
    clang \
    llvm \
    pkg-config \
    libssl-dev \
    flex \
    bison \
    help2man \
    # Utilities
    locales \
    vim \
    nano \
    htop \
    && locale-gen en_US.UTF-8 && update-locale LANG=en_US.UTF-8 \
    && rm -rf /var/lib/apt/lists/*

# Set locale environment variables
ENV LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8

# Build and install a recent Verilator from source to ensure compatibility with
# marlin / the --lib-create and -j 0 options used by the test harness.
RUN git clone --depth 1 https://github.com/verilator/verilator /tmp/verilator \
    && cd /tmp/verilator \
    && autoconf \
    && ./configure \
    && make -j"$(nproc)" \
    && make install \
    && rm -rf /tmp/verilator

# Create a non-root user for development
ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=$USER_UID

RUN groupadd --gid $USER_GID $USERNAME \
    && useradd --uid $USER_UID --gid $USER_GID -m $USERNAME \
    && apt-get update \
    && apt-get install -y sudo \
    && echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME \
    && rm -rf /var/lib/apt/lists/*

# Switch to non-root user
USER $USERNAME

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable \
    && . $HOME/.cargo/env \
    && rustup target add riscv32imafc-unknown-none-elf \
    && cargo install cargo-watch cargo-expand

# Add Rust to PATH
ENV PATH="/home/$USERNAME/.cargo/bin:${PATH}"

# Set working directory
WORKDIR /workspace

# Verify installations (run as vscode user to check the full environment)
RUN . $HOME/.cargo/env && echo "=== Installed Tools ===" && \
    verilator --version && \
    rustc --version && \
    cargo --version && \
    riscv64-unknown-elf-gcc --version | head -1 && \
    rustup target list --installed | grep riscv32imafc-unknown-none-elf && \
    echo "=== Setup Complete ==="
