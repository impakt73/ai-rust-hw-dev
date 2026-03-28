# .github/runner/Dockerfile
#
# Pre-baked CI/Copilot runner image for the RISC-V hardware verification project.
#
# Base: ubuntu:24.04 — matches the OS version used by GitHub's ubuntu-latest hosted
# runners (GitHub switched ubuntu-latest to 24.04 starting Dec 2024 and completing
# in Jan 2025).
# Keeping the base in sync with the host runner avoids glibc / kernel ABI surprises.
#
# BUILD LOCALLY (for testing; run from the repository root):
#   docker build \
#     --build-arg REPO_URL=https://github.com/<org>/<repo> \
#     -t ghcr.io/<org>/<repo>/runner:test \
#     .github/runner          # context matches build-runner-image.yml
#
# USE IN A JOB (after image is pushed):
#   jobs:
#     test:
#       runs-on: ubuntu-latest
#       container:
#         image: ghcr.io/<org>/<repo>/runner:latest
#         credentials:
#           username: ${{ github.actor }}
#           password: ${{ secrets.GITHUB_TOKEN }}
#
# See docs/research/custom-runner-image.md for the full migration guide and
# GitHub plan/runner requirements.

FROM ubuntu:24.04

# Pass the full repository URL at build time so the image is correctly attributed
# in GHCR.  The build-runner-image.yml workflow sets this automatically via
# docker/metadata-action.  For local builds, override with:
#   --build-arg REPO_URL=https://github.com/<org>/<repo>
ARG REPO_URL=https://github.com/impakt73/ai-rust-hw-dev
LABEL org.opencontainers.image.source="${REPO_URL}"
LABEL org.opencontainers.image.description="Pre-baked CI/Copilot runner: Verilator, FPGA tools, RISC-V GCC, Rust stable"

ENV DEBIAN_FRONTEND=noninteractive

# ---------------------------------------------------------------------------
# System packages
# Unified list from ci.yml + copilot-setup-steps.yml + devcontainer.
# NOTE: Verilator is installed from apt to match current ci.yml behaviour.
#       See docs/research/custom-runner-image.md for the devcontainer discrepancy
#       (devcontainer builds Verilator from source for marlin compatibility).
#       If the apt package ever lacks required marlin options such as
#       `--lib-create` or `-j 0`, switch this image to the devcontainer's
#       build-from-source recipe before activating it in CI.
# ---------------------------------------------------------------------------
RUN apt-get update && apt-get install -y --no-install-recommends \
    # ---------- Core build tools ----------
    build-essential \
    curl \
    git \
    ca-certificates \
    pkg-config \
    # ---------- RTL simulation ----------
    verilator \
    # ---------- FPGA toolchain ----------
    yosys \
    fpga-trellis \
    fpga-trellis-database \
    nextpnr-ecp5 \
    openfpgaloader \
    # ---------- RISC-V cross-compiler ----------
    # Required by copilot-setup-steps; absent from ci.yml (intentional gap now closed).
    gcc-riscv64-unknown-elf \
    # ---------- Rust system dependencies ----------
    libasound2-dev \
    libudev-dev \
    && rm -rf /var/lib/apt/lists/*

# ---------------------------------------------------------------------------
# Rust toolchain
# Installed system-wide (RUSTUP_HOME + CARGO_HOME under /usr/local) so that
# GitHub Actions container jobs — which run as root — can use cargo/rustc
# without sourcing a per-user .cargo/env file.
# ---------------------------------------------------------------------------
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --no-modify-path --default-toolchain stable \
    && rustup target add riscv32imafc-unknown-none-elf \
    # Smoke-test Rust install
    && rustc --version \
    && cargo --version \
    && rustup target list --installed | grep riscv32imafc-unknown-none-elf

# ---------------------------------------------------------------------------
# Smoke-test all other tooling — fail the image build early if anything is
# missing or broken rather than discovering it mid-CI run.
# ---------------------------------------------------------------------------
RUN verilator --version \
    && yosys --version \
    && nextpnr-ecp5 --version \
    && riscv64-unknown-elf-gcc --version \
    && command -v openFPGALoader
