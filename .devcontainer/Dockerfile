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
    # Verilator - SystemVerilog simulator (CRITICAL for this project)
    verilator \
    # RISC-V GCC cross-compiler toolchain
    gcc-riscv64-unknown-elf \
    # Additional development tools
    clang \
    llvm \
    pkg-config \
    libssl-dev \
    # Utilities
    vim \
    nano \
    htop \
    && rm -rf /var/lib/apt/lists/*

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
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable

# Add Rust to PATH
ENV PATH="/home/$USERNAME/.cargo/bin:${PATH}"

# Install RISC-V Rust target for cross-compilation
RUN /home/$USERNAME/.cargo/bin/rustup target add riscv32imafc-unknown-none-elf

# Install Rust development tools
RUN /home/$USERNAME/.cargo/bin/cargo install \
    cargo-watch \
    cargo-expand

# Set working directory
WORKDIR /workspace

# Verify installations
RUN echo "=== Installed Tools ===" && \
    verilator --version && \
    rustc --version && \
    cargo --version && \
    riscv64-unknown-elf-gcc --version | head -1 && \
    rustup target list --installed | grep riscv32imafc-unknown-none-elf && \
    echo "=== Setup Complete ==="
